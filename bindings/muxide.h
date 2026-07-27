#ifndef MUXIDE_H
#define MUXIDE_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque handles. The concrete layout is private to the Rust cdylib. */
typedef struct MuxideMuxer MuxideMuxer;
typedef struct MuxideFragmentedMuxer MuxideFragmentedMuxer;

/* Error codes. Must match the constants in src/ffi.rs exactly.
 * All functions return i32: 0 = success, negative = error. */
#define MUXIDE_OK 0
#define MUXIDE_ERR_NULL -1
#define MUXIDE_ERR_INVALID_CONFIG -2
#define MUXIDE_ERR_MUXER -3
#define MUXIDE_ERR_ALREADY_FINISHED -4

/* Metadata mirror of the #[repr(C)] MuxideMetadata struct in src/ffi.rs. */
typedef struct MuxideMetadata {
    const char* title;
    const char* language;
    uint64_t creation_time;
    uint8_t has_creation_time;
} MuxideMetadata;

/* --- Muxer lifecycle --- */
MuxideMuxer* muxide_new(int32_t video_codec, uint32_t width, uint32_t height, double fps);
int32_t muxide_add_audio(MuxideMuxer* handle, int32_t audio_codec, uint32_t sample_rate, uint16_t channels);
MuxideMuxer* muxide_new_with_audio(int32_t video_codec, uint32_t width, uint32_t height, double fps,
                                   int32_t audio_codec, uint32_t sample_rate, uint16_t channels);
void muxide_free(MuxideMuxer* handle);

/* --- Writing frames --- */
int32_t muxide_write_video(MuxideMuxer* handle, double pts, const uint8_t* data, size_t data_len, uint8_t is_keyframe);
int32_t muxide_write_video_with_dts(MuxideMuxer* handle, double pts, double dts, const uint8_t* data, size_t data_len, uint8_t is_keyframe);
int32_t muxide_write_audio(MuxideMuxer* handle, double pts, const uint8_t* data, size_t data_len);

/* --- Finalization --- */
int32_t muxide_finish(MuxideMuxer* handle);
size_t muxide_output_length(const MuxideMuxer* handle);
size_t muxide_output_copy(const MuxideMuxer* handle, uint8_t* buf, size_t buf_len);

/* --- Error handling --- */
size_t muxide_last_error(char* buf, size_t buf_len);

/* --- Fragmented MP4 --- */
MuxideFragmentedMuxer* muxide_fragmented_new(int32_t video_codec, uint32_t width, uint32_t height,
                                              const uint8_t* sps, size_t sps_len,
                                              const uint8_t* pps, size_t pps_len);
void muxide_fragmented_free(MuxideFragmentedMuxer* handle);
int32_t muxide_fragmented_init(MuxideFragmentedMuxer* handle);
int32_t muxide_fragmented_write_video(MuxideFragmentedMuxer* handle, uint64_t pts, uint64_t dts,
                                      const uint8_t* data, size_t data_len, uint8_t is_sync);
int32_t muxide_fragmented_flush(MuxideFragmentedMuxer* handle);
size_t muxide_fragmented_output_length(const MuxideFragmentedMuxer* handle);
size_t muxide_fragmented_output_copy(const MuxideFragmentedMuxer* handle, uint8_t* buf, size_t buf_len);

#ifdef __cplusplus
}
#endif

#endif /* MUXIDE_H */
