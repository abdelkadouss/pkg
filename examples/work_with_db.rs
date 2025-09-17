use std::path::PathBuf;

use pkg::db::*;

fn main() -> miette::Result<()> {
    let db = Db::new(&PathBuf::from("cal.db"))?;

    let installed = db.get_pkgs()?;
    println!("{:#?}", installed);

    Ok(())
}
