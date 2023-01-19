use crate::{crawler::CrawlerState, prelude::*};
use atom::Atom;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    io,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem},
    Frame, Terminal,
};

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
    loop {
        current_state = state.take(Ordering::Relaxed).or(current_state);

        if let Some(state) = &current_state {
            terminal.draw(|f| draw_widgets(f, state))?;
        }

        let timeout = tick_duration
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    return Ok(());
                }
            }
        }
        if last_tick.elapsed() >= tick_duration {
            last_tick = Instant::now();
        }
    }
}

fn draw_widgets(f: &mut Frame<impl Backend>, state: &CrawlerState) {
    let metrics = List::new([
        ListItem::new(format!("Number of requests: {}", state.requests)),
        ListItem::new(format!(
            "Number of requests in flight: {}",
            state.requests_in_flight.len()
        )),
        ListItem::new(format!(
            "Number of successfull requests: {}",
            state.successfull_requests
        )),
        ListItem::new(format!(
            "Number of new links found: {}",
            state.new_links_found
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title("Metrics"));

    let requests = state
        .requests_in_flight
        .iter()
        .map(|r| r.url.to_string())
        .map(ListItem::new)
        .collect::<Vec<_>>();
    let requests = List::new(requests).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Requests in flight"),
    );

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Max(6), Constraint::Percentage(50)].as_ref())
        .split(f.size());

    f.render_widget(metrics, layout[0]);
    f.render_widget(requests, layout[1]);
}
