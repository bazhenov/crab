use crate::{crawler::CrawlerState, prelude::*};
use atom::Atom;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    fmt::Display,
    io,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Row, Table},
    Frame, Terminal,
};

/// Encodes which information main panel is showing
#[derive(Copy, Clone)]
enum MainPanelMode {
    InFlightRequests,
    Proxies,
}

pub(crate) fn ui(state: Arc<Atom<Box<CrawlerState>>>, tick_rate: Duration) -> Result<()> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    // initialize terminal
    enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;

    let res = run_terminal(&mut terminal, state, tick_rate);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res?;
    Ok(())
}

fn run_terminal<B: Backend>(
    terminal: &mut Terminal<B>,
    state: Arc<Atom<Box<CrawlerState>>>,
    tick_duration: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    let mut current_state = None;
    let mut main_panel_mode = MainPanelMode::InFlightRequests;
    loop {
        current_state = state.take(Ordering::Relaxed).or(current_state);

        if let Some(state) = &current_state {
            terminal.draw(|f| draw_widgets(f, state, main_panel_mode))?;
        }

        let timeout = tick_duration
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('p') => main_panel_mode = MainPanelMode::Proxies,
                    KeyCode::Char('r') => main_panel_mode = MainPanelMode::InFlightRequests,
                    KeyCode::Char('q') => return Ok(()),
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_duration {
            last_tick = Instant::now();
        }
    }
}

fn metric<T: Display>(name: &'static str, value: T) -> ListItem<'static> {
    ListItem::new(format!("{}: {}", name, value))
}

fn draw_widgets(f: &mut Frame<impl Backend>, state: &CrawlerState, main_panel_mode: MainPanelMode) {
    let metrics = List::new([
        metric("Number of requests", state.requests),
        metric(
            "Number of requests in flight",
            state.requests_in_flight.len(),
        ),
        metric("Number of successfull requests", state.successfull_requests),
        metric("Number of new links found", state.new_links_found),
    ])
    .block(create_block("Metrics"));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Max(6), Constraint::Percentage(50)].as_ref())
        .margin(1)
        .split(f.size());
    let metrics_panel = layout[0];
    let main_panel = layout[1];

    f.render_widget(metrics, metrics_panel);

    match main_panel_mode {
        MainPanelMode::InFlightRequests => {
            let requests = state
                .requests_in_flight
                .iter()
                .map(|r| r.url.to_string())
                .map(ListItem::new)
                .collect::<Vec<_>>();
            let list = List::new(requests).block(create_block("Requests in flight"));
            f.render_widget(list, main_panel);
        }
        MainPanelMode::Proxies => {
            let proxies = state
                .proxies
                .iter()
                .map(|(proxy, stat)| {
                    Row::new(vec![
                        format!("{:>5}", stat.requests.to_string()),
                        format!("{:>5}", stat.successfull_requests.to_string()),
                        format!("{:?}", proxy),
                    ])
                })
                .collect::<Vec<_>>();

            let header = Row::new(vec!["Requests", "Successfull", "Proxy"])
                .style(Style::default().fg(Color::Yellow));
            let table = Table::new(proxies).header(header).widths(&[
                Constraint::Length(5),
                Constraint::Length(5),
                Constraint::Percentage(80),
            ]);
            f.render_widget(table, main_panel);
        }
    };
}

fn create_block(title: &str) -> Block {
    Block::default().borders(Borders::ALL).title(title)
}
