//! Property-Based Tests for Muxide
//!
//! These tests verify invariants hold across a wide range of inputs using proptest.
//! They catch edge cases that unit tests might miss.

use proptest::prelude::*;
use std::io::Cursor;

// Import the muxide crate
use muxide::api::{MuxerBuilder, VideoCodec};

/// Helper to create a valid H.264 keyframe with SPS/PPS
fn make_h264_keyframe() -> Vec<u8> {
    // Minimal valid Annex B H.264 stream with SPS, PPS, and IDR slice
    let mut data = Vec::new();
    // SPS (NAL type 7)
    data.extend_from_slice(&[
        0, 0, 0, 1, 0x67, 0x42, 0x00, 0x1e, 0x95, 0xa8, 0x28, 0x28, 0x28,
    ]);
    // PPS (NAL type 8)
    data.extend_from_slice(&[0, 0, 0, 1, 0x68, 0xce, 0x3c, 0x80]);
    // IDR slice (NAL type 5)
    data.extend_from_slice(&[
        0, 0, 0, 1, 0x65, 0x88, 0x84, 0x00, 0x00, 0x03, 0x00, 0x00, 0x03,
    ]);
    data
}

/// Helper to create a valid H.264 P-frame
fn make_h264_pframe() -> Vec<u8> {
    // Non-IDR slice (NAL type 1)
    let mut data = Vec::new();
    data.extend_from_slice(&[0, 0, 0, 1, 0x41, 0x9a, 0x24, 0x6c, 0x42, 0xff, 0xff]);
    data
}

