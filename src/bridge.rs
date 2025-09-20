use crate::{DEFAULT_LOG_DIR, DEFAULT_WORKING_DIR, db::Db, input::PkgDeclaration};
use miette::{Diagnostic, IntoDiagnostic, Result};
use std::{
    collections::HashMap,
    env,
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    process::{self, Output},
};
use thiserror::Error;

use crate::{Pkg, PkgType, PkgVersion, input};

#[derive(Debug, Clone)]
struct Bridge {
    name: String,
    entry_point: PathBuf,
}

#[derive(Debug)]
pub struct BridgeApi {
    bridges: Vec<Bridge>,
    db: Db,
}

#[derive(Debug)]
pub struct BridgeOutput {
    version: PkgVersion,
    pkg_path: PathBuf,
    pkg_type: PkgType,
}

#[derive(Debug, PartialEq)]
pub enum Operation {
    Install,
    Update,
    Remove,
}

#[derive(Debug)]
pub enum OperationResult {
    Installed(Pkg),
    Updated(Pkg),
    Removed(bool),
}

#[derive(Error, Debug, Diagnostic)]
pub enum BridgeApiError {
    #[error(transparent)]
    #[diagnostic(code(bridge::io_error))]
    IoError(#[from] std::io::Error),

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
            "The bridge set path should be a directory that contains bridges (directories that contains executable scripts)"
        )
    )]
    BridgeSetPathAreNotADirectory(PathBuf),

    #[error("Bridge returned an error: {0}")]
    #[diagnostic(code(bridge::bridge_error))]
    BridgeError(String),

    #[error("Bridge entry point is not executable: {0}")]
    #[diagnostic(
        code(bridge::bridge_entry_point_not_executable),
        help("Try: `chmod +x <entry_point>`")
    )]
    BridgeEntryPointNotExecutable(PathBuf),

    #[error("Bridge returned a wrong output: {0}")]
    #[diagnostic(
        code(bridge::bridge_wrong_output),
        help(
            "Bridge output should be a new line separated list of three elements: pkg_path,pkg_version,pkg_entry_point(if pkg type is 'Directory')"
        )
    )]
    BridgeWrongOutput(String),

    #[error("Bridge failed at runtime, error: {0}")]
    #[diagnostic(code(bridge::bridge_failed))]
    BridgeFailedAtRuntime(String),

    #[error("Bridge returned a wrong version format: {0}")]
    #[diagnostic(
        code(bridge::bridge_wrong_version_format),
        help(
            "Version format should be three integers (can be strings but not recommended) separated by a dot '.'"
        )
    )]
    BridgeWrongVersionFormat(String),

    #[error("Bridge returned not valid path: {0}")]
    #[diagnostic(code(bridge::bridge_wrong_path))]
    BridgeNotValid(PathBuf),

    #[error("Bridge returned not valid entry point: {0}")]
    #[diagnostic(code(bridge::bridge_wrong_entry_point))]
    BridgeNotValidEntryPoint(PathBuf),

    #[error("Failed to create log file: {0}")]
    #[diagnostic(code(bridge::bridge_failed_to_create_log_file))]
    BridgeFailedToCreateLogFile(String),

    #[error("Failed to open log file: {0}")]
    #[diagnostic(code(bridge::bridge_failed_to_open_log_file))]
    BridgeFailedToOpenLogFile(String),
}

mod default_impls {
    use std::path::PathBuf;

    use miette::{IntoDiagnostic, Result};
    pub fn remove() -> Result<bool> {
        let pkg_path = std::env::var("pkg_path").unwrap();
        let mut removed = false;
        if PathBuf::from(&pkg_path).exists() {
            if PathBuf::from(&pkg_path).is_dir() {
                std::fs::remove_dir_all(&pkg_path).into_diagnostic()?;
            } else {
                std::fs::remove_file(&pkg_path).into_diagnostic()?;
            }
            removed = true;
        }
        Ok(removed)
    }
}

// NOTE: unix only
fn is_executable(path: &PathBuf) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = path.metadata().into_diagnostic()?;
    let permissions = metadata.permissions();
    Ok(permissions.mode() & 0o111 != 0) // Check if any execute bit is set
}

