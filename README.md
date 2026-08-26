<p align="center">
  <img src="https://raw.githubusercontent.com/Michael-A-Kuykendall/muxide/main/assets/muxide-logo.png" alt="Muxide" width="350"><br>
  <strong>The last mile from encoder to playable MP4.</strong><br><br>
  <a href="https://crates.io/crates/muxide"><img src="https://img.shields.io/crates/v/muxide.svg" alt="Crates.io"></a> <a href="https://translifeline.org/"><img src="https://pride-badges.pony.workers.dev/static/v1?label=trans%20rights&stripeWidth=6&stripeColors=5BCEFA,F5A9B8,FFFFFF,F5A9B8,5BCEFA" alt="trans rights"></a> <a href="https://www.thetrevorproject.org/"><img src="https://pride-badges.pony.workers.dev/static/v1?label=lgbtq%2B%20friendly&stripeWidth=6&stripeColors=E40303,FF8C00,FFED00,008026,24408E,732982" alt="lgbtq+ friendly"></a>
  <a href="https://crates.io/crates/muxide"><img src="https://img.shields.io/crates/d/muxide.svg" alt="Downloads"></a>
  <a href="https://docs.rs/muxide"><img src="https://docs.rs/muxide/badge.svg" alt="Documentation"></a>
    <a href="#license"><img src="https://img.shields.io/crates/l/muxide.svg" alt="License"></a>
    <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/MSRV-1.74-blue.svg" alt="MSRV"></a>
  <a href="https://github.com/Michael-A-Kuykendall/muxide/actions"><img src="https://github.com/Michael-A-Kuykendall/muxide/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
</p>

<p align="center">
  <code>cargo add muxide</code>
</p>

<p align="center">
  <strong>Languages:</strong> <a href="docs/zh-CN/README.md">简体中文</a> · <a href="docs/zh-TW/README.md">繁體中文</a>
</p>

---

> **Muxide** takes correctly-timestamped, already-encoded audio/video frames and produces a standards-compliant MP4 — **pure Rust, minimal external dependencies, no FFmpeg.**

### 💝 Support Muxide

