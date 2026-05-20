//! Integration tests for HEVC (H.265) video muxing.

mod support;

use muxide::api::{MuxerBuilder, VideoCodec};
use support::SharedBuffer;

/// Helper to build a minimal HEVC keyframe with VPS, SPS, PPS, and IDR slice.
fn build_hevc_keyframe() -> Vec<u8> {
    let mut data = Vec::new();

    // VPS NAL (type 32) - 0x40 = (32 << 1)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // Start code
    data.extend_from_slice(&[
        0x40, 0x01, 0x0c, 0x01, 0xff, 0xff, 0x01, 0x60, 0x00, 0x00, 0x03, 0x00, 0x90, 0x00, 0x00,
        0x03, 0x00, 0x00, 0x03, 0x00, 0x5d, 0x95, 0x98, 0x09,
    ]);

    // SPS NAL (type 33) - 0x42 = (33 << 1)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // Start code
    data.extend_from_slice(&[
        0x42, 0x01, 0x01, 0x01, 0x60, 0x00, 0x00, 0x03, 0x00, 0x90, 0x00, 0x00, 0x03, 0x00, 0x00,
        0x03, 0x00, 0x5d, 0xa0, 0x02, 0x80, 0x80, 0x2d, 0x16, 0x59, 0x59, 0xa4, 0x93, 0x24, 0xb8,
    ]);

    // PPS NAL (type 34) - 0x44 = (34 << 1)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // Start code
    data.extend_from_slice(&[0x44, 0x01, 0xc0, 0x73, 0xc0, 0x4c, 0x90]);

    // IDR slice NAL (type 19 = IDR_W_RADL) - 0x26 = (19 << 1)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // Start code
    data.extend_from_slice(&[
        0x26, 0x01, 0xaf, 0x06, 0xb8, 0x63, 0xef, 0x3e, 0xb6, 0xb4, 0x8e, 0x19,
    ]);

    data
}

/// Helper to build a minimal HEVC P-frame (non-keyframe).
fn build_hevc_p_frame() -> Vec<u8> {
    let mut data = Vec::new();

    // TRAIL_R slice NAL (type 1) - 0x02 = (1 << 1)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // Start code
    data.extend_from_slice(&[0x02, 0x01, 0xd0, 0x10, 0xf3, 0x95, 0x27, 0x41, 0xfe, 0xfc]);

    data
}

/// Recursively search for a 4CC in an MP4 container by pattern matching
fn contains_box(data: &[u8], fourcc: &[u8; 4]) -> bool {
    // Simple pattern search - look for the fourcc anywhere in the data
    data.windows(4).any(|window| window == fourcc)
}

#[test]
fn hevc_muxer_produces_hvc1_sample_entry() {
    let (writer, buffer) = SharedBuffer::new();

    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::H265, 1920, 1080, 30.0)
        .build()
        .expect("build should succeed");

    // Write a keyframe
    let keyframe = build_hevc_keyframe();
    muxer
        .write_video(0.0, &keyframe, true)
        .expect("write_video should succeed");

    // Write a P-frame
    let p_frame = build_hevc_p_frame();
    muxer
        .write_video(1.0 / 30.0, &p_frame, false)
        .expect("write_video should succeed");

    muxer.finish().expect("finish should succeed");

    let produced = buffer.lock().unwrap();

    // Find the hvc1 box
    assert!(
        contains_box(&produced, b"hvc1"),
        "Output should contain hvc1 sample entry"
    );

    // Find the hvcC box
    assert!(
        contains_box(&produced, b"hvcC"),
        "Output should contain hvcC configuration box"
    );
}

#[test]
fn hevc_first_frame_must_be_keyframe() {
    let (writer, _buffer) = SharedBuffer::new();

    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::H265, 1920, 1080, 30.0)
        .build()
        .expect("build should succeed");

    // Try to write a P-frame first (should fail)
    let p_frame = build_hevc_p_frame();
    let result = muxer.write_video(0.0, &p_frame, false);

    assert!(result.is_err(), "First frame must be a keyframe");
}

