//! NFC and RFID at 13.56 MHz (`nfc`): the tag type and the UID broadcast during anticollision and inventory, and the name of every
//! frame. Memory contents, NDEF records and APDU payloads are never shown, and a MIFARE Classic session after its authentication
//! request is "encrypted, not decoded". Nothing is transmitted, cloned or emulated.
//!
//! Standard: ISO/IEC 14443-2 and -3, ISO/IEC 15693-2 and -3.

use crate::family::{hex, min_rate_message, set_status, wall, CleanScene, REPORT_S};
use crate::typed::{
    NfcDetectFields, NfcFields, NfcFrameRow, NfcTagEvent, NfcTagRow, NFC_FRAMES, NFC_REMOVE,
    NFC_TAG, NFC_TAGS, NFC_TAG_ROW,
};
use sdrkit_algo_dsp::iir::ComplexOnePole;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::scene::Scene;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::AutoGain;
use sdrkit_core::SignalType;
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState, Receiver, Record,
    Results, ResultsDecl, Schema, SignalInfo, StackConfig, StackDescriptor, StateBlob, StateError,
    TableId, Tuning, Values,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError, Io, Item, Kept,
    Kind, Port, RateFrom, RunContext, StackGraph, Timed,
};
use sdrkit_util_bits::crc::catalog::{CRC16_IBM_SDLC, CRC16_ISO_IEC_14443_3_A};
use sdrkit_util_bits::line::{
    modified_miller_decode, modified_miller_encode, ppm_decode, ppm_encode, uart_encode,
};
use sdrkit_util_bits::pack::pack;
use sdrkit_util_bits::{BitOrder, Pad};
use sdrkit_util_objects::commands;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tables,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// The carrier.
pub const FC: f64 = 13.56e6;
/// One elementary time unit (s): 128 carrier periods. ISO/IEC 14443-2 clause 3.
pub const ETU: f64 = 128.0 / FC;
/// The Type A and Type B subcarrier, fc/16 (Hz).
pub const SUB_A: f64 = FC / 16.0;
/// The ISO 15693 subcarrier, fc/32 (Hz).
pub const SUB_V: f64 = FC / 32.0;

/// One frame (`nfc.frame`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NfcFrame {
    /// 'A', 'B' or 'V' (ISO 15693).
    pub link: char,
    /// From the reader (true) or the tag.
    pub reader: bool,
    /// Bits (7 for a short frame).
    pub bits: usize,
    /// Parity good (Type A).
    pub parity_ok: bool,
    /// The bytes (CRC included).
    pub bytes: Vec<u8>,
    /// Time.
    pub time: f64,
    /// Level (dBFS).
    pub level_db: f32,
}

impl Item for NfcFrame {
    const KIND: &'static str = "nfc.frame";
}

/// Type A bits of bytes, LSB first, each followed by an odd parity bit. ISO/IEC 14443-3 clause 6.2.3.1.
pub fn a_bits(bytes: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(9 * bytes.len());
    for &b in bytes {
        v.extend((0..8).map(|i| (b >> i) & 1));
        v.push(((b.count_ones() + 1) % 2) as u8);
    }
    v
}

/// The bytes of Type A bits: 7 bits are a short frame, otherwise whole 9-bit groups with odd parity. Returns (bytes, parity
/// good, bits used).
pub fn a_bytes(bits: &[u8]) -> (Vec<u8>, bool, usize) {
    if bits.len() == 7 {
        let v = bits.iter().enumerate().fold(0u8, |a, (i, &b)| a | (b << i));
        return (vec![v], true, 7);
    }
    let n = bits.len() / 9;
    let mut ok = true;
    let mut out = Vec::with_capacity(n);
    for g in bits.as_chunks::<9>().0 {
        let v = g[..8]
            .iter()
            .enumerate()
            .fold(0u8, |a, (i, &b)| a | (b << i));
        if (v.count_ones() + g[8] as u32) % 2 != 1 {
            ok = false;
        }
        out.push(v);
    }
    (out, ok, n * 9)
}

/// The CRC_A of `data` (ISO/IEC 14443-3 annex B), sent low byte first.
pub fn crc_a(data: &[u8]) -> u16 {
    CRC16_ISO_IEC_14443_3_A.checksum(data)
}

/// The CRC_B of `data` (ISO/IEC 14443-3 annex B, and ISO/IEC 15693-3), sent low byte first.
pub fn crc_b(data: &[u8]) -> u16 {
    CRC16_IBM_SDLC.checksum(data)
}

fn crc_ok_a(b: &[u8]) -> bool {
    let n = b.len();
    n >= 3 && crc_a(&b[..n - 2]) == u16::from_le_bytes([b[n - 2], b[n - 1]])
}

fn crc_ok_b(b: &[u8]) -> bool {
    let n = b.len();
    n >= 3 && crc_b(&b[..n - 2]) == u16::from_le_bytes([b[n - 2], b[n - 1]])
}

/// Three complex one-pole low-passes in cascade.
#[derive(Clone)]
struct Lp3([ComplexOnePole; 3]);

impl Lp3 {
    fn new(cut_hz: f64, rate: f64) -> Result<Lp3, ConfigError> {
        let p = ComplexOnePole::lowpass(rate, cut_hz)?;
        Ok(Lp3([p.clone(), p.clone(), p]))
    }
    #[inline]
    fn run(&mut self, x: Complex32) -> Complex32 {
        let a = self.0[0].process_sample(x);
        let b = self.0[1].process_sample(a);
        self.0[2].process_sample(b)
    }
}

/// `nfc.detect`: field, pauses, subcarriers, bursts and their decoding.
pub struct Detect {
    rate: f64,
    mix: Option<Nco>,
    nco_a: Option<Nco>,
    nco_v: Option<Nco>,
    lp_a: Option<Lp3>,
    lp_v: Option<Lp3>,
    s_buf: Vec<Complex32>,
    osc_a: Vec<Complex32>,
    osc_v: Vec<Complex32>,
    n: u64,
    field: f32,
    noise_a: f32,
    noise_v: f32,
    open: bool,
    quiet: usize,
    start: u64,
    env: Vec<f32>,
    za: Vec<Complex32>,
    zv: Vec<Complex32>,
    fld: Vec<f32>,
    pre: VecDeque<(f32, Complex32, Complex32, f32)>,
    had_pause: bool,
    step_run: usize,
    hold: usize,
    coh: Complex32,
    carrier: f32,
    out: Vec<NfcFrame>,
    published: Published,
}

impl Default for Detect {
    fn default() -> Self {
        Detect {
            rate: 4e6,
            mix: None,
            nco_a: None,
            nco_v: None,
            lp_a: None,
            lp_v: None,
            s_buf: vec![],
            osc_a: vec![],
            osc_v: vec![],
            n: 0,
            field: 0.0,
            noise_a: 1.0,
            noise_v: 1.0,
            open: false,
            quiet: 0,
            start: 0,
            env: vec![],
            za: vec![],
            zv: vec![],
            fld: vec![],
            pre: VecDeque::new(),
            had_pause: false,
            step_run: 0,
            hold: 0,
            coh: Complex32::default(),
            carrier: 0.0,
            out: vec![],
            published: Published::new(NfcDetectFields::SHAPE.fields),
        }
    }
}

fn dbfs(a: f32) -> f64 {
    20.0 * (a.max(1e-10) as f64).log10()
}

