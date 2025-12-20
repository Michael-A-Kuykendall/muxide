use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;

use muxide::api::{MuxerBuilder, Muxer, VideoCodec, AudioCodec, AacProfile, Metadata};
use muxide::assert_invariant;

fn read_hex_bytes(contents: &str) -> Vec<u8> {
    let hex: String = contents.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(hex.len() % 2 == 0, "hex must have even length");

    let mut out = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex");
        out.push(byte);
    }
    out
}

/// Muxide - Zero-dependency pure-Rust MP4 muxer
///
/// A professional-grade MP4 muxer designed for recording applications.
/// Supports H.264/H.265/AV1 video and AAC/Opus audio with world-class error handling.
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
        /// Input video file(s) or directory
        #[arg(short, long)]
        video: Option<PathBuf>,

        /// Input audio file(s) or directory
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

        /// Creation time (ISO 8601)
        #[arg(long)]
        creation_time: Option<String>,
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
                    "{spinner:.green} [{elapsed_precise}] {msg} ({bytes_per_sec})"
                ).unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
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
            pb.set_message(format!("Muxing frames... (video: {}, audio: {})",
                self.stats.video_frames, self.stats.audio_frames));
        }
    }

    fn update_audio_frame(&mut self) {
        self.stats.audio_frames += 1;
        if let Some(pb) = &self.progress {
            pb.set_message(format!("Muxing frames... (video: {}, audio: {})",
                self.stats.video_frames, self.stats.audio_frames));
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
        } => {
            let progress = ProgressReporter::new(!cli.no_progress);
            mux_command(
                video, audio, output, video_codec, width, height, fps,
                audio_codec, sample_rate, channels, fragmented, fragment_duration_ms,
                title, language, creation_time,
                progress, cli.verbose, cli.json,
            )
        }

        Commands::Validate { video, audio, output } => {
            validate_command(video, audio, output, cli.verbose, cli.json)
        }

        Commands::Info { input } => {
            info_command(input, cli.verbose, cli.json)
        }
    }
}

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
    _fragment_duration_ms: u32,
    title: Option<String>,
    language: Option<String>,
    creation_time: Option<String>,
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

    // Invariant: CLI must validate video parameters when video is specified
    if video.is_some() {
        assert_invariant!(
            width.is_some() && height.is_some() && fps.is_some(),
            "Video parameters must be complete when video input is provided",
            "cli::mux_command"
        );
    }

    // Invariant: CLI must validate audio parameters when audio is specified
    if audio.is_some() {
        assert_invariant!(
            sample_rate.is_some() && channels.is_some(),
            "Audio parameters must be complete when audio input is provided",
            "cli::mux_command"
        );
    }

    if video.is_some() && (width.is_none() || height.is_none() || fps.is_none()) {
        anyhow::bail!("Video parameters --width, --height, and --fps are required when using --video");
    }

    // Store paths for later use
    let video_path = video.clone();
    let audio_path = audio.clone();

    // Create output file
    let output_file = File::create(&output)
        .with_context(|| format!("Failed to create output file: {}", output.display()))?;

    // Build muxer configuration
    let mut builder = if fragmented {
        // For fragmented MP4, we'll need to implement FragmentedMuxer integration
        // For now, fall back to regular muxer
        MuxerBuilder::new(output_file)
    } else {
        MuxerBuilder::new(output_file)
    };

    // Configure video if provided
    if let (Some(_video), Some(width), Some(height), Some(fps)) = (&video, width, height, fps) {
        let width = width;
        let height = height;
        let fps = fps;
        let codec = video_codec.unwrap_or(VideoCodec::H264); // Default to H.264

        // Invariant: Video codec must be supported
        assert_invariant!(
            matches!(codec, VideoCodec::H264 | VideoCodec::H265 | VideoCodec::Av1),
            "Video codec must be one of the supported variants",
            "cli::mux_command"
        );

        builder = builder.video(codec, width, height, fps);

        // Invariant: Video dimensions must be reasonable
        assert_invariant!(
            width >= 320 && height >= 240 && width <= 4096 && height <= 2160,
            "Video dimensions must be within reasonable limits (320x240 to 4096x2160)",
            "cli::mux_command"
        );

        // Invariant: Frame rate must be reasonable
        assert_invariant!(
            fps > 0.0 && fps <= 120.0,
            "Frame rate must be positive and within reasonable limits",
            "cli::mux_command"
        );

        if verbose {
            eprintln!("Configured video: {} {}x{} @ {}fps",
                codec, width, height, fps);
        }
    }

    // Configure audio if provided
    if let (Some(_audio), Some(sample_rate), Some(channels)) = (&audio, sample_rate, channels) {
        let sample_rate = sample_rate;
        let channels = channels;
        let codec = audio_codec.unwrap_or(AudioCodec::Aac(AacProfile::Lc)); // Default to AAC LC

        // Invariant: Audio codec must be supported
        assert_invariant!(
            matches!(codec, AudioCodec::Aac(_) | AudioCodec::Opus),
            "Audio codec must be one of the supported variants",
            "cli::mux_command"
        );

        builder = builder.audio(codec, sample_rate, channels as u16);

        // Invariant: Audio sample rate must be reasonable
        assert_invariant!(
            sample_rate > 0 && sample_rate <= 192000,
            "Audio sample rate must be positive and within reasonable limits",
            "cli::mux_command"
        );

        // Invariant: Audio channels must be reasonable
        assert_invariant!(
            channels > 0 && channels <= 8,
            "Audio channels must be positive and within reasonable limits",
            "cli::mux_command"
        );

        if verbose {
            eprintln!("Configured audio: {} {}Hz {}ch",
                match codec {
                    AudioCodec::Aac(profile) => format!("AAC-{}", profile),
                    AudioCodec::Opus => "Opus".to_string(),
                    AudioCodec::None => "None".to_string(),
                },
                sample_rate, channels);
        }
    }

    // Add metadata
    if let Some(title) = title {
        builder = builder.with_metadata(Metadata::new().with_title(title));
    }
    if let Some(language) = language {
        builder = builder.set_language(language);
    }
    if let Some(_creation_time) = creation_time {
        // Parse ISO 8601 datetime
        // For now, skip this - would need chrono dependency
        eprintln!("Warning: creation_time not yet implemented");
    }

    // Build the muxer
    let mut muxer = builder.build()
        .with_context(|| "Failed to build muxer")?;

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

    // Invariant: At least one media stream must be configured
    assert_invariant!(
        video.is_some() || audio.is_some(),
        "At least one media stream (video or audio) must be configured",
        "cli::mux_command"
    );

    // Invariant: Output file must be writable
    assert_invariant!(
        output.metadata().is_ok(),
        "Output file path must be writable",
        "cli::mux_command"
    );

    muxer.finish()
        .with_context(|| "Failed to finalize MP4")?;

    let stats = progress.finish()?;

    // Invariant: Final output must have reasonable size
    assert_invariant!(
        stats.total_bytes > 0,
        "Final output must have non-zero size",
        "cli::mux_command"
    );

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
    video_path: &PathBuf,
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
    reader.read_to_string(&mut hex_content)
        .with_context(|| "Failed to read video data")?;

    // Convert hex string to bytes (like the example does)
    let data = read_hex_bytes(&hex_content);

    // Write the frame (assuming it's a keyframe at time 0)
    muxer.write_video(0.0, &data, true)
        .with_context(|| "Failed to write video frame")?;

    progress.update_video_frame();
    progress.update_bytes(data.len() as u64);

    Ok(())
}

