//! A fake iiod server on a fake network, for tests: the context of an AD9361 board, attribute reads and writes, receive and
//! transmit buffers over the text and binary protocols, and a count of open connections.

use super::binary::{op, Header};
use sdrkit_core::net::{Conn, FakeTransport, Transport};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct FakeOpts {
    pub two_receivers: bool,
    pub fw: &'static str,
    pub version: &'static str,
    pub lo_range: &'static str,
    /// The board reports this rate instead of what was written (a coarse clock).
    pub rate_cap: Option<f64>,
    /// Close each stream connection after this many buffers.
    pub drop_stream_after: Option<usize>,
    /// Offer the binary protocol (a libiio 1.x server).
    pub binary: bool,
    /// Send every buffer as fast as possible, not paced to the rate.
    pub fast: bool,
    /// Stream all four receive channels whatever was requested (another client requested them).
    pub widen: bool,
    pub ensm: &'static str,
    /// Refuse the FIR configuration.
    pub refuse_fir: bool,
    /// Refuse the receive FIR alone: only both filters together (the `out` channel) switch on, as the ANTSDR E310 did.
    pub fir_needs_both: bool,
    /// Take the receive FIR alone but refuse a rate below the converters' lowest until both filters are on (the `out` channel),
    /// as an ANTSDR E310 also did.
    pub low_rate_needs_both: bool,
    /// An earlier program left the FIR switches at this value ("1": on).
    pub tx_fir_left_on: Option<&'static str>,
    /// The FIR switches cannot be turned off (every write of them is taken but nothing changes).
    pub tx_fir_stuck: bool,
    /// The transmit buffer plays at the sample rate, as the converter of a radio does: a transfer waits while the
    /// kernel buffers of the connection (four unless it set a number) are queued, the samples that were not played when
    /// the output is muted count in [`Board::tx_cut`], and the time the converter ran dry while unmuted counts in
    /// [`Board::tx_gap`].
    pub tx_plays: bool,
    /// The scan channels of TX1 are played through the attenuator of TX2 and the reverse, as an ANTSDR with a Z7020 and an AD9361 did. Off:
    /// each pair is played through the attenuator of its own number.
    pub tx_crossed: bool,
    /// The most bytes a second the board takes for transmit. A transmit buffer that needs more runs dry: the samples
    /// of time without data count in [`Board::tx_gap`].
    pub tx_max_bytes_per_s: Option<f64>,
    /// With [`FakeOpts::tx_plays`]: after 100 ms without a transmit transfer, each of the next `.0` transfers takes `.1`
    /// times the time that its buffer plays, as the first transfers to a radio after a pause do.
    pub tx_slow_start: Option<(usize, f64)>,
    /// The capture device offers its registers (`direct_reg_access`), and an answer that [`Board::stall_ms`] makes late
    /// by more than the kernel buffers hold overflows: the samples that do not fit are dropped ([`Board::rx_overflowed`])
    /// and the overflow flag (bit 2 of 0x80000088) is set until written.
    pub overflow_reg: bool,
    /// The most bytes a second that the link from the board to a client carries (a slow Wi-Fi link): every answer to a
    /// `READBUF` is written in pieces at this pace.
    pub link_bytes_per_s: Option<f64>,
}

impl Default for FakeOpts {
    fn default() -> Self {
        FakeOpts {
            two_receivers: false,
            fw: "v0.39-1-g7bcc-dirty",
            version: "0.26.v0.26  ",
            lo_range: "[70000000 1 6000000000]",
            rate_cap: None,
            drop_stream_after: None,
            binary: false,
            fast: false,
            widen: false,
            ensm: "fdd",
            refuse_fir: false,
            fir_needs_both: false,
            low_rate_needs_both: false,
            tx_fir_left_on: None,
            tx_fir_stuck: false,
            tx_plays: false,
            tx_crossed: false,
            tx_max_bytes_per_s: None,
            tx_slow_start: None,
            overflow_reg: false,
            link_bytes_per_s: None,
        }
    }
}

/// One transmit transfer of one output that held samples other than zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Played {
    /// The pair of scan channels that held the samples: 0 for `voltage0` and `voltage1`, 1 for `voltage2` and `voltage3`.
    pub pair: usize,
    /// The phy channel of the attenuator the board played them through.
    pub attenuator: usize,
    /// The gain of that attenuator when the transfer arrived, in dB.
    pub gain_db: f64,
}

