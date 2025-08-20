use crate::{Pkg, db::Db};
use miette::{Diagnostic, IntoDiagnostic, Result, SourceSpan};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug)]
pub struct Fs {
    target_dir: PathBuf,
    load_path: PathBuf,
    db: Db,
}

#[derive(Error, Debug, Diagnostic)]
pub enum FsError {
    #[error(transparent)]
    #[diagnostic(code(fs::io_error))]
    IoError(#[from] std::io::Error),
}

impl Fs {
    pub fn new(target_dir: PathBuf, load_path: PathBuf, db: Db) -> Self {
        Self {
            target_dir,
            load_path,
            db,
        }
    }

    pub fn link(&self) -> Result<()> {
        let pkgs = self.db.get_pkgs()?;

        if !self.load_path.exists() {
            std::fs::create_dir_all(&self.load_path).into_diagnostic()?;
        }

        for pkg in pkgs {
            let target = self.load_path.join(pkg.name);

            if target.exists() {
                std::fs::remove_file(&target).into_diagnostic()?;
            }

            std::fs::hard_link(pkg.path, &target).into_diagnostic()?;
        }

        Ok(())
    }

    pub fn store_or_overwrite(&self, pkgs: &[Pkg], bridge_name: Option<&str>) -> Result<()> {
        if !self.target_dir.exists() {
            std::fs::create_dir_all(&self.target_dir).into_diagnostic()?;
        }

        for pkg in pkgs {
            let target = self
                .target_dir
                .join(bridge_name.unwrap_or(""))
                .join(&pkg.name);

            if target.exists() {
                std::fs::remove_file(&target).into_diagnostic()?;
            }

            std::fs::rename(&pkg.path, &target).into_diagnostic()?;
        }

        Ok(())
    }
}
