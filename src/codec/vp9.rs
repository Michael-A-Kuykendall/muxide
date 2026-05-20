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
    /// VP9 level (0-255, typically 0 for most content).
    pub level: u8,
    /// Video full range flag (0 = limited range, 1 = full range).
    pub full_range_flag: u8,
}

/// Errors that can occur during VP9 parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum Vp9Error {
    /// Frame data is too short to contain a valid VP9 frame header.
    FrameTooShort,
    /// Invalid frame marker (top 2 bits of first byte must be `10`, value 2).
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

/// Minimal bit reader for VP9 uncompressed frame-header parsing.
///
/// VP9 packs all header fields at the bit level (MSB first), so byte-level
/// indexing into the raw frame data produces wrong results.  This reader
/// consumes bits one at a time from the MSB of each byte.
struct Vp9BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8, // 0 = MSB of current byte, 7 = LSB
}

impl<'a> Vp9BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, byte_pos: 0, bit_pos: 0 }
    }

    fn read_bit(&mut self) -> Option<u8> {
        if self.byte_pos >= self.data.len() {
            return None;
        }
        let bit = (self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Some(bit)
    }

    fn read_bits(&mut self, count: usize) -> Option<u32> {
        let mut val = 0u32;
        for _ in 0..count {
            val = (val << 1) | (self.read_bit()? as u32);
        }
        Some(val)
    }
}

/// Check if a VP9 frame is a keyframe (intra frame).
///
/// VP9 keyframes have `frame_type = 0` in the uncompressed header.
/// The header is bit-packed (MSB first); the mandatory `frame_marker`
/// occupies the two most-significant bits of the first byte and must
/// equal 2 (`0b10`).
pub fn is_vp9_keyframe(frame: &[u8]) -> Result<bool, Vp9Error> {
    if frame.is_empty() {
        return Err(Vp9Error::FrameTooShort);
    }

    let mut r = Vp9BitReader::new(frame);

    // frame_marker: 2 bits, must be 2 (binary 10)
    let frame_marker = r.read_bits(2).ok_or(Vp9Error::FrameTooShort)?;
    if frame_marker != 2 {
        return Err(Vp9Error::InvalidFrameMarker);
    }

    // profile_low_bit (1) + profile_high_bit (1)
    let profile_low = r.read_bit().ok_or(Vp9Error::FrameTooShort)?;
    let profile_high = r.read_bit().ok_or(Vp9Error::FrameTooShort)?;
    let profile = (profile_high << 1) | profile_low;

    // INV-405: VP9 profile must be valid (0-3)
    assert_invariant!(
        profile <= 3,
        "VP9 profile must be valid (0-3)",
        "codec::vp9::is_vp9_keyframe"
    );

    // Profile 3 inserts a reserved_zero_bit before show_existing_frame
    if profile == 3 {
        r.read_bit().ok_or(Vp9Error::FrameTooShort)?;
    }

    // show_existing_frame: 1 bit
    let show_existing = r.read_bit().ok_or(Vp9Error::FrameTooShort)?;
    if show_existing != 0 {
        return Ok(false);
    }

    // frame_type: 1 bit (0 = KEY_FRAME, 1 = NON_KEY_FRAME)
    let frame_type = r.read_bit().ok_or(Vp9Error::FrameTooShort)?;
    Ok(frame_type == 0)
}

/// Extract VP9 configuration from a keyframe.
///
/// Parses the uncompressed header of a VP9 keyframe (bit-level, MSB first)
/// to extract the `vpcC` box fields required by the MP4 container.
///
/// Returns `None` if the data is not a valid VP9 keyframe with an intact
/// frame-sync code (`0x49 0x83 0x42`).
pub fn extract_vp9_config(keyframe: &[u8]) -> Option<Vp9Config> {
    if keyframe.len() < 4 {
        return None;
    }

    let mut r = Vp9BitReader::new(keyframe);

    // frame_marker: 2 bits, must be 2
    let frame_marker = r.read_bits(2)?;
    if frame_marker != 2 {
        return None;
    }

    // INV-401: VP9 frame_marker must be 2
    assert_invariant!(
        frame_marker == 2,
        "INV-401: VP9 frame_marker must be 2",
        "codec::vp9::extract_vp9_config"
    );

    // profile
    let profile_low = r.read_bit()?;
    let profile_high = r.read_bit()?;
    let profile = (profile_high << 1) | profile_low;

    // INV-402: VP9 profile must be valid (0-3)
    assert_invariant!(
        profile <= 3,
        "INV-402: VP9 profile must be valid (0-3)",
        "codec::vp9::extract_vp9_config"
    );

    // Profile 3 reserved_zero_bit
    if profile == 3 {
        r.read_bit()?;
    }

    // show_existing_frame
    let show_existing = r.read_bit()?;
    if show_existing != 0 {
        return None;
    }

    // frame_type: must be 0 (KEY_FRAME)
    let frame_type = r.read_bit()?;
    if frame_type != 0 {
        return None;
    }

    // show_frame + error_resilient_mode (skip)
    r.read_bit()?;
    r.read_bit()?;

    // frame_sync_code: 3 bytes (0x49, 0x83, 0x42)
    let sync0 = r.read_bits(8)?;
    let sync1 = r.read_bits(8)?;
    let sync2 = r.read_bits(8)?;
    if sync0 != 0x49 || sync1 != 0x83 || sync2 != 0x42 {
        return None;
    }

    // color_config
    // bit_depth: fixed at 8 for profiles 0/1; read ten_or_twelve_bit for profiles 2/3
    let bit_depth: u8 = if profile >= 2 {
        let ten_or_twelve = r.read_bit()?;
        if ten_or_twelve != 0 { 12 } else { 10 }
    } else {
        8
    };

    // color_space: 3 bits (MSB first)
    let color_space = r.read_bits(3)? as u8;

    // full_range_flag (color_range in VP9 spec): 1 bit when color_space != sRGB (7)
    // For profiles 1/3 there are also subsampling bits, which we skip.
    let full_range_flag: u8 = if color_space != 7 {
        let cr = r.read_bit()? as u8;
        if profile == 1 || profile == 3 {
            r.read_bit()?; // subsampling_x
            r.read_bit()?; // subsampling_y
            r.read_bit()?; // reserved_zero
        }
        cr
    } else {
        // sRGB: color_range is implicitly full (1)
        1
    };

    // frame_size: fixed 16-bit fields (frame_width_minus_1, frame_height_minus_1)
    let width_minus_1 = r.read_bits(16)?;
    let height_minus_1 = r.read_bits(16)?;

    Some(Vp9Config {
        width: width_minus_1 + 1,
        height: height_minus_1 + 1,
        profile,
        bit_depth,
        color_space,
        transfer_function: 0,
        matrix_coefficients: 0,
        level: 0,
        full_range_flag,
    })
}

