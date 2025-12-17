<p align="center">
  <img src="assets/muxide-logo.png" alt="Muxide" width="350"><br>
  <em>Zero-dependency pure-Rust MP4 muxer for recording applications.</em><br>
  <a href="https://crates.io/crates/muxide"><img src="https://img.shields.io/crates/v/muxide.svg" alt="Crates.io"></a>
  <a href="https://docs.rs/muxide"><img src="https://docs.rs/muxide/badge.svg" alt="Documentation"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/MSRV-1.70-blue.svg" alt="MSRV"></a>
  <a href="https://github.com/Michael-A-Kuykendall/muxide/actions"><img src="https://github.com/Michael-A-Kuykendall/muxide/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
</p>

> **Muxide turns correctly-timestamped, already-encoded audio/video frames into a standards-compliant MP4 — pure Rust, zero runtime deps, no external tooling required for muxing.**

## Why Muxide Exists

If you are building a recording pipeline in Rust, you already know the tradeoffs:

- **FFmpeg** is powerful, but usually implies an external binary (distribution, process orchestration, and “what was this build configured with?” questions).
- Low-level MP4 writing APIs exist, but they require container expertise (sample tables, interleaving, seeking/indexing, and box layout).
- Many “minimal” approaches fall down on fast-start layout, strict validation, or production-grade ergonomics.

Muxide exists to solve one job **cleanly and completely**:

> Take already-encoded audio/video frames with correct timestamps and produce a
> **standards-compliant, immediately-playable MP4** using **pure Rust**.

Nothing more. Nothing less.

## Core Invariant

Muxide enforces a strict contract:

- **Input must be already encoded** (H.264/H.265/AV1 video; AAC/Opus audio)
- **Timestamps must be correct** (monotonic PTS; provide DTS explicitly for B-frames)
- **Muxide does not guess or “fix” broken streams**

In return, Muxide guarantees:

- A valid ISO-BMFF (MP4) file
- Correct sample tables and offsets
- Optional fast-start layout (moov before mdat)
- No post-processing or external tools required

If the input violates the contract, Muxide fails fast with explicit errors.

## Features

### Video Codecs
- ✅ **H.264/AVC** (Annex B format)
- ✅ **H.265/HEVC** (Annex B format with VPS/SPS/PPS)
- ✅ **AV1** (OBU stream format)

### Audio Codecs
- ✅ **AAC** (ADTS format)
- ✅ **Opus** (raw packets, 48kHz)

### Container Features
- ✅ **Fast-start** (moov before mdat) for web-friendly playback
- ✅ **B-frame support** via explicit PTS/DTS
- ✅ **Fragmented MP4** for DASH/HLS streaming
- ✅ **Metadata** (title, creation time)

### Philosophy
- ✅ **Zero runtime dependencies** (only std)
- ✅ **Pure Rust** (no unsafe, no FFI)
- ✅ **Thread-safe** (`Send + Sync` when writer is)
- ✅ **Extensive test suite** (unit, integration, and property tests)
- ✅ **MIT OR Apache-2.0** (no GPL)

> **Note:** `no_std` is not supported. Muxide requires `std::io::Write` for output.

## Quick Start

```rust
use muxide::api::{MuxerBuilder, VideoCodec, AudioCodec, Metadata};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create("recording.mp4")?;
    
    let mut muxer = MuxerBuilder::new(file)
        .video(VideoCodec::H264, 1920, 1080, 30.0)
        .audio(AudioCodec::Aac, 48000, 2)
        .with_metadata(Metadata::new().with_title("My Recording"))
        .with_fast_start(true)
        .build()?;

    // Write encoded frames (from your encoder)
    // muxer.write_video(pts_seconds, h264_annex_b_bytes, is_keyframe)?;
    // muxer.write_audio(pts_seconds, aac_adts_bytes)?;

    let stats = muxer.finish_with_stats()?;
    println!("Wrote {} frames, {} bytes", stats.video_frames, stats.bytes_written);
    Ok(())
}
```

### Other Codecs

```rust
// HEVC/H.265 - requires VPS, SPS, PPS in first keyframe
let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::H265, 3840, 2160, 30.0)  // 4K HEVC
    .build()?;
muxer.write_video(0.0, &hevc_annexb_with_vps_sps_pps, true)?;

// AV1 - requires Sequence Header OBU in first keyframe
let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::Av1, 1920, 1080, 60.0)
    .build()?;
muxer.write_video(0.0, &av1_obu_with_sequence_header, true)?;

// Opus audio - sample rate is always 48kHz per Opus spec
// (input sample_rate parameter is accepted but internally normalized)
let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::H264, 1920, 1080, 30.0)
    .audio(AudioCodec::Opus, 48000, 2)  // Opus requires 48kHz
    .build()?;
muxer.write_audio(0.0, &opus_packet)?;
```

## What Muxide Is Not

Muxide is intentionally opinionated. It does **not**:

- Encode or decode audio/video
- Transcode between codecs
- Read/demux existing MP4 files
- Perform heuristic timestamp correction
- Support non-MP4 containers (MKV, WebM, etc.)
- Handle DRM or encryption

Muxide is the **last mile**: from encoder output to a playable file.

## Example: Fast-Start Proof

The `faststart_proof` example demonstrates a structural MP4 invariant:

- Two MP4 files are generated from the same encoded inputs
- One with fast-start enabled, one without
- No external tools are used at any stage

```text
$ cargo run --example faststart_proof --release

output: recording_faststart.mp4
    layout invariant: moov before mdat = YES

output: recording_normal.mp4
    layout invariant: moov before mdat = NO
```

When served over HTTP, the fast-start file can begin playback without waiting for the full download (player behavior varies, but the layout property is deterministic).

This example is intentionally minimal:

- Timestamps are generated in-code
- No B-frames/DTS paths are exercised
- The goal is container layout correctness, not encoding quality

## Who This Is For

Muxide is a good fit if you are:

- Building a screen recorder, camera app, or capture pipeline
- Writing a video editor/exporter in Rust
- Shipping MP4 output without bundling external binaries
- Generating fMP4 segments for streaming
- Operating in environments where external tooling is undesirable

Muxide is probably not a fit if you need encoding/transcoding, container introspection/editing, or legacy codec support.

## Performance

Muxide is designed for minimal overhead. Benchmarks run on a standard development machine:

| Scenario | Time | Throughput |
|----------|------|------------|
| 1000 H.264 frames | 264 µs | **3.7M frames/sec** |
| 1000 H.264 frames (fast-start) | 362 µs | 2.8M frames/sec |
| 1000 video + 1500 audio | 457 µs | 2.2M frames/sec |
| 100 4K frames (~6.5 MB) | 14 ms | **464 MB/sec** |

Run benchmarks yourself: `cargo bench`

In practice, encoding is always the bottleneck—muxing overhead is negligible.

## Documentation

- [API Reference (docs.rs)](https://docs.rs/muxide)
- [Design Charter](docs/charter.md)
- [API Contract](docs/contract.md)

## License

Dual licensed under MIT OR Apache-2.0.

Muxide is designed to be boring in the best way: predictable, strict, fast, and invisible once integrated.