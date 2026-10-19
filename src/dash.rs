//! DASH MPD (Media Presentation Description) manifest builder.
//!
//! This module generates a minimal static MPD (Media Presentation Description)
//! suitable for use with fragmented MP4 output from FragmentedMuxer.
//!
//! # Example
//!
//! ```
//! use muxide::dash::{MpdConfig, DashTrackInfo, DashSegment};
//!
//! let config = MpdConfig {
//!     media_presentation_duration: 10000,
//!     min_buffer_time: 2.0,
//!     timescale: 1000,
//!     video: Some(DashTrackInfo {
//!         codec: "avc1.42e01e".to_string(),
//!         width: Some(1920),
//!         height: Some(1080),
//!         frame_rate: Some(30.0),
//!         sample_rate: None,
//!         channels: None,
//!         segments: vec![DashSegment {
//!             number: 1,
//!             duration: 1000,
//!             path: "video_1.m4s".to_string(),
//!             size: 1000,
//!         }],
//!     }),
//!     audio: None,
//!     base_url: "http://example.com/".to_string(),
//! };
//!
//! let mpd = muxide::dash::build_mpd(&config);
//! assert!(mpd.contains("<MPD"));
//! ```

/// Configuration for building a DASH MPD manifest.
#[derive(Debug, Clone)]
pub struct MpdConfig {
    /// Total presentation duration in timescale units
    pub media_presentation_duration: u64,
    /// Minimum buffer time in seconds (ISO 8601 PT format)
    pub min_buffer_time: f64,
    /// Timescale for the presentation
    pub timescale: u32,
    /// Video track info
    pub video: Option<DashTrackInfo>,
    /// Audio track info
    pub audio: Option<DashTrackInfo>,
    /// Base URL for segments
    pub base_url: String,
}

/// Track information for MPD.
#[derive(Debug, Clone)]
pub struct DashTrackInfo {
    /// Codec string (e.g., "avc1.42e01e", "mp4a.40.2")
    pub codec: String,
    /// Track width (video only)
    pub width: Option<u32>,
    /// Track height (video only)
    pub height: Option<u32>,
    /// Frame rate (video only)
    pub frame_rate: Option<f64>,
    /// Sample rate (audio only)
    pub sample_rate: Option<u32>,
    /// Channel count (audio only)
    pub channels: Option<u16>,
    /// Segment list for this track
    pub segments: Vec<DashSegment>,
}

/// A single media segment in a DASH presentation.
#[derive(Debug, Clone)]
pub struct DashSegment {
    /// Segment number (1-based)
    pub number: u32,
    /// Duration in timescale units
    pub duration: u64,
    /// Segment file path or URL
    pub path: String,
    /// Segment size in bytes
    pub size: u64,
}

