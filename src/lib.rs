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

pub use app::{find_monitor, App};
pub use cli::{Cli, Commands};
pub use ddc::{
    detect_monitors, get_brightness_raw, parse_detect_output, parse_getvcp_output,
    set_brightness_percent, set_brightness_raw, ConnectorType, MonitorId, MonitorInfo,
};
pub use error::{Result, XtmonctlError};
pub use units::{BrightnessPercent, BrightnessRaw};
