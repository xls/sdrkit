//! Second-order statistics of a block of IQ samples and the blind reading of a quadrature mixer from them: the DC of I and
//! Q, the gain mismatch `10 log10(V_Q / V_I)`, the phase error `asin(C_IQ / sqrt(V_I V_Q))` and the image rejection
//! `-10 log10(max((e^2 + p^2) / 4, 1e-12))` with `e = sqrt(V_I / V_Q) - 1` and `p` the phase error in radians. The reading
//! assumes a complex signal with no correlation between I and Q (signals spread over both sides of the centre). General
//! theory of quadrature mixers; no standard.

use num_complex::Complex32;

/// The sums of a block.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IqMoments {
    n: u64,
    si: f64,
    sq: f64,
    sii: f64,
    sqq: f64,
    siq: f64,
}

/// The reading of one block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IqReading {
    /// The samples.
    pub n: u64,
    /// The mean of I.
    pub dc_i: f64,
    /// The mean of Q.
    pub dc_q: f64,
    /// The mean of `I^2 + Q^2` (DC included).
    pub power: f64,
    /// The variance of I about its mean.
    pub var_i: f64,
    /// The variance of Q about its mean.
    pub var_q: f64,
    /// The covariance of I and Q.
    pub cov_iq: f64,
}

impl IqMoments {
    /// Adds `x` to the sums.
    pub fn push(&mut self, x: &[Complex32]) {
        for z in x {
            let (i, q) = (f64::from(z.re), f64::from(z.im));
            self.si += i;
            self.sq += q;
            self.sii += i * i;
            self.sqq += q * q;
            self.siq += i * q;
        }
        self.n += x.len() as u64;
    }

    /// The samples summed.
    pub fn count(&self) -> u64 {
        self.n
    }

    /// The reading of the sums; `None` without a sample.
    pub fn reading(&self) -> Option<IqReading> {
        if self.n == 0 {
            return None;
        }
        let n = self.n as f64;
        let (mi, mq) = (self.si / n, self.sq / n);
        Some(IqReading {
            n: self.n,
            dc_i: mi,
            dc_q: mq,
            power: (self.sii + self.sqq) / n,
            var_i: self.sii / n - mi * mi,
            var_q: self.sqq / n - mq * mq,
            cov_iq: self.siq / n - mi * mq,
        })
    }
}

impl IqReading {
    /// `10 log10(V_Q / V_I)`, dB; `None` without variance.
    pub fn gain_mismatch_db(&self) -> Option<f64> {
        (self.var_i > 0.0 && self.var_q > 0.0).then(|| 10.0 * (self.var_q / self.var_i).log10())
    }

    /// `asin(C_IQ / sqrt(V_I V_Q))`, radians; `None` without variance.
    pub fn phase_error_rad(&self) -> Option<f64> {
        (self.var_i > 0.0 && self.var_q > 0.0).then(|| {
            (self.cov_iq / (self.var_i * self.var_q).sqrt())
                .clamp(-1.0, 1.0)
                .asin()
        })
    }

    /// The image rejection that the mismatch implies, dB, at most 120.
    pub fn image_rejection_db(&self) -> Option<f64> {
        let p = self.phase_error_rad()?;
        let e = (self.var_i / self.var_q).sqrt() - 1.0;
        Some((-10.0 * ((e * e + p * p) / 4.0).max(1e-12).log10()).min(120.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_mismatch_and_offset_read_back() {
        let (g, phi) = (10f64.powf(0.8 / 20.0), 2.5f64.to_radians());
        let mut m = IqMoments::default();
        let mut s = 0x1234_5678_9abc_def1u64;
        let mut u = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let x: Vec<Complex32> = (0..400_000)
            .map(|_| {
                let r = (-u().ln()).sqrt();
                let a = std::f64::consts::TAU * u();
                let (i, q) = (r * a.cos(), r * a.sin());
                Complex32::new(
                    (i + 0.01) as f32,
                    (g * (q * phi.cos() + i * phi.sin()) - 0.02) as f32,
                )
            })
            .collect();
        m.push(&x);
        let r = m.reading().unwrap();
        assert!(
            (r.dc_i - 0.01).abs() < 0.005 && (r.dc_q + 0.02).abs() < 0.005,
            "{r:?}"
        );
        assert!((r.gain_mismatch_db().unwrap() - 0.8).abs() < 0.05);
        assert!((r.phase_error_rad().unwrap().to_degrees() - 2.5).abs() < 0.2);
        let ir = r.image_rejection_db().unwrap();
        let (e, p) = (1.0 / g - 1.0, phi);
        let want = -10.0 * ((e * e + p * p) / 4.0).log10();
        assert!((ir - want).abs() < 0.5, "{ir} against {want}");
        assert!(IqMoments::default().reading().is_none());
    }
}
