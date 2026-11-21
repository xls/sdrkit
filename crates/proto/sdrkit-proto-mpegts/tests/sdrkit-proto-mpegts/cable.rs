//! The transport stream of a DVB-T cable recording (219.5 MHz, about 10 s, nine services, two runs of lost samples) as a
//! file that `SDRKIT_TS_CABLE_A` names. The sound of every service decodes through its damaged frames. Without the
//! variable the test is skipped.

use sdrkit_core::codec::{CodecError, CodecSet, Pcm};
use sdrkit_core::media::{ChannelLayout, Codec, MediaFormat, MediaType};
use sdrkit_proto_mpegts::components::decodes;
use sdrkit_proto_mpegts::{Demultiplexer, DemuxOutput};

/// The 188-byte packets of the file, or `None` with the reason printed.
fn packets() -> Option<Vec<Vec<u8>>> {
    let Some(path) = std::env::var_os("SDRKIT_TS_CABLE_A").filter(|p| !p.is_empty()) else {
        eprintln!("skipped: set SDRKIT_TS_CABLE_A to the transport stream of the recording");
        return None;
    };
    let ts = std::fs::read(path).expect("the file reads");
    Some(ts.as_chunks::<188>().0.iter().map(|c| c.to_vec()).collect())
}

/// What the sound of one service gave.
#[derive(Debug, Default)]
struct Sound {
    codec: Option<Codec>,
    /// `None` when `codecs` has no decoder that takes the format.
    decoded: Option<bool>,
    rate: u32,
    channels: u8,
    layout: ChannelLayout,
    /// Samples per channel.
    samples: usize,
    /// Samples per channel of a second decoder whose format asks for stereo, and the layout it gave.
    stereo: (usize, ChannelLayout),
}

/// The sound of service `sid`, decoded as the graph decodes it: the packets that the demultiplexer component passes on,
/// and the decoder of the stream ends at the first `Unsupported`.
fn sound(packets: &[Vec<u8>], sid: u16, codecs: &CodecSet) -> Sound {
    let mut d = Demultiplexer::new();
    let mut out = DemuxOutput::default();
    let mut decoder = None;
    let mut stereo = None;
    let mut pcm = Pcm::default();
    let mut s = Sound::default();
    for p in packets {
        if d.selection.service.is_none() && d.si.service(sid).is_some() {
            d.select(Some(sid));
        }
        d.push(p, None, false, &mut out);
        for m in out.media.drain(..).filter(|m| m.decodable()) {
            if m.format.codec.media() != MediaType::Audio {
                continue;
            }
            if s.codec.is_none() {
                s.codec = Some(m.format.codec);
                s.decoded = decodes(codecs, &m.format);
                let mut two = MediaFormat::clone(&m.format);
                if let Some(a) = two.audio.as_mut() {
                    a.channels = 2;
                }
                let entry = codecs.find(m.format.fourcc(), "");
                decoder = entry.and_then(|e| (e.factory)(&m.format));
                stereo = entry.and_then(|e| (e.factory)(&two));
            }
            let (Some(dec), Some(two)) = (decoder.as_mut(), stereo.as_mut()) else {
                continue;
            };
            let mut p2 = Pcm::default();
            if two.decode(m.packet.bytes(), false, &mut p2).is_ok() && p2.channels > 0 {
                s.stereo.0 += p2.samples.len() / usize::from(p2.channels);
                s.stereo.1 = p2.layout();
            }
            pcm.samples.clear();
            match dec.decode(m.packet.bytes(), false, &mut pcm) {
                Ok(()) if pcm.channels > 0 => {
                    s.rate = pcm.rate_hz;
                    s.channels = pcm.channels;
                    s.layout = pcm.layout();
                    s.samples += pcm.samples.len() / usize::from(pcm.channels);
                }
                Ok(()) | Err(CodecError::Frame) => {}
                Err(e) => panic!("service {sid}: {e:?}: the stream would have no more sound"),
            }
        }
        out.clear();
    }
    s
}

