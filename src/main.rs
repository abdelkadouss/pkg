#[cfg(feature = "api")]
mod api;
mod bridge;
mod config;
mod fs;
mod utils;

fn main() -> miette::Result<()> {
    Ok(())
}
