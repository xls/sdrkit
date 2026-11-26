//! The Wi-Fi test signal: a schedule of DSSS and non-HT OFDM PPDUs, separated by silence. An OFDM frame can sit anywhere in a
//! wide capture and be repeated at further offsets (the non-HT duplicate of a wider BSS). The framework adds noise and the
//! receiver's impairments from the standard `gen_*` settings.
//! Standard: IEEE Std 802.11-2020 clause 16 (DSSS) and clause 17 (OFDM).

use super::frames::*;
use super::plan::{wide_centre, Band};
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::scene::{Burst, BurstSource, Scene};
use sdrkit_core::{Complex32, Generator, Hz, SignalInfo};
use sdrkit_proto_wlan_phy::{dsss, ofdm};

/// The PHY of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phy {
    /// DSSS at 1 or 2 Mbit/s.
    Dsss,
    /// Non-HT OFDM.
    Ofdm,
}

/// One frame of the schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct GenFrame {
    /// The MPDU with its FCS.
    pub mpdu: Vec<u8>,
    /// The PHY.
    pub phy: Phy,
    /// The OFDM rate (6 Mbit/s by default).
    pub ofdm_rate: ofdm::Rate,
    /// The DSSS rate (1 Mbit/s by default).
    pub dsss_rate: dsss::Rate,
    /// The DSSS preamble (long by default).
    pub preamble: dsss::Preamble,
    /// Silence after it, s.
    pub gap_s: f64,
    /// Amplitude scale of this transmitter (1 is full power).
    pub scale: f64,
    /// This transmitter's own carrier offset, Hz.
    pub offset_hz: f64,
    /// Its position in the capture, Hz.
    pub position_hz: f64,
    /// Further copies at these positions (the non-HT duplicate).
    pub duplicates: Vec<f64>,
    /// The OFDM scrambler seed (1 to 127).
    pub seed: u8,
}

impl GenFrame {
    /// A frame with defaults: gap 2 ms, scale 1, no offset, 6 Mbit/s OFDM or 1 Mbit/s DSSS with the long preamble.
    pub fn new(mpdu: Vec<u8>, phy: Phy) -> GenFrame {
        GenFrame {
            mpdu,
            phy,
            ofdm_rate: ofdm::Rate::R6,
            dsss_rate: dsss::Rate::Dbpsk1,
            preamble: dsss::Preamble::Long,
            gap_s: 2e-3,
            scale: 1.0,
            offset_hz: 0.0,
            position_hz: 0.0,
            duplicates: Vec::new(),
            seed: 0x5D,
        }
    }
    /// With a gap.
    pub fn gap(mut self, s: f64) -> Self {
        self.gap_s = s;
        self
    }
    /// With an amplitude scale.
    pub fn scale(mut self, s: f64) -> Self {
        self.scale = s;
        self
    }
    /// With its own carrier offset.
    pub fn offset(mut self, hz: f64) -> Self {
        self.offset_hz = hz;
        self
    }
    /// At this OFDM rate.
    pub fn ofdm_rate(mut self, r: ofdm::Rate) -> Self {
        self.ofdm_rate = r;
        self
    }
    /// At this DSSS rate and preamble.
    pub fn dsss(mut self, r: dsss::Rate, p: dsss::Preamble) -> Self {
        self.dsss_rate = r;
        self.preamble = p;
        self
    }
    /// The test flag "bad FCS": bit 4 of the last FCS byte inverted.
    pub fn bad_fcs(mut self) -> Self {
        if let Some(b) = self.mpdu.last_mut() {
            *b ^= 0x10;
        }
        self
    }
}

/// What the generator sends.
#[derive(Clone, Debug, PartialEq)]
pub struct WifiGenParams {
    /// Output rate, Hz.
    pub rate_hz: Hz,
    /// Silence before the first frame, s.
    pub lead_s: f64,
    /// Repeat the schedule.
    pub looped: bool,
    /// The schedule.
    pub frames: Vec<GenFrame>,
}

impl Default for WifiGenParams {
    fn default() -> Self {
        WifiGenParams {
            rate_hz: 20e6,
            lead_s: 1e-3,
            looped: true,
            frames: scene_24(),
        }
    }
}

/// A station that the scenes' frames are sent to.
pub const CLIENT: [u8; 6] = [0x3A, 0x01, 0x02, 0x03, 0x04, 0x05];

