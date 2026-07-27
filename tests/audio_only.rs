//! Integration tests for audio-only MP4 and audio-only fragmented MP4.
//!
//! Covers beads issues muxide-9ue.4.1 / 9ue.4.2 / 9ue.4.3:
//! - `MuxerBuilder::build()` with only an audio track succeeds
//! - Audio-only output contains a valid `ftyp` + `moov` + `mdat` structure
//!   with an audio (`soun`) trak and no video (`vide`) trak
//! - Audio-only fragmented MP4 init segments contain an audio `trak` + `mvex`
//! - Video+audio muxing still works (regression)

use std::io::{Read, Write};

use muxide::api::{AacProfile, AudioCodec, MuxerBuilder, MuxerError, VideoCodec};

/// A minimal valid AAC-LC ADTS frame: 48 kHz, stereo, 9 bytes total
/// (7-byte header + 2 payload bytes).
fn adts_frame_48k_stereo() -> Vec<u8> {
    vec![0xff, 0xf1, 0x4c, 0x80, 0x01, 0x3f, 0xfc, 0xaa, 0xbb]
}

/// Minimal H.264 Annex B keyframe with SPS + PPS + IDR slice.
fn h264_keyframe() -> Vec<u8> {
    let mut data = Vec::new();
    // SPS (NAL type 7)
    data.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1e, 0xab, 0x40, 0xf0, 0x28, 0xd0,
    ]);
    // PPS (NAL type 8)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x38, 0x80]);
    // IDR slice (NAL type 5)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00, 0x10]);
    data
}

/// Parse the top-level box layout of an MP4 byte stream, validating that the
/// declared box sizes exactly tile the file. Returns `(fourcc, offset, size)`
/// per box.
fn parse_top_level_boxes(data: &[u8]) -> Vec<(String, usize, usize)> {
    let mut boxes = Vec::new();
    let mut pos = 0usize;
    while pos < data.len() {
        assert!(
            pos + 8 <= data.len(),
            "truncated box header at offset {}",
            pos
        );
        let size = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let typ = String::from_utf8_lossy(&data[pos + 4..pos + 8]).to_string();
        assert!(size >= 8, "box '{}' has invalid size {}", typ, size);
        assert!(
            pos + size <= data.len(),
            "box '{}' (size {}) overruns file of {} bytes",
            typ,
            size,
            data.len()
        );
        boxes.push((typ, pos, size));
        pos += size;
    }
    assert_eq!(pos, data.len(), "boxes must exactly tile the file");
    boxes
}

/// Return the byte range of the first top-level box with the given type.
fn find_top_level_box<'a>(data: &'a [u8], fourcc: &str) -> Option<&'a [u8]> {
    parse_top_level_boxes(data)
        .into_iter()
        .find(|(typ, _, _)| typ == fourcc)
        .map(|(_, off, size)| &data[off..off + size])
}

/// Search for a raw 4CC pattern anywhere inside a byte slice.
fn contains_fourcc(data: &[u8], fourcc: &[u8; 4]) -> bool {
    data.windows(4).any(|w| w == fourcc)
}

// ---------------------------------------------------------------------------
// 9ue.4.1 — builder behaviour
// ---------------------------------------------------------------------------

#[test]
fn build_with_neither_video_nor_audio_still_fails() {
    let result = MuxerBuilder::new(Vec::<u8>::new()).build();
    assert!(
        matches!(result, Err(MuxerError::MissingVideoConfig)),
        "empty builder must return MissingVideoConfig"
    );
}

