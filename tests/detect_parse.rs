use xtmonctl::ddc::{parse_detect_output, ConnectorType};

#[test]
fn parses_single_monitor() {
    let output = "Display 1\n   I2C bus:  /dev/i2c-4\n   DRM connector:           card1-HDMI-A-1\n   EDID synopsis:\n      Mfg id:               MSI - Microstep\n      Model:                MSI MP223\n      Serial number:        PB9H163C00508\n";
    let monitors = parse_detect_output(output).unwrap();
    assert_eq!(monitors.len(), 1);
    assert_eq!(monitors[0].id.i2c_bus, 4);
    assert_eq!(monitors[0].manufacturer, "MSI");
    assert_eq!(monitors[0].connector_type, Some(ConnectorType::Hdmi));
}

#[test]
fn skips_invalid_displays() {
    let output = "Display 1\n   I2C bus:  /dev/i2c-4\n   EDID synopsis:\n      Mfg id:               MSI - Microstep\n      Model:                MSI MP223\n\nInvalid display\n   I2C bus:  /dev/i2c-12\n";
    let monitors = parse_detect_output(output).unwrap();
    assert_eq!(monitors.len(), 1);
    assert_eq!(monitors[0].id.display_number, 1);
}
