# Muxide v0.1.0 Final Feature Slices

These 5 slices complete v0.1.0 before public release. Each is fully specified for rapid implementation.

---

## Slice A: `audio_frames` in MuxerStats

**Goal:** Add audio frame count to stats for completeness.

**Files:**
- `src/api.rs`

**Changes:**

```rust
// In MuxerStats struct, add:
pub audio_frames: u64,

// In finish_in_place_with_stats(), add:
let audio_frames = self.writer.audio_sample_count();

// In MuxerStats construction:
Ok(MuxerStats {
    video_frames,
    audio_frames,  // NEW
    duration_secs,
    bytes_written,
})
```

**In mp4.rs, add:**
```rust
pub(crate) fn audio_sample_count(&self) -> u64 {
    self.audio_samples.len() as u64
}
```

**Test:** Update `tests/stats.rs` to verify audio frame count.

**Acceptance:** `cargo test --test stats` passes with audio frame verification.

---

## Slice B: Metadata Box (`udta`)

**Goal:** Support optional title and creation time in MP4 metadata.

**API Changes (api.rs):**

```rust
// Add to MuxerConfig:
pub struct MuxerConfig {
    pub width: u32,
    pub height: u32,
    pub framerate: f64,
    pub audio: Option<AudioTrackConfig>,
    pub metadata: Option<Metadata>,  // NEW
}

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub creation_time: Option<u64>,  // Seconds since 1904-01-01 (MP4 epoch)
}

impl MuxerConfig {
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}
```

**MP4 Box Structure:**
```
moov
├── mvhd
├── trak (video)
├── trak (audio, optional)
└── udta           <-- NEW
    └── meta
        ├── hdlr
        └── ilst
            ├── ©nam (title)
            └── ©day (creation date)
```

**Implementation (mp4.rs):**

```rust
fn build_udta_box(metadata: &Metadata) -> Vec<u8> {
    let mut ilst_payload = Vec::new();
    
    if let Some(title) = &metadata.title {
        ilst_payload.extend_from_slice(&build_ilst_string_item(b"\xa9nam", title));
    }
    
    if let Some(creation_time) = metadata.creation_time {
        // Format as ISO 8601: "2025-12-15T12:00:00Z"
        let date_str = format_mp4_date(creation_time);
        ilst_payload.extend_from_slice(&build_ilst_string_item(b"\xa9day", &date_str));
    }
    
    if ilst_payload.is_empty() {
        return Vec::new();  // No metadata, skip udta entirely
    }
    
    let ilst_box = build_box(b"ilst", &ilst_payload);
    
    // meta box requires hdlr
    let hdlr_payload = build_meta_hdlr();
    let hdlr_box = build_box(b"hdlr", &hdlr_payload);
    
    // meta is a full box (version + flags)
    let mut meta_payload = vec![0u8; 4];  // version=0, flags=0
    meta_payload.extend_from_slice(&hdlr_box);
    meta_payload.extend_from_slice(&ilst_box);
    let meta_box = build_box(b"meta", &meta_payload);
    
    build_box(b"udta", &meta_box)
}

fn build_ilst_string_item(atom_type: &[u8; 4], value: &str) -> Vec<u8> {
    // data box: type indicator (1 = UTF-8) + locale (0) + string
    let mut data_payload = Vec::new();
    data_payload.extend_from_slice(&[0, 0, 0, 1]);  // type = UTF-8
    data_payload.extend_from_slice(&[0, 0, 0, 0]);  // locale = 0
    data_payload.extend_from_slice(value.as_bytes());
    
    let data_box = build_box(b"data", &data_payload);
    build_box(atom_type, &data_box)
}

fn build_meta_hdlr() -> Vec<u8> {
    let mut payload = vec![0u8; 4];  // version + flags
    payload.extend_from_slice(&[0, 0, 0, 0]);  // pre_defined
    payload.extend_from_slice(b"mdir");  // handler_type
    payload.extend_from_slice(b"appl");  // manufacturer
    payload.extend_from_slice(&[0, 0, 0, 0]);  // reserved
    payload.extend_from_slice(&[0, 0, 0, 0]);  // reserved
    payload.push(0);  // name (empty, null-terminated)
    payload
}
```

**Test:** New `tests/metadata.rs` verifying udta box presence.

**Acceptance:** Files with metadata play and show title in media players.

---

## Slice C: Fast-Start (`moov` Before `mdat`)

**Goal:** Write `moov` box before `mdat` so web players can start immediately.

**Current structure:**
```
ftyp | mdat (samples) | moov (metadata)
```

**Fast-start structure:**
```
ftyp | moov (metadata) | mdat (samples)
```

**Problem:** We don't know `mdat` size or sample offsets until all frames written.

**Solution:** Two-pass write with seekable writer OR buffer all samples in memory (current approach already buffers).

