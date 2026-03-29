use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use serde::Serialize;

use crate::app::App;
use crate::error::Result;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "xtmonctl",
    version,
    about = "External monitor brightness control via ddcutil"
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    List,
    Get { monitor: String },
    Set { monitor: String, value: String },
    All { value: String },
    Alias(AliasCommand),
    Config(ConfigCommand),
}

#[derive(Debug, Clone, Args)]
pub struct AliasCommand {
    #[command(subcommand)]
    pub command: AliasSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AliasSubcommand {
    List,
    Set { monitor: String, alias: String },
    Clear { monitor: String },
}

#[derive(Debug, Clone, Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub command: ConfigSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigSubcommand {
    Path,
}

#[derive(Debug, Serialize)]
struct MonitorListEntry {
    display_number: u32,
    bus: String,
    display_name: String,
    alias: Option<String>,
    serial: String,
    brightness_percent: Option<u8>,
    brightness_raw: Option<u16>,
    brightness_max: Option<u16>,
}

#[derive(Debug, Serialize)]
struct BrightnessEntry {
    monitor: String,
    bus: String,
    percent: u8,
    raw: u16,
    max: u16,
}

#[derive(Debug, Serialize)]
struct AliasEntry {
    bus: String,
    alias: String,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let app = match cli.config.clone() {
        Some(path) => App::load_with_path(path)?,
        None => App::load()?,
    };

    match cli.command {
        None => crate::tui::run(&app),
        Some(Commands::List) => list_monitors(&app, cli.json),
        Some(Commands::Get { monitor }) => get_brightness(&app, &monitor, cli.json),
        Some(Commands::Set { monitor, value }) => set_brightness(&app, &monitor, &value, cli.json),
        Some(Commands::All { value }) => set_all_brightness(&app, &value, cli.json),
        Some(Commands::Alias(command)) => run_alias_command(&app, command, cli.json),
        Some(Commands::Config(command)) => run_config_command(&app, command, cli.json),
    }
}

fn list_monitors(app: &App, json: bool) -> Result<()> {
    let monitors = app.list_monitors()?;
    if monitors.is_empty() {
        if json {
            print_json(&Vec::<MonitorListEntry>::new())?;
            return Ok(());
        }
        println!("No external monitors detected.");
        return Ok(());
    }

    let entries = monitors
        .iter()
        .map(|monitor| {
            let brightness = app.get_monitor_brightness(monitor).ok();
            MonitorListEntry {
                display_number: monitor.id.display_number,
                bus: monitor.id.bus_name(),
                display_name: monitor.display_name(),
                alias: app.alias_for(monitor),
                serial: monitor.serial.clone(),
                brightness_percent: brightness.map(|value| value.to_percent().value()),
                brightness_raw: brightness.map(|value| value.value),
                brightness_max: brightness.map(|value| value.max),
            }
        })
        .collect::<Vec<_>>();

    if json {
        return print_json(&entries);
    }

    for entry in entries {
        let display_name = match entry.alias.as_deref() {
            Some(alias) => format!("{alias} [{}]", entry.display_name),
            None => entry.display_name,
        };
        println!("{}: {}", entry.display_number, display_name);
        println!("   Bus: {}", entry.bus);
        if let (Some(percent), Some(raw), Some(max)) = (
            entry.brightness_percent,
            entry.brightness_raw,
            entry.brightness_max,
        ) {
            println!("   Brightness: {}% ({}/{})", percent, raw, max);
        } else {
            println!("   Brightness: N/A");
        }
        if !entry.serial.is_empty() {
            println!("   Serial: {}", entry.serial);
        }
        println!();
    }

    Ok(())
}

fn get_brightness(app: &App, identifier: &str, json: bool) -> Result<()> {
    let monitors = app.list_monitors()?;
    let monitor = app.find_monitor(&monitors, identifier)?;
    let brightness = app.get_monitor_brightness(monitor)?;
    if json {
        return print_json(&BrightnessEntry {
            monitor: app.display_label(monitor),
            bus: monitor.id.bus_name(),
            percent: brightness.to_percent().value(),
            raw: brightness.value,
            max: brightness.max,
        });
    }
    println!("{}", brightness.to_percent().value());
    Ok(())
}

