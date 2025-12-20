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

## Next Goals (v0.2.0) - Quality Differentiation
- [ ] CLI tool for immediate developer utility
- [ ] VP9 video codec support (complement AV1)
- [ ] Performance benchmarks and SIMD optimizations
- [ ] Enhanced documentation with real-world examples
- [ ] Async I/O support (optional feature flag)
- [ ] Chapter marker metadata support

## Future Possibilities (v0.3.0+)
- [ ] DASH manifest generation
- [ ] Hardware-accelerated muxing
- [ ] Plugin system for custom codecs

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
