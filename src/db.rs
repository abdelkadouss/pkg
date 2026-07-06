use std::{collections::HashMap, path::PathBuf};

use miette::IntoDiagnostic;
use rusqlite::{Connection, Row, fallible_iterator::FallibleIterator};

use crate::{bridge::BridgeNewPkgMetadata, pkg::Pkg};

#[derive(Debug)]
pub struct PkgFromDb {
    pub name: String,
    pub path: String,
    pub version: Option<String>,
    pub type_name: String,
    pub just_a_dep: bool,
    pub deps: Option<String>,
    pub linked_paths: Option<String>,
}

impl From<PkgFromDb> for Pkg {
    fn from(value: PkgFromDb) -> Self {
        Self {
            name: value.name,
            path: PathBuf::from(value.path),
            version: value.version,
            type_name: value.type_name,
            just_a_dep: value.just_a_dep,
            deps: value.deps.map(|it| vec![it]).unwrap_or(vec![]),
            linked_paths: value
                .linked_paths
                .map(|it| vec![PathBuf::from(it)])
                .unwrap_or(vec![]),
        }
    }
}

impl<'a> TryFrom<&'a Row<'a>> for PkgFromDb {
    type Error = rusqlite::Error;

    fn try_from(value: &'a Row<'a>) -> Result<Self, rusqlite::Error> {
        Ok(PkgFromDb {
            name: value.get(0)?,
            path: value.get(1)?,
            version: value.get(2)?,
            type_name: value.get(3)?,
            just_a_dep: value.get(4)?,
            deps: value.get(5).unwrap_or_default(),
            linked_paths: value.get(6).unwrap_or_default(),
        })
    }
}

pub struct Db {
    path: PathBuf,
    conn: Connection,
}

mod sql {
    // NOTE: the set_db_config method is not wokring for some resone.
    pub const CONFIG_DB: &str = r#"
        PRAGMA foreign_keys = ON;
    "#;

    pub const MAKE_PKG_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS pkg (
            name TEXT UNIQUE NOT NULL,
            path TEXT UNIQUE NOT NULL,
            version TEXT DEFAULT NULL,
            type_name TEXT NOT NULL,
            just_a_dep BOOLEAN NOT NULL DEFAULT false,
            PRIMARY KEY(name)
        );
    "#;

    pub const MAKE_LINK_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS link (
            pkg TEXT NOT NULL,
            path_in_pkg TEXT NOT NULL,
            PRIMARY KEY(pkg, path_in_pkg)
        );
    "#;

    pub const MAKE_DEP_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS dep (
            pkg TEXT NOT NULL,
            depends_on TEXT NOT NULL,
            PRIMARY KEY(pkg, depends_on),
            CHECK ( pkg != depends_on )
        );
    "#;

    pub const LOAD_ALL_PKGS: &str = r#"
        SELECT p.name, p.path, p.version, p.type_name, p.just_a_dep, d.depends_on as deps, l.path_in_pkg as linked_paths
        FROM pkg p
        LEFT JOIN link l
        ON p.name = l.pkg
        LEFT JOIN dep d
        ON p.name = d.pkg;
    "#;

    pub const ADD_PKG: &str = "INSERT INTO pkg (name, path, version, type_name, just_a_dep) VALUES ( ?1, ?2, ?3, ?4, ?5 )";
    pub const LINK_PKG: &str = "INSERT INTO link (pkg, path_in_pkg) VALUES ( ?1, ?2 )";
    pub const DEPAND_ON_PKG: &str = "INSERT INTO dep (pkg, depends_on) VALUES ( ?1, ?2 )";
}

impl Db {
    pub fn new(path: PathBuf) -> miette::Result<Self> {
        let conn = Connection::open(&path).into_diagnostic()?;

        conn.execute(sql::CONFIG_DB, []).into_diagnostic()?;

        conn.execute(sql::MAKE_PKG_TABLE, []).into_diagnostic()?;
        conn.execute(sql::MAKE_LINK_TABLE, []).into_diagnostic()?;
        conn.execute(sql::MAKE_DEP_TABLE, []).into_diagnostic()?;

        Ok(Self { path, conn })
    }

    /// insert new pkg to db.
    pub fn install(&self, pkgs: Vec<Pkg>) -> miette::Result<()> {
        for pkg in pkgs {
            self.conn
                .execute(
                    sql::ADD_PKG,
                    (
                        &pkg.name,
                        &pkg.path.to_string_lossy(),
                        &pkg.version,
                        &pkg.type_name,
                        &pkg.just_a_dep,
                    ),
                )
                .into_diagnostic()?;

            for path in pkg.linked_paths {
                self.conn
                    .execute(sql::LINK_PKG, (&pkg.name, path.to_string_lossy()))
                    .into_diagnostic()?;
            }

            for dep in pkg.deps {
                self.conn
                    .execute(sql::DEPAND_ON_PKG, (&pkg.name, dep))
                    .into_diagnostic()?;
            }
        }

        Ok(())
    }

