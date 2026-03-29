use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct MonitorId {
    pub display_number: u32,
    pub i2c_bus: u32,
}

impl MonitorId {
    pub fn bus_name(self) -> String {
        format!("i2c-{}", self.i2c_bus)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ConnectorType {
    Hdmi,
    DisplayPort,
    Edp,
    Dvi,
    Vga,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MonitorInfo {
    pub id: MonitorId,
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    pub drm_connector: Option<String>,
    pub connector_type: Option<ConnectorType>,
}

impl MonitorInfo {
    pub fn display_name(&self) -> String {
        let name = format!("{} {}", self.manufacturer, self.model);
        let trimmed = name.trim();
        if trimmed.is_empty() {
            format!("Display {}", self.id.display_number)
        } else {
            trimmed.to_string()
        }
    }
}
