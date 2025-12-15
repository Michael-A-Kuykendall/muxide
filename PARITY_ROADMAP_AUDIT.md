# Muxide Parity Roadmap - External Audit Request

**Document Purpose:** Complete specification for achieving feature parity (and beyond) with all Rust MP4/container muxing solutions. This document is intended for third-party evaluation, prioritization, and architectural review.

**Created:** December 15, 2025  
**Author:** Human + AI collaboration  
**Target Reviewer:** High-order reasoning model for audit and optimization

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current State Assessment](#current-state-assessment)
3. [Competitive Landscape](#competitive-landscape)
4. [Feature Parity Matrix](#feature-parity-matrix)
5. [Detailed Feature Specifications](#detailed-feature-specifications)
6. [Testing Strategy](#testing-strategy)
7. [Documentation Requirements](#documentation-requirements)
8. [Risk Assessment](#risk-assessment)
9. [Prioritization Request](#prioritization-request)
10. [Success Criteria](#success-criteria)

---

## 1. Executive Summary

### What is Muxide?

Muxide is a **pure-Rust, zero-dependency MP4 muxer** designed for recording applications (screen capture, camera recording, game capture). It transforms raw H.264 video and AAC audio into playable MP4 files.

### Why Does It Exist?

The Rust ecosystem lacks a serious, actively-maintained MP4 muxer that:
- Has zero external dependencies
- Supports fast-start (moov-before-mdat) for instant web playback
- Provides a recording-oriented API (not a generic container toolkit)
- Is MIT licensed (no GPL contamination)

### Current Velocity

- **Day 1 output:** 2,693 lines of source code, 1,233 lines of tests (22 tests), fully functional muxer
- **Estimated complexity:** 13 Fibonacci points
- **Architecture:** Clean separation between public API (`api.rs`), MP4 box writing (`muxer/mp4.rs`), and fragmented streaming (`fragmented.rs`)

### Goal of This Document

Provide complete specifications for all features needed to achieve **full parity** with every Rust MP4/container solution, plus **differentiated features** that no competitor has. Request third-party audit for:
1. Prioritization of features
2. Architectural recommendations
3. Risk identification
4. Optimal slicing strategy

---

## 2. Current State Assessment

### 2.1 Architecture Overview

```
muxide/
├── src/
│   ├── lib.rs              (35 lines)   - Crate entry point, re-exports
│   ├── api.rs              (525 lines)  - Public API: MuxerBuilder, Muxer, MuxerConfig
│   ├── config.rs           (25 lines)   - Configuration types
│   ├── fragmented.rs       (700 lines)  - Fragmented MP4 (fMP4) for streaming
│   └── muxer/
│       ├── mod.rs          (1 line)     - Module declaration
│       └── mp4.rs          (1407 lines) - Core MP4 box writing logic
├── tests/                  (1233 lines) - 22 integration tests across 19 files
└── docs/
    ├── charter.md          - Design principles
    └── contract.md         - API invariants and guarantees
```

**Total Source:** 2,693 lines  
**Total Tests:** 1,233 lines (22 tests)  
**Test Ratio:** 0.46 (tests per source line)

### 2.2 Implemented Features

| Feature | Status | Implementation Location |
|---------|--------|------------------------|
| H.264/AVC muxing | ✅ Complete | `muxer/mp4.rs` |
| AAC audio (ADTS input) | ✅ Complete | `muxer/mp4.rs` |
| Fast-start (moov first) | ✅ Complete | `muxer/mp4.rs:finalize_fast_start()` |
| Metadata (udta box) | ✅ Complete | `api.rs:Metadata`, `muxer/mp4.rs` |
| B-frame support (ctts v1) | ✅ Complete | `muxer/mp4.rs`, `api.rs:write_video_with_dts()` |
| Fragmented MP4 | ✅ Complete | `fragmented.rs:FragmentedMuxer` |
| Builder pattern API | ✅ Complete | `api.rs:MuxerBuilder` |
| Statistics on finish | ✅ Complete | `api.rs:MuxerStats` |
| Zero dependencies | ✅ Complete | `Cargo.toml` |

### 2.3 Public API Surface

```rust
// Primary muxing API
pub struct MuxerBuilder<Writer> { ... }
pub struct Muxer<Writer> { ... }
pub struct MuxerConfig { ... }
pub struct MuxerStats { ... }
pub struct Metadata { ... }
pub enum VideoCodec { H264 }
pub enum AudioCodec { Aac, None }
pub enum MuxerError { ... }

// Fragmented MP4 API
pub struct FragmentedMuxer { ... }
pub struct FragmentConfig { ... }

// Key methods
impl MuxerBuilder {
    pub fn new(writer: Writer) -> Self;
    pub fn video(self, codec, width, height, framerate) -> Self;
    pub fn audio(self, codec, sample_rate, channels) -> Self;
    pub fn with_metadata(self, metadata: Metadata) -> Self;
    pub fn with_fast_start(self, enabled: bool) -> Self;
    pub fn build(self) -> Result<Muxer<Writer>, MuxerError>;
}

impl Muxer {
    pub fn write_video(&mut self, pts: u64, data: &[u8], is_keyframe: bool) -> Result<(), MuxerError>;
    pub fn write_video_with_dts(&mut self, pts: u64, dts: u64, data: &[u8], is_keyframe: bool) -> Result<(), MuxerError>;
    pub fn write_audio(&mut self, pts: u64, data: &[u8]) -> Result<(), MuxerError>;
    pub fn finish(self) -> Result<(), MuxerError>;
    pub fn finish_with_stats(self) -> Result<MuxerStats, MuxerError>;
}

impl FragmentedMuxer {
    pub fn new(config: FragmentConfig) -> Self;
    pub fn init_segment(&mut self) -> Vec<u8>;
    pub fn write_video(&mut self, pts: u64, dts: u64, data: &[u8], is_sync: bool);
    pub fn flush_segment(&mut self) -> Option<Vec<u8>>;
    pub fn ready_to_flush(&self) -> bool;
}
```

### 2.4 Test Coverage Summary

| Test File | Tests | Purpose |
|-----------|-------|---------|
| `api_test.rs` | 1 | Builder API validation |
| `audio_samples.rs` | 2 | AAC audio muxing |
| `basic_flow.rs` | 1 | End-to-end workflow |
| `bframe_ctts.rs` | 2 | B-frame decode/display order |
| `builder_test.rs` | 1 | Builder pattern |
| `error_test.rs` | 1 | Error handling |
| `finalize_test.rs` | 1 | File finalization |
| `fragmented.rs` | 3 | Fragmented MP4 (unit tests) |
| `keyframe_test.rs` | 1 | Keyframe detection |
| `metadata_fast_start.rs` | 3 | Metadata + fast-start |
| `minimal.rs` | 1 | Minimal valid MP4 |
| `multi_gop.rs` | 1 | Multiple GOPs |
| `single_frame.rs` | 1 | Single frame edge case |
| `video_samples.rs` | 1 | Video sample tables |
| `video_setup.rs` | 1 | Track setup |
| `write_test.rs` | 1 | Write operations |
| **Total** | **22** | |

### 2.5 Known Limitations

1. **Single video track only** - no multi-camera support
2. **H.264 only** - no HEVC, VP9, or AV1
3. **AAC only** - no Opus, MP3, or raw PCM
4. **No chapter markers** - udta exists but no chpl
5. **No subtitle support** - no TTML, WebVTT, or SRT
6. **No edit lists (elst)** - timestamps start at 0
7. **Limited metadata** - only title and creation_time
8. **No encryption** - no CENC/DRM support
9. **32-bit chunk offsets only** - no co64 for >4GB files

---

## 3. Competitive Landscape

### 3.1 Direct Competitors

#### `mp4` crate (alfg/mp4-rust)
- **Stars:** 341
- **Downloads:** 303K/month
- **Last update:** 2 years ago (stale)
- **Dependencies:** 6 (byteorder, bytes, serde, num-rational, thiserror, ...)
- **Size:** 310KB / 9K SLoC
- **License:** MIT

**Strengths:**
- Feature-rich (read + write)
- Wide codec support
- Used by 1000+ projects

**Weaknesses:**
- Not maintained (23 open PRs)
- No fast-start support
- Complex API requiring ISO-BMFF knowledge
- Heavy dependencies

#### `mp4e` crate (xjunl22/mp4e)
- **Stars:** 2
- **Downloads:** 2.5K total
- **Last update:** 2 months ago
- **Dependencies:** 0
- **Size:** 78KB
- **License:** MIT

**Strengths:**
- Zero dependencies (like muxide)
- H.265 support
- Fragmented MP4

**Weaknesses:**
- docs.rs build is broken
- No fast-start
- No metadata support
- Duration-based API (not PTS)
- No builder pattern
- Minimal documentation

#### `minimp4` crate
- **Stars:** Unknown
- **Downloads:** 8K total
- **Last update:** 1 year ago
- **Dependencies:** C FFI binding
- **License:** Unknown

**Strengths:**
- Based on proven C library

**Weaknesses:**
- Not pure Rust (FFI)
- Build complexity
- Unsafe code

#### `async-mp4` crate
- **Downloads:** 3K total
- **Last update:** 3 years ago (abandoned)

**Status:** Not viable - abandoned, async complexity

### 3.2 Indirect Competitors

| Solution | Type | Pros | Cons |
|----------|------|------|------|
| FFmpeg (via ffmpeg-next) | FFI | Everything | License (GPL), binary deps |
| GStreamer (via gstreamer-rs) | FFI | Professional | Huge, complex |
| libav bindings | FFI | Mature | Unsafe, deps |

### 3.3 Competitive Matrix

| Feature | `mp4` | `mp4e` | `minimp4` | **muxide** |
|---------|-------|--------|-----------|------------|
| Pure Rust | ✅ | ✅ | ❌ | ✅ |
| Zero deps | ❌ | ✅ | ❌ | ✅ |
| Maintained | ❌ | 🟡 | ❌ | ✅ |
| Working docs | ✅ | ❌ | 🟡 | 🟡 |
| Fast-start | ❌ | ❌ | ❓ | ✅ |
| B-frames | 🟡 | ❓ | ✅ | ✅ |
| fMP4 | 🟡 | ✅ | ❌ | ✅ |
| Metadata | ❌ | ❌ | ❓ | ✅ |
| H.264 | ✅ | ✅ | ✅ | ✅ |
| H.265 | ✅ | ✅ | ✅ | ❌ |
| VP9 | ✅ | ❌ | ❌ | ❌ |
| AV1 | ❌ | ❌ | ❌ | ❌ |
| AAC | ✅ | ✅ | ✅ | ✅ |
| Opus | ❌ | ✅ | ❌ | ❌ |
| Builder API | ❌ | ❌ | ❌ | ✅ |
| Stats | ❌ | ❌ | ❌ | ✅ |
| Multi-track | ✅ | ❌ | ❓ | ❌ |
| Chapters | ❌ | ❌ | ❌ | ❌ |
| Subtitles | ❌ | ❌ | ❌ | ❌ |

---

## 4. Feature Parity Matrix

### 4.1 Video Codec Support

| Codec | ISO Box | Complexity | Priority | Points |
|-------|---------|------------|----------|--------|
| H.264/AVC | avc1, avcC | ✅ Done | - | - |
| H.265/HEVC | hvc1, hvcC | High | P0 | 8 |
| VP9 | vp09, vpcC | Medium | P1 | 5 |
| AV1 | av01, av1C | High | P1 | 8 |
| VP8 | vp08 | Low | P2 | 3 |
| MPEG-4 Part 2 | mp4v | Low | P3 | 3 |

**H.265/HEVC Details:**
- NAL unit types differ from H.264
- VPS (Video Parameter Set) in addition to SPS/PPS
- hvcC box structure different from avcC
- Need to parse NAL headers for VPS/SPS/PPS extraction
- Reference: ISO/IEC 14496-15 Section 8

**VP9 Details:**
- Simpler than HEVC - no parameter sets
- vpcC box contains profile, level, color info
- Reference: VP9 Bitstream Specification

**AV1 Details:**
- OBU (Open Bitstream Unit) parsing
- Sequence Header OBU extraction for av1C
- Complex configOBU structure
- Reference: AV1 Codec ISO Media File Format Binding

### 4.2 Audio Codec Support

| Codec | ISO Box | Complexity | Priority | Points |
|-------|---------|------------|----------|--------|
| AAC (ADTS) | mp4a, esds | ✅ Done | - | - |
| Opus | Opus, dOps | Medium | P0 | 3 |
| MP3 | mp4a | Low | P2 | 2 |
| FLAC | fLaC, dfLa | Medium | P2 | 3 |
| AC-3 | ac-3, dac3 | Medium | P3 | 3 |
| E-AC-3 | ec-3, dec3 | Medium | P3 | 3 |
| PCM | sowt/twos | Low | P2 | 2 |

**Opus Details:**
- dOps (Opus Specific Box) required
- Pre-skip and output gain handling
- Channel mapping family
- Reference: Opus in ISOBMFF

### 4.3 Container Features

| Feature | ISO Box | Complexity | Priority | Points |
|---------|---------|------------|----------|--------|
| Fast-start (moov first) | - | ✅ Done | - | - |
| Fragmented MP4 | moof, mdat | ✅ Done | - | - |
| Metadata (udta) | udta | ✅ Done | - | - |
| Chapter markers | chpl | Low | P0 | 2 |
| Edit lists | elst | Medium | P1 | 3 |
| Multi-track video | trak | Medium | P1 | 5 |
| Multi-track audio | trak | Low | P1 | 2 |
| Subtitle tracks | tx3g/stpp | Medium | P2 | 3 |
| 64-bit offsets | co64 | Low | P1 | 2 |
| Extended metadata | ilst, meta | Medium | P2 | 3 |
| Color information | colr | Low | P1 | 1 |
| HDR metadata | mdcv, clli | Medium | P2 | 3 |
| Sample groups | sbgp, sgpd | High | P3 | 5 |

**Chapter Markers Details:**
- chpl box inside udta
- Array of (timestamp, title) pairs
- Timescale conversion required
- Most players support this

**Edit Lists Details:**
- elst box for timeline manipulation
- Handle initial delays, A/V sync
- Empty edits for proper timestamps

**64-bit Offsets Details:**
- Required for files >4GB
- Replace stco with co64
- Check total mdat size before finalizing

### 4.4 Streaming Features

| Feature | Complexity | Priority | Points |
|---------|------------|----------|--------|
| fMP4 init segment | ✅ Done | - | - |
| fMP4 media segments | ✅ Done | - | - |
| DASH manifest generation | Medium | P2 | 3 |
| HLS playlist generation | Medium | P2 | 3 |
| CMAF compliance | Medium | P2 | 3 |
| Low-latency CMAF | High | P3 | 5 |
| LL-HLS support | High | P3 | 5 |

### 4.5 Quality & Polish

| Feature | Complexity | Priority | Points |
|---------|------------|----------|--------|
| 100+ tests | Medium | P0 | 3 |
| Property-based tests | Medium | P1 | 2 |
| Fuzz testing | Medium | P1 | 2 |
| docs.rs documentation | Low | P0 | 2 |
| Usage examples | Low | P0 | 1 |
| Error message quality | Low | P0 | 1 |
| Performance benchmarks | Medium | P1 | 2 |
| Memory usage profiling | Medium | P2 | 2 |

---

## 5. Detailed Feature Specifications

### 5.1 H.265/HEVC Support (8 points)

**Objective:** Accept H.265 Annex B bitstream and produce valid MP4 with hvc1/hvcC boxes.

**Input Format:**
- Annex B byte stream (0x00 0x00 0x00 0x01 or 0x00 0x00 0x01 start codes)
- NAL unit types: VPS (32), SPS (33), PPS (34), IDR (19, 20), non-IDR (1)

**Required Boxes:**
```
moov
└── trak
    └── mdia
        └── minf
            └── stbl
                └── stsd
                    └── hvc1 (or hev1)
                        └── hvcC (HEVC Configuration Box)
```

**hvcC Structure:**
```
configurationVersion = 1
general_profile_space (2 bits)
general_tier_flag (1 bit)
general_profile_idc (5 bits)
general_profile_compatibility_flags (32 bits)
general_constraint_indicator_flags (48 bits)
general_level_idc (8 bits)
min_spatial_segmentation_idc (12 bits)
parallelismType (2 bits)
chroma_format_idc (2 bits)
bit_depth_luma_minus8 (3 bits)
bit_depth_chroma_minus8 (3 bits)
avgFrameRate (16 bits)
constantFrameRate (2 bits)
numTemporalLayers (3 bits)
temporalIdNested (1 bit)
lengthSizeMinusOne (2 bits)
numOfArrays (8 bits)
[arrays of VPS, SPS, PPS]
```

**API Changes:**
```rust
pub enum VideoCodec {
    H264,
    H265,  // NEW
}
```

**Implementation Tasks:**
1. NAL unit type detection for HEVC (different from H.264)
2. VPS/SPS/PPS extraction from bitstream
3. hvcC box construction
4. hvc1 sample entry box
5. Integration with existing Muxer
6. Tests with real HEVC bitstreams

**Test Cases:**
- Single HEVC I-frame
- HEVC GOP with B-frames
- HEVC with HDR metadata
- Invalid NAL handling

**References:**
- ISO/IEC 14496-15 Section 8
- ITU-T H.265 / ISO/IEC 23008-2

---

### 5.2 Opus Audio Support (3 points)

**Objective:** Accept Opus packets and produce valid MP4 with Opus/dOps boxes.

**Input Format:**
- Raw Opus packets (no container framing)
- Sample rate: typically 48000 Hz
- Channels: 1-8

**Required Boxes:**
```
moov
└── trak
    └── mdia
        └── minf
            └── stbl
                └── stsd
                    └── Opus
                        └── dOps (Opus Specific Box)
```

**dOps Structure:**
```
Version (8 bits) = 0
OutputChannelCount (8 bits)
PreSkip (16 bits) - samples to discard
InputSampleRate (32 bits) - original sample rate
OutputGain (16 bits, signed) - dB * 256
ChannelMappingFamily (8 bits)
[if ChannelMappingFamily != 0]
    StreamCount (8 bits)
    CoupledCount (8 bits)
    ChannelMapping[OutputChannelCount] (8 bits each)
```

**API Changes:**
```rust
pub enum AudioCodec {
    Aac,
    Opus,  // NEW
    None,
}
```

**Implementation Tasks:**
1. Opus sample entry box construction
2. dOps box construction
3. Proper sample duration handling (Opus uses variable frame sizes)
4. PreSkip handling for gapless playback
5. Integration with existing Muxer

**Test Cases:**
- Mono Opus stream
- Stereo Opus stream
- Opus with music (longer frames)
- Opus with speech (shorter frames)

**References:**
- RFC 7845 (Opus in Ogg)
- Opus in ISOBMFF specification

---

### 5.3 Chapter Markers (2 points)

**Objective:** Allow users to add named chapter markers at specific timestamps.

**Required Box:**
```
moov
└── udta
    └── chpl (Chapter List Box)
```

**chpl Structure:**
```
version (8 bits) = 0
flags (24 bits) = 0
reserved (32 bits) = 0
chapter_count (8 bits)
[for each chapter]
    timestamp (64 bits) - in 100ns units (!)
    title_length (8 bits)
    title (UTF-8 string)
```

**Note:** chpl uses 100-nanosecond units, not the movie timescale!

**API Changes:**
```rust
pub struct Chapter {
    /// Timestamp in milliseconds from start
    pub timestamp_ms: u64,
    /// Chapter title (UTF-8)
    pub title: String,
}

impl MuxerBuilder {
    pub fn with_chapters(self, chapters: Vec<Chapter>) -> Self;
}

// Or runtime addition:
impl Muxer {
    pub fn add_chapter(&mut self, timestamp_ms: u64, title: &str);
}
```

**Implementation Tasks:**
1. Chapter storage in Muxer state
2. chpl box construction
3. Timestamp conversion (ms → 100ns)
4. UTF-8 title encoding
5. Integration with udta box

**Test Cases:**
- Single chapter
- Multiple chapters
- Unicode chapter titles
- Chapter at timestamp 0

---

### 5.4 VP9 Support (5 points)

**Objective:** Accept VP9 bitstream and produce valid MP4 with vp09/vpcC boxes.

**Input Format:**
- VP9 superframes or single frames
- No start codes (unlike H.264/H.265)

**Required Boxes:**
```
moov
└── trak
    └── mdia
        └── minf
            └── stbl
                └── stsd
                    └── vp09
                        └── vpcC (VP9 Configuration Box)
```

**vpcC Structure:**
```
version (8 bits) = 1
flags (24 bits) = 0
profile (8 bits)
level (8 bits)
bitDepth (4 bits)
chromaSubsampling (3 bits)
videoFullRangeFlag (1 bit)
colourPrimaries (8 bits)
transferCharacteristics (8 bits)
matrixCoefficients (8 bits)
codecInitializationDataSize (16 bits)
codecInitializationData[]
```

**API Changes:**
```rust
pub enum VideoCodec {
    H264,
    H265,
    Vp9,  // NEW
}
```

**Implementation Tasks:**
1. VP9 frame header parsing (for keyframe detection)
2. vpcC box construction from first keyframe
3. vp09 sample entry
4. Superframe handling
5. Profile/level detection

**Test Cases:**
- VP9 Profile 0 (8-bit)
- VP9 Profile 2 (10-bit)
- VP9 with alpha channel (Profile 1)

**References:**
- VP9 Bitstream Specification
- VP Codec ISO Media File Format Binding

---

### 5.5 AV1 Support (8 points)

**Objective:** Accept AV1 OBU stream and produce valid MP4 with av01/av1C boxes.

**Input Format:**
- Low-overhead bitstream format (OBUs with size fields)
- OBU types: Sequence Header (1), Frame Header (3), Frame (6), etc.

**Required Boxes:**
```
moov
└── trak
    └── mdia
        └── minf
            └── stbl
                └── stsd
                    └── av01
                        └── av1C (AV1 Configuration Box)
```

**av1C Structure:**
```
marker (1 bit) = 1
version (7 bits) = 1
seq_profile (3 bits)
seq_level_idx_0 (5 bits)
seq_tier_0 (1 bit)
high_bitdepth (1 bit)
twelve_bit (1 bit)
monochrome (1 bit)
chroma_subsampling_x (1 bit)
chroma_subsampling_y (1 bit)
chroma_sample_position (2 bits)
reserved (3 bits) = 0
initial_presentation_delay_present (1 bit)
initial_presentation_delay_minus_one (4 bits) [if present]
configOBUs[] - Sequence Header OBU
```

**API Changes:**
```rust
pub enum VideoCodec {
    H264,
    H265,
    Vp9,
    Av1,  // NEW
}
```

**Implementation Tasks:**
1. OBU parsing (type, size, data)
2. Sequence Header OBU extraction
3. av1C box construction
4. av01 sample entry
5. Temporal unit to sample conversion
6. Keyframe detection from frame header

**Test Cases:**
- AV1 Main Profile
- AV1 High Profile (10-bit)
- AV1 with film grain
- Various tile configurations

**References:**
- AV1 Bitstream Specification
- AV1 Codec ISO Media File Format Binding

---

### 5.6 64-bit Chunk Offsets (2 points)

**Objective:** Support files larger than 4GB by using co64 instead of stco.

**Current State:**
- stco box uses 32-bit offsets (max ~4GB)
- Files >4GB will have incorrect offsets

**Required Changes:**
- Detect when mdat size exceeds 32-bit range
- Use co64 box instead of stco
- May need to use 64-bit box size for mdat

**API Changes:**
None required - automatic detection.

**Implementation Tasks:**
1. Track cumulative mdat size during writing
2. Decision logic: stco vs co64
3. co64 box construction
4. 64-bit mdat box size handling
5. Update fast-start logic for co64

**Test Cases:**
- Generate 4.5GB synthetic file (sparse write)
- Verify co64 offsets are correct

---

### 5.7 Edit Lists (3 points)

**Objective:** Support edit lists for proper A/V synchronization and timeline manipulation.

**Use Cases:**
- Initial audio delay (audio starts before video)
- Proper timestamp handling (first frame not at 0)
- Seeking accuracy

**Required Box:**
```
moov
└── trak
    └── edts
        └── elst (Edit List Box)
```

**elst Structure:**
```
version (8 bits) - 0 or 1
flags (24 bits) = 0
entry_count (32 bits)
[for each entry]
    segment_duration (32 or 64 bits)
    media_time (32 or 64 bits, signed)
    media_rate_integer (16 bits)
    media_rate_fraction (16 bits)
```

**API Changes:**
```rust
pub struct EditEntry {
    /// Duration of this edit segment in movie timescale
    pub segment_duration: u64,
    /// Start time in media timescale (-1 for empty edit)
    pub media_time: i64,
    /// Playback rate (1.0 = normal)
    pub rate: f32,
}

impl MuxerBuilder {
    pub fn with_edit_list(self, edits: Vec<EditEntry>) -> Self;
}
```

**Implementation Tasks:**
1. Edit list storage in Muxer
2. elst box construction
3. Version selection (0 vs 1 based on values)
4. Integration with track finalization

**Test Cases:**
- Empty edit (delay start)
- Single edit (normal playback)
- Multiple edits

---

### 5.8 Multi-Track Video (5 points)

**Objective:** Support multiple video tracks (e.g., multi-camera recording).

**Use Cases:**
- Multi-camera sync recording
- Picture-in-picture
- Main + alternate angles

**Required Changes:**
- Track ID management
- Multiple trak boxes in moov
- Cross-track timing synchronization
- Sample interleaving in mdat

**API Changes:**
```rust
pub struct TrackId(u32);

impl MuxerBuilder {
    pub fn add_video_track(self, codec, width, height, framerate) -> (Self, TrackId);
}

impl Muxer {
    pub fn write_video_to_track(&mut self, track: TrackId, pts, data, is_keyframe) -> Result<...>;
}
```

**Implementation Tasks:**
1. Track ID allocation
2. Multiple video track state
3. Interleaved sample writing
4. Proper stco/stsc for each track
5. Track reference boxes (if needed)

**Test Cases:**
- Two 1080p tracks
- Mixed resolution tracks
- Track sync verification

---

### 5.9 Comprehensive Test Suite (3 points)

**Objective:** Expand from 22 tests to 100+ tests with high confidence.

**Test Categories:**

| Category | Current | Target | New Tests Needed |
|----------|---------|--------|------------------|
| Unit tests | 3 | 30 | 27 |
| Integration tests | 19 | 40 | 21 |
| Property-based tests | 0 | 20 | 20 |
| Fuzz tests | 0 | 10 | 10 |
| Compatibility tests | 0 | 10 | 10 |
| **Total** | **22** | **110** | **88** |

**Property-Based Test Ideas:**
- Any valid H.264 NAL sequence produces valid MP4
- PTS values always increase in output
- Box sizes are always correct
- stco offsets match actual mdat positions

**Fuzz Test Ideas:**
- Random NAL data → no panic
- Invalid ADTS → graceful error
- Extreme timestamps → correct handling
- Zero-length frames → handled

**Compatibility Tests:**
- Output plays in FFmpeg
- Output plays in VLC
- Output plays in Chrome
- Output plays in Safari
- Mediainfo parses correctly

**Implementation Tasks:**
1. Add proptest dependency (dev only)
2. Create property test generators
3. Set up cargo-fuzz
4. Create fuzz targets
5. Add CI for fuzz testing

---

### 5.10 Documentation & Examples (3 points)

**Objective:** Production-quality documentation for docs.rs.

**Documentation Requirements:**

| Component | Status | Required |
|-----------|--------|----------|
| Crate-level docs | 🟡 Basic | Full tutorial |
| MuxerBuilder docs | 🟡 Basic | All methods documented |
| Muxer docs | 🟡 Basic | All methods with examples |
| Error enum docs | ❌ Missing | Each variant explained |
| Feature flags docs | N/A | No features yet |
| README.md | 🟡 Basic | Complete with badges |

**Example Files Needed:**
1. `examples/simple.rs` - Basic H.264 + AAC muxing
2. `examples/fast_start.rs` - Web-optimized output
3. `examples/metadata.rs` - Adding title and chapters
4. `examples/fragmented.rs` - DASH/HLS streaming
5. `examples/bframes.rs` - B-frame handling

**Implementation Tasks:**
1. Write crate-level documentation with tutorial
2. Document all public items
3. Add doc tests that compile and run
4. Create example files
5. Add README badges (crates.io, docs.rs, CI)

---

## 6. Testing Strategy

### 6.1 Test Pyramid

```
        /\
       /  \     Compatibility (10) - Real player validation
      /----\
     /      \   Integration (40) - Full muxing workflows
    /--------\
   /          \ Property (20) - Invariant checking
  /------------\
 /              \ Fuzz (10) - Crash prevention
/----------------\
       Unit (30)  - Individual function testing
```

### 6.2 Test Infrastructure

**Required Tooling:**
- `proptest` - Property-based testing
- `cargo-fuzz` / `libfuzzer` - Fuzz testing
- FFmpeg/FFprobe - Output validation
- Mediainfo - Metadata verification

**CI Integration:**
```yaml
# Suggested GitHub Actions workflow
jobs:
  test:
    - cargo test
    - cargo test --all-features
  fuzz:
    - cargo fuzz run fuzz_h264 -- -max_total_time=60
  compatibility:
    - ffprobe output.mp4
    - mediainfo output.mp4
```

### 6.3 Test Data

**Required Test Assets:**
- Minimal H.264 (single I-frame)
- H.264 with B-frames
- Various GOP structures
- AAC mono, stereo, 5.1
- Edge cases (empty, huge, etc.)

**Synthetic Test Data:**
- Generated NAL units
- Generated ADTS frames
- Timestamp edge cases

---

## 7. Documentation Requirements

### 7.1 README Structure

```markdown
# muxide

Zero-dependency pure-Rust MP4 muxer for recording applications.

[badges: crates.io, docs.rs, CI, license]

## Features
- ✅ H.264/AVC video
- ✅ AAC audio  
- ✅ Fast-start (moov first)
- ✅ Fragmented MP4 for streaming
- ✅ B-frame support
- ✅ Metadata (title, chapters)
- 🚧 H.265/HEVC (coming soon)

## Quick Start
[code example]

## Why muxide?
[comparison table]

## Documentation
[link to docs.rs]

## License
MIT OR Apache-2.0
```

### 7.2 API Documentation Standards

Every public item must have:
1. One-line summary
2. Extended description (if complex)
3. Example code (where appropriate)
4. Panics section (if applicable)
5. Errors section (for Result-returning functions)
6. See Also links

---

## 8. Risk Assessment

### 8.1 Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| HEVC parsing complexity | Medium | High | Start with simple profiles |
| AV1 OBU complexity | High | High | Reference implementation study |
| 64-bit file testing | Medium | Medium | Sparse file generation |
| Browser compatibility | Low | High | Test matrix across browsers |
| Performance regression | Low | Medium | Benchmark suite |

### 8.2 Ecosystem Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `mp4` crate revives | Low | Medium | Differentiate on API/features |
| New competitor appears | Medium | Low | Move fast, ship often |
| Breaking spec changes | Very Low | High | Pin to ISO spec versions |

### 8.3 Resource Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Scope creep | High | Medium | Strict prioritization |
| Burnout | Medium | High | Sustainable pace |
| Dependency on single dev | High | High | Documentation |

---

## 9. Prioritization Request

### 9.1 Questions for Reviewer

1. **Feature Priority:** Given the competitive landscape, which features provide the highest differentiation-to-effort ratio?

2. **Test Coverage:** Is 100 tests sufficient for production confidence, or should we target higher?

3. **Codec Priority:** Should we prioritize H.265 (most common) or AV1 (future-proof)?

4. **API Design:** Is the current builder pattern optimal, or would a different approach (e.g., typestate) be better for preventing misuse?

5. **Streaming Focus:** Should we invest more in DASH/HLS manifest generation, or leave that to consumers?

6. **Breaking Changes:** Should we design the API now for multi-track support even if we don't implement it immediately?

### 9.2 Suggested Prioritization Framework

**P0 - Ship Blockers:**
- Features without which we should not publish
- Test coverage minimum
- Documentation minimum

**P1 - Differentiation:**
- Features that make us clearly better than alternatives
- Performance benchmarks
- Compatibility guarantees

**P2 - Parity:**
- Features competitors have that we lack
- Nice-to-have quality improvements

**P3 - Future:**
- Features nobody has yet
- Speculative additions

### 9.3 Proposed Priority Assignment

| Priority | Features | Total Points |
|----------|----------|--------------|
| P0 | Tests (3), Docs (2), Chapters (2), Errors (1) | 8 |
| P1 | HEVC (8), Opus (3), 64-bit (2), Edit lists (3), Color info (1) | 17 |
| P2 | VP9 (5), Multi-track (5), Subtitles (3), Extended meta (3) | 16 |
| P3 | AV1 (8), DASH/HLS (6), LL-CMAF (5), Sample groups (5) | 24 |

**Cumulative:**
- P0 complete: 8 points → **Ship-ready**
- P0+P1 complete: 25 points → **Competitive**
- P0+P1+P2 complete: 41 points → **Dominant**
- All complete: 65 points → **Complete solution**

---

## 10. Success Criteria

### 10.1 Ship-Ready (P0 Complete)

- [ ] 100+ tests passing
- [ ] All public items documented
- [ ] 5 example files
- [ ] Chapter markers working
- [ ] All error variants have helpful messages
- [ ] README complete with badges
- [ ] CI green on all platforms
- [ ] No clippy warnings
- [ ] Cargo fmt clean

### 10.2 Launch Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| crates.io downloads (week 1) | 500 | crates.io stats |
| GitHub stars (month 1) | 50 | GitHub |
| Issues filed | <10 bugs | GitHub issues |
| Documentation complaints | 0 | GitHub issues |

### 10.3 Long-Term Success

| Metric | 3-Month Target | 12-Month Target |
|--------|----------------|-----------------|
| Downloads/month | 5,000 | 50,000 |
| GitHub stars | 200 | 500 |
| Dependent crates | 10 | 50 |
| Contributors | 3 | 10 |

---

## Appendix A: ISO Box Reference

### A.1 Required Boxes (Current Implementation)

| Box | Parent | Purpose |
|-----|--------|---------|
| ftyp | root | File type and compatibility |
| moov | root | Movie metadata container |
| mvhd | moov | Movie header (timescale, duration) |
| trak | moov | Track container |
| tkhd | trak | Track header |
| mdia | trak | Media container |
| mdhd | mdia | Media header |
| hdlr | mdia | Handler reference |
| minf | mdia | Media information |
| vmhd | minf | Video media header |
| smhd | minf | Sound media header |
| dinf | minf | Data information |
| dref | dinf | Data reference |
| stbl | minf | Sample table |
| stsd | stbl | Sample description |
| stts | stbl | Time to sample |
| stsc | stbl | Sample to chunk |
| stsz | stbl | Sample sizes |
| stco | stbl | Chunk offsets |
| stss | stbl | Sync samples (keyframes) |
| ctts | stbl | Composition time offset |
| mdat | root | Media data |
| udta | moov | User data |

### A.2 Additional Boxes (Parity Features)

| Box | Parent | Purpose | Feature |
|-----|--------|---------|---------|
| co64 | stbl | 64-bit chunk offsets | Large files |
| edts | trak | Edit container | Edit lists |
| elst | edts | Edit list | Edit lists |
| chpl | udta | Chapter list | Chapters |
| colr | stsd child | Color information | Color info |
| meta | moov/trak | Metadata container | Extended meta |
| ilst | meta | iTunes metadata list | Extended meta |

---

## Appendix B: NAL Unit Reference

### B.1 H.264 NAL Types

| Type | Name | Keyframe |
|------|------|----------|
| 1 | Non-IDR slice | No |
| 5 | IDR slice | Yes |
| 6 | SEI | - |
| 7 | SPS | - |
| 8 | PPS | - |
| 9 | AUD | - |

### B.2 H.265 NAL Types

| Type | Name | Keyframe |
|------|------|----------|
| 0-9 | Trailing pictures | No |
| 16-21 | Leading pictures | No |
| 19 | IDR_W_RADL | Yes |
| 20 | IDR_N_LP | Yes |
| 21 | CRA | Yes |
| 32 | VPS | - |
| 33 | SPS | - |
| 34 | PPS | - |

---

## Appendix C: Glossary

| Term | Definition |
|------|------------|
| **Annex B** | H.264/H.265 byte stream format with start codes |
| **AVCC** | H.264 format with length-prefixed NAL units |
| **ctts** | Composition Time to Sample box (B-frame timing) |
| **DTS** | Decode Timestamp (order frames are decoded) |
| **fMP4** | Fragmented MP4 (moof+mdat segments) |
| **GOP** | Group of Pictures (I-frame to next I-frame) |
| **ISOBMFF** | ISO Base Media File Format (MP4 foundation) |
| **mdat** | Media Data box (actual video/audio bytes) |
| **moov** | Movie box (all metadata) |
| **NAL** | Network Abstraction Layer (H.264/H.265 unit) |
| **OBU** | Open Bitstream Unit (AV1 unit) |
| **PTS** | Presentation Timestamp (order frames are shown) |
| **stco** | Sample Table Chunk Offset |

---

## Appendix D: Reference Specifications

1. **ISO/IEC 14496-12** - ISO Base Media File Format
2. **ISO/IEC 14496-14** - MP4 File Format
3. **ISO/IEC 14496-15** - Carriage of NAL unit structured video
4. **ITU-T H.264** - AVC Video Coding
5. **ITU-T H.265** - HEVC Video Coding
6. **RFC 6716** - Opus Audio Codec
7. **VP9 Bitstream Specification** - Google
8. **AV1 Bitstream Specification** - AOMedia

---

*End of document. Ready for third-party audit and prioritization.*
