# Muxide

**Zero-dependency pure-Rust MP4 muxer for recording applications.**

[![Crates.io](https://img.shields.io/crates/v/muxide.svg)](https://crates.io/crates/muxide)
[![Documentation](https://docs.rs/muxide/badge.svg)](https://docs.rs/muxide)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

> **Muxide guarantees that any correctly-timestamped, already-encoded audio/video stream can be turned into a standards-compliant, immediately-playable MP4 without external tooling.**

## Features

- ✅ **H.264/AVC** video (Annex B format)
- ✅ **AAC** audio (ADTS format)
- ✅ **Fast-start** (moov before mdat) for instant web playback
- ✅ **B-frame support** via explicit PTS/DTS
- ✅ **Fragmented MP4** for DASH/HLS streaming
- ✅ **Metadata** (title, creation time)
- ✅ **Zero dependencies** (only std)
- ✅ **MIT licensed** (no GPL)

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

## What Muxide Is

| ✅ Muxide Does | ❌ Muxide Does NOT |
|----------------|--------------------|
| Accept encoded frames + timestamps | Encode or decode video/audio |
| Output playable MP4 files | Read or parse MP4 files |
| Fast-start for web streaming | Fix broken timestamps |
| Fragmented MP4 for live streaming | DRM or encryption |
| Strict input validation | MKV, WebM, or other formats |

## Why Muxide?

| Feature | muxide | `mp4` crate | `mp4e` | FFmpeg |
|---------|--------|-------------|--------|--------|
| Pure Rust | ✅ | ✅ | ✅ | ❌ |
| Zero deps | ✅ | ❌ (6 deps) | ✅ | ❌ |
| Fast-start | ✅ | ❌ | ❌ | ✅ |
| MIT license | ✅ | ✅ | ✅ | ❌ (GPL) |
| Maintained | ✅ | ❌ (2yr stale) | 🟡 | ✅ |
| Builder API | ✅ | ❌ | ❌ | N/A |

## Documentation

- [API Reference (docs.rs)](https://docs.rs/muxide)
- [Design Charter](docs/charter.md)
- [API Contract](docs/contract.md)

## License

Dual licensed under MIT OR Apache-2.0.