# Changelog

## 0.1.5 (December 29, 2025) - Quality & Completeness

### 🎯 **VP9 Production Readiness**
- **Full-Range Support**: Implemented proper parsing of VP9 color configuration full-range flags, ensuring accurate vpcC boxes for all VP9 streams
- **Metadata Accuracy**: Fixed VP9 muxing to generate correct color space metadata instead of hardcoded defaults

### 🛠️ **API Modernization**
- **Breaking Change**: Removed deprecated `Muxer::new()` and `Muxer::simple()` constructors
- **Unified API**: Standardized all muxer construction through `MuxerBuilder` for consistency and maintainability
- **Migration Path**: Updated all examples and documentation to use the modern API

### 🔍 **Enhanced CLI Diagnostics**
- **Smart Codec Detection**: `muxide info` command now identifies video codecs (H.264, H.265, VP9) and detects audio presence
- **Better MP4 Analysis**: Improved file validation with detailed codec information for troubleshooting

### 📚 **Documentation & Testing**
- **Accuracy First**: Corrected VP9 feature claims in README to match actual implementation capabilities
- **Test Quality**: Replaced placeholder CLI tests with functional validation, ensuring command reliability
- **Roadmap Alignment**: Updated development roadmap to reflect completed VP9 feature parity work

### 🔧 **Under the Hood**
- **Code Quality**: Eliminated deprecated APIs and improved internal consistency
- **Validation API**: Refactored validation functions to use structured config objects for better maintainability
- **Build Cleanliness**: Resolved all compilation warnings for pristine release builds
- **Test Coverage**: Maintained 100% test pass rate across 123+ unit tests and property-based validations

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
