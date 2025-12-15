use std::fmt;
use std::io::{self, Write};

const SPS_NAL: &[u8] = &[0x67, 0x42, 0x00, 0x1e, 0xda, 0x02, 0x80, 0x2d, 0x8b, 0x11];
const PPS_NAL: &[u8] = &[0x68, 0xce, 0x38, 0x80];

pub const VIDEO_TIMESCALE: u32 = 1000;

/// Minimal MP4 writer used by the early slices.
pub struct Mp4Writer<Writer> {
    writer: Writer,
    samples: Vec<SampleInfo>,
    sample_data: Vec<u8>,
    prev_pts: Option<u64>,
    last_delta: Option<u32>,
}

/// Simplified video track information used when writing the header.
pub struct Mp4VideoTrack {
    pub width: u32,
    pub height: u32,
}

struct SampleInfo {
    size: u32,
    is_keyframe: bool,
    duration: Option<u32>,
}

struct SampleTables {
    sample_count: u32,
    durations: Vec<u32>,
    sizes: Vec<u32>,
    keyframes: Vec<u32>,
    chunk_offset: Option<u32>,
}

impl SampleTables {
    fn from_samples(
        samples: &[SampleInfo],
        chunk_offset: Option<u32>,
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
        let sizes = samples.iter().map(|sample| sample.size).collect();
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
            chunk_offset,
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
    /// Computed sample duration overflowed a `u32`.
    DurationOverflow,
}

impl fmt::Display for Mp4WriterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mp4WriterError::NonIncreasingTimestamp => write!(f, "timestamps must grow"),
            Mp4WriterError::FirstFrameMustBeKeyframe => {
                write!(f, "first frame must be a keyframe")
            }
            Mp4WriterError::DurationOverflow => write!(f, "sample duration overflow"),
        }
    }
}

impl std::error::Error for Mp4WriterError {}

impl<Writer: Write> Mp4Writer<Writer> {
    /// Wraps the provided writer for MP4 container output.
    pub fn new(writer: Writer) -> Self {
        Self {
            writer,
            samples: Vec::new(),
            sample_data: Vec::new(),
            prev_pts: None,
            last_delta: None,
        }
    }

    /// Queues a video sample for later `mdat` emission.
    pub fn write_video_sample(
        &mut self,
        pts: u64,
        data: &[u8],
        is_keyframe: bool,
    ) -> Result<(), Mp4WriterError> {
        if let Some(prev) = self.prev_pts {
            if pts <= prev {
                return Err(Mp4WriterError::NonIncreasingTimestamp);
            }
            let delta = pts - prev;
            if delta > u64::from(u32::MAX) {
                return Err(Mp4WriterError::DurationOverflow);
            }
            let delta = delta as u32;
            if let Some(last) = self.samples.last_mut() {
                last.duration = Some(delta);
            }
            self.last_delta = Some(delta);
        } else if !is_keyframe {
            return Err(Mp4WriterError::FirstFrameMustBeKeyframe);
        }

        let sample_size = data.len();
        if sample_size > u32::MAX as usize {
            return Err(Mp4WriterError::DurationOverflow);
        }

        self.samples.push(SampleInfo {
            size: sample_size as u32,
            is_keyframe,
            duration: None,
        });
        self.sample_data.extend_from_slice(data);
        self.prev_pts = Some(pts);
        Ok(())
    }

    /// Finalises the MP4 file by writing the header boxes and sample data.
    pub fn finalize(mut self, video: &Mp4VideoTrack) -> io::Result<()> {
        let ftyp_box = build_ftyp_box();
        let ftyp_len = ftyp_box.len() as u32;
        self.writer.write_all(&ftyp_box)?;

        let chunk_offset = if !self.sample_data.is_empty() {
            let mdat_size = 8 + self.sample_data.len() as u32;
            self.writer.write_all(&mdat_size.to_be_bytes())?;
            self.writer.write_all(b"mdat")?;
            self.writer.write_all(&self.sample_data)?;
            Some(ftyp_len + 8)
        } else {
            None
        };

        let tables = SampleTables::from_samples(&self.samples, chunk_offset, self.last_delta);
        let moov_box = build_moov_box(video, &tables);
        self.writer.write_all(&moov_box)
    }
}

