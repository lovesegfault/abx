pub mod events;

use std::{
    ops::Deref,
    path::{Path, PathBuf},
    sync::{atomic::AtomicUsize, Arc, Mutex},
    thread::JoinHandle,
};

use anyhow::{Context, Error};
use gstreamer::{prelude::*, ClockTime, Element, ElementFactory, Pipeline, State};

#[derive(Clone)]
pub struct AudioSource {
    pub path: PathBuf,
    sink: Element,
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
        let sink = ElementFactory::make("pulsesink", None)
            .with_context(|| "failed to create pulsesink")?;

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
            .add(&sink)
            .with_context(|| "failed to add autoaudiosink to pipeline")?;

        src.link(&dec)
            .with_context(|| "failed to link filesrc and decodebin")?;

        {
            let sink_pad = sink
                .sink_pads()
                .get(0)
                .with_context(|| "failed to get sink pad for pulsesink")?
                .clone();
            dec.connect_pad_added(move |_, pad| {
                pad.link(&sink_pad)
                    .expect("failed to link decodebin and pulsesink");
            });
        }

        Ok(AudioSource { path, sink })
    }

    pub fn mute(&self) -> Result<(), Error> {
        self.sink
            .set_property("mute", &true)
            .with_context(|| "failed to mute AudioSource pad")
    }

    pub fn unmute(&self) -> Result<(), Error> {
        self.sink
            .set_property("mute", &false)
            .with_context(|| "failed to unmute AudioSource pad")
    }
}

#[derive(Clone)]
pub struct AudioSelector {
    pipeline: Pipeline,
    runner: Option<Arc<JoinHandle<Result<(), Error>>>>,
    pub sources: Arc<Mutex<Vec<AudioSource>>>,
    pub selected: Arc<AtomicUsize>,
}

impl AudioSelector {
    pub fn new() -> Result<Self, Error> {
        let pipeline = Pipeline::new(None);
        let sources = Arc::new(Mutex::new(Vec::new()));

        Ok(Self {
            pipeline,
            runner: None,
            sources,
            selected: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn with_source<P: AsRef<Path>>(self, file: P) -> Result<Self, Error> {
        let src = AudioSource::new(file, &self)?;
        src.mute()?;

        self.sources
            .lock()
            .expect("sources lock is poisoned")
            .push(src);

        Ok(self)
    }

    pub fn run(mut self) -> Result<Self, Error> {
        let pipeline = self.pipeline.clone();
        let runner = std::thread::spawn(move || {
            let bus = pipeline
                .bus()
                .with_context(|| "failed to get bus for AudioPipeline")?;

            use gstreamer::MessageView::*;
            while let Some(msg) = bus.pop() {
                match msg.view() {
                    Eos(_) => {
                        pipeline
                            .set_state(State::Null)
                            .map(|_| ())
                            .with_context(|| "failed to set AudioPipeline to Null")?;
                        break;
                    }
                    Error(e) => {
                        // FIXME
                        eprintln!("{:?}", e);
                        pipeline
                            .set_state(State::Null)
                            .map(|_| ())
                            .with_context(|| "failed to set AudioPipeline to Null")?;
                        break;
                    }
                    _ => (),
                }
            }

            Ok(())
        });

        self.runner = Some(Arc::new(runner));

        self.pipeline
            .set_state(State::Playing)
            .with_context(|| "failed to set AudioPipeline to Playing")?;
        self.sources
            .lock()
            .expect("sources lock is poisoned")
            .get(0)
            .map(|src| src.unmute())
            .transpose()?;

        Ok(self)
    }

    pub fn play(&self) -> Result<(), Error> {
        self.pipeline
            .set_state(State::Playing)
            .map(|_| ())
            .with_context(|| "failed to set AudioPipeline to Playing")
    }

    pub fn pause(&self) -> Result<(), Error> {
        self.pipeline
            .set_state(State::Paused)
            .map(|_| ())
            .with_context(|| "failed to set AudioPipeline to Paused")
    }

    pub fn toggle(&self) -> Result<(), Error> {
        match self.pipeline.current_state() {
            State::Playing => {
                self.pause()
            }
            State::Paused => {
                self.play()
            }
            _ => {
                Ok(())
            }
        }
    }

    pub fn progress(&self) -> Result<f64, Error> {
        let duration: f64 = *self
            .pipeline
            .query_duration::<ClockTime>()
            .with_context(|| "failed to query pipeline duration")?
            .deref() as f64;
        let position: f64 = *self
            .pipeline
            .query_position::<ClockTime>()
            .with_context(|| "failed to query pipeline position")?
            .deref() as f64;
        Ok((position / duration) * 100.0)
    }

    pub fn select_source(&mut self, source: usize) -> Result<(), Error> {
        let sources = self.sources.lock().expect("sources lock is poisoned");
        sources
            .get(self.selected.load(std::sync::atomic::Ordering::SeqCst))
            .map(|src| src.mute())
            .transpose()?;
        sources.get(source).map(|src| src.unmute()).transpose()?;

        self.selected
            .store(source, std::sync::atomic::Ordering::SeqCst);

        Ok(())
    }

    pub fn next_source(&mut self) -> Result<(), Error> {
        let sources = self.sources.lock().expect("sources lock is poisoned");
        let selected = self.selected.load(std::sync::atomic::Ordering::SeqCst);
        let idx = (selected + 1) & (sources.len() - 1);
        drop(sources);
        self.select_source(idx)?;
        Ok(())
    }
}
