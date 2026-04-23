use std::collections::HashMap;
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
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap};
use ratatui::Terminal;

use crate::app::App;
use crate::ddc::MonitorInfo;
use crate::error::{Result, XtmonctlError};
use crate::units::{BrightnessPercent, BrightnessRaw};

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
        generation: 1,
        theme: ThemeKind::from_config_name(&app.tui_theme()),
        ..TuiState::default()
    };
    request_refresh(app.clone(), tx.clone(), state.generation);

    loop {
        handle_messages(&mut state, &rx);
        terminal
            .draw(|frame| {
                let theme = state.theme.palette();
                let footer_height = if state.command_mode {
                    10
                } else if state.active_panel.is_some() {
                    8
                } else {
                    1
                };
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(10),
                        Constraint::Length(footer_height),
                    ])
                    .split(frame.area());

                render_header(frame, chunks[0], &state, theme);

                let main_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
                    .split(chunks[1]);

                let detail_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                    .split(main_chunks[1]);

                let items = state
                    .monitors
                    .iter()
                    .enumerate()
                    .map(|(index, monitor)| {
                        let details = state.row_status(monitor.info.id.i2c_bus);
                        let line_style = if index == state.selected {
                            theme.selected_line
                        } else {
                            theme.primary_text
                        };
                        let meta_style = if index == state.selected {
                            theme.selected_meta
                        } else {
                            theme.muted_text
                        };

                        ListItem::new(vec![
                            Line::from(vec![Span::styled(
                                format!(
                                    "{}  {}",
                                    selection_marker(index == state.selected),
                                    monitor.label
                                ),
                                line_style,
                            )]),
                            Line::from(vec![Span::styled(
                                format!("  {}  {}", monitor.info.id.bus_name(), details),
                                meta_style,
                            )]),
                        ])
                    })
                    .collect::<Vec<_>>();

                let list = List::new(items).block(panel_block(" Monitor List ", theme));
                frame.render_widget(list, main_chunks[0]);

                let detail = Paragraph::new(selected_detail(&state))
                    .block(panel_block(" Selected Monitor ", theme))
                    .wrap(Wrap { trim: true });
                frame.render_widget(detail, detail_chunks[0]);

                let gauge = match state.selected_brightness() {
                    Some(raw) => Gauge::default()
                        .block(panel_block(" Brightness ", theme))
                        .gauge_style(theme.gauge)
                        .percent(raw.to_percent().value() as u16)
                        .label(format!(
                            "{}% ({}/{})",
                            raw.to_percent().value(),
                            raw.value,
                            raw.max
                        )),
                    None => Gauge::default()
                        .block(panel_block(" Brightness ", theme))
                        .gauge_style(theme.dim_gauge)
                        .percent(0)
                        .label("Unavailable"),
                };
                frame.render_widget(gauge, detail_chunks[1]);

                render_footer(frame, chunks[2], &state, theme);
            })
            .map_err(io_error)?;

        if event::poll(Duration::from_millis(250)).map_err(io_error)? {
            let Event::Key(key) = event::read().map_err(io_error)? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if state.command_mode {
                match key.code {
                    KeyCode::Esc => state.close_command_palette(),
                    KeyCode::Enter => state.run_command(app),
                    KeyCode::Tab => state.autocomplete_command(),
                    KeyCode::Up => state.move_command_selection(-1),
                    KeyCode::Down => state.move_command_selection(1),
                    KeyCode::Backspace => {
                        state.command_input.pop();
                        state.command_selected = 0;
                        state.adjust_command_scroll(
                            filtered_palette_commands(&state.command_input).len(),
                        );
                    }
                    KeyCode::Char(ch) => {
                        state.command_input.push(ch);
                        state.command_selected = 0;
                        state.adjust_command_scroll(
                            filtered_palette_commands(&state.command_input).len(),
                        );
                    }
                    _ => {}
                }
                continue;
            }

            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Esc => state.active_panel = None,
                KeyCode::Char('?') => state.active_panel = Some(BottomPanel::Help),
                KeyCode::Char('/') => state.open_command_palette_with_slash(),
                KeyCode::Tab => state.open_command_palette(),
                KeyCode::Char('t') => cycle_theme(app, &mut state),
                KeyCode::Char('r') => {
                    state.loading = true;
                    state.message = "Refreshing monitors...".into();
                    state.generation += 1;
                    request_refresh(app.clone(), tx.clone(), state.generation);
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
    info: MonitorInfo,
    label: String,
}

#[derive(Debug)]
struct TuiState {
    monitors: Vec<MonitorRow>,
    brightness: HashMap<u32, BrightnessRaw>,
    brightness_errors: HashMap<u32, String>,
    selected: usize,
    message: String,
    loading: bool,
    generation: u64,
    theme: ThemeKind,
    active_panel: Option<BottomPanel>,
    command_mode: bool,
    command_input: String,
    command_selected: usize,
    command_scroll: usize,
}

impl Default for TuiState {
    fn default() -> Self {
        Self {
            monitors: Vec::new(),
            brightness: HashMap::new(),
            brightness_errors: HashMap::new(),
            selected: 0,
            message: String::new(),
            loading: false,
            generation: 0,
            theme: ThemeKind::Ocean,
            active_panel: None,
            command_mode: false,
            command_input: String::new(),
            command_selected: 0,
            command_scroll: 0,
        }
    }
}

impl TuiState {
    fn selected_monitor(&self) -> Option<&MonitorRow> {
        self.monitors.get(self.selected)
    }

    fn selected_brightness(&self) -> Option<BrightnessRaw> {
        self.selected_monitor()
            .and_then(|monitor| self.brightness.get(&monitor.info.id.i2c_bus).copied())
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

    fn status_style(&self) -> Style {
        let palette = self.theme.palette();
        if self.loading {
            palette.warning
        } else if self.message.starts_with("Set ") || self.message.starts_with("Refreshed") {
            palette.success
        } else if self.message.starts_with("Setting ") {
            palette.accent
        } else if self.message.starts_with("Error") || self.message.contains("not found") {
            palette.error
        } else {
            palette.border
        }
    }

    fn active_panel_style(&self) -> Style {
        if self.active_panel == Some(BottomPanel::Status) {
            self.status_style()
        } else {
            self.theme.palette().border
        }
    }

    fn open_command_palette(&mut self) {
        self.command_mode = true;
        if self.command_input.is_empty() {
            self.command_input.push('/');
        }
        self.command_selected = 0;
        self.command_scroll = 0;
    }

    fn open_command_palette_with_slash(&mut self) {
        self.command_mode = true;
        self.command_input.clear();
        self.command_input.push('/');
        self.command_selected = 0;
        self.command_scroll = 0;
    }

    fn close_command_palette(&mut self) {
        self.command_mode = false;
        self.command_input.clear();
        self.command_selected = 0;
        self.command_scroll = 0;
    }

    fn autocomplete_command(&mut self) {
        let options = filtered_palette_commands(&self.command_input);
        if options.is_empty() {
            return;
        }
        let index = self.command_selected.min(options.len().saturating_sub(1));
        self.command_input = options[index].command.to_string();
        self.command_selected = index;
        self.adjust_command_scroll(options.len());
    }

    fn move_command_selection(&mut self, delta: isize) {
        let options = filtered_palette_commands(&self.command_input);
        if options.is_empty() {
            self.command_selected = 0;
            self.command_scroll = 0;
            return;
        }
        let len = options.len() as isize;
        let next = (self.command_selected as isize + delta).rem_euclid(len);
        self.command_selected = next as usize;
        self.adjust_command_scroll(options.len());
    }

    fn adjust_command_scroll(&mut self, total: usize) {
        let visible = 5usize;
        if total <= visible {
            self.command_scroll = 0;
            return;
        }
        if self.command_selected < self.command_scroll {
            self.command_scroll = self.command_selected;
        } else if self.command_selected >= self.command_scroll + visible {
            self.command_scroll = self.command_selected + 1 - visible;
        }
    }

    fn run_command(&mut self, app: &App) {
        match selected_palette_command(&self.command_input, self.command_selected) {
            Ok(PaletteCommand::Show(panel)) => {
                self.active_panel = Some(panel);
                self.message = format!("Opened {}", panel.title().trim());
            }
            Ok(PaletteCommand::Hide) => {
                self.active_panel = None;
                self.message = "Closed bottom panel".into();
            }
            Ok(PaletteCommand::ThemeNext) => {
                self.theme = self.theme.next();
                self.message = format!("Theme switched to {}", self.theme.display_name());
                let _ = app.set_tui_theme(self.theme.config_name());
            }
            Err(message) => {
                self.message = message;
            }
        }
        self.close_command_palette();
    }
}

enum UiMessage {
    Refreshed {
        generation: u64,
        monitors: Vec<MonitorRow>,
        brightness: Vec<(u32, BrightnessRaw)>,
        brightness_errors: Vec<(u32, String)>,
        message: String,
    },
    Updated {
        generation: u64,
        bus: u32,
        brightness: BrightnessRaw,
        message: String,
    },
    Error {
        generation: u64,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BottomPanel {
    Status,
    Controls,
    Presets,
    Help,
}

impl BottomPanel {
    fn title(self) -> &'static str {
        match self {
            Self::Status => " Status ",
            Self::Controls => " Controls ",
            Self::Presets => " Presets ",
            Self::Help => " Help ",
        }
    }
}

#[derive(Clone, Copy)]
enum PaletteCommand {
    Show(BottomPanel),
    Hide,
    ThemeNext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeKind {
    Ocean,
    Forest,
    Ember,
}

impl ThemeKind {
    fn from_config_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "forest" => Self::Forest,
            "ember" => Self::Ember,
            _ => Self::Ocean,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Ocean => Self::Forest,
            Self::Forest => Self::Ember,
            Self::Ember => Self::Ocean,
        }
    }

    fn config_name(self) -> &'static str {
        match self {
            Self::Ocean => "ocean",
            Self::Forest => "forest",
            Self::Ember => "ember",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Ocean => "Ocean",
            Self::Forest => "Forest",
            Self::Ember => "Ember",
        }
    }

    fn palette(self) -> Palette {
        match self {
            Self::Ocean => Palette {
                header_bg: Color::Rgb(14, 28, 44),
                header_fg: Color::Rgb(180, 224, 255),
                border: Style::default().fg(Color::Rgb(63, 109, 165)),
                primary_text: Style::default().fg(Color::Rgb(228, 239, 249)),
                muted_text: Style::default().fg(Color::Rgb(129, 155, 178)),
                selected_line: Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(126, 213, 255))
                    .add_modifier(Modifier::BOLD),
                selected_meta: Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(90, 177, 222)),
                gauge: Style::default().fg(Color::Rgb(76, 201, 240)),
                dim_gauge: Style::default().fg(Color::Rgb(56, 72, 89)),
                accent: Style::default().fg(Color::Rgb(88, 200, 255)),
                success: Style::default().fg(Color::Rgb(101, 210, 161)),
                warning: Style::default().fg(Color::Rgb(247, 200, 92)),
                error: Style::default().fg(Color::Rgb(255, 132, 132)),
            },
            Self::Forest => Palette {
                header_bg: Color::Rgb(18, 33, 24),
                header_fg: Color::Rgb(210, 239, 189),
                border: Style::default().fg(Color::Rgb(74, 124, 89)),
                primary_text: Style::default().fg(Color::Rgb(232, 241, 228)),
                muted_text: Style::default().fg(Color::Rgb(141, 165, 146)),
                selected_line: Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(163, 208, 134))
                    .add_modifier(Modifier::BOLD),
                selected_meta: Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(123, 174, 99)),
                gauge: Style::default().fg(Color::Rgb(126, 200, 129)),
                dim_gauge: Style::default().fg(Color::Rgb(61, 77, 64)),
                accent: Style::default().fg(Color::Rgb(163, 208, 134)),
                success: Style::default().fg(Color::Rgb(142, 219, 134)),
                warning: Style::default().fg(Color::Rgb(227, 204, 111)),
                error: Style::default().fg(Color::Rgb(244, 126, 105)),
            },
            Self::Ember => Palette {
                header_bg: Color::Rgb(42, 20, 16),
                header_fg: Color::Rgb(255, 220, 186),
                border: Style::default().fg(Color::Rgb(177, 96, 68)),
                primary_text: Style::default().fg(Color::Rgb(248, 236, 228)),
                muted_text: Style::default().fg(Color::Rgb(179, 147, 134)),
                selected_line: Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(255, 171, 92))
                    .add_modifier(Modifier::BOLD),
                selected_meta: Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(227, 134, 76)),
                gauge: Style::default().fg(Color::Rgb(255, 140, 85)),
                dim_gauge: Style::default().fg(Color::Rgb(90, 62, 54)),
                accent: Style::default().fg(Color::Rgb(255, 171, 92)),
                success: Style::default().fg(Color::Rgb(247, 195, 113)),
                warning: Style::default().fg(Color::Rgb(255, 205, 107)),
                error: Style::default().fg(Color::Rgb(255, 120, 105)),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Palette {
    header_bg: Color,
    header_fg: Color,
    border: Style,
    primary_text: Style,
    muted_text: Style,
    selected_line: Style,
    selected_meta: Style,
    gauge: Style,
    dim_gauge: Style,
    accent: Style,
    success: Style,
    warning: Style,
    error: Style,
}

