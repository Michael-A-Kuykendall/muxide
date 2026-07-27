// Package muxide provides an idiomatic Go wrapper around the Muxide C FFI
// (see bindings/muxide.h, generated from src/ffi.rs).
//
// The Rust cdylib must be built first and available to the linker:
//
//	cargo rustc --release --crate-type cdylib
//
// which produces target/release/libmuxide (or muxide.dll / libmuxide.dylib).
package muxide

/*
#cgo CFLAGS: -I../../bindings
#cgo LDFLAGS: -L../../target/release -lmuxide

#include <stdlib.h>
#include <string.h>
#include "muxide.h"
*/
import "C"

import (
	"errors"
	"runtime"
	"unsafe"
)

// VideoCodec identifies the video compression format for a muxer.
type VideoCodec int

const (
	CodecH264 VideoCodec = 0
	CodecH265 VideoCodec = 1
	CodecAv1  VideoCodec = 2
	CodecVP9  VideoCodec = 3
)

// AudioCodec identifies the audio compression format.
type AudioCodec int

const (
	AudioAACLC   AudioCodec = 0
	AudioAACMain AudioCodec = 1
	AudioAACHe   AudioCodec = 2
	AudioAACHev2 AudioCodec = 3
	AudioOpus    AudioCodec = 4
)

// Muxer wraps a native MuxideMuxer handle. Always release it with Close
// (or rely on the finalizer) to free the underlying memory.
type Muxer struct {
	ptr *C.MuxideMuxer
}

// lastError reads the most recent Rust error message into a Go error.
func lastError() error {
	const n = 1024
	buf := make([]byte, n)
	cnt := C.muxide_last_error((*C.char)(unsafe.Pointer(&buf[0])), C.size_t(len(buf)))
	if cnt == 0 {
		return errors.New("muxide: operation failed")
	}
	return errors.New(string(buf[:cnt]))
}

// NewMuxer creates a muxer for the given video codec and frame parameters.
func NewMuxer(codec VideoCodec, width, height uint32, fps float64) (*Muxer, error) {
	h := C.muxide_new(C.int32_t(codec), C.uint32_t(width), C.uint32_t(height), C.double(fps))
	if h == nil {
		return nil, lastError()
	}
	m := &Muxer{ptr: h}
	runtime.SetFinalizer(m, func(m *Muxer) { _ = m.Close() })
	return m, nil
}

// NewMuxerWithAudio creates a muxer with audio configured up front.
func NewMuxerWithAudio(codec VideoCodec, width, height uint32, fps float64, acodec AudioCodec, sampleRate uint32, channels uint16) (*Muxer, error) {
	h := C.muxide_new_with_audio(
		C.int32_t(codec), C.uint32_t(width), C.uint32_t(height), C.double(fps),
		C.int32_t(acodec), C.uint32_t(sampleRate), C.uint16_t(channels),
	)
	if h == nil {
		return nil, lastError()
	}
	m := &Muxer{ptr: h}
	runtime.SetFinalizer(m, func(m *Muxer) { _ = m.Close() })
	return m, nil
}

// WriteVideo appends a video sample. keyframe must be true for IDR samples.
func (m *Muxer) WriteVideo(pts float64, data []byte, keyframe bool) error {
	if m.ptr == nil {
		return errors.New("muxide: muxer closed")
	}
	var p *C.uint8_t
	if len(data) > 0 {
		p = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	kf := C.uint8_t(0)
	if keyframe {
		kf = 1
	}
	if rc := C.muxide_write_video(m.ptr, C.double(pts), p, C.size_t(len(data)), kf); rc != 0 {
		return lastError()
	}
	return nil
}

// WriteVideoWithDTS appends a video sample with an explicit decode timestamp.
func (m *Muxer) WriteVideoWithDTS(pts, dts float64, data []byte, keyframe bool) error {
	if m.ptr == nil {
		return errors.New("muxide: muxer closed")
	}
	var p *C.uint8_t
	if len(data) > 0 {
		p = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	kf := C.uint8_t(0)
	if keyframe {
		kf = 1
	}
	if rc := C.muxide_write_video_with_dts(m.ptr, C.double(pts), C.double(dts), p, C.size_t(len(data)), kf); rc != 0 {
		return lastError()
	}
	return nil
}

// WriteAudio appends an audio sample.
func (m *Muxer) WriteAudio(pts float64, data []byte) error {
	if m.ptr == nil {
		return errors.New("muxide: muxer closed")
	}
	var p *C.uint8_t
	if len(data) > 0 {
		p = (*C.uint8_t)(unsafe.Pointer(&data[0]))
	}
	if rc := C.muxide_write_audio(m.ptr, C.double(pts), p, C.size_t(len(data))); rc != 0 {
		return lastError()
	}
	return nil
}

// Finish finalizes the mux and returns the complete MP4 byte stream.
func (m *Muxer) Finish() ([]byte, error) {
	if m.ptr == nil {
		return nil, errors.New("muxide: muxer closed")
	}
	if rc := C.muxide_finish(m.ptr); rc != 0 {
		return nil, lastError()
	}
	length := uint64(C.muxide_output_length(m.ptr))
	out := make([]byte, length)
	if length > 0 {
		C.muxide_output_copy(m.ptr, (*C.uint8_t)(unsafe.Pointer(&out[0])), C.size_t(length))
	}
	return out, nil
}

// Close releases the native muxer handle. It is safe to call multiple times.
func (m *Muxer) Close() error {
	if m.ptr == nil {
		return nil
	}
	C.muxide_free(m.ptr)
	m.ptr = nil
	runtime.SetFinalizer(m, nil)
	return nil
}
