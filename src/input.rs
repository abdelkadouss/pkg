use std::{collections::HashMap, fs, path::PathBuf};

use kdl::{KdlDocument, KdlError, KdlNode};
use miette::{Diagnostic, IntoDiagnostic, Report, Result};
use thiserror::Error;

#[derive(Debug)]
pub enum PkgType {
    SingleExecutable, // so the entry point is the pkg path itself
    Folder(PathBuf),  // the entry point of the package
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PkgDeclaration {
    pub name: String,
    pub input: String,
    pub attributes: HashMap<String, AttributeValue>,
}

#[derive(Debug)]
pub struct Bridge {
    pub name: String,
    pub pkgs: Vec<PkgDeclaration>,
}

#[derive(Debug)]
pub struct Input {
    pub path: PathBuf,
    pub bridges: Vec<Bridge>,
}

#[derive(Error, Debug, Diagnostic)]
pub enum InputError {
    #[error(transparent)]
    #[diagnostic(code(input::io_error))]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    #[diagnostic(code(input::parse_error))]
    KdlError(#[from] KdlError),

    #[error("Unsupported attribute type {0}")]
    #[diagnostic(code(input::wrong_value))]
    UnSupportedAttributeType(String),

    #[error("Missing required field")]
    #[diagnostic(code(input::missing_field))]
    MissingField,

    #[error("Invalid attribute format")]
    #[diagnostic(code(input::invalid_attribute))]
    InvalidAttribute,

    #[error("Duplicate package declaration: {0}")]
    #[diagnostic(code(input::duplicate_pkg))]
    DuplicatePkgDeclaration(String),
}

fn detect_pkg_kdl_files(path: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut inputs_paths = Vec::new();
    for entry in fs::read_dir(path).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && ext.eq_ignore_ascii_case("kdl")
                && let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                && !file_name.starts_with('.')
            {
                inputs_paths.push(path);
            }
        } else if path.is_dir() {
            inputs_paths.extend(detect_pkg_kdl_files(&path)?);
        }
    }
    Ok(inputs_paths)
}

fn parse_inputs_kdl(inputs_paths: &[PathBuf]) -> Result<Vec<KdlDocument>> {
    inputs_paths
        .iter()
        .map(|path| {
            fs::read_to_string(path)
                .into_diagnostic()?
                .parse::<KdlDocument>()
                .into_diagnostic()
        })
        .collect()
}

fn parse_attributes(node: &KdlNode) -> Result<HashMap<String, AttributeValue>, InputError> {
    let mut attributes = HashMap::new();

    for entry in node.entries().iter().skip(1) {
        // Skip first entry which is the input
        let name = entry.name().ok_or(InputError::MissingField)?;
        let value = entry.value();

        let attr_value = if value.is_string() {
            AttributeValue::String(value.as_string().unwrap().to_string())
        } else if value.is_integer() {
            AttributeValue::Integer(value.as_integer().unwrap() as i64)
        } else if value.is_bool() {
            AttributeValue::Boolean(value.as_bool().unwrap())
        } else if value.is_float() {
            AttributeValue::Float(value.as_float().unwrap())
        } else {
            return Err(InputError::UnSupportedAttributeType(value.to_string()));
        };

        attributes.insert(name.to_string(), attr_value);
    }

    Ok(attributes)
}

fn parse_bridges(kdl_docs: &[KdlDocument]) -> Result<Vec<Bridge>> {
    let mut bridges = Vec::<Bridge>::new();

    for doc in kdl_docs {
        for bridge_node in doc.nodes() {
            let bridge_name = bridge_node.name().to_string();
            let mut bridge = Bridge {
                name: bridge_name.clone(),
                pkgs: Vec::new(),
            };

            let children = bridge_node.children().ok_or(InputError::MissingField)?;

            for pkg_decl_node in children.nodes() {
                let input = pkg_decl_node
                    .entries()
                    .first()
                    .map(|entry| {
                        entry
                            .value()
                            .as_string()
                            .ok_or(InputError::InvalidAttribute)
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| Ok(pkg_decl_node.name().to_string()))?;

                let pkg_decl = PkgDeclaration {
                    name: pkg_decl_node.name().to_string(),
                    input,
                    attributes: parse_attributes(pkg_decl_node)?,
                };

                if bridges.iter().any(|b: &Bridge| {
                    b.pkgs
                        .iter()
                        .any(|p: &PkgDeclaration| p.name == pkg_decl.name)
                }) {
                    return Err(Report::new(InputError::DuplicatePkgDeclaration(
                        pkg_decl.name.clone(),
                    )));
                }

                bridge.pkgs.push(pkg_decl);
            }
            if let Some(existing_bridge) = bridges.iter_mut().find(|b| b.name == bridge.name) {
                existing_bridge.pkgs.extend(bridge.pkgs);
            } else {
                bridges.push(bridge);
            }
        }
    }

    Ok(bridges)
}

impl Input {
    pub fn load(path: &PathBuf) -> Result<Self> {
        let inputs_paths = detect_pkg_kdl_files(path)?;
        let kdl_docs = parse_inputs_kdl(&inputs_paths)?;
        let bridges = parse_bridges(&kdl_docs)?;

        Ok(Self {
            path: path.clone(),
            bridges,
        })
    }
}
