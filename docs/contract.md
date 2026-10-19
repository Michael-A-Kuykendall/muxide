# Muxide API Contract (v0.5.0)

This document defines the public API contract and invariants for Muxide 0.5.0. It is the reference for users of the crate and for implementers. Public items in `muxide::api` are covered below. Fragmented MP4 lives in `muxide::fragmented`. Release notes are in `CHANGELOG.md`.

## High‑Level API

Muxide exposes a builder pattern for creating a `Muxer` instance that writes an MP4 container to an arbitrary writer (implementing `std::io::Write`).

### Types

* `VideoCodec`: Enumeration of supported video codecs.
  * `H264` — H.264/AVC video codec. Bitstreams must be in Annex B format.
  * `H265` — H.265/HEVC video codec. Bitstreams must be in Annex B format.
  * `Av1` — AV1 video codec. Bitstreams must be supplied as an OBU stream.
  * `Vp9` — VP9 video codec. Bitstreams must be supplied as compressed frames.

* `AudioCodec`: Enumeration of supported audio codecs.
  * `Aac(AacProfile)` — AAC audio codec, encoded as ADTS frames. Profile selects LC, Main, SSR, LTP, HE, or HEv2; LC offers the broadest playback compatibility.
  * `Opus` — Opus audio codec, supplied as raw Opus packets. (In MP4, Opus is always signaled at 48 kHz.)
  * `None` — Indicates that no audio track will be created.

* `MuxerBuilder<Writer>` — Type parameterised by an output writer.  Provides methods to configure the container and tracks and to build a `Muxer`.  The builder consumes itself on `build`.

* `Muxer<Writer>` — Type parameterised by an output writer.  Provides methods to write video and audio frames and to finalise the file.  The generic parameter is preserved to allow any type implementing `Write` as the underlying sink.

* `MuxerConfig` — Simple configuration struct for integrations that prefer a config-driven constructor.

* `MuxerStats` — Statistics returned when finishing a mux.

* `MuxerError` — Enumeration of error conditions that may be returned by builder or runtime operations.  This enum may grow as the implementation matures.

### Timebase

The progressive `Muxer` converts timestamps in seconds (`pts: f64`) into a fixed media timebase.

- Progressive video and audio both use a **90 kHz** media timescale (`pts * 90000`).
- `tkhd` and `mvhd` durations are in the movie timescale (**1000**). A duration above `u32::MAX` is stored in a version-1 box. Fragmented `tkhd` duration stays 0.
- On an audio-only or two-track `FragmentedMuxer`, `write_audio` timestamps are **audio samples** at the configured sample rate, and the audio `mdhd` timescale is that sample rate. Video fragment timestamps stay in `FragmentConfig::timescale` (typically 90000).

### MuxerBuilder Methods

* `new(writer: Writer) -> Self` — Constructs a builder for the given writer.  The writer is consumed by the builder and later moved into the `Muxer`.

* `video(self, codec: VideoCodec, width: u32, height: u32, framerate: f64) -> Self` — Configures the video track. Width and height must each be in `1..=65535`. `build` and `new_with_fragment` return `MuxerError::InvalidDimensions` otherwise. Non-integer frame rates (for example 29.97) are permitted. `MuxerBuilder` does not cap the frame rate. The separate `muxide::validation` helper, and the CLI `validate` path, still reject rates above 120 fps and sizes outside 320×240 through 4096×2160.

* `audio(self, codec: AudioCodec, sample_rate: u32, channels: u16) -> Self` — Configures an optional audio track. At most one audio track is stored. If `audio` is omitted, the file contains only video. If `codec` is `AudioCodec::None`, the sample rate and channels are ignored and no audio track is created. Audio without video is a valid audio-only file.

* `build(self) -> Result<Muxer<Writer>, MuxerError>` — Validates the configuration and returns a `Muxer` instance on success. Validation rules:
  1. At least one track must be configured. An empty builder, or video omitted together with `AudioCodec::None`, returns `MuxerError::MissingTrackConfig`. `MuxerError::MissingVideoConfig` is a deprecated alias. `build` does not return it.
  2. When video is configured, width and height must be in `1..=65535`.
  3. Video-only, audio-only, and video plus audio are all valid.

### Muxer Methods

