# Changelog

## 0.4.0 (July 27, 2026) - Audio-Only MP4, Dependency Upgrades & MSRV Bump

### ✨ New Features
- **Audio-only MP4 and fragmented MP4 (fMP4)** (`muxide-9ue.4`): `MuxerBuilder` now accepts an audio track without a video track (and vice-versa). `FragmentedMuxer::new_audio_only()` produces an init segment with a single audio `trak` (AAC `mp4a` or Opus). Added `tests/audio_only.rs` integration coverage.

### 🔧 Dependency Upgrades
- **`thiserror` 1 → 2**, **`criterion` 0.5 → 0.8**, dropped `lazy_static`, unpinned `proptest` (`=1.5.0` → `1`). ASCII-only validation checks; gated dead `is_valid_vp9_frame()` behind `#[cfg(test)]`.
- **MSRV bumped 1.81 → 1.86** (required by `criterion` 0.8.2). CI matrix and `MSRV Check` job updated accordingly.

## 0.3.0 (July 26, 2026) - WASM, Go & C Language Bindings

### ✨ **New Features**
- **WASM bindings** (`wasm` feature, `src/wasm.rs`): compile the muxer to WebAssembly via wasm-bindgen. `WasmMuxerBuilder` / `WasmMuxer` expose video/audio configuration and frame writing from JS/TS.
- **C FFI layer** (`src/ffi.rs` + `bindings/muxide.h`): a stable C ABI with 18 `extern "C"` functions, opaque `MuxideMuxer` / `MuxideFragmentedMuxer` handles, and error-code constants. Enables embedding Muxide in any C-ABI language.
- **Go bindings** (`bindings/go`): idiomatic Go wrapper over the C FFI using cgo, with `runtime.SetFinalizer` handle management and Rust error propagation. A `go.mod` (module `github.com/Michael-A-Kuykendall/muxide`) lets `go build ./bindings/go/...` resolve from the repo root.
- **CI**: added a `wasm-check` job that builds the wasm32 target on every push/PR.

### 🔒 **Security**
- **Bumped `rand` 0.8.5 → 0.8.6** (RUSTSEC-2026-0097, low). `rand` is a transitive dev-dependency via `proptest`. No code changes required; resolves the Dependabot alert on the default branch.

### 🧪 **Testing**
- Added end-to-end functional tests for the new bindings: a Rust FFI integration test (`tests/ffi_smoke.rs`), a strengthened Go test that validates MP4 structure, and a wasm-bindgen test run via `wasm-bindgen-test-runner`.
- Exposed `MuxideMuxer` / `MuxideFragmentedMuxer` as `pub` (opaque handles) so external crates can hold the pointers.
- Target-gated non-wasm dev-dependencies so `cargo test --target wasm32-unknown-unknown` compiles; aligned `wasm-bindgen` to 0.2.108 for the test runner.

## 0.2.6 (July 26, 2026) - Production Audit: Code Quality & CI Hardening

### 🐛 **Critical Bug Fixes**
- **Silent fallback to default H.264 config for non-H.264 codecs** (`muxer/mp4.rs`): When `finalize()` was called without extracted codec config (e.g., H.265/AV1/VP9 with no frames), the code silently fell back to a default H.264 AVC config, producing a broken MP4. Now returns a descriptive `io::Error` explaining that codec parameter sets are required.

### 🔧 **Code Quality**
- **Clippy: `needless_range_loop`** (`examples/interop_av_h264_aac.rs:182`): Replaced index-based loop with iterator + enumerate for idiomatic Rust.
- **Clippy: `manual_split_once`** (`examples/interop_av_h264_aac.rs:209`): Replaced `splitn(2, '=').nth(1)` with `split_once('=').map(|x| x.1)`.
- **Dead code: `build_moof()` wrapper** (`fragmented.rs`): Removed unused wrapper function; call site now uses `build_moof_with_offset` directly.
- **Unused parameter: `_timescale`** (`fragmented.rs:769`): Removed unused parameter from `build_media_segment`.
- **Manual `new()` instead of `Default`** (`bin/muxide.rs:156`): `MuxStats` now derives `Default` instead of implementing a manual `new()` method.
- **Missing context in `assert_invariant!`** (`codec/h265.rs:191`): Added missing context string `"codec::h265::extract_hevc_config"` to 2-arg macro call.

