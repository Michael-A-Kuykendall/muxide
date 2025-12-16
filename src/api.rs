/// Public API definitions for the Muxide crate.
///
/// This module contains the types and traits that form the public contract
/// for users of the crate.  Concrete implementations live in private
/// modules.  The API defined here intentionally exposes only the
/// capabilities promised by the charter and contract documents.  It does
/// not contain any implementation details.
use crate::muxer::mp4::{Mp4AudioTrack, Mp4VideoTrack, Mp4Writer, Mp4WriterError, MEDIA_TIMESCALE};
use std::fmt;
use std::io::Write;

/// Enumeration of supported video codecs for the initial version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    /// H.264/AVC video codec.  Only the AVC Annex B stream format is
    /// currently supported.  B‑frames are not permitted in v0.
    H264,
    /// H.265/HEVC video codec. Annex B stream format with VPS/SPS/PPS.
    /// Requires first keyframe to contain VPS, SPS, and PPS NALs.
    H265,
    /// AV1 video codec. OBU (Open Bitstream Unit) stream format.
    /// Requires first keyframe to contain Sequence Header OBU.
    Av1,
}

/// Enumeration of supported audio codecs for the initial version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    /// AAC (Advanced Audio Coding) with ADTS framing.  Only AAC LC is
    /// expected to work in v0.
    Aac,
    /// Opus audio codec. Raw Opus packets (no container framing).
    /// Sample rate is always 48kHz per Opus spec.
    Opus,
    /// No audio.  Use this variant when only video is being muxed.
    None,
}

/// High-level muxer configuration intended for simple integrations (e.g. CrabCamera).
#[derive(Debug, Clone)]
pub struct MuxerConfig {
    pub width: u32,
    pub height: u32,
    pub framerate: f64,
    pub audio: Option<AudioTrackConfig>,
    pub metadata: Option<Metadata>,
    pub fast_start: bool,
}

/// Metadata to embed in the MP4 file (title, creation time, etc.)
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    /// Title of the recording (appears in media players)
    pub title: Option<String>,
    /// Creation timestamp in seconds since Unix epoch (1970-01-01)
    pub creation_time: Option<u64>,
}

impl Metadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_creation_time(mut self, unix_timestamp: u64) -> Self {
        self.creation_time = Some(unix_timestamp);
        self
    }

    /// Set creation time to current system time
    pub fn with_current_time(mut self) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        if let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) {
            self.creation_time = Some(duration.as_secs());
        }
        self
    }
}

impl MuxerConfig {
    pub fn new(width: u32, height: u32, framerate: f64) -> Self {
        Self {
            width,
            height,
            framerate,
            audio: None,
            metadata: None,
            fast_start: true,  // Default ON for web compatibility
        }
    }

    pub fn with_audio(mut self, codec: AudioCodec, sample_rate: u32, channels: u16) -> Self {
        if codec == AudioCodec::None {
            self.audio = None;
        } else {
            self.audio = Some(AudioTrackConfig {
                codec,
                sample_rate,
                channels,
            });
        }
        self
    }

    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_fast_start(mut self, enabled: bool) -> Self {
        self.fast_start = enabled;
        self
    }
}

/// Summary statistics returned when finishing a mux.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MuxerStats {
    pub video_frames: u64,
    pub audio_frames: u64,
    pub duration_secs: f64,
    pub bytes_written: u64,
}

/// Builder for constructing a new muxer instance.
///
/// The builder follows a fluent API pattern: each method returns a
/// modified builder, allowing method chaining.  Only the configuration
/// necessary for the initial v0 release is included.  Additional
/// configuration (such as B‑frame support, fragmented MP4 or other
/// containers) will be added in future slices.
pub struct MuxerBuilder<Writer> {
    /// The underlying writer to which container data will be written.
    writer: Writer,
    /// Optional video configuration.
    video: Option<(VideoCodec, u32, u32, f64)>,
    /// Optional audio configuration.
    audio: Option<(AudioCodec, u32, u16)>,
    /// Metadata to embed in the output file.
    metadata: Option<Metadata>,
    /// Whether to enable fast-start (moov before mdat).
    fast_start: bool,
}