**Implementation Strategy:**

Since we already buffer all samples in `video_samples` and `audio_samples` vectors, fast-start is actually simple:

1. Calculate total `mdat` size from buffered samples
2. Calculate `moov` box size (build it first)
3. Write: `ftyp` → `moov` → `mdat`
4. Chunk offsets = `ftyp_size + moov_size + 8` (mdat header)

**API Changes:**

```rust
// MuxerConfig addition:
pub struct MuxerConfig {
    // ... existing fields ...
    pub fast_start: bool,  // NEW - default true
}

impl MuxerConfig {
    pub fn new(width: u32, height: u32, framerate: f64) -> Self {
        Self {
            width,
            height,
            framerate,
            audio: None,
            metadata: None,
            fast_start: true,  // Default ON for web compatibility
        }
    }
    
    pub fn with_fast_start(mut self, enabled: bool) -> Self {
        self.fast_start = enabled;
        self
    }
}
```

**Changes to finalize() in mp4.rs:**

```rust
pub fn finalize(&mut self, video: &Mp4VideoTrack, fast_start: bool) -> io::Result<()> {
    // ... existing validation ...
    
    if fast_start {
        self.finalize_fast_start(video)
    } else {
        self.finalize_standard(video)  // Current implementation
    }
}

fn finalize_fast_start(&mut self, video: &Mp4VideoTrack) -> io::Result<()> {
    let ftyp_box = build_ftyp_box();
    let ftyp_len = ftyp_box.len() as u32;
    
    // Calculate mdat payload size
    let mut mdat_payload_size: u32 = 0;
    for sample in &self.video_samples {
        mdat_payload_size += sample.data.len() as u32;
    }
    for sample in &self.audio_samples {
        mdat_payload_size += sample.data.len() as u32;
    }
    let mdat_header_size = 8u32;
    let mdat_total_size = mdat_header_size + mdat_payload_size;
    
    // Build moov with placeholder offsets, measure size
    let placeholder_moov = self.build_moov_for_fast_start(video, ftyp_len, 0)?;
    let moov_len = placeholder_moov.len() as u32;
    
    // Now we know: chunk offsets start at ftyp_len + moov_len + 8
    let mdat_data_start = ftyp_len + moov_len + mdat_header_size;
    let final_moov = self.build_moov_for_fast_start(video, ftyp_len, moov_len)?;
    
    // Write: ftyp → moov → mdat header → samples
    Self::write_counted(&mut self.writer, &mut self.bytes_written, &ftyp_box)?;
    Self::write_counted(&mut self.writer, &mut self.bytes_written, &final_moov)?;
    Self::write_counted(&mut self.writer, &mut self.bytes_written, &mdat_total_size.to_be_bytes())?;
    Self::write_counted(&mut self.writer, &mut self.bytes_written, b"mdat")?;
    
    // Write interleaved samples (same logic as current)
    self.write_interleaved_samples()?;
    
    Ok(())
}
```

**Test:** New `tests/fast_start.rs`:
- Verify moov appears before mdat in output
- Verify file plays in browser (manual) or structural check

**Acceptance:** 
- `cargo test --test fast_start` passes
- Output plays instantly in Chrome without buffering entire file

---

## Slice D: B-Frame Support (`ctts` Box)

**Goal:** Support B-frames where PTS ≠ DTS via composition time offset table.

**Background:**
- **PTS (Presentation Time):** When frame should display
- **DTS (Decode Time):** When frame should decode
- For B-frames: DTS < PTS (decode before display)
- `ctts` box stores: `composition_offset = PTS - DTS`

**API Changes:**

```rust
// Enhanced write_video signature (non-breaking, dts is optional):
pub fn write_video_with_dts(
    &mut self,
    pts: f64,
    dts: Option<f64>,  // None = assume PTS == DTS (no B-frames)
    data: &[u8],
    is_keyframe: bool,
) -> Result<(), MuxerError>
```

**SampleInfo changes:**

```rust
struct SampleInfo {
    pts: u64,
    dts: Option<u64>,  // NEW
    data: Vec<u8>,
    is_keyframe: bool,
    duration: Option<u32>,
}
```

**ctts box structure:**
```
ctts (full box, version 0 or 1)
├── entry_count: u32
└── entries[]:
    ├── sample_count: u32
    └── sample_offset: i32 (version 1) or u32 (version 0)
```

**Implementation:**

