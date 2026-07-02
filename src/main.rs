#[cfg(feature = "api")]
mod api;
mod bridge;
mod config;
mod fs;
mod pkg_type;
mod utils;

fn main() -> miette::Result<()> {
    Ok(())
}
