//! Fragmented MP4: a box walker, the kind of a segment, the track of an init segment and the samples of a media segment.
//!
//! Standard: ISO/IEC 14496-12 clauses 4.2 and 8.16.

use crate::error::Malformed;
use crate::error::{Error, Result};

/// One box: its type, where it starts in its parent, its header length and its body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mp4Box<'a> {
    /// The box type.
    pub kind: [u8; 4],
    /// The offset of the box start in the slice walked.
    pub offset: usize,
    /// The header length: 8, or 16 for a 64-bit size.
    pub header_len: usize,
    /// The body.
    pub body: &'a [u8],
}

/// The boxes of a slice, in order. It stops at the first box that does not fit (see [`Boxes::truncated`]).
#[derive(Clone, Debug)]
pub struct Boxes<'a> {
    b: &'a [u8],
    at: usize,
    truncated: bool,
}

/// Walks the boxes of `b`.
pub fn boxes(b: &[u8]) -> Boxes<'_> {
    Boxes {
        b,
        at: 0,
        truncated: false,
    }
}

impl Boxes<'_> {
    /// True when the walk stopped at a box that did not fit.
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

impl<'a> Iterator for Boxes<'a> {
    type Item = Mp4Box<'a>;
    fn next(&mut self) -> Option<Mp4Box<'a>> {
        let b = self.b;
        let at = self.at;
        if at == b.len() {
            return None;
        }
        if at + 8 > b.len() {
            self.truncated = true;
            return None;
        }
        let size32 = u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as u64;
        let kind = [b[at + 4], b[at + 5], b[at + 6], b[at + 7]];
        let (hl, size) = match size32 {
            1 => {
                let Some(s) = b.get(at + 8..at + 16) else {
                    self.truncated = true;
                    return None;
                };
                (
                    16,
                    u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]),
                )
            }
            0 => (8, (b.len() - at) as u64),
            s => (8, s),
        };
        if size < hl as u64 || size > (b.len() - at) as u64 {
            self.truncated = true;
            return None;
        }
        let size = size as usize;
        self.at = at + size;
        Some(Mp4Box {
            kind,
            offset: at,
            header_len: hl,
            body: &b[at + hl..at + size],
        })
    }
}

/// The first child of `b` of type `kind`.
pub fn child<'a>(b: &'a [u8], kind: &[u8; 4]) -> Option<Mp4Box<'a>> {
    boxes(b).find(|x| &x.kind == kind)
}

/// The box at `path` under `b`.
fn path<'a>(mut b: &'a [u8], p: &[&[u8; 4]]) -> Option<&'a [u8]> {
    for k in p {
        b = child(b, k)?.body;
    }
    Some(b)
}

/// What a segment is, from its first box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment {
    /// An init segment (`ftyp` or `moov` first).
    Init,
    /// A media segment (`styp`, `moof`, `sidx` or `emsg` first).
    Media,
    /// Something else.
    Other,
}

/// The kind of segment `b` holds.
pub fn segment_kind(b: &[u8]) -> Segment {
    match b.get(4..8) {
        Some(b"ftyp") | Some(b"moov") => Segment::Init,
        Some(b"styp") | Some(b"moof") | Some(b"sidx") | Some(b"emsg") => Segment::Media,
        _ => Segment::Other,
    }
}

/// The track of an init segment: its sample entry, timescale, decoder configuration and the defaults of `trex`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Track {
    /// The track ID.
    pub track_id: u32,
    /// The sample entry type (`hvc1`, `hev1`, `avc1`, `avc3`, `mp4a`, `ac-4`, `ec-3`, `ac-3` ...).
    pub entry: [u8; 4],
    /// The media timescale (ticks per second, from `mdhd`).
    pub timescale: u32,
    /// The decoder configuration: the body of `hvcC` or `avcC`, or the AudioSpecificConfig of `esds`.
    pub config: Vec<u8>,
    /// `trex` default sample duration.
    pub default_duration: u32,
    /// `trex` default sample size.
    pub default_size: u32,
    /// `trex` default sample flags.
    pub default_flags: u32,
}

