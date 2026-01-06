use std::str::FromStr;
use std::time::Duration;

use crate::ddc::detect::run_ddcutil;
use crate::ddc::types::MonitorInfo;
use crate::error::{Result, XtmonctlError};
use crate::units::{BrightnessPercent, BrightnessRaw};

pub fn get_brightness_raw(monitor: &MonitorInfo, timeout: Duration) -> Result<BrightnessRaw> {
    let output = run_ddcutil(
        [
            "getvcp",
            "10",
            "--bus",
            &monitor.id.i2c_bus.to_string(),
            "--brief",
        ],
        timeout,
    )?;

    if !output.status.success() {
        return classify_ddc_error(
            format!("ddcutil getvcp 10 --bus {}", monitor.id.i2c_bus),
            &String::from_utf8_lossy(&output.stderr),
            monitor,
        );
    }

    parse_getvcp_output(&String::from_utf8_lossy(&output.stdout))
}

pub fn set_brightness_raw(
    monitor: &MonitorInfo,
    brightness: BrightnessRaw,
    timeout: Duration,
) -> Result<()> {
    let output = run_ddcutil(
        [
            "setvcp",
            "10",
            &brightness.value.to_string(),
            "--bus",
            &monitor.id.i2c_bus.to_string(),
        ],
        timeout,
    )?;

    if !output.status.success() {
        return classify_ddc_error(
            format!(
                "ddcutil setvcp 10 {} --bus {}",
                brightness.value, monitor.id.i2c_bus
            ),
            &String::from_utf8_lossy(&output.stderr),
            monitor,
        );
    }

    Ok(())
}

pub fn set_brightness_percent(
    monitor: &MonitorInfo,
    percent: BrightnessPercent,
    timeout: Duration,
) -> Result<BrightnessRaw> {
    let current = get_brightness_raw(monitor, timeout)?;
    let next = percent.to_raw(current.max);
    set_brightness_raw(monitor, next, timeout)?;
    Ok(next)
}

pub fn parse_getvcp_output(output: &str) -> Result<BrightnessRaw> {
    let mut parts = output.split_whitespace();
    while let Some(part) = parts.next() {
        if part == "VCP" {
            let code = parts.next().unwrap_or_default();
            let mode = parts.next().unwrap_or_default();
            let current = parts.next().unwrap_or_default();
            let max = parts.next().unwrap_or_default();
            if code == "10" && mode == "C" {
                let current = u16::from_str(current).map_err(|_| {
                    XtmonctlError::ParseError(format!("invalid current brightness: {current}"))
                })?;
                let max = u16::from_str(max).map_err(|_| {
                    XtmonctlError::ParseError(format!("invalid max brightness: {max}"))
                })?;
                return BrightnessRaw::new(current, max);
            }
        }
    }

    Err(XtmonctlError::ParseError(
        "missing brightness value in getvcp output".into(),
    ))
}

fn classify_ddc_error<T>(command: String, stderr: &str, monitor: &MonitorInfo) -> Result<T> {
    let stderr_lower = stderr.to_ascii_lowercase();
    if stderr_lower.contains("permission denied") || stderr_lower.contains("eacces") {
        Err(XtmonctlError::PermissionDenied)
    } else if stderr_lower.contains("no displays") || stderr_lower.contains("invalid") {
        Err(XtmonctlError::MonitorNotFound(monitor.id.bus_name()))
    } else {
        Err(XtmonctlError::CommandFailed {
            command,
            message: stderr.trim().to_string(),
        })
    }
}
