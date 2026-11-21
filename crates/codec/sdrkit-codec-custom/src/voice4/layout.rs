//! The voice codec 4 configuration and the frame layout that follows from it: tier, field sizes, pulse counts and bit classes.

use super::pvq;
use crate::{Error, Param};

/// The input and output sample rate of voice codec 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SampleRate {
    /// 8000 samples per second. The core only, no high band.
    Hz8000,
    /// 16000 samples per second. The core plus the parametric high band.
    Hz16000,
}

impl SampleRate {
    /// The rate in samples per second.
    pub const fn hz(self) -> u32 {
        match self {
            SampleRate::Hz8000 => 8000,
            SampleRate::Hz16000 => 16000,
        }
    }
}

/// The voice codec 4 frame length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrameLength {
    /// 20 ms frames.
    Ms20,
    /// 40 ms frames.
    Ms40,
}

impl FrameLength {
    /// The length in milliseconds.
    pub const fn ms(self) -> u32 {
        match self {
            FrameLength::Ms20 => 20,
            FrameLength::Ms40 => 40,
        }
    }
}

/// The voice codec 4 bit budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Profile {
    /// 3.2 kbit/s (64 bits per 20 ms). Only with 40 ms frames.
    P3_2,
    /// 4.8 kbit/s (96 bits per 20 ms).
    P4_8,
    /// 6.0 kbit/s (120 bits per 20 ms).
    P6_0,
    /// 8.0 kbit/s (160 bits per 20 ms).
    P8_0,
    /// 12 kbit/s (240 bits per 20 ms).
    Hq12,
    /// 16 kbit/s (320 bits per 20 ms).
    Hq16,
    /// A custom payload size in bits per frame. The size per 20 ms must be in 64 ..= 400.
    Custom(u16),
}

impl Profile {
    fn bits_per_20ms(self) -> Option<u32> {
        Some(match self {
            Profile::P3_2 => 64,
            Profile::P4_8 => 96,
            Profile::P6_0 => 120,
            Profile::P8_0 => 160,
            Profile::Hq12 => 240,
            Profile::Hq16 => 320,
            Profile::Custom(_) => return None,
        })
    }
}

/// The settings that the encoder and the decoder share.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Config {
    /// The sample rate.
    pub rate: SampleRate,
    /// The frame length.
    pub frame: FrameLength,
    /// The bit budget.
    pub profile: Profile,
    /// If true, bit 0 of the payload marks the frame as speech (0) or SID (1).
    pub type_in_band: bool,
}

/// One row of the tier table (section 3.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tier {
    pub min_b20: u32,
    pub l: usize,
    pub lsf_bits: u32,
    pub res: u32,
    pub lagd_bits: u32,
    pub gp_bits: u32,
    pub gc_bits: u32,
    pub hb_level_bits: u32,
    pub hb_mode: u32,
    pub hb_time_bits: u32,
    pub gn: f64,
    pub gd: f64,
    pub pitch_strength: f64,
}

const fn tier(
    min_b20: u32,
    l: usize,
    lsf_bits: u32,
    res: u32,
    lagd_bits: u32,
    gp_bits: u32,
    gc_bits: u32,
    hb: (u32, u32, u32),
    pf: (f64, f64, f64),
) -> Tier {
    Tier {
        min_b20,
        l,
        lsf_bits,
        res,
        lagd_bits,
        gp_bits,
        gc_bits,
        hb_level_bits: hb.0,
        hb_mode: hb.1,
        hb_time_bits: hb.2,
        gn: pf.0,
        gd: pf.1,
        pitch_strength: pf.2,
    }
}

const TIERS: [Tier; 6] = [
    tier(60, 80, 20, 1, 3, 3, 3, (4, 0, 2), (0.55, 0.72, 0.50)),
    tier(80, 80, 22, 1, 4, 3, 4, (4, 0, 2), (0.55, 0.72, 0.50)),
    tier(110, 80, 24, 2, 4, 3, 4, (5, 1, 2), (0.55, 0.72, 0.50)),
    tier(140, 40, 26, 2, 4, 3, 4, (5, 1, 2), (0.60, 0.75, 0.40)),
    tier(200, 40, 28, 2, 5, 4, 5, (5, 1, 2), (0.65, 0.75, 0.30)),
    tier(280, 40, 32, 2, 5, 4, 5, (5, 3, 2), (0.70, 0.75, 0.25)),
];

/// The Lloyd-Max distortion of a unit Gaussian for 0 to 6 bits (section 3.2.1).
const LLOYD_D: [f64; 7] = [1.0, 0.3634, 0.1175, 0.03454, 0.009497, 0.002499, 0.000642];