#[test]
fn audio_only_build_succeeds_and_write_video_is_rejected() {
    let mut muxer = MuxerBuilder::new(Vec::<u8>::new())
        .audio(AudioCodec::Aac(AacProfile::Lc), 48000, 2)
        .build()
        .expect("audio-only build must succeed");

    let keyframe = h264_keyframe();
    let result = muxer.write_video(0.0, &keyframe, true);
    assert!(
        matches!(result, Err(MuxerError::VideoNotConfigured)),
        "write_video on an audio-only muxer must return VideoNotConfigured, got {:?}",
        result
    );

    let result = muxer.write_video_with_dts(0.0, 0.0, &keyframe, true);
    assert!(
        matches!(result, Err(MuxerError::VideoNotConfigured)),
        "write_video_with_dts on an audio-only muxer must return VideoNotConfigured"
    );

    let result = muxer.encode_video(&keyframe, 33);
    assert!(
        matches!(result, Err(MuxerError::VideoNotConfigured)),
        "encode_video on an audio-only muxer must return VideoNotConfigured"
    );
}

// ---------------------------------------------------------------------------
// 9ue.4.3 (a) — non-fragmented audio-only MP4, AAC-LC 48kHz stereo, ~100 frames
// ---------------------------------------------------------------------------

#[test]
fn audio_only_aac_mp4_written_to_tempfile_has_valid_box_structure() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tempfile");

    {
        let file = tmp.as_file_mut().try_clone().expect("clone file handle");
        let mut muxer = MuxerBuilder::new(file)
            .audio(AudioCodec::Aac(AacProfile::Lc), 48000, 2)
            .build()
            .expect("audio-only build must succeed");

        let frame = adts_frame_48k_stereo();
        // 100 AAC frames of 1024 samples each at 48 kHz.
        for i in 0..100u64 {
            let pts = i as f64 * 1024.0 / 48000.0;
            muxer
                .write_audio(pts, &frame)
                .unwrap_or_else(|e| panic!("write_audio frame {} failed: {}", i, e));
        }

        let stats = muxer.finish_with_stats().expect("finish must succeed");
        assert_eq!(stats.video_frames, 0);
        assert_eq!(stats.audio_frames, 100);
        assert!(stats.bytes_written > 0);
    }

    let mut bytes = Vec::new();
    tmp.as_file_mut().flush().expect("flush");
    {
        use std::io::Seek;
        tmp.as_file_mut()
            .seek(std::io::SeekFrom::Start(0))
            .expect("seek");
    }
    tmp.as_file_mut().read_to_end(&mut bytes).expect("read");
    assert!(!bytes.is_empty(), "output file must not be empty");

    // Validate top-level box structure by parsing box headers in Rust.
    let boxes = parse_top_level_boxes(&bytes);
    let types: Vec<&str> = boxes.iter().map(|(t, _, _)| t.as_str()).collect();
    assert!(types.contains(&"ftyp"), "missing ftyp box: {:?}", types);
    assert!(types.contains(&"moov"), "missing moov box: {:?}", types);
    assert!(types.contains(&"mdat"), "missing mdat box: {:?}", types);
    assert_eq!(types[0], "ftyp", "ftyp must be the first box");

    // The moov must contain exactly an audio track: soun handler + mp4a
    // sample entry, and no video track.
    let moov = find_top_level_box(&bytes, "moov").expect("moov box");
    assert!(contains_fourcc(moov, b"trak"), "moov must contain a trak");
    assert!(
        contains_fourcc(moov, b"soun"),
        "moov must contain a sound handler"
    );
    assert!(
        contains_fourcc(moov, b"mp4a"),
        "moov must contain an mp4a sample entry"
    );
    assert!(
        contains_fourcc(moov, b"esds"),
        "mp4a entry must contain an esds box"
    );
    assert!(
        !contains_fourcc(moov, b"vide"),
        "audio-only moov must not contain a video handler"
    );
    assert!(
        !contains_fourcc(moov, b"avc1"),
        "audio-only moov must not contain a video sample entry"
    );
}

#[test]
fn audio_only_mp4_without_fast_start_places_moov_after_mdat() {
    let mut muxer = MuxerBuilder::new(Vec::<u8>::new())
        .audio(AudioCodec::Aac(AacProfile::Lc), 48000, 2)
        .with_fast_start(false)
        .build()
        .expect("audio-only build must succeed");

    let frame = adts_frame_48k_stereo();
    for i in 0..10u64 {
        let pts = i as f64 * 1024.0 / 48000.0;
        muxer.write_audio(pts, &frame).expect("write_audio");
    }
    muxer.finish_in_place().expect("finish");
    let bytes = muxer.into_writer();

    let boxes = parse_top_level_boxes(&bytes);
    let types: Vec<&str> = boxes.iter().map(|(t, _, _)| t.as_str()).collect();
    assert_eq!(types, vec!["ftyp", "mdat", "moov"]);
}

