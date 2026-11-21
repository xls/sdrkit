//! ARDOP: the frame type table, the six-bit packing and the frame bodies, a receiver that lists the frames and the stations
//! it hears, the generator of a test script, the frame sender and the ARQ link ([`tx`], [`link`]).
//!
//! Standard: R. Muething KN6KB, "ARDOP specification" (the frame types of appendix B, the leader, the type coding, CRC and
//! Reed-Solomon of each frame).

use crate::decoder::{
    self, flush_logs, put_text, text_into, Emit, HeardCommand, HfGenerator, Modulator,
};
use crate::typed::{
    ArdopFields, ArdopFrameEvent, ArdopFrameRow, ArdopStationRow, StationArgs, ARDOP_FRAME,
    ARDOP_FRAMES, ARDOP_STATIONS, REMOVE,
};
use crate::util::db;
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_fec::rs::{codes, Rs};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::Record;
use sdrkit_core::{
    Complex32, ConfigError, GenConfig, Generator, LockState, SeriesId, SignalInfo, Status, Values,
};
use sdrkit_graph::{no_values, Format, Item, Kept, Outbox, Port, RateFrom, Timed};
use sdrkit_util_bits::crc::cyclic::ardop_crc16;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, ObjectTable, Seen, Stamp};
use sdrkit_util_text::store::TextStore;
use std::f64::consts::TAU;

pub mod link;
pub mod tx;

/// What a frame type is.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameType {
    /// The type byte.
    pub code: u8,
    /// Its name ("ConReq500M", "4FSK.200.50S.E").
    pub name: String,
    /// What kind of frame ("connect request", "ID", "data" ...).
    pub kind: &'static str,
    /// Modulation of the body.
    pub modulation: &'static str,
    /// Baud of the body (0 for none).
    pub baud: u32,
    /// Band, Hz.
    pub band: u32,
    /// Data bytes and Reed-Solomon bytes per carrier (data frames).
    pub data: usize,
    /// See `data`.
    pub rs: usize,
    /// Carriers.
    pub carriers: u32,
    /// The body is read here.
    pub readable: bool,
}

const DATA: [(u8, &str, &str, u32, u32, u32, usize, usize, bool); 18] = [
    (0x40, "4PSK.200.100", "4PSK", 1, 100, 200, 64, 32, false),
    (0x42, "4PSK.200.100S", "4PSK", 1, 100, 200, 16, 8, false),
    (0x44, "8PSK.200.100", "8PSK", 1, 100, 200, 108, 36, false),
    (0x46, "16QAM.200.100", "16QAM", 1, 100, 200, 128, 64, false),
    (0x48, "4FSK.200.50S", "4FSK", 1, 50, 200, 16, 4, true),
    (0x4A, "4FSK.500.100", "4FSK", 1, 100, 500, 64, 16, true),
    (0x4C, "4FSK.500.100S", "4FSK", 1, 100, 500, 32, 8, true),
    (0x50, "4PSK.500.100", "4PSK", 2, 100, 500, 64, 32, false),
    (0x52, "8PSK.500.100", "8PSK", 2, 100, 500, 108, 36, false),
    (0x54, "16QAM.500.100", "16QAM", 2, 100, 500, 128, 64, false),
    (0x60, "4PSK.1000.100", "4PSK", 4, 100, 1000, 64, 32, false),
    (0x62, "8PSK.1000.100", "8PSK", 4, 100, 1000, 108, 36, false),
    (
        0x64,
        "16QAM.1000.100",
        "16QAM",
        4,
        100,
        1000,
        128,
        64,
        false,
    ),
    (0x70, "4PSK.2000.100", "4PSK", 8, 100, 2000, 64, 32, false),
    (0x72, "8PSK.2000.100", "8PSK", 8, 100, 2000, 108, 36, false),
    (
        0x74,
        "16QAM.2000.100",
        "16QAM",
        8,
        100,
        2000,
        128,
        64,
        false,
    ),
    (0x7A, "4FSK.2000.600", "4FSK", 1, 600, 2000, 600, 150, false),
    (0x7C, "4FSK.2000.600S", "4FSK", 1, 600, 2000, 200, 50, false),
];

fn control(code: u8, name: String, kind: &'static str, readable: bool) -> FrameType {
    FrameType {
        code,
        name,
        kind,
        modulation: "4FSK",
        baud: if readable { 50 } else { 0 },
        band: 200,
        data: 0,
        rs: 0,
        carriers: 1,
        readable,
    }
}

/// The frame type of a type byte (`None`: not a frame type).
pub fn frame_type(code: u8) -> Option<FrameType> {
    let bw = ["200", "500", "1000", "2000"];
    Some(match code {
        0x00..=0x1F => control(code, "DataNAK".into(), "nak", false),
        0xE0..=0xFF => control(code, "DataACK".into(), "ack", false),
        0x23 => control(code, "BREAK".into(), "break", false),
        0x24 => control(code, "IDLE".into(), "idle", false),
        0x29 => control(code, "DISC".into(), "disconnect", false),
        0x2C => control(code, "END".into(), "end", false),
        0x2D => control(code, "ConRejBusy".into(), "connect reject", false),
        0x2E => control(code, "ConRejBW".into(), "connect reject", false),
        0x30 => control(code, "IDFrame".into(), "ID", true),
        0x31..=0x34 => control(
            code,
            format!("ConReq{}M", bw[(code - 0x31) as usize]),
            "connect request",
            true,
        ),
        0x35..=0x38 => control(
            code,
            format!("ConReq{}F", bw[(code - 0x35) as usize]),
            "connect request",
            true,
        ),
        0x39..=0x3C => control(
            code,
            format!("ConAck{}", bw[(code - 0x39) as usize]),
            "connect ack",
            true,
        ),
        0x3D => control(code, "PingAck".into(), "ping ack", true),
        0x3E => control(code, "Ping".into(), "ping", true),
        _ => {
            let even = code & !1;
            let d = DATA.iter().find(|d| d.0 == even)?;
            FrameType {
                code,
                name: format!("{}.{}", d.1, if code & 1 == 0 { "E" } else { "O" }),
                kind: "data",
                modulation: d.2,
                baud: d.4,
                band: d.5,
                data: d.6,
                rs: d.7,
                carriers: d.3,
                readable: d.8,
            }
        }
    })
}

