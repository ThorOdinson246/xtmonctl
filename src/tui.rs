use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, ExecutableCommand};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap};
use ratatui::Terminal;

use crate::app::App;
use crate::ddc::MonitorInfo;
use crate::error::{Result, XtmonctlError};
use crate::units::BrightnessPercent;

pub fn run(app: &App) -> Result<()> {
    enable_raw_mode().map_err(io_error)?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(io_error)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(io_error)?;

    let run_result = run_loop(&mut terminal, app);

    disable_raw_mode().map_err(io_error)?;
    io::stdout()
        .execute(LeaveAlternateScreen)
        .map_err(io_error)?;
    run_result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &App) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut state = TuiState {
        loading: true,
        message: "Refreshing monitors...".into(),
        ..TuiState::default()
    };
    request_refresh(app.clone(), tx.clone());

    loop {
        handle_messages(&mut state, &rx);
        terminal
            .draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(8),
                        Constraint::Length(5),
                        Constraint::Length(3),
                    ])
                    .split(frame.area());

                let detail_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(chunks[1]);

                let items = state
                    .monitors
                    .iter()
                    .enumerate()
                    .map(|(index, monitor)| {
                        let style = if index == state.selected {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        let details = state.row_status(monitor.id.i2c_bus);
                        ListItem::new(Line::from(vec![Span::styled(
                            format!(
                                "{} ({}) - {}",
                                monitor.label,
                                monitor.id.bus_name(),
                                details
                            ),
                            style,
                        )]))
                    })
                    .collect::<Vec<_>>();

                let list = List::new(items)
                    .block(Block::default().title("Monitors").borders(Borders::ALL));
                frame.render_widget(list, chunks[0]);

                let detail_text = selected_detail(&state);
                let detail = Paragraph::new(detail_text)
                    .block(Block::default().title("Selected").borders(Borders::ALL));
                frame.render_widget(detail, detail_chunks[0]);

                let gauge = match state.selected_brightness() {
                    Some(raw) => Gauge::default()
                        .block(Block::default().title("Brightness").borders(Borders::ALL))
                        .gauge_style(Style::default().fg(Color::Cyan))
                        .percent(raw.to_percent().value() as u16)
                        .label(format!(
                            "{}% ({}/{})",
                            raw.to_percent().value(),
                            raw.value,
                            raw.max
                        )),
                    None => Gauge::default()
                        .block(Block::default().title("Brightness").borders(Borders::ALL))
                        .gauge_style(Style::default().fg(Color::DarkGray))
                        .percent(0)
                        .label("Unavailable"),
                };
                frame.render_widget(gauge, detail_chunks[1]);

                let help = Paragraph::new(state.help_text())
                    .block(Block::default().title("Status").borders(Borders::ALL))
                    .wrap(Wrap { trim: true });
                frame.render_widget(help, chunks[2]);
            })
            .map_err(io_error)?;

        if event::poll(Duration::from_millis(250)).map_err(io_error)? {
            let Event::Key(key) = event::read().map_err(io_error)? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('?') => state.show_help = !state.show_help,
                KeyCode::Char('r') => {
                    state.loading = true;
                    state.message = "Refreshing monitors...".into();
                    request_refresh(app.clone(), tx.clone());
                }
                KeyCode::Char('j') | KeyCode::Down if !state.monitors.is_empty() => {
                    state.selected = (state.selected + 1) % state.monitors.len();
                }
                KeyCode::Char('k') | KeyCode::Up if !state.monitors.is_empty() => {
                    state.selected =
                        (state.selected + state.monitors.len() - 1) % state.monitors.len();
                }
                KeyCode::Char('h') | KeyCode::Left => request_adjust(
                    app.clone(),
                    tx.clone(),
                    &mut state,
                    -(app.default_step_percent() as i16),
                ),
                KeyCode::Char('l') | KeyCode::Right => request_adjust(
                    app.clone(),
                    tx.clone(),
                    &mut state,
                    app.default_step_percent() as i16,
                ),
                KeyCode::Char('H') => request_adjust(
                    app.clone(),
                    tx.clone(),
                    &mut state,
                    -(app.large_step_percent() as i16),
                ),
                KeyCode::Char('L') => request_adjust(
                    app.clone(),
                    tx.clone(),
                    &mut state,
                    app.large_step_percent() as i16,
                ),
                KeyCode::Char(digit) if digit.is_ascii_digit() => {
                    let value = if digit == '0' {
                        100
                    } else {
                        digit.to_digit(10).unwrap_or(0) as u8 * 10
                    };
                    request_set(app.clone(), tx.clone(), &mut state, value);
                }
                _ => {}
            }
        }
    }
}

fn io_error(error: io::Error) -> XtmonctlError {
    XtmonctlError::CommandFailed {
        command: "terminal".into(),
        message: error.to_string(),
    }
}

#[derive(Debug, Clone)]
struct MonitorRow {
    id: crate::ddc::MonitorId,
    label: String,
    serial: String,
}

#[derive(Debug, Default)]
struct TuiState {
    monitors: Vec<MonitorRow>,
    brightness: std::collections::HashMap<u32, crate::units::BrightnessRaw>,
    brightness_errors: std::collections::HashMap<u32, String>,
    selected: usize,
    message: String,
    show_help: bool,
    loading: bool,
}

impl TuiState {
    fn selected_monitor(&self) -> Option<&MonitorRow> {
        self.monitors.get(self.selected)
    }

    fn selected_brightness(&self) -> Option<crate::units::BrightnessRaw> {
        self.selected_monitor()
            .and_then(|monitor| self.brightness.get(&monitor.id.i2c_bus).copied())
    }

