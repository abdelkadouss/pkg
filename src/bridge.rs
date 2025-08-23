use crate::{DEFAULT_LOG_DIR, DEFAULT_WORKING_DIR, db::Db, input::PkgDeclaration};
use miette::{Diagnostic, IntoDiagnostic, Result};
use mlua::{Function as LuaFunction, Lua, Result as LuaResult, Table as LuaTable};
use std::{collections::HashMap, io::Write, path::PathBuf};
use thiserror::Error;

use crate::{Pkg, PkgType, PkgVersion, input};

#[derive(Debug, Clone)]
pub struct LuaBridgeImplementation {
    pub install_fn: LuaFunction,
    pub remove_fn: LuaFunction,
    pub update_fn: LuaFunction,
    pub name: String,
}

#[derive(Debug)]
pub struct BridgeApi {
    pub lua: Lua,
    pub needed_bridges: Vec<String>,
    bridges: Vec<LuaBridgeImplementation>,
    db: Db,
}

#[derive(Error, Debug, Diagnostic)]
pub enum BridgeApiError {
    #[error(transparent)]
    #[diagnostic(code(bridge::lua_error))]
    LuaError(#[from] mlua::Error),

    #[error(transparent)]
    #[diagnostic(code(bridge::io_error))]
    IoError(#[from] std::io::Error),

    #[error("Bridge set is missing main.lua")]
    #[diagnostic(code(bridge::missing_main_dot_lua_file))]
    MissingMainDotLuaFile,

    #[error("Bridge is missing required install function")]
    #[diagnostic(code(bridge::missing_install_fn))]
    MissingInstallFunction,

    #[error("Bridge not found: {0}")]
    #[diagnostic(code(bridge::bridge_not_found))]
    BridgeNotFound(String),

    #[error("Bridge set not found: {0}")]
    #[diagnostic(code(bridge::bridge_not_found))]
    BridgeSetNotFound(PathBuf),

    #[error("Bridge set not found: {0}")]
    #[diagnostic(
        code(bridge::bridge_not_found),
        help(
            "The bridge set path sould be a directory that contains bridges (directories that contains lua scripts)"
        )
    )]
    BridgeSetPathAreNotADirectory(PathBuf),

    #[error("Bridge is missing return table: {0}.\nlua error: {1}")]
    #[diagnostic(
        code(bridge::bridge_missing_return_table),
        help(
            "The bridge should return a table with the install function and optionally the remove and update functions"
        )
    )]
    BridgeMissingReturnTable(PathBuf, mlua::Error),

    #[error("Bridge {0} is missing a function: {1}.\nlua error: {2}")]
    #[diagnostic(
        code(bridge::bridge_missing_function),
        help("the install, remove and update functions are required in the bridge return table")
    )]
    BridgeMissingFunction(PathBuf, String, mlua::Error),

    #[error("Unsupported attribute type {0}")]
    #[diagnostic(code(bridge::wrong_value))]
    UnSupportedAttributeType(String),

    #[error("Bridge is missing return a pkg_name")]
    #[diagnostic(code(bridge::missing_pkg_name))]
    MissingPkgName,

    #[error("Bridge is missing return a pkg_version")]
    #[diagnostic(code(bridge::missing_pkg_version))]
    MissingPkgVersion,

    #[error("Bridge is missing return a pkg_path")]
    #[diagnostic(code(bridge::missing_pkg_path))]
    MissingPkgPath,

    #[error("Bridge is missing return a pkg_path")]
    #[diagnostic(code(bridge::db_error))]
    DbError,

    #[error("Bridge returned an error: {0}")]
    #[diagnostic(code(bridge::lua_error))]
    BridgeError(String),

    #[error("Bridge returned an invalid pkg path: {0}")]
    #[diagnostic(code(bridge::bridge_returned_un_valid_pkg_path))]
    BridgeReturnedUnValidPkgPath(PathBuf),

    #[error("Bridge returned an invalid pkg path as entry point: {0}")]
    #[diagnostic(code(bridge::entry_point_not_found))]
    BridgeReturnedUnValidPkgPathAsEntryPoint(PathBuf),
}

fn get_bridges_paths(bridge_set_path: PathBuf) -> Result<Vec<PathBuf>> {
    if !bridge_set_path.exists() {
        return Err(BridgeApiError::BridgeSetNotFound(bridge_set_path).into());
    };

    if !bridge_set_path.is_dir() {
        return Err(BridgeApiError::BridgeSetPathAreNotADirectory(bridge_set_path).into());
    }

    let content = bridge_set_path
        .read_dir()
        .map_err(BridgeApiError::IoError)?;

    let mut bridges_paths = Vec::<PathBuf>::new();

    for file in content {
        let file = file.map_err(BridgeApiError::IoError)?;

        if file.file_type().map_err(BridgeApiError::IoError)?.is_dir() {
            let bridge_dir = file.path();

            let main_lua_path = bridge_dir.join("main.lua");
            if main_lua_path.exists() && main_lua_path.is_file() {
                bridges_paths.push(main_lua_path);
            }
        }
    }

    Ok(bridges_paths)
}