/// Builds a static MPD document from the provided configuration.
///
/// Returns the MPD as a UTF-8 encoded XML string.
pub fn build_mpd(config: &MpdConfig) -> String {
    let mut xml = String::new();

    // XML declaration
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");

    // MPD root element
    xml.push_str("<MPD ");
    xml.push_str("xmlns=\"urn:mpeg:dash:schema:mpd:2011\" ");
    xml.push_str("type=\"static\" ");
    xml.push_str(&format!(
        "mediaPresentationDuration=\"{}\" ",
        format_duration(config.media_presentation_duration, config.timescale)
    ));
    xml.push_str(&format!(
        "minBufferTime=\"{}\" ",
        format_duration(config.min_buffer_time as u64, 1)
    ));
    xml.push_str("profiles=\"urn:mpeg:dash:profile:isoff-live:2011\"");
    xml.push_str(">\n");

    // Base URL
    if !config.base_url.is_empty() {
        xml.push_str("  <BaseURL>");
        xml.push_str(&escape_xml(&config.base_url));
        xml.push_str("</BaseURL>\n");
    }

    // Period
    xml.push_str("  <Period id=\"0\" start=\"PT0S\">\n");

    // Video AdaptationSet
    if let Some(video) = &config.video {
        xml.push_str("    <AdaptationSet ");
        xml.push_str("id=\"0\" ");
        xml.push_str("contentType=\"video\" ");
        xml.push_str("mimeType=\"video/mp4\" ");
        xml.push_str(&format!("codecs=\"{}\" ", escape_xml(&video.codec)));
        if let Some(w) = video.width {
            xml.push_str(&format!("width=\"{}\" ", w));
        }
        if let Some(h) = video.height {
            xml.push_str(&format!("height=\"{}\" ", h));
        }
        if let Some(fps) = video.frame_rate {
            xml.push_str(&format!("frameRate=\"{:.2}\" ", fps));
        }
        xml.push_str("startWithSAP=\"1\" ");
        xml.push_str("segmentAlignment=\"true\"");
        xml.push_str(">\n");

        // SegmentTemplate for video
        if !video.segments.is_empty() {
            let seg = &video.segments[0];
            xml.push_str("      <SegmentTemplate ");
            xml.push_str(&format!("timescale=\"{}\" ", config.timescale));
            xml.push_str(&format!("duration=\"{}\" ", seg.duration));
            xml.push_str("startNumber=\"1\" ");
            xml.push_str(&format!(
                "media=\"{}/video_$$Number$$.m4s\" ",
                escape_xml(&config.base_url)
            ));
            xml.push_str(&format!(
                "initialization=\"{}/video_init.mp4\" ",
                escape_xml(&config.base_url)
            ));
            xml.push_str("/>\n");
        }

        xml.push_str("    </AdaptationSet>\n");
    }

    // Audio AdaptationSet
    if let Some(audio) = &config.audio {
        xml.push_str("    <AdaptationSet ");
        xml.push_str("id=\"1\" ");
        xml.push_str("contentType=\"audio\" ");
        xml.push_str("mimeType=\"audio/mp4\" ");
        xml.push_str(&format!("codecs=\"{}\" ", escape_xml(&audio.codec)));
        if let Some(sr) = audio.sample_rate {
            xml.push_str(&format!("audioSamplingRate=\"{}\" ", sr));
        }
        if let Some(ch) = audio.channels {
            xml.push_str(&format!("channels=\"{}\" ", ch));
        }
        xml.push_str("segmentAlignment=\"true\"");
        xml.push_str(">\n");

        // SegmentTemplate for audio
        if !audio.segments.is_empty() {
            let seg = &audio.segments[0];
            xml.push_str("      <SegmentTemplate ");
            xml.push_str(&format!("timescale=\"{}\" ", config.timescale));
            xml.push_str(&format!("duration=\"{}\" ", seg.duration));
            xml.push_str("startNumber=\"1\" ");
            xml.push_str(&format!(
                "media=\"{}/audio_$$Number$$.m4s\" ",
                escape_xml(&config.base_url)
            ));
            xml.push_str(&format!(
                "initialization=\"{}/audio_init.mp4\" ",
                escape_xml(&config.base_url)
            ));
            xml.push_str("/>\n");
        }

        xml.push_str("    </AdaptationSet>\n");
    }

    xml.push_str("  </Period>\n");
    xml.push_str("</MPD>\n");

    xml
}

/// Formats a duration in the given timescale as an ISO 8601 duration string (PT#S or PT#.#S).
fn format_duration(duration: u64, timescale: u32) -> String {
    if timescale == 1 {
        format!("PT{}S", duration)
    } else {
        let secs = duration as f64 / timescale as f64;
        if secs.fract() == 0.0 {
            format!("PT{}S", secs as u64)
        } else {
            let s = format!("{:.3}", secs);
            let trimmed = s.trim_end_matches('0').trim_end_matches('.');
            format!("PT{}S", trimmed)
        }
    }
}

/// Escapes special XML characters.
fn escape_xml(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            c => result.push(c),
        }
    }
    result
}