impl<Writer> MuxerBuilder<Writer> {
    /// Create a new builder for the given output writer.
    pub fn new(writer: Writer) -> Self {
        Self {
            writer,
            video: None,
            audio: None,
            metadata: None,
            fast_start: true,  // Default ON for web compatibility
        }
    }

    /// Configure the video track.
    pub fn video(mut self, codec: VideoCodec, width: u32, height: u32, framerate: f64) -> Self {
        self.video = Some((codec, width, height, framerate));
        self
    }

    /// Configure the audio track.
    pub fn audio(mut self, codec: AudioCodec, sample_rate: u32, channels: u16) -> Self {
        self.audio = Some((codec, sample_rate, channels));
        self
    }

    /// Set metadata to embed in the output file (title, creation time, etc.)
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Enable or disable fast-start mode (moov before mdat).
    /// Default is `true` for web streaming compatibility.
    pub fn with_fast_start(mut self, enabled: bool) -> Self {
        self.fast_start = enabled;
        self
    }

    /// Finalise the builder and produce a `Muxer` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if required configuration is missing or invalid.
    pub fn build(self) -> Result<Muxer<Writer>, MuxerError>
    where
        Writer: Write,
    {
        // In v0, we perform minimal validation: video configuration must be
        // present.  Future releases may relax this to allow audio‑only
        // streams.
        let (codec, width, height, framerate) = self.video.ok_or(MuxerError::MissingVideoConfig)?;
        let video_track = VideoTrackConfig {
            codec,
            width,
            height,
            framerate,
        };

        let audio_track = self.audio.and_then(|(codec, sample_rate, channels)| {
            if codec == AudioCodec::None {
                None
            } else {
                Some(AudioTrackConfig {
                    codec,
                    sample_rate,
                    channels,
                })
            }
        });

        let mut writer = Mp4Writer::new(self.writer, video_track.codec);
        if let Some(audio) = &audio_track {
            writer.enable_audio(Mp4AudioTrack {
                sample_rate: audio.sample_rate,
                channels: audio.channels,
                codec: audio.codec,
            });
        }

        Ok(Muxer {
            writer,
            video_track,
            audio_track,
            metadata: self.metadata,
            fast_start: self.fast_start,
            first_video_pts: None,
            last_video_pts: None,
            last_video_dts: None,
            last_audio_pts: None,
            video_frame_count: 0,
            audio_frame_count: 0,
            finished: false,
        })
    }
}

/// Configuration for a video track.
#[derive(Debug, Clone)]
pub struct VideoTrackConfig {
    /// Video codec.
    pub codec: VideoCodec,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Frame rate (frames per second).
    pub framerate: f64,
}

/// Configuration for an audio track.
#[derive(Debug, Clone)]
pub struct AudioTrackConfig {
    /// Audio codec.
    pub codec: AudioCodec,
    /// Sample rate (Hz).
    pub sample_rate: u32,
    /// Number of audio channels.
    pub channels: u16,
}

/// Opaque muxer type.  Users interact with this type to write frames
/// into the container.  Implementation details are hidden in a private
/// module.
pub struct Muxer<Writer> {
    writer: Mp4Writer<Writer>,
    video_track: VideoTrackConfig,
    audio_track: Option<AudioTrackConfig>,
    metadata: Option<Metadata>,
    fast_start: bool,
    first_video_pts: Option<f64>,
    last_video_pts: Option<f64>,
    last_video_dts: Option<f64>,
    last_audio_pts: Option<f64>,
    video_frame_count: u64,
    audio_frame_count: u64,
    finished: bool,
}

