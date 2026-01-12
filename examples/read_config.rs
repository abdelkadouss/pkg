use pkg_rs::config::Config;
use std::path::PathBuf;

fn main() -> miette::Result<()> {
    let config = Config::load(PathBuf::from("examples/assets/config.kdl"))?;
    println!("{:#?}", config);
    Ok(())
}