#[derive(Clone, Copy)]
struct PaletteCommandSpec {
    command: &'static str,
    aliases: &'static [&'static str],
    description: &'static str,
    action: PaletteCommand,
}

fn render_header(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState, theme: Palette) {
    let width = area.width as usize;
    let left_text = if width < 80 {
        " xtmonctl ".to_string()
    } else {
        " xtmonctl  external monitor control".to_string()
    };
    let center_text = if width < 60 {
        format!("{} ", state.theme.display_name())
    } else {
        format!(" Theme: {} ", state.theme.display_name())
    };
    let right_text = if width < 72 {
        format!(
            "{} monitor{}",
            state.monitors.len(),
            if state.monitors.len() == 1 { "" } else { "s" }
        )
    } else {
        format!(
            "Tab command  ? help  t theme  Monitors: {}",
            state.monitors.len()
        )
    };

    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(20),
            Constraint::Percentage(35),
        ])
        .split(area);

    let left = Paragraph::new(Line::from(vec![Span::styled(
        left_text,
        Style::default()
            .fg(theme.header_fg)
            .bg(theme.header_bg)
            .add_modifier(Modifier::BOLD),
    )]))
    .alignment(Alignment::Left)
    .block(Block::default().style(Style::default().bg(theme.header_bg)));
    frame.render_widget(left, header_chunks[0]);

    let center = Paragraph::new(Line::from(vec![Span::styled(
        center_text,
        Style::default()
            .fg(theme.header_fg)
            .bg(theme.header_bg)
            .add_modifier(Modifier::BOLD),
    )]))
    .alignment(Alignment::Center)
    .block(Block::default().style(Style::default().bg(theme.header_bg)));
    frame.render_widget(center, header_chunks[1]);

    let right = Paragraph::new(Line::from(vec![Span::styled(
        right_text,
        Style::default()
            .fg(theme.header_fg)
            .bg(theme.header_bg)
            .add_modifier(Modifier::DIM),
    )]))
    .alignment(Alignment::Right)
    .block(Block::default().style(Style::default().bg(theme.header_bg)));
    frame.render_widget(right, header_chunks[2]);
}

