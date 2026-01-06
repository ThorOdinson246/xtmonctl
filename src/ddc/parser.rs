use crate::ddc::types::{ConnectorType, MonitorId, MonitorInfo};
use crate::error::{Result, XtmonctlError};

pub fn parse_detect_output(output: &str) -> Result<Vec<MonitorInfo>> {
    let mut monitors = Vec::new();
    let mut current: Option<MonitorBuilder> = None;
    let mut is_valid_display = false;

    for line in output.lines() {
        let stripped = line.trim();

        if let Some(display_number) = parse_display_start(stripped) {
            if let Some(builder) = current.take() {
                if is_valid_display {
                    monitors.push(builder.build()?);
                }
            }
            current = Some(MonitorBuilder::new(display_number));
            is_valid_display = true;
            continue;
        }

        if stripped == "Invalid display" {
            if let Some(builder) = current.take() {
                if is_valid_display {
                    monitors.push(builder.build()?);
                }
            }
            is_valid_display = false;
            continue;
        }

        if !is_valid_display {
            continue;
        }

        if let Some(builder) = current.as_mut() {
            builder.update(stripped);
        }
    }

    if let Some(builder) = current {
        if is_valid_display {
            monitors.push(builder.build()?);
        }
    }

    Ok(monitors)
}

fn parse_display_start(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("Display ")?;
    rest.parse().ok()
}

fn extract_connector_type(value: &str) -> Option<ConnectorType> {
    let upper = value.to_ascii_uppercase();
    if upper.contains("HDMI") {
        Some(ConnectorType::Hdmi)
    } else if upper.contains("DP-") || upper.contains("DISPLAYPORT") {
        Some(ConnectorType::DisplayPort)
    } else if upper.contains("EDP") {
        Some(ConnectorType::Edp)
    } else if upper.contains("DVI") {
        Some(ConnectorType::Dvi)
    } else if upper.contains("VGA") {
        Some(ConnectorType::Vga)
    } else {
        None
    }
}

#[derive(Debug, Default)]
struct MonitorBuilder {
    display_number: u32,
    i2c_bus: Option<u32>,
    manufacturer: String,
    model: String,
    serial: String,
    drm_connector: Option<String>,
    connector_type: Option<ConnectorType>,
}

impl MonitorBuilder {
    fn new(display_number: u32) -> Self {
        Self {
            display_number,
            ..Self::default()
        }
    }

    fn update(&mut self, line: &str) {
        if let Some(value) = line.strip_prefix("I2C bus:") {
            if let Some(bus_number) = value.trim().split("i2c-").nth(1) {
                self.i2c_bus = bus_number.parse().ok();
            }
        } else if let Some(value) = line.strip_prefix("Mfg id:") {
            let trimmed = value.trim();
            self.manufacturer = trimmed
                .split(" - ")
                .next()
                .unwrap_or(trimmed)
                .trim()
                .to_string();
        } else if let Some(value) = line.strip_prefix("Model:") {
            self.model = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Serial number:") {
            if !line.contains("Binary serial number:") {
                self.serial = value.trim().to_string();
            }
        } else if let Some(value) = line.strip_prefix("DRM connector:") {
            let value = value.trim().to_string();
            self.connector_type = extract_connector_type(&value);
            self.drm_connector = Some(value);
        }
    }

    fn build(self) -> Result<MonitorInfo> {
        let i2c_bus = self
            .i2c_bus
            .ok_or_else(|| XtmonctlError::ParseError("missing i2c bus in detect output".into()))?;

        Ok(MonitorInfo {
            id: MonitorId {
                display_number: self.display_number,
                i2c_bus,
            },
            manufacturer: self.manufacturer,
            model: self.model,
            serial: self.serial,
            drm_connector: self.drm_connector,
            connector_type: self.connector_type,
        })
    }
}