impl Detect {
    /// A reader frame of ISO 15693 1-of-4 pulse position: from the first pause (the SOF), symbols of 8 etu. ISO/IEC 15693-2
    /// clause 8.2.
    fn decode_v_reader(&mut self, pauses: &[f64], etu: f64, t0: f64, level: f32) {
        let p0 = pauses[0];
        let mut bits = Vec::new();
        let mut chips = [1u8; 8];
        for k in 0.. {
            let s = p0 - 0.5 * etu + (8.0 + 8.0 * k as f64) * etu;
            chips.fill(1);
            let mut any = false;
            for &p in pauses.iter().filter(|&&p| p >= s && p < s + 8.0 * etu) {
                chips[(((p - s) / etu) as usize).min(7)] = 0;
                any = true;
            }
            // The EOF pause is in an even slot, which is not a symbol: the frame ends there.
            if !any || ppm_decode(&chips, 2, &mut bits).is_err() {
                break;
            }
        }
        let mut bytes = Vec::with_capacity(bits.len() / 8);
        let nb = pack(&bits, BitOrder::Lsb, Pad::Drop, &mut bytes);
        if nb >= 1 {
            self.out.push(NfcFrame {
                link: 'V',
                reader: true,
                bits: nb * 8,
                parity_ok: true,
                bytes,
                time: t0,
                level_db: level,
            });
        }
    }

    /// A Type A reader frame of modified Miller pauses on the half-etu grid. ISO/IEC 14443-2 clause 8.1.3.
    fn decode_a_reader(&mut self, pauses: &[f64], etu: f64, t0: f64, level: f32) {
        let p0 = pauses[0];
        let mut marks: Vec<usize> = Vec::with_capacity(pauses.len());
        for &p in pauses {
            let m = (p - p0) / (etu / 2.0);
            if (m - m.round()).abs() <= 0.45 {
                marks.push(m.round() as usize);
            }
        }
        let last = *marks.last().unwrap_or(&0);
        // The chips up to the end of the last pause's bit time, then two bit times without pause (the end of communication).
        let mut chips = vec![1u8; 2 * (last / 2 + 1) + 4];
        for &m in &marks {
            chips[m] = 0;
        }
        let mut bits = Vec::new();
        if modified_miller_decode(&chips, &mut bits).is_err() || bits.len() < 7 {
            return;
        }
        let (bytes, ok, used) = a_bytes(&bits);
        // Fragments of one or two bytes failing parity are dropped.
        if bytes.is_empty() || (used != 7 && bytes.len() <= 2 && !ok) {
            return;
        }
        self.out.push(NfcFrame {
            link: 'A',
            reader: true,
            bits: used,
            parity_ok: ok,
            bytes,
            time: t0,
            level_db: level,
        });
    }

    fn decode(&mut self) {
        let etu = ETU * self.rate;
        let n = self.env.len();
        if n < 4 {
            return;
        }
        let field = self.fld.iter().copied().fold(0f32, f32::max);
        let t0 = self.start as f64 / self.rate;
        let level = dbfs(field) as f32;
        let mut pauses: Vec<f64> = Vec::new();
        let mut i = 0;
        while i < n {
            if self.env[i] < 0.4 * self.fld[i] {
                let s = i;
                while i < n && self.env[i] < 0.4 * self.fld[i] {
                    i += 1;
                }
                pauses.push((s + i) as f64 / 2.0);
            }
            i += 1;
        }
        if !pauses.is_empty() {
            let spacing = if pauses.len() > 1 {
                (pauses[pauses.len() - 1] - pauses[0]) / (pauses.len() - 1) as f64
            } else {
                0.0
            };
            if spacing > 4.0 * etu {
                self.decode_v_reader(&pauses, etu, t0, level);
            } else {
                self.decode_a_reader(&pauses, etu, t0, level);
            }
            return;
        }
        // 10 % steps: a Type B reader.
        let low: Vec<bool> = (0..n)
            .map(|i| self.env[i] < 0.95 * self.fld[i] && self.env[i] > 0.6 * self.fld[i])
            .collect();
        let nlow = low.iter().filter(|&&b| b).count();
        let mag_a: Vec<f32> = self.za.iter().map(|z| z.norm()).collect();
        let mag_v: Vec<f32> = self.zv.iter().map(|z| z.norm()).collect();
        let pa = mag_a.iter().copied().fold(0f32, f32::max);
        let pv = mag_v.iter().copied().fold(0f32, f32::max);
        if nlow as f64 > 8.0 * etu
            && pa < (0.05 * field).max(10.0 * self.noise_a)
            && pv < (0.05 * field).max(10.0 * self.noise_v)
        {
            let high: Vec<bool> = low.iter().map(|&b| !b).collect();
            if let Some(f) = nrz_chars(&high, etu) {
                self.out.push(NfcFrame {
                    link: 'B',
                    reader: true,
                    bits: f.len() * 8,
                    parity_ok: true,
                    bytes: f,
                    time: t0,
                    level_db: level,
                });
            }
            return;
        }
        // Tag answers.
        if pa >= pv && pa > (0.05 * field).max(10.0 * self.noise_a) {
            let thr = 0.3 * pa;
            let first = mag_a.iter().position(|&m| m > thr).unwrap_or(0) as f64;
            // BPSK: phase reversals with the subcarrier always on.
            let on = mag_a
                .iter()
                .skip(first as usize)
                .take((20.0 * etu) as usize)
                .filter(|&&m| m > thr)
                .count();
            if on as f64 > 18.0 * etu {
                let refz = self.za[(first + 2.0 * etu) as usize % n];
                let lv: Vec<bool> = self.za.iter().map(|z| (z * refz.conj()).re > 0.0).collect();
                let tr1 = lv
                    .iter()
                    .skip(first as usize)
                    .position(|&b| !b)
                    .map(|p| p as f64 + first)
                    .unwrap_or(first);
                if let Some(f) = nrz_chars(&lv[tr1 as usize..], etu) {
                    self.out.push(NfcFrame {
                        link: 'B',
                        reader: false,
                        bits: f.len() * 8,
                        parity_ok: true,
                        bytes: f,
                        time: t0,
                        level_db: level,
                    });
                }
                return;
            }
            // Each half etu is read over its later part, where the subcarrier detector has settled.
            let half = |k: usize, h: usize| -> f32 {
                let a = (first + (k as f64 + h as f64 * 0.5 + 0.2) * etu) as usize;
                let b = (first + (k as f64 + h as f64 * 0.5 + 0.5) * etu) as usize;
                if b > n || a >= b {
                    return 0.0;
                }
                mag_a[a..b].iter().sum::<f32>() / (b - a) as f32
            };
            let mut k = 0;
            if half(0, 0) > thr && half(0, 1) > thr {
                k = 1;
            }
            let mut bits = Vec::new();
            let mut contrast = 0f32;
            loop {
                let (a, b) = (half(k, 0), half(k, 1));
                if a <= thr && b <= thr {
                    break;
                }
                contrast += (a - b).abs() / (a + b).max(1e-12);
                bits.push((a > b) as u8);
                k += 1;
            }
            if bits.is_empty() || contrast / (bits.len() as f32) < 0.2 {
                return;
            }
            let (bytes, ok, used) = a_bytes(&bits);
            let looks = (bytes.len() >= 3 && crc_ok_a(&bytes))
                || (bytes.len() == 5 && bytes.iter().fold(0, |x, y| x ^ y) == 0)
                || (bytes.len() == 2
                    && ok
                    && bytes[0] >> 6 != 3
                    && bytes[0] & 0x20 == 0
                    && bytes[1] >> 4 == 0)
                || (bytes.len() == 1 && ok)
                || (bytes.len() > 4 && ok);
            if looks {
                self.out.push(NfcFrame {
                    link: 'A',
                    reader: false,
                    bits: used,
                    parity_ok: ok,
                    bytes,
                    time: t0,
                    level_db: level,
                });
            }
            return;
        }
        if pv > (0.05 * field).max(10.0 * self.noise_v) {
            let thr = 0.3 * pv;
            let first = mag_v.iter().position(|&m| m > thr).unwrap_or(0) as f64;
            // SOF: one bit time of subcarrier. Data bits of 4 etu: logic 0 is subcarrier in the first half, 1 in the second.
            let half = |k: usize, h: usize| -> f32 {
                let a = (first + (4.0 + 4.0 * k as f64 + h as f64 * 2.0) * etu) as usize;
                let b = (first + (4.0 + 4.0 * k as f64 + h as f64 * 2.0 + 2.0) * etu) as usize;
                if b > n || a >= b {
                    return 0.0;
                }
                mag_v[a..b].iter().sum::<f32>() / (b - a) as f32
            };
            let mut bits = Vec::new();
            let mut contrast = 0f32;
            let mut k = 0;
            loop {
                let (a, b) = (half(k, 0), half(k, 1));
                if a <= thr && b <= thr {
                    break;
                }
                contrast += (a - b).abs() / (a + b).max(1e-12);
                bits.push((b > a) as u8);
                k += 1;
            }
            let mut bytes = Vec::with_capacity(bits.len() / 8);
            let nb = pack(&bits, BitOrder::Lsb, Pad::Drop, &mut bytes);
            if nb >= 3 && contrast / bits.len() as f32 >= 0.2 {
                self.out.push(NfcFrame {
                    link: 'V',
                    reader: false,
                    bits: nb * 8,
                    parity_ok: true,
                    bytes,
                    time: t0,
                    level_db: level,
                });
            }
        }
    }
}

