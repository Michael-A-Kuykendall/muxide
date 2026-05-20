//! Functional interop test: H.264 video + AAC audio muxed together.
//!
//! # Prerequisites
//!
//! ffmpeg must be on `PATH`. Run:
//!   ffmpeg -f lavfi -i "sine=frequency=1000:sample_rate=44100:duration=1" \
//!     -c:a aac -b:a 128k -f adts target/interop_av/audio.aac
//!   ffmpeg -f lavfi -i testsrc=size=640x480:rate=30 -t 1 \
//!     -pix_fmt yuv420p -c:v libx264 -x264-params aud=1:repeat-headers=1 \
//!     -f h264 target/interop_av/video.h264
//!
//! # Usage
//!
//!   cargo run --example interop_av_h264_aac -- target/interop_av
//!
//! # Validate
//!
//!   ffprobe -v error -show_streams -of compact target/interop_av/muxide_av.mp4

use muxide::{
    api::{AacProfile, AudioCodec, MuxerBuilder, VideoCodec},
    codec::{
        common::AnnexBNalIter,
        h264::is_h264_keyframe,
    },
};
use std::{
    env,
    fs::{self, File},
    io::{Read},
    path::{Path, PathBuf},
    process::Command,
};

fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(buf)
}

fn split_access_units(annexb: &[u8]) -> Vec<Vec<u8>> {
    let start_code: [u8; 4] = [0, 0, 0, 1];
    let mut aus: Vec<Vec<u8>> = Vec::new();
    let mut current: Vec<u8> = Vec::new();

    for nal in AnnexBNalIter::new(annexb) {
        if nal.is_empty() {
            continue;
        }
        let nal_type = nal[0] & 0x1f;
        if nal_type == 9 && !current.is_empty() {
            aus.push(current);
            current = Vec::new();
        }
        current.extend_from_slice(&start_code);
        current.extend_from_slice(nal);
    }
    if !current.is_empty() {
        aus.push(current);
    }
    aus
}

