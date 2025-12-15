# v0.1.0 Slice Ladder

This document enumerates the planned slices for the v0.1.0 release of **Muxide**.  Each slice is a small, self‑contained work unit that can be implemented and verified within a limited context window.  Slices must be completed in order; a slice is not considered done until its acceptance gate passes.  See the *Slice‑Gated Engineering Doctrine* for the philosophy behind this structure.

## Slice 01 — Initialise Project Skeleton

**Goal:** Create the project structure, including `Cargo.toml`, `README.md`, source tree, and documentation skeleton.  Write the charter and contract documents based on the v0.1.0 scope.

**Files touched:**
- `muxide/Cargo.toml`
- `muxide/README.md`
- `muxide/src/lib.rs`
- `muxide/src/api.rs`
- `muxide/docs/charter.md`
- `muxide/docs/contract.md`
- `muxide/slice_ladder.md`

**Inputs:** None.

**Outputs:** A compiles-without-warnings library crate with placeholder types and documentation files.

**Acceptance gate:**
- Command: `cargo check` (or equivalent) must succeed without errors.
- The charter and contract documents exist and contain the agreed‑upon contents.

**Non‑goals:** Implementing any container logic, writing tests, or adding dependencies beyond the Rust standard library.

---

## Slice 02 — Define Test Harness & Golden Fixtures

**Goal:** Establish the testing infrastructure and truth anchors.  Add golden MP4 files produced by a known‑good tool to the repository and add tests that read these files and perform basic checks (e.g. verifying that the `ftyp` and `moov` boxes exist at the expected locations).  The goal is to prepare fixtures for later comparison once the muxer can produce files.

**Files touched:**
- `muxide/tests/golden.rs`
- `muxide/fixtures/` (directory for golden files)
- `muxide/Cargo.toml` (to enable the `mp4parse` crate or similar for inspection)

**Inputs:** Golden MP4 files generated outside this project (to be committed before this slice begins).

**Outputs:** A test module that can parse the golden files and assert basic properties without panicking.

**Acceptance gate:**
- Command: `cargo test --test golden` must pass, indicating that the golden files are readable and the test expectations are correct.

**Non‑goals:** Producing any files with Muxide; we are only reading fixtures in this slice.

---

## Slice 03 — Build Minimal MP4 Writer (Headers Only)

**Goal:** Implement the ability to write a minimal MP4 file containing only the file type (`ftyp`) and minimal movie (`moov`) boxes with no tracks.  The writer should produce a valid MP4 file even though it contains no media data.  This will serve as the scaffold for adding tracks in later slices.

**Files touched:**
- `muxide/src/muxer/` (new module for writer implementation)
- `muxide/src/api.rs` (wire builder to writer creation)
- `muxide/tests/minimal.rs` (unit test for minimal file)

**Inputs:** None.

**Outputs:** Ability to call `MuxerBuilder::new(...).video(...).build()?.finish()` and produce a file with `ftyp` and `moov` boxes, but no samples.

**Acceptance gate:**
- Command: `cargo test --test minimal` passes.
- The produced file can be parsed by the test harness without errors and matches the structure of the golden minimal file.

**Non‑goals:** Writing any `mdat` or sample data; interleaving logic; audio support.

---

## Slice 04 — Implement Video Track Setup

**Goal:** Extend the MP4 writer to write a single video track (`trak`) with a stub `stbl` (sample table) but no samples.  Add support for writing the SPS/PPS codec configuration in the `avcC` box.  The muxer should accept video configuration parameters and write appropriate boxes in the `moov` box.

**Files touched:**
- `muxide/src/muxer/mp4.rs` (track creation)
- `muxide/src/api.rs` (update builder validation if necessary)
- `muxide/tests/video_setup.rs`

**Inputs:** Encoded SPS/PPS extracted from golden bitstreams (fixtures).

**Outputs:** MP4 file containing `ftyp`, `moov` with one `trak`, `mdia`, `minf`, `stbl` with an `avcC` box configured correctly, but still no `mdat` samples.

