//! Health indicators: the colour of the status lock states of a receiver screen, the signal quality bar and the needle meter.

sdrkit_core::choice_enum! {
    /// The colour of one indicator.
    #[derive(Default)]
    pub enum Light {
        /// Red, or off.
        #[default]
        Off = ("off", "Off"),
        /// Amber.
        Amber = ("amber", "Amber"),
        /// Green.
        Green = ("green", "Green"),
    }
}

sdrkit_core::choice_enum! {
    /// The words of the signal quality bar.
    #[derive(Default)]
    pub enum Quality {
        /// Below 0.3.
        #[default]
        Poor = ("poor", "Poor"),
        /// 0.3 to 0.55.
        Fair = ("fair", "Fair"),
        /// Above 0.55.
        Good = ("good", "Good"),
        /// Above 0.8.
        Excellent = ("excellent", "Excellent"),
    }
}

/// Every indicator, with the quality bar and the needle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Health {
    /// Locked.
    pub sync: Light,
    /// PRS peak: green above 40, amber above 15.
    pub prs: Light,
    /// At least 11 of 12 FIBs good in the last frame. Amber for some good, off for none while locked.
    pub fic: Light,
    /// The ensemble is valid. Amber for FIBs good but not valid yet.
    pub ensemble: Light,
    /// Superframes good and bad at most a quarter of good. Amber for logical frames arriving.
    pub msc: Light,
    /// Decoding. Amber for selected but not decoding.
    pub audio: Light,
    /// Concealed below 0.5 %. Amber below 5 %.
    pub concealed: Light,
    /// The quality bar, (SNR − 4 dB) / 18 dB held to 0 … 1.
    pub quality: f32,
    /// Its words.
    pub quality_text: Quality,
    /// The needle, SNR / 40 dB held to 0 … 1.
    pub needle: f32,
}

/// What the indicators read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Signals {
    /// The receiver is locked.
    pub locked: bool,
    /// The PRS peak over its median.
    pub prs_peak: f32,
    /// FIBs good since the start.
    pub fib_ok: u64,
    /// FIBs good in the last frame.
    pub fib_ok_last_frame: u8,
    /// The SNR measure, dB.
    pub snr_db: f32,
    /// The ensemble is valid.
    pub ensemble_valid: bool,
    /// A service is selected.
    pub selected: bool,
    /// The service playing is MPEG Layer II.
    pub mp2: bool,
    /// Layer II frames, and the bad ones.
    pub mp2_frames: (u64, u64),
    /// Good and bad superframes.
    pub superframes: (u64, u64),
    /// Logical frames of the sub-channel playing.
    pub logical_frames: u64,
    /// The audio decodes.
    pub decoding: bool,
    /// The share of access units concealed, percent.
    pub concealed_pct: f64,
}

/// The quality bar for an SNR measure.
pub fn quality_bar(snr_db: f32) -> (f32, Quality) {
    let q = ((snr_db - 4.0) / 18.0).clamp(0.0, 1.0);
    let q = if q.is_nan() { 0.0 } else { q };
    let t = if q > 0.8 {
        Quality::Excellent
    } else if q > 0.55 {
        Quality::Good
    } else if q < 0.3 {
        Quality::Poor
    } else {
        Quality::Fair
    };
    (q, t)
}

/// The needle meter for an SNR measure (0 when stopped or not locked).
pub fn needle(snr_db: f32) -> f32 {
    let n = (snr_db / 40.0).clamp(0.0, 1.0);
    if n.is_nan() {
        0.0
    } else {
        n
    }
}

/// The indicators for what the receiver measured.
pub fn health(d: &Signals) -> Health {
    let light = |green: bool, amber: bool| {
        if green {
            Light::Green
        } else if amber {
            Light::Amber
        } else {
            Light::Off
        }
    };
    let locked = d.locked;
    let sync = light(locked, false);
    let prs = if locked {
        light(d.prs_peak > 40.0, d.prs_peak > 15.0)
    } else {
        Light::Off
    };
    let fic = if locked {
        light(d.fib_ok_last_frame >= 11, d.fib_ok_last_frame > 0)
    } else {
        Light::Off
    };
    let ensemble = light(d.ensemble_valid, d.fib_ok > 0);
    let (good, bad) = if d.mp2 { d.mp2_frames } else { d.superframes };
    let msc = if d.selected {
        light(good > 0 && bad * 4 <= good, d.logical_frames > 0)
    } else {
        Light::Off
    };
    let audio = if d.selected {
        light(d.decoding, true)
    } else {
        Light::Off
    };
    let concealed = light(d.concealed_pct < 0.5, d.concealed_pct < 5.0);
    let snr = if locked { d.snr_db } else { 0.0 };
    let (quality, quality_text) = quality_bar(snr);
    Health {
        sync,
        prs,
        fic,
        ensemble,
        msc,
        audio,
        concealed,
        quality,
        quality_text,
        needle: needle(snr),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_state_rules() {
        let mut d = Signals::default();
        let h = health(&d);
        assert_eq!(h.sync, Light::Off);
        assert_eq!(h.ensemble, Light::Off);
        assert_eq!(h.msc, Light::Off);
        assert_eq!(h.audio, Light::Off);
        assert_eq!(h.quality, 0.0);
        assert_eq!(h.needle, 0.0);

        d.locked = true;
        d.prs_peak = 30.0;
        d.fib_ok = 10;
        d.fib_ok_last_frame = 5;
        d.snr_db = 20.0;
        d.selected = true;
        d.logical_frames = 3;
        let h = health(&d);
        assert_eq!(h.sync, Light::Green);
        assert_eq!(h.prs, Light::Amber);
        assert_eq!(h.fic, Light::Amber);
        assert_eq!(h.ensemble, Light::Amber);
        assert_eq!(h.msc, Light::Amber);
        assert_eq!(h.audio, Light::Amber);
        assert!((h.quality - 16.0 / 18.0).abs() < 1e-6);
        assert_eq!(h.quality_text, Quality::Excellent);
        assert!((h.needle - 0.5).abs() < 1e-6);

        d.prs_peak = 120.0;
        d.fib_ok_last_frame = 11;
        d.ensemble_valid = true;
        d.decoding = true;
        d.superframes = (8, 2);
        d.concealed_pct = 1.0;
        let h = health(&d);
        assert_eq!(h.prs, Light::Green);
        assert_eq!(h.fic, Light::Green);
        assert_eq!(h.ensemble, Light::Green);
        assert_eq!(h.msc, Light::Green);
        assert_eq!(h.audio, Light::Green);
        assert_eq!(h.concealed, Light::Amber);
        d.superframes.1 = 3;
        d.concealed_pct = 5.0;
        let h = health(&d);
        assert_eq!(h.msc, Light::Amber);
        assert_eq!(h.concealed, Light::Off);

        // Locked with no good FIB in the last frame is red.
        d.fib_ok_last_frame = 0;
        assert_eq!(health(&d).fic, Light::Off);
    }

    #[test]
    fn quality_words() {
        assert_eq!(quality_bar(4.0), (0.0, Quality::Poor));
        assert_eq!(quality_bar(10.0).1, Quality::Fair);
        assert_eq!(quality_bar(14.5).1, Quality::Good);
        assert_eq!(quality_bar(30.0), (1.0, Quality::Excellent));
        assert_eq!(quality_bar(f32::NAN).0, 0.0);
        assert_eq!(needle(50.0), 1.0);
    }
}
