# Muxide 手冊（繁體中文）

> 本手冊為繁體中文版。[English](../README.md) · [简体中文](zh-CN/README.md)

**Muxide** 接收已經正確打時間戳、編碼完成的音視頻幀，並生成符合標準的 MP4 —— **純 Rust、極少外部依賴、無需 FFmpeg。**

Muxide 只做一件事，並且把它做對：

> 接收已編碼、時間戳正確的幀 → 生成**符合標準、可立即播放的 MP4** → 使用**純 Rust**。

---

## Muxide 是什麼

如果你在用 Rust 構建錄製管線，就知道其中的取捨：

| 方案 | 取捨 |
|------|------|
| **FFmpeg 命令列/函式庫** | 外部二進位、GPL 許可顧慮、「這個構建是哪個版本？」 |
| **GStreamer** | 複雜的插件系統、C 依賴、龐大的執行時 |
| **手寫 MP4** | 需要 ISO-BMFF 專業知識（樣本表、交錯、moov 佈局） |
| **「極簡」crate** | 常常缺少 fast-start、嚴格校驗或生產級易用性 |

Muxide 乾淨地解決**這一個任務**：把已編碼幀變成可播放的 MP4。

---

## 安裝與使用

### 作為函式庫

```bash
cargo add muxide
```

```rust
use muxide::api::{MuxerBuilder, VideoCodec};

let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::H264, 1920, 1080, 30.0)
    .build()?;

// 寫入你編碼好的幀……
muxer.write_video(0.0, &h264_frame, true)?;
muxer.finish()?;
```

### 作為命令列工具

```bash
cargo install muxide
muxide mux --video stream.h264 --output output.mp4 --width 1920 --height 1080 --fps 30
muxide mux --video video.h264 --audio audio.aac --output output.mp4
muxide validate --video input.h264 --audio input.aac
muxide info input.mp4
```

---

## 核心契約

Muxide 強制執行嚴格的契約：

| 你的責任 | Muxide 的保證 |
|----------|---------------|
| ✓ 幀已經完成編碼 | ✓ 合法的 ISO-BMFF（MP4） |
| ✓ 時間戳單調遞增 | ✓ 正確的樣本表 |
| ✓ B 幀提供 DTS | ✓ Fast-start 佈局 |
| ✓ 關鍵幀包含 codec 頭 | ✓ 無需後處理 |

如果輸入違反契約，Muxide 會**快速失敗**並給出明確的錯誤——不會靜默損壞，也不會猜測。

---

## 功能特性

| 類別 | 支援 | 說明 |
|------|------|------|
| **視頻** | H.264/AVC | Annex B 格式 |
| | H.265/HEVC | 帶 VPS/SPS/PPS 的 Annex B |
| | AV1 | OBU 流格式 |
| | VP9 | 幀頭解析、解析度/位元深度/色彩配置提取 |
| **音頻** | AAC | 全部 profile：LC、Main、SSR、LTP、HE、HEv2 |
| | Opus | 原始封包，48kHz |
| **容器** | Fast-start | `moov` 位於 `mdat` 之前，適合網頁播放 |
| | B 幀 | 顯式 PTS/DTS 支援 |
| | 分片 MP4 | 用於 DASH/HLS 串流 |
| | 元數據 | 標題、建立時間、語言 |
| **品質** | 詳細錯誤報告 | 十六進位 dump、JSON 輸出、可操作的錯誤訊息 |
| | 生產就緒 | 已驗證與 FFmpeg 相容 |
| | 全面測試 | 200+ 測試、基於屬性的校驗 |
| **綁定** | WASM / Go / C | 可選的 wasm-bindgen 與 C FFI 綁定 |

### 設計原則

| 原則 | 實作 |
|------|------|
| 🦀 **純 Rust 核心** | 可選的 WASM + C FFI 綁定（Go 透過 cgo） |
| 📦 **極少依賴** | 僅必要的 Rust crate，無外部二進位 |
| 🧵 **執行緒安全** | writer 可 `Send + Sync` |
| ✅ **測試充分** | 單元、整合、屬性測試 |
| 📜 **寬鬆許可** | 雙許可：MIT OR Apache-2.0 |

---

## 快速開始（Rust）

```rust
use muxide::api::{AacProfile, AudioCodec, MuxerBuilder, Metadata, VideoCodec};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create("recording.mp4")?;
    let mut muxer = MuxerBuilder::new(file)
        .video(VideoCodec::H264, 1920, 1080, 30.0)
        .audio(AudioCodec::Aac(AacProfile::Lc), 48000, 2)
        .with_metadata(Metadata::new().with_title("My Recording"))
        .with_fast_start(true)
        .build()?;

    // 寫入已編碼幀（來自你的編碼器）
    // muxer.write_video(pts_seconds, h264_annex_b_bytes, is_keyframe)?;
    // muxer.write_audio(pts_seconds, aac_adts_bytes)?;

    let stats = muxer.finish_with_stats()?;
    println!("寫入 {} 幀，{} 位元組", stats.video_frames, stats.bytes_written);
    Ok(())
}
```

