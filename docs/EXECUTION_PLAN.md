# Muxide — Slice‑Gated, Context‑Preserved Execution Checklist (Local‑AI Ready)

**Created:** 2025‑12‑15  
**Audience:** Local AI / lower‑context agents executing deterministic tasks  
**Core principle:** *No performance theater.* Every claim must have a gate, evidence, and reproducible checks.

---

## 0) Executive Pin: What Muxide Is

### 0.1 Product invariant (single sentence)
Muxide guarantees that any **correctly‑timestamped**, **already‑encoded** audio/video stream can be turned into a **standards‑compliant**, **immediately‑playable** MP4 **without external tooling**.

### 0.2 What this implies (non‑negotiable)
- **Write‑only** MP4 muxer (not demuxer).
- **No encoding/decoding/transcoding.**
- **Strict input contract:** reject bad timestamps; do not “fix.”
- **Zero non‑std deps** in production crate (dev‑deps allowed for tests).
- **Fast‑start** is a first‑class feature (moov before mdat).

### 0.3 What this explicitly refuses (guardrails)
- MP4 reading/demuxing
- Encoding, decoding, transcoding
- DRM/CENC/Widevine/FairPlay
- MKV/WebM/MOV as primary focus
- “Fix my broken timestamps” heuristics
- Streaming server responsibilities (manifest/ABR logic)

### 0.4 Positioning (how to pitch)
**“Recording‑to‑playback muxer”**: Give encoded frames + timestamps; get a fast‑start MP4 that plays in browsers/VLC/FFmpeg.

---

## 1) Current Built State (as of this chat)

### 1.1 Implemented
- H.264/AVC muxing (Annex B input assumed in roadmap; verify actual implementation)
- AAC audio via ADTS input
- Fast‑start finalize path (`finalize_fast_start()` noted)
- `udta` metadata (title + creation_time noted)
- B‑frames support using `ctts` v1 + `write_video_with_dts()`
- Fragmented MP4 (fMP4) via `FragmentedMuxer`
- Builder pattern API, stats on finish
- Zero dependencies (Cargo.toml)
- Tests: ~22 integration tests

### 1.2 Known limitations
- Single video track only
- H.264 only (no HEVC/VP9/AV1)
- AAC only (no Opus)
- No chapter markers (`chpl`)
- No edit lists (`elst`)
- No subtitles
- No encryption
- 32‑bit chunk offsets only (`stco` only; no `co64`)

---

## 2) Slice‑Gated Doctrine Adapted for Muxide

### 2.1 Slice definition
Each slice must define:
1) **Scope** (what changes)
2) **Non‑goals** (what must not change)
3) **Acceptance criteria** (user‑visible + internal invariants)
4) **Evidence** (tests, fixtures, external validators)
5) **Regression gates** (CI steps)

### 2.2 Golden rules for local AI
- Do **not** expand scope beyond the slice.
- Do **not** introduce non‑std dependencies in `dependencies`.
- Dev‑deps are allowed only for testing slices.
- Every new behavior must come with:
  - deterministic unit/integration test
  - at least one “rejection” test if it’s a precondition
  - ffprobe/mediainfo validation if applicable

---

## 3) Work Breakdown Overview (Top‑Down)

**Slice 0:** Product lock‑in (docs + scope) ✅ COMPLETE (commit 9b7e541)  
**Slice 1:** Input contract enforcement (reject‑not‑fix) ✅ COMPLETE (commit 294d1ab)  
**Slice 2:** Parsing infrastructure (config extraction; minimal header parsing) ✅ COMPLETE (commit 6582232)  
**Slice 3:** Codec expansion (HEVC, Opus, AV1; VP9 optional)  
**Slice 4:** Ship‑grade confidence (property/fuzz/compat CI)  
**Slice 5:** Market surface (examples + docs + HN pitch)  
**Slice 6:** Domination (benchmarks + diagnostics; no swiss‑army expansion)

---

# SLICE 0 — Product Lock‑In (Docs + Guardrails)

## S0.1 Deliverables
- README:
  - invariant one‑liner
  - “Muxide is / is not” table
  - minimal quickstart
  - refusal script / out‑of‑scope policy
- `docs/contract.md`:
  - preconditions (timestamps, keyframe, config)
  - invariants (box correctness, moov order w/ fast‑start)
  - error semantics (reject behavior)
- `docs/charter.md`:
  - zero‑deps philosophy
  - recording‑oriented API
  - compatibility focus (browser/VLC/ffprobe)

