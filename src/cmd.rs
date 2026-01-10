use clap::{ColorChoice, Parser, Subcommand};

#[cfg(feature = "cli_complation")]
#[derive(Clone, Debug, clap::ValueEnum)]
pub enum Shell {
    Bash,
    Fish,
    Zsh,
    Elvish,
    Nushell,
    #[allow(clippy::enum_variant_names)]
    PowerShell, // NOTE: this is not needed really because this is unix only
}

#[derive(Parser)]
#[command(name = "pkg")]
#[command(version, about, long_about = None)] // Read from `Cargo.toml`
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

    #[cfg(feature = "cli_complation")]
    /// Generate shell completion scripts for your clap::Command
    #[command(alias = "compl")]
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

// Helper function to parse CLI arguments
pub fn parse_args() -> Cli {
    Cli::parse()
}
