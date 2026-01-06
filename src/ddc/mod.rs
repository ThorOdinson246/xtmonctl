pub mod brightness;
pub mod detect;
pub mod parser;
pub mod types;

pub use brightness::{get_brightness_raw, parse_getvcp_output, set_brightness_percent, set_brightness_raw};
pub use detect::detect_monitors;
pub use types::{ConnectorType, MonitorId, MonitorInfo};