/// Validate that a buffer contains a valid VP9 frame.
///
/// Returns `true` when the top two bits of the first byte equal `10`
/// (the mandatory VP9 `frame_marker` value of 2).
pub fn is_valid_vp9_frame(frame: &[u8]) -> bool {
    if frame.is_empty() {
        return false;
    }
    // VP9 frame_marker occupies the two most-significant bits and must be 2
    (frame[0] >> 6) == 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_frame_marker() {
        // 0x00 >> 6 = 0 ≠ 2 — invalid
        let invalid_frame = [0x00, 0x00, 0x00];
        assert!(!is_valid_vp9_frame(&invalid_frame));
        assert!(matches!(
            is_vp9_keyframe(&invalid_frame),
            Err(Vp9Error::InvalidFrameMarker)
        ));
    }

    #[test]
    fn test_frame_too_short() {
        let empty_frame: &[u8] = &[];
        assert!(!is_valid_vp9_frame(empty_frame));
        assert!(matches!(
            is_vp9_keyframe(empty_frame),
            Err(Vp9Error::FrameTooShort)
        ));
    }

    #[test]
    fn test_valid_frame_marker() {
        // 0x82 = 10000010 — top 2 bits are 10 = frame_marker 2 ✓
        let valid_frame = [0x82, 0x49, 0x83, 0x42, 0x00, 0x00];
        assert!(is_valid_vp9_frame(&valid_frame));
    }

    #[test]
    fn test_is_vp9_keyframe_valid() {
        // 0x82 = frame_marker(10) profile(00) show_existing(0) frame_type(0=KEY) show_frame(1) err_resilient(0)
        let keyframe = [0x82];
        assert_eq!(is_vp9_keyframe(&keyframe), Ok(true));
    }

    #[test]
    fn test_is_vp9_keyframe_pframe() {
        // 0x84 = frame_marker(10) profile(00) show_existing(0) frame_type(1=INTER) ...
        let pframe = [0x84];
        assert_eq!(is_vp9_keyframe(&pframe), Ok(false));
    }

    #[test]
    fn test_is_vp9_keyframe_show_existing() {
        // 0x88 = frame_marker(10) profile(00) show_existing(1) ...
        let show_existing = [0x88];
        assert_eq!(is_vp9_keyframe(&show_existing), Ok(false));
    }

    #[test]
    fn test_extract_vp9_config_valid() {
        // Profile 0, 256×256, color_space=0 (unspecified), limited range.
        //
        // Bit layout after byte 0 (0x82) and sync code (bytes 1-3):
        //   byte 4:  color_space(000) color_range(0) width_minus1[15:12](0000)  = 0x00
        //   byte 5:  width_minus1[11:4](00001111)                               = 0x0F
        //   byte 6:  width_minus1[3:0](1111) height_minus1[15:12](0000)         = 0xF0
        //   byte 7:  height_minus1[11:4](00001111)                              = 0x0F
        //   byte 8:  height_minus1[3:0](1111) render_same(0) ...               = 0xF0
        let keyframe = vec![
            0x82, 0x49, 0x83, 0x42, // frame header byte + sync code
            0x00, 0x0F, 0xF0, 0x0F, 0xF0, // color_config + frame_size (256×256)
        ];
        let config = extract_vp9_config(&keyframe).unwrap();
        assert_eq!(config.width, 256);
        assert_eq!(config.height, 256);
        assert_eq!(config.profile, 0);
        assert_eq!(config.bit_depth, 8);
        assert_eq!(config.level, 0);
        assert_eq!(config.full_range_flag, 0);
    }

    #[test]
    fn test_extract_vp9_config_invalid_marker() {
        // 0x00 >> 6 = 0 ≠ 2
        let invalid_frame = [0x00, 0x00, 0x00, 0x00];
        assert!(extract_vp9_config(&invalid_frame).is_none());
    }

    #[test]
    fn test_extract_vp9_config_pframe() {
        // 0x84 = profile 0, inter frame — extract_vp9_config must return None
        let pframe = [0x84, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(extract_vp9_config(&pframe).is_none());
    }
}
