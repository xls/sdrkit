//! The service of the ATSC 3.0 test signal: a service list with one linear TV service, its service layer signalling and a
//! ROUTE session with one HEVC video component and one AC-4 audio component. The bytes of the access units are synthetic:
//! they follow from the segment and sample numbers and no decoder plays them.
//!
//! Standard: ATSC A/331 clauses 6 and 7 and annex A; ISO/IEC 14496-12.

use super::svc::{build_lls, LLS_ADDR, LLS_PORT, TABLE_SLT};
use sdrkit_proto_ipcast::lct::LctBuilder;
use sdrkit_proto_ipcast::mp4::write_box;
use std::collections::VecDeque;
use std::net::Ipv4Addr;
use std::sync::OnceLock;

/// The id of the service.
pub const SERVICE_ID: u16 = 1001;
/// Its major and minor channel numbers.
pub const CHANNEL: (u16, u16) = (39, 1);
/// Its short name.
pub const SERVICE_NAME: &str = "TEST 1";
/// The source address of every datagram of the test signal.
pub const SOURCE: Ipv4Addr = Ipv4Addr::new(10, 0, 0, 1);
/// The destination address of the ROUTE session of the service.
pub const ROUTE_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 10, 2);
/// Its UDP port.
pub const ROUTE_PORT: u16 = 5002;
/// The LCT channel of the video component.
pub const VIDEO_TSI: u32 = 1;
/// The LCT channel of the audio component.
pub const AUDIO_TSI: u32 = 2;
/// The object number of an init segment.
pub const INIT_TOI: u32 = 1;
/// The object number of media segment 0. Segment `k` has this number plus `k`.
pub const FIRST_SEGMENT_TOI: u32 = 2;
/// The object number of the signalling package (A/331 annex C): bundle description, session description and media
/// presentation description, version 1.
pub const SLS_TOI: u32 = 1 << 16 | 1 << 17 | 1 << 18 | 1;
/// The code point of a file in file mode (A/331 table A.3.6).
pub const CODEPOINT_FILE: u8 = 1;
/// The code point of the signalling package: unsigned package mode.
pub const CODEPOINT_SLS: u8 = 3;
/// The code point of the first init segment: a new init segment that starts a timeline.
pub const CODEPOINT_INIT: u8 = 5;
/// The code point of a repeated init segment.
pub const CODEPOINT_INIT_AGAIN: u8 = 7;
/// The code point of a media segment in file mode.
pub const CODEPOINT_MEDIA: u8 = 8;

/// Ticks per second of the video track.
pub const VIDEO_TIMESCALE: u32 = 90_000;
/// The duration of a video access unit: 25 pictures per second.
pub const VIDEO_DURATION: u32 = 3600;
/// The composition offset of every video access unit: presentation is one picture after decoding.
pub const VIDEO_OFFSET: u32 = 3600;
/// The picture size that the video sample entry states.
pub const VIDEO_SIZE: (u16, u16) = (1280, 720);
/// Ticks per second of the audio track.
pub const AUDIO_TIMESCALE: u32 = 48_000;
/// The duration of an audio access unit: 25 frames per second.
pub const AUDIO_DURATION: u32 = 1920;
/// Access units in a media segment, of the video and of the audio.
pub const SAMPLES_PER_SEGMENT: u32 = 2;
/// The bytes of an LCT packet's part of an object: with the LCT header and the start offset a datagram of 1200 bytes, the
/// size of a test datagram.
pub const CHUNK: usize = 1176;
/// The bytes of every object of the service: three parts. The signalling package needs them, and the other objects are
/// filled to the same size, so the packets and the objects that a receiver holds have one size each.
pub const OBJECT: usize = 3 * CHUNK;
/// The bytes of a datagram of the test signal.
pub const DATAGRAM: usize = 1200;