/// Error type for builder validation and runtime errors.
///
/// All errors include context to help diagnose issues. Error messages are designed
/// to be educational—they explain what went wrong and how to fix it.
#[derive(Debug)]
pub enum MuxerError {
    /// Video configuration is missing.  In v0, a video track is required.
    MissingVideoConfig,
    /// Low-level IO error while writing the container.
    Io(std::io::Error),
    /// The muxer has already been finished.
    AlreadyFinished,
    /// Video `pts` must be non-negative.
    NegativeVideoPts {
        pts: f64,
        frame_index: u64,
    },
    /// Audio `pts` must be non-negative.
    NegativeAudioPts {
        pts: f64,
        frame_index: u64,
    },
    /// Audio was written but no audio track was configured.
    AudioNotConfigured,
    /// Audio sample is empty.
    EmptyAudioFrame {
        frame_index: u64,
    },
    /// Video sample is empty.
    EmptyVideoFrame {
        frame_index: u64,
    },
    /// Video timestamps must be strictly increasing.
    NonIncreasingVideoPts {
        prev_pts: f64,
        curr_pts: f64,
        frame_index: u64,
    },
    /// Audio timestamps must be non-decreasing.
    DecreasingAudioPts {
        prev_pts: f64,
        curr_pts: f64,
        frame_index: u64,
    },
    /// Audio may not precede the first video frame.
    AudioBeforeFirstVideo {
        audio_pts: f64,
        first_video_pts: Option<f64>,
    },
    /// The first video frame must be a keyframe.
    FirstVideoFrameMustBeKeyframe,
    /// The first video frame must include SPS/PPS (H.264/H.265).
    FirstVideoFrameMissingSpsPps,
    /// The first AV1 keyframe must include a Sequence Header OBU.
    FirstAv1FrameMissingSequenceHeader,
    /// Audio sample is not a valid ADTS frame.
    InvalidAdts {
        frame_index: u64,
    },
    /// Audio sample is not a valid Opus packet.
    InvalidOpusPacket {
        frame_index: u64,
    },
    /// DTS must be monotonically increasing.
    NonIncreasingDts {
        prev_dts: f64,
        curr_dts: f64,
        frame_index: u64,
    },
}

impl From<std::io::Error> for MuxerError {
    fn from(err: std::io::Error) -> Self {
        MuxerError::Io(err)
    }
}

impl fmt::Display for MuxerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MuxerError::MissingVideoConfig => {
                write!(f, "missing video configuration: call .video() on MuxerBuilder before .build()")
            }
            MuxerError::Io(err) => write!(f, "IO error: {}", err),
            MuxerError::AlreadyFinished => {
                write!(f, "muxer already finished: cannot write frames after calling finish()")
            }
            MuxerError::NegativeVideoPts { pts, frame_index } => {
                write!(f, "video frame {} has negative PTS ({:.3}s): timestamps must be >= 0.0", 
                       frame_index, pts)
            }
            MuxerError::NegativeAudioPts { pts, frame_index } => {
                write!(f, "audio frame {} has negative PTS ({:.3}s): timestamps must be >= 0.0",
                       frame_index, pts)
            }
            MuxerError::AudioNotConfigured => {
                write!(f, "audio track not configured: call .audio() on MuxerBuilder to enable audio")
            }
            MuxerError::EmptyAudioFrame { frame_index } => {
                write!(f, "audio frame {} is empty: ADTS frames must contain data", frame_index)
            }
            MuxerError::EmptyVideoFrame { frame_index } => {
                write!(f, "video frame {} is empty: video samples must contain NAL units", frame_index)
            }
            MuxerError::NonIncreasingVideoPts { prev_pts, curr_pts, frame_index } => {
                write!(f, "video frame {} has PTS {:.3}s which is not greater than previous PTS {:.3}s: \
                          video timestamps must strictly increase. For B-frames, use write_video_with_dts()",
                       frame_index, curr_pts, prev_pts)
            }
            MuxerError::DecreasingAudioPts { prev_pts, curr_pts, frame_index } => {
                write!(f, "audio frame {} has PTS {:.3}s which is less than previous PTS {:.3}s: \
                          audio timestamps must not decrease",
                       frame_index, curr_pts, prev_pts)
            }
            MuxerError::AudioBeforeFirstVideo { audio_pts, first_video_pts } => {
                match first_video_pts {
                    Some(v) => write!(f, "audio PTS {:.3}s arrives before first video PTS {:.3}s: \
                                         write video frames first, or ensure audio PTS >= video PTS",
                                      audio_pts, v),
                    None => write!(f, "audio frame arrived before any video frame: \
                                       write at least one video frame before writing audio"),
                }
            }
            MuxerError::FirstVideoFrameMustBeKeyframe => {
                write!(f, "first video frame must be a keyframe (IDR): \
                          set is_keyframe=true and ensure the frame contains an IDR NAL unit")
            }
            MuxerError::FirstVideoFrameMissingSpsPps => {
                write!(f, "first video frame must contain SPS and PPS NAL units: \
                          prepend SPS (NAL type 7) and PPS (NAL type 8) to the first keyframe")
            }
            MuxerError::FirstAv1FrameMissingSequenceHeader => {
                write!(f, "first AV1 frame must contain a Sequence Header OBU: \
                          ensure the first keyframe includes OBU type 1 (SEQUENCE_HEADER)")
            }
            MuxerError::InvalidAdts { frame_index } => {
                write!(f, "audio frame {} is not valid ADTS: ensure the frame starts with 0xFFF sync word",
                       frame_index)
            }
            MuxerError::InvalidOpusPacket { frame_index } => {
                write!(f, "audio frame {} is not a valid Opus packet: ensure the frame has valid TOC byte",
                       frame_index)
            }
            MuxerError::NonIncreasingDts { prev_dts, curr_dts, frame_index } => {
                write!(f, "video frame {} has DTS {:.3}s which is not greater than previous DTS {:.3}s: \
                          DTS (decode timestamps) must strictly increase",
                       frame_index, curr_dts, prev_dts)
            }
        }
    }
}