/// The class split of one field: its class-A, class-B and class-C bit counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Split {
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

/// The kind of a field in the walk order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FieldKind {
    Lsf(u8),
    Interp,
    FirstLag,
    DeltaLag,
    PitchGain,
    CodeGain,
    Pulse(u8),
    HbLevel,
    HbShape,
    HbTime,
}

/// One field of a speech frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Field {
    pub kind: FieldKind,
    pub split: Split,
}

/// The largest number of fields in a frame.
pub(crate) const MAX_FIELDS: usize = 10 + 1 + 8 * 4 + 2 * (1 + 6 + 4);

/// The complete layout of a voice codec 4 frame for one configuration (Appendix A).
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub(crate) cfg: Config,
    /// The frame length in ms.
    pub frame_ms: u32,
    /// Samples per frame at the codec rate.
    pub frame_samples: usize,
    /// Core samples per frame (8 kHz).
    pub nc: usize,
    /// The subframe length in core samples.
    pub l: usize,
    /// Subframes per frame.
    pub s: usize,
    /// 20 ms halves per frame.
    pub halves: usize,
    /// The payload size in bits.
    pub payload_bits: u32,
    /// The payload size in bytes.
    pub payload_bytes: usize,
    /// The number of speech bits (the payload without the type bit).
    pub speech_bits: u32,
    /// LSF bits per KLT coefficient.
    pub lsf_bits: [u32; 10],
    /// Interpolation index bits (2 for 40 ms frames, else 0).
    pub interp_bits: u32,
    /// The lag resolution: 1 for integer lags, 2 for half-sample lags.
    pub res: u32,
    /// First-lag field bits.
    pub lag0_bits: u32,
    /// Delta-lag field bits.
    pub lagd_bits: u32,
    /// Pitch-gain index bits.
    pub gp_bits: u32,
    /// Code-gain index bits.
    pub gc_bits: u32,
    /// True when the high band is coded (16 kHz only).
    pub hb_on: bool,
    /// High-band level bits.
    pub hb_level_bits: u32,
    /// High-band shape mode (0 to 3).
    pub hb_mode: u32,
    /// High-band time-envelope bits per block.
    pub hb_time_bits: u32,
    /// Pulses per subframe.
    pub pulses: [u32; 8],
    /// Pulse index bits per subframe.
    pub pulse_bits: [u32; 8],
    /// Zero bits at the end of the speech bits.
    pub pad_bits: u32,
    /// Class-A bits (with the type bit when it is in band).
    pub class_a: u32,
    /// Class-B bits.
    pub class_b: u32,
    /// Class-C bits.
    pub class_c: u32,
    /// The size of a SID frame in bytes.
    pub sid_bytes: usize,
    /// The decoder output delay against the encoder input, in samples, without the noise suppressor.
    pub delay: usize,
    pub(crate) table: usize,
    pub(crate) rho: f64,
    pub(crate) tier: Tier,
    /// b20 without the type bit, for the enhancer tier.
    pub(crate) enh_b20: u32,
    pub(crate) fields: [Field; MAX_FIELDS],
    pub(crate) nfields: usize,
}

fn shape_bits(mode: u32) -> u32 {
    match mode {
        0 => 3,
        1 => 5,
        2 => 12,
        _ => 18,
    }
}

/// The greedy LSF bit allocation of section 3.2.1.
pub(crate) fn lsf_allocation(total: u32, table: usize) -> [u32; 10] {
    let sd = &super::tables::KLT_SD[table];
    let mut b = [0u32; 10];
    for _ in 0..total {
        let mut best = usize::MAX;
        let mut best_v = f64::NEG_INFINITY;
        for k in 0..10 {
            if b[k] >= 6 {
                continue;
            }
            let v = sd[k] * sd[k] * (LLOYD_D[b[k] as usize] - LLOYD_D[b[k] as usize + 1]);
            if best == usize::MAX || v > best_v {
                best = k;
                best_v = v;
            }
        }
        if best == usize::MAX {
            break;
        }
        b[best] += 1;
    }
    b
}

fn split_for(kind: FieldKind, width: u32) -> Split {
    let (a_share, b_all) = match kind {
        FieldKind::Lsf(k) if k < 3 => (2, true),
        FieldKind::Lsf(k) if k < 6 => (1, true),
        FieldKind::Lsf(_) => (0, true),
        FieldKind::Interp => (u32::MAX, true),
        FieldKind::FirstLag => (5, true),
        FieldKind::DeltaLag => (1, true),
        FieldKind::PitchGain => (2, true),
        FieldKind::CodeGain => (3, true),
        FieldKind::HbLevel => (3, true),
        FieldKind::HbShape => (0, true),
        FieldKind::Pulse(_) | FieldKind::HbTime => (0, false),
    };
    let a = a_share.min(width);
    if b_all {
        Split {
            a,
            b: width - a,
            c: 0,
        }
    } else {
        Split {
            a,
            b: 0,
            c: width - a,
        }
    }
}

