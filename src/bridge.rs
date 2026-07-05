const DEFAULT_BRIGE_ENTRY_POINT_FINE: &str = "run.lua";

use std::{fs, path::PathBuf};

use miette::{IntoDiagnostic, miette};
use mlua::{ExternalResult, Lua};
use serde::{Deserialize, Serialize};

use crate::{pkg::Os, utils::LuaResultExt};

#[derive(Default, Debug)]
struct BridgeFeatures {
    pkg_type: Vec<String>,
    opts: Vec<String>,
    specify_version: bool,
    hooks: BridgeHook,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub enum BridgeOutput {
    New(BridgeNewPkgMetadata),
    Remove(/*name*/ String),
    Update(Option<BridgeNewPkgMetadata>),
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct BridgeNewPkgMetadata {
    pub path: PathBuf,
    pub pkg_type: Option<String>,
    pub version: Option<String>,
    pub link: Option<Vec<PathBuf>>,
}

#[derive(Default, Debug, PartialEq)]
struct BridgeHook {
    after: ActionToggles,
    before: ActionToggles,
}

#[derive(Default, Debug, PartialEq)]
struct ActionToggles {
    install: bool,
    remove: bool,
    update: bool,
}

#[derive(Debug)]
pub enum BridgeDep {
    ExecName(String),
    FetchImpl(mlua::Function),
}

#[derive(Debug)]
struct Bridge {
    pub features: BridgeFeatures,
    pub just_a_dep: bool,
    pub os: Option<Os>,
    pub deps: Vec<BridgeDep>,
    pub install: mlua::Function,
    pub update: Option<mlua::Function>,
    pub remove: Option<mlua::Function>,
}

impl mlua::FromLua for BridgeDep {
    fn from_lua(value: mlua::Value, _: &Lua) -> mlua::Result<Self> {
        if let mlua::Value::String(dep) = value {
            Ok(Self::ExecName(dep.to_string_lossy()))
        } else if let mlua::Value::Function(fetch_impl) = value {
            Ok(Self::FetchImpl(fetch_impl))
        } else {
            Err("").into_lua_err()
        }
    }
}

impl mlua::FromLua for BridgeHook {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        if let mlua::Value::Table(hooks) = value {
            Ok(BridgeHook {
                after: hooks
                    .get("after")
                    .map(|after: mlua::Table| ActionToggles {
                        install: after.get("install").unwrap_or_default(),
                        remove: after.get("remove").unwrap_or_default(),
                        update: after.get("update").unwrap_or_default(),
                    })
                    .unwrap_or(ActionToggles::default()),
                before: hooks
                    .get("before")
                    .map(|after: mlua::Table| ActionToggles {
                        install: after.get("install").unwrap_or_default(),
                        remove: after.get("remove").unwrap_or_default(),
                        update: after.get("update").unwrap_or_default(),
                    })
                    .unwrap_or(ActionToggles::default()),
            })
        } else if let mlua::Value::Nil = value {
            Ok(BridgeHook::default())
        } else {
            Err(format!("worng hook value, expext table found {:#?}", value)).into_lua_err()
        }
    }
}

impl mlua::FromLua for Bridge {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        if let mlua::Value::Table(bridge_def) = value {
            Ok(Bridge {
                features: bridge_def.get("featurs_support").unwrap_or_default(),
                just_a_dep: bridge_def.get("just_a_dep").unwrap_or(false),
                os: bridge_def.get("os")?,
                deps: bridge_def.get("deps").unwrap_or(vec![]),
                install: bridge_def.get::<mlua::Function>("install")?,
                update: bridge_def.get("update")?,
                remove: bridge_def.get("remove")?,
            })
        } else {
            Err(format!(
                "bad bridge definition - return wrong value, expext table found: {:#?}",
                value
            ))
            .into_lua_err()
        }
    }
}

impl mlua::FromLua for BridgeFeatures {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        if let mlua::Value::Table(features) = value {
            Ok(BridgeFeatures {
                pkg_type: features.get("pkg_type").unwrap_or(Vec::<String>::new()),
                opts: features.get("opts").unwrap_or(Vec::<String>::new()),
                specify_version: features.get("specify_version").unwrap_or(false),
                hooks: features.get("hooks").unwrap_or_default(),
            })
        } else {
            Err(format!( "bad bridge definition - worng value for featurs_support, expext table found: {:#?}", value)).into_lua_err()
        }
    }
}

pub mod default_impl {
    use mlua::prelude::{Lua, LuaResult};

    pub fn update(lua: &Lua, name: String) -> LuaResult<()> {
        todo!()
    }

    pub fn remove(lua: &Lua, name: String) -> LuaResult<()> {
        todo!()
    }
}

fn load_bridges(engine: &Lua, bridges_path: PathBuf) -> miette::Result<Vec<Bridge>> {
    let mut out = Vec::<Bridge>::new();

    let bridge_dir = fs::read_dir(&bridges_path).into_diagnostic()?;
    for entry in bridge_dir {
        let entry = entry.into_diagnostic()?;

        // skip hiding entrys
        if entry.file_name().to_string_lossy().starts_with(".") || !entry.path().is_dir() {
            continue;
        }

        let entry_path = if entry.path().is_symlink() {
            fs::read_link(entry.path()).into_diagnostic()?
        } else {
            entry.path().to_path_buf()
        };

        let brige_entry_point_path = entry_path.join(DEFAULT_BRIGE_ENTRY_POINT_FINE);

        if !brige_entry_point_path.exists() {
            return Err(miette!(format!(
                "bridge {} don't have an entry point",
                entry
                    .file_name()
                    .to_str()
                    .unwrap_or(bridges_path.to_str().unwrap()) // FIXME: handle this better
            )));
        }

        out.push(
            engine
                .load(fs::read_to_string(entry_path).into_diagnostic()?)
                .eval()
                .into_report()?,
        );
    }

    Ok(out)
}

#[cfg(test)]
#[test]
fn load_bridge() -> miette::Result<()> {
    const LUA_BRIDGE: &str = r#"
        return {
            featurs_support = {
                pkg_type = { 'single_executable' },
                opts = { 'locked' },
                specify_version = true,
                hooks = {
                    after = {
                        install = true,
                        update = true,
                        remove = true
                    },
                    before = { remove = true }
                }
            },
            os = {
                kernal = 'linux',
                name = 'void linux'
            },
            -- just_a_dep = true, -- don't install if nothign depand on
                install = function(input, version, opts)
                -- do some thing ...
                return {
                    {
                        path = 'out/bin',
                        type = 'single_executable',
                        version = result.versoion,
                    },
            }
            end
        }
    "#;

    use mlua::Lua;

    use crate::utils::LuaResultExt;

    let bridge: Bridge = Lua::new().load(LUA_BRIDGE).eval().into_report()?;

    assert!(!bridge.just_a_dep);
    assert_eq!(bridge.remove, None);
    assert_eq!(bridge.update, None);
    assert_eq!(
        bridge.os,
        Some(Os::Full {
            kernal: "linux".to_string(),
            name: "void linux".to_string()
        })
    );
    assert_eq!(bridge.features.opts, ["locked"]);
    assert!(bridge.features.specify_version);
    assert_eq!(bridge.features.pkg_type, ["single_executable"]);
    assert_eq!(
        bridge.features.hooks,
        BridgeHook {
            after: ActionToggles {
                install: true,
                remove: true,
                update: true
            },
            before: ActionToggles {
                install: false,
                remove: true,
                update: false
            }
        }
    );

    Ok(())
}
