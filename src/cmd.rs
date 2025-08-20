use clap::{ArgAction, Parser, Subcommand};
use rpassword::read_password;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "pkg")]
#[command(version = "0.1.0")]
#[command(about = "A package manager with Lua bridge support", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Sync packages with configuration (install missing packages)
    Build {
        /// Force reinstall even if package already exists
        #[arg(short, long)]
        update: bool,
    },

    /// Force sync all packages (reinstall everything)
    Rebuild {},

    /// Update packages
    Update {
        /// Specific packages to update (default: all)
        packages: Option<Vec<String>>,
    },

    /// List installed packages
    Info {
        /// A packge to show information about [default: all]
        package: Option<Vec<String>>,

        /// Show detailed information
        #[arg(short, long)]
        long: bool,

        /// Filter by bridge
        #[arg(short, long)]
        bridge: Option<String>,
    },

    /// Clean cache and temporary files
    Clean {
        /// Remove all cached files, including logs
    },
}

// Helper function to parse CLI arguments
pub fn parse_args() -> Cli {
    Cli::parse()
}
