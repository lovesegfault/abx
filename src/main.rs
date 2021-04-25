use anyhow::{anyhow, Context, Error};
use gstreamer::{prelude::*, ClockTime, Element, Pad, Pipeline};
use structopt::StructOpt;

use std::path::{Path, PathBuf};

#[derive(Clone)]
struct AudioPipeline {
    pipeline: Pipeline,
}

impl AudioPipeline {
    pub fn new(name: Option<&str>) -> Self {
        let pipeline = gstreamer::Pipeline::new(name);
        Self { pipeline }
    }

    pub fn play(&self) -> Result<(), Error> {
        self.pipeline
            .set_state(gstreamer::State::Playing)
            .with_context(|| "failed to play AudioPipeline")
            .map(|_| ())
    }

    pub fn pause(&self) -> Result<(), Error> {
        self.pipeline
            .set_state(gstreamer::State::Paused)
            .with_context(|| "failed to pause AudioPipeline")
            .map(|_| ())
    }

    pub fn run(self) -> Result<(), Error> {
        self.play()?;
        let bus = self
            .pipeline
            .get_bus()
            .expect("failed to get bus from AudioPipeline");
        while let Some(msg) = bus.timed_pop(gstreamer::ClockTime::none()) {
            use gstreamer::MessageView::*;
            match msg.view() {
                Eos(_) => {
                    self.pause()?;
                    break;
                }
                Error(e) => {
                    self.pause()?;
                    eprintln!("{:?}", e);
                    break;
                }
                _ => continue,
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct AudioDecoder {
    src: Element,
    dec: Element,
}

impl AudioDecoder {
    pub fn new<P: AsRef<Path>>(pipeline: &AudioPipeline, file: P) -> Result<Self, Error> {
        let src = gstreamer::ElementFactory::make("filesrc", None).with_context(|| "")?;
        src.set_property("location", &file.as_ref().to_str())
            .with_context(|| format!("failed to set location to {:?}", file.as_ref()))?;
        let dec = gstreamer::ElementFactory::make("decodebin", None)?;

        pipeline
            .pipeline
            .add(&src)
            .with_context(|| "Failed to create ")?;
        pipeline.pipeline.add(&dec)?;
        src.link(&dec)?;

        Ok(Self { src, dec })
    }

    pub fn src_path(&self) -> String {
        self.src
            .get_property("location")
            .expect("AudioDecoder src did not have location property")
            .get()
            .map(|o| o.expect("location not set for AudioDecoder"))
            .expect("failed to get location from AudioDecoder")
    }

    pub fn link_with(self, other: &Pad) {
        let other = other.clone();
        let path = self.src_path();
        self.dec.connect_pad_added(move |_, pad| {
            pad.link(&other).unwrap_or_else(|_| {
                panic!("failed to link decoder for {}", path);
            });
        });
    }
}

struct AudioMixer {
    mixer: Element,
}

impl AudioMixer {
    pub fn new(pipeline: &AudioPipeline) -> Result<Self, Error> {
        let mixer = gstreamer::ElementFactory::make("audiomixer", None)?;
        let sink = gstreamer::ElementFactory::make("autoaudiosink", None)?;

        pipeline
            .pipeline
            .add(&mixer)
            .with_context(|| "failed to add mixer to pipeline")?;
        pipeline
            .pipeline
            .add(&sink)
            .with_context(|| "failed to add sink to pipeline")?;

        mixer.link(&sink)?;
        Ok(Self { mixer })
    }

    fn get_pad(&self) -> Result<Pad, Error> {
        self.mixer
            .request_pad(
                &self
                    .mixer
                    .get_pad_template("sink_%u")
                    .expect("failed to get mixer pad template sink_%u"),
                None,
                None,
            )
            .ok_or_else(|| anyhow!("failed to request mixer template"))
    }

    pub fn link_decoder(&self, dec: AudioDecoder) -> Result<Pad, Error> {
        let pad = self.get_pad()?;
        dec.link_with(&pad);
        Ok(pad)
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "abx", about = "CLI utility to ABX audio files.")]
struct Opt {
    a: PathBuf,
    b: PathBuf,
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();

    gstreamer::init()?;
    let pipeline = AudioPipeline::new(Some("abx"));

    let a = AudioDecoder::new(&pipeline, opt.a)?;
    let b = AudioDecoder::new(&pipeline, opt.b)?;

    let mixer = AudioMixer::new(&pipeline)?;

    let a_pad = mixer.link_decoder(a.clone())?;
    let _b_pad = mixer.link_decoder(b)?;

    a_pad.set_property("mute", &true)?;

    let pipeline_th = pipeline;
    let soundth = std::thread::spawn(move || pipeline_th.run().unwrap());

    while let Some(t) = a.src.query_duration::<ClockTime>() {
        eprintln!("{}", t);
    }
    soundth.join().unwrap();
    Ok(())
}
