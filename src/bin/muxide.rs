use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;

use muxide::api::{AacProfile, AudioCodec, Metadata, Muxer, MuxerBuilder, VideoCodec};
use muxide::codec::{h264 as h264_codec, vp9 as vp9_codec};
use muxide::fragmented::{FragmentConfig, FragmentedMuxer};

/// Decode a whitespace-tolerant lowercase hex string into bytes.
///
/// Fixture files (`.264`, `.aac.adts`) are stored as hex text so they can
/// live in version control without binary-diff noise.
fn read_hex_bytes(contents: &str) -> Result<Vec<u8>> {
    let hex: String = contents.chars().filter(|c| !c.is_whitespace()).collect();
    anyhow::ensure!(!hex.is_empty(), "hex input is empty");
    anyhow::ensure!(
        hex.len() % 2 == 0,
        "hex input has odd number of characters ({})",
        hex.len()
    );
    let mut out = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16)
            .with_context(|| format!("invalid hex byte '{}'", &hex[i..i + 2]))?;
        out.push(byte);
    }
    Ok(out)
}

/// Muxide - Minimal-dependency pure-Rust MP4 muxer
///
/// A pure-Rust MP4 muxer for recording applications.
/// Supports H.264/H.265/AV1/VP9 video and AAC/Opus audio.
#[derive(Parser)]
#[command(name = "muxide")]
#[command(version, about, long_about)]
#[command(propagate_version = true)]
#[command(arg_required_else_help = true)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Output to JSON (for automation)
    #[arg(long)]
    json: bool,

    /// Disable progress bars
    #[arg(long)]
    no_progress: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Mux encoded frames into MP4 (default command)
    #[command(alias = "m")]
    Mux {
        /// Input video file
        #[arg(short, long)]
        video: Option<PathBuf>,

        /// Input audio file
        #[arg(short, long)]
        audio: Option<PathBuf>,

        /// Output MP4 file
        #[arg(short, long)]
        output: PathBuf,

        /// Video codec (auto-detected if not specified)
        #[arg(long)]
        video_codec: Option<VideoCodec>,

        /// Video width
        #[arg(long)]
        width: Option<u32>,

        /// Video height
        #[arg(long)]
        height: Option<u32>,

        /// Video frame rate
        #[arg(long)]
        fps: Option<f64>,

        /// Audio codec (auto-detected if not specified)
        #[arg(long)]
        audio_codec: Option<AudioCodec>,

        /// Audio sample rate
        #[arg(long)]
        sample_rate: Option<u32>,

        /// Audio channels
        #[arg(long)]
        channels: Option<u8>,

        /// Enable fragmented MP4 (for DASH/HLS)
        #[arg(long)]
        fragmented: bool,

        /// Fragment duration in milliseconds
        #[arg(long, default_value = "2000")]
        fragment_duration_ms: u32,

        /// Video title
        #[arg(long)]
        title: Option<String>,

        /// Content language (ISO 639-2/T)
        #[arg(long)]
        language: Option<String>,

        /// Creation time as Unix timestamp (seconds since 1970-01-01 UTC)
        #[arg(long)]
        creation_time: Option<u64>,

        /// Validate inputs without creating output file
        #[arg(long)]
        dry_run: bool,
    },

    /// Validate frame data without muxing
    #[command(alias = "v")]
    Validate {
        /// Input video file(s)
        #[arg(short, long)]
        video: Option<PathBuf>,

        /// Input audio file(s)
        #[arg(short, long)]
        audio: Option<PathBuf>,

        /// Output validation report (JSON)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Display codec and frame information
    #[command(alias = "i")]
    Info {
        /// Input file to analyze
        input: PathBuf,
    },
}

#[derive(Debug, Serialize)]
struct MuxStats {
    video_frames: u64,
    audio_frames: u64,
    total_bytes: u64,
    duration_ms: u64,
}

impl MuxStats {
    fn new() -> Self {
        Self {
            video_frames: 0,
            audio_frames: 0,
            total_bytes: 0,
            duration_ms: 0,
        }
    }
}

