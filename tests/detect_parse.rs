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

#[test]
fn parses_multiple_monitors() {
    let output = "Display 1\n   I2C bus:  /dev/i2c-4\n   EDID synopsis:\n      Mfg id:               DEL - Dell Inc.\n      Model:                U2722D\n\nDisplay 2\n   I2C bus:  /dev/i2c-7\n   DRM connector:           card1-DP-1\n   EDID synopsis:\n      Mfg id:               SAM - Samsung\n      Model:                S27A\n";
    let monitors = parse_detect_output(output).unwrap();
    assert_eq!(monitors.len(), 2);
    assert_eq!(monitors[1].id.i2c_bus, 7);
    assert_eq!(monitors[1].connector_type, Some(ConnectorType::DisplayPort));
}

#[test]
fn skips_malformed_display_blocks_instead_of_failing_all_detection() {
    let output = "Display 1\n   Model: Missing Bus\n\nDisplay 2\n   I2C bus:  /dev/i2c-9\n   EDID synopsis:\n      Mfg id:               MSI - Microstep\n      Model:                G274Q\n";
    let monitors = parse_detect_output(output).unwrap();
    assert_eq!(monitors.len(), 1);
    assert_eq!(monitors[0].id.display_number, 2);
    assert_eq!(monitors[0].id.i2c_bus, 9);
}
