//! The channel filter of one LoRa channel, and the resampler to twice the chip rate.

use crate::error::{LoraError, Param, Result};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_core::Complex32;

/// The room kept for a carrier offset at each side of the LoRa band: 25 kHz or 0.2 BW, the larger.
pub fn offset_room(bw: f64) -> f64 {
    (25e3f64).max(0.2 * bw)
}

/// True if a channel at `offset_hz` of `bw` fits an input at `rate`: `|offset| + BW/2 + 25 kHz < 0.45 rate`, and the rate is
/// 2.2 BW or more.
pub fn in_band(rate: f64, offset_hz: f64, bw: f64) -> bool {
    offset_hz.abs() + bw / 2.0 + 25e3 < 0.45 * rate && rate >= 2.2 * bw
}

/// The decimation of the channel filter: `max(1, floor(rate / 3 BW))`, made smaller until the output is 2 BW or more.
pub fn decimation(rate: f64, bw: f64) -> usize {
    let mut d = ((rate / (3.0 * bw)).floor() as usize).max(1);
    while d > 1 && rate / (d as f64) < 2.0 * bw {
        d -= 1;
    }
    d
}

fn check(rate: f64, bw: f64) -> Result<()> {
    if !(rate.is_finite() && bw.is_finite() && bw > 0.0 && rate >= 2.0 * bw) {
        return Err(LoraError::BadParameter(Param::Rate));
    }
    Ok(())
}

/// Shifts, filters and decimates one channel.
#[derive(Clone, Debug)]
pub struct ChannelFilter {
    nco: Nco,
    stage1: Option<FirDecimator<Complex32>>,
    stage2: Option<Fir<Complex32>>,
    mixed: Vec<Complex32>,
    out_rate: f64,
    delay_in: f64,
    d: usize,
}

impl ChannelFilter {
    /// The channel at `offset_hz` of `bw` in an input at `rate`. If the rate is below 2 BW, or a value is not finite, this
    /// returns an error.
    pub fn new(rate: f64, offset_hz: f64, bw: f64) -> Result<Self> {
        check(rate, bw)?;
        if !offset_hz.is_finite() {
            return Err(LoraError::BadParameter(Param::ChannelOffset));
        }
        let d = decimation(rate, bw);
        let out = rate / d as f64;
        let pass = (bw / 2.0 + offset_room(bw)).min(0.42 * out);
        let stage1 = if d > 1 {
            let alias_free = pass.min(out - pass - 1.0);
            let stop = (out - alias_free).min(rate / 2.0);
            if stop <= pass.min(alias_free) {
                return Err(LoraError::BadParameter(Param::ChannelFilter));
            }
            Some(
                FirDecimator::design(rate, d, pass.min(alias_free), alias_free, 55.0)
                    .map_err(LoraError::Dsp)?,
            )
        } else {
            None
        };
        let stop = (pass + 0.3 * bw).min(0.5 * out);
        let stage2 = if stop > pass * 1.02 {
            let taps =
                sdrkit_algo_dsp::fir::lowpass(out, pass, stop, 55.0).map_err(LoraError::Dsp)?;
            Some(Fir::new(&taps))
        } else {
            None
        };
        let delay_in = stage1.as_ref().map_or(0.0, |s| s.delay())
            + stage2.as_ref().map_or(0.0, |f| f.group_delay()) * d as f64;
        Ok(ChannelFilter {
            nco: Nco::new(rate, -offset_hz),
            stage1,
            stage2,
            mixed: Vec::new(),
            out_rate: out,
            delay_in,
            d,
        })
    }

    /// The output rate, Hz.
    pub fn output_rate(&self) -> f64 {
        self.out_rate
    }

    /// The decimation.
    pub fn decimation(&self) -> usize {
        self.d
    }

    /// The delay of the filters, in input samples.
    pub fn delay(&self) -> f64 {
        self.delay_in
    }

    /// Appends the channel samples for `input` to `out`.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        self.mixed.resize(input.len(), Complex32::default());
        self.nco.mix(input, &mut self.mixed);
        let start = out.len();
        match &mut self.stage1 {
            Some(s) => s.process(&self.mixed, out),
            None => out.extend_from_slice(&self.mixed),
        }
        if let Some(f) = &mut self.stage2 {
            f.process_in_place(&mut out[start..]);
        }
    }
}

/// Resamples a channel to exactly twice the chip rate.
#[derive(Clone, Debug)]
pub struct ChipResampler {
    res: Option<FractionalResampler<Complex32>>,
    delay_in: f64,
}

impl ChipResampler {
    /// A resampler from a channel at `rate` to `2 bw`. If the rate is below 2 BW, or the filter does not fit, this returns an
    /// error.
    pub fn new(rate: f64, bw: f64) -> Result<Self> {
        check(rate, bw)?;
        let out = 2.0 * bw;
        if (rate - out).abs() < 1e-6 * out {
            return Ok(ChipResampler {
                res: None,
                delay_in: 0.0,
            });
        }
        let low = rate.min(out);
        let pass = (bw / 2.0 + offset_room(bw)).min(0.4 * low);
        let stop = (low - pass).min(rate - pass).max(pass * 1.05);
        let res = FractionalResampler::new(rate, out, pass, stop, 55.0).map_err(LoraError::Dsp)?;
        let delay_in = res.delay();
        Ok(ChipResampler {
            res: Some(res),
            delay_in,
        })
    }

    /// The delay, in channel samples.
    pub fn delay(&self) -> f64 {
        self.delay_in
    }

    /// Appends the resampled samples to `out`.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        match &mut self.res {
            Some(r) => r.process(input, out),
            None => out.extend_from_slice(input),
        }
    }
}
