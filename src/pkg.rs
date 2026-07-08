use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::IntoDiagnostic;
use mlua::{ExternalResult, Lua, ObjectLike};
use serde::{Deserialize, Serialize};

use crate::{bridge::BridgeNewPkgMetadata, pkg_type::PkgType, utils::LuaResultExt};

#[derive(Debug, PartialEq, Clone)]
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

pub type Input = Vec<PkgUserDef>;

/// the represent how to pkg defined in the user inputs
#[derive(Default, Debug, PartialEq)]
pub struct PkgUserDef {
    pub name: String,
    pub input: String,
    pub opts: Option<PkgOptionMap>,
    pub deps: Option<Vec<String>>,
    pub version: Option<String>,
    pub just_a_dep: Option<bool>,
    pub os: Option<Os>,
}

#[derive(Debug)]
pub struct PkgMap(pub Vec<PkgUserDef>);

impl mlua::FromLua for PkgMap {
    fn from_lua(value: mlua::Value, _: &Lua) -> mlua::Result<Self> {
        let mlua::Value::Table(table) = value else {
            return Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "table".into(),
                message: Some("expext table".into()),
            });
        };

        let pkgs = table
            .pairs::<String, mlua::Value>()
            .map(|piar| -> Result<PkgUserDef, mlua::Error> {
                let (name, data) = piar?;

                if let mlua::Value::String(input) = data {
                    Ok(PkgUserDef {
                        name,
                        input: input.to_string_lossy(),
                        ..Default::default()
                    })
                } else if let mlua::Value::Table(data) = data {
                    Ok(PkgUserDef {
                        name,
                        input: data.get("input")?,
                        opts: data.get("opts").unwrap_or_default(),
                        deps: data.get("deps").unwrap_or_default(),
                        version: data.get("version").unwrap_or_default(),
                        just_a_dep: data.get("just_a_dep").unwrap_or_default(),
                        os: data.get("os").unwrap_or_default(),
                    })
                } else {
                    Err(mlua::Error::FromLuaConversionError {
                        from: data.type_name(),
                        to: "user difined pkg".into(),
                        message: Some("expext string or table".into()),
                    })
                }
            })
            .collect::<mlua::Result<Vec<PkgUserDef>>>();

        if let Ok(mut pkgs) = pkgs {
            Ok(Self(pkgs))
        } else {
            Err(pkgs.unwrap_err())
        }
    }
}

#[derive(Default, Debug)]
pub struct BridgeMap(pub Vec<(/*bridge name*/ String, PkgMap)>);

impl mlua::FromLua for BridgeMap {
    fn from_lua(value: mlua::Value, lua: &Lua) -> mlua::Result<Self> {
        let mlua::Value::Table(table) = value else {
            return Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "table".into(),
                message: Some("expext table".into()),
            });
        };

        let brdige_map = table
            .pairs::<String, mlua::Table>()
            .map(|piar| {
                let (name, data) = piar?;

                Ok((
                    name,
                    <PkgMap as mlua::FromLua>::from_lua(data.to_value(), lua)?,
                ))
            })
            .collect::<mlua::Result<Vec<(String, PkgMap)>>>();

        if let Ok(brdiges) = brdige_map {
            Ok(Self(brdiges))
        } else {
            Err(brdige_map.unwrap_err())
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub enum Os {
    Kernal(String),
    Name(String),
    Full { kernal: String, name: String },
}

#[derive(Debug, PartialEq)]
pub enum PkgOption {
    String { name: String, value: String },
    Num { name: String, value: f64 },
    Bool { name: String, value: bool },
    Function { name: String, value: mlua::Function },
    Nil { name: String },
}

#[derive(Default, Debug, PartialEq)]
pub struct PkgOptionMap(pub Vec<PkgOption>);

impl mlua::FromLua for PkgOptionMap {
    fn from_lua(value: mlua::Value, lua: &Lua) -> mlua::Result<Self> {
        let mlua::Value::Table(table) = value else {
            return Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "pkg option map".into(),
                message: Some("expect table".into()),
            });
        };

        let opts = table
            .pairs::<String, mlua::Value>()
            .map(|pair| -> mlua::Result<PkgOption> {
                let (name, value) = pair?;
                match value {
                    mlua::Value::Nil => Ok(PkgOption::Nil { name }),
                    mlua::Value::Boolean(value) => Ok(PkgOption::Bool { name, value }),
                    mlua::Value::Integer(value) => Ok(PkgOption::Num {
                        name,
                        value: value as f64,
                    }),
                    mlua::Value::Number(value) => Ok(PkgOption::Num { name, value }),
                    mlua::Value::String(value) => Ok(PkgOption::String {
                        name,
                        value: value.to_string_lossy(),
                    }),
                    mlua::Value::Function(value) => Ok(PkgOption::Function { name, value }),
                    _ => Err(mlua::Error::FromLuaConversionError {
                        from: value.type_name(),
                        to: "pkg option".into(),
                        message: Some(
                            "expext one of [nil, string, integer, number, boolean]".into(),
                        ),
                    }),
                }
            })
            .collect::<mlua::Result<Vec<PkgOption>>>();

        if let Ok(opts) = opts {
            Ok(Self(opts))
        } else {
            Err(opts.unwrap_err())
        }
    }
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

impl PkgUserDef {
    pub fn load(engine: &Lua, input_path: &Path) -> miette::Result<BridgeMap> {
        let mut out: BridgeMap = BridgeMap::default();

        for input in fs::read_dir(input_path).into_diagnostic()? {
            let input = input.into_diagnostic()?;

            if input.path().is_file() {
                let mut brdiges: BridgeMap = engine
                    .load(fs::read_to_string(input.path()).into_diagnostic()?)
                    .eval()
                    .into_report()?;

                out.0.append(&mut brdiges.0);
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
#[test]
fn load_pkg_user_def() -> miette::Result<()> {
    const INPUT_ONE_VALUE: &str = r#"
        return {
            bridge_one = {
                pkg1 = "input",
                pkg2 = {
                    input = "input",
                    opts = {
                        opt1 = true,
                        opt2 = 'something',
                        opt3 = 3.14,
                        opt4 = 0,
                        opt5 = function() print("thank's to Allah") end
                    }
                }
            },
        }
        "#;

    use std::io::Write;

    use tempfile::{NamedTempFile, tempdir};

    let lua = Lua::new();
    let inputs_dir = tempdir().into_diagnostic()?;

    let mut input1 = NamedTempFile::new_in(&inputs_dir).into_diagnostic()?;
    input1.write(INPUT_ONE_VALUE.as_bytes()).into_diagnostic()?;

    let pkgs = PkgUserDef::load(&lua, inputs_dir.path())?.0;

    let (bridge_name, pkgs) = &pkgs[0];
    let pkg1 = &pkgs.0.iter().find(|pkg| pkg.name == "pkg1").unwrap();

    assert_eq!(bridge_name, "bridge_one");
    assert_eq!(pkg1.name, "pkg1");
    assert_eq!(pkg1.input, "input");

    Ok(())
}
