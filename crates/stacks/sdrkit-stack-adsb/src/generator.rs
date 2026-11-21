//! The clean 1090 MHz test signal: a simulated airspace that sends the Mode S and ADS-B message mix at its real rates, rendered as
//! shaped pulses at any rate from 2 Msps up. Also scripted bursts for tests, and the message encoders. Noise and the receiver's
//! impairments come from the framework's `gen_*` settings.

use crate::codes::{
    encode_ac12_25ft, encode_ac12_gillham, encode_ac13_25ft, encode_ac13_gillham, encode_callsign,
    encode_squawk,
};
use crate::commb;
use crate::crc::parity;
use crate::geo::{bearing_deg, destination, distance_nm};
use crate::message::{encode_movement, Frame};
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, Values};
use sdrkit_util_geo::cpr::{self as cpr, CprFormat};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

/// The random numbers of the test signals (`sdrkit-algo-sim`).
pub use sdrkit_algo_sim::rng::Rng;

// ---------------------------------------------------------------- encoders

fn put(v: &mut [u8], a: usize, b: usize, x: u64) {
    for i in a..=b {
        let bit = (x >> (b - i)) & 1;
        let byte = (i - 1) / 8;
        let m = 0x80 >> ((i - 1) % 8);
        if bit == 1 {
            v[byte] |= m;
        } else {
            v[byte] &= !m;
        }
    }
}

/// A PI frame (DF11, 17, 18, 19): `overlay` is XORed into the parity (an interrogator code for DF11, 0 otherwise).
pub fn pi_frame(mut bytes: Vec<u8>, overlay: u32) -> Frame {
    let n = bytes.len() - 3;
    let p = parity(&bytes[..n]) ^ overlay;
    bytes[n] = (p >> 16) as u8;
    bytes[n + 1] = (p >> 8) as u8;
    bytes[n + 2] = p as u8;
    Frame::new(&bytes).expect("7 or 14 bytes")
}

/// An AP frame: the address is XORed into the parity.
pub fn ap_frame(bytes: Vec<u8>, address: u32) -> Frame {
    pi_frame(bytes, address & 0xFF_FFFF)
}

/// An extended squitter (DF17 with CA, or DF18 with CF) carrying a 56-bit ME.
pub fn es_frame(df: u8, code: u8, address: u32, me: u64) -> Frame {
    let mut v = vec![0u8; 14];
    put(&mut v, 1, 5, df as u64);
    put(&mut v, 6, 8, code as u64);
    put(&mut v, 9, 32, (address & 0xFF_FFFF) as u64);
    put(&mut v, 33, 88, me);
    pi_frame(v, 0)
}

fn me_put(me: &mut u64, a: u32, b: u32, x: u64) {
    let n = b - a + 1;
    let mask = ((1u64 << n) - 1) << (56 - b);
    *me = (*me & !mask) | ((x << (56 - b)) & mask);
}

/// TC 1 to 4 identification.
pub fn me_ident(set: char, code: u8, callsign: &str) -> u64 {
    let tc = 4 - (set as u8 - b'A').min(3);
    let mut me = 0;
    me_put(&mut me, 1, 5, tc as u64);
    me_put(&mut me, 6, 8, code as u64);
    me_put(&mut me, 9, 56, encode_callsign(callsign));
    me
}

/// The two 17-bit CPR fields of a position (zeros for a position out of range).
fn cpr_fields(fmt: &CprFormat, lat: f64, lon: f64, odd: bool) -> (u32, u32) {
    cpr::encode(fmt, &sdrkit_util_geo::Position::new(lat, lon), odd)
        .map(|f| (f.lat, f.lon))
        .unwrap_or((0, 0))
}

/// TC 9 to 18 (or 20 to 22) airborne position with a 12-bit altitude field.
pub fn me_airborne(tc: u8, ss: u8, alt12: u32, odd: bool, lat: f64, lon: f64) -> u64 {
    let (yz, xz) = cpr_fields(&CprFormat::AIRBORNE, lat, lon, odd);
    let mut me = 0;
    me_put(&mut me, 1, 5, tc as u64);
    me_put(&mut me, 6, 7, ss as u64);
    me_put(&mut me, 9, 20, alt12 as u64);
    me_put(&mut me, 22, 22, odd as u64);
    me_put(&mut me, 23, 39, yz as u64);
    me_put(&mut me, 40, 56, xz as u64);
    me
}

/// TC 5 to 8 surface position.
pub fn me_surface(
    tc: u8,
    speed_kt: f64,
    track_deg: Option<f64>,
    odd: bool,
    lat: f64,
    lon: f64,
) -> u64 {
    let (yz, xz) = cpr_fields(&CprFormat::SURFACE, lat, lon, odd);
    let mut me = 0;
    me_put(&mut me, 1, 5, tc as u64);
    me_put(&mut me, 6, 12, encode_movement(speed_kt) as u64);
    if let Some(t) = track_deg {
        me_put(&mut me, 13, 13, 1);
        me_put(
            &mut me,
            14,
            20,
            ((t.rem_euclid(360.0) * 128.0 / 360.0).round() as u64) % 128,
        );
    }
    me_put(&mut me, 22, 22, odd as u64);
    me_put(&mut me, 23, 39, yz as u64);
    me_put(&mut me, 40, 56, xz as u64);
    me
}

fn vrate_bits(me: &mut u64, vrate_fpm: Option<i32>, baro: bool) {
    me_put(me, 36, 36, baro as u64);
    if let Some(v) = vrate_fpm {
        me_put(me, 37, 37, (v < 0) as u64);
        me_put(
            me,
            38,
            46,
            ((v.unsigned_abs() as f64 / 64.0).round() as u64 + 1).min(511),
        );
    }
}

