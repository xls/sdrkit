//! The MAC test-signal generator: a loop of frames is built once, then pre-emphasis, the 25 Hz energy dispersal and FM at 13.5
//! MHz/V make the clean signal at the output rate.
//!
//! Standard: ETS 300 250.

use super::codes::{dispersal, PACKETS, PACKET_BITS};
use super::frame::*;
use super::packet::{
    encode_group, encode_params, group_packets, ids, sound_block, Command, Packet, Param, DUMMY,
    NEXT, SI_ADDRESS,
};
use crate::card::{Card, Pattern};
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_core::{Complex32, Generator, Hz};
use std::f64::consts::TAU;

/// The MAC generator's parameters.
#[derive(Clone, Debug)]
pub struct MacGenParams {
    /// Output rate.
    pub rate_hz: f64,
    /// D2-MAC or D-MAC.
    pub variant: Variant,
    /// EuroCrypt scrambling.
    pub scrambled: bool,
    /// 16:9.
    pub wide: bool,
    /// The picture.
    pub pattern: Pattern,
    /// Tone, Hz.
    pub tone_hz: f64,
    /// Channel name.
    pub channel: String,
    /// Network name.
    pub network: String,
    /// Country.
    pub country: String,
    /// Channel id.
    pub chid: u16,
    /// Sound address.
    pub sound_address: u16,
    /// ECM address.
    pub ecm_address: u16,
    /// Frames in the loop.
    pub loop_frames: usize,
    /// Amplitude.
    pub amplitude: f64,
}

impl Default for MacGenParams {
    fn default() -> Self {
        MacGenParams {
            rate_hz: 20e6,
            variant: Variant::D2,
            scrambled: false,
            wide: false,
            pattern: Pattern::Card,
            tone_hz: 1000.0,
            channel: "TEST TV".into(),
            network: "TEST SATELLITE".into(),
            country: "TEST COUNTRY".into(),
            chid: 0x1A2B,
            sound_address: 224,
            ecm_address: 346,
            loop_frames: 2,
            amplitude: 0.3,
        }
    }
}

/// The SI data groups the generator sends (the network and the TV service).
pub fn si_groups(p: &MacGenParams) -> Vec<Vec<u8>> {
    let mut nwo = vec![0x12u8, 0, 0];
    nwo.extend_from_slice(p.country.as_bytes());
    let net = encode_group(
        2,
        &[Command {
            ci: ids::CI_NETWORK,
            params: vec![
                Param {
                    pi: ids::NWO,
                    data: nwo,
                },
                Param {
                    pi: ids::NWNAME,
                    data: p.network.as_bytes().to_vec(),
                },
                Param {
                    pi: ids::LISTX,
                    data: vec![
                        1,
                        (p.sound_address & 0xFF) as u8,
                        (p.sound_address >> 8) as u8,
                    ],
                },
            ],
        }],
    );
    let mut sref = vec![0u8];
    sref.extend_from_slice(p.channel.as_bytes());
    let vconf = if p.wide { 6u8 << 2 } else { 4u8 << 2 };
    let mut tv = vec![
        Param {
            pi: ids::SREF,
            data: sref,
        },
        Param {
            pi: ids::VCONF,
            data: vec![vconf],
        },
    ];
    let sound = Param {
        pi: ids::TV_SOUND,
        data: vec![(p.sound_address & 0xFF) as u8, (p.sound_address >> 8) as u8],
    };
    if p.scrambled {
        let accm = Param {
            pi: ids::ACCM,
            data: vec![
                (p.ecm_address & 0xFF) as u8,
                ((p.ecm_address >> 8) as u8) | 0x80,
                ids::EUROCRYPT,
            ],
        };
        tv.push(Param {
            pi: ids::PG,
            data: encode_params(&[accm, sound]),
        });
    } else {
        tv.push(sound);
    }
    let tvg = encode_group(
        3,
        &[Command {
            ci: ids::CI_TV,
            params: tv,
        }],
    );
    vec![net, tvg]
}

