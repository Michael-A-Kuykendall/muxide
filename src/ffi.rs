//! C FFI layer for language bindings (Go, Python, etc.).
//!
//! Provides a stable C ABI for creating and using Muxide muxers from other languages.
//! Build as a shared library: `cargo rustc --crate-type cdylib`
//!
//! # Error Handling
//!
//! All functions return `i32` error codes. `0` = success, negative = error.
//! Call [`muxide_last_error`] to retrieve the error message after a failure.
//!
//! # Output Retrieval
//!
//! After calling [`muxide_finish`] or [`muxide_fragmented_init`], use
//! [`muxide_output_length`] and [`muxide_output_copy`] to retrieve the output bytes.

#![allow(private_interfaces)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::os::raw::c_char;
use std::ptr;
use std::slice;

use crate::api::{AacProfile, AudioCodec, MuxerBuilder, VideoCodec};
use crate::fragmented::{FragmentConfig, FragmentedMuxer};

const ERR_OK: i32 = 0;
const ERR_NULL_PTR: i32 = -1;
const ERR_INVALID_CONFIG: i32 = -2;
const ERR_MUXER_ERROR: i32 = -3;
const ERR_ALREADY_FINISHED: i32 = -4;

thread_local! {
    static LAST_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn set_error(msg: String) {
    LAST_ERROR.with(|e| *e.borrow_mut() = msg);
}

pub struct MuxideMuxer {
    inner: Option<crate::api::Muxer<Vec<u8>>>,
    output: Vec<u8>,
    finished: bool,
}

pub struct MuxideFragmentedMuxer {
    inner: FragmentedMuxer,
    output: Vec<u8>,
}

#[repr(C)]
pub struct MuxideMetadata {
    title: *const c_char,
    language: *const c_char,
    creation_time: u64,
    has_creation_time: u8,
}

// --- Lifecycle ---

#[no_mangle]
pub extern "C" fn muxide_new(
    video_codec: i32,
    width: u32,
    height: u32,
    fps: f64,
) -> *mut MuxideMuxer {
    let codec = match video_codec {
        0 => VideoCodec::H264,
        1 => VideoCodec::H265,
        2 => VideoCodec::Av1,
        3 => VideoCodec::Vp9,
        _ => {
            set_error(format!("invalid video codec: {}", video_codec));
            return ptr::null_mut();
        }
    };

    let muxer = match MuxerBuilder::new(Vec::new())
        .video(codec, width, height, fps)
        .build()
    {
        Ok(m) => m,
        Err(e) => {
            set_error(e.to_string());
            return ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(MuxideMuxer {
        inner: Some(muxer),
        output: Vec::new(),
        finished: false,
    }))
}

#[no_mangle]
pub extern "C" fn muxide_add_audio(
    handle: *mut MuxideMuxer,
    audio_codec: i32,
    sample_rate: u32,
    channels: u16,
) -> i32 {
    if handle.is_null() {
        return ERR_NULL_PTR;
    }
    // Audio must be configured at build time, so we rebuild the muxer.
    // This is a limitation — audio should be set before writing frames.
    // For now, return an error if already writing.
    let _ = (audio_codec, sample_rate, channels);
    set_error("audio must be configured via muxide_new_with_audio".to_string());
    ERR_INVALID_CONFIG
}

#[no_mangle]
pub extern "C" fn muxide_new_with_audio(
    video_codec: i32,
    width: u32,
    height: u32,
    fps: f64,
    audio_codec: i32,
    sample_rate: u32,
    channels: u16,
) -> *mut MuxideMuxer {
    let vcodec = match video_codec {
        0 => VideoCodec::H264,
        1 => VideoCodec::H265,
        2 => VideoCodec::Av1,
        3 => VideoCodec::Vp9,
        _ => {
            set_error(format!("invalid video codec: {}", video_codec));
            return ptr::null_mut();
        }
    };

    let acodec = match audio_codec {
        0 => AudioCodec::Aac(AacProfile::Lc),
        1 => AudioCodec::Aac(AacProfile::Main),
        2 => AudioCodec::Aac(AacProfile::He),
        3 => AudioCodec::Aac(AacProfile::Hev2),
        4 => AudioCodec::Opus,
        _ => {
            set_error(format!("invalid audio codec: {}", audio_codec));
            return ptr::null_mut();
        }
    };

    let muxer = match MuxerBuilder::new(Vec::new())
        .video(vcodec, width, height, fps)
        .audio(acodec, sample_rate, channels)
        .build()
    {
        Ok(m) => m,
        Err(e) => {
            set_error(e.to_string());
            return ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(MuxideMuxer {
        inner: Some(muxer),
        output: Vec::new(),
        finished: false,
    }))
}

#[no_mangle]
pub extern "C" fn muxide_free(handle: *mut MuxideMuxer) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

// --- Writing frames ---

#[no_mangle]
pub extern "C" fn muxide_write_video(
    handle: *mut MuxideMuxer,
    pts: f64,
    data: *const u8,
    data_len: usize,
    is_keyframe: u8,
) -> i32 {
    if handle.is_null() || data.is_null() {
        return ERR_NULL_PTR;
    }
    let muxer = unsafe { &mut *handle };
    if muxer.finished {
        return ERR_ALREADY_FINISHED;
    }
    let bytes = unsafe { slice::from_raw_parts(data, data_len) };
    match muxer
        .inner
        .as_mut()
        .unwrap()
        .write_video(pts, bytes, is_keyframe != 0)
    {
        Ok(()) => ERR_OK,
        Err(e) => {
            set_error(e.to_string());
            ERR_MUXER_ERROR
        }
    }
}

#[no_mangle]
pub extern "C" fn muxide_write_video_with_dts(
    handle: *mut MuxideMuxer,
    pts: f64,
    dts: f64,
    data: *const u8,
    data_len: usize,
    is_keyframe: u8,
) -> i32 {
    if handle.is_null() || data.is_null() {
        return ERR_NULL_PTR;
    }
    let muxer = unsafe { &mut *handle };
    if muxer.finished {
        return ERR_ALREADY_FINISHED;
    }
    let bytes = unsafe { slice::from_raw_parts(data, data_len) };
    match muxer
        .inner
        .as_mut()
        .unwrap()
        .write_video_with_dts(pts, dts, bytes, is_keyframe != 0)
    {
        Ok(()) => ERR_OK,
        Err(e) => {
            set_error(e.to_string());
            ERR_MUXER_ERROR
        }
    }
}

#[no_mangle]
pub extern "C" fn muxide_write_audio(
    handle: *mut MuxideMuxer,
    pts: f64,
    data: *const u8,
    data_len: usize,
) -> i32 {
    if handle.is_null() || data.is_null() {
        return ERR_NULL_PTR;
    }
    let muxer = unsafe { &mut *handle };
    if muxer.finished {
        return ERR_ALREADY_FINISHED;
    }
    let bytes = unsafe { slice::from_raw_parts(data, data_len) };
    match muxer.inner.as_mut().unwrap().write_audio(pts, bytes) {
        Ok(()) => ERR_OK,
        Err(e) => {
            set_error(e.to_string());
            ERR_MUXER_ERROR
        }
    }
}

// --- Finalization ---

#[no_mangle]
pub extern "C" fn muxide_finish(handle: *mut MuxideMuxer) -> i32 {
    if handle.is_null() {
        return ERR_NULL_PTR;
    }
    let muxer = unsafe { &mut *handle };
    if muxer.finished {
        return ERR_ALREADY_FINISHED;
    }
    let mut inner = match muxer.inner.take() {
        Some(m) => m,
        None => {
            set_error("muxer already consumed".to_string());
            return ERR_MUXER_ERROR;
        }
    };
    match inner.finish_in_place_with_stats() {
        Ok(_) => {
            muxer.output = inner.into_writer();
            muxer.finished = true;
            ERR_OK
        }
        Err(e) => {
            set_error(e.to_string());
            ERR_MUXER_ERROR
        }
    }
}

#[no_mangle]
pub extern "C" fn muxide_output_length(handle: *const MuxideMuxer) -> usize {
    if handle.is_null() {
        return 0;
    }
    let muxer = unsafe { &*handle };
    muxer.output.len()
}

#[no_mangle]
pub extern "C" fn muxide_output_copy(
    handle: *const MuxideMuxer,
    buf: *mut u8,
    buf_len: usize,
) -> usize {
    if handle.is_null() || buf.is_null() {
        return 0;
    }
    let muxer = unsafe { &*handle };
    let copy_len = muxer.output.len().min(buf_len);
    if copy_len > 0 {
        unsafe {
            ptr::copy_nonoverlapping(muxer.output.as_ptr(), buf, copy_len);
        }
    }
    copy_len
}

// --- Error handling ---

#[no_mangle]
pub extern "C" fn muxide_last_error(buf: *mut c_char, buf_len: usize) -> usize {
    if buf.is_null() || buf_len == 0 {
        return 0;
    }
    LAST_ERROR.with(|e| {
        let msg = e.borrow();
        let bytes = msg.as_bytes();
        let copy_len = bytes.len().min(buf_len - 1);
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
            *buf.add(copy_len) = 0;
        }
        copy_len
    })
}

// --- Fragmented MP4 ---

#[no_mangle]
pub extern "C" fn muxide_fragmented_new(
    video_codec: i32,
    width: u32,
    height: u32,
    sps: *const u8,
    sps_len: usize,
    pps: *const u8,
    pps_len: usize,
) -> *mut MuxideFragmentedMuxer {
    let sps_data = if sps.is_null() || sps_len == 0 {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(sps, sps_len) }.to_vec()
    };
    let pps_data = if pps.is_null() || pps_len == 0 {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(pps, pps_len) }.to_vec()
    };

    let vps = match video_codec {
        1 => Some(Vec::new()), // H.265 needs VPS, but caller must provide via fragmented_new_hevc
        _ => None,
    };

    let config = FragmentConfig {
        width,
        height,
        timescale: 90000,
        fragment_duration_ms: 2000,
        sps: sps_data,
        pps: pps_data,
        vps,
        av1_sequence_header: None,
        vp9_config: None,
    };

    Box::into_raw(Box::new(MuxideFragmentedMuxer {
        inner: FragmentedMuxer::new(config),
        output: Vec::new(),
    }))
}