🚀 **If Muxide helps you, consider [sponsoring](https://github.com/sponsors/Michael-A-Kuykendall) — 100% of support goes to keeping it free forever.**

- **$5/month**: Coffee Hero ☕ — Eternal gratitude + name in [SPONSORS.md](SPONSORS.md)
- **$25/month**: Developer Supporter 🐛 — Priority bug response + roadmap influence
- **$100/month**: Corporate Backer 🏢 — Logo in README + release-note recognition
- **$500/month**: Enterprise Partner 🚀 — Prominent logo + monthly office hours + roadmap input

[**🎯 Become a Sponsor**](https://github.com/sponsors/Michael-A-Kuykendall) | See our amazing [sponsors](SPONSORS.md) 🙏

**Thank you to our sponsors:** [ZephyrCloudIO](https://github.com/ZephyrCloudIO) (Corporate Backer) · alistairheath (Coffee Hero)

---

<table>
<tr>
<td align="center"><strong>Your Encoder</strong><br><sub>H.264 / HEVC / AV1<br>AAC / Opus</sub></td>
<td align="center">➡️</td>
<td align="center"><strong>Muxide</strong><br><sub>Pure Rust<br>Minimal external deps</sub></td>
<td align="center">➡️</td>
<td align="center"><strong>playable.mp4</strong><br><sub>Standards-compliant<br>Fast-start ready</sub></td>
</tr>
</table>

---

## Why Muxide Exists

If you're building a recording pipeline in Rust, you know the tradeoffs:

| Approach | Tradeoff |
|----------|----------|
| **FFmpeg CLI/libs** | External binary, GPL licensing concerns, "which build is this?" |
| **GStreamer** | Complex plugin system, C dependencies, heavy runtime |
| **Raw MP4 writing** | ISO-BMFF expertise required (sample tables, interleaving, moov layout) |
| **"Minimal" crates** | Often missing fast-start, strict validation, or production ergonomics |

Muxide solves **one job cleanly**:

> Take already-encoded frames with correct timestamps → produce a **standards-compliant, immediately-playable MP4** → using **pure Rust**.

Nothing more. Nothing less.

## Installation & Usage

### As a Library
```bash
cargo add muxide
```

```rust
use muxide::api::{MuxerBuilder, VideoCodec};

let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::H264, 1920, 1080, 30.0)
    .build()?;

// Write your encoded frames...
muxer.write_video(0.0, &h264_frame, true)?;
muxer.finish()?;
```

### As a CLI Tool
```bash
# Install globally

cargo install muxide

# Or download pre-built binary from releases
# Then use:
muxide --help

# Quick examples:
muxide mux --video stream.h264 --output output.mp4 --width 1920 --height 1080 --fps 30
muxide mux --video video.h264 --audio audio.aac --output output.mp4
muxide validate --video input.h264 --audio input.aac
muxide info input.mp4
```

The CLI tool accepts hex-encoded input files and produces MP4 output.

## Core Invariant

Muxide enforces a strict contract:

| Your Responsibility | Muxide's Guarantee |
|:-------------------:|:------------------:|
| ✓ Frames are already encoded | ✓ Valid ISO-BMFF (MP4) |
| ✓ Timestamps are monotonic | ✓ Correct sample tables |
| ✓ DTS provided for B-frames | ✓ Fast-start layout |
| ✓ Codec headers in keyframes | ✓ No post-processing needed |

If input violates the contract, Muxide **fails fast** with explicit errors—no silent corruption, no guessing.

---

## Features

| Category | Supported | Notes |
|----------|-----------|-------|
| **Video** | H.264/AVC | Annex B format |
| | H.265/HEVC | Annex B with VPS/SPS/PPS |
| | AV1 | OBU stream format |
| | VP9 | Frame header parsing, resolution/bit-depth/color config extraction |
| **Audio** | AAC | All profiles: LC, Main, SSR, LTP, HE, HEv2 |
| | Opus | Raw packets, 48kHz |
| **Container** | Fast-start | `moov` before `mdat` for web playback |
| | B-frames | Explicit PTS/DTS support |
| | Fragmented MP4 | For DASH/HLS streaming |
| | Metadata | Title, creation time, language |
| **Quality** | Detailed error reporting | Hex dumps, JSON output, actionable messages |
| | Production tested | FFmpeg compatibility verified |
| | Comprehensive testing | 200+ tests, property-based validation |

### Design Principles

| Principle | Implementation |
|-----------|----------------|
| 🦀 **Pure Rust core** | Optional WASM + C FFI bindings (Go via cgo) |
| 📦 **Minimal deps** | Only essential Rust crates — no external binaries |
| 🧵 **Thread-safe** | `Send + Sync` when writer is |
| ✅ **Well-tested** | Unit, integration, property tests |
| 📜 **Permissive license** | Dual-licensed: MIT OR Apache-2.0 |
| 🛠️ **Developer-friendly** | Detailed error context with hex dumps and actionable suggestions |

> **Note:** `no_std` is not supported. Muxide requires `std::io::Write`.

---

## Quick Start

```rust
use muxide::api::{AacProfile, AudioCodec, MuxerBuilder, Metadata, VideoCodec};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create("recording.mp4")?;
    
    let mut muxer = MuxerBuilder::new(file)
        .video(VideoCodec::H264, 1920, 1080, 30.0)
        .audio(AudioCodec::Aac(AacProfile::Lc), 48000, 2)
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

<details>
<summary><strong>📹 More Examples: HEVC, AV1, Opus, Fragmented MP4</strong></summary>

### HEVC/H.265 (4K)

```rust
// Requires VPS, SPS, PPS in first keyframe
let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::H265, 3840, 2160, 30.0)
    .build()?;
muxer.write_video(0.0, &hevc_annexb_with_vps_sps_pps, true)?;
```

### AV1

```rust
// Requires Sequence Header OBU in first keyframe
let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::Av1, 1920, 1080, 60.0)
    .build()?;
muxer.write_video(0.0, &av1_obu_with_sequence_header, true)?;
```

### Opus Audio

```rust
// Opus always uses 48kHz internally (per spec)
let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::H264, 1920, 1080, 30.0)
    .audio(AudioCodec::Opus, 48000, 2)
    .build()?;