---

## 命令列工具

```bash
# 僅視頻
muxide mux --video keyframes.h264 --width 1920 --height 1080 --fps 30 --output recording.mp4

# 視頻 + 音頻 + 元數據
muxide mux --video stream.h264 --audio stream.aac \
  --width 1920 --height 1080 --fps 30 \
  --title "My Recording" --language eng --output final.mp4

# JSON 輸出便於自動化
muxide mux --json [參數...] > stats.json
```

---

## 語言綁定（WASM、Go、C）

Muxide 的核心是純 Rust，但它也提供**可選的**語言綁定，讓你可以從非 Rust 程式碼生成 MP4。所有綁定都構建在同一個複用器之上，生成完全相同、符合標準的 MP4。

### WASM（瀏覽器 / JS）

`wasm` feature 透過 [`wasm-bindgen`](https://crates.io/crates/wasm-bindgen) 把複用器編譯為 WebAssembly。

```bash
cargo build --target wasm32-unknown-unknown --features wasm
# 或使用 wasm-pack：
wasm-pack build --target web --features wasm
```

```js
import { WasmMuxerBuilder, VideoCodec } from "./pkg/muxide.js";

const builder = new WasmMuxerBuilder();
builder.video(VideoCodec.H264, 1280, 720, 30.0);
const muxer = builder.build();

// H.264 首個關鍵幀必須包含 SPS（NAL 7）+ PPS（NAL 8）
muxer.writeVideo(0.0, h264Keyframe, true);

const bytes = muxer.finish(); // 完整 MP4 的 Uint8Array
```

`WasmMuxerBuilder` 還支援 `.audio(...)`，`WasmMuxer` 支援 `writeVideoWithDts`、`writeAudio`、`encodeVideo`、`encodeAudio`。

### Go（透過 C FFI + cgo）

C FFI 層位於 `src/ffi.rs`，配套標頭檔為 `bindings/muxide.h`。`bindings/go` 下的 Go 套件用符合慣例的 API 封裝了它。

```bash
# 1) 建構 C 動態函式庫（cdylib）
cargo rustc --release --lib --crate-type cdylib
# 2) Go 套件連結 target/release/libmuxide
go build ./bindings/go/...
```

```go
package main

import (
    "fmt"
    muxide "github.com/Michael-A-Kuykendall/muxide/bindings/go"
)

func main() {
    m, err := muxide.NewMuxer(muxide.CodecH264, 1280, 720, 30.0)
    if err != nil {
        panic(err)
    }
    defer m.Close() // finalizer 也會釋放句柄

    keyframe := []byte{0x00, 0x00, 0x00, 0x01, 0x67, 0x64, 0x00, 0x1f, /* SPS */
        0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x3c, 0x80, /* PPS */
        0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x00 /* IDR */}
    if err := m.WriteVideo(0.0, keyframe, true); err != nil {
        panic(err)
    }
    out, err := m.Finish()
    if err != nil {
        panic(err)
    }
    fmt.Printf("寫入 %d 位元組 MP4\n", len(out))
}
```

Go API：`NewMuxer` / `NewMuxerWithAudio`、`WriteVideo`、`WriteVideoWithDTS`、`WriteAudio`、`Finish() ([]byte, error)`、`Close()`。Rust 的錯誤透過 `muxide_last_error` 轉為 Go 的 `error` 值。

### C / C++

包含 `bindings/muxide.h` 並連結 cdylib。不透明句柄 `MuxideMuxer` / `MuxideFragmentedMuxer`；所有函式回傳 `int32_t` 錯誤碼（`MUXIDE_OK = 0`，負值為錯誤）。

```c
#include "muxide.h"

MuxideMuxer* m = muxide_new(0 /*H264*/, 1280, 720, 30.0);
muxide_write_video(m, 0.0, keyframe, keyframe_len, 1 /*is_keyframe*/);
muxide_finish(m);
size_t len = muxide_output_length(m);
uint8_t* buf = malloc(len);
muxide_output_copy(m, buf, len);
muxide_free(m);
```

完整 18 個 `extern "C"` 函式、錯誤常數與 `MuxideMetadata` 結構體見 [`bindings/muxide.h`](../../bindings/muxide.h)。

---

## Muxide 不是什麼

Muxide 刻意保持**專注**。它**不**做：

| 不支援 | 原因 |
|--------|------|
| 編碼/解碼 | 使用 `openh264`、`x264`、`rav1e` 等 |
| 轉碼 | 不是 codec 函式庫 |
| 解複用/讀取 MP4 | 設計為只寫 |
| 時間戳修正 | 垃圾進 = 錯誤出 |
| 非 MP4 容器 | 不支援 MKV、WebM、AVI |
| DRM/加密 | 超出範圍 |

**Muxide 是最後一哩路**：編碼器輸出 → 可播放檔案。

---

## 使用場景

- 🎥 **螢幕錄製器** —— 採集 → 編碼 → 複用 → 發布
- 📹 **攝影機應用** —— 網路攝影機/IP 攝影機錄製管線
- 🎬 **視頻編輯器** —— 匯出時間線為 MP4
- 📡 **串流** —— 為 DASH/HLS 生成 fMP4 分段
- 🏭 **嵌入式系統** —— 單一二進位，無外部依賴
- 🔬 **科學應用** —— 確定性、可重現的輸出

---

## 文件

| 資源 | 說明 |
|------|------|
| [📚 API 參考](https://docs.rs/muxide) | 完整 API 文件 |
| [📜 設計章程](../../docs/charter.md) | 架構決策與理由 |
| [📋 API 契約](../../docs/contract.md) | 輸入/輸出保證 |

---

## 常見問題

**為什麼不直接用 FFmpeg？** FFmpeg 很優秀，但有外部二進位依賴、部分構建的 GPL 許可顧慮、程序編排開銷，以及「這個構建帶了什麼 flag？」的除錯難題。Muxide 只需一次 `cargo add`，極少外部依賴。

**Muxide 能編碼視頻嗎？** 不能。Muxide **只做複用（muxing）**。編碼請使用 `openh264`、`rav1e`、`x264`/`x265`。

**我的時間戳錯了會怎樣？** Muxide 會拒絕非單調遞增的時間戳並給出清晰錯誤。它不會嘗試「修復」損壞的輸入——這是設計使然，以保證可預測的輸出。

**Muxide 生產就緒嗎？** 是的。Muxide 擁有廣泛的測試套件（單元、整合、基於屬性的測試），設計目標是可預測、確定的行為。

---

## 許可證

採用以下任一許可：

- Apache License, Version 2.0
- MIT license

由你選擇。

---

## 更新日誌（0.3.0，未發布）—— WASM、Go 與 C 語言綁定

### ✨ 新功能
- **WASM 綁定**（`wasm` feature，`src/wasm.rs`）：透過 wasm-bindgen 把複用器編譯為 WebAssembly。`WasmMuxerBuilder` / `WasmMuxer` 可從 JS/TS 配置音視頻並寫入幀。
- **C FFI 層**（`src/ffi.rs` + `bindings/muxide.h`）：包含 18 個 `extern "C"` 函式、不透明句柄 `MuxideMuxer` / `MuxideFragmentedMuxer` 與錯誤碼常數的穩定 C ABI。可在任何 C-ABI 語言中使用。
- **Go 綁定**（`bindings/go`）：透過 cgo 封裝 C FFI 的符合慣例的 Go 套件，帶 `runtime.SetFinalizer` 句柄管理與 Rust 錯誤傳播。根目錄 `go.mod`（模組 `github.com/Michael-A-Kuykendall/muxide`）讓 `go build ./bindings/go/...` 可從倉庫根解析。
- **CI**：新增 `wasm-check` 作業，在每次 push/PR 時構建 wasm32 目標。

### 🔒 安全
- **將 `rand` 從 0.8.5 升級到 0.8.6**（RUSTSEC-2026-0097，低危）。`rand` 是經由 `proptest` 的傳遞性開發依賴，無需程式碼改動；解決預設分支上的 Dependabot 告警。

### 🧪 測試
- 為新增綁定添加端到端功能測試：Rust FFI 整合測試（`tests/ffi_smoke.rs`）、校驗 MP4 結構的強化版 Go 測試，以及透過 `wasm-bindgen-test-runner` 執行的 wasm-bindgen 測試。
- 將 `MuxideMuxer` / `MuxideFragmentedMuxer` 暴露為 `pub`（不透明句柄），以便外部 crate 持有指標。
- 對非 wasm 的開發依賴做目標條件化，使 `cargo test --target wasm32-unknown-unknown` 可編譯；將 `wasm-bindgen` 對齊到 0.2.108 以匹配測試執行器。

---

<p align="center">
  <em>Muxide 的設計目標是在整合後變得<strong>無感</strong>：可預測、嚴格、快速、隱形。</em>
</p>