struct ProgressReporter {
    progress: Option<ProgressBar>,
    stats: MuxStats,
}

impl ProgressReporter {
    fn new(enabled: bool) -> Self {
        let progress = if enabled {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} [{elapsed_precise}] {msg} ({bytes_per_sec})",
                )
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
            );
            pb.set_message("Muxing frames...");
            Some(pb)
        } else {
            None
        };

        Self {
            progress,
            stats: MuxStats::new(),
        }
    }

    fn update_video_frame(&mut self) {
        self.stats.video_frames += 1;
        if let Some(pb) = &self.progress {
            pb.set_message(format!(
                "Muxing frames... (video: {}, audio: {})",
                self.stats.video_frames, self.stats.audio_frames
            ));
        }
    }

    fn update_audio_frame(&mut self) {
        self.stats.audio_frames += 1;
        if let Some(pb) = &self.progress {
            pb.set_message(format!(
                "Muxing frames... (video: {}, audio: {})",
                self.stats.video_frames, self.stats.audio_frames
            ));
        }
    }

    fn update_bytes(&mut self, bytes: u64) {
        self.stats.total_bytes += bytes;
        if let Some(pb) = &self.progress {
            pb.set_length(self.stats.total_bytes);
        }
    }

    fn finish(self) -> Result<MuxStats> {
        if let Some(pb) = self.progress {
            pb.finish_with_message("Muxing complete!");
        }
        Ok(self.stats)
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on verbosity
    if cli.verbose {
        eprintln!("Muxide v{} - Starting...", env!("CARGO_PKG_VERSION"));
    }

    match cli.command {
        Commands::Mux {
            video,
            audio,
            output,
            video_codec,
            width,
            height,
            fps,
            audio_codec,
            sample_rate,
            channels,
            fragmented,
            fragment_duration_ms,
            title,
            language,
            creation_time,
            dry_run,
        } => {
            let progress = ProgressReporter::new(!cli.no_progress);
            mux_command(
                video,
                audio,
                output,
                video_codec,
                width,
                height,
                fps,
                audio_codec,
                sample_rate,
                channels,
                fragmented,
                fragment_duration_ms,
                title,
                language,
                creation_time,
                dry_run,
                progress,
                cli.verbose,
                cli.json,
            )
        }

        Commands::Validate {
            video,
            audio,
            output,
        } => validate_command(video, audio, output, cli.verbose, cli.json),

        Commands::Info { input } => info_command(input, cli.verbose, cli.json),
    }
}

