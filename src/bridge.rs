use mlua::ExternalResult;

#[derive(Default, Debug)]
struct BridgeFeatures {
    pkg_type: Vec<String>,
    opts: Vec<String>,
    specify_version: bool,
    hooks: BridgeHook,
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
struct Bridge {
    features: BridgeFeatures,
    just_a_dep: bool,
    install: mlua::Function,
    update: Option<mlua::Function>,
    remove: Option<mlua::Function>,
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
                install: bridge_def.get::<mlua::Function>("install")?,
                update: bridge_def
                    .get::<mlua::Function>("update")
                    .map(Some)
                    .unwrap_or(None),
                remove: bridge_def.get("remove").map(Some).unwrap_or(None),
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
