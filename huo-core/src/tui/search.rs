use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::io::{self, stdout};

use crate::plugins::traits::{MangaPlugin, SearchResult};

pub struct SearchApp {
    query: String,
    results: Vec<SearchResult>,
    list_state: ListState,
    is_searching: bool,
    error_message: Option<String>,
    selected_url: Option<String>,
}

impl SearchApp {
    fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            list_state: ListState::default(),
            is_searching: false,
            error_message: None,
            selected_url: None,
        }
    }

    fn next(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.results.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.results.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.results.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn select(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if i < self.results.len() {
                self.selected_url = Some(self.results[i].url.clone());
            }
        }
    }
}

pub async fn run_search_tui(
    client: &reqwest::Client,
    plugin: &dyn MangaPlugin,
) -> io::Result<Option<String>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = SearchApp::new();
    let result = run_app(&mut terminal, &mut app, client, plugin).await;

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut SearchApp,
    client: &reqwest::Client,
    plugin: &dyn MangaPlugin,
) -> io::Result<Option<String>> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                        KeyCode::Enter => {
                            if !app.results.is_empty() && app.list_state.selected().is_some() {
                                // Select result
                                app.select();
                                return Ok(app.selected_url.clone());
                            } else if !app.query.is_empty()
                                && !app.is_searching
                                && app.results.is_empty()
                            {
                                // Perform search
                                app.is_searching = true;
                                app.error_message = None;
                                terminal.draw(|f| ui(f, app))?;

                                match plugin.search(client, &app.query).await {
                                    Ok(results) => {
                                        app.results = results;
                                        app.list_state.select(if app.results.is_empty() {
                                            None
                                        } else {
                                            Some(0)
                                        });
                                        if app.results.is_empty() {
                                            app.error_message =
                                                Some("No results found".to_string());
                                        }
                                    }
                                    Err(e) => {
                                        app.error_message = Some(format!("Search failed: {}", e));
                                    }
                                }
                                app.is_searching = false;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => app.next(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous(),
                        KeyCode::Char(c) if !c.is_control() => {
                            app.query.push(c);
                            app.results.clear();
                            app.list_state.select(None);
                            app.error_message = None;
                        }
                        KeyCode::Backspace => {
                            app.query.pop();
                            app.results.clear();
                            app.list_state.select(None);
                            app.error_message = None;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &SearchApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Search input
    let input_text = if app.is_searching {
        format!("{}█ [Searching...]", app.query)
    } else {
        format!("{}█", app.query)
    };

    let input = Paragraph::new(input_text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("🔍 Search Manga (Type query, Enter to search, Esc/q to quit)"),
        );
    f.render_widget(input, chunks[0]);

    // Results list
    let items: Vec<ListItem> = app
        .results
        .iter()
        .map(|result| {
            let content = Line::from(vec![
                Span::styled("📖 ", Style::default().fg(Color::Cyan)),
                Span::raw(&result.title),
            ]);
            ListItem::new(content)
        })
        .collect();

    let list_title = if app.results.is_empty() && !app.query.is_empty() && !app.is_searching {
        "Results (No results found)"
    } else if app.results.is_empty() {
        "Results (Type to search)"
    } else {
        "Results (↑↓ to navigate, Enter to select)"
    };

    let results_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("→ ");

    f.render_stateful_widget(results_list, chunks[1], &mut app.list_state.clone());

    // Status bar
    let status_text = if let Some(ref error) = app.error_message {
        Line::from(vec![
            Span::styled("❌ ", Style::default().fg(Color::Red)),
            Span::styled(error, Style::default().fg(Color::Red)),
        ])
    } else if let Some(i) = app.list_state.selected() {
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::raw(format!("Selected: {} / {}", i + 1, app.results.len())),
        ])
    } else {
        Line::from(vec![
            Span::styled("ℹ ", Style::default().fg(Color::Blue)),
            Span::raw("Enter a search query and press Enter"),
        ])
    };

    let status =
        Paragraph::new(status_text).block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(status, chunks[2]);
}