fn panel_block<'a>(title: &'a str, theme: Palette) -> Block<'a> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border)
}

fn render_footer(frame: &mut ratatui::Frame<'_>, area: Rect, state: &TuiState, theme: Palette) {
    if state.command_mode {
        let command = Paragraph::new(command_palette_lines(
            &state.command_input,
            state.command_selected,
            state.command_scroll,
            theme,
        ))
        .block(
            Block::default()
                .title(" Command ")
                .borders(Borders::ALL)
                .border_style(theme.accent),
        )
        .wrap(Wrap { trim: true });
        frame.render_widget(command, area);
        return;
    }

    if let Some(panel) = state.active_panel {
        let panel_widget = Paragraph::new(panel_lines(state, panel))
            .block(
                Block::default()
                    .title(panel.title())
                    .borders(Borders::ALL)
                    .border_style(state.active_panel_style()),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(panel_widget, area);
        return;
    }

    let hint = Paragraph::new(vec![footer_hint_line(theme)])
        .alignment(Alignment::Left)
        .block(Block::default().border_style(theme.border));
    frame.render_widget(hint, area);
}

fn selected_detail(state: &TuiState) -> String {
    match state.selected_monitor() {
        Some(monitor) => {
            let serial = if monitor.info.serial.is_empty() {
                "Serial: n/a".to_string()
            } else {
                format!("Serial: {}", monitor.info.serial)
            };
            let connector = monitor
                .info
                .connector_type
                .map(|kind| format!("{kind:?}"))
                .unwrap_or_else(|| "Unknown".into());
            let drm = monitor
                .info
                .drm_connector
                .clone()
                .unwrap_or_else(|| "DRM connector unavailable".into());
            format!(
                "{}\n{}\n{}\nConnector: {}\nDRM: {}",
                monitor.label,
                monitor.info.id.bus_name(),
                serial,
                connector,
                drm
            )
        }
        None => "No monitors detected.".into(),
    }
}

fn handle_messages(state: &mut TuiState, rx: &Receiver<UiMessage>) {
    while let Ok(message) = rx.try_recv() {
        match message {
            UiMessage::Refreshed {
                generation,
                monitors,
                brightness,
                brightness_errors,
                message,
            } => {
                if generation != state.generation {
                    continue;
                }
                state.monitors = monitors;
                state.brightness = brightness.into_iter().collect();
                state.brightness_errors = brightness_errors.into_iter().collect();
                state.selected = state.selected.min(state.monitors.len().saturating_sub(1));
                state.message = message;
                state.loading = false;
            }
            UiMessage::Updated {
                generation,
                bus,
                brightness,
                message,
            } => {
                if generation != state.generation {
                    continue;
                }
                state.brightness.insert(bus, brightness);
                state.brightness_errors.remove(&bus);
                state.message = message;
                state.loading = false;
            }
            UiMessage::Error {
                generation,
                message,
            } => {
                if generation != state.generation {
                    continue;
                }
                state.message = message;
                state.loading = false;
            }
        }
    }
}

fn request_refresh(app: App, tx: Sender<UiMessage>, generation: u64) {
    thread::spawn(move || match app.list_monitors() {
        Ok(monitors) => {
            let mut rows = Vec::new();
            let mut brightness = Vec::new();
            let mut brightness_errors = Vec::new();
            for monitor in monitors {
                let bus = monitor.id.i2c_bus;
                rows.push(MonitorRow {
                    info: monitor.clone(),
                    label: app.display_label(&monitor),
                });
                match app.get_monitor_brightness(&monitor) {
                    Ok(raw) => brightness.push((bus, raw)),
                    Err(error) => brightness_errors.push((bus, error.to_string())),
                }
            }
            let _ = tx.send(UiMessage::Refreshed {
                generation,
                monitors: rows,
                brightness,
                brightness_errors,
                message: "Refreshed monitor list".into(),
            });
        }
        Err(error) => {
            let _ = tx.send(UiMessage::Error {
                generation,
                message: error.to_string(),
            });
        }
    });
}

fn request_adjust(app: App, tx: Sender<UiMessage>, state: &mut TuiState, delta: i16) {
    let Some(row) = state.selected_monitor().cloned() else {
        return;
    };
    let current = state
        .brightness
        .get(&row.info.id.i2c_bus)
        .copied()
        .map(|raw| raw.to_percent())
        .or_else(|| app.last_brightness_for_bus(row.info.id.i2c_bus))
        .unwrap_or_else(default_percent);
    let target = current.saturating_add(delta);
    state.loading = true;
    state.message = format!("Setting {} to {}%...", row.label, target.value());
    request_update_monitor(app, tx, state.generation, row.info, target);
}

fn request_set(app: App, tx: Sender<UiMessage>, state: &mut TuiState, value: u8) {
    let Some(row) = state.selected_monitor().cloned() else {
        return;
    };
    if let Ok(target) = BrightnessPercent::new(value) {
        state.loading = true;
        state.message = format!("Setting {} to {}%...", row.label, target.value());
        request_update_monitor(app, tx, state.generation, row.info, target);
    }
}

fn request_update_monitor(
    app: App,
    tx: Sender<UiMessage>,
    generation: u64,
    monitor: MonitorInfo,
    target: BrightnessPercent,
) {
    thread::spawn(move || match app.set_monitor_brightness(&monitor, target) {
        Ok(raw) => {
            let _ = tx.send(UiMessage::Updated {
                generation,
                bus: monitor.id.i2c_bus,
                brightness: raw,
                message: format!("Set {} to {}%", app.display_label(&monitor), target.value()),
            });
        }
        Err(error) => {
            let _ = tx.send(UiMessage::Error {
                generation,
                message: error.to_string(),
            });
        }
    });
}

fn cycle_theme(app: &App, state: &mut TuiState) {
    state.theme = state.theme.next();
    state.message = format!("Theme switched to {}", state.theme.display_name());
    let _ = app.set_tui_theme(state.theme.config_name());
}

fn default_percent() -> BrightnessPercent {
    match BrightnessPercent::new(5) {
        Ok(percent) => percent,
        Err(_) => unreachable!("built-in default percent is valid"),
    }
}

fn selection_marker(selected: bool) -> &'static str {
    if selected {
        ">"
    } else {
        " "
    }
}

