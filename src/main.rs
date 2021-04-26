use std::path::PathBuf;

use anyhow::Error;
use glib::MainLoop;
use structopt::StructOpt;

use abx::AudioPipeline;

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

    let _pipeline = AudioPipeline::new()?
        .with_source(&opt.a)?
        .with_source(&opt.b)?
        .with_mainloop(&main)?;

    main.run();

    Ok(())
}
