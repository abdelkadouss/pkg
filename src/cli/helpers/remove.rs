use std::os::unix::net::UnixStream;

use mlua::Lua;

use crate::{
    bridge::{BridgeOutput, NamedBridge, default_impl},
    pkg::PkgUserDef,
    process::send_msg,
    utils::LuaResultExt,
};

pub fn remove(
    bridge: &NamedBridge,
    pkgs: &Vec<PkgUserDef>,
    lua: &Lua,
    socket: &mut UnixStream,
) -> miette::Result<()> {
    for pkg in pkgs {
        if let Some(method) = &bridge.bridge.remove {
            method.call::<()>(()).into_report()?;
        } else {
            default_impl::remove(lua, &pkg.name)?;
        }

        send_msg(socket, BridgeOutput::Remove(pkg.name.clone()))?;
    }

    Ok(())
}