/// The LLS datagram with the service list table. A comment of letters after the table fills the datagram to
/// [`DATAGRAM`] bytes when a length of the comment gives that size; without one the datagram has no comment.
pub fn lls() -> &'static [u8] {
    static LLS: OnceLock<Vec<u8>> = OnceLock::new();
    LLS.get_or_init(|| {
        let plain = slt();
        let mut xml = format!("{plain}<!--");
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..4 * DATAGRAM {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            xml.push(char::from(b'a' + (x >> 32) as u8 % 26));
            let table = build_lls(TABLE_SLT, 0, 1, &format!("{xml}-->"));
            if table.len() == DATAGRAM {
                return table;
            }
        }
        build_lls(TABLE_SLT, 0, 1, &plain)
    })
}

/// The service list table of the test signal.
pub fn slt() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<SLT xmlns=\"tag:atsc.org,2016:XMLSchemas/ATSC3/Delivery/SLT/1.0/\" bsid=\"1\">\
<Service serviceId=\"{SERVICE_ID}\" globalServiceID=\"urn:atsc:serviceid:test_1\" sltSvcSeqNum=\"0\" protected=\"false\" \
majorChannelNo=\"{}\" minorChannelNo=\"{}\" serviceCategory=\"1\" shortServiceName=\"{SERVICE_NAME}\">\
<BroadcastSvcSignaling slsProtocol=\"1\" slsMajorProtocolVersion=\"1\" slsMinorProtocolVersion=\"0\" \
slsDestinationIpAddress=\"{ROUTE_ADDR}\" slsDestinationUdpPort=\"{ROUTE_PORT}\" slsSourceIpAddress=\"{SOURCE}\"/>\
</Service></SLT>",
        CHANNEL.0, CHANNEL.1
    )
}

/// The session description of the service: one ROUTE session with the video and the audio channel.
pub fn stsid() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<S-TSID xmlns=\"tag:atsc.org,2016:XMLSchemas/ATSC3/Delivery/S-TSID/1.0/\">\
<RS sIpAddr=\"{SOURCE}\" dIpAddr=\"{ROUTE_ADDR}\" dPort=\"{ROUTE_PORT}\">\
<LS tsi=\"{VIDEO_TSI}\"><SrcFlow rt=\"true\"><EFDT><FDT-Instance Expires=\"4294967295\" afdt:efdtVersion=\"1\" \
afdt:maxTransportSize=\"65536\" afdt:fileTemplate=\"video-$TOI$.m4s\" xmlns=\"urn:ietf:params:xml:ns:fdt\" \
xmlns:afdt=\"tag:atsc.org,2016:XMLSchemas/ATSC3/Delivery/ATSC-FDT/1.0/\">\
<File Content-Location=\"video-init.mp4\" TOI=\"{INIT_TOI}\"/></FDT-Instance></EFDT>\
<ContentInfo><MediaInfo contentType=\"video\" repId=\"video\"/></ContentInfo></SrcFlow></LS>\
<LS tsi=\"{AUDIO_TSI}\"><SrcFlow rt=\"true\"><EFDT><FDT-Instance Expires=\"4294967295\" afdt:efdtVersion=\"1\" \
afdt:maxTransportSize=\"65536\" afdt:fileTemplate=\"audio-$TOI$.m4s\" xmlns=\"urn:ietf:params:xml:ns:fdt\" \
xmlns:afdt=\"tag:atsc.org,2016:XMLSchemas/ATSC3/Delivery/ATSC-FDT/1.0/\">\
<File Content-Location=\"audio-init.mp4\" TOI=\"{INIT_TOI}\"/></FDT-Instance></EFDT>\
<ContentInfo><MediaInfo contentType=\"audio\" repId=\"audio\"/></ContentInfo></SrcFlow></LS>\
</RS></S-TSID>"
    )
}

/// The bundle description of the service.
pub fn usbd() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<BundleDescriptionROUTE xmlns=\"tag:atsc.org,2016:XMLSchemas/ATSC3/Delivery/ROUTEUSD/1.0/\">\
<UserServiceDescription serviceId=\"{SERVICE_ID}\"><DeliveryMethod><BroadcastAppService>\
<BasePattern>video-</BasePattern><BasePattern>audio-</BasePattern>\
</BroadcastAppService></DeliveryMethod></UserServiceDescription></BundleDescriptionROUTE>"
    )
}

