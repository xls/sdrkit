//! The senders of the frame stacks: a frame sent through a stack's transmit stream comes back from the receive side of a
//! new instance of the stack with the same fields; a frame is refused while transmit is off; `halt` ends a frame.

use num_complex::Complex32;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{owner, InputInfo, Output, Owner, Receiver, StackDescriptor, Value};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_ism::typed::IsmTxFields;
use std::sync::{Arc, Mutex};

/// The transmit stream: each write with its end mark.
#[derive(Default)]
struct Sink {
    writes: Mutex<Vec<(Vec<Complex32>, bool)>>,
}

impl HostStreams for Sink {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (HostPort::Transmit, HostData::Iq { samples, end, .. }) = (port, data) {
            self.writes.lock().unwrap().push((samples.to_vec(), end));
        }
    }
}

impl Sink {
    /// The bursts: their samples and whether the end mark came.
    fn bursts(&self) -> Vec<(Vec<Complex32>, bool)> {
        let mut out: Vec<(Vec<Complex32>, bool)> = Vec::new();
        let mut open = false;
        for (s, end) in self.writes.lock().unwrap().iter() {
            if !open {
                out.push((Vec::new(), false));
            }
            let b = out.last_mut().unwrap();
            b.0.extend_from_slice(s);
            b.1 = *end;
            open = !end;
        }
        out
    }
}

/// Where a test runs: the stack, its settings, the rate, the centre and the tuned frequency from the centre.
pub struct Station<'a> {
    pub stack: &'static StackDescriptor,
    pub set: &'a [(&'a str, Value)],
    pub rate: f64,
    pub hz: f64,
    pub tune: f64,
}

