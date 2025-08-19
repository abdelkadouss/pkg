use pkg::{
    bridge::*,
    input::{Input, PkgDeclaration},
};

fn main() {
    let bridge_set_path = std::path::PathBuf::from("examples/assets/bridges");

    let input = Input::load(std::path::PathBuf::from("examples/assets/inputs")).unwrap();
    let needed_bridges = input
        .bridges
        .iter()
        .map(|b| b.name.clone())
        .collect::<Vec<String>>();

    let bridge_api = BridgeApi::new(bridge_set_path, needed_bridges).unwrap();

    for bridge in input.bridges {
        for pkg in bridge.pkgs {
            let pkg_name = pkg.name.clone();
            bridge_api.install(&bridge.name, pkg).map_err(|_| {
                println!("Failed to install {}", pkg_name);
            });
        }
    }
}