/// The media presentation description of the service: one period with the two representations.
pub fn mpd() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<MPD xmlns=\"urn:mpeg:dash:schema:mpd:2011\" type=\"dynamic\" availabilityStartTime=\"1970-01-01T00:00:00Z\" \
minimumUpdatePeriod=\"PT1S\" minBufferTime=\"PT1S\" profiles=\"urn:mpeg:dash:profile:isoff-broadcast:2015\">\
<Period id=\"p0\" start=\"PT0S\">\
<AdaptationSet contentType=\"video\" mimeType=\"video/mp4\" segmentAlignment=\"true\" startWithSAP=\"1\">\
<SegmentTemplate timescale=\"{VIDEO_TIMESCALE}\" duration=\"{}\" startNumber=\"{FIRST_SEGMENT_TOI}\" \
presentationTimeOffset=\"0\" initialization=\"video-init.mp4\" media=\"video-$Number$.m4s\"/>\
<Representation id=\"video\" codecs=\"hev1.2.4.L123.B0\" width=\"{}\" height=\"{}\" frameRate=\"25\" bandwidth=\"2000000\"/>\
</AdaptationSet>\
<AdaptationSet contentType=\"audio\" mimeType=\"audio/mp4\" lang=\"en\" segmentAlignment=\"true\" startWithSAP=\"1\">\
<SegmentTemplate timescale=\"{AUDIO_TIMESCALE}\" duration=\"{}\" startNumber=\"{FIRST_SEGMENT_TOI}\" \
presentationTimeOffset=\"0\" initialization=\"audio-init.mp4\" media=\"audio-$Number$.m4s\"/>\
<Representation id=\"audio\" codecs=\"ac-4.02.00.00\" audioSamplingRate=\"{AUDIO_TIMESCALE}\" bandwidth=\"96000\"/>\
</AdaptationSet></Period></MPD>",
        VIDEO_DURATION * SAMPLES_PER_SEGMENT,
        VIDEO_SIZE.0,
        VIDEO_SIZE.1,
        AUDIO_DURATION * SAMPLES_PER_SEGMENT,
    )
}

/// The metadata envelope of the signalling package: it names the three fragments and their versions.
pub fn envelope() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<metadataEnvelope xmlns=\"urn:3gpp:metadata:2005:MBMS:envelope\">\
<item metadataURI=\"usbd.xml\" version=\"1\" contentType=\"application/route-usd+xml\"/>\
<item metadataURI=\"stsid.xml\" version=\"1\" contentType=\"application/route-s-tsid+xml\"/>\
<item metadataURI=\"test.mpd\" version=\"1\" contentType=\"application/dash+xml\"/>\
</metadataEnvelope>"
        .into()
}

/// The signalling package of the service: the metadata envelope and the three fragments as the parts of one package.
pub fn sls_object() -> Vec<u8> {
    let mut out = Vec::new();
    for (kind, name, xml) in [
        ("application/mbms-envelope+xml", "envelope.xml", envelope()),
        ("application/route-usd+xml", "usbd.xml", usbd()),
        ("application/route-s-tsid+xml", "stsid.xml", stsid()),
        ("application/dash+xml", "test.mpd", mpd()),
    ] {
        out.extend_from_slice(
            format!("--sls\r\nContent-Type: {kind}\r\nContent-Location: {name}\r\n\r\n").as_bytes(),
        );
        out.extend_from_slice(xml.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"--sls--\r\n");
    // Line ends after the last boundary belong to no part.
    out.resize(out.len().max(OBJECT), b'\n');
    out
}

/// The file description of the signalling channel: it lists the signalling package.
pub fn sls_files() -> String {
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<FDT-Instance xmlns=\"urn:ietf:params:xml:ns:fdt\" Expires=\"4294967295\">\
<File Content-Location=\"sls.pkg\" TOI=\"{SLS_TOI}\" Content-Type=\"multipart/related\"/></FDT-Instance>"
    );
    // White space after the document element fills the object.
    format!("{xml:<OBJECT$}")
}

/// Appends a `free` box that fills `file` to [`OBJECT`] bytes.
fn fill(file: &mut Vec<u8>) {
    // A box has a head of 8 bytes.
    if file.len() + 8 <= OBJECT {
        write_box(file, b"free", &vec![0; OBJECT - file.len() - 8]);
    }
}