fn setup_working_directory(bridge_name: &str, pkg: &PkgDeclaration) -> Result<PathBuf> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let tmp_dir_base = PathBuf::from(DEFAULT_WORKING_DIR)
        .join(bridge_name)
        .join(&pkg.name);

    let tmp_dir = loop {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let tmp_dir = tmp_dir_base.join(format!("{timestamp}"));

        if !tmp_dir.exists() {
            break tmp_dir;
        }
    };

    // Create the directory
    std::fs::create_dir_all(&tmp_dir).into_diagnostic()?;

    // Change to the directory
    std::env::set_current_dir(&tmp_dir).into_diagnostic()?;

    Ok(tmp_dir)
}

fn get_bridges(
    lua: &Lua,
    bridges_paths: &[PathBuf],
    needed_bridges: &[String],
) -> Result<Vec<LuaBridgeImplementation>> {
    let mut bridges = Vec::<LuaBridgeImplementation>::new();

    for bridge_path in bridges_paths {
        let bridge_name = bridge_path
            .parent()
            .ok_or(BridgeApiError::MissingMainDotLuaFile)? //FIXME: use io error
            .file_stem()
            .ok_or(BridgeApiError::MissingMainDotLuaFile)?
            .to_str()
            .ok_or(BridgeApiError::MissingMainDotLuaFile)?
            .to_string();

        if !needed_bridges.contains(&bridge_name) {
            continue;
        }

        let lua_code = std::fs::read_to_string(bridge_path).map_err(BridgeApiError::IoError)?;

        let bridge_table: LuaTable = lua.load(&lua_code).eval::<LuaTable>().map_err(|lua_err| {
            BridgeApiError::BridgeMissingReturnTable(bridge_path.clone(), lua_err)
        })?;

        let install_fn: LuaFunction = bridge_table.get("install").map_err(|lua_err| {
            BridgeApiError::BridgeMissingFunction(
                bridge_path.clone(),
                "install".to_string(),
                lua_err,
            )
        })?;

        // Create remove function
        let remove = lua
            .create_function(
                |_lua: &Lua, (_input, opts): (String, LuaTable)| -> LuaResult<bool> {
                    let pkg_path = opts.get::<String>("pkg_path")?;
                    let mut removed = false;
                    if PathBuf::from(&pkg_path).exists() {
                        if PathBuf::from(&pkg_path).is_dir() {
                            std::fs::remove_dir_all(&pkg_path)?;
                        } else {
                            std::fs::remove_file(&pkg_path)?;
                        }
                        removed = true;
                    }
                    Ok(removed)
                },
            )
            .into_diagnostic()?;

        let bridge_table_clone = bridge_table.clone();
        let update = lua
            .create_function(
                move |_: &Lua, (input, opts): (String, LuaTable)| -> LuaResult<LuaTable> {
                    // Get the install function from the bridge table each time
                    let install_fn: LuaFunction =
                        bridge_table_clone.get("install").map_err(|e| {
                            mlua::Error::RuntimeError(format!("Missing install function: {}", e))
                        })?;

                    let output = install_fn.call::<LuaTable>((input, opts.clone()))?;

                    if let Ok(error) = output.get::<String>("error") {
                        return Err(mlua::Error::RuntimeError(error));
                    }

                    let pkg_path = opts.get::<String>("pkg_path")?;

                    if PathBuf::from(&pkg_path).exists() {
                        if PathBuf::from(&pkg_path).is_dir() {
                            std::fs::remove_dir_all(&pkg_path)?;
                        } else {
                            std::fs::remove_file(&pkg_path)?;
                        }
                    }

                    Ok(output)
                },
            )
            .into_diagnostic()?;

        let log_file =
            std::path::PathBuf::from(format!("{}/{}.log", &DEFAULT_LOG_DIR, &bridge_name));

        let bridge_name_copy = bridge_name.clone();

        // Try to create the directory, but don't panic if it fails
        let _ = std::fs::create_dir_all(log_file.parent().unwrap());

        let print = lua
            .create_function(move |_: &Lua, input: String| -> LuaResult<()> {
                // Try to create the log file, but if it fails, just print to stderr
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_file)
                {
                    Ok(mut file) => {
                        if let Err(e) = file.write_all(input.as_bytes()) {
                            eprintln!("Failed to write to log file {}: {}", log_file.display(), e);
                        }
                        if let Err(e) = file.write_all(b"\n") {
                            eprintln!("Failed to write newline to log file: {}", e);
                        }
                    }
                    Err(e) => {
                        // Fall back to stderr if we can't create the log file
                        eprintln!("[LOG - {}] {}", &bridge_name_copy, input);
                        eprintln!("(Could not create log file {}: {})", log_file.display(), e);
                    }
                }
                Ok(())
            })
            .into_diagnostic()?;

        lua.globals().set("print", print).into_diagnostic()?;

        let remove_fn: LuaFunction = bridge_table.get("remove").unwrap_or(remove);
        let update_fn: LuaFunction = bridge_table.get("update").unwrap_or(update);

        bridges.push(LuaBridgeImplementation {
            install_fn,
            remove_fn,
            update_fn,
            name: bridge_name,
        });
    }

    Ok(bridges)
}