#[test]
fn every_service_of_the_cable_recording_has_sound() {
    let Some(packets) = packets() else {
        return;
    };
    let mut codecs = CodecSet::default();
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    sdrkit_codec_apple::register(&mut codecs);
    sdrkit_codec_aac::register(&mut codecs);
    sdrkit_codec_mpa::register(&mut codecs);
    let apple = cfg!(any(target_os = "macos", target_os = "ios"));
    // Service id, codec, channels.
    let services = [
        (1573, Codec::Mp2, 2),
        (1569, Codec::Ac3, 6),
        (1576, Codec::Ac3, 6),
        (1575, Codec::Ac3, 6),
        (1571, Codec::Mp2, 2),
        (1570, Codec::Ac3, 6),
        (1572, Codec::Ac3, 6),
        (1574, Codec::Aac, 2),
        (1577, Codec::Aac, 2),
    ];
    for (sid, codec, channels) in services {
        let s = sound(&packets, sid, &codecs);
        eprintln!("service {sid}: {s:?}");
        assert_eq!(s.codec, Some(codec), "service {sid}");
        if codec == Codec::Ac3 && !apple {
            assert_eq!(s.decoded, Some(false), "service {sid}");
            continue;
        }
        assert_eq!(s.decoded, Some(true), "service {sid}");
        assert_eq!((s.rate, s.channels), (48_000, channels), "service {sid}");
        let layout = if channels == 6 {
            ChannelLayout::Surround51
        } else {
            ChannelLayout::Stereo
        };
        assert_eq!(s.layout, layout, "service {sid}");
        // A format that asks for stereo gets the same sound in two channels.
        assert_eq!(
            s.stereo,
            (s.samples, ChannelLayout::Stereo),
            "service {sid}"
        );
        // About 8 s of sound in the 10 s of the file: the PMT comes after about 1 s, and damaged frames are dropped.
        assert!(
            s.samples > 7 * 48_000,
            "service {sid}: {} samples",
            s.samples
        );
    }
}

/// A stack decoder's stand-in: the packets of the file, 200 for each call, 0.1 ms apart.
struct FileTs {
    packets: Vec<Vec<u8>>,
    at: usize,
    out: sdrkit_proto_mpegts::stream::TsOut,
}

impl sdrkit_graph::Component for FileTs {
    fn info(&self) -> sdrkit_graph::ComponentInfo {
        sdrkit_graph::ComponentInfo::processor(
            "test.file_ts",
            vec![sdrkit_graph::Port::new("in", sdrkit_graph::Kind::Iq)],
            vec![sdrkit_proto_mpegts::stream::port("out")],
        )
    }
    fn configure(
        &mut self,
        _: &[sdrkit_graph::Format],
    ) -> Result<Vec<sdrkit_graph::Format>, sdrkit_core::ConfigError> {
        Ok(vec![sdrkit_graph::Format::new(
            sdrkit_proto_mpegts::stream::kind(),
            0.0,
        )])
    }
    fn process(&mut self, io: &mut sdrkit_graph::Io) {
        use sdrkit_proto_mpegts::stream::TsPacket;
        let end = (self.at + 200).min(self.packets.len());
        for (n, p) in self.packets[self.at..end].iter().enumerate() {
            // A packet that the file marks as not corrected keeps its mark.
            let t = (self.at + n) as f64 * 1e-4;
            let p: &[u8; 188] = p.as_slice().try_into().expect("188 bytes");
            self.out.push(io.items_out::<TsPacket>(0), p, t, false);
        }
        self.at = end;
    }
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
}

/// The media packets of the video of service 1573 (MPEG-2 video, 720x576, field pictures), as the layer of a stack
/// gives them with the setting `decode_video` at `decode`.
fn video_of_the_layer(packets: Vec<Vec<u8>>, decode: bool) -> Vec<sdrkit_core::Elementary> {
    use sdrkit_core::{Complex32, InputInfo, Output, Receiver, SettingSet, StreamKind, Value};
    use sdrkit_proto_mpegts::components::{TsDemuxComponent, TsDemuxSetting};
    use sdrkit_proto_mpegts::layer::{self, attach, TsLayer};
    use sdrkit_proto_mpegts::publish::SERVICE_ROW;
    let calls = packets.len().div_ceil(200) + 1;
    let source = FileTs {
        packets,
        at: 0,
        out: Default::default(),
    };
    let b = sdrkit_graph::GraphBuilder::new()
        .add("fec", source)
        .input("fec");
    let mut g = attach(b, "fec.out", &TsLayer::basic()).build().unwrap();
    Receiver::configure(&mut g, &InputInfo::new(1e6, 0.0, 0.0).recording()).unwrap();
    let id = TsDemuxSetting::schema().id("decode_video").unwrap();
    let demux = g.component_mut(layer::DEMUX).unwrap();
    demux.apply(id, &Value::Bool(decode)).unwrap();
    let mut out = Output::default();
    let mut video = Vec::new();
    let mut selected = false;
    for _ in 0..calls {
        out.clear();
        g.feed(&[Complex32::new(0.0, 0.0); 16], &mut out);
        let listed = g
            .component_as::<TsDemuxComponent>(layer::DEMUX)
            .is_some_and(|d| d.demux().si.service(1573).is_some());
        if listed && !selected {
            selected = true;
            video.clear();
            let c = g.component_mut(layer::DEMUX).unwrap();
            let id = c.commands().id("service.select").unwrap();
            let mut args = c.commands().def(id).unwrap().new_args();
            args.set(&SERVICE_ROW.field, "1573");
            let args = c.commands().check(id, &args).unwrap();
            c.invoke(id, &args).unwrap();
            continue;
        }
        let kinds = [
            StreamKind::RawVideo,
            StreamKind::Mpeg2Video,
            StreamKind::H264,
        ];
        video.extend(
            out.media
                .iter()
                .filter(|m| selected && kinds.contains(&m.kind))
                .cloned(),
        );
    }
    video
}