/// Checks that XML tags are balanced (naive well-formedness check).
#[cfg(test)]
fn check_well_formed(xml: &str) -> bool {
    let mut depth: i32 = 0;
    let mut chars = xml.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            if chars.peek() == Some(&'?') {
                continue;
            }
            if chars.peek() == Some(&'!') {
                continue;
            }
            if chars.peek() == Some(&'/') {
                chars.next();
                for sc in chars.by_ref() {
                    if sc == '>' {
                        depth -= 1;
                        break;
                    }
                }
            } else {
                let mut is_self_closing = false;
                let mut in_quote: Option<char> = None;
                for sc in chars.by_ref() {
                    if let Some(q) = in_quote {
                        if sc == q {
                            in_quote = None;
                        }
                    } else if sc == '"' || sc == '\'' {
                        in_quote = Some(sc);
                    } else if sc == '/' {
                        is_self_closing = true;
                    } else if sc == '>' {
                        if !is_self_closing {
                            depth += 1;
                        }
                        break;
                    }
                }
            }
        }
    }
    depth == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> MpdConfig {
        MpdConfig {
            media_presentation_duration: 10000,
            min_buffer_time: 2.0,
            timescale: 1000,
            video: Some(DashTrackInfo {
                codec: "avc1.42e01e".to_string(),
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(30.0),
                sample_rate: None,
                channels: None,
                segments: vec![DashSegment {
                    number: 1,
                    duration: 1000,
                    path: "video_1.m4s".to_string(),
                    size: 1000,
                }],
            }),
            audio: Some(DashTrackInfo {
                codec: "mp4a.40.2".to_string(),
                width: None,
                height: None,
                frame_rate: None,
                sample_rate: Some(48000),
                channels: Some(2),
                segments: vec![DashSegment {
                    number: 1,
                    duration: 1000,
                    path: "audio_1.m4s".to_string(),
                    size: 500,
                }],
            }),
            base_url: "http://example.com/".to_string(),
        }
    }

    #[test]
    fn test_build_mpd_basic() {
        let config = sample_config();
        let mpd = build_mpd(&config);

        assert!(mpd.contains("<MPD"));
        assert!(mpd.contains("type=\"static\""));
        assert!(mpd.contains("mediaPresentationDuration=\"PT10S\""));
        assert!(mpd.contains("AdaptationSet"));
        assert!(mpd.contains("codecs=\"avc1.42e01e\""));
        assert!(mpd.contains("codecs=\"mp4a.40.2\""));
        assert!(mpd.contains("SegmentTemplate"));
    }

    #[test]
    fn test_mpd_has_required_fields() {
        let config = sample_config();
        let mpd = build_mpd(&config);

        // Required MPD root attributes
        assert!(mpd.contains("xmlns=\"urn:mpeg:dash:schema:mpd:2011\""));
        assert!(mpd.contains("type=\"static\""));
        assert!(mpd.contains("mediaPresentationDuration="));
        assert!(mpd.contains("minBufferTime="));
        assert!(mpd.contains("profiles="));

        // Period
        assert!(mpd.contains("<Period"));
        assert!(mpd.contains("</Period>"));

        // Video AdaptationSet
        assert!(mpd.contains("contentType=\"video\""));
        assert!(mpd.contains("mimeType=\"video/mp4\""));
        assert!(mpd.contains("width=\"1920\""));
        assert!(mpd.contains("height=\"1080\""));
        assert!(mpd.contains("frameRate=\"30.00\""));
        assert!(mpd.contains("startWithSAP=\"1\""));
        assert!(mpd.contains("segmentAlignment=\"true\""));

        // Audio AdaptationSet
        assert!(mpd.contains("contentType=\"audio\""));
        assert!(mpd.contains("mimeType=\"audio/mp4\""));
        assert!(mpd.contains("audioSamplingRate=\"48000\""));

        // SegmentTemplate
        assert!(mpd.contains("timescale=\"1000\""));
        assert!(mpd.contains("duration=\"1000\""));
        assert!(mpd.contains("startNumber=\"1\""));
        assert!(mpd.contains("media="));
        assert!(mpd.contains("initialization="));
        assert!(mpd.contains("video_init.mp4"));
        assert!(mpd.contains("audio_init.mp4"));
    }

    #[test]
    fn test_mpd_video_only() {
        let config = MpdConfig {
            media_presentation_duration: 5000,
            min_buffer_time: 1.5,
            timescale: 1000,
            video: Some(DashTrackInfo {
                codec: "hvc1.1.6.L93.B0".to_string(),
                width: Some(3840),
                height: Some(2160),
                frame_rate: Some(60.0),
                sample_rate: None,
                channels: None,
                segments: vec![DashSegment {
                    number: 1,
                    duration: 1000,
                    path: "video_1.m4s".to_string(),
                    size: 5000,
                }],
            }),
            audio: None,
            base_url: "".to_string(),
        };

        let mpd = build_mpd(&config);
        assert!(mpd.contains("contentType=\"video\""));
        assert!(!mpd.contains("contentType=\"audio\""));
        assert!(!mpd.contains("<BaseURL>"));
    }

    #[test]
    fn test_mpd_audio_only() {
        let config = MpdConfig {
            media_presentation_duration: 5000,
            min_buffer_time: 1.5,
            timescale: 1000,
            video: None,
            audio: Some(DashTrackInfo {
                codec: "mp4a.40.2".to_string(),
                width: None,
                height: None,
                frame_rate: None,
                sample_rate: Some(44100),
                channels: Some(2),
                segments: vec![DashSegment {
                    number: 1,
                    duration: 1000,
                    path: "audio_1.m4s".to_string(),
                    size: 500,
                }],
            }),
            base_url: "segments/".to_string(),
        };

        let mpd = build_mpd(&config);
        assert!(!mpd.contains("contentType=\"video\""));
        assert!(mpd.contains("contentType=\"audio\""));
        assert!(mpd.contains("<BaseURL>segments/</BaseURL>"));
    }

    #[test]
    fn test_xml_well_formed() {
        let config = sample_config();
        let mpd = build_mpd(&config);
        assert!(check_well_formed(&mpd), "MPD XML should have balanced tags");
    }

    #[test]
    fn test_deterministic_output() {
        let config = sample_config();
        let mpd1 = build_mpd(&config);
        let mpd2 = build_mpd(&config);
        assert_eq!(mpd1, mpd2, "Identical inputs must produce identical MPD");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(1000, 1000), "PT1S");
        assert_eq!(format_duration(500, 1000), "PT0.5S");
        assert_eq!(format_duration(10, 1), "PT10S");
        assert_eq!(format_duration(0, 1000), "PT0S");
        assert_eq!(format_duration(333, 1000), "PT0.333S");
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("a&b<c>d"), "a&amp;b&lt;c&gt;d");
        assert_eq!(escape_xml("hello"), "hello");
        assert_eq!(escape_xml("\"quote\""), "&quot;quote&quot;");
        assert_eq!(escape_xml("it's"), "it&apos;s");
        assert_eq!(escape_xml(""), "");
    }

    #[test]
    fn test_escape_in_codec_string() {
        let config = MpdConfig {
            media_presentation_duration: 1000,
            min_buffer_time: 1.0,
            timescale: 1000,
            video: Some(DashTrackInfo {
                codec: "avc1.42e01e&test".to_string(),
                width: Some(1280),
                height: Some(720),
                frame_rate: Some(24.0),
                sample_rate: None,
                channels: None,
                segments: vec![DashSegment {
                    number: 1,
                    duration: 1000,
                    path: "v.m4s".to_string(),
                    size: 100,
                }],
            }),
            audio: None,
            base_url: "".to_string(),
        };

        let mpd = build_mpd(&config);
        assert!(mpd.contains("&amp;"));
        assert!(!mpd.contains("avc1.42e01e&test"));
    }
}
