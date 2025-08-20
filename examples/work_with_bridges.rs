use std::collections::HashMap;

use pkg::{
    bridge::*,
    db::*,
    input::{Input, PkgDeclaration},
};
use tempfile::NamedTempFile;

fn main() {
    let db_file = NamedTempFile::new().unwrap();
    let db = Db::new(db_file.path().to_path_buf()).unwrap();
    let pkgs = vec![Pkg {
        name: "pkg1".into(),
        version: Version {
            first_cell: "1".into(),
            second_cell: "2".into(),
            third_cell: "3".into(),
        },
        path: "/Users/abdelkdous/remove_me".into(),
        pkg_type: PkgType::SingleExecutable,
    }];

    let bridge = "bridge1".to_string();

    let res = db.install_bridge_pkgs(&pkgs, &bridge);
    assert!(res.is_ok());

    let bridge_set_path = std::path::PathBuf::from("examples/assets/bridges");

    let input = Input::load(std::path::PathBuf::from("examples/assets/inputs")).unwrap();
    let needed_bridges = input
        .bridges
        .iter()
        .map(|b| b.name.clone())
        .collect::<Vec<String>>();

    let bridge_api = BridgeApi::new(bridge_set_path, needed_bridges, db.path.clone()).unwrap();

    for bridge in input.bridges {
        for pkg in bridge.pkgs {
            let pkg_name = pkg.name.clone();
            let _ = bridge_api.install(&bridge.name, pkg).map_err(|_| {
                println!("Failed to install {}", pkg_name);
            });
        }
    }

    let pkg = PkgDeclaration {
        name: "pkg1".to_string(),
        input: "pkg1".to_string(),
        attributes: HashMap::new(),
    };

    bridge_api.update("bridge1", pkg).unwrap();
}