/// Every frame type byte.
pub fn all_types() -> Vec<u8> {
    (0..=255u8).filter(|&c| frame_type(c).is_some()).collect()
}

/// The check symbol of a type byte: 1 XOR its four 2-bit groups (a 4-ary symbol, not a bit parity).
pub fn type_check(b: u8) -> u8 {
    1 ^ (b >> 6) ^ ((b >> 4) & 3) ^ ((b >> 2) & 3) ^ (b & 3)
}

/// The ten frame-type symbols for a type and session id.
pub fn type_symbols(t: u8, session: u8) -> [u8; 10] {
    let p = type_check(t);
    let b2 = t ^ session;
    [
        t >> 6,
        (t >> 4) & 3,
        (t >> 2) & 3,
        t & 3,
        p,
        b2 >> 6,
        (b2 >> 4) & 3,
        (b2 >> 2) & 3,
        b2 & 3,
        p,
    ]
}

/// The ARDOP frame check of `data`: a 16-bit register started at all ones, each bit (most significant first) shifted in at the
/// bottom, XOR 0x8810 when a 1 leaves the top. The effective generator is 0x18810 (bytes 1 to 12 give 0xD58C).
pub fn frame_check(data: &[u8]) -> u16 {
    ardop_crc16(data)
}

/// The Reed-Solomon codes of the ARDOP frames, made once for each word length and parity count. ARDOP uses RS over GF(256) with
/// the polynomial 0x11D and the roots alpha^(255 - n) to alpha^254, the parity after the data.
#[derive(Debug, Default)]
pub struct RsCodes {
    codes: Vec<((usize, usize), Rs)>,
}

impl RsCodes {
    /// No code made yet.
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&mut self, len: usize, nroots: usize) -> Option<&mut Rs> {
        let i = match self.codes.iter().position(|(k, _)| *k == (len, nroots)) {
            Some(i) => i,
            None => {
                let rs = Rs::new(codes::ardop(len as u16, nroots as u16)).ok()?;
                self.codes.push(((len, nroots), rs));
                self.codes.len() - 1
            }
        };
        Some(&mut self.codes[i].1)
    }

    /// The `nroots` parity bytes of `data`.
    pub fn parity(&mut self, data: &[u8], nroots: usize) -> Vec<u8> {
        let mut p = vec![0u8; nroots];
        if let Some(rs) = self.get(data.len() + nroots, nroots) {
            let _ = rs.encode(data, &mut p);
        }
        p
    }

    /// Corrects a received word (the data, then `nroots` parity bytes) in place. Returns the symbols corrected, or `None` when the
    /// word cannot be corrected (it is then left as received).
    pub fn correct(&mut self, word: &mut [u8], nroots: usize) -> Option<usize> {
        let rs = self.get(word.len(), nroots)?;
        rs.decode(word, &[]).ok().map(|d| d.corrected as usize)
    }
}

/// Packs up to eight characters (space to underscore, lower case folded up, others as space) into six bytes.
pub fn pack6(s: &str) -> [u8; 6] {
    let mut v = [0u8; 8];
    for (i, c) in s.bytes().take(8).enumerate() {
        let c = c.to_ascii_uppercase();
        v[i] = if (0x20..=0x5F).contains(&c) {
            c - 0x20
        } else {
            0
        };
    }
    let mut out = [0u8; 6];
    for g in 0..2 {
        let x = ((v[4 * g] as u32) << 18)
            | ((v[4 * g + 1] as u32) << 12)
            | ((v[4 * g + 2] as u32) << 6)
            | v[4 * g + 3] as u32;
        out[3 * g] = (x >> 16) as u8;
        out[3 * g + 1] = (x >> 8) as u8;
        out[3 * g + 2] = x as u8;
    }
    out
}

/// Unpacks six bytes to text, trailing spaces removed.
pub fn unpack6(b: &[u8]) -> String {
    let mut s = String::new();
    for g in 0..2 {
        let x = ((b[3 * g] as u32) << 16) | ((b[3 * g + 1] as u32) << 8) | b[3 * g + 2] as u32;
        for k in 0..4 {
            s.push((((x >> (18 - 6 * k)) & 63) as u8 + 0x20) as char);
        }
    }
    s.trim_end().to_string()
}