impl Operation {
    pub fn display(&self) -> String {
        match self {
            Operation::Install => "install".to_string(),
            Operation::Update => "update".to_string(),
            Operation::Remove => "remove".to_string(),
        }
    }
}

impl BridgeApi {
    pub fn new(
        bridge_set_path: PathBuf,
        needed_bridges: &Vec<String>,
        db_path: &PathBuf,
    ) -> Result<Self> {
        let bridges = Self::load_bridges(&bridge_set_path, needed_bridges)?;

        let db = Db::new(db_path)?;

        Ok(Self { bridges, db })
    }

    pub fn run_operation(
        &self,
        bridge_name: &str,
        pkg: &PkgDeclaration,
        operation: Operation,
    ) -> Result<Option<Pkg>> {
        let bridge_entry_point = &self
            .bridges
            .iter()
            .find(|b| b.name == bridge_name)
            .ok_or(BridgeApiError::BridgeNotFound(bridge_name.to_string()))?
            .entry_point;

        Self::setup_working_directory(bridge_name, &pkg.name)?;

        let input = pkg.input.to_string();
        let attributes = &pkg.attributes;

        let log_file = PathBuf::from(format!("{}/{}.log", &DEFAULT_LOG_DIR, &bridge_name));

        let log_file_parent = log_file.parent().unwrap();
        let _ = std::fs::create_dir_all(log_file_parent)
            .map_err(|err| BridgeApiError::BridgeFailedToCreateLogFile(err.to_string()));
        if !log_file.exists() {
            std::fs::File::create(&log_file)
                .map_err(|err| BridgeApiError::BridgeFailedToCreateLogFile(err.to_string()))?;
        }

        let mut pkg_path = None;

        if (operation == Operation::Update) || (operation == Operation::Remove) {
            pkg_path = self
                .db
                .get_pkgs_by_name(std::slice::from_ref(&pkg.name))?
                .first()
                .map(|p| p.path.clone());
            // NOTE: this is good to do not break if
            // some thing is wrong or db is manually modified, but it's not returned
            // the correct result
        }

        Self::pass_opts_to_env(attributes, pkg_path, &log_file.to_string_lossy())?;

        let mut bridge = process::Command::new(bridge_entry_point);
        bridge.arg(operation.display());
        bridge.arg(input.clone());

        let bridge_output = bridge.output();

        Self::clear_env(&attributes.keys().map(|s| s.to_string()).collect())?;

        let mut log_file_handle = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .map_err(|err| BridgeApiError::BridgeFailedToOpenLogFile(err.to_string()))?;

        // Write stdout to log
        if let Ok(output) = &bridge_output {
            log_file_handle
                .write_all(format!("\n|PKG={}|:::::::\n", &pkg.name).as_bytes())
                .into_diagnostic()?;
            log_file_handle
                .write_all("|STDOUT|::::::::\n".as_bytes())
                .into_diagnostic()?;
            log_file_handle
                .write_all(&output.stdout)
                .into_diagnostic()?;
            log_file_handle.write_all(b"\n").into_diagnostic()?;
            log_file_handle
                .write_all("\n|STDERR|::::::::\n".as_bytes())
                .into_diagnostic()?;
            log_file_handle
                .write_all(&output.stderr)
                .into_diagnostic()?;
            log_file_handle.write_all(b"\n").into_diagnostic()?;
        }

        match bridge_output {
            Ok(output) => {
                // Bridge command succeeded
                match operation {
                    Operation::Install => {
                        let parsed_output = Self::parse_bridge_output(output)?;
                        let pkg = Pkg {
                            name: pkg.name.clone(),
                            version: parsed_output.version,
                            path: parsed_output.pkg_path,
                            pkg_type: parsed_output.pkg_type,
                        };
                        Ok(Some(pkg))
                    }
                    Operation::Update => {
                        let success = output.status.success();
                        let stderr = String::from_utf8(output.stderr.clone()).into_diagnostic()?;
                        let stderr = stderr.trim();

                        let output = if !success
                            && output.status.code().unwrap() == 1
                            && stderr == "__IMPL_DEFAULT"
                        {
                            let _ = default_impls::remove()?;

                            let result = process::Command::new(bridge_entry_point)
                                .arg(Operation::Install.display())
                                .arg(input.clone())
                                .output();

                            result.into_diagnostic()?
                        } else {
                            output
                        };

                        let parsed_output = Self::parse_bridge_output(output)?;
                        let pkg = Pkg {
                            name: pkg.name.clone(),
                            version: parsed_output.version,
                            path: parsed_output.pkg_path,
                            pkg_type: parsed_output.pkg_type,
                        };
                        Ok(Some(pkg))
                    }
                    Operation::Remove => {
                        let success = output.status.success();
                        let stderr = String::from_utf8(output.stderr).into_diagnostic()?;
                        let stderr = stderr.trim();

                        if !success // if it failed
                            && output.status.code().unwrap() == 1 // and return 1
                            && stderr == "__IMPL_DEFAULT"
                        // and print the the
                        // stderr __IMPL_DEFAULT
                        // a log right
                        {
                            default_impls::remove()?;
                        } else {
                            return Err(BridgeApiError::BridgeError(stderr.to_string()).into());
                        }
                        Ok(None)
                    }
                }
            }
            Err(err) => Err(BridgeApiError::BridgeFailedAtRuntime(err.to_string()).into()),
        }
    }