/// NRZ-L characters (start 0, 8 data bits LSB first, stop 1) after a SOF of 10 to 11 etu of 0 and 2 to 3 etu of 1, read at the
/// centre of each bit time and synchronised again on each start bit. `lv` is the logic level per sample from the frame start.
/// ISO/IEC 14443-3 clause 7.1.
fn nrz_chars(lv: &[bool], etu: f64) -> Option<Vec<u8>> {
    let n = lv.len();
    let s0 = lv.iter().position(|&b| !b)?;
    let s1 = s0 + lv[s0..].iter().position(|&b| b)?;
    let low = (s1 - s0) as f64 / etu;
    if !(9.0..12.5).contains(&low) {
        return None;
    }
    let s2 = s1 + lv[s1..].iter().position(|&b| !b)?;
    let at = |t: f64| -> Option<bool> { lv.get(t as usize).copied() };
    let mut out = Vec::new();
    let mut c = s2 as f64;
    loop {
        if c + 10.0 * etu > n as f64 {
            break;
        }
        if at(c + 0.5 * etu)? {
            break;
        }
        let mut v = 0u8;
        for i in 0..8 {
            if at(c + (1.5 + i as f64) * etu)? {
                v |= 1 << i;
            }
        }
        if !at(c + 9.5 * etu)? {
            break; // EOF: no stop bit
        }
        out.push(v);
        // The next start bit's falling edge within the next 2 etu.
        let from = (c + 10.0 * etu) as usize;
        let next = lv[from.min(n)..(from + (2.0 * etu) as usize).min(n)]
            .iter()
            .position(|&b| !b);
        c = match next {
            Some(p) => (from + p) as f64,
            None => c + 10.0 * etu,
        };
    }
    (!out.is_empty()).then_some(out)
}

impl Detect {
    /// Writes the levels of the field and of the subcarriers.
    fn put_levels(&mut self) {
        self.published.put(&NfcDetectFields {
            field_dbfs: Some(dbfs(self.field)),
            subcarrier_dbfs: Some(dbfs(self.noise_a.max(self.noise_v))),
        });
    }
}

