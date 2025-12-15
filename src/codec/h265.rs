//! H.265/HEVC codec configuration extraction.
//!
//! **Status:** Stub - implementation coming in Slice 3.
//!
//! # Overview
//!
//! H.265 (HEVC) uses a similar NAL unit structure to H.264 but with
//! different NAL unit types and an additional VPS (Video Parameter Set).
//!
//! # NAL Unit Types (HEVC)
//!
//! | Type | Name | Purpose |
//! |------|------|---------|
//! | 19-20 | IDR_W_RADL/IDR_N_LP | Keyframe (IDR) |
//! | 32 | VPS | Video Parameter Set |
//! | 33 | SPS | Sequence Parameter Set |
//! | 34 | PPS | Picture Parameter Set |
//!
//! # Differences from H.264
//!
//! - NAL type is in bits 1-6 of the first byte (shifted by 1)
//! - Requires VPS in addition to SPS/PPS
//! - Configuration box is `hvcC` instead of `avcC`

use super::common::AnnexBNalIter;

/// H.265 NAL unit type constants.
pub mod nal_type {
    /// IDR with RADL pictures
    pub const IDR_W_RADL: u8 = 19;
    /// IDR without leading pictures
    pub const IDR_N_LP: u8 = 20;
    /// Video Parameter Set
    pub const VPS: u8 = 32;
    /// Sequence Parameter Set
    pub const SPS: u8 = 33;
    /// Picture Parameter Set
    pub const PPS: u8 = 34;
}

/// HEVC (H.265) codec configuration.
///
/// Contains VPS, SPS, and PPS NAL units needed to build the
/// hvcC box in MP4 containers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HevcConfig {
    /// Video Parameter Set NAL unit (without start code)
    pub vps: Vec<u8>,
    /// Sequence Parameter Set NAL unit (without start code)
    pub sps: Vec<u8>,
    /// Picture Parameter Set NAL unit (without start code)
    pub pps: Vec<u8>,
}

/// Extract the NAL unit type from an H.265 NAL header.
///
/// H.265 NAL type is in bits 1-6 of the first byte:
/// `(nal_header[0] >> 1) & 0x3f`
#[inline]
pub fn hevc_nal_type(nal: &[u8]) -> u8 {
    if nal.is_empty() {
        return 0;
    }
    (nal[0] >> 1) & 0x3f
}

/// Check if the given NAL type represents a keyframe.
#[inline]
pub fn is_hevc_keyframe_nal_type(nal_type: u8) -> bool {
    nal_type == nal_type::IDR_W_RADL || nal_type == nal_type::IDR_N_LP
}

/// Extract HEVC configuration from Annex B data.
///
/// **Not yet implemented** - returns `None`.
pub fn extract_hevc_config(_data: &[u8]) -> Option<HevcConfig> {
    // TODO: Implement in Slice 3
    // Will need to scan for VPS (32), SPS (33), PPS (34)
    None
}

/// Check if the given Annex B data represents an HEVC keyframe.
pub fn is_hevc_keyframe(data: &[u8]) -> bool {
    for nal in AnnexBNalIter::new(data) {
        if nal.is_empty() {
            continue;
        }
        let nal_type = hevc_nal_type(nal);
        if is_hevc_keyframe_nal_type(nal_type) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hevc_nal_type_extraction() {
        // VPS NAL: type 32, so first byte has (32 << 1) = 64 = 0x40
        let vps_nal = [0x40, 0x01, 0x0c];
        assert_eq!(hevc_nal_type(&vps_nal), 32);

        // SPS NAL: type 33, so first byte has (33 << 1) = 66 = 0x42
        let sps_nal = [0x42, 0x01, 0x01];
        assert_eq!(hevc_nal_type(&sps_nal), 33);
    }

    #[test]
    fn test_is_hevc_keyframe_nal_type() {
        assert!(is_hevc_keyframe_nal_type(nal_type::IDR_W_RADL));
        assert!(is_hevc_keyframe_nal_type(nal_type::IDR_N_LP));
        assert!(!is_hevc_keyframe_nal_type(nal_type::VPS));
        assert!(!is_hevc_keyframe_nal_type(nal_type::SPS));
    }

    #[test]
    fn test_extract_hevc_config_stub() {
        // Currently returns None (stub)
        assert!(extract_hevc_config(&[]).is_none());
    }
}
