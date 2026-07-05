use std::path::PathBuf;

use mlua::ExternalResult;
use serde::{Deserialize, Serialize};

use crate::{
    bridge::BridgeNewPkgMetadata,
    pkg_type::{PkgLinkOptions, PkgType},
};

#[derive(Debug, PartialEq)]
pub struct Pkg {
    pub name: String,
    pub path: PathBuf,
    pub version: Option<String>,
    pub type_name: String,
    pub just_a_dep: bool,
    pub deps: Vec<String>,
    pub linked_paths: Vec<PathBuf>,
}

impl From<(PkgUserDef, BridgeNewPkgMetadata, PkgType)> for Pkg {
    fn from(
        value: (
            /*how user define the pkg in inputs*/ PkgUserDef,
            /*the bridge output*/ BridgeNewPkgMetadata,
            /*the user define of the pkg type*/ PkgType,
        ),
    ) -> Self {
        Self {
            name: value.0.name,
            path: value.1.path,
            version: value.1.version,
            type_name: value.1.pkg_type.unwrap_or(value.2.name),
            just_a_dep: value.0.just_a_dep.unwrap_or(value.2.just_a_dep),
            deps: value.0.deps.unwrap_or_default(),
            linked_paths: value.1.link.unwrap_or_default(),
        }
    }
}

/// the represent how to pkg defined in the user inputs
#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct PkgUserDef {
    pub name: String,
    pub input: String,
    pub opts: Option<Vec<PkgOption>>,
    pub deps: Option<Vec<String>>,
    pub version: Option<String>,
    pub just_a_dep: Option<bool>,
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
