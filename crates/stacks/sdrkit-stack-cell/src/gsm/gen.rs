//! The GSM / GSM-R test signal: base station carriers with the 51-multiframe on timeslot 0 and dummy bursts
//! on the other timeslots, placed at each ARFCN relative to the radio centre. The signal is clean, at unit mean power.

use super::codes::{
    dummy_burst, encode_bcch, encode_sch, fcch_burst, interleave, normal_burst, sch_burst, Sch,
};
use super::gmsk::SYMBOL_RATE;
use super::si::{
    build_empty, build_si1, build_si2, build_si3, build_si4, fill_frame, Lai, Si3Fields,
};
use crate::bands::arfcn_to_hz;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_sim::modulate::{Cpm, Ramp};
use sdrkit_core::{Complex32, Generator, Hz, SignalInfo};

/// The radio centre of the test scenes, Hz. The host gives the frequency of a real channel.
pub const CENTRE_HZ: f64 = 922.8e6;
/// The bandwidth of one carrier's channel, Hz (45.005 2).
pub const CHANNEL_HZ: f64 = 200e3;

/// One base station carrier.
#[derive(Clone, Debug)]
pub struct GsmCarrier {
    /// ARFCN.
    pub arfcn: u32,
    /// NCC.
    pub ncc: u8,
    /// BCC.
    pub bcc: u8,
    /// LAC.
    pub lac: u16,
    /// Cell identity.
    pub cell_id: u16,
    /// Start offset in samples at 8 samples per symbol.
    pub start: usize,
    /// First frame number.
    pub first_fn: u32,
    /// Amplitude relative to the other carriers of the scene.
    pub amp: f32,
}

/// The carriers of scene 0, 1 or 2.
pub fn scene(n: u32) -> Vec<GsmCarrier> {
    let c = |arfcn, ncc, bcc, lac, cell_id, start, first_fn, amp| GsmCarrier {
        arfcn,
        ncc,
        bcc,
        lac,
        cell_id,
        start,
        first_fn,
        amp,
    };
    match n {
        1 => vec![c(955, 3, 6, 9001, 777, 1234, 4000, 0.28)],
        2 => vec![
            c(957, 0, 1, 4711, 20001, 500, 51, 0.30),
            c(964, 0, 4, 4712, 20002, 9100, 6000, 0.26),
            c(971, 0, 7, 4713, 20003, 4400, 20000, 0.08),
        ],
        _ => vec![
            c(960, 1, 2, 4711, 12001, 3100, 100, 0.30),
            c(968, 1, 5, 4711, 12002, 7777, 1900, 0.22),
        ],
    }
}

/// The SI values every test carrier uses.
pub fn common_fields(cell_id: u16) -> Si3Fields {
    Si3Fields {
        cell_id,
        attach: true,
        bs_ag_blks_res: 1,
        ccch_conf: 0,
        bs_pa_mfrms: 3,
        t3212_decihours: 10,
        radio_link_timeout_index: 11,
        dtx: 2,
        pwrc: false,
        cell_reselect_hysteresis_db: 4,
        ms_txpwr_max_cch: 5,
        rxlev_access_min: 12,
        neci: true,
        acs: false,
        max_retrans: 3,
        tx_integer: 9,
        cell_bar_access: false,
        access_classes_barred: 0,
    }
}

const HYPER: u32 = 26 * 51 * 2048;
/// The mean power of a carrier of amplitude 1. Each timeslot of 156.25 symbols holds 147 symbols at full power and two ramp
/// symbols at 3/8 power (the mean of the squared raised-cosine ramp). The rest of the guard period is quiet.
const BURST_FILL: f64 = (147.0 + 2.0 * 0.375) / 156.25;
const TS_LEN: [usize; 8] = [157, 156, 156, 156, 157, 156, 156, 156];
/// Samples per symbol of the native signal.
const SPS: usize = 8;

