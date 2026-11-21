//! The HaLow test signal: a schedule of S1G PPDUs at 1 to 16 MHz, each built on its own grid and resampled to the radio rate,
//! separated by silence. The framework adds noise and the receiver's impairments from the standard `gen_*` settings.
//! Standard: IEEE Std 802.11-2020 clause 23.

use super::mac::{data_frame, probe_response, s1g_beacon, HalowBss};
use super::phy::{build_ppdu, grid, PpduSpec, Preamble, Sig};
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, BurstSource, Scene};
use sdrkit_core::{Complex32, Generator, Hz, SignalInfo};
use sdrkit_proto_wlan_phy::mac::push_fcs;

/// One PPDU of the schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct GenPpdu {
    /// What to build.
    pub spec: PpduSpec,
    /// Silence after it, s.
    pub gap_s: f64,
    /// Amplitude scale.
    pub scale: f64,
    /// Its own carrier offset, Hz.
    pub offset_hz: f64,
}

impl GenPpdu {
    /// A PPDU carrying `psdu` (FCS included) at a width and MCS, default preamble, 3 ms gap.
    pub fn new(psdu: Vec<u8>, width: u32, mcs: u8) -> GenPpdu {
        GenPpdu {
            spec: PpduSpec {
                width,
                preamble: if width == 1 {
                    Preamble::OneM
                } else {
                    Preamble::Short
                },
                sig: Sig {
                    nsts: 1,
                    mcs,
                    length: psdu.len() as u16,
                    ..Default::default()
                },
                psdu,
                seed: 0x5D,
                tilt: 0.0,
            },
            gap_s: 3e-3,
            scale: 1.0,
            offset_hz: 0.0,
        }
    }
    /// With its own offset.
    pub fn offset(mut self, hz: f64) -> Self {
        self.offset_hz = hz;
        self
    }
    /// With a scrambler seed.
    pub fn seed(mut self, s: u8) -> Self {
        self.spec.seed = s;
        self
    }
    /// With a gap.
    pub fn gap(mut self, s: f64) -> Self {
        self.gap_s = s;
        self
    }
    /// With a preamble.
    pub fn preamble(mut self, p: Preamble) -> Self {
        self.spec.preamble = p;
        self
    }
    /// The test flag "bad FCS": one PSDU bit flipped after the FCS was computed.
    pub fn bad_fcs(mut self) -> Self {
        if let Some(b) = self.spec.psdu.get_mut(1) {
            *b ^= 0x04;
        }
        self
    }
}

/// What the generator sends.
#[derive(Clone, Debug, PartialEq)]
pub struct HalowGenParams {
    /// Output rate, Hz.
    pub rate_hz: Hz,
    /// Silence before the first PPDU, s.
    pub lead_s: f64,
    /// Repeat the schedule.
    pub looped: bool,
    /// The seed of the filler data and of the start of each PPDU within a grid sample.
    pub seed: u64,
    /// The schedule.
    pub ppdus: Vec<GenPpdu>,
}

impl Default for HalowGenParams {
    fn default() -> Self {
        HalowGenParams {
            rate_hz: 4e6,
            lead_s: 0.5e-3,
            looped: true,
            seed: 1,
            ppdus: test_schedule(4e6),
        }
    }
}

/// The networks of the test schedule.
pub fn schedule_bss() -> Vec<HalowBss> {
    let mut home = HalowBss::new(
        [0x24, 0x0A, 0xC4, 0x12, 0x34, 0x56],
        "HaLow-Home",
        "WPA3-SAE",
        1,
        6,
        3,
    );
    home.extras = true;
    let mut farm = HalowBss::new(
        [0x00, 0x15, 0x6D, 0xAA, 0xBB, 0x01],
        "Farm-Sensors",
        "WPA2-PSK",
        2,
        7,
        6,
    );
    farm.interval = 200;
    let cam = HalowBss::new(
        [0x4C, 0x5E, 0x0C, 0x00, 0x11, 0x22],
        "OpenCam",
        "Open",
        4,
        2,
        4,
    );
    let mut meter = HalowBss::new(
        [0x02, 0x11, 0x22, 0x33, 0x44, 0x55],
        "Meter-Net",
        "WPA3-SAE",
        1,
        6,
        7,
    );
    meter.compressed_only = true;
    let lab = HalowBss::new(
        [0x8C, 0x1F, 0x64, 0x00, 0x08, 0x08],
        "Lab-8MHz",
        "WPA3-SAE",
        8,
        4,
        8,
    );
    let wide = HalowBss::new(
        [0x8C, 0x1F, 0x64, 0x00, 0x16, 0x16],
        "Wide-16MHz",
        "Open",
        16,
        5,
        16,
    );
    vec![home, farm, cam, meter, lab, wide]
}

