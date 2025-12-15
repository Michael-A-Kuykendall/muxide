//! AV1 codec configuration extraction.
//!
//! **Status:** Stub - implementation coming in Slice 3.
//!
//! # Overview
//!
//! AV1 uses OBU (Open Bitstream Unit) framing instead of NAL units.
//! Configuration requires extracting the Sequence Header OBU.
//!
//! # OBU Types
//!
//! | Type | Name | Purpose |
//! |------|------|---------|
//! | 1 | OBU_SEQUENCE_HEADER | Sequence configuration |
//! | 3 | OBU_FRAME_HEADER | Frame metadata |
//! | 6 | OBU_FRAME | Complete frame |
//!
//! # Key Differences from H.264/H.265
//!
//! - No start codes; uses length-prefixed OBUs
//! - OBU header is 1-2 bytes (has_extension flag)
//! - Configuration box is `av1C`
//! - Keyframes are identified by `frame_type == KEY_FRAME` in header

/// AV1 OBU type constants.
pub mod obu_type {
    /// Sequence Header OBU
    pub const SEQUENCE_HEADER: u8 = 1;
    /// Frame Header OBU
    pub const FRAME_HEADER: u8 = 3;
    /// Frame OBU (contains header + tile data)
    pub const FRAME: u8 = 6;
}

/// AV1 codec configuration.
///
/// Contains the Sequence Header OBU needed to build the
/// av1C box in MP4 containers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Av1Config {
    /// Raw Sequence Header OBU bytes
    pub sequence_header: Vec<u8>,
}

/// Extract the OBU type from an OBU header byte.
///
/// OBU type is in bits 3-6 of the first byte:
/// `(obu_header >> 3) & 0x0f`
#[inline]
pub fn obu_type(header_byte: u8) -> u8 {
    (header_byte >> 3) & 0x0f
}

/// Check if the OBU header has an extension byte.
#[inline]
pub fn obu_has_extension(header_byte: u8) -> bool {
    (header_byte & 0x04) != 0
}

/// Check if the OBU has a size field.
#[inline]
pub fn obu_has_size(header_byte: u8) -> bool {
    (header_byte & 0x02) != 0
}

/// Extract AV1 configuration from bitstream data.
///
/// **Not yet implemented** - returns `None`.
pub fn extract_av1_config(_data: &[u8]) -> Option<Av1Config> {
    // TODO: Implement in Slice 3
    // Will need to parse OBU structure and find Sequence Header
    None
}

/// Check if the given data contains an AV1 keyframe.
///
/// **Not yet implemented** - returns `false`.
pub fn is_av1_keyframe(_data: &[u8]) -> bool {
    // TODO: Implement in Slice 3
    // Requires parsing frame header to check frame_type
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obu_type_extraction() {
        // OBU type 1 (Sequence Header): (1 << 3) = 8 = 0x08
        let seq_header = 0x08;
        assert_eq!(obu_type(seq_header), 1);

        // OBU type 6 (Frame): (6 << 3) = 48 = 0x30
        let frame = 0x30;
        assert_eq!(obu_type(frame), 6);
    }

    #[test]
    fn test_obu_flags() {
        // OBU with extension: bit 2 set
        let with_ext = 0x04;
        assert!(obu_has_extension(with_ext));
        assert!(!obu_has_extension(0x00));

        // OBU with size: bit 1 set
        let with_size = 0x02;
        assert!(obu_has_size(with_size));
        assert!(!obu_has_size(0x00));
    }

    #[test]
    fn test_extract_av1_config_stub() {
        // Currently returns None (stub)
        assert!(extract_av1_config(&[]).is_none());
    }

    #[test]
    fn test_is_av1_keyframe_stub() {
        // Currently returns false (stub)
        assert!(!is_av1_keyframe(&[]));
    }
}
