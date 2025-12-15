use std::fmt;
use std::io::{self, Write};

const DEFAULT_SPS_NAL: &[u8] = &[0x67, 0x42, 0x00, 0x1e, 0xda, 0x02, 0x80, 0x2d, 0x8b, 0x11];
const DEFAULT_PPS_NAL: &[u8] = &[0x68, 0xce, 0x38, 0x80];

const MOVIE_TIMESCALE: u32 = 1000;
/// Track/media timebase used for converting `pts` seconds into MP4 sample deltas.
///
/// v0.1.0 uses a 90 kHz media timescale (common for MP4/H.264 workflows).
pub const MEDIA_TIMESCALE: u32 = 90_000;

/// Minimal MP4 writer used by the early slices.
pub struct Mp4Writer<Writer> {
    writer: Writer,
    video_samples: Vec<SampleInfo>,
    video_prev_pts: Option<u64>,
    video_last_delta: Option<u32>,
    video_avc_config: Option<AvcConfig>,
    audio_track: Option<Mp4AudioTrack>,
    audio_samples: Vec<SampleInfo>,
    audio_prev_pts: Option<u64>,
    audio_last_delta: Option<u32>,
    finalized: bool,
}

#[derive(Clone, Debug)]
struct AvcConfig {
    sps: Vec<u8>,
    pps: Vec<u8>,
}

/// Simplified video track information used when writing the header.
pub struct Mp4VideoTrack {
    pub width: u32,
    pub height: u32,
}

pub struct Mp4AudioTrack {
    pub sample_rate: u32,
    pub channels: u16,
}

struct SampleInfo {
    pts: u64,
    data: Vec<u8>,
    is_keyframe: bool,
    duration: Option<u32>,
}

struct SampleTables {
    sample_count: u32,
    durations: Vec<u32>,
    sizes: Vec<u32>,
    keyframes: Vec<u32>,
    chunk_offsets: Vec<u32>,
    samples_per_chunk: u32,
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
        Self {
            sample_count,
            durations,
            sizes,
            keyframes,
            chunk_offsets,
            samples_per_chunk,
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
    /// Audio sample is not a valid ADTS frame.
    InvalidAdts,
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
            Mp4WriterError::InvalidAdts => write!(f, "invalid ADTS frame"),
            Mp4WriterError::AudioNotEnabled => write!(f, "audio track not enabled"),
            Mp4WriterError::DurationOverflow => write!(f, "sample duration overflow"),
            Mp4WriterError::AlreadyFinalized => write!(f, "writer already finalised"),
        }
    }
}

impl std::error::Error for Mp4WriterError {}

impl<Writer: Write> Mp4Writer<Writer> {
    /// Wraps the provided writer for MP4 container output.
    pub fn new(writer: Writer) -> Self {
        Self {
            writer,
            video_samples: Vec::new(),
            video_prev_pts: None,
            video_last_delta: None,
            video_avc_config: None,
            audio_track: None,
            audio_samples: Vec::new(),
            audio_prev_pts: None,
            audio_last_delta: None,
            finalized: false,
        }
    }

    pub fn enable_audio(&mut self, track: Mp4AudioTrack) {
        self.audio_track = Some(track);
    }

    /// Queues a video sample for later `mdat` emission.
    pub fn write_video_sample(
        &mut self,
        pts: u64,
        data: &[u8],
        is_keyframe: bool,
    ) -> Result<(), Mp4WriterError> {
        if self.finalized {
            return Err(Mp4WriterError::AlreadyFinalized);
        }
        if let Some(prev) = self.video_prev_pts {
            if pts <= prev {
                return Err(Mp4WriterError::NonIncreasingTimestamp);
            }
            let delta = pts - prev;
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
            let avc_config = extract_avc_config_from_keyframe(data);
            if avc_config.is_none() {
                return Err(Mp4WriterError::FirstFrameMissingSpsPps);
            }
            self.video_avc_config = avc_config;
        }

        let converted = annexb_to_avcc(data);
        if converted.len() > u32::MAX as usize {
            return Err(Mp4WriterError::DurationOverflow);
        }

        self.video_samples.push(SampleInfo {
            pts,
            data: converted,
            is_keyframe,
            duration: None,
        });
        self.video_prev_pts = Some(pts);
        Ok(())
    }