impl std::error::Error for MuxerError {}

// Placeholder for future implementation.  The actual encoding logic will
// live in a private `muxer` module.  For now we provide stub methods
// returning errors.  These stubs ensure that the API compiles and can be
// used by downstream code while implementation proceeds in later slices.
impl<Writer: Write> Muxer<Writer> {
    /// Convenience constructor for config-driven integrations.
    pub fn new(writer: Writer, config: MuxerConfig) -> Result<Self, MuxerError> {
        let mut builder = MuxerBuilder::new(writer).video(
            VideoCodec::H264,
            config.width,
            config.height,
            config.framerate,
        );
        if let Some(audio) = config.audio {
            builder = builder.audio(audio.codec, audio.sample_rate, audio.channels);
        }
        let mut muxer = builder.build()?;
        muxer.metadata = config.metadata;
        muxer.fast_start = config.fast_start;
        Ok(muxer)
    }

    /// Write a video frame to the container.
    ///
    /// `pts` is the presentation timestamp in seconds.  Frames must
    /// be supplied in strictly increasing PTS order.  The `data` slice
    /// contains the encoded frame bitstream in Annex B format (for H.264).
    ///
    /// For streams with B-frames (where PTS != DTS), use `write_video_with_dts()` instead.
    pub fn write_video(
        &mut self,
        pts: f64,
        data: &[u8],
        is_keyframe: bool,
    ) -> Result<(), MuxerError> {
        if self.finished {
            return Err(MuxerError::AlreadyFinished);
        }
        
        let frame_index = self.video_frame_count;
        
        // Reject empty frames - they cause playback issues
        if data.is_empty() {
            return Err(MuxerError::EmptyVideoFrame { frame_index });
        }
        
        // Validate PTS is non-negative
        if pts < 0.0 {
            return Err(MuxerError::NegativeVideoPts { pts, frame_index });
        }
        
        // Validate PTS is strictly increasing
        if let Some(prev) = self.last_video_pts {
            if pts <= prev {
                return Err(MuxerError::NonIncreasingVideoPts {
                    prev_pts: prev,
                    curr_pts: pts,
                    frame_index,
                });
            }
        }
        
        let scaled_pts = (pts * MEDIA_TIMESCALE as f64).round();
        let pts_units = scaled_pts as u64;
        
        if self.first_video_pts.is_none() {
            self.first_video_pts = Some(pts);
        }
        
        self.writer
            .write_video_sample(pts_units, data, is_keyframe)
            .map_err(|e| self.convert_mp4_error(e, frame_index))?;
        
        self.last_video_pts = Some(pts);
        self.video_frame_count += 1;
        Ok(())
    }