/// A full box body: version, flags, then `rest`.
fn full(version: u8, flags: u32, rest: &[u8]) -> Vec<u8> {
    let mut v = vec![
        version,
        (flags >> 16) as u8,
        (flags >> 8) as u8,
        flags as u8,
    ];
    v.extend_from_slice(rest);
    v
}

/// An init segment of track 1: sample entry `entry` with the body `sample_entry`, handler `handler`, `timescale` ticks per
/// second and a default sample duration.
fn init(
    entry: &[u8; 4],
    sample_entry: &[u8],
    handler: &[u8; 4],
    timescale: u32,
    duration: u32,
) -> Vec<u8> {
    let mut out = Vec::new();
    write_box(&mut out, b"ftyp", b"iso6\0\0\0\0iso6dash");
    let mut tkhd = vec![0u8; 84];
    tkhd[3] = 3;
    tkhd[12..16].copy_from_slice(&1u32.to_be_bytes());
    let mut mdhd = vec![0u8; 24];
    mdhd[12..16].copy_from_slice(&timescale.to_be_bytes());
    let mut hdlr = vec![0u8; 25];
    hdlr[8..12].copy_from_slice(handler);
    let mut stsd = full(0, 0, &1u32.to_be_bytes());
    write_box(&mut stsd, entry, sample_entry);
    let mut stbl = Vec::new();
    write_box(&mut stbl, b"stsd", &stsd);
    let mut minf = Vec::new();
    write_box(&mut minf, b"stbl", &stbl);
    let mut mdia = Vec::new();
    write_box(&mut mdia, b"mdhd", &mdhd);
    write_box(&mut mdia, b"hdlr", &hdlr);
    write_box(&mut mdia, b"minf", &minf);
    let mut trak = Vec::new();
    write_box(&mut trak, b"tkhd", &tkhd);
    write_box(&mut trak, b"mdia", &mdia);
    let mut trex = full(0, 0, &1u32.to_be_bytes());
    trex.extend_from_slice(&1u32.to_be_bytes());
    trex.extend_from_slice(&duration.to_be_bytes());
    trex.extend_from_slice(&[0; 8]);
    let mut mvex = Vec::new();
    write_box(&mut mvex, b"trex", &trex);
    let mut moov = Vec::new();
    write_box(&mut moov, b"trak", &trak);
    write_box(&mut moov, b"mvex", &mvex);
    write_box(&mut out, b"moov", &moov);
    fill(&mut out);
    out
}

/// The decoder configuration record of the video track: Main 10 profile, level 4.1, NAL unit lengths of 4 bytes and no
/// parameter set arrays (the sample entry is `hev1`: parameter sets come in the samples).
pub fn video_config() -> [u8; 23] {
    let mut c = [0u8; 23];
    c[0] = 1;
    c[1] = 2;
    c[2] = 0x20;
    c[12] = 123;
    c[13] = 0xF0;
    c[15] = 0xFC;
    c[16] = 0xFD;
    c[17] = 0xFA;
    c[18] = 0xFA;
    c[21] = 0x0F;
    c
}

/// The bytes of the decoder configuration box of the audio track. They are synthetic.
pub const AUDIO_CONFIG: [u8; 11] = [0x20, 0xA4, 0x01, 0x40, 0, 0, 0, 0x1F, 0xFF, 0xFF, 0xE0];

/// The init segment of the video component.
pub fn video_init() -> Vec<u8> {
    let mut se = vec![0u8; 78];
    se[7] = 1;
    se[24..26].copy_from_slice(&VIDEO_SIZE.0.to_be_bytes());
    se[26..28].copy_from_slice(&VIDEO_SIZE.1.to_be_bytes());
    se[28..32].copy_from_slice(&0x0048_0000u32.to_be_bytes());
    se[32..36].copy_from_slice(&0x0048_0000u32.to_be_bytes());
    se[41] = 1;
    se[75] = 0x18;
    se[76..78].copy_from_slice(&[0xFF, 0xFF]);
    write_box(&mut se, b"hvcC", &video_config());
    init(b"hev1", &se, b"vide", VIDEO_TIMESCALE, VIDEO_DURATION)
}

