use std::os::unix::net::UnixStream;

use crate::{
    bridge::{BridgeNewPkgMetadata, BridgeOutput, NamedBridge},
    pkg::{PkgOptionMap, PkgUserDef},
    process::{ChildBridgeMessage, send_msg},
    utils::LuaResultExt,
};

pub fn install(
    bridge: &NamedBridge,
    pkgs: &Vec<PkgUserDef>,
    socket: &mut UnixStream,
) -> miette::Result<()> {
    for pkg in pkgs {
        let new_pkgs = bridge
            .bridge
            .install
            .call::<Vec<BridgeNewPkgMetadata>>((
                pkg.input.clone(),
                pkg.version.clone(),
                pkg.opts.clone().unwrap_or(PkgOptionMap(vec![])),
            ))
            .into_report()?;

        send_msg(
            socket,
            ChildBridgeMessage::Done {
                user_input: pkg.to_owned(),
                output: BridgeOutput::New(new_pkgs),
            },
        )?;
    }

    Ok(())
}