```rust
fn build_ctts_box(samples: &[SampleInfo]) -> Option<Vec<u8>> {
    // Check if any sample has DTS != PTS
    let needs_ctts = samples.iter().any(|s| s.dts.is_some() && s.dts != Some(s.pts));
    if !needs_ctts {
        return None;  // Skip ctts entirely for simple streams
    }
    
    let mut entries: Vec<(u32, i32)> = Vec::new();
    
    for sample in samples {
        let dts = sample.dts.unwrap_or(sample.pts);
        let offset = (sample.pts as i64 - dts as i64) as i32;
        
        // Run-length encode consecutive identical offsets
        if let Some(last) = entries.last_mut() {
            if last.1 == offset {
                last.0 += 1;
                continue;
            }
        }
        entries.push((1, offset));
    }
    
    // Use version 1 for signed offsets (B-frames can have negative)
    let mut payload = Vec::new();
    payload.extend_from_slice(&[0, 0, 0, 1]);  // version=1, flags=0
    payload.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    
    for (count, offset) in entries {
        payload.extend_from_slice(&count.to_be_bytes());
        payload.extend_from_slice(&offset.to_be_bytes());
    }
    
    Some(build_box(b"ctts", &payload))
}
```

**Add to stbl box building:**
```rust
fn build_stbl_box(...) -> Vec<u8> {
    // ... existing boxes ...
    
    if let Some(ctts) = build_ctts_box(samples) {
        payload.extend_from_slice(&ctts);
    }
    
    // ...
}
```

**Test:** New `tests/bframes.rs` with fixture containing B-frames.

**Acceptance:** 
- `cargo test --test bframes` passes
- B-frame stream plays correctly in VLC

---

## Slice E: Fragmented MP4 (fMP4)

**Goal:** Support fragmented MP4 for streaming (HLS/DASH compatible).

**Structure Difference:**

Regular MP4:
```
ftyp | moov | mdat
```

Fragmented MP4:
```
ftyp | moov (no samples) | moof | mdat | moof | mdat | moof | mdat | mfra (optional)
```

**Key Concepts:**
- **moov:** Contains track definitions but NO sample tables (empty stts/stsz/stco)
- **moof (Movie Fragment):** Contains sample metadata for one segment
- **mdat:** Contains actual sample data for that segment
- **Segment:** One moof + one mdat = one playable chunk

**API Design:**

```rust
/// Fragmented MP4 muxer for streaming.
pub struct FragmentedMuxer<Writer> {
    writer: Writer,
    config: FragmentedConfig,
    segment_duration: Duration,
    current_segment: SegmentBuilder,
    segments_written: u32,
    initialized: bool,
}

#[derive(Debug, Clone)]
pub struct FragmentedConfig {
    pub width: u32,
    pub height: u32,
    pub framerate: f64,
    pub segment_duration: Duration,  // e.g., 2 seconds
    pub audio: Option<AudioTrackConfig>,
}

impl<Writer: Write> FragmentedMuxer<Writer> {
    pub fn new(writer: Writer, config: FragmentedConfig) -> Result<Self, MuxerError>;
    
    /// Write video frame. Automatically starts new segment at keyframes
    /// when segment_duration exceeded.
    pub fn write_video(&mut self, pts: f64, data: &[u8], is_keyframe: bool) -> Result<(), MuxerError>;
    
    pub fn write_audio(&mut self, pts: f64, data: &[u8]) -> Result<(), MuxerError>;
    
    /// Flush current segment and finalize stream.
    pub fn finish(self) -> Result<FragmentedStats, MuxerError>;
}

#[derive(Debug)]
pub struct FragmentedStats {
    pub segments_written: u32,
    pub total_duration_secs: f64,
    pub bytes_written: u64,
}
```

**Box Structures:**

**Initial moov (no samples):**
```
moov
├── mvhd
├── mvex           <-- NEW: Movie Extends (signals fragmented)
│   └── trex       <-- Track Extends (default sample properties)
└── trak
    └── ... (empty stbl)
```

**moof (per segment):**
```
moof
├── mfhd           <-- Movie Fragment Header (sequence number)
└── traf           <-- Track Fragment
    ├── tfhd       <-- Track Fragment Header
    ├── tfdt       <-- Track Fragment Decode Time (base DTS)
    └── trun       <-- Track Run (sample table for this fragment)
```

**Implementation Sketch:**