impl Layout {
    /// Computes the layout of a configuration.
    ///
    /// Errors: [`Error::BadConfig`] when the budget is out of range, when 3.2 kbit/s or a small budget is used with 20 ms frames,
    /// when the side information leaves fewer than 6 bits per subframe for the pulses, or when a pulse index needs more than
    /// 63 bits.
    pub fn new(cfg: Config) -> Result<Self, Error> {
        let frame_ms = cfg.frame.ms();
        let bc = match cfg.profile.bits_per_20ms() {
            Some(b) => b * frame_ms / 20,
            None => match cfg.profile {
                Profile::Custom(b) => b as u32,
                _ => 0,
            },
        };
        let b20 = bc * 20 / frame_ms;
        if !(64..=400).contains(&b20) {
            return Err(Error::BadConfig {
                param: Param::Budget,
            });
        }
        if b20 < 72 && frame_ms != 40 {
            return Err(Error::BadConfig {
                param: Param::BudgetFrameLength,
            });
        }
        let b = if cfg.type_in_band { bc - 1 } else { bc };
        let tier = *TIERS
            .iter()
            .rev()
            .find(|t| t.min_b20 <= b20)
            .ok_or(Error::BadConfig { param: Param::Tier })?;
        let nc = 8 * frame_ms as usize;
        let l = tier.l;
        let s = nc / l;
        let halves = (frame_ms / 20) as usize;
        let interp_bits = if frame_ms == 40 { 2 } else { 0 };
        let rho = if frame_ms == 20 { 0.6 } else { 0.45 };
        let table = if frame_ms == 20 { 0 } else { 1 };
        let hb_on = cfg.rate == SampleRate::Hz16000;
        let lag0_bits = if tier.res == 2 { 8 } else { 7 };
        let hb_bits = if hb_on {
            tier.hb_level_bits + shape_bits(tier.hb_mode) + 4 * tier.hb_time_bits
        } else {
            0
        };
        let lsf_bits = lsf_allocation(tier.lsf_bits, table);
        let per_half_sub = s / halves;
        let side = tier.lsf_bits
            + interp_bits
            + halves as u32 * (lag0_bits + (per_half_sub as u32 - 1) * tier.lagd_bits)
            + s as u32 * (tier.gp_bits + tier.gc_bits)
            + halves as u32 * hb_bits;
        if b < side || b - side < 6 * s as u32 {
            return Err(Error::BadConfig {
                param: Param::PulseBits,
            });
        }
        let rem = b - side;
        let mut pulses = [0u32; 8];
        let mut pulse_bits = [0u32; 8];
        let mut used = 0u32;
        for sf in 0..s {
            let share = (rem - used) / (s - sf) as u32;
            let mut k = 0;
            for kk in 0..=64 {
                if pvq::index_bits(l, kk) <= share {
                    k = kk;
                }
            }
            pulses[sf] = k as u32;
            pulse_bits[sf] = pvq::index_bits(l, k);
            used += pulse_bits[sf];
        }
        loop {
            let mut grew = false;
            for sf in 0..s {
                let k = pulses[sf] as usize;
                if k >= 64 {
                    continue;
                }
                let nb = pvq::index_bits(l, k + 1);
                let d = nb.saturating_sub(pulse_bits[sf]);
                if nb != 99 && d <= rem - used {
                    used += d;
                    pulses[sf] += 1;
                    pulse_bits[sf] = nb;
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        let pad_bits = rem - used;
        if pulse_bits[..s].iter().any(|&b| b > 63) {
            return Err(Error::BadConfig {
                param: Param::PulseIndexBits,
            });
        }

        let mut fields = [Field {
            kind: FieldKind::Interp,
            split: Split::default(),
        }; MAX_FIELDS];
        let mut n = 0;
        let mut push = |kind: FieldKind, width: u32| {
            fields[n] = Field {
                kind,
                split: split_for(kind, width),
            };
            n += 1;
        };
        for (k, &bits) in lsf_bits.iter().enumerate() {
            push(FieldKind::Lsf(k as u8), bits);
        }
        if interp_bits > 0 {
            push(FieldKind::Interp, interp_bits);
        }
        for h in 0..halves {
            for j in 0..per_half_sub {
                let sf = h * per_half_sub + j;
                if j == 0 {
                    push(FieldKind::FirstLag, lag0_bits);
                } else {
                    push(FieldKind::DeltaLag, tier.lagd_bits);
                }
                push(FieldKind::PitchGain, tier.gp_bits);
                push(FieldKind::CodeGain, tier.gc_bits);
                if pulse_bits[sf] > 0 {
                    push(FieldKind::Pulse(sf as u8), pulse_bits[sf]);
                }
            }
            if hb_on {
                push(FieldKind::HbLevel, tier.hb_level_bits);
                match tier.hb_mode {
                    0 => push(FieldKind::HbShape, 3),
                    1 => {
                        push(FieldKind::HbShape, 3);
                        push(FieldKind::HbShape, 2);
                    }
                    m => {
                        for _ in 0..6 {
                            push(FieldKind::HbShape, if m == 2 { 2 } else { 3 });
                        }
                    }
                }
                if tier.hb_time_bits > 0 {
                    for _ in 0..4 {
                        push(FieldKind::HbTime, tier.hb_time_bits);
                    }
                }
            }
        }
        let nfields = n;
        let mut class_a = u32::from(cfg.type_in_band);
        let mut class_b = 0;
        let mut class_c = 0;
        for f in &fields[..nfields] {
            class_a += f.split.a;
            class_b += f.split.b;
            class_c += f.split.c;
        }
        let payload_bytes = bc.div_ceil(8) as usize;
        let sid_bytes = if cfg.type_in_band { payload_bytes } else { 5 };
        let delay = if hb_on { 173 } else { 40 };
        Ok(Self {
            cfg,
            frame_ms,
            frame_samples: (cfg.rate.hz() * frame_ms / 1000) as usize,
            nc,
            l,
            s,
            halves,
            payload_bits: bc,
            payload_bytes,
            speech_bits: b,
            lsf_bits,
            interp_bits,
            res: tier.res,
            lag0_bits,
            lagd_bits: tier.lagd_bits,
            gp_bits: tier.gp_bits,
            gc_bits: tier.gc_bits,
            hb_on,
            hb_level_bits: tier.hb_level_bits,
            hb_mode: tier.hb_mode,
            hb_time_bits: tier.hb_time_bits,
            pulses,
            pulse_bits,
            pad_bits,
            class_a,
            class_b,
            class_c,
            sid_bytes,
            delay,
            table,
            rho,
            tier,
            enh_b20: b * 20 / frame_ms,
            fields,
            nfields,
        })
    }

    /// The configuration of this layout.
    pub fn config(&self) -> Config {
        self.cfg
    }

    /// The size of a speech frame in bytes: the payload with the type bit in band, else ceil(B / 8).
    pub fn speech_bytes(&self) -> usize {
        if self.cfg.type_in_band {
            self.payload_bytes
        } else {
            self.speech_bits.div_ceil(8) as usize
        }
    }

    pub(crate) fn fields(&self) -> &[Field] {
        &self.fields[..self.nfields]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Section 3.2.1: the allocation results for every total, both tables.
    #[test]
    fn lsf_allocations_match_the_table() {
        let rows: [(u32, [u32; 10], [u32; 10]); 6] = [
            (
                20,
                [4, 3, 3, 2, 2, 2, 2, 1, 1, 0],
                [4, 3, 3, 2, 2, 2, 2, 1, 1, 0],
            ),
            (
                22,
                [5, 4, 3, 2, 2, 2, 2, 1, 1, 0],
                [5, 4, 3, 2, 2, 2, 2, 1, 1, 0],
            ),
            (
                24,
                [5, 4, 3, 3, 2, 2, 2, 2, 1, 0],
                [5, 4, 3, 3, 2, 2, 2, 2, 1, 0],
            ),
            (
                26,
                [5, 4, 3, 3, 2, 2, 2, 2, 2, 1],
                [5, 4, 3, 3, 2, 2, 2, 2, 2, 1],
            ),
            (
                28,
                [5, 4, 4, 3, 3, 2, 2, 2, 2, 1],
                [5, 4, 4, 3, 3, 2, 2, 2, 2, 1],
            ),
            (
                32,
                [5, 4, 4, 3, 3, 3, 3, 3, 2, 2],
                [5, 5, 4, 4, 3, 3, 3, 2, 2, 1],
            ),
        ];
        for (t, a, b) in rows {
            assert_eq!(lsf_allocation(t, 0), a, "table 0 total {t}");
            assert_eq!(lsf_allocation(t, 1), b, "table 1 total {t}");
        }
    }
}
