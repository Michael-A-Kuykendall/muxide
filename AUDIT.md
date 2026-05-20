# Muxide Codebase Audit

**Scope**: Full top-to-bottom review of muxide v0.2.5.  
**Coverage**: All source files (`src/`), CLI (`src/bin/`), tests (`tests/`), docs, examples, Cargo.toml.  
**Methodology**: Read every file from scratch; flag bugs, correctness issues, API design problems, documentation errors, dead code, and idiomatic Rust issues.  
**Status at audit**: 123 tests passing, clean clippy. No fixes applied during this audit.

---

## Table of Contents

1. [Critical Bugs](#1-critical-bugs)
2. [Spec Non-Compliance (MP4 Container)](#2-spec-non-compliance-mp4-container)
3. [Dead / Impossible Invariants](#3-dead--impossible-invariants)
4. [API Design Problems](#4-api-design-problems)
5. [Documentation Bugs](#5-documentation-bugs)
6. [Correctness and Idiomatic Rust](#6-correctness-and-idiomatic-rust)
7. [CLI (src/bin/muxide.rs)](#7-cli-srcbinmuxiders)
8. [ROADMAP / README Staleness](#8-roadmap--readme-staleness)
9. [Summary Table](#9-summary-table)

---

## 1. Critical Bugs

### 1.1 `src/fragmented.rs::build_vpcc_fmp4` — Invalid vpcC box layout

The `vpcC` box in the non-fragmented writer (`src/muxer/mp4.rs::build_vpcc_box`) is correct: it emits the FullBox version+flags header (4 bytes) before the VPCodecConfigurationRecord, and packs `bitDepth(4) | chromaSubsampling(3) | videoFullRangeFlag(1)` into one byte, then appends the 2-byte `codecInitializationDataSize = 0`.

The fragmented version does none of these things:

```rust
// fragmented.rs (WRONG)
fn build_vpcc_fmp4(config: &FragmentConfig) -> Vec<u8> {
    let mut payload = Vec::new();
    if let Some(vp9_config) = &config.vp9_config {
        payload.push(1);                            // "version" — skips 3-byte flags!
        payload.push(vp9_config.profile);           // Now in flags position
        payload.push(vp9_config.level);             // Still in flags bytes
        payload.push(vp9_config.bit_depth);         // Raw u8 (8 or 10), not packed byte
        payload.push(vp9_config.color_space);       // In wrong field position
        payload.push(vp9_config.transfer_function);
        payload.push(vp9_config.matrix_coefficients);
        payload.push(vp9_config.full_range_flag);   // codecInitDataSize missing
    }
    build_box(b"vpcC", &payload)
}
```

Compared to the correct mp4.rs version:

```rust
// mp4.rs (CORRECT)
payload.push(1u8);                               // version = 1
payload.extend_from_slice(&[0u8, 0u8, 0u8]);    // flags = 0  (3 bytes)
payload.push(vp9_config.profile);
payload.push(vp9_config.level);
payload.push(depth_chroma_range);                // (bit_depth<<4)|(chroma<<1)|full_range
payload.push(vp9_config.color_space);
payload.push(vp9_config.transfer_function);
payload.push(vp9_config.matrix_coefficients);
payload.extend_from_slice(&0u16.to_be_bytes()); // codecInitializationDataSize = 0
```

Three distinct errors: (a) missing 3-byte FullBox flags field, (b) raw `bit_depth` instead of packed byte, (c) missing `codecInitializationDataSize`. Every fragmented VP9 file produced by this crate has a malformed `vpcC` box.

---

### 1.2 `src/muxer/mp4.rs::build_audio_specific_config` — AudioObjectType always LC regardless of profile

`build_audio_specific_config` hardcodes `aot = 2u8` (AAC-LC):

```rust
fn build_audio_specific_config(sample_rate: u32, channels: u16) -> [u8; 2] {
    // ...
    let aot = 2u8;   // AAC-LC always — never reads the codec profile
```

The `Mp4AudioTrack` struct carries `codec: AudioCodec` which contains the actual `AacProfile`. When a user muxes HE-AAC (AOT=5) or HE-AACv2 (AOT=29), the `AudioSpecificConfig` in the `esds` box will claim the stream is AAC-LC. Decoders that trust the container metadata will attempt to decode the wrong profile, producing audio glitches or silence.

The `AacProfile` → `AudioObjectType` mapping:
- `Lc` → 2
- `Main` → 1
- `Ssr` → 3
- `Ltp` → 4
- `He` → 5
- `Hev2` → 29

---

### 1.3 `src/codec/h265.rs::is_hevc_keyframe` — Panic on empty input (public function)

`is_hevc_keyframe` is a public function. Passing an empty slice panics through `INV-503`:

```rust
pub fn is_hevc_keyframe(data: &[u8]) -> bool {
    // INV-503 fires if data is empty — but this is a panic, not a graceful false
    assert_invariant!(!data.is_empty(), "INV-503: ...", "codec::h265::is_hevc_keyframe");
    // ...
}
```

Public API that panics on empty input is a contract violation. The function should return `false` for empty data. (`is_h264_keyframe` returns `false` on empty; `is_vp9_keyframe` returns `Err(FrameTooShort)` — none of the equivalents panic.)

---

### 1.4 `src/muxer/mp4.rs::build_mdhd_box_with_timescale_and_duration` — Silent duration truncation

Duration is passed as `u64` but silently cast to `u32` when building the version-0 mdhd box:

```rust
payload.extend_from_slice(&(duration as u32).to_be_bytes()); // truncates silently
```

At 90 kHz (`MEDIA_TIMESCALE`), a 32-bit duration saturates at ~13.6 hours. Longer recordings produce a corrupt `mdhd` (and therefore incorrect chapter markers and seek times) with no error or warning. The fix is to detect overflow and either emit a version-1 mdhd (64-bit) or return an error.

---

### 1.5 `src/fragmented.rs::build_hvcc_fmp4` — All-zero hvcC profile/tier/level

```rust
fn build_hvcc_fmp4(config: &FragmentConfig) -> Vec<u8> {
    // ...
    payload.push(0x00);  // general_profile_space=0, general_tier_flag=0, general_profile_idc=0
    payload.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // profile_compat flags
    payload.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // constraint flags
    payload.push(0x00);  // general_level_idc = 0 (invalid)
```

The non-fragmented `mp4.rs::build_hvcc_box` extracts these from the actual SPS bytes (`HevcConfig::general_profile_space()`, `general_tier_flag()`, etc.). The fragmented path never does this — every fragmented HEVC file has `general_level_idc = 0` in its `hvcC`, which means Level 0 (not a valid level). Strict decoders may refuse to play such content.

---

### 1.6 `src/fragmented.rs::build_av1c_fmp4` — All-zero av1C vs. populated av1C in mp4.rs

```rust
fn build_av1c_fmp4(config: &FragmentConfig) -> Vec<u8> {
    // ...
    payload.push(1);    // version
    payload.push(0);    // ALL seq_profile / seq_level_idx_0 = 0
    payload.push(0);    // ALL seq_tier/high_bitdepth/twelve_bit/monochrome/chroma fields = 0
    payload.push(0);    // initial_presentation_delay_present = 0
    // configOBUs: none written
```

`mp4.rs::build_av1c_box` correctly packs `seq_profile`, `seq_level_idx`, `seq_tier`, `high_bitdepth`, `twelve_bit`, `monochrome`, and `chroma_subsampling_*` from the `Av1Config` struct (which is populated by `parse_sequence_header`). The fragmented version hardcodes everything to zero. For any AV1 stream that is not Profile 0 or that uses high bitdepth, the fragmented `av1C` is wrong.

---

### 1.7 `src/fragmented.rs` — `flush_segment()` before `init_segment()` silently produces undecodable output

```rust
pub fn flush_segment(&mut self) -> Option<Vec<u8>> {
    // No check that init_segment() was ever called
    // No error returned; just produces a media segment
```

A media segment is not self-contained; it requires the init segment (moov/ftyp) to be parseable. If a caller writes the media segment to a socket before sending the init segment — a plausible mistake — the client receives undecodable data with no diagnostic. Should return `Result<..., FragmentedError>` and fail if the init segment has not yet been emitted.

---

## 2. Spec Non-Compliance (MP4 Container)

### 2.1 `src/muxer/mp4.rs::build_tkhd_box_with_id` — tkhd flags = 0 disables the track

```rust
payload.extend_from_slice(&0u32.to_be_bytes()); // version=0, flags=0
```

ISO 14496-12 tkhd flags:
- Bit 0 (`0x000001`): `track_enabled`
- Bit 1 (`0x000002`): `track_in_movie`

Setting flags = 0 means the track is disabled and not included in the presentation. `fragmented.rs::build_tkhd_fmp4` correctly uses `0x0000_0003`. Most players will still play the track as a fallback, but it is a spec violation. The audio tkhd also uses flags = 0 via `build_tkhd_box_with_id(2, 0x0100, 0, 0)`.

---

### 2.2 `src/muxer/mp4.rs::build_vmhd_box` — vmhd flags must be 1

```rust
fn build_vmhd_box() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes()); // version=0, flags=0 — WRONG
```

ISO 14496-12 §12.1.2 requires the vmhd box to set `flags = 0x000001` ("this box and the Track it is contained in are independent"). `fragmented.rs::build_vmhd` correctly writes `0x0000_0001_u32`. The non-fragmented writer has this wrong.

---

### 2.3 `src/muxer/mp4.rs::build_mvhd_payload` — `next_track_ID` hardcoded to 2

```rust
payload.extend_from_slice(&2u32.to_be_bytes()); // next_track_ID
```

When an audio track is present (track ID 2), the `next_track_ID` should be 3, not 2. The spec requires `next_track_ID` to be one greater than the largest track ID present. Some tools use this field when appending tracks.

---

### 2.4 `src/muxer/mp4.rs::adts_to_raw` — CRC validation not implemented

The comment states:

```rust
// Note: Full CRC validation would require implementing CRC calculation
// For now, we just check that CRC bytes exist
```

The `AdtsErrorKind::CrcMismatch` variant is defined, included in display coverage tests, but can **never actually be returned** for a CRC mismatch. ADTS frames with valid-length CRC fields but corrupt data are accepted silently. The comment should either be removed (if CRC checking is intended) or the variant should be removed/deprecated.

---

## 3. Dead / Impossible Invariants

These invariants are unreachable because the condition they guard against was already checked earlier in the same function. They clutter the invariant log without providing value.

### 3.1 INV-203 in `src/codec/av1.rs::parse_sequence_header`

```rust
let payload = &obu_data[header_size..];
if payload.is_empty() {
    return None;  // <-- returns here if empty
}
// INV-203 fires here — payload can NEVER be empty at this point
assert_invariant!(
    !payload.is_empty(),
    "AV1 sequence header payload must be non-empty",
    "codec::av1::parse_sequence_header"
);
```

---

### 3.2 INV-401 in `src/codec/vp9.rs::extract_vp9_config`

```rust
let frame_marker = r.read_bits(2)?;
if frame_marker != 2 {
    return None;  // <-- returns here if ≠ 2
}
// INV-401: always true, frame_marker is guaranteed == 2 here
assert_invariant!(
    frame_marker == 2,
    "INV-401: VP9 frame_marker must be 2",
    "codec::vp9::extract_vp9_config"
);
```

---

### 3.3 INV-302 in `src/codec/h264.rs::extract_avc_config`

The invariant checks `!sps_data.is_empty() && !pps_data.is_empty()` after the caller already skips empty slices with `if nal.is_empty() { continue; }`. The SPS and PPS buffers are only populated from non-empty NAL units, so they can never be empty when the invariant fires.

---

### 3.4 INV-201 in `src/codec/av1.rs::extract_av1_config`

```rust
assert_invariant!(
    info.obu_type <= 15,
    "INV-201: AV1 OBU type must be valid (0-15)",
    ...
);
```

`obu_type()` masks the raw byte with `& 0x0f` before returning it. The value is therefore always in range [0, 15]. The invariant is structurally impossible to violate.

---

## 4. API Design Problems

### 4.1 `channels: u16` vs `channels: u8` — type mismatch across the public surface

`MuxerConfig::with_audio` and `MuxerBuilder::audio` accept `channels: u16`. `validate_audio_config` in `validation.rs` takes `channels: u8`. A user using the validation API before building must cast, and a value `> 255` passed to the builder would silently truncate when processed internally. The two types should agree.

---

### 4.2 `Muxer<W>` and `MuxerBuilder<W>` have no `Debug` impl

Neither `Muxer<W>` nor `MuxerBuilder<W>` implements `Debug`. Library users cannot log or inspect muxer state. A conditional bound `where W: Debug` (or a manual impl that omits the writer) would be the idiomatic fix.

---

### 4.3 `FragmentedError` is not `#[non_exhaustive]`

`MuxerError` has `#[non_exhaustive]` (added in a prior fix), but `FragmentedError` does not. Adding error variants to `FragmentedError` in a minor release would break downstream code that matches exhaustively.

---

### 4.4 `FragmentedMuxer` has no audio support

The `FragmentedMuxer` API has no method to add audio samples and the init segment only creates a video track. Fragmented MP4 is most commonly used for DASH/HLS streaming which requires synchronised audio. This is a significant gap for production use.

---

### 4.5 `encode_video(data, duration_ms: 0)` — silent failure mode

`encode_video` takes `duration_ms: u32`. Passing `0` does not advance `current_video_pts`, so the very next call to `encode_video` or `write_video` fails with `NonIncreasingVideoPts`. The error is reported on the *second* call, not the first, making the cause non-obvious. The API should either validate `duration_ms > 0` or explicitly document this behaviour.

---

### 4.6 `write_video` and `encode_video` must not be mixed — undocumented

`encode_video` advances an internal `current_video_pts` counter. `write_video` uses an explicit caller-supplied `pts`. Mixing the two will produce a `NonIncreasingVideoPts` error on the first mixed call, but neither the doc for `encode_video` nor `write_video` says they cannot be combined. This should be documented as a restriction.

---

### 4.7 `MuxerBuilder::with_fast_start()` silently ignored for fragmented output

`new_with_fragment` (which builds a `FragmentedMuxer`) reads the `fast_start` field from `MuxerBuilder` but does nothing with it. Fast-start makes no sense for streaming fragmented MP4, but a caller who configures `with_fast_start().new_with_fragment(...)` receives no error or warning that the setting was ignored.

---

### 4.8 `MuxerError::FirstVideoFrameMissingSpsPps` — misleading Display for HEVC

The `Display` impl (and the internal `Mp4WriterError::FirstFrameMissingSpsPps`) says:

> "first frame must contain SPS (NAL type 7) and PPS (NAL type 8)"

NAL types 7 and 8 are H.264 terminology. H.265 also needs VPS (NAL type 32). This error is raised for both H.264 and H.265 first-frame failures, so the message is wrong for HEVC users.

---

### 4.9 `encode_language_code` duplicated in mp4.rs and fragmented.rs

Two byte-for-byte identical copies of the same function exist in `src/muxer/mp4.rs` and `src/fragmented.rs`. If language encoding ever changes, both must be updated in sync. Should live in a shared utility module.

---

## 5. Documentation Bugs

### 5.1 `src/codec/h264.rs::default_avc_config()` — completely wrong doc

```rust
/// Returns a valid configuration for 1080p @ High Profile, Level 4.0.
```

The `DEFAULT_SPS` bytes are `[0x67, 0x42, 0x00, 0x1e, ...]`:
- `0x67` = NAL unit type 7 (SPS header byte)
- `0x42` = `profile_idc` = 66 = **Baseline Profile** (not High Profile)
- `0x1e` = `level_idc` = 30 = **Level 3.0** (not Level 4.0)

The correct description is **640×480 @ Baseline Profile, Level 3.0**, which matches the `DEFAULT_SPS` constant's own doc comment. The function-level doc is the opposite of the truth.

---

### 5.2 `src/api.rs::write_video` doc — mentions only H.264

```rust
/// data slice contains the encoded frame bitstream in Annex B format (for H.264)
```

The parenthetical implies other codecs might use the same Annex B format but doesn't say. H.265 also uses Annex B (identical format). AV1 uses OBU streams. VP9 uses raw compressed frames. The doc should describe the expected format for each supported codec.

---

### 5.3 BLA NAL type constants have identical doc comments

`src/codec/h265.rs` defines:

```rust
/// Coded slice segment of a BLA picture
pub const BLA_W_LP: u8 = 16;
/// Coded slice segment of a BLA picture
pub const BLA_W_RADL: u8 = 17;
/// Coded slice segment of a BLA picture
pub const BLA_N_LP: u8 = 18;
```

All three share the same doc comment. The subtypes differ meaningfully:
- `BLA_W_LP`: with leading pictures
- `BLA_W_RADL`: with random access decodable leading pictures
- `BLA_N_LP`: no leading pictures

---

### 5.4 `ROADMAP.md` — version is stale

`ROADMAP.md` says:

```
## Current Status: v0.2.3 ✅
```

The crate is at v0.2.5. The roadmap should be kept in sync with the actual released version.

---

### 5.5 `invariant_ppt` module is `pub` in `lib.rs`

```rust
// src/lib.rs
pub mod invariant_ppt;
```

The `invariant_ppt` module is an internal testing framework, not part of the public API contract. Exposing it publicly means users see internal implementation details on docs.rs. The module-level doc acknowledges it's for internal use. Should be `pub(crate)` or, if public visibility is needed for integration tests, marked `#[doc(hidden)]`.

---

### 5.6 `src/api.rs::MuxerStats::duration_secs` — aspirational doc

The field doc says "Total stream duration in seconds (PTS of last frame + last frame duration)". In practice this is `max_end_pts / MEDIA_TIMESCALE`, where `max_end_pts` is `last_sample.pts + last_delta`. Whether `last_delta` truly represents the last frame's duration depends on whether the caller supplied the correct inter-frame spacing. The doc is correct for well-behaved callers but implies a guarantee the implementation can't always give.

---

## 6. Correctness and Idiomatic Rust

### 6.1 `src/codec/common.rs::AnnexBNalIter` yields empty slices

When two start codes appear consecutively with no bytes between them, `AnnexBNalIter` yields an empty slice. Every call site must guard against this:

```rust
for nal in AnnexBNalIter::new(data) {
    if nal.is_empty() { continue; }  // required boilerplate at every call site
    // ...
}
```

An iterator should not yield invalid items. Empty NAL slices represent a no-op and should be skipped internally. Making callers responsible for this guard is an iterator design smell and a footgun for future call sites.

---

### 6.2 `src/codec/h265.rs::hevc_nal_type()` — returns 0 for empty input

```rust
pub fn hevc_nal_type(nal: &[u8]) -> u8 {
    if nal.is_empty() {
        return 0; // NAL type 0 is valid in H.265! Conflates empty with type 0
    }
```

NAL type 0 (`TRAIL_N`) is a real H.265 NAL type. Returning 0 as a sentinel for "no data" allows callers to silently misidentify an empty slice as a trailing non-reference slice. The function should return `Option<u8>` or require non-empty input.

---

### 6.3 `src/codec/h265.rs::HevcConfig::general_level_idc()` — hardcoded byte offset 14

```rust
pub fn general_level_idc(&self) -> u8 {
    self.sps.get(14).copied().unwrap_or(0)
}
```

H.265 SPS has variable-length fields before `general_level_idc` (profile_tier_level structure includes profile compatibility flags and constraint indicator bytes that are fixed length, but there are also variable-length fields in the preceding `sps_video_parameter_set_id`, `sps_max_sub_layers_minus1`, etc.). Byte offset 14 is only correct for a narrow class of SPS configurations. For most real H.265 streams this returns the wrong value.

Compare with `general_profile_idc()` and `general_tier_flag()` which also use hardcoded offsets (bytes 12 and 13) for the same reason — the entire family of `HevcConfig` accessor functions needs proper SPS parsing.

---

### 6.4 `src/codec/vp9.rs::Vp9Config` does not derive `Eq`

`Vp9Config` derives `Clone, Debug, PartialEq` but not `Eq`. All fields are primitive types that implement `Eq` (`u32`, `u8`). `AvcConfig` and `HevcConfig` both derive `Eq`. The asymmetry is confusing and prevents `Vp9Config` from being used as a `HashMap` key or in `BTreeSet`.

---

### 6.5 `src/codec/vp9.rs::is_valid_vp9_frame` — misleading name

```rust
/// Validate that a buffer contains a valid VP9 frame.
pub fn is_valid_vp9_frame(frame: &[u8]) -> bool {
    if frame.is_empty() { return false; }
    (frame[0] >> 6) == 2  // only checks frame_marker bits
}
```

The function checks exactly one thing: that the top 2 bits equal `0b10`. It does not check anything else about the frame. The name `is_valid_vp9_frame` implies broader validation than is performed. A more accurate name would be `has_vp9_frame_marker` or `looks_like_vp9_frame`.

---

### 6.6 `src/codec/vp9.rs::Vp9Config` — `transfer_function`, `matrix_coefficients`, `level` always 0

```rust
Some(Vp9Config {
    // ...
    transfer_function: 0,       // not parsed from bitstream
    matrix_coefficients: 0,     // not parsed from bitstream
    level: 0,                   // not parsed from bitstream
    // ...
})
```

The VP9 bitstream contains transfer characteristics and matrix coefficients in the `color_config()` section. The level is derivable from the frame dimensions and framerate. Leaving these at 0 produces incorrect color metadata in the `vpcC` box. For SDR content this is usually harmless; for HDR or wide-gamut content the display pipeline may apply incorrect tone-mapping.

---

### 6.7 `src/fragmented.rs::FALLBACK_FRAME_DURATION_TICKS = 3000` — hardcoded for 30 fps

```rust
const FALLBACK_FRAME_DURATION_TICKS: u64 = 3000; // 30fps at 90kHz
```

This value is used when a fragment contains only one sample (no inter-sample DTS delta to compute from). At `timescale = 90000`:
- 30 fps → 3000 ticks ✓
- 24 fps → 3750 ticks ✗
- 25 fps → 3600 ticks ✗
- 29.97 fps → ~3003 ticks ✗

The fallback should derive the per-frame tick count from `timescale / framerate`, using the `FragmentConfig::fragment_duration_ms` and known frame count, or the timescale should be stored alongside a framerate.

---

### 6.8 `src/fragmented.rs::FragmentConfig::default()` — production footgun

```rust
impl Default for FragmentConfig {
    fn default() -> Self { ... }
}
```

The doc comment says "Note: This default provides example SPS/PPS for testing." The default SPS/PPS bytes are for a specific resolution. Any production code that calls `FragmentConfig::default()` without overriding `sps`/`pps` will produce a `hvcC` or `avcC` for the wrong resolution. The `Default` impl should be removed or gated behind `#[cfg(test)]`.

---

### 6.9 `src/validation.rs` — enforces limits tighter than `Muxer` itself

`validate_video_config` rejects dimensions below 320×240 or above 4096×2160. `MuxerBuilder` and `Mp4Writer` do not enforce these limits (only 16-bit overflow is checked). A user who calls `validate_video_config` and gets a pass will later find `Muxer` accepts 1×1, and vice versa — a user who never calls `validate_video_config` can build a valid 320×239 file. The validation module and the runtime should enforce identical constraints or the discrepancy should be clearly documented.

---

### 6.10 `src/muxer/mp4.rs::compute_interleave_schedule` — called three times in fast-start path

In `finalize_fast_start`, `compute_interleave_schedule` is called three times:
1. Once with placeholder offsets (to measure `moov` size)
2. Once with real offsets (to compute the final `moov`)
3. Once again during the actual sample-write loop

All three calls produce the same ordering (deterministic sort). The result could be computed once and reused, avoiding two unnecessary re-sorts. Not a correctness bug, but an avoidable allocation/computation cost that grows with frame count.

---

### 6.11 `src/invariant_ppt.rs` — `INVARIANT_LOG` is thread-local; cross-thread tests cannot share logs

```rust
thread_local! {
    static INVARIANT_LOG: RefCell<HashSet<String>> = ...;
}
```

If code under test runs in a worker thread (e.g., Tokio runtime, rayon), the invariants fired in that thread are invisible to the test thread's `get_logged_invariants()`. Tests using the PPT framework would silently pass even if invariants fired in other threads. This is not documented.

---

### 6.12 `src/invariant_ppt.rs` — `INVARIANT_LOG` deduplicates by message string

The `HashSet` deduplicates invariant messages. If the same invariant fires multiple times (e.g., inside a loop), it is only recorded once. `contract_test` cannot observe how many times an invariant fired, only whether it ever fired. This limits the precision of contract testing.

---

## 7. CLI (`src/bin/muxide.rs`)

### 7.1 `process_video_frames` writes entire file as a single frame at PTS=0

```rust
fn process_video_frames(...) -> Result<()> {
    // reads entire file into one Vec<u8>
    let data = read_hex_bytes(&hex_content)?;
    // writes everything as a single keyframe at time 0
    muxer.write_video(0.0, &data, true)?;
```

For any video input with more than one frame (which is the normal case), the CLI wraps all data in a single `write_video` call. If that data contains multiple Annex B NAL units, they will be muxed as a single logical frame. The CLI cannot produce usable multi-frame video output in its current state. This renders the `mux` subcommand largely non-functional for real inputs.

---

### 7.2 `--audio` silently ignored when `--sample-rate` or `--channels` is missing

```rust
if let (Some(_audio), Some(sample_rate), Some(channels)) = (&audio, sample_rate, channels) {
    builder = builder.audio(codec, sample_rate, channels as u16);
}
```

If `--audio` is given but `--sample-rate` or `--channels` is omitted, the audio path is silently not configured. The subsequent `write_audio` call returns `MuxerError::AudioNotConfigured` — a confusing error that doesn't tell the user they forgot to pass the required flags. The CLI should produce a clear error message in this case.

---

### 7.3 `--dry-run` validates only file existence, not content

The dry-run mode checks that input files exist and that the output path is writable. It does not validate:
- Whether the video file contains valid Annex B data
- Whether codec parameters match the file content
- Whether the output path has sufficient disk space

The feature is named "dry run" which implies meaningful pre-flight validation. The gap should be documented.

---

### 7.4 `ProgressBar::set_length` called with running total, not final total

```rust
fn update_bytes(&mut self, bytes: u64) {
    self.stats.total_bytes += bytes;
    if let Some(pb) = &self.progress {
        pb.set_length(self.stats.total_bytes); // grows with each update
    }
}
```

`set_length` sets the "total" for progress bar percentage calculation. Calling it with an ever-increasing running total means the progress bar is always at 100%. The progress bar should either be a spinner (no percentage) or have its final length set once at the start.

---

## 8. ROADMAP / README Staleness

| Item | Current State | Document Claim |
|------|--------------|----------------|
| `ROADMAP.md` current version | v0.2.5 | "v0.2.3 ✅" |
| README code example | Shows `MuxerBuilder::new(file).video(...).build()?` | Correct |
| ROADMAP "200+ tests" | 123 tests | "200+ unit and integration tests" |

---

## 9. Summary Table

| # | File | Issue | Severity |
|---|------|-------|----------|
| 1.1 | `src/fragmented.rs` | `build_vpcc_fmp4` — malformed vpcC (missing FullBox flags, wrong byte packing) | **Critical** |
| 1.2 | `src/muxer/mp4.rs` | `build_audio_specific_config` — AudioObjectType always LC regardless of profile | **Critical** |
| 1.3 | `src/codec/h265.rs` | `is_hevc_keyframe` panics on empty input | **Critical** |
| 1.4 | `src/muxer/mp4.rs` | `mdhd` duration silently truncated to u32 | **High** |
| 1.5 | `src/fragmented.rs` | `build_hvcc_fmp4` all-zero profile/level in hvcC | **High** |
| 1.6 | `src/fragmented.rs` | `build_av1c_fmp4` all-zero fields, diverged from mp4.rs | **High** |
| 1.7 | `src/fragmented.rs` | `flush_segment()` before `init_segment()` — no error | **High** |
| 2.1 | `src/muxer/mp4.rs` | tkhd flags=0 disables both video and audio tracks | **Medium** |
| 2.2 | `src/muxer/mp4.rs` | vmhd flags=0 (should be 1) | **Medium** |
| 2.3 | `src/muxer/mp4.rs` | `next_track_ID = 2` even when audio track (ID 2) present | **Low** |
| 2.4 | `src/muxer/mp4.rs` | `CrcMismatch` error variant can never be returned | **Low** |
| 3.1 | `src/codec/av1.rs` | INV-203 dead invariant | Low |
| 3.2 | `src/codec/vp9.rs` | INV-401 dead invariant | Low |
| 3.3 | `src/codec/h264.rs` | INV-302 dead invariant | Low |
| 3.4 | `src/codec/av1.rs` | INV-201 dead invariant | Low |
| 4.1 | `src/api.rs`, `src/validation.rs` | `channels: u16` vs `channels: u8` type mismatch | **Medium** |
| 4.2 | `src/api.rs` | `Muxer<W>` and `MuxerBuilder<W>` missing `Debug` | Low |
| 4.3 | `src/fragmented.rs` | `FragmentedError` not `#[non_exhaustive]` | **Medium** |
| 4.4 | `src/fragmented.rs` | `FragmentedMuxer` has no audio support | **High** |
| 4.5 | `src/api.rs` | `encode_video(data, 0)` fails on next call with no useful error | **Medium** |
| 4.6 | `src/api.rs` | Mixing `write_video` and `encode_video` undocumented restriction | **Medium** |
| 4.7 | `src/api.rs` | `with_fast_start()` silently ignored for fragmented output | Low |
| 4.8 | `src/api.rs` | `FirstVideoFrameMissingSpsPps` error message H.264-only language | Low |
| 4.9 | `src/muxer/mp4.rs`, `src/fragmented.rs` | `encode_language_code` duplicated | Low |
| 5.1 | `src/codec/h264.rs` | `default_avc_config()` doc is completely wrong (says 1080p High/4.0, is 640×480 Baseline/3.0) | **High** |
| 5.2 | `src/api.rs` | `write_video` doc mentions only H.264 Annex B format | **Medium** |
| 5.3 | `src/codec/h265.rs` | BLA NAL constant docs all identical | Low |
| 5.4 | `ROADMAP.md` | Version stale (says v0.2.3, is v0.2.5); test count inflated | Low |
| 5.5 | `src/lib.rs` | `invariant_ppt` module is `pub`; internal framework exposed | Low |
| 5.6 | `src/api.rs` | `MuxerStats::duration_secs` doc slightly aspirational | Low |
| 6.1 | `src/codec/common.rs` | `AnnexBNalIter` yields empty slices; every caller must filter | **Medium** |
| 6.2 | `src/codec/h265.rs` | `hevc_nal_type()` returns 0 for empty (conflates with valid type 0) | **Medium** |
| 6.3 | `src/codec/h265.rs` | `HevcConfig::general_level_idc()` hardcodes byte offset 14 — wrong for most SPS | **High** |
| 6.4 | `src/codec/vp9.rs` | `Vp9Config` missing `Eq` derive | Low |
| 6.5 | `src/codec/vp9.rs` | `is_valid_vp9_frame` name misleading | Low |
| 6.6 | `src/codec/vp9.rs` | `transfer_function`, `matrix_coefficients`, `level` always 0 | **Medium** |
| 6.7 | `src/fragmented.rs` | `FALLBACK_FRAME_DURATION_TICKS = 3000` hardcoded for 30 fps | **Medium** |
| 6.8 | `src/fragmented.rs` | `FragmentConfig::default()` SPS/PPS for wrong resolution | **Medium** |
| 6.9 | `src/validation.rs` | Validation limits tighter than actual muxer limits | **Medium** |
| 6.10 | `src/muxer/mp4.rs` | `compute_interleave_schedule` called 3× in fast-start path | Low |
| 6.11 | `src/invariant_ppt.rs` | Thread-local log invisible across thread boundaries | **Medium** |
| 6.12 | `src/invariant_ppt.rs` | Log deduplicates; cannot observe how many times invariant fired | Low |
| 7.1 | `src/bin/muxide.rs` | `process_video_frames` writes entire file as one frame at PTS=0 | **Critical** |
| 7.2 | `src/bin/muxide.rs` | `--audio` without `--sample-rate`/`--channels` silently ignored | **High** |
| 7.3 | `src/bin/muxide.rs` | Dry-run validates only file existence | **Medium** |
| 7.4 | `src/bin/muxide.rs` | Progress bar `set_length` called with running total | Low |
| 8.1 | `ROADMAP.md` | "200+ tests" claim (actual: 123) | Low |

---

*Audit performed against commit ec08801 (v0.2.5).*
