use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Result, XtmonctlError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitorConfig {
    pub alias: Option<String>,
    pub last_brightness_percent: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default)]
    pub monitors: HashMap<String, MonitorConfig>,
    #[serde(default = "default_step_percent")]
    pub default_step_percent: u8,
    #[serde(default = "default_large_step_percent")]
    pub large_step_percent: u8,
    #[serde(default = "default_detection_timeout_secs")]
    pub detection_timeout_secs: u64,
    #[serde(default = "default_command_timeout_secs")]
    pub command_timeout_secs: u64,
    #[serde(default = "default_tui_theme")]
    pub tui_theme: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            monitors: HashMap::new(),
            default_step_percent: default_step_percent(),
            large_step_percent: default_large_step_percent(),
            detection_timeout_secs: default_detection_timeout_secs(),
            command_timeout_secs: default_command_timeout_secs(),
            tui_theme: default_tui_theme(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        Self::load_from_path(&config_path())
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let data = fs::read_to_string(path).map_err(|source| XtmonctlError::ConfigIo {
            path: path.to_path_buf(),
            source,
        })?;
        serde_yaml::from_str(&data).map_err(|error| XtmonctlError::ConfigFormat {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
    }

    pub fn save(&self) -> Result<()> {
        self.save_to_path(&config_path())
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| XtmonctlError::ConfigIo {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let data = serde_yaml::to_string(self).map_err(|error| XtmonctlError::ConfigFormat {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

        fs::write(path, data).map_err(|source| XtmonctlError::ConfigIo {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn alias_for(&self, bus_name: &str) -> Option<&str> {
        self.monitors
            .get(bus_name)
            .and_then(|entry| entry.alias.as_deref())
    }

    pub fn last_brightness_for(&self, bus_name: &str) -> Option<u8> {
        self.monitors
            .get(bus_name)
            .and_then(|entry| entry.last_brightness_percent)
    }

    pub fn aliases(&self) -> impl Iterator<Item = (&str, &str)> {
        self.monitors.iter().filter_map(|(bus_name, entry)| {
            entry
                .alias
                .as_deref()
                .filter(|alias| !alias.trim().is_empty())
                .map(|alias| (bus_name.as_str(), alias))
        })
    }

    pub fn set_last_brightness(&mut self, bus_name: &str, percent: u8) {
        let entry = self
            .monitors
            .entry(bus_name.to_string())
            .or_insert(MonitorConfig {
                alias: None,
                last_brightness_percent: None,
            });
        entry.last_brightness_percent = Some(percent);
    }

    pub fn set_alias(&mut self, bus_name: &str, alias: String) {
        let entry = self
            .monitors
            .entry(bus_name.to_string())
            .or_insert(MonitorConfig {
                alias: None,
                last_brightness_percent: None,
            });
        entry.alias = Some(alias);
    }

    pub fn clear_alias(&mut self, bus_name: &str) -> bool {
        if let Some(entry) = self.monitors.get_mut(bus_name) {
            let had_alias = entry.alias.take().is_some();
            if entry.alias.is_none() && entry.last_brightness_percent.is_none() {
                self.monitors.remove(bus_name);
            }
            had_alias
        } else {
            false
        }
    }

    pub fn tui_theme(&self) -> &str {
        &self.tui_theme
    }

    pub fn set_tui_theme(&mut self, theme: impl Into<String>) {
        self.tui_theme = theme.into();
    }
}

pub fn config_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(custom).join("xtmonctl");
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("xtmonctl")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.yaml")
}

const fn default_step_percent() -> u8 {
    5
}

const fn default_large_step_percent() -> u8 {
    10
}

const fn default_detection_timeout_secs() -> u64 {
    15
}

const fn default_command_timeout_secs() -> u64 {
    5
}

fn default_tui_theme() -> String {
    "ocean".to_string()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{Config, MonitorConfig};

    #[test]
    fn missing_file_returns_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let config = Config::load_from_path(&path).unwrap();
        assert_eq!(config.default_step_percent, 5);
        assert_eq!(config.tui_theme(), "ocean");
    }

    #[test]
    fn config_round_trip_preserves_monitor_data() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let mut config = Config::default();
        config.monitors.insert(
            "i2c-4".into(),
            MonitorConfig {
                alias: Some("Main".into()),
                last_brightness_percent: Some(42),
            },
        );
        config.set_tui_theme("forest");

        config.save_to_path(&path).unwrap();
        let loaded = Config::load_from_path(&path).unwrap();
        assert_eq!(loaded.alias_for("i2c-4"), Some("Main"));
        assert_eq!(loaded.last_brightness_for("i2c-4"), Some(42));
        assert_eq!(loaded.tui_theme(), "forest");
    }
}
