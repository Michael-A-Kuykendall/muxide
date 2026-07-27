use criterion::{criterion_group, criterion_main, Criterion};
use muxide::api::{AacProfile, AudioCodec, MuxerBuilder, VideoCodec};
use std::io::Cursor;

fn decode_hex_fixture(contents: &str) -> Vec<u8> {
    let hex: String = contents.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16).expect("valid fixture hex");
        out.push(byte);
    }
    out
}

fn h264_keyframe() -> Vec<u8> {
    decode_hex_fixture(include_str!("../fixtures/video_samples/frame0_key.264"))
}

fn h264_pframe() -> Vec<u8> {
    decode_hex_fixture(include_str!("../fixtures/video_samples/frame1_p.264"))
}

fn aac_frame() -> Vec<u8> {
    decode_hex_fixture(include_str!("../fixtures/audio_samples/frame0.aac.adts"))
}

fn bench_h264_muxing(c: &mut Criterion) {
    let keyframe = h264_keyframe();
    let pframe = h264_pframe();

    c.bench_function("mux_1000_h264_frames", |b| {
        b.iter(|| {
            let mut buffer = Vec::new();
            let writer = Cursor::new(&mut buffer);
            let mut muxer = MuxerBuilder::new(writer)
                .video(VideoCodec::H264, 1920, 1080, 30.0)
                .build()
                .expect("build muxer");

            for i in 0..1000 {
                let pts = i as f64 / 30.0;
                let (frame, is_keyframe) = if i == 0 || i % 30 == 0 {
                    (&keyframe, true)
                } else {
                    (&pframe, false)
                };
                muxer
                    .write_video(pts, frame, is_keyframe)
                    .expect("write video sample");
            }
            muxer.finish().expect("finish muxer");
            std::hint::black_box(buffer);
        });
    });
}

fn bench_h264_with_audio(c: &mut Criterion) {
    let keyframe = h264_keyframe();
    let pframe = h264_pframe();
    let audio = aac_frame();

    c.bench_function("mux_1000_h264_audio_frames", |b| {
        b.iter(|| {
            let mut buffer = Vec::new();
            let writer = Cursor::new(&mut buffer);
            let mut muxer = MuxerBuilder::new(writer)
                .video(VideoCodec::H264, 1920, 1080, 30.0)
                .audio(AudioCodec::Aac(AacProfile::Lc), 48000, 2)
                .build()
                .expect("build muxer");

            for i in 0..1000 {
                let pts = i as f64 / 30.0;
                let (frame, is_keyframe) = if i == 0 || i % 30 == 0 {
                    (&keyframe, true)
                } else {
                    (&pframe, false)
                };
                muxer
                    .write_video(pts, frame, is_keyframe)
                    .expect("write video sample");
                muxer.write_audio(pts, &audio).expect("write audio sample");
            }
            muxer.finish().expect("finish muxer");
            std::hint::black_box(buffer);
        });
    });
}

criterion_group!(benches, bench_h264_muxing, bench_h264_with_audio);
criterion_main!(benches);