/// TC 19 subtype 1: ground speed from east and north components (kt).
pub fn me_velocity_ground(
    ve: f64,
    vn: f64,
    vrate_fpm: Option<i32>,
    baro: bool,
    gnss_diff_ft: Option<i32>,
) -> u64 {
    let mut me = 0;
    me_put(&mut me, 1, 5, 19);
    me_put(&mut me, 6, 8, 1);
    me_put(&mut me, 14, 14, (ve < 0.0) as u64);
    me_put(&mut me, 15, 24, (ve.abs().round() as u64 + 1).min(1023));
    me_put(&mut me, 25, 25, (vn < 0.0) as u64);
    me_put(&mut me, 26, 35, (vn.abs().round() as u64 + 1).min(1023));
    vrate_bits(&mut me, vrate_fpm, baro);
    if let Some(d) = gnss_diff_ft {
        me_put(&mut me, 49, 49, (d < 0) as u64);
        me_put(
            &mut me,
            50,
            56,
            ((d.unsigned_abs() as f64 / 25.0).round() as u64 + 1).min(127),
        );
    }
    me
}

/// TC 19 subtype 3: magnetic heading and IAS or TAS.
pub fn me_velocity_air(
    heading: f64,
    airspeed: f64,
    tas: bool,
    vrate_fpm: Option<i32>,
    baro: bool,
) -> u64 {
    let mut me = 0;
    me_put(&mut me, 1, 5, 19);
    me_put(&mut me, 6, 8, 3);
    me_put(&mut me, 14, 14, 1);
    me_put(
        &mut me,
        15,
        24,
        ((heading.rem_euclid(360.0) * 1024.0 / 360.0).round() as u64) % 1024,
    );
    me_put(&mut me, 25, 25, tas as u64);
    me_put(&mut me, 26, 35, (airspeed.round() as u64 + 1).min(1023));
    vrate_bits(&mut me, vrate_fpm, baro);
    me
}

/// TC 31 subtype 0 operational status.
pub fn me_op_status(version: u8, nacp: u8, sil: u8) -> u64 {
    let mut me = 0;
    me_put(&mut me, 1, 5, 31);
    me_put(&mut me, 41, 43, version as u64);
    me_put(&mut me, 45, 48, nacp as u64);
    me_put(&mut me, 51, 52, sil as u64);
    me
}

/// TC 29 subtype 1 target state.
pub fn me_target_state(sel_alt_ft: i32, baro_hpa: f64, sel_heading: Option<f64>, nacp: u8) -> u64 {
    let mut me = 0;
    me_put(&mut me, 1, 5, 29);
    me_put(&mut me, 6, 7, 1);
    me_put(&mut me, 10, 20, (sel_alt_ft / 32 + 1).clamp(0, 2047) as u64);
    me_put(
        &mut me,
        21,
        29,
        (((baro_hpa - 800.0) / 0.8).round() as i64 + 1).clamp(0, 511) as u64,
    );
    if let Some(h) = sel_heading {
        let mut d = h.rem_euclid(360.0);
        if d >= 180.0 {
            d -= 360.0;
        }
        let raw = ((d * 256.0 / 180.0).round() as i64) & 0x1FF;
        me_put(&mut me, 30, 30, 1);
        me_put(&mut me, 31, 39, raw as u64);
    }
    me_put(&mut me, 40, 43, nacp as u64);
    me
}

/// TC 28 subtype 1 emergency status.
pub fn me_status(emergency: u8, squawk: u16) -> u64 {
    let mut me = 0;
    me_put(&mut me, 1, 5, 28);
    me_put(&mut me, 6, 8, 1);
    me_put(&mut me, 9, 11, emergency as u64);
    me_put(&mut me, 12, 24, encode_squawk(squawk) as u64);
    me
}

/// DF11 all-call reply with an interrogator code.
pub fn df11(ca: u8, address: u32, ic: u8) -> Frame {
    let mut v = vec![0u8; 7];
    put(&mut v, 1, 5, 11);
    put(&mut v, 6, 8, ca as u64);
    put(&mut v, 9, 32, (address & 0xFF_FFFF) as u64);
    pi_frame(v, ic as u32 & 0x7F)
}

/// DF4 or DF20 (with an MB field): flight status and 13-bit AC.
pub fn df4_20(fs: u8, ac13: u32, mb: Option<u64>, address: u32) -> Frame {
    let mut v = vec![0u8; if mb.is_some() { 14 } else { 7 }];
    put(&mut v, 1, 5, if mb.is_some() { 20 } else { 4 });
    put(&mut v, 6, 8, fs as u64);
    put(&mut v, 20, 32, ac13 as u64);
    if let Some(m) = mb {
        put(&mut v, 33, 88, m);
    }
    ap_frame(v, address)
}

/// DF5 or DF21: flight status and identity.
pub fn df5_21(fs: u8, squawk: u16, mb: Option<u64>, address: u32) -> Frame {
    let mut v = vec![0u8; if mb.is_some() { 14 } else { 7 }];
    put(&mut v, 1, 5, if mb.is_some() { 21 } else { 5 });
    put(&mut v, 6, 8, fs as u64);
    put(&mut v, 20, 32, encode_squawk(squawk) as u64);
    if let Some(m) = mb {
        put(&mut v, 33, 88, m);
    }
    ap_frame(v, address)
}

/// DF0 or DF16 (with an MV field): vertical status and 13-bit AC.
pub fn df0_16(vs: bool, ac13: u32, mv: Option<u64>, address: u32) -> Frame {
    let mut v = vec![0u8; if mv.is_some() { 14 } else { 7 }];
    put(&mut v, 1, 5, if mv.is_some() { 16 } else { 0 });
    put(&mut v, 6, 6, vs as u64);
    put(&mut v, 14, 17, 3); // RI: ACAS with vertical-only resolution capability
    put(&mut v, 20, 32, ac13 as u64);
    if let Some(m) = mv {
        put(&mut v, 33, 88, m);
    }
    ap_frame(v, address)
}

// ---------------------------------------------------------------- rendering

/// The response of the radio to one edge: a step with a 25 ns Gaussian rise through a third-order Butterworth low-pass, placed
/// where it reaches half, tabulated at 1 ns.
#[derive(Clone, Debug)]
pub struct EdgeTable {
    t0: f64,
    vals: Vec<f32>,
}

const EDGE_DT: f64 = 1e-9;