/// Parse ADTS frames from a raw ADTS byte stream. Returns each frame as a Vec<u8>.
fn split_adts_frames(data: &[u8]) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    let mut i = 0;
    while i + 7 <= data.len() {
        // Sync word: 0xFFF
        if data[i] != 0xFF || (data[i + 1] & 0xF0) != 0xF0 {
            i += 1;
            continue;
        }
        // protection_absent is bit 0 of data[i+1]
        let protection_absent = (data[i + 1] & 0x01) != 0;
        let header_size = if protection_absent { 7 } else { 9 };
        if i + header_size > data.len() {
            break;
        }
        // frame length is bits 30-43 of the header (13 bits)
        let frame_len = (((data[i + 3] & 0x03) as usize) << 11)
            | ((data[i + 4] as usize) << 3)
            | ((data[i + 5] as usize) >> 5);
        if frame_len < header_size || i + frame_len > data.len() {
            break;
        }
        frames.push(data[i..i + frame_len].to_vec());
        i += frame_len;
    }
    frames
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir: PathBuf = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/interop_av"));

    fs::create_dir_all(&out_dir)?;

    // --- Generate inputs with ffmpeg if not already present ---
    let video_path = out_dir.join("video.h264");
    let audio_path = out_dir.join("audio.aac");

    if !video_path.exists() {
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=size=640x480:rate=30",
                "-t", "1",
                "-pix_fmt", "yuv420p",
                "-c:v", "libx264",
                "-x264-params", "aud=1:repeat-headers=1",
                "-f", "h264",
                video_path.to_str().unwrap(),
            ])
            .status()?;
        assert!(status.success(), "ffmpeg video generation failed");
    }

    if !audio_path.exists() {
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "sine=frequency=1000:sample_rate=44100:duration=1",
                "-c:a", "aac",
                "-b:a", "128k",
                "-f", "adts",
                audio_path.to_str().unwrap(),
            ])
            .status()?;
        assert!(status.success(), "ffmpeg audio generation failed");
    }

    // --- Read and parse inputs ---
    let annexb = read_file(&video_path)?;
    let adts_bytes = read_file(&audio_path)?;

    let access_units = split_access_units(&annexb);
    let audio_frames = split_adts_frames(&adts_bytes);

    println!("Video access units: {}", access_units.len());
    println!("Audio ADTS frames: {}", audio_frames.len());
    assert!(!access_units.is_empty(), "no video access units found");
    assert!(!audio_frames.is_empty(), "no audio frames found");

    // Find first keyframe
    let first_key_idx = access_units
        .iter()
        .position(|au| is_h264_keyframe(au))
        .expect("no keyframe found in H.264 stream");

    // --- Mux with muxide ---
    let out_path = out_dir.join("muxide_av.mp4");
    let out_file = File::create(&out_path)?;

    let mut muxer = MuxerBuilder::new(out_file)
        .video(VideoCodec::H264, 640, 480, 30.0)
        .audio(AudioCodec::Aac(AacProfile::Lc), 44100, 2)
        .build()?;

    // Write video frames (audio interleaved at matching PTS)
    let timescale = 90_000u64;
    let frame_ticks = timescale / 30; // 3000 per frame

    // Number of audio frames per video frame (approx)
    let audio_total = audio_frames.len();
    let video_total = access_units.len() - first_key_idx;

    for (vi, au) in access_units[first_key_idx..].iter().enumerate() {
        let pts = (vi as u64) * frame_ticks;
        let is_key = is_h264_keyframe(au);
        // The muxer converts Annex B → AVCC internally; pass the raw AU
        muxer.write_video(pts as f64 / timescale as f64, au, is_key)?;

        // Interleave audio frames that fall within this video frame's time window
        let audio_start = vi * audio_total / video_total;
        let audio_end = ((vi + 1) * audio_total / video_total).min(audio_total);
        for ai in audio_start..audio_end {
            let audio_pts = ai as f64 * 1024.0 / 44100.0; // 1024 samples/frame at 44100 Hz
            muxer.write_audio(audio_pts, &audio_frames[ai])?;
        }
    }

    muxer.finish()?;
    let out_size = std::fs::metadata(&out_path)?.len();
    println!("Wrote: {} ({} bytes)", out_path.display(), out_size);

    // --- Validate with ffprobe ---
    println!("\n--- ffprobe validation ---");
    let ffprobe_out = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_streams",
            "-of", "compact",
            out_path.to_str().unwrap(),
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&ffprobe_out.stdout);
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split('|').collect();
        let extract = |key: &str| -> &str {
            fields.iter()
                .find(|f| f.starts_with(key) && f[key.len()..].starts_with('='))
                .and_then(|f| f.splitn(2, '=').nth(1))
                .unwrap_or("?")
        };
        println!(
            "  track {}: codec={} type={} {}x{} {}Hz ch={} duration={}s disposition:default={}",
            extract("index"),
            extract("codec_name"),
            extract("codec_type"),
            extract("width"),
            extract("height"),
            extract("sample_rate"),
            extract("channels"),
            extract("duration"),
            extract("disposition:default"),
        );
    }

    // Check for ffprobe errors
    let stderr = String::from_utf8_lossy(&ffprobe_out.stderr);
    if !stderr.trim().is_empty() {
        eprintln!("ffprobe stderr: {}", stderr);
    }

    // --- Decode test ---
    println!("\n--- Decode test (ffmpeg -f null) ---");
    let decode_out = Command::new("ffmpeg")
        .args(["-i", out_path.to_str().unwrap(), "-f", "null", "-"])
        .output()?;
    let decode_stderr = String::from_utf8_lossy(&decode_out.stderr);
    // Print summary line
    for line in decode_stderr.lines() {
        if line.contains("frame=") || line.contains("error") {
            println!("  {}", line.trim());
        }
    }

    println!("\n✅ interop_av_h264_aac complete");
    Ok(())
}