impl Component for Detect {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "nfc.detect",
            vec![Port::iq("in")],
            vec![Port::typed::<NfcFrame>("frames").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        let off = f.signal_offset_hz();
        self.mix = (off != 0.0).then(|| Nco::new(f.rate, -off));
        self.nco_a = Some(Nco::new(f.rate, -SUB_A));
        self.nco_v = Some(Nco::new(f.rate, -SUB_V));
        self.lp_a = Some(Lp3::new(160e3, f.rate)?);
        self.lp_v = Some(Lp3::new(55e3, f.rate)?);
        self.open = false;
        self.pre = VecDeque::with_capacity((2.0 * ETU * f.rate) as usize + 1);
        self.put_levels();
        let mut o = f.with_kind(Kind::typed::<NfcFrame>());
        o.rate = 0.0;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let (Some(nco_a), Some(nco_v)) = (self.nco_a.as_mut(), self.nco_v.as_mut()) else {
            return;
        };
        let (Some(mut lp_a), Some(mut lp_v)) = (self.lp_a.take(), self.lp_v.take()) else {
            return;
        };
        let len = x.len();
        self.s_buf.resize(len, Complex32::default());
        self.osc_a.resize(len, Complex32::default());
        self.osc_v.resize(len, Complex32::default());
        match self.mix.as_mut() {
            Some(m) => m.mix(x, &mut self.s_buf),
            None => self.s_buf.copy_from_slice(x),
        }
        nco_a.fill(&mut self.osc_a);
        nco_v.fill(&mut self.osc_v);
        let rate = self.rate;
        let etu = ETU * rate;
        let pre = (2.0 * etu) as usize;
        for k in 0..len {
            let s = self.s_buf[k];
            let e = s.norm();
            // The carrier itself is taken out first, so its image after the subcarrier mixers stays far below the subcarrier.
            let sc = s - self.coh;
            let za = lp_a.run(sc * self.osc_a[k]);
            let zv = lp_v.run(sc * self.osc_v[k]);
            let (ma, mv) = (za.norm(), zv.norm());
            // The field follows the carrier, not a tag's load modulation on top of it.
            let loaded = ma > (0.05 * self.field).max(4.0 * self.noise_a)
                || mv > (0.05 * self.field).max(4.0 * self.noise_v);
            if loaded {
                self.hold = (2.0 * etu) as usize;
            } else if self.hold > 0 {
                self.hold -= 1;
            }
            if e > self.field {
                if self.hold == 0 {
                    self.field += 0.05 * (e - self.field);
                }
            } else {
                self.field += 0.0005 * (e - self.field);
            }
            // A carrier is a steady phasor: its slow complex mean holds most of the envelope (noise alone averages away).
            if (self.hold == 0 && e > 0.6 * self.field) || self.field < 0.003 {
                self.coh += (s - self.coh) * 0.001;
            }
            if self.hold == 0 && e > 0.6 * self.field {
                self.carrier += 0.002 * (e - self.carrier);
            }
            self.n += 1;
            let field = self.field;
            if field < 0.003 || self.coh.norm() < 0.5 * field {
                continue;
            }
            let pause = e < 0.4 * field;
            // A 10 % step counts once it has held for half an etu (a tag's load modulation flickers much faster).
            let cl = self.carrier.max(0.6 * field);
            self.step_run = if e < 0.95 * cl && e > 0.6 * cl && self.hold == 0 {
                self.step_run + 1
            } else {
                0
            };
            let step = self.step_run as f64 > 0.5 * etu;
            let sub_a = ma > (0.05 * field).max(4.0 * self.noise_a);
            let sub_v = mv > (0.05 * field).max(4.0 * self.noise_v);
            let active = pause || step || sub_a || sub_v;
            if !self.open {
                for (m, nz) in [(ma, &mut self.noise_a), (mv, &mut self.noise_v)] {
                    if m < *nz {
                        *nz += 0.01 * (m - *nz);
                    } else {
                        *nz += 0.0002 * (m - *nz);
                    }
                }
                self.pre.push_back((e, za, zv, field));
                if self.pre.len() > pre {
                    self.pre.pop_front();
                }
                if active {
                    self.open = true;
                    self.quiet = 0;
                    self.had_pause = pause;
                    self.start = self.n - self.pre.len() as u64;
                    self.env.clear();
                    self.za.clear();
                    self.zv.clear();
                    self.fld.clear();
                    for &(e0, a0, v0, f0) in &self.pre {
                        self.env.push(e0);
                        self.za.push(a0);
                        self.zv.push(v0);
                        self.fld.push(f0);
                    }
                    self.pre.clear();
                }
                continue;
            }
            self.env.push(e);
            self.za.push(za);
            self.zv.push(zv);
            self.fld.push(field);
            self.had_pause |= pause;
            self.quiet = if active { 0 } else { self.quiet + 1 };
            let close_after = if self.had_pause {
                16.0 * etu
            } else {
                6.0 * etu
            };
            if self.quiet as f64 > close_after || self.env.len() as f64 > 0.02 * rate {
                self.open = false;
                self.decode();
            }
        }
        self.lp_a = Some(lp_a);
        self.lp_v = Some(lp_v);
        if io.tick() {
            self.put_levels();
        }
        if !self.out.is_empty() {
            let v = self.out.drain(..).map(|f| Timed {
                time: f.time,
                value: f,
            });
            io.items_out::<NfcFrame>(0).extend(v);
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn forget(&mut self, _kept: &Kept) {
        let d = Detect::default();
        self.field = d.field;
        self.noise_a = d.noise_a;
        self.noise_v = d.noise_v;
        self.coh = d.coh;
        self.carrier = d.carrier;
        self.open = false;
        self.quiet = 0;
        self.hold = 0;
        self.step_run = 0;
        self.pre.clear();
        self.published.put(&NfcDetectFields {
            field_dbfs: None,
            subcarrier_dbfs: None,
        });
    }
}

/// A tag, keyed by its UID in the tag table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tag {
    /// UID (hex, most significant first).
    pub uid: String,
    /// Type.
    pub kind: String,
    /// Card-type hint.
    pub hint: Option<String>,
    /// ATQA.
    pub atqa: Option<u16>,
    /// SAK.
    pub sak: Option<u8>,
    /// ATS historical bytes (hex).
    pub historical: Option<String>,
    /// PUPI application data (Type B, hex).
    pub app_data: Option<String>,
    /// DSFID.
    pub dsfid: Option<u8>,
    /// AFI.
    pub afi: Option<u8>,
    /// Blocks and block size.
    pub blocks: Option<(u16, u16)>,
    /// The IC version byte of the system information (ISO/IEC 15693-3).
    pub ic_ref: Option<u8>,
}

impl Shift for Tag {
    fn shift_times(&mut self, _: f64) {}
}

/// A frame log line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameLine {
    /// Signal time.
    pub time_s: f64,
    /// "reader" or "tag".
    pub direction: String,
    /// "Type A", "Type B", "ISO 15693".
    pub link: String,
    /// Bytes in hex (empty for content not shown).
    pub hex: String,
    /// Name.
    pub name: String,
    /// CRC checked and good (`None`: not checked).
    pub crc_ok: Option<bool>,
    /// Parity good.
    pub parity_ok: bool,
    /// Level.
    pub level_dbfs: f32,
}