fn footer_hint_line(theme: Palette) -> Line<'static> {
    Line::from(vec![Span::styled(
        "Tab opens command palette. Try /status, /controls, /presets, /help, /hide, /theme.",
        theme.muted_text.add_modifier(Modifier::DIM),
    )])
}

fn command_palette_lines(
    input: &str,
    selected: usize,
    scroll: usize,
    theme: Palette,
) -> Vec<Line<'static>> {
    let matches = filtered_palette_commands(input);
    let selected_spec = matches
        .get(selected.min(matches.len().saturating_sub(1)))
        .copied();
    let mut lines = vec![
        Line::from(vec![
            Span::styled("> ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(input.to_string()),
        ]),
        Line::from("Type a slash command. Up/Down selects, Tab completes, Enter runs, Esc closes."),
    ];

    if matches.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "No matching commands",
            theme.muted_text,
        )]));
        return lines;
    }

    let visible = 5usize;
    let start = scroll.min(matches.len().saturating_sub(1));
    let end = (start + visible).min(matches.len());

    for (index, spec) in matches.iter().enumerate().skip(start).take(end - start) {
        let is_selected = index == selected.min(matches.len().saturating_sub(1));
        let command_style = if is_selected {
            theme.selected_line
        } else {
            theme.primary_text
        };
        let description_style = if is_selected {
            theme.selected_meta
        } else {
            theme.muted_text
        };
        let alias_suffix = if spec.aliases.is_empty() {
            String::new()
        } else {
            format!("  {}", spec.aliases.join(", "))
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", selection_marker(is_selected)), command_style),
            Span::styled(spec.command, command_style.add_modifier(Modifier::BOLD)),
            Span::styled(alias_suffix, theme.muted_text.add_modifier(Modifier::DIM)),
            Span::raw("  "),
            Span::styled(spec.description, description_style),
        ]));
    }

    if matches.len() > end {
        lines.push(Line::from(vec![Span::styled(
            format!("... {} more command(s)", matches.len() - end),
            theme.muted_text.add_modifier(Modifier::DIM),
        )]));
    }

    if let Some(spec) = selected_spec {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Selected: ", theme.accent.add_modifier(Modifier::BOLD)),
            Span::styled(
                spec.command,
                theme.primary_text.add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(spec.description, theme.selected_meta),
        ]));
    }

    lines
}

