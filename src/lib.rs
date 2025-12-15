//! Muxide library crate entry point.
//!
//! Muxide is a recording-oriented MP4 muxer.
//!
//! - Input: encoded H.264 Annex B frames (`0x00000001` start codes) and optional AAC-in-ADTS.
//! - Output: an MP4 (ISOBMFF) file with a keyframe index (`stss`).
//!
//! See `docs/charter.md` and `docs/contract.md` for invariants.
//!
//! # Example
//!
//! ```no_run
//! use muxide::api::{Muxer, MuxerConfig};
//! use std::fs::File;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let file = File::create("out.mp4")?;
//! let config = MuxerConfig::new(1920, 1080, 30.0);
//! let mut muxer = Muxer::new(file, config)?;
//!
//! // Write frames (encoded elsewhere).
//! // muxer.write_video(pts_secs, annex_b_bytes, is_keyframe)?;
//!
//! let _stats = muxer.finish_with_stats()?;
//! # Ok(())
//! # }
//! ```

mod muxer;

// Re-export the API module so users can simply `use muxide::api::...`.
pub mod api;
