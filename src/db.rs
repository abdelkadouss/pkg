use std::{fmt::Debug, path::PathBuf};

use miette::{Diagnostic, IntoDiagnostic, Result};
use rusqlite::{Connection, Error as RusqliteError};
use thiserror::Error;

pub type EntryPoint = PathBuf;

#[derive(Debug)]
pub enum PkgType {
    SingleExecutable,
    Directory(EntryPoint),
}

#[derive(Debug)]
pub struct Version {
    // FIXME: use u32 instead of String
    pub first_cell: String,
    pub second_cell: String,
    pub third_cell: String,
}

pub type Verstion = Version;

#[derive(Debug)]
pub struct Pkg {
    pub name: String,
    pub version: Version,
    pub path: PathBuf,
    pub pkg_type: PkgType,
}

#[derive(Debug)]
pub struct Db {
    pub conn: Connection,
    pub path: PathBuf,
}

#[derive(Error, Debug, Diagnostic)]
pub enum DbError {
    #[error(transparent)]
    #[diagnostic(code(db::sqlite_error))]
    SqliteError(#[from] RusqliteError),

    #[error(transparent)]
    #[diagnostic(code(db::io_error))]
    IoError(#[from] std::io::Error),

    #[error("Invalid UTF-8 in package path")]
    #[diagnostic(code(db::invalid_utf8))]
    InvalidPath,
}

mod sql {
    pub const CREATE_PKGS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS packages (
        name TEXT NOT NULL,
        version TEXT NOT NULL,
        path TEXT NOT NULL,
        pkg_type TEXT NOT NULL,
        entry_point TEXT NOT NULL,
        bridge TEXT NOT NULL,
        PRIMARY KEY (name)
    );
    "#; // NOTE: installing a package twice with or without a deficient version are not allowd in this implementing. and this is just my decision
    pub const GET_PKGS: &str = r#"
    SELECT name, version, path, pkg_type, entry_point FROM packages;
    "#;

    pub const GET_PKGS_BY_NAME: &str = r#"
    SELECT name, version, path, pkg_type FROM packages WHERE name = ?;
    "#;

    pub const GET_PKGS_BY_NAMES: &str = r#"
    SELECT name, version, path, pkg_type FROM packages WHERE name IN ({});
    "#;
    pub const INSERT_PKGS: &str = r#"
    INSERT INTO packages (name, version, path, pkg_type, entry_point, bridge)
    VALUES (?, ?, ?, ?, ?, ?);
    "#;
    pub const DELETE_PKGS: &str = r#"
    DELETE FROM packages WHERE name = ?;
    "#;
}

impl Db {
    pub fn new(path: &PathBuf) -> Result<Self> {
        let conn = Connection::open(path).into_diagnostic()?;

        conn.execute(sql::CREATE_PKGS_TABLE, []).into_diagnostic()?;

        Ok(Self {
            conn,
            path: path.clone(),
        })
    }

    // wiil to be clean i don't understand everything here because my code make a lifetime
    // error so ai fix it with this code that has this weird 'a syntax
    pub fn which_pkgs_are_installed<'a>(&'a self, pkgs: &'a [String]) -> Result<Vec<&'a String>> {
        let mut installed_pkgs = Vec::new();
        let mut stmt = self.conn.prepare(sql::GET_PKGS_BY_NAME).into_diagnostic()?;

        for pkg in pkgs {
            // We only care if the query returns any rows, not the actual data
            let exists = stmt.exists([&pkg]).into_diagnostic()?;
            if exists {
                installed_pkgs.push(pkg);
            }
        }

        Ok(installed_pkgs)
    }

    pub fn install_bridge_pkgs(&self, pkgs: &[&Pkg], bridge: &String) -> Result<()> {
        let mut stmt = self.conn.prepare(sql::INSERT_PKGS).into_diagnostic()?;

        for pkg in pkgs {
            let pkg_version = format!(
                "{}.{}.{}",
                pkg.version.first_cell, pkg.version.second_cell, pkg.version.third_cell
            );

            let pkg_type = match &pkg.pkg_type {
                PkgType::SingleExecutable => "SingleExecutable".to_string(),
                PkgType::Directory(_) => "Directory".to_string(),
            };

            let pkg_path = pkg.path.to_str().ok_or(DbError::InvalidPath)?.to_string();

            let entry_point = match &pkg.pkg_type {
                PkgType::SingleExecutable => pkg_path.to_string(), // Convert &str to String
                PkgType::Directory(ep) => ep.to_string_lossy().into_owned(), // Handle path conversion
            };

            stmt.execute([
                &pkg.name,
                &pkg_version,
                &pkg_path,
                &pkg_type,
                &entry_point,
                bridge,
            ])
            .into_diagnostic()?;
        }

        Ok(())
    }

    pub fn remove_pkgs(&self, pkgs_names: &[String]) -> Result<()> {
        let mut stmt = self.conn.prepare(sql::DELETE_PKGS).into_diagnostic()?;

        for pkg_name in pkgs_names {
            stmt.execute([&pkg_name]).into_diagnostic()?;
        }

        Ok(())
    }

    pub fn get_pkgs(&self) -> Result<Vec<Pkg>> {
        let mut stmt = self.conn.prepare(sql::GET_PKGS).into_diagnostic()?;
        let rows = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let version: String = row.get(1)?;
                let path: String = row.get(2)?;
                let pkg_type: String = row.get(3)?;
                let entry_point: String = row.get(4)?;

                // Parse version string into components
                let version_parts: Vec<&str> = version.split('.').collect();
                if version_parts.len() != 3 {
                    return Err(RusqliteError::InvalidQuery);
                }

                // Parse package type
                let pkg_type = match pkg_type.as_str() {
                    "SingleExecutable" => PkgType::SingleExecutable,
                    "Directory" => PkgType::Directory(PathBuf::from(&entry_point)),
                    _ => return Err(RusqliteError::InvalidQuery),
                };

                Ok(Pkg {
                    name,
                    version: Version {
                        first_cell: version_parts[0].to_string(),
                        second_cell: version_parts[1].to_string(),
                        third_cell: version_parts[2].to_string(),
                    },
                    path: PathBuf::from(path),
                    pkg_type,
                })
            })
            .into_diagnostic()?;

        let mut pkgs = Vec::new();
        for pkg in rows {
            pkgs.push(pkg.into_diagnostic()?);
        }

        Ok(pkgs)
    }

