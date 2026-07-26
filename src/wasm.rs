//! WebAssembly bindings for Muxide.
//!
//! Provides a JS-friendly API for muxing MP4 files in the browser.
//! Enable with the `wasm` feature: `cargo build --target wasm32-unknown-unknown --features wasm`
//!
//! # Example (JavaScript)
//!
//! ```js
//! import { WasmMuxerBuilder, VideoCodec } from 'muxide';
//!
//! const builder = new WasmMuxerBuilder();
//! builder.video(VideoCodec.H264, 1920, 1080, 30.0);
//! const muxer = builder.build();
//!
//! muxer.writeVideo(0.0, keyframeBytes, true);
//! muxer.writeVideo(0.033, pFrameBytes, false);
//!
//! const mp4Bytes = muxer.finish();
//! ```

use wasm_bindgen::prelude::*;

use crate::api::{AacProfile, AudioCodec, MuxerBuilder, VideoCodec as CoreVideoCodec};

#[wasm_bindgen]
pub enum VideoCodec {
    H264,
    H265,
    Av1,
    Vp9,
}

impl VideoCodec {
    fn to_core(self) -> CoreVideoCodec {
        match self {
            VideoCodec::H264 => CoreVideoCodec::H264,
            VideoCodec::H265 => CoreVideoCodec::H265,
            VideoCodec::Av1 => CoreVideoCodec::Av1,
            VideoCodec::Vp9 => CoreVideoCodec::Vp9,
        }
    }
}

#[wasm_bindgen]
pub enum AudioCodecKind {
    AacLc,
    AacMain,
    AacHe,
    AacHev2,
    Opus,
}

#[wasm_bindgen]
pub struct WasmMuxerBuilder {
    inner: MuxerBuilder<Vec<u8>>,
}

#[wasm_bindgen]
impl WasmMuxerBuilder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: MuxerBuilder::new(Vec::new()),
        }
    }

    pub fn video(&mut self, codec: VideoCodec, width: u32, height: u32, framerate: f64) {
        let builder = std::mem::replace(&mut self.inner, MuxerBuilder::new(Vec::new()));
        self.inner = builder.video(codec.to_core(), width, height, framerate);
    }

    pub fn audio(&mut self, codec: AudioCodecKind, sample_rate: u32, channels: u16) {
        let ac = match codec {
            AudioCodecKind::AacLc => AudioCodec::Aac(AacProfile::Lc),
            AudioCodecKind::AacMain => AudioCodec::Aac(AacProfile::Main),
            AudioCodecKind::AacHe => AudioCodec::Aac(AacProfile::He),
            AudioCodecKind::AacHev2 => AudioCodec::Aac(AacProfile::Hev2),
            AudioCodecKind::Opus => AudioCodec::Opus,
        };
        let builder = std::mem::replace(&mut self.inner, MuxerBuilder::new(Vec::new()));
        self.inner = builder.audio(ac, sample_rate, channels);
    }

    pub fn build(self) -> Result<WasmMuxer, JsValue> {
        let muxer = self
            .inner
            .build()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(WasmMuxer { inner: Some(muxer) })
    }
}

#[wasm_bindgen]
pub struct WasmMuxer {
    inner: Option<crate::api::Muxer<Vec<u8>>>,
}

#[wasm_bindgen]
impl WasmMuxer {
    pub fn write_video(&mut self, pts: f64, data: &[u8], is_keyframe: bool) -> Result<(), JsValue> {
        self.inner
            .as_mut()
            .ok_or_else(|| JsValue::from_str("muxer already finished"))?
            .write_video(pts, data, is_keyframe)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn write_video_with_dts(
        &mut self,
        pts: f64,
        dts: f64,
        data: &[u8],
        is_keyframe: bool,
    ) -> Result<(), JsValue> {
        self.inner
            .as_mut()
            .ok_or_else(|| JsValue::from_str("muxer already finished"))?
            .write_video_with_dts(pts, dts, data, is_keyframe)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn write_audio(&mut self, pts: f64, data: &[u8]) -> Result<(), JsValue> {
        self.inner
            .as_mut()
            .ok_or_else(|| JsValue::from_str("muxer already finished"))?
            .write_audio(pts, data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn encode_video(&mut self, data: &[u8], duration_ms: u32) -> Result<(), JsValue> {
        self.inner
            .as_mut()
            .ok_or_else(|| JsValue::from_str("muxer already finished"))?
            .encode_video(data, duration_ms)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn encode_audio(&mut self, data: &[u8], samples: u32) -> Result<(), JsValue> {
        self.inner
            .as_mut()
            .ok_or_else(|| JsValue::from_str("muxer already finished"))?
            .encode_audio(data, samples)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Finalize the MP4 and return the output bytes.
    pub fn finish(&mut self) -> Result<Vec<u8>, JsValue> {
        let muxer = self
            .inner
            .take()
            .ok_or_else(|| JsValue::from_str("muxer already finished"))?;
        let mut muxer = muxer;
        muxer
            .finish_in_place_with_stats()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(muxer.into_writer())
    }
}