/// The GMSK modulator of 45.004: BT 0.3, h 0.5, each burst from the start of its first bit to the end of its last, with a
/// raised-cosine ramp over one bit at each end.
fn gmsk() -> Cpm {
    // The parameters are constants in range, so the modulator always builds.
    Cpm::gmsk(0.3, SPS as f64)
        .and_then(|m| {
            m.with_held_edges()
                .with_ramp(Ramp::RaisedCosine(SPS as f64))
        })
        .unwrap_or_else(|_| unreachable!("GMSK BT 0.3 at 8 samples per symbol"))
}

struct CarrierState {
    cfg: GsmCarrier,
    cpm: Cpm,
    prev_bit: u8,
    levels: Vec<f32>,
    native: Vec<Complex32>,
    resampled: Vec<Complex32>,
    rs: FractionalResampler<Complex32>,
    osc: Nco,
    fnum: u32,
    lead: usize,
    block: [[u8; 114]; 4],
    frames: [[u8; 23]; 8],
}

impl CarrierState {
    /// Modulates one frame of (bit, amplitude) pairs into `native`: each run of bits at amplitude 1 is one burst, and a bit at
    /// amplitude 0 is a symbol of silence. The bits are differentially encoded first (45.004 clause 2.3), also through the
    /// silence.
    fn modulate(&mut self, bits: &[(u8, f32)]) {
        let mut i = 0;
        while i < bits.len() {
            if bits[i].1 == 0.0 {
                self.prev_bit = bits[i].0;
                self.native
                    .extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), SPS));
                i += 1;
                continue;
            }
            self.levels.clear();
            while i < bits.len() && bits[i].1 != 0.0 {
                let d = bits[i].0 ^ self.prev_bit;
                self.prev_bit = bits[i].0;
                self.levels.push(1.0 - 2.0 * d as f32);
                i += 1;
            }
            self.cpm.push(&self.levels, &mut self.native);
            self.cpm.flush(&mut self.native);
        }
    }

    fn frame_bits(&mut self) -> Vec<(u8, f32)> {
        let f = self.fnum;
        let t = f % 51;
        let bsic = self.cfg.ncc << 3 | self.cfg.bcc;
        let tsc = self.cfg.bcc as usize;
        let ts0: [u8; 148] = if t.is_multiple_of(10) && t < 50 {
            fcch_burst()
        } else if t % 10 == 1 && t < 50 {
            sch_burst(&encode_sch(&Sch::from_fn(bsic, f)))
        } else if t == 50 {
            dummy_burst()
        } else {
            let start = if (2..=5).contains(&t) {
                2
            } else {
                t - (t % 10) + if t % 10 >= 6 { 6 } else { 2 }
            };
            if t == start {
                let msg = if (2..=5).contains(&t) {
                    let tc = (f / 51) % 8;
                    self.frames[tc as usize]
                } else {
                    fill_frame()
                };
                self.block = interleave(&encode_bcch(&msg));
            }
            normal_burst(&self.block[(t - start) as usize], tsc, 1)
        };
        let mut v = Vec::with_capacity(1250);
        for (tn, &len) in TS_LEN.iter().enumerate() {
            let b = if tn == 0 { ts0 } else { dummy_burst() };
            for &bit in &b {
                v.push((bit, 1.0));
            }
            for _ in 148..len {
                v.push((0, 0.0));
            }
        }
        self.fnum = (self.fnum + 1) % HYPER;
        v
    }
}

/// The GSM generator.
pub struct GsmGenerator {
    rate: f64,
    carriers: Vec<CarrierState>,
    buf: Vec<Complex32>,
    pos: usize,
}

