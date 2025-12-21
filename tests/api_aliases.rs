//! Test the new API methods

use muxide::api::{MuxerBuilder, VideoCodec};
use muxide::fragmented::FragmentedMuxer;

#[test]
fn test_new_with_fragment_creates_fragmented_muxer() {
    let writer: Vec<u8> = Vec::new();
    let result = MuxerBuilder::new(writer)
        .video(VideoCodec::H264, 1920, 1080, 30.0)
        .new_with_fragment();

    assert!(result.is_ok());
    let mut muxer: FragmentedMuxer = result.unwrap();

    // Verify init segment is available
    let init_segment = muxer.init_segment();
    assert!(!init_segment.is_empty());
    // Check that it contains "ftyp" box (may not be at the start due to box structure)
    assert!(init_segment.windows(4).any(|w| w == b"ftyp"));
}

#[test]
fn test_flush_alias_for_finish() {
    let writer: Vec<u8> = Vec::new();
    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::H264, 1920, 1080, 30.0)
        .build()
        .unwrap();

    // Write a minimal video frame
    let sps_pps = vec![
        0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x0A, 0xF8, 0x41, 0xA2, // SPS
        0x00, 0x00, 0x00, 0x01, 0x68, 0xCE, 0x38, 0x80, // PPS
    ];
    let keyframe = vec![
        0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00, 0x20, // IDR slice
    ];
    let mut frame_data = sps_pps;
    frame_data.extend(keyframe);

    muxer.write_video(0.0, &frame_data, true).unwrap();

    // Test that flush() works as an alias for finish()
    let result = muxer.flush();
    assert!(result.is_ok());
}
