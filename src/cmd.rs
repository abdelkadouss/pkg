use clap::{Parser, Subcommand};

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
    /// Sync packages with configuration (install/remove as configured)
    #[command(alias = "sync")]
    Build {
        /// even update the installed packages via the update command
        #[arg(short, long)]
        update: bool,
    },

    /// Force sync all packages (reinstall everything)
    Rebuild,

    /// Update packages
    Update {
        /// Specific packages to update ( default: all )
        packages: Option<Vec<String>>,
    },

    /// List installed packages
    Info {
        /// A packge to show information about ( default: all )
        package: Option<Vec<String>>,
    },

    /// Clean cache and temporary files
    Clean,
}

// Helper function to parse CLI arguments
pub fn parse_args() -> Cli {
    Cli::parse()
}
