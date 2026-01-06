use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::Config;
use crate::ddc::{detect_monitors, get_brightness_raw, set_brightness_percent, MonitorInfo};
use crate::error::{Result, XtmonctlError};
use crate::units::BrightnessPercent;

#[derive(Debug, Clone)]
pub struct App {
    pub detection_timeout: Duration,
    pub command_timeout: Duration,
    config: Arc<Mutex<Config>>,
    config_path: PathBuf,
}

impl Default for App {
    fn default() -> Self {
        Self::load()
            .unwrap_or_else(|_| Self::from_config(Config::default(), crate::config::config_path()))
    }
}

impl App {
    pub fn load() -> Result<Self> {
        let path = crate::config::config_path();
        let config = Config::load_from_path(&path)?;
        Ok(Self::from_config(config, path))
    }

    fn from_config(config: Config, config_path: PathBuf) -> Self {
        Self {
            detection_timeout: Duration::from_secs(config.detection_timeout_secs),
            command_timeout: Duration::from_secs(config.command_timeout_secs),
            config: Arc::new(Mutex::new(config)),
            config_path,
        }
    }

    pub fn list_monitors(&self) -> Result<Vec<MonitorInfo>> {
        detect_monitors(self.detection_timeout)
    }

    pub fn find_monitor<'a>(
        &self,
        monitors: &'a [MonitorInfo],
        identifier: &str,
    ) -> Result<&'a MonitorInfo> {
        find_monitor(monitors, identifier)
    }

    pub fn get_monitor_brightness(
        &self,
        monitor: &MonitorInfo,
    ) -> Result<crate::units::BrightnessRaw> {
        get_brightness_raw(monitor, self.command_timeout)
    }

    pub fn set_monitor_brightness(
        &self,
        monitor: &MonitorInfo,
        brightness: BrightnessPercent,
    ) -> Result<crate::units::BrightnessRaw> {
        let raw = set_brightness_percent(monitor, brightness, self.command_timeout)?;
        let mut config = self.config_lock()?;
        config.set_last_brightness(&monitor.id.bus_name(), brightness.value());
        config.save_to_path(&self.config_path)?;
        Ok(raw)
    }

    pub fn set_all_monitors(
        &self,
        brightness: BrightnessPercent,
    ) -> Result<Vec<(MonitorInfo, crate::units::BrightnessRaw)>> {
        let monitors = self.list_monitors()?;
        let mut results = Vec::with_capacity(monitors.len());
        for monitor in monitors {
            let raw = self.set_monitor_brightness(&monitor, brightness)?;
            results.push((monitor, raw));
        }
        Ok(results)
    }

    pub fn alias_for(&self, monitor: &MonitorInfo) -> Option<String> {
        self.config
            .lock()
            .ok()
            .and_then(|config| config.alias_for(&monitor.id.bus_name()).map(str::to_string))
    }

    pub fn last_brightness_for(&self, monitor: &MonitorInfo) -> Option<u8> {
        self.config
            .lock()
            .ok()
            .and_then(|config| config.last_brightness_for(&monitor.id.bus_name()))
    }

    pub fn display_label(&self, monitor: &MonitorInfo) -> String {
        match self.alias_for(monitor) {
            Some(alias) if !alias.is_empty() => format!("{alias} [{}]", monitor.display_name()),
            _ => monitor.display_name(),
        }
    }

    pub fn default_step_percent(&self) -> u8 {
        self.config
            .lock()
            .map(|config| config.default_step_percent)
            .unwrap_or(5)
    }

    pub fn large_step_percent(&self) -> u8 {
        self.config
            .lock()
            .map(|config| config.large_step_percent)
            .unwrap_or(10)
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    fn config_lock(&self) -> Result<std::sync::MutexGuard<'_, Config>> {
        self.config
            .lock()
            .map_err(|_| XtmonctlError::State("configuration lock poisoned".into()))
    }
}

pub fn find_monitor<'a>(monitors: &'a [MonitorInfo], identifier: &str) -> Result<&'a MonitorInfo> {
    let trimmed = identifier.trim();
    if let Some(bus) = trimmed
        .strip_prefix("i2c-")
        .and_then(|value| value.parse::<u32>().ok())
    {
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
