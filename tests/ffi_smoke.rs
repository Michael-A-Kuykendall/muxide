//! End-to-end functional test of the C FFI layer, exercised directly (no cgo/Go).
//! Proves the `extern "C"` API in `src/ffi.rs` drives the muxer to a valid MP4.

use muxide::ffi;
use std::ptr;

fn concat(parts: &[&[u8]]) -> Vec<u8> {
    let mut v = Vec::new();
    for p in parts {
        v.extend_from_slice(p);
    }
    v
}

fn read_last_error() -> String {
    let mut buf = [0u8; 1024];
    let n = unsafe { ffi::muxide_last_error(buf.as_mut_ptr() as *mut _, buf.len()) };
    if n == 0 {
        return "(no error message)".to_string();
    }
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

#[test]
fn ffi_muxes_valid_mp4() {
    let sps = [0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x1f];
    let pps = [0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x3c, 0x80];
    let idr = [0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00];
    let slice = [0x00, 0x00, 0x00, 0x01, 0x61, 0x88, 0x84, 0x00];
    let keyframe = concat(&[&sps, &pps, &idr]);
    let pframe = concat(&[&slice]);

    unsafe {
        let h = ffi::muxide_new(0, 1280, 720, 30.0);
        assert!(
            !h.is_null(),
            "muxide_new returned null: {}",
            read_last_error()
        );
        assert_eq!(
            ffi::muxide_write_video(h, 0.0, keyframe.as_ptr(), keyframe.len(), 1),
            0,
            "write keyframe: {}",
            read_last_error()
        );
        assert_eq!(
            ffi::muxide_write_video(h, 1.0 / 30.0, pframe.as_ptr(), pframe.len(), 0),
            0,
            "write pframe: {}",
            read_last_error()
        );
        assert_eq!(ffi::muxide_finish(h), 0, "finish: {}", read_last_error());

        let len = ffi::muxide_output_length(h);
        assert!(len > 0, "empty output");
        let mut out = vec![0u8; len];
        let copied = ffi::muxide_output_copy(h, out.as_mut_ptr(), len);
        assert_eq!(copied, len, "output_copy length mismatch");
        ffi::muxide_free(h);

        assert_eq!(
            &out[4..8],
            b"ftyp",
            "output is not an MP4 (missing ftyp box)"
        );
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("moov"), "MP4 missing moov box");
        assert!(s.contains("mdat"), "MP4 missing mdat box");
    }
}
