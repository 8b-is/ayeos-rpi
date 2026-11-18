// ayeos-rpi / burner / src / tui.rs — ratatui TUI scaffold (feature "tui")
// Compiles only when `tui` feature is enabled and ratatui is available.
#![allow(dead_code)]

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};

pub struct App {
    devices: Vec<String>,
    selected: usize,
    status: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            selected: 0,
            status: "Select a device".into(),
        }
    }
}

pub fn run(terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>, app: &mut App) -> std::io::Result<()> {
    terminal.draw(|f| ui(f, app))?;
    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
        .split(f.size());

    let title = Paragraph::new("ayeos-rpi burner").block(Block::default().borders(Borders::ALL).title("burner"));
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = app
        .devices
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let style = if i == app.selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(d.as_str()).style(style)
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("devices"));
    f.render_widget(list, chunks[1]);

    let status = Paragraph::new(app.status.as_str()).block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[2]);
}