    pub fn write_audio_sample(&mut self, pts: u64, data: &[u8]) -> Result<(), Mp4WriterError> {
        if self.finalized {
            return Err(Mp4WriterError::AlreadyFinalized);
        }
        if self.audio_track.is_none() {
            return Err(Mp4WriterError::AudioNotEnabled);
        }

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

        let raw = adts_to_raw(data).ok_or(Mp4WriterError::InvalidAdts)?;
        if raw.len() > u32::MAX as usize {
            return Err(Mp4WriterError::DurationOverflow);
        }

        self.audio_samples.push(SampleInfo {
            pts,
            data: raw.to_vec(),
            is_keyframe: false,
            duration: None,
        });
        self.audio_prev_pts = Some(pts);
        Ok(())
    }

    /// Finalises the MP4 file by writing the header boxes and sample data.
    pub fn finalize(&mut self, video: &Mp4VideoTrack) -> io::Result<()> {
        if self.finalized {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "mp4 writer already finalised",
            ));
        }
        self.finalized = true;
        let ftyp_box = build_ftyp_box();
        let ftyp_len = ftyp_box.len() as u32;
        self.writer.write_all(&ftyp_box)?;

        let audio_present = self.audio_track.is_some();

        let avc_config = self
            .video_avc_config
            .clone()
            .or_else(|| if self.video_samples.is_empty() { Some(default_avc_config()) } else { None })
            .unwrap_or_else(default_avc_config);

        if !audio_present {
            let chunk_offset = if !self.video_samples.is_empty() {
                let mut payload_size: u32 = 0;
                for sample in &self.video_samples {
                    payload_size += sample.data.len() as u32;
                }

                let mdat_size = 8 + payload_size;
                self.writer.write_all(&mdat_size.to_be_bytes())?;
                self.writer.write_all(b"mdat")?;
                for sample in &self.video_samples {
                    self.writer.write_all(&sample.data)?;
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
            let moov_box = build_moov_box(video, &tables, None, &avc_config);
            return self.writer.write_all(&moov_box);
        }

        let mut total_payload_size: u32 = 0;
        for sample in &self.video_samples {
            total_payload_size += sample.data.len() as u32;
        }
        for sample in &self.audio_samples {
            total_payload_size += sample.data.len() as u32;
        }

        let mdat_size = 8 + total_payload_size;
        self.writer.write_all(&mdat_size.to_be_bytes())?;
        self.writer.write_all(b"mdat")?;

        #[derive(Clone, Copy)]
        enum TrackKind {
            Video,
            Audio,
        }

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

        let mut video_chunk_offsets = Vec::with_capacity(self.video_samples.len());
        let mut audio_chunk_offsets = Vec::with_capacity(self.audio_samples.len());
        let mut cursor = ftyp_len + 8;

        for (_, kind, idx) in schedule {
            match kind {
                TrackKind::Video => {
                    video_chunk_offsets.push(cursor);
                    let sample = &self.video_samples[idx];
                    self.writer.write_all(&sample.data)?;
                    cursor += sample.data.len() as u32;
                }
                TrackKind::Audio => {
                    audio_chunk_offsets.push(cursor);
                    let sample = &self.audio_samples[idx];
                    self.writer.write_all(&sample.data)?;
                    cursor += sample.data.len() as u32;
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
            &avc_config,
        );
        self.writer.write_all(&moov_box)
    }
}

fn extract_avc_config_from_keyframe(data: &[u8]) -> Option<AvcConfig> {
    let mut sps: Option<&[u8]> = None;
    let mut pps: Option<&[u8]> = None;

    for nal in annexb_iter_nals(data) {
        if nal.is_empty() {
            continue;
        }
        let nal_type = nal[0] & 0x1f;
        if nal_type == 7 && sps.is_none() {
            sps = Some(nal);
        } else if nal_type == 8 && pps.is_none() {
            pps = Some(nal);
        }
        if sps.is_some() && pps.is_some() {
            break;
        }
    }

    Some(AvcConfig {
        sps: sps?.to_vec(),
        pps: pps?.to_vec(),
    })
}

fn default_avc_config() -> AvcConfig {
    AvcConfig {
        sps: DEFAULT_SPS_NAL.to_vec(),
        pps: DEFAULT_PPS_NAL.to_vec(),
    }
}

fn annexb_to_avcc(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for nal in annexb_iter_nals(data) {
        if nal.is_empty() {
            continue;
        }
        let len = nal.len() as u32;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(nal);
    }

    if out.is_empty() {
        let len = data.len() as u32;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(data);
    }

    out
}

fn annexb_iter_nals(mut data: &[u8]) -> AnnexBNalIter<'_> {
    AnnexBNalIter { data, cursor: 0 }
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

struct AnnexBNalIter<'a> {
    data: &'a [u8],
    cursor: usize,
}

impl<'a> Iterator for AnnexBNalIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let (start_code_pos, start_code_len) = find_start_code(self.data, self.cursor)?;
        let nal_start = start_code_pos + start_code_len;
        let search_from = nal_start;
        let nal_end = match find_start_code(self.data, search_from) {
            Some((next_pos, _)) => next_pos,
            None => self.data.len(),
        };
        self.cursor = nal_end;
        Some(&self.data[nal_start..nal_end])
    }
}

