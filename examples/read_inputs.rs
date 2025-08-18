use pkg::input::*;
use std::path::PathBuf;

fn main() -> miette::Result<()> {
    let _ = Input::load(PathBuf::from("examples/assets/inputs"))?;

    Ok(())
}