## S0.2 Checklist
- [ ] Add invariant sentence verbatim to crate docs (`//!`) and README.
- [ ] Add explicit non‑goals list.
- [ ] Document what inputs are accepted:
  - video: H.264 NALs (clarify Annex B vs AVCC) and keyframe signaling expectations
  - audio: AAC ADTS (clarify if raw AAC frames supported)
  - timestamps: units, timescale mapping, required monotonicity rules
- [ ] Document fast‑start behavior and constraints.
- [ ] Document fMP4 behavior (init segment / media segment policy).

## S0.3 Gates
- [ ] New user can decide in <10 seconds if muxide fits.
- [ ] No ambiguous scope claims.
- [ ] Docs.rs renders without warnings.

---

# SLICE 1 — Input Contract Enforcement (Reject‑Not‑Fix)

## S1.0 Objective
Make the invariant enforceable: **bad inputs are rejected deterministically with actionable errors**.

## S1.1 Timestamp contract (normative rules)
Define these rules precisely in docs and enforce them:

### Video
- **PTS must be non‑decreasing** (or strictly increasing; pick one and codify).
- If B‑frames exist (composition offsets needed):
  - DTS must be provided
  - require `write_video_with_dts()`
- For any sample: **PTS >= DTS**.
- First video sample must be a sync sample (keyframe) unless explicit config allows otherwise.

### Audio
- **Audio PTS must be non‑decreasing**.
- A/V alignment policy must be explicit:
  - either allow audio before video (with edit list later) or reject until edit lists exist.
  - current plan earlier suggested rejecting `AudioBeforeVideo`; decide and freeze.

### Units
- Define what `pts: u64` means (timescale ticks vs nanoseconds vs milliseconds).
- Define track timescale conversions.

## S1.2 Error model (must be educational)
Create/extend `MuxerError` variants for deterministic violations.

### Minimum required variants (example)
- `NonMonotonicPts { track, prev, curr, frame_index, hint }`
- `PtsBeforeDts { pts, dts, frame_index, hint }`
- `BFramesRequireDts { hint }`
- `FirstFrameNotKeyframe { hint }`
- `MissingCodecConfig { needed: [SPS,PPS,(VPS)], hint }`
- `InvalidAdts { hint }`

**Rule:** every error variant used for contract enforcement must include a **hint** string.

## S1.3 Implementation checklist
- [ ] Add timestamp validator layer per track.
- [ ] Store last pts/dts per track; check monotonicity.
- [ ] Detect B‑frame usage:
  - if API provides `dts` path, use that; otherwise if composition offsets are needed, force error.
  - If current implementation can detect reordering from provided pts/dts, then enforce `write_video_with_dts` when pts != dts patterns appear.
- [ ] Enforce first frame keyframe rules.
- [ ] Ensure errors do not require heap allocations beyond formatting; keep std.

## S1.4 Test checklist (must include reject tests)
Add tests that intentionally violate rules:
- [ ] video pts decreases → `NonMonotonicPts`
- [ ] pts < dts → `PtsBeforeDts`
- [ ] B‑frames but using `write_video` only → `BFramesRequireDts`
- [ ] first frame non‑keyframe → `FirstFrameNotKeyframe`
- [ ] missing SPS/PPS before frames → `MissingCodecConfig`
- [ ] invalid ADTS → `InvalidAdts`

## S1.5 Gate
- [ ] All violations fail deterministically.
- [ ] No silent correction.
- [ ] Error messages provide fix guidance.

---

# SLICE 2 — Parsing Infrastructure (Minimal Header Parsing)

## S2.0 Critical clarification
Muxide **must** parse bitstreams to build codec configuration boxes (avcC/hvcC/av1C). This is not “decoding.”

**Goal:** Implement minimal parsing needed for:
- extracting config (SPS/PPS/VPS or Sequence Header OBU)
- keyframe detection from headers

## S2.1 Scope constraints
- No full bitstream decode
- No frame reconstruction
- No deblocking, no entropy decode
- No reordering beyond using provided PTS/DTS

## S2.2 Architecture
Create internal modules:
- `src/codec/mod.rs`
- `src/codec/h264.rs`
- `src/codec/h265.rs`
- `src/codec/av1.rs` (stub until Slice 3)
- (optional) `src/codec/common.rs` for start code scanning

### Suggested internal trait
```rust
trait CodecExtractor {
    type Config;
    fn push_access_unit(&mut self, bytes: &[u8]) -> Result<(), CodecError>;
    fn config(&self) -> Option<&Self::Config>;
    fn is_keyframe(&self, bytes: &[u8]) -> bool;
}
```