/// A decoded frame (`ardop.frame`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArdopFrame {
    /// Wall-clock time (Unix seconds) when known, else 0.
    pub utc: f64,
    /// Signal time, seconds.
    pub time: f64,
    /// The type byte.
    pub code: u8,
    /// Name.
    pub name: String,
    /// Kind.
    pub kind: String,
    /// Modulation.
    pub modulation: String,
    /// Baud.
    pub baud: u32,
    /// Band.
    pub band: u32,
    /// Session id (0xFF unconnected).
    pub session: u8,
    /// Sender.
    pub from: Option<String>,
    /// Addressee.
    pub to: Option<String>,
    /// Locator (ID frames).
    pub locator: Option<String>,
    /// Printable payload.
    pub text: Option<String>,
    /// Payload as hex.
    pub hex: Option<String>,
    /// The data bytes of a good data frame.
    pub bytes: Vec<u8>,
    /// Quality (ACK, NAK, PingAck).
    pub quality: Option<u32>,
    /// PingAck SNR, dB.
    pub snr_db: Option<i32>,
    /// ConAck leader, ms.
    pub leader_ms: Option<u32>,
    /// The header's check symbols agree.
    pub header_ok: bool,
    /// The body was read.
    pub body_read: bool,
    /// The body checked (RS / CRC / votes).
    pub body_ok: bool,
    /// Reed-Solomon corrections.
    pub rs_corrections: u32,
    /// Tone SNR, dB.
    pub tone_snr_db: f32,
    /// Tone offset, Hz.
    pub tone_offset_hz: f32,
}

impl Item for ArdopFrame {
    const KIND: &'static str = "ardop.frame";
}

/// The counts of the receiver.
#[derive(Clone, Debug, Default)]
struct Counts {
    leaders: u64,
    good_headers: u64,
    unreadable: u64,
    data_ok: u64,
    data_bad: u64,
    rs_corrections: u64,
    control: u64,
    other: u64,
    levels: Vec<f32>,
}

impl Shaped for ArdopFrame {
    type Row = ArdopFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ArdopFrameRow {
            time_s: self.time,
            r#type: i64::from(self.code),
            name: &self.name,
            kind: &self.kind,
            modulation: &self.modulation,
            baud: i64::from(self.baud),
            session: i64::from(self.session),
            from: self.from.as_deref(),
            to: self.to.as_deref(),
            locator: self.locator.as_deref(),
            text: self.text.as_deref(),
            quality: self.quality.map(i64::from),
            snr_db: self.snr_db.map(f64::from),
            leader_ms: self.leader_ms.map(i64::from),
            header_ok: self.header_ok,
            body_read: self.body_read,
            body_ok: self.body_ok,
            rs_corrections: i64::from(self.rs_corrections),
            tone_offset_hz: Hertz(f64::from(self.tone_offset_hz)),
            tone_snr_db: f64::from(self.tone_snr_db),
        });
    }
}

/// A station heard.
#[derive(Clone, Debug, Default)]
struct Station {
    call: String,
    locator: Option<String>,
    last_type: String,
    called: Option<String>,
}

impl Shaped for Station {
    type Row = ArdopStationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ArdopStationRow {
            callsign: &self.call,
            locator: self.locator.as_deref(),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
            last_type: &self.last_type,
            called: self.called.as_deref(),
        });
    }
}

/// The leader's two tones, Hz from 1500.
const LEADER: [f64; 2] = [-25.0, 25.0];
/// Trial tuning errors.
const TRIALS: [f64; 7] = [-45.0, -30.0, -15.0, 0.0, 15.0, 30.0, 45.0];
const WIN: usize = 160;
const MSS: u64 = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Idle,
    /// A leader is on. The lowest score since it started, and when.
    Leader {
        min: f32,
        at: u64,
        low: bool,
    },
    /// Reading a frame whose type symbols start at sample `t0`. Nothing more until `until`.
    Frame {
        t0: u64,
    },
    /// Skipping a frame's body.
    Skip {
        until: u64,
    },
}

/// The receiver.
pub struct ArdopDecoder {
    nco: Nco,
    lpf: Fir<Complex32>,
    hist: SampleRing<Complex32>,
    t: u64,
    kernels: Vec<Vec<Complex32>>,
    run: u32,
    best_trial: usize,
    state: State,
    frames: LogRing<ArdopFrame>,
    new_frames: Vec<ArdopFrame>,
    stations: ObjectTable<String, Station>,
    det: Counts,
    rs: RsCodes,
    dial: f64,
    store: TextStore,
    logs: Vec<String>,
    status: Status,
    text: String,
    shown: Option<u64>,
    levels: Option<SeriesId>,
    last_frame: f64,
    level_acc: f64,
    level_n: u64,
    offset: f64,
    snr: f32,
    reading: bool,
    refined: Option<u64>,
    hdr: Option<Option<(FrameType, u8, bool, f32)>>,
}

impl Default for ArdopDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ArdopDecoder {
    /// A receiver.
    pub fn new() -> Self {
        let mut kernels = vec![];
        for &tr in &TRIALS {
            for &l in &LEADER {
                kernels.push(
                    (0..WIN)
                        .map(|n| {
                            let p = -TAU * (l + tr) * n as f64 / 8000.0;
                            Complex32::new(p.cos() as f32, p.sin() as f32)
                        })
                        .collect(),
                );
            }
        }
        ArdopDecoder {
            nco: Nco::new(8000.0, -1500.0),
            lpf: Fir::new(
                &lowpass(8000.0, 320.0, 450.0, 50.0).expect("the filter settings are valid"),
            ),
            hist: SampleRing::new(8000 * 5).expect("a ring of at least one sample"),
            t: 0,
            kernels,
            run: 0,
            best_trial: 3,
            state: State::Idle,
            frames: LogRing::new(300),
            new_frames: Vec::new(),
            stations: ObjectTable::new(100),
            det: Counts::default(),
            rs: RsCodes::new(),
            dial: 0.0,
            store: decoder::store(4000, 1),
            logs: Vec::new(),
            status: Status::default(),
            text: String::new(),
            shown: None,
            levels: None,
            last_frame: f64::NEG_INFINITY,
            level_acc: 0.0,
            level_n: 0,
            offset: 0.0,
            snr: 0.0,
            reading: false,
            refined: None,
            hdr: None,
        }
    }

