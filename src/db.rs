use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
};

use miette::{IntoDiagnostic, miette};
use rusqlite::{Connection, Row, fallible_iterator::FallibleIterator, params};

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

    pub const LOAD_PKG: &str = r#"
        SELECT p.name, p.path, p.version, p.type_name, p.just_a_dep, d.depends_on as deps, l.path_in_pkg as linked_paths
        FROM pkg p
        LEFT JOIN link l
        ON p.name = l.pkg
        LEFT JOIN dep d
        ON p.name = d.pkg
        WHERE name = ?1;
    "#;

    pub const GET_PKG_NAME: &str = "SELECT name FROM pkg WHERE name = ?1";

    pub const ADD_PKG: &str = "INSERT INTO pkg (name, path, version, type_name, just_a_dep) VALUES ( ?1, ?2, ?3, ?4, ?5 )";

    pub const LINK_PKG: &str = "INSERT INTO link (pkg, path_in_pkg) VALUES ( ?1, ?2 )";

    pub const UNLINK_PKG: &str = "DELETE FROM link WHERE pkg = ?1 AND path_in_pkg = ?2";

    pub const REMOVE_FROM_LINK_TABLE: &str = "DELETE FROM link WHERE pkg = ?1";

    pub const REMOVE_PKG: &str = "DELETE FROM pkg WHERE name = ?1";

    pub const DEPAND_ON_PKG: &str = "INSERT INTO dep (pkg, depends_on) VALUES ( ?1, ?2 )";

    pub const UNDEPAND: &str = "DELETE FROM dep WHERE pkg = ?1 AND depends_on = ?2";

    pub const UPDATE_JUST_A_DEP_VALUE_FOR_PKG: &str =
        "UPDATE pkg SET just_a_dep = ?2 WHERE name = ?1";

    pub const UPDATE_PKG_INFO: &str =
        "UPDATE pkg SET version = ?2, just_a_dep = ?3, path = ?4, type_name = ?5 WHERE name = ?1";
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
        pkg_name: &str,
    ) -> miette::Result<Vec</*pkg that depand on pkg to remove*/ String>> {
        // TDOO: remove the pkg that to remove pkg depends_on and marked as just_a_dep
        let dep_stuck = self.follow_dependency(pkg_name)?;

        let mut unlink_statm = self
            .conn
            .prepare_cached(sql::REMOVE_FROM_LINK_TABLE)
            .into_diagnostic()?;

        let mut remove_pkg_statm = self
            .conn
            .prepare_cached(sql::REMOVE_PKG)
            .into_diagnostic()?;

        for dep in dep_stuck.iter() {
            unlink_statm.execute(params![dep]).into_diagnostic()?;

            remove_pkg_statm.execute(params![dep]).into_diagnostic()?;
        }

        self.conn
            .execute(sql::REMOVE_PKG, params![pkg_name])
            .into_diagnostic()?;

        Ok(dep_stuck)
    }

    pub fn follow_dependency(&self, pkg_name: &str) -> miette::Result<Vec<String>> {
        let pkg = self.load(pkg_name)?;

        let mut deps_stuck: Vec<String> = vec![];

        let mut follow_path: VecDeque<String> = VecDeque::from([pkg.name.clone()]);

        // FIXME: make this const
        let mut statm = self
            .conn
            .prepare("SELECT pkg FROM dep WHERE depends_on = ?1")
            .into_diagnostic()?;

        while let Some(dep) = follow_path.pop_front() {
            let deps_on = statm
                .query([dep])
                .into_diagnostic()?
                .map(|it| it.get(0))
                .collect::<Vec<String>>()
                .into_diagnostic()?;

            // FIXME: the dep loop is posibale insha'Allah
            follow_path.append(&mut deps_on.clone().into());

            for dep in deps_on {
                if !deps_stuck.contains(&dep) {
                    deps_stuck.push(dep);
                }
            }
        }

        Ok(deps_stuck)
    }

    /// update pkg info in db.
    pub fn update(&self, pkgs: HashMap</*name*/ String, Pkg>) -> miette::Result<()> {
        let mut update_pkg_into_statm = self
            .conn
            .prepare_cached(sql::UPDATE_PKG_INFO)
            .into_diagnostic()?;

        for (pkg_name, pkg) in pkgs {
            let old_pkg = self.load(&pkg_name)?;

            update_pkg_into_statm
                .execute((
                    pkg_name,
                    pkg.version,
                    pkg.just_a_dep,
                    pkg.path.to_string_lossy(),
                    pkg.type_name,
                ))
                .into_diagnostic()?;

            todo!("update and link/unlink paths base on need");
            todo!("update and depand/undepnad paths base on need");
        }

        Ok(())
    }

    pub fn mark_just_a_dep_flag(&self, pkg_name: &str, new_value: bool) -> miette::Result<()> {
        if !self.exists(pkg_name)? {
            return Err(miette!("can't update just_a_dep flag for a non exists pkg"));
        }

        self.conn
            .execute(sql::UPDATE_JUST_A_DEP_VALUE_FOR_PKG, (pkg_name, new_value))
            .map(|_| ())
            .into_diagnostic()
    }

    pub fn load_all(&self) -> miette::Result<Vec<Pkg>> {
        let mut statm = self.conn.prepare(sql::LOAD_ALL_PKGS).into_diagnostic()?;

        let pkgs: Vec<PkgFromDb> = statm
            .query([])
            .into_diagnostic()?
            .map(|row| row.try_into())
            .collect::<Vec<PkgFromDb>>()
            .into_diagnostic()?;

        Ok(join_matched_pkgs(pkgs))
    }

    /// load one pkg
    pub fn load(&self, pkg_name: &str) -> miette::Result<Pkg> {
        let mut statm = self.conn.prepare(sql::LOAD_PKG).into_diagnostic()?;

        let pkgs: Vec<PkgFromDb> = statm
            .query([pkg_name])
            .into_diagnostic()?
            .map(|row| row.try_into())
            .collect::<Vec<PkgFromDb>>()
            .into_diagnostic()?;

        if pkgs.is_empty() {
            return Err(miette!("you try to load a pkg that not exist"));
        }

        let pkg = join_matched_pkgs(pkgs).first().cloned();

        Ok(pkg.unwrap())
    }

    /// remove dependency of a pkg on pkg
    pub fn undepand(&self, pkg_name: &str, deps_names: &[&str]) -> miette::Result<()> {
        if !self.exists(pkg_name)? {
            return Err(miette!("can't remove depandncy for a non exists pkg"));
        }

        let pkg = self.load(pkg_name)?;

        if !deps_names
            .iter()
            .all(|dep| pkg.deps.contains(&dep.to_string()))
        {
            return Err(miette!("can't undepand on a pkg that is not in dependency"));
        };

        for dep in deps_names {
            self.conn
                .execute(sql::UNDEPAND, (pkg_name, dep))
                .into_diagnostic()?;
        }

        Ok(())
    }

    /// remove link of path in pkg
    pub fn unlink(&self, pkg_name: &str, paths: &[&str]) -> miette::Result<()> {
        if !self.exists(pkg_name)? {
            return Err(miette!("can't remove link for a non exists pkg"));
        }

        let pkg = self.load(pkg_name)?;

        if !paths
            .iter()
            .all(|path| pkg.linked_paths.contains(&path.to_string().into()))
        {
            return Err(miette!(
                "can't unlink path that is not linked in the first place"
            ));
        };

        for path in paths {
            self.conn
                .execute(sql::UNLINK_PKG, (pkg_name, path))
                .into_diagnostic()?;
        }

        Ok(())
    }

    /// link pkg path
    pub fn link(&self, pkg_name: &str, paths: &[&str]) -> miette::Result<()> {
        if !self.exists(pkg_name)? {
            return Err(miette!("can't make link for a non exists pkg"));
        }

        let mut statm = self.conn.prepare(sql::LINK_PKG).into_diagnostic()?;

        for path in paths {
            statm.execute((pkg_name, path)).into_diagnostic()?;
        }

        Ok(())
    }

    /// depand on pkg
    pub fn depand(&self, pkg_name: &str, deps: &[&str]) -> miette::Result<()> {
        if !self.exists(pkg_name)? {
            return Err(miette!("can't make depandncy for a non exists pkg"));
        }

        let mut statm = self.conn.prepare(sql::DEPAND_ON_PKG).into_diagnostic()?;

        for dep in deps {
            statm.execute((pkg_name, dep)).into_diagnostic()?;
        }

        Ok(())
    }

    /// check if pkg exists
    pub fn exists(&self, pkg_name: &str) -> miette::Result<bool> {
        let mut statm = self.conn.prepare(sql::GET_PKG_NAME).into_diagnostic()?;

        if let Some(name) = statm
            .query([pkg_name])
            .into_diagnostic()?
            .map(|row| row.get(0))
            .collect::<Vec<String>>()
            .into_diagnostic()?
            .first()
            && name == pkg_name
        {
            Ok(true)
        } else {
            Ok(false)
        }
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
            if let Some(path) = pkg.linked_paths
                && out[index].linked_paths.iter().all(|p| path != *p)
            {
                out[index].linked_paths.push(path.into());
            }

            if let Some(dep) = pkg.deps
                && out[index].deps.iter().all(|d| dep != *d)
            {
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

    use std::path::PathBuf;

    use miette::IntoDiagnostic;
    use tempfile::NamedTempFile;

    use crate::db::{Db, Pkg};

    #[test]
    fn load_pkg_from_db() -> miette::Result<()> {
        let (db, _file) = make_new_db()?;
        inject_mock(&db)?;

        let pkg0 = Pkg {
            name: "pkg0".into(),
            deps: ["pkg1".into(), "pkg2".into()].into(),
            version: None,
            linked_paths: [].into(),
            path: "/path/pkg0".into(),
            type_name: "type".into(),
            just_a_dep: false,
        };

        let pkgs = db.load_all()?;

        assert_eq!(
            pkgs,
            [
                pkg0.clone(),
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

        let pkg = db.load("pkg0")?;

        assert_eq!(pkg, pkg0);

        Ok(())
    }

    #[test]
    fn depand_undepand_link_unlink_update_just_a_dep() -> miette::Result<()> {
        let (db, _file) = make_new_db()?;
        inject_mock(&db)?;

        let before = db.load_all()?;
        db.link("pkg2", &["new"])?;
        db.depand("pkg2", &["pkg1"])?;
        db.undepand("pkg0", &["pkg1"])?;
        db.unlink("pkg1", &["bin1"])?;
        db.mark_just_a_dep_flag("pkg1", true)?;
        let after = db.load_all()?;

        let find = |pkg: &str, vec: &Vec<Pkg>| vec.iter().find(|p| p.name == pkg).unwrap().clone();

        let find_before = |pkg| find(pkg, &before);

        let find_after = |pkg| find(pkg, &after);

        let (bpkg0, bpkg1, bpkg2) = (
            find_before("pkg0"),
            find_before("pkg1"),
            find_before("pkg2"),
        );

        let (fpkg0, fpkg1, fpkg2) = (find_after("pkg0"), find_after("pkg1"), find_after("pkg2"));

        assert!(bpkg2.linked_paths.is_empty());
        assert!(bpkg2.deps.is_empty());
        assert!(bpkg0.deps.contains(&"pkg1".into()));
        assert!(!bpkg1.just_a_dep);
        assert!(bpkg1.linked_paths.contains(&"bin1".into()));

        assert_eq!(fpkg2.linked_paths, [PathBuf::from("new")]);
        assert_eq!(fpkg2.deps, ["pkg1"]);
        assert!(!fpkg0.deps.contains(&"pkg1".into()));
        assert!(fpkg1.just_a_dep);
        assert!(!fpkg1.linked_paths.contains(&"bin1".into()));

        Ok(())
    }

    #[test]
    fn check_if_pkg_exist() -> miette::Result<()> {
        let (db, _file) = make_new_db()?;
        inject_mock(&db)?;

        assert!(db.exists("pkg0")?);
        assert!(!db.exists("not_exists")?);

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

        let pkgs = db.load_all()?;

        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs, input);

        Ok(())
    }

    #[test]
    fn remove_depandncy() -> miette::Result<()> {
        let (db, _file) = make_new_db()?;
        inject_mock(&db)?;

        let pkg0 = db
            .load_all()?
            .iter()
            .find(|pkg| pkg.name == "pkg0")
            .unwrap()
            .clone();

        assert_eq!(pkg0.deps, ["pkg1", "pkg2"]);

        db.undepand("pkg0", &["pkg1"])?;

        let pkg0 = db
            .load_all()?
            .iter()
            .find(|pkg| pkg.name == "pkg0")
            .unwrap()
            .clone();

        assert_eq!(pkg0.deps, ["pkg2"]);

        Ok(())
    }

    #[test]
    fn panic_on_undepand_on_not_a_dep() -> miette::Result<()> {
        let (db, _file) = make_new_db()?;
        inject_mock(&db)?;
        let err = db.undepand("pkg1", &["pkg0"]).unwrap_err();
        assert!(
            err.to_string()
                .contains("can't undepand on a pkg that is not in dependency"),
            "unexpected error: {err}"
        );

        Ok(())
    }

    #[test]
    fn unlink() -> miette::Result<()> {
        let (db, _file) = make_new_db()?;
        inject_mock(&db)?;

        let pkg1 = db.load("pkg1")?;

        let first_val: Vec<PathBuf> = vec!["bin1".into(), "bin2".into(), "bin3".into()];

        assert_eq!(pkg1.linked_paths, first_val);

        db.unlink("pkg1", &["bin1"])?;

        let pkg1 = db.load("pkg1")?;

        let expected_val: Vec<PathBuf> = vec!["bin2".into(), "bin3".into()];
        assert_eq!(pkg1.linked_paths, expected_val);

        Ok(())
    }

    #[test]
    fn remove_a_pkg_and_every_thing_deapnds_on() -> miette::Result<()> {
        let (db, _file) = make_new_db()?;
        inject_mock(&db)?;

        let pkg_should_be_removed = [
            Pkg {
                name: "dep_on_pkg0".into(),
                path: "/some/where/dep_for_pkg0".into(),
                version: None,
                type_name: "".into(),
                just_a_dep: false,
                deps: ["pkg0".into()].into(),
                linked_paths: [].into(), // no links
            },
            Pkg {
                name: "dep_on_pkg0_and_pkg1".into(),
                path: "/some/where/dep_on_pkg0_and_pkg1".into(),
                version: None,
                type_name: "".into(),
                just_a_dep: false,
                deps: ["pkg0".into(), "pkg1".into()].into(),
                linked_paths: [].into(), // no links
            },
            Pkg {
                name: "dep_on_dep_on_pkg0".into(),
                path: "/some/where/dep_on_dep_on_pkg0".into(),
                version: None,
                type_name: "".into(),
                just_a_dep: false,
                deps: ["dep_on_pkg0".into()].into(),
                linked_paths: [].into(), // no links
            },
        ];

        db.install(pkg_should_be_removed.into())?;

        let deps = db.remove_and_pkg_depand_on("pkg0")?;

        let pkgs = db.load_all()?;

        assert_eq!(pkgs.len(), 2);
        assert_eq!(
            pkgs.iter().map(|p| p.name.clone()).collect::<Vec<String>>(),
            ["pkg1", "pkg2"]
        );
        assert_eq!(
            deps,
            ["dep_on_pkg0", "dep_on_pkg0_and_pkg1", "dep_on_dep_on_pkg0"]
        );

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