    pub fn install(&self, bridge_name: &str, pkg: &PkgDeclaration) -> Result<Pkg> {
        self.run_operation(bridge_name, pkg, Operation::Install)
            .map(|p| p.unwrap())
    }

    pub fn update(&self, bridge_name: &str, pkg: &PkgDeclaration) -> Result<Pkg> {
        self.run_operation(bridge_name, pkg, Operation::Update)
            .map(|p| p.unwrap())
    }

    pub fn remove(&self, bridge_name: &str, pkg: &PkgDeclaration) -> Result<bool> {
        let res = self.run_operation(bridge_name, pkg, Operation::Remove)?;
        Ok(res.is_none())
    }

    pub fn default_impls_remove(&self, pkg_name: &str) -> Result<bool> {
        let pkg_path = self
            .db
            .get_pkgs_by_name(std::slice::from_ref(&pkg_name.to_string()))?
            .first()
            .expect("Failed to get pkg from db, can't remove it")
            .path
            .clone();
        unsafe {
            std::env::set_var("pkg_path", pkg_path);
        }
        use default_impls::remove;

        remove()
    }

    fn parse_bridge_output(bridge_output: Output) -> Result<BridgeOutput> {
        const BRIDGE_OUTPUT_SEPARATOR: char = ',';
        const VERSION_SEPARATOR: char = '.';

        if !bridge_output.status.success() {
            return Err(BridgeApiError::BridgeError(
                String::from_utf8(bridge_output.stderr)
                    .unwrap_or("failed to parse bridge output".to_string()),
            ))?;
        }

        // to string
        let bridge_output = String::from_utf8(bridge_output.stdout).into_diagnostic()?;

        // get the first line of the bridge output
        let first_line =
            bridge_output
                .lines()
                .next()
                .ok_or(BridgeApiError::IoError(std::io::Error::other(
                    "Wrong bridge output, no thing is returned",
                )))?;

        let first_line = first_line.trim();

        let split = first_line
            .split(BRIDGE_OUTPUT_SEPARATOR)
            .collect::<Vec<&str>>();

        let pkg_path;
        let version;
        let pkg_type;

        if split.len() > 3 || split.len() < 2 {
            return Err(BridgeApiError::BridgeWrongOutput(bridge_output))?;
        } else {
            pkg_path = PathBuf::from(split.first().unwrap().to_string());
            let version_str = split.get(1).unwrap().to_string();
            pkg_type = match split.get(2) {
                Some(entry_point) => PkgType::Directory(PathBuf::from(entry_point)),
                None => PkgType::SingleExecutable,
            };

            let version_split = version_str.split(VERSION_SEPARATOR).collect::<Vec<&str>>();

            if version_split.len() != 3 {
                return Err(BridgeApiError::BridgeWrongVersionFormat(version_str))?;
            } else {
                version = PkgVersion {
                    first_cell: version_split[0].to_string(),
                    second_cell: version_split[1].to_string(),
                    third_cell: version_split[2].to_string(),
                };
            }
        }

        let pwd = std::env::current_dir().into_diagnostic()?;

        let pkg_path = if pkg_path.is_relative() {
            pwd.join(pkg_path)
        } else {
            pkg_path
        };

        let pkg_type = match pkg_type {
            PkgType::Directory(path) => {
                let path = if path.is_relative() {
                    pwd.join(path)
                } else {
                    path
                };
                PkgType::Directory(path)
            }
            _ => pkg_type,
        };

        if !pkg_path.exists() {
            return Err(BridgeApiError::BridgeNotValid(pkg_path))?;
        }

        if let PkgType::Directory(path) = &pkg_type
            && !path.exists()
        {
            return Err(BridgeApiError::BridgeNotValidEntryPoint(path.clone()))?;
        }

        Ok(BridgeOutput {
            version,
            pkg_path,
            pkg_type,
        })
    }

