use std::io;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum XtmonctlError {
    #[error("ddcutil not found. Install it with your package manager.")]
    DdcutilNotFound,
    #[error("permission denied accessing I2C devices")]
    PermissionDenied,
    #[error("i2c-dev kernel module not loaded")]
    I2cNotLoaded,
    #[error("command timed out after {0:?}")]
    CommandTimeout(Duration),
    #[error("monitor not found: {0}")]
    MonitorNotFound(String),
    #[error("failed to parse ddcutil output: {0}")]
    ParseError(String),
    #[error("failed to read config file {path}: {source}")]
    ConfigIo { path: PathBuf, source: io::Error },
    #[error("failed to parse config file {path}: {message}")]
    ConfigFormat { path: PathBuf, message: String },
    #[error("invalid brightness value: {0}")]
    InvalidBrightness(String),
    #[error("command failed: {command}: {message}")]
    CommandFailed { command: String, message: String },
}

pub type Result<T> = std::result::Result<T, XtmonctlError>;
