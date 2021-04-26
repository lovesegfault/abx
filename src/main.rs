use std::path::PathBuf;

use anyhow::Error;
use crossterm::event::{read, Event, KeyCode};
use crossterm::terminal::enable_raw_mode;
use crossterm::{event::KeyModifiers, terminal::disable_raw_mode};
use glib::{MainLoop, PRIORITY_HIGH};
use structopt::StructOpt;

use abx::AudioSelector;

#[derive(Debug, StructOpt)]
#[structopt(name = "abx", about = "CLI utility to ABX audio files.")]
struct Opt {
    a: PathBuf,
    b: PathBuf,
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();

    gstreamer::init()?;

    let main = MainLoop::new(None, false);
    let ctx = main.get_context();

    let pipeline = AudioSelector::new()?
        .with_source(&opt.a)?
        .with_source(&opt.b)?
        .with_mainloop(&main)?
        .play()?;

    {
        let mut pipeline = pipeline.clone();
        ctx.invoke_with_priority(PRIORITY_HIGH, move || {
            enable_raw_mode().unwrap();
            loop {
                match read().unwrap() {
                    Event::Key(event) => {
                        use KeyCode::*;
                        match event.code {
                            Char('n') => pipeline.next_source().unwrap(),
                            _ => eprintln!("{:?}", event),
                        }
                    }
                    _ => continue,
                }
            }
        });
    }

    {
        let pipeline = pipeline.clone();
        ctx.invoke(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let status = pipeline
                .progress()
                .expect("failed to get pipeline progress");
            eprintln!(">>>> {}", status);
        })
    }

    main.run();

    disable_raw_mode().unwrap();
    Ok(())
}
