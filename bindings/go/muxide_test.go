package muxide

import (
	"bytes"
	"testing"
)

func TestNewMuxerWriteFinish(t *testing.T) {
	m, err := NewMuxer(CodecH264, 1280, 720, 30.0)
	if err != nil {
		t.Fatalf("NewMuxer: %v", err)
	}
	defer func() {
		if err := m.Close(); err != nil {
			t.Errorf("Close: %v", err)
		}
	}()

	// Valid H.264 keyframe: SPS (NAL 7) + PPS (NAL 8) + IDR (NAL 5).
	sps := []byte{0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x1f}
	pps := []byte{0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x3c, 0x80}
	idr := []byte{0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00}
	keyframe := append(append(append([]byte{}, sps...), pps...), idr...)
	if err := m.WriteVideo(0.0, keyframe, true); err != nil {
		t.Fatalf("WriteVideo: %v", err)
	}
	slice := []byte{0x00, 0x00, 0x00, 0x01, 0x61, 0x88, 0x84, 0x00}
	if err := m.WriteVideo(1.0/30.0, slice, false); err != nil {
		t.Fatalf("WriteVideo#2: %v", err)
	}

	out, err := m.Finish()
	if err != nil {
		t.Fatalf("Finish: %v", err)
	}
	if len(out) == 0 {
		t.Fatal("expected non-empty MP4 output")
	}
	if string(out[4:8]) != "ftyp" {
		t.Fatal("output is not an MP4 (missing ftyp box)")
	}
	if !bytes.Contains(out, []byte("moov")) || !bytes.Contains(out, []byte("mdat")) {
		t.Fatal("output is not a valid MP4 (missing moov or mdat box)")
	}
}

func TestCloseIsIdempotent(t *testing.T) {
	m, err := NewMuxer(CodecVP9, 640, 480, 24.0)
	if err != nil {
		t.Fatalf("NewMuxer: %v", err)
	}
	if err := m.Close(); err != nil {
		t.Fatalf("first Close: %v", err)
	}
	if err := m.Close(); err != nil {
		t.Fatalf("second Close: %v", err)
	}
}
