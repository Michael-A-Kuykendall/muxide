# Muxide Roadmap

Muxide is a minimal-dependency, pure-Rust MP4 muxer.
Its mission is **simple muxing done right**: encoded frames in, playable MP4 out.

## Current Status: v0.5.0 ✅

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
- ✅ **Bug fix** (v0.5.0): progressive `tkhd` duration is the track duration in the movie timescale. Fragmented `tkhd` duration stays 0.

### Advanced Features (v0.1.1-0.1.5)
- ✅ **Comprehensive AAC Support**: All profiles (LC, Main, SSR, LTP, HE, HEv2)
- ✅ **Detailed ADTS Error Reporting**: Severity indicators, hex dumps, JSON serialisation
- ✅ **Metadata Support**: Creation time, language encoding (ISO 639-2/T)
- ✅ **API Compatibility**: Builder pattern with fluent API methods
- ✅ **Production Validation**: FFmpeg/ffprobe compatibility verified
- ✅ **Extensive Testing**: 138 unit tests, 10 doc tests, 115 integration tests (263 total)
- ✅ **PPT Framework**: Runtime invariant enforcement with contract tests
- ✅ **Real-World Examples**: Working demos with fixture data
- ✅ **CLI Tool**: Command-line interface with immediate developer utility
- ✅ **VP9 Production Readiness**: Full-range support, accurate color metadata
- ✅ **API Modernization**: Unified MuxerBuilder, removed deprecated constructors
- ✅ **Enhanced CLI Diagnostics**: Smart codec detection, better MP4 analysis

### v0.3.0 — WASM, Go & C Language Bindings
- ✅ **WASM bindings** (`wasm` feature, `src/wasm.rs`): compile the muxer to WebAssembly via wasm-bindgen
- ✅ **C FFI layer** (`src/ffi.rs` + `bindings/muxide.h`): stable C ABI with 18 `extern "C"` functions
- ✅ **Go bindings** (`bindings/go`): idiomatic Go wrapper over the C FFI using cgo
- ✅ **CI**: `wasm-check` job builds the wasm32 target on every push/PR

### v0.4.0 — Audio-Only MP4, Dependency Upgrades & MSRV Bump
- ✅ **Audio-only MP4 and fragmented MP4** (`muxide-9ue.4`): `MuxerBuilder` accepts audio without video (and vice-versa). `FragmentedMuxer::new_audio_only()` writes a single audio `trak`.
- ✅ **Dependency upgrades**: thiserror 2, criterion 0.8, MSRV 1.86

### v0.5.0 — Playback correctness
- ✅ **DASH MPD manifest builder** (`muxide-1uq.4`): `muxide::dash` emits a minimal static MPD
- ✅ **Two-track FragmentedMuxer** (`muxide-9hl.4`): video track 1 and audio track 2. Audio timestamps are samples at the audio sample rate. The audio `mdhd` timescale is that sample rate.
- ✅ **Chapter markers** (`muxide-1uq.3`): `chpl` box in `udta`
- ✅ **Async fMP4 muxer** (`muxide-1uq.2`): `AsyncFragmentedMuxer` behind the `async` feature. `new` returns `Result`.
- ✅ **Multi-frame CLI muxing** (`muxide-bt1`): `.frames` records as well as hex blobs
- ✅ **Audio-only fMP4 track id** (GitHub issue #2): `moof` uses track 1 when the muxer has no video track, and track 2 when a video track exists. Each `trun` `data_offset` points at that track's bytes in `mdat`.
- ✅ **Progressive `tkhd` duration** is the track duration in the movie timescale. Fragmented `tkhd` duration stays 0. Durations above `u32::MAX` use a version-1 box.
- ✅ **AV1 sequence headers** without timing info parse. `decoder_model_info_present_flag` is read only when timing info is present (credit: thoroc, PR #9).
- ✅ **`hvcC` profile, tier, and level** are copied from the SPS in both writers. `lengthSizeMinusOne` stays 3.
- ✅ **Zero durations** (`encode_video` / `encode_audio`) are rejected on that call.
- ✅ **Dimensions** outside `1..=65535` are errors from `MuxerBuilder` and from `FragmentedMuxer::new` / `new_with_audio`. `FragmentedMuxer::new` and `AsyncFragmentedMuxer::new` return `Result`.
- ✅ **CLI**: `--audio` without `--sample-rate` and `--channels` fails and names those flags.
- ✅ **Docs**: MSRV 1.86. `assert_invariant!` panics in release builds. Only the invariant log is debug-only.

## Next Goals

### High Priority
- [ ] **Performance Benchmarks** (`muxide-1uq.6`): Establish baseline performance metrics and optimization targets; commit `bench/BASELINE.md`
- [ ] **CLI DASH manifest** (`muxide-1uq.5`): `--dash-manifest` flag writes MPD beside fMP4 output
- [ ] **Enhanced Documentation**: More real-world examples, tutorials, and API docs

### Medium Priority
- [ ] **Streaming Optimizations**: Further improvements for DASH/HLS low-latency streaming

### Repo Housekeeping (`muxide-0mc`)
- [x] **Sync ROADMAP.md** (`muxide-0mc.1`): v0.4.0 and v0.5.0 match `CHANGELOG.md`

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
- **v0.5.0**: Playback correctness. Audio-only fMP4 track ids, per-track `trun` offsets, progressive `tkhd` duration, AV1 sequence-header parsing, audio sample clock, `hvcC` from SPS, and `FragmentedMuxer::new` returning `Result`. Also DASH MPD, two-track fMP4, chapters, async fMP4, and `.frames` CLI input.
- **v0.4.0**: Audio-only MP4/fMP4, dependency upgrades, MSRV 1.86
- **v0.3.0**: WASM, Go & C language bindings
- **v0.2.6**: Production audit - fixed silent fallback to default AVC config for non-H.264 codecs, removed dead code, fixed clippy warnings, improved CI (removed daily cron, nightly now runs on releases)
- **v0.2.5**: MP4 spec compliance fixes, codec box correctness (vpcC, hvcC, av1C, mp4a), audio/video interop improvements
- **v0.2.3**: Documentation cleanup, dead code removal, and ANSI codes removed from the ADTS error hex dump
- **v0.2.2/v0.2.3**: Correctness fixes, DRY improvements, and API cleanup
- **Codebase Cleanup**: Removed external crate references, focused on core muxing functionality
- **Quality Assurance**: Comprehensive testing with property-based tests and contract verification

## Governance
- **Lead Maintainer:** Michael A. Kuykendall
- Contributions follow the process described in [CONTRIBUTING.md](CONTRIBUTING.md)
- The roadmap is set by the lead maintainer to preserve project vision