fn get_bridge(
    bridges: &[LuaBridgeImplementation],
    target: &str,
) -> Result<LuaBridgeImplementation, BridgeApiError> {
    let bridge = bridges
        .iter()
        .find(|bridge| bridge.name == target)
        .ok_or(BridgeApiError::BridgeNotFound(target.to_string()))?
        .clone();

    Ok(bridge)
}

fn parse_attributes(
    bridge_api: &BridgeApi,
    attributes: &HashMap<String, input::AttributeValue>,
    pkg_name: String,
    bridge_name: &str,
) -> Result<LuaTable, BridgeApiError> {
    let table = bridge_api
        .lua
        .create_table()
        .map_err(BridgeApiError::LuaError)?;

    for (key, value) in attributes {
        let _ = match value {
            input::AttributeValue::String(value) => table.set(key.clone(), value.clone()),
            input::AttributeValue::Integer(value) => {
                table.set(key.clone(), mlua::Integer::from(*value))
            }
            input::AttributeValue::Float(value) => {
                table.set(key.clone(), mlua::Number::from(*value))
            }
            input::AttributeValue::Boolean(value) => table.set(key.clone(), *value),
        };
    }

    let pkg_path = bridge_api
        .db
        .get_pkgs_by_name(&[pkg_name])
        .map_err(|_| BridgeApiError::DbError)?
        .first()
        .ok_or(BridgeApiError::MissingPkgPath)?
        .path
        .clone();

    let log_file = format!("{}/{}.log", &DEFAULT_LOG_DIR, &bridge_name);

    let _ = table.set(
        "pkg_path".to_string(),
        pkg_path.to_string_lossy().into_owned(),
    );

    let _ = table.set("log_file".to_string(), log_file.to_string());

    Ok(table)
}

fn convert_lua_table_to_pkg(lua_table: LuaTable, pkg_name: &str) -> Result<Pkg> {
    let pkg_name = pkg_name.to_string().clone();

    let lua_error = lua_table.get::<String>("error");
    if let Ok(error) = lua_error {
        return Err(BridgeApiError::BridgeError(error).into());
    }

    let pkg_version = lua_table
        .get::<String>("version")
        .map_err(|_| BridgeApiError::MissingPkgVersion)?;
    let pkg_path = lua_table
        .get::<String>("path")
        .map_err(|_| BridgeApiError::MissingPkgPath)?;
    let pkg_type = lua_table.get::<String>("entry_point");
    let pkg_type = if let Ok(entry_point) = pkg_type {
        PkgType::Directory(PathBuf::from(entry_point))
    } else {
        PkgType::SingleExecutable
    };

    let pkg_version = pkg_version
        .split('.')
        .map(|s| s.parse::<String>().unwrap())
        .collect::<Vec<String>>();

    let pkg_version = PkgVersion {
        first_cell: pkg_version[0].clone(),
        second_cell: pkg_version[1].clone(),
        third_cell: pkg_version[2].clone(),
    };

    let mut pkg_path = PathBuf::from(pkg_path);

    // get the absolute path of the pkg_path
    if pkg_path.is_relative() {
        pkg_path = std::env::current_dir().into_diagnostic()?.join(pkg_path);
    }

    if !pkg_path.exists() {
        return Err(BridgeApiError::BridgeReturnedUnValidPkgPath(pkg_path)).into_diagnostic()?;
    }

    let pkg_type = if let PkgType::Directory(ref entry_point) = pkg_type {
        // get the absolute path of the entry_point
        let entry_point = if entry_point.is_relative() {
            std::env::current_dir().into_diagnostic()?.join(entry_point)
        } else {
            entry_point.clone()
        };

        if !entry_point.exists() {
            return Err(BridgeApiError::BridgeReturnedUnValidPkgPathAsEntryPoint(
                entry_point.clone(),
            ))
            .into_diagnostic()?;
        }
        PkgType::Directory(entry_point)
    } else {
        pkg_type
    };

    Ok(Pkg {
        name: pkg_name,
        version: pkg_version,
        path: pkg_path,
        pkg_type,
    })
}

