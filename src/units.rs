use crate::error::{Result, XtmonctlError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrightnessPercent(u8);

impl BrightnessPercent {
    pub fn new(value: u8) -> Result<Self> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err(XtmonctlError::InvalidBrightness(value.to_string()))
        }
    }

    pub fn value(self) -> u8 {
        self.0
    }

    pub fn saturating_add(self, delta: i16) -> Self {
        let next = (i16::from(self.0) + delta).clamp(0, 100) as u8;
        Self(next)
    }

    pub fn to_raw(self, max: u16) -> BrightnessRaw {
        let scaled = ((u32::from(self.0) * u32::from(max)) + 50) / 100;
        BrightnessRaw {
            value: scaled as u16,
            max,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrightnessRaw {
    pub value: u16,
    pub max: u16,
}

impl BrightnessRaw {
    pub fn new(value: u16, max: u16) -> Result<Self> {
        if max == 0 || value > max {
            return Err(XtmonctlError::InvalidBrightness(format!("{value}/{max}")));
        }
        Ok(Self { value, max })
    }

    pub fn to_percent(self) -> BrightnessPercent {
        let scaled = ((u32::from(self.value) * 100) + (u32::from(self.max) / 2)) / u32::from(self.max);
        BrightnessPercent(scaled.min(100) as u8)
    }
}
