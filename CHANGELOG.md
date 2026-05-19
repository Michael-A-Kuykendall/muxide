# Changelog

## 0.2.1 (May 19, 2026) - Bug Fix & CLI Fragmented MP4

### 🐛 **Critical Bug Fix**
- **`tkhd` duration overflow**: The non-fragmented muxer was writing the `duration` field in version-0 Track Header boxes as `u64` (8 bytes) instead of `u32` (4 bytes) per ISO 14496-12 §8.3.2. This inserted 4 extra bytes into every `tkhd` box, shifting the matrix, width, and height fields to wrong offsets. Players would infer an incorrect Sample Aspect Ratio (e.g. `24:5` instead of `1:1`), causing stretched or squished video playback. The fragmented path was already correct; this aligns the non-fragmented path to match. Reported by @peteralm80, confirmed by @zkvsky (issue #5).

### ✨ **CLI Fragmented MP4 (H.264 + VP9)**
- The `--fragmented` flag in the `mux` subcommand now works for H.264 and VP9 inputs. The CLI extracts SPS/PPS automatically from the Annex B bitstream for H.264 and VP9 config from the first keyframe for VP9, then writes a spec-correct `ftyp + moov + moof + mdat` fragmented MP4. H.265 and AV1 fragmented output require additional parameter-set arguments not yet exposed in the CLI; those will continue to use the library API directly.

### 🗺️ **Roadmap Cleanup**
- Removed duplicate sections and speculative AI-generated items ("Quantum-Safe Metadata", "Holographic Video Support", "Blockchain-Integrated Provenance", "Neural Codec Interfaces") that had no grounding in the project's mission.


### 🎬 **Fragmented MP4 (fMP4) Multi-Codec Support**
- **Multi-Codec fMP4**: Fragmented MP4 init+media segment support for **H.264, H.265/HEVC, AV1, and VP9**
- **HEVC `hvcC` Correctness**: Config box structure aligns with emitted parameter set arrays; signals 4-byte NAL length prefixes
- **Explicit Sample Contract**: Fragmented video samples are **MP4 length-prefixed** (4-byte NAL length prefixes), not Annex B start codes

### 🧱 **Safety & Contract Enforcement**
- **Monotonic DTS Enforcement**: Fragmented muxer rejects decreasing DTS instead of underflowing
- **Timestamp Validation**: Rejects non-finite timestamps (NaN/Inf) for video PTS/DTS and audio PTS with specific error variants
- **MP4 Size Overflow Hardening**: Prevents `mdat` size/offset overflow with deterministic errors

### 🔍 **Interop / Conformance Scope**
- **Interop Note**: H.264 outputs were spot-checked with FFmpeg/ffprobe (non-fragmented MP4 and concatenated fMP4 init+segment); broader player/device conformance validation is ongoing.

## 0.1.5 (December 30, 2025) - Quality & Completeness

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