/// The test schedule for a radio rate (the 4 MHz PPDU from 4 Msps, 8 MHz from 10 Msps, 16 MHz from 20 Msps).
pub fn test_schedule(rate: f64) -> Vec<GenPpdu> {
    let b = schedule_bss();
    let mut v = vec![
        GenPpdu::new(s1g_beacon(&b[0]), 1, 0).offset(6e3),
        GenPpdu::new(s1g_beacon(&b[1]), 2, 0).offset(-9e3),
    ];
    if rate >= 4e6 {
        v.push(GenPpdu::new(s1g_beacon(&b[2]), 4, 0).offset(14e3));
    }
    v.push(
        GenPpdu::new(s1g_beacon(&b[3]), 1, 10)
            .seed(0x2B)
            .offset(-12e3),
    );
    v.push(
        GenPpdu::new(
            probe_response(&b[3], [0xB8, 0x27, 0xEB, 0x01, 0x02, 0x03]),
            2,
            0,
        )
        .seed(0x11)
        .offset(4e3),
    );
    v.push(
        GenPpdu::new(
            data_frame(b[0].bssid, [0x02, 0x24, 0x0A, 0xC4, 0x00, 0x01], 60),
            2,
            0,
        )
        .offset(-3e3),
    );
    let mut mimo = vec![0x5Au8; 86];
    push_fcs(&mut mimo);
    let mut m = GenPpdu::new(mimo, 2, 3).offset(8e3);
    m.spec.sig.nsts = 2;
    v.push(m);
    if rate >= 10e6 {
        v.push(GenPpdu::new(s1g_beacon(&b[4]), 8, 2).offset(-7e3));
    }
    if rate >= 20e6 {
        v.push(GenPpdu::new(s1g_beacon(&b[5]), 16, 1).offset(9e3));
    }
    v
}

/// The clean samples of one PPDU at `rate_hz`, at unit mean power, starting `lead` grid samples late.
pub fn ppdu_samples(g: &GenPpdu, rate_hz: f64, lead: usize, rng: &mut Rng) -> Vec<Complex32> {
    let x = build_ppdu(&g.spec, rng);
    let (grate, _) = grid(g.spec.width);
    let mut padded = vec![Complex32::new(0.0, 0.0); lead];
    padded.extend_from_slice(&x);
    padded.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 256));
    let mut out = Vec::new();
    if (rate_hz - grate).abs() < 1.0 {
        out = padded;
    } else if let Ok(mut rs) = FractionalResampler::with_rates(grate, rate_hz) {
        rs.process(&padded, &mut out);
    }
    let p = x.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / x.len().max(1) as f64;
    if p > 0.0 {
        let k = (1.0 / p).sqrt() as f32;
        out.iter_mut().for_each(|s| *s *= k);
    }
    out
}

/// Makes the bursts of the schedule when they are due.
struct Schedule {
    p: HalowGenParams,
    rng: Rng,
    index: usize,
    at_s: f64,
    done: bool,
}

impl BurstSource for Schedule {
    fn next_burst(&mut self) -> Option<Burst> {
        if self.done || self.p.ppdus.is_empty() {
            return None;
        }
        if self.index >= self.p.ppdus.len() {
            if !self.p.looped {
                self.done = true;
                return None;
            }
            self.index = 0;
        }
        let g = &self.p.ppdus[self.index];
        self.index += 1;
        // A random start within a grid sample: the PPDU is not locked to the receiver's clock.
        let lead = self.rng.below(4) as usize;
        let samples = ppdu_samples(g, self.p.rate_hz, lead, &mut self.rng);
        let start_s = self.at_s;
        self.at_s += samples.len() as f64 / self.p.rate_hz + g.gap_s;
        Some(Burst {
            start_s,
            offset_hz: g.offset_hz,
            level_db: 20.0 * g.scale.max(1e-9).log10(),
            samples,
        })
    }
}

/// The HaLow test-signal generator.
pub struct HalowGenerator {
    rate_hz: f64,
    scene: Option<Scene>,
}

impl HalowGenerator {
    /// A generator of `p`.
    pub fn new(p: HalowGenParams) -> HalowGenerator {
        let rate_hz = p.rate_hz;
        let scene = Scene::new(rate_hz).ok().map(|mut s| {
            s.add_source(Box::new(Schedule {
                rng: Rng::new(p.seed),
                at_s: p.lead_s,
                p,
                index: 0,
                done: false,
            }));
            s
        });
        HalowGenerator { rate_hz, scene }
    }
}

impl Generator for HalowGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate_hz
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        match &mut self.scene {
            Some(s) => s.generate(out),
            None => 0,
        }
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The noise of the test signal is stated in 1 MHz, the width of the narrowest channel.
        Some(SignalInfo::new(1e6))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// How many PPDUs of a schedule start within `seconds` (lead silence included).
pub fn sent_in(p: &HalowGenParams, seconds: f64) -> usize {
    let mut t = p.lead_s;
    let mut n = 0;
    'outer: loop {
        for g in &p.ppdus {
            if t >= seconds {
                break 'outer;
            }
            let mut sig = g.spec.sig;
            sig.bw = g.spec.width;
            let lay = super::phy::Layout::of(g.spec.preamble, &sig, g.spec.width);
            let dur =
                (lay.total() + 4) as f64 / grid(g.spec.width).0 + 256.0 / grid(g.spec.width).0;
            if t + dur > seconds {
                break 'outer;
            }
            n += 1;
            t += dur + g.gap_s;
        }
        if !p.looped {
            break;
        }
    }
    n
}
