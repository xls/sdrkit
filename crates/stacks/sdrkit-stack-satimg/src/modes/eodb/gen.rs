//! The test signal of `eodb`: each profile's test picture packed as its instrument sends it, in packets, frames and CADUs,
//! coded, differentially coded and modulated as the profile says. The CADUs alone make a frame file.

use super::profiles::{Conv, Diff, Header, Instrument, Profile, Rand};
use crate::gen::{space_packet, BitSource, CcsdsCoder, CodeOptions, GenError, ModParams, Modulator, ZoneStream};
use crate::instruments::avhrr_dump::AvhrrSource;
use crate::instruments::direct::{TestPacket, TestSource};
use crate::instruments::modis::ModisSource;
use crate::instruments::small::{Amsr2Source, MetimageSource, TirsSource, VegetationSource};
use crate::instruments::symbols::{ocm_pn, ocm_test_frame, quadrant, KanopusSource, OlsSource};
use crate::instruments::viirs::ViirsSource;
use crate::psk::Modulation;
use sdrkit_core::{Complex32, SignalInfo};
use sdrkit_proto_ccsds::channel::{BasisPolicy, ChannelCoder, ChannelConfig};
use sdrkit_util_bits::xorshift::Xorshift64;
use std::collections::BTreeMap;

/// The channel coding of the bytes after the marker of `p`: the randomiser before the Reed-Solomon decoding (B) or none, and
/// the Reed-Solomon code with its interleave and virtual fill; an uncoded frame otherwise.
pub fn channel(p: &Profile) -> ChannelConfig {
    let c = match p.rs {
        Some(r) => {
            let c = if r.e == 8 {
                ChannelConfig::rs_239(r.interleave)
            } else {
                ChannelConfig::rs_223(r.interleave)
            };
            c.with_virtual_fill(r.fill)
        }
        None => ChannelConfig::uncoded(p.frame - 4),
    }
    .with_basis(BasisPolicy::Fixed(sdrkit_algo_fec::rs::Basis::Dual));
    if p.rand == Rand::Before {
        c
    } else {
        c.without_randomiser()
    }
}

fn test_packets(i: Instrument) -> Option<Box<dyn TestSource>> {
    Some(match i {
        Instrument::Viirs => Box::new(ViirsSource::default()),
        Instrument::Modis => Box::new(ModisSource::default()),
        Instrument::Avhrr => Box::new(AvhrrSource::default()),
        Instrument::Metimage => Box::new(MetimageSource::default()),
        Instrument::Tirs => Box::new(TirsSource::default()),
        Instrument::Vegetation => Box::new(VegetationSource::default()),
        Instrument::Amsr2 => Box::new(Amsr2Source::default()),
        _ => return None,
    })
}

/// The CADUs of the test signal of a profile, one by one.
pub struct TestCadus {
    p: &'static Profile,
    source: Option<Box<dyn TestSource>>,
    zs: ZoneStream,
    seq: BTreeMap<u16, u16>,
    pk: Vec<TestPacket>,
    counter: u32,
    coder: Option<ChannelCoder>,
    rng: Xorshift64,
}

impl TestCadus {
    /// The CADUs of `p`.
    pub fn new(p: &'static Profile) -> Self {
        TestCadus {
            p,
            source: test_packets(p.instrument),
            zs: ZoneStream::default(),
            seq: BTreeMap::new(),
            pk: Vec::new(),
            counter: 0,
            coder: ChannelCoder::new(channel(p)).ok(),
            rng: Xorshift64::new(0x5EED),
        }
    }

