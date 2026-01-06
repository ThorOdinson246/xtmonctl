use xtmonctl::{parse_getvcp_output, BrightnessPercent, BrightnessRaw};

#[test]
fn parses_standard_brightness_output() {
    let brightness = parse_getvcp_output("VCP 10 C 50 100").unwrap();
    assert_eq!(brightness, BrightnessRaw { value: 50, max: 100 });
    assert_eq!(brightness.to_percent().value(), 50);
}

#[test]
fn converts_non_hundred_max_honestly() {
    let brightness = BrightnessRaw::new(128, 255).unwrap();
    assert_eq!(brightness.to_percent().value(), 50);
}

#[test]
fn converts_percent_to_raw() {
    let percent = BrightnessPercent::new(50).unwrap();
    assert_eq!(percent.to_raw(255), BrightnessRaw { value: 128, max: 255 });
}

#[test]
fn clamps_relative_changes() {
    let percent = BrightnessPercent::new(3).unwrap();
    assert_eq!(percent.saturating_add(-10).value(), 0);
    assert_eq!(percent.saturating_add(200).value(), 100);
}