    fn load_bridges(
        bridge_set_path: &PathBuf,
        needed_bridges: &Vec<String>,
    ) -> Result<Vec<Bridge>> {
        const BRIDGE_ENTRY_POINT_NAME: &str = "run";

        if !bridge_set_path.exists() {
            return Err(BridgeApiError::BridgeSetNotFound(bridge_set_path.clone()).into());
        };

        if !bridge_set_path.is_dir() {
            return Err(
                BridgeApiError::BridgeSetPathAreNotADirectory(bridge_set_path.clone()).into(),
            );
        }

        let content = bridge_set_path
            .read_dir()
            .map_err(BridgeApiError::IoError)?;

        let mut bridges = Vec::<Bridge>::new();

        for file in content {
            let file = file.map_err(BridgeApiError::IoError)?;

            if file.file_type().map_err(BridgeApiError::IoError)?.is_dir() {
                let bridge_dir = file.path();
                let bridge_name = bridge_dir
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();

                if !needed_bridges.contains(&bridge_name) {
                    continue;
                }

                let entry_point_path = bridge_dir.join(BRIDGE_ENTRY_POINT_NAME);
                if entry_point_path.exists() && entry_point_path.is_file() {
                    if !is_executable(&entry_point_path)? {
                        Err(BridgeApiError::BridgeEntryPointNotExecutable(
                            entry_point_path.clone(),
                        ))?;
                    }

                    bridges.push(Bridge {
                        name: bridge_name,
                        entry_point: entry_point_path,
                    });
                }
            }
        }

        let missing_bridges = needed_bridges
            .iter()
            .filter(|b| !bridges.iter().any(|bridge| &bridge.name == *b))
            .cloned()
            .collect::<Vec<String>>();

        if !missing_bridges.is_empty() {
            return Err(BridgeApiError::BridgeNotFound(
                missing_bridges.first().unwrap().to_string(),
            )
            .into());
        }

        Ok(bridges)
    }

    fn pass_opts_to_env(
        attributes: &HashMap<String, input::AttributeValue>,
        pkg_path: Option<PathBuf>,
        log_file: &str,
    ) -> Result<(), BridgeApiError> {
        unsafe {
            if let Some(path) = pkg_path {
                if env::var("pkg_path").is_ok() {
                    env::remove_var("pkg_path");
                }
                env::set_var("pkg_path", path);
            }

            if env::var("pkg_log_file").is_ok() {
                env::remove_var("pkg_log_file");
            }
            env::set_var("pkg_log_file", log_file);
        }

        for (key, value) in attributes {
            let value = match value {
                input::AttributeValue::String(value) => value.to_string(),
                input::AttributeValue::Integer(value) => value.to_string(),
                input::AttributeValue::Float(value) => value.to_string(),
                input::AttributeValue::Boolean(value) => value.to_string(),
            };

            if env::var(key).is_ok() {
                unsafe {
                    env::remove_var(key);
                }
            }

            unsafe {
                env::set_var(key, value);
            }
        }

        Ok(())
    }

    fn clear_env(attributes_keys: &Vec<String>) -> Result<()> {
        for key in attributes_keys {
            if env::var(key).is_ok() {
                unsafe {
                    env::remove_var(key);
                }
            }
        }
        Ok(())
    }

    fn setup_working_directory(bridge_name: &str, pkg_name: &str) -> Result<PathBuf> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let tmp_dir_base = PathBuf::from(DEFAULT_WORKING_DIR)
            .join(bridge_name)
            .join(pkg_name);

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
}