**Acceptance gate:**
- Command: `cargo test --test video_setup` passes.
- Structure matches expectations when inspected with the test harness and `ftyp`/`moov` boxes.

**Non‑goals:** Writing actual video samples; audio support; dynamic timing.

---

## Slice 05 — Write Video Samples Sequentially

**Goal:** Implement writing of video samples into the `mdat` box and updating the sample table (`stts`, `stsz`, `stco`, `stss`) accordingly.  Support only keyframes (no B‑frames) and enforce monotonic `pts`.  Use fixtures for encoded frames to verify sample counts and sizes.

**Files touched:**
- `muxide/src/muxer/mp4.rs` (sample writing logic)
- `muxide/src/api.rs` (expose `write_video` functionality)
- `muxide/tests/video_samples.rs`

**Inputs:** Encoded H.264 frames in Annex B format (fixtures).

**Outputs:** MP4 file with a single video track containing samples; player matrix should play the resulting file.

**Acceptance gate:**
- Command: `cargo test --test video_samples` passes.
- File plays back correctly in QuickTime and other players (manual check) or passes a structural MP4 validator.

**Non‑goals:** Audio support; variable frame rates beyond constant frame rate; B‑frames.

---

## Slice 06 — Add Optional AAC Audio Track

**Goal:** Add the ability to configure and write a single AAC audio track interleaved with video.  Implement `write_audio` to append audio samples and update the audio track sample table.  Enforce that audio `pts` is non‑decreasing and does not precede video `pts`.

**Files touched:**
- `muxide/src/muxer/mp4.rs` (audio track support)
- `muxide/src/api.rs` (enable audio builder configuration)
- `muxide/tests/audio_samples.rs`

**Inputs:** Encoded AAC frames in ADTS format (fixtures).

**Outputs:** MP4 files containing both video and audio tracks.  Files should play correctly and audio/video should remain in sync for the duration of the test.

**Acceptance gate:**
- Command: `cargo test --test audio_samples` passes.
- File plays with audio and video in sync across the player matrix.

**Non‑goals:** Support for multiple audio tracks; support for codecs other than AAC.

---

## Slice 07 — Finalisation and Clean‑Up

**Goal:** Implement the `finish` method to finalise the file, write any outstanding metadata (e.g. `mdat` size, `moov` offset), and return the underlying writer if necessary.  Ensure that double‑calls to `finish` or writes after finalisation produce errors.

**Files touched:**
- `muxide/src/muxer/mp4.rs` (finalisation logic)
- `muxide/src/api.rs` (expose `finish` behaviour)
- `muxide/tests/finalisation.rs`

**Inputs:** None beyond previous fixtures.

**Outputs:** Finalised MP4 file matching golden fixtures for the same input streams.

**Acceptance gate:**
- Command: `cargo test --test finalisation` passes.
- Files produced before and after finalisation are identical (idempotent finish).

**Non‑goals:** Streaming/fragmented MP4; recovery after crash.

---

## Slice 08 — Error Handling & Reporting

**Goal:** Flesh out the `MuxerError` enum with specific error variants for all failure modes encountered in slices 05–07 (e.g. non‑monotonic timestamps, missing SPS/PPS, invalid ADTS frames).  Ensure that error messages are descriptive and tests cover each variant.

**Files touched:**
- `muxide/src/api.rs` (error enum updates)
- `muxide/src/muxer/` (error propagation)
- `muxide/tests/error_handling.rs`

**Inputs:** Malformed inputs (invalid frames, out‑of‑order timestamps) provided as fixtures.

**Outputs:** Comprehensive error messages and corresponding tests.

**Acceptance gate:**
- Command: `cargo test --test error_handling` passes, confirming that each error case produces the expected variant and message.

**Non‑goals:** Adding any new functionality beyond better error handling.

---

## Slice 09 — CrabCamera Timebase (90 kHz) & Drift Control