    /// Write a video frame with explicit decode timestamp for B-frame support.
    ///
    /// - `pts` is the presentation timestamp in seconds (display order)
    /// - `dts` is the decode timestamp in seconds (decode order)
    /// 
    /// For streams with B-frames, PTS and DTS may differ. The only constraint is that
    /// DTS must be strictly monotonically increasing (frames must be fed in decode order).
    ///
    /// Example GOP: I P B B where decode order is I,P,B,B but display order is I,B,B,P
    /// - I: DTS=0, PTS=0
    /// - P: DTS=1, PTS=3 (decoded second, displayed fourth)
    /// - B: DTS=2, PTS=1 (decoded third, displayed second)
    /// - B: DTS=3, PTS=2 (decoded fourth, displayed third)
    pub fn write_video_with_dts(
        &mut self,
        pts: f64,
        dts: f64,
        data: &[u8],
        is_keyframe: bool,
    ) -> Result<(), MuxerError> {
        if self.finished {
            return Err(MuxerError::AlreadyFinished);
        }
        
        let frame_index = self.video_frame_count;
        
        // Reject empty frames - they cause playback issues
        if data.is_empty() {
            return Err(MuxerError::EmptyVideoFrame { frame_index });
        }
        
        // Validate PTS is non-negative
        if pts < 0.0 {
            return Err(MuxerError::NegativeVideoPts { pts, frame_index });
        }
        
        // Validate DTS is non-negative
        if dts < 0.0 {
            return Err(MuxerError::NegativeVideoPts { pts: dts, frame_index });
        }
        
        // Note: PTS can be less than DTS for B-frames (displayed before their decode position)
        // This is valid and expected for B-frame streams.
        
        // Validate DTS is strictly increasing
        if let Some(prev_dts) = self.last_video_dts {
            if dts <= prev_dts {
                return Err(MuxerError::NonIncreasingDts {
                    prev_dts,
                    curr_dts: dts,
                    frame_index,
                });
            }
        }
        
        let scaled_pts = (pts * MEDIA_TIMESCALE as f64).round();
        let pts_units = scaled_pts as u64;
        let scaled_dts = (dts * MEDIA_TIMESCALE as f64).round();
        let dts_units = scaled_dts as u64;
        
        if self.first_video_pts.is_none() {
            self.first_video_pts = Some(pts);
        }
        
        self.writer
            .write_video_sample_with_dts(pts_units, dts_units, data, is_keyframe)
            .map_err(|e| self.convert_mp4_error(e, frame_index))?;
        
        self.last_video_pts = Some(pts);
        self.last_video_dts = Some(dts);
        self.video_frame_count += 1;
        Ok(())
    }
    
