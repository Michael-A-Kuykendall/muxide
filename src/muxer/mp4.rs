use std::fmt;
use std::io::{self, Write};

use crate::api::{AudioCodec, Metadata, VideoCodec};
use crate::codec::h264::{AvcConfig, extract_avc_config, default_avc_config, annexb_to_avcc};
use crate::codec::h265::{HevcConfig, extract_hevc_config, hevc_annexb_to_hvcc};
use crate::codec::av1::{Av1Config, extract_av1_config};
use crate::codec::opus::{OpusConfig, is_valid_opus_packet, OPUS_SAMPLE_RATE};

const MOVIE_TIMESCALE: u32 = 1000;
/// Track/media timebase used for converting `pts` seconds into MP4 sample deltas.
///
/// v0.1.0 uses a 90 kHz media timescale (common for MP4/H.264 workflows).
pub const MEDIA_TIMESCALE: u32 = 90_000;

/// Video codec configuration extracted from the first keyframe.
#[derive(Clone, Debug)]
pub enum VideoConfig {
    /// H.264/AVC configuration (SPS + PPS)
    Avc(AvcConfig),
    /// H.265/HEVC configuration (VPS + SPS + PPS)
    Hevc(HevcConfig),
    /// AV1 configuration (Sequence Header OBU)
    Av1(Av1Config),
}

/// Minimal MP4 writer used by the early slices.
pub struct Mp4Writer<Writer> {
    writer: Writer,
    video_codec: VideoCodec,
    video_samples: Vec<SampleInfo>,
    video_prev_pts: Option<u64>,
    video_last_delta: Option<u32>,
    video_config: Option<VideoConfig>,
    audio_track: Option<Mp4AudioTrack>,
    audio_samples: Vec<SampleInfo>,
    audio_prev_pts: Option<u64>,
    audio_last_delta: Option<u32>,
    finalized: bool,
    bytes_written: u64,
}

/// Simplified video track information used when writing the header.
pub struct Mp4VideoTrack {
    pub width: u32,
    pub height: u32,
}

pub struct Mp4AudioTrack {
    pub sample_rate: u32,
    pub channels: u16,
    pub codec: AudioCodec,
}

struct SampleInfo {
    pts: u64,
    dts: u64,  // Decode time (for B-frames: dts != pts)
    data: Vec<u8>,
    is_keyframe: bool,
    duration: Option<u32>,
}

struct SampleTables {
    durations: Vec<u32>,
    sizes: Vec<u32>,
    keyframes: Vec<u32>,
    chunk_offsets: Vec<u32>,
    samples_per_chunk: u32,
    cts_offsets: Vec<i32>,  // Composition time offsets (pts - dts) for ctts box
    has_bframes: bool,       // True if any sample has pts != dts
}

impl SampleTables {
    fn from_samples(
        samples: &[SampleInfo],
        chunk_offsets: Vec<u32>,
        samples_per_chunk: u32,
        fallback_duration: Option<u32>,
    ) -> Self {
        let sample_count = samples.len() as u32;
        let mut durations = Vec::with_capacity(sample_count as usize);
        for (idx, sample) in samples.iter().enumerate() {
            let duration = sample.duration.unwrap_or_else(|| {
                if idx == samples.len() - 1 {
                    fallback_duration.unwrap_or(1)
                } else {
                    1
                }
            });
            durations.push(duration);
        }
        let sizes = samples
            .iter()
            .map(|sample| sample.data.len() as u32)
            .collect();
        let keyframes = samples
            .iter()
            .enumerate()
            .filter_map(|(idx, sample)| {
                if sample.is_keyframe {
                    Some(idx as u32 + 1)
                } else {
                    None
                }
            })
            .collect();
        
        // Compute composition time offsets (cts = pts - dts)
        let mut has_bframes = false;
        let cts_offsets: Vec<i32> = samples
            .iter()
            .map(|sample| {
                let offset = (sample.pts as i64 - sample.dts as i64) as i32;
                if offset != 0 {
                    has_bframes = true;
                }
                offset
            })
            .collect();
        
        let _ = sample_count;
        Self {
            durations,
            sizes,
            keyframes,
            chunk_offsets,
            samples_per_chunk,
            cts_offsets,
            has_bframes,
        }
    }
}

/// Errors produced while queuing video samples.
#[derive(Debug)]
pub enum Mp4WriterError {
    /// Video frames must have strictly increasing timestamps.
    NonIncreasingTimestamp,
    /// The first frame must be a keyframe containing SPS/PPS data.
    FirstFrameMustBeKeyframe,
    /// The first keyframe must include SPS and PPS NAL units.
    FirstFrameMissingSpsPps,
    /// The first AV1 keyframe must include a Sequence Header OBU.
    FirstFrameMissingSequenceHeader,
    /// Audio sample is not a valid ADTS frame.
    InvalidAdts,
    /// Audio sample is not a valid Opus packet.
    InvalidOpusPacket,
    /// Audio track is not enabled on this writer.
    AudioNotEnabled,
    /// Computed sample duration overflowed a `u32`.
    DurationOverflow,
    /// The writer has already been finalised.
    AlreadyFinalized,
}

impl fmt::Display for Mp4WriterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mp4WriterError::NonIncreasingTimestamp => write!(f, "timestamps must grow"),
            Mp4WriterError::FirstFrameMustBeKeyframe => {
                write!(f, "first frame must be a keyframe")
            }
            Mp4WriterError::FirstFrameMissingSpsPps => {
                write!(f, "first frame must contain SPS/PPS")
            }
            Mp4WriterError::FirstFrameMissingSequenceHeader => {
                write!(f, "first AV1 frame must contain Sequence Header OBU")
            }
            Mp4WriterError::InvalidAdts => write!(f, "invalid ADTS frame"),
            Mp4WriterError::InvalidOpusPacket => write!(f, "invalid Opus packet"),
            Mp4WriterError::AudioNotEnabled => write!(f, "audio track not enabled"),
            Mp4WriterError::DurationOverflow => write!(f, "sample duration overflow"),
            Mp4WriterError::AlreadyFinalized => write!(f, "writer already finalised"),
        }
    }
}

impl std::error::Error for Mp4WriterError {}

impl<Writer: Write> Mp4Writer<Writer> {
    /// Wraps the provided writer for MP4 container output.
    pub fn new(writer: Writer, video_codec: VideoCodec) -> Self {
        Self {
            writer,
            video_codec,
            video_samples: Vec::new(),
            video_prev_pts: None,
            video_last_delta: None,
            video_config: None,
            audio_track: None,
            audio_samples: Vec::new(),
            audio_prev_pts: None,
            audio_last_delta: None,
            finalized: false,
            bytes_written: 0,
        }
    }

    pub(crate) fn video_sample_count(&self) -> u64 {
        self.video_samples.len() as u64
    }

    pub(crate) fn audio_sample_count(&self) -> u64 {
        self.audio_samples.len() as u64
    }