#[allow(clippy::too_many_arguments)]
fn mux_command(
    video: Option<PathBuf>,
    audio: Option<PathBuf>,
    output: PathBuf,
    video_codec: Option<VideoCodec>,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<f64>,
    audio_codec: Option<AudioCodec>,
    sample_rate: Option<u32>,
    channels: Option<u8>,
    fragmented: bool,
    fragment_duration_ms: u32,
    title: Option<String>,
    language: Option<String>,
    creation_time: Option<u64>,
    dry_run: bool,
    mut progress: ProgressReporter,
    verbose: bool,
    json: bool,
) -> Result<()> {
    if verbose {
        eprintln!("Setting up muxer...");
    }

    // Validate required parameters
    if video.is_none() && audio.is_none() {
        anyhow::bail!("At least one of --video or --audio must be specified");
    }

    if video.is_some() && (width.is_none() || height.is_none() || fps.is_none()) {
        anyhow::bail!(
            "Video parameters --width, --height, and --fps are required when using --video"
        );
    }

    // For dry run, validate inputs but don't create output
    if dry_run {
        if verbose {
            eprintln!("Dry run: Validating inputs...");
        }

        // Validate that input files exist and are readable
        if let Some(ref video_path) = video {
            if !video_path.exists() {
                anyhow::bail!("Video input file does not exist: {}", video_path.display());
            }
            if verbose {
                eprintln!("✓ Video input: {}", video_path.display());
            }
        }

        if let Some(ref audio_path) = audio {
            if !audio_path.exists() {
                anyhow::bail!("Audio input file does not exist: {}", audio_path.display());
            }
            if verbose {
                eprintln!("✓ Audio input: {}", audio_path.display());
            }
        }

        // Validate output path (basic check for dry run)
        if output.exists() && !output.is_file() {
            anyhow::bail!("Output path exists but is not a file: {}", output.display());
        }

        if json {
            println!("{{\"dry_run\": true, \"valid\": true}}");
        } else {
            println!("✅ Dry run complete - inputs are valid!");
            if let Some(ref video_path) = video {
                println!("   Video input: {}", video_path.display());
            }
            if let Some(ref audio_path) = audio {
                println!("   Audio input: {}", audio_path.display());
            }
            println!("   Output would be: {}", output.display());
        }
        return Ok(());
    }

    // Store paths for later use
    let video_path = video.clone();
    let audio_path = audio.clone();

    // Create output file
    let mut output_file = File::create(&output)
        .with_context(|| format!("Failed to create output file: {}", output.display()))?;

    if fragmented {
        if video_path.is_none() {
            anyhow::bail!("--video is required for fragmented MP4");
        }
        let (vid_w, vid_h) = match (width, height) {
            (Some(w), Some(h)) => (w, h),
            _ => anyhow::bail!("--width and --height are required for fragmented MP4"),
        };
        let codec = video_codec.unwrap_or(VideoCodec::H264);
        if matches!(codec, VideoCodec::H265 | VideoCodec::Av1) {
            anyhow::bail!(
                "Fragmented {} via CLI requires VPS/SPS/PPS params not yet exposed here. Use the library API with FragmentedMuxer directly.",
                codec
            );
        }

        // Read input
        let frag_file = File::open(video_path.as_ref().unwrap())
            .with_context(|| "Failed to open video file")?;
        let mut reader = BufReader::new(frag_file);
        let mut hex_content = String::new();
        reader.read_to_string(&mut hex_content)?;
        let frame_data = read_hex_bytes(&hex_content)?;

        // Build FragmentConfig and convert frame to segment-ready format
        let (segment_data, config) = match codec {
            VideoCodec::H264 => {
                let avc = h264_codec::extract_avc_config(&frame_data)
                    .unwrap_or_else(h264_codec::default_avc_config);
                let avcc = h264_codec::annexb_to_avcc(&frame_data);
                let cfg = FragmentConfig {
                    width: vid_w,
                    height: vid_h,
                    timescale: 90000,
                    fragment_duration_ms,
                    sps: avc.sps,
                    pps: avc.pps,
                    vps: None,
                    av1_sequence_header: None,
                    vp9_config: None,
                };
                (avcc, cfg)
            }
            VideoCodec::Vp9 => {
                let vp9_cfg = vp9_codec::extract_vp9_config(&frame_data);
                let cfg = FragmentConfig {
                    width: vid_w,
                    height: vid_h,
                    timescale: 90000,
                    fragment_duration_ms,
                    sps: vec![],
                    pps: vec![],
                    vps: None,
                    av1_sequence_header: None,
                    vp9_config: vp9_cfg,
                };
                (frame_data.clone(), cfg)
            }
            _ => unreachable!(),
        };

        let is_sync = match codec {
            VideoCodec::H264 => h264_codec::is_h264_keyframe(&frame_data),
            VideoCodec::Vp9 => vp9_codec::is_vp9_keyframe(&frame_data).unwrap_or(false),
            _ => true,
        };

        let mut fmp4 = FragmentedMuxer::new(config);

        let init = fmp4.init_segment();
        output_file
            .write_all(&init)
            .with_context(|| "Failed to write fMP4 init segment")?;
        progress.update_bytes(init.len() as u64);

        fmp4.write_video(0, 0, &segment_data, is_sync)
            .map_err(|e| anyhow::anyhow!("Failed to write video sample: {e}"))?;
        progress.update_video_frame();

        if let Some(segment) = fmp4.flush_segment() {
            progress.update_bytes(segment.len() as u64);
            output_file
                .write_all(&segment)
                .with_context(|| "Failed to write fMP4 media segment")?;
        }

        let stats = progress.finish()?;
        if json {
            println!("{}", serde_json::to_string_pretty(&stats)?);
        } else {
            println!("✅ Fragmented MP4 muxing complete!");
            println!("   Video frames: {}", stats.video_frames);
            println!("   Total size: {} bytes", stats.total_bytes);
            println!("   Output: {}", output.display());
        }
        return Ok(());
    }

    // Build muxer configuration
    let mut builder = MuxerBuilder::new(output_file);

    // Configure video if provided
    if let (Some(_video), Some(width), Some(height), Some(fps)) = (&video, width, height, fps) {
        let codec = video_codec.unwrap_or(VideoCodec::H264);

        anyhow::ensure!(
            (320..=4096).contains(&width) && (240..=2160).contains(&height),
            "video dimensions {}x{} out of supported range (320x240 to 4096x2160)",
            width,
            height
        );
        anyhow::ensure!(
            fps > 0.0 && fps <= 120.0,
            "frame rate {fps:.1} out of supported range (0, 120]"
        );

        builder = builder.video(codec, width, height, fps);

        if verbose {
            eprintln!(
                "Configured video: {} {}x{} @ {}fps",
                codec, width, height, fps
            );
        }
    }

    // Configure audio if provided
    if let (Some(_audio), Some(sample_rate), Some(channels)) = (&audio, sample_rate, channels) {
        let codec = audio_codec.unwrap_or(AudioCodec::Aac(AacProfile::Lc));

        anyhow::ensure!(
            (1..=192_000).contains(&sample_rate),
            "audio sample rate {sample_rate} Hz out of supported range [1, 192000]"
        );
        anyhow::ensure!(
            (1..=8).contains(&channels),
            "audio channel count {channels} out of supported range [1, 8]"
        );

        builder = builder.audio(codec, sample_rate, channels as u16);

        if verbose {
            eprintln!(
                "Configured audio: {} {}Hz {}ch",
                match codec {
                    AudioCodec::Aac(profile) => format!("AAC-{}", profile),
                    AudioCodec::Opus => "Opus".to_string(),
                    AudioCodec::None => "None".to_string(),
                },
                sample_rate,
                channels
            );
        }
    }

    // Build and apply metadata
    let mut meta = Metadata::new();
    let mut has_meta = false;
    if let Some(t) = title {
        meta = meta.with_title(t);
        has_meta = true;
    }
    if let Some(ts) = creation_time {
        meta = meta.with_creation_time(ts);
        has_meta = true;
    }
    if let Some(lang) = language {
        meta = meta.with_language(lang);
        has_meta = true;
    }
    if has_meta {
        builder = builder.with_metadata(meta);
    }

    // Build the muxer
    let mut muxer = builder.build().with_context(|| "Failed to build muxer")?;

    // Process video frames
    if let Some(video_path) = video_path {
        process_video_frames(&video_path, &mut muxer, &mut progress, verbose)?;
    }

    // Process audio frames
    if let Some(audio_path) = audio_path {
        process_audio_frames(&audio_path, &mut muxer, &mut progress, verbose)?;
    }

    // Finalize muxing
    if verbose {
        eprintln!("Finalizing MP4...");
    }

    let muxer_stats = muxer
        .finish_with_stats()
        .with_context(|| "Failed to finalize MP4")?;

    let mut stats = progress.finish()?;
    stats.duration_ms = (muxer_stats.duration_secs * 1000.0) as u64;

    if json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        println!("✅ Muxing complete!");
        println!("   Video frames: {}", stats.video_frames);
        println!("   Audio frames: {}", stats.audio_frames);
        println!("   Total size: {} bytes", stats.total_bytes);
        println!("   Output: {}", output.display());
    }

    Ok(())
}

