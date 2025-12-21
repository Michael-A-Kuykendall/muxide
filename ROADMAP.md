# Muxide Roadmap

Muxide is a zero-dependency, pure-Rust MP4 muxer.
Its mission is **simple muxing done right**: encoded frames in, playable MP4 out.

## Current Status: v0.1.1 - Advanced Features Complete ✅

### Core Features (v0.1.0)
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

### Advanced Features (v0.1.1)
- ✅ **Comprehensive AAC Support**: All profiles (LC, Main, SSR, LTP, HE, HEv2)
- ✅ **World-Class Error Handling**: Detailed diagnostics, hex dumps, JSON output, actionable suggestions
- ✅ **Metadata Support**: Creation time, language encoding (ISO 639-2/T)
- ✅ **API Compatibility**: Builder pattern with fluent API methods
- ✅ **Production Validation**: FFmpeg/ffprobe compatibility verified
- ✅ **Extensive Testing**: 80+ unit tests, property-based tests, 88% coverage
- ✅ **PPT Framework**: Runtime invariant enforcement with 13 contract tests
- ✅ **CI/CD Integration**: Fast unit tests on every commit, comprehensive property tests on PRs
- ✅ **Real-World Examples**: Working demos with fixture data
- ✅ **CLI Tool**: Command-line interface for immediate developer utility

## Next Goals (v0.2.0) - Developer Experience & Performance

### High Priority
- [ ] **VP9 Video Codec**: Complement AV1 with VP9 support
- [ ] **Performance Benchmarks**: Establish baseline performance metrics

### Medium Priority
- [ ] **SIMD Optimizations**: Performance improvements for hot paths
- [ ] **Enhanced Documentation**: More real-world examples and tutorials
- [ ] **Async I/O Support**: Optional tokio-based async operations

### Lower Priority
- [ ] **Chapter Markers**: Metadata support for navigation points
- [ ] **Streaming Optimizations**: Further improvements for DASH/HLS

## Future Possibilities (v0.3.0+)
- [ ] DASH manifest generation
- [ ] Hardware-accelerated muxing
- [ ] Plugin system for custom codecs
- [ ] Advanced metadata formats (chapters, subtitles)

## Non-Goals
- **Encoding/decoding** - Muxide is a muxer only, bring your own codec
- **Demuxing/parsing** - We write MP4s, not read them
- **Fixing broken input** - Garbage in, error out
- **Feature bloat** - Every feature must justify its complexity

---

## Recent Achievements
- **v0.1.1 Release**: Advanced AAC support, world-class error handling, metadata features
- **Codebase Cleanup**: Removed all external crate references, focused on Muxide's unique value
- **Quality Assurance**: Comprehensive testing suite with real-world validation
- **Developer Experience**: Detailed error messages that make debugging 10x faster

## Governance
- **Lead Maintainer:** Michael A. Kuykendall
- Contributions are welcome via Pull Requests
- The roadmap is set by the lead maintainer to preserve project vision
- All PRs require maintainer review and approval
