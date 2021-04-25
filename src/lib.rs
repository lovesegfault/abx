use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    thread::{spawn, JoinHandle},
};

use anyhow::{anyhow, bail, Context, Error};
use gstreamer::{prelude::*, ClockTime, Element, ElementFactory, Pad, Pipeline, State};

#[derive(Clone)]
struct AudioPipeline {
    pipeline: Pipeline,
    mixer: Element,
    sources: HashMap<PathBuf, Pad>,
}

impl AudioPipeline {
    pub fn new() -> Result<Self, Error> {
        let pipeline = Pipeline::new(None);
        let mixer = ElementFactory::make("audiomixer", None)
            .with_context(|| "failed to create audiomixer")?;
        let sink = ElementFactory::make("autoaudiosink", None)
            .with_context(|| "failed to create autoaudiosink")?;
        let sources = HashMap::new();

        pipeline
            .add(&mixer)
            .with_context(|| "failed to add audiomixer to pipeline")?;
        pipeline
            .add(&sink)
            .with_context(|| "failed to add autoaudiosink to pipeline")?;

        mixer
            .link(&sink)
            .with_context(|| "failed to link audiomixer and autoaudiosink")?;

        Ok(Self {
            pipeline,
            mixer,
            sources,
        })
    }

    pub fn with_source<P: AsRef<Path>>(mut self, file: P) -> Result<Self, Error> {
        let src =
            ElementFactory::make("filesrc", None).with_context(|| "failed to create filesrc")?;
        src.set_property("location", &file.as_ref().to_str())
            .with_context(|| format!("failed to set filesrc location to {:?}", file.as_ref()))?;
        let dec = ElementFactory::make("decodebin", None)
            .with_context(|| "failed to create decodebin")?;

        self.pipeline
            .add(&src)
            .with_context(|| "failed to add filesrc to pipeline")?;

        self.pipeline
            .add(&dec)
            .with_context(|| "failed to add decodebin to pipeline")?;

        src.link(&dec)
            .with_context(|| "failed to link filesrc and decodebin")?;

        let mixer_pad = self
            .mixer
            .request_pad(
                &self
                    .mixer
                    .get_pad_template("sink_%u")
                    .expect("failed to get audiomixer pad template sink_%u"),
                None,
                None,
            )
            .with_context(|| "failed to get sink pad on audiomixer")?;

        {
            let mixer_pad = mixer_pad.clone();
            dec.connect_pad_added(move |_, pad| {
                pad.link(&mixer_pad)
                    .with_context(|| "failed to link decodebin and audiomixer")
                    .unwrap();
            });
        }

        self.sources.insert(file.as_ref().to_owned(), mixer_pad);

        Ok(self)
    }

    pub fn play(&self) -> Result<(), Error> {
        self.pipeline
            .set_state(State::Playing)
            .with_context(|| "failed to set AudioPipeline to Playing")
            .map(|_| ())
    }

    pub fn pause(&self) -> Result<(), Error> {
        self.pipeline
            .set_state(State::Paused)
            .with_context(|| "failed to set AudioPipeline to Playing")
            .map(|_| ())
    }

    pub fn run(&self) -> Result<JoinHandle<Result<(), Error>>, Error> {
        let pipeline = self.clone();
        let bus = pipeline
            .pipeline
            .get_bus()
            .with_context(|| "failed to get bus for AudioPipeline")?;

        let player = spawn(move || {
            while let Some(msg) = bus.timed_pop(ClockTime::none()) {
                use gstreamer::MessageView::*;
                match msg.view() {
                    Eos(_) => {
                        pipeline.pause()?;
                        break;
                    }
                    Error(e) => {
                        pipeline.pause()?;
                        bail!("received error while streaming: {:?}", e);
                    }
                    _ => continue,
                }
            }
            Ok(())
        });
        Ok(player)
    }
}
