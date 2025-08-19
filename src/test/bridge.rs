use crate::bridge::*;

#[test]
fn init_a_bridge_api() {
    let bridge_set_path = std::path::PathBuf::from("examples/assets/bridges");

    let bridge_api = BridgeApi::new(bridge_set_path, vec!["bridge1".to_string()]).unwrap();
}