fn set_brightness(app: &App, identifier: &str, value: &str, json: bool) -> Result<()> {
    let monitors = app.list_monitors()?;
    let monitor = app.find_monitor(&monitors, identifier)?;
    let target = app.resolve_target_percent(monitor, value)?;
    let raw = app.set_monitor_brightness(monitor, target)?;
    if json {
        return print_json(&BrightnessEntry {
            monitor: app.display_label(monitor),
            bus: monitor.id.bus_name(),
            percent: target.value(),
            raw: raw.value,
            max: raw.max,
        });
    }
    println!(
        "Set {} brightness to {}%",
        app.display_label(monitor),
        target.value()
    );
    Ok(())
}

fn set_all_brightness(app: &App, value: &str, json: bool) -> Result<()> {
    let results = app.set_all_from_value(value)?;
    if results.is_empty() {
        if json {
            print_json(&Vec::<BrightnessEntry>::new())?;
            return Ok(());
        }
        println!("No external monitors detected.");
        return Ok(());
    }

    if json {
        let payload = results
            .into_iter()
            .map(|(monitor, raw, target)| BrightnessEntry {
                monitor: app.display_label(&monitor),
                bus: monitor.id.bus_name(),
                percent: target.value(),
                raw: raw.value,
                max: raw.max,
            })
            .collect::<Vec<_>>();
        return print_json(&payload);
    }

    for (monitor, _raw, target) in results {
        println!("Set {} to {}%", app.display_label(&monitor), target.value());
    }

    Ok(())
}

fn run_alias_command(app: &App, command: AliasCommand, json: bool) -> Result<()> {
    match command.command {
        AliasSubcommand::List => list_aliases(app, json),
        AliasSubcommand::Set { monitor, alias } => set_alias(app, &monitor, &alias, json),
        AliasSubcommand::Clear { monitor } => clear_alias(app, &monitor, json),
    }
}

fn run_config_command(app: &App, command: ConfigCommand, json: bool) -> Result<()> {
    match command.command {
        ConfigSubcommand::Path => {
            let path = app.config_path().display().to_string();
            if json {
                return print_json(&serde_json::json!({ "config_path": path }));
            }
            println!("{path}");
            Ok(())
        }
    }
}

fn list_aliases(app: &App, json: bool) -> Result<()> {
    let entries = app
        .list_aliases()
        .into_iter()
        .map(|(bus, alias)| AliasEntry { bus, alias })
        .collect::<Vec<_>>();

    if json {
        return print_json(&entries);
    }

    if entries.is_empty() {
        println!("No monitor aliases configured.");
        return Ok(());
    }

    for entry in entries {
        println!("{}: {}", entry.bus, entry.alias);
    }
    Ok(())
}

fn set_alias(app: &App, identifier: &str, alias: &str, json: bool) -> Result<()> {
    let monitors = app.list_monitors()?;
    let monitor = app.find_monitor(&monitors, identifier)?;
    app.set_monitor_alias(monitor, alias)?;

    if json {
        return print_json(&AliasEntry {
            bus: monitor.id.bus_name(),
            alias: alias.to_string(),
        });
    }

    println!("Set alias for {} to {}", monitor.id.bus_name(), alias);
    Ok(())
}

fn clear_alias(app: &App, identifier: &str, json: bool) -> Result<()> {
    let monitors = app.list_monitors()?;
    let monitor = app.find_monitor(&monitors, identifier)?;
    let removed = app.clear_monitor_alias(monitor)?;

    if json {
        return print_json(&serde_json::json!({
            "bus": monitor.id.bus_name(),
            "removed": removed
        }));
    }

    if removed {
        println!("Cleared alias for {}", monitor.id.bus_name());
    } else {
        println!("No alias set for {}", monitor.id.bus_name());
    }
    Ok(())
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let encoded = serde_json::to_string_pretty(value)
        .map_err(|error| crate::error::XtmonctlError::State(error.to_string()))?;
    println!("{encoded}");
    Ok(())
}