#[test]
fn hevc_keyframe_must_have_vps_sps_pps() {
    let (writer, _buffer) = SharedBuffer::new();

    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::H265, 1920, 1080, 30.0)
        .build()
        .expect("build should succeed");

    // Try to write a keyframe with only IDR slice (no VPS/SPS/PPS)
    let bad_keyframe = vec![
        0x00, 0x00, 0x00, 0x01, 0x26, 0x01, 0xaf, 0x06, // IDR only
    ];
    let result = muxer.write_video(0.0, &bad_keyframe, true);

    assert!(result.is_err(), "Keyframe must contain VPS, SPS, and PPS");
}

/// Regression test: BLA IRAP NAL types (16-18) must be treated as keyframes by encode_video.
/// Before the fix, api::is_keyframe only checked NAL types 19-21, missing BLA frames.
#[test]
fn hevc_encode_video_detects_bla_w_lp_as_keyframe() {
    let (writer, buffer) = SharedBuffer::new();

    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::H265, 640, 480, 30.0)
        .build()
        .expect("build should succeed");

    // Build a minimal BLA_W_LP frame: VPS + SPS + PPS + BLA_W_LP slice.
    // BLA_W_LP = NAL type 16, first header byte = (16 << 1) = 0x20.
    let mut bla_keyframe = build_hevc_keyframe();
    // Replace the IDR NAL header byte (0x26 = type 19) with BLA_W_LP (0x20 = type 16).
    let idx = bla_keyframe
        .windows(2)
        .position(|w| w == [0x26, 0x01])
        .expect("IDR slice must be present in fixture");
    bla_keyframe[idx] = 0x20; // Change to BLA_W_LP

    // encode_video auto-detects keyframe from bitstream; this must NOT return FirstVideoFrameMustBeKeyframe.
    let result = muxer.encode_video(&bla_keyframe, 33);
    assert!(
        result.is_ok(),
        "BLA_W_LP frame must be auto-detected as a keyframe by encode_video: {:?}",
        result.err()
    );
    muxer.finish().expect("finish should succeed");
    assert!(!buffer.lock().unwrap().is_empty());
}

/// Regression test: H264 consecutive Annex B start codes produce empty NAL slices.
/// api::is_keyframe must not panic on these.
#[test]
fn h264_encode_video_no_panic_on_consecutive_start_codes() {
    let (writer, buffer) = SharedBuffer::new();

    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::H264, 640, 480, 30.0)
        .build()
        .expect("build should succeed");

    // Build a valid H264 keyframe with a stray consecutive start code at the start
    // (empty NAL between the two leading start codes).
    let mut frame = Vec::new();
    frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // extra start code → empty NAL
    // SPS (NAL type 7)
    frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1e, 0x95, 0xa8]);
    // PPS (NAL type 8)
    frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x3c, 0x80]);
    // IDR slice (NAL type 5)
    frame.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00, 0x00, 0x03, 0x00, 0x00, 0x03,
    ]);

    // Must not panic on empty NAL, and must correctly detect IDR as keyframe.
    let result = muxer.encode_video(&frame, 33);
    assert!(
        result.is_ok(),
        "H264 consecutive start codes must not panic: {:?}",
        result.err()
    );
    muxer.finish().expect("finish should succeed");
    assert!(!buffer.lock().unwrap().is_empty());
}

/// Regression test: H265 consecutive Annex B start codes produce empty NAL slices.
/// api::is_keyframe must not panic on these.
#[test]
fn hevc_encode_video_no_panic_on_consecutive_start_codes() {
    let (writer, buffer) = SharedBuffer::new();

    let mut muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::H265, 640, 480, 30.0)
        .build()
        .expect("build should succeed");

    // Prepend an extra start code to the valid HEVC keyframe fixture.
    let mut frame = vec![0x00, 0x00, 0x00, 0x01]; // extra start code → empty NAL
    frame.extend_from_slice(&build_hevc_keyframe());

    // Must not panic on empty NAL slice, and IDR must still be detected as keyframe.
    let result = muxer.encode_video(&frame, 33);
    assert!(
        result.is_ok(),
        "HEVC consecutive start codes must not panic: {:?}",
        result.err()
    );
    muxer.finish().expect("finish should succeed");
    assert!(!buffer.lock().unwrap().is_empty());
}
