use clap::{ColorChoice, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "pkg")]
#[command(version = "0.2.0")]
#[command(about = "A package manager with Lua bridge support", long_about = None)]
#[command(color = ColorChoice::Always)] // Always show colors
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Sync packages with configuration (install/remove as configured)
    #[command(alias = "sync", alias = "b", alias = "s")]
    Build {
        /// even update the installed packages via the update command
        #[arg(short, long)]
        update: bool,
    },

    /// Force sync all packages (reinstall everything)
    Rebuild,

    /// Update packages
    #[command(alias = "u")]
    Update {
        /// Specific packages to update ( default: all )
        packages: Option<Vec<String>>,
    },

    /// List installed packages
    Info {
        /// A packge to show information about ( default: all )
        package: Option<Vec<String>>,
    },

    /// Link packages in PATH
    Link,

    /// Clean cache and temporary files
    Clean,

    /// Some notes can help insha'Allah
    Docs,
}

// Helper function to parse CLI arguments
pub fn parse_args() -> Cli {
    Cli::parse()
}
