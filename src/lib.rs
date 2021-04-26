use std::path::{Path, PathBuf};

use anyhow::{Context, Error};
use glib::MainLoop;
use gstreamer::{prelude::*, Element, ElementFactory, Pad, Pipeline, State};

#[derive(Clone)]
pub struct AudioSource {
    path: PathBuf,
    volume: Element,
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
        let volume =
            ElementFactory::make("volume", None).with_context(|| "failed to create volume")?;
        let sink = ElementFactory::make("autoaudiosink", None)
            .with_context(|| "failed to create autoaudiosink")?;

        selector
            .pipeline
            .add(&src)
            .with_context(|| "failed to add filesrc to pipeline")?;

        selector
            .pipeline
            .add(&dec)
            .with_context(|| "failed to add decodebin to pipeline")?;

        selector
            .pipeline
            .add(&volume)
            .with_context(|| "failed to add volume to pipeline")?;

        selector
            .pipeline
            .add(&sink)
            .with_context(|| "failed to add autoaudiosink to pipeline")?;

        src.link(&dec)
            .with_context(|| "failed to link filesrc and decodebin")?;

        {
            let volume_pad = volume
                .get_sink_pads()
                .get(0)
                .with_context(|| "failed to get src pad for volume")?
                .clone();
            dec.connect_pad_added(move |_, pad| {
                pad.link(&volume_pad)
                    .expect("failed to link decodebin and volume");
            });
        }

        volume
            .link(&sink)
            .with_context(|| "failed to link volume and autoaudiosink")?;

        Ok(AudioSource { path, volume })
    }

    pub fn mute(&self) -> Result<(), Error> {
        self.volume
            .set_property("mute", &true)
            .with_context(|| "failed to mute AudioSource pad")
    }

    pub fn unmute(&self) -> Result<(), Error> {
        self.volume
            .set_property("mute", &false)
            .with_context(|| "failed to unmute AudioSource pad")
    }
}

#[derive(Clone)]
pub struct AudioSelector {
    pipeline: Pipeline,
    sources: Vec<AudioSource>,
    selected: usize,
}

impl AudioSelector {
    pub fn new() -> Result<Self, Error> {
        let pipeline = Pipeline::new(None);
        let sources = Vec::new();

        Ok(Self {
            pipeline,
            sources,
            selected: 0,
        })
    }

    pub fn with_source<P: AsRef<Path>>(mut self, file: P) -> Result<Self, Error> {
        let src = AudioSource::new(file, &self)?;
        src.mute()?;

        self.sources.push(src);

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

    pub fn play(self) -> Result<Self, Error> {
        self.pipeline
            .set_state(State::Playing)
            .with_context(|| "failed to set AudioPipeline to Playing")?;
        self.sources.get(0).map(|src| src.unmute()).transpose()?;
        Ok(self)
    }

    pub fn stop(&self) -> Result<(), Error> {
        self.pipeline
            .set_state(State::Paused)
            .map(|_| ())
            .with_context(|| "failed to set AudioPipeline to Paused")?;
        self.pipeline
            .set_state(State::Null)
            .map(|_| ())
            .with_context(|| "failed to set AudioPipeline to Null")
    }

    pub fn select_source(&mut self, source: usize) -> Result<(), Error> {
        self.sources
            .get(self.selected)
            .map(|src| src.mute())
            .transpose()?;
        self.sources
            .get(source)
            .map(|src| src.unmute())
            .transpose()?;

        self.selected = source;
        Ok(())
    }

    pub fn next_source(&mut self) -> Result<(), Error> {
        let idx = (self.selected + 1) & (self.sources.len() - 1);
        self.select_source(idx)?;
        Ok(())
    }
}
