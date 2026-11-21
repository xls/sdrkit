//! Voice codec 4 frame fields: packing by bit class, unpacking, the damaged-class rules and the SID frame.

use super::layout::{FieldKind, Layout, MAX_FIELDS};
use super::lpc::stabilise;
use super::tables::LSF_MEAN;
use sdrkit_util_bits::{BitReader, BitWriter};

/// The decoded fields of one speech frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Params {
    pub lsf: [u32; 10],
    pub interp: u32,
    pub lag: [u32; 8],
    pub gp: [u32; 8],
    pub gc: [u32; 8],
    pub pulse: [u64; 8],
    pub hb_level: [u32; 2],
    pub hb_shape: [[u32; 6]; 2],
    pub hb_time: [[u32; 4]; 2],
    /// Subframes whose pulse field is damaged.
    pub pulse_bad: [bool; 8],
    /// True when a time-envelope field is damaged.
    pub time_bad: bool,
}

/// Which classes a container reports as damaged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Damage {
    /// Class A is damaged. The decoder treats the frame as lost.
    pub a: bool,
    /// Class B is damaged.
    pub b: bool,
    /// Class C is damaged.
    pub c: bool,
}

/// Walks the fields in order and calls `f` with the field kind and a mutable value.
fn walk(lay: &Layout, p: &mut Params, mut f: impl FnMut(FieldKind, &mut u64)) {
    let per_half = lay.s / lay.halves;
    let mut next_sf = 0usize;
    let mut sf = 0usize;
    let mut half = 0usize;
    let mut shape_i = 0usize;
    let mut time_i = 0usize;
    for field in lay.fields() {
        let slot: &mut u64 = &mut 0;
        let kind = field.kind;
        match kind {
            FieldKind::Lsf(k) => *slot = p.lsf[k as usize] as u64,
            FieldKind::Interp => *slot = p.interp as u64,
            FieldKind::FirstLag | FieldKind::DeltaLag => {
                sf = next_sf;
                next_sf += 1;
                *slot = p.lag[sf] as u64;
            }
            FieldKind::PitchGain => *slot = p.gp[sf] as u64,
            FieldKind::CodeGain => *slot = p.gc[sf] as u64,
            FieldKind::Pulse(s) => *slot = p.pulse[s as usize],
            FieldKind::HbLevel => {
                half = sf / per_half;
                shape_i = 0;
                time_i = 0;
                *slot = p.hb_level[half] as u64;
            }
            FieldKind::HbShape => *slot = p.hb_shape[half][shape_i] as u64,
            FieldKind::HbTime => *slot = p.hb_time[half][time_i] as u64,
        }
        f(kind, slot);
        let v = *slot;
        match kind {
            FieldKind::Lsf(k) => p.lsf[k as usize] = v as u32,
            FieldKind::Interp => p.interp = v as u32,
            FieldKind::FirstLag | FieldKind::DeltaLag => p.lag[sf] = v as u32,
            FieldKind::PitchGain => p.gp[sf] = v as u32,
            FieldKind::CodeGain => p.gc[sf] = v as u32,
            FieldKind::Pulse(s) => p.pulse[s as usize] = v,
            FieldKind::HbLevel => p.hb_level[half] = v as u32,
            FieldKind::HbShape => {
                p.hb_shape[half][shape_i] = v as u32;
                shape_i += 1;
            }
            FieldKind::HbTime => {
                p.hb_time[half][time_i] = v as u32;
                time_i += 1;
            }
        }
    }
}

/// Packs the speech fields into `out` (cleared first): the type bit if it is in band, then class A, B and C, then the pad.
pub(crate) fn pack(lay: &Layout, p: &Params, out: &mut Vec<u8>) {
    out.clear();
    let mut vals = [0u64; MAX_FIELDS];
    let mut n = 0;
    let mut pc = *p;
    walk(lay, &mut pc, |_, v| {
        vals[n] = *v;
        n += 1;
    });
    let fields = lay.fields();
    let mut w = BitWriter::new(out);
    if lay.cfg.type_in_band {
        w.put(0, 1);
    }
    for (i, f) in fields.iter().enumerate() {
        if f.split.a > 0 {
            w.put(vals[i] >> (f.split.b + f.split.c), f.split.a);
        }
    }
    for (i, f) in fields.iter().enumerate() {
        if f.split.b > 0 {
            w.put(
                (vals[i] >> f.split.c) & ((1u64 << f.split.b) - 1),
                f.split.b,
            );
        }
    }
    for (i, f) in fields.iter().enumerate() {
        if f.split.c > 0 {
            let mask = if f.split.c >= 64 {
                u64::MAX
            } else {
                (1u64 << f.split.c) - 1
            };
            w.put(vals[i] & mask, f.split.c);
        }
    }
    let mut pad = lay.pad_bits;
    while pad > 0 {
        let k = pad.min(32);
        w.put(0, k);
        pad -= k;
    }
    w.finish();
    out.resize(lay.speech_bytes(), 0);
}