fn panel_lines(state: &TuiState, panel: BottomPanel) -> Vec<Line<'static>> {
    match panel {
        BottomPanel::Status => status_panel_lines(state),
        BottomPanel::Controls => controls_panel_lines(),
        BottomPanel::Presets => preset_panel_lines(),
        BottomPanel::Help => help_panel_lines(),
    }
}

fn status_panel_lines(state: &TuiState) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(if state.message.is_empty() {
                "Ready".to_string()
            } else {
                state.message.clone()
            }),
        ]),
        Line::from("r rescans monitors and reloads brightness from ddcutil."),
        Line::from("t changes the TUI theme and saves it to your config."),
        Line::from(feature_summary_text()),
    ]
}

fn controls_panel_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(primary_controls_text()),
        Line::from(secondary_controls_text()),
        Line::from("Tab switches bottom panels. ? jumps straight to Help."),
        Line::from("q quits the TUI."),
    ]
}

fn preset_panel_lines() -> Vec<Line<'static>> {
    vec![
        Line::from("Presets: 1-9 => 10% through 90%"),
        Line::from("0 sets 100% on the selected monitor"),
        Line::from("h/l uses normal step size from config"),
        Line::from("H/L uses large step size from config"),
    ]
}

fn help_panel_lines() -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from("Use j/k or the arrow keys to move between monitors."),
        Line::from("Use h/l for normal brightness steps and H/L for larger jumps."),
    ];
    lines.extend(expanded_help_lines());
    lines
}