    pub(crate) fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    pub(crate) fn max_end_pts(&self) -> Option<u64> {
        fn track_end(samples: &[SampleInfo], last_delta: Option<u32>) -> Option<u64> {
            let last = samples.last()?;
            Some(last.pts + u64::from(last_delta.unwrap_or(0)))
        }

        let video_end = track_end(&self.video_samples, self.video_last_delta);
        let audio_end = track_end(&self.audio_samples, self.audio_last_delta);

        match (video_end, audio_end) {
            (Some(v), Some(a)) => Some(v.max(a)),
            (Some(v), None) => Some(v),
            (None, Some(a)) => Some(a),
            (None, None) => None,
        }
    }

    fn write_counted(writer: &mut Writer, bytes_written: &mut u64, buf: &[u8]) -> io::Result<()> {
        *bytes_written = bytes_written.saturating_add(buf.len() as u64);
        writer.write_all(buf)
    }

    pub fn enable_audio(&mut self, track: Mp4AudioTrack) {
        self.audio_track = Some(track);
    }

    /// Queues a video sample for later `mdat` emission.
    /// For backward compatibility, dts is assumed equal to pts.
    pub fn write_video_sample(
        &mut self,
        pts: u64,
        data: &[u8],
        is_keyframe: bool,
    ) -> Result<(), Mp4WriterError> {
        self.write_video_sample_with_dts(pts, pts, data, is_keyframe)
    }

    /// Queues a video sample with explicit DTS for B-frame support.
    /// - `pts`: Presentation timestamp (display order)
    /// - `dts`: Decode timestamp (decode order) - must be monotonically increasing
    pub fn write_video_sample_with_dts(
        &mut self,
        pts: u64,
        dts: u64,
        data: &[u8],
        is_keyframe: bool,
    ) -> Result<(), Mp4WriterError> {
        if self.finalized {
            return Err(Mp4WriterError::AlreadyFinalized);
        }
        // DTS must be monotonically increasing (decode order)
        if let Some(prev) = self.video_prev_pts {
            if dts <= prev {
                return Err(Mp4WriterError::NonIncreasingTimestamp);
            }
            let delta = dts - prev;
            if delta > u64::from(u32::MAX) {
                return Err(Mp4WriterError::DurationOverflow);
            }
            let delta = delta as u32;
            if let Some(last) = self.video_samples.last_mut() {
                last.duration = Some(delta);
            }
            self.video_last_delta = Some(delta);
        } else {
            if !is_keyframe {
                return Err(Mp4WriterError::FirstFrameMustBeKeyframe);
            }
            // Extract codec configuration based on video codec type
            let config = match self.video_codec {
                VideoCodec::H264 => {
                    extract_avc_config(data).map(VideoConfig::Avc)
                }
                VideoCodec::H265 => {
                    extract_hevc_config(data).map(VideoConfig::Hevc)
                }
                VideoCodec::Av1 => {
                    extract_av1_config(data).map(VideoConfig::Av1)
                }
            };
            if config.is_none() {
                return Err(match self.video_codec {
                    VideoCodec::Av1 => Mp4WriterError::FirstFrameMissingSequenceHeader,
                    _ => Mp4WriterError::FirstFrameMissingSpsPps,
                });
            }
            self.video_config = config;
        }

        // Convert Annex B to length-prefixed format based on codec
        // AV1 uses OBU format which doesn't need conversion
        let converted = match self.video_codec {
            VideoCodec::H264 => annexb_to_avcc(data),
            VideoCodec::H265 => hevc_annexb_to_hvcc(data),
            VideoCodec::Av1 => data.to_vec(),  // AV1 OBUs passed as-is
        };
        if converted.len() > u32::MAX as usize {
            return Err(Mp4WriterError::DurationOverflow);
        }

        self.video_samples.push(SampleInfo {
            pts,
            dts,
            data: converted,
            is_keyframe,
            duration: None,
        });
        self.video_prev_pts = Some(dts);  // Track DTS for monotonic check
        Ok(())
    }

    pub fn write_audio_sample(&mut self, pts: u64, data: &[u8]) -> Result<(), Mp4WriterError> {
        if self.finalized {
            return Err(Mp4WriterError::AlreadyFinalized);
        }
        let audio_track = self.audio_track.as_ref().ok_or(Mp4WriterError::AudioNotEnabled)?;

        if let Some(prev) = self.audio_prev_pts {
            if pts < prev {
                return Err(Mp4WriterError::NonIncreasingTimestamp);
            }
            let delta = pts - prev;
            if delta > u64::from(u32::MAX) {
                return Err(Mp4WriterError::DurationOverflow);
            }
            let delta = delta as u32;
            if let Some(last) = self.audio_samples.last_mut() {
                last.duration = Some(delta);
            }
            self.audio_last_delta = Some(delta);
        }

        // Process audio data based on codec
        let sample_data = match audio_track.codec {
            AudioCodec::Aac => {
                let raw = adts_to_raw(data).ok_or(Mp4WriterError::InvalidAdts)?;
                raw.to_vec()
            }
            AudioCodec::Opus => {
                // Validate Opus packet structure
                if !is_valid_opus_packet(data) {
                    return Err(Mp4WriterError::InvalidOpusPacket);
                }
                // Opus packets are passed through as-is (no container framing)
                data.to_vec()
            }
            AudioCodec::None => {
                return Err(Mp4WriterError::AudioNotEnabled);
            }
        };

        if sample_data.len() > u32::MAX as usize {
            return Err(Mp4WriterError::DurationOverflow);
        }

        self.audio_samples.push(SampleInfo {
            pts,
            dts: pts,  // Audio: dts == pts (no B-frames)
            data: sample_data,
            is_keyframe: false,
            duration: None,
        });
        self.audio_prev_pts = Some(pts);
        Ok(())
    }

