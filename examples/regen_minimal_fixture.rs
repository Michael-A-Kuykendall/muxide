use muxide::api::{MuxerBuilder, VideoCodec};
use std::{fs::File, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/minimal.mp4");
    let file = File::create(&out)?;

    let muxer = MuxerBuilder::new(file)
        .video(VideoCodec::H264, 640, 480, 30.0)
        .build()?;

    muxer.finish()?;

    let size = std::fs::metadata(&out)?.len();
    println!("Wrote {} bytes to {}", size, out.display());
    Ok(())
}
