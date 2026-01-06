use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::ddc::parser::parse_detect_output;
use crate::ddc::types::MonitorInfo;
use crate::error::{Result, XtmonctlError};

pub fn detect_monitors(timeout: Duration) -> Result<Vec<MonitorInfo>> {
    let output = run_ddcutil(["detect"], timeout)?;

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

pub(crate) struct CommandOutput {
    pub status: std::process::ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub(crate) fn run_ddcutil<I, S>(args: I, timeout: Duration) -> Result<CommandOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect::<Vec<_>>();
    let command_repr = format!("ddcutil {}", args.join(" "));

    let mut child = Command::new("ddcutil")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                XtmonctlError::DdcutilNotFound
            } else {
                XtmonctlError::CommandFailed {
                    command: command_repr.clone(),
                    message: error.to_string(),
                }
            }
        })?;

    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(XtmonctlError::CommandTimeout(timeout));
        }
        Err(error) => {
            return Err(XtmonctlError::CommandFailed {
                command: command_repr,
                message: error.to_string(),
            });
        }
    };

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_end(&mut stdout);
    }
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_end(&mut stderr);
    }

    Ok(CommandOutput {
        status,
        stdout,
        stderr,
    })
}