impl EdgeTable {
    /// For a cutoff (half the radio's filter width), Hz.
    pub fn new(cutoff_hz: f64) -> EdgeTable {
        let wc = std::f64::consts::TAU * cutoff_hz;
        let s = |t: f64| -> f64 {
            if t <= 0.0 {
                0.0
            } else {
                1.0 - (-wc * t).exp()
                    - 2.0 / 3f64.sqrt() * (-wc * t / 2.0).exp() * (3f64.sqrt() / 2.0 * wc * t).sin()
            }
        };
        let sigma = 25e-9;
        let (lo, hi) = (-1.5e-6, 5e-6);
        let n = ((hi - lo) / EDGE_DT) as usize;
        let gk: Vec<f64> = (-100..=100)
            .map(|k| (-(k as f64 * EDGE_DT).powi(2) / (2.0 * sigma * sigma)).exp())
            .collect();
        let gs: f64 = gk.iter().sum();
        let raw: Vec<f64> = (0..n)
            .map(|i| {
                let t = lo + i as f64 * EDGE_DT;
                gk.iter()
                    .enumerate()
                    .map(|(k, g)| g * s(t - (k as f64 - 100.0) * EDGE_DT))
                    .sum::<f64>()
                    / gs
            })
            .collect();
        let half = raw.iter().position(|&v| v >= 0.5).unwrap_or(0);
        let shift = lo + half as f64 * EDGE_DT;
        // Re-tabulate around the half point: t' = t - shift.
        let t0 = lo - shift;
        EdgeTable {
            t0,
            vals: raw.iter().map(|&v| v as f32).collect(),
        }
    }
    /// The response at time `t` after the edge (s).
    #[inline]
    pub fn at(&self, t: f64) -> f32 {
        let x = (t - self.t0) / EDGE_DT;
        if x <= 0.0 {
            return 0.0;
        }
        let i = x as usize;
        if i + 1 >= self.vals.len() {
            return 1.0;
        }
        let f = (x - i as f64) as f32;
        self.vals[i] + (self.vals[i + 1] - self.vals[i]) * f
    }
    /// How long before the edge the response starts, and how long after it it has settled.
    pub fn span(&self) -> (f64, f64) {
        (-self.t0, self.t0 + self.vals.len() as f64 * EDGE_DT)
    }
}

/// One burst of pulses.
#[derive(Clone, Debug)]
pub struct Burst {
    /// The time of the first edge (s).
    pub start: f64,
    /// Rising (+1) and falling (-1) edges, by time.
    pub edges: Vec<(f64, f32)>,
    /// Pulse amplitude.
    pub amp: f32,
    /// Carrier offset, Hz.
    pub freq: f64,
    /// Carrier phase at `start`.
    pub phase: f64,
    /// The time of the last edge.
    pub end: f64,
}

impl Burst {
    /// A burst from pulses (start, length) in seconds after `t`.
    pub fn from_pulses(t: f64, pulses: &[(f64, f64)], amp: f32, freq: f64, phase: f64) -> Burst {
        let mut edges: Vec<(f64, f32)> = Vec::with_capacity(pulses.len() * 2);
        for &(a, l) in pulses {
            edges.push((t + a, 1.0));
            edges.push((t + a + l, -1.0));
        }
        edges.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Equal and opposite edges at the same time cancel (a 0 followed by a 1 is one 1 µs pulse).
        let mut merged: Vec<(f64, f32)> = Vec::with_capacity(edges.len());
        for e in edges {
            if let Some(l) = merged.last_mut() {
                if (l.0 - e.0).abs() < 1e-12 {
                    l.1 += e.1;
                    continue;
                }
            }
            merged.push(e);
        }
        merged.retain(|e| e.1 != 0.0);
        let start = merged.first().map_or(t, |e| e.0);
        let end = merged.last().map_or(t, |e| e.0);
        Burst {
            start,
            edges: merged,
            amp,
            freq,
            phase,
            end,
        }
    }

    /// A Mode S reply: the preamble and 56 or 112 bits.
    pub fn mode_s(t: f64, f: &Frame, amp: f32, freq: f64, phase: f64) -> Burst {
        let mut p: Vec<(f64, f64)> = vec![
            (0.0, 0.5e-6),
            (1.0e-6, 0.5e-6),
            (3.5e-6, 0.5e-6),
            (4.5e-6, 0.5e-6),
        ];
        for k in 0..f.bits as usize {
            let one = f.bytes[k / 8] >> (7 - k % 8) & 1 == 1;
            p.push((
                (8.0 + k as f64 + if one { 0.0 } else { 0.5 }) * 1e-6,
                0.5e-6,
            ));
        }
        Burst::from_pulses(t, &p, amp, freq, phase)
    }

    /// A Mode A/C reply: framing pulses 20.3 µs apart and the code pulses (C1 A1 C2 A2 C4 A4 X B1 D1 B2 D2 B4 D4) on a 1.45 µs
    /// grid, 0.45 µs each, with the SPI pulse 4.35 µs after F2.
    pub fn mode_ac(t: f64, code13: u32, spi: bool, amp: f32, freq: f64, phase: f64) -> Burst {
        let w = 0.45e-6;
        let mut p = vec![(0.0, w), (20.3e-6, w)];
        for i in 0..13 {
            if code13 >> (12 - i) & 1 == 1 && i != 6 {
                p.push(((i as f64 + 1.0) * 1.45e-6, w));
            }
        }
        if spi {
            p.push((24.65e-6, w));
        }
        Burst::from_pulses(t, &p, amp, freq, phase)
    }

    /// One pulse of impulse noise.
    pub fn impulse(t: f64, len: f64, amp: f32, freq: f64, phase: f64) -> Burst {
        Burst::from_pulses(t, &[(0.0, len)], amp, freq, phase)
    }