#[derive(Default)]
pub struct Board {
    pub attrs: Mutex<HashMap<String, String>>,
    /// Every write, as "target attr=value", and the stream commands, in order.
    pub log: Mutex<Vec<String>>,
    pub streams: Mutex<usize>,
    /// Bytes received by WRITEBUF, per device, in order.
    pub tx_bytes: Mutex<Vec<u8>>,
    /// What went on the air: every transmit transfer with samples other than zero, per output.
    pub tx_played: Mutex<Vec<Played>>,
    /// With [`FakeOpts::tx_plays`]: the samples that were queued but not played when the output was muted.
    pub tx_cut: std::sync::atomic::AtomicU64,
    /// With [`FakeOpts::tx_plays`] or [`FakeOpts::tx_max_bytes_per_s`]: the samples of time that the converter had
    /// nothing to play inside a burst.
    pub tx_gap: std::sync::atomic::AtomicU64,
    /// With [`FakeOpts::tx_plays`]: the part of [`Board::tx_gap`] that fell between two transfers that both held samples
    /// other than zero. Only this dry time damages a signal: before, after and between transfers of zeros it is silent.
    pub tx_gap_in_signal: std::sync::atomic::AtomicU64,
    /// The last transmit transfer held samples other than zero.
    pub tx_last_signal: AtomicBool,
    /// When the converter has played what it was given, and whether the output is unmuted and has played a transfer.
    pub tx_play: Mutex<(Option<Instant>, bool, bool)>,
    /// Connections open now, and in all.
    pub open: AtomicUsize,
    pub total: AtomicUsize,
    /// Every answer to an attribute read waits this long, in ms (a busy board).
    pub read_delay_ms: std::sync::atomic::AtomicU64,
    /// Attribute reads that waited [`Board::read_delay_ms`].
    pub slow_reads: AtomicUsize,
    /// Every attribute write waits this long, in ms (a slow retune).
    pub write_delay_ms: std::sync::atomic::AtomicU64,
    /// With a value: the receive channels carry noise, not the ramp of [`value`]: Gaussian, cut at 13 dB above its mean (the
    /// peak-to-mean ratio of OFDM), at this mean level in dBFS plus the `hardwaregain` of the receiver, clipped at the 12 bits
    /// of the converter.
    pub level_dbfs: Mutex<Option<f64>>,
    /// The receive sample instants sent, on all connections.
    pub rx_samples: std::sync::atomic::AtomicU64,
    /// The next this many text `READBUF`s are answered with an errno (-110), and their samples are not sent.
    pub fail_readbufs: AtomicUsize,
    /// The samples of the failed `READBUF`s.
    pub rx_withheld: std::sync::atomic::AtomicU64,
    /// The next text `READBUF` is answered this many ms late.
    pub stall_ms: std::sync::atomic::AtomicU64,
    /// With [`FakeOpts::overflow_reg`]: the samples dropped by overflows, and the overflow flag.
    pub rx_overflowed: std::sync::atomic::AtomicU64,
    pub capture_overflow: AtomicBool,
    /// The next `READBUF` answer stops after its first half for this many ms (a stall in the middle of a refill).
    pub stall_mid_ms: std::sync::atomic::AtomicU64,
    /// The next `READBUF` answer starts with a line that is not a number: the client loses the framing.
    pub garble_next: AtomicBool,
    /// Lines that a client sent on a connection after the answer that garbled it.
    pub after_garble: AtomicUsize,
    /// Lines that a client sent while an answer to a `READBUF` was still being written to it: a break of the protocol.
    pub violations: Mutex<Vec<String>>,
    /// The connections that hold the receive buffer open.
    pub rx_holders: AtomicUsize,
    /// With a value: the link carries this many bytes a second from now on, in place of [`FakeOpts::link_bytes_per_s`].
    pub link_now: Mutex<Option<f64>>,
}

impl Board {
    pub fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
    pub fn has(&self, s: &str) -> bool {
        self.log().iter().any(|l| l == s)
    }
    pub fn push(&self, s: impl Into<String>) {
        self.log.lock().unwrap().push(s.into());
    }
    /// The lines that a client sent in the middle of an answer.
    pub fn violations(&self) -> Vec<String> {
        self.violations.lock().unwrap().clone()
    }
    /// The receive buffer is held by one more (`up`) or one less connection; its `enable` follows.
    fn hold_rx(&self, up: bool) {
        let n = if up {
            self.rx_holders.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            self.rx_holders.fetch_sub(1, Ordering::SeqCst) - 1
        };
        let v = if n > 0 { "1" } else { "0" };
        self.attrs
            .lock()
            .unwrap()
            .insert("iio:device4 BUFFER enable".into(), v.into());
    }
    /// The gain of the transmit attenuator of phy channel `chan`, in dB.
    pub fn tx_gain(&self, chan: usize) -> f64 {
        self.attrs
            .lock()
            .unwrap()
            .get(&format!("iio:device1 OUTPUT voltage{chan} hardwaregain"))
            .and_then(|v| v.split_whitespace().next()?.parse().ok())
            .unwrap_or(f64::NAN)
    }
    /// The log entries that touch the transmit side.
    pub fn tx_log(&self) -> Vec<String> {
        self.log()
            .into_iter()
            .filter(|l| {
                l.contains("iio:device3")
                    || l.contains("OUTPUT voltage")
                    || l.contains("altvoltage1")
            })
            .collect()
    }
}

pub struct FakeIiod {
    pub port: u16,
    pub board: Arc<Board>,
    net: FakeTransport,
    stop: Arc<AtomicBool>,
    host: String,
}

impl Drop for FakeIiod {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.net.stop_serving(&self.host, self.port);
    }
}