fn process_video_frames(
    video_path: &std::path::Path,
    muxer: &mut Muxer<File>,
    progress: &mut ProgressReporter,
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!("Processing video frames from: {}", video_path.display());
    }

    let file = File::open(video_path)
        .with_context(|| format!("Failed to open video file: {}", video_path.display()))?;

    let mut reader = BufReader::new(file);
    let mut hex_content = String::new();
    reader
        .read_to_string(&mut hex_content)
        .with_context(|| "Failed to read video data")?;

    // Convert hex string to bytes (like the example does)
    let data = read_hex_bytes(&hex_content)?;

    // Write the frame (assuming it's a keyframe at time 0)
    muxer
        .write_video(0.0, &data, true)
        .with_context(|| "Failed to write video frame")?;

    progress.update_video_frame();
    progress.update_bytes(data.len() as u64);

    Ok(())
}

fn process_audio_frames(
    audio_path: &std::path::Path,
    muxer: &mut Muxer<File>,
    progress: &mut ProgressReporter,
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!("Processing audio frames from: {}", audio_path.display());
    }

    let file = File::open(audio_path)
        .with_context(|| format!("Failed to open audio file: {}", audio_path.display()))?;

    let mut reader = BufReader::new(file);
    let mut hex_content = String::new();
    reader
        .read_to_string(&mut hex_content)
        .with_context(|| "Failed to read audio data")?;

    // Convert hex string to bytes
    let data = read_hex_bytes(&hex_content)?;

    // Write the frame at time 0
    muxer
        .write_audio(0.0, &data)
        .with_context(|| "Failed to write audio frame")?;

    progress.update_audio_frame();
    progress.update_bytes(data.len() as u64);

    Ok(())
}