    /// Adds the burst into `out`, whose sample i is sample `n0 + i` of a stream at `rate` (its time `(n0 + i) / rate`).
    pub fn render(&self, table: &EdgeTable, n0: u64, rate: f64, out: &mut [Complex32]) {
        let (pre, post) = table.span();
        let a = self.start - pre;
        let b = self.end + post;
        let t0 = n0 as f64 / rate;
        let i0 = ((a - t0) * rate).ceil().max(0.0) as usize;
        let i1 = (((b - t0) * rate).floor() + 1.0).max(0.0) as usize;
        let i1 = i1.min(out.len());
        if i0 >= i1 {
            return;
        }
        let mut lo = 0usize;
        let w = std::f64::consts::TAU * self.freq;
        // The envelope before edge `lo` is the sum of the edges already settled.
        let mut settled = 0.0f32;
        for (i, o) in out.iter_mut().enumerate().take(i1).skip(i0) {
            // The time is computed from the sample's own index, so it does not depend on where the stream was cut.
            let t = (n0 + i as u64) as f64 / rate;
            while lo < self.edges.len() && t - self.edges[lo].0 > post {
                settled += self.edges[lo].1;
                lo += 1;
            }
            let mut env = settled;
            for e in &self.edges[lo..] {
                let d = t - e.0;
                if d < -pre {
                    break;
                }
                env += e.1 * table.at(d);
            }
            if env != 0.0 {
                let (sn, cs) = (self.phase + w * (t - self.start)).sin_cos();
                let g = self.amp as f64 * env as f64;
                *o += Complex32::new((cs * g) as f32, (sn * g) as f32);
            }
        }
    }
}

/// The renderer shared by the generators: the bursts of the clean signal, each pulse edge shaped by the pulse filter.
pub struct Renderer {
    rate: f64,
    table: EdgeTable,
    n: u64,
    active: Vec<Burst>,
    scratch: Vec<Complex32>,
}

impl Renderer {
    /// For a rate and the width of the filter that shapes the pulse edges (Hz).
    pub fn new(rate: f64, filter_hz: f64) -> Renderer {
        Renderer {
            rate,
            table: EdgeTable::new(filter_hz / 2.0),
            n: 0,
            active: Vec::new(),
            scratch: Vec::new(),
        }
    }
    /// The rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }
    /// The signal time of sample `n`.
    pub fn time_of(&self, n: u64) -> f64 {
        n as f64 / self.rate
    }
    /// The index of the next sample.
    pub fn now_index(&self) -> u64 {
        self.n
    }
    /// The time at the end of what was rendered.
    pub fn now(&self) -> f64 {
        self.time_of(self.n)
    }
    /// Adds a burst (in start order).
    pub fn add(&mut self, b: Burst) {
        self.active.push(b);
    }
    /// Renders the next `out.len()` samples.
    pub fn render(&mut self, out: &mut [Complex32]) {
        self.scratch.clear();
        self.scratch.resize(out.len(), Complex32::new(0.0, 0.0));
        for b in &self.active {
            b.render(&self.table, self.n, self.rate, &mut self.scratch);
        }
        let post = self.table.span().1;
        let t_end = self.time_of(self.n + out.len() as u64);
        self.active.retain(|b| b.end + post > t_end);
        out.copy_from_slice(&self.scratch);
        self.n += out.len() as u64;
    }
}

/// What the 1090 MHz test signals declare: replies of 1 Mbit/s in about 2 MHz, at the power of an aircraft 50 NM away (amplitude
/// 0.1), so the framework's SNR is the SNR of that aircraft.
pub fn signal_info() -> sdrkit_core::SignalInfo {
    sdrkit_core::SignalInfo {
        bandwidth_hz: 2e6,
        symbol_rate_hz: Some(1e6),
        bit_rate_hz: Some(1e6),
        power: 0.01,
    }
}

// ---------------------------------------------------------------- the airspace

/// The generator's options.
#[derive(Clone, Debug)]
pub struct AirspaceOptions {
    /// Aircraft (0 to 300).
    pub aircraft: usize,
    /// Radar replies (DF0, 4, 5, 16, 20, 21).
    pub replies: bool,
    /// Emergency demonstration.
    pub emergency: bool,
    /// Message rate multiplier (0.1 to 20).
    pub multiplier: f64,
    /// Width of the filter that shapes the pulses, Hz (None: automatic).
    pub filter_hz: Option<f64>,
    /// The centre of the airspace (latitude, longitude).
    pub centre: (f64, f64),
}

impl Default for AirspaceOptions {
    fn default() -> Self {
        AirspaceOptions {
            aircraft: 12,
            replies: true,
            emergency: false,
            multiplier: 1.0,
            filter_hz: None,
            centre: (25.25, 55.36),
        }
    }
}

impl AirspaceOptions {
    /// From the stack's settings (the `gen_*` keys).
    pub fn from_values(v: &Values) -> AirspaceOptions {
        use crate::settings::{
            GEN_AIRCRAFT, GEN_CENTRE_LAT, GEN_CENTRE_LON, GEN_EMERGENCY, GEN_FILTER_MHZ, GEN_RATE,
            GEN_REPLIES,
        };
        let filt = v.get(&GEN_FILTER_MHZ);
        AirspaceOptions {
            aircraft: v.get(&GEN_AIRCRAFT).clamp(0, 300) as usize,
            replies: v.get(&GEN_REPLIES),
            emergency: v.get(&GEN_EMERGENCY),
            multiplier: v.get(&GEN_RATE),
            filter_hz: (filt > 0.0).then_some(filt * 1e6),
            centre: (v.get(&GEN_CENTRE_LAT), v.get(&GEN_CENTRE_LON)),
        }
    }
    /// The radio filter width at a rate.
    pub fn filter_at(&self, rate: f64) -> f64 {
        self.filter_hz
            .unwrap_or_else(|| (0.875 * rate).clamp(1.75e6, 5e6))
    }
}