impl GsmGenerator {
    /// The `carriers` at output rate `rate`, placed relative to a radio at `centre_hz`. A carrier outside the sample band is left
    /// out, as the radio's filter takes it out. The amplitudes are scaled so the sum of the carriers has unit mean power.
    pub fn new(rate: f64, centre_hz: f64, carriers: Vec<GsmCarrier>) -> Self {
        let native = 8.0 * SYMBOL_RATE;
        let place = |c: &GsmCarrier| arfcn_to_hz(c.arfcn).unwrap_or(centre_hz) - centre_hz;
        let mut carriers: Vec<GsmCarrier> = carriers
            .into_iter()
            .filter(|c| place(c).abs() < rate / 2.0 - CHANNEL_HZ / 2.0)
            .collect();
        let power: f64 = carriers.iter().map(|c| f64::from(c.amp).powi(2)).sum();
        if power > 0.0 {
            let scale = (BURST_FILL * power).sqrt().recip() as f32;
            carriers.iter_mut().for_each(|c| c.amp *= scale);
        }
        let carriers = carriers
            .into_iter()
            .map(|cfg| {
                let f = place(&cfg);
                let lai = Lai {
                    mcc: 1,
                    mnc: 1,
                    mnc_digits: 2,
                    lac: cfg.lac,
                };
                let fl = common_fields(cfg.cell_id);
                let a = cfg.arfcn as u16;
                let alloc = [a - 2, a, a + 2];
                let neigh: Vec<u16> = [955u16, 958, 961, 967, 970, 973]
                    .into_iter()
                    .filter(|&x| x != a)
                    .collect();
                let si3 = build_si3(&lai, &fl);
                let si4 = build_si4(&lai, &fl);
                let frames = [
                    build_si1(&alloc, &fl),
                    build_si2(&neigh, 0xFF, &fl),
                    si3,
                    si4,
                    build_empty(0x03),
                    fill_frame(),
                    si3,
                    si4,
                ];
                CarrierState {
                    cpm: gmsk(),
                    prev_bit: 0,
                    levels: Vec::with_capacity(160),
                    native: Vec::new(),
                    resampled: Vec::new(),
                    rs: FractionalResampler::with_rates(native, rate).expect("rates"),
                    // The oscillator mixes by -f; a carrier above the centre needs +f.
                    osc: Nco::new(rate, f),
                    fnum: cfg.first_fn % HYPER,
                    lead: cfg.start,
                    block: [[0; 114]; 4],
                    frames,
                    cfg,
                }
            })
            .collect();
        GsmGenerator {
            rate,
            carriers,
            buf: Vec::new(),
            pos: 0,
        }
    }

    fn refill(&mut self) {
        // One TDMA frame of every carrier, summed at the output rate.
        let mut sum: Vec<Complex32> = Vec::new();
        for c in self.carriers.iter_mut() {
            c.native.clear();
            if c.lead > 0 {
                let n = c.lead.min(10_000);
                c.native
                    .extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), n));
                c.lead -= n;
            } else {
                let bits = c.frame_bits();
                c.modulate(&bits);
            }
            for v in c.native.iter_mut() {
                *v *= c.cfg.amp;
            }
            c.rs.process(&c.native, &mut c.resampled);
        }
        let n = self
            .carriers
            .iter()
            .map(|c| c.resampled.len())
            .min()
            .unwrap_or(0);
        sum.resize(n, Complex32::new(0.0, 0.0));
        for c in self.carriers.iter_mut() {
            let mut part: Vec<Complex32> = c.resampled.drain(..n).collect();
            c.osc.mix_in_place(&mut part);
            for (s, p) in sum.iter_mut().zip(&part) {
                *s += p;
            }
        }
        self.buf.drain(..self.pos);
        self.pos = 0;
        self.buf.extend(sum);
    }
}

impl Generator for GsmGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    /// One 200 kHz channel per carrier, GMSK at 270.833 kbaud. An SNR is measured in the channels of all carriers.
    fn signal_info(&self) -> Option<SignalInfo> {
        let channels = self.carriers.len().max(1) as f64;
        Some(SignalInfo::new(channels * CHANNEL_HZ).with_symbol_rate(SYMBOL_RATE))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pos >= self.buf.len() {
                self.refill();
                if self.buf.is_empty() {
                    break;
                }
            }
            let n = (self.buf.len() - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        i
    }
}