    /// The samples `a .. a + n` (absolute indices), when still held.
    fn window(&self, a: u64, n: usize) -> Option<&[Complex32]> {
        self.hist.window(a, n)
    }

    /// The power of a tone `f` Hz from 1500 over the samples `a .. a + n`.
    fn tone(&self, a: u64, n: usize, f: f64) -> f32 {
        let Some(w) = self.window(a, n) else {
            return 0.0;
        };
        let r = -TAU * f / 8000.0;
        let rot = Complex32::new(r.cos() as f32, r.sin() as f32);
        let mut ph = Complex32::new(1.0, 0.0);
        let mut s = Complex32::default();
        for &x in w {
            s += x * ph;
            ph *= rot;
        }
        s.norm_sqr() / n as f32
    }

    /// The leader score of the 20 ms ending now: the share of the power in the two leader tones, at the best trial offset.
    fn leader_score(&self) -> (f32, usize) {
        let Some(w) = self.window(self.t - WIN as u64, WIN) else {
            return (0.0, 3);
        };
        let e: f32 = w.iter().map(|x| x.norm_sqr()).sum();
        if e <= 0.0 {
            return (0.0, 3);
        }
        let mut best = (0.0f32, 3usize);
        let trials: &[usize] = match self.state {
            State::Leader { .. } => &[self.best_trial][..],
            _ => &[0, 1, 2, 3, 4, 5, 6][..],
        };
        for &t in trials {
            let mut p = [0.0f32; 2];
            for (k, pk) in p.iter_mut().enumerate() {
                let kern = &self.kernels[2 * t + k];
                let mut s = Complex32::default();
                for (x, c) in w.iter().zip(kern) {
                    s += *x * *c;
                }
                *pk = s.norm_sqr();
            }
            // Both tones, about equally strong.
            let sc = 2.0 * p[0].min(p[1]) / (WIN as f32 * e);
            if sc > best.0 {
                best = (sc, t);
            }
        }
        best
    }

    /// The tuning error from the leader before sample `end`: the phase turn of each tone between two halves.
    fn leader_offset(&self, end: u64) -> f64 {
        let n = 320usize;
        let a = end.saturating_sub(2 * n as u64);
        let tr = TRIALS[self.best_trial];
        let mut df = 0.0;
        for &l in &LEADER {
            let f = l + tr;
            let corr = |start: u64| -> Complex32 {
                let Some(w) = self.window(start, n) else {
                    return Complex32::default();
                };
                let mut s = Complex32::default();
                for (i, &x) in w.iter().enumerate() {
                    let p = -TAU * f * (start + i as u64) as f64 / 8000.0;
                    s += x * Complex32::new(p.cos() as f32, p.sin() as f32);
                }
                s
            };
            let (c1, c2) = (corr(a), corr(a + n as u64));
            let ang = (c2 * c1.conj()).arg() as f64;
            df += f + ang / (TAU * n as f64 / 8000.0) - l;
        }
        df / 2.0
    }

    /// The four tones' power for a 4FSK symbol starting at `a`, `n` samples long.
    fn fsk(&self, a: u64, n: usize, base: f64, spacing: f64) -> [f32; 4] {
        let m = n / 8;
        let mut e = [0.0; 4];
        for (k, v) in e.iter_mut().enumerate() {
            *v = self.tone(
                a + m as u64,
                n - 2 * m,
                base + k as f64 * spacing + self.offset,
            );
        }
        e
    }