impl Shift for FrameLine {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

/// Tags kept.
pub const TAG_CAP: usize = 100;
/// Frames kept in the log.
pub const LOG_CAP: usize = 200;

impl Shaped for Tag {
    type Row = NfcTagRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let mut sys = Vec::new();
        if let Some(d) = self.dsfid {
            sys.push(format!("DSFID 0x{d:02X}"));
        }
        if let Some(a) = self.afi {
            sys.push(format!("AFI 0x{a:02X}"));
        }
        if let Some((n, s)) = self.blocks {
            sys.push(format!("{n} blocks of {s} bytes"));
        }
        if let Some(r) = self.ic_ref {
            sys.push(format!("IC 0x{r:02X}"));
        }
        let atqa = self.atqa.map(|a| format!("0x{a:04X}"));
        let sak = self.sak.map(|a| format!("0x{a:02X}"));
        let system = (!sys.is_empty()).then(|| sys.join(", "));
        out.put(&NfcTagRow {
            uid: &self.uid,
            r#type: &self.kind,
            hint: self.hint.as_deref(),
            atqa: atqa.as_deref(),
            sak: sak.as_deref(),
            historical: self.historical.as_deref(),
            app_data: self.app_data.as_deref(),
            system: system.as_deref(),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

impl Shaped for FrameLine {
    type Row = NfcFrameRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&NfcFrameRow {
            time_s: self.time_s,
            direction: &self.direction,
            link: &self.link,
            hex: &self.hex,
            name: &self.name,
            crc: match self.crc_ok {
                None => "not checked",
                Some(true) => "good",
                Some(false) => "bad",
            },
            parity_ok: self.parity_ok,
            level_dbfs: f64::from(self.level_dbfs),
        });
    }
}

/// What `nfc` keeps through a restart and in the saved state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Store {
    /// The tags, by UID.
    pub tags: ObjectTable<String, Tag>,
    /// The frame log.
    pub log: LogRing<FrameLine>,
    /// Frames heard.
    pub count: u64,
    /// CRC or parity failures outside encrypted sessions.
    pub failures: u64,
    /// Frames from tags.
    pub tag_frames: u64,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            tags: ObjectTable::new(TAG_CAP),
            log: LogRing::new(LOG_CAP),
            count: 0,
            failures: 0,
            tag_frames: 0,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.tags.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.nfc", 1);
    fn clear(&mut self) {
        self.tags.clear();
        self.log.clear();
        self.count = 0;
        self.failures = 0;
        self.tag_frames = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.tags.adopt(saved.tags);
        self.log.adopt(saved.log);
        self.count = saved.count;
        self.failures = saved.failures;
        self.tag_frames = saved.tag_frames;
    }
}

/// The card-type hint of an ATQA and SAK (the common public table).
fn hint(atqa: Option<u16>, sak: u8) -> Option<String> {
    Some(
        match sak {
            0x08 => "probably MIFARE Classic 1K",
            0x18 => "probably MIFARE Classic 4K",
            0x09 => "probably MIFARE Mini",
            0x00 if atqa == Some(0x0044) => {
                "probably NFC Forum Type 2 (MIFARE Ultralight / NTAG family)"
            }
            0x20 => "probably ISO 14443-4 (DESFire, payment and identity cards)",
            0x28 | 0x38 => "probably ISO 14443-4 with Classic emulation",
            _ => return None,
        }
        .into(),
    )
}

/// `nfc.tags`: frame names, sessions, tag assembly and the results.
pub struct Tags {
    store: Store,
    published: Published,
    tags_table: TableId,
    summary: String,
    events: Vec<Event>,
    encrypted: bool,
    cur_uid: Vec<u8>,
    cur_atqa: Option<u16>,
    carrier_hz: f64,
    now: f64,
    live: bool,
}

impl Default for Tags {
    fn default() -> Self {
        let published = Published::from_decl(Arc::new(
            ResultsDecl::new()
                .with_fields(NfcFields::SHAPE.fields)
                .with_table(&NFC_TAGS)
                .with_table(&NFC_FRAMES),
        ));
        let tags_table = published
            .results()
            .decl()
            .table_id(NFC_TAGS.key)
            .expect("the tables of nfc.tags are declared");
        Tags {
            store: Store::default(),
            published,
            tags_table,
            summary: String::new(),
            events: vec![],
            encrypted: false,
            cur_uid: vec![],
            cur_atqa: None,
            carrier_hz: FC,
            now: 0.0,
            live: false,
        }
    }
}

impl Tags {
    /// What is kept.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn stamp(&self, t: f64) -> Stamp {
        match wall(self.live) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        }
    }

    /// The tag with this UID, heard at `t`, made with `kind` when new. Returns its key, and whether it is new.
    fn tag(&mut self, uid: &[u8], kind: &str, t: f64) -> (String, bool) {
        let key = hex(uid);
        let at = self.stamp(t);
        let new = match self.store.tags.upsert(&key, at, || Tag {
            uid: hex(uid),
            kind: kind.into(),
            ..Default::default()
        }) {
            Ok(u) => u.new,
            Err(_) => false,
        };
        (key, new)
    }

    fn tag_mut(&mut self, key: &str) -> Option<&mut Tag> {
        self.store.tags.get_mut(key)
    }

    fn event(&mut self, t: f64, text: String, uid: &str) {
        self.events.push(NFC_TAG.event(
            t,
            self.carrier_hz,
            &NfcTagEvent {
                source: uid,
                text: &text,
                family: Family::Sensor,
                uid,
            },
        ));
    }

    /// Names a frame, follows the session and assembles the tags.
    fn frame(&mut self, f: &NfcFrame) {
        self.store.count += 1;
        if !f.reader {
            self.store.tag_frames += 1;
        }
        self.now = self.now.max(f.time);
        let b = &f.bytes;
        let mut crc: Option<bool> = None;
        let mut show = true;
        let name: String = match (f.link, f.reader) {
            ('A', true) => {
                if f.bits == 7 {
                    match b[0] {
                        0x26 | 0x52 => {
                            self.encrypted = false;
                            self.cur_uid.clear();
                            self.cur_atqa = None;
                            if b[0] == 0x26 { "REQA" } else { "WUPA" }.into()
                        }
                        _ => "7-bit short frame".into(),
                    }
                } else if b.len() == 4 && b[0] == 0x50 && b[1] == 0x00 && crc_ok_a(b) {
                    crc = Some(true);
                    self.encrypted = false;
                    "HLTA (halt)".into()
                } else if b.len() == 3 && b[0] == 0xE0 && crc_ok_a(b) {
                    crc = Some(true);
                    "RATS".into()
                } else if matches!(b.first(), Some(0x93 | 0x95 | 0x97)) && b.len() >= 2 {
                    let cl = (b[0] - 0x93) / 2 + 1;
                    self.encrypted = false;
                    if b[1] == 0x70 && b.len() == 9 {
                        let bcc_ok = b[2..6].iter().fold(0, |a, x| a ^ x) == b[6];
                        crc = Some(crc_ok_a(b));
                        if bcc_ok && crc == Some(true) {
                            let more = b[2] == 0x88;
                            let part = if more { &b[3..6] } else { &b[2..6] };
                            if cl == 1 {
                                self.cur_uid.clear();
                            }
                            self.cur_uid.extend_from_slice(part);
                            if !more {
                                let uid = self.cur_uid.clone();
                                let (key, new) = self.tag(
                                    &uid,
                                    "ISO 14443-A (UID from the reader's SELECT)",
                                    f.time,
                                );
                                if new {
                                    self.event(
                                        f.time,
                                        format!("NFC: tag {key} selected by the reader"),
                                        &key,
                                    );
                                }
                            }
                        }
                        format!(
                            "SELECT CL{cl} {}{}",
                            hex(&b[2..6]),
                            if bcc_ok { "" } else { " (BCC wrong)" }
                        )
                    } else {
                        format!("Anticollision CL{cl}")
                    }
                } else if matches!(b.first(), Some(0x60 | 0x61)) && b.len() == 4 && crc_ok_a(b) {
                    crc = Some(true);
                    self.encrypted = true;
                    format!(
                        "MIFARE Classic authentication request: key {}, block {}",
                        if b[0] == 0x60 { "A" } else { "B" },
                        b[1]
                    )
                } else if self.encrypted && !crc_ok_a(b) {
                    show = false;
                    "encrypted (MIFARE Classic Crypto1), not decoded".into()
                } else {
                    show = false;
                    format!("frame (reader), {} bytes", b.len())
                }
            }
            ('A', false) => {
                if self.encrypted && !crc_ok_a(b) {
                    show = false;
                    "encrypted (MIFARE Classic Crypto1), not decoded".into()
                } else if b.len() == 2 && f.parity_ok {
                    let atqa = u16::from_le_bytes([b[0], b[1]]);
                    self.cur_atqa = Some(atqa);
                    let size = ["4", "7", "10", "?"][(b[0] >> 6) as usize];
                    format!("ATQA 0x{atqa:04X} ({size}-byte UID)")
                } else if b.len() == 1 || (b.len() == 3 && crc_ok_a(b)) {
                    if b.len() == 3 {
                        crc = Some(true);
                    }
                    let sak = b[0];
                    let mut s = format!("SAK 0x{sak:02X}");
                    s.push_str(if sak & 0x04 == 0 {
                        ", UID complete"
                    } else {
                        ", cascade"
                    });
                    if sak & 0x20 != 0 {
                        s.push_str(", ISO 14443-4");
                    }
                    if sak & 0x04 == 0 && !self.cur_uid.is_empty() {
                        let uid = self.cur_uid.clone();
                        let kind = if sak & 0x20 != 0 {
                            "ISO 14443-A (ISO 14443-4)"
                        } else {
                            "ISO 14443-A"
                        };
                        let (key, _) = self.tag(&uid, kind, f.time);
                        let atqa = self.cur_atqa;
                        if let Some(t) = self.tag_mut(&key) {
                            t.kind = kind.into();
                            t.sak = Some(sak);
                            t.atqa = atqa;
                            t.hint = hint(atqa, sak);
                        }
                        self.event(f.time, format!("NFC: tag {key}, SAK 0x{sak:02X}"), &key);
                    }
                    s
                } else if b.len() == 5 && b.iter().fold(0, |a, x| a ^ x) == 0 {
                    let more = b[0] == 0x88;
                    self.cur_uid
                        .extend_from_slice(if more { &b[1..4] } else { &b[0..4] });
                    if more {
                        "UID (cascade tag, more follows)".to_string()
                    } else {
                        format!("UID {}", hex(&b[..4]))
                    }
                } else if !b.is_empty() && b[0] as usize == b.len() - 2 && crc_ok_a(b) {
                    crc = Some(true);
                    // TL, T0, the interface bytes the T0 announces, then the historical bytes.
                    if let (Some(&t0), false) = (b.get(1), self.cur_uid.is_empty()) {
                        let skip = 2 + (t0 >> 4 & 7).count_ones() as usize;
                        let hist = b.get(skip..b.len() - 2).map(hex);
                        let key = hex(&self.cur_uid);
                        if let Some(t) = self.tag_mut(&key) {
                            t.historical = hist.filter(|h| !h.is_empty());
                        }
                    }
                    "ATS".into()
                } else {
                    show = false;
                    format!("frame (tag), {} bytes", b.len())
                }
            }
            ('B', true) => {
                crc = Some(crc_ok_b(b));
                if b.first() == Some(&0x05) && b.len() >= 3 {
                    format!(
                        "{} AFI 0x{:02X}, {} slots",
                        if b[2] & 0x08 != 0 { "WUPB" } else { "REQB" },
                        b[1],
                        1u32 << (b[2] & 7)
                    )
                } else {
                    show = false;
                    format!("frame (reader), {} bytes", b.len())
                }
            }
            ('B', false) => {
                crc = Some(crc_ok_b(b));
                if b.first() == Some(&0x50) && b.len() >= 14 && crc == Some(true) {
                    let (key, new) = self.tag(&b[1..5], "ISO 14443-B", f.time);
                    if let Some(t) = self.tag_mut(&key) {
                        t.app_data = Some(hex(&b[5..9]));
                    }
                    if new {
                        self.event(f.time, format!("NFC: tag {key}, ATQB"), &key);
                    }
                    format!("ATQB PUPI {key}")
                } else {
                    show = false;
                    format!("frame (tag), {} bytes", b.len())
                }
            }
            ('V', true) => {
                crc = Some(crc_ok_b(b));
                match b.get(1) {
                    Some(0x01) => "Inventory".into(),
                    Some(0x2B) => "Get System Information".into(),
                    Some(0x20 | 0x23) => {
                        show = false;
                        "Read block".into()
                    }
                    _ => "request".into(),
                }
            }
            _ => {
                crc = Some(crc_ok_b(b));
                if crc == Some(true) && b.len() >= 12 && b[0] & 1 == 0 {
                    let mut uid = b[2..10].to_vec();
                    uid.reverse();
                    let maker = sdrkit_util_ids::ic_maker::name(uid[1]).unwrap_or("manufacturer");
                    let (key, new) = self.tag(&uid, &format!("ISO 15693 ({maker})"), f.time);
                    if new {
                        self.event(f.time, format!("NFC: tag {key}, ISO 15693"), &key);
                    }
                    if b.len() == 12 {
                        if let Some(t) = self.tag_mut(&key) {
                            t.dsfid = Some(b[1]);
                        }
                        format!("Inventory response, UID {key}")
                    } else {
                        let info = b[1];
                        let mut k = 10;
                        let mut dsfid = None;
                        let mut afi = None;
                        let mut blocks = None;
                        let mut ic_ref = None;
                        if info & 1 != 0 {
                            dsfid = b.get(k).copied();
                            k += 1;
                        }
                        if info & 2 != 0 {
                            afi = b.get(k).copied();
                            k += 1;
                        }
                        if info & 4 != 0 && k + 1 < b.len() {
                            blocks = Some((b[k] as u16 + 1, (b[k + 1] & 0x1F) as u16 + 1));
                            k += 2;
                        }
                        if info & 8 != 0 {
                            ic_ref = b.get(k).copied();
                        }
                        if let Some(t) = self.tag_mut(&key) {
                            t.dsfid = dsfid.or(t.dsfid);
                            t.afi = afi.or(t.afi);
                            t.blocks = blocks.or(t.blocks);
                            t.ic_ref = ic_ref.or(t.ic_ref);
                        }
                        "System information response".into()
                    }
                } else if b.first().is_some_and(|x| x & 1 != 0) {
                    format!(
                        "error response, code 0x{:02X}",
                        b.get(1).copied().unwrap_or(0)
                    )
                } else {
                    show = false;
                    format!("response, {} bytes", b.len())
                }
            }
        };
        let enc_frame = name.starts_with("encrypted");
        if !enc_frame && (crc == Some(false) || (f.link == 'A' && !f.parity_ok && f.bits != 7)) {
            self.store.failures += 1;
        }
        self.store.log.push(FrameLine {
            time_s: f.time,
            direction: if f.reader { "reader" } else { "tag" }.into(),
            link: match f.link {
                'A' => "Type A",
                'B' => "Type B",
                _ => "ISO 15693",
            }
            .into(),
            hex: if show { hex(b) } else { String::new() },
            name,
            crc_ok: crc,
            parity_ok: f.parity_ok,
            level_dbfs: f.level_db,
        });
    }

    fn publish(&mut self) {
        let s = &self.store;
        let state = if !s.tags.is_empty() {
            2
        } else if s.count > 0 {
            1
        } else {
            0
        };
        let newest = s.tags.iter_newest_first().next().map(|r| r.key);
        self.summary.clear();
        if s.count == 0 {
            self.summary.push_str("no frames");
        } else {
            let _ = write!(self.summary, "{} frames, {} tags", s.count, s.tags.len());
            if let Some(u) = newest {
                let _ = write!(self.summary, " ({u})");
            }
        }
        let p = &mut self.published;
        set_status(p, LockState::from_level(state), &self.summary);
        p.put(&NfcFields {
            frames: s.count as i64,
            tags: s.tags.len() as i64,
            failures: s.failures as i64,
            tag_frames: s.tag_frames as i64,
        });
        shaped::rows(&s.tags, &NFC_TAGS, self.now, p);
        shaped::log_rows(&s.log, &NFC_FRAMES, self.now, p);
    }
}

