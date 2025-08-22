use crate::{Pkg, db::Db};
use miette::{Diagnostic, IntoDiagnostic, Result};
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
    pub fn new(target_dir: PathBuf, load_path: PathBuf, db_path: &PathBuf) -> Self {
        let db = Db::new(db_path).unwrap();

        let _ = std::fs::create_dir_all(&target_dir);
        let _ = std::fs::create_dir_all(&load_path);

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

            std::os::unix::fs::symlink(&pkg.path, &target).into_diagnostic()?;
        }

        Ok(())
    }

    pub fn store_or_overwrite(
        &self,
        pkgs: &[&Pkg],
        bridge_name: Option<&str>,
    ) -> Result<Vec<PathBuf>> {
        if !self.target_dir.exists() {
            std::fs::create_dir_all(&self.target_dir).into_diagnostic()?;
        }

        let mut new_paths = Vec::new();

        for pkg in pkgs {
            let target_dir = self.target_dir.join(bridge_name.unwrap_or(""));

            if !target_dir.exists() {
                std::fs::create_dir_all(&target_dir).into_diagnostic()?;
            }

            let target = target_dir.join(&pkg.name);

            if target.exists() {
                std::fs::remove_file(&target).into_diagnostic()?;
            }

            std::fs::rename(&pkg.path, &target).into_diagnostic()?;

            new_paths.push(target);
        }

        Ok(new_paths)
    }

    pub fn remove_pkgs(&self, pkgs: &[&String]) -> Result<bool> {
        let pkgs = pkgs.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        let pkgs = pkgs.as_slice();

        let mut removed = false;

        let pkgs = self.db.get_pkgs_by_name(pkgs)?;

        for pkg in pkgs {
            let target = self.target_dir.join(&pkg.name);

            if target.exists() {
                if target.is_dir() {
                    std::fs::remove_dir_all(&target).into_diagnostic()?;
                } else {
                    std::fs::remove_file(&target).into_diagnostic()?;
                }
                removed = true;
            }
        }
        Ok(removed)
    }
}