impl Station<'_> {
    /// A receiver of the stack, configured.
    fn receiver(&self) -> Box<dyn Receiver> {
        let cfg = crate::util::stack_with(self.stack, self.set);
        let mut rx = (self.stack.make_receiver)(&cfg).expect("the stack builds");
        rx.configure(&InputInfo::new(self.rate, self.hz, self.hz + self.tune).recording())
            .expect("the rate");
        rx
    }

    /// A receiver with transmit `on` and its transmit stream.
    fn transmitter(&self, on: bool) -> (Box<dyn Receiver>, Arc<Sink>) {
        let mut rx = self.receiver();
        let sink = Arc::new(Sink::default());
        let g = rx
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<Graph>())
            .expect("the stack is a graph");
        let mut ctx: RunContext = g.context().clone();
        ctx.streams = Some(sink.clone());
        g.set_context(ctx);
        assert!(rx.transmits());
        rx.set_transmit(on);
        (rx, sink)
    }

    /// Feeds `seconds` of silence in pieces of 10 ms.
    fn idle(&self, rx: &mut dyn Receiver, seconds: f64) {
        let zeros = vec![Complex32::default(); (self.rate / 100.0) as usize];
        let mut out = Output::default();
        for _ in 0..(seconds * 100.0).round() as usize {
            out.clear();
            rx.feed(&zeros, &mut out);
        }
    }

    /// The one burst that `send` with `args` makes; it ends with its end mark.
    pub fn send(&self, args: &[(&str, Value)]) -> Vec<Complex32> {
        let (mut rx, sink) = self.transmitter(true);
        crate::util::run(rx.as_mut(), "send", args).expect("the frame is sent");
        for _ in 0..1000 {
            self.idle(rx.as_mut(), 0.01);
            let f = IsmTxFields::read(Owner::results(rx.as_ref()).fields()).expect("the fields");
            if !f.tx_sending && f.tx_waiting == 0 {
                break;
            }
        }
        self.idle(rx.as_mut(), 0.05);
        let f = IsmTxFields::read(Owner::results(rx.as_ref()).fields()).expect("the fields");
        assert_eq!(f.tx_sent, 1, "one frame sent");
        let mut b = sink.bursts();
        assert_eq!(b.len(), 1, "one burst");
        assert!(b[0].1, "the end mark");
        let x = b.remove(0).0;
        let peak = x.iter().map(|z| z.norm()).fold(0.0f32, f32::max);
        assert!(peak > 0.3 && peak <= 0.5 + 1e-3, "the peak {peak} is the level");
        x
    }

    /// A new receiver of the stack fed `x` with `pad` seconds of silence before and after, and its events.
    pub fn receive(&self, x: &[Complex32], pad: f64) -> (Box<dyn Receiver>, Vec<sdrkit_core::Event>) {
        let mut rx = self.receiver();
        let silence = vec![Complex32::default(); (pad * self.rate) as usize];
        let mut y = silence.clone();
        y.extend_from_slice(x);
        y.extend_from_slice(&silence);
        let mut out = Output::default();
        let mut events = Vec::new();
        for c in y.chunks(16_384) {
            rx.feed(c, &mut out);
            events.append(&mut out.events);
            out.clear();
        }
        rx.finish(&mut out);
        events.append(&mut out.events);
        (rx, events)
    }

    /// The error of `send` with `args`, with transmit `on`.
    pub fn refusal(&self, on: bool, args: &[(&str, Value)]) -> String {
        let (mut rx, sink) = self.transmitter(on);
        let e = crate::util::run(rx.as_mut(), "send", args).expect_err("the frame is refused");
        self.idle(rx.as_mut(), 0.05);
        assert!(sink.bursts().is_empty(), "nothing is sent");
        e.to_string()
    }

    /// The burst that `send` with `args` starts when `halt` follows after `after_s`: it ends with its end mark.
    pub fn halted(&self, args: &[(&str, Value)], after_s: f64) -> Vec<Complex32> {
        let (mut rx, sink) = self.transmitter(true);
        crate::util::run(rx.as_mut(), "send", args).expect("the frame is sent");
        self.idle(rx.as_mut(), after_s);
        let id = Owner::commands(rx.as_ref()).id("halt").unwrap();
        let a = Owner::commands(rx.as_ref()).def(id).unwrap().new_args();
        owner::invoke(rx.as_mut(), id, &a).expect("halt runs");
        self.idle(rx.as_mut(), 0.2);
        let mut b = sink.bursts();
        assert_eq!(b.len(), 1, "one burst");
        assert!(b[0].1, "the end mark");
        b.remove(0).0
    }
}

/// The mean frequency of `x` at `rate`, Hz: the carrier of a balanced FSK frame.
pub fn mean_hz(x: &[Complex32], rate: f64) -> f64 {
    let sum: Complex32 = x.windows(2).map(|p| p[1] * p[0].conj()).sum();
    f64::from(sum.arg()) * rate / std::f64::consts::TAU
}

/// Text.
pub fn text(s: &str) -> Value {
    Value::Text(s.into())
}

mod zwave {
    use super::*;
    use sdrkit_stack_ism::typed::{ZwaveFrameRow, ZWAVE_FRAMES};
    use sdrkit_stack_ism::zwave::ZWAVE;

    const RATE: f64 = 2e6;
    const CENTRE: f64 = 869.13e6;

    fn eu() -> Value {
        Value::Text(crate::util::zwave_region("EU").channels_text())
    }

    /// (rate, home id, route, sequence, ack requested, MPDU, frequency) of each frame the stack reads.
    fn frames(s: &Station, x: &[Complex32]) -> Vec<(String, String, String, i64, bool, String, f64)> {
        let (rx, _) = s.receive(x, 0.05);
        ZWAVE_FRAMES
            .rows::<ZwaveFrameRow>(Owner::results(rx.as_ref()))
            .map(|(_, r)| {
                (
                    r.rate.to_owned(),
                    r.home_id.to_owned(),
                    r.route.to_owned(),
                    r.seq,
                    r.ack_requested,
                    r.mpdu.to_owned(),
                    r.frequency.0,
                )
            })
            .collect()
    }

