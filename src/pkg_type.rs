use std::{fs, path::PathBuf};

use miette::{IntoDiagnostic, miette};
use mlua::{ExternalResult, FromLua, Lua};

use crate::utils::LuaResultExt;

#[derive(Default, Debug, PartialEq, Clone)]
pub enum PkgLinkOptions {
    Single {
        required: bool,
    },
    Multi {
        required: bool,
    },
    #[default]
    Dont,
}

impl FromLua for PkgLinkOptions {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        if let mlua::Value::Boolean(toggle) = value {
            Ok(if toggle {
                Self::Single { required: true }
            } else {
                Self::Dont
            })
        } else if let mlua::Value::String(opt) = value {
            match opt.to_str()?.to_string().as_str() {
                "can" => Ok(Self::Single { required: false }),
                "can_multi" => Ok(Self::Multi { required: false }),
                "should" => Ok(Self::Single { required: true }),
                "should_multi" => Ok(Self::Multi { required: true }),
                _ => Err("link valid opts is [ can, can_multi, should, should_multi,  false]")
                    .into_lua_err(),
            }
        } else {
            Err(format!(
                "link opts should be a string or bool, found: {:#?}",
                value
            ))
            .into_lua_err()
        }
    }
}

#[derive(Default, Debug, PartialEq, Clone)]
pub enum PkgPathType {
    Dir,
    File,
    #[default]
    Executable,
    ExecutableOrDir,
    FileOrDir,
    Any,
}

#[derive(Default, Clone)]
pub struct PkgHooks {
    after: HooksDef,
    before: HooksDef,
}

#[derive(Default, Clone)]
struct HooksDef {
    isntall: Option<mlua::Function>,
    remove: Option<mlua::Function>,
    update: Option<mlua::Function>,
}

#[derive(Default)]
pub struct PkgTypeLuaDef {
    pub out: Option<PathBuf>,
    pub link: PkgLinkOptions,
    pub version_track: bool,
    pub path_type: PkgPathType,
    pub install_as_assets: bool,
    pub install_paths: Vec<PathBuf>,
    pub hooks: PkgHooks,
}

#[derive(Clone)]
struct PkgType {
    pub name: String,
    pub out: PathBuf,
    pub link: PkgLinkOptions,
    pub version_track: bool,
    pub path_type: PkgPathType,
    pub install_as_assets: bool,
    pub install_paths: Vec<PathBuf>,
    pub hooks: PkgHooks,
}

impl PkgType {
    fn build(name: String, lua_def: PkgTypeLuaDef, default_out: PathBuf) -> Self {
        Self {
            name,
            out: lua_def.out.unwrap_or(default_out),
            link: lua_def.link,
            version_track: lua_def.version_track,
            hooks: lua_def.hooks,
            path_type: lua_def.path_type,
            install_as_assets: lua_def.install_as_assets,
            install_paths: lua_def.install_paths,
        }
    }
}

impl FromLua for PkgTypeLuaDef {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        if let mlua::Value::Table(pkg_type_def) = value {
            Ok(PkgTypeLuaDef {
                out: pkg_type_def.get("out").unwrap_or_default(),
                link: pkg_type_def.get("link").unwrap_or_default(),
                version_track: pkg_type_def.get("version_track").unwrap_or_default(),
                path_type: pkg_type_def.get("path_type").unwrap_or_default(),
                install_as_assets: pkg_type_def.get("install_as_assets").unwrap_or_default(),
                install_paths: pkg_type_def.get("install_paths").unwrap_or(Vec::new()),
                hooks: pkg_type_def.get("hooks").unwrap_or_default(),
            })
        } else {
            Err(format!(
                "pkg type defecation return a wrong value, expected: table, found: {:#?}",
                value
            ))
            .into_lua_err()
        }
    }
}

impl FromLua for PkgPathType {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        if let mlua::Value::String(path_type) = value {
            match path_type.to_str().into_lua_err()?.to_string().as_str() {
                "dir" => Ok( PkgPathType::Dir ),
                "file" => Ok( PkgPathType::File ),
                "executable" => Ok( PkgPathType::Executable ),
                "executable_or_dir" => Ok( PkgPathType::ExecutableOrDir ),
                "file_or_dir" => Ok( PkgPathType::FileOrDir ),
                "any" => Ok( PkgPathType::Any ),
                _ => Err("pkg path type should be one of: [dir, file, executable, executable_or_dir, file_or_dir, any]").into_lua_err()
            }
        } else {
            Err("pkg path type value should be a string!").into_lua_err() // FIXME: improve the err
        }
    }
}

