# Muxide Feature Parity Plan: Dominating MP4E

## Current Status (Post-41-Second Sprint + AAC Testing)
✅ **Implemented:** Simple API (`Muxer::simple`, `encode_video`, `encode_audio`, keyframe detection, PTS tracking)
✅ **Extended AAC Support:** Full AAC profile support (LC, Main, SSR, LTP, HE, HEv2) with runtime invariants
✅ **Tested:** 80/80 tests pass + AAC-specific functional tests + contract tests verifying invariants
✅ **Verified:** No regressions, backward compatibility maintained, PPT invariants actively enforced

## MP4E Feature Analysis

### MP4E's Core Features
1. **Simple API**: `new()`, `new_with_fragment()`, `set_video_track()`, `set_audio_track()`, `encode_video()`, `encode_audio()`, `flush()`
2. **Codec Support**: H.264/AVC, H.265/HEVC, AAC (LC/Main/SSR/LTP/HE-AAC/HE-AAC-v2), Opus
3. **Metadata**: Creation time (`set_create_time()`), Language (`set_language()`)
4. **Fragmented MP4**: `new_with_fragment()` for fMP4
5. **Basic Error Handling**: Simple error types

### MP4E's Limitations
- No AV1 support
- Basic error messages
- No B-frame support
- No property-based testing
- Limited AAC variants in practice (only LC works well)

## Muxide's Current Advantages
- ✅ AV1 support
- ✅ B-frame support (DTS/PTS)
- ✅ Educational error messages
- ✅ 86% test coverage + property tests
- ✅ Fragmented MP4
- ✅ Fast-start MP4
- ✅ Builder pattern for advanced config

## Feature Parity Roadmap (Fibonacci Points)

### Phase 1: API Parity (8 points total)
**Goal:** Match MP4E's API surface exactly

1. **✅ Extended AAC Support** (3 points)
   - Expand `AudioCodec` enum: `Aac(AacProfile)` with LC, Main, SSR, LTP, HE, HEv2
   - Update ADTS parsing in `codec/aac.rs`
   - Test with different AAC profiles

2. **Metadata Enhancements** (3 points)
   - Add `creation_time` and `language` to `Metadata` struct
   - Add `set_create_time()` and `set_language()` methods
   - Update MP4 box writing

3. **API Method Aliases** (2 points)
   - Add `new()` and `new_with_fragment()` as aliases to builder
   - Add `set_video_track()` and `set_audio_track()` methods
   - Ensure `flush()` works (already exists as `finish()`)

### Phase 2: Quality Differentiation (11 points total)
**Goal:** Exceed MP4E in reliability and features

4. **Enhanced Error Messages** (3 points)
   - Improve error context for AAC parsing failures
   - Add byte offset information where possible

5. **Performance Optimizations** (5 points)
   - SIMD for box writing (if applicable)
   - Memory usage profiling

6. **Extended Testing** (3 points)
   - Add tests for all AAC profiles
   - Integration tests with real MP4E-style usage patterns

### Phase 3: Market Domination (12 points total)
**Goal:** Build tools and ecosystem

7. **CLI Tool** (8 points)
   - Create `muxide-cli` binary
   - Support basic muxing from files
   - MP4E-compatible command line interface

8. **Documentation Overhaul** (2 points)
   - Add MP4E migration guide
   - Real-world examples (screen recording, webcam)
   - Performance benchmarks

9. **VP9 Support** (5 points)
   - Add VP9 codec parsing
   - Update enums and boxes

## Implementation Priority

**High Priority (Do Next):**
1. Extended AAC support (3 points) - Closes the biggest gap
2. Metadata enhancements (3 points) - Easy win
3. API aliases (2 points) - Makes migration trivial

**Medium Priority:**
4. CLI tool (8 points) - Immediate utility for users
5. Enhanced errors (3 points) - Differentiates quality

**Low Priority:**
6. VP9 support (5 points) - Nice to have
7. Performance opts (5 points) - Optimization phase

## Success Metrics

- **API Parity:** 100% MP4E API compatibility
- **Test Coverage:** Maintain 86%+ coverage
- **Downloads:** Surpass MP4E's 2.5k within 6 months
- **User Feedback:** "Easier to use than MP4E with better errors"

## Risk Assessment

- **Low Risk:** AAC extensions, metadata, API aliases (existing infrastructure)
- **Medium Risk:** CLI tool (new binary, but simple)
- **High Risk:** Performance opts (could introduce bugs)

## Total Effort Estimate

**31 story points** across 9 tasks (realistic for 1-2 week sprint with proper velocity)

Ready to start with Phase 1? Let's tackle extended AAC support (3 points) first.