#[no_mangle]
pub extern "C" fn muxide_fragmented_free(handle: *mut MuxideFragmentedMuxer) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

#[no_mangle]
pub extern "C" fn muxide_fragmented_init(handle: *mut MuxideFragmentedMuxer) -> i32 {
    if handle.is_null() {
        return ERR_NULL_PTR;
    }
    let muxer = unsafe { &mut *handle };
    muxer.output = muxer.inner.init_segment();
    ERR_OK
}

#[no_mangle]
pub extern "C" fn muxide_fragmented_write_video(
    handle: *mut MuxideFragmentedMuxer,
    pts: u64,
    dts: u64,
    data: *const u8,
    data_len: usize,
    is_sync: u8,
) -> i32 {
    if handle.is_null() || data.is_null() {
        return ERR_NULL_PTR;
    }
    let muxer = unsafe { &mut *handle };
    let bytes = unsafe { slice::from_raw_parts(data, data_len) };
    match muxer.inner.write_video(pts, dts, bytes, is_sync != 0) {
        Ok(()) => ERR_OK,
        Err(e) => {
            set_error(e.to_string());
            ERR_MUXER_ERROR
        }
    }
}

#[no_mangle]
pub extern "C" fn muxide_fragmented_flush(handle: *mut MuxideFragmentedMuxer) -> i32 {
    if handle.is_null() {
        return ERR_NULL_PTR;
    }
    let muxer = unsafe { &mut *handle };
    if let Some(segment) = muxer.inner.flush_segment() {
        muxer.output = segment;
        ERR_OK
    } else {
        muxer.output = Vec::new();
        ERR_OK
    }
}

#[no_mangle]
pub extern "C" fn muxide_fragmented_output_length(handle: *const MuxideFragmentedMuxer) -> usize {
    if handle.is_null() {
        return 0;
    }
    let muxer = unsafe { &*handle };
    muxer.output.len()
}

#[no_mangle]
pub extern "C" fn muxide_fragmented_output_copy(
    handle: *const MuxideFragmentedMuxer,
    buf: *mut u8,
    buf_len: usize,
) -> usize {
    if handle.is_null() || buf.is_null() {
        return 0;
    }
    let muxer = unsafe { &*handle };
    let copy_len = muxer.output.len().min(buf_len);
    if copy_len > 0 {
        unsafe {
            ptr::copy_nonoverlapping(muxer.output.as_ptr(), buf, copy_len);
        }
    }
    copy_len
}
