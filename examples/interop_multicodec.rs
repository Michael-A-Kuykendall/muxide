//! Interop check for H.265, VP9, and AV1 using ffmpeg-generated streams.
//!
//! Generates a 1-second test stream with each codec, muxes it into an MP4 file
//! using the regular non-fragmented API, and writes the file for inspection.
//!
//! # Prerequisites
//!
//! ffmpeg must be on `PATH` with `libx265`, `libvpx-vp9`, and `libaom-av1`.
//!
//! # Usage
//!
//! ```text
//! cargo run --example interop_multicodec -- [out_dir]
//! # default out_dir is target/interop_mc
//! ```
//!
//! # Validation
//!
//! ```text
//! ffprobe -v error -show_streams -of compact target/interop_mc/hevc.mp4
//! ffprobe -v error -show_streams -of compact target/interop_mc/vp9.mp4
//! ffprobe -v error -show_streams -of compact target/interop_mc/av1.mp4
//! ```

use muxide::{
    api::{MuxerBuilder, VideoCodec},
    codec::{
        av1::is_av1_keyframe,
        common::AnnexBNalIter,
        h265::{hevc_nal_type, is_hevc_keyframe, nal_type as h265_nal},
        vp9::is_vp9_keyframe,
    },
};
use std::{
    env,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

// ── helpers ─────────────────────────────────────────────────────────────────

fn run_ffmpeg(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("ffmpeg").args(args).status()?;
    if !status.success() {
        return Err(format!("ffmpeg failed with args: {:?}", args).into());
    }
    Ok(())
}

fn read_file_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Split a raw H.265 Annex B stream into access units delimited by AUD NALs
/// (type 35).  Each returned slice is a complete Annex B access unit
/// (start-code + NAL units up to but not including the next AUD).
fn split_hevc_access_units(data: &[u8]) -> Vec<Vec<u8>> {
    let start_code: [u8; 4] = [0, 0, 0, 1];
    let mut aus: Vec<Vec<u8>> = Vec::new();
    let mut current: Vec<u8> = Vec::new();

    for nal in AnnexBNalIter::new(data) {
        if nal.is_empty() {
            continue;
        }
        let nal_type = hevc_nal_type(nal);
        if nal_type == h265_nal::AUD && !current.is_empty() {
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

/// Parse IVF container frames.
///
/// IVF file header: 32 bytes (`DKIF` signature + metadata).
/// Each frame: 4-byte LE `size` + 8-byte LE `pts` + `size` bytes of frame data.
///
/// Returns `Vec<(pts, frame_data)>`.
fn parse_ivf_frames(data: &[u8]) -> Vec<(u64, Vec<u8>)> {
    if data.len() < 32 || &data[0..4] != b"DKIF" {
        return Vec::new();
    }
    let mut frames = Vec::new();
    let mut pos = 32usize;
    while pos + 12 <= data.len() {
        let frame_size = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let pts = u64::from_le_bytes(data[pos + 4..pos + 12].try_into().unwrap());
        pos += 12;
        if pos + frame_size > data.len() {
            break;
        }
        frames.push((pts, data[pos..pos + frame_size].to_vec()));
        pos += frame_size;
    }
    frames
}

// ── codec runners ────────────────────────────────────────────────────────────

fn run_h265(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let raw_path = out_dir.join("stream.hevc");
    let mp4_path = out_dir.join("hevc.mp4");

    run_ffmpeg(&[
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=640x480:rate=30",
        "-t",
        "1",
        "-pix_fmt",
        "yuv420p",
        "-c:v",
        "libx265",
        "-x265-params",
        "aud=1:repeat-headers=1:keyint=30",
        "-f",
        "hevc",
        raw_path.to_str().unwrap(),
    ])?;

    let data = read_file_bytes(&raw_path)?;
    let aus = split_hevc_access_units(&data);
    eprintln!("H.265: {} access units", aus.len());

    let file = File::create(&mp4_path)?;
    let mut muxer = MuxerBuilder::new(file)
        .video(VideoCodec::H265, 640, 480, 30.0)
        .build()?;

    for (i, au) in aus.iter().enumerate() {
        let pts = i as f64 / 30.0;
        muxer.write_video(pts, au, is_hevc_keyframe(au))?;
    }
    muxer.finish()?;
    eprintln!("Wrote: {}", mp4_path.display());
    Ok(())
}

fn run_vp9(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let ivf_path = out_dir.join("stream.vp9.ivf");
    let mp4_path = out_dir.join("vp9.mp4");

    run_ffmpeg(&[
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=640x480:rate=30",
        "-t",
        "1",
        "-pix_fmt",
        "yuv420p",
        "-c:v",
        "libvpx-vp9",
        "-g",
        "30",
        "-f",
        "ivf",
        ivf_path.to_str().unwrap(),
    ])?;

    let data = read_file_bytes(&ivf_path)?;
    let frames = parse_ivf_frames(&data);
    eprintln!("VP9: {} frames", frames.len());

    let file = File::create(&mp4_path)?;
    let mut muxer = MuxerBuilder::new(file)
        .video(VideoCodec::Vp9, 640, 480, 30.0)
        .build()?;

    for (i, (_pts, frame)) in frames.iter().enumerate() {
        let ts = i as f64 / 30.0;
        let is_key = is_vp9_keyframe(frame).unwrap_or(false);
        muxer.write_video(ts, frame, is_key)?;
    }
    muxer.finish()?;
    eprintln!("Wrote: {}", mp4_path.display());
    Ok(())
}

fn run_av1(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let ivf_path = out_dir.join("stream.av1.ivf");
    let mp4_path = out_dir.join("av1.mp4");

    // -cpu-used 8 = fastest libaom preset; -g 30 = one keyframe per second
    run_ffmpeg(&[
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=640x480:rate=30",
        "-t",
        "1",
        "-pix_fmt",
        "yuv420p",
        "-c:v",
        "libaom-av1",
        "-cpu-used",
        "8",
        "-g",
        "30",
        "-f",
        "ivf",
        ivf_path.to_str().unwrap(),
    ])?;

    let data = read_file_bytes(&ivf_path)?;
    let frames = parse_ivf_frames(&data);
    eprintln!("AV1: {} frames", frames.len());

    let file = File::create(&mp4_path)?;
    let mut muxer = MuxerBuilder::new(file)
        .video(VideoCodec::Av1, 640, 480, 30.0)
        .build()?;

    for (i, (_pts, frame)) in frames.iter().enumerate() {
        let ts = i as f64 / 30.0;
        muxer.write_video(ts, frame, is_av1_keyframe(frame))?;
    }
    muxer.finish()?;
    eprintln!("Wrote: {}", mp4_path.display());
    Ok(())
}

// ── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir: PathBuf = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/interop_mc"));

    fs::create_dir_all(&out_dir)?;

    eprintln!("==> H.265 interop ...");
    run_h265(&out_dir)?;

    eprintln!("==> VP9 interop ...");
    run_vp9(&out_dir)?;

    eprintln!("==> AV1 interop (libaom is slow; ~30 s) ...");
    run_av1(&out_dir)?;

    eprintln!("\nAll three codecs written. Validate with:");
    eprintln!(
        "  ffprobe -v error -show_streams -of compact {}",
        out_dir.join("hevc.mp4").display()
    );
    eprintln!(
        "  ffprobe -v error -show_streams -of compact {}",
        out_dir.join("vp9.mp4").display()
    );
    eprintln!(
        "  ffprobe -v error -show_streams -of compact {}",
        out_dir.join("av1.mp4").display()
    );
    Ok(())
}
