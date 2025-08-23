use crate::{
    Pkg,
    db::{Db, PkgType},
};
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

    #[error("The given load path is exist and is a file")]
    #[diagnostic(code(fs::load_path_is_file))]
    LoadPathIsFile,
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
        } else if !self.load_path.is_dir() {
            std::fs::remove_dir_all(&self.load_path).into_diagnostic()?;
            std::fs::create_dir_all(&self.load_path).into_diagnostic()?;
        } else {
            return Err(FsError::LoadPathIsFile).into_diagnostic()?;
        }

        for pkg in pkgs {
            let target = self.load_path.join(pkg.name);

            if target.exists() {
                std::fs::remove_file(&target).into_diagnostic()?;
            }

            match pkg.pkg_type {
                PkgType::SingleExecutable => {
                    std::os::unix::fs::symlink(&pkg.path, &target).into_diagnostic()?;
                }
                PkgType::Directory(ref entry_point) => {
                    std::os::unix::fs::symlink(entry_point, &target).into_diagnostic()?;
                }
            }
        }

        Ok(())
    }

    pub fn store_or_overwrite(
        &self,
        pkgs: &mut [&mut Pkg],
        bridge_name: Option<&str>,
    ) -> Result<()> {
        if !self.target_dir.exists() {
            std::fs::create_dir_all(&self.target_dir).into_diagnostic()?;
        }

        for pkg in pkgs {
            let target_dir = self.target_dir.join(bridge_name.unwrap_or(""));

            if !target_dir.exists() {
                std::fs::create_dir_all(&target_dir).into_diagnostic()?;
            }

            let target = target_dir.join(&pkg.name);

            if target.exists() {
                if target.is_dir() {
                    std::fs::remove_dir_all(&target).into_diagnostic()?;
                } else {
                    std::fs::remove_file(&target).into_diagnostic()?;
                }
            }

            std::fs::rename(&pkg.path, &target).into_diagnostic()?;

            if let PkgType::Directory(ref entry_point) = pkg.pkg_type {
                // change the entry point parent to the target dir
                let entry_point_str = entry_point.to_str().unwrap();
                let old_path_str = pkg.path.to_str().unwrap();
                let target_str = target.to_str().unwrap();

                let new_entry_point_str = entry_point_str.replace(old_path_str, target_str);
                pkg.pkg_type = PkgType::Directory(PathBuf::from(new_entry_point_str))
            };

            pkg.path = target;
        }

        Ok(())
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