/// The init segment of the audio component: two channels at 48 kHz.
pub fn audio_init() -> Vec<u8> {
    let mut se = vec![0u8; 28];
    se[7] = 1;
    se[17] = 2;
    se[19] = 16;
    se[24..28].copy_from_slice(&(AUDIO_TIMESCALE << 16).to_be_bytes());
    write_box(&mut se, b"dac4", &AUDIO_CONFIG);
    init(b"ac-4", &se, b"soun", AUDIO_TIMESCALE, AUDIO_DURATION)
}

/// `len` bytes that follow from `key`.
fn synthetic(key: u64, len: usize, out: &mut Vec<u8>) {
    let mut x = key.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    for _ in 0..len {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push((x >> 32) as u8);
    }
}

/// Access unit `sample` of video segment `segment`: one NAL unit behind its 4-byte length. The first of a segment is an
/// IDR picture (NAL unit type 19) of 600 bytes, the others are trailing pictures (type 1) of 200 bytes.
pub fn video_sample(segment: u64, sample: u32) -> Vec<u8> {
    let (kind, len) = if sample == 0 { (19u8, 600) } else { (1, 200) };
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&(len as u32 - 4).to_be_bytes());
    out.extend_from_slice(&[kind << 1, 1]);
    synthetic(segment << 8 | u64::from(sample), len - 6, &mut out);
    out
}

/// Access unit `sample` of audio segment `segment`: 96 bytes.
pub fn audio_sample(segment: u64, sample: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(96);
    synthetic(1 << 40 | segment << 8 | u64::from(sample), 96, &mut out);
    out
}

/// A media segment of track 1: sequence number `segment + 1`, base decoding time `time`, and the samples with their flags
/// and composition offsets (`None`: the run carries only the sizes).
fn media_segment(segment: u64, time: u64, samples: &[Vec<u8>], video: bool) -> Vec<u8> {
    let mut seg = Vec::new();
    write_box(&mut seg, b"styp", b"msdh\0\0\0\0msdhdash");
    let mfhd = full(0, 0, &(segment as u32 + 1).to_be_bytes());
    let tfhd = full(0, 0x02_0000, &1u32.to_be_bytes());
    let tfdt = full(1, 0, &time.to_be_bytes());
    let flags = if video { 0x01 | 0x200 | 0x400 | 0x800 } else { 0x01 | 0x200 };
    let build = |offset: u32| {
        let mut trun = full(0, flags, &(samples.len() as u32).to_be_bytes());
        trun.extend_from_slice(&offset.to_be_bytes());
        for (k, s) in samples.iter().enumerate() {
            trun.extend_from_slice(&(s.len() as u32).to_be_bytes());
            if video {
                // The first depends on no other sample; the others depend on one and are no sync samples.
                let f: u32 = if k == 0 { 0x0200_0000 } else { 0x0101_0000 };
                trun.extend_from_slice(&f.to_be_bytes());
                trun.extend_from_slice(&VIDEO_OFFSET.to_be_bytes());
            }
        }
        let mut traf = Vec::new();
        write_box(&mut traf, b"tfhd", &tfhd);
        write_box(&mut traf, b"tfdt", &tfdt);
        write_box(&mut traf, b"trun", &trun);
        let mut body = Vec::new();
        write_box(&mut body, b"mfhd", &mfhd);
        write_box(&mut body, b"traf", &traf);
        let mut moof = Vec::new();
        write_box(&mut moof, b"moof", &body);
        moof
    };
    // The data offset counts from the first byte of the movie fragment box to the first sample.
    let offset = build(0).len() as u32 + 8;
    seg.extend(build(offset));
    let data: Vec<u8> = samples.concat();
    write_box(&mut seg, b"mdat", &data);
    fill(&mut seg);
    seg
}

/// Media segment `segment` of the video component.
pub fn video_segment(segment: u64) -> Vec<u8> {
    let samples: Vec<Vec<u8>> = (0..SAMPLES_PER_SEGMENT)
        .map(|k| video_sample(segment, k))
        .collect();
    let time = segment * u64::from(VIDEO_DURATION * SAMPLES_PER_SEGMENT);
    media_segment(segment, time, &samples, true)
}