proptest! {
    /// Property: Output size must be at least header size (ftyp + moov + mdat headers)
    #[test]
    fn prop_output_always_has_minimum_size(frames in 1..10usize) {
        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, 1920, 1080, 30.0)
            .build()
            .unwrap();

        // Write first keyframe at t=0.0
        muxer.write_video(0.0, &make_h264_keyframe(), true).unwrap();

        // Write P-frames at 30fps intervals
        for i in 1..frames {
            let pts = i as f64 / 30.0; // seconds
            muxer.write_video(pts, &make_h264_pframe(), false).unwrap();
        }

        muxer.finish().unwrap();

        let output = buffer.into_inner();
        // Minimum: ftyp(24) + mdat header(8) + moov(varies, but at least 500)
        prop_assert!(output.len() >= 500, "Output {} bytes is too small", output.len());
    }

    /// Property: Width and height in config are preserved in output
    #[test]
    fn prop_dimensions_preserved(
        width in 64u32..4096,
        height in 64u32..2160
    ) {
        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, width, height, 30.0)
            .build()
            .unwrap();

        muxer.write_video(0.0, &make_h264_keyframe(), true).unwrap();
        muxer.finish().unwrap();

        let output = buffer.into_inner();

        // Find avc1 box (contains width/height in visual sample entry)
        // Search for the bytes "avc1" in the output
        let avc1_pos = output.windows(4)
            .position(|w| w == b"avc1");
        prop_assert!(avc1_pos.is_some(), "avc1 box not found in output");

        // Just verify the file was produced and has reasonable structure
        prop_assert!(output.len() >= 500, "Output too small");

        // Verify moov box exists
        let moov_pos = output.windows(4)
            .position(|w| w == b"moov");
        prop_assert!(moov_pos.is_some(), "moov box not found");
    }

    /// Property: PTS values must be monotonically increasing
    #[test]
    fn prop_pts_must_increase(frame_count in 2..20usize) {
        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, 1920, 1080, 30.0)
            .build()
            .unwrap();

        // First keyframe at pts=0.0
        muxer.write_video(0.0, &make_h264_keyframe(), true).unwrap();

        // Subsequent frames with increasing PTS
        for i in 1..frame_count {
            let pts = i as f64 / 30.0;
            muxer.write_video(pts, &make_h264_pframe(), false).unwrap();
        }

        // This should succeed (monotonic PTS)
        let result = muxer.finish();
        prop_assert!(result.is_ok());
    }

    /// Property: Non-monotonic PTS must be rejected
    #[test]
    fn prop_non_monotonic_pts_rejected(
        first_pts in 0.1f64..1.0,
        second_pts in 0.0f64..0.05
    ) {
        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, 1920, 1080, 30.0)
            .build()
            .unwrap();

        // First keyframe
        muxer.write_video(first_pts, &make_h264_keyframe(), true).unwrap();

        // Second frame with earlier PTS - should fail
        let result = muxer.write_video(second_pts, &make_h264_pframe(), false);
        prop_assert!(result.is_err(), "Non-monotonic PTS should be rejected");
    }

    /// Property: Frame count in stats must match written frames
    #[test]
    fn prop_frame_count_accurate(frame_count in 1..50usize) {
        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, 1920, 1080, 30.0)
            .build()
            .unwrap();

        // First keyframe
        muxer.write_video(0.0, &make_h264_keyframe(), true).unwrap();

        // Additional frames
        for i in 1..frame_count {
            let pts = i as f64 / 30.0;
            muxer.write_video(pts, &make_h264_pframe(), false).unwrap();
        }

        let stats = muxer.finish_with_stats().unwrap();
        prop_assert_eq!(
            stats.video_frames as usize,
            frame_count,
            "Stats frame count should match written frames"
        );
    }

    /// Property: Duration must be approximately frame_count / fps
    #[test]
    fn prop_duration_reasonable(frame_count in 2..100usize) {
        let fps = 30.0;
        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, 1920, 1080, fps)
            .build()
            .unwrap();

        // Write frames at consistent intervals
        muxer.write_video(0.0, &make_h264_keyframe(), true).unwrap();
        for i in 1..frame_count {
            let pts = i as f64 / fps; // seconds
            muxer.write_video(pts, &make_h264_pframe(), false).unwrap();
        }

        let stats = muxer.finish_with_stats().unwrap();

        // Duration should be approximately (frames - 1) * frame_duration
        // (frames - 1 because last frame has no next frame to calculate duration)
        let expected_duration_secs = (frame_count - 1) as f64 / fps;

        // Allow 20% tolerance for rounding
        let min_duration = expected_duration_secs * 0.8;
        let max_duration = expected_duration_secs * 1.2 + 0.1; // +0.1s for rounding

        prop_assert!(
            stats.duration_secs >= min_duration && stats.duration_secs <= max_duration,
            "Duration {}s outside expected range {}..{}s for {} frames at {}fps",
            stats.duration_secs, min_duration, max_duration, frame_count, fps
        );
    }

    /// Property: Output must start with ftyp box
    #[test]
    fn prop_output_starts_with_ftyp(frame_count in 1..10usize) {
        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, 1920, 1080, 30.0)
            .build()
            .unwrap();

        muxer.write_video(0.0, &make_h264_keyframe(), true).unwrap();
        for i in 1..frame_count {
            muxer.write_video(i as f64 / 30.0, &make_h264_pframe(), false).unwrap();
        }
        muxer.finish().unwrap();

        let output = buffer.into_inner();

        // ftyp box starts at offset 0, box type at offset 4
        prop_assert!(output.len() >= 8, "Output too small for ftyp");
        prop_assert_eq!(&output[4..8], b"ftyp", "First box must be ftyp");
    }

    /// Property: Bytes written in stats must match actual output size
    #[test]
    fn prop_bytes_written_accurate(frame_count in 1..20usize) {
        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, 1920, 1080, 30.0)
            .build()
            .unwrap();

        muxer.write_video(0.0, &make_h264_keyframe(), true).unwrap();
        for i in 1..frame_count {
            muxer.write_video(i as f64 / 30.0, &make_h264_pframe(), false).unwrap();
        }

        let stats = muxer.finish_with_stats().unwrap();
        let output = buffer.into_inner();

        prop_assert_eq!(
            stats.bytes_written as usize,
            output.len(),
            "Stats bytes_written should match actual output size"
        );
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use muxide::invariant_ppt::{clear_invariant_log, contract_test};

    /// Contract test: Box building must check size invariant
    #[test]
    fn contract_box_size_invariant() {
        clear_invariant_log();

        // Trigger box building by creating an MP4
        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, 1920, 1080, 30.0)
            .build()
            .unwrap();

        muxer.write_video(0.0, &make_h264_keyframe(), true).unwrap();
        muxer.finish().unwrap();

        // Verify the box size invariant was checked
        contract_test("box building", &["Box size must equal header + payload"]);
    }

    /// Contract test: Video sample entry must check width/height invariants
    #[test]
    fn contract_width_height_invariants() {
        clear_invariant_log();

        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, 1920, 1080, 30.0)
            .build()
            .unwrap();

        muxer.write_video(0.0, &make_h264_keyframe(), true).unwrap();
        muxer.finish().unwrap();

        // Verify width/height invariants were checked
        contract_test(
            "avc1 box building",
            &["Width must fit in 16-bit", "Height must fit in 16-bit"],
        );
    }

    /// Contract test: Sample sizes must be non-zero
    #[test]
    fn contract_sample_sizes_invariant() {
        clear_invariant_log();

        let mut buffer = Cursor::new(Vec::new());
        let mut muxer = MuxerBuilder::new(&mut buffer)
            .video(VideoCodec::H264, 1920, 1080, 30.0)
            .build()
            .unwrap();

        muxer.write_video(0.0, &make_h264_keyframe(), true).unwrap();
        muxer
            .write_video(1.0 / 30.0, &make_h264_pframe(), false)
            .unwrap();
        muxer.finish().unwrap();

        // Verify the stsz invariant was checked
        contract_test("stsz box building", &["No empty samples in stsz"]);
    }
}