/// Builds the baseband (volts) of a loop of frames at 20.25 MHz.
pub fn build_loop(p: &MacGenParams) -> Vec<f32> {
    let w = if p.wide { 1024 } else { 768 };
    let mut card = Card::new(w, 576, p.pattern, "TEST CARD", "MAC", "625/50");
    let frames = p.loop_frames.max(1);
    let mut out = vec![0.5f32; frames * LINES as usize * SAMPLES];
    let groups = si_groups(p);
    let si_packets: Vec<[u8; 91]> = groups.iter().flat_map(|g| group_packets(g)).collect();
    let mut ci = std::collections::HashMap::<u16, u8>::new();
    let mut next_ci = |a: u16| {
        let c = ci.entry(a).or_insert(0);
        let v = *c;
        *c = (*c + 1) & 3;
        v
    };
    let mut pre = crate::nicam::j17_pre();
    let amp = 10f64.powf(-10.0 / 20.0);
    let mut t32 = 0u64;
    let mut cut_rng = sdrkit_algo_sim::rng::Rng::new(0x1234_5678);
    let mut sound_prbs = dispersal();
    for f in 0..frames {
        card.update(f as f64 / 25.0, f as u64);
        // The packet multiplex of subframe 1: sound (20 a frame), SI, ECM, dummies.
        let mut packets: Vec<Packet> = Vec::with_capacity(PACKETS);
        let mut si_i = 0;
        let mut sound_left = 20;
        for slot in 0..PACKETS {
            let pk = if slot % 4 == 1 && sound_left > 0 {
                sound_left -= 1;
                let mut x = [0i32; 64];
                for v in x.iter_mut() {
                    let s = amp * (TAU * p.tone_hz * t32 as f64 / 32_000.0).sin();
                    *v = (pre.process_f64(s) * 8192.0).round().clamp(-8192.0, 8191.0) as i32;
                    t32 += 1;
                }
                let mut d = [0u8; 91];
                d[0] = NEXT;
                let mut blk = sound_block(&x);
                if p.scrambled {
                    for b in blk.iter_mut() {
                        let mut m = 0u8;
                        for i in 0..8 {
                            m |= sound_prbs.bit() << i;
                        }
                        *b ^= m;
                    }
                }
                d[1..].copy_from_slice(&blk);
                Packet::new(p.sound_address, next_ci(p.sound_address), d)
            } else if slot % 4 == 3 && si_i < si_packets.len() {
                let d = si_packets[si_i];
                si_i += 1;
                Packet::new(SI_ADDRESS, next_ci(SI_ADDRESS), d)
            } else if p.scrambled && slot == 42 {
                let mut d = [0u8; 91];
                d[0] = NEXT;
                Packet::new(p.ecm_address, next_ci(p.ecm_address), d)
            } else {
                Packet::new(DUMMY, 0, [0x55; 91])
            };
            packets.push(pk);
        }
        let mut sub = vec![0u8; 623 * 99];
        for (k, pk) in packets.iter().enumerate() {
            let b = pk.bits();
            sub[k * PACKET_BITS..(k + 1) * PACKET_BITS].copy_from_slice(&b);
        }
        let mut sub2 = vec![0u8; 623 * 99];
        if p.variant == Variant::D {
            for k in 0..PACKETS {
                let b = Packet::new(DUMMY, 0, [0xAA; 91]).bits();
                sub2[k * PACKET_BITS..(k + 1) * PACKET_BITS].copy_from_slice(&b);
            }
        }
        let mut prbs = dispersal();
        let periods = p.variant.periods();
        let odd_frame = f % 2 == 1;
        let sdf = Sdf {
            chid: p.chid,
            sdfscr: 0,
            mvscg: Sdf::mvscg_of(!p.wide, p.scrambled, true, p.scrambled),
            cafcnt: f as u32,
        };
        let rdf = Rdf {
            tdmcid: 1,
            fcnt: (f % 256) as u8,
        };
        let special = special_data(&sdf, &rdf);
        for l in 1..=LINES {
            let base = (f * LINES as usize + (l - 1) as usize) * SAMPLES;
            let line = &mut out[base..base + SAMPLES];
            // Data bits of the line.
            let mut bits: Vec<u8> = Vec::with_capacity(648);
            if l == 625 {
                for i in 0..FSS_BITS {
                    bits.push(fss_bit(i, odd_frame));
                }
                bits.extend_from_slice(&special);
            } else {
                bits.extend_from_slice(&sync_word(l));
                let nb = p.variant.burst_bits();
                let mut d: Vec<u8> = Vec::with_capacity(nb);
                if l <= 623 {
                    let a = (l as usize - 1) * 99;
                    d.extend_from_slice(&sub[a..a + 99]);
                    if p.variant == Variant::D {
                        d.extend_from_slice(&sub2[a..a + 99]);
                    }
                }
                d.resize(nb - 6, 0);
                for b in d.iter() {
                    let e = if l <= 623 { prbs.bit() } else { 0 };
                    bits.push(b ^ e);
                }
                // The sequence steps once a bit period.
                if l <= 623 {
                    for _ in d.len()..periods - 6 {
                        prbs.bit();
                    }
                }
            }
            // Duobinary.
            let mut pol = 1.0f32;
            let rate2 = l == 625 || p.variant == Variant::D2;
            for (i, &b) in bits.iter().enumerate() {
                let v = if b == 1 { 0.5 + 0.4 * pol } else { 0.5 };
                if b == 0 {
                    pol = -pol;
                }
                if rate2 {
                    let g = 3 + 2 * i;
                    if g + 1 < SAMPLES {
                        line[g] = v;
                        line[g + 1] = v;
                    }
                } else if i < SAMPLES {
                    line[i] = v;
                }
            }
            if l == 624 {
                for g in REF_GREY.0..REF_GREY.1 {
                    line[g] = 0.5;
                }
                for g in REF_WHITE.0..REF_WHITE.1 {
                    line[g] = 1.0;
                }
                for g in REF_BLACK.0..REF_BLACK.1 {
                    line[g] = 0.0;
                }
            }
            // Vision.
            let row_of = |l: u32| -> Option<usize> {
                if (23..=310).contains(&l) {
                    Some(2 * (l - 23) as usize)
                } else if (335..=622).contains(&l) {
                    Some(2 * (l - 335) as usize + 1)
                } else {
                    None
                }
            };
            if let Some(row) = row_of(l) {
                let pl = &card.planes;
                let black_luma = l == 23 || l == 335;
                let nl = LUMA.1 - LUMA.0;
                for k in 0..nl {
                    let x = ((k as f64 + 0.5) / nl as f64 * w as f64).floor() as usize;
                    line[LUMA.0 + k] = if black_luma {
                        0.0
                    } else {
                        pl.y[row * w + x.min(w - 1)]
                    };
                }
                // The colour difference for the next line's luminance.
                if let Some(nrow) = row_of(l + 1) {
                    let nc = COLOUR.1 - COLOUR.0;
                    for k in 0..nc {
                        let x = ((k as f64 + 0.5) / nc as f64 * w as f64).floor() as usize;
                        let i = nrow * w + x.min(w - 1);
                        let c = if l % 2 == 1 {
                            MAC_KU * pl.u[i] as f64 / sdrkit_codec_image::colour::KU as f64
                        } else {
                            MAC_KV * pl.v[i] as f64 / sdrkit_codec_image::colour::KV as f64
                        };
                        line[COLOUR.0 + k] = (0.5 + c) as f32;
                    }
                }
                if p.scrambled {
                    // Single-cut line rotation.
                    let span = &mut line[COLOUR.0..LUMA.1];
                    let cut = (cut_rng.next_u64() % span.len() as u64) as usize;
                    span.rotate_left(cut);
                }
            }
        }
    }
    // A light transmit filter (the vision band, and the duobinary shaping).
    let taps = sdrkit_algo_dsp::fir::lowpass(CLOCK, 7.0e6, 10.0e6, 40.0)
        .expect("the filter settings are valid");
    let h = taps.len() / 2;
    let n = out.len();
    let mut y = vec![0f32; n];
    for i in 0..n {
        let mut s = 0.0;
        for (j, &t) in taps.iter().enumerate() {
            let k = (i + n + j - h) % n;
            s += t * out[k];
        }
        y[i] = s;
    }
    y
}

