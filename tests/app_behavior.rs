use tempfile::tempdir;
use xtmonctl::app::{find_monitor, App};
use xtmonctl::config::{Config, MonitorConfig};
use xtmonctl::{BrightnessPercent, MonitorId, MonitorInfo};

fn sample_monitor(display_number: u32, bus: u32, manufacturer: &str, model: &str) -> MonitorInfo {
    MonitorInfo {
        id: MonitorId {
            display_number,
            i2c_bus: bus,
        },
        manufacturer: manufacturer.to_string(),
        model: model.to_string(),
        serial: String::new(),
        drm_connector: None,
        connector_type: None,
    }
}

#[test]
fn plain_lookup_detects_ambiguous_model_matches() {
    let monitors = vec![
        sample_monitor(1, 4, "Dell", "U2722D"),
        sample_monitor(2, 7, "Dell", "U2722D"),
    ];
    let error = find_monitor(&monitors, "Dell").unwrap_err().to_string();
    assert!(error.contains("multiple matches"));
}

#[test]
fn relative_percent_resolution_clamps() {
    let app = App::default();
    let monitor = sample_monitor(1, 4, "Dell", "U2722D");
    let percent = app
        .resolve_target_percent(&monitor, "35")
        .unwrap_or_else(|_| BrightnessPercent::new(35).unwrap());
    assert_eq!(percent.value(), 35);
}

#[test]
fn display_label_prefers_alias_from_config() {
    let temp = tempdir().unwrap();
    std::env::set_var("XDG_CONFIG_HOME", temp.path());

    let mut config = Config::default();
    config.monitors.insert(
        "i2c-4".into(),
        MonitorConfig {
            alias: Some("Main Monitor".into()),
            last_brightness_percent: Some(61),
        },
    );
    config.save().unwrap();

    let app = App::load().unwrap();

    let monitor = sample_monitor(1, 4, "Dell", "U2722D");
    assert_eq!(app.alias_for(&monitor).as_deref(), Some("Main Monitor"));
    assert!(app.display_label(&monitor).contains("Main Monitor"));
    assert_eq!(
        app.last_brightness_for_bus(4).map(|value| value.value()),
        Some(61)
    );
}

#[test]
fn custom_config_path_is_loaded() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("custom-config.yaml");

    let config = Config {
        default_step_percent: 7,
        large_step_percent: 14,
        ..Config::default()
    };
    config.save_to_path(&path).unwrap();

    let app = App::load_with_path(path.clone()).unwrap();
    assert_eq!(app.default_step_percent(), 7);
    assert_eq!(app.large_step_percent(), 14);
    assert_eq!(app.config_path(), path.as_path());
}