fn validate_command(
    video: Option<PathBuf>,
    audio: Option<PathBuf>,
    output: Option<PathBuf>,
    verbose: bool,
    json: bool,
) -> Result<()> {
    if verbose {
        eprintln!("Running validation...");
    }

    let mut is_valid = true;
    let mut checks = Vec::new();

    // Validate video input
    if let Some(ref video_path) = video {
        if !video_path.exists() {
            checks.push(serde_json::json!({
                "type": "video_file",
                "status": "error",
                "message": format!("Video file does not exist: {}", video_path.display())
            }));
            is_valid = false;
        } else {
            // Try to read and validate hex content
            match validate_hex_file(video_path, "video") {
                Ok(msg) => checks.push(serde_json::json!({
                    "type": "video_file",
                    "status": "success",
                    "message": msg
                })),
                Err(e) => {
                    checks.push(serde_json::json!({
                        "type": "video_file",
                        "status": "error",
                        "message": format!("Video file validation failed: {}", e)
                    }));
                    is_valid = false;
                }
            }
        }
    }

    // Validate audio input
    if let Some(ref audio_path) = audio {
        if !audio_path.exists() {
            checks.push(serde_json::json!({
                "type": "audio_file",
                "status": "error",
                "message": format!("Audio file does not exist: {}", audio_path.display())
            }));
            is_valid = false;
        } else {
            // Try to read and validate hex content
            match validate_hex_file(audio_path, "audio") {
                Ok(msg) => checks.push(serde_json::json!({
                    "type": "audio_file",
                    "status": "success",
                    "message": msg
                })),
                Err(e) => {
                    checks.push(serde_json::json!({
                        "type": "audio_file",
                        "status": "error",
                        "message": format!("Audio file validation failed: {}", e)
                    }));
                    is_valid = false;
                }
            }
        }
    }

    if video.is_none() && audio.is_none() {
        checks.push(serde_json::json!({
            "type": "input",
            "status": "error",
            "message": "At least one of video or audio input must be specified"
        }));
        is_valid = false;
    }

    let report = serde_json::json!({
        "status": if is_valid { "success" } else { "failed" },
        "valid": is_valid,
        "checks": checks
    });

    if let Some(output_path) = output {
        std::fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;
        if !json {
            println!("Validation report written to: {}", output_path.display());
        }
    } else if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        if is_valid {
            println!("✅ Validation successful!");
        } else {
            println!("❌ Validation failed!");
        }
        for check in &checks {
            let status = check["status"].as_str().unwrap();
            let message = check["message"].as_str().unwrap();
            if status == "error" {
                println!("   ❌ {}", message);
            } else {
                println!("   ✅ {}", message);
            }
        }
    }

    Ok(())
}