/// The 2.4 GHz scene on channel 6, all DSSS (3.14).
pub fn scene_24() -> Vec<GenFrame> {
    let mut home = BssSpec::new(
        [0x00, 0x04, 0x0E, 0x12, 0x34, 0x56],
        "Test-Home",
        Band::G2,
        6,
        Sec::Wpa2,
    );
    home.tsf = 123_456_789;
    home.seq = 100;
    let mut cafe = BssSpec::new(
        [0xFC, 0xEC, 0xDA, 0xAB, 0xCD, 0xEF],
        "Test-Cafe",
        Band::G2,
        6,
        Sec::Wpa2Wpa3,
    );
    cafe.tsf = 987_654_321;
    let mut hidden = BssSpec::new(
        [0x02, 0x11, 0x22, 0x33, 0x44, 0x55],
        "",
        Band::G2,
        6,
        Sec::Open,
    );
    hidden.hidden = true;
    hidden.rates = Some(vec![0x82, 0x84, 0x8B, 0x96]);
    hidden.interval = 200;
    vec![
        GenFrame::new(beacon(&home), Phy::Dsss),
        GenFrame::new(beacon(&cafe), Phy::Dsss)
            .scale(0.6)
            .offset(12e3),
        GenFrame::new(data_frame(home.bssid, CLIENT, 120, 1), Phy::Dsss).gap(1e-3),
        GenFrame::new(beacon(&hidden), Phy::Dsss)
            .scale(0.8)
            .offset(-8e3),
        GenFrame::new(probe_response(&home, CLIENT), Phy::Dsss).gap(3e-3),
    ]
}

/// The three access points of the 5 or 6 GHz scene on primary `p`.
pub fn scene_aps(band: Band, p: u8) -> [BssSpec; 3] {
    let wide = |w: u32| {
        if wide_centre(band, p as u32, w).is_some() {
            w
        } else {
            20
        }
    };
    if band == Band::G6 {
        let mut a = BssSpec::new(
            [0x24, 0xA4, 0x3C, 0x60, 0x00, 0x01],
            "Test-6E",
            band,
            p,
            Sec::Wpa3,
        );
        a.width = wide(80);
        a.he = Some(HeSpec {
            streams: 2,
            twt: true,
            color: 17,
            color_disabled: false,
            reg: 0,
            dup: true,
        });
        a.eht = true;
        a.country = Some("DE I".into());
        a.power_constraint = Some(3);
        a.mdid = Some(0x1234);
        let mut b = BssSpec::new(
            [0x00, 0x04, 0x0E, 0x77, 0x88, 0x99],
            "Test-6E-Open",
            band,
            p,
            Sec::Owe,
        );
        b.he = Some(HeSpec {
            streams: 1,
            twt: false,
            color: 33,
            color_disabled: false,
            reg: 1,
            dup: false,
        });
        let mut c = BssSpec::new(
            [0xFC, 0xEC, 0xDA, 0x12, 0x34, 0x56],
            "Hidden-6E",
            band,
            p,
            Sec::Wpa3,
        );
        c.hidden = true;
        c.he = Some(HeSpec {
            streams: 4,
            twt: false,
            color: 0,
            color_disabled: true,
            reg: 0,
            dup: false,
        });
        [a, b, c]
    } else {
        let mut a = BssSpec::new(
            [0x24, 0xA4, 0x3C, 0x50, 0x00, 0x01],
            "Test-5G",
            band,
            p,
            Sec::Wpa2Wpa3,
        );
        a.width = wide(80);
        a.ht = Some(2);
        a.vht = Some(2);
        a.he = Some(HeSpec {
            streams: 2,
            twt: true,
            color: 17,
            color_disabled: false,
            reg: 0,
            dup: false,
        });
        a.country = Some("DE I".into());
        a.power_constraint = Some(3);
        a.mdid = Some(0x1234);
        a.rnr = vec![RnrSpec {
            class: 131,
            channel: 5,
            bssid: [0x24, 0xA4, 0x3C, 0x60, 0x00, 0x01],
            short_ssid: short_ssid("Test-6E"),
            params: 0x02 | 0x20 | 0x40,
        }];
        let mut b = BssSpec::new(
            [0x00, 0x04, 0x0E, 0x77, 0x88, 0x99],
            "Test-Old",
            band,
            p,
            Sec::Wpa2,
        );
        b.ht = Some(3);
        let mut c = BssSpec::new(
            [0xFC, 0xEC, 0xDA, 0x12, 0x34, 0x56],
            "Test-ac",
            band,
            p,
            Sec::Wpa2,
        );
        c.ht = Some(4);
        c.vht = Some(4);
        c.width = wide(40);
        [a, b, c]
    }
}

/// The 5 or 6 GHz scene on primary `p`, all OFDM, gaps 0.8 ms.
pub fn scene_56(band: Band, p: u8) -> Vec<GenFrame> {
    let [a, b, c] = scene_aps(band, p);
    let mut v = vec![
        GenFrame::new(beacon(&a), Phy::Ofdm).gap(0.8e-3),
        GenFrame::new(beacon(&b), Phy::Ofdm)
            .gap(0.8e-3)
            .scale(0.7)
            .offset(15e3),
        GenFrame::new(beacon(&c), Phy::Ofdm)
            .gap(0.8e-3)
            .scale(0.85)
            .offset(-20e3),
        GenFrame::new(probe_response(&a, CLIENT), Phy::Ofdm).gap(0.8e-3),
    ];
    if band == Band::G6 {
        v.push(GenFrame::new(fils(&a), Phy::Ofdm).gap(0.8e-3));
        v.push(GenFrame::new(probe_response(&a, [0xFF; 6]), Phy::Ofdm).gap(0.8e-3));
    }
    v
}

