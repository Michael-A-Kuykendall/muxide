use muxide::api::{MuxerBuilder, VideoCodec};
use muxide::fragmented::{FragmentConfig, FragmentedMuxer};

fn sps_with_profile_tier_level() -> Vec<u8> {
    let mut sps = vec![0x42, 0x01, 0x01, 0x01];
    sps.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]);
    sps.extend_from_slice(&[0x55, 0x66, 0x77, 0x88, 0x99, 0xaa]);
    sps.push(0x5d);
    sps
}

fn hvcc_payload(data: &[u8]) -> &[u8] {
    let pos = data.windows(4).position(|w| w == b"hvcC").expect("hvcC");
    let size = u32::from_be_bytes(data[pos - 4..pos].try_into().unwrap()) as usize;
    &data[pos + 4..pos - 4 + size]
}

fn assert_hvcc_matches_sps(payload: &[u8], sps: &[u8]) {
    assert_eq!(payload[0], 1);
    assert_eq!(payload[1], sps[3]);
    assert_eq!(&payload[2..6], &sps[4..8]);
    assert_eq!(&payload[6..12], &sps[8..14]);
    assert_eq!(payload[12], sps[14]);
    assert_eq!(payload[21] & 0x03, 3, "lengthSizeMinusOne");
}

#[test]
fn hvcc_profile_tier_level_matches_sps() {
    let sps = sps_with_profile_tier_level();
    let vps = vec![0x40, 0x01];
    let pps = vec![0x44, 0x01];

    let mut fragmented = FragmentedMuxer::new(FragmentConfig {
        width: 320,
        height: 240,
        timescale: 90_000,
        fragment_duration_ms: 1000,
        sps: sps.clone(),
        pps: pps.clone(),
        vps: Some(vps.clone()),
        av1_sequence_header: None,
        vp9_config: None,
    })
    .unwrap();
    let init = fragmented.init_segment();
    assert_hvcc_matches_sps(hvcc_payload(&init), &sps);

    let idr = vec![0x26, 0x01];
    let mut annexb = Vec::new();
    for nal in [&vps, &sps, &pps, &idr] {
        annexb.extend_from_slice(&[0, 0, 0, 1]);
        annexb.extend_from_slice(nal);
    }
    let mut muxer = MuxerBuilder::new(Vec::<u8>::new())
        .video(VideoCodec::H265, 320, 240, 30.0)
        .build()
        .unwrap();
    muxer.write_video(0.0, &annexb, true).unwrap();
    muxer.write_video(0.1, &annexb, false).unwrap();
    muxer.finish_in_place().unwrap();
    let bytes = muxer.into_writer();
    assert_hvcc_matches_sps(hvcc_payload(&bytes), &sps);
}
