use pkg::{Bridge, db::*};

fn main() -> miette::Result<()> {
    let db = Db::new("local.db".into())?;

    let installed = db.get_pkgs()?;
    println!("{:#?}", installed);

    Ok(())
}