/// The MAC generator.
pub struct MacGenerator {
    p: MacGenParams,
    base: Vec<f32>,
    pos: usize,
    up_prev: f32,
    pre: MacEmphasis,
    phase: f64,
    t: u64,
    res: FractionalResampler<Complex32>,
    buf: Vec<Complex32>,
    pending: Vec<Complex32>,
    rd: usize,
}

impl MacGenerator {
    /// A generator.
    pub fn new(p: MacGenParams) -> Self {
        let base = build_loop(&p);
        let grid = 2.0 * CLOCK;
        let pass = (0.48 * p.rate_hz).min(18e6);
        let stop = (p.rate_hz - pass).max(pass + 0.5e6).min(grid / 2.0);
        let res = FractionalResampler::new(grid, p.rate_hz, pass, stop, 50.0)
            .unwrap_or_else(|_| FractionalResampler::with_rates(grid, p.rate_hz).expect("rates"));
        MacGenerator {
            pre: MacEmphasis::new(grid, false),
            base,
            pos: 0,
            up_prev: 0.5,
            phase: 0.0,
            t: 0,
            res,
            buf: Vec::with_capacity(8192),
            pending: Vec::new(),
            rd: 0,
            p,
        }
    }

    fn block(&mut self) {
        self.buf.clear();
        let grid = 2.0 * CLOCK;
        let n = self.base.len();
        for _ in 0..2048 {
            let v = self.base[self.pos];
            self.pos = (self.pos + 1) % n;
            for half in 0..2 {
                let x = if half == 0 {
                    0.5 * (self.up_prev + v)
                } else {
                    v
                } as f64;
                let y = self.pre.process(x - 0.5);
                // 25 Hz triangle of 600 kHz peak to peak.
                let tri = {
                    let ph = (self.t as f64 / grid * 25.0).fract();
                    if ph < 0.5 {
                        4.0 * ph - 1.0
                    } else {
                        3.0 - 4.0 * ph
                    }
                };
                let f = HZ_PER_V * y + 300e3 * tri;
                self.phase = (self.phase + f / grid).rem_euclid(1.0);
                let (s, c) = sdrkit_algo_dsp::approx::sin_cos(TAU * self.phase);
                let z = Complex32::new(c, s) * self.p.amplitude as f32;
                self.buf.push(z);
                self.t += 1;
            }
            self.up_prev = v;
        }
        if self.rd > 0 {
            self.pending.drain(..self.rd);
            self.rd = 0;
        }
        self.res.process(&self.buf, &mut self.pending);
    }
}

impl Generator for MacGenerator {
    fn rate_hz(&self) -> Hz {
        self.p.rate_hz
    }
    /// The FM carrier has a constant envelope: its power is the stated power, in the 27 MHz satellite channel.
    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        Some(sdrkit_core::SignalInfo::new(27e6).with_power(self.p.amplitude * self.p.amplitude))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            while self.rd >= self.pending.len() {
                self.block();
            }
            *o = self.pending[self.rd];
            self.rd += 1;
        }
        out.len()
    }
}