    /// Finalises the MP4 file by writing the header boxes and sample data.
    pub fn finalize(&mut self, video: &Mp4VideoTrack, metadata: Option<&Metadata>, fast_start: bool) -> io::Result<()> {
        if self.finalized {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "mp4 writer already finalised",
            ));
        }
        self.finalized = true;

        let video_config = self
            .video_config
            .clone()
            .or_else(|| {
                if self.video_samples.is_empty() {
                    // Default config based on codec type
                    match self.video_codec {
                        VideoCodec::H264 => Some(VideoConfig::Avc(default_avc_config())),
                        VideoCodec::H265 => None, // No default for HEVC, must have frames
                        VideoCodec::Av1 => None, // No default for AV1, must have frames
                    }
                } else {
                    None
                }
            })
            .unwrap_or_else(|| VideoConfig::Avc(default_avc_config()));

        if fast_start {
            self.finalize_fast_start(video, metadata, &video_config)
        } else {
            self.finalize_standard(video, metadata, &video_config)
        }
    }

    fn finalize_standard(&mut self, video: &Mp4VideoTrack, metadata: Option<&Metadata>, video_config: &VideoConfig) -> io::Result<()> {
        let ftyp_box = build_ftyp_box();
        let ftyp_len = ftyp_box.len() as u32;
        Self::write_counted(&mut self.writer, &mut self.bytes_written, &ftyp_box)?;

        let audio_present = self.audio_track.is_some();

        if !audio_present {
            let chunk_offset = if !self.video_samples.is_empty() {
                let mut payload_size: u32 = 0;
                for sample in &self.video_samples {
                    payload_size += sample.data.len() as u32;
                }

                let mdat_size = 8 + payload_size;
                Self::write_counted(&mut self.writer, &mut self.bytes_written, &mdat_size.to_be_bytes())?;
                Self::write_counted(&mut self.writer, &mut self.bytes_written, b"mdat")?;
                for sample in &self.video_samples {
                    Self::write_counted(&mut self.writer, &mut self.bytes_written, &sample.data)?;
                }
                Some(ftyp_len + 8)
            } else {
                None
            };

            let (chunk_offsets, samples_per_chunk) = match chunk_offset {
                Some(offset) => (vec![offset], self.video_samples.len() as u32),
                None => (Vec::new(), 0),
            };

            let tables = SampleTables::from_samples(
                &self.video_samples,
                chunk_offsets,
                samples_per_chunk,
                self.video_last_delta,
            );
            let moov_box = build_moov_box(video, &tables, None, video_config, metadata);
            return Self::write_counted(&mut self.writer, &mut self.bytes_written, &moov_box);
        }

        // Audio present - write interleaved mdat then moov
        let mut total_payload_size: u32 = 0;
        for sample in &self.video_samples {
            total_payload_size += sample.data.len() as u32;
        }
        for sample in &self.audio_samples {
            total_payload_size += sample.data.len() as u32;
        }

        let mdat_size = 8 + total_payload_size;
        Self::write_counted(&mut self.writer, &mut self.bytes_written, &mdat_size.to_be_bytes())?;
        Self::write_counted(&mut self.writer, &mut self.bytes_written, b"mdat")?;

        // Write interleaved samples and collect chunk offsets
        let schedule = self.compute_interleave_schedule();
        let mut video_chunk_offsets = Vec::with_capacity(self.video_samples.len());
        let mut audio_chunk_offsets = Vec::with_capacity(self.audio_samples.len());
        let mut cursor = ftyp_len + 8;  // After ftyp + mdat header

        for (_, kind, idx) in schedule {
            match kind {
                TrackKind::Video => {
                    video_chunk_offsets.push(cursor);
                    let sample = &self.video_samples[idx];
                    let sample_len = sample.data.len() as u32;
                    Self::write_counted(&mut self.writer, &mut self.bytes_written, &sample.data)?;
                    cursor += sample_len;
                }
                TrackKind::Audio => {
                    audio_chunk_offsets.push(cursor);
                    let sample = &self.audio_samples[idx];
                    let sample_len = sample.data.len() as u32;
                    Self::write_counted(&mut self.writer, &mut self.bytes_written, &sample.data)?;
                    cursor += sample_len;
                }
            }
        }

        let video_tables = SampleTables::from_samples(
            &self.video_samples,
            video_chunk_offsets,
            1,
            self.video_last_delta,
        );
        let audio_tables = SampleTables::from_samples(
            &self.audio_samples,
            audio_chunk_offsets,
            1,
            self.audio_last_delta,
        );

        let audio_track = self.audio_track.as_ref().expect("audio_present implies track");
        let moov_box = build_moov_box(
            video,
            &video_tables,
            Some((audio_track, &audio_tables)),
            video_config,
            metadata,
        );
        Self::write_counted(&mut self.writer, &mut self.bytes_written, &moov_box)
    }

    fn finalize_fast_start(&mut self, video: &Mp4VideoTrack, metadata: Option<&Metadata>, video_config: &VideoConfig) -> io::Result<()> {
        let ftyp_box = build_ftyp_box();
        let ftyp_len = ftyp_box.len() as u32;

        // Calculate total mdat payload size
        let mut mdat_payload_size: u32 = 0;
        for sample in &self.video_samples {
            mdat_payload_size += sample.data.len() as u32;
        }
        for sample in &self.audio_samples {
            mdat_payload_size += sample.data.len() as u32;
        }
        let mdat_header_size = 8u32;
        let mdat_total_size = mdat_header_size + mdat_payload_size;

        let audio_present = self.audio_track.is_some();

        // Build moov with placeholder offsets to measure its size
        let (placeholder_video_tables, placeholder_audio_tables) = if audio_present {
            // For fast-start with audio, we need to compute interleaved offsets
            // First, compute the interleave schedule
            let schedule = self.compute_interleave_schedule();
            
            // Placeholder offsets - will be recalculated after we know moov size
            let mut video_offsets = Vec::with_capacity(self.video_samples.len());
            let mut audio_offsets = Vec::with_capacity(self.audio_samples.len());
            let mut cursor = 0u32;
            for (_, kind, _) in &schedule {
                match kind {
                    TrackKind::Video => {
                        video_offsets.push(cursor);
                        cursor += 1; // placeholder
                    }
                    TrackKind::Audio => {
                        audio_offsets.push(cursor);
                        cursor += 1; // placeholder
                    }
                }
            }
            
            let video_tables = SampleTables::from_samples(&self.video_samples, video_offsets, 1, self.video_last_delta);
            let audio_tables = SampleTables::from_samples(&self.audio_samples, audio_offsets, 1, self.audio_last_delta);
            (video_tables, Some(audio_tables))
        } else {
            // Video-only: all samples in one chunk
            let chunk_offsets = if self.video_samples.is_empty() {
                Vec::new()
            } else {
                vec![0u32]  // Single placeholder chunk offset (will be replaced with real value)
            };
            let samples_per_chunk = if self.video_samples.is_empty() { 0 } else { self.video_samples.len() as u32 };
            let video_tables = SampleTables::from_samples(
                &self.video_samples,
                chunk_offsets,
                samples_per_chunk,
                self.video_last_delta,
            );
            (video_tables, None)
        };

        let placeholder_moov = if let Some(ref audio_tables) = placeholder_audio_tables {
            let audio_track = self.audio_track.as_ref().unwrap();
            build_moov_box(video, &placeholder_video_tables, Some((audio_track, audio_tables)), video_config, metadata)
        } else {
            build_moov_box(video, &placeholder_video_tables, None, video_config, metadata)
        };
        let moov_len = placeholder_moov.len() as u32;

        // Now we know: mdat starts at ftyp_len + moov_len
        let mdat_data_start = ftyp_len + moov_len + mdat_header_size;

        // Rebuild moov with correct offsets
        let (final_video_tables, final_audio_tables) = if audio_present {
            let schedule = self.compute_interleave_schedule();
            
            let mut video_offsets = Vec::with_capacity(self.video_samples.len());
            let mut audio_offsets = Vec::with_capacity(self.audio_samples.len());
            let mut cursor = mdat_data_start;
            
            for (_, kind, idx) in &schedule {
                match kind {
                    TrackKind::Video => {
                        video_offsets.push(cursor);
                        cursor += self.video_samples[*idx].data.len() as u32;
                    }
                    TrackKind::Audio => {
                        audio_offsets.push(cursor);
                        cursor += self.audio_samples[*idx].data.len() as u32;
                    }
                }
            }
            
            let video_tables = SampleTables::from_samples(&self.video_samples, video_offsets, 1, self.video_last_delta);
            let audio_tables = SampleTables::from_samples(&self.audio_samples, audio_offsets, 1, self.audio_last_delta);
            (video_tables, Some(audio_tables))
        } else {
            // Video only - all samples in one chunk
            let chunk_offsets = if self.video_samples.is_empty() {
                Vec::new()
            } else {
                vec![mdat_data_start]
            };
            let samples_per_chunk = if self.video_samples.is_empty() { 0 } else { self.video_samples.len() as u32 };
            let video_tables = SampleTables::from_samples(&self.video_samples, chunk_offsets, samples_per_chunk, self.video_last_delta);
            (video_tables, None)
        };

        let final_moov = if let Some(ref audio_tables) = final_audio_tables {
            let audio_track = self.audio_track.as_ref().unwrap();
            build_moov_box(video, &final_video_tables, Some((audio_track, audio_tables)), video_config, metadata)
        } else {
            build_moov_box(video, &final_video_tables, None, video_config, metadata)
        };

        // Write: ftyp → moov → mdat header → samples
        Self::write_counted(&mut self.writer, &mut self.bytes_written, &ftyp_box)?;
        Self::write_counted(&mut self.writer, &mut self.bytes_written, &final_moov)?;
        Self::write_counted(&mut self.writer, &mut self.bytes_written, &mdat_total_size.to_be_bytes())?;
        Self::write_counted(&mut self.writer, &mut self.bytes_written, b"mdat")?;

        // Write samples in interleaved order
        if audio_present {
            let schedule = self.compute_interleave_schedule();
            for (_, kind, idx) in schedule {
                match kind {
                    TrackKind::Video => {
                        Self::write_counted(&mut self.writer, &mut self.bytes_written, &self.video_samples[idx].data)?;
                    }
                    TrackKind::Audio => {
                        Self::write_counted(&mut self.writer, &mut self.bytes_written, &self.audio_samples[idx].data)?;
                    }
                }
            }
        } else {
            for sample in &self.video_samples {
                Self::write_counted(&mut self.writer, &mut self.bytes_written, &sample.data)?;
            }
        }

        Ok(())
    }

    fn compute_interleave_schedule(&self) -> Vec<(u64, TrackKind, usize)> {
        let mut schedule: Vec<(u64, TrackKind, usize)> = Vec::new();
        for (idx, sample) in self.video_samples.iter().enumerate() {
            schedule.push((sample.pts, TrackKind::Video, idx));
        }
        for (idx, sample) in self.audio_samples.iter().enumerate() {
            schedule.push((sample.pts, TrackKind::Audio, idx));
        }
        schedule.sort_by_key(|(pts, kind, idx)| {
            let kind_order = match kind {
                TrackKind::Video => 0u8,
                TrackKind::Audio => 1u8,
            };
            (*pts, kind_order, *idx)
        });
        schedule
    }
}

