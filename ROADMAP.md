# Muxide Roadmap

Muxide is a zero-dependency, pure-Rust MP4 muxer.
Its mission is **simple muxing done right**: encoded frames in, playable MP4 out.

## Current Milestones
- ✅ H.264/AVC video muxing (Annex B format)
- ✅ H.265/HEVC video muxing with VPS/SPS/PPS extraction
- ✅ AV1 video muxing (OBU format)
- ✅ AAC audio muxing (ADTS format)
- ✅ Opus audio muxing (48kHz raw packets)
- ✅ Fast-start layout (moov before mdat)
- ✅ Fragmented MP4 for DASH/HLS streaming
- ✅ B-frame support via explicit PTS/DTS
- ✅ Property-based test suite
- ✅ Published to crates.io

## Next Goals (v0.2.0)
- [ ] Subtitle track support (WebVTT/TTML in mp4)
- [ ] Chapter markers for long recordings
- [ ] Multiple video track support
- [ ] Improved error messages with byte offsets
- [ ] `no_std` support (optional, behind feature flag)

## Future Possibilities (v0.3.0+)
- [ ] VP9 video codec support
- [ ] FLAC audio codec support
- [ ] Edit lists for gapless audio
- [ ] Encryption support (CENC/CBCS)
- [ ] Muxer statistics and diagnostics API

## Non-Goals
- **Encoding/decoding** - Muxide is a muxer only, bring your own codec
- **Demuxing/parsing** - We write MP4s, not read them
- **Fixing broken input** - Garbage in, error out
- **Feature bloat** - Every feature must justify its complexity

---

## Governance
- **Lead Maintainer:** Michael A. Kuykendall
- Contributions are welcome via Pull Requests
- The roadmap is set by the lead maintainer to preserve project vision
- All PRs require maintainer review and approval