fn xml(o: &FakeOpts) -> String {
    let attr = |n: &str| format!("<attribute name=\"{n}\" filename=\"x\" />");
    let attrs = |names: &[&str]| names.iter().map(|a| attr(a)).collect::<String>();
    let phy_ch = |id: &str| {
        format!(
            "<channel id=\"{id}\" type=\"input\">{}</channel><channel id=\"{id}\" type=\"output\">{}</channel>",
            attrs(&[
                "bb_dc_offset_tracking_en",
                "filter_fir_en",
                "gain_control_mode",
                "gain_control_mode_available",
                "hardwaregain",
                "hardwaregain_available",
                "quadrature_tracking_en",
                "rf_bandwidth",
                "rf_dc_offset_tracking_en",
                "rf_port_select",
                "rf_port_select_available",
                "rssi",
                "sampling_frequency",
            ]),
            attrs(&[
                "filter_fir_en",
                "hardwaregain",
                "hardwaregain_available",
                "rf_bandwidth",
                "rf_port_select",
                "rf_port_select_available",
                "sampling_frequency",
            ])
        )
    };
    let mut phy = phy_ch("voltage0");
    if o.two_receivers {
        phy += &phy_ch("voltage1");
    }
    phy += &format!(
        "<channel id=\"temp0\" type=\"input\">{}</channel>",
        attr("input")
    );
    phy += &format!(
        "<channel id=\"out\" type=\"input\">{}</channel>",
        attr("voltage_filter_fir_en")
    );
    let n = if o.two_receivers { 4 } else { 2 };
    let scan = |i: usize, out: bool, f: &str| {
        format!(
            "<channel id=\"voltage{i}\" type=\"{}\"><scan-element index=\"{i}\" format=\"{f}\" scale=\"1.000000\" /></channel>",
            if out { "output" } else { "input" }
        )
    };
    let rx: String = (0..n)
        .map(|i| scan(i, false, "le:S12/16&gt;&gt;0"))
        .collect();
    let mut dds: String = (0..n)
        .map(|i| scan(i, true, "le:S16/16&gt;&gt;0"))
        .collect();
    for i in 0..2 * n {
        dds += &format!(
            "<channel id=\"altvoltage{i}\" type=\"output\">{}</channel>",
            attrs(&["frequency", "raw", "scale"])
        );
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><!DOCTYPE context [<!ELEMENT context (device | context-attribute)*><!ATTLIST context name CDATA #REQUIRED>]>\
         <context name=\"network\"><context-attribute name=\"hw_model\" value=\"Fake AD9361 board\" /><context-attribute name=\"fw_version\" value=\"{}\" />\
         <device id=\"iio:device0\" name=\"adm1177\"></device>\
         <device id=\"iio:device1\" name=\"ad9361-phy\">{phy}<channel id=\"altvoltage0\" name=\"RX_LO\" type=\"output\">{}</channel>\
         <channel id=\"altvoltage1\" name=\"TX_LO\" type=\"output\">{}</channel>{}</device>\
         <device id=\"iio:device3\" name=\"cf-ad9361-dds-core-lpc\">{dds}<buffer-attribute name=\"enable\" /></device>\
         <device id=\"iio:device4\" name=\"cf-ad9361-lpc\">{rx}<buffer-attribute name=\"enable\" /><buffer-attribute name=\"length\" />{}</device></context>",
        o.fw,
        attrs(&["frequency", "frequency_available", "powerdown"]),
        attrs(&["frequency", "frequency_available", "powerdown"]),
        attrs(&["calib_mode", "ensm_mode", "filter_fir_config", "xo_correction"]),
        if o.overflow_reg {
            "<debug-attribute name=\"direct_reg_access\" />"
        } else {
            ""
        },
    )
}

/// The value a fake receive channel holds at sample k.
pub fn value(chan: usize, k: i16) -> i16 {
    match chan {
        0 => k,
        1 => -k,
        2 => k + 1000,
        _ => -(k + 1000),
    }
}

impl FakeIiod {
    /// A board on a fake network of its own, at `127.0.0.1` on the iiod port.
    pub fn start(o: FakeOpts) -> FakeIiod {
        Self::start_on(FakeTransport::new(), o)
    }

    /// A board on the fake network `net`, at `127.0.0.1` on the iiod port.
    pub fn start_on(net: FakeTransport, o: FakeOpts) -> FakeIiod {
        Self::start_at(net, "127.0.0.1", o)
    }

    /// A board on the fake network `net`, at `host` (an IP address) on the iiod port. An address that is not the loopback
    /// is a board across a network link.
    pub fn start_at(net: FakeTransport, host: &str, o: FakeOpts) -> FakeIiod {
        let port = super::proto::IIOD_PORT;
        let board = Arc::new(Board::default());
        {
            let mut a = board.attrs.lock().unwrap();
            let mut set = |k: &str, v: &str| {
                a.insert(k.into(), v.into());
            };
            set("iio:device1 xo_correction", "40000000");
            set("iio:device1 ensm_mode", o.ensm);
            set("iio:device1 calib_mode", "auto");
            set(
                "iio:device1 OUTPUT altvoltage0 frequency_available",
                o.lo_range,
            );
            set(
                "iio:device1 OUTPUT altvoltage1 frequency_available",
                "[46875001 1 6000000000]",
            );
            set("iio:device1 INPUT temp0 input", "41250");
            for ch in ["voltage0", "voltage1"] {
                set(
                    &format!("iio:device1 INPUT {ch} hardwaregain_available"),
                    "[-1 1 73]",
                );
                set(
                    &format!("iio:device1 INPUT {ch} hardwaregain"),
                    "71.000000 dB",
                );
                set(&format!("iio:device1 INPUT {ch} rssi"), "85.25 dB");
                set(
                    &format!("iio:device1 INPUT {ch} gain_control_mode_available"),
                    "manual fast_attack slow_attack hybrid",
                );
                set(
                    &format!("iio:device1 INPUT {ch} rf_port_select_available"),
                    "A_BALANCED B_BALANCED C_BALANCED A_N A_P B_N B_P C_N C_P TX_MONITOR1 TX_MONITOR2 TX_MONITOR1_2",
                );
                set(
                    &format!("iio:device1 OUTPUT {ch} hardwaregain_available"),
                    "[-89.750000 0.250000 0.000000]",
                );
                // The firmware starts with every transmit attenuator at 10 dB.
                set(
                    &format!("iio:device1 OUTPUT {ch} hardwaregain"),
                    "-10.000000 dB",
                );
                set(
                    &format!("iio:device1 OUTPUT {ch} rf_port_select_available"),
                    "A B",
                );
                set(&format!("iio:device1 OUTPUT {ch} filter_fir_en"), "0");
            }
            set("iio:device1 INPUT out voltage_filter_fir_en", "0");
            if let Some(v) = o.tx_fir_left_on {
                set("iio:device1 OUTPUT voltage0 filter_fir_en", v);
                set("iio:device1 OUTPUT voltage1 filter_fir_en", v);
                set("iio:device1 INPUT out voltage_filter_fir_en", v);
            }
            set("iio:device1 INPUT voltage0 sampling_frequency", "30720000");
            set("iio:device1 OUTPUT voltage0 sampling_frequency", "30720000");
        }
        let stop = Arc::new(AtomicBool::new(false));
        let (b, s, x) = (board.clone(), stop.clone(), xml(&o));
        net.serve(host, port, move |c| {
            let (s, x, o) = (s.clone(), x.clone(), o.clone());
            b.open.fetch_add(1, Ordering::SeqCst);
            b.total.fetch_add(1, Ordering::SeqCst);
            serve(c, &b, s, x, o);
            b.open.fetch_sub(1, Ordering::SeqCst);
        });
        FakeIiod {
            port,
            board,
            net,
            stop,
            host: host.to_string(),
        }
    }

    /// The network the board is on, to open radios through.
    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.net.clone())
    }

    /// The fake network the board is on.
    pub fn net(&self) -> &FakeTransport {
        &self.net
    }

    pub fn addr(&self) -> String {
        format!("ip:{}:{}", self.host, self.port)
    }

    /// Connections open now.
    pub fn open_connections(&self) -> usize {
        self.board.open.load(Ordering::SeqCst)
    }
}