    fn header(&mut self, t0: u64) -> Option<(FrameType, u8, bool, f32)> {
        let mut soft = [[0.0f32; 4]; 10];
        let mut sig = 0.0;
        let mut noise = 0.0;
        for (i, sv) in soft.iter_mut().enumerate() {
            let e = self.fsk(t0 + i as u64 * 160, 160, -75.0, 50.0);
            let tot: f32 = e.iter().sum::<f32>().max(1e-20);
            let mx = e.iter().cloned().fold(0.0, f32::max);
            sig += mx;
            noise += (tot - mx) / 3.0;
            for k in 0..4 {
                sv[k] = e[k] / tot;
            }
        }
        let snr = 10.0 * (sig / noise.max(1e-20)).log10();
        let hard: Vec<u8> = soft
            .iter()
            .map(|v| {
                (0..4)
                    .max_by(|&a, &b| v[a].total_cmp(&v[b]))
                    .expect("the range is not empty") as u8
            })
            .collect();
        // The type: the table entry nearest in the soft sense over the first byte and both parity symbols.
        let mut best: Option<(f32, u8)> = None;
        for t in all_types() {
            let sy = type_symbols(t, 0);
            let d: f32 = [0usize, 1, 2, 3, 4, 9]
                .iter()
                .map(|&i| 1.0 - soft[i][sy[i] as usize])
                .sum();
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, t));
            }
        }
        let (d, t) = best?;
        if d > 3.0 {
            return None;
        }
        let b2 = (hard[5] << 6) | (hard[6] << 4) | (hard[7] << 2) | hard[8];
        let p = type_check(t);
        let ok = hard[4] == p && hard[9] == p && hard[..4] == type_symbols(t, 0)[..4];
        Some((frame_type(t)?, t ^ b2, ok, snr))
    }

    fn read_bytes(&self, a: u64, nbytes: usize, baud: u32) -> Vec<u8> {
        let n = (8000 / baud) as usize;
        let (base, sp) = if baud == 50 {
            (-75.0, 50.0)
        } else {
            (-150.0, 100.0)
        };
        let mut out = Vec::with_capacity(nbytes);
        for b in 0..nbytes {
            let mut v = 0u8;
            for k in 0..4 {
                let s = a + ((4 * b + k) * n) as u64;
                let e = self.fsk(s, n, base, sp);
                let sym = (0..4)
                    .max_by(|&x, &y| e[x].total_cmp(&e[y]))
                    .expect("the range is not empty") as u8;
                v = (v << 2) | sym;
            }
            out.push(v);
        }
        out
    }

    fn body_len(ft: &FrameType) -> usize {
        match ft.code {
            0x30..=0x38 | 0x3E => 16,
            0x39..=0x3D => 3,
            _ if ft.kind == "data" && ft.readable => 1 + ft.data + 2 + ft.rs,
            _ => 0,
        }
    }

    /// How clean the ten type symbols are when they start at `t0`: the mean share of each symbol's power in its strongest tone.
    fn header_clean(&self, t0: u64) -> f32 {
        let mut c = 0.0;
        for i in 0..10u64 {
            let e = self.fsk(t0 + i * 160, 160, -75.0, 50.0);
            let tot: f32 = e.iter().sum::<f32>().max(1e-20);
            c += e.iter().cloned().fold(0.0, f32::max) / tot;
        }
        c / 10.0
    }

    fn frame(&mut self, t0: u64, out: &mut Outbox) -> Option<u64> {
        if self.t < t0 + 10 * 160 + 120 {
            return None;
        }
        // The symbol timing: the start that produces the cleanest symbols near the estimate (once per frame).
        let t0 = match self.refined {
            Some(r) => r,
            None => {
                let mut best = (f32::MIN, t0);
                let mut c = t0.saturating_sub(96);
                while c <= t0 + 96 {
                    let v = self.header_clean(c);
                    if v > best.0 {
                        best = (v, c);
                    }
                    c += 4;
                }
                let mut f = best.1.saturating_sub(3);
                while f <= best.1 + 3 {
                    let v = self.header_clean(f);
                    if v > best.0 {
                        best = (v, f);
                    }
                    f += 1;
                }
                self.refined = Some(best.1);
                best.1
            }
        };
        let hdr = match self.hdr.clone() {
            Some(h) => h,
            None => {
                let mut h = self.header(t0);
                if !h.as_ref().is_some_and(|x| x.2) {
                    // A symbol early or late: the start whose parity checks.
                    for alt in [t0.saturating_sub(160), t0 + 160] {
                        let a = self.header(alt);
                        if a.as_ref().is_some_and(|x| x.2) {
                            h = a;
                            self.refined = Some(alt);
                            break;
                        }
                    }
                }
                self.hdr = Some(h.clone());
                h
            }
        };
        let t0 = self.refined.unwrap_or(t0);
        let header_end = t0 + 10 * 160;
        let Some((ft, session, ok, snr)) = hdr else {
            self.det.unreadable += 1;
            self.logs.push("leader heard, no frame".into());
            self.refined = None;
            self.hdr = None;
            return Some(header_end);
        };
        let blen = Self::body_len(&ft);
        let sym = 8000u32.checked_div(ft.baud).unwrap_or(0) as u64;
        let body_end = header_end + blen as u64 * 4 * sym;
        if ft.readable && self.t < body_end + 8 {
            return None;
        }
        let time = t0 as f64 / 8000.0;
        let mut f = ArdopFrame {
            time,
            code: ft.code,
            name: ft.name.clone(),
            kind: ft.kind.into(),
            modulation: ft.modulation.into(),
            baud: ft.baud,
            band: ft.band,
            session,
            header_ok: ok,
            tone_snr_db: snr,
            tone_offset_hz: self.offset as f32,
            ..Default::default()
        };
        if matches!(ft.code, 0x00..=0x1F | 0xE0..=0xFF) {
            f.quality = Some(38 + 2 * (ft.code & 0x1F) as u32);
        }
        if ok {
            self.det.good_headers += 1;
        }
        if ft.kind == "data" && !ft.readable {
            f.text = Some("header only (PSK, QAM or 600 baud data)".into());
            self.det.other += 1;
        }
        if ft.readable && blen > 0 {
            f.body_read = true;
            let mut b = self.read_bytes(header_end, blen, ft.baud);
            match ft.code {
                0x30..=0x38 | 0x3E => {
                    if let Some(c) = self.rs.correct(&mut b, 4) {
                        f.body_ok = true;
                        f.rs_corrections = c as u32;
                        self.det.rs_corrections += c as u64;
                        let a = unpack6(&b[0..6]);
                        let z = unpack6(&b[6..12]);
                        f.from = Some(a);
                        if ft.code == 0x30 {
                            f.locator = Some(z);
                        } else {
                            f.to = Some(z);
                        }
                    }
                    self.det.control += 1;
                }
                0x39..=0x3C => {
                    if let Some(v) = vote(&b) {
                        f.body_ok = true;
                        f.leader_ms = Some(v as u32 * 10);
                    }
                    self.det.control += 1;
                }
                0x3D => {
                    if let Some(v) = vote(&b) {
                        f.body_ok = true;
                        f.snr_db = Some(((v >> 3) as i32) - 10);
                        f.quality = Some(30 + 10 * (v & 7) as u32);
                    }
                    self.det.control += 1;
                }
                _ => {
                    let k = blen - ft.rs;
                    let fixed = self.rs.correct(&mut b, ft.rs);
                    let count = b[0] as usize;
                    let crc = frame_check(&b[..k - 2]);
                    let crc_ok = b[k - 2] == (crc >> 8) as u8 && b[k - 1] == (crc as u8) ^ ft.code;
                    if fixed.is_some() && crc_ok && count <= ft.data {
                        f.body_ok = true;
                        f.rs_corrections = fixed.unwrap_or(0) as u32;
                        self.det.rs_corrections += f.rs_corrections as u64;
                        let data = &b[1..1 + count];
                        f.hex = Some(data.iter().map(|x| format!("{x:02X}")).collect());
                        f.bytes = data.to_vec();
                        let txt: String = data
                            .iter()
                            .map(|&c| {
                                if (32..127).contains(&c) {
                                    c as char
                                } else {
                                    '.'
                                }
                            })
                            .collect();
                        for &c in data {
                            self.store.push_byte(c, time, self.dial + 1500.0, 0);
                        }
                        self.store.push('\n', time, self.dial + 1500.0, 0);
                        f.text = Some(txt);
                        self.det.data_ok += 1;
                    } else {
                        self.det.data_bad += 1;
                    }
                }
            }
        } else if !matches!(ft.kind, "data") {
            self.det.control += 1;
        }
        self.note_station(&f);
        self.logs.push(format!(
            "{:.1} s: {} {}{}",
            time,
            f.name,
            f.from.as_deref().unwrap_or(""),
            f.to.as_deref()
                .map(|t| format!(" > {t}"))
                .unwrap_or_default()
        ));
        let freq = self.dial + 1500.0 + self.offset;
        out.event(ARDOP_FRAME.event(
            time,
            freq,
            &ArdopFrameEvent {
                source: f.from.as_deref(),
                target: f.to.as_deref(),
                family: Family::Message,
                name: &f.name,
                r#type: i64::from(f.code),
            },
        ));
        self.last_frame = time;
        self.snr = snr;
        self.frames.push(f.clone());
        self.new_frames.push(f);
        self.refined = None;
        self.hdr = None;
        Some(if ft.readable { body_end } else { header_end })
    }

    fn note_station(&mut self, f: &ArdopFrame) {
        let Some(call) = f.from.clone() else { return };
        if let Ok(u) = self
            .stations
            .upsert(&call, Stamp::at(f.time), Station::default)
        {
            let s = u.row;
            if s.call != call {
                s.call.clone_from(&call);
            }
            s.last_type = f.name.clone();
            if f.locator.is_some() {
                s.locator = f.locator.clone();
            }
            if f.to.is_some() {
                s.called = f.to.clone();
            }
        }
    }

    fn every_ms(&mut self, out: &mut Outbox) {
        match self.state {
            State::Idle => {
                let (sc, tr) = self.leader_score();
                if sc > 0.5 {
                    self.run += 1;
                    self.best_trial = tr;
                } else {
                    self.run = 0;
                }
                if self.run >= 60 {
                    self.state = State::Leader {
                        min: f32::MAX,
                        at: 0,
                        low: false,
                    };
                    self.det.leaders += 1;
                    self.reading = true;
                }
            }
            State::Leader { min, at, low } => {
                let (sc, _) = self.leader_score();
                // The sync symbol: the leader with its phase reversed cuts the window in half, a dip of the score.
                if sc < 0.35 {
                    let (min, at) = if sc < min { (sc, self.t) } else { (min, at) };
                    if low && self.t > at + 25 * MSS {
                        // No recovery: take the dip anyway.
                        self.begin_frame(at);
                    } else {
                        self.state = State::Leader { min, at, low: true };
                    }
                } else if low {
                    // Back up: the window is inside the sync symbol.
                    self.begin_frame(at);
                }
            }
            State::Frame { t0 } => {
                if let Some(end) = self.frame(t0, out) {
                    self.state = State::Skip { until: end };
                }
            }
            State::Skip { until } => {
                if self.t >= until {
                    self.state = State::Idle;
                    self.run = 0;
                    self.reading = false;
                }
            }
        }
    }

    fn begin_frame(&mut self, dip: u64) {
        // The dip is at the reversal + 10 ms (the window's end). The type symbols start 20 ms after the reversal.
        let rev = dip.saturating_sub(80);
        self.offset = self.leader_offset(rev.saturating_sub(40));
        self.state = State::Frame { t0: rev + 160 };
    }
}