impl Track {
    /// Reads the first track of an init segment.
    pub fn parse_init(b: &[u8]) -> Result<Track> {
        let moov = path(b, &[b"moov"]).ok_or(Error::Invalid(Malformed::InitNoMoov))?;
        let trak = path(moov, &[b"trak"]).ok_or(Error::Invalid(Malformed::InitNoTrak))?;
        let tkhd = path(trak, &[b"tkhd"]).ok_or(Error::Invalid(Malformed::InitNoTkhd))?;
        let track_id = match tkhd.first() {
            Some(1) => u32_at(tkhd, 20),
            Some(_) => u32_at(tkhd, 12),
            None => None,
        }
        .ok_or(Error::Short)?;
        let mdhd = path(trak, &[b"mdia", b"mdhd"]).ok_or(Error::Invalid(Malformed::InitNoMdhd))?;
        let timescale = match mdhd.first() {
            Some(1) => u32_at(mdhd, 20),
            Some(_) => u32_at(mdhd, 12),
            None => None,
        }
        .ok_or(Error::Short)?;
        let stsd = path(trak, &[b"mdia", b"minf", b"stbl", b"stsd"])
            .ok_or(Error::Invalid(Malformed::InitNoStsd))?;
        // stsd: version and flags (4), entry count (4), then the sample entries as boxes.
        let entry = boxes(stsd.get(8..).ok_or(Error::Short)?)
            .next()
            .ok_or(Error::Short)?;
        // A visual sample entry has 78 bytes before its child boxes, an audio sample entry 28 (clauses 12.1.3, 12.2.3).
        let children = |skip: usize| entry.body.get(skip..).unwrap_or(&[]);
        let config = match &entry.kind {
            b"hvc1" | b"hev1" => child(children(78), b"hvcC").map(|c| c.body.to_vec()),
            b"avc1" | b"avc3" => child(children(78), b"avcC").map(|c| c.body.to_vec()),
            b"mp4a" => child(children(28), b"esds")
                .and_then(|e| audio_specific_config(e.body))
                .map(|c| c.to_vec()),
            _ => None,
        }
        .unwrap_or_default();
        let mut t = Track {
            track_id,
            entry: entry.kind,
            timescale,
            config,
            ..Track::default()
        };
        if let Some(mvex) = path(moov, &[b"mvex"]) {
            for x in boxes(mvex).filter(|x| &x.kind == b"trex") {
                if u32_at(x.body, 4) == Some(track_id) {
                    t.default_duration = u32_at(x.body, 12).unwrap_or(0);
                    t.default_size = u32_at(x.body, 16).unwrap_or(0);
                    t.default_flags = u32_at(x.body, 20).unwrap_or(0);
                }
            }
        }
        Ok(t)
    }

