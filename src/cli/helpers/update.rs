use std::os::unix::net::UnixStream;

use mlua::Lua;

use crate::{
    bridge::{BridgeNewPkgMetadata, BridgeOutput, NamedBridge, default_impl},
    pkg::PkgUserDef,
    process::send_msg,
    utils::LuaResultExt,
};

pub fn update(
    bridge: &NamedBridge,
    pkgs: &Vec<PkgUserDef>,
    lua: &Lua,
    socket: &mut UnixStream,
) -> miette::Result<()> {
    for pkg in pkgs {
        let pkgs_with_new_data = if let Some(method) = &bridge.bridge.update {
            method
                .call::<Option<Vec<BridgeNewPkgMetadata>>>(())
                .into_report()?
        } else {
            default_impl::update(lua, &pkg.name)?
        };

        send_msg(socket, BridgeOutput::Update(pkgs_with_new_data))?;
    }

    Ok(())
}
