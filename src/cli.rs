use clap::{Parser, Subcommand};

use crate::app::App;
use crate::error::Result;
use crate::units::BrightnessPercent;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "xtmonctl",
    version,
    about = "External monitor brightness control via ddcutil"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    List,
    Get { monitor: String },
    Set { monitor: String, value: String },
    All { value: String },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let app = App::load()?;

    match cli.command {
        None => crate::tui::run(&app),
        Some(Commands::List) => list_monitors(&app),
        Some(Commands::Get { monitor }) => get_brightness(&app, &monitor),
        Some(Commands::Set { monitor, value }) => set_brightness(&app, &monitor, &value),
        Some(Commands::All { value }) => set_all_brightness(&app, &value),
    }
}

fn list_monitors(app: &App) -> Result<()> {
    let monitors = app.list_monitors()?;
    if monitors.is_empty() {
        println!("No external monitors detected.");
        return Ok(());
    }

    for monitor in monitors {
        let brightness = app.get_monitor_brightness(&monitor).ok();
        println!(
            "{}: {}",
            monitor.id.display_number,
            app.display_label(&monitor)
        );
        println!("   Bus: {}", monitor.id.bus_name());
        if let Some(brightness) = brightness {
            println!(
                "   Brightness: {}% ({}/{})",
                brightness.to_percent().value(),
                brightness.value,
                brightness.max
            );
        } else {
            println!("   Brightness: N/A");
        }
        if !monitor.serial.is_empty() {
            println!("   Serial: {}", monitor.serial);
        }
        println!();
    }

    Ok(())
}

fn get_brightness(app: &App, identifier: &str) -> Result<()> {
    let monitors = app.list_monitors()?;
    let monitor = app.find_monitor(&monitors, identifier)?;
    let brightness = app.get_monitor_brightness(monitor)?;
    println!("{}", brightness.to_percent().value());
    Ok(())
}

fn set_brightness(app: &App, identifier: &str, value: &str) -> Result<()> {
    let monitors = app.list_monitors()?;
    let monitor = app.find_monitor(&monitors, identifier)?;
    let target = parse_target_percent(app, monitor, value)?;
    app.set_monitor_brightness(monitor, target)?;
    println!(
        "Set {} brightness to {}%",
        app.display_label(monitor),
        target.value()
    );
    Ok(())
}

fn set_all_brightness(app: &App, value: &str) -> Result<()> {
    let monitors = app.list_monitors()?;
    if monitors.is_empty() {
        println!("No external monitors detected.");
        return Ok(());
    }

    for monitor in &monitors {
        let target = parse_target_percent(app, monitor, value)?;
        app.set_monitor_brightness(monitor, target)?;
        println!("Set {} to {}%", app.display_label(monitor), target.value());
    }

    Ok(())
}

fn parse_target_percent(
    app: &App,
    monitor: &crate::ddc::MonitorInfo,
    value: &str,
) -> Result<BrightnessPercent> {
    if value.starts_with('+') || value.starts_with('-') {
        let delta = value
            .parse::<i16>()
            .map_err(|_| crate::error::XtmonctlError::InvalidBrightness(value.to_string()))?;
        let current = app.get_monitor_brightness(monitor)?.to_percent();
        Ok(current.saturating_add(delta))
    } else {
        let percent = value
            .parse::<u8>()
            .map_err(|_| crate::error::XtmonctlError::InvalidBrightness(value.to_string()))?;
        BrightnessPercent::new(percent)
    }
}
