use tempfile::NamedTempFile;

#[cfg(test)]
use crate::db::*;

#[test]
fn init_and_install() {
    let db_file = NamedTempFile::new().unwrap();
    let db = Db::new(db_file.path().to_path_buf()).unwrap();
    let pkgs = vec![Pkg {
        name: "pkg1".into(),
        version: Version {
            first_cell: "1".into(),
            second_cell: "2".into(),
            third_cell: "3".into(),
        },
        path: "some/path".into(),
        pkg_type: PkgType::SingleExecutable,
    }];

    assert!(db.install_bridge_pkgs(&pkgs, &"bridge".to_string()).is_ok());

    // Get the installed packages
    let pkgs_names: Vec<String> = pkgs.iter().map(|p| p.name.clone()).collect();
    let installed = db.which_pkgs_are_installed(&pkgs_names).unwrap();

    assert_eq!(installed, pkgs.iter().map(|p| &p.name).collect::<Vec<_>>());

    assert_eq!(installed.len(), pkgs.len());

    db.conn.close().unwrap();
}

#[test]
fn remove_pkgs() {
    let db_file = NamedTempFile::new().unwrap();
    let db = Db::new(db_file.path().to_path_buf()).unwrap();

    let pkgs = vec![
        Pkg {
            name: "pkg1".into(),
            version: Version {
                first_cell: "1".into(),
                second_cell: "2".into(),
                third_cell: "3".into(),
            },
            path: "some/path".into(),
            pkg_type: PkgType::SingleExecutable,
        },
        Pkg {
            name: "pkg2".into(),
            version: Version {
                first_cell: "1".into(),
                second_cell: "2".into(),
                third_cell: "3".into(),
            },
            path: "some/path".into(),
            pkg_type: PkgType::SingleExecutable,
        },
    ];

    assert!(db.install_bridge_pkgs(&pkgs, &"bridge".to_string()).is_ok());
    let installed = db.get_pkgs().unwrap();

    assert_eq!(
        installed.iter().map(|p| &p.name).collect::<Vec<_>>(),
        pkgs.iter().map(|p| &p.name).collect::<Vec<_>>()
    );

    db.remove_pkgs(&["pkg2".to_string()]).ok();
    let installed = db.get_pkgs().unwrap();
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].name, "pkg1");

    db.remove_pkgs(&pkgs.iter().map(|p| p.name.clone()).collect::<Vec<_>>())
        .ok();
    let installed = db.get_pkgs().unwrap();
    assert_eq!(installed.len(), 0);
}