fn primary_controls_text() -> &'static str {
    "Move: j/k or Up/Down    Adjust: h/l or Left/Right"
}

fn secondary_controls_text() -> &'static str {
    "Large step: H/L    Presets: 0-9    Theme: t    Refresh: r"
}

fn feature_summary_text() -> &'static str {
    "View: aliases, raw + percent brightness, connector details, saved theme"
}

fn expanded_help_lines() -> Vec<Line<'static>> {
    vec![
        Line::from("0 sets 100%, while 1-9 set 10% through 90% on the selected monitor."),
        Line::from("Refresh rescans monitors and reloads live brightness values from ddcutil."),
        Line::from(
            "Theme switching is available directly in the TUI and is saved to the config file.",
        ),
        Line::from(
            "Brightness changes are applied directly without forcing a full monitor rescan.",
        ),
    ]
}

fn palette_commands() -> &'static [PaletteCommandSpec] {
    &[
        PaletteCommandSpec {
            command: "/status",
            aliases: &["/s"],
            description: "Show current status and recent activity",
            action: PaletteCommand::Show(BottomPanel::Status),
        },
        PaletteCommandSpec {
            command: "/controls",
            aliases: &["/c"],
            description: "Show movement and brightness shortcuts",
            action: PaletteCommand::Show(BottomPanel::Controls),
        },
        PaletteCommandSpec {
            command: "/presets",
            aliases: &["/p"],
            description: "Show preset brightness shortcuts",
            action: PaletteCommand::Show(BottomPanel::Presets),
        },
        PaletteCommandSpec {
            command: "/help",
            aliases: &["/h"],
            description: "Show the full help panel",
            action: PaletteCommand::Show(BottomPanel::Help),
        },
        PaletteCommandSpec {
            command: "/theme",
            aliases: &["/t"],
            description: "Cycle to the next theme",
            action: PaletteCommand::ThemeNext,
        },
        PaletteCommandSpec {
            command: "/hide",
            aliases: &["/x"],
            description: "Hide the bottom panel",
            action: PaletteCommand::Hide,
        },
    ]
}