impl BridgeApi {
    pub fn new(
        bridge_set_path: PathBuf,
        needed_bridges: Vec<String>,
        db_path: &PathBuf,
    ) -> Result<Self> {
        let bridge_set_dir_content = get_bridges_paths(bridge_set_path.clone())?;
        let lua = Lua::new();

        // FIXME: use an env var to set the lua modules path
        let pkg_lua_module_path = std::env::var("PKG_LUA_MODULE_PATH").unwrap_or("".to_string());

        // Set global package path for all bridges
        let lua_modules_path = &bridge_set_path
            .join(&pkg_lua_module_path)
            .join("lua_modules")
            .join("share")
            .join("lua")
            .join("5.4")
            .join("?.lua");

        let lua_modules_cpath = &bridge_set_path
            .join(&pkg_lua_module_path)
            .join("lua_modules")
            .join("lib")
            .join("lua")
            .join("5.4")
            .join("?.so");

        let package_path = format!(
            "{};{}/?.lua",
            lua_modules_path.to_string_lossy(),
            &bridge_set_path.to_string_lossy()
        );

        let package_cpath = format!(
            "{};{}/?.so",
            lua_modules_cpath.to_string_lossy(),
            &bridge_set_path.to_string_lossy()
        );

        lua.load(format!(
            r#"
            package.path = "{};" .. package.path
            package.cpath = "{};" .. package.cpath
        "#,
            package_path, package_cpath
        ))
        .exec()
        .map_err(BridgeApiError::LuaError)?;

        let bridges = get_bridges(&lua, &bridge_set_dir_content, &needed_bridges)?;

        let db = Db::new(db_path)?;

        Ok(Self {
            lua,
            needed_bridges,
            bridges,
            db,
        })
    }

    pub fn install(&self, bridge_name: &str, pkg: &input::PkgDeclaration) -> Result<Pkg> {
        let bridge = get_bridge(&self.bridges, bridge_name)?;

        setup_working_directory(bridge_name, pkg)?;

        let input = pkg.input.to_string();
        let attributes = &pkg.attributes;
        let table = self.lua.create_table().map_err(BridgeApiError::LuaError)?;

        for (key, value) in attributes {
            let _ = match value {
                input::AttributeValue::String(value) => table.set(key.clone(), value.clone()),
                input::AttributeValue::Integer(value) => {
                    table.set(key.clone(), mlua::Integer::from(*value))
                }
                input::AttributeValue::Float(value) => {
                    table.set(key.clone(), mlua::Number::from(*value))
                }
                input::AttributeValue::Boolean(value) => table.set(key.clone(), *value),
            };
        }

        let log_file = format!("{}/{}.log", &DEFAULT_LOG_DIR, &bridge_name);

        let _ = table.set("log_file".to_string(), log_file.to_string());

        let attributes = table;

        let bridge_output = bridge
            .install_fn
            .call::<LuaTable>((input, attributes))
            .map_err(BridgeApiError::LuaError)?;

        convert_lua_table_to_pkg(bridge_output, &pkg.name)
    }

    pub fn remove(&self, bridge_name: &str, pkg: &input::PkgDeclaration) -> Result<bool> {
        let bridge = get_bridge(&self.bridges, bridge_name)?;

        setup_working_directory(bridge_name, pkg)?;

        let input = pkg.input.to_string();

        let attributes = parse_attributes(self, &pkg.attributes, pkg.name.clone(), bridge_name)?;

        let bridge_output = bridge
            .remove_fn
            .call::<bool>((input, attributes))
            .map_err(BridgeApiError::LuaError)?;

        Ok(bridge_output)
    }

    pub fn update(&self, bridge_name: &str, pkg: &input::PkgDeclaration) -> Result<Pkg> {
        let bridge = get_bridge(&self.bridges, bridge_name)?;

        setup_working_directory(bridge_name, pkg)?;

        let input = pkg.input.to_string();
        let attributes = parse_attributes(self, &pkg.attributes, pkg.name.clone(), bridge_name)?;

        let bridge_output = bridge
            .update_fn
            .call::<LuaTable>((input, attributes))
            .map_err(BridgeApiError::LuaError)?;

        convert_lua_table_to_pkg(bridge_output, &pkg.name)
    }
}