/// With `decode_video` on, the MPEG-2 video of a service leaves as frames of raw pixels with rising time stamps; while
/// the stream has lost pictures, the last good frame leaves again. With it off, the coded pictures leave.
#[test]
fn mpeg2_video_leaves_as_frames_when_the_setting_asks() {
    use sdrkit_core::media::{ColourMatrix, PixelFormat};
    use sdrkit_core::StreamKind;
    let Some(packets) = packets() else { return };
    let coded = video_of_the_layer(packets.clone(), false);
    assert!(coded.len() > 400, "{} coded pictures", coded.len());
    assert!(coded.iter().all(|m| m.kind == StreamKind::Mpeg2Video));
    assert!(coded
        .iter()
        .all(|m| m.format.codec == Codec::Mpeg2Video && m.format.pixels.is_none()));

    let frames = video_of_the_layer(packets, true);
    assert!(frames.len() > 200, "{} frames", frames.len());
    let mut held = 0;
    for (n, m) in frames.iter().enumerate() {
        assert_eq!(m.kind, StreamKind::RawVideo);
        assert_eq!(m.format.codec, Codec::Pixels(PixelFormat::I420));
        assert_eq!(m.format.timebase, (1, 90_000));
        let layout = m.format.pixels.as_ref().expect("a layout");
        assert_eq!((layout.width, layout.height), (720, 576));
        assert_eq!(layout.strides, [720, 360, 360]);
        assert_eq!(
            (layout.matrix, layout.full_range),
            (ColourMatrix::Bt601, false)
        );
        assert_eq!(m.packet.len, 720 * 576 * 3 / 2);
        let v = m.format.video.as_ref().expect("video properties");
        assert_eq!((v.width, v.height), (720, 576));
        assert_eq!((v.aspect, v.frame_rate), (Some((16, 9)), Some((25, 1))));
        assert!(v.interlaced && v.top_field_first, "frame {n}");
        if v.damaged {
            // The frame is the last good one again, not a copy of it.
            held += 1;
            let good = frames[..n]
                .iter()
                .rev()
                .find(|f| !f.format.video.as_ref().unwrap().damaged);
            let good = good.expect("a good frame before");
            assert!(std::sync::Arc::ptr_eq(&good.packet.data, &m.packet.data));
            assert_eq!(good.packet.start, m.packet.start);
        }
    }
    // Every frame is whole frame times after the one before, also over the pictures that the recording lost.
    let pts: Vec<i64> = frames
        .iter()
        .map(|m| m.packet.pts.expect("a time stamp"))
        .collect();
    for p in pts.windows(2) {
        let step = p[1] - p[0];
        assert!(step > 0 && step % 3600 == 0, "a step of {step}");
    }
    let lost = (pts[pts.len() - 1] - pts[0]) / 3600 + 1 - pts.len() as i64;
    // The last good frame leaves again only where the recording lost pictures: each run of such frames spans
    // time stamps that are missing, and ends at the next I picture, which comes every 24 frames.
    let marks: Vec<bool> = frames
        .iter()
        .map(|m| m.format.video.as_ref().unwrap().damaged)
        .collect();
    let mut runs = Vec::new();
    let mut n = 0;
    for run in marks.chunk_by(|a, b| a == b) {
        if run[0] {
            assert!(n > 0, "a good frame comes first");
            let span = (pts[n + run.len() - 1] - pts[n - 1]) / 3600;
            assert!(
                span > run.len() as i64,
                "no picture is lost in the frames {n}.."
            );
            assert!(run.len() <= 30, "{} frames", run.len());
            runs.push(run.len());
        }
        n += run.len();
    }
    eprintln!(
        "{} frames, {held} of them the last good one again in runs of {runs:?}, {lost} lost",
        frames.len()
    );
    assert!(!runs.is_empty() && !marks[marks.len() - 1]);
    assert!(lost <= 12, "{lost} frames lost");
    // The picture has content: a frame is not one flat value.
    let y = &frames[20].packet.bytes()[..720 * 576];
    assert!(y.iter().max().unwrap() - y.iter().min().unwrap() > 100);
}