/// One transmission, as the generator sent it.
#[derive(Clone, Debug, PartialEq)]
pub struct Transmission {
    /// Preamble time, s.
    pub time: f64,
    /// The frame.
    pub frame: Frame,
    /// Pulse amplitude.
    pub amp: f32,
    /// Carrier offset, Hz.
    pub freq: f64,
    /// Another burst overlaps it.
    pub overlapped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Chan {
    Position,
    Velocity,
    Ident,
    OpStatus,
    TargetState,
    Status,
    AllCall,
    Df4,
    Df5,
    Df20,
    Df21,
    Df0,
    Df16,
}

impl Chan {
    fn period_jitter(self) -> (f64, f64) {
        match self {
            Chan::Position | Chan::Velocity => (0.5, 0.2),
            Chan::Ident => (5.0, 0.1),
            Chan::OpStatus => (2.5, 0.1),
            Chan::TargetState | Chan::Status => (1.25, 0.2),
            Chan::AllCall => (2.0, 0.5),
            Chan::Df4 => (3.0, 0.5),
            Chan::Df5 => (4.0, 0.5),
            Chan::Df20 => (5.0, 0.5),
            Chan::Df21 => (7.0, 0.5),
            Chan::Df0 => (8.0, 0.5),
            Chan::Df16 => (12.0, 0.5),
        }
    }
}

const PREFIXES: [&str; 20] = [
    "SIMA", "SIMB", "SIMC", "SIMD", "SIME", "SIMF", "SIMG", "SIMH", "SIMI", "SIMJ", "SIMK", "SIML",
    "SIMM", "SIMN", "SIMO", "SIMP", "SIMQ", "SIMR", "SIMS", "SIMT",
];

/// A simulated aircraft.
#[derive(Clone, Debug)]
pub struct Plane {
    /// Address.
    pub address: u32,
    /// Callsign.
    pub callsign: String,
    /// Squawk.
    pub squawk: u16,
    /// Gillham altitudes.
    pub gillham: bool,
    /// Sends target state.
    pub target_state: bool,
    /// ADS-B version.
    pub version: u8,
    /// NACp.
    pub nacp: u8,
    /// Position type code.
    pub pos_tc: u8,
    /// Category.
    pub category: (char, u8),
    /// Baro setting.
    pub baro_hpa: f64,
    /// Position.
    pub lat: f64,
    /// Position.
    pub lon: f64,
    /// Track, deg.
    pub track: f64,
    /// Ground speed, kt.
    pub gs: f64,
    /// Altitude, ft.
    pub alt: f64,
    /// ft/min.
    pub vrate: f64,
    /// Climb or descent target.
    pub target_alt: f64,
    /// Taxiing.
    pub ground: bool,
    /// Selected heading.
    pub sel_heading: f64,
    /// Carrier offset, Hz.
    pub carrier: f64,
    /// Emergency demonstration.
    pub emergency: bool,
    t: f64,
    fade_db: f64,
    fade_target: f64,
    odd: bool,
    commb_turn: u8,
}

impl Plane {
    /// The time of its state.
    pub fn time(&self) -> f64 {
        self.t
    }
    /// Indicated airspeed as the generator sends it.
    pub fn ias(&self) -> f64 {
        (0.62 * self.gs).clamp(150.0, 320.0)
    }
    /// Mach as the generator sends it.
    pub fn mach(&self) -> f64 {
        (self.gs / 600.0).clamp(0.3, 0.88)
    }
    /// The altitude as sent (rounded to 25 ft, or 100 ft in Gillham).
    pub fn alt_sent(&self) -> i32 {
        if self.ground {
            return 0;
        }
        let a = (self.alt / 25.0).round() as i32 * 25;
        if self.gillham {
            ((a as f64 / 100.0).round() as i32) * 100
        } else {
            a
        }
    }
}

/// The simulated airspace: aircraft and their transmissions in time order.
pub struct Airspace {
    opts: AirspaceOptions,
    rng: Rng,
    /// The aircraft.
    pub planes: Vec<Plane>,
    queue: BinaryHeap<Reverse<(u64, usize, Chan)>>,
    used: std::collections::HashSet<u32>,
}

fn ns(t: f64) -> u64 {
    (t.max(0.0) * 1e9).round() as u64
}

impl Airspace {
    /// A new airspace.
    pub fn new(opts: AirspaceOptions, seed: u64) -> Airspace {
        let mut a = Airspace {
            rng: Rng::new(seed),
            planes: Vec::new(),
            queue: BinaryHeap::new(),
            used: Default::default(),
            opts,
        };
        let n = a.opts.aircraft;
        for i in 0..n {
            let ground = n >= 4 && i == 0;
            let p = a.new_plane(0.0, ground, false);
            a.planes.push(p);
        }
        if a.opts.emergency && n > 0 {
            let mid = n / 2;
            let m = if a.planes[mid].ground {
                (mid + 1) % n
            } else {
                mid
            };
            a.planes[m].squawk = 0o7700;
            a.planes[m].emergency = true;
        }
        for i in 0..n {
            a.schedule_first(i, 0.0);
        }
        a
    }
    /// Changes the message rate multiplier (takes effect from the next transmissions).
    pub fn set_multiplier(&mut self, m: f64) {
        self.opts.multiplier = m.clamp(0.1, 20.0);
    }

