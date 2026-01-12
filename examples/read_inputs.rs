use pkg_cli::input::*;
use std::path::PathBuf;

fn main() -> miette::Result<()> {
    let input = Input::load(&PathBuf::from("examples/assets/inputs"))?;
    println!("{:#?}", input);

    Ok(())
}