/// The value that two of three copies agree on.
fn vote(b: &[u8]) -> Option<u8> {
    if b.len() < 3 {
        return None;
    }
    if b[0] == b[1] || b[0] == b[2] {
        Some(b[0])
    } else if b[1] == b[2] {
        Some(b[1])
    } else {
        None
    }
}

impl sdrkit_graph::Decoder for ArdopDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.ardop"
    }
    fn outputs(&self) -> Vec<Port> {
        let mut o = decoder::outputs();
        o.push(Port::typed::<ArdopFrame>("frames").rate_from(RateFrom::Fixed(0.0)));
        o
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        crate::typed::results("ardop")
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.state = State::Idle;
        self.run = 0;
        let mut f = decoder::formats();
        f.push(Format::new(sdrkit_graph::Kind::typed::<ArdopFrame>(), 0.0));
        Ok(f)
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.drop_sync();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        for &s in x {
            let y = self.lpf.process_sample(s * self.nco.next_sample());
            self.level_acc += y.norm_sqr() as f64;
            self.level_n += 1;
            if self.level_n == 8000 {
                self.det.levels.push(db(self.level_acc / 8000.0) as f32);
                if self.det.levels.len() > 120 {
                    self.det.levels.remove(0);
                }
                self.level_acc = 0.0;
                self.level_n = 0;
            }
            self.hist.push(y);
            self.t += 1;
            if self.t.is_multiple_of(MSS) && self.t > WIN as u64 {
                self.every_ms(out);
            }
        }
        if !self.new_frames.is_empty() {
            let o = out.items::<ArdopFrame>(decoder::EXTRA);
            for f in self.new_frames.drain(..) {
                o.push(Timed {
                    time: f.time,
                    value: f,
                });
            }
        }
    }
    fn report(&mut self, res: &mut Published) {
        let now = self.t as f64 / 8000.0;
        let state: u8 = if now - self.last_frame < 30.0 {
            2
        } else if self.reading {
            1
        } else {
            0
        };
        let st = &mut self.status;
        st.lock_state =
            [LockState::Searching, LockState::Tracking, LockState::Locked][state as usize];
        st.offset_hz = Some(self.offset);
        st.snr_db = Some(self.snr);
        if !self.frames.is_empty() {
            put_text(
                &mut st.summary,
                format_args!(
                    "ARDOP: {} frames, {} stations",
                    self.frames.len(),
                    self.stations.len()
                ),
            );
        } else if self.det.leaders > 0 {
            put_text(
                &mut st.summary,
                format_args!("ARDOP: leader heard, no frame yet"),
            );
        } else {
            put_text(&mut st.summary, format_args!("ARDOP: no signal"));
        }
        res.results_mut().set_status(st);
        let d = &self.det;
        if self.shown != Some(self.store.total) {
            self.shown = Some(self.store.total);
            text_into(&self.store, &mut self.text);
        }
        res.put(&ArdopFields {
            leaders: d.leaders as i64,
            good_headers: d.good_headers as i64,
            unreadable_leaders: d.unreadable as i64,
            data_ok: d.data_ok as i64,
            data_bad: d.data_bad as i64,
            rs_corrections: d.rs_corrections as i64,
            control: d.control as i64,
            other: d.other as i64,
            stations_heard: self.stations.len() as i64,
            frames_ok: self.frames.len() as i64,
            text: (!self.text.is_empty()).then_some(self.text.as_str()),
        });
        if self.levels.is_none() {
            self.levels = res.results().decl().series_id("levels");
        }
        if let Some(id) = self.levels {
            res.results_mut().update_series(id, |sr| {
                sr.points.clear();
                for (i, &l) in d.levels.iter().enumerate() {
                    sr.points.push([i as f32, l]);
                }
                true
            });
        }
        shaped::log_rows(&self.frames, &ARDOP_FRAMES, now, res);
        shaped::rows(&self.stations, &ARDOP_STATIONS, now, res);
        flush_logs(&mut self.logs, res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        HeardCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match HeardCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            HeardCommand::Remove => {
                let a: StationArgs = REMOVE.args(args)?;
                if self.stations.remove(a.row).is_none() {
                    return Err(self.results().no_such_row(&ARDOP_STATIONS, a.row));
                }
            }
        }
        Ok(None)
    }
    fn clear(&mut self) {
        self.frames.clear();
        self.stations.clear();
        self.store.clear();
        self.shown = None;
    }
    fn forget(&mut self, kept: &Kept) {
        self.drop_sync();
        if !kept.table(ARDOP_FRAMES.key) {
            self.frames.clear();
        }
        if !kept.table(ARDOP_STATIONS.key) {
            self.stations.clear();
        }
        if !kept.field("text") {
            self.store.clear();
            self.shown = None;
        }
    }
}