    fn new_plane(&mut self, t: f64, ground: bool, entering: bool) -> Plane {
        let r = &mut self.rng;
        let mut address;
        loop {
            address = 0x400000 + r.below(0xAFFFFF - 0x400000 + 1) as u32;
            if self.used.insert(address) {
                break;
            }
        }
        let r = &mut self.rng;
        let prefix = PREFIXES[r.below(20) as usize];
        let num = if r.uniform() < 0.5 {
            1 + r.below(9)
        } else {
            100 * (1 + r.below(9)) + r.below(100)
        };
        let mut callsign = format!("{prefix}{num}");
        callsign.truncate(8);
        let mut squawk = (r.below(8) << 9 | r.below(8) << 6 | r.below(8) << 3 | r.below(8)) as u16;
        if matches!(squawk, 0o7500 | 0o7600 | 0o7700 | 0) {
            squawk = 0o2201;
        }
        let carrier = (r.normal() * 100e3).clamp(-400e3, 400e3);
        let gillham = r.uniform() < 0.12;
        let target_state = r.uniform() < 0.5;
        let version = if r.uniform() < 0.7 {
            2
        } else if r.uniform() < 0.5 {
            1
        } else {
            0
        };
        let nacp = 8 + r.below(3) as u8;
        let pos_tc = if r.uniform() < 0.7 {
            11
        } else {
            9 + r.below(6) as u8
        };
        let u = r.uniform();
        let category = if u < 0.2 {
            ('A', 5)
        } else if u < 0.88 {
            ('A', 3)
        } else {
            ('A', 2)
        };
        let baro_hpa = 1013.2 - 0.8 * (r.below(15) as f64 - 6.0);
        let (rl, ro) = self.opts.centre;
        let (lat, lon, track, gs, alt, vrate, target_alt);
        if ground {
            let d = r.range(0.4, 2.5);
            let b = r.range(0.0, 360.0);
            let (la, lo, _) = destination(rl, ro, b, d);
            lat = la;
            lon = lo;
            track = r.range(0.0, 360.0);
            gs = r.range(8.0, 22.0);
            alt = 0.0;
            vrate = 0.0;
            target_alt = 0.0;
            squawk = 0o2000;
        } else {
            let (d, b, trk) = if entering {
                let d = r.range(185.0, 205.0);
                let b = r.range(0.0, 360.0);
                let (la, lo, _) = destination(rl, ro, b, d);
                let to_ref = bearing_deg(la, lo, rl, ro);
                (d, b, (to_ref + r.range(-55.0, 55.0)).rem_euclid(360.0))
            } else {
                (
                    12.0 + 178.0 * r.uniform().sqrt(),
                    r.range(0.0, 360.0),
                    r.range(0.0, 360.0),
                )
            };
            let (la, lo, _) = destination(rl, ro, b, d);
            lat = la;
            lon = lo;
            track = trk;
            let u = r.uniform();
            if u < 0.6 {
                alt = 1000.0 * (24 + r.below(16)) as f64;
                target_alt = alt;
                vrate = 0.0;
                gs = r.range(430.0, 520.0);
            } else if u < 0.8 {
                alt = r.range(4000.0, 30000.0);
                target_alt = 1000.0 * (30 + r.below(9)) as f64;
                vrate = r.range(1400.0, 2600.0);
                gs = r.range(300.0, 450.0);
            } else {
                alt = r.range(12000.0, 36000.0);
                target_alt = r.range(2500.0, 9000.0);
                vrate = -r.range(1200.0, 2400.0);
                gs = r.range(300.0, 450.0);
            }
        }
        let alt = (alt / 25.0).round() * 25.0;
        Plane {
            address,
            callsign,
            squawk,
            gillham,
            target_state,
            version,
            nacp,
            pos_tc,
            category,
            baro_hpa,
            lat,
            lon,
            track,
            gs,
            alt,
            vrate,
            target_alt: (target_alt / 25.0).round() * 25.0,
            ground,
            sel_heading: track.floor(),
            carrier,
            emergency: false,
            t,
            fade_db: 0.0,
            fade_target: 0.0,
            odd: false,
            commb_turn: 0,
        }
    }

    fn channels(&self, i: usize) -> Vec<Chan> {
        let p = &self.planes[i];
        let mut v = vec![Chan::Position, Chan::Ident, Chan::OpStatus, Chan::AllCall];
        if !p.ground {
            v.push(Chan::Velocity);
            if p.target_state {
                v.push(Chan::TargetState);
            }
        }
        if p.emergency {
            v.push(Chan::Status);
        }
        if self.opts.replies {
            v.push(Chan::Df4);
            v.push(Chan::Df5);
            if !p.ground {
                v.extend([Chan::Df20, Chan::Df21, Chan::Df0, Chan::Df16]);
            }
        }
        v
    }

    fn schedule_first(&mut self, i: usize, t: f64) {
        for c in self.channels(i) {
            let first = t + self.rng.uniform() * c.period_jitter().0;
            self.queue.push(Reverse((ns(first), i, c)));
        }
    }

    fn advance(&mut self, i: usize, t: f64) {
        let (rl, ro) = self.opts.centre;
        let r = &mut self.rng;
        let p = &mut self.planes[i];
        let dt = (t - p.t).max(0.0);
        if dt == 0.0 {
            return;
        }
        if p.ground {
            if r.uniform() < 1.0 - (-0.1 * dt).exp() {
                p.track = (p.track + r.range(-90.0, 90.0)).rem_euclid(360.0);
            }
            if distance_nm(rl, ro, p.lat, p.lon) > 3.0 {
                p.track = bearing_deg(p.lat, p.lon, rl, ro);
            }
            let (la, lo, _) = destination(p.lat, p.lon, p.track, p.gs * dt / 3600.0);
            p.lat = la;
            p.lon = lo;
        } else {
            let (la, lo, trk) = destination(p.lat, p.lon, p.track, p.gs * dt / 3600.0);
            p.lat = la;
            p.lon = lo;
            p.track = trk;
            if p.vrate != 0.0 {
                p.alt += p.vrate * dt / 60.0;
                if (p.vrate > 0.0 && p.alt >= p.target_alt)
                    || (p.vrate < 0.0 && p.alt <= p.target_alt)
                {
                    p.alt = p.target_alt;
                    p.vrate = 0.0;
                }
            }
        }
        if r.uniform() < 1.0 - (-0.5 * dt).exp() {
            p.fade_target = r.range(-2.5, 2.0);
        }
        p.fade_db += (p.fade_target - p.fade_db) * (1.0 - (-0.5 * dt).exp());
        p.t = t;
    }