impl Component for Tags {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "nfc.tags",
            vec![Port::typed::<NfcFrame>("frames")],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if inputs[0].tuned_hz > 0.0 {
            self.carrier_hz = inputs[0].tuned_hz;
        }
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for f in io.items::<NfcFrame>(0) {
            self.frame(&f.value);
            new = true;
        }
        if io.tick() {
            self.now += REPORT_S;
        }
        if new || io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        TagsCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TagsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TagsCommand::Clear => {
                self.store.clear();
                self.published.clear_rows(&NFC_TAGS);
                self.published.clear_rows(&NFC_FRAMES);
            }
            TagsCommand::Remove => commands::remove(
                &mut self.store.tags,
                self.tags_table,
                &mut self.published,
                args,
                &NFC_TAG_ROW,
            )?,
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now)?;
        self.publish();
        Ok(())
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(NFC_TAGS.key) {
            self.store.tags.clear();
            self.published.clear_rows(&NFC_TAGS);
        }
        if !kept.table(NFC_FRAMES.key) {
            self.store.log.clear();
            self.published.clear_rows(&NFC_FRAMES);
        }
        if !kept.field("frames") {
            self.store.count = 0;
        }
        if !kept.field("failures") {
            self.store.failures = 0;
        }
        if !kept.field("tag_frames") {
            self.store.tag_frames = 0;
        }
        self.encrypted = false;
        self.cur_uid.clear();
        self.cur_atqa = None;
        self.publish();
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