```rust
fn build_mvex_box() -> Vec<u8> {
    // trex: track_ID=1, default_sample_description_index=1,
    // default_sample_duration=0, default_sample_size=0, default_sample_flags=0
    let trex_payload = [
        0, 0, 0, 0,  // version + flags
        0, 0, 0, 1,  // track_ID
        0, 0, 0, 1,  // default_sample_description_index
        0, 0, 0, 0,  // default_sample_duration
        0, 0, 0, 0,  // default_sample_size
        0, 0, 0, 0,  // default_sample_flags
    ];
    let trex = build_box(b"trex", &trex_payload);
    build_box(b"mvex", &trex)
}

fn build_moof_box(sequence_number: u32, samples: &[SampleInfo], base_offset: u64) -> Vec<u8> {
    let mfhd = build_mfhd_box(sequence_number);
    let traf = build_traf_box(samples, base_offset);
    
    let mut payload = Vec::new();
    payload.extend_from_slice(&mfhd);
    payload.extend_from_slice(&traf);
    build_box(b"moof", &payload)
}

fn build_traf_box(samples: &[SampleInfo], base_data_offset: u64) -> Vec<u8> {
    let tfhd = build_tfhd_box(1, base_data_offset);  // track_ID=1
    let tfdt = build_tfdt_box(samples[0].dts.unwrap_or(samples[0].pts));
    let trun = build_trun_box(samples);
    
    let mut payload = Vec::new();
    payload.extend_from_slice(&tfhd);
    payload.extend_from_slice(&tfdt);
    payload.extend_from_slice(&trun);
    build_box(b"traf", &payload)
}

fn build_trun_box(samples: &[SampleInfo]) -> Vec<u8> {
    // flags: 0x000001 = data-offset-present
    //        0x000100 = sample-duration-present
    //        0x000200 = sample-size-present
    //        0x000400 = sample-flags-present
    let flags: u32 = 0x000001 | 0x000100 | 0x000200 | 0x000400;
    
    let mut payload = Vec::new();
    payload.push(0);  // version
    payload.extend_from_slice(&flags.to_be_bytes()[1..]);  // 3-byte flags
    payload.extend_from_slice(&(samples.len() as u32).to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());  // data_offset (filled later)
    
    for sample in samples {
        let duration = sample.duration.unwrap_or(3000);  // default ~33ms at 90kHz
        let size = sample.data.len() as u32;
        let flags = if sample.is_keyframe { 0x02000000 } else { 0x00010000 };
        
        payload.extend_from_slice(&duration.to_be_bytes());
        payload.extend_from_slice(&size.to_be_bytes());
        payload.extend_from_slice(&flags.to_be_bytes());
    }
    
    build_box(b"trun", &payload)
}
```

**Segment Emission Logic:**

```rust
impl<Writer: Write> FragmentedMuxer<Writer> {
    pub fn write_video(&mut self, pts: f64, data: &[u8], is_keyframe: bool) -> Result<(), MuxerError> {
        // Initialize with ftyp + moov on first frame
        if !self.initialized {
            self.write_init_segment()?;
            self.initialized = true;
        }
        
        // Check if we should start a new segment
        let segment_full = self.current_segment.duration() >= self.segment_duration;
        if segment_full && is_keyframe {
            self.flush_segment()?;
        }
        
        self.current_segment.add_video(pts, data, is_keyframe);
        Ok(())
    }
    
    fn flush_segment(&mut self) -> Result<(), MuxerError> {
        if self.current_segment.is_empty() {
            return Ok(());
        }
        
        self.segments_written += 1;
        let samples = self.current_segment.take_samples();
        
        // Build moof
        let moof = build_moof_box(self.segments_written, &samples, self.bytes_written);
        let moof_len = moof.len() as u32;
        
        // Build mdat
        let mdat_payload_size: u32 = samples.iter().map(|s| s.data.len() as u32).sum();
        let mdat_size = 8 + mdat_payload_size;
        
        // Write moof
        self.write_bytes(&moof)?;
        
        // Write mdat
        self.write_bytes(&mdat_size.to_be_bytes())?;
        self.write_bytes(b"mdat")?;
        for sample in &samples {
            self.write_bytes(&sample.data)?;
        }
        
        Ok(())
    }
}
```

**Test:** New `tests/fragmented.rs`:
- Verify init segment structure (ftyp + moov with mvex)
- Verify segment structure (moof + mdat)
- Verify playback in hls.js or similar

**Acceptance:**
- `cargo test --test fragmented` passes
- Output plays in HLS.js demo player

---

## Implementation Order

1. **Slice A: audio_frames** (15 min) - Trivial, do first
2. **Slice B: Metadata** (1 hr) - Independent, can parallelize
3. **Slice C: Fast-start** (2 hrs) - High value, do early
4. **Slice D: B-frames** (2 hrs) - Independent of fast-start
5. **Slice E: Fragmented MP4** (2-3 hrs) - Largest, do last

**Total estimated time: 6-8 hours focused work**

---

## Test Matrix for v0.1.0 Release

| Feature | Unit Test | Integration Test | Player Validation |
|---------|-----------|------------------|-------------------|
| Basic MP4 | ✅ | ✅ | VLC, QuickTime, Chrome |
| AAC Audio | ✅ | ✅ | VLC, QuickTime |
| Fast-start | ✅ | ✅ | Chrome (instant play) |
| Metadata | ✅ | ✅ | VLC (shows title) |
| B-frames | ✅ | ✅ | VLC |
| Fragmented | ✅ | ✅ | hls.js, Shaka Player |

---

Ready to implement. Start with Slice A?