/// The unit noise of [`Board::level_dbfs`]: Gaussian values cut at 13 dB above their mean power.
fn noise() -> &'static [f32] {
    static NOISE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    NOISE.get_or_init(|| {
        let cut = 10f64.powf(13.0 / 20.0);
        let mut x: u64 = 0x2545_f491_4f6c_dd1d;
        let mut uniform = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            ((x >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        (0..1 << 17)
            .map(|_| {
                let (a, b) = (uniform(), uniform());
                let g = (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos();
                g.clamp(-cut, cut) as f32
            })
            .collect()
    })
}

/// Makes receive data for `samples` instants of the channels in `mask` (bits 0 to 3): the ramp from sample counter `k`, or
/// with [`Board::level_dbfs`] noise from position `pos` of [`noise`].
fn rx_fill(b: &Board, mask: u32, samples: usize, k: &mut i16, pos: &mut usize, out: &mut Vec<u8>) {
    b.rx_samples.fetch_add(samples as u64, Ordering::Relaxed);
    let level = *b.level_dbfs.lock().unwrap();
    let Some(level) = level.filter(|_| mask != 1) else {
        return rx_data(mask, samples, k, out);
    };
    let scale: Vec<f64> = {
        let a = b.attrs.lock().unwrap();
        (0..4)
            .map(|ch| {
                let gain = a
                    .get(&format!("iio:device1 INPUT voltage{} hardwaregain", ch / 2))
                    .and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok())
                    .unwrap_or(0.0);
                2048.0 * 10f64.powf((level + gain) / 20.0)
            })
            .collect()
    };
    let n = noise();
    out.clear();
    for _ in 0..samples {
        for (ch, s) in scale.iter().enumerate() {
            if mask >> ch & 1 == 1 {
                let v = (f64::from(n[*pos]) * s).round().clamp(-2048.0, 2047.0) as i16;
                out.extend(v.to_le_bytes());
                *pos = (*pos + 1) % n.len();
            }
        }
    }
}

/// Makes receive data for `samples` instants of the channels in `mask` (bits 0 to 3), from sample counter `k`.
fn rx_data(mask: u32, samples: usize, k: &mut i16, out: &mut Vec<u8>) {
    out.clear();
    let cs8 = mask == 1;
    for _ in 0..samples {
        if cs8 {
            out.extend([(*k % 100) as i8 as u8, (-(*k % 100) as i8) as u8]);
        } else {
            for ch in 0..4 {
                if mask >> ch & 1 == 1 {
                    out.extend(value(ch, *k).to_le_bytes());
                }
            }
        }
        *k = (*k + 1) % 1000;
    }
}

fn rate_of(b: &Board) -> f64 {
    b.attrs
        .lock()
        .unwrap()
        .get("iio:device1 INPUT voltage0 sampling_frequency")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1e6)
}

struct Pace {
    started: Instant,
    sent: u64,
}