fn validate_hex_file(path: &std::path::Path, file_type: &str) -> Result<String> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open {} file: {}", file_type, path.display()))?;

    let mut reader = BufReader::new(file);
    let mut content = String::new();
    reader
        .read_to_string(&mut content)
        .with_context(|| format!("Failed to read {} file content", file_type))?;

    let bytes = read_hex_bytes(&content)
        .with_context(|| format!("{file_type} file contains invalid hex"))?;

    Ok(format!("{} file is valid hex ({} bytes)", file_type, bytes.len()))
}

fn info_command(input: PathBuf, verbose: bool, json: bool) -> Result<()> {
    if verbose {
        eprintln!("Analyzing file: {}", input.display());
    }

    if !input.exists() {
        anyhow::bail!("Input file does not exist: {}", input.display());
    }

    let file =
        File::open(&input).with_context(|| format!("Failed to open file: {}", input.display()))?;

    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader
        .read_to_end(&mut buffer)
        .with_context(|| "Failed to read file content")?;

    if buffer.len() < 8 {
        anyhow::bail!("File too small to be a valid MP4");
    }

    // Parse basic MP4 structure
    let mut boxes = Vec::new();
    let mut offset = 0;
    while offset + 8 <= buffer.len() {
        let size = u32::from_be_bytes(buffer[offset..offset + 4].try_into().unwrap()) as usize;
        let typ = &buffer[offset + 4..offset + 8];

        if size == 0 {
            break; // Last box
        }

        if offset + size > buffer.len() {
            boxes.push(serde_json::json!({
                "type": "invalid",
                "size": size,
                "offset": offset,
                "error": "Box size exceeds file size"
            }));
            break;
        }

        let box_type = std::str::from_utf8(typ).unwrap_or("????");
        boxes.push(serde_json::json!({
            "type": box_type,
            "size": size,
            "offset": offset
        }));

        offset += size;
    }

    // Scan the full buffer for codec FourCCs — these are deeply nested
    // inside moov/trak/mdia/minf/stbl/stsd and cannot be found by only
    // examining top-level box types.
    let has_ftyp = boxes.iter().any(|b| b["type"] == "ftyp");
    let has_moov = boxes.iter().any(|b| b["type"] == "moov");
    let _has_mdat = boxes.iter().any(|b| b["type"] == "mdat");
    let is_valid_mp4 = has_ftyp && has_moov;

    let has_avc1 = buffer.windows(4).any(|w| w == b"avc1");
    let has_hvc1 = buffer.windows(4).any(|w| w == b"hvc1");
    let has_mp4a = buffer.windows(4).any(|w| w == b"mp4a");
    let has_vp09 = buffer.windows(4).any(|w| w == b"vp09");
    let has_av01 = buffer.windows(4).any(|w| w == b"av01");

    let video_codec = if has_avc1 {
        "H.264/AVC"
    } else if has_hvc1 {
        "H.265/HEVC"
    } else if has_vp09 {
        "VP9"
    } else if has_av01 {
        "AV1"
    } else {
        "Unknown"
    };
    let has_audio = has_mp4a;

    let info = serde_json::json!({
        "file": input.display().to_string(),
        "file_size": buffer.len(),
        "is_valid_mp4": is_valid_mp4,
        "video_codec": video_codec,
        "has_audio": has_audio,
        "boxes": boxes
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("File: {}", input.display());
        println!("Size: {} bytes", buffer.len());
        println!("Valid MP4: {}", if is_valid_mp4 { "Yes" } else { "No" });
        println!("Video Codec: {}", video_codec);
        println!("Has Audio: {}", if has_audio { "Yes" } else { "No" });
        println!("Boxes found: {}", info["boxes"].as_array().unwrap().len());
        for box_info in info["boxes"].as_array().unwrap() {
            let typ = box_info["type"].as_str().unwrap();
            let size = box_info["size"].as_u64().unwrap();
            println!("  {}: {} bytes", typ, size);
        }
    }

    Ok(())
}