#[derive(Clone, Copy)]
enum TrackKind {
    Video,
    Audio,
}

fn adts_to_raw(frame: &[u8]) -> Option<&[u8]> {
    if frame.len() < 7 {
        return None;
    }

    // Syncword 0xFFF (12 bits)
    if frame[0] != 0xFF || (frame[1] & 0xF0) != 0xF0 {
        return None;
    }

    let protection_absent = (frame[1] & 0x01) != 0;
    let header_len = if protection_absent { 7 } else { 9 };
    if frame.len() < header_len {
        return None;
    }

    // aac_frame_length: 13 bits across bytes 3..5 (includes header)
    let aac_frame_length: usize = (((frame[3] & 0x03) as usize) << 11)
        | ((frame[4] as usize) << 3)
        | (((frame[5] & 0xE0) as usize) >> 5);

    if aac_frame_length < header_len || aac_frame_length > frame.len() {
        return None;
    }

    Some(&frame[header_len..aac_frame_length])
}

fn build_moov_box(
    video: &Mp4VideoTrack,
    video_tables: &SampleTables,
    audio: Option<(&Mp4AudioTrack, &SampleTables)>,
    video_config: &VideoConfig,
    metadata: Option<&Metadata>,
) -> Vec<u8> {
    let mvhd_payload = build_mvhd_payload();
    let mvhd_box = build_box(b"mvhd", &mvhd_payload);
    let trak_box = build_trak_box(video, video_tables, video_config);

    let mut payload = Vec::new();
    payload.extend_from_slice(&mvhd_box);
    payload.extend_from_slice(&trak_box);
    if let Some((audio_track, audio_tables)) = audio {
        let audio_trak = build_audio_trak_box(audio_track, audio_tables);
        payload.extend_from_slice(&audio_trak);
    }
    
    // Add metadata (udta box) if present
    if let Some(meta) = metadata {
        let udta_box = build_udta_box(meta);
        if !udta_box.is_empty() {
            payload.extend_from_slice(&udta_box);
        }
    }
    
    build_box(b"moov", &payload)
}

fn build_audio_trak_box(audio: &Mp4AudioTrack, tables: &SampleTables) -> Vec<u8> {
    let tkhd_box = build_audio_tkhd_box();
    let mdia_box = build_audio_mdia_box(audio, tables);

    let mut payload = Vec::new();
    payload.extend_from_slice(&tkhd_box);
    payload.extend_from_slice(&mdia_box);
    build_box(b"trak", &payload)
}

fn build_audio_tkhd_box() -> Vec<u8> {
    build_tkhd_box_with_id(2, 0x0100, 0, 0)
}

fn build_audio_mdia_box(audio: &Mp4AudioTrack, tables: &SampleTables) -> Vec<u8> {
    let mdhd_box = build_mdhd_box_with_timescale(MEDIA_TIMESCALE);
    let hdlr_box = build_sound_hdlr_box();
    let minf_box = build_audio_minf_box(audio, tables);

    let mut payload = Vec::new();
    payload.extend_from_slice(&mdhd_box);
    payload.extend_from_slice(&hdlr_box);
    payload.extend_from_slice(&minf_box);
    build_box(b"mdia", &payload)
}

fn build_audio_minf_box(audio: &Mp4AudioTrack, tables: &SampleTables) -> Vec<u8> {
    let smhd_box = build_smhd_box();
    let dinf_box = build_dinf_box();
    let stbl_box = build_audio_stbl_box(audio, tables);

    let mut payload = Vec::new();
    payload.extend_from_slice(&smhd_box);
    payload.extend_from_slice(&dinf_box);
    payload.extend_from_slice(&stbl_box);
    build_box(b"minf", &payload)
}

fn build_audio_stbl_box(audio: &Mp4AudioTrack, tables: &SampleTables) -> Vec<u8> {
    let stsd_box = build_audio_stsd_box(audio);
    let stts_box = build_stts_box(&tables.durations);
    let stsc_box = build_stsc_box(tables.samples_per_chunk, tables.chunk_offsets.len() as u32);
    let stsz_box = build_stsz_box(&tables.sizes);
    let stco_box = build_stco_box(&tables.chunk_offsets);

    let mut payload = Vec::new();
    payload.extend_from_slice(&stsd_box);
    payload.extend_from_slice(&stts_box);
    payload.extend_from_slice(&stsc_box);
    payload.extend_from_slice(&stsz_box);
    payload.extend_from_slice(&stco_box);
    build_box(b"stbl", &payload)
}