**Note:** Keep this private until stabilized.

## S2.3 H.264 specifics
- Input format decision:
  - If Annex B: implement start‑code scanner and NAL splitting.
  - If AVCC: implement length‑prefixed splitting.
  - If both: make it explicit via config.

Minimum parsing required:
- NAL header type (5/7/8)
- Extract SPS (7), PPS (8)
- Build avcC:
  - must include profile/level, lengthSizeMinusOne, SPS/PPS arrays

Keyframe detection:
- IDR slice NAL type 5 indicates keyframe

## S2.4 Gate
- [ ] avcC emitted and validated by ffprobe on produced MP4.
- [ ] Keyframe detection matches existing tests.

---

# SLICE 3 — Codec Expansion (HEVC + Opus + AV1)

## S3.0 Priority rule
Ignore mp4e as “direction.” It’s not a market signal; it’s a reference point at best. Parity is guided by **real user demand** and **compatibility**.

## S3.1 HEVC (H.265) — P0
### Scope
- Add `VideoCodec::H265`
- Accept HEVC Annex B
- Build `hvcC` + sample entry `hvc1` (or `hev1` policy)

### Checklist
- [ ] Implement NAL type parsing:
  - VPS 32, SPS 33, PPS 34
  - IDR types 19/20/21 for sync
- [ ] Extract VPS/SPS/PPS from stream before first sample entry finalization.
- [ ] Build minimal compliant `hvcC`:
  - prefer derivation from SPS where possible
  - if full profile fields are hard, start with constrained baseline (documented) but must still play
- [ ] Write sample entry `hvc1` with `hvcC`.

### Tests
- [ ] Single IDR AU test vector
- [ ] GOP test vector (with B‑frames) requiring DTS path
- [ ] invalid/missing VPS/SPS/PPS rejection
- [ ] ffprobe validates track codec

### Gate
- [ ] Chrome + VLC playback for produced MP4
- [ ] ffprobe prints codec as hevc

## S3.2 Opus — P0
### Scope
- Add `AudioCodec::Opus`
- Implement `Opus` sample entry + `dOps`

### Checklist
- [ ] Decide required input: raw Opus packets + user‑provided packet duration OR infer from TOC.
- [ ] Implement packet duration handling (Opus frame sizes variable).
- [ ] Implement `dOps` fields: channel count, preskip, sample rate, gain, mapping family.
- [ ] Ensure audio timescale consistent (likely 48k).

### Tests
- [ ] mono and stereo packets
- [ ] variable frame duration packets
- [ ] ffprobe shows opus track

### Gate
- [ ] Playback in VLC
- [ ] ffprobe validates sample entry

## S3.3 AV1 — P1
### Scope
- Add `VideoCodec::Av1`
- Parse OBU stream and extract Sequence Header OBU
- Build `av1C` + `av01` sample entry

### Checklist
- [ ] Implement OBU reader (leb128 size fields)
- [ ] Extract seq header OBU for configOBUs
- [ ] Determine keyframe from frame header OBU

### Gate
- [ ] ffprobe sees AV1
- [ ] Playback in Chrome (if supported)

## S3.4 VP9 — P2 (optional)
Only after HEVC/Opus/AV1 stable.

---

# SLICE 4 — Ship‑Grade Confidence (Testing + CI)

## S4.1 Target
Move from “22 tests” to “ship confidence.”

## S4.2 Test categories & required artifacts

### Unit tests
- box writing primitives
- size calculations
- offset tables

### Integration tests
- end‑to‑end mux output
- fast‑start output
- fMP4 init + segment

### Property tests (dev‑dep allowed)
**Goal:** invariants, not random success.
- [ ] box sizes always consistent
- [ ] stco/co64 offsets match actual positions
- [ ] monotonic timestamp invariants

### Fuzz tests
- [ ] no panics on malformed NAL/OBU
- [ ] no OOM / runaway allocations

### Compatibility tests
- [ ] ffprobe parse
- [ ] mediainfo parse (if available in CI)
- [ ] optional: minimal playback smoke (hard in CI)

## S4.3 CI Gates
- [ ] `cargo fmt --check`
- [ ] `cargo clippy -- -D warnings`
- [ ] `cargo test`
- [ ] `cargo test --all-features` (if features exist)
- [ ] ffprobe validation on golden outputs

---

# SLICE 5 — Market Surface (Docs + Examples)

## S5.1 Deliverables
- `examples/simple.rs` (H.264 + AAC)
- `examples/fast_start.rs`
- `examples/fragmented.rs`
- `examples/bframes.rs` (PTS/DTS)
- `examples/audio_only.rs` (if supported)