    /// The frame after decoding: the header, the insert zone, the packet zone, and random bytes to the end.
    fn frame(&mut self) -> Vec<u8> {
        let p = self.p;
        let n = p.data_bytes();
        let mut f = vec![0u8; n];
        for b in f.iter_mut() {
            *b = self.rng.step() as u8;
        }
        let vc = p.vc.unwrap_or(0);
        let scid = u16::from(p.test_spacecraft);
        let (fh, zone) = if p.zone > 0 {
            if let Some(src) = &mut self.source {
                while self.zs.len() < p.zone {
                    self.pk.clear();
                    src.next(&mut self.pk);
                    for t in &self.pk {
                        let s = self.seq.entry(t.apid).or_insert(0);
                        self.zs.push(&space_packet(t.apid, t.flags, *s, &t.data, false));
                        *s = (*s + 1) & 0x3FFF;
                    }
                }
            } else {
                while self.zs.len() < p.zone {
                    self.zs.push(&space_packet(0x7FF, 3, 0, &[0; 100], false));
                }
            }
            let (fh, z) = self.zs.take(p.zone);
            (fh.field(), Some(z))
        } else {
            (0x7FF, None)
        };
        match p.header {
            Header::Aos(insert) => {
                f[0] = 0x40 | (scid >> 2) as u8;
                f[1] = ((scid & 3) as u8) << 6 | (vc & 0x3F);
                f[2..5].copy_from_slice(&self.counter.to_be_bytes()[1..]);
                f[5] = 0;
                f[6..6 + insert].fill(0);
                if let Some(z) = zone {
                    f[6 + insert] = (fh >> 8) as u8 & 7;
                    f[7 + insert] = fh as u8;
                    f[8 + insert..8 + insert + z.len()].copy_from_slice(&z);
                }
            }
            Header::Tm => {
                f[0] = (scid >> 4) as u8 & 0x3F;
                f[1] = ((scid & 0xF) as u8) << 4 | (vc & 7) << 1;
                f[2] = self.counter as u8;
                f[3] = self.counter as u8;
                f[4] = (fh >> 8) as u8 & 7;
                f[5] = fh as u8;
                if let Some(z) = zone {
                    f[6..6 + z.len()].copy_from_slice(&z);
                }
            }
            Header::None => {}
        }
        self.counter = (self.counter + 1) & 0xFF_FFFF;
        f
    }

    /// The next CADU as sent: marker, then the coded, randomised (and for some profiles inverted) bytes.
    pub fn next_cadu(&mut self) -> Vec<u8> {
        let mut f = self.frame();
        if self.p.rand == Rand::After {
            sdrkit_proto_ccsds::randomiser::tm_bytes(&mut f);
        }
        let mut out = Vec::with_capacity(self.p.frame);
        if let Some(c) = &mut self.coder {
            if c.encode(&f, &mut out).is_err() {
                out.clear();
            }
        }
        out.resize(self.p.frame, 0);
        out[..4].copy_from_slice(&[0x1A, 0xCF, 0xFC, 0x1D]);
        if self.p.invert {
            out[4..].iter_mut().for_each(|b| *b = !*b);
        }
        out
    }
}

/// A frame file of `n` test CADUs of `p`.
pub fn frame_file(p: &'static Profile, n: usize) -> Vec<u8> {
    let mut t = TestCadus::new(p);
    let mut out = Vec::with_capacity(n * p.frame);
    for _ in 0..n {
        out.extend(t.next_cadu());
    }
    out
}

enum Frames {
    Cadus(Box<TestCadus>, CcsdsCoder),
    Ocm { y: usize, pn: Vec<u8>, v: u8, rng: Xorshift64 },
    Kanopus(Box<KanopusSource>),
    Ols(OlsSource),
}

/// The channel bits of the test signal of a profile (I then Q of each symbol for QPSK).
pub struct EodbSource {
    p: &'static Profile,
    frames: Frames,
    rails: [u8; 2],
    bits: Vec<u8>,
}

impl EodbSource {
    /// The source of `p`.
    pub fn new(p: &'static Profile) -> Self {
        let frames = match p.instrument {
            Instrument::Ocm => Frames::Ocm {
                y: 0,
                pn: ocm_pn(),
                v: 0,
                rng: Xorshift64::new(77),
            },
            Instrument::Kanopus => Frames::Kanopus(Box::default()),
            Instrument::Ols => Frames::Ols(OlsSource::default()),
            _ => Frames::Cadus(
                Box::new(TestCadus::new(p)),
                CcsdsCoder::new(CodeOptions {
                    nrzm: p.diff == Diff::NrzM,
                    invert_g2: true,
                    conv: matches!(p.conv, Conv::Ccsds | Conv::Swapped),
                }),
            ),
        };
        EodbSource {
            p,
            frames,
            rails: [0; 2],
            bits: Vec::new(),
        }
    }
}