    /// The next transmission: (time, frame, amplitude, carrier offset), in time order.
    pub fn next_transmission(&mut self) -> Option<(f64, Frame, f32, f64)> {
        loop {
            let Reverse((tn, i, c)) = self.queue.pop()?;
            let t = tn as f64 * 1e-9;
            self.advance(i, t);
            // Replace an aircraft that flew away.
            let (rl, ro) = self.opts.centre;
            let far = {
                let p = &self.planes[i];
                !p.ground && distance_nm(rl, ro, p.lat, p.lon) > 215.0
            };
            if far {
                let old = self.planes[i].address;
                self.used.remove(&old);
                let np = self.new_plane(t, false, true);
                self.planes[i] = np;
                // Drop this aircraft's queued channels and start new ones.
                let rest: Vec<_> = self.queue.drain().filter(|e| e.0 .1 != i).collect();
                self.queue.extend(rest);
                self.schedule_first(i, t);
                continue;
            }
            let (period, jitter) = c.period_jitter();
            let next = t + period / self.opts.multiplier
                * (1.0 + jitter * (2.0 * self.rng.uniform() - 1.0));
            self.queue.push(Reverse((ns(next), i, c)));
            let f = self.message(i, c);
            let p = &self.planes[i];
            let d = distance_nm(rl, ro, p.lat, p.lon);
            let amp = (0.5f64.min(0.1 * 50.0 / d.max(12.0)) * 10f64.powf(p.fade_db / 20.0)) as f32;
            return Some((t, f, amp, p.carrier));
        }
    }

    fn message(&mut self, i: usize, c: Chan) -> Frame {
        let ic = if self.rng.uniform() < 0.7 {
            0
        } else {
            1 + self.rng.below(15) as u8
        };
        let p = &mut self.planes[i];
        let ca = if p.ground { 4 } else { 5 };
        let a = p.address;
        let alt = p.alt_sent();
        let ac12 = if p.gillham {
            encode_ac12_gillham(alt)
        } else {
            encode_ac12_25ft(alt)
        }
        .unwrap_or(0);
        let ac13 = if p.gillham {
            encode_ac13_gillham(alt)
        } else {
            encode_ac13_25ft(alt)
        }
        .unwrap_or(0);
        let vr = (p.vrate / 64.0).round() as i32 * 64;
        match c {
            Chan::Position => {
                p.odd = !p.odd;
                let me = if p.ground {
                    me_surface(7, p.gs, Some(p.track), p.odd, p.lat, p.lon)
                } else {
                    me_airborne(p.pos_tc, 0, ac12, p.odd, p.lat, p.lon)
                };
                es_frame(17, ca, a, me)
            }
            Chan::Velocity => {
                let me = if a.is_multiple_of(11) {
                    me_velocity_air(p.track, p.ias(), false, Some(vr), true)
                } else {
                    let (ve, vn) = (
                        p.gs * p.track.to_radians().sin(),
                        p.gs * p.track.to_radians().cos(),
                    );
                    me_velocity_ground(
                        ve,
                        vn,
                        Some(vr),
                        false,
                        Some(25 * ((a >> 4) % 9) as i32 - 100),
                    )
                };
                es_frame(17, ca, a, me)
            }
            Chan::Ident => es_frame(17, ca, a, me_ident(p.category.0, p.category.1, &p.callsign)),
            Chan::OpStatus => es_frame(17, ca, a, me_op_status(p.version, p.nacp, 3)),
            Chan::TargetState => es_frame(
                17,
                ca,
                a,
                me_target_state(p.target_alt as i32, p.baro_hpa, Some(p.sel_heading), p.nacp),
            ),
            Chan::Status => es_frame(17, ca, a, me_status(1, p.squawk)),
            Chan::AllCall => df11(ca, a, ic),
            Chan::Df4 => {
                if p.ground {
                    df4_20(1, encode_ac13_25ft(0).unwrap_or(0), None, a)
                } else {
                    df4_20(0, ac13, None, a)
                }
            }
            Chan::Df5 => df5_21(if p.ground { 1 } else { 0 }, p.squawk, None, a),
            Chan::Df20 | Chan::Df21 => {
                p.commb_turn = (p.commb_turn + 1) % 4;
                let vr32 = (p.vrate / 32.0).floor() as i32 * 32;
                let mb = match p.commb_turn {
                    1 => commb::encode_20(&p.callsign),
                    2 => {
                        let s = ((p.target_alt as i32) / 16) * 16;
                        commb::encode_40(Some(s), Some(s), Some(p.baro_hpa))
                    }
                    3 => commb::encode_50(0.0, p.track, p.gs, 0.0, 1.04 * p.gs),
                    _ => commb::encode_60(p.track, p.ias(), p.mach(), vr32, vr32),
                };
                if c == Chan::Df20 {
                    df4_20(0, ac13, Some(mb), a)
                } else {
                    df5_21(0, p.squawk, Some(mb), a)
                }
            }
            Chan::Df0 => df0_16(false, encode_ac13_25ft(alt).unwrap_or(0), None, a),
            Chan::Df16 => df0_16(false, ac13, Some(0), a),
        }
    }
}

// ---------------------------------------------------------------- the generator

enum Source {
    Air(Box<Airspace>),
    Script(VecDeque<(Burst, Option<Frame>)>),
}

/// The 1090 MHz generator: an airspace (or a script of bursts), rendered at its rate.
pub struct AdsbGenerator {
    src: Source,
    r: Renderer,
    rng: Rng,
    peek: Option<(Burst, Option<Frame>)>,
    /// Every transmission, when `keep_log` is set.
    pub log: Vec<Transmission>,
    /// Keep the transmissions in `log`.
    pub keep_log: bool,
    last_end: f64,
}

impl AdsbGenerator {
    /// An airspace at `rate` (at least 2 Msps).
    pub fn airspace(rate: Hz, opts: AirspaceOptions, seed: u64) -> AdsbGenerator {
        let r = Renderer::new(rate, opts.filter_at(rate));
        AdsbGenerator {
            src: Source::Air(Box::new(Airspace::new(opts, seed))),
            r,
            rng: Rng::new(seed.wrapping_mul(31).wrapping_add(7)),
            peek: None,
            log: Vec::new(),
            keep_log: false,
            last_end: f64::MIN,
        }
    }
    /// A script of bursts (in time order), with pulses shaped by a filter `filter_hz` wide. `seed` is not used by a script and
    /// keeps the signature of the airspace.
    pub fn script(
        rate: Hz,
        filter_hz: f64,
        bursts: Vec<(Burst, Option<Frame>)>,
        seed: u64,
    ) -> AdsbGenerator {
        let mut b = bursts;
        b.sort_by(|x, y| x.0.start.total_cmp(&y.0.start));
        AdsbGenerator {
            src: Source::Script(b.into()),
            r: Renderer::new(rate, filter_hz),
            rng: Rng::new(seed),
            peek: None,
            log: Vec::new(),
            keep_log: false,
            last_end: f64::MIN,
        }
    }
    /// From a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> AdsbGenerator {
        AdsbGenerator::airspace(
            cfg.rate_hz,
            AirspaceOptions::from_values(&cfg.settings),
            cfg.seed,
        )
    }
    /// The airspace, when it is one.
    pub fn airspace_ref(&self) -> Option<&Airspace> {
        match &self.src {
            Source::Air(a) => Some(a),
            _ => None,
        }
    }
    /// Changes the message rate multiplier while it plays.
    pub fn set_multiplier(&mut self, m: f64) {
        if let Source::Air(a) = &mut self.src {
            a.set_multiplier(m);
        }
    }
    fn next_burst(&mut self) -> Option<(Burst, Option<Frame>)> {
        match &mut self.src {
            Source::Air(a) => {
                let (t, f, amp, freq) = a.next_transmission()?;
                let ph = self.rng.range(0.0, std::f64::consts::TAU);
                Some((Burst::mode_s(t, &f, amp, freq, ph), Some(f)))
            }
            Source::Script(q) => q.pop_front(),
        }
    }
}

