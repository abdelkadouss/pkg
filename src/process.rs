const MESSAGE_SEPARATOR: &'static [u8; 1] = b"\n";

use std::{io::Write, os::unix::net::UnixStream};

use miette::IntoDiagnostic;
use serde::{Deserialize, Serialize};

use crate::pkg::PkgDef;

#[derive(Deserialize, Serialize)]
pub enum ChildBridgeMessage {
    Done(PkgDef),
    HightPrivApiReq(HightPrivApiCmd),
    ReturnErr(String), // NOTE: u may wanna add level: u8, look at the error
                       // function in lua.
}

#[derive(Deserialize, Serialize)]
pub enum HightPrivApiCmd {
    Sh(/*argv*/ Vec<String>),
    // todo
}

fn send_msg<T: Serialize>(stream: &mut UnixStream, data: T) -> miette::Result<()> {
    let json = serde_json::to_string(&data).into_diagnostic()?;
    stream.write_all(json.as_bytes()).into_diagnostic()?;
    stream.write_all(MESSAGE_SEPARATOR).into_diagnostic()?;
    Ok(())
}
