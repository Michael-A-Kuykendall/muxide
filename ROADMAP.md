# Muxide Roadmap

Muxide is a minimal-dependency, pure-Rust MP4 muxer.
Its mission is **simple muxing done right**: encoded frames in, playable MP4 out.

## Current Status: v0.2.5 ✅

### Core Features (v0.1.0 – v0.2.1)
- ✅ H.264/AVC video muxing (Annex B format)
- ✅ H.265/HEVC video muxing with VPS/SPS/PPS extraction
- ✅ AV1 video muxing (OBU format)
- ✅ VP9 video muxing (complete - frame header parsing, resolution/bit-depth extraction)
- ✅ AAC audio muxing (ADTS format)
- ✅ Opus audio muxing (48kHz raw packets)
- ✅ Fast-start layout (moov before mdat)
- ✅ **Fragmented MP4 for DASH/HLS streaming** (H.264, H.265, AV1, VP9 — library API; H.264 and VP9 via CLI)
- ✅ B-frame support via explicit PTS/DTS
- ✅ Property-based test suite
- ✅ Published to crates.io
- ✅ **Bug fix**: `tkhd` version-0 duration was written as `u64` instead of `u32`, corrupting width/height in non-fragmented MP4 (issue #5)
- ✅ **Bug fix**: `tkhd` track duration field was always 0; now threads actual track duration (in movie timescale) into both video and audio `tkhd` boxes

### Advanced Features (v0.1.1-0.1.5)
- ✅ **Comprehensive AAC Support**: All profiles (LC, Main, SSR, LTP, HE, HEv2)
- ✅ **Detailed ADTS Error Reporting**: Severity indicators, hex dumps, JSON serialisation
- ✅ **Metadata Support**: Creation time, language encoding (ISO 639-2/T)
- ✅ **API Compatibility**: Builder pattern with fluent API methods
- ✅ **Production Validation**: FFmpeg/ffprobe compatibility verified
- ✅ **Extensive Testing**: 123 unit and integration tests, property-based tests
- ✅ **PPT Framework**: Runtime invariant enforcement with contract tests
- ✅ **Real-World Examples**: Working demos with fixture data
- ✅ **CLI Tool**: Command-line interface with immediate developer utility
- ✅ **VP9 Production Readiness**: Full-range support, accurate color metadata
- ✅ **API Modernization**: Unified MuxerBuilder, removed deprecated constructors
- ✅ **Enhanced CLI Diagnostics**: Smart codec detection, better MP4 analysis

## Next Goals (v0.3.0+)

### High Priority
- [ ] **Audio-only MP4/fMP4**: Support muxing audio tracks without a video track (currently `MissingVideoConfig` error)
- [ ] **Performance Benchmarks**: Establish baseline performance metrics and optimization targets
- [ ] **Enhanced Documentation**: More real-world examples, tutorials, and API docs
- [ ] **Async I/O Support**: Optional tokio-based async operations for large file handling
- [ ] **WebAssembly Target**: Browser-based MP4 muxing for web applications
- [ ] **CLI H.265/AV1 Fragmented MP4**: Expose VPS/SPS/PPS/sequence-header params in CLI for HEVC and AV1 fragmented output

### Medium Priority
- [ ] **Chapter Markers**: Metadata support for navigation points in long videos
- [ ] **DASH Manifest Generation**: Automatic streaming manifest creation alongside fMP4 output
- [ ] **Streaming Optimizations**: Further improvements for DASH/HLS low-latency streaming

## Future Possibilities
- [ ] **Hardware-accelerated Muxing**: GPU-assisted frame processing and I/O
- [ ] **Plugin System**: Extensible architecture for custom codecs and formats
- [ ] **Advanced Metadata**: Subtitles, custom metadata formats
- [ ] **Real-time Streaming**: RTMP/RTSP output for live broadcasting
- [ ] **Container Extensions**: Support for MKV, AVI, MOV formats

## Non-Goals
- **Encoding/decoding** - Muxide is a muxer only, bring your own codec
- **Demuxing/parsing** - We write MP4s, not read them
- **Fixing broken input** - Garbage in, error out
- **Feature bloat** - Every feature must justify its complexity

---

## Recent Achievements
- **v0.2.3**: Documentation cleanup, dead code removal, and ANSI codes removed from the ADTS error hex dump
- **v0.2.2/v0.2.3**: Correctness fixes, DRY improvements, and API cleanup
- **Codebase Cleanup**: Removed external crate references, focused on core muxing functionality
- **Quality Assurance**: Comprehensive testing with property-based tests and contract verification

## Governance
- **Lead Maintainer:** Michael A. Kuykendall
- Contributions follow the process described in [CONTRIBUTING.md](CONTRIBUTING.md)
- The roadmap is set by the lead maintainer to preserve project vision
