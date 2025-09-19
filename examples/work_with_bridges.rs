use std::{collections::HashMap, path::PathBuf};

use miette::Result;
use pkg::{bridge::*, config::Config, input::PkgDeclaration};

fn main() -> Result<()> {
    let config = Config::load(PathBuf::from(".tmp/config/config.kdl"))?;
    let db_path = tempfile::NamedTempFile::new().unwrap().path().to_path_buf();
    let bridge_api = BridgeApi::new(
        config.bridges_set.clone(),
        &vec!["bridge1".to_string()],
        &db_path,
    )?;

    let res = bridge_api.update(
        "bridge1",
        &PkgDeclaration {
            name: "pkg1".to_string(),
            input: "pkg1".to_string(),
            attributes: HashMap::new(),
        },
    )?;

    println!("{:#?}", res);

    Ok(())
}
