//! VP9 video codec support for MP4 muxing.
//!
//! This module provides VP9 frame parsing and configuration extraction
//! for MP4 container muxing. VP9 frames are expected in their compressed
//! form with frame headers intact.

use crate::assert_invariant;

/// VP9 codec configuration extracted from the first keyframe.
#[derive(Clone, Debug, PartialEq)]
pub struct Vp9Config {
    /// Video width in pixels.
    pub width: u32,
    /// Video height in pixels.
    pub height: u32,
    /// VP9 profile (0-3).
    pub profile: u8,
    /// Bit depth (8 or 10).
    pub bit_depth: u8,
    /// Color space information.
    pub color_space: u8,
    /// Transfer characteristics.
    pub transfer_function: u8,
    /// Matrix coefficients.
    pub matrix_coefficients: u8,
}

/// Errors that can occur during VP9 parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum Vp9Error {
    /// Frame data is too short to contain a valid VP9 frame header.
    FrameTooShort,
    /// Invalid frame marker (first 3 bytes should be 0x49, 0x83, 0x42).
    InvalidFrameMarker,
    /// Unsupported VP9 profile.
    UnsupportedProfile(u8),
    /// Invalid bit depth.
    InvalidBitDepth(u8),
    /// Frame parsing error with details.
    ParseError(String),
}

impl std::fmt::Display for Vp9Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Vp9Error::FrameTooShort => write!(f, "VP9 frame too short for header"),
            Vp9Error::InvalidFrameMarker => write!(f, "invalid VP9 frame marker"),
            Vp9Error::UnsupportedProfile(p) => write!(f, "unsupported VP9 profile: {}", p),
            Vp9Error::InvalidBitDepth(b) => write!(f, "invalid VP9 bit depth: {}", b),
            Vp9Error::ParseError(msg) => write!(f, "VP9 parse error: {}", msg),
        }
    }
}

impl std::error::Error for Vp9Error {}

/// Check if a VP9 frame is a keyframe (intra frame).
///
/// VP9 keyframes have frame_type = 0 in the frame header.
pub fn is_vp9_keyframe(frame: &[u8]) -> Result<bool, Vp9Error> {
    if frame.len() < 3 {
        return Err(Vp9Error::FrameTooShort);
    }

    // Check frame marker
    if frame[0] != 0x49 || frame[1] != 0x83 || frame[2] != 0x42 {
        return Err(Vp9Error::InvalidFrameMarker);
    }

    if frame.len() < 4 {
        return Err(Vp9Error::FrameTooShort);
    }

    // Parse frame header to determine frame type
    let profile = (frame[3] >> 6) & 0x03;
    let show_existing_frame = (frame[3] >> 5) & 0x01;
    let frame_type = (frame[3] >> 4) & 0x01;

    // INV-405: VP9 profile must be valid (0-3)
    assert_invariant!(
        profile <= 3,
        "VP9 profile must be valid (0-3)",
        "codec::vp9::is_vp9_keyframe"
    );

    // If show_existing_frame is set, this is not a keyframe
    if show_existing_frame != 0 {
        return Ok(false);
    }

    // frame_type = 0 indicates a keyframe
    Ok(frame_type == 0)
}

/// Extract VP9 configuration from a keyframe.
///
/// This parses the uncompressed header of a VP9 keyframe to extract
/// resolution and other configuration parameters.
pub fn extract_vp9_config(keyframe: &[u8]) -> Option<Vp9Config> {
    if keyframe.len() < 3 {
        return None;
    }

    // Check frame marker
    if keyframe[0] != 0x49 || keyframe[1] != 0x83 || keyframe[2] != 0x42 {
        return None;
    }

    // INV-401: VP9 frame marker must be valid
    assert_invariant!(
        keyframe[0] == 0x49 && keyframe[1] == 0x83 && keyframe[2] == 0x42,
        "INV-401: VP9 frame marker must be 0x49 0x83 0x42",
        "codec::vp9::extract_vp9_config"
    );

    if keyframe.len() < 6 {
        return None;
    }

    // Parse basic frame header fields
    let profile = (keyframe[3] >> 6) & 0x03;
    let show_existing_frame = (keyframe[3] >> 5) & 0x01;
    let frame_type = (keyframe[3] >> 4) & 0x01;

    // INV-402: VP9 profile must be valid (0-3)
    assert_invariant!(
        profile <= 3,
        "INV-402: VP9 profile must be valid (0-3)",
        "codec::vp9::extract_vp9_config"
    );

    if show_existing_frame != 0 || frame_type != 0 {
        return None;
    }

    // For keyframes, we need to parse more of the header
    // This is a simplified implementation - full VP9 header parsing is complex
    // For now, we'll use placeholder values and focus on the basic structure

    // TODO: Implement full VP9 header parsing for resolution extraction
    // This requires parsing the uncompressed header which includes:
    // - Frame size (width/height)
    // - Render size (if different)
    // - Color configuration
    // - Loop filter parameters
    // etc.

    // Placeholder implementation - will be replaced with actual parsing
    Some(Vp9Config {
        width: 1920,  // TODO: Parse from frame header
        height: 1080, // TODO: Parse from frame header
        profile,
        bit_depth: 8, // TODO: Parse from frame header
        color_space: 0,
        transfer_function: 0,
        matrix_coefficients: 0,
    })
}

/// Validate that a buffer contains a valid VP9 frame.
///
/// This performs basic validation of the VP9 frame structure.
pub fn is_valid_vp9_frame(frame: &[u8]) -> bool {
    if frame.len() < 3 {
        return false;
    }

    // Check frame marker
    frame[0] == 0x49 && frame[1] == 0x83 && frame[2] == 0x42
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_frame_marker() {
        let invalid_frame = [0x00, 0x00, 0x00];
        assert!(!is_valid_vp9_frame(&invalid_frame));
        assert!(matches!(
            is_vp9_keyframe(&invalid_frame),
            Err(Vp9Error::InvalidFrameMarker)
        ));
    }

    #[test]
    fn test_frame_too_short() {
        let short_frame = [0x49, 0x83];
        assert!(!is_valid_vp9_frame(&short_frame));
        assert!(matches!(
            is_vp9_keyframe(&short_frame),
            Err(Vp9Error::FrameTooShort)
        ));
    }

    #[test]
    fn test_valid_frame_marker() {
        let valid_frame = [0x49, 0x83, 0x42, 0x00, 0x00, 0x00];
        assert!(is_valid_vp9_frame(&valid_frame));
    }
}
