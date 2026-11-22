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

    const KEY: &str = "C0C1C2C3C4C5C6C7C8C9CACBCCCDCECF";
    const OTHER: &str = "000102030405060708090A0B0C0D0E0F";

    /// (length, details) of each frame that a stack with the setting `keys` reads.
    fn keyed(keys: &str, x: &[Complex32]) -> Vec<(i64, String, bool)> {
        let set = [("keys", text(keys))];
        let st = Station { set: &set, ..station() };
        let (rx, _) = st.receive(x, 0.002);
        ZIGBEE_FRAMES
            .rows::<ZigbeeFrameRow>(Owner::results(rx.as_ref()))
            .map(|(_, r)| (r.len, r.details.to_owned(), r.encrypted))
            .collect()
    }

    #[test]
    fn secured_frames_open_with_their_key() {
        let s = station();
        let nwk = "0800 0000 0100 1E 01 4000 0600 0401 01 00 01 01";
        let beacon = "FF CF 00 00 22 84 04 03 02 01 00 4B 12 00 FF FF FF 03";
        // (frame type, destination, payload, level, length, details, encrypted on air)
        let cases = [
            (1u16, "0000", nwk, 5i64, 15 + 6 + 18 + 4 + 2, "APS cluster 0x0006 On/Off", true),
            (1, "0000", nwk, 4, 15 + 6 + 18 + 2, "APS cluster 0x0006 On/Off", true),
            (1, "0000", nwk, 2, 15 + 6 + 18 + 8 + 2, "APS cluster 0x0006 On/Off", false),
            (1, "0000", nwk, 7, 15 + 6 + 18 + 16 + 2, "APS cluster 0x0006 On/Off", true),
            (2, "", "07", 6, 13 + 6 + 1 + 8 + 2, "Beacon request", true),
            (0, "", beacon, 5, 13 + 6 + 18 + 4 + 2, "join allowed", true),
        ];
        for (k, dst, payload, level, len, details, enc) in cases {
            let x = s.send(&[
                ("pan_id", text("1A62")),
                ("destination", text(dst)),
                ("source", text("001788010A0B0C0D")),
                ("payload", text(payload)),
                ("frame_type", Value::Choice(k)),
                ("security_level", Value::I64(level)),
                ("key", text(KEY)),
                ("frame_counter", Value::I64(0x0102_0304)),
            ]);
            // Level 4 has no MIC: the first key opens it, so it gets the one key.
            let (keys, n) = if level == 4 { (KEY.to_string(), 1) } else { (format!("{OTHER}; {KEY}"), 2) };
            let how = match level {
                4 => "decrypted, no MIC to check (key 1)".to_string(),
                l if l > 4 => format!("decrypted (key {n})"),
                _ => format!("MIC good (key {n})"),
            };
            let got = keyed(&keys, &x);
            assert_eq!(got.len(), 1, "level {level}: {got:?}");
            assert_eq!(got[0].0, len as i64, "level {level}");
            assert!(got[0].1.contains(details) && got[0].1.contains(&how), "level {level}: {}", got[0].1);
            assert_eq!(got[0].2, enc, "level {level}");
            let shut = keyed("", &x);
            if enc {
                assert!(shut[0].1.contains("encrypted"), "level {level}: {}", shut[0].1);
                assert!(!shut[0].1.contains(details), "level {level}: {}", shut[0].1);
            }
            let wrong = keyed(OTHER, &x);
            if level != 4 {
                assert!(wrong[0].1.contains("no key fits"), "level {level}: {}", wrong[0].1);
            } else {
                assert!(!wrong[0].1.contains(details), "level 4, another key: {}", wrong[0].1);
            }
        }
        // Key identifier mode 3 with a key source, a short destination broadcast.
        let x = s.send(&[
            ("pan_id", text("1A62")),
            ("destination", text("FFFF")),
            ("source", text("001788010A0B0C0D")),
            ("payload", text(nwk)),
            ("security_level", Value::I64(6)),
            ("key", text(KEY)),
            ("key_id_mode", Value::I64(3)),
            ("key_source", text("0011223344556677")),
            ("key_index", Value::I64(9)),
        ]);
        let got = keyed(KEY, &x);
        assert_eq!(got[0].0, 15 + 14 + 18 + 8 + 2);
        assert!(got[0].1.contains("decrypted (key 1)"), "{}", got[0].1);
    }

    #[test]
    fn a_frame_is_refused_while_transmit_is_off_and_for_a_bad_field() {
        let s = station();
        let secured = |src: &str, key: &str| {
            vec![
                ("pan_id", text("1A62")),
                ("destination", text("0000")),
                ("source", text(src)),
                ("payload", text("00")),
                ("security_level", Value::I64(5)),
                ("key", text(key)),
            ]
        };
        assert!(s.refusal(true, &secured("0001", KEY)).contains("extended source"));
        assert!(s.refusal(true, &secured("001788010A0B0C0D", "00")).contains("key"));
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
    use sdrkit_core::Choice;
    use sdrkit_stack_ism::ism::send::IsmFamily;
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

    /// The argument `family` of `f`.
    fn fam(f: IsmFamily) -> (&'static str, Value) {
        ("family", Value::Choice(f.index()))
    }

    /// One family: the arguments of `send` and what comes back (model, id, channel, battery good and parts of the
    /// readings).
    struct Case {
        args: Vec<(&'static str, Value)>,
        model: &'static str,
        id: &'static str,
        channel: Option<&'static str>,
        battery_ok: Option<bool>,
        has: &'static [&'static str],
    }

    fn cases() -> Vec<Case> {
        use IsmFamily as F;
        let num = |k: &'static str, v: f64| (k, Value::F64(v));
        let int = |k: &'static str, v: i64| (k, Value::I64(v));
        let hex = |v: &str| ("payload", text(v));
        let batt = |b: bool| ("battery_ok", Value::Bool(b));
        vec![
            Case {
                args: vec![fam(F::Remote), int("id", 0x1234), int("command", 0x42)],
                model: "Generic-Remote",
                id: "4660",
                channel: None,
                battery_ok: None,
                has: &["Command 66"],
            },
            Case {
                args: vec![
                    fam(F::Tx141thBv2),
                    int("id", 0x5B),
                    int("channel", 2),
                    batt(false),
                    num("temperature", -3.4),
                    int("humidity", 71),
                ],
                model: "LaCrosse-TX141THBv2",
                id: "5b",
                channel: Some("2"),
                battery_ok: Some(false),
                has: &["Temperature -3.4", "Humidity 71"],
            },
            Case {
                args: vec![
                    fam(F::Nexus),
                    int("id", 91),
                    int("channel", 2),
                    batt(false),
                    num("temperature", -3.4),
                    int("humidity", 71),
                ],
                model: "Nexus-TH",
                id: "91",
                channel: Some("2"),
                battery_ok: Some(false),
                has: &["Temperature -3.4", "Humidity 71"],
            },
            Case {
                args: vec![fam(F::Nexus), int("id", 17), int("channel", 3), num("temperature", 25.0), int("humidity", 0)],
                model: "Nexus-T",
                id: "17",
                channel: Some("3"),
                battery_ok: Some(true),
                has: &["Temperature 25.0"],
            },
            Case {
                args: vec![
                    fam(F::AcuriteTower),
                    int("id", 2748),
                    int("channel", 2),
                    num("temperature", 12.3),
                    int("humidity", 44),
                ],
                model: "Acurite-Tower",
                id: "2748",
                channel: Some("B"),
                battery_ok: Some(true),
                has: &["Temperature 12.3", "Humidity 44"],
            },
            Case {
                args: vec![
                    fam(F::Wh1080),
                    int("id", 69),
                    num("temperature", 7.5),
                    int("humidity", 63),
                    num("wind_speed", 3.4),
                    num("wind_gust", 6.8),
                    num("rain", 30.0),
                    num("wind_dir", 225.0),
                ],
                model: "Fineoffset-WHx080",
                id: "69",
                channel: None,
                battery_ok: Some(true),
                has: &["Temperature 7.5", "Humidity 63", "Wind direction 225", "Wind speed 3.40", "Wind gust 6.80", "Rain 30.0"],
            },
            Case {
                args: vec![
                    fam(F::F007th),
                    int("id", 200),
                    int("channel", 5),
                    batt(false),
                    num("temperature", 21.5),
                    int("humidity", 33),
                ],
                model: "Ambientweather-F007TH",
                id: "200",
                channel: Some("5"),
                battery_ok: Some(false),
                has: &["Temperature 21.5", "Humidity 33"],
            },
            Case {
                args: vec![
                    fam(F::OregonThgr810),
                    int("id", 119),
                    int("channel", 2),
                    num("temperature", -7.3),
                    int("humidity", 41),
                ],
                model: "Oregon-THGR810",
                id: "119",
                channel: Some("2"),
                battery_ok: Some(true),
                has: &["Temperature -7.30", "Humidity 41"],
            },
            Case {
                args: vec![
                    fam(F::OregonThgr228n),
                    int("id", 44),
                    int("channel", 3),
                    batt(false),
                    num("temperature", 23.8),
                    int("humidity", 56),
                ],
                model: "Oregon-THGR228N",
                id: "44",
                channel: Some("3"),
                battery_ok: Some(false),
                has: &["Temperature 23.80", "Humidity 56"],
            },
            Case {
                args: vec![
                    fam(F::Schrader),
                    int("id", 0x0AB_CDEF),
                    int("flags", 0x1C),
                    num("pressure", 250.0),
                    num("temperature", 18.0),
                ],
                model: "Schrader",
                id: "0ABCDEF",
                channel: None,
                battery_ok: None,
                has: &["Pressure 250.0", "Temperature 18", "Flags 28"],
            },
            Case {
                args: vec![
                    fam(F::Toyota),
                    int("id", 0x1122_3344),
                    int("flags", 0x05),
                    num("pressure", 241.3),
                    num("temperature", 21.0),
                ],
                model: "Toyota",
                id: "11223344",
                channel: None,
                battery_ok: None,
                has: &["Pressure 241.3", "Temperature 21", "Status 5"],
            },
            Case {
                args: vec![
                    fam(F::Renault),
                    int("id", 0xABCD),
                    int("flags", 0x12),
                    num("pressure", 210.0),
                    num("temperature", 15.0),
                ],
                model: "Renault",
                id: "00abcd",
                channel: None,
                battery_ok: None,
                has: &["Pressure 210.00", "Temperature 15", "Flags 18"],
            },
            Case {
                args: vec![fam(F::Ford), int("id", 0x0BAD_CAFE), num("pressure", 220.6), num("temperature", 9.0)],
                model: "Ford",
                id: "0badcafe",
                channel: None,
                battery_ok: None,
                has: &["Pressure 220.6", "Temperature 9"],
            },
            Case {
                args: vec![fam(F::Efergy), int("id", 4321), num("current", 7.5), int("interval", 18)],
                model: "Efergy-e2CT",
                id: "4321",
                channel: None,
                battery_ok: Some(true),
                has: &["Current 7.50", "Interval 18"],
            },
            Case {
                args: vec![fam(F::CurrentCost), int("id", 1000), int("power", 2345)],
                model: "CurrentCost-TX",
                id: "1000",
                channel: None,
                battery_ok: None,
                has: &["Power 0 2345"],
            },
            Case {
                args: vec![
                    fam(F::Bresser6),
                    int("id", 0x0A0B_0C0D),
                    int("channel", 3),
                    num("temperature", -5.3),
                    int("humidity", 48),
                    num("wind_gust", 7.2),
                    num("wind_speed", 4.6),
                    num("wind_dir", 300.0),
                    num("uv", 5.1),
                ],
                model: "Bresser-6in1",
                id: "0a0b0c0d",
                channel: Some("3"),
                battery_ok: Some(true),
                has: &["Temperature -5.3", "Humidity 48", "Wind gust 7.2", "Wind speed 4.6", "Wind direction 300", "UV index 5.1"],
            },
            Case {
                args: vec![
                    fam(F::Bresser5),
                    int("id", 51),
                    num("temperature", -12.4),
                    int("humidity", 77),
                    num("wind_gust", 9.9),
                    num("wind_speed", 6.1),
                    num("wind_dir", 135.0),
                    num("rain", 45.6),
                ],
                model: "Bresser-5in1",
                id: "51",
                channel: None,
                battery_ok: Some(true),
                has: &["Temperature -12.4", "Humidity 77", "Wind gust 9.9", "Wind speed 6.1", "Wind direction 135.0", "Rain 45.6"],
            },
            Case {
                args: vec![fam(F::Tx29), int("id", 21), num("temperature", 4.2), int("humidity", 58)],
                model: "LaCrosse-TX29IT",
                id: "21",
                channel: None,
                battery_ok: Some(true),
                has: &["Temperature 4.2", "Humidity 58"],
            },
            Case {
                args: vec![fam(F::Tx35), int("id", 9), batt(false), num("temperature", -8.1), int("humidity", 0)],
                model: "LaCrosse-TX35DTHIT",
                id: "9",
                channel: None,
                battery_ok: Some(false),
                has: &["Temperature -8.1"],
            },
            Case {
                args: vec![
                    fam(F::Wh24),
                    int("id", 77),
                    num("temperature", 15.2),
                    int("humidity", 66),
                    num("wind_dir", 123.0),
                    num("wind_speed", 2.8),
                    num("wind_gust", 3.36),
                    num("rain", 6.0),
                    num("uv", 3.0),
                    num("light", 1234.5),
                ],
                model: "Fineoffset-WH24",
                id: "77",
                channel: None,
                battery_ok: Some(true),
                has: &[
                    "Temperature 15.2",
                    "Humidity 66",
                    "Wind direction 123",
                    "Wind speed 2.8",
                    "Wind gust 3.4",
                    "Rain 6.0",
                    "UV index 3",
                    "Light 1234.5",
                ],
            },
            Case {
                args: vec![
                    fam(F::Prologue),
                    int("id", 58),
                    int("channel", 2),
                    num("temperature", 19.6),
                    int("humidity", 61),
                ],
                model: "Prologue-TH",
                id: "58",
                channel: Some("2"),
                battery_ok: None,
                has: &["Temperature 19.6", "Humidity 61"],
            },
            Case {
                args: vec![fam(F::AcuriteTxr), int("id", 0), hex("C1 23 78 00 08 60 2D")],
                model: "Acurite-5n1",
                id: "291",
                channel: Some("A"),
                battery_ok: Some(true),
                has: &["Temperature 22.2", "Humidity 45", "Wind speed 0.0"],
            },
            Case {
                args: vec![fam(F::Bresser3ch), int("id", 0), hex("5C 26 2C 34")],
                model: "Bresser-3CH",
                id: "92",
                channel: Some("2"),
                battery_ok: Some(true),
                has: &["Temperature 20.0", "Humidity 52"],
            },
            Case {
                args: vec![fam(F::Citroen), int("id", 0), hex("20 01 02 03 04 12 A0 46 55")],
                model: "Citroen",
                id: "01020304",
                channel: None,
                battery_ok: None,
                has: &["Pressure 218", "Temperature 20", "Repeat 2", "Flags 1"],
            },
            Case {
                args: vec![fam(F::SchraderEg53), int("id", 0), hex("15 55 55 55 55 01 02 03 04 0A BC DE 64 4D")],
                model: "Schrader-EG53MA4",
                id: "0ABCDE",
                channel: None,
                battery_ok: None,
                has: &["Pressure 250.0", "Temperature 25.0"],
            },
            Case {
                args: vec![
                    fam(F::Bresser7),
                    int("id", 0),
                    hex("12 34 27 00 BB 05 50 32 00 01 23 00 21 70 55 01 23 45 04 50 00 00 00"),
                ],
                model: "Bresser-7in1",
                id: "4660",
                channel: Some("1"),
                battery_ok: Some(true),
                has: &["Temperature 21.7", "Humidity 55", "Wind direction 270", "Wind gust 5.5", "Wind speed 3.2", "Rain 12.3", "Light 12345", "UV index 4.5"],
            },
            Case {
                args: vec![fam(F::Rubicson), int("id", 91), int("channel", 3), batt(false), num("temperature", -3.4)],
                model: "Rubicson-Temperature",
                id: "91",
                channel: Some("3"),
                battery_ok: Some(false),
                has: &["Temperature -3.4"],
            },
            Case {
                args: vec![fam(F::Solight), int("id", 17), int("channel", 2), num("temperature", 25.0)],
                model: "Solight-TE44",
                id: "17",
                channel: Some("2"),
                battery_ok: Some(true),
                has: &["Temperature 25.00"],
            },
            Case {
                args: vec![fam(F::GenericTemp), int("id", 33), int("flags", 1), num("temperature", 12.5)],
                model: "Generic-Temperature",
                id: "33",
                channel: None,
                battery_ok: None,
                has: &["Temperature 12.50", "Battery bits 1"],
            },
            Case {
                args: vec![fam(F::Tx141b), int("id", 0x5B), int("channel", 0), num("temperature", 18.2)],
                model: "LaCrosse-TX141B",
                id: "5b",
                channel: None,
                battery_ok: Some(true),
                has: &["Temperature 18.2"],
            },
            Case {
                args: vec![fam(F::Tx141bv2), int("id", 0x22), int("channel", 1), batt(false), num("temperature", -7.5)],
                model: "LaCrosse-TX141Bv2",
                id: "22",
                channel: Some("1"),
                battery_ok: Some(false),
                has: &["Temperature -7.5"],
            },
            Case {
                args: vec![fam(F::Tx141bv3), int("id", 0x33), int("channel", 2), num("temperature", 30.1)],
                model: "LaCrosse-TX141Bv3",
                id: "33",
                channel: Some("2"),
                battery_ok: Some(true),
                has: &["Temperature 30.1"],
            },
            Case {
                args: vec![fam(F::Acurite606), int("id", 77), int("channel", 2), num("temperature", -1.2)],
                model: "Acurite-606TX",
                id: "77",
                channel: Some("2"),
                battery_ok: Some(true),
                has: &["Temperature -1.2"],
            },
            Case {
                args: vec![fam(F::Acurite609), int("id", 99), batt(false), num("temperature", 22.4), int("humidity", 61)],
                model: "Acurite-609TXC",
                id: "99",
                channel: None,
                battery_ok: Some(false),
                has: &["Temperature 22.4", "Humidity 61"],
            },
            Case {
                args: vec![fam(F::Kedsum), int("id", 12), int("channel", 3), num("temperature", 18.0), int("humidity", 52)],
                model: "Kedsum-TH",
                id: "12",
                channel: Some("3"),
                battery_ok: Some(true),
                has: &["Temperature 18.0", "Humidity 52"],
            },
            Case {
                args: vec![fam(F::Gtwt02), int("id", 40), int("channel", 2), num("temperature", 19.9), int("humidity", 55)],
                model: "GT-WT02",
                id: "40",
                channel: Some("2"),
                battery_ok: Some(true),
                has: &["Temperature 19.9", "Humidity 55"],
            },
            Case {
                args: vec![fam(F::Wh2), int("id", 120), num("temperature", -4.6), int("humidity", 70)],
                model: "Fineoffset-WH2",
                id: "120",
                channel: None,
                battery_ok: None,
                has: &["Temperature -4.6", "Humidity 70"],
            },
            Case {
                args: vec![fam(F::Wh2a), int("id", 121), num("temperature", 5.5), int("humidity", 40)],
                model: "Fineoffset-WH2A",
                id: "121",
                channel: None,
                battery_ok: None,
                has: &["Temperature 5.5", "Humidity 40"],
            },
            Case {
                args: vec![fam(F::Wh5), int("id", 122), num("temperature", 10.0), int("humidity", 33)],
                model: "Fineoffset-WH5",
                id: "122",
                channel: None,
                battery_ok: None,
                has: &["Temperature 10.0", "Humidity 33"],
            },
            Case {
                args: vec![fam(F::TelldusProove), int("id", 123), num("temperature", 2.2), int("humidity", 80)],
                model: "Fineoffset-TelldusProove",
                id: "123",
                channel: None,
                battery_ok: None,
                has: &["Temperature 2.2", "Humidity 80"],
            },
            Case {
                args: vec![fam(F::Tfa303225), int("id", 124), batt(false), num("temperature", 8.8)],
                model: "TFA-303225",
                id: "124",
                channel: None,
                battery_ok: Some(false),
                has: &["Temperature 8.8"],
            },
            Case {
                args: vec![fam(F::Tp11), int("id", 1234), num("temperature", 21.7)],
                model: "Thermopro-TP11",
                id: "1234",
                channel: None,
                battery_ok: None,
                has: &["Temperature 21.7"],
            },
            Case {
                args: vec![fam(F::Tp12), int("id", 200), num("temperature", 65.0), num("temperature_2", 110.5)],
                model: "Thermopro-TP12",
                id: "200",
                channel: None,
                battery_ok: None,
                has: &["Temperature 1 65.0", "Temperature 2 110.5"],
            },
            Case {
                args: vec![fam(F::Ft004b), int("id", 0), num("temperature", 25.0)],
                model: "FT-004B",
                id: "0",
                channel: None,
                battery_ok: None,
                has: &["Temperature 25.0"],
            },
            Case {
                args: vec![fam(F::ErtScm), int("id", 33_554_433), int("meter_type", 12), int("consumption", 98_765)],
                model: "ERT-SCM",
                id: "33554433",
                channel: None,
                battery_ok: None,
                has: &["Consumption 98765", "ERT type 12"],
            },
            Case {
                args: vec![fam(F::ErtIdm), int("id", 0), hex("1C 5C C6 04 07 00 AC 17 1D F6 BC 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 05 30 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 DC EA BA")],
                model: "ERT-IDM",
                id: "11278109",
                channel: None,
                battery_ok: None,
                has: &["ERT type 7", "Last consumption count 339972"],
            },
            Case {
                args: vec![fam(F::Honeywell), int("id", 0xABCDE), int("channel", 3), int("flags", 0x80), batt(false)],
                model: "Honeywell-Security",
                id: "abcde",
                channel: Some("3"),
                battery_ok: Some(false),
                has: &["Door open"],
            },
            Case {
                args: vec![fam(F::Kerui), int("id", 0x54321), int("command", 14)],
                model: "Kerui-Security",
                id: "54321",
                channel: None,
                battery_ok: Some(true),
                has: &["open"],
            },
            Case {
                args: vec![fam(F::Springfield), int("id", 66), int("channel", 2), num("temperature", 15.5), int("moisture", 70)],
                model: "Springfield-Soil",
                id: "66",
                channel: Some("2"),
                battery_ok: Some(true),
                has: &["Temperature 15.5", "Moisture 70"],
            },
            Case {
                args: vec![fam(F::Pmv107j), int("id", 0x123_4567), num("pressure", 230.0), num("temperature", 30.0)],
                model: "PMV-107J",
                id: "01234567",
                channel: None,
                battery_ok: Some(true),
                has: &["Pressure 230.6", "Temperature 30"],
            },
            Case {
                args: vec![fam(F::Hyundai), int("id", 0), hex("21 0A 0B 0C 0D 12 A0 4B 00")],
                model: "Hyundai-VDO",
                id: "0a0b0c0d",
                channel: None,
                battery_ok: None,
                has: &["Pressure 220.0", "Repeat 2"],
            },
            Case {
                args: vec![fam(F::Abarth), int("id", 0), hex("0A 0B 0C 0D 00 A0 4B 00")],
                model: "Abarth-124Spider",
                id: "0a0b0c0d",
                channel: None,
                battery_ok: None,
                has: &["Pressure 220.8", "Temperature 25"],
            },
        ]
    }

    /// A family whose frames another protocol reads too: the Solight rows of 37 bits pass the Rubicson test (36 to 38 bits).
    const ALSO: [(&str, &str); 1] = [("Solight-TE44", "Rubicson-Temperature")];

    #[test]
    fn every_family_comes_back_with_its_fields() {
        let s = station();
        for c in cases() {
            let x = s.send(&c.args);
            let got = devices(&x);
            let mine: Vec<&DeviceLine> = got.iter().filter(|g| g.0 == c.model).collect();
            assert_eq!(mine.len(), 1, "{}: {got:?}", c.model);
            assert!(
                got.iter().all(|g| g.0 == c.model || ALSO.contains(&(c.model, g.0.as_str()))),
                "{}: {got:?}",
                c.model
            );
            let g = mine[0];
            assert_eq!((g.0.as_str(), g.1.as_str()), (c.model, c.id), "{got:?}");
            assert_eq!(g.2.as_deref(), c.channel, "{}: {got:?}", c.model);
            assert_eq!(g.3, c.battery_ok, "{}: {got:?}", c.model);
            for h in c.has {
                assert!(g.4.contains(h), "{}: {h} in {}", c.model, g.4);
            }
        }
    }

    #[test]
    fn every_family_is_offered() {
        let fields: Vec<&str> = cases().iter().map(|c| c.model).collect();
        for f in (0..).map_while(IsmFamily::from_index) {
            assert!(
                cases().iter().any(|c| c.args[0].1 == Value::Choice(f.index())),
                "{f:?} has no case in {fields:?}"
            );
        }
    }

    #[test]
    fn a_frame_is_refused_while_transmit_is_off_and_for_a_bad_field() {
        let s = station();
        assert!(s.refusal(false, &[("id", Value::I64(1))]).contains("off"));
        assert!(s.refusal(true, &[("id", Value::I64(0))]).contains("id"));
        let tx = [("family", Value::Choice(1)), ("id", Value::I64(300))];
        assert!(s.refusal(true, &tx).contains("id"));
        let nexus = [fam(IsmFamily::Nexus), ("id", Value::I64(5)), ("channel", Value::I64(4))];
        assert!(s.refusal(true, &nexus).contains("channel"));
        let raw = [fam(IsmFamily::Citroen), ("id", Value::I64(0)), ("payload", text("20 01"))];
        assert!(s.refusal(true, &raw).contains("payload"));
    }
}