fn build_moov_box(video: &Mp4VideoTrack, tables: &SampleTables) -> Vec<u8> {
    let mvhd_payload = build_mvhd_payload();
    let mvhd_box = build_box(b"mvhd", &mvhd_payload);
    let trak_box = build_trak_box(video, tables);

    let mut payload = Vec::new();
    payload.extend_from_slice(&mvhd_box);
    payload.extend_from_slice(&trak_box);
    build_box(b"moov", &payload)
}

fn build_trak_box(video: &Mp4VideoTrack, tables: &SampleTables) -> Vec<u8> {
    let tkhd_box = build_tkhd_box(video);
    let mdia_box = build_mdia_box(video, tables);

    let mut payload = Vec::new();
    payload.extend_from_slice(&tkhd_box);
    payload.extend_from_slice(&mdia_box);
    build_box(b"trak", &payload)
}

fn build_mdia_box(video: &Mp4VideoTrack, tables: &SampleTables) -> Vec<u8> {
    let mdhd_box = build_mdhd_box();
    let hdlr_box = build_hdlr_box();
    let minf_box = build_minf_box(video, tables);

    let mut payload = Vec::new();
    payload.extend_from_slice(&mdhd_box);
    payload.extend_from_slice(&hdlr_box);
    payload.extend_from_slice(&minf_box);
    build_box(b"mdia", &payload)
}

fn build_minf_box(video: &Mp4VideoTrack, tables: &SampleTables) -> Vec<u8> {
    let vmhd_box = build_vmhd_box();
    let dinf_box = build_dinf_box();
    let stbl_box = build_stbl_box(video, tables);

    let mut payload = Vec::new();
    payload.extend_from_slice(&vmhd_box);
    payload.extend_from_slice(&dinf_box);
    payload.extend_from_slice(&stbl_box);
    build_box(b"minf", &payload)
}

fn build_stbl_box(video: &Mp4VideoTrack, tables: &SampleTables) -> Vec<u8> {
    let stsd_box = build_stsd_box(video);
    let stts_box = build_stts_box(&tables.durations);
    let stsc_box = build_stsc_box(tables.sample_count);
    let stsz_box = build_stsz_box(&tables.sizes);
    let stco_box = build_stco_box(tables.chunk_offset);

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

fn build_stsd_box(video: &Mp4VideoTrack) -> Vec<u8> {
    let avc1_box = build_avc1_box(video);

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

fn build_stsc_box(sample_count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());

    if sample_count == 0 {
        payload.extend_from_slice(&0u32.to_be_bytes());
        return build_box(b"stsc", &payload);
    }

    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&sample_count.to_be_bytes());
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

fn build_stco_box(chunk_offset: Option<u32>) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    if let Some(offset) = chunk_offset {
        payload.extend_from_slice(&1u32.to_be_bytes());
        payload.extend_from_slice(&offset.to_be_bytes());
    } else {
        payload.extend_from_slice(&0u32.to_be_bytes());
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

fn build_avc1_box(video: &Mp4VideoTrack) -> Vec<u8> {
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
    let avc_c_box = build_avcc_box();
    payload.extend_from_slice(&avc_c_box);
    build_box(b"avc1", &payload)
}

fn build_avcc_box() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(1);
    payload.push(0x42);
    payload.push(0x00);
    payload.push(0x1e);
    payload.push(0xff);
    payload.push(0xe1);
    payload.extend_from_slice(&(SPS_NAL.len() as u16).to_be_bytes());
    payload.extend_from_slice(SPS_NAL);
    payload.push(1);
    payload.extend_from_slice(&(PPS_NAL.len() as u16).to_be_bytes());
    payload.extend_from_slice(PPS_NAL);
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
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&VIDEO_TIMESCALE.to_be_bytes());
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

fn build_tkhd_box(video: &Mp4VideoTrack) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(&0u32.to_be_bytes());
    payload.extend_from_slice(&0u64.to_be_bytes());
    payload.extend_from_slice(&0u64.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
    payload.extend_from_slice(&0u16.to_be_bytes());
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
    let width_fixed = (video.width << 16) as u32;
    let height_fixed = (video.height << 16) as u32;
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
    payload.extend_from_slice(&VIDEO_TIMESCALE.to_be_bytes());
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
