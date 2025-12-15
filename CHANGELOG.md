# Changelog

## 0.1.0

- MP4 writer with a single H.264 video track (Annex B input).
- Optional AAC audio track (ADTS input).
- 90 kHz media timebase for track timing.
- Dynamic `avcC` configuration derived from SPS/PPS in the first keyframe.
- Deterministic finalisation with explicit errors on double-finish and post-finish writes.
- Specific `MuxerError` variants for common failure modes.
- Convenience API: `Muxer::new(writer, MuxerConfig)`.
- Finish statistics: `finish_with_stats` / `finish_in_place_with_stats`.