impl ArdopDecoder {
    /// Drops the frame being received.
    fn drop_sync(&mut self) {
        self.state = State::Idle;
        self.run = 0;
        self.refined = None;
        self.hdr = None;
        // The index carries on, so it stays equal to `t`.
        self.hist.clear();
    }
}

/// One frame to send: its type, session id, body and leader.
#[derive(Clone, Debug, PartialEq)]
pub struct TxFrame {
    /// Type byte.
    pub code: u8,
    /// Session id.
    pub session: u8,
    /// Body bytes (already with RS / CRC / copies).
    pub body: Vec<u8>,
    /// Leader, ms.
    pub leader_ms: u32,
}

impl TxFrame {
    /// A control frame without a body.
    pub fn bare(code: u8, session: u8) -> Self {
        TxFrame {
            code,
            session,
            body: vec![],
            leader_ms: 200,
        }
    }
    /// ID, ConReq or Ping: two packed fields and RS(4).
    pub fn two_fields(rs: &mut RsCodes, code: u8, a: &str, b: &str) -> Self {
        let mut body = pack6(a).to_vec();
        body.extend(pack6(b));
        let p = rs.parity(&body, 4);
        body.extend(p);
        TxFrame {
            code,
            session: 0xFF,
            body,
            leader_ms: 200,
        }
    }
    /// ConAck with its leader length.
    pub fn con_ack(code: u8, leader_ms: u32, session: u8) -> Self {
        let v = (leader_ms / 10) as u8;
        TxFrame {
            code,
            session,
            body: vec![v, v, v],
            leader_ms: 200,
        }
    }
    /// PingAck with SNR and quality.
    pub fn ping_ack(snr: i32, quality: u32) -> Self {
        let s = if snr >= 21 {
            0xF8
        } else {
            (((snr + 10).max(0) as u8) << 3) & 0xF8
        };
        let v = s | (((quality.saturating_sub(30)) / 10) as u8 & 7);
        TxFrame {
            code: 0x3D,
            session: 0xFF,
            body: vec![v, v, v],
            leader_ms: 200,
        }
    }
    /// A 4FSK data frame of the text.
    pub fn data(rs: &mut RsCodes, code: u8, session: u8, text: &str) -> Self {
        Self::data_bytes(rs, code, session, text.as_bytes())
    }
    /// A 4FSK data frame of `data` (at most the frame's data size; more is cut): the count byte, the data padded with zeros,
    /// the CRC-16 high byte, the CRC-16 low byte XOR the type byte, the Reed-Solomon parity.
    pub fn data_bytes(rs: &mut RsCodes, code: u8, session: u8, data: &[u8]) -> Self {
        let ft = frame_type(code).expect("a data frame type");
        let n = data.len().min(ft.data);
        let mut b = vec![n as u8];
        b.extend_from_slice(&data[..n]);
        b.resize(1 + ft.data, 0);
        let crc = frame_check(&b);
        b.push((crc >> 8) as u8);
        b.push((crc as u8) ^ code);
        let p = rs.parity(&b, ft.rs);
        b.extend(p);
        TxFrame {
            code,
            session,
            body: b,
            leader_ms: 200,
        }
    }
    /// The same frame with a leader of `ms`.
    #[must_use]
    pub fn with_leader(mut self, ms: u32) -> Self {
        self.leader_ms = ms;
        self
    }
    /// The time on air, seconds: the leader, ten type symbols of 20 ms and four symbols a body byte.
    pub fn air_s(&self) -> f64 {
        let n = (self.leader_ms / 20).clamp(5, 50);
        f64::from(n) * 0.02 + 0.2 + self.body.len() as f64 * 4.0 * body_symbol_s(self.code)
    }
}

