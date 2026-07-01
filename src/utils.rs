use std::{env, path::PathBuf};

use miette::{IntoDiagnostic, miette};
use path_absolutize::Absolutize;

pub trait ExtandPath {
    fn extand_path(&self) -> miette::Result<PathBuf>;
}

impl ExtandPath for PathBuf {
    /// give the absoloute path, replace ~ with home dir and handle ./ syntax.
    fn extand_path(&self) -> miette::Result<PathBuf> {
        let path_string = self.to_str().unwrap().replace(
            "~",
            env::home_dir()
                .ok_or(miette!("fiald to get home dir, did u set $HOME?"))?
                .to_str()
                .unwrap(),
        );

        Ok(PathBuf::from(path_string)
            .absolutize()
            .into_diagnostic()?
            .to_path_buf())
    }
}

#[cfg(test)]
#[test]
fn extand_path() -> miette::Result<()> {
    let my_home = env::home_dir().unwrap();

    let input = PathBuf::from("~/././some/../some/./path.txt");
    let expaxted = format!("{}/some/path.txt", my_home.to_str().unwrap());

    assert_eq!(input.extand_path().unwrap(), PathBuf::from(expaxted));

    Ok(())
}
