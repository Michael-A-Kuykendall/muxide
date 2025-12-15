//! Codec configuration extraction for container muxing.
//!
//! This module provides minimal bitstream parsing required to build codec
//! configuration boxes (avcC, hvcC, av1C). It does NOT perform decoding,
//! transcoding, or frame reconstruction.
//!
//! # Supported Codecs
//!
//! - **H.264/AVC**: Extract SPS/PPS from Annex B NAL units
//! - **H.265/HEVC**: (stub - coming in Slice 3)
//! - **AV1**: (stub - coming in Slice 3)
//!
//! # Input Format
//!
//! All video input is expected in **Annex B** format (start code delimited):
//! - 4-byte start code: `0x00 0x00 0x00 0x01`
//! - 3-byte start code: `0x00 0x00 0x01`
//!
//! The muxer internally converts to length-prefixed format (AVCC) for MP4.

pub mod common;
pub mod h264;

// Stubs for future codec expansion (Slice 3)
pub mod h265;
pub mod av1;

pub use common::{find_start_code, AnnexBNalIter};
pub use h264::{AvcConfig, extract_avc_config, annexb_to_avcc, is_h264_keyframe};
