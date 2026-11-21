//! The `dect_nrplus` test signal: packets of an STF and 9 OFDM symbols at unit power, the DRS on symbols 1 and 6, the PCC on
//! symbols 1 and 2, random QPSK elsewhere, one packet every 10 ms in turn from three devices (a type 1 field, a type 2 field
//! with the closed-loop MIMO mask, a type 1 field with the beamforming mask), at the centre of the samples.

use super::phy::{
    bin, carrier, encode, pcc_cells, qpsk, scrambling, stf_base, ControlField, COVER, CP, DRS,
    NFFT, OCCUPIED, PCC_CINIT, RATE_HZ,
};
use sdrkit_algo_dsp::resample::resample_span;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, SignalInfo};

/// OFDM symbols after the STF in a test packet.
pub const SYMBOLS: usize = 9;
/// Seconds between two packets.
pub const PACKET_EVERY_S: f64 = 0.010;

/// The three fields of the test signal and their masks.
pub fn test_fields() -> [(ControlField, u16); 3] {
    let t1 = ControlField {
        kind: 1,
        format: 0,
        slots: false,
        length: 4,
        network: 0x5A,
        transmitter: 0x1234,
        power: 5,
        mcs: 2,
        receiver: None,
        harq: None,
    };
    let t2 = ControlField {
        kind: 2,
        transmitter: 0x5678,
        receiver: Some(0x1234),
        harq: Some((0, 0, true, 1, 0, 0)),
        ..t1
    };
    let t3 = ControlField {
        transmitter: 0x9ABC,
        mcs: 1,
        ..t1
    };
    [(t1, 0x0000), (t2, 0x5555), (t3, 0xAAAA)]
}

/// One packet at 1.728 Msps with control field `f` and mask `mask`.
pub fn packet(f: &ControlField, mask: u16, rng: &mut Rng) -> Vec<Complex32> {
    let s = stf_base();
    let mut x: Vec<Complex32> = COVER
        .iter()
        .flat_map(|&c| s.iter().map(move |z| z * c))
        .collect();
    let mut bits = encode(&f.bits(), mask);
    for (b, c) in bits.iter_mut().zip(scrambling(PCC_CINIT)) {
        *b ^= c;
    }
    let mut cells = vec![[None::<Complex32>; OCCUPIED]; SYMBOLS + 1];
    for l in (1..=SYMBOLS).step_by(5) {
        let n = (l - 1) / 5;
        for i in 0..14 {
            cells[l][4 * i + 2 * (n % 2)] = Some(Complex32::new(f32::from(DRS[4 * i]), 0.0));
        }
    }
    for (q, &(l, k)) in pcc_cells().iter().enumerate() {
        cells[l][k] = Some(qpsk(bits[2 * q], bits[2 * q + 1]));
    }
    let scale = 1.0 / (OCCUPIED as f32).sqrt();
    for row in cells.iter_mut().skip(1) {
        let mut spec = [Complex32::default(); NFFT];
        for (k, c) in row.iter_mut().enumerate() {
            let v = c.unwrap_or_else(|| qpsk(rng.bit(), rng.bit()));
            spec[bin(carrier(k))] = v;
        }
        let body: Vec<Complex32> = (0..NFFT)
            .map(|n| {
                spec.iter()
                    .enumerate()
                    .filter(|(_, v)| v.norm_sqr() > 0.0)
                    .map(|(b, v)| {
                        let p = std::f32::consts::TAU * (b * n) as f32 / NFFT as f32;
                        v * Complex32::from_polar(1.0, p)
                    })
                    .sum::<Complex32>()
                    * scale
            })
            .collect();
        x.extend_from_slice(&body[NFFT - CP..]);
        x.extend_from_slice(&body);
    }
    x
}

/// The test signal at `rate`.
///
/// # Errors
///
/// [`ConfigError`] if the scene refuses the rate.
pub fn scene(rate: f64, seed: u64) -> Result<Scene, ConfigError> {
    let mut scene = Scene::new(rate)?;
    let mut rng = Rng::stream(seed, 0xDEC2);
    let fields = test_fields();
    let mut k = 0usize;
    let mut t = 0.004;
    let mut work = Vec::new();
    scene.add_source(Box::new(move || {
        let (f, mask) = fields[k % fields.len()];
        let x = packet(&f, mask, &mut rng);
        let samples = if (rate - RATE_HZ).abs() < 1.0 {
            x
        } else {
            resample_span(&x, RATE_HZ, rate, &mut work);
            work.clone()
        };
        let b = Burst {
            start_s: t,
            offset_hz: 0.0,
            level_db: 0.0,
            samples,
        };
        k += 1;
        t += PACKET_EVERY_S;
        Some(b)
    }));
    Ok(scene)
}

/// The `dect_nrplus` generator.
pub struct NrPlusGenerator {
    scene: Scene,
    rate: f64,
}

impl NrPlusGenerator {
    /// The test signal of `cfg`.
    ///
    /// # Errors
    ///
    /// As [`scene`].
    pub fn from_config(cfg: &GenConfig) -> Result<NrPlusGenerator, ConfigError> {
        Ok(NrPlusGenerator {
            scene: scene(cfg.rate_hz, cfg.seed)?,
            rate: cfg.rate_hz,
        })
    }
}

impl Generator for NrPlusGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(1.512e6)
                .with_symbol_rate(RATE_HZ)
                .with_power(1.0),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}