sdrkit_core::commands! {
    /// The commands of [`Tags`].
    pub enum TagsCommand {
        /// Forgets everything heard.
        Clear = commands::CLEAR,
        /// Forgets one tag.
        Remove = NFC_REMOVE.command,
    }
}

/// The lowest sample rate: the 847.5 kHz sidebands must fit.
pub const MIN_RATE_HZ: f64 = 2e6;

/// Builds the `nfc` graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn build_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .catalog(catalog)
        .add("detect", Detect::default())
        .add("tags", Tags::default())
        .input("detect")
        .connect("detect", "tags")
        .output("tags.events")
        .clock(REPORT_S)
        .status_from("tags")
        .build()
}

/// The settings of the `nfc` stack: none.
pub fn schema() -> Arc<Schema> {
    Arc::default()
}

/// The `nfc` receiver.
///
/// # Errors
///
/// The errors of [`build_graph`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among them a
/// refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape("nfc"), build_graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE_HZ,
            &min_rate_message("NFC / RFID 13.56 MHz", MIN_RATE_HZ),
        )
        .command("clear", sdrkit_graph::CommandTarget::of("tags", "clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("tags", "remove"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// One frame of the scripted exchange.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptFrame {
    /// From the reader.
    pub reader: bool,
    /// 'A', 'B' or 'V'.
    pub link: char,
    /// The bytes (CRC included).
    pub bytes: Vec<u8>,
    /// A 7-bit short frame.
    pub short: bool,
}

/// The scripted exchange of the generator: Type A, ISO 15693 and Type B sessions with made-up tags.
pub fn script() -> Vec<ScriptFrame> {
    let uid = [0x04, 0xA1, 0xB2, 0xC3];
    let bcc = uid.iter().fold(0, |a, x| a ^ x);
    let with_a = |mut v: Vec<u8>| {
        let c = crc_a(&v);
        v.extend(c.to_le_bytes());
        v
    };
    let with_b = |mut v: Vec<u8>| {
        let c = crc_b(&v);
        v.extend(c.to_le_bytes());
        v
    };
    let mut sel = vec![0x93, 0x70];
    sel.extend(uid);
    sel.push(bcc);
    let mut inv_rsp = vec![0x00, 0x00];
    inv_rsp.extend([0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x04, 0xE0]);
    let mut si_rsp = vec![0x00, 0x0F];
    si_rsp.extend([0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x04, 0xE0]);
    si_rsp.extend([0x00, 0x00, 27, 3, 0x01]);
    let mut uid_a = uid.to_vec();
    uid_a.push(bcc);
    let f = |reader, link, bytes, short| ScriptFrame {
        reader,
        link,
        bytes,
        short,
    };
    vec![
        f(true, 'A', vec![0x26], true),
        f(false, 'A', vec![0x04, 0x00], false),
        f(true, 'A', vec![0x93, 0x20], false),
        f(false, 'A', uid_a, false),
        f(true, 'A', with_a(sel), false),
        f(false, 'A', with_a(vec![0x00]), false),
        f(true, 'A', with_a(vec![0x50, 0x00]), false),
        f(true, 'V', with_b(vec![0x26, 0x01, 0x00]), false),
        f(false, 'V', with_b(inv_rsp), false),
        f(
            true,
            'V',
            with_b(vec![
                0x22, 0x2B, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x04, 0xE0,
            ]),
            false,
        ),
        f(false, 'V', with_b(si_rsp), false),
        f(true, 'B', with_b(vec![0x05, 0x00, 0x00]), false),
        f(
            false,
            'B',
            with_b(vec![
                0x50, 0x11, 0x22, 0x33, 0x44, 0xAA, 0xBB, 0xCC, 0xDD, 0x00, 0x71, 0x85,
            ]),
            false,
        ),
    ]
}

/// How the field is keyed at one sample of a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keying {
    /// The field amplitude, relative to the unmodulated field.
    pub amplitude: f32,
    /// The subcarrier frequency (Hz), 0 for none.
    pub sub_hz: f64,
    /// The subcarrier phase, 0 or a half cycle (BPSK).
    pub sub_half: bool,
}

const FIELD: Keying = Keying {
    amplitude: 1.0,
    sub_hz: 0.0,
    sub_half: false,
};

