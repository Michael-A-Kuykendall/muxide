# CrabCamera Alignment Plan (Muxide v0.1.0)

This document captures the delta between the current v0.1.0 slice ladder scope and the additional requirements needed for CrabCamera integration.

It is intentionally **planning-only**: items here are either explicitly pulled into the v0.1.0 slice ladder (and will be implemented) or explicitly deferred as post-v0.1.0 follow-ups.

## Source of Truth

- CrabCamera-facing requirements: `docs/crabcamera_requirements.md`
- Muxide product scope: `docs/charter.md`
- Muxide API contract: `docs/contract.md`
- Execution plan: `slice_ladder.md`

## Current State Summary

Muxide already:
- Accepts H.264 Annex B frames with `pts: f64` and `is_keyframe`.
- Enforces first-frame keyframe and SPS/PPS presence.
- Writes MP4 with `ftyp`/`mdat`/`moov` and a keyframe index (`stss`).
- Supports optional AAC-in-ADTS audio.

## v0.1.0 Must-Haves for CrabCamera

These items are required to claim that Muxide v0.1.0 is “CrabCamera-ready”:

1. **90 kHz media timebase** and drift control
   - Rationale: CrabCamera `pts = frame_number / framerate` and long recordings must not accumulate rounding drift.
   - Implementation: Use a 90 kHz per-track media timescale for `mdhd` and `stts` deltas; ensure conversion is stable for common rates.

2. **Dynamic `avcC` SPS/PPS from the actual stream**
   - Rationale: Hard-coded SPS/PPS is unsafe with real openh264 settings; mismatches can break playback.
   - Implementation: Extract SPS/PPS from the first keyframe and write those into `avcC`.

3. **CrabCamera-friendly API + stats (non-breaking additions)**
   - Rationale: CrabCamera wants a config-driven constructor and finish stats.
   - Implementation: Add a convenience constructor and a stats-returning finish variant while keeping the existing builder API.

These are pulled into the slice ladder as Slices 09–11.

## Deferred Follow-ups (Post-v0.1.0)

These are useful but not required to satisfy CrabCamera’s v0.5.0 needs:

- Fast-start MP4 (`moov` at front)
- Player-matrix automation (QuickTime/VLC/WMP/Chrome) beyond documented manual validation
- Handling mid-stream SPS/PPS changes
- Fragmented MP4
- B-frames / reordering

## Acceptance Philosophy

- Keep slice gates objective and runnable via `cargo test --test <gate>`.
- Prefer deterministic fixtures and structural validations over byte-for-byte golden comparison when changes affect global headers/timebases.