fn find_start_code(data: &[u8], from: usize) -> Option<(usize, usize)> {
    if data.len() < 3 || from >= data.len() {
        return None;
    }

    let mut i = from;
    while i + 3 <= data.len() {
        if i + 4 <= data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            return Some((i, 4));
        }
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            return Some((i, 3));
        }
        i += 1;
    }
    None
}

fn build_moov_box(
    video: &Mp4VideoTrack,
    video_tables: &SampleTables,
    audio: Option<(&Mp4AudioTrack, &SampleTables)>,
    avc_config: &AvcConfig,
) -> Vec<u8> {
    let mvhd_payload = build_mvhd_payload();
    let mvhd_box = build_box(b"mvhd", &mvhd_payload);
    let trak_box = build_trak_box(video, video_tables, avc_config);

    let mut payload = Vec::new();
    payload.extend_from_slice(&mvhd_box);
    payload.extend_from_slice(&trak_box);
    if let Some((audio_track, audio_tables)) = audio {
        let audio_trak = build_audio_trak_box(audio_track, audio_tables);
        payload.extend_from_slice(&audio_trak);
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
    let mp4a_box = build_mp4a_box(audio);

    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&mp4a_box);
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

fn build_trak_box(video: &Mp4VideoTrack, tables: &SampleTables, avc_config: &AvcConfig) -> Vec<u8> {
    let tkhd_box = build_tkhd_box(video);
    let mdia_box = build_mdia_box(video, tables, avc_config);

    let mut payload = Vec::new();
    payload.extend_from_slice(&tkhd_box);
    payload.extend_from_slice(&mdia_box);
    build_box(b"trak", &payload)
}

fn build_mdia_box(video: &Mp4VideoTrack, tables: &SampleTables, avc_config: &AvcConfig) -> Vec<u8> {
    let mdhd_box = build_mdhd_box();
    let hdlr_box = build_hdlr_box();
    let minf_box = build_minf_box(video, tables, avc_config);

    let mut payload = Vec::new();
    payload.extend_from_slice(&mdhd_box);
    payload.extend_from_slice(&hdlr_box);
    payload.extend_from_slice(&minf_box);
    build_box(b"mdia", &payload)
}

fn build_minf_box(video: &Mp4VideoTrack, tables: &SampleTables, avc_config: &AvcConfig) -> Vec<u8> {
    let vmhd_box = build_vmhd_box();
    let dinf_box = build_dinf_box();
    let stbl_box = build_stbl_box(video, tables, avc_config);

    let mut payload = Vec::new();
    payload.extend_from_slice(&vmhd_box);
    payload.extend_from_slice(&dinf_box);
    payload.extend_from_slice(&stbl_box);
    build_box(b"minf", &payload)
}

fn build_stbl_box(video: &Mp4VideoTrack, tables: &SampleTables, avc_config: &AvcConfig) -> Vec<u8> {
    let stsd_box = build_stsd_box(video, avc_config);
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
    if !tables.keyframes.is_empty() {
        let stss_box = build_stss_box(&tables.keyframes);
        payload.extend_from_slice(&stss_box);
    }
    build_box(b"stbl", &payload)
}

fn build_stsd_box(video: &Mp4VideoTrack, avc_config: &AvcConfig) -> Vec<u8> {
    let avc1_box = build_avc1_box(video, avc_config);

    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&avc1_box);
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