fn build_audio_stsd_box(audio: &Mp4AudioTrack) -> Vec<u8> {
    let sample_entry_box = match audio.codec {
        AudioCodec::Aac => build_mp4a_box(audio),
        AudioCodec::Opus => build_opus_box(audio),
        AudioCodec::None => build_mp4a_box(audio), // Fallback, shouldn't happen
    };

    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&sample_entry_box);
    build_box(b"stsd", &payload)
}

fn build_mp4a_box(audio: &Mp4AudioTrack) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&[0u8; 6]);
    payload.extend_from_slice(&1u16.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&audio.channels.to_be_bytes());
    payload.extend_from_slice(&16u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    let rate_fixed = (audio.sample_rate as u32) << 16;
    payload.extend_from_slice(&rate_fixed.to_be_bytes());
    let esds = build_esds_box(audio);
    payload.extend_from_slice(&esds);
    build_box(b"mp4a", &payload)
}

fn build_esds_box(audio: &Mp4AudioTrack) -> Vec<u8> {
    let asc = build_audio_specific_config(audio.sample_rate, audio.channels);

    let mut dec_specific = Vec::new();
    dec_specific.push(0x05);
    dec_specific.push(asc.len() as u8);
    dec_specific.extend_from_slice(&asc);

    let mut dec_config_payload = Vec::new();
    dec_config_payload.push(0x40);
    dec_config_payload.push(0x15);
    dec_config_payload.extend_from_slice(&[0x00, 0x00, 0x00]);
    dec_config_payload.extend_from_slice(&0u32.to_be_bytes());
    dec_config_payload.extend_from_slice(&0u32.to_be_bytes());
    dec_config_payload.extend_from_slice(&dec_specific);

    let mut dec_config = Vec::new();
    dec_config.push(0x04);
    dec_config.push(dec_config_payload.len() as u8);
    dec_config.extend_from_slice(&dec_config_payload);

    let sl_config = [0x06u8, 0x01u8, 0x02u8];

    let mut es_payload = Vec::new();
    es_payload.extend_from_slice(&1u16.to_be_bytes());
    es_payload.push(0);
    es_payload.extend_from_slice(&dec_config);
    es_payload.extend_from_slice(&sl_config);

    let mut es_desc = Vec::new();
    es_desc.push(0x03);
    es_desc.push(es_payload.len() as u8);
    es_desc.extend_from_slice(&es_payload);

    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&es_desc);
    build_box(b"esds", &payload)
}

fn build_audio_specific_config(sample_rate: u32, channels: u16) -> [u8; 2] {
    let sfi = match sample_rate {
        96000 => 0,
        88200 => 1,
        64000 => 2,
        48000 => 3,
        44100 => 4,
        32000 => 5,
        24000 => 6,
        22050 => 7,
        16000 => 8,
        12000 => 9,
        11025 => 10,
        8000 => 11,
        7350 => 12,
        _ => 4,
    };
    let aot = 2u8;
    let chan = (channels.min(15) as u8) & 0x0f;
    let byte0 = (aot << 3) | (sfi >> 1);
    let byte1 = ((sfi & 1) << 7) | (chan << 3);
    [byte0, byte1]
}

/// Build an Opus sample entry box.
fn build_opus_box(audio: &Mp4AudioTrack) -> Vec<u8> {
    let mut payload = Vec::new();
    // Reserved (6 bytes)
    payload.extend_from_slice(&[0u8; 6]);
    // Data reference index
    payload.extend_from_slice(&1u16.to_be_bytes());
    // Reserved (2 x u32)
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    // Channel count
    payload.extend_from_slice(&audio.channels.to_be_bytes());
    // Sample size (16 bits)
    payload.extend_from_slice(&16u16.to_be_bytes());
    // Pre-defined
    payload.extend_from_slice(&0u16.to_be_bytes());
    // Reserved
    payload.extend_from_slice(&0u16.to_be_bytes());
    // Sample rate (fixed point 16.16, always 48000 for Opus)
    let rate_fixed = (OPUS_SAMPLE_RATE as u32) << 16;
    payload.extend_from_slice(&rate_fixed.to_be_bytes());
    
    // dOps box (Opus decoder configuration)
    let dops = build_dops_box(audio);
    payload.extend_from_slice(&dops);
    
    build_box(b"Opus", &payload)
}

/// Build the dOps (Opus Decoder Configuration) box.
///
/// Structure per ISO/IEC 14496-3 Amendment 4:
/// - Version (1 byte) = 0
/// - OutputChannelCount (1 byte)
/// - PreSkip (2 bytes, big-endian)
/// - InputSampleRate (4 bytes, big-endian)
/// - OutputGain (2 bytes, signed, big-endian)
/// - ChannelMappingFamily (1 byte)
/// - If ChannelMappingFamily != 0:
///   - StreamCount (1 byte)
///   - CoupledCount (1 byte)
///   - ChannelMapping (OutputChannelCount bytes)
fn build_dops_box(audio: &Mp4AudioTrack) -> Vec<u8> {
    let config = OpusConfig::default()
        .with_channels(audio.channels as u8);
    
    let mut payload = Vec::new();
    // Version = 0
    payload.push(config.version);
    // OutputChannelCount
    payload.push(config.output_channel_count);
    // PreSkip (big-endian)
    payload.extend_from_slice(&config.pre_skip.to_be_bytes());
    // InputSampleRate (big-endian)
    payload.extend_from_slice(&config.input_sample_rate.to_be_bytes());
    // OutputGain (signed, big-endian)
    payload.extend_from_slice(&config.output_gain.to_be_bytes());
    // ChannelMappingFamily
    payload.push(config.channel_mapping_family);
    
    // Extended channel mapping for family != 0
    if config.channel_mapping_family != 0 {
        payload.push(config.stream_count.unwrap_or(1));
        payload.push(config.coupled_count.unwrap_or(0));
        if let Some(mapping) = &config.channel_mapping {
            payload.extend_from_slice(mapping);
        } else {
            // Default mapping for stereo
            for i in 0..config.output_channel_count {
                payload.push(i);
            }
        }
    }
    
    build_box(b"dOps", &payload)
}

fn build_trak_box(video: &Mp4VideoTrack, tables: &SampleTables, video_config: &VideoConfig) -> Vec<u8> {
    let tkhd_box = build_tkhd_box(video);
    let mdia_box = build_mdia_box(video, tables, video_config);

    let mut payload = Vec::new();
    payload.extend_from_slice(&tkhd_box);
    payload.extend_from_slice(&mdia_box);
    build_box(b"trak", &payload)
}

fn build_mdia_box(video: &Mp4VideoTrack, tables: &SampleTables, video_config: &VideoConfig) -> Vec<u8> {
    let mdhd_box = build_mdhd_box();
    let hdlr_box = build_hdlr_box();
    let minf_box = build_minf_box(video, tables, video_config);

    let mut payload = Vec::new();
    payload.extend_from_slice(&mdhd_box);
    payload.extend_from_slice(&hdlr_box);
    payload.extend_from_slice(&minf_box);
    build_box(b"mdia", &payload)
}

