use std::time::Duration;

use crate::ddc::{detect_monitors, get_brightness_raw, set_brightness_percent, MonitorInfo};
use crate::error::{Result, XtmonctlError};
use crate::units::BrightnessPercent;

#[derive(Debug, Clone)]
pub struct App {
    pub detection_timeout: Duration,
    pub command_timeout: Duration,
}

impl Default for App {
    fn default() -> Self {
        Self {
            detection_timeout: Duration::from_secs(15),
            command_timeout: Duration::from_secs(5),
        }
    }
}

impl App {
    pub fn list_monitors(&self) -> Result<Vec<MonitorInfo>> {
        detect_monitors(self.detection_timeout)
    }

    pub fn find_monitor<'a>(&self, monitors: &'a [MonitorInfo], identifier: &str) -> Result<&'a MonitorInfo> {
        find_monitor(monitors, identifier)
    }

    pub fn get_monitor_brightness(&self, monitor: &MonitorInfo) -> Result<crate::units::BrightnessRaw> {
        get_brightness_raw(monitor, self.command_timeout)
    }

    pub fn set_monitor_brightness(
        &self,
        monitor: &MonitorInfo,
        brightness: BrightnessPercent,
    ) -> Result<crate::units::BrightnessRaw> {
        set_brightness_percent(monitor, brightness, self.command_timeout)
    }
}

pub fn find_monitor<'a>(monitors: &'a [MonitorInfo], identifier: &str) -> Result<&'a MonitorInfo> {
    let trimmed = identifier.trim();
    if let Some(bus) = trimmed.strip_prefix("i2c-").and_then(|value| value.parse::<u32>().ok()) {
        return monitors
            .iter()
            .find(|monitor| monitor.id.i2c_bus == bus)
            .ok_or_else(|| XtmonctlError::MonitorNotFound(trimmed.to_string()));
    }

    if let Ok(number) = trimmed.parse::<u32>() {
        if let Some(monitor) = monitors
            .iter()
            .find(|monitor| monitor.id.display_number == number || monitor.id.i2c_bus == number)
        {
            return Ok(monitor);
        }
    }

    let lowered = trimmed.to_ascii_lowercase();
    let matches = monitors
        .iter()
        .filter(|monitor| {
            monitor.model.to_ascii_lowercase().contains(&lowered)
                || monitor.manufacturer.to_ascii_lowercase().contains(&lowered)
        })
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [single] => Ok(*single),
        [] => Err(XtmonctlError::MonitorNotFound(trimmed.to_string())),
        _ => Err(XtmonctlError::MonitorNotFound(format!(
            "{trimmed} (multiple matches)"
        ))),
    }
}
