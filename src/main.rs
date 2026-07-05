#[cfg(feature = "api")]
mod api;
mod bridge;
mod cli;
mod config;
mod db;
mod fs;
mod pkg;
mod pkg_type;
mod process;
mod utils;

fn main() -> miette::Result<()> {
    cli::Cli::route()
}
