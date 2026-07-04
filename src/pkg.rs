use std::path::PathBuf;

use mlua::ExternalResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct PkgDef {
    pub input: String,
    pub opts: Option<Vec<PkgOption>>,
    pub deps: Option<Vec<String>>,
    pub version: Option<String>,
    pub os: Option<Os>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub enum Os {
    Kernal(String),
    Name(String),
    Full { kernal: String, name: String },
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub enum PkgOption {
    String { name: String, value: String },
    Int { name: String, value: i32 },
    Bool { name: String, value: bool },
    // Function { name: String, value: mlua::Function },
    Nil { name: String },
}

impl mlua::FromLua for Os {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        if let mlua::Value::String(kernal) = value {
            Ok(Self::Kernal(kernal.to_string_lossy()))
        } else if let mlua::Value::Table(tab) = value {
            let kernal: Option<String> = tab.get("kernal")?;
            let name: Option<String> = tab.get("name")?;

            match (kernal, name) {
                (Some(k), None) => Ok(Self::Kernal(k)),
                (None, Some(n)) => Ok(Self::Name(n)),
                (Some(k), Some(n)) => Ok(Self::Full { kernal: k, name: n }),
                (None, None) => Err("should set ether 'kernal' or 'name' in the os table or pass the kernal name as string").into_lua_err(),
            }
        } else {
            Err("the value of field os sould be a string or a table").into_lua_err()
        }
    }
}