fn build_minf_box(video: &Mp4VideoTrack, tables: &SampleTables, video_config: &VideoConfig) -> Vec<u8> {
    let vmhd_box = build_vmhd_box();
    let dinf_box = build_dinf_box();
    let stbl_box = build_stbl_box(video, tables, video_config);

    let mut payload = Vec::new();
    payload.extend_from_slice(&vmhd_box);
    payload.extend_from_slice(&dinf_box);
    payload.extend_from_slice(&stbl_box);
    build_box(b"minf", &payload)
}

fn build_stbl_box(video: &Mp4VideoTrack, tables: &SampleTables, video_config: &VideoConfig) -> Vec<u8> {
    let stsd_box = build_stsd_box(video, video_config);
    let stts_box = build_stts_box(&tables.durations);
    let stsc_box = build_stsc_box(tables.samples_per_chunk, tables.chunk_offsets.len() as u32);
    let stsz_box = build_stsz_box(&tables.sizes);
    let stco_box = build_stco_box(&tables.chunk_offsets);

    let mut payload = Vec::new();
    payload.extend_from_slice(&stsd_box);
    payload.extend_from_slice(&stts_box);
    // Add ctts box if B-frames are present (pts != dts for any sample)
    if tables.has_bframes {
        let ctts_box = build_ctts_box(&tables.cts_offsets);
        payload.extend_from_slice(&ctts_box);
    }
    payload.extend_from_slice(&stsc_box);
    payload.extend_from_slice(&stsz_box);
    payload.extend_from_slice(&stco_box);
    if !tables.keyframes.is_empty() {
        let stss_box = build_stss_box(&tables.keyframes);
        payload.extend_from_slice(&stss_box);
    }
    build_box(b"stbl", &payload)
}

fn build_stsd_box(video: &Mp4VideoTrack, video_config: &VideoConfig) -> Vec<u8> {
    let sample_entry = match video_config {
        VideoConfig::Avc(avc_config) => build_avc1_box(video, avc_config),
        VideoConfig::Hevc(hevc_config) => build_hvc1_box(video, hevc_config),
        VideoConfig::Av1(av1_config) => build_av01_box(video, av1_config),
    };

    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&sample_entry);
    build_box(b"stsd", &payload)
}

fn build_stts_box(durations: &[u32]) -> Vec<u8> {
    let mut entries: Vec<(u32, u32)> = Vec::new();
    for &duration in durations {
        if let Some(last) = entries.last_mut() {
            if last.1 == duration {
                last.0 += 1;
                continue;
            }
        }
        entries.push((1u32, duration));
    }

    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    for (count, delta) in entries {
        payload.extend_from_slice(&count.to_be_bytes());
        payload.extend_from_slice(&delta.to_be_bytes());
    }
    build_box(b"stts", &payload)
}

fn build_stsc_box(samples_per_chunk: u32, chunk_count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());

    if chunk_count == 0 || samples_per_chunk == 0 {
        payload.extend_from_slice(&0u32.to_be_bytes());
        return build_box(b"stsc", &payload);
    }

    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&samples_per_chunk.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    build_box(b"stsc", &payload)
}

fn build_stsz_box(sizes: &[u32]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&(sizes.len() as u32).to_be_bytes());
    for size in sizes {
        payload.extend_from_slice(&size.to_be_bytes());
    }
    build_box(b"stsz", &payload)
}

fn build_stco_box(chunk_offsets: &[u32]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());

    payload.extend_from_slice(&(chunk_offsets.len() as u32).to_be_bytes());
    for offset in chunk_offsets {
        payload.extend_from_slice(&offset.to_be_bytes());
    }
    build_box(b"stco", &payload)
}

fn build_stss_box(keyframes: &[u32]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&(keyframes.len() as u32).to_be_bytes());
    for index in keyframes {
        payload.extend_from_slice(&index.to_be_bytes());
    }
    build_box(b"stss", &payload)
}

/// Build ctts (Composition Time to Sample) box for B-frame support.
/// Uses version 1 which supports signed offsets (required for some B-frame patterns).
fn build_ctts_box(cts_offsets: &[i32]) -> Vec<u8> {
    // Run-length encode the offsets
    let mut entries: Vec<(u32, i32)> = Vec::new();
    for &offset in cts_offsets {
        if let Some(last) = entries.last_mut() {
            if last.1 == offset {
                last.0 += 1;
                continue;
            }
        }
        entries.push((1, offset));
    }

    let mut payload = Vec::new();
    // Version 1 (supports signed offsets), flags 0
    payload.extend_from_slice(&0x0100_0000_u32.to_be_bytes());
    payload.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    for (count, offset) in entries {
        payload.extend_from_slice(&count.to_be_bytes());
        payload.extend_from_slice(&offset.to_be_bytes());
    }
    build_box(b"ctts", &payload)
}

fn build_avc1_box(video: &Mp4VideoTrack, avc_config: &AvcConfig) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&[0u8; 6]);
    payload.extend_from_slice(&1u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&video.width.to_be_bytes());
    payload.extend_from_slice(&video.height.to_be_bytes());
    payload.extend_from_slice(&0x0048_0000_u32.to_be_bytes());
    payload.extend_from_slice(&0x0048_0000_u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&1u16.to_be_bytes());
    payload.extend_from_slice(&[0u8; 32]);
    payload.extend_from_slice(&0x0018u16.to_be_bytes());
    payload.extend_from_slice(&0xffffu16.to_be_bytes());
    let avc_c_box = build_avcc_box(avc_config);
    payload.extend_from_slice(&avc_c_box);
    build_box(b"avc1", &payload)
}

fn build_avcc_box(avc_config: &AvcConfig) -> Vec<u8> {
    let mut payload = Vec::new();

    let (profile_indication, profile_compat, level_indication) = if avc_config.sps.len() >= 4 {
        (avc_config.sps[1], avc_config.sps[2], avc_config.sps[3])
    } else {
        (0x42, 0x00, 0x1e)
    };

    payload.push(1);
    payload.push(profile_indication);
    payload.push(profile_compat);
    payload.push(level_indication);
    payload.push(0xff);
    payload.push(0xe1);
    payload.extend_from_slice(&(avc_config.sps.len() as u16).to_be_bytes());
    payload.extend_from_slice(&avc_config.sps);
    payload.push(1);
    payload.extend_from_slice(&(avc_config.pps.len() as u16).to_be_bytes());
    payload.extend_from_slice(&avc_config.pps);
    build_box(b"avcC", &payload)
}

/// Build an hvc1 sample entry box for HEVC video.
fn build_hvc1_box(video: &Mp4VideoTrack, hevc_config: &HevcConfig) -> Vec<u8> {
    let mut payload = Vec::new();
    // Reserved (6 bytes)
    payload.extend_from_slice(&[0u8; 6]);
    // Data reference index
    payload.extend_from_slice(&1u16.to_be_bytes());
    // Pre-defined + reserved
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    // Width and height
    payload.extend_from_slice(&video.width.to_be_bytes());
    payload.extend_from_slice(&video.height.to_be_bytes());
    // Horizontal/vertical resolution (72 dpi fixed point)
    payload.extend_from_slice(&0x0048_0000_u32.to_be_bytes());
    payload.extend_from_slice(&0x0048_0000_u32.to_be_bytes());
    // Reserved
    payload.extend_from_slice(&0u32.to_be_bytes());
    // Frame count
    payload.extend_from_slice(&1u16.to_be_bytes());
    // Compressor name (32 bytes, empty)
    payload.extend_from_slice(&[0u8; 32]);
    // Depth
    payload.extend_from_slice(&0x0018u16.to_be_bytes());
    // Pre-defined
    payload.extend_from_slice(&0xffffu16.to_be_bytes());
    // hvcC box
    let hvcc_box = build_hvcc_box(hevc_config);
    payload.extend_from_slice(&hvcc_box);
    build_box(b"hvc1", &payload)
}

