use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, ExecutableCommand};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;

use crate::app::App;
use crate::error::{Result, XtmonctlError};

pub fn run(app: &App) -> Result<()> {
    enable_raw_mode().map_err(io_error)?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(io_error)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(io_error)?;

    let run_result = run_loop(&mut terminal, app);

    disable_raw_mode().map_err(io_error)?;
    io::stdout().execute(LeaveAlternateScreen).map_err(io_error)?;
    run_result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &App) -> Result<()> {
    let monitors = app.list_monitors()?;
    let mut selected = 0usize;

    loop {
        terminal
            .draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(2)])
                    .split(frame.area());

                let items = monitors
                    .iter()
                    .enumerate()
                    .map(|(index, monitor)| {
                        let style = if index == selected {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ListItem::new(Line::from(vec![Span::styled(
                            format!("{} ({})", monitor.display_name(), monitor.id.bus_name()),
                            style,
                        )]))
                    })
                    .collect::<Vec<_>>();

                let list = List::new(items).block(Block::default().title("Monitors").borders(Borders::ALL));
                frame.render_widget(list, chunks[0]);

                let help = Paragraph::new("j/k or arrows: select  q: quit")
                    .block(Block::default().borders(Borders::ALL));
                frame.render_widget(help, chunks[1]);
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
                KeyCode::Char('j') | KeyCode::Down => {
                    if !monitors.is_empty() {
                        selected = (selected + 1) % monitors.len();
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if !monitors.is_empty() {
                        selected = (selected + monitors.len() - 1) % monitors.len();
                    }
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