#[test]
fn audio_only_opus_mp4_has_opus_sample_entry() {
    let mut muxer = MuxerBuilder::new(Vec::<u8>::new())
        .audio(AudioCodec::Opus, 48000, 2)
        .build()
        .expect("audio-only Opus build must succeed");

    // TOC: config=4 (SILK 20ms), s=1 (stereo), c=0 (1 frame)
    let packet = vec![0x24, 0xc0, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
    for i in 0..20u64 {
        let pts = i as f64 * 0.02;
        muxer.write_audio(pts, &packet).expect("write_audio");
    }
    muxer.finish_in_place().expect("finish");
    let bytes = muxer.into_writer();

    let moov = find_top_level_box(&bytes, "moov").expect("moov box");
    assert!(contains_fourcc(moov, b"soun"));
    assert!(contains_fourcc(moov, b"Opus"), "missing Opus sample entry");
    assert!(contains_fourcc(moov, b"dOps"), "missing dOps box");
    assert!(!contains_fourcc(moov, b"vide"));
}

// ---------------------------------------------------------------------------
// 9ue.4.2 / 9ue.4.3 (b) — audio-only fragmented MP4 init segment
// ---------------------------------------------------------------------------

#[test]
fn audio_only_fmp4_init_segment_contains_audio_trak_and_mvex() {
    let mut muxer = MuxerBuilder::new(Vec::<u8>::new())
        .audio(AudioCodec::Aac(AacProfile::Lc), 48000, 2)
        .new_with_fragment()
        .expect("audio-only fragmented muxer must build");

    let init = muxer.init_segment();
    assert!(!init.is_empty());

    // Top-level structure must be exactly ftyp + moov.
    let boxes = parse_top_level_boxes(&init);
    let types: Vec<&str> = boxes.iter().map(|(t, _, _)| t.as_str()).collect();
    assert_eq!(types, vec!["ftyp", "moov"]);

    let moov = find_top_level_box(&init, "moov").expect("moov box");
    assert!(contains_fourcc(moov, b"mvhd"), "moov must contain mvhd");
    assert!(
        contains_fourcc(moov, b"trak"),
        "init segment moov must contain an audio trak"
    );
    assert!(
        contains_fourcc(moov, b"soun"),
        "audio trak must use the soun handler"
    );
    assert!(
        contains_fourcc(moov, b"mp4a"),
        "stsd must contain an mp4a sample entry for AAC"
    );
    assert!(
        contains_fourcc(moov, b"esds"),
        "mp4a entry must contain esds"
    );
    assert!(contains_fourcc(moov, b"mvex"), "moov must contain mvex");
    assert!(contains_fourcc(moov, b"trex"), "mvex must contain trex");
    assert!(
        !contains_fourcc(moov, b"vide"),
        "audio-only init segment must not contain a video handler"
    );
    assert!(
        !contains_fourcc(moov, b"avc1"),
        "audio-only init segment must not contain a video sample entry"
    );

    // trex must reference track ID 1 (the single audio track).
    let trex_pos = moov
        .windows(4)
        .position(|w| w == b"trex")
        .expect("trex fourcc");
    // trex payload: version+flags (4 bytes) then track ID (4 bytes)
    let track_id_off = trex_pos + 4 + 4;
    let track_id = u32::from_be_bytes(moov[track_id_off..track_id_off + 4].try_into().unwrap());
    assert_eq!(track_id, 1, "trex must reference the audio track (ID 1)");

    // Media segments still work: queue raw AAC samples and flush a segment.
    let raw_aac = vec![0xaa, 0xbb, 0xcc, 0xdd];
    muxer.write_audio(0, 0, &raw_aac).expect("write_audio");
    muxer
        .write_audio(1024, 1024, &raw_aac)
        .expect("write_audio");
    let segment = muxer.flush_segment().expect("segment");
    let seg_boxes = parse_top_level_boxes(&segment);
    let seg_types: Vec<&str> = seg_boxes.iter().map(|(t, _, _)| t.as_str()).collect();
    assert_eq!(seg_types, vec!["moof", "mdat"]);
}

#[test]
fn audio_only_fmp4_opus_init_segment_uses_opus_sample_entry() {
    let mut muxer = MuxerBuilder::new(Vec::<u8>::new())
        .audio(AudioCodec::Opus, 48000, 2)
        .new_with_fragment()
        .expect("audio-only Opus fragmented muxer must build");

    let init = muxer.init_segment();
    let moov = find_top_level_box(&init, "moov").expect("moov box");
    assert!(contains_fourcc(moov, b"soun"));
    assert!(contains_fourcc(moov, b"Opus"), "missing Opus sample entry");
    assert!(contains_fourcc(moov, b"dOps"), "missing dOps box");
    assert!(!contains_fourcc(moov, b"vide"));
}

#[test]
fn fragmented_builder_with_neither_track_fails() {
    let result = MuxerBuilder::new(Vec::<u8>::new()).new_with_fragment();
    assert!(matches!(result, Err(MuxerError::MissingVideoConfig)));
}

// ---------------------------------------------------------------------------
// 9ue.4.3 (c) — regression: video + audio still works
// ---------------------------------------------------------------------------

#[test]
fn video_plus_audio_still_works_after_audio_only_changes() {
    let mut muxer = MuxerBuilder::new(Vec::<u8>::new())
        .video(VideoCodec::H264, 640, 480, 30.0)
        .audio(AudioCodec::Aac(AacProfile::Lc), 48000, 2)
        .build()
        .expect("video+audio build must succeed");

    let keyframe = h264_keyframe();
    muxer
        .write_video(0.0, &keyframe, true)
        .expect("write_video");
    muxer
        .write_video(1.0 / 30.0, &keyframe, false)
        .expect("write_video");

    let audio = adts_frame_48k_stereo();
    muxer.write_audio(0.0, &audio).expect("write_audio");
    muxer
        .write_audio(1024.0 / 48000.0, &audio)
        .expect("write_audio");

    let stats = muxer.finish_in_place_with_stats().expect("finish");
    assert_eq!(stats.video_frames, 2);
    assert_eq!(stats.audio_frames, 2);

    let bytes = muxer.into_writer();
    let boxes = parse_top_level_boxes(&bytes);
    let types: Vec<&str> = boxes.iter().map(|(t, _, _)| t.as_str()).collect();
    assert!(types.contains(&"ftyp"));
    assert!(types.contains(&"moov"));
    assert!(types.contains(&"mdat"));

    let moov = find_top_level_box(&bytes, "moov").expect("moov box");
    assert!(
        contains_fourcc(moov, b"vide"),
        "moov must contain video handler"
    );
    assert!(
        contains_fourcc(moov, b"avc1"),
        "moov must contain avc1 entry"
    );
    assert!(
        contains_fourcc(moov, b"soun"),
        "moov must contain sound handler"
    );
    assert!(
        contains_fourcc(moov, b"mp4a"),
        "moov must contain mp4a entry"
    );

    // Video-still-required-first ordering for audio must still be enforced
    // when a video track is configured.
    let mut muxer = MuxerBuilder::new(Vec::<u8>::new())
        .video(VideoCodec::H264, 640, 480, 30.0)
        .audio(AudioCodec::Aac(AacProfile::Lc), 48000, 2)
        .build()
        .expect("build");
    let audio = adts_frame_48k_stereo();
    assert!(
        matches!(
            muxer.write_audio(0.0, &audio),
            Err(MuxerError::AudioBeforeFirstVideo { .. })
        ),
        "audio before first video must still fail when video is configured"
    );
}