* `write_video(&mut self, pts: f64, data: &[u8], is_keyframe: bool) -> Result<(), MuxerError>` — Writes a video frame to the container.

  **Invariants:**
  - `pts` **must be non‑negative and strictly greater than the `pts` of the previous video frame**. Violations produce `MuxerError::NegativeVideoPts` or `MuxerError::NonIncreasingVideoPts`.
  - `data` must contain a complete encoded frame in the codec's expected format (Annex B for H.264/H.265; OBU stream for AV1; compressed frame for VP9).  The first video frame must be a keyframe and must include the codec configuration headers (SPS/PPS for H.264/H.265, Sequence Header OBU for AV1, frame header with parameters for VP9); missing headers produce `MuxerError::FirstVideoFrameMustBeKeyframe`, `MuxerError::FirstVideoFrameMissingSpsPps`, `MuxerError::FirstAv1FrameMissingSequenceHeader`, or `MuxerError::FirstVp9FrameMissingSequenceHeader`.
  - `is_keyframe` must accurately reflect whether the frame is a keyframe.  Incorrect keyframe flags may result in unseekable files.

  **B-frames:**
  - `write_video()` is intended for streams where **PTS == DTS** (no reordering).
  - For streams with B-frames (PTS != DTS), use `write_video_with_dts()` and feed frames in decode order.

* `write_audio(&mut self, pts: f64, data: &[u8]) -> Result<(), MuxerError>` — Writes an audio frame to the container.

  **Invariants:**
  - `pts` **must be non‑negative and strictly greater than or equal to the `pts` of the previous audio frame**.
  - When a video track is configured, audio must not arrive before the first video frame, including a write that happens before any video frame. The error is `MuxerError::AudioBeforeFirstVideo`. An audio-only muxer has no video track, so this check does not apply.
  - `data` must contain a complete encoded audio frame:
    - AAC must be ADTS; invalid ADTS is rejected.
    - Opus must be a structurally valid Opus packet; invalid packets are rejected.

* `encode_video(&mut self, data: &[u8], duration_ms: u32) -> Result<(), MuxerError>` — Writes one video frame and advances PTS by `duration_ms`. `duration_ms == 0` returns `MuxerError::ZeroVideoDuration` on that call. An empty frame returns `MuxerError::EmptyVideoFrame`. Calling this on an audio-only muxer returns `MuxerError::VideoNotConfigured`.

* `encode_audio(&mut self, data: &[u8], samples: u32) -> Result<(), MuxerError>` — Writes one audio frame and advances PTS by `samples / sample_rate`. `samples == 0` returns `MuxerError::ZeroAudioDuration` on that call.

* `finish(self) -> Result<(), MuxerError>` — Finalises the container.  After calling this method, no further `write_*` calls may be made.  This method writes any pending metadata (e.g. `moov` box) to the output writer.

* `finish_with_stats(self) -> Result<MuxerStats, MuxerError>` — Finalises the container and returns muxing statistics.

* `finish_in_place(&mut self) -> Result<(), MuxerError>` — Finalises the container without consuming the muxer.  This is a convenience for applications that want an explicit “finalised” error on double-finish and on writes after finishing.

* `finish_in_place_with_stats(&mut self) -> Result<MuxerStats, MuxerError>` — In-place finalisation that returns muxing statistics.

### Error Semantics

All functions that can fail return a `MuxerError`. New error variants may be added in minor versions.

### Concurrency & Thread Safety

`Muxer<W>` is `Send` when `W: Send` and `Sync` when `W: Sync`.

Muxide itself is implemented as a single-threaded writer; thread-safety here refers to moving/sharing the muxer value when the underlying writer type supports it.

## Invariants & Correctness Rules

