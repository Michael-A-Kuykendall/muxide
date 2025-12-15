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
}

/// Enumeration of supported audio codecs for the initial version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    /// AAC (Advanced Audio Coding) with ADTS framing.  Only AAC LC is
    /// expected to work in v0.
    Aac,
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
}

impl MuxerConfig {
    pub fn new(width: u32, height: u32, framerate: f64) -> Self {
        Self {
            width,
            height,
            framerate,
            audio: None,
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
}

/// Summary statistics returned when finishing a mux.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MuxerStats {
    pub video_frames: u64,
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
}

impl<Writer> MuxerBuilder<Writer> {
    /// Create a new builder for the given output writer.
    pub fn new(writer: Writer) -> Self {
        Self {
            writer,
            video: None,
            audio: None,
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

        let mut writer = Mp4Writer::new(self.writer);
        if let Some(audio) = &audio_track {
            writer.enable_audio(Mp4AudioTrack {
                sample_rate: audio.sample_rate,
                channels: audio.channels,
            });
        }

        Ok(Muxer {
            writer,
            video_track,
            audio_track,
            first_video_pts: None,
            last_audio_pts: None,
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
    first_video_pts: Option<u64>,
    last_audio_pts: Option<u64>,
    finished: bool,
}

/// Error type for builder validation and runtime errors.
#[derive(Debug)]
pub enum MuxerError {
    /// Video configuration is missing.  In v0, a video track is required.
    MissingVideoConfig,
    /// Low-level IO error while writing the container.
    Io(std::io::Error),
    /// The muxer has already been finished.
    AlreadyFinished,
    /// Video `pts` must be non-negative.
    NegativeVideoPts,
    /// Audio `pts` must be non-negative.
    NegativeAudioPts,
    /// Audio was written but no audio track was configured.
    AudioNotConfigured,
    /// Audio sample is empty.
    EmptyAudioFrame,
    /// Video timestamps must be strictly increasing.
    NonIncreasingVideoPts,
    /// Audio timestamps must be non-decreasing.
    NonDecreasingAudioPts,
    /// Audio may not precede the first video frame.
    AudioBeforeFirstVideo,
    /// The first video frame must be a keyframe.
    FirstVideoFrameMustBeKeyframe,
    /// The first video frame must include SPS/PPS.
    FirstVideoFrameMissingSpsPps,
    /// Audio sample is not a valid ADTS frame.
    InvalidAdts,
}

impl From<std::io::Error> for MuxerError {
    fn from(err: std::io::Error) -> Self {
        MuxerError::Io(err)
    }
}

impl From<Mp4WriterError> for MuxerError {
    fn from(err: Mp4WriterError) -> Self {
        match err {
            Mp4WriterError::NonIncreasingTimestamp => MuxerError::NonIncreasingVideoPts,
            Mp4WriterError::FirstFrameMustBeKeyframe => MuxerError::FirstVideoFrameMustBeKeyframe,
            Mp4WriterError::FirstFrameMissingSpsPps => MuxerError::FirstVideoFrameMissingSpsPps,
            Mp4WriterError::InvalidAdts => MuxerError::InvalidAdts,
            Mp4WriterError::AudioNotEnabled => MuxerError::AudioNotConfigured,
            Mp4WriterError::DurationOverflow => MuxerError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "duration overflow",
            )),
            Mp4WriterError::AlreadyFinalized => MuxerError::AlreadyFinished,
        }
    }
}

impl fmt::Display for MuxerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MuxerError::MissingVideoConfig => write!(f, "missing video configuration"),
            MuxerError::Io(err) => write!(f, "io error: {}", err),
            MuxerError::AlreadyFinished => write!(f, "muxer already finished"),
            MuxerError::NegativeVideoPts => write!(f, "video pts must be non-negative"),
            MuxerError::NegativeAudioPts => write!(f, "audio pts must be non-negative"),
            MuxerError::AudioNotConfigured => write!(f, "audio track not configured"),
            MuxerError::EmptyAudioFrame => write!(f, "audio frame must be non-empty"),
            MuxerError::NonIncreasingVideoPts => {
                write!(f, "video pts must be strictly increasing")
            }
            MuxerError::NonDecreasingAudioPts => {
                write!(f, "audio pts must be non-decreasing")
            }
            MuxerError::AudioBeforeFirstVideo => {
                write!(f, "audio must not arrive before first video frame")
            }
            MuxerError::FirstVideoFrameMustBeKeyframe => {
                write!(f, "first video frame must be a keyframe")
            }
            MuxerError::FirstVideoFrameMissingSpsPps => {
                write!(f, "first video frame must contain SPS/PPS")
            }
            MuxerError::InvalidAdts => write!(f, "invalid ADTS frame"),
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
        builder.build()
    }

    /// Write a video frame to the container.
    ///
    /// `pts` is the presentation timestamp in seconds.  In v0, frames must
    /// be supplied in monotonically increasing order.  The `data` slice
    /// contains the encoded frame bitstream in Annex B format (for H.264).
    pub fn write_video(
        &mut self,
        pts: f64,
        data: &[u8],
        is_keyframe: bool,
    ) -> Result<(), MuxerError> {
        if self.finished {
            return Err(MuxerError::AlreadyFinished);
        }
        if pts < 0.0 {
            return Err(MuxerError::NegativeVideoPts);
        }
        let scaled_pts = (pts * MEDIA_TIMESCALE as f64).round();
        let pts_units = scaled_pts as u64;
        if self.first_video_pts.is_none() {
            self.first_video_pts = Some(pts_units);
        }
        self.writer
            .write_video_sample(pts_units, data, is_keyframe)
            .map_err(MuxerError::from)
    }

    /// Write an audio frame to the container.
    ///
    /// `pts` is the presentation timestamp in seconds.  The `data` slice
    /// contains the encoded audio frame (e.g. an AAC ADTS frame).  In v0,
    /// audio is optional and must have the same timescale as video when
    /// present.
    pub fn write_audio(&mut self, pts: f64, data: &[u8]) -> Result<(), MuxerError> {
        if self.finished {
            return Err(MuxerError::AlreadyFinished);
        }
        if self.audio_track.is_none() {
            return Err(MuxerError::AudioNotConfigured);
        }
        if pts < 0.0 {
            return Err(MuxerError::NegativeAudioPts);
        }
        if data.is_empty() {
            return Err(MuxerError::EmptyAudioFrame);
        }

        let scaled_pts = (pts * MEDIA_TIMESCALE as f64).round();
        let pts_units = scaled_pts as u64;

        if let Some(prev) = self.last_audio_pts {
            if pts_units < prev {
                return Err(MuxerError::NonDecreasingAudioPts);
            }
        }

        if let Some(first_video) = self.first_video_pts {
            if pts_units < first_video {
                return Err(MuxerError::AudioBeforeFirstVideo);
            }
        } else {
            return Err(MuxerError::AudioBeforeFirstVideo);
        }

        self.writer.write_audio_sample(pts_units, data)?;
        self.last_audio_pts = Some(pts_units);
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
        self.writer.finalize(&params)?;
        self.finished = true;

        let video_frames = self.writer.video_sample_count();
        let duration_ticks = self.writer.max_end_pts().unwrap_or(0);
        let duration_secs = duration_ticks as f64 / MEDIA_TIMESCALE as f64;
        let bytes_written = self.writer.bytes_written();

        Ok(MuxerStats {
            video_frames,
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
