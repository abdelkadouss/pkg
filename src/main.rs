const UNPRIVILEGED_USER: &str = "nobody";
const UNPRIVILEGED_GROUP: &str = "nogroup";

use miette::{IntoDiagnostic, miette};
use nix::unistd::{ForkResult, Gid, Group, Uid, User, fork, setgid, setuid};
use std::{
    io::{BufReader, BufWriter},
    os::unix::net::UnixStream,
};

use crate::process::{ChildBridgeMessage, reserve_msg};

#[cfg(feature = "api")]
mod api;
mod bridge;
mod config;
mod fs;
mod pkg;
mod pkg_type;
mod process;
mod utils;

fn main() -> miette::Result<()> {
    let (parent_socket, child_socket) = UnixStream::pair().into_diagnostic()?;

    match unsafe { fork() }.into_diagnostic()? {
        ForkResult::Child => {
            drop(parent_socket);

            down_grade_privileges()?;

            if Uid::current().is_root() {
                return Err(miette!(
                    "the bridge runner worker still was root privileges, fiald to down grade."
                ));
            }

            let socket_copy = child_socket.try_clone().into_diagnostic()?;
            let mut _writer = BufWriter::new(child_socket);
            let mut _reader = BufReader::new(socket_copy);

            // TODO: execute brdiges
        }
        ForkResult::Parent { child } => {
            drop(child_socket);

            if !Uid::current().is_root() {
                return Err(miette!("re-run this as root"));
            }

            let socket_copy = parent_socket.try_clone().into_diagnostic()?;
            let mut writer = BufWriter::new(parent_socket);
            let mut reader = BufReader::new(socket_copy);

            serve_child(&mut reader, &mut writer)?;

            nix::sys::wait::waitpid(child, None).into_diagnostic()?;
        }
    };

    Ok(())
}

fn serve_child(
    reader: &mut BufReader<UnixStream>,
    _writer: &mut BufWriter<UnixStream>,
) -> miette::Result<()> {
    loop {
        let msg: Option<ChildBridgeMessage> = reserve_msg(reader)?;

        if msg.is_none() {
            break Ok(());
        }

        match msg.unwrap() {
            ChildBridgeMessage::Done(_pkg_def) => todo!("install/remove/update pkg"),
            ChildBridgeMessage::HightPrivApiReq(_cmd) => {
                todo!("ask user and then execute cmd with hight priv")
            }
            ChildBridgeMessage::ReturnErr(_bridge_error) => todo!("handle error"),
        }
    }
}

fn find_unused_user_id() -> Option<u32> {
    const START: u32 = 1;
    const END: u32 = 200;

    for uid in START..END {
        if let Ok(None) = User::from_uid(uid.into()) {
            return Some(uid);
        }
    }
    None
}

fn find_unused_group_id() -> Option<u32> {
    const START: u32 = 1;
    const END: u32 = 200;

    for uid in START..END {
        if let Ok(None) = Group::from_gid(uid.into()) {
            return Some(uid);
        }
    }
    None
}

fn down_grade_privileges() -> miette::Result<()> {
    let uid: Uid = User::from_name(UNPRIVILEGED_USER)
        .ok()
        .and_then(|user| {
            if let Some(user) = user {
                user.uid.into()
            } else {
                Uid::from_raw(find_unused_user_id()?).into()
            }
        })
        .unwrap();

    let gid: Gid = Group::from_name(UNPRIVILEGED_GROUP)
        .ok()
        .and_then(|group| {
            if let Some(group) = group {
                group.gid.into()
            } else {
                Gid::from_raw(find_unused_group_id()?).into()
            }
        })
        .unwrap();

    setgid(gid).into_diagnostic()?;
    setuid(uid).into_diagnostic()?;

    Ok(())
}

#[cfg(test)]
#[test]
fn try_down_grade_privileges() -> miette::Result<()> {
    if !Uid::current().is_root() {
        return Ok(());
    }

    down_grade_privileges()?;

    assert!(!Uid::current().is_root());

    Ok(())
}