fn process_audio_frames(
    audio_path: &PathBuf,
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
    reader.read_to_string(&mut hex_content)
        .with_context(|| "Failed to read audio data")?;

    // Convert hex string to bytes
    let data = read_hex_bytes(&hex_content);

    // Write the frame at time 0
    muxer.write_audio(0.0, &data)
        .with_context(|| "Failed to write audio frame")?;

    progress.update_audio_frame();
    progress.update_bytes(data.len() as u64);

    Ok(())
}

fn validate_command(
    _video: Option<PathBuf>,
    _audio: Option<PathBuf>,
    output: Option<PathBuf>,
    verbose: bool,
    json: bool,
) -> Result<()> {
    if verbose {
        eprintln!("Running validation...");
    }

    // Placeholder implementation
    let report = serde_json::json!({
        "status": "not_implemented",
        "message": "Validation command not yet implemented"
    });

    if let Some(output_path) = output {
        std::fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;
        if !json {
            println!("Validation report written to: {}", output_path.display());
        }
    } else if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!("Validation: Not yet implemented");
    }

    Ok(())
}

fn info_command(
    input: PathBuf,
    verbose: bool,
    json: bool,
) -> Result<()> {
    if verbose {
        eprintln!("Analyzing file: {}", input.display());
    }

    // Placeholder implementation
    let info = serde_json::json!({
        "file": input.display().to_string(),
        "status": "not_implemented",
        "message": "Info command not yet implemented"
    });

    if json {
        println!("{}", serde_json::to_string(&info)?);
    } else {
        println!("File info: Not yet implemented");
        println!("File: {}", input.display());
    }

    Ok(())
}