/// Build an hvcC configuration box for HEVC.
fn build_hvcc_box(hevc_config: &HevcConfig) -> Vec<u8> {
    let mut payload = Vec::new();

    // Extract profile/tier/level from SPS
    let general_profile_space = hevc_config.general_profile_space();
    let general_tier_flag = hevc_config.general_tier_flag();
    let general_profile_idc = hevc_config.general_profile_idc();
    let general_level_idc = hevc_config.general_level_idc();

    // configurationVersion = 1
    payload.push(1);
    
    // general_profile_space (2) + general_tier_flag (1) + general_profile_idc (5)
    let byte1 = (general_profile_space << 6) 
              | (if general_tier_flag { 0x20 } else { 0 })
              | (general_profile_idc & 0x1f);
    payload.push(byte1);
    
    // general_profile_compatibility_flags (4 bytes)
    // For simplicity, set Main profile compatibility (bit 1)
    payload.extend_from_slice(&[0x60, 0x00, 0x00, 0x00]);
    
    // general_constraint_indicator_flags (6 bytes)
    payload.extend_from_slice(&[0x90, 0x00, 0x00, 0x00, 0x00, 0x00]);
    
    // general_level_idc
    payload.push(general_level_idc);
    
    // min_spatial_segmentation_idc (12 bits) with reserved (4 bits)
    payload.extend_from_slice(&[0xf0, 0x00]);
    
    // parallelismType (2 bits) with reserved (6 bits)
    payload.push(0xfc);
    
    // chromaFormat (2 bits) with reserved (6 bits) - assume 4:2:0
    payload.push(0xfd);
    
    // bitDepthLumaMinus8 (3 bits) with reserved (5 bits) - assume 8-bit
    payload.push(0xf8);
    
    // bitDepthChromaMinus8 (3 bits) with reserved (5 bits) - assume 8-bit
    payload.push(0xf8);
    
    // avgFrameRate (16 bits) - 0 = unspecified
    payload.extend_from_slice(&0u16.to_be_bytes());
    
    // constantFrameRate (2) + numTemporalLayers (3) + temporalIdNested (1) + lengthSizeMinusOne (2)
    // lengthSizeMinusOne = 3 (4-byte NAL length)
    payload.push(0x03);
    
    // numOfArrays = 3 (VPS, SPS, PPS)
    payload.push(3);
    
    // VPS array
    payload.push(0x20 | 32); // array_completeness=1 + nal_unit_type=32 (VPS)
    payload.extend_from_slice(&1u16.to_be_bytes()); // numNalus = 1
    payload.extend_from_slice(&(hevc_config.vps.len() as u16).to_be_bytes());
    payload.extend_from_slice(&hevc_config.vps);
    
    // SPS array
    payload.push(0x20 | 33); // array_completeness=1 + nal_unit_type=33 (SPS)
    payload.extend_from_slice(&1u16.to_be_bytes()); // numNalus = 1
    payload.extend_from_slice(&(hevc_config.sps.len() as u16).to_be_bytes());
    payload.extend_from_slice(&hevc_config.sps);
    
    // PPS array
    payload.push(0x20 | 34); // array_completeness=1 + nal_unit_type=34 (PPS)
    payload.extend_from_slice(&1u16.to_be_bytes()); // numNalus = 1
    payload.extend_from_slice(&(hevc_config.pps.len() as u16).to_be_bytes());
    payload.extend_from_slice(&hevc_config.pps);
    
    build_box(b"hvcC", &payload)
}

/// Build an av01 sample entry box for AV1 video.
fn build_av01_box(video: &Mp4VideoTrack, av1_config: &Av1Config) -> Vec<u8> {
    let mut payload = Vec::new();
    // Reserved (6 bytes)
    payload.extend_from_slice(&[0u8; 6]);
    // Data reference index
    payload.extend_from_slice(&1u16.to_be_bytes());
    // Pre-defined + reserved
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    // Width and height
    payload.extend_from_slice(&video.width.to_be_bytes());
    payload.extend_from_slice(&video.height.to_be_bytes());
    // Horizontal/vertical resolution (72 dpi fixed point)
    payload.extend_from_slice(&0x0048_0000_u32.to_be_bytes());
    payload.extend_from_slice(&0x0048_0000_u32.to_be_bytes());
    // Reserved
    payload.extend_from_slice(&0u32.to_be_bytes());
    // Frame count
    payload.extend_from_slice(&1u16.to_be_bytes());
    // Compressor name (32 bytes, empty)
    payload.extend_from_slice(&[0u8; 32]);
    // Depth (24-bit)
    payload.extend_from_slice(&0x0018u16.to_be_bytes());
    // Pre-defined (-1)
    payload.extend_from_slice(&0xffffu16.to_be_bytes());
    // av1C box
    let av1c_box = build_av1c_box(av1_config);
    payload.extend_from_slice(&av1c_box);
    build_box(b"av01", &payload)
}

/// Build an av1C configuration box for AV1.
///
/// ISO/IEC 14496-12:2022 and AV1 Codec ISO Media File Format Binding spec.
fn build_av1c_box(av1_config: &Av1Config) -> Vec<u8> {
    let mut payload = Vec::new();

    // Byte 0: marker (1) + version (7) = 0x81
    payload.push(0x81);
    
    // Byte 1: seq_profile (3) + seq_level_idx_0 (5)
    let byte1 = ((av1_config.seq_profile & 0x07) << 5) 
              | (av1_config.seq_level_idx & 0x1f);
    payload.push(byte1);
    
    // Byte 2: seq_tier_0 (1) + high_bitdepth (1) + twelve_bit (1) + monochrome (1) 
    //       + chroma_subsampling_x (1) + chroma_subsampling_y (1) + chroma_sample_position (2)
    let byte2 = ((av1_config.seq_tier & 0x01) << 7)
              | (if av1_config.high_bitdepth { 0x40 } else { 0 })
              | (if av1_config.twelve_bit { 0x20 } else { 0 })
              | (if av1_config.monochrome { 0x10 } else { 0 })
              | (if av1_config.chroma_subsampling_x { 0x08 } else { 0 })
              | (if av1_config.chroma_subsampling_y { 0x04 } else { 0 })
              | (av1_config.chroma_sample_position & 0x03);
    payload.push(byte2);
    
    // Byte 3: reserved (1) + initial_presentation_delay_present (1) + reserved (4) OR initial_presentation_delay_minus_one (4)
    // Set to 0 (no initial presentation delay)
    payload.push(0x00);
    
    // configOBUs: Append the Sequence Header OBU
    payload.extend_from_slice(&av1_config.sequence_header);
    
    build_box(b"av1C", &payload)
}

