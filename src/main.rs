use anyhow::{anyhow, Error};
use gstreamer::prelude::*;
use gstreamer_player::{PlayerGMainContextSignalDispatcher, PlayerSignalDispatcher};
use structopt::StructOpt;
use url::Url;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

struct Player {
    player: gstreamer_player::Player,
    error: Arc<Mutex<Result<(), glib::Error>>>,
}

impl Player {
    pub fn new(uri: &Url, main: &glib::MainLoop) -> Self {
        let dispatcher = PlayerGMainContextSignalDispatcher::new(None);
        let player = gstreamer_player::Player::new(
            None,
            Some(&dispatcher.upcast::<PlayerSignalDispatcher>()),
        );

        player.set_uri(uri.as_str());

        let error = Arc::new(Mutex::new(Ok(())));

        let main_clone = main.clone();
        player.connect_end_of_stream(move |player| {
            let main = &main_clone;
            player.stop();
            main.quit();
        });

        let main_clone = main.clone();
        let error_clone = error.clone();
        player.connect_error(move |player, err| {
            let main = &main_clone;
            let error = &error_clone;

            *error.lock().expect("poisoned error mutex") = Err(err.clone());

            player.stop();
            main.quit();
        });

        Self { player, error }
    }

    pub fn play(&self) {
        self.player.play();
    }

    pub fn stop(&self) {
        self.player.stop();
    }

    pub fn as_error(&self) -> Result<(), glib::Error> {
        self.error
            .as_ref()
            .lock()
            .expect("poined error mutex")
            .clone()
            .map_err(Into::into)
    }
}

fn gst_main(a: Url, b: Url) -> Result<(), Error> {
    gstreamer::init()?;
    let main = glib::MainLoop::new(None, false);

    let a = Player::new(&a, &main);
    let b = Player::new(&b, &main);

    a.play();
    b.play();

    main.run();

    a.as_error()?;
    b.as_error()?;

    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "abx", about = "CLI utility to ABX audio files.")]
struct Opt {
    a: PathBuf,
    b: PathBuf,
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();
    let a = Url::from_file_path(opt.a).map_err(|_| anyhow!("Failed to convert path A to URI"))?;
    let b = Url::from_file_path(opt.b).map_err(|_| anyhow!("Failed to convert path A to URI"))?;
    gst_main(a, b)
}
