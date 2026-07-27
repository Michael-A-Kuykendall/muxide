# Muxide 手册（简体中文）

> 本手册为简体中文版。[English](../README.md) · [繁體中文](zh-TW/README.md)

**Muxide** 接收已经正确打时间戳、编码完成的音视频帧，并生成符合标准的 MP4 —— **纯 Rust、极少外部依赖、无需 FFmpeg。**

Muxide 只做一件事，并且把它做对：

> 接收已编码、时间戳正确的帧 → 生成**符合标准、可立即播放的 MP4** → 使用**纯 Rust**。

---

## Muxide 是什么

如果你在用 Rust 构建录制管线，就知道其中的取舍：

| 方案 | 取舍 |
|------|------|
| **FFmpeg 命令行/库** | 外部二进制、GPL 许可顾虑、“这个构建是哪个版本？” |
| **GStreamer** | 复杂的插件系统、C 依赖、庞大的运行时 |
| **手写 MP4** | 需要 ISO-BMFF 专业知识（样本表、交错、moov 布局） |
| **“极简” crate** | 常常缺少 fast-start、严格校验或生产级易用性 |

Muxide 干净地解决**这一个任务**：把已编码帧变成可播放的 MP4。

---

## 安装与使用

### 作为库

```bash
cargo add muxide
```

```rust
use muxide::api::{MuxerBuilder, VideoCodec};

let mut muxer = MuxerBuilder::new(file)
    .video(VideoCodec::H264, 1920, 1080, 30.0)
    .build()?;

// 写入你编码好的帧……
muxer.write_video(0.0, &h264_frame, true)?;
muxer.finish()?;
```

### 作为命令行工具

```bash
cargo install muxide
muxide mux --video stream.h264 --output output.mp4 --width 1920 --height 1080 --fps 30
muxide mux --video video.h264 --audio audio.aac --output output.mp4
muxide validate --video input.h264 --audio input.aac
muxide info input.mp4
```

---

## 核心约束

Muxide 强制执行严格的契约：

| 你的责任 | Muxide 的保证 |
|----------|---------------|
| ✓ 帧已经完成编码 | ✓ 合法的 ISO-BMFF（MP4） |
| ✓ 时间戳单调递增 | ✓ 正确的样本表 |
| ✓ B 帧提供 DTS | ✓ Fast-start 布局 |
| ✓ 关键帧包含 codec 头 | ✓ 无需后处理 |

如果输入违反契约，Muxide 会**快速失败**并给出明确的错误——不会静默损坏，也不会猜测。

---

## 功能特性

| 类别 | 支持 | 说明 |
|------|------|------|
| **视频** | H.264/AVC | Annex B 格式 |
| | H.265/HEVC | 带 VPS/SPS/PPS 的 Annex B |
| | AV1 | OBU 流格式 |
| | VP9 | 帧头解析、分辨率/位深/色彩配置提取 |
| **音频** | AAC | 全部 profile：LC、Main、SSR、LTP、HE、HEv2 |
| | Opus | 原始包，48kHz |
| **容器** | Fast-start | `moov` 位于 `mdat` 之前，适合网页播放 |
| | B 帧 | 显式 PTS/DTS 支持 |
| | 分片 MP4 | 用于 DASH/HLS 流式传输 |
| | 元数据 | 标题、创建时间、语言 |
| **质量** | 详细错误报告 | 十六进制 dump、JSON 输出、可操作的错误信息 |
| | 生产就绪 | 已验证与 FFmpeg 兼容 |
| | 全面测试 | 200+ 测试、基于属性的校验 |
| **绑定** | WASM / Go / C | 可选的 wasm-bindgen 与 C FFI 绑定 |

### 设计原则

