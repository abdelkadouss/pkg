pub const DEFAULT_CONFIG_FILE_NAME: &str = ".config";
pub const DEFAULT_CONFIG_FILE_EXTENSION: &str = "kdl";
pub const DEFAULT_LOG_DIR: &str = "/var/log/pkg";
pub const DEFAULT_WORKING_DIR: &str = "/var/tmp/pkg";

pub mod config;

pub mod input;
pub use input::Bridge;

pub mod db;
use db::{Pkg, PkgType, Version as PkgVersion};

pub mod bridge;

pub mod fs;

pub mod cmd;

#[cfg(test)]
mod test;