1. **Monotonic Timestamps:** For each track, presentation timestamps (`pts`) must be non‑negative and strictly increasing (video) or non‑decreasing (audio). If this invariant is violated, the operation must fail.
2. **Keyframes:** The first video frame must be a keyframe and must include the codec configuration required by that codec: SPS/PPS for H.264, VPS/SPS/PPS for H.265, a Sequence Header OBU for AV1, and valid frame header parameters for VP9. Subsequent keyframes must be marked via the `is_keyframe` flag. Files produced without proper keyframe signalling will not play back correctly and are considered incorrect.
3. **Tracks:** At most one video track and at most one audio track. A file may be video-only, audio-only, or both. An empty configuration is `MuxerError::MissingTrackConfig`. The builder stores one video configuration and one audio configuration; it has no API for a second track of either kind.
4. **Single Audio Track:** At most one audio track is supported.
5. **B‑frames:** Streams with reordering (B-frames) are supported when callers use `write_video_with_dts()`:
  - Frames must be supplied in **decode order**.
  - DTS must be strictly increasing.
  - PTS may differ from DTS.
  For streams without reordering, callers may use `write_video()` which assumes PTS == DTS.
6. **Bitstream formats:**
  - H.264/H.265 video must be provided in Annex B format (start-code-prefixed NAL units).
  - AV1 video must be provided as an OBU stream.
  - VP9 video must be provided as compressed VP9 frames (with frame headers intact, not Annex B).
7. **Audio formats:**
  - AAC audio must be provided as ADTS frames.
  - Opus audio must be provided as raw Opus packets.

## B-frame Support

Muxide supports B-frames via the `write_video_with_dts()` method:

- When B-frames are present, callers must provide both PTS (presentation timestamp) and DTS (decode timestamp)
- DTS must be monotonically increasing (decode order)
- PTS may differ from DTS (display order ≠ decode order)
- The `ctts` (composition time offset) box is automatically generated

For streams without B-frames, use `write_video()` which assumes PTS == DTS.

## Fragmented MP4

`FragmentedMuxer` has three configurations:

1. **Video-only** — `FragmentedMuxer::new(config) -> Result<Self, FragmentedError>`. Track id 1. `write_audio` returns `FragmentedError::AudioTrackNotConfigured`.
2. **Audio-only** — `FragmentedMuxer::new_audio_only(audio)`. Track id 1, matching the init segment `trex`. `write_video` returns `FragmentedError::VideoTrackNotConfigured`.
3. **Video plus audio** — `FragmentedMuxer::new_with_audio(config, audio) -> Result<Self, FragmentedError>`. Video is track 1. Audio is track 2. Each `trun` `data_offset` points at that track's bytes in `mdat` (video bytes, then audio bytes).

`new` and `new_with_audio` return `FragmentedError::InvalidDimensions` when width or height is outside `1..=65535`. `new_audio_only` does not take video dimensions. `AsyncFragmentedMuxer::new` also returns `Result`. `muxide_fragmented_new` returns a null pointer when those dimensions are rejected.

`MuxerBuilder::new_with_fragment` builds an audio-only fragmented muxer when audio is configured and video is not. When video is configured it builds a video fragmented muxer and requires the codec parameter sets (`with_sps` / `with_pps`, plus `with_vps` for H.265, `with_av1_sequence_header` for AV1, `with_vp9_config` for VP9). Two-track fMP4 is `FragmentedMuxer::new_with_audio`.

A single AAC sample with no following sample lasts 1024 samples. A single Opus packet uses its TOC duration, or 960 samples when the packet has no TOC. The video single-sample fallback is `timescale / 30`.

## Examples (Pseudo‑Code)

```
use muxide::api::{AacProfile, AudioCodec, MuxerBuilder, VideoCodec};
use std::fs::File;

// Create an output file
let file = File::create("out.mp4")?;

// Build a muxer for 1920x1080 30 fps video and 48 kHz stereo audio
let mut mux = MuxerBuilder::new(file)
    .video(VideoCodec::H264, 1920, 1080, 30.0)
    .audio(AudioCodec::Aac(AacProfile::Lc), 48_000, 2)
    .build()?;

// Write frames (encoded elsewhere)
for (i, frame) in video_frames.iter().enumerate() {
    let pts = (i as f64) / 30.0;
    let is_key = i == 0 || i % 30 == 0;
    mux.write_video(pts, &frame.data, is_key)?;
    // Optionally interleave audio
}

// Finish the file
mux.finish()?;
```

## Stability

Muxide 0.5.0 is a pre-1.0 release. Breaking changes are allowed in 0.x and are listed in `CHANGELOG.md`. `FragmentedMuxer::new` and `AsyncFragmentedMuxer::new` return `Result` as of 0.5.0. `MuxerError` and `FragmentedError` are `non_exhaustive` and may gain variants.