/// Media segment `segment` of the audio component.
pub fn audio_segment(segment: u64) -> Vec<u8> {
    let samples: Vec<Vec<u8>> = (0..SAMPLES_PER_SEGMENT)
        .map(|k| audio_sample(segment, k))
        .collect();
    let time = segment * u64::from(AUDIO_DURATION * SAMPLES_PER_SEGMENT);
    media_segment(segment, time, &samples, false)
}

/// One UDP datagram of the test signal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Datagram {
    /// The destination address.
    pub dst: Ipv4Addr,
    /// The destination port.
    pub port: u16,
    /// The payload.
    pub payload: Vec<u8>,
}

/// Appends the ROUTE packets of `object` on channel `tsi` of the service's session: parts of [`CHUNK`] bytes, each with the
/// transfer length, the last with the close object flag.
pub fn route_packets(tsi: u32, toi: u32, codepoint: u8, object: &[u8], out: &mut VecDeque<Datagram>) {
    let last = object.len().saturating_sub(1) / CHUNK;
    for (k, part) in object.chunks(CHUNK).enumerate() {
        let mut payload = Vec::with_capacity(24 + part.len());
        LctBuilder {
            tsi,
            toi,
            codepoint,
            transfer_len: Some(object.len() as u64),
            close_object: k == last,
        }
        .write(&mut payload)
        .expect("an object below the transfer length limit");
        payload.extend_from_slice(&((k * CHUNK) as u32).to_be_bytes());
        payload.extend_from_slice(part);
        out.push_back(Datagram {
            dst: ROUTE_ADDR,
            port: ROUTE_PORT,
            payload,
        });
    }
}

/// Appends the datagrams of round `segment` of the service: the service list, the signalling channel, the two init segments
/// and media segment `segment` of the video and of the audio. Every round repeats the signalling and the init segments, so a
/// receiver that starts at any round has all it needs.
pub fn round(segment: u64, out: &mut VecDeque<Datagram>) {
    out.push_back(Datagram {
        dst: Ipv4Addr::from(LLS_ADDR),
        port: LLS_PORT,
        payload: lls().to_vec(),
    });
    route_packets(0, 0, CODEPOINT_FILE, sls_files().as_bytes(), out);
    route_packets(0, SLS_TOI, CODEPOINT_SLS, &sls_object(), out);
    let init = if segment == 0 {
        CODEPOINT_INIT
    } else {
        CODEPOINT_INIT_AGAIN
    };
    route_packets(VIDEO_TSI, INIT_TOI, init, &video_init(), out);
    route_packets(AUDIO_TSI, INIT_TOI, init, &audio_init(), out);
    let toi = FIRST_SEGMENT_TOI.wrapping_add(segment as u32);
    route_packets(VIDEO_TSI, toi, CODEPOINT_MEDIA, &video_segment(segment), out);
    route_packets(AUDIO_TSI, toi, CODEPOINT_MEDIA, &audio_segment(segment), out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atsc3::svc::{parse_lls, parse_slt};

    #[test]
    fn every_datagram_of_a_round_has_one_size() {
        let mut out = VecDeque::new();
        round(0, &mut out);
        round(7, &mut out);
        assert!(out.iter().all(|d| d.payload.len() == DATAGRAM));
        assert_eq!(out.len(), 2 * (1 + 6 * OBJECT / CHUNK));
        assert_eq!(sls_object().len(), OBJECT);
        // The filled service list still reads.
        let read = parse_lls(lls());
        assert_eq!((read.tables.len(), read.refused), (1, 0));
        let slt = parse_slt(&read.tables[0].xml).expect("a service list");
        assert_eq!(slt.services.len(), 1);
        let s = &slt.services[0];
        assert_eq!((s.id, s.major, s.minor), (SERVICE_ID, CHANNEL.0, CHANNEL.1));
        assert_eq!((s.short_name.as_str(), s.sls_protocol, s.sls_port), (SERVICE_NAME, 1, ROUTE_PORT));
    }
}
