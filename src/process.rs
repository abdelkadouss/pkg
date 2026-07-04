const MESSAGE_SEPARATOR: &'static [u8; 1] = b"\n";

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
};

use miette::IntoDiagnostic;
use serde::{Deserialize, Serialize};

use crate::pkg::PkgDef;

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub enum ChildBridgeMessage {
    Done(PkgDef),
    HightPrivApiReq(HightPrivApiCmd),
    ReturnErr(String), // NOTE: u may wanna add `level: u8`, look at the error
                       // function in lua.
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub enum HightPrivApiCmd {
    Sh(/*argv*/ Vec<String>),
    // todo
}

pub fn send_msg<T: Serialize>(stream: &mut UnixStream, data: T) -> miette::Result<()> {
    let json = serde_json::to_string(&data).into_diagnostic()?;
    stream.write_all(json.as_bytes()).into_diagnostic()?;
    stream.write_all(MESSAGE_SEPARATOR).into_diagnostic()?;
    Ok(())
}

pub fn reserve_msg<T: for<'a> Deserialize<'a>>(
    reader: &mut BufReader<UnixStream>,
) -> miette::Result<Option<T>> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).into_diagnostic()?;
    if n == 0 {
        Ok(None)
    } else {
        Ok(Some(serde_json::from_str(&line).into_diagnostic()?))
    }
}

#[cfg(test)]
#[test]
fn communicate_between_two_process() -> miette::Result<()> {
    use nix::unistd::{ForkResult, fork};

    let (parent_sock, child_sock) = UnixStream::pair().into_diagnostic()?;

    match unsafe { fork() }.into_diagnostic()? {
        ForkResult::Child => {
            use std::{thread::sleep, time::Duration};

            drop(parent_sock);
            let mut writer = child_sock;
            sleep(Duration::from_secs(1)); // even after while.

            writer
                .write_all(
                    serde_json::to_string(&ChildBridgeMessage::ReturnErr("error".to_string()))
                        .into_diagnostic()?
                        .as_bytes(),
                )
                .into_diagnostic()?;
        }
        ForkResult::Parent { .. } => {
            drop(child_sock);
            let mut reader = BufReader::new(parent_sock);

            let msg: Option<ChildBridgeMessage> = reserve_msg(&mut reader)?;
            assert_eq!(
                Some(ChildBridgeMessage::ReturnErr("error".to_string())),
                msg
            );
        }
    }

    Ok(())
}