    #[test]
    fn a_frame_at_each_rate_comes_back_on_its_channel() {
        let set = [("channels", eu())];
        let s = Station {
            stack: &ZWAVE,
            set: &set,
            rate: RATE,
            hz: CENTRE,
            tune: 0.0,
        };
        for (k, want_hz) in [(0u16, 868.42e6), (1, 868.40e6), (2, 869.85e6)] {
            let x = s.send(&[
                ("home_id", text("C0FFEE01")),
                ("source", Value::I64(2)),
                ("destination", Value::I64(1)),
                ("payload", text("25 01 FF")),
                ("rate", Value::Choice(k)),
                ("ack", Value::Bool(true)),
                ("sequence", Value::I64(5)),
            ]);
            let r = k as u8 + 1;
            let mpdu = sdrkit_stack_ism::zwave::frame::build(
                0xC0FFEE01,
                2,
                0x41,
                5,
                &[1],
                &[0x25, 0x01, 0xFF],
                r,
            );
            let got = frames(&s, &x);
            assert_eq!(got.len(), 1, "R{r}: {got:?}");
            let g = &got[0];
            assert_eq!(
                (g.0.as_str(), g.1.as_str(), g.2.as_str(), g.3, g.4),
                (format!("R{r}").as_str(), "C0FFEE01", "2 > 1", 5, true),
                "R{r}"
            );
            assert_eq!(g.5, sdrkit_stack_ism::family::hex(&mpdu), "R{r}");
            // Channels 60 kHz apart or closer share one front end and one frequency in the log.
            assert!((g.6 - want_hz).abs() < 30e3, "R{r} at {} Hz", g.6);
            let f = mean_hz(&x, RATE);
            assert!((f - (want_hz - CENTRE)).abs() < 3e3, "R{r} sent at {f} Hz from the centre");
        }
    }

    #[test]
    fn a_frame_is_refused_while_transmit_is_off_and_for_a_bad_field() {
        let s = Station {
            stack: &ZWAVE,
            set: &[],
            rate: RATE,
            hz: CENTRE,
            tune: 0.0,
        };
        let args = |home: &str, payload: &str| {
            vec![
                ("home_id", text(home)),
                ("source", Value::I64(2)),
                ("destination", Value::I64(1)),
                ("payload", text(payload)),
            ]
        };
        assert!(s.refusal(false, &args("C0FFEE01", "2501FF")).contains("off"));
        assert!(s.refusal(true, &args("C0FFEE", "2501FF")).contains("home id"));
        assert!(s.refusal(true, &args("C0FFEE01", "25 0")).contains("hex"));
        assert!(s.refusal(true, &args("C0FFEE01", "")).contains("octets"));
    }

    #[test]
    fn halt_ends_the_frame_with_its_end_mark() {
        let s = Station {
            stack: &ZWAVE,
            set: &[],
            rate: RATE,
            hz: CENTRE,
            tune: 0.0,
        };
        let args = [
            ("home_id", text("C0FFEE01")),
            ("source", Value::I64(2)),
            ("destination", Value::I64(1)),
            ("payload", text(&"00".repeat(54))),
            ("rate", Value::Choice(0)),
        ];
        let whole = s.send(&args);
        let cut = s.halted(&args, 0.02);
        assert!(cut.len() < whole.len() / 2, "{} of {}", cut.len(), whole.len());
    }
}

mod zigbee {
    use super::*;
    use sdrkit_stack_ism::typed::{ZigbeeFrameRow, ZIGBEE_FRAMES};
    use sdrkit_stack_ism::zigbee::ZIGBEE;

    const RATE: f64 = 4e6;
    const HZ: f64 = 2405e6;

