use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use anyhow::{Context, Error};
use glib::MainLoop;
use gstreamer::{prelude::*, Element, ElementFactory, Pad, Pipeline, State};

#[derive(Clone)]
pub struct AudioSource {
    path: PathBuf,
    pad: Pad,
}

impl AudioSource {
    pub fn new<P: AsRef<Path>>(path: P, selector: &AudioSelector) -> Result<Self, Error> {
        let path = path.as_ref().to_owned();
        let src =
            ElementFactory::make("filesrc", None).with_context(|| "failed to create filesrc")?;
        src.set_property("location", &path.to_str())
            .with_context(|| format!("failed to set filesrc location to {:?}", &path))?;
        let dec = ElementFactory::make("decodebin", None)
            .with_context(|| "failed to create decodebin")?;

        selector
            .pipeline
            .add(&src)
            .with_context(|| "failed to add filesrc to pipeline")?;

        selector
            .pipeline
            .add(&dec)
            .with_context(|| "failed to add decodebin to pipeline")?;

        src.link(&dec)
            .with_context(|| "failed to link filesrc and decodebin")?;

        let pad = selector
            .mixer
            .request_pad(
                &selector
                    .mixer
                    .get_pad_template("sink_%u")
                    .expect("failed to get audiomixer pad template sink_%u"),
                None,
                None,
            )
            .with_context(|| "failed to get sink pad on audiomixer")?;

        {
            let mixer_pad = pad.clone();
            dec.connect_pad_added(move |_, pad| {
                pad.link(&mixer_pad)
                    .expect("failed to link decodebin and audiomixer");
            });
        }

        Ok(AudioSource { path, pad })
    }

    pub fn mute(&self) -> Result<(), Error> {
        self.pad
            .set_property("mute", &true)
            .with_context(|| "failed to mute AudioSource pad")
    }

    pub fn unmute(&self) -> Result<(), Error> {
        self.pad
            .set_property("mute", &false)
            .with_context(|| "failed to unmute AudioSource pad")
    }
}

#[derive(Clone)]
pub struct AudioSelector {
    pipeline: Pipeline,
    mixer: Element,
    sources: VecDeque<AudioSource>,
}

impl AudioSelector {
    pub fn new() -> Result<Self, Error> {
        let pipeline = Pipeline::new(None);
        let mixer = ElementFactory::make("audiomixer", None)
            .with_context(|| "failed to create audiomixer")?;
        let sink = ElementFactory::make("autoaudiosink", None)
            .with_context(|| "failed to create autoaudiosink")?;
        let sources = VecDeque::new();

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
        let src = AudioSource::new(file, &self)?;
        src.mute()?;

        self.sources.push_back(src);

        Ok(self)
    }

    pub fn play(self) -> Result<Self, Error> {
        self.pipeline
            .set_state(State::Playing)
            .with_context(|| "failed to set AudioPipeline to Playing")?;
        self.sources.get(0).map(|src| src.unmute()).transpose()?;
        Ok(self)
    }

    pub fn with_mainloop(self, main: &MainLoop) -> Result<Self, Error> {
        let bus = self
            .pipeline
            .get_bus()
            .with_context(|| "failed to get bus for AudioPipeline")?;

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
}

impl Drop for AudioSelector {
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
