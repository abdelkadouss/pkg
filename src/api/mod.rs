use crate::{config::Config, db::Db};
use mlua::Lua;

pub struct Api {
    pub db: Db,
    pub config: Config,
    pub lua: Lua,
}

fn load_plugins() {
    todo!()
}

pub fn inject_plugins() {}