    fn station() -> Station<'static> {
        Station {
            stack: &ZIGBEE,
            set: &[],
            rate: RATE,
            hz: HZ,
            tune: 0.0,
        }
    }

    /// (type, route, PAN, length, details) of each frame the stack reads.
    fn frames(x: &[Complex32]) -> Vec<(String, String, Option<String>, i64, String)> {
        let (rx, _) = station().receive(x, 0.002);
        ZIGBEE_FRAMES
            .rows::<ZigbeeFrameRow>(Owner::results(rx.as_ref()))
            .map(|(_, r)| {
                (
                    r.r#type.to_owned(),
                    r.route.to_owned(),
                    r.pan.map(str::to_owned),
                    r.len,
                    r.details.to_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn data_command_and_beacon_frames_come_back() {
        let s = station();
        // A Zigbee NWK data frame with an APS header and a ZCL On command.
        let nwk = "0800 0000 0100 1E 01 4000 0600 0401 01 00 01 01";
        let cases = [
            ("Data", 1u16, "0000", "0001", nwk, "0x0001 > 0x0000", 9 + 18 + 2, "APS cluster 0x0006 On/Off"),
            ("Data", 1, "FFFF", "001788010A0B0C0D", "", "00:17:88:01:0A:0B:0C:0D > broadcast", 9 + 6 + 2, "0 bytes"),
            ("MAC command", 2, "", "1234", "07", "0x1234 > -", 7 + 1 + 2, "Beacon request"),
            ("Beacon", 0, "", "0000", "FF CF 00 00 22 84 04 03 02 01 00 4B 12 00 FF FF FF 03", "0x0000 > -", 7 + 18 + 2, "join allowed"),
        ];
        for (ty, k, dst, src, payload, route, len, details) in cases {
            let x = s.send(&[
                ("pan_id", text("1A62")),
                ("destination", text(dst)),
                ("source", text(src)),
                ("payload", text(payload)),
                ("frame_type", Value::Choice(k)),
                ("sequence", Value::I64(0x41)),
            ]);
            let got = frames(&x);
            assert_eq!(got.len(), 1, "{ty}: {got:?}");
            let g = &got[0];
            assert_eq!(g.1, route, "{ty}");
            assert_eq!(g.2.as_deref(), Some("0x1A62"), "{ty}");
            assert_eq!(g.0, ty);
            assert_eq!(g.3, len as i64, "{ty}");
            assert!(g.4.contains(details), "{ty}: {}", g.4);
        }
    }

    #[test]
    fn a_frame_is_refused_while_transmit_is_off_and_for_a_bad_field() {
        let s = station();
        let args = |pan: &str, dst: &str| {
            vec![
                ("pan_id", text(pan)),
                ("destination", text(dst)),
                ("source", text("0001")),
                ("payload", text("00")),
            ]
        };
        assert!(s.refusal(false, &args("1A62", "0000")).contains("off"));
        assert!(s.refusal(true, &args("1A6", "0000")).contains("PAN id"));
        assert!(s.refusal(true, &args("1A62", "000")).contains("destination"));
    }
}

mod wmbus {
    use super::*;
    use sdrkit_stack_ism::typed::{WmbusFrameRow, WmbusMeterRow, WMBUS_FRAMES, WMBUS_METERS};
    use sdrkit_stack_ism::wmbus::WMBUS;

    const RATE: f64 = 2e6;

    /// Mode, frequency and frame bytes.
    type FrameLine = (String, f64, String);
    /// Maker, id and device type.
    type MeterLine = (String, String, String);

    /// The frames and the meters that the stack reads.
    fn heard(s: &Station, x: &[Complex32]) -> (Vec<FrameLine>, Vec<MeterLine>) {
        let (rx, _) = s.receive(x, 0.05);
        let r = Owner::results(rx.as_ref());
        let frames = WMBUS_FRAMES
            .rows::<WmbusFrameRow>(r)
            .map(|(_, f)| (f.phy.to_owned(), f.frequency.0, f.frame.to_owned()))
            .collect();
        let meters = WMBUS_METERS
            .rows::<WmbusMeterRow>(r)
            .map(|(_, m)| (m.maker.to_owned(), m.meter_id.to_owned(), m.device_type.to_owned()))
            .collect();
        (frames, meters)
    }

    #[test]
    fn a_frame_in_each_mode_comes_back_on_its_channel() {
        // A short header (access number, status, configuration) and a volume of 123456 litres.
        let payload = "7A 2A 00 00 00 0C 13 56 34 12 00";
        for (k, mode, centre, hz, chip_rate) in [
            (0u16, "S1", 868.625e6, 868.3e6, 32768.0),
            (1, "T1", 868.625e6, 868.95e6, 1e5),
            (2, "C1 (A)", 868.625e6, 868.95e6, 1e5),
            (3, "C1 (B)", 868.625e6, 868.95e6, 1e5),
            (4, "R2", 868.1e6, 868.09e6, 4800.0),
            (5, "N1", 169.44e6, 169.44375e6, 4800.0),
        ] {
            let s = Station {
                stack: &WMBUS,
                set: &[],
                rate: RATE,
                hz: centre,
                tune: 0.0,
            };
            let x = s.send(&[
                ("mode", Value::Choice(k)),
                ("manufacturer", text("KAM")),
                ("address", text("12345678")),
                ("version", Value::I64(0x1B)),
                ("device_type", Value::I64(7)),
                ("payload", text(payload)),
            ]);
            let (frames, meters) = heard(&s, &x);
            assert_eq!(frames.len(), 1, "{mode}: {frames:?}");
            let (phy, f_hz, frame) = &frames[0];
            assert_eq!(phy, mode);
            assert!((f_hz - hz).abs() < 1.0, "{mode} on {f_hz} Hz");
            assert!(frame.ends_with("2D2C785634121B077A2A0000000C1356341200"), "{mode}: {frame}");
            assert_eq!(meters.len(), 1, "{mode}");
            assert_eq!(meters[0].0, "KAM");
            assert_eq!(meters[0].1, "12345678");
            assert_eq!(meters[0].2, "water");
            // The 40 chips of the preamble alternate: their mean frequency is the carrier.
            let f = mean_hz(&x[..(36.0 * RATE / chip_rate) as usize], RATE);
            assert!((f - (hz - centre)).abs() < 3e3, "{mode} sent at {f} Hz from the centre");
        }
    }

    #[test]
    fn a_frame_is_refused_while_transmit_is_off_and_for_a_bad_field() {
        let s = Station {
            stack: &WMBUS,
            set: &[],
            rate: RATE,
            hz: 868.625e6,
            tune: 0.0,
        };
        let args = |maker: &str, n1: bool| {
            vec![
                ("mode", Value::Choice(if n1 { 5 } else { 1 })),
                ("manufacturer", text(maker)),
                ("address", text("12345678")),
                ("version", Value::I64(1)),
                ("device_type", Value::I64(7)),
                ("payload", text("7A000000")),
            ]
        };
        assert!(s.refusal(false, &args("KAM", false)).contains("off"));
        assert!(s.refusal(true, &args("KA", false)).contains("manufacturer"));
        assert!(s.refusal(true, &args("KAM", true)).contains("no channel"));
    }
}

mod ant {
    use super::*;
    use sdrkit_stack_ism::ant::ANT;
    use sdrkit_stack_ism::typed::{AntFrameRow, ANT_FRAMES};

    fn station() -> Station<'static> {
        Station {
            stack: &ANT,
            set: &[],
            rate: 4e6,
            hz: 2457e6,
            tune: 0.0,
        }
    }

    #[test]
    fn a_heart_rate_frame_comes_back() {
        a_heart_rate_frame_comes_back_at(station());
    }

    #[test]
    fn a_frame_comes_back_when_the_radio_is_tuned_off_its_centre() {
        let s = Station {
            tune: 700e3,
            ..station()
        };
        a_heart_rate_frame_comes_back_at(s);
    }

    fn a_heart_rate_frame_comes_back_at(s: Station) {
        // Heart rate page 0: beat time 0x1234, beat count 7, 72 beats a minute.
        let x = s.send(&[
            ("device", Value::I64(0x6941)),
            ("device_type", Value::I64(120)),
            ("tx_type", Value::I64(1)),
            ("payload", text("00 FF FF FF 34 12 07 48")),
        ]);
        let (rx, _) = s.receive(&x, 0.002);
        let got: Vec<(i64, i64, String, String)> = ANT_FRAMES
            .rows::<AntFrameRow>(Owner::results(rx.as_ref()))
            .map(|(_, r)| (r.device, r.tx_type, r.marker.to_owned(), r.payload.to_owned()))
            .collect();
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].0, 0x6941);
        assert_eq!(got[0].1, 1);
        assert_eq!(got[0].2, "A6C5");
        assert_eq!(got[0].3.replace(' ', ""), "00FFFFFF34120748");
    }

    #[test]
    fn a_frame_is_refused_while_transmit_is_off_and_for_a_bad_field() {
        let s = station();
        let args = |payload: &str| {
            vec![
                ("device", Value::I64(1)),
                ("device_type", Value::I64(120)),
                ("tx_type", Value::I64(1)),
                ("payload", text(payload)),
            ]
        };
        assert!(s.refusal(false, &args("0000000000000000")).contains("off"));
        assert!(s.refusal(true, &args("00000000000000")).contains("8 octets"));
    }
}