    /// Convert internal Mp4WriterError to MuxerError with context
    fn convert_mp4_error(&self, err: Mp4WriterError, frame_index: u64) -> MuxerError {
        match err {
            Mp4WriterError::NonIncreasingTimestamp => MuxerError::NonIncreasingVideoPts {
                prev_pts: self.last_video_pts.unwrap_or(0.0),
                curr_pts: 0.0, // We don't have access here, but validation above catches this
                frame_index,
            },
            Mp4WriterError::FirstFrameMustBeKeyframe => MuxerError::FirstVideoFrameMustBeKeyframe,
            Mp4WriterError::FirstFrameMissingSpsPps => MuxerError::FirstVideoFrameMissingSpsPps,
            Mp4WriterError::FirstFrameMissingSequenceHeader => MuxerError::FirstAv1FrameMissingSequenceHeader,
            Mp4WriterError::InvalidAdts => MuxerError::InvalidAdts { frame_index },
            Mp4WriterError::InvalidOpusPacket => MuxerError::InvalidOpusPacket { frame_index },
            Mp4WriterError::AudioNotEnabled => MuxerError::AudioNotConfigured,
            Mp4WriterError::DurationOverflow => MuxerError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "duration overflow",
            )),
            Mp4WriterError::AlreadyFinalized => MuxerError::AlreadyFinished,
        }
    }

    /// Write an audio frame to the container.
    ///
    /// `pts` is the presentation timestamp in seconds.  The `data` slice
    /// contains the encoded audio frame (an AAC ADTS frame).
    /// Audio timestamps must be non-decreasing and must not precede the first video frame.
    pub fn write_audio(&mut self, pts: f64, data: &[u8]) -> Result<(), MuxerError> {
        if self.finished {
            return Err(MuxerError::AlreadyFinished);
        }
        if self.audio_track.is_none() {
            return Err(MuxerError::AudioNotConfigured);
        }
        
        let frame_index = self.audio_frame_count;
        
        // Validate PTS is non-negative
        if pts < 0.0 {
            return Err(MuxerError::NegativeAudioPts { pts, frame_index });
        }
        
        // Validate frame is not empty
        if data.is_empty() {
            return Err(MuxerError::EmptyAudioFrame { frame_index });
        }
        
        // Validate PTS is non-decreasing
        if let Some(prev) = self.last_audio_pts {
            if pts < prev {
                return Err(MuxerError::DecreasingAudioPts {
                    prev_pts: prev,
                    curr_pts: pts,
                    frame_index,
                });
            }
        }
        
        // Validate audio doesn't precede first video
        if let Some(first_video) = self.first_video_pts {
            if pts < first_video {
                return Err(MuxerError::AudioBeforeFirstVideo {
                    audio_pts: pts,
                    first_video_pts: Some(first_video),
                });
            }
        } else {
            return Err(MuxerError::AudioBeforeFirstVideo {
                audio_pts: pts,
                first_video_pts: None,
            });
        }

        let scaled_pts = (pts * MEDIA_TIMESCALE as f64).round();
        let pts_units = scaled_pts as u64;
        
        self.writer.write_audio_sample(pts_units, data)
            .map_err(|e| self.convert_mp4_error(e, frame_index))?;
        
        self.last_audio_pts = Some(pts);
        self.audio_frame_count += 1;
        Ok(())
    }

    /// Finalise the container and flush any buffered data.
    ///
    /// In the current slice this writes the `ftyp`/`moov` boxes, resulting
    /// in a minimal MP4 header that can be inspected by the slice 02 tests.
    pub fn finish_in_place(&mut self) -> Result<(), MuxerError> {
        self.finish_in_place_with_stats().map(|_| ())
    }

    /// Finalise the container and return muxing statistics.
    pub fn finish_in_place_with_stats(&mut self) -> Result<MuxerStats, MuxerError> {
        if self.finished {
            return Err(MuxerError::AlreadyFinished);
        }
        let params = Mp4VideoTrack {
            width: self.video_track.width,
            height: self.video_track.height,
        };
        self.writer.finalize(&params, self.metadata.as_ref(), self.fast_start)?;
        self.finished = true;

        let video_frames = self.writer.video_sample_count();
        let audio_frames = self.writer.audio_sample_count();
        let duration_ticks = self.writer.max_end_pts().unwrap_or(0);
        let duration_secs = duration_ticks as f64 / MEDIA_TIMESCALE as f64;
        let bytes_written = self.writer.bytes_written();

        Ok(MuxerStats {
            video_frames,
            audio_frames,
            duration_secs,
            bytes_written,
        })
    }

    pub fn finish(mut self) -> Result<(), MuxerError> {
        self.finish_in_place()
    }

    /// Finalise the container and return muxing statistics.
    pub fn finish_with_stats(mut self) -> Result<MuxerStats, MuxerError> {
        self.finish_in_place_with_stats()
    }
}
