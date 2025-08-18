use kdl::{KdlDocument, KdlError};
use miette::{Diagnostic, IntoDiagnostic, Result, SourceSpan};
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug)]
pub struct Config {
    pub path: PathBuf,
    pub source_dir: PathBuf,
    pub bridges_set: PathBuf,
    pub target_dir: PathBuf,
    pub db_path: PathBuf,
    pub load_path: PathBuf,
}

#[derive(Error, Debug, Diagnostic)]
pub enum ConfigError {
    #[error(transparent)]
    #[diagnostic(code(config::io_error))]
    IoError(#[from] std::io::Error),

    #[error("Invalid KDL document")]
    #[diagnostic(code(config::parse_error))]
    KdlError(#[from] KdlError),

    #[error("Invalid value for {0}")]
    #[diagnostic(code(config::wrong_value))]
    WrongValue(&'static str),

    #[error("Missing required field: {0}")]
    #[diagnostic(code(config::missing_value))]
    MissingValue(&'static str),

    #[error("Invalid path format")]
    #[diagnostic(
        code(config::invalid_path),
        help("Paths must be strings and can use ~ for home directory")
    )]
    InvalidPath {
        #[source_code]
        src: String,
        #[label("This path is invalid")]
        bad_span: SourceSpan,
    },
}

impl Config {
    pub fn load(path: PathBuf) -> Result<Self> {
        let config_file = std::fs::read_to_string(&path).into_diagnostic()?;

        let kdl = config_file.parse::<KdlDocument>().into_diagnostic()?;

        let config_node = kdl
            .get("config")
            .ok_or(ConfigError::MissingValue("config"))?;

        let content = config_node
            .children()
            .ok_or(ConfigError::MissingValue("config node is empty"))?;

        // Store the children documents in a HashMap
        let mut config = HashMap::new();

        // Helper function to get and clone children documents
        fn get_and_store_children(
            config: &mut HashMap<String, KdlDocument>,
            parent: &KdlDocument,
            key: &'static str,
        ) -> Result<(), ConfigError> {
            let children = parent
                .get(key)
                .ok_or(ConfigError::MissingValue(key))?
                .children()
                .ok_or(ConfigError::MissingValue(key))?
                .clone(); // Clone to get owned KdlDocument

            config.insert(key.to_string(), children);
            Ok(())
        }

        get_and_store_children(&mut config, content, "inputs")?;
        get_and_store_children(&mut config, content, "output")?;
        get_and_store_children(&mut config, content, "db")?;

        fn expand_home(path: &str) -> PathBuf {
            if let Some(stripped) = path.strip_prefix("~/") {
                if let Some(home_dir) = env::var_os("HOME") {
                    // Linux/MacOS
                    Path::new(&home_dir).join(stripped)
                } else if let Some(home_dir) = env::var_os("USERPROFILE") {
                    // Windows
                    Path::new(&home_dir).join(stripped)
                } else {
                    PathBuf::from(path)
                }
            } else {
                PathBuf::from(path)
            }
        }

        // Helper function to get string values from nodes
        fn get_node_value_as_string(
            parent: &KdlDocument,
            node_name: &'static str,
            src: &str,
        ) -> Result<PathBuf, ConfigError> {
            let node = parent
                .get(node_name)
                .ok_or(ConfigError::MissingValue(node_name))?;

            let value = node
                .entries()
                .first()
                .ok_or(ConfigError::MissingValue(node_name))?
                .value()
                .as_string()
                .ok_or_else(|| ConfigError::InvalidPath {
                    src: src.to_string(),
                    bad_span: node.span(),
                })?
                .to_owned();

            Ok(expand_home(value.as_str()))
        }

        let src = kdl.to_string();

        Ok(Self {
            path,
            source_dir: get_node_value_as_string(config.get("inputs").unwrap(), "path", &src)?,
            bridges_set: get_node_value_as_string(
                config.get("inputs").unwrap(),
                "bridges-set",
                &src,
            )?,
            target_dir: get_node_value_as_string(
                config.get("output").unwrap(),
                "target-dir",
                &src,
            )?,
            load_path: get_node_value_as_string(config.get("output").unwrap(), "load-path", &src)?,
            db_path: get_node_value_as_string(config.get("db").unwrap(), "path", &src)?,
        })
    }
}