fn build_vmhd_box() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    build_box(b"vmhd", &payload)
}

fn build_dinf_box() -> Vec<u8> {
    let dref_box = build_dref_box();
    build_box(b"dinf", &dref_box)
}

fn build_dref_box() -> Vec<u8> {
    let url_box = build_url_box();
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&url_box);
    build_box(b"dref", &payload)
}

fn build_url_box() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1u32.to_be_bytes());
    build_box(b"url ", &payload)
}

fn build_mdhd_box() -> Vec<u8> {
    build_mdhd_box_with_timescale(MEDIA_TIMESCALE)
}

fn build_mdhd_box_with_timescale(timescale: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&timescale.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0x55c4u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    build_box(b"mdhd", &payload)
}

fn build_hdlr_box() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(b"vide");
    payload.extend_from_slice(&[0u8; 12]);
    payload.extend_from_slice(b"VideoHandler");
    payload.push(0);
    build_box(b"hdlr", &payload)
}

fn build_sound_hdlr_box() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(b"soun");
    payload.extend_from_slice(&[0u8; 12]);
    payload.extend_from_slice(b"SoundHandler");
    payload.push(0);
    build_box(b"hdlr", &payload)
}

fn build_smhd_box() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    build_box(b"smhd", &payload)
}

fn build_tkhd_box(video: &Mp4VideoTrack) -> Vec<u8> {
    build_tkhd_box_with_id(1, 0, video.width, video.height)
}

fn build_tkhd_box_with_id(track_id: u32, volume: u16, width: u32, height: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&track_id.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u64.to_be_bytes());
    payload.extend_from_slice(&0u64.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&volume.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    let matrix = [
        0x0001_0000_u32,
        0,
        0,
        0,
        0x0001_0000_u32,
        0,
        0,
        0,
        0x4000_0000_u32,
    ];
    for value in matrix {
        payload.extend_from_slice(&value.to_be_bytes());
    }
    let width_fixed = (width << 16) as u32;
    let height_fixed = (height << 16) as u32;
    payload.extend_from_slice(&width_fixed.to_be_bytes());
    payload.extend_from_slice(&height_fixed.to_be_bytes());
    build_box(b"tkhd", &payload)
}

fn build_ftyp_box() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"isom");
    payload.extend_from_slice(&0x200_u32.to_be_bytes());
    payload.extend_from_slice(b"isommp41");
    build_box(b"ftyp", &payload)
}

fn build_mvhd_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&MOVIE_TIMESCALE.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
    payload.extend_from_slice(&0x0100u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u64.to_be_bytes());
    let matrix = [
        0x0001_0000_u32,
        0,
        0,
        0,
        0x0001_0000_u32,
        0,
        0,
        0,
        0x4000_0000_u32,
    ];
    for value in matrix {
        payload.extend_from_slice(&value.to_be_bytes());
    }
    for _ in 0..6 {
        payload.extend_from_slice(&0u32.to_be_bytes());
    }
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload
}

fn build_box(typ: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let length = (8 + payload.len()) as u32;
    let mut buffer = Vec::with_capacity(payload.len() + 8);
    buffer.extend_from_slice(&length.to_be_bytes());
    buffer.extend_from_slice(typ);
    buffer.extend_from_slice(payload);
    buffer
}

// ============================================================================
// Metadata (udta/meta/ilst) box building
// ============================================================================

fn build_udta_box(metadata: &Metadata) -> Vec<u8> {
    let mut ilst_payload = Vec::new();
    
    if let Some(title) = &metadata.title {
        ilst_payload.extend_from_slice(&build_ilst_string_item(b"\xa9nam", title));
    }
    
    if let Some(creation_time) = metadata.creation_time {
        // Format as ISO 8601: "YYYY-MM-DDTHH:MM:SSZ"
        let date_str = format_unix_timestamp(creation_time);
        ilst_payload.extend_from_slice(&build_ilst_string_item(b"\xa9day", &date_str));
    }
    
    if ilst_payload.is_empty() {
        return Vec::new();  // No metadata, skip udta entirely
    }
    
    let ilst_box = build_box(b"ilst", &ilst_payload);
    
    // meta box requires hdlr
    let hdlr_box = build_meta_hdlr_box();
    
    // meta is a full box (version + flags)
    let mut meta_payload = vec![0u8; 4];  // version=0, flags=0
    meta_payload.extend_from_slice(&hdlr_box);
    meta_payload.extend_from_slice(&ilst_box);
    let meta_box = build_box(b"meta", &meta_payload);
    
    build_box(b"udta", &meta_box)
}

fn build_ilst_string_item(atom_type: &[u8; 4], value: &str) -> Vec<u8> {
    // data box: type indicator (1 = UTF-8) + locale (0) + string
    let mut data_payload = Vec::new();
    data_payload.extend_from_slice(&[0, 0, 0, 1]);  // type = UTF-8
    data_payload.extend_from_slice(&[0, 0, 0, 0]);  // locale = 0
    data_payload.extend_from_slice(value.as_bytes());
    
    let data_box = build_box(b"data", &data_payload);
    build_box(atom_type, &data_box)
}

fn build_meta_hdlr_box() -> Vec<u8> {
    let mut payload = vec![0u8; 4];  // version + flags
    payload.extend_from_slice(&[0, 0, 0, 0]);  // pre_defined
    payload.extend_from_slice(b"mdir");  // handler_type (metadata directory)
    payload.extend_from_slice(b"appl");  // manufacturer
    payload.extend_from_slice(&[0, 0, 0, 0]);  // reserved
    payload.extend_from_slice(&[0, 0, 0, 0]);  // reserved
    payload.push(0);  // name (empty, null-terminated)
    build_box(b"hdlr", &payload)
}

fn format_unix_timestamp(unix_secs: u64) -> String {
    // Simple conversion - days since epoch calculation
    // This is approximate but good enough for metadata
    const SECS_PER_MIN: u64 = 60;
    const SECS_PER_HOUR: u64 = 3600;
    const SECS_PER_DAY: u64 = 86400;
    
    let days_since_epoch = unix_secs / SECS_PER_DAY;
    let remaining_secs = unix_secs % SECS_PER_DAY;
    
    let hours = remaining_secs / SECS_PER_HOUR;
    let minutes = (remaining_secs % SECS_PER_HOUR) / SECS_PER_MIN;
    let seconds = remaining_secs % SECS_PER_MIN;
    
    // Calculate year, month, day from days since 1970-01-01
    // Using a simplified algorithm
    let (year, month, day) = days_to_ymd(days_since_epoch);
    
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hours, minutes, seconds)
}

fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    // Simplified algorithm - works for dates from 1970 to ~2100
    let mut remaining_days = days as i64;
    let mut year = 1970u32;
    
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    
    let days_in_months: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    
    let mut month = 1u32;
    for &days_in_month in &days_in_months {
        if remaining_days < days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }
    
    let day = (remaining_days + 1) as u32;
    (year, month, day)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