impl Pace {
    fn wait(&mut self, samples: usize, rate: f64) {
        self.sent += samples as u64;
        let due = Duration::from_secs_f64(self.sent as f64 / rate);
        if let Some(w) = due.checked_sub(self.started.elapsed()) {
            std::thread::sleep(w);
        }
    }
}

/// The receive buffer that one connection holds open; given back when the connection ends.
struct Holder<'a> {
    b: &'a Board,
    on: bool,
}

impl Holder<'_> {
    fn set(&mut self, on: bool) {
        if on != self.on {
            self.on = on;
            self.b.hold_rx(on);
        }
    }
}

impl Drop for Holder<'_> {
    fn drop(&mut self) {
        self.set(false);
    }
}

/// The lines other than `READBUF` that the client has sent and that wait to be read, without taking them; `None` when it
/// closed the connection. A `READBUF` sent ahead while answers are owed is within the protocol.
fn client_spoke(r: &mut BufReader<Box<dyn Conn>>) -> Option<Vec<String>> {
    if r.buffer().is_empty() {
        let _ = r.get_mut().set_read_timeout(Some(Duration::from_millis(1)));
        let got = r.fill_buf().map(|b| b.is_empty());
        let _ = r.get_mut().set_read_timeout(None);
        if matches!(got, Ok(true)) {
            return None;
        }
    }
    let text = String::from_utf8_lossy(r.buffer()).to_string();
    Some(
        text.split('\n')
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with("READBUF "))
            .map(String::from)
            .collect(),
    )
}

