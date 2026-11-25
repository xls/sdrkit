//! From the ROUTE objects of a service to media packets: the LCT channels that a session description names, and the
//! media format of the track of an init segment.
//!
//! Standard: ATSC A/331 clause 7.1.4 and annex A.3; ISO/IEC 14496-12 clauses 8.5.2 and 12; ISO/IEC 14496-15.

use sdrkit_core::FourCc;
use sdrkit_graph::media::{AudioProps, Codec, MediaFormat, Packaging, VideoProps};
use sdrkit_proto_ipcast::mp4::{boxes, child, Mp4Box, Track};
use std::net::IpAddr;
use std::sync::Arc;

/// One LCT channel of a ROUTE session: where a component of a service comes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel {
    /// The destination address of the session.
    pub dst: IpAddr,
    /// The destination UDP port of the session.
    pub port: u16,
    /// The transport session id of the channel.
    pub tsi: u64,
    /// The code points 128 to 255 that the session description gives the entity mode, one bit each from 128.
    pub entity: u128,
}

impl Channel {
    /// The objects of the channel with this code point come in entity mode (A/331 table A.3.6 and the `Payload` elements).
    pub fn entity_mode(&self, codepoint: u8) -> bool {
        match codepoint {
            2 | 9 => true,
            128.. => self.entity >> (codepoint - 128) & 1 == 1,
            _ => false,
        }
    }

    /// The channel is that of an object to `dst`, `port` with `tsi`.
    pub fn is(&self, dst: IpAddr, port: u16, tsi: u64) -> bool {
        self.tsi == tsi && self.port == port && self.dst == dst
    }
}

/// One tag of an XML document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tag<'a> {
    /// The element name without its namespace prefix.
    name: &'a str,
    /// The text between the name and the end of the tag: the attributes.
    attributes: &'a str,
    /// An end tag.
    end: bool,
    /// An element without content (`<a/>`).
    empty: bool,
}

/// The start and end tags of `xml` in their order. Comments, processing instructions and declarations are skipped. A `>`
/// inside a quoted attribute value does not end a tag.
fn tags(xml: &str) -> impl Iterator<Item = Tag<'_>> {
    let mut rest = xml;
    std::iter::from_fn(move || loop {
        let at = rest.find('<')?;
        rest = &rest[at..];
        let skipped = [("<!--", "-->"), ("<?", "?>"), ("<!", ">")]
            .into_iter()
            .find(|(open, _)| rest.starts_with(open));
        if let Some((_, close)) = skipped {
            let end = rest.find(close)?;
            rest = &rest[end + close.len()..];
            continue;
        }
        let mut quote = None;
        let end = rest.char_indices().skip(1).find_map(|(i, c)| {
            match (quote, c) {
                (None, '>') => return Some(i),
                (None, '"' | '\'') => quote = Some(c),
                (Some(q), c) if q == c => quote = None,
                _ => {}
            }
            None
        })?;
        let body = &rest[1..end];
        rest = &rest[end + 1..];
        let (body, close) = match body.strip_prefix('/') {
            Some(b) => (b, true),
            None => (body, false),
        };
        let (body, empty) = match body.strip_suffix('/') {
            Some(b) => (b, true),
            None => (body, false),
        };
        let cut = body
            .find(|c: char| c.is_whitespace())
            .unwrap_or(body.len());
        let name = &body[..cut];
        let name = name.rsplit(':').next().unwrap_or(name);
        if name.is_empty() {
            continue;
        }
        return Some(Tag {
            name,
            attributes: &body[cut..],
            end: close,
            empty,
        });
    })
}

/// The value of attribute `name` in the attributes of a tag. The name is compared whole, without a namespace prefix.
fn attribute<'a>(attributes: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = attributes;
    loop {
        rest = rest.trim_start();
        let eq = rest.find('=')?;
        let key = rest[..eq].trim_end();
        let key = key.rsplit(':').next().unwrap_or(key);
        let value = rest[eq + 1..].trim_start();
        let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let end = value[1..].find(quote)? + 1;
        if key == name {
            return Some(&value[1..end]);
        }
        rest = &value[end + 1..];
    }
}

