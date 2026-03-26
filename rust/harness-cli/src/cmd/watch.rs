use anyhow::Result;
use clap::Args;
use crossterm::event::{self, Event as TermEvent, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use std::io::stdout;
use std::time::Duration;

use harness_core::events::{EventFilter, EventType, Severity};
use harness_core::storage::sqlite::SqliteStore;
use harness_core::storage::EventStore;

use crate::util;

#[derive(Args)]
pub struct WatchCmd {
    /// Filter by event type
    #[arg(long)]
    filter: Option<String>,

    /// Poll interval in seconds
    #[arg(long, default_value = "2")]
    interval: u64,
}

impl WatchCmd {
    pub fn run(self) -> Result<()> {
        let store = open_store()?;

        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

        let type_filter = self
            .filter
            .as_deref()
            .map(|s| s.parse::<EventType>())
            .transpose()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let result = run_loop(&mut terminal, &store, type_filter, self.interval);

        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;

        result
    }
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    store: &SqliteStore,
    type_filter: Option<EventType>,
    interval: u64,
) -> Result<()> {
    loop {
        let filter = EventFilter {
            event_type: type_filter,
            limit: Some(50),
            ..Default::default()
        };
        let events = store.list(&filter)?;
        let stats = store.stats(&EventFilter::default())?;

        terminal.draw(|frame| {
            let area = frame.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(1),
                ])
                .split(area);

            // Header
            let header = Paragraph::new(Line::from(vec![
                Span::styled(
                    " SYNODIC ",
                    Style::default().fg(Color::Black).bg(Color::Cyan).bold(),
                ),
                Span::raw("  AI Agent Governance  "),
                Span::styled("LIVE", Style::default().fg(Color::Green).bold()),
            ]))
            .block(Block::default().borders(Borders::BOTTOM));
            frame.render_widget(header, chunks[0]);

            // Stats bar
            let stats_line = Line::from(vec![
                Span::raw(" Total: "),
                Span::styled(stats.total.to_string(), Style::default().bold()),
                Span::raw("  Unresolved: "),
                Span::styled(
                    stats.unresolved.to_string(),
                    Style::default()
                        .fg(if stats.unresolved > 0 {
                            Color::Red
                        } else {
                            Color::Green
                        })
                        .bold(),
                ),
                Span::raw("  Resolution: "),
                Span::styled(
                    format!(
                        "{}%",
                        if stats.total > 0 {
                            (stats.total - stats.unresolved) * 100 / stats.total
                        } else {
                            0
                        }
                    ),
                    Style::default().bold(),
                ),
                if let Some(ref t) = type_filter {
                    Span::styled(
                        format!("  Filter: {}", t),
                        Style::default().fg(Color::Yellow),
                    )
                } else {
                    Span::raw("")
                },
            ]);
            frame.render_widget(Paragraph::new(stats_line), chunks[1]);

            // Events table
            let header_row = Row::new(vec![
                Cell::from("TYPE").style(Style::default().fg(Color::DarkGray)),
                Cell::from("SEVERITY").style(Style::default().fg(Color::DarkGray)),
                Cell::from("TITLE").style(Style::default().fg(Color::DarkGray)),
                Cell::from("SOURCE").style(Style::default().fg(Color::DarkGray)),
                Cell::from("STATUS").style(Style::default().fg(Color::DarkGray)),
                Cell::from("TIME").style(Style::default().fg(Color::DarkGray)),
            ]);

            let rows: Vec<Row> = events
                .iter()
                .map(|e| {
                    let sev_color = match e.severity {
                        Severity::Critical => Color::Red,
                        Severity::High => Color::LightRed,
                        Severity::Medium => Color::Yellow,
                        Severity::Low => Color::Green,
                    };
                    let status = if e.resolved { "resolved" } else { "open" };
                    let status_color = if e.resolved { Color::Green } else { Color::Red };
                    let title = if e.title.len() > 50 {
                        format!("{}...", &e.title[..47])
                    } else {
                        e.title.clone()
                    };

                    Row::new(vec![
                        Cell::from(e.event_type.as_str()),
                        Cell::from(e.severity.as_str()).style(Style::default().fg(sev_color)),
                        Cell::from(title),
                        Cell::from(e.source.as_str()),
                        Cell::from(status).style(Style::default().fg(status_color)),
                        Cell::from(e.created_at.format("%H:%M:%S").to_string())
                            .style(Style::default().fg(Color::DarkGray)),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Length(22),
                    Constraint::Length(10),
                    Constraint::Min(30),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Length(10),
                ],
            )
            .header(header_row)
            .block(Block::default().title(" Events ").borders(Borders::ALL));
            frame.render_widget(table, chunks[2]);

            // Footer
            let footer = Paragraph::new(Span::styled(
                " q: quit  r: refresh ",
                Style::default().fg(Color::DarkGray),
            ));
            frame.render_widget(footer, chunks[3]);
        })?;

        // Poll for keyboard input
        if event::poll(Duration::from_secs(interval))? {
            if let TermEvent::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('r') => continue, // force refresh
                        _ => {}
                    }
                }
            }
        }
    }
}

fn open_store() -> Result<SqliteStore> {
    let root = util::find_repo_root()?;
    let db_path = root.join(".harness").join("synodic.db");
    if !db_path.exists() {
        anyhow::bail!("Database not found. Run `synodic init` first.");
    }
    SqliteStore::open(&db_path)
}