fn filtered_palette_commands(input: &str) -> Vec<PaletteCommandSpec> {
    let query = input.trim().to_ascii_lowercase();
    if query.is_empty() || query == "/" {
        return palette_commands().to_vec();
    }

    let mut direct = palette_commands()
        .iter()
        .copied()
        .filter(|spec| {
            spec.command.starts_with(&query)
                || spec.aliases.iter().any(|alias| alias.starts_with(&query))
        })
        .collect::<Vec<_>>();

    let desc_query = query.trim_start_matches('/');
    let mut descriptive = palette_commands()
        .iter()
        .copied()
        .filter(|spec| {
            !direct
                .iter()
                .any(|existing| existing.command == spec.command)
                && spec.description.to_ascii_lowercase().contains(desc_query)
        })
        .collect::<Vec<_>>();

    direct.append(&mut descriptive);
    direct
}

fn selected_palette_command(
    input: &str,
    selected: usize,
) -> std::result::Result<PaletteCommand, String> {
    let matches = filtered_palette_commands(input);
    if matches.is_empty() {
        let trimmed = input.trim();
        if trimmed.is_empty() || trimmed == "/" {
            return Err("Type a slash command like /status or /help".into());
        }
        return Err(format!("Unknown command: {trimmed}"));
    }

    let trimmed = input.trim().to_ascii_lowercase();
    if let Some(exact) = matches
        .iter()
        .find(|spec| spec.command == trimmed || spec.aliases.iter().any(|alias| *alias == trimmed))
    {
        return Ok(exact.action);
    }

    let index = selected.min(matches.len().saturating_sub(1));
    Ok(matches[index].action)
}