muxer.write_audio(0.0, &opus_packet)?;
```

### Fragmented MP4 (DASH/HLS)

```rust
use muxide::codec::vp9::Vp9Config;

// H.264
let sps_bytes = vec![0x67, 0x42, 0x00, 0x1e, 0xda, 0x02, 0x80, 0x2d, 0x8b, 0x11];
let pps_bytes = vec![0x68, 0xce, 0x38, 0x80];

let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::H264, 1920, 1080, 30.0)
    .with_sps(sps_bytes)
    .with_pps(pps_bytes)
    .new_with_fragment()?;

// H.265
let vps_bytes = vec![0x40, 0x01, 0x0c, 0x01, 0xff, 0xff, 0x01, 0x60, 0x00];
let sps_bytes = vec![0x42, 0x01, 0x01, 0x01, 0x60, 0x00, 0x00, 0x03, 0x00, 0x90, 0x00];
let pps_bytes = vec![0x44, 0x01, 0xc0, 0x73, 0xc0, 0x4c, 0x90];

let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::H265, 1920, 1080, 30.0)
    .with_vps(vps_bytes)
    .with_sps(sps_bytes)
    .with_pps(pps_bytes)
    .new_with_fragment()?;

// AV1
let seq_header_bytes = vec![
    0x0A, 0x10, // OBU header + size (example)
    0x00, 0x00, 0x00, 0x00,
];

let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::Av1, 1920, 1080, 30.0)
    .with_av1_sequence_header(seq_header_bytes)
    .new_with_fragment()?;

// VP9
let vp9_config = Vp9Config {
    width: 1920,
    height: 1080,
    profile: 0,
    bit_depth: 8,
    color_space: 0,
    transfer_function: 0,
    matrix_coefficients: 0,
    level: 0,
    full_range_flag: 0,
};

let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::Vp9, 1920, 1080, 30.0)
    .with_vp9_config(vp9_config)
    .new_with_fragment()?;

// Get init segment (ftyp + moov)
let init_segment = muxer.init_segment();

// Write frames...
muxer.write_video(0, 0, &frame, true)?;

// Get media segments (moof + mdat)
if let Some(segment) = muxer.flush_segment() {
    // Send segment to client
}
```

### B-Frames with Explicit DTS

```rust
// When encoder produces B-frames, provide both PTS and DTS
muxer.write_video_with_dts(
    pts_seconds,  // Presentation timestamp
    dts_seconds,  // Decode timestamp (for B-frame ordering)
    &frame_data,
    is_keyframe
)?;
```

</details>

---

## Command Line Tool

Muxide includes a command-line tool for quick testing and development workflows:

```bash
# Install the CLI tool
cargo install muxide

# Basic video-only muxing
muxide mux \
  --video keyframes.h264 \
  --width 1920 --height 1080 --fps 30 \
  --output recording.mp4

# Video + audio with metadata
muxide mux \
  --video stream.h264 \
  --audio stream.aac \
  --video-codec h264 \
  --audio-codec aac-he \
  --width 1920 --height 1080 --fps 30 \
  --sample-rate 44100 --channels 2 \
  --title "My Recording" \
  --language eng \
  --output final.mp4

# JSON output for automation
muxide mux --json [args...] > stats.json

# Validate input files without muxing
muxide validate --video input.h264 --audio input.aac