### 📝 **Documentation Fixes**
- **Misleading "approximate" comment** (`muxer/mp4.rs:2439`): Removed comment claiming `format_unix_timestamp` was approximate; the algorithm is exact.
- **Misleading "~2100" limit comment** (`muxer/mp4.rs:2462`): Removed comment claiming `days_to_ymd` only works until ~2100; the algorithm works for any u64-representable date.

### 🚀 **CI/CD Improvements**
- **Removed daily cron schedule** (`.github/workflows/ci.yml`): CI no longer runs on a daily schedule; only runs on push to main/master, pull requests, and tag pushes (deploy).
- **Nightly job now runs on release tags** (`.github/workflows/ci.yml`): Changed from `schedule` trigger to `startsWith(github.ref, 'refs/tags/v')` so comprehensive testing runs on releases.

## 0.2.5 (May 20, 2026) - MP4 Spec Compliance, Codec Box Correctness, Audio/Video Interop

### 🐛 **Critical Bug Fixes**
- **`build_vpcc_fmp4` emitted a malformed vpcC box** (`fragmented.rs`): missing FullBox header (version + flags), wrong bit-packing for the `bit_depth/chroma_subsampling/full_range_flag` byte, and missing `codecInitializationDataSize` field. All three issues fixed.
- **`build_hvcc_fmp4` emitted all-zero profile/level fields** (`fragmented.rs`): now extracts `general_profile_idc`, `general_tier_flag`, `general_profile_space`, and `general_level_idc` from the HEVC SPS via `HevcConfig`, matching the logic in `build_hvcc_box`.
- **`build_av1c_fmp4` emitted all-zero codec config fields** (`fragmented.rs`): now parses `seq_profile`, `seq_level_idx`, `seq_tier`, `high_bitdepth`, monochrome, and chroma subsampling from the AV1 Sequence Header OBU via `extract_av1_config`, matching `build_av1c_box`.
- **`build_audio_specific_config` always wrote `AudioObjectType = 2` (AAC-LC)** (`muxer/mp4.rs`): regardless of the `AacProfile` passed by the caller. Now correctly sets AOT 1 (Main), 2 (LC), 3 (SSR), 4 (LTP), 5 (HE), or 29 (HE-v2) based on the profile.
- **`build_mdhd_box` silently truncated durations > 13.6 hours** (`muxer/mp4.rs`): the `u64 → u32` cast wrapped silently. Now uses a version-1 (64-bit) mdhd box when `duration_ms > u32::MAX`.
- **`is_hevc_keyframe` panicked on empty input** (`codec/h265.rs`): `assert_invariant!(INV-503)` panicked instead of returning `false`. Replaced with a graceful early return.