| 原则 | 实现 |
|------|------|
| 🦀 **纯 Rust 核心** | 可选的 WASM + C FFI 绑定（Go 通过 cgo） |
| 📦 **极少依赖** | 仅必要的 Rust crate，无外部二进制 |
| 🧵 **线程安全** | writer 可 `Send + Sync` |
| ✅ **测试充分** | 单元、集成、属性测试 |
| 📜 **宽松许可** | 双许可：MIT OR Apache-2.0 |

---

## 快速开始（Rust）

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

    // 写入已编码帧（来自你的编码器）
    // muxer.write_video(pts_seconds, h264_annex_b_bytes, is_keyframe)?;
    // muxer.write_audio(pts_seconds, aac_adts_bytes)?;

    let stats = muxer.finish_with_stats()?;
    println!("写入 {} 帧，{} 字节", stats.video_frames, stats.bytes_written);
    Ok(())
}
```

---

## 命令行工具

```bash
# 仅视频
muxide mux --video keyframes.h264 --width 1920 --height 1080 --fps 30 --output recording.mp4

# 视频 + 音频 + 元数据
muxide mux --video stream.h264 --audio stream.aac \
  --width 1920 --height 1080 --fps 30 \
  --title "My Recording" --language eng --output final.mp4

# JSON 输出便于自动化
muxide mux --json [参数...] > stats.json
```

---

## 语言绑定（WASM、Go、C）

Muxide 的核心是纯 Rust，但它也提供**可选的**语言绑定，让你可以从非 Rust 代码生成 MP4。所有绑定都构建在同一个复用器之上，生成完全相同、符合标准的 MP4。

### WASM（浏览器 / JS）

`wasm` feature 通过 [`wasm-bindgen`](https://crates.io/crates/wasm-bindgen) 把复用器编译为 WebAssembly。

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

// H.264 首个关键帧必须包含 SPS（NAL 7）+ PPS（NAL 8）
muxer.writeVideo(0.0, h264Keyframe, true);

const bytes = muxer.finish(); // 完整 MP4 的 Uint8Array
```

`WasmMuxerBuilder` 还支持 `.audio(...)`，`WasmMuxer` 支持 `writeVideoWithDts`、`writeAudio`、`encodeVideo`、`encodeAudio`。

### Go（通过 C FFI + cgo）

C FFI 层位于 `src/ffi.rs`，配套头文件为 `bindings/muxide.h`。`bindings/go` 下的 Go 包用符合习惯的 API 封装了它。

```bash
# 1) 构建 C 动态库（cdylib）
cargo rustc --release --lib --crate-type cdylib
# 2) Go 包链接 target/release/libmuxide
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
    defer m.Close() // finalizer 也会释放句柄

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
    fmt.Printf("写入 %d 字节 MP4\n", len(out))
}
```

Go API：`NewMuxer` / `NewMuxerWithAudio`、`WriteVideo`、`WriteVideoWithDTS`、`WriteAudio`、`Finish() ([]byte, error)`、`Close()`。Rust 的错误通过 `muxide_last_error` 转为 Go 的 `error` 值。

### C / C++

包含 `bindings/muxide.h` 并链接 cdylib。不透明句柄 `MuxideMuxer` / `MuxideFragmentedMuxer`；所有函数返回 `int32_t` 错误码（`MUXIDE_OK = 0`，负值为错误）。

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

完整 18 个 `extern "C"` 函数、错误常量与 `MuxideMetadata` 结构体见 [`bindings/muxide.h`](../../bindings/muxide.h)。

---

## Muxide 不是什么

Muxide 刻意保持**专注**。它**不**做：

| 不支持 | 原因 |
|--------|------|
| 编码/解码 | 使用 `openh264`、`x264`、`rav1e` 等 |
| 转码 | 不是 codec 库 |
| 解复用/读取 MP4 | 设计为只写 |
| 时间戳修正 | 垃圾进 = 错误出 |
| 非 MP4 容器 | 不支持 MKV、WebM、AVI |
| DRM/加密 | 超出范围 |

**Muxide 是最后一公里**：编码器输出 → 可播放文件。

---

## 使用场景

