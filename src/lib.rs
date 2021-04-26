use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Error};
use glib::MainLoop;
use gstreamer::{prelude::*, Element, ElementFactory, Pad, Pipeline, State};

#[derive(Clone)]
pub struct AudioPipeline {
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
                    .expect("failed to link decodebin and audiomixer");
            });
        }

        self.sources.insert(file.as_ref().to_owned(), mixer_pad);

        Ok(self)
    }

    pub fn with_mainloop(self, main: &MainLoop) -> Result<Self, Error> {
        let bus = self
            .pipeline
            .get_bus()
            .with_context(|| "failed to get bus for AudioPipeline")?;
        self.play()?;

        let main = main.clone();
        bus.add_watch(move |_, msg| {
            use gstreamer::MessageView::*;
            match msg.view() {
                Eos(_) => main.quit(),
                Error(e) => {
                    // FIXME
                    eprintln!("{:?}", e);
                    main.quit();
                }
                _ => (),
            }

            glib::Continue(true)
        })
        .with_context(|| "failed to add bus watch to pipeline")?;

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
}

impl Drop for AudioPipeline {
    fn drop(&mut self) {
        self.pipeline
            .set_state(State::Null)
            .map(|_| ())
            .unwrap_or_else(|_| {
                eprintln!("failed to set pipeline state to Null");
            });
        let bus = self
            .pipeline
            .get_bus()
            .expect("failed to get bus for AudioPipeline");
        bus.remove_watch()
            .expect("failed to remove watch from AudioPipeline bus");
    }
}
