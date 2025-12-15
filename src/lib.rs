//! Muxide library crate entry point.
//!
//! This crate exposes a high-level API for muxing encoded audio and video
//! streams into container formats.  The focus of the initial version is on
//! writing MP4 files with H.264 video and optional AAC audio, with a strong
//! emphasis on correctness and compatibility.  See the `docs/charter.md`
//! and `docs/contract.md` for detailed information about the goals,
//! non-goals and API invariants.

mod muxer;

// Re-export the API module so users can simply `use muxide::api::...`.
pub mod api;