# Get info about an MP4 file
muxide info input.mp4
```

**Supported Codecs:**
- **Video:** H.264 (AVC), H.265 (HEVC), AV1, VP9
- **Audio:** AAC (all profiles), Opus

**Features:**
- Progress reporting with `--verbose`
- JSON output for CI/CD integration
- Comprehensive error messages
- Fast-start MP4 layout by default
- Metadata support (title, language, creation time)

---

## Language Bindings (WASM, Go, C)

Muxide's core is pure Rust, but it also ships **optional** language bindings so you can mux MP4 from non-Rust code. All bindings sit on top of the same muxer and emit identical, standards-compliant MP4.

### WASM (browser / JS)

The `wasm` feature compiles the muxer to WebAssembly via [`wasm-bindgen`](https://crates.io/crates/wasm-bindgen).

```bash
# Build the wasm package
cargo build --target wasm32-unknown-unknown --features wasm
# or, with wasm-pack:
wasm-pack build --target web --features wasm
```

```js
import { WasmMuxerBuilder, VideoCodec } from "./pkg/muxide.js";

const builder = new WasmMuxerBuilder();
builder.video(VideoCodec.H264, 1280, 720, 30.0);
const muxer = builder.build();

// First H.264 keyframe must contain SPS (NAL 7) + PPS (NAL 8).
muxer.writeVideo(0.0, h264Keyframe, true);

const bytes = muxer.finish(); // Uint8Array of the complete MP4
```

`WasmMuxerBuilder` also supports `.audio(...)` and `WasmMuxer` supports `writeVideoWithDts`, `writeAudio`, `encodeVideo`, and `encodeAudio`.

### Go (via C FFI + cgo)

The C FFI layer lives in `src/ffi.rs` with a matching header at `bindings/muxide.h`. The Go package under `bindings/go` wraps it with an idiomatic API.

```bash
# 1) Build the C dynamic library (cdylib)
cargo rustc --release --lib --crate-type cdylib
# 2) The Go package links against target/release/libmuxide
go build ./bindings/go/...
```

```go
package main

import (
    "fmt"
    muxide "github.com/Michael-A-Kuykendall/muxide/bindings/go"
)

func main() {
    m, err := muxide.NewMuxer(muxide.CodecH264, 1280, 720, 30.0)
    if err != nil {
        panic(err)
    }
    defer m.Close() // runtime finalizer also frees the handle

    keyframe := []byte{0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x1f, /* SPS */
        0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x3c, 0x80, /* PPS */
        0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00 /* IDR */}
    if err := m.WriteVideo(0.0, keyframe, true); err != nil {
        panic(err)
    }
    out, err := m.Finish()
    if err != nil {
        panic(err)
    }
    fmt.Printf("wrote %d bytes of MP4\n", len(out))
}
```

The Go API: `NewMuxer` / `NewMuxerWithAudio`, `WriteVideo`, `WriteVideoWithDTS`, `WriteAudio`, `Finish() ([]byte, error)`, and `Close()`. Rust errors are surfaced as Go `error` values via `muxide_last_error`.

### C / C++

Include `bindings/muxide.h` and link the cdylib. Opaque handles `MuxideMuxer` / `MuxideFragmentedMuxer`; all functions return `int32_t` error codes (`MUXIDE_OK = 0`, negatives for errors).

```c
#include "muxide.h"

