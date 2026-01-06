use std::process::Command;
use std::time::Duration;

use crate::ddc::parser::parse_detect_output;
use crate::ddc::types::MonitorInfo;
use crate::error::{Result, XtmonctlError};

pub fn detect_monitors(_timeout: Duration) -> Result<Vec<MonitorInfo>> {
    let output = Command::new("ddcutil").arg("detect").output();
    let output = match output {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(XtmonctlError::DdcutilNotFound);
        }
        Err(error) => {
            return Err(XtmonctlError::CommandFailed {
                command: "ddcutil detect".into(),
                message: error.to_string(),
            });
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_lower = stderr.to_ascii_lowercase();
    if stderr_lower.contains("permission denied") || stderr_lower.contains("eacces") {
        return Err(XtmonctlError::PermissionDenied);
    }
    if stderr_lower.contains("i2c") && stderr_lower.contains("no such file") {
        return Err(XtmonctlError::I2cNotLoaded);
    }
    if !output.status.success() {
        return Err(XtmonctlError::CommandFailed {
            command: "ddcutil detect".into(),
            message: stderr.trim().to_string(),
        });
    }

    parse_detect_output(&String::from_utf8_lossy(&output.stdout))
}
