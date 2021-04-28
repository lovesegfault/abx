use std::{io, path::PathBuf};

use anyhow::Error;
use structopt::StructOpt;
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState},
    Terminal,
};

use abx::{
    events::{Event, Events},
    AudioSelector,
};

#[derive(Debug, StructOpt)]
#[structopt(name = "abx", about = "CLI utility to ABX audio files.")]
struct Opt {
    a: PathBuf,
    b: PathBuf,
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();

    gstreamer::init()?;

    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let events = Events::new();

    let mut pipeline = AudioSelector::new()?
        .with_source(&opt.a)?
        .with_source(&opt.b)?
        .run()?;
    let mut state = ListState::default();
    state.select(Some(0));
    loop {
        terminal
            .draw(|f| {
                let rects = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(2)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
                    .split(f.size());

                let gauge = Gauge::default()
                    .block(
                        Block::default()
                            .title("Song Progress")
                            .borders(Borders::ALL),
                    )
                    .gauge_style(Style::default().fg(Color::Yellow))
                    .percent(pipeline.progress().unwrap_or(0.0) as u16);
                f.render_widget(gauge, rects[0]);

                let list_of_songs = pipeline
                    .sources
                    .lock()
                    .expect("poisoned source lock")
                    .iter()
                    .map(|s| s.path.to_string_lossy().as_ref().to_owned())
                    .map(|n| ListItem::new(n))
                    .collect::<Vec<_>>();
                let list = List::new(list_of_songs)
                    .block(Block::default().title("Songs").borders(Borders::ALL))
                    .highlight_style(
                        Style::default()
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("♫ ");
                f.render_stateful_widget(list, rects[1], &mut state);
            })
            .unwrap();

        match events.next().unwrap() {
            Event::Input(input) => match input {
                Key::Char('q') => {
                    break;
                }
                Key::Char('n') => {
                    pipeline.next_source().unwrap();
                    state.select(Some(
                        pipeline.selected.load(std::sync::atomic::Ordering::SeqCst),
                    ));
                }
                _ => (),
            },
            Event::Tick => (),
        }
    }

    Ok(())
}