mod wisun {
    use super::*;
    use sdrkit_stack_ism::typed::{WisunFrameRow, WISUN_FRAMES};
    use sdrkit_stack_ism::wisun::WISUN;

    fn station() -> Station<'static> {
        Station {
            stack: &WISUN,
            set: &[],
            rate: 2e6,
            hz: 920e6,
            tune: 0.0,
        }
    }

    /// (frequency, bit rate, type, route, details, MPDU) of each frame the stack reads.
    fn frames(x: &[Complex32]) -> Vec<(f64, i64, String, String, String, String)> {
        let (rx, _) = station().receive(x, 0.01);
        WISUN_FRAMES
            .rows::<WisunFrameRow>(Owner::results(rx.as_ref()))
            .map(|(_, r)| {
                (
                    r.freq.0,
                    r.kbps,
                    r.r#type.to_owned(),
                    r.route.to_owned(),
                    r.details.to_owned(),
                    r.mpdu.to_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn a_pan_advert_and_a_unicast_frame_come_back() {
        let s = station();
        let x = s.send(&[
            ("frame_type", Value::Choice(0)),
            ("pan_id", text("7A31")),
            ("source", text("00124BFFFE1A2B3C")),
            ("destination", text("")),
            // A WP-IE with the PAN-IE and the network name.
            ("payload", text("11A0 0504 0300 0000 42 0805 546573742D46414E")),
        ]);
        let got = frames(&x);
        assert_eq!(got.len(), 1, "{got:?}");
        let g = &got[0];
        assert_eq!((g.0, g.1, g.2.as_str()), (920e6, 50, "PAN Advert"));
        assert_eq!(g.3, "00:12:4B:FF:FE:1A:2B:3C > broadcast");
        assert!(g.4.contains("\"Test-FAN\", PAN size 3, routing cost 0"), "{}", g.4);
        assert!(g.5.ends_with("00000000003F11A0050403000000420805546573742D46414E"), "{}", g.5);
        let x = s.send(&[
            ("frame_type", Value::Choice(6)),
            ("pan_id", text("7A31")),
            ("source", text("02A1B2FFFEC3D4E5")),
            ("destination", text("000D6FFFFE445566")),
            ("payload", text("")),
            ("sequence", Value::I64(9)),
            ("kbps", Value::I64(100)),
            ("ufsi", Value::I64(0xABCDEF)),
        ]);
        let got = frames(&x);
        assert_eq!(got.len(), 1, "{got:?}");
        let g = &got[0];
        assert_eq!((g.1, g.2.as_str()), (100, "EAPOL"));
        assert_eq!(g.3, "02:A1:B2:FF:FE:C3:D4:E5 > 00:0D:6F:FF:FE:44:55:66");
        // Sequence 9, the destination, the source, the UTT-IE of type 6 and the header termination 2.
        assert!(g.5.starts_with("41EE09665544FEFF6F0D00E5D4C3FEFFB2A102"), "{}", g.5);
        assert!(g.5.ends_with("06EFCDAB803F"), "{}", g.5);
    }

    #[test]
    fn a_frame_is_refused_while_transmit_is_off_and_for_a_bad_field() {
        let s = station();
        let args = |src: &str, kbps: i64| {
            vec![
                ("pan_id", text("7A31")),
                ("source", text(src)),
                ("destination", text("")),
                ("payload", text("")),
                ("kbps", Value::I64(kbps)),
            ]
        };
        assert!(s.refusal(false, &args("00124BFFFE1A2B3C", 0)).contains("off"));
        assert!(s.refusal(true, &args("00124B", 0)).contains("source"));
        assert!(s.refusal(true, &args("00124BFFFE1A2B3C", 75)).contains("no mode"));
    }
}

mod ism {
    use super::*;
    use sdrkit_stack_ism::ism::ISM;
    use sdrkit_stack_ism::typed::{IsmDeviceRow, ISM_DEVICES};

    fn station() -> Station<'static> {
        Station {
            stack: &ISM,
            set: &[],
            rate: 1e6,
            hz: 433.92e6,
            tune: 0.0,
        }
    }

    /// Model, id, channel, battery good and readings.
    type DeviceLine = (String, String, Option<String>, Option<bool>, String);

    /// The devices that the stack reads.
    fn devices(x: &[Complex32]) -> Vec<DeviceLine> {
        let (rx, _) = station().receive(x, 0.05);
        ISM_DEVICES
            .rows::<IsmDeviceRow>(Owner::results(rx.as_ref()))
            .map(|(_, r)| {
                (
                    r.model.to_owned(),
                    r.id.to_owned(),
                    r.channel.map(str::to_owned),
                    r.battery_ok,
                    r.readings.to_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn a_remote_and_a_thermometer_come_back() {
        let s = station();
        let x = s.send(&[
            ("family", Value::Choice(0)),
            ("id", Value::I64(0x1234)),
            ("command", Value::I64(0x42)),
        ]);
        let got = devices(&x);
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!((got[0].0.as_str(), got[0].1.as_str()), ("Generic-Remote", "4660"));
        assert!(got[0].4.contains("66"), "{}", got[0].4);
        let x = s.send(&[
            ("family", Value::Choice(1)),
            ("id", Value::I64(0x5B)),
            ("channel", Value::I64(2)),
            ("battery_ok", Value::Bool(false)),
            ("temperature", Value::F64(-3.4)),
            ("humidity", Value::I64(71)),
        ]);
        let got = devices(&x);
        assert_eq!(got.len(), 1, "{got:?}");
        let g = &got[0];
        assert_eq!((g.0.as_str(), g.1.as_str()), ("LaCrosse-TX141THBv2", "5b"));
        assert_eq!(g.2.as_deref(), Some("2"));
        assert_eq!(g.3, Some(false));
        assert!(g.4.contains("-3.4") && g.4.contains("71"), "{}", g.4);
    }

    #[test]
    fn a_frame_is_refused_while_transmit_is_off_and_for_a_bad_field() {
        let s = station();
        assert!(s.refusal(false, &[("id", Value::I64(1))]).contains("off"));
        assert!(s.refusal(true, &[("id", Value::I64(0))]).contains("id"));
        let tx = [("family", Value::Choice(1)), ("id", Value::I64(300))];
        assert!(s.refusal(true, &tx).contains("id"));
    }
}