    pub fn get_pkgs_by_name(&self, pkg_names: &[String]) -> Result<Vec<Pkg>> {
        if pkg_names.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = pkg_names.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = sql::GET_PKGS_BY_NAMES.replace("{}", &placeholders);

        let mut stmt = self.conn.prepare(&sql).into_diagnostic()?;

        let params: Vec<&str> = pkg_names.iter().map(|s| s.as_str()).collect();

        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                let name: String = row.get(0)?;
                let version: String = row.get(1)?;
                let path: String = row.get(2)?;
                let pkg_type: String = row.get(3)?;

                // Parse version string into components
                let version_parts: Vec<&str> = version.split('.').collect();
                if version_parts.len() != 3 {
                    return Err(RusqliteError::InvalidQuery);
                }

                // Parse package type
                let pkg_type = match pkg_type.as_str() {
                    "SingleExecutable" => PkgType::SingleExecutable,
                    "Directory" => PkgType::Directory(PathBuf::from(&path)),
                    _ => return Err(RusqliteError::InvalidQuery),
                };

                Ok(Pkg {
                    name,
                    version: Version {
                        first_cell: version_parts[0].to_string(),
                        second_cell: version_parts[1].to_string(),
                        third_cell: version_parts[2].to_string(),
                    },
                    path: PathBuf::from(path),
                    pkg_type,
                })
            })
            .into_diagnostic()?;

        let mut pkgs = Vec::new();
        for pkg in rows {
            pkgs.push(pkg.into_diagnostic()?);
        }

        Ok(pkgs)
    }

    pub fn which_pkgs_are_not_installed<'a>(
        &'a self,
        pkgs: &'a [String],
    ) -> Result<Vec<&'a String>> {
        let installed_pkgs = self.get_pkgs()?;
        let mut not_installed_pkgs = Vec::new();

        for pkg in pkgs {
            if !installed_pkgs
                .iter()
                .any(|installed| installed.name == pkg.clone())
            {
                not_installed_pkgs.push(pkg);
            }
        }

        Ok(not_installed_pkgs)
    }
}
