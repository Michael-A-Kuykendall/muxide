# Muxide

**Muxide** is a recording‑oriented multimedia container writer for Rust.  Its goal is to provide a simple, ergonomic API for muxing encoded video and audio frames into an MP4 container with real‑world playback guarantees.

This crate is built following the principles of the *Slice‑Gated Engineering Doctrine*, where work is broken into small, verifiable slices with clear acceptance gates.  See the `docs/charter.md` and `docs/contract.md` for the high‑level goals and API contract of the project.

> *This README only explains what the project aims to be.  Implementation details and API stability are driven by the charter and contract documents.*

## Quick start

```rust
use muxide::api::{Muxer, MuxerConfig};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let out = File::create("out.mp4")?;
	let config = MuxerConfig::new(1920, 1080, 30.0);
	let mut muxer = Muxer::new(out, config)?;

	// muxer.write_video(pts_secs, annex_b_bytes, is_keyframe)?;

	let stats = muxer.finish_with_stats()?;
	println!("wrote {} bytes", stats.bytes_written);
	Ok(())
}
```