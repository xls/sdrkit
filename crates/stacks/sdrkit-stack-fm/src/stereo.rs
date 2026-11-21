//! The stereo decoder: MPX in, audio at the working rate (MPX rate / 5) out, and the pilot loop's sample for every MPX sample
//! (the carrier of the RDS demodulator). It holds the pilot loop, the sum and difference signals with their 15 kHz low-pass
//! and decimation by 5, the blend and the matrix.
//!
//! Standard: ITU-R BS.450 (pilot-tone system).

use crate::pilot::{Pilot, PilotSample};
use crate::settings::StereoChoice;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::Frame;

/// What the stereo decoder reports.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StereoReport {
    /// Reported stereo.
    pub stereo: bool,
    /// Blend in percent while stereo is reported, 0 otherwise.
    pub stereo_pct: f32,
    /// Pilot injection in percent of 75 kHz while locked, 0 otherwise.
    pub pilot_pct: f32,
    /// The PLL is locked.
    pub locked: bool,
}

/// The stereo decoder.
#[derive(Clone, Debug)]
pub struct StereoDecoder {
    fm: f64,
    pll: Pilot,
    fir: Fir<Frame<2>>,
    count: usize,
    blend: f32,
    wanted: Option<f32>,
    choice: StereoChoice,
}

impl StereoDecoder {
    /// A decoder for MPX at `fm`.
    pub fn new(fm: f64, choice: StereoChoice) -> Self {
        StereoDecoder {
            fm,
            pll: Pilot::new(fm),
            fir: Fir::new(
                &sdrkit_algo_dsp::fir::lowpass(fm, 15e3, 19e3, 60.0)
                    .expect("the filter settings are valid"),
            ),
            count: 0,
            blend: 0.0,
            wanted: None,
            choice,
        }
    }

    /// The audio working rate.
    pub fn output_rate(&self) -> f64 {
        self.fm / 5.0
    }

    /// The stereo choice.
    pub fn set_choice(&mut self, choice: StereoChoice) {
        self.choice = choice;
    }

    /// Sets the latest SNR measure: the target blend `clamp((SNR - 24) / 12, 0, 1)`, smoothed with about 1 s (the first one
    /// seeds it).
    pub fn set_snr(&mut self, snr_db: f32) {
        let w = ((snr_db - 24.0) / 12.0).clamp(0.0, 1.0);
        self.wanted = Some(match self.wanted {
            Some(p) => p + 0.1 * (w - p),
            None => w,
        });
    }

    fn wanted(&self) -> f32 {
        self.wanted.unwrap_or(1.0)
    }

    /// Decodes MPX samples: appends frames of `channels` channels (1: the sum signal, 2: left and right) at the working rate
    /// to `audio`, and one pilot sample per MPX sample to `pilot`.
    pub fn process(
        &mut self,
        mpx: &[f32],
        channels: u16,
        audio: &mut Vec<f32>,
        pilot: &mut Vec<PilotSample>,
    ) {
        let gain = 0.8 / 0.9f32;
        let wanted = self.wanted();
        for &x in mpx {
            let ps = self.pll.process(x);
            pilot.push(ps);
            self.fir.push(Frame([x, x * 2.0 * ps.sin2]));
            self.count += 1;
            if self.count < 5 {
                continue;
            }
            self.count = 0;
            let Frame([sum, diff]) = self.fir.output();
            let target = match self.choice {
                StereoChoice::Mono => 0.0,
                StereoChoice::Stereo => (ps.amplitude > 0.006) as u8 as f32,
                StereoChoice::Auto => {
                    if ps.locked {
                        wanted
                    } else {
                        0.0
                    }
                }
            };
            self.blend += 0.002 * (target - self.blend);
            if channels == 1 {
                audio.push(sum * gain);
            } else {
                audio.push((sum + self.blend * diff) * gain);
                audio.push((sum - self.blend * diff) * gain);
            }
        }
    }

    /// The report, given whether a carrier is present.
    pub fn report(&self, carrier: bool) -> StereoReport {
        let locked = self.pll.locked();
        let amp = self.pll.amplitude();
        let pilot = locked || (self.choice == StereoChoice::Stereo && amp > 0.006);
        let wanted = if self.choice == StereoChoice::Stereo {
            1.0
        } else {
            self.wanted()
        };
        let stereo = pilot
            && self.blend > 0.5 * wanted.max(0.1)
            && self.choice != StereoChoice::Mono
            && carrier;
        StereoReport {
            stereo,
            stereo_pct: if stereo { 100.0 * self.blend } else { 0.0 },
            pilot_pct: if locked && carrier { 100.0 * amp } else { 0.0 },
            locked,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_dev_test::measure::tone_amplitude;
    use std::f64::consts::TAU;

    #[test]
    fn separates_left_from_right_and_follows_the_choice() {
        let fm = 250e3;
        let mpx: Vec<f32> = (0..250_000)
            .map(|i| {
                let t = i as f64 / fm;
                let th = TAU * 19e3 * t;
                let l = 0.5 * (TAU * 1000.0 * t).sin();
                (0.45 * l + 0.45 * l * (2.0 * th).sin() + 0.09 * th.sin()) as f32
            })
            .collect();
        for (choice, sep_min, sep_max) in [
            (StereoChoice::Auto, 40.0, 200.0),
            (StereoChoice::Mono, -1.0, 1.0),
        ] {
            let mut s = StereoDecoder::new(fm, choice);
            s.set_snr(50.0);
            let (mut audio, mut pilot) = (Vec::new(), Vec::new());
            s.process(&mpx, 2, &mut audio, &mut pilot);
            assert_eq!(pilot.len(), mpx.len());
            assert_eq!(audio.len(), 2 * (mpx.len() / 5));
            let (l, r) = sdrkit_dev_test::measure::deinterleave(&audio[50_000..]);
            let sep = 20.0
                * (tone_amplitude(&l, 50e3, 1000.0) / tone_amplitude(&r, 50e3, 1000.0)).log10();
            assert!(sep > sep_min && sep < sep_max, "{choice:?}: {sep}");
            let rep = s.report(true);
            assert_eq!(rep.stereo, choice == StereoChoice::Auto);
            assert!((rep.pilot_pct - 9.0).abs() < 0.5);
        }
    }
}