/// Unpacks the speech fields of `data` and applies the damaged-class rules of section 3.3.1 for classes B and C.
pub(crate) fn unpack(lay: &Layout, data: &[u8], damage: Damage) -> Params {
    let fields = lay.fields();
    let mut vals = [0u64; MAX_FIELDS];
    let mut r = BitReader::new(data);
    if lay.cfg.type_in_band {
        r.skip(1);
    }
    for (i, f) in fields.iter().enumerate() {
        if f.split.a > 0 {
            vals[i] = r.u(f.split.a);
        }
    }
    for (i, f) in fields.iter().enumerate() {
        if f.split.b > 0 {
            vals[i] = (vals[i] << f.split.b) | r.u(f.split.b);
        }
    }
    for (i, f) in fields.iter().enumerate() {
        if f.split.c > 0 {
            vals[i] = (vals[i] << f.split.c) | r.u(f.split.c);
        }
    }
    let mut p = Params::default();
    let mut i = 0;
    let mut pulse_bad = [false; 8];
    let mut time_bad = false;
    walk(lay, &mut p, |kind, v| {
        let f = fields[i];
        let mut val = vals[i];
        i += 1;
        let hit = (damage.b && f.split.b > 0) || (damage.c && f.split.c > 0);
        if hit {
            match kind {
                FieldKind::Pulse(s) => pulse_bad[s as usize] = true,
                FieldKind::HbTime => time_bad = true,
                _ => {
                    let low = if damage.b {
                        f.split.b + f.split.c
                    } else {
                        f.split.c
                    };
                    if low > 0 {
                        val = (val >> low) << low;
                        val |= 1u64 << (low - 1);
                    }
                }
            }
        }
        *v = val;
    });
    p.pulse_bad = pulse_bad;
    p.time_bad = time_bad;
    p
}

/// The steps of the SID LSF quantiser.
pub(crate) const SID_STEP: [f64; 10] = [0.05, 0.07, 0.09, 0.10, 0.10, 0.10, 0.10, 0.09, 0.12, 0.10];

/// The parameters of a SID frame (section 3.7.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Sid {
    pub lsf: [f64; 10],
    pub res: f64,
    pub hb: f64,
    pub tilt: f64,
}

impl Sid {
    /// The reset values: the LSF mean, 0 dB, -100 dB and 0 dB.
    pub(crate) fn reset() -> Self {
        Self {
            lsf: LSF_MEAN,
            res: 0.0,
            hb: -100.0,
            tilt: 0.0,
        }
    }
}

/// The SID field indices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SidIdx {
    pub lsf: [u32; 10],
    pub res: u32,
    pub hb: u32,
    pub tilt: u32,
}

pub(crate) fn sid_bits(i: usize) -> u32 {
    if i < 8 {
        3
    } else {
        2
    }
}

/// Reads the 40 SID bits that start at bit `offset`.
pub(crate) fn sid_unpack(data: &[u8], offset: usize) -> SidIdx {
    let mut r = BitReader::new(data);
    r.skip(offset);
    let mut s = SidIdx::default();
    for i in 0..10 {
        s.lsf[i] = r.u(sid_bits(i)) as u32;
    }
    s.res = r.u(6) as u32;
    s.hb = r.u(5) as u32;
    s.tilt = r.u(1) as u32;
    s
}

/// Writes the 40 SID bits.
pub(crate) fn sid_pack(s: &SidIdx, w: &mut BitWriter<'_>) {
    for i in 0..10 {
        w.put(s.lsf[i] as u64, sid_bits(i));
    }
    w.put(s.res as u64, 6);
    w.put(s.hb as u64, 5);
    w.put(s.tilt as u64, 1);
}

/// Dequantises the SID indices.
pub(crate) fn sid_dequant(s: &SidIdx) -> Sid {
    let mut lsf = [0f64; 10];
    for i in 0..10 {
        let mid = if i < 8 { 3.5 } else { 1.5 };
        lsf[i] = LSF_MEAN[i] + (s.lsf[i] as f64 - mid) * SID_STEP[i];
    }
    stabilise(&mut lsf, 0.025);
    Sid {
        lsf,
        res: s.res as f64 * 1.5,
        hb: if s.hb == 0 { -100.0 } else { s.hb as f64 * 3.0 },
        tilt: if s.tilt == 0 { -9.0 } else { 0.0 },
    }
}