impl FromLua for HooksDef {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        if let mlua::Value::Table(impl_table) = value {
            Ok(Self {
                isntall: impl_table.get("install").unwrap_or_default(),
                remove: impl_table.get("remove").unwrap_or_default(),
                update: impl_table.get("update").unwrap_or_default(),
            })
        } else {
            Err(format!(
                "hooks fiald should have a value of type table, found: {:#?}",
                value
            ))
            .into_lua_err()
        }
    }
}

impl FromLua for PkgHooks {
    fn from_lua(value: mlua::Value, _: &mlua::Lua) -> mlua::Result<Self> {
        if let mlua::Value::Table(hooks_table) = value {
            Ok(Self {
                after: hooks_table.get("after").unwrap_or_default(),
                before: hooks_table.get("before").unwrap_or_default(),
            })
        } else {
            Err(format!(
                "value of the hooks feild should be a table, found: {:#?}",
                value
            ))
            .into_lua_err()
        }
    }
}

impl PkgType {
    fn load(
        engine: &Lua,
        pkg_types_def_path: &PathBuf,
        default_out: PathBuf,
    ) -> miette::Result<Vec<PkgType>> {
        let mut out: Vec<PkgType> = vec![];

        for entry in fs::read_dir(pkg_types_def_path).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;

            if entry.file_name().to_string_lossy().starts_with(".") || entry.path().is_dir() {
                continue;
            }

            let def_path = if entry.path().is_symlink() {
                fs::read_link(entry.path()).into_diagnostic()?
            } else {
                entry.path().to_path_buf()
            };

            let lua_def = engine
                .load(fs::read_to_string(def_path).into_diagnostic()?)
                .eval::<PkgTypeLuaDef>()
                .into_report()?;
            out.push(PkgType::build(
                entry
                    .file_name()
                    .into_string()
                    .map_err(|e| miette!(format!("{:?}", e)))?,
                lua_def,
                default_out.clone(),
            ));
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const LUA_PKG_TYPE_DEF: &str = r#"
        return {
            out = '~/.local/share/nushell/plugins',
            link = false,
            version_track = true,
            path_type = 'file',       -- opts: file, executable, dir, executable_or_dir, file_or_dir, any
            install_as_assets = false, -- can't be install with other pkgs
            hooks = {
                after = { install = function(info) os.execute("plugins add" .. info.pkg_path) end }
            }
        }
        "#;

    #[test]
    fn load_pkg_type_def() -> miette::Result<()> {
        let lua = Lua::new();

        let pkg_type: PkgTypeLuaDef = lua.load(LUA_PKG_TYPE_DEF).eval().into_report()?;

        assert_eq!(pkg_type.link, PkgLinkOptions::Dont);
        assert!(pkg_type.version_track);
        assert_eq!(pkg_type.path_type, PkgPathType::File);
        assert!(!pkg_type.install_as_assets);

        Ok(())
    }

    #[test]
    fn load_pkg_type() -> miette::Result<()> {
        use tempfile::{NamedTempFile, tempdir};

        let temp_dir = tempdir().into_diagnostic()?; // Keep TempDir alive
        let dir_path = temp_dir.path().to_path_buf();
        let file_path = NamedTempFile::with_prefix_in("not_dot", &dir_path)
            .into_diagnostic()?
            .path()
            .to_path_buf();

        fs::write(&file_path, LUA_PKG_TYPE_DEF).into_diagnostic()?;

        let lua = Lua::new();

        let pkg_type: PkgType =
            PkgType::load(&lua, &dir_path, PathBuf::from("/opt/pkg"))?[0].clone();

        assert_eq!(pkg_type.link, PkgLinkOptions::Dont);
        assert_eq!(
            pkg_type.name,
            file_path.file_name().unwrap().to_string_lossy().to_string()
        );
        assert_eq!(
            pkg_type.out,
            PathBuf::from("~/.local/share/nushell/plugins")
        );

        Ok(())
    }
}
