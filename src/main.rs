use anyhow::{Context, Error};
use gstreamer::prelude::*;
use structopt::StructOpt;

use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;

#[derive(Debug, StructOpt)]
#[structopt(name = "abx", about = "CLI utility to ABX audio files.")]
struct Opt {
    a: PathBuf,
    b: PathBuf,
}

struct AudioStreamSelector {
    selection: AtomicUsize,
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();

    gstreamer::init()?;

    let pipeline = gstreamer::Pipeline::new(Some("abx"));

    let a_src = gstreamer::ElementFactory::make("filesrc", None)?;
    a_src.set_property("location", &opt.a.as_os_str().to_str())?;
    let b_src = gstreamer::ElementFactory::make("filesrc", None)?;
    b_src.set_property("location", &opt.b.as_os_str().to_str())?;

    let a_decoder = gstreamer::ElementFactory::make("decodebin", None)?;
    let b_decoder = gstreamer::ElementFactory::make("decodebin", None)?;

    let mixer = gstreamer::ElementFactory::make("audiomixer", Some("mixer"))?;

    let audiosink = gstreamer::ElementFactory::make("autoaudiosink", None)?;

    pipeline.add_many(&[&a_src, &a_decoder, &b_src, &b_decoder, &mixer, &audiosink])?;

    a_src
        .link(&a_decoder)
        .with_context(|| "Failed to link A to decoder")?;

    let a_mixer_pad = mixer
        .request_pad(
            &mixer
                .get_pad_template("sink_%u")
                .expect("Failed to get sink template for mixer"),
            None,
            None,
        )
        .expect("Failed to get a_mixer_pad");

    // a_mixer_pad.set_property("mute", &true)?;

    a_decoder.connect_pad_added(move |_, pad| {
        pad.link(&a_mixer_pad).unwrap();
    });

    b_src
        .link(&b_decoder)
        .with_context(|| "Failed to link B to decoder")?;

    let b_mixer_pad = mixer
        .request_pad(
            &mixer
                .get_pad_template("sink_%u")
                .expect("Failed to get sink template for mixer"),
            None,
            None,
        )
        .expect("Failed to get b_mixer_pad");

    b_mixer_pad.set_property("mute", &true)?;

    b_decoder.connect_pad_added(move |_, pad| {
        pad.link(&b_mixer_pad).unwrap();
    });

    mixer
        .link(&audiosink)
        .with_context(|| "Failed to link mixer to audio sink")?;

    pipeline.set_state(gstreamer::State::Playing)?;

    let bus = pipeline.get_bus().expect("FIXME");
    loop {
        let message = bus.timed_pop(gstreamer::ClockTime::none());
        if let Some(msg) = message {
            use gstreamer::MessageView::*;
            match msg.view() {
                Eos(_) => {
                    pipeline.set_state(gstreamer::State::Paused)?;
                    break;
                }
                Error(e) => {
                    eprintln!("{:?}", e);
                }
                Progress(p) => {
                    println!("{:?}", p);
                }
                _ => continue,
            }
        }
    }
    Ok(())
}