#[cfg(test)]
mod tests {
    use super::{
        filtered_palette_commands, handle_messages, selected_palette_command, BottomPanel,
        MonitorRow, PaletteCommand, ThemeKind, TuiState, UiMessage,
    };
    use crate::ddc::{MonitorId, MonitorInfo};
    use crate::units::BrightnessRaw;
    use std::sync::mpsc;

    #[test]
    fn ignores_stale_refresh_messages() {
        let (tx, rx) = mpsc::channel();
        let mut state = TuiState {
            generation: 2,
            message: "current".into(),
            ..TuiState::default()
        };

        tx.send(UiMessage::Refreshed {
            generation: 1,
            monitors: vec![MonitorRow {
                info: MonitorInfo {
                    id: MonitorId {
                        display_number: 1,
                        i2c_bus: 4,
                    },
                    manufacturer: "MSI".into(),
                    model: "MP223".into(),
                    serial: String::new(),
                    drm_connector: None,
                    connector_type: None,
                },
                label: "Old".into(),
            }],
            brightness: vec![(
                4,
                BrightnessRaw {
                    value: 10,
                    max: 100,
                },
            )],
            brightness_errors: Vec::new(),
            message: "old".into(),
        })
        .expect("send stale refresh");

        handle_messages(&mut state, &rx);

        assert!(state.monitors.is_empty());
        assert_eq!(state.message, "current");
    }

    #[test]
    fn applies_current_generation_messages() {
        let (tx, rx) = mpsc::channel();
        let mut state = TuiState {
            generation: 3,
            loading: true,
            ..TuiState::default()
        };

        tx.send(UiMessage::Error {
            generation: 3,
            message: "boom".into(),
        })
        .expect("send current error");

        handle_messages(&mut state, &rx);

        assert_eq!(state.message, "boom");
        assert!(!state.loading);
    }

    #[test]
    fn theme_cycles_in_expected_order() {
        assert_eq!(ThemeKind::Ocean.next(), ThemeKind::Forest);
        assert_eq!(ThemeKind::Forest.next(), ThemeKind::Ember);
        assert_eq!(ThemeKind::Ember.next(), ThemeKind::Ocean);
    }

    #[test]
    fn bottom_panel_cycles_in_expected_order() {
        assert!(matches!(
            selected_palette_command("/status", 0),
            Ok(PaletteCommand::Show(BottomPanel::Status))
        ));
        assert!(matches!(
            selected_palette_command("/controls", 0),
            Ok(PaletteCommand::Show(BottomPanel::Controls))
        ));
        assert!(matches!(
            selected_palette_command("/presets", 0),
            Ok(PaletteCommand::Show(BottomPanel::Presets))
        ));
        assert!(matches!(
            selected_palette_command("/help", 0),
            Ok(PaletteCommand::Show(BottomPanel::Help))
        ));
    }

    #[test]
    fn slash_shows_all_commands() {
        assert!(filtered_palette_commands("/").len() >= 6);
    }

    #[test]
    fn prefix_filters_theme_command() {
        let matches = filtered_palette_commands("/t");
        assert!(matches.iter().any(|spec| spec.command == "/theme"));
    }

    #[test]
    fn alias_filters_status_command() {
        let matches = filtered_palette_commands("/s");
        assert!(matches.iter().any(|spec| spec.command == "/status"));
    }

    #[test]
    fn alias_executes_matching_command() {
        assert!(matches!(
            selected_palette_command("/c", 0),
            Ok(PaletteCommand::Show(BottomPanel::Controls))
        ));
    }

    #[test]
    fn tab_completion_uses_selected_command() {
        let mut state = TuiState {
            command_mode: true,
            command_input: "/t".into(),
            command_selected: 0,
            ..TuiState::default()
        };

        state.autocomplete_command();

        assert_eq!(state.command_input, "/theme");
    }
}
