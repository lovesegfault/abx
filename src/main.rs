use anyhow::{anyhow, Result};
use gstreamer::prelude::*;
use structopt::StructOpt;
use url::Url;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn gst_main(a: Url) -> Result<()> {
    gstreamer::init()?;
    let main = glib::MainLoop::new(None, false);
    let dispatcher = gstreamer_player::PlayerGMainContextSignalDispatcher::new(None);
    let player = gstreamer_player::Player::new(
        None,
        Some(&dispatcher.upcast::<gstreamer_player::PlayerSignalDispatcher>()),
    );
    player.set_uri(a.as_str());
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

        *error.lock().unwrap() = Err(err.clone());

        player.stop();
        main.quit();
    });

    player.play();
    main.run();

    let guard = error.as_ref().lock().unwrap();

    guard.clone().map_err(|e| e.into())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "abx", about = "CLI utility to ABX audio files.")]
struct Opt {
    a: PathBuf,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    gst_main(Url::from_file_path(opt.a).map_err(|_| anyhow!("Failed to convert path A to URI"))?)
}