    /// The FourCC the host decoders know for this track (`hvc1`, `avc1`, `aac `, `aach` for HE-AAC, `ac-4`, `ec-3`, `ac-3`).
    pub fn codec_fourcc(&self) -> [u8; 4] {
        match &self.entry {
            b"hvc1" | b"hev1" => *b"hvc1",
            b"avc1" | b"avc3" => *b"avc1",
            b"mp4a" => {
                let aot = self.config.first().map_or(2, |b| b >> 3);
                if aot == 5 || aot == 29 {
                    *b"aach"
                } else {
                    *b"aac "
                }
            }
            e => *e,
        }
    }
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    b.get(i..i + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn u64_at(b: &[u8], i: usize) -> Option<u64> {
    b.get(i..i + 8)
        .map(|s| u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
}

/// The AudioSpecificConfig in the body of an `esds` box: version and flags, then the ES_Descriptor (tag 3), its
/// DecoderConfigDescriptor (tag 4) and its DecoderSpecificInfo (tag 5). Descriptor sizes are expandable: 7 bits per byte, the
/// top bit set while more bytes follow (ISO/IEC 14496-1 clause 8.3.3).
pub fn audio_specific_config(esds: &[u8]) -> Option<&[u8]> {
    let es = descriptor(esds.get(4..)?, 3)?;
    // ES_Descriptor: ES_ID (2), flags (1), then optional fields by the flags.
    let flags = *es.get(2)?;
    let mut at = 3;
    if flags & 0x80 != 0 {
        at += 2;
    }
    if flags & 0x40 != 0 {
        at += 1 + *es.get(at)? as usize;
    }
    if flags & 0x20 != 0 {
        at += 2;
    }
    let dc = descriptor(es.get(at..)?, 4)?;
    // DecoderConfigDescriptor: objectTypeIndication (1), streamType (1), bufferSizeDB (3), maxBitrate (4), avgBitrate (4).
    descriptor(dc.get(13..)?, 5)
}

/// The body of the first descriptor in `b` if its tag is `tag`.
fn descriptor(b: &[u8], tag: u8) -> Option<&[u8]> {
    if *b.first()? != tag {
        return None;
    }
    let mut size = 0usize;
    let mut at = 1;
    for _ in 0..4 {
        let x = *b.get(at)?;
        at += 1;
        size = size << 7 | (x & 0x7F) as usize;
        if x & 0x80 == 0 {
            return b.get(at..at + size);
        }
    }
    None
}

/// One sample of a media segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample<'a> {
    /// The decoding time in the track timescale.
    pub dts: i64,
    /// The presentation time (dts plus the composition offset).
    pub pts: i64,
    /// The duration.
    pub duration: u32,
    /// True for a sync sample (sample_is_non_sync_sample clear).
    pub keyframe: bool,
    /// The sample bytes.
    pub data: &'a [u8],
}

/// Calls `each` for every sample of `track` in a media segment. It returns the number of samples.
pub fn samples<'a>(
    seg: &'a [u8],
    track: &Track,
    mut each: impl FnMut(Sample<'a>),
) -> Result<usize> {
    let moof = boxes(seg)
        .find(|x| &x.kind == b"moof")
        .ok_or(Error::Invalid(Malformed::MediaNoMoof))?;
    let mut count = 0;
    for traf in boxes(moof.body).filter(|x| &x.kind == b"traf") {
        let tfhd = child(traf.body, b"tfhd")
            .ok_or(Error::Invalid(Malformed::TrafNoTfhd))?
            .body;
        let fl = u32_at(tfhd, 0).ok_or(Error::Short)? & 0x00FF_FFFF;
        let id = u32_at(tfhd, 4).ok_or(Error::Short)?;
        if id != track.track_id && track.track_id != 0 {
            continue;
        }
        let mut at = 8;
        let mut take = |bit: u32, n: usize| -> Result<Option<u64>> {
            if fl & bit == 0 {
                return Ok(None);
            }
            let v = if n == 8 {
                u64_at(tfhd, at)
            } else {
                u32_at(tfhd, at).map(u64::from)
            };
            at += n;
            v.map(Some).ok_or(Error::Short)
        };
        let base_offset = take(0x01, 8)?;
        let _sample_description = take(0x02, 4)?;
        let def_dur = take(0x08, 4)?.map_or(track.default_duration, |v| v as u32);
        let def_size = take(0x10, 4)?.map_or(track.default_size, |v| v as u32);
        let def_flags = take(0x20, 4)?.map_or(track.default_flags, |v| v as u32);
        let base = match base_offset {
            Some(b) => b as i64,
            None => moof.offset as i64,
        };
        let mut dts = match child(traf.body, b"tfdt") {
            Some(t) if t.body.first() == Some(&1) => u64_at(t.body, 4).ok_or(Error::Short)? as i64,
            Some(t) => u32_at(t.body, 4).ok_or(Error::Short)? as i64,
            None => 0,
        };
        // Without a data offset, a run starts where the previous one ended (clause 8.8.8.1).
        let mut pos = base;
        for trun in boxes(traf.body).filter(|x| &x.kind == b"trun") {
            let t = trun.body;
            let rf = u32_at(t, 0).ok_or(Error::Short)?;
            let version = rf >> 24;
            let n = u32_at(t, 4).ok_or(Error::Short)? as usize;
            let mut at = 8;
            let mut word = |b: &[u8]| -> Result<u32> {
                let v = u32_at(b, at).ok_or(Error::Short)?;
                at += 4;
                Ok(v)
            };
            if rf & 0x01 != 0 {
                pos = base + word(t)? as i32 as i64;
            }
            let first_flags = if rf & 0x04 != 0 { Some(word(t)?) } else { None };
            for i in 0..n {
                let duration = if rf & 0x100 != 0 { word(t)? } else { def_dur };
                let size = if rf & 0x200 != 0 { word(t)? } else { def_size } as i64;
                let flags = if rf & 0x400 != 0 {
                    word(t)?
                } else if i == 0 {
                    first_flags.unwrap_or(def_flags)
                } else {
                    def_flags
                };
                let cto = if rf & 0x800 != 0 {
                    let w = word(t)?;
                    if version == 0 {
                        w as i64
                    } else {
                        w as i32 as i64
                    }
                } else {
                    0
                };
                if pos < 0 || pos + size > seg.len() as i64 {
                    return Err(Error::Invalid(Malformed::SampleOutsideSegment));
                }
                each(Sample {
                    dts,
                    pts: dts + cto,
                    duration,
                    keyframe: flags & 0x0001_0000 == 0,
                    data: &seg[pos as usize..(pos + size) as usize],
                });
                count += 1;
                pos += size;
                dts += duration as i64;
            }
        }
    }
    Ok(count)
}

/// Appends a box of `kind` with `body` to `out`.
pub fn write_box(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
}