/// The keying of one frame at `rate`, one value per sample from the frame start (ISO/IEC 14443-2 clauses 8 and 9, ISO/IEC
/// 15693-2 clauses 7 and 8).
pub fn frame_waveform(
    reader: bool,
    link: char,
    bytes: &[u8],
    short: bool,
    rate: f64,
) -> Vec<Keying> {
    let etu = ETU * rate;
    let mut w: Vec<Keying> = Vec::new();
    let put = |w: &mut Vec<Keying>, from_etu: f64, to_etu: f64, v: Keying| {
        let a = (from_etu * etu).round() as usize;
        let b = (to_etu * etu).round() as usize;
        if w.len() < b {
            w.resize(b, FIELD);
        }
        for x in w[a..b].iter_mut() {
            *x = v;
        }
    };
    let pause = Keying {
        amplitude: 0.02,
        ..FIELD
    };
    match (reader, link) {
        (true, 'A') => {
            let bits: Vec<u8> = if short {
                (0..7).map(|i| (bytes[0] >> i) & 1).collect()
            } else {
                a_bits(bytes)
            };
            let mut chips = Vec::new();
            modified_miller_encode(&bits, &mut chips);
            let len = 2.5e-6 / ETU;
            for (h, &c) in chips.iter().enumerate() {
                if c == 0 {
                    put(&mut w, h as f64 * 0.5, h as f64 * 0.5 + len, pause);
                }
            }
            let end = chips.len() as f64 * 0.5;
            put(&mut w, end, end + 1.0, FIELD);
        }
        (true, 'V') => {
            // SOF: pauses at 0 and 6 etu. Data: 1-of-4 symbols of 8 etu from 8 etu, one etu per chip. EOF: one pause in slot 2.
            put(&mut w, 0.0, 1.0, pause);
            put(&mut w, 6.0, 7.0, pause);
            let mut bits = Vec::with_capacity(8 * bytes.len());
            sdrkit_util_bits::pack::unpack(bytes, BitOrder::Lsb, &mut bits);
            let mut chips = Vec::new();
            let _ = ppm_encode(&bits, 2, &mut chips);
            for (i, &c) in chips.iter().enumerate() {
                if c == 0 {
                    put(&mut w, 8.0 + i as f64, 9.0 + i as f64, pause);
                }
            }
            let s = 8.0 + chips.len() as f64;
            put(&mut w, s + 2.0, s + 3.0, pause);
            put(&mut w, s + 8.0, s + 9.0, FIELD);
        }
        (true, _) => {
            // Type B reader: 10 % ASK NRZ-L, SOF, characters, EOF.
            let lo = Keying {
                amplitude: 0.9,
                ..FIELD
            };
            put(&mut w, 0.0, 10.0, lo);
            put(&mut w, 10.0, 12.0, FIELD);
            let mut bits = Vec::new();
            uart_encode(bytes, &mut bits);
            for (i, &b) in bits.iter().enumerate() {
                let t = 12.0 + i as f64;
                put(&mut w, t, t + 1.0, if b == 1 { FIELD } else { lo });
            }
            let t = 12.0 + bits.len() as f64;
            put(&mut w, t, t + 10.0, lo);
            put(&mut w, t + 10.0, t + 11.0, FIELD);
        }
        (false, 'A') => {
            // Manchester on the 847.5 kHz subcarrier: SOF one etu on, a 1 on in the first half, a 0 in the second.
            let on = Keying {
                sub_hz: SUB_A,
                ..FIELD
            };
            put(&mut w, 0.0, 1.0, on);
            for (k, &b) in a_bits(bytes).iter().enumerate() {
                let s = 1.0 + k as f64;
                let (first, second) = if b != 0 { (on, FIELD) } else { (FIELD, on) };
                put(&mut w, s, s + 0.5, first);
                put(&mut w, s + 0.5, s + 1.0, second);
            }
            let e = 1.0 + 9.0 * bytes.len() as f64;
            put(&mut w, e, e + 1.0, FIELD);
        }
        (false, 'V') => {
            // Manchester on the 423.75 kHz subcarrier, 4 etu per bit: a 0 is on first, a 1 on second.
            let on = Keying {
                sub_hz: SUB_V,
                ..FIELD
            };
            put(&mut w, 0.0, 4.0, on);
            let mut bits = Vec::with_capacity(8 * bytes.len());
            sdrkit_util_bits::pack::unpack(bytes, BitOrder::Lsb, &mut bits);
            for (k, &b) in bits.iter().enumerate() {
                let s = 4.0 + 4.0 * k as f64;
                let (first, second) = if b != 0 { (FIELD, on) } else { (on, FIELD) };
                put(&mut w, s, s + 2.0, first);
                put(&mut w, s + 2.0, s + 4.0, second);
            }
            let e = 4.0 + 4.0 * bits.len() as f64;
            put(&mut w, e, e + 4.0, FIELD);
        }
        _ => {
            // Type B tag: BPSK of 847.5 kHz; TR1 of logic 1, SOF, characters, EOF.
            let one = Keying {
                sub_hz: SUB_A,
                ..FIELD
            };
            let zero = Keying {
                sub_half: true,
                ..one
            };
            put(&mut w, 0.0, 20.0, one);
            put(&mut w, 20.0, 30.0, zero);
            put(&mut w, 30.0, 32.0, one);
            let mut bits = Vec::new();
            uart_encode(bytes, &mut bits);
            for (i, &b) in bits.iter().enumerate() {
                let t = 32.0 + i as f64;
                put(&mut w, t, t + 1.0, if b == 1 { one } else { zero });
            }
            let t = 32.0 + bits.len() as f64;
            put(&mut w, t, t + 10.0, zero);
            put(&mut w, t + 10.0, t + 11.0, FIELD);
        }
    }
    w
}

/// The samples of a frame's keying from sample `base` of the stream: the field at amplitude `field`, a tag's subcarrier as
/// on-off load modulation of depth 0.35 (a square wave, so no trigonometry per sample).
pub fn keyed_samples(w: &[Keying], base: usize, rate: f64, field: f32, out: &mut Vec<Complex32>) {
    for (k, v) in w.iter().enumerate() {
        let mut a = field * v.amplitude;
        if v.sub_hz > 0.0 {
            let cycles = v.sub_hz * (base + k) as f64 / rate + if v.sub_half { 0.5 } else { 0.0 };
            let frac = cycles.fract();
            if !(0.25..0.75).contains(&frac) {
                a *= 1.35;
            }
        }
        out.push(Complex32::new(a, 0.0));
    }
}

/// The test signal options.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GenParams {
    /// Only reader frames (a capture beside a reader).
    pub reader_only: bool,
}

/// The field amplitude of the generator: unit power while the field is on.
pub const GEN_FIELD: f32 = 1.0;

/// The `nfc` generator: the field, reader pauses to 2 %, tag load modulation of depth 0.35; the session loops. Clean.
///
/// # Errors
///
/// [`ConfigError::RateTooLow`] below [`MIN_RATE_HZ`], and the errors of the scene.
pub fn generator(rate: f64, p: GenParams) -> Result<CleanScene, ConfigError> {
    if !(rate >= MIN_RATE_HZ && rate.is_finite()) {
        return Err(ConfigError::RateTooLow {
            need: MIN_RATE_HZ,
            got: rate,
        });
    }
    let gap = (300e-6 * rate) as usize;
    let mut session: Vec<Complex32> = Vec::new();
    for f in script().into_iter().filter(|f| !p.reader_only || f.reader) {
        session.extend(std::iter::repeat_n(Complex32::new(GEN_FIELD, 0.0), gap));
        let w = frame_waveform(f.reader, f.link, &f.bytes, f.short, rate);
        let base = session.len();
        keyed_samples(&w, base, rate, GEN_FIELD, &mut session);
    }
    session.extend(std::iter::repeat_n(Complex32::new(GEN_FIELD, 0.0), gap * 4));
    let period = session.len() as f64 / rate;
    let mut scene = Scene::new(rate)?;
    scene.add_repeating(0.0, period, 0.0, 0.0, session)?;
    Ok(CleanScene::new(
        scene,
        rate,
        SignalInfo::new(2.0 * SUB_A + 2.0 / ETU).with_symbol_rate(1.0 / ETU),
    ))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator(cfg.rate_hz, GenParams::default())?))
}

/// NFC / RFID at 13.56 MHz.
pub static NFC: StackDescriptor = StackDescriptor {
    id: "nfc",
    name: "NFC / RFID 13.56 MHz",
    group: "Contactless, 13.56 MHz",
    blurb: "Contactless cards and tags at 13.56 MHz: the tag type and the UID broadcast during anticollision and inventory, and the name of every frame.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 13.553e6,
        max_hz: 13.567e6,
        carrier_at_centre: true,
        not_circular: true,
        no_offset_tuning: true,
        ..Tuning::anywhere(13.56e6, 4e6, MIN_RATE_HZ, 2.4e6)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Nfc],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, Type A, Type B and ISO 15693 sessions", o, 0.0);
    c.seconds = o.seconds.max(0.5);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = NfcFields::read(r.fields());
        let tags = f.as_ref().map_or(0, |f| f.tags);
        let frames = f.as_ref().map_or(0, |f| f.frames);
        vec![
            CheckLine::new("tags", tags >= 3, format!("{tags}, at least 3 expected")),
            CheckLine::new(
                "frames",
                frames >= 13,
                format!("{frames}, at least 13 expected"),
            ),
        ]
    });
    vec![c]
}

/// The test-signal cases of `nfc`.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmarks of `nfc`: the whole receiver at 4 Msps on the test signal (`ism.nfc_receiver_4m`).
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "ism.nfc_receiver_4m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = 4e6;
    let mut x = vec![Complex32::default(); (rate * 0.1) as usize];
    if let Ok(mut g) = generator(rate, GenParams { reader_only: false }) {
        g.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(Values::defaults(&schema()))) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let _ = rx.configure(&sdrkit_core::InputInfo::new(rate, 13.56e6, 13.56e6).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 0.1, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}