### 🔧 **MP4 Spec Compliance**
- **`build_tkhd_box` emitted `flags = 0`** (`muxer/mp4.rs`): ISO 14496-12 §8.3.2 requires bit 0 (track_enabled) and bit 1 (track_in_movie) set. Fixed to `0x0000_0003`.
- **`build_tkhd_box` track duration was always 0** (`muxer/mp4.rs`): ISO 14496-12 §8.3.2 requires the `duration` field to carry the track duration in movie timescale. Both video and audio `tkhd` boxes now receive the actual computed duration instead of 0. (Reported via PR #4 and PR #6.)
- **`build_vmhd_box` emitted `flags = 0`** (`muxer/mp4.rs`): ISO 14496-12 §12.1.2 requires `flags = 1`. Fixed.
- **`build_mvhd_payload` hardcoded `next_track_ID = 2`** (`muxer/mp4.rs`): even when both video and audio tracks were present. Now passes the correct value (2 for video-only, 3 for video+audio).

### 🗑️ **Dead Invariants Removed**
- **INV-201** (`codec/av1.rs`): `obu_type & 0x0f ≤ 15` — always true due to the 4-bit mask.
- **INV-203** (`codec/av1.rs`): `!payload.is_empty()` after a prior early-return guard.
- **INV-302** (`codec/h264.rs`): non-empty SPS/PPS check after the iterator already filtered empties.
- **INV-401** (`codec/vp9.rs`): `frame_marker == 2` after a prior early-return guard.

### ✨ **API / Code Quality**
- **`Muxer<W>` lacked a `Debug` impl**: added manual impl exposing track counts and status without printing the writer.
- **`MuxerBuilder<W>` lacked a `Debug` impl**: added manual impl showing video/audio config and SPS/PPS/VPS presence.
- **`MuxerError::FirstVideoFrameMissingSpsPps` Display was H.264-only**: updated to mention H.265 NAL types 32/33/34 as well.
- **`write_video` doc only mentioned H.264**: updated to cover all supported codecs.
- **`encode_language_code` was duplicated** between `mp4.rs` and `fragmented.rs`: made `pub(crate)` in `mp4.rs`, removed the copy in `fragmented.rs`.
- **`FragmentedError` was missing `#[non_exhaustive]`**: attribute added.
- **`Vp9Config` was missing `Eq` derive**: added.
- **`AudioValidationConfig.channels`** and **`validate_audio_config` parameter**: promoted from `u8` to `u16` to match the API (`AudioTrackConfig.channels: u16`).
- **`invariant_ppt` module** marked `#[doc(hidden)]` (internal testing infrastructure).

### 📖 **Documentation**
- **`default_avc_config()` doc claimed "1080p @ High Profile, Level 4.0"**: corrected to "640×480 @ Baseline Profile, Level 3.0" (the actual config values).
- **BLA NAL type doc strings were all identical** (`codec/h265.rs`): `BLA_W_LP`, `BLA_W_RADL`, and `BLA_N_LP` now have distinct descriptions.
- **ROADMAP.md**: version updated from v0.2.3 to v0.2.5; test count corrected from "200+" to "123".

## 0.2.5 (May 19, 2026) - Safety Fixes: Empty-NAL Panics, HEVC BLA Keyframes, Non-Exhaustive Error

### 🐛 **Bug Fixes**
- **`encode_video` could panic on H.264/H.265 streams with consecutive Annex B start codes**: `AnnexBNalIter` can yield empty `&[u8]` slices when two start codes appear back-to-back (which some encoders emit). The H.264 branch of `is_keyframe()` accessed `nal[0]` unconditionally, and the H.265 branch did the same. Both now guard with `!nal.is_empty()` before indexing. Added regression tests for both codecs.
- **`encode_video` failed to detect HEVC BLA frames as keyframes**: The H.265 branch of `is_keyframe()` only checked NAL types 19–21 (IDR + CRA), missing BLA types 16–18 (`BLA_W_LP`, `BLA_W_RADL`, `BLA_N_LP`). All three are IRAP frames and must be treated as keyframes. Range corrected to `16..=21`. Added regression test.
- **`is_hevc_keyframe()` panicked on data with no Annex B start codes**: An `assert_invariant!` at the function's exit path (INV-505) fired whenever the data contained no NAL units, which can happen when validation code is given malformed or non-Annex-B input. Replaced with a graceful `false` return. Added unit test.
- **Redundant `as u8` cast in VP9 parser** (`vp9.rs:229`): `read_bit()` already returns `u8`; the cast was flagged by `clippy::cast_lossless`. Removed.

### 🔒 **API Correctness**
- **`MuxerError` was not `#[non_exhaustive]`**: Adding error variants in a minor release would have been a breaking change for downstream code using exhaustive `match` arms. Attribute added along with a doc note advising callers to use a wildcard arm.

### 🔑 **Discoverability**
- **`Cargo.toml` keywords**: Replaced the generic `"multimedia"` keyword with `"hevc"` — a primary codec supported by the crate and a far more targeted search term on crates.io.

## 0.2.4 (May 19, 2026) - Test Rename, Benchmark Repair, Doc Consistency

### 🐛 **Bug Fixes**
- **Windows test failure resolved**: Renamed `tests/video_setup.rs` to `tests/video_track_structure.rs`. Windows installer-compatibility heuristics were triggering a UAC elevation prompt on any executable containing "setup" in its name, causing that integration test binary to fail on Windows with OS error 740.

### 🔧 **Benchmark Fix**
- **`benches/muxing.rs` used invalid dummy frames**: Both benchmark functions wrote 10 KB zero-byte buffers which fail H.264 validation. Replaced with real fixture data (`frame0_key.264`, `frame1_p.264`, `frame0.aac.adts`) so benchmarks measure a valid mux path end-to-end.

### ✨ **Documentation**
- **`docs/contract.md`**: Keyframe invariant now lists per-codec header requirements (SPS/PPS for H.264, VPS/SPS/PPS for H.265, Sequence Header OBU for AV1, frame header params for VP9). Pseudo-code example updated to `Aac(AacProfile::Lc)`.
- **`README.md`**: Removed fabricated benchmark table with specific µs/throughput numbers that were measured on development hardware; replaced with a pointer to `cargo bench`. Fixed a stray `</details>` tag.
- **`ROADMAP.md`**: Removed "audit pass" language from Recent Achievements; entries now describe changes, not process.
- **`release-notes/v0.2.1.md`, `release-notes/v0.2.2.md`**: Added missing release-note files.

## 0.2.3 (May 21, 2026) - Documentation, DRY, and Residual Bugs

### 🐛 **Bug Fixes**
- **`encode_video()` would panic on empty input instead of returning an error**: `encode_video()` called `is_keyframe(data)` before `write_video()`'s empty-data guard. If `data` was empty, the `assert_invariant!(!data.is_empty())` inside `is_keyframe()` would panic rather than return `MuxerError::EmptyVideoFrame`. Fixed by adding an early empty-data guard at the top of `encode_video()` so `is_keyframe()` is never called with empty data.
- **`MuxStats.duration_ms` was always 0 in CLI JSON output**: The `duration_ms` field was initialized to 0 and never populated. Fixed by switching from `muxer.finish()` to `muxer.finish_with_stats()` and deriving the value from the returned `MuxerStats`.

### 🧹 **Removed Vapor / Dead Code**
- **Dead `let _ = sample_count;`** in `SampleTables::from_samples`: `sample_count` was already used for `Vec::with_capacity`; the suppression binding at the end of the function was pure dead weight.
- **Removed unused `use crate::assert_invariant;`** from `api.rs` top-level imports (the macro is now referenced inline with `use` inside `is_keyframe()`).

### ✨ **Documentation**
- **`MuxerBuilder` struct doc was stale**: Said "B-frame support, fragmented MP4 will be added in future slices" — both are implemented. Replaced with an accurate summary of all supported codecs and output modes.
- **`build()` comment was stale**: Said "In v0, we perform minimal validation... Future releases may relax this." Removed development-phase language.
- **`finish_in_place()` doc leaked an internal test reference**: Removed the stale implementation-specific wording and replaced it with an accurate description.
- **`finish()` and `finish_with_stats()` had no doc comments.** Both now documented.
- **`finish_in_place_with_stats()` doc improved**: Now cross-references `finish_in_place()`.
- **`write_audio()` doc only mentioned AAC**: Now explicitly covers Opus packets too.
- **`encode_video()` and `encode_audio()` docs were terse single-liners**: Expanded to explain the auto-timestamp behaviour, when to use these vs. the explicit-PTS methods, and the meaning of the `samples` parameter.
- **`MuxerConfig` fields undocumented**: All six fields (`width`, `height`, `framerate`, `audio`, `metadata`, `fast_start`) now have field-level doc comments.
- **`MuxerStats` fields undocumented**: All four fields now have field-level doc comments.
- **`Metadata::new()`, `with_title()`, `with_creation_time()`** had no doc comments. Added.
- **`MuxerConfig::new()`, `with_audio()`, `with_metadata()`, `with_fast_start()`** had no doc comments. Added.

### 🔀 **Code Organisation**
- **`simple_api_works` test was in `thread_safety_tests` module**: Moved to the `tests` module where it belongs. The `make_h264_keyframe()` helper moved with it.
- **`test_poisoned_lock_paths_are_handled` renamed** in `invariant_ppt.rs`: The test was renamed from a prior RwLock-poisoning test. Now named `test_invariant_log_records_and_clears` which accurately describes what it does. Stale comment removed.

### ♻️ **Refactor**
- **Magic `3000` in `fragmented.rs` replaced** with a named constant `FALLBACK_FRAME_DURATION_TICKS` (90 000 Hz timescale ticks for one frame at 30 fps). Both uses in `flush_segment` and `build_trun` now reference the constant.
- **Duplicate DTS span calculation extracted** in `FragmentedMuxer`: `ready_to_flush()` and `current_fragment_duration_ms()` both computed the same span. The shared logic is now in a private `buffered_duration_ms()` helper. Both public methods delegate to it.

## 0.2.2 (May 20, 2026) - Codebase Audit: Correctness & Idiomatic Rust

### 🐛 **Bug Fixes**
- **AV1 keyframe detection**: `is_keyframe()` for AV1 previously returned `true` only for the first frame (a "first frame = keyframe" heuristic). It now delegates to `codec::av1::is_av1_keyframe()`, which parses actual OBU headers and checks the `frame_type` bits for correctness.
- **`write_video()` missing finished guard**: `write_video()` did not check `self.finished`, so calling it after `finish()` would silently write into a finalized muxer. It now returns `MuxerError::AlreadyFinished` consistently with `write_video_with_dts()`.
- **`fragmented.rs` moov box order**: `build_moov_fmp4()` was emitting `mvhd → mvex → trak`. ISO 14496-12 §8.3 requires `trak` to precede `mvex`. Fixed to `mvhd → trak → mvex`.
- **`info_command` codec detection always returned "Unknown"**: The CLI scanned only top-level MP4 box types for FourCCs like `avc1`, `hvc1`, `vp09`, `mp4a`. These are deeply nested (inside `moov/trak/mdia/minf/stbl/stsd`) and were never found. Fixed by scanning the full buffer with `buffer.windows(4)`. AV1 (`av01`) detection also added.
- **`creation_time` CLI arg was a stub**: `--creation-time` previously printed "Warning: creation_time not yet implemented" and did nothing. Changed to `Option<u64>` (Unix timestamp, no chrono dep), now properly builds and applies `Metadata::with_creation_time()`.
- **`encode_audio()` anti-pattern**: `is_none()` check followed immediately by `.unwrap()` replaced with `let Some(...) else` binding.
- **Misleading invariant in `convert_mp4_error()`**: `curr_pts: 0.0` hardcoded in the `NonIncreasingTimestamp` error arm. Now uses `self.last_video_pts.unwrap_or(0.0)` with a comment clarifying the arm is unreachable (PTS is validated before the inner call).

### 🧹 **Removed Vapor / Dead Code**
- **Deleted `src/config.rs`**: File was never declared in `lib.rs`, was completely invisible to the compiler, and duplicated structs already defined in `api.rs`. Pure dead weight.
- **Removed duplicate `MuxerBuilder` methods**: `set_create_time()`, `set_language()`, `set_video_track()`, `set_audio_track()` were exact duplicates of `with_metadata()`, `video()`, and `audio()`. Callers (CLI) migrated to the canonical methods.
- **Removed `Muxer::flush()`**: Exact alias for `finish()` with no semantic difference. Removed to eliminate the confusion between "flush" (partial write) and "finish" (finalize). The `FragmentedMuxer::flush_segment()` method is unrelated and unaffected.
- **Removed bogus `assert_invariant!` calls from `api.rs`**: The AV1 invariant `is_key || video_frame_count > 0` was a tautology (always true). The VP9 invariant `is_key || data.len() >= 3` was misleading and wrong as a correctness check.
- **Removed `assert_invariant!` calls from CLI (`src/bin/muxide.rs`)**: Ten assertions in `mux_command` panicked instead of returning errors, including always-true enum checks and post-fact checks on already-set state. Replaced meaningful range checks with `anyhow::ensure!` before builder calls; removed the rest entirely.

### ✨ **Improvements**
- **`MuxerConfig::into_builder()`**: New method converts a `MuxerConfig` into a pre-configured `MuxerBuilder`, transferring audio, metadata, and fast_start automatically. Previously callers had to pull out individual fields.
- **`src/api.rs` module doc fixed**: Doc comment was placed between `use` statements as a `///` item doc. Corrected to `//!` at the top of the file.
- **`read_hex_bytes()` returns `Result<Vec<u8>>`**: Previously used `assert!` and `.expect()` (panics). Now returns `anyhow::Result` with descriptive errors. All callers updated to propagate with `?`.
- **`&PathBuf` → `&Path` in CLI functions**: `process_video_frames()`, `process_audio_frames()`, `validate_hex_file()` now accept `&std::path::Path` per Rust API guidelines.
- **`validate_hex_file()` simplified**: Redundant manual hex-character validation loop removed. Now delegates to `read_hex_bytes()` which already performs full validation.
- **Metadata building unified in CLI**: Title, creation_time, and language are now built into a single `Metadata` struct instead of being set through disparate methods.

## 0.2.1 (May 19, 2026) - Bug Fix & CLI Fragmented MP4

### 🐛 **Critical Bug Fix**
- **`tkhd` duration overflow**: The non-fragmented muxer was writing the `duration` field in version-0 Track Header boxes as `u64` (8 bytes) instead of `u32` (4 bytes) per ISO 14496-12 §8.3.2. This inserted 4 extra bytes into every `tkhd` box, shifting the matrix, width, and height fields to wrong offsets. Players would infer an incorrect Sample Aspect Ratio (e.g. `24:5` instead of `1:1`), causing stretched or squished video playback. The fragmented path was already correct; this aligns the non-fragmented path to match. Reported by @peteralm80, confirmed by @zkvsky (issue #5).

### ✨ **CLI Fragmented MP4 (H.264 + VP9)**
- The `--fragmented` flag in the `mux` subcommand now works for H.264 and VP9 inputs. The CLI extracts SPS/PPS automatically from the Annex B bitstream for H.264 and VP9 config from the first keyframe for VP9, then writes a spec-correct `ftyp + moov + moof + mdat` fragmented MP4. H.265 and AV1 fragmented output require additional parameter-set arguments not yet exposed in the CLI; those will continue to use the library API directly.

### 🗺️ **Roadmap Cleanup**
- Removed duplicate sections and speculative placeholder items ("Quantum-Safe Metadata", "Holographic Video Support", "Blockchain-Integrated Provenance", "Neural Codec Interfaces") that had no grounding in the project's mission.

## 0.2.0 - Fragmented MP4 Multi-Codec & Safety

### 🎬 **Fragmented MP4 (fMP4) Multi-Codec Support**
- **Multi-Codec fMP4**: Fragmented MP4 init+media segment support for **H.264, H.265/HEVC, AV1, and VP9**
- **HEVC `hvcC` Correctness**: Config box structure aligns with emitted parameter set arrays; signals 4-byte NAL length prefixes
- **Explicit Sample Contract**: Fragmented video samples are **MP4 length-prefixed** (4-byte NAL length prefixes), not Annex B start codes

### 🧱 **Safety & Contract Enforcement**
- **Monotonic DTS Enforcement**: Fragmented muxer rejects decreasing DTS instead of underflowing
- **Timestamp Validation**: Rejects non-finite timestamps (NaN/Inf) for video PTS/DTS and audio PTS with specific error variants
- **MP4 Size Overflow Hardening**: Prevents `mdat` size/offset overflow with deterministic errors

### 🔍 **Interop / Conformance Scope**
- **Interop Note**: H.264 outputs were spot-checked with FFmpeg/ffprobe (non-fragmented MP4 and concatenated fMP4 init+segment); broader player/device conformance validation is ongoing.

## 0.1.5 (December 30, 2025) - Quality & Completeness

### 🎯 **VP9 Production Readiness**
- **Full-Range Support**: Implemented proper parsing of VP9 color configuration full-range flags, ensuring accurate vpcC boxes for all VP9 streams
- **Metadata Accuracy**: Fixed VP9 muxing to generate correct color space metadata instead of hardcoded defaults

### 🛠️ **API Modernization**
- **Breaking Change**: Removed deprecated `Muxer::new()` and `Muxer::simple()` constructors
- **Unified API**: Standardized all muxer construction through `MuxerBuilder` for consistency and maintainability
- **Migration Path**: Updated all examples and documentation to use the modern API

### 🔍 **Enhanced CLI Diagnostics**
- **Smart Codec Detection**: `muxide info` command now identifies video codecs (H.264, H.265, VP9) and detects audio presence
- **Better MP4 Analysis**: Improved file validation with detailed codec information for troubleshooting

### 📚 **Documentation & Testing**
- **Accuracy First**: Corrected VP9 feature claims in README to match actual implementation capabilities
- **Test Quality**: Replaced placeholder CLI tests with functional validation, ensuring command reliability
- **Roadmap Alignment**: Updated development roadmap to reflect completed VP9 feature parity work

### 🔧 **Under the Hood**
- **Code Quality**: Eliminated deprecated APIs and improved internal consistency
- **Validation API**: Refactored validation functions to use structured config objects for better maintainability
- **Build Cleanliness**: Resolved all compilation warnings for pristine release builds
- **Test Coverage**: Maintained 100% test pass rate across 123+ unit tests and property-based validations

## 0.1.4

No functional changes relative to v0.1.3. Version bump only.

Full Changelog: https://github.com/Michael-A-Kuykendall/muxide/compare/v0.1.3...v0.1.4

## 0.1.3

No functional changes relative to v0.1.2. Version bump only.

Full Changelog: https://github.com/Michael-A-Kuykendall/muxide/compare/v0.1.2...v0.1.3

## 0.1.2

- **CLI Tool**: Complete command-line interface with progress bars, JSON output, and comprehensive muxing options
- **Code Quality**: Artifact cleanup, improved error handling patterns, and clippy compliance
- **Documentation**: Enhanced README with complete feature documentation
- **Release Polish**: Final production-ready codebase with all warnings addressed and comprehensive tests

## 0.1.1

- **AAC Profile Support**: Complete implementation of all 6 AAC profiles (LC, Main, SSR, LTP, HE, HEv2)
- **ADTS Error Handling**: Comprehensive ADTS validation with detailed diagnostics, hex dumps, and recovery suggestions
- **MP4E-Compatible APIs**: Added `new_with_fragment()`, `flush()`, `set_create_time()`, `set_language()` methods
- **Metadata Support**: Title, creation time, and language metadata in MP4 files
- **HEVC/H.265 Support**: Annex B format with VPS/SPS/PPS configuration
- **AV1 Support**: OBU stream format with Sequence Header OBU configuration
- **Opus Support**: Raw Opus packets with 48kHz sample rate
- **CLI Tool**: Command-line interface with progress bars, JSON output, and comprehensive options
- **Invariant PPT Framework**: Property-based testing with contract verification
- **Documentation**: Complete README, governance files (CODE_OF_CONDUCT, CONTRIBUTING, etc.), and roadmap

## 0.1.0

- MP4 writer with a single H.264 video track (Annex B input).
- Optional AAC audio track (ADTS input).
- 90 kHz media timebase for track timing.
- Dynamic `avcC` configuration derived from SPS/PPS in the first keyframe.
- Deterministic finalisation with explicit errors on double-finish and post-finish writes.
- Specific `MuxerError` variants for common failure modes.
- Convenience API: `Muxer::new(writer, MuxerConfig)`.
- Finish statistics: `finish_with_stats` / `finish_in_place_with_stats`.