MuxideMuxer* m = muxide_new(0 /*H264*/, 1280, 720, 30.0);
muxide_write_video(m, 0.0, keyframe, keyframe_len, 1 /*is_keyframe*/);
muxide_finish(m);
size_t len = muxide_output_length(m);
uint8_t* buf = malloc(len);
muxide_output_copy(m, buf, len);
muxide_free(m);
```

See [`bindings/muxide.h`](bindings/muxide.h) for the full list of 18 `extern "C"` functions, error constants, and the `MuxideMetadata` struct.

---

## What Muxide Is Not

Muxide is intentionally **focused**. It does **not**:

| Not Supported | Why |
|---------------|-----|
| Encoding/decoding | Use `openh264`, `x264`, `rav1e`, etc. |
| Transcoding | Not a codec library |
| Demuxing/reading MP4 | Write-only by design |
| Timestamp correction | Garbage in = error out |
| Non-MP4 containers | MKV, WebM, AVI not supported |
| DRM/encryption | Out of scope |

**Muxide is the last mile**: encoder output → playable file.

---

## Use Cases

Muxide is a great fit for:

- 🎥 **Screen recorders** — capture → encode → mux → ship
- 📹 **Camera apps** — webcam/IP camera recording pipelines
- 🎬 **Video editors** — export timeline to MP4
- 📡 **Streaming** — generate fMP4 segments for DASH/HLS
- 🏭 **Embedded systems** — single binary, no external deps
- 🔬 **Scientific apps** — deterministic, reproducible output

Probably **not** a fit if you need encoding, demuxing, or legacy codecs (MPEG-2, etc.).

---

## Performance

Muxide is designed for **low overhead**, but exact throughput depends on your bitstreams, storage, and hardware.

Criterion benchmarks live in [benches/muxing.rs](benches/muxing.rs). Run `cargo bench` on your target environment to measure realistic throughput for your workload.

> **Note:** Benchmark results are environment-specific. Encoding is typically the bottleneck; measure muxing overhead on representative inputs before publishing exact numbers.

### H.264/AVC

- **Format:** Annex B (start codes: `00 00 00 01` or `00 00 01`)
- **First keyframe must contain:** SPS and PPS NAL units
- **NAL unit types:** IDR (keyframe), non-IDR, SPS, PPS

### H.265/HEVC

- **Format:** Annex B (start codes)
- **First keyframe must contain:** VPS, SPS, and PPS NAL units
- **NAL unit types:** IDR_W_RADL, IDR_N_LP, CRA, VPS, SPS, PPS

### AV1

- **Format:** OBU (Open Bitstream Unit) stream
- **First keyframe must contain:** Sequence Header OBU
- **OBU types:** Sequence Header, Frame, Frame Header, Tile Group

### AAC

- **Format:** ADTS (Audio Data Transport Stream)
- **Header:** 7-byte ADTS header per frame
- **Profiles:** LC-AAC recommended

### Opus

- **Format:** Raw Opus packets (no container)
- **Sample rate:** Always 48000 Hz (Opus specification)
- **Channels:** 1 (mono) or 2 (stereo)

---

## Documentation

| Resource | Description |
|----------|-------------|
| [📚 API Reference](https://docs.rs/muxide) | Complete API documentation |
| [📜 Design Charter](docs/charter.md) | Architecture decisions and rationale |
| [📋 API Contract](docs/contract.md) | Input/output guarantees |

---

## FAQ

<details>
<summary><strong>Why not just use FFmpeg?</strong></summary>

FFmpeg is excellent, but:
- External binary dependency (distribution complexity)
- GPL licensing concerns for some builds
- Process orchestration overhead
- "What flags was this built with?" debugging

Muxide is a single `cargo add` with minimal external dependencies.

</details>

<details>
<summary><strong>Can Muxide encode video?</strong></summary>

No. Muxide is **muxing only**. For encoding, use:
- `openh264` — H.264 encoding (BSD)
- `rav1e` — AV1 encoding (BSD)
- `x264`/`x265` — H.264/HEVC (GPL, via FFI)

</details>

<details>
<summary><strong>What if my timestamps are wrong?</strong></summary>

Muxide will reject non-monotonic timestamps with a clear error. It does not attempt to "fix" broken input — this is by design to ensure predictable output.

</details>

<details>
<summary><strong>Is Muxide production-ready?</strong></summary>

Yes. Muxide has an extensive test suite (unit, integration, property-based tests) and is designed for predictable, deterministic behavior.

</details>

---

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

---

<p align="center">
  <em>Muxide is designed to be <strong>boring</strong> in the best way:<br>predictable, strict, fast, and invisible once integrated.</em>

## Support

This project is a safe space. Trans rights are human rights.

If you or someone you love needs support:

- [The Trevor Project](https://www.thetrevorproject.org/) — 24/7 for LGBTQ+ young people. Call 1-866-488-7386 or text START to 678-678
- [Trans Lifeline](https://translifeline.org/) — peer support run by and for trans people. US: 877-565-8860
- [988 Suicide & Crisis Lifeline](https://988lifeline.org/) — call or text 988

