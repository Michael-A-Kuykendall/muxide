mod support;

use muxide::api::{MuxerBuilder, VideoCodec};
use support::{parse_boxes, SharedBuffer};

#[test]
fn video_track_structure_contains_expected_boxes() -> Result<(), Box<dyn std::error::Error>> {
    let (writer, buffer) = SharedBuffer::new();

    let muxer = MuxerBuilder::new(writer)
        .video(VideoCodec::H264, 1920, 1080, 30.0)
        .build()?;
    muxer.finish()?;

    let produced = buffer.lock().unwrap();
    let top_boxes = parse_boxes(&produced);
    let moov = top_boxes
        .iter()
        .find(|b| b.typ == *b"moov")
        .expect("moov box must exist");

    let moov_payload = moov.payload(&produced);
    let moov_children = parse_boxes(moov_payload);
    assert!(
        moov_children.iter().any(|b| b.typ == *b"trak"),
        "trak missing"
    );
    let trak = moov_children.iter().find(|b| b.typ == *b"trak").unwrap();

    let trak_payload = trak.payload(moov_payload);
    let mdia_boxes = parse_boxes(trak_payload);
    let mdia = mdia_boxes.iter().find(|b| b.typ == *b"mdia").unwrap();

    let mdia_payload = mdia.payload(trak_payload);
    let minf_boxes = parse_boxes(mdia_payload);
    let minf = minf_boxes.iter().find(|b| b.typ == *b"minf").unwrap();

    let minf_payload = minf.payload(mdia_payload);
    let stbl_boxes = parse_boxes(minf_payload);
    let stbl = stbl_boxes.iter().find(|b| b.typ == *b"stbl").unwrap();

    let stbl_payload = stbl.payload(minf_payload);
    let stsd_boxes = parse_boxes(stbl_payload);
    let stsd = stsd_boxes.iter().find(|b| b.typ == *b"stsd").unwrap();

    let stsd_payload = stsd.payload(stbl_payload);
    let entries_payload = &stsd_payload[8..];
    let avc1_boxes = parse_boxes(entries_payload);
    let avc1 = avc1_boxes.iter().find(|b| b.typ == *b"avc1").unwrap();

    let avc1_payload = avc1.payload(entries_payload);
    let avc_c_index = avc1_payload
        .windows(4)
        .position(|window| window == b"avcC")
        .expect("avcC box must exist in avc1");
    let size_start = avc_c_index - 4;
    let avc_c_size =
        u32::from_be_bytes(avc1_payload[size_start..size_start + 4].try_into().unwrap()) as usize;
    let avc_c_payload = &avc1_payload[size_start + 8..size_start + avc_c_size];
    assert!(
        avc_c_payload.windows(1).any(|w| w[0] == 0x67),
        "SPS missing in avcC"
    );
    assert!(
        avc_c_payload.windows(1).any(|w| w[0] == 0x68),
        "PPS missing in avcC"
    );

    Ok(())
}

fn box_duration(payload: &[u8], version0_offset: usize, version1_offset: usize) -> u64 {
    if payload[0] == 1 {
        u64::from_be_bytes(
            payload[version1_offset..version1_offset + 8]
                .try_into()
                .unwrap(),
        )
    } else {
        u32::from_be_bytes(
            payload[version0_offset..version0_offset + 4]
                .try_into()
                .unwrap(),
        ) as u64
    }
}

fn h264_keyframe() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1e, 0xab, 0x40, 0xf0, 0x28, 0xd0,
    ]);
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x38, 0x80]);
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00, 0x10]);
    data
}

fn adts_frame() -> Vec<u8> {
    vec![0xff, 0xf1, 0x4c, 0x80, 0x01, 0x3f, 0xfc, 0xaa, 0xbb]
}

#[test]
fn tkhd_duration_matches_media() {
    use muxide::api::{AacProfile, AudioCodec};

    let mut muxer = MuxerBuilder::new(Vec::<u8>::new())
        .video(VideoCodec::H264, 320, 240, 30.0)
        .audio(AudioCodec::Aac(AacProfile::Lc), 48_000, 2)
        .build()
        .unwrap();
    let frame = h264_keyframe();
    let audio = adts_frame();
    muxer.write_video(0.0, &frame, true).unwrap();
    muxer.write_video(0.5, &frame, false).unwrap();
    muxer.write_audio(0.0, &audio).unwrap();
    muxer.write_audio(0.5, &audio).unwrap();
    muxer.finish_in_place().unwrap();
    let bytes = muxer.into_writer();

    let top = parse_boxes(&bytes);
    let moov = top.iter().find(|b| b.typ == *b"moov").unwrap();
    let moov_payload = moov.payload(&bytes);
    let children = parse_boxes(moov_payload);
    let mvhd = children.iter().find(|b| b.typ == *b"mvhd").unwrap();
    let mvhd_duration = box_duration(mvhd.payload(moov_payload), 16, 24);

    let traks: Vec<_> = children.iter().filter(|b| b.typ == *b"trak").collect();
    assert_eq!(traks.len(), 2);
    for trak in traks {
        let trak_payload = trak.payload(moov_payload);
        let trak_children = parse_boxes(trak_payload);
        let tkhd = trak_children.iter().find(|b| b.typ == *b"tkhd").unwrap();
        let duration = box_duration(tkhd.payload(trak_payload), 20, 28);
        assert!(
            duration.abs_diff(mvhd_duration) <= 1,
            "tkhd {duration} vs mvhd {mvhd_duration}"
        );
        assert!(duration > 0, "tkhd duration must be written");
    }
    assert_eq!(mvhd_duration, 1000);
}

#[test]
fn fragmented_tkhd_duration_stays_zero() {
    use muxide::fragmented::{FragmentConfig, FragmentedMuxer};

    let mut muxer = FragmentedMuxer::new(FragmentConfig::new(320, 240, 90_000)).unwrap();
    let init = muxer.init_segment();
    let top = parse_boxes(&init);
    let moov = top.iter().find(|b| b.typ == *b"moov").unwrap();
    let moov_payload = moov.payload(&init);
    let children = parse_boxes(moov_payload);
    let trak = children.iter().find(|b| b.typ == *b"trak").unwrap();
    let trak_payload = trak.payload(moov_payload);
    let tkhd = parse_boxes(trak_payload)
        .into_iter()
        .find(|b| b.typ == *b"tkhd")
        .unwrap();
    assert_eq!(box_duration(tkhd.payload(trak_payload), 20, 28), 0);
}