impl Generator for AdsbGenerator {
    fn rate_hz(&self) -> Hz {
        self.r.rate()
    }
    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        Some(signal_info())
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let t_end = self.r.time_of(self.r.n + out.len() as u64) + 1e-6;
        loop {
            if self.peek.is_none() {
                self.peek = self.next_burst();
            }
            let due = self.peek.take_if(|(b, _)| {
                let later = b.start - self.r.table.span().0 > t_end;
                !later
            });
            let Some((b, f)) = due else { break };
            if self.keep_log {
                if let Some(f) = f {
                    let overlapped = b.start < self.last_end + 1e-6;
                    if overlapped {
                        if let Some(l) = self.log.last_mut() {
                            l.overlapped = true;
                        }
                    }
                    self.log.push(Transmission {
                        time: b.start,
                        frame: f,
                        amp: b.amp,
                        freq: b.freq,
                        overlapped,
                    });
                }
            }
            self.last_end = self.last_end.max(b.end);
            self.r.add(b);
        }
        self.r.render(out);
        out.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{es, Es};

    #[test]
    fn encoders_reproduce_published_frames() {
        let f = es_frame(17, 5, 0x4840D6, me_ident('A', 0, "KLM1023"));
        assert_eq!(f.hex(), "8D4840D6202CC371C32CE0576098");
        let f = df11(5, 0x484FDE, 22);
        assert_eq!(f.hex(), "5D484FDEA248F5");
        let f = df4_20(0, encode_ac13_25ft(36000).unwrap(), None, 0x4CA7E8);
        assert_eq!(f.hex(), "2000171806A983");
        // The published identity reply has UM bits and the X bit set: compared field by field.
        let f = df5_21(2, 0o356, None, 0x510AF9);
        let p = Frame::from_hex("2A00516D492B80").unwrap();
        let (h, hp) = (
            crate::message::header(&f, f.remainder()),
            crate::message::header(&p, p.remainder()),
        );
        assert_eq!(
            (h.df, h.code, h.squawk, h.key),
            (hp.df, hp.code, hp.squawk, hp.key)
        );
        let me = me_velocity_ground(-8.0, -159.0, Some(-832), false, Some(550));
        match Es::decode(me) {
            Es::Velocity {
                speed_kt,
                heading_deg,
                vrate_fpm,
                gnss_diff_ft,
                ..
            } => {
                assert!((speed_kt.unwrap() - 159.2).abs() < 0.1);
                assert!((heading_deg.unwrap() - 182.88).abs() < 0.01);
                assert_eq!((vrate_fpm, gnss_diff_ft), (Some(-832), Some(550)));
            }
            e => panic!("{e:?}"),
        }
        let me = me_airborne(11, 0, 0xC38, false, 52.2572, 3.91937);
        let f = es_frame(17, 5, 0x40621D, me);
        assert_eq!(f.hex(), "8D40621D58C382D690C8AC2863A7");
        let me = me_airborne(11, 0, 0xC38, true, 52.26578, 3.93891);
        assert_eq!(
            es_frame(17, 5, 0x40621D, me).hex(),
            "8D40621D58C386435CC412692AD6"
        );
        assert!(matches!(es(&f), Some(Es::Airborne { .. })));
    }

    #[test]
    fn edge_table_is_a_step() {
        let t = EdgeTable::new(0.875e6);
        assert!(t.at(-2e-6) == 0.0);
        assert!((t.at(0.0) - 0.5).abs() < 0.01);
        assert!((t.at(4e-6) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn the_signal_does_not_depend_on_block_sizes() {
        let mk = || AdsbGenerator::airspace(4e6, AirspaceOptions::default(), 5);
        let mut a = mk();
        let mut b = mk();
        let mut x = vec![Complex32::new(0.0, 0.0); 400_000];
        a.generate(&mut x);
        let mut y = Vec::new();
        let mut buf = vec![Complex32::new(0.0, 0.0); 4099];
        let mut sizes = [1usize, 7, 4099, 333, 2000].iter().cycle();
        while y.len() < x.len() {
            let n = (*sizes.next().unwrap()).min(x.len() - y.len());
            b.generate(&mut buf[..n]);
            y.extend_from_slice(&buf[..n]);
        }
        assert_eq!(x, y);
        let peak = x
            .iter()
            .map(|v| v.re.abs().max(v.im.abs()))
            .fold(0.0, f32::max);
        let rms = (x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64).sqrt();
        assert!(peak < 0.96 && rms > 0.001 && rms < 0.1, "{peak} {rms}");
    }
}