impl BitSource for EodbSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let start = out.len();
        match &mut self.frames {
            Frames::Cadus(t, coder) => {
                let c = t.next_cadu();
                coder.push(&c, out);
                if self.p.conv == Conv::Swapped {
                    for pair in out[start..].as_chunks_mut::<2>().0 {
                        pair.swap(0, 1);
                    }
                }
                if self.p.diff == Diff::PerRail {
                    for (i, b) in out[start..].iter_mut().enumerate() {
                        let r = &mut self.rails[i % 2];
                        *r ^= *b;
                        *b = *r;
                    }
                }
            }
            Frames::Ocm { y, pn, v, rng } => {
                let f = ocm_test_frame(*y, pn);
                *y += 1;
                for b in &f {
                    for k in (0..8).rev() {
                        let bit = (b >> k) & 1;
                        *v = (*v + 2 * bit + (rng.step() & 1) as u8) & 3;
                        let (i, q) = match *v {
                            0 => (1, 1),
                            1 => (0, 1),
                            2 => (0, 0),
                            _ => (1, 0),
                        };
                        debug_assert_eq!(quadrant(i == 1, q == 1), *v);
                        out.push(i);
                        out.push(q);
                    }
                }
            }
            Frames::Kanopus(k) => k.next_frame(out),
            Frames::Ols(o) => {
                self.bits.clear();
                for _ in 0..64 {
                    o.next_frame(&mut self.bits);
                }
                out.extend_from_slice(&self.bits);
            }
        }
    }
}

/// The test signal of a profile: the channel bits at unit power with root raised cosine pulses of the profile's roll-off,
/// turned by the ambiguity (90 degree steps, then mirrored from 4 on).
pub struct EodbGenerator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<EodbSource>,
    turn: Complex32,
    mirror: bool,
}

impl EodbGenerator {
    /// The signal of `p` at `rate` with `ambiguity` (0 to 7).
    ///
    /// # Errors
    ///
    /// [`GenError`] if the rate is below one sample a symbol or the pulse shaper refuses it.
    pub fn new(rate: f64, p: &'static Profile, ambiguity: u8) -> Result<Self, GenError> {
        if rate < p.symbol_rate {
            return Err(GenError::RateTooLow {
                need: p.symbol_rate,
                got: rate,
            });
        }
        let modulation = match p.instrument {
            Instrument::Ols => Modulation::Bpsk,
            _ => p.modulation,
        };
        let mp = ModParams::new(rate, p.symbol_rate, p.roll_off, modulation);
        let info = mp.signal_info();
        let k = ambiguity & 3;
        let turn = Complex32::from_polar(1.0, k as f32 * std::f32::consts::FRAC_PI_2);
        Ok(EodbGenerator {
            rate,
            info,
            m: Modulator::new(mp, EodbSource::new(p))?,
            turn,
            mirror: ambiguity >= 4,
        })
    }
}

impl sdrkit_core::Generator for EodbGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let n = self.m.generate(out);
        for z in out.iter_mut() {
            let v = if self.mirror { z.conj() } else { *z };
            *z = v * self.turn;
        }
        n
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
}

#[cfg(test)]
mod tests {
    use super::super::profiles::profile;
    use super::*;

    #[test]
    fn test_cadus_have_the_marker_and_the_frame_length() {
        for id in ["jpss_hrd", "metop_dump", "proba_s", "gcom_w1", "oceansat3_x", "landsat_x"] {
            let p = profile(Some(id));
            let f = frame_file(p, 3);
            assert_eq!(f.len(), 3 * p.frame, "{id}");
            assert_eq!(f[p.frame..p.frame + 4], [0x1A, 0xCF, 0xFC, 0x1D], "{id}");
        }
    }
}