/// The symbol length of the body of frame type `code`, seconds: 10 ms for 4FSK.500.100 and 4FSK.500.100S, else 20 ms.
pub fn body_symbol_s(code: u8) -> f64 {
    if (0x4A..=0x4D).contains(&code) {
        0.01
    } else {
        0.02
    }
}

/// The test script: ID, ConReq500M, ConAck500 with 160 ms, Ping, PingAck 12 dB / 80, a 50 baud data frame, ACK
/// quality 70, a short 500 Hz data frame, ACK, NAK quality 48, DISC, an 8PSK header.
pub fn test_script() -> Vec<TxFrame> {
    let rs = &mut RsCodes::new();
    vec![
        TxFrame::two_fields(rs, 0x30, "K0TST", "FN42AA"),
        TxFrame::two_fields(rs, 0x32, "K0TST", "N0XYZ"),
        TxFrame::con_ack(0x3A, 160, 90),
        TxFrame::two_fields(rs, 0x3E, "N0XYZ", "K0TST"),
        TxFrame::ping_ack(12, 80),
        TxFrame::data(rs, 0x48, 90, "TEST DATA ONE"),
        TxFrame::bare(0xF0, 90),
        TxFrame::data(rs, 0x4C, 90, "TEST DATA TWO FROM K0TST"),
        TxFrame::bare(0xF4, 90),
        TxFrame::bare(0x05, 90),
        TxFrame::bare(0x29, 90),
        TxFrame::bare(0x44, 90),
    ]
}

/// The generator's signal: the script with the frame writer of the sender at full scale, 0.3 s of silence first, 0.5 s
/// after each frame and 1.5 s more after the last. Repeated.
pub struct ArdopModulator {
    rate: f64,
    script: Vec<TxFrame>,
    w: tx::FrameWriter,
}

impl ArdopModulator {
    /// A script at `rate`. A frame whose code is not a frame type is left out.
    pub fn new(rate: f64, script: &[TxFrame]) -> Self {
        let script: Vec<TxFrame> = script
            .iter()
            .filter(|f| frame_type(f.code).is_some())
            .cloned()
            .collect();
        let w = Self::pass(rate, &script);
        ArdopModulator { rate, script, w }
    }

    fn pass(rate: f64, script: &[TxFrame]) -> tx::FrameWriter {
        let mut w = tx::FrameWriter::new(rate, 1.0, false);
        w.push_quiet(0.3);
        for f in script {
            w.push_frame(f);
            w.push_quiet(0.5);
        }
        w.push_quiet(1.5);
        w
    }
}

impl Modulator for ArdopModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        let mut i = 0;
        while i < out.len() {
            i += self.w.fill(&mut out[i..]);
            if i < out.len() {
                self.w = Self::pass(self.rate, &self.script);
            }
        }
    }
}

/// The generator: the test script.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let m = ArdopModulator::new(cfg.rate_hz, &test_script());
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        m,
        SignalInfo::new(500.0)
            .with_symbol_rate(50.0)
            .with_power(0.8),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_facts() {
        assert_eq!(all_types().len(), 121);
        for c in [0x25u8, 0x3F, 0x4E, 0x80] {
            assert!(frame_type(c).is_none(), "{c:#x}");
        }
        assert_eq!(frame_type(0x32).unwrap().name, "ConReq500M");
        assert_eq!(frame_type(0x3A).unwrap().name, "ConAck500");
        assert_eq!(frame_type(0x49).unwrap().name, "4FSK.200.50S.O");
        assert_eq!(frame_type(0x05).unwrap().name, "DataNAK");
    }

    #[test]
    fn packing() {
        assert_eq!(unpack6(&pack6("LY2ABC-5")), "LY2ABC-5");
        assert_eq!(unpack6(&pack6("ly2abc")), "LY2ABC");
        assert_eq!(&pack6("A")[..3], &[0x84, 0, 0]);
        let rs = &mut RsCodes::new();
        assert_eq!(
            TxFrame::two_fields(rs, 0x30, "K0TST", "FN42AA").body.len(),
            16
        );
        assert_eq!(TxFrame::data(rs, 0x48, 1, "X").body.len(), 1 + 16 + 2 + 4);
    }
}