/// The clean samples of one frame at `rate_hz`, at unit mean power. Errors for a frame the PHY refuses.
pub fn frame_samples(
    f: &GenFrame,
    rate_hz: f64,
) -> Result<Vec<Complex32>, crate::testgen::GenError> {
    let mut x = Vec::new();
    match f.phy {
        Phy::Dsss => {
            let h = dsss::PlcpHeader::for_psdu(f.dsss_rate, f.mpdu.len())?;
            let mut bits = Vec::new();
            dsss::ppdu_bits(f.preamble, &h, &f.mpdu, &mut bits)?;
            dsss::scramble(&mut bits, f.preamble);
            let mut sym = Vec::new();
            dsss::symbols(&bits, f.preamble, f.dsss_rate, &mut sym)?;
            let mut chips = Vec::new();
            dsss::spread(&sym, &mut chips);
            if rate_hz >= ofdm::SAMPLE_RATE * 0.999_999 {
                let mut m = Linear::new(Shape::Rect, 1.0, rate_hz / dsss::CHIP_RATE)?;
                m.push(&chips, &mut x);
                m.flush(&mut x);
            } else {
                // Below 20 Msps the chips are made at two samples a chip and resampled, as a radio's filter would.
                let mut y = Vec::new();
                let mut m = Linear::new(Shape::Rect, 1.0, 2.0)?;
                m.push(&chips, &mut y);
                m.flush(&mut y);
                // The tail carries the last chips out of the resampler's filter, however long it is.
                let mut r = FractionalResampler::with_rates(2.0 * dsss::CHIP_RATE, rate_hz)?;
                y.extend(std::iter::repeat_n(Complex32::default(), 4096));
                r.process(&y, &mut x);
            }
        }
        Phy::Ofdm => {
            let mut y = Vec::new();
            ofdm::build_ppdu(&f.mpdu, f.ofdm_rate, f.seed, &mut y)?;
            if (rate_hz - ofdm::SAMPLE_RATE).abs() < 1.0 {
                x = y;
            } else {
                let mut r = FractionalResampler::with_rates(ofdm::SAMPLE_RATE, rate_hz)?;
                y.extend(std::iter::repeat_n(Complex32::default(), 64));
                r.process(&y, &mut x);
            }
        }
    }
    // Both PHYs are at unit power by construction: DSSS chips have unit magnitude and the OFDM fields have unit mean power.
    // A scale over the whole buffer would count the filter tails and lift the frame above full scale.
    Ok(x)
}

/// Makes the bursts of the schedule when they are due: one burst for each position of a frame.
struct Schedule {
    p: WifiGenParams,
    index: usize,
    at_s: f64,
    pending: Vec<Burst>,
    done: bool,
}

impl BurstSource for Schedule {
    fn next_burst(&mut self) -> Option<Burst> {
        if let Some(b) = self.pending.pop() {
            return Some(b);
        }
        if self.done || self.p.frames.is_empty() {
            return None;
        }
        if self.index >= self.p.frames.len() {
            if !self.p.looped {
                self.done = true;
                return None;
            }
            self.index = 0;
        }
        let f = &self.p.frames[self.index];
        self.index += 1;
        let samples = frame_samples(f, self.p.rate_hz).ok()?;
        let start_s = self.at_s;
        self.at_s += samples.len() as f64 / self.p.rate_hz + f.gap_s;
        let level_db = 20.0 * f.scale.max(1e-9).log10();
        for &pos in f.duplicates.iter().rev() {
            self.pending.push(Burst {
                start_s,
                offset_hz: pos + f.offset_hz,
                level_db,
                samples: samples.clone(),
            });
        }
        Some(Burst {
            start_s,
            offset_hz: f.position_hz + f.offset_hz,
            level_db,
            samples,
        })
    }
}

/// The Wi-Fi test-signal generator.
pub struct WifiGenerator {
    rate_hz: f64,
    scene: Option<Scene>,
}

impl WifiGenerator {
    /// A generator of `p`.
    pub fn new(p: WifiGenParams) -> WifiGenerator {
        let rate_hz = p.rate_hz;
        let scene = Scene::new(rate_hz).ok().map(|mut s| {
            s.add_source(Box::new(Schedule {
                at_s: p.lead_s,
                p,
                index: 0,
                pending: Vec::new(),
                done: false,
            }));
            s
        });
        WifiGenerator { rate_hz, scene }
    }
}

impl Generator for WifiGenerator {
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
        Some(SignalInfo {
            bandwidth_hz: 20e6,
            symbol_rate_hz: None,
            bit_rate_hz: None,
            power: 1.0,
        })
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