    /// remove a pkg and all the pkgs depand on from db.
    pub fn remove_and_pkg_depand_on(
        &self,
        pkgs_names: &[&str],
    ) -> miette::Result<Vec</*pkg that depand on pkg to remove*/ String>> {
        todo!()
    }

    /// update pkg info in db.
    pub fn update(
        &self,
        pkgs: HashMap</*name*/ String, BridgeNewPkgMetadata>,
    ) -> miette::Result<()> {
        todo!()
    }

    pub fn load(&self) -> miette::Result<Vec<Pkg>> {
        let mut statm = self.conn.prepare(sql::LOAD_ALL_PKGS).into_diagnostic()?;

        let pkgs: Vec<PkgFromDb> = statm
            .query([])
            .into_diagnostic()?
            .map(|row| row.try_into())
            .collect::<Vec<PkgFromDb>>()
            .into_diagnostic()?;

        Ok(join_matched_pkgs(pkgs))
    }

    /// remove dependency of a pkg on pkg
    fn undepand(pkg_name: &str, deps_names: &[&str]) -> miette::Result<()> {
        todo!()
    }
}

fn join_matched_pkgs(pkgs: Vec<PkgFromDb>) -> Vec<Pkg> {
    let mut out: Vec<Pkg> = vec![];

    for pkg in pkgs {
        if let Some(index) = out
            .iter()
            .enumerate()
            .find(|it| it.1.name == pkg.name)
            .map(|it| it.0)
        {
            if let Some(path) = pkg.linked_paths {
                out[index].linked_paths.push(path.into());
            }

            if let Some(dep) = pkg.deps {
                out[index].deps.push(dep);
            }
        } else {
            out.push(pkg.into());
        }
    }

    out
}

#[cfg(test)]
mod test {
    const MOCK_DATA: [&str; 8] = [
        r#"
        INSERT INTO pkg (name, path, type_name) VALUES ("pkg0", "/path/pkg0", "type");
        "#,
        r#"
        INSERT INTO pkg (name, path, version, type_name) VALUES ("pkg1", "/path/pkg1", "0.0.1", "type");
        "#,
        r#"
        INSERT INTO pkg (name, path, version, type_name) VALUES ("pkg2", "/path/pkg2", "0.0.2", "an_other_type");
        "#,
        r#"
        INSERT INTO link (pkg, path_in_pkg) VALUES ("pkg1", "bin1");
        "#,
        r#"
        INSERT INTO link (pkg, path_in_pkg) VALUES ("pkg1", "bin2");
        "#,
        r#"
        INSERT INTO link (pkg, path_in_pkg) VALUES ("pkg1", "bin3");
        "#,
        r#"
        INSERT INTO dep (pkg, depends_on) VALUES ("pkg0", "pkg1");
        "#,
        r#"
        INSERT INTO dep (pkg, depends_on) VALUES ("pkg0", "pkg2");
        "#,
    ];

    use miette::IntoDiagnostic;
    use tempfile::NamedTempFile;

    use crate::db::{Db, Pkg};

    #[test]
    fn load_pkg_from_db() -> miette::Result<()> {
        let (db, _file) = make_new_db()?;
        inject_mock(&db)?;

        let pkgs = db.load()?;

        assert_eq!(
            pkgs,
            [
                Pkg {
                    name: "pkg0".into(),
                    deps: ["pkg1".into(), "pkg2".into()].into(),
                    version: None,
                    linked_paths: [].into(),
                    path: "/path/pkg0".into(),
                    type_name: "type".into(),
                    just_a_dep: false
                },
                Pkg {
                    name: "pkg1".into(),
                    deps: [].into(),
                    version: Some("0.0.1".into()),
                    linked_paths: ["bin1".into(), "bin2".into(), "bin3".into()].into(),
                    path: "/path/pkg1".into(),
                    type_name: "type".into(),
                    just_a_dep: false
                },
                Pkg {
                    name: "pkg2".into(),
                    deps: [].into(),
                    version: Some("0.0.2".into()),
                    linked_paths: [].into(),
                    path: "/path/pkg2".into(),
                    type_name: "an_other_type".into(),
                    just_a_dep: false
                }
            ]
        );

        Ok(())
    }

    #[test]
    fn add_new_pkg_to_db() -> miette::Result<()> {
        let (db, _file) = make_new_db()?;

        let input: Vec<Pkg> = [Pkg {
            name: "pkg".into(),
            path: "/some/path".into(),
            version: Some("0.0.1".into()),
            type_name: "dir".into(),
            just_a_dep: false,
            deps: vec![],
            linked_paths: vec!["bin".into()],
        }]
        .into();

        db.install(input.clone())?;

        let pkgs = db.load()?;

        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs, input);

        Ok(())
    }

    fn make_new_db() -> miette::Result<(Db, NamedTempFile)> {
        let file = NamedTempFile::new().into_diagnostic()?;
        let db = Db::new(file.path().to_path_buf())?;

        Ok((db, file))
    }

    fn inject_mock(db: &Db) -> miette::Result<()> {
        for query in MOCK_DATA {
            db.conn.execute(query, []).into_diagnostic()?;
        }

        Ok(())
    }
}