/// Writes the answer `out` to a `READBUF`: in pieces at the pace of [`FakeOpts::link_bytes_per_s`], stopping after the first
/// half for `stall_ms`. A client line that arrives before the last piece is a violation. False when the client went away.
fn send_answer(
    w: &mut Box<dyn Conn>,
    r: &mut BufReader<Box<dyn Conn>>,
    out: &[u8],
    b: &Board,
    o: &FakeOpts,
    mut stall_ms: u64,
) -> bool {
    let link = (*b.link_now.lock().unwrap()).or(o.link_bytes_per_s);
    let paced = link.is_some() || stall_ms > 0;
    if !paced {
        return w.write_all(out).is_ok();
    }
    // Pieces of about 5 ms of the link, so that the pace holds with the coarse sleeps of the system.
    let piece = link.map_or(16 << 10, |bps| ((bps * 0.005) as usize).max(16 << 10));
    let start = Instant::now();
    let mut sent = 0;
    let mut paused = Duration::ZERO;
    let mut looked: Option<Instant> = None;
    let mut seen: Vec<String> = Vec::new();
    while sent < out.len() {
        if looked.is_none_or(|t| t.elapsed() >= Duration::from_millis(20)) {
            looked = Some(Instant::now());
            let Some(lines) = client_spoke(r) else {
                return false;
            };
            for l in lines {
                if !seen.contains(&l) {
                    b.violations.lock().unwrap().push(l.clone());
                    seen.push(l);
                }
            }
        }
        if stall_ms > 0 && sent >= out.len() / 2 {
            // The stall ends early when the client goes away, as a server's write fails on a closed connection.
            let t = Instant::now();
            let d = Duration::from_millis(std::mem::take(&mut stall_ms));
            while t.elapsed() < d {
                if client_spoke(r).is_none() {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            paused += t.elapsed();
            continue;
        }
        let n = piece.min(out.len() - sent);
        if w.write_all(&out[sent..sent + n]).is_err() {
            return false;
        }
        sent += n;
        if let Some(bps) = link {
            let due = start + paused + Duration::from_secs_f64(sent as f64 / bps);
            if let Some(wait) = due.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
        }
    }
    true
}

fn serve(c: Box<dyn Conn>, b: &Board, stop: Arc<AtomicBool>, xml: String, o: FakeOpts) {
    let mut w = c.try_clone().unwrap();
    let mut r = BufReader::new(c);
    let mut holder = Holder { b, on: false };
    let mut garbled = false;
    let mut readbufs = 0;
    let mut mask = String::new();
    let mut k: i16 = 0;
    let mut pos = 0;
    let mut pace = Pace {
        started: Instant::now(),
        sent: 0,
    };
    let mut fast_cache: Option<(usize, Vec<u8>)> = None;
    let mut data = Vec::new();
    // The transmit side of this connection: the kernel buffers it set, the last transfer, the slow transfers left.
    let mut kbuffers = 4u32;
    let mut last_tx: Option<Instant> = None;
    let mut slow_left = 0;
    loop {
        let mut line = String::new();
        if stop.load(Ordering::Relaxed) || r.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if garbled {
            b.after_garble.fetch_add(1, Ordering::SeqCst);
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(&cmd) = words.first() else { continue };
        let reply = |w: &mut Box<dyn Conn>, s: &str| w.write_all(s.as_bytes()).is_ok();
        let key = |ws: &[&str]| ws.join(" ");
        let ok = match cmd {
            "VERSION" => reply(&mut w, &format!("{}\n", o.version)),
            "TIMEOUT" => reply(&mut w, "0\n"),
            "PRINT" => reply(&mut w, &format!("{}\n{xml}\n", xml.len())),
            "BINARY" if o.binary => {
                if !reply(&mut w, "0\n") {
                    return;
                }
                b.push("BINARY");
                serve_binary(r, w, b, &o);
                return;
            }
            "READ" => {
                let wait = b.read_delay_ms.load(Ordering::Relaxed);
                if wait > 0 {
                    std::thread::sleep(Duration::from_millis(wait));
                    b.slow_reads.fetch_add(1, Ordering::Relaxed);
                }
                let k = key(&words[1..]);
                let v = if k.ends_with("DEBUG direct_reg_access") {
                    let on = b.capture_overflow.load(Ordering::Relaxed);
                    Some(format!("0x{:x}", if on { 4 } else { 0 }))
                } else {
                    b.attrs.lock().unwrap().get(&k).cloned()
                };
                match v {
                    Some(v) => {
                        let mut out = format!("{}\n", v.len() + 1).into_bytes();
                        out.extend(v.as_bytes());
                        out.extend(b"\0\n");
                        w.write_all(&out).is_ok()
                    }
                    None => reply(&mut w, "-2\n"),
                }
            }
            "WRITE" => {
                let wait = b.write_delay_ms.load(Ordering::Relaxed);
                if wait > 0 {
                    std::thread::sleep(Duration::from_millis(wait));
                }
                let len: usize = words.last().unwrap().parse().unwrap();
                let mut v = vec![0u8; len];
                r.read_exact(&mut v).unwrap();
                let v = String::from_utf8(v).unwrap();
                let target = key(&words[1..words.len() - 1]);
                let attr = words[words.len() - 2];
                if attr == "direct_reg_access" {
                    let flag = v.split_whitespace().nth(1).and_then(|x| {
                        u64::from_str_radix(x.trim_start_matches("0x"), 16).ok()
                    });
                    if flag.is_some_and(|f| f & 4 != 0) {
                        b.capture_overflow.store(false, Ordering::Relaxed);
                    }
                }
                let num: f64 = v.trim().parse().unwrap_or(0.0);
                let refuse = (attr == "frequency"
                    && target.contains("altvoltage0")
                    && !(325e6..=3.8e9).contains(&num)
                    && o.lo_range.contains("3800000000"))
                    || (attr == "hardwaregain" && num > 73.0)
                    || (attr == "filter_fir_config" && o.refuse_fir)
                    || (attr == "filter_fir_en" && v == "1" && o.fir_needs_both)
                    || (attr == "sampling_frequency"
                        && num < 25e6 / 12.0
                        && o.low_rate_needs_both
                        && b.attrs
                            .lock()
                            .unwrap()
                            .get("iio:device1 INPUT out voltage_filter_fir_en")
                            .map(String::as_str)
                            != Some("1"));
                if o.tx_plays && attr == "hardwaregain" && target.contains("OUTPUT") {
                    let rate: f64 = b
                        .attrs
                        .lock()
                        .unwrap()
                        .get("iio:device1 OUTPUT voltage0 sampling_frequency")
                        .and_then(|r| r.parse().ok())
                        .unwrap_or(1e6);
                    let mut p = b.tx_play.lock().unwrap();
                    if num > -80.0 {
                        p.1 = true;
                        p.2 = false;
                    } else if p.1 {
                        let left = p.0.map_or(0.0, |u| {
                            u.saturating_duration_since(Instant::now()).as_secs_f64()
                        });
                        b.tx_cut.fetch_add((left * rate) as u64, Ordering::Relaxed);
                        *p = (None, false, false);
                    }
                }
                b.push(format!(
                    "{target}={}",
                    if attr == "filter_fir_config" {
                        format!("<fir {}>", v.lines().next().unwrap_or(""))
                    } else {
                        v.clone()
                    }
                ));
                if refuse {
                    reply(&mut w, "-22\n")
                } else {
                    let stored = match (attr, o.rate_cap) {
                        ("sampling_frequency", Some(cap)) => format!("{}", num.min(cap)),
                        _ => v.clone(),
                    };
                    let mut a = b.attrs.lock().unwrap();
                    // The AD9361's RX and TX share one rate.
                    if attr == "sampling_frequency" {
                        a.insert(
                            "iio:device1 INPUT voltage0 sampling_frequency".into(),
                            stored.clone(),
                        );
                        a.insert(
                            "iio:device1 OUTPUT voltage0 sampling_frequency".into(),
                            stored.clone(),
                        );
                    }
                    let stuck = o.tx_fir_stuck && attr.ends_with("filter_fir_en");
                    if !stuck {
                        a.insert(target, stored);
                    }
                    drop(a);
                    reply(&mut w, &format!("{len}\n"))
                }
            }
            "SET" | "CLOSE" => {
                b.push(key(&words));
                if cmd == "SET" && words.get(2) == Some(&"BUFFERS_COUNT") {
                    kbuffers = words.get(3).and_then(|n| n.parse().ok()).unwrap_or(4);
                }
                if cmd == "CLOSE" && words[1] == "iio:device3" {
                    b.attrs.lock().unwrap().remove("iio:device3 BUFFER enable");
                }
                if cmd == "CLOSE" && words[1] == "iio:device4" {
                    holder.set(false);
                }
                reply(&mut w, "0\n")
            }
            "OPEN" => {
                b.push(key(&words));
                *b.streams.lock().unwrap() += 1;
                mask = words[3].to_string();
                if words[1] == "iio:device4" {
                    holder.set(true);
                }
                if o.widen && words[1] == "iio:device4" {
                    mask = "0000000f".into();
                }
                reply(&mut w, "0\n")
            }
            "READBUF" => {
                readbufs += 1;
                if o.drop_stream_after.is_some_and(|n| readbufs > n) {
                    return;
                }
                let bytes: usize = words[2].parse().unwrap();
                let m = u32::from_str_radix(&mask, 16).unwrap_or(3);
                let per = if m == 1 {
                    2
                } else {
                    2 * m.count_ones() as usize
                };
                let samples = bytes / per;
                let skip = |k: &mut i16, n: u64| *k = ((*k as u64 + n) % 1000) as i16;
                let stall = b.stall_ms.swap(0, Ordering::Relaxed);
                if stall > 0 {
                    std::thread::sleep(Duration::from_millis(stall));
                }
                if o.overflow_reg && !o.fast && stall > 0 {
                    // What the kernel buffers could not hold while the answer was late is gone. Only a stall that the
                    // test asked for overflows: a client that a loaded machine slows down loses nothing here.
                    let due = (pace.started.elapsed().as_secs_f64() * rate_of(b)) as u64;
                    let room = u64::from(kbuffers) * samples as u64;
                    let behind = due.saturating_sub(pace.sent + samples as u64);
                    if behind > room {
                        let gone = behind - room;
                        skip(&mut k, gone);
                        pace.sent += gone;
                        b.rx_overflowed.fetch_add(gone, Ordering::Relaxed);
                        b.capture_overflow.store(true, Ordering::Relaxed);
                    }
                }
                let failing = {
                    let left = b.fail_readbufs.load(Ordering::Relaxed);
                    left > 0 && {
                        b.fail_readbufs.store(left - 1, Ordering::Relaxed);
                        true
                    }
                };
                if failing {
                    if !o.fast {
                        pace.wait(samples, rate_of(b));
                    }
                    skip(&mut k, samples as u64);
                    b.rx_withheld.fetch_add(samples as u64, Ordering::Relaxed);
                    if !reply(&mut w, "-110\n") {
                        return;
                    }
                    continue;
                }
                let levelled = b.level_dbfs.lock().unwrap().is_some();
                let payload: &[u8] = if o.fast && !levelled {
                    if fast_cache.as_ref().map(|c| c.0) != Some(bytes) {
                        rx_data(m, samples, &mut k, &mut data);
                        fast_cache = Some((bytes, data.clone()));
                    }
                    b.rx_samples.fetch_add(samples as u64, Ordering::Relaxed);
                    &fast_cache.as_ref().unwrap().1
                } else {
                    if !o.fast {
                        pace.wait(samples, rate_of(b));
                    }
                    rx_fill(b, m, samples, &mut k, &mut pos, &mut data);
                    &data
                };
                // In two chunks, with the mask on the first only.
                let half = payload.len() / 2 / per * per;
                if b.garble_next.swap(false, Ordering::SeqCst) {
                    garbled = true;
                    let mut out = format!("{half}x\n{mask}\n").into_bytes();
                    out.extend(&payload[..half]);
                    w.write_all(&out).is_ok()
                } else {
                    let mut out = format!("{half}\n{mask}\n").into_bytes();
                    out.extend(&payload[..half]);
                    out.extend(format!("{}\n", payload.len() - half).as_bytes());
                    out.extend(&payload[half..]);
                    let stall = b.stall_mid_ms.swap(0, Ordering::Relaxed);
                    send_answer(&mut w, &mut r, &out, b, &o, stall)
                }
            }
            "WRITEBUF" => {
                let bytes: usize = words[2].parse().unwrap();
                if !reply(&mut w, "0\n") {
                    return;
                }
                let mut v = vec![0u8; bytes];
                if r.read_exact(&mut v).is_err() {
                    return;
                }
                b.push(format!("WRITEBUF {} {bytes}", words[1]));
                b.tx_bytes.lock().unwrap().extend_from_slice(&v);
                // The enabled scan channels, in the order of their samples.
                let m = u32::from_str_radix(&mask, 16).unwrap_or(3);
                let enabled: Vec<usize> = (0..4).filter(|ch| m >> ch & 1 == 1).collect();
                let stride = 2 * enabled.len().max(1);
                let mut any_signal = false;
                for pair in 0..2 {
                    let at = |ch: usize| enabled.iter().position(|&e| e == ch);
                    let (Some(i), Some(q)) = (at(2 * pair), at(2 * pair + 1)) else {
                        continue;
                    };
                    let signal = v
                        .chunks_exact(stride)
                        .any(|s| s[2 * i..2 * i + 2] != [0, 0] || s[2 * q..2 * q + 2] != [0, 0]);
                    any_signal |= signal;
                    if signal {
                        let attenuator = if o.tx_crossed { 1 - pair } else { pair };
                        b.tx_played.lock().unwrap().push(Played {
                            pair,
                            attenuator,
                            gain_db: b.tx_gain(attenuator),
                        });
                    }
                }
                let rate: f64 = b
                    .attrs
                    .lock()
                    .unwrap()
                    .get("iio:device1 OUTPUT voltage0 sampling_frequency")
                    .and_then(|r| r.parse().ok())
                    .unwrap_or(1e6);
                if let Some(max) = o.tx_max_bytes_per_s {
                    // The transfer arrives at `max` bytes a second and is played at the sample rate: the converter
                    // has nothing to play for the rest of the time.
                    let need = stride as f64 * rate;
                    if need > max {
                        let dry = (bytes / stride) as f64 * (need / max - 1.0);
                        b.tx_gap.fetch_add(dry as u64, Ordering::Relaxed);
                    }
                }
                if o.tx_plays {
                    let time = Duration::from_secs_f64((bytes / stride) as f64 / rate);
                    if let Some((n, factor)) = o.tx_slow_start {
                        if last_tx.is_none_or(|t| t.elapsed() > Duration::from_millis(100)) {
                            slow_left = n;
                        }
                        if slow_left > 0 {
                            slow_left -= 1;
                            std::thread::sleep(time.mul_f64(factor));
                        }
                    }
                    let now = Instant::now();
                    let end = {
                        let mut p = b.tx_play.lock().unwrap();
                        let (until, on_air, played) = *p;
                        let start = match until {
                            Some(u) if u > now => u,
                            Some(u) => {
                                if on_air && played {
                                    let gap = (now - u).as_secs_f64() * rate;
                                    b.tx_gap.fetch_add(gap as u64, Ordering::Relaxed);
                                    if any_signal && b.tx_last_signal.load(Ordering::Relaxed) {
                                        b.tx_gap_in_signal.fetch_add(gap as u64, Ordering::Relaxed);
                                    }
                                }
                                now
                            }
                            None => now,
                        };
                        *p = (Some(start + time), on_air, on_air);
                        start + time
                    };
                    // The kernel of the radio holds the transfers that the connection set: the next one waits.
                    let wait = end
                        .saturating_duration_since(now)
                        .saturating_sub(kbuffers * time);
                    std::thread::sleep(wait);
                    last_tx = Some(Instant::now());
                }
                b.tx_last_signal.store(any_signal, Ordering::Relaxed);
                reply(&mut w, &format!("{bytes}\n"))
            }
            "EXIT" => return,
            _ => reply(&mut w, "-22\n"),
        };
        if !ok {
            return;
        }
    }
}

/// The binary protocol, receive side: open, blocks, enable, transfers, close.
fn serve_binary(mut r: BufReader<Box<dyn Conn>>, mut w: Box<dyn Conn>, b: &Board, o: &FakeOpts) {
    let mut k: i16 = 0;
    let mut pos = 0;
    let mut mask = 0u32;
    let mut sizes: HashMap<u16, usize> = HashMap::new();
    let mut pace = Pace {
        started: Instant::now(),
        sent: 0,
    };
    let mut data = Vec::new();
    let mut fast: Option<Vec<u8>> = None;
    let mut transfers = 0;
    let send = |w: &mut Box<dyn Conn>, client: u16, code: i32, payload: &[u8]| {
        let h = Header {
            client,
            op: op::RESPONSE,
            dev: 0,
            code,
        };
        let mut out = h.bytes().to_vec();
        out.extend_from_slice(payload);
        w.write_all(&out).is_ok()
    };
    loop {
        let mut hb = [0u8; 8];
        if r.read_exact(&mut hb).is_err() {
            return;
        }
        let h = Header::parse(&hb);
        let ok = match h.op {
            op::OPEN_BUFFER => {
                let mut m = [0u8; 4];
                r.read_exact(&mut m).unwrap();
                mask = u32::from_le_bytes(m);
                b.push(format!("B OPEN dev{} mask {mask:x}", h.dev));
                *b.streams.lock().unwrap() += 1;
                send(&mut w, h.client, 4, &mask.to_le_bytes())
            }
            op::CREATE_BLOCK => {
                let mut s = [0u8; 8];
                r.read_exact(&mut s).unwrap();
                let size = u64::from_le_bytes(s) as usize;
                sizes.insert((h.code >> 16) as u16, size);
                b.push(format!("B BLOCK {} {size}", h.code >> 16));
                send(&mut w, h.client, 0, &[])
            }
            op::ENABLE_BUFFER | op::DISABLE_BUFFER | op::CLOSE_BUFFER | op::FREE_BLOCK => {
                b.push(format!("B OP {}", h.op));
                send(&mut w, h.client, 0, &[])
            }
            op::TRANSFER_BLOCK => {
                let mut s = [0u8; 8];
                r.read_exact(&mut s).unwrap();
                transfers += 1;
                if o.drop_stream_after
                    .is_some_and(|n| transfers > n + sizes.len())
                {
                    return;
                }
                let bytes = u64::from_le_bytes(s) as usize;
                let per = 2 * mask.count_ones() as usize;
                let samples = bytes / per;
                let levelled = b.level_dbfs.lock().unwrap().is_some();
                let payload: &[u8] = if o.fast && !levelled {
                    if fast.as_ref().map(|f| f.len()) != Some(bytes) {
                        rx_data(mask, samples, &mut k, &mut data);
                        fast = Some(data.clone());
                    }
                    b.rx_samples.fetch_add(samples as u64, Ordering::Relaxed);
                    fast.as_ref().unwrap()
                } else {
                    if !o.fast {
                        pace.wait(samples, rate_of(b));
                    }
                    rx_fill(b, mask, samples, &mut k, &mut pos, &mut data);
                    &data
                };
                send(&mut w, h.client, payload.len() as i32, payload)
            }
            _ => send(&mut w, h.client, -22, &[]),
        };
        if !ok {
            return;
        }
    }
}