/// Reads a session description (`S-TSID`) that came on the signalling channel of the session to `dst`, `port`, and appends
/// to `out` the LCT channels with a source flow, in document order. An `RS` element without a destination address or port
/// has that of the signalling session. An `LS` element without a number, or with a repair flow only, gives no channel.
pub fn parse_stsid(xml: &str, dst: IpAddr, port: u16, out: &mut Vec<Channel>) {
    let mut session = (dst, port);
    // The channel of the `LS` element that is open, and whether a source flow was seen in it.
    let mut open: Option<(Channel, bool)> = None;
    for t in tags(xml) {
        match (t.name, t.end) {
            ("RS", false) => {
                session = (
                    attribute(t.attributes, "dIpAddr")
                        .and_then(|a| a.trim().parse().ok())
                        .unwrap_or(dst),
                    attribute(t.attributes, "dPort")
                        .and_then(|p| p.trim().parse().ok())
                        .unwrap_or(port),
                );
            }
            ("LS", false) => {
                let tsi = attribute(t.attributes, "tsi").and_then(|v| v.trim().parse().ok());
                open = tsi.filter(|_| !t.empty).map(|tsi| {
                    let channel = Channel {
                        dst: session.0,
                        port: session.1,
                        tsi,
                        entity: 0,
                    };
                    (channel, false)
                });
            }
            ("SrcFlow", false) => {
                if let Some(o) = &mut open {
                    o.1 = true;
                }
            }
            ("Payload", false) => {
                let number = |name| {
                    attribute(t.attributes, name).and_then(|v| v.trim().parse::<u8>().ok())
                };
                if let (Some(o), Some(cp @ 128..), Some(2)) =
                    (&mut open, number("codePoint"), number("formatId"))
                {
                    o.0.entity |= 1 << (cp - 128);
                }
            }
            ("LS", true) => {
                if let Some((channel, true)) = open.take() {
                    out.push(channel);
                }
            }
            _ => {}
        }
    }
}

/// The file of an object in entity mode: the bytes after the header lines, which an empty line ends. An object without
/// an empty line is given whole.
pub fn entity_body(object: &[u8]) -> &[u8] {
    let after = |end: &[u8]| {
        object
            .windows(end.len())
            .position(|w| w == end)
            .map(|i| &object[i + end.len()..])
    };
    after(b"\r\n\r\n").or_else(|| after(b"\n\n")).unwrap_or(object)
}

sdrkit_core::choice_enum! {
    /// What a component of a service carries.
    pub enum ComponentKind {
        /// Video.
        Video = ("video", "Video"),
        /// Sound.
        Audio = ("audio", "Sound"),
        /// Subtitles.
        Subtitles = ("subtitles", "Subtitles"),
        /// Something else.
        Other = ("other", "Other"),
    }
}

/// The track of an init segment as a component of a service.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackInfo {
    /// The sample entry type of the clear track (`hev1`, `ac-4`): for an encrypted track the type that its
    /// `frma` box names, or `encv` or `enca` without one.
    pub entry: [u8; 4],
    /// What the track carries.
    pub kind: ComponentKind,
    /// The sample entry is `encv` or `enca`: the samples are encrypted.
    pub encrypted: bool,
    /// The format of its packets. `None` for an encrypted track, a track whose codec the shared media form does not name,
    /// a track without a decoder configuration that its codec needs, and a timescale of 0.
    pub format: Option<Arc<MediaFormat>>,
}

/// The first sample entry of the first track of an init segment.
fn sample_entry(init: &[u8]) -> Option<Mp4Box<'_>> {
    let mut b = init;
    for kind in [b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsd"] {
        b = child(b, kind)?.body;
    }
    boxes(b.get(8..)?).next()
}