- 🎥 **屏幕录制器** —— 采集 → 编码 → 复用 → 发布
- 📹 **摄像头应用** —— 网络摄像头/IP 摄像头录制管线
- 🎬 **视频编辑器** —— 导出时间线为 MP4
- 📡 **流式传输** —— 为 DASH/HLS 生成 fMP4 分段
- 🏭 **嵌入式系统** —— 单一二进制，无外部依赖
- 🔬 **科学应用** —— 确定性、可复现的输出

---

## 文档

| 资源 | 说明 |
|------|------|
| [📚 API 参考](https://docs.rs/muxide) | 完整 API 文档 |
| [📜 设计章程](../../docs/charter.md) | 架构决策与理由 |
| [📋 API 契约](../../docs/contract.md) | 输入/输出保证 |

---

## 常见问题

**为什么不直接用 FFmpeg？** FFmpeg 很优秀，但有外部二进制依赖、部分构建的 GPL 许可顾虑、进程编排开销，以及“这个构建带了什么 flag？”的调试难题。Muxide 只需一次 `cargo add`，极少外部依赖。

**Muxide 能编码视频吗？** 不能。Muxide **只做复用（muxing）**。编码请使用 `openh264`、`rav1e`、`x264`/`x265`。

**我的时间戳错了会怎样？** Muxide 会拒绝非单调递增的时间戳并给出清晰错误。它不会尝试“修复”损坏的输入——这是设计使然，以保证可预测的输出。

**Muxide 生产就绪吗？** 是的。Muxide 拥有广泛的测试套件（单元、集成、基于属性的测试），设计目标是可预测、确定的行为。

---

## 许可证

采用以下任一许可：

- Apache License, Version 2.0
- MIT license

由你选择。

---

## 更新日志（0.3.0，未发布）—— WASM、Go 与 C 语言绑定

### ✨ 新功能
- **WASM 绑定**（`wasm` feature，`src/wasm.rs`）：通过 wasm-bindgen 把复用器编译为 WebAssembly。`WasmMuxerBuilder` / `WasmMuxer` 可从 JS/TS 配置音视频并写入帧。
- **C FFI 层**（`src/ffi.rs` + `bindings/muxide.h`）：包含 18 个 `extern "C"` 函数、不透明句柄 `MuxideMuxer` / `MuxideFragmentedMuxer` 与错误码常量的稳定 C ABI。可在任何 C-ABI 语言中使用。
- **Go 绑定**（`bindings/go`）：通过 cgo 封装 C FFI 的符合习惯的 Go 包，带 `runtime.SetFinalizer` 句柄管理与 Rust 错误传播。根目录 `go.mod`（模块 `github.com/Michael-A-Kuykendall/muxide`）让 `go build ./bindings/go/...` 可从仓库根解析。
- **CI**：新增 `wasm-check` 作业，在每次 push/PR 时构建 wasm32 目标。

### 🔒 安全
- **将 `rand` 从 0.8.5 升级到 0.8.6**（RUSTSEC-2026-0097，低危）。`rand` 是经由 `proptest` 的传递性开发依赖，无需代码改动；解决默认分支上的 Dependabot 告警。

### 🧪 测试
- 为新增绑定添加端到端功能测试：Rust FFI 集成测试（`tests/ffi_smoke.rs`）、校验 MP4 结构的强化版 Go 测试，以及通过 `wasm-bindgen-test-runner` 运行的 wasm-bindgen 测试。
- 将 `MuxideMuxer` / `MuxideFragmentedMuxer` 暴露为 `pub`（不透明句柄），以便外部 crate 持有指针。
- 对非 wasm 的开发依赖做目标条件化，使 `cargo test --target wasm32-unknown-unknown` 可编译；将 `wasm-bindgen` 对齐到 0.2.108 以匹配测试运行器。

---

<p align="center">
  <em>Muxide 的设计目标是在集成后变得<strong>无感</strong>：可预测、严格、快速、隐形。</em>
</p>
