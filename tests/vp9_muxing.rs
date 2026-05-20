mod support;

use muxide::api::{MuxerBuilder, VideoCodec};
use support::SharedBuffer;

/// Build a minimal VP9 keyframe with a valid uncompressed header.
///
/// Byte layout (bit-packed, MSB first per the VP9 spec):
///   Byte 0 (0x82): frame_marker=2, profile=0, show_existing=0,
///                  frame_type=0 (KEY), show_frame=1, error_resilient=0
///   Bytes 1-3:     frame_sync_code 0x49 0x83 0x42
///   Byte 4 (0x40): color_space=2 (BT.709), color_range=0, width_minus1[15:12]=0
///   Byte 5 (0x06): width_minus1[11:4]   → width_minus1 = 99 → width = 100
///   Byte 6 (0x30): width_minus1[3:0]=3, height_minus1[15:12]=0
///   Byte 7 (0x06): height_minus1[11:4]  → height_minus1 = 99 → height = 100
///   Byte 8 (0x30): height_minus1[3:0]=3, render_same=0, padding
fn build_vp9_keyframe() -> Vec<u8> {
    vec![
        0x82, 0x49, 0x83, 0x42, // frame header byte + frame_sync_code
        0x40, 0x06, 0x30, 0x06, 0x30, // color_config + frame_size (100×100, BT.709)
        0x00, 0x00, 0x00, 0x00, // minimal compressed payload
    ]
}

/// Build a minimal VP9 inter (P-) frame.
///
/// Byte 0 (0x86): frame_marker=2, profile=0, show_existing=0,
///                frame_type=1 (INTER), show_frame=1, error_resilient=0
fn build_vp9_pframe() -> Vec<u8> {
    vec![0x86, 0x00, 0x00]
}

/// Recursively search for a 4CC in an MP4 container by pattern matching
fn contains_box(data: &[u8], fourcc: &[u8; 4]) -> bool {
    data.windows(4).any(|window| window == fourcc)
}

#[test]
fn vp9_first_frame_must_be_keyframe() {
    let (writer, _buffer) = SharedBuffer::new();
    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::Vp9, 100, 100, 30.0)
        .build()
        .unwrap();

    // Try to write a P-frame first - should fail
    let pframe = build_vp9_pframe();
    let result = muxer.write_video(0.0, &pframe, false);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("first video frame must be a keyframe"));
}

#[test]
fn vp9_keyframe_must_have_valid_config() {
    let (writer, _buffer) = SharedBuffer::new();
    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::Vp9, 100, 100, 30.0)
        .build()
        .unwrap();

    // Try to write an invalid VP9 frame (wrong marker)
    let invalid_frame = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let result = muxer.write_video(0.0, &invalid_frame, true);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("first VP9 frame must contain sequence parameters"));
}

#[test]
fn vp9_muxer_produces_vp09_sample_entry() {
    let (writer, buffer) = SharedBuffer::new();
    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::Vp9, 100, 100, 30.0)
        .build()
        .unwrap();

    // Write keyframe
    let keyframe = build_vp9_keyframe();
    muxer.write_video(0.0, &keyframe, true).unwrap();

    // Write P-frame
    let pframe = build_vp9_pframe();
    muxer.write_video(1.0 / 30.0, &pframe, false).unwrap();

    // Finalize
    muxer.finish().unwrap();

    // Check output contains vp09 sample entry
    let output = buffer.lock().unwrap();
    assert!(contains_box(&output, b"vp09"));
}

#[test]
fn vp9_muxer_produces_vpcc_config_box() {
    let (writer, buffer) = SharedBuffer::new();
    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::Vp9, 100, 100, 30.0)
        .build()
        .unwrap();

    // Write keyframe
    let keyframe = build_vp9_keyframe();
    muxer.write_video(0.0, &keyframe, true).unwrap();

    // Finalize
    muxer.finish().unwrap();

    // Check output contains vpcC configuration box
    let output = buffer.lock().unwrap();
    assert!(contains_box(&output, b"vpcC"));
}

#[test]
fn vp9_config_extraction_works() {
    use muxide::codec::vp9::extract_vp9_config;

    let keyframe = build_vp9_keyframe();
    let config = extract_vp9_config(&keyframe).unwrap();

    assert_eq!(config.width, 100);
    assert_eq!(config.height, 100);
    assert_eq!(config.profile, 0);
    assert_eq!(config.bit_depth, 8);
    assert_eq!(config.color_space, 2); // BT.709
}

#[test]
fn vp9_keyframe_detection_works() {
    use muxide::codec::vp9::is_vp9_keyframe;

    let keyframe = build_vp9_keyframe();
    assert!(is_vp9_keyframe(&keyframe).unwrap());

    let pframe = build_vp9_pframe();
    assert!(!is_vp9_keyframe(&pframe).unwrap());
}