**Goal:** Align the muxer timing model with CrabCamera’s expected usage (`pts = frame_number / framerate`) by using a 90 kHz media timescale (common in MP4/H.264 workflows) and ensuring that timestamp conversion does not accumulate drift over long recordings.

**Files touched:**
- `muxide/src/muxer/mp4.rs`
- `muxide/src/api.rs`
- `muxide/tests/timebase.rs`

**Inputs:** Existing Annex B fixtures (reused).

**Outputs:** Video and audio sample tables (`stts`, `mdhd`) expressed in a 90 kHz timescale; long runs do not accumulate rounding drift for common frame rates.

**Acceptance gate:**
- Command: `cargo test --test timebase` passes.

**Non‑goals:** B‑frame support; fragmented MP4; fast-start (`moov` at front).

---

## Slice 10 — Dynamic AVC Config (`avcC`) From Stream

**Goal:** Populate `avcC` using SPS/PPS parsed from the first video keyframe (as provided by openh264 / CrabCamera) instead of any baked-in codec configuration. This improves real-world playback compatibility.

**Files touched:**
- `muxide/src/muxer/mp4.rs`
- `muxide/tests/avcc_dynamic.rs`
- `muxide/fixtures/` (new fixtures if needed)

**Inputs:** Annex B fixtures containing SPS/PPS.

**Outputs:** `avcC` reflects the actual bitstream SPS/PPS for the recording.

**Acceptance gate:**
- Command: `cargo test --test avcc_dynamic` passes.

**Non‑goals:** Supporting mid-stream SPS/PPS changes; multiple parameter sets.

---

## Slice 11 — CrabCamera Convenience API & Stats

**Goal:** Provide a CrabCamera-friendly constructor and finalisation stats without breaking the existing builder-based API.

**Files touched:**
- `muxide/src/api.rs`
- `muxide/tests/stats.rs`
- `muxide/docs/contract.md`

**Inputs:** Existing fixtures (reused).

**Outputs:**
- A `MuxerConfig`/`MuxerStats` pair suitable for CrabCamera integration.
- A convenience constructor (`Muxer::new(...)` or equivalent) and a stats-returning finish variant.

**Acceptance gate:**
- Command: `cargo test --test stats` passes.

**Non‑goals:** Adding new codecs or container features.

---

## Slice 12 — Public Documentation & Examples

**Goal:** Write crate‑level documentation and examples demonstrating how to use Muxide.  Include a tutorial that shows reading encoded frames from a file, muxing them into an MP4, and playing back the result.  Verify that examples compile with `cargo doc --document-private-items --no-deps`.

**Files touched:**
- `muxide/src/lib.rs` (doc comments)
- `muxide/examples/` (example binaries)
- `muxide/Cargo.toml` (example dependencies)

**Inputs:** None.

**Outputs:** Rich documentation visible on docs.rs that explains the API and its invariants.  Examples compile and run successfully.

**Acceptance gate:**
- Command: `cargo doc --document-private-items --no-deps` succeeds without warnings.
- Example binaries compile and run, producing files that play back in the player matrix.

**Non‑goals:** Implementation changes; performance tuning.

---

## Slice 13 — Release Preparation

**Goal:** Prepare the crate for publication.  Audit the crate for unused code, ensure license headers are present, and bump the version to `0.1.0`.  Write a `CHANGELOG.md` summarising features.  Publish to crates.io (manual step beyond the scope of this agent).  Create Git tags and release notes.

**Files touched:**
- `muxide/Cargo.toml` (version bump)
- `muxide/CHANGELOG.md`
- Release scripts (if any)

**Inputs:** None.

**Outputs:** A ready‑to‑publish crate with version `0.1.0` and matching tags.

**Acceptance gate:**
- Command: `cargo package` succeeds without warnings.
- The package contents match the expected file list (no unwanted files included).

**Non‑goals:** Adding new features; optimisations; refactors.

---

Each slice after 13 is considered beyond the v0.1.0 scope and will be defined in a future slice ladder under a new charter.