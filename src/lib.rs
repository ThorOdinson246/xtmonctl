pub mod app;
pub mod cli;
pub mod config;
pub mod ddc;
pub mod error;
pub mod tui;
pub mod units;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
