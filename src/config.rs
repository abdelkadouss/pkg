#[allow(dead_code)]
const DEFAULT_CONFIG_DIR_PATH: &str = "~/.config/pkg";
#[allow(dead_code)]
const DEFAULT_CONFIG_FILE_NAME: &str = ".config.lua";
#[allow(dead_code)]
const UNIX_DEFAULT_CONFIG_DIR_ENV_VAR_INDICATOR_NAME: &str = "XDG_CONFIG_HOME";

#[allow(unused)]
use crate::utils::ExtandPath;

#[allow(unused)]
use std::env;
use std::{fs, path::PathBuf};

use miette::{IntoDiagnostic, miette};
use mlua::{Lua, LuaSerdeExt};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PathsConfig {
    pub inputs: PathBuf,
    pub link: PathBuf,
    pub bridges: PathBuf,
    pub pkg_types_definition: PathBuf,
    pub default_out: PathBuf,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum CleanMode {
    Soft,
    Strict,
}

#[derive(Serialize, Deserialize)]
pub struct SystemConfig {
    pub database_path: PathBuf,
    pub logs_path: PathBuf,
    pub max_thread_number: u8,
    pub clean_mode: CleanMode,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub paths: PathsConfig,
    pub system: SystemConfig,
}

impl Config {
    pub fn load(engine: &Lua) -> miette::Result<Self> {
        let config_path = Self::path()?;

        match config_path.try_exists() {
            Ok(exists) => {
                if !exists {
                    return Err(miette!("config file not exists!"));
                }
            }
            Err(err) => {
                return Err(miette!("fialed to check if config file exists.\n\t{err}",));
            }
        };

        let mut config: Config = engine
            .from_value(
                engine
                    .load(fs::read_to_string(&config_path).into_diagnostic()?)
                    .eval()
                    .map_err(|e| miette!("lua error when loading config: {e}"))?,
            )
            .map_err(|e| miette!("fiald read lua config: {e}"))?;

        // get the absolute paths
        config.paths.link = config.paths.link.extand_path()?;
        config.paths.default_out = config.paths.default_out.extand_path()?;
        config.paths.inputs = config.paths.inputs.extand_path()?;
        config.paths.bridges = config.paths.bridges.extand_path()?;
        config.paths.pkg_types_definition = config.paths.pkg_types_definition.extand_path()?;

        fs::create_dir_all(&config.paths.link).unwrap();
        fs::create_dir_all(&config.paths.default_out).into_diagnostic()?;
        fs::create_dir_all(&config.system.logs_path).into_diagnostic()?;
        if let Some(parent) = &config.system.database_path.parent() {
            fs::create_dir_all(parent).into_diagnostic()?;
        }

        Ok(config)
    }

    pub fn path() -> miette::Result<PathBuf> {
        #[cfg(not(test))]
        let config_path = PathBuf::from(
            env::var(UNIX_DEFAULT_CONFIG_DIR_ENV_VAR_INDICATOR_NAME)
                .map(|dir_str| {
                    format!(
                        "{}/{}",
                        dir_str,
                        PathBuf::from(DEFAULT_CONFIG_DIR_PATH)
                            .file_name()
                            .unwrap()
                            .to_string_lossy()
                    )
                })
                .unwrap_or(DEFAULT_CONFIG_DIR_PATH.to_string()),
        )
        .join(DEFAULT_CONFIG_FILE_NAME)
        .extand_path()?;

        #[cfg(test)]
        let config_path = test::make_test_config_file()?;

        Ok(config_path)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::{fs, path::PathBuf};

    use miette::{IntoDiagnostic, Result};
    use tempfile::NamedTempFile;

    #[test]
    fn load_config() -> miette::Result<()> {
        let lua = Lua::new();
        let config = Config::load(&lua)?;

        assert_eq!(
            config.paths.inputs,
            PathBuf::from("~/.config/pkg").extand_path()?
        );
        assert_eq!(config.system.max_thread_number, 10);
        assert_eq!(config.system.clean_mode, CleanMode::Soft);

        Ok(())
    }

    pub fn make_test_config_file() -> Result<PathBuf> {
        const LUA_CONFIG_CONTANT: &str = r#"
            return {
                paths = {
                    inputs = '~/.config/pkg',
                    link = '/usr/local/pkg',
                    bridges = '~/.config/pkg/.bridge',
                    pkg_types_definition = '~/.config/pkg/.type',
                    default_out = '/opt/pkg'
                },

                system = {
                    database_path = '/var/db/pkg/packages.db',
                    logs_path = '/var/log/pkg',
                    max_thread_number = 10,
                    clean_mode = 'Soft'
                },
            }
            "#;

        let config_path = NamedTempFile::new().into_diagnostic()?.path().to_path_buf();
        fs::write(&config_path, LUA_CONFIG_CONTANT).into_diagnostic()?;

        Ok(config_path)
    }
}
