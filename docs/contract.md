# Muxide API Contract (v0.1.0)

This document defines the public API contract and invariants for the v0.1.0 release of Muxide.  The contract is intended to be a stable reference for users of the crate and for implementers working on the internals.  All public items in the `muxide::api` module are covered by this contract.

## High‑Level API

Muxide exposes a builder pattern for creating a `Muxer` instance that writes an MP4 container to an arbitrary writer (implementing `std::io::Write`).  The API is intentionally minimal; configuration options beyond those described here are not available in v0.1.0.

### Types

* `VideoCodec`: Enumeration of supported video codecs.  In v0.1.0 this enum has a single variant:
  * `H264` — Represents the H.264/AVC video codec.  Bitstreams must be in Annex B format.  B‑frames are not supported; frames must be supplied in presentation order.

* `AudioCodec`: Enumeration of supported audio codecs.  In v0.1.0 this enum has two variants:
  * `Aac` — Represents the AAC audio codec, encoded in ADTS frames.  Only AAC Low Complexity (LC) profiles are expected to play back correctly.
  * `None` — Indicates that no audio track will be created.  Use this when only video is being muxed.

* `MuxerBuilder<Writer>` — Type parameterised by an output writer.  Provides methods to configure the container and tracks and to build a `Muxer`.  The builder consumes itself on `build`.

* `Muxer<Writer>` — Type parameterised by an output writer.  Provides methods to write video and audio frames and to finalise the file.  The generic parameter is preserved to allow any type implementing `Write` as the underlying sink.

* `MuxerError` — Enumeration of error conditions that may be returned by builder or runtime operations.  This enum will grow as the implementation matures; clients should treat unknown variants exhaustively.

### MuxerBuilder Methods

* `new(writer: Writer) -> Self` — Constructs a builder for the given writer.  The writer is consumed by the builder and later moved into the `Muxer`.

* `video(self, codec: VideoCodec, width: u32, height: u32, framerate: f64) -> Self` — Configures the video track.  Exactly one call to `video` is required for v0.1.0.  The frame rate must be positive and reasonable (e.g. between 1 and 120).  Non‑integer frame rates (e.g. 29.97) are permitted.

* `audio(self, codec: AudioCodec, sample_rate: u32, channels: u16) -> Self` — Configures an optional audio track.  At most one call to `audio` may be made.  Audio is optional; if omitted, the file will contain only video.  If `codec` is `None`, the sample rate and channels are ignored.

* `build(self) -> Result<Muxer<Writer>, MuxerError>` — Validates the configuration and returns a `Muxer` instance on success.  In v0.1.0 the following validation rules apply:
  1. A video track must have been configured.  Otherwise a `MuxerError::MissingVideoConfig` is returned.
  2. If an audio track is configured, it must have a positive sample rate and channel count.  Invalid values result in a `MuxerError::Other` with a descriptive message.

### Muxer Methods

* `write_video(&mut self, pts: f64, data: &[u8], is_keyframe: bool) -> Result<(), MuxerError>` — Writes a video frame to the container.

  **Invariants:**
  - `pts` **must be non‑negative and strictly greater than the `pts` of the previous video frame**.  Frames with non‑monotonic timestamps cause `MuxerError::Other` with a descriptive message.
  - `data` must contain a complete encoded frame in Annex B format.  The first video frame of a file must be a keyframe and must contain SPS and PPS NAL units; otherwise a `MuxerError::Other` is returned.
  - `is_keyframe` must accurately reflect whether the frame is a keyframe (IDR picture).  Incorrect keyframe flags may result in unseekable files.

* `write_audio(&mut self, pts: f64, data: &[u8]) -> Result<(), MuxerError>` — Writes an audio frame to the container.

  **Invariants:**
  - `pts` **must be non‑negative and strictly greater than or equal to the `pts` of the previous audio frame**.
  - Audio must not arrive before the first video frame (i.e. audio `pts` must be >= video `pts`).
  - `data` must contain a complete encoded AAC frame (ADTS).  Empty frames cause an error.

* `finish(self) -> Result<(), MuxerError>` — Finalises the container.  After calling this method, no further `write_*` calls may be made.  This method writes any pending metadata (e.g. `moov` box) to the output writer.  Errors returned at this stage indicate IO failures or internal state errors.  The writer is consumed by `finish`; if you need access to the inner writer, call `into_inner` instead (not available in v0.1.0).

### Error Semantics

All functions that can fail return a `MuxerError`.  Clients must handle each error case explicitly; ignoring an error or assuming that all errors are recoverable is incorrect.  New error variants may be added in minor versions.  The error enum is deliberately non‑exhaustive: clients should include a wildcard arm when matching on errors.

### Concurrency & Thread Safety

The v0.1.0 implementation is single‑threaded and does not make any guarantees about `Send` or `Sync` for the `Muxer` type.  Future releases may add asynchronous and multi‑threaded variants.

## Invariants & Correctness Rules

1. **Monotonic Timestamps:** For each track, presentation timestamps (`pts`) must be non‑negative and strictly increasing (video) or non‑decreasing (audio).  If this invariant is violated, the operation must fail.
2. **Keyframes:** The first video frame must be a keyframe containing SPS and PPS.  Subsequent keyframes must be marked via the `is_keyframe` flag.  Files produced without proper keyframe signalling will not play back correctly and are considered incorrect.
3. **Single Video Track:** Exactly one video track is supported.  Multiple video tracks or the absence of a video track is an error.
4. **Single Audio Track:** At most one audio track is supported.  Adding multiple audio tracks is not allowed.
5. **No B‑frames:** The v0.1.0 implementation does not support frame reordering (no B‑frames).  Inputs containing B‑frames must be rejected.
6. **Annex B only:** H.264 bitstreams must be provided in Annex B format (NAL units prefaced by start codes).  MP4 native format (length‑prefixed NAL units) is not supported for input.
7. **ADTS only:** AAC audio must be provided as ADTS frames.  Raw AAC or other container formats (e.g. MP4 audio atoms) are not supported.

## Examples (Pseudo‑Code)

```
use muxide::api::{MuxerBuilder, VideoCodec, AudioCodec};
use std::fs::File;

// Create an output file
let file = File::create("out.mp4")?;

// Build a muxer for 1920x1080 30 fps video and 48 kHz stereo audio
let mut mux = MuxerBuilder::new(file)
    .video(VideoCodec::H264, 1920, 1080, 30.0)
    .audio(AudioCodec::Aac, 48_000, 2)
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

The API described here must not change in any breaking way during the v0.1.x series.  Additional methods may be added, but existing signatures and invariants must remain stable.  Breaking changes require a new major version or a new charter.
