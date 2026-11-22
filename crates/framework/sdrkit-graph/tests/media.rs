//! Compressed media streams: codec descriptions in formats, codec checks on connect and configure, decoders chosen by codec code,
//! packets passed on without copies, pictures.

use sdrkit_core::{Complex32, ConfigError, InputInfo, Output, Receiver, StreamKind};
use sdrkit_graph::media::{Codec, MediaFormat, MediaSpec, Packaging, PixelFormat};
use sdrkit_graph::*;
use std::sync::{Arc, Mutex};

fn info() -> InputInfo {
    InputInfo {
        rate_hz: 10e3,
        centre_hz: 0.0,
        tuned_hz: 0.0,
        live: true,
    }
}

/// Makes one access unit per 1000 input samples, in `codec` and `packaging`; declares them on its port when `declare` is set.
struct Encoder {
    codec: Codec,
    packaging: Packaging,
    declare: bool,
    rate: f64,
    shared: Arc<[u8]>,
}
impl Encoder {
    fn new(codec: Codec, packaging: Packaging, declare: bool) -> Self {
        Encoder {
            codec,
            packaging,
            declare,
            rate: 0.0,
            shared: Arc::from(vec![0u8, 0, 0, 1, 0x65, 0x88, 0x84].as_slice()),
        }
    }
}
impl Component for Encoder {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        let specs = if self.declare {
            vec![MediaSpec::new(self.codec, self.packaging)]
        } else {
            vec![]
        };
        ComponentInfo::processor(
            "encoder",
            vec![Port::iq("in")],
            vec![Port::media("out", &specs)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        let mut m = MediaFormat::new(self.codec, self.packaging);
        m.config = Arc::from(&[0x67u8, 0x64, 0x00, 0x1f][..]);
        m.video = Some(media::VideoProps {
            width: 1280,
            height: 720,
            frame_rate: Some((25, 1)),
            aspect: Some((16, 9)),
            ..Default::default()
        });
        Ok(vec![Format {
            media: Some(m),
            ..Format::new(Kind::Bytes, 0.0)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let b = io.input(0);
        let (first, end) = (b.first(), b.end());
        for at in (first.div_ceil(1000) * 1000..end).step_by(1000) {
            let k = (at / 1000) as i64;
            let p = Packet::new(self.shared.clone(), at as f64 / self.rate).stamped(
                Some(k * 3600),
                Some(k * 3600 - 1800),
                k % 5 == 0,
            );
            io.bytes_out(0).push(p);
            if k == 3 {
                let out = io.output(0);
                let pos = out.end() - 1;
                out.push_tag(
                    pos,
                    TagKind::ConfigChanged {
                        config: Arc::from(&[0x67u8, 0x4d][..]),
                    },
                );
            }
        }
    }
}

/// A decoder that accepts `specs` and keeps the packets it gets.
struct Decoder(
    Vec<MediaSpec>,
    Arc<Mutex<Vec<Packet>>>,
    Arc<Mutex<Vec<Tag>>>,
);
impl Component for Decoder {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("decoder", vec![Port::media("in", &self.0)])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        self.1.lock().unwrap().extend_from_slice(io.bytes(0));
        self.2.lock().unwrap().extend_from_slice(io.input(0).tags());
    }
}

type Kept<T> = Arc<Mutex<Vec<T>>>;

fn decoder(specs: &[MediaSpec]) -> (Decoder, Kept<Packet>, Kept<Tag>) {
    let (p, t) = (
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    );
    (Decoder(specs.to_vec(), p.clone(), t.clone()), p, t)
}

#[test]
fn an_h264_annexb_producer_connects_to_a_matching_decoder_only() {
    let (dec, packets, tags) = decoder(&[
        MediaSpec::new(Codec::H264, Packaging::AnnexB),
        MediaSpec::new(Codec::H264, Packaging::LengthPrefixed(4)),
    ]);
    let mut g = GraphBuilder::new()
        .add("enc", Encoder::new(Codec::H264, Packaging::AnnexB, true))
        .add("dec", dec)
        .input("enc")
        .connect("enc", "dec")
        .build()
        .unwrap();
    g.configure(&info()).unwrap();
    let m = g.format("enc").unwrap().media.clone().unwrap();
    assert_eq!(
        (m.codec, m.fourcc().as_str(), m.packaging),
        (Codec::H264, "avc1", Packaging::AnnexB)
    );
    assert_eq!(m.video.unwrap().width, 1280);
    let mut out = Output::default();
    for c in vec![Complex32::default(); 10_000].chunks(700) {
        g.feed(c, &mut out);
    }
    let p = packets.lock().unwrap();
    assert_eq!(p.len(), 10);
    assert_eq!(
        (p[2].pts, p[2].dts, p[5].keyframe, p[6].keyframe),
        (Some(7200), Some(5400), true, false)
    );
    assert_eq!(p[0].bytes(), &[0, 0, 0, 1, 0x65, 0x88, 0x84]);
    // Every packet shares the producer's one allocation: passing packets on copies nothing.
    assert!(p.iter().all(|q| Arc::ptr_eq(&q.data, &p[0].data)));
    assert_eq!(
        tags.lock().unwrap()[0],
        Tag {
            at: 3,
            kind: TagKind::ConfigChanged {
                config: Arc::from(&[0x67u8, 0x4d][..])
            }
        }
    );

    // A decoder of HEVC only is refused when the graph is built.
    let (hevc, _, _) = decoder(&[MediaSpec::any(Codec::Hevc)]);
    let e = GraphBuilder::new()
        .add("enc", Encoder::new(Codec::H264, Packaging::AnnexB, true))
        .add("dec", hevc)
        .input("enc")
        .connect("enc", "dec")
        .build()
        .unwrap_err();
    assert!(
        matches!(&e, GraphError::Connect { error, .. }
            if matches!(**error, ConnectError::CodecMismatch { .. })),
        "{e}"
    );

    // So is the right codec in the wrong packaging.
    let (lp, _, _) = decoder(&[MediaSpec::new(Codec::H264, Packaging::LengthPrefixed(4))]);
    let e = GraphBuilder::new()
        .add("enc", Encoder::new(Codec::H264, Packaging::AnnexB, true))
        .add("dec", lp)
        .input("enc")
        .connect("enc", "dec")
        .build();
    assert!(matches!(e, Err(GraphError::Connect { error, .. })
        if matches!(*error, ConnectError::CodecMismatch { .. })));
}

#[test]
fn a_codec_known_only_when_configured_is_checked_then() {
    let (hevc, _, _) = decoder(&[MediaSpec::any(Codec::Hevc)]);
    let mut g = GraphBuilder::new()
        .add("enc", Encoder::new(Codec::H264, Packaging::AnnexB, false))
        .add("dec", hevc)
        .input("enc")
        .connect("enc", "dec")
        .build()
        .expect("nothing to check before the stream starts");
    let e = g.configure(&info()).unwrap_err();
    assert!(
        e.to_string()
            .contains("input in gets h264 AnnexB, it accepts hevc"),
        "{e}"
    );
}

#[test]
fn media_leaves_the_graph_for_the_hosts_decoders() {
    let mut g = GraphBuilder::new()
        .add("enc", Encoder::new(Codec::H264, Packaging::AnnexB, true))
        .input("enc")
        .output("enc")
        .build()
        .unwrap();
    g.configure(&info()).unwrap();
    let mut out = Output::default();
    g.feed(&vec![Complex32::default(); 3000], &mut out);
    assert_eq!(out.media.len(), 3);
    let m = &out.media[1];
    assert_eq!(m.kind, StreamKind::H264);
    assert_eq!(m.packet.bytes(), &[0, 0, 0, 1, 0x65, 0x88, 0x84]);
    assert_eq!(m.packet.pts, Some(3600));
    assert_eq!(m.format.codec, Codec::H264);
    assert_eq!(m.format.packaging, Packaging::AnnexB);
    // No copy at the edge: the host gets the graph's own bytes, and one shared format for every packet.
    let Data::Bytes(p) = &g.buffer("enc").unwrap().data else {
        panic!("bytes")
    };
    assert!(std::sync::Arc::ptr_eq(&p[1].data, &m.packet.data));
    assert!(std::sync::Arc::ptr_eq(
        &out.media[0].format,
        &out.media[2].format
    ));
}

#[test]
fn the_registry_picks_a_hardware_decoder_before_a_software_one() {
    let mut r = Registry::standard();
    let avc1 = Codec::H264.fourcc();
    let mk = || -> Box<dyn Component> { Box::new(decoder(&[MediaSpec::any(Codec::H264)]).0) };
    r.add_decoder("h264_soft", &[avc1], false, mk);
    assert_eq!(r.decoder_for(avc1).unwrap().name, "h264_soft");
    r.add_decoder("h264_hard", &[avc1, FourCc::new("hvc1")], true, mk);
    assert_eq!(r.decoder_for(avc1).unwrap().name, "h264_hard");
    assert_eq!(
        r.decoder_for(FourCc::new("hvc1")).unwrap().name,
        "h264_hard"
    );
    assert!(r.decoder_for(FourCc::new("mp2v")).is_none());
    assert!(r.make("h264_soft").is_ok());
}

/// Makes a raw grey picture line every call (an SSTV-like source).
struct Lines;
impl Component for Lines {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "lines",
            vec![Port::iq("in")],
            vec![Port::typed::<Picture>("out")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Picture>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        let layout = PixelLayout::packed(PixelFormat::Grey8, n as u32, 1);
        let data: Arc<[u8]> = (0..n).map(|i| i as u8).collect::<Vec<u8>>().into();
        let time = io.input_format(0).time_of(io.input(0).first());
        io.items_out::<Picture>(0).push(Timed {
            time,
            value: Picture {
                codec: Codec::Pixels(PixelFormat::Grey8),
                layout: Some(layout),
                name: None,
                id: 1,
                state: sdrkit_core::PictureState::Finished,
                data,
            },
        });
    }
}

#[test]
fn pictures_are_shared_items_and_leave_the_graph_as_pictures() {
    let mut g = GraphBuilder::new()
        .add("lines", Lines)
        .input("lines")
        .output("lines")
        .build()
        .unwrap();
    g.configure(&info()).unwrap();
    let mut tap = g.tap("lines", 4).unwrap();
    let mut out = Output::default();
    g.feed(&[Complex32::default(); 640], &mut out);
    // A picture output goes to the host as a picture, with its pixel layout, not as an event.
    assert!(out.events.is_empty());
    assert_eq!(out.media[0].kind, sdrkit_core::StreamKind::Picture);
    assert_eq!(out.media[0].format.codec, Codec::Pixels(PixelFormat::Grey8));
    assert_eq!(out.media[0].format.pixels.as_ref().unwrap().width, 640);
    let mut b = Buffer::default();
    assert!(tap.pop(&mut b));
    let Data::Items(Some(items)) = &b.data else {
        panic!("typed items")
    };
    let pics = items
        .as_any()
        .downcast_ref::<Vec<Timed<Picture>>>()
        .unwrap();
    let in_graph = g.buffer("lines").unwrap();
    let Data::Items(Some(orig)) = &in_graph.data else {
        panic!()
    };
    let orig = orig.as_any().downcast_ref::<Vec<Timed<Picture>>>().unwrap();
    assert!(
        Arc::ptr_eq(&pics[0].value.data, &orig[0].value.data),
        "a tap shares the picture's bytes"
    );
}

/// Makes one 188-byte transport stream packet per 1000 input samples, as plain bytes: a byte stream with no media format.
struct TsSource {
    rate: f64,
    packet: Arc<[u8]>,
}
impl Component for TsSource {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("test.ts", vec![Port::iq("in")], vec![Port::bytes("out")])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        Ok(vec![Format::new(Kind::Bytes, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let b = io.input(0);
        for at in (b.first().div_ceil(1000) * 1000..b.end()).step_by(1000) {
            let p = Packet::new(self.packet.clone(), at as f64 / self.rate);
            io.bytes_out(0).push(p);
        }
    }
}

#[test]
fn a_byte_output_without_a_codec_stays_in_the_graph_and_a_codec_the_host_lacks_is_refused() {
    // ISO/IEC 13818-1 2.4.3.2: a transport packet is 188 bytes and starts with the sync byte 0x47.
    let mut ts = vec![0u8; 188];
    ts[0] = 0x47;
    let mut g = GraphBuilder::new()
        .add(
            "ts",
            TsSource {
                rate: 0.0,
                packet: Arc::from(ts.as_slice()),
            },
        )
        .input("ts")
        .output("ts")
        .build()
        .unwrap();
    g.configure(&info()).unwrap();
    let mut out = Output::default();
    g.feed(&vec![Complex32::default(); 5000], &mut out);
    // Not media for the host's decoders: the host or the stack reads it from the graph.
    assert!(out.media.is_empty());
    match &g.buffer("ts.out").unwrap().data {
        stream::Data::Bytes(p) => {
            assert_eq!(p.len(), 5);
            assert!(p
                .iter()
                .all(|p| p.bytes().len() == 188 && p.bytes()[0] == 0x47));
        }
        d => panic!("{d:?}"),
    }

    // Media in a codec the host does not decode (speech goes to a codec plug-in, not to the host) is refused, with the codec
    // named.
    let mut g = GraphBuilder::new()
        .add("enc", Encoder::new(Codec::Ambe2, Packaging::Raw, false))
        .input("enc")
        .output("enc")
        .build()
        .unwrap();
    let e = g.configure(&info()).unwrap_err().to_string();
    assert!(e.contains("ambe2") && e.contains("does not decode"), "{e}");
}

/// Makes one AAC access unit per 1000 input samples, as `MediaPacket` items; the format changes at sample 3000 (a new service),
/// and the packet at sample 4000 is in a codec the host does not decode.
struct Service {
    rate: f64,
    first: Arc<MediaFormat>,
    second: Arc<MediaFormat>,
    other: Arc<MediaFormat>,
    unit: Arc<[u8]>,
}
impl Service {
    fn new() -> Service {
        let mut a = MediaFormat::new(Codec::HeAac, Packaging::Raw);
        a.config = Arc::from(&[0x13u8, 0x10][..]); // AudioSpecificConfig: SBR, 24 kHz core, mono
        let mut b = MediaFormat::new(Codec::HeAac, Packaging::Raw);
        b.config = Arc::from(&[0x13u8, 0x90][..]); // the same at 2 channels: another service
        Service {
            rate: 0.0,
            first: Arc::new(a),
            second: Arc::new(b),
            other: Arc::new(MediaFormat::new(Codec::Ambe2, Packaging::Raw)),
            unit: Arc::from(&[0x21u8, 0x10, 0x04][..]),
        }
    }
}
impl Component for Service {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.service",
            vec![Port::iq("in")],
            vec![Port::typed::<MediaPacket>("media")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        Ok(vec![Format::new(Kind::typed::<MediaPacket>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let b = io.input(0);
        for at in (b.first().div_ceil(1000) * 1000..b.end()).step_by(1000) {
            let t = at as f64 / self.rate;
            let format = match at {
                0..3000 => &self.first,
                4000 => &self.other,
                _ => &self.second,
            };
            let p = Packet::new(self.unit.clone(), t).stamped(Some(at as i64), None, true);
            io.items_out::<MediaPacket>(0).push(Timed {
                time: t,
                value: MediaPacket::new(p, format.clone()),
            });
        }
    }
}

#[test]
fn a_media_output_changes_its_format_from_one_packet_to_the_next() {
    let mut g = GraphBuilder::new()
        .add("svc", Service::new())
        .input("svc")
        .output("svc")
        .build()
        .unwrap();
    g.configure(&info()).unwrap();
    let mut out = Output::default();
    g.feed(&vec![Complex32::default(); 6000], &mut out);
    // Packets at 0, 1000, ... 5000; the one at 4000 is not for the host.
    let pts: Vec<_> = out.media.iter().map(|m| m.packet.pts.unwrap()).collect();
    assert_eq!(pts, [0, 1000, 2000, 3000, 5000]);
    assert!(out.media.iter().all(|m| m.kind == StreamKind::HeAac));
    let configs: Vec<_> = out.media.iter().map(|m| m.format.config[1]).collect();
    assert_eq!(configs, [0x10, 0x10, 0x10, 0x90, 0x90]);
    // The format is shared with the component, not copied, and so are the bytes.
    assert!(Arc::ptr_eq(&out.media[0].format, &out.media[2].format));
    assert!(Arc::ptr_eq(
        &out.media[0].packet.data,
        &out.media[4].packet.data
    ));
    assert!(!Arc::ptr_eq(&out.media[2].format, &out.media[3].format));
}

#[test]
fn a_speech_packet_reaches_the_codec_as_a_frame_a_damaged_frame_or_an_erasure() {
    use sdrkit_core::codec::{AudioCodec, CodecError, CodecSet, Pcm};
    use sdrkit_graph::codec::GuardedCodec;
    /// 8 kHz, 20 ms: 160 samples per frame. The first payload byte times 100, 0 for a concealed frame,
    /// and -1 when the frame arrives with no bytes and no erasure flag.
    struct Echo;
    impl AudioCodec for Echo {
        fn decode(&mut self, frame: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
            pcm.rate_hz = 8000;
            pcm.channels = 1;
            let v = match (erasure, frame.first()) {
                (true, _) => 0,
                (false, Some(b)) => *b as i16 * 100,
                (false, None) => -1,
            };
            pcm.samples.extend(std::iter::repeat_n(v, 160));
            Ok(())
        }
    }
    let ambe2 = FourCc::new("amb2");
    let mut set = CodecSet::default();
    set.register(
        ambe2,
        Some("codewords"),
        Arc::new(|_: &sdrkit_core::media::MediaFormat| Some(Box::new(Echo) as Box<dyn AudioCodec>)),
    );
    let mut g = GuardedCodec::start(
        set.find(ambe2, "codewords").unwrap(),
        &sdrkit_core::media::MediaFormat::new(
            sdrkit_core::media::Codec::Ambe2,
            sdrkit_core::media::Packaging::Raw,
        ),
    );

    let words = Packet::new(Arc::from(&[7u8; 12][..]), 0.5).stamped(Some(4000), None, true);
    let good = SpeechPacket::frame(
        ambe2,
        sdrkit_graph::SpeechPackaging::Codewords,
        20,
        words.clone(),
    )
    .in_call(3);
    let pcm = g.decode_packet(&good).unwrap();
    assert_eq!(
        (pcm.samples.len(), pcm.samples[0]),
        (good.samples_8k(), 700)
    );
    let bad = SpeechPacket::frame(ambe2, sdrkit_graph::SpeechPackaging::Codewords, 20, words)
        .damaged(Some(4));
    assert_eq!(
        g.decode_packet(&bad).unwrap().samples[0],
        700,
        "the codec's own FEC acts on it"
    );
    let gap = SpeechPacket::erasure(
        ambe2,
        sdrkit_graph::SpeechPackaging::Codewords,
        20,
        4160,
        0.52,
    );
    assert_eq!(
        (gap.packet.pts, gap.packet.time, gap.payload().len()),
        (Some(4160), 0.52, 0)
    );
    let pcm = g.decode_packet(&gap).unwrap();
    assert_eq!((pcm.samples.len(), pcm.samples[0]), (160, 0));
    assert_eq!(Kind::typed::<SpeechPacket>().to_string(), "media.speech");
}