/// The handler type of the first track of an init segment (`vide`, `soun`, `subt`, `text`).
fn handler(init: &[u8]) -> Option<[u8; 4]> {
    let mut b = init;
    for kind in [b"moov", b"trak", b"mdia", b"hdlr"] {
        b = child(b, kind)?.body;
    }
    b.get(8..12)?.try_into().ok()
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

/// Bytes of a visual sample entry before its child boxes, and of an audio sample entry.
const VISUAL_ENTRY: usize = 78;
const AUDIO_ENTRY: usize = 28;

/// Describes `track`, read from the init segment `init`. The picture size, the channel count and the sample rate are
/// those of the sample entry. The decoder configuration of the format is the body of `hvcC` or `avcC`, the audio specific
/// configuration of an `mp4a` track, or the body of `dac4`, `dec3` or `dac3`.
pub fn describe(init: &[u8], track: &Track) -> TrackInfo {
    let entry = sample_entry(init);
    let body = entry.as_ref().map_or(&[][..], |e| e.body);
    let encrypted = matches!(&track.entry, b"encv" | b"enca");
    // The sample entry type of an encrypted track before encryption: `sinf` holds `frma`.
    let clear = if encrypted {
        let skip = if &track.entry == b"encv" {
            VISUAL_ENTRY
        } else {
            AUDIO_ENTRY
        };
        body.get(skip..)
            .and_then(|c| child(c, b"sinf"))
            .and_then(|s| child(s.body, b"frma"))
            .and_then(|f| f.body.get(..4)?.try_into().ok())
            .unwrap_or(track.entry)
    } else {
        track.entry
    };
    let kind = match &clear {
        b"hvc1" | b"hev1" | b"hvc2" | b"hev2" | b"avc1" | b"avc3" | b"vvc1" | b"vvi1" | b"encv" => {
            ComponentKind::Video
        }
        b"mp4a" | b"ac-4" | b"ec-3" | b"ac-3" | b"mhm1" | b"mhm2" | b"mha1" | b"mha2" | b"enca" => {
            ComponentKind::Audio
        }
        b"stpp" | b"wvtt" => ComponentKind::Subtitles,
        _ => match handler(init).as_ref() {
            Some(b"vide") => ComponentKind::Video,
            Some(b"soun") => ComponentKind::Audio,
            Some(b"subt" | b"text") => ComponentKind::Subtitles,
            _ => ComponentKind::Other,
        },
    };
    let config = |skip: usize, kind: &[u8; 4]| -> Option<Arc<[u8]>> {
        Some(Arc::from(child(body.get(skip..)?, kind)?.body))
    };
    // The size of the NAL unit length: two bits of the configuration record, 4 bytes when the record is too short.
    let nal_length = |c: &[u8], at: usize| c.get(at).map_or(4, |b| (b & 3) + 1);
    let codec: Option<(Codec, Packaging, Arc<[u8]>)> = match &track.entry {
        b"hvc1" | b"hev1" => config(VISUAL_ENTRY, b"hvcC")
            .map(|c| (Codec::Hevc, Packaging::LengthPrefixed(nal_length(&c, 21)), c)),
        b"avc1" | b"avc3" => config(VISUAL_ENTRY, b"avcC")
            .map(|c| (Codec::H264, Packaging::LengthPrefixed(nal_length(&c, 4)), c)),
        b"mp4a" => (!track.config.is_empty()).then(|| {
            let codec = match track.config[0] >> 3 {
                5 => Codec::HeAac,
                29 => Codec::HeAacV2,
                _ => Codec::Aac,
            };
            (codec, Packaging::Raw, Arc::from(&track.config[..]))
        }),
        b"ac-4" => config(AUDIO_ENTRY, b"dac4").map(|c| (Codec::Ac4, Packaging::Raw, c)),
        b"ec-3" => config(AUDIO_ENTRY, b"dec3").map(|c| (Codec::EAc3, Packaging::Raw, c)),
        b"ac-3" => config(AUDIO_ENTRY, b"dac3").map(|c| (Codec::Ac3, Packaging::Raw, c)),
        _ => None,
    };
    let format = codec
        .filter(|_| track.timescale > 0)
        .map(|(codec, packaging, config)| {
            let mut f = MediaFormat::new(codec, packaging);
            f.config = config;
            f.timebase = (1, track.timescale);
            if kind == ComponentKind::Video {
                f.video = Some(VideoProps {
                    width: u32::from(u16_at(body, 24).unwrap_or(0)),
                    height: u32::from(u16_at(body, 26).unwrap_or(0)),
                    ..VideoProps::default()
                });
            } else {
                f.audio = Some(AudioProps {
                    sample_rate: u32::from(u16_at(body, 24).unwrap_or(0)),
                    channels: u16_at(body, 16).unwrap_or(0).min(255) as u8,
                });
            }
            let code = FourCc(track.entry);
            if code != codec.fourcc() {
                f.container_code = Some(code);
            }
            Arc::new(f)
        });
    TrackInfo {
        entry: clear,
        kind,
        encrypted,
        format,
    }
}

/// Whether a sync sample of `format` is a clean start: nothing after it refers to data before it. For HEVC the first
/// picture NAL unit is an IDR or a broken link access picture (types 16 to 20), for H.264 an IDR picture (type 5). A
/// sample of another codec is a clean start.
pub fn clean_start(format: &MediaFormat, sample: &[u8]) -> bool {
    let Packaging::LengthPrefixed(n) = format.packaging else {
        return true;
    };
    let n = usize::from(n);
    let mut at = 0;
    while let Some(head) = sample.get(at..at + n) {
        let len = head.iter().fold(0usize, |a, &b| a << 8 | usize::from(b));
        let Some(&first) = sample.get(at + n) else {
            break;
        };
        match format.codec {
            Codec::Hevc => {
                let kind = first >> 1 & 0x3F;
                if kind < 32 {
                    return (16..=20).contains(&kind);
                }
            }
            Codec::H264 => {
                let kind = first & 0x1F;
                if (1..=5).contains(&kind) {
                    return kind == 5;
                }
            }
            _ => return true,
        }
        at += n + len;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atsc3::testsvc;
    use std::net::Ipv4Addr;

    #[test]
    fn the_session_description_names_the_channels() {
        let sls = IpAddr::V4(Ipv4Addr::new(239, 255, 1, 1));
        let xml = r#"<?xml version="1.0"?><!-- <LS tsi="99"> -->
<S-TSID xmlns="tag:atsc.org,2016:XMLSchemas/ATSC3/Delivery/S-TSID/1.0/" xmlns:x="urn:x">
 <RS><LS tsi="7" bw="1000"><SrcFlow rt="true"><Payload codePoint="128" formatId="2"/><Payload codePoint="130" formatId="1"/></SrcFlow></LS>
     <LS tsi="8"><RepairFlow/></LS><LS xtsi="9" x:tsi = '10'><SrcFlow/></LS></RS>
 <x:RS dIpAddr="239.255.1.2" dPort="6000" sIpAddr="10.0.0.9"><x:LS tsi="7"><x:SrcFlow note="a > b"/></x:LS><LS tsi="3"/></x:RS>
</S-TSID>"#;
        let mut out = Vec::new();
        parse_stsid(xml, sls, 5000, &mut out);
        let got: Vec<(String, u16, u64)> = out
            .iter()
            .map(|c| (c.dst.to_string(), c.port, c.tsi))
            .collect();
        assert_eq!(
            got,
            [
                ("239.255.1.1".to_string(), 5000, 7),
                ("239.255.1.1".to_string(), 5000, 10),
                ("239.255.1.2".to_string(), 6000, 7),
            ]
        );
        assert!(out[0].entity_mode(128) && !out[0].entity_mode(130) && !out[0].entity_mode(8));
        assert!(out[1].entity_mode(9) && out[1].entity_mode(2) && !out[1].entity_mode(128));
        // The session description of the test signal.
        out.clear();
        parse_stsid(&testsvc::stsid(), sls, 1, &mut out);
        let route = IpAddr::V4(testsvc::ROUTE_ADDR);
        assert!(out[0].is(route, testsvc::ROUTE_PORT, u64::from(testsvc::VIDEO_TSI)));
        assert!(out[1].is(route, testsvc::ROUTE_PORT, u64::from(testsvc::AUDIO_TSI)));
        assert_eq!(out.len(), 2);
        // Cut anywhere: no panic.
        for cut in 0..xml.len() {
            if xml.is_char_boundary(cut) {
                parse_stsid(&xml[..cut], sls, 5000, &mut out);
            }
        }
    }

    #[test]
    fn an_init_segment_gives_the_format_of_its_track() {
        let init = testsvc::video_init();
        let track = Track::parse_init(&init).expect("an init segment");
        let v = describe(&init, &track);
        assert_eq!((&v.entry, v.kind, v.encrypted), (b"hev1", ComponentKind::Video, false));
        let f = v.format.expect("a format");
        assert_eq!((f.codec, f.packaging), (Codec::Hevc, Packaging::LengthPrefixed(4)));
        assert_eq!(f.timebase, (1, testsvc::VIDEO_TIMESCALE));
        assert_eq!(&f.config[..], &testsvc::video_config()[..]);
        assert_eq!(f.container_code, Some(FourCc::new("hev1")));
        let p = f.video.as_ref().expect("video properties");
        assert_eq!((p.width, p.height), (1280, 720));
        assert!(clean_start(&f, &testsvc::video_sample(3, 0)));
        assert!(!clean_start(&f, &testsvc::video_sample(3, 1)));
        assert!(!clean_start(&f, &[0, 0]));

        let init = testsvc::audio_init();
        let track = Track::parse_init(&init).expect("an init segment");
        let a = describe(&init, &track);
        assert_eq!((&a.entry, a.kind, a.encrypted), (b"ac-4", ComponentKind::Audio, false));
        let f = a.format.expect("a format");
        assert_eq!((f.codec, f.packaging), (Codec::Ac4, Packaging::Raw));
        assert_eq!(f.timebase, (1, testsvc::AUDIO_TIMESCALE));
        assert_eq!(&f.config[..], &testsvc::AUDIO_CONFIG[..]);
        assert_eq!(f.audio, Some(AudioProps { sample_rate: 48_000, channels: 2 }));
        assert_eq!(f.container_code, None);
        assert!(clean_start(&f, &[1, 2, 3]));
    }

    #[test]
    fn an_encrypted_or_unnamed_track_has_no_format() {
        // The video init segment with the sample entry renamed: an encrypted track without a `frma` box.
        let mut init = testsvc::video_init();
        let at = init.windows(4).position(|w| w == b"hev1").expect("the entry");
        init[at..at + 4].copy_from_slice(b"encv");
        let track = Track::parse_init(&init).expect("an init segment");
        let v = describe(&init, &track);
        assert!(v.encrypted && v.format.is_none());
        assert_eq!(v.kind, ComponentKind::Video);
        // An MPEG-H track: sound that the shared media form has no name for.
        let mut init = testsvc::audio_init();
        let at = init.windows(4).position(|w| w == b"ac-4").expect("the entry");
        init[at..at + 4].copy_from_slice(b"mhm1");
        let track = Track::parse_init(&init).expect("an init segment");
        let a = describe(&init, &track);
        assert_eq!((&a.entry, a.kind, a.encrypted), (b"mhm1", ComponentKind::Audio, false));
        assert!(a.format.is_none());
    }

    #[test]
    fn entity_mode_gives_the_file_after_the_header_lines() {
        assert_eq!(entity_body(b"Content-Type: video/mp4\r\nContent-Length: 2\r\n\r\nab"), b"ab");
        assert_eq!(entity_body(b"Content-Type: video/mp4\n\nab"), b"ab");
        assert_eq!(entity_body(b"abc"), b"abc");
    }
}
