use crate::{config::Config, db::Db};
use miette::{Diagnostic, IntoDiagnostic, Result, SourceSpan};
use mlua::{Lua, Result as LuaResult};
use thiserror::Error;

pub struct PluginApi {
    pub db: Db,
    pub config: Config,
    pub lua: Lua,
}

//TODO: fn load_plugins() {}

impl PluginApi {
    pub fn new(db: Db, config: Config, lua: Lua) -> Self {
        Self { db, config, lua }
    }

    pub fn inject_plugins(&self, lua: &mut Lua) -> Result<()> {
        let default_remove_function = lua
            .create_function(|_, (input, opts): (String, mlua::Table)| {
                let pkg_name = opts.get::<String>("pkg_name").unwrap();
                let pkg_path = self
                    .db
                    .get_pkgs_by_name(&[pkg_name])
                    .unwrap()
                    .first()
                    .unwrap()
                    .path
                    .clone();

                std::fs::remove_dir_all(pkg_path).unwrap();

                Ok(true)
            })
            .into_diagnostic()?;

        lua.globals()
            .set("default_remove_function", default_remove_function)
            .into_diagnostic()?;

        let default_update_function = lua
            .create_function(|_, (input, opts): (String, mlua::Table)| {
                let pkg_name = opts.get::<String>("pkg_name").unwrap();
                let pkg_path = self
                    .db
                    .get_pkgs_by_name(&[pkg_name])
                    .unwrap()
                    .first()
                    .unwrap()
                    .path
                    .clone();

                std::fs::remove_dir_all(pkg_path).unwrap();

                Ok(true)
            })
            .into_diagnostic()?;

        *lua = self.lua.clone();

        Ok(())
    }
}