## S5.2 README: must answer 4 questions
1) What is it?
2) Who is it for?
3) What do I pass in?
4) What do I get out?

## S5.3 HN launch pitch (copy)
“I built a zero‑dependency Rust MP4 muxer for recording apps. Give it encoded frames + timestamps; it outputs a fast‑start MP4 that plays everywhere. No FFmpeg binaries, no GPL, no post‑processing.”

## S5.4 Gate
- [ ] `cargo doc` contains runnable doctests
- [ ] examples compile and run
- [ ] README has a 10‑second decision surface

---

# SLICE 6 — Domination Without Becoming a Swiss‑Army Knife

## S6.1 Allowed expansion axes
- Better diagnostics and hints
- Performance benchmarks (real end‑to‑end)
- Memory profiling
- Strict conformance improvements

## S6.2 Forbidden axes
- decoding/encoding/transcoding
- new containers
- DRM
- manifest generation beyond minimal helpers

---

# 4) Decision Points (Must be Answered Once, Then Frozen)

Local AI should ask and freeze the answers in `docs/contract.md`:

1) **Timestamp units**: are PTS/DTS in:
   - track timescale ticks?
   - nanoseconds?
   - milliseconds?
2) **PTS monotonicity**: strictly increasing or non‑decreasing?
3) **Audio before video**: reject today, or allow with later edit lists?
4) **Input format**: Annex B only, AVCC only, or both?
5) **Keyframe source**: user‑provided `is_keyframe` trusted, or verify via parsing?
   - If both: define which is authoritative.

---

# 5) Evidence Artifacts (Golden Outputs)

Maintain a `tests/assets/` plan:
- `h264_minimal_annexb.h264` (SPS/PPS/IDR)
- `h264_bframes_annexb.h264` (requires DTS)
- `aac_adts_mono.aac`
- `aac_adts_stereo.aac`
- Later:
  - `hevc_minimal_annexb.h265` (VPS/SPS/PPS/IDR)
  - `opus_packets.bin` (document framing)
  - `av1_seqhdr_obu.bin`

For each, store:
- origin (how generated)
- license status (must be safe)
- expected ffprobe summary output

---

# 6) “Local AI Runbook” — How to Execute This Plan

## 6.1 Operating mode
For each slice:
1) Create `docs/slices/SX.md` describing scope + gates
2) Implement code
3) Add tests
4) Run `cargo test` + any external validators
5) Capture evidence (ffprobe output) into `docs/evidence/SX/`

## 6.2 Anti‑theater requirements
- No claim of compliance without:
  - test
  - external validator output (when relevant)
  - reproducible fixture

## 6.3 Definition of Done
A slice is “done” only when:
- all gates pass
- evidence committed
- no extra scope slipped in

---

# 7) Immediate Next Task Pack (Start Here)

## Pack A: Slice 0 completion
- [ ] Update README with invariant + refusal list + quickstart
- [ ] Ensure crate docs (`lib.rs`) contains invariant
- [ ] Promote contract.md to normative

## Pack B: Slice 1 timestamp enforcement
- [ ] Define timestamp units + monotonicity
- [ ] Implement validators + error variants
- [ ] Add 6 rejection tests

## Pack C: Slice 2 H.264 config extraction hardening
- [ ] Confirm input format (Annex B vs AVCC)
- [ ] Ensure SPS/PPS extraction is deterministic
- [ ] Ensure avcC built correctly
- [ ] Validate with ffprobe in tests (if CI allows)

---

# 8) Risk/Assumption Audit (Must Be Tracked)

## Unproven / must validate
- That current output is truly standards‑compliant across players (needs compatibility suite)
- That “zero dependencies” is preserved when adding tests (dev‑deps ok)
- That current parsing is sufficient to build codec config boxes correctly
- That current fast‑start implementation handles offset rewriting correctly for large files

## Key technical risk
- HEVC `hvcC` correctness is non‑trivial; start with minimal playable baseline and expand.

---

# 9) Flagged Claims (Statements an expert can challenge)

- “Fully functional muxer” (needs playback validation and ffprobe evidence)
- “Standards‑compliant” (needs conformance criteria + validators)
- “B‑frame support complete” (needs DTS/CTTS tests + player verification)
- “Fast‑start complete” (needs moov‑before‑mdat + offset correctness evidence)

---

## End State (What Success Looks Like)

Muxide becomes the **default Rust choice** for recording apps to output MP4: modern codecs, strict contract, fast‑start, zero deps, and boring reliability.