    fn help_text(&self) -> String {
        if self.show_help {
            "j/k or arrows: select  h/l: adjust  H/L: large adjust  0-9: preset  r: refresh  q: quit".into()
        } else if self.loading {
            "Refreshing monitors...".into()
        } else if self.message.is_empty() {
            "Press ? for help".into()
        } else {
            self.message.clone()
        }
    }

    fn row_status(&self, bus: u32) -> String {
        if let Some(raw) = self.brightness.get(&bus) {
            format!("{}% ({}/{})", raw.to_percent().value(), raw.value, raw.max)
        } else if let Some(error) = self.brightness_errors.get(&bus) {
            format!("ERR: {error}")
        } else {
            "loading".to_string()
        }
    }
}

enum UiMessage {
    Refreshed {
        monitors: Vec<MonitorRow>,
        brightness: Vec<(u32, crate::units::BrightnessRaw)>,
        brightness_errors: Vec<(u32, String)>,
        message: String,
    },
    Updated {
        bus: u32,
        brightness: crate::units::BrightnessRaw,
        message: String,
    },
    Error(String),
}

fn selected_detail(state: &TuiState) -> String {
    match state.selected_monitor() {
        Some(monitor) => {
            let serial = if monitor.serial.is_empty() {
                "Serial: n/a".to_string()
            } else {
                format!("Serial: {}", monitor.serial)
            };
            format!("{}\n{}\n{}", monitor.label, monitor.id.bus_name(), serial)
        }
        None => "No monitors detected.".into(),
    }
}

fn handle_messages(state: &mut TuiState, rx: &Receiver<UiMessage>) {
    while let Ok(message) = rx.try_recv() {
        match message {
            UiMessage::Refreshed {
                monitors,
                brightness,
                brightness_errors,
                message,
            } => {
                state.monitors = monitors;
                state.brightness = brightness.into_iter().collect();
                state.brightness_errors = brightness_errors.into_iter().collect();
                state.selected = state.selected.min(state.monitors.len().saturating_sub(1));
                state.message = message;
                state.loading = false;
            }
            UiMessage::Updated {
                bus,
                brightness,
                message,
            } => {
                state.brightness.insert(bus, brightness);
                state.brightness_errors.remove(&bus);
                state.message = message;
                state.loading = false;
            }
            UiMessage::Error(message) => {
                state.message = message;
                state.loading = false;
            }
        }
    }
}

fn request_refresh(app: App, tx: Sender<UiMessage>) {
    thread::spawn(move || match app.list_monitors() {
        Ok(monitors) => {
            let mut rows = Vec::new();
            let mut brightness = Vec::new();
            let mut brightness_errors = Vec::new();
            for monitor in monitors {
                let bus = monitor.id.i2c_bus;
                rows.push(MonitorRow {
                    id: monitor.id,
                    label: app.display_label(&monitor),
                    serial: monitor.serial.clone(),
                });
                match app.get_monitor_brightness(&monitor) {
                    Ok(raw) => brightness.push((bus, raw)),
                    Err(error) => brightness_errors.push((bus, error.to_string())),
                }
            }
            let _ = tx.send(UiMessage::Refreshed {
                monitors: rows,
                brightness,
                brightness_errors,
                message: "Refreshed monitor list".into(),
            });
        }
        Err(error) => {
            let _ = tx.send(UiMessage::Error(error.to_string()));
        }
    });
}

fn request_adjust(app: App, tx: Sender<UiMessage>, state: &mut TuiState, delta: i16) {
    let Some(row) = state.selected_monitor().cloned() else {
        return;
    };
    let current = state
        .brightness
        .get(&row.id.i2c_bus)
        .copied()
        .map(|raw| raw.to_percent())
        .or_else(|| app.last_brightness_for_bus(row.id.i2c_bus))
        .unwrap_or_else(default_percent);
    let target = current.saturating_add(delta);
    state.loading = true;
    state.message = format!("Setting {} to {}%...", row.label, target.value());
    request_update_bus(app, tx, row.id.i2c_bus, target);
}

fn request_set(app: App, tx: Sender<UiMessage>, state: &mut TuiState, value: u8) {
    let Some(row) = state.selected_monitor().cloned() else {
        return;
    };
    if let Ok(target) = BrightnessPercent::new(value) {
        state.loading = true;
        state.message = format!("Setting {} to {}%...", row.label, target.value());
        request_update_bus(app, tx, row.id.i2c_bus, target);
    }
}

fn request_update_bus(app: App, tx: Sender<UiMessage>, bus: u32, target: BrightnessPercent) {
    thread::spawn(move || match update_monitor_by_bus(&app, bus, target) {
        Ok((monitor, raw)) => {
            let _ = tx.send(UiMessage::Updated {
                bus,
                brightness: raw,
                message: format!("Set {} to {}%", app.display_label(&monitor), target.value()),
            });
        }
        Err(error) => {
            let _ = tx.send(UiMessage::Error(error.to_string()));
        }
    });
}

fn update_monitor_by_bus(
    app: &App,
    bus: u32,
    target: BrightnessPercent,
) -> Result<(MonitorInfo, crate::units::BrightnessRaw)> {
    let monitors = app.list_monitors()?;
    let monitor = monitors
        .into_iter()
        .find(|monitor| monitor.id.i2c_bus == bus)
        .ok_or_else(|| XtmonctlError::MonitorNotFound(format!("i2c-{bus}")))?;
    let raw = app.set_monitor_brightness(&monitor, target)?;
    Ok((monitor, raw))
}

fn default_percent() -> BrightnessPercent {
    match BrightnessPercent::new(5) {
        Ok(percent) => percent,
        Err(_) => unreachable!("built-in default percent is valid"),
    }
}
