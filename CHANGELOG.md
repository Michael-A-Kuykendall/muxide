# Changelog

## 0.1.4

- **Ecosystem Integration**: Added "Used By CrabCamera" section in README to highlight integration with the popular desktop camera plugin
- **Cross-Promotion**: Enhanced documentation to showcase real-world usage in production applications

## 0.1.3

- **Validation Features**: Added comprehensive MP4 validation with `validate()` method and CLI command
- **Error Recovery**: Enhanced error handling with detailed diagnostics and recovery suggestions
- **CLI Enhancements**: Improved command-line interface with validation and info commands
- **Production Polish**: Final optimizations and testing for production deployment

## 0.1.2

- **CLI Tool**: Complete command-line interface with progress bars, JSON output, and comprehensive muxing options
- **Code Quality**: Comprehensive AI artifact cleanup, improved error handling patterns, and clippy compliance
- **Documentation**: Enhanced README with professional presentation and complete feature documentation
- **Release Polish**: Final production-ready codebase with all warnings addressed and comprehensive testing

## 0.1.1

- **AAC Profile Support**: Complete implementation of all 6 AAC profiles (LC, Main, SSR, LTP, HE, HEv2)
- **World-Class Error Handling**: Comprehensive ADTS validation with detailed diagnostics, hex dumps, and recovery suggestions
- **MP4E-Compatible APIs**: Added `new_with_fragment()`, `flush()`, `set_create_time()`, `set_language()` methods
- **Metadata Support**: Title, creation time, and language metadata in MP4 files
- **HEVC/H.265 Support**: Annex B format with VPS/SPS/PPS configuration
- **AV1 Support**: OBU stream format with Sequence Header OBU configuration
- **Opus Support**: Raw Opus packets with 48kHz sample rate
- **CLI Tool**: Command-line interface with progress bars, JSON output, and comprehensive options
- **Invariant PPT Framework**: Property-based testing with 86%+ code coverage
- **Documentation**: Complete README, governance files (CODE_OF_CONDUCT, CONTRIBUTING, etc.), and roadmap
- **License**: Simplified to MIT-only

## 0.1.0

- MP4 writer with a single H.264 video track (Annex B input).
- Optional AAC audio track (ADTS input).
- 90 kHz media timebase for track timing.
- Dynamic `avcC` configuration derived from SPS/PPS in the first keyframe.
- Deterministic finalisation with explicit errors on double-finish and post-finish writes.
- Specific `MuxerError` variants for common failure modes.
- Convenience API: `Muxer::new(writer, MuxerConfig)`.
- Finish statistics: `finish_with_stats` / `finish_in_place_with_stats`.
