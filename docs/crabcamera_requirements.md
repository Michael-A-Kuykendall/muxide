# Muxide Requirements from CrabCamera v0.5.0

**Consumer:** CrabCamera video recording module  
**Producer:** Muxide v0.1.0

## What CrabCamera Will Send

```rust
// Video frames from openh264 encoder
struct VideoFrame {
    pts: f64,           // Presentation timestamp in seconds (monotonic, starts at 0.0)
    data: Vec<u8>,      // H.264 NAL units in Annex B format (start codes: 0x00000001)
    is_keyframe: bool,  // true for IDR frames (first frame MUST be keyframe with SPS/PPS)
}

// Audio frames (future, not v0.5.0)
struct AudioFrame {
    pts: f64,           // Presentation timestamp in seconds
    data: Vec<u8>,      // AAC in ADTS format
}
```

## What CrabCamera Needs from Muxide

```rust
// Minimal API contract
trait Muxer {
    fn new(writer: impl Write, config: MuxerConfig) -> Result<Self, MuxerError>;
    fn write_video(&mut self, pts: f64, data: &[u8], is_keyframe: bool) -> Result<(), MuxerError>;
    fn write_audio(&mut self, pts: f64, data: &[u8]) -> Result<(), MuxerError>;  // optional for v0.1
    fn finish(self) -> Result<MuxerStats, MuxerError>;
}

struct MuxerConfig {
    width: u32,         // e.g., 1920
    height: u32,        // e.g., 1080
    framerate: f64,     // e.g., 30.0
    // audio config optional for v0.1
}

struct MuxerStats {
    video_frames: u64,
    duration_secs: f64,
    bytes_written: u64,
}
```

## Critical Invariants

1. **First frame MUST be keyframe with SPS/PPS** - openh264 guarantees this
2. **PTS is monotonically increasing** - CrabCamera guarantees this
3. **Annex B format only** - openh264 outputs Annex B (start codes), NOT length-prefixed
4. **No B-frames** - openh264 default config has no B-frames, PTS == DTS
5. **Output must play in:** VLC, QuickTime, Windows Media Player, Chrome `<video>`

## Frame Rate & Timing

- CrabCamera sends PTS as `frame_number / framerate` (e.g., frame 30 at 30fps = 1.0 sec)
- Muxide must convert to MP4 timescale (typically 90000 for video)
- No gaps expected - continuous recording

## File Output

- Container: MP4 (ISOBMFF)
- Video codec: H.264/AVC (profile: Baseline or Main, openh264 default)
- moov box: Can be at end (simpler) or front (fast-start, nice-to-have)
- Keyframe index (stss): Required for seeking

## What CrabCamera Does NOT Need (v0.5.0)

- Audio (future)
- B-frames / frame reordering
- Fragmented MP4
- Multiple tracks
- Metadata/chapters
- MKV/WebM containers

---

**TL;DR for Muxide:** Accept H.264 Annex B frames with monotonic PTS, write valid MP4 that plays everywhere. That's it.
