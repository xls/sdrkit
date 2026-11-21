//! SSB transmitter test meter (`ssb_test`): the transmitter's suppressed carrier is the dial and it sends one or two audio
//! tones. One tone: the carrier leak, the opposite sideband and harmonics 2 to 5 of the tone, in dB under it. Two tones within
//! a set level of each other (the two-tone test of ITU-R SM.326): the third, fifth and seventh order products, each the
//! stronger of its pair, under one tone and under the peak envelope power. A product under the noise is shown as the floor of
//! this measurement with a flag; so is a carrier that is not seen, as the floor of its search. It measures at this radio:
//! an overloaded radio shows its own products. The rules are confirmed by no public document.

pub mod gen;

use crate::instrument::{Front, Log};
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{SsbSideband, SsbTestEventRow, SsbTestFields, SsbTestKind, SSB_TEST_EVENTS};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "ssb_test";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 12e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 12e3;
/// The pass edge of the channel filter, Hz.
pub const PASS_HZ: f64 = 4200.0;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.2;
/// The samples of a window.
pub const WINDOW: usize = 1 << 15;
/// The noise is read at these audio frequencies, Hz: from the first, in steps of the second, under the third.
pub const NOISE_GRID: (f64, f64, f64) = (200.0, 7.3, 3800.0);
/// A tone candidate is this many times the noise per bin (25 dB).
pub const TONE_RATIO: f64 = 316.0;
/// A second candidate is within this many dB of the strongest.
pub const CANDIDATE_DB: f64 = 30.0;
/// A second candidate is more than this far from the strongest, Hz.
pub const CANDIDATE_GAP_HZ: f64 = 40.0;
/// The carrier is searched this far from the dial, Hz.
pub const CARRIER_SEARCH_HZ: f64 = 60.0;
/// The carrier is found if the 7-bin sum at the strongest bin of its search is this many times that of the noise
/// (10 dB). The search spans about 330 bins: in 3000 windows of noise alone, 4 times was passed in 6 % and this ratio in
/// none.
pub const CARRIER_FOUND_RATIO: f64 = 10.0;
/// A level is searched this far from its expected frequency, Hz.
pub const LEVEL_SEARCH_HZ: f64 = 2.5;
/// The highest audio frequency of a harmonic or a product, Hz.
pub const AUDIO_TOP_HZ: f64 = 3900.0;
/// The lowest audio frequency of a product, Hz.
pub const PRODUCT_LOW_HZ: f64 = 30.0;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures an SSB transmitter on one or two test tones: carrier leak, opposite sideband, harmonics \
2 to 5, and the products of orders 3, 5 and 7 under one tone and under PEP; decodes nothing. A level under the noise is the \
floor of this measurement, not of the transmitter. An overloaded radio shows its own products: change the attenuator and \
see that the numbers do not move. The rules are confirmed by no public document.";

/// The low edge of the tone search.
pub const TONE_MIN_HZ: Setting<field::F64> =
    Setting::new("tone_min_hz", "Tone search, low edge", "Hz", 100.0).range(50.0, 3800.0, 0.0);
/// The high edge of the tone search.
pub const TONE_MAX_HZ: Setting<field::F64> =
    Setting::new("tone_max_hz", "Tone search, high edge", "Hz", 3500.0).range(50.0, 3800.0, 0.0);
/// The second tone is within this level of the first.
pub const TWO_TONE_DB: Setting<field::F64> =
    Setting::new("two_tone_db", "Second tone within", "dB", 10.0).range(3.0, 20.0, 0.0);
/// PEP over one tone.
pub const PEP_OFFSET_DB: Setting<field::F64> =
    Setting::new("pep_offset_db", "PEP over one tone", "dB", 6.02)
        .range(0.0, 10.0, 0.0)
        .help("6.02 dB for two equal tones.");

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The low edge.
        ToneMinHz = TONE_MIN_HZ,
        /// The high edge.
        ToneMaxHz = TONE_MAX_HZ,
        /// The two-tone level.
        TwoToneDb = TWO_TONE_DB,
        /// The PEP offset.
        PepOffsetDb = PEP_OFFSET_DB,
    }
}

sdrkit_core::settings! {
    /// The settings of the `ssb_test` stack.
    pub enum SsbTestSetting {
        /// The low edge.
        ToneMinHz = TONE_MIN_HZ,
        /// The high edge.
        ToneMaxHz = TONE_MAX_HZ,
        /// The two-tone level.
        TwoToneDb = TWO_TONE_DB,
        /// The PEP offset.
        PepOffsetDb = PEP_OFFSET_DB,
        /// The tones of the test signal.
        GenTones = gen::GEN_TONES,
        /// The sideband.
        GenSideband = gen::GEN_SIDEBAND,
        /// The carrier leak.
        GenCarrierDbc = gen::GEN_CARRIER_DBC,
    }
}

/// A level under the level of a tone: dB under and the floor flag.
type Level = Option<(f64, bool)>;

/// The readings of one window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Reading {
    upper: bool,
    tone1: f64,
    tone2: Option<f64>,
    diff: Option<f64>,
    /// dB under the tone and the floor flag; a carrier not found is the floor of its search.
    carrier: (f64, bool),
    carrier_found: bool,
    opposite: Level,
    h: [Level; 4],
    imd: [Level; 3],
    floor: f64,
}

/// The measurement of `ssb_test`.
#[derive(Default)]
pub struct SsbTest {
    front: Option<Front>,
    fft: Option<Fft>,
    window: Vec<f32>,
    buf: Vec<Complex32>,
    x: Vec<Complex32>,
    p: Vec<f64>,
    rate: f64,
    lo: f64,
    hi: f64,
    two_db: f64,
    pep: f64,
    t: f64,
    windows: u64,
    last_kind: Option<SsbTestKind>,
    last: Option<Reading>,
    log: Log<SsbTestEventRow>,
}

impl SsbTest {
    fn bin(&self, f: f64) -> usize {
        let n = WINDOW as i64;
        ((f * n as f64 / self.rate).round() as i64).rem_euclid(n) as usize
    }

    fn sum7(&self, k: usize) -> f64 {
        let n = WINDOW;
        (0..7).map(|d| self.p[(k + n - 3 + d) % n]).sum()
    }

    /// The strongest bin within `half` bins of frequency `f` and its 7-bin sum.
    fn strongest_near(&self, f: f64, half: usize) -> (usize, f64) {
        let n = WINDOW;
        let c = self.bin(f);
        let k = (0..=2 * half)
            .map(|d| (c + n - half + d) % n)
            .max_by(|&a, &b| self.p[a].total_cmp(&self.p[b]))
            .unwrap_or(c);
        (k, self.sum7(k))
    }

    fn measure(&mut self) -> Option<Reading> {
        let n = WINDOW;
        let b = self.rate / n as f64;
        let mut grid = Vec::new();
        let mut f = NOISE_GRID.0;
        while f < NOISE_GRID.2 {
            grid.push(self.p[self.bin(f)]);
            grid.push(self.p[self.bin(-f)]);
            f += NOISE_GRID.1;
        }
        let m = grid.len() / 2;
        let (_, med, _) = grid.select_nth_unstable_by(m, f64::total_cmp);
        let noise = *med / std::f64::consts::LN_2;
        let n7 = 7.0 * noise;
        let (k0, k1) = (
            (self.lo / b).ceil() as usize,
            (self.hi / b).floor() as usize,
        );
        let mut cands: Vec<(f64, f64)> = Vec::new();
        for sign in [1.0, -1.0] {
            for j in k0..=k1 {
                let k = self.bin(sign * j as f64 * b);
                let pk = self.p[k];
                if pk < TONE_RATIO * noise {
                    continue;
                }
                if (1..=4).any(|d| self.p[(k + d) % n] > pk || self.p[(k + n - d) % n] > pk) {
                    continue;
                }
                cands.push((sign * j as f64 * b, self.sum7(k)));
            }
        }
        let up: f64 = cands.iter().filter(|c| c.0 > 0.0).map(|c| c.1).sum();
        let down: f64 = cands.iter().filter(|c| c.0 < 0.0).map(|c| c.1).sum();
        let upper = up >= down;
        let mut side: Vec<(f64, f64)> =
            cands.into_iter().filter(|c| (c.0 > 0.0) == upper).collect();
        side.sort_by(|a, b| b.1.total_cmp(&a.1));
        let &(f1, l1) = side.first()?;
        let others: Vec<(f64, f64)> = side[1..]
            .iter()
            .copied()
            .filter(|c| {
                10.0 * (l1 / c.1).log10() <= CANDIDATE_DB
                    && (c.0.abs() - f1.abs()).abs() > CANDIDATE_GAP_HZ
            })
            .collect();
        let second = others
            .iter()
            .copied()
            .find(|c| 10.0 * (l1 / c.1).log10() <= self.two_db);
        let half_c = 2usize.max((CARRIER_SEARCH_HZ / b).round() as usize);
        let (kc, pc) = self.strongest_near(0.0, half_c);
        let carrier_found = pc > CARRIER_FOUND_RATIO * n7;
        let fc = if carrier_found {
            let j = if kc > n / 2 {
                kc as f64 - n as f64
            } else {
                kc as f64
            };
            j * b
        } else {
            0.0
        };
        let s = if upper { 1.0 } else { -1.0 };
        let half_l = 2usize.max((LEVEL_SEARCH_HZ / b).round() as usize);
        let level = |g: f64, pref: f64| -> (f64, bool) {
            let (_, pg) = self.strongest_near(g, half_l);
            let under = pg < 4.0 * n7;
            (
                10.0 * (pref / if under { 4.0 * n7 } else { pg }).log10(),
                under,
            )
        };
        // A carrier not found is under the threshold of its search: that threshold is the bound.
        let carrier = |pref: f64| -> (f64, bool) {
            if carrier_found {
                level(fc, pref)
            } else {
                (10.0 * (pref / (CARRIER_FOUND_RATIO * n7)).log10(), true)
            }
        };
        let mut r = Reading {
            upper,
            carrier_found,
            ..Reading::default()
        };
        match second {
            None => {
                let a1 = (f1 - fc).abs();
                r.tone1 = a1;
                r.carrier = carrier(l1);
                r.opposite = Some(level(fc - s * a1, l1));
                for (i, h) in r.h.iter_mut().enumerate() {
                    let k = (i + 2) as f64;
                    *h = (k * a1 < AUDIO_TOP_HZ).then(|| level(fc + s * k * a1, l1));
                }
                r.floor = 10.0 * (l1 / n7).log10();
            }
            Some((f2, l2)) => {
                let (fa, fb) = ((f1 - fc).abs(), (f2 - fc).abs());
                let (a1, a2) = (fa.min(fb), fa.max(fb));
                let pref = l1.min(l2);
                r.tone1 = a1;
                r.tone2 = Some(a2);
                r.diff = Some(10.0 * (l1.max(l2) / pref).log10());
                r.carrier = carrier(pref);
                r.opposite = Some(level(fc - s * a1, pref));
                for (i, m) in [(2.0, 1.0), (3.0, 2.0), (4.0, 3.0)].into_iter().enumerate() {
                    let pair = [m.0 * a1 - m.1 * a2, m.0 * a2 - m.1 * a1];
                    r.imd[i] = pair
                        .into_iter()
                        .filter(|a| (PRODUCT_LOW_HZ..=AUDIO_TOP_HZ).contains(a))
                        .map(|a| level(fc + s * a, pref))
                        .min_by(|x, y| x.0.total_cmp(&y.0));
                }
                r.floor = 10.0 * (pref / n7).log10();
            }
        }
        Some(r)
    }

    fn row(&self, r: &Option<Reading>, kind: SsbTestKind) -> SsbTestEventRow {
        let v = self.values(r, kind);
        SsbTestEventRow {
            time_s: Seconds(self.t),
            kind: v.kind,
            sideband: v.sideband,
            tone1_hz: v.tone1_hz,
            tone2_hz: v.tone2_hz,
            tone_level_diff_db: v.tone_level_diff_db,
            carrier_db: v.carrier_db,
            carrier_under: v.carrier_under,
            carrier_found: v.carrier_found,
            opposite_db: v.opposite_db,
            opposite_under: v.opposite_under,
            h2_db: v.h2_db,
            h3_db: v.h3_db,
            h4_db: v.h4_db,
            h5_db: v.h5_db,
            h2_under: v.h2_under,
            h3_under: v.h3_under,
            h4_under: v.h4_under,
            h5_under: v.h5_under,
            imd3_db: v.imd3_db,
            imd5_db: v.imd5_db,
            imd7_db: v.imd7_db,
            imd3_under: v.imd3_under,
            imd5_under: v.imd5_under,
            imd7_under: v.imd7_under,
            imd3_pep_db: v.imd3_pep_db,
            imd5_pep_db: v.imd5_pep_db,
            imd7_pep_db: v.imd7_pep_db,
            floor_db: v.floor_db,
        }
    }

    fn values(&self, r: &Option<Reading>, kind: SsbTestKind) -> SsbTestFields {
        let db = |l: Level| l.map(|v| v.0);
        let un = |l: Level| l.map(|v| v.1);
        let h = |i: usize| r.and_then(|r| r.h[i]);
        let imd = |i: usize| r.and_then(|r| r.imd[i]);
        let pep = |i: usize| imd(i).map(|v| v.0 + self.pep);
        SsbTestFields {
            kind,
            sideband: r.map(|r| {
                if r.upper {
                    SsbSideband::Upper
                } else {
                    SsbSideband::Lower
                }
            }),
            tone1_hz: r.map(|r| Hertz(r.tone1)),
            tone2_hz: r.and_then(|r| r.tone2).map(Hertz),
            tone_level_diff_db: r.and_then(|r| r.diff),
            carrier_db: r.map(|r| r.carrier.0),
            carrier_under: r.map(|r| r.carrier.1),
            carrier_found: r.map(|r| r.carrier_found),
            opposite_db: db(r.and_then(|r| r.opposite)),
            opposite_under: un(r.and_then(|r| r.opposite)),
            h2_db: db(h(0)),
            h3_db: db(h(1)),
            h4_db: db(h(2)),
            h5_db: db(h(3)),
            h2_under: un(h(0)),
            h3_under: un(h(1)),
            h4_under: un(h(2)),
            h5_under: un(h(3)),
            imd3_db: db(imd(0)),
            imd5_db: db(imd(1)),
            imd7_db: db(imd(2)),
            imd3_under: un(imd(0)),
            imd5_under: un(imd(1)),
            imd7_under: un(imd(2)),
            imd3_pep_db: pep(0),
            imd5_pep_db: pep(1),
            imd7_pep_db: pep(2),
            floor_db: r.map(|r| r.floor),
        }
    }

    fn window(&mut self, c: &mut Ctx) {
        let Some(fft) = self.fft.as_mut() else { return };
        for ((b, &s), &w) in self.buf.iter_mut().zip(&self.x).zip(&self.window) {
            *b = s * w;
        }
        fft.process(&mut self.buf);
        for (p, b) in self.p.iter_mut().zip(&self.buf) {
            *p = f64::from(b.norm_sqr());
        }
        self.t += WINDOW as f64 / self.rate;
        self.windows += 1;
        let r = self.measure();
        let kind = match r {
            None => SsbTestKind::NoTest,
            Some(Reading { tone2: None, .. }) => SsbTestKind::OneTone,
            Some(_) => SsbTestKind::TwoTones,
        };
        if r.is_some() {
            c.tally.good(self.t);
            if self.last_kind != Some(kind) || self.windows % 4 == 1 {
                let row = self.row(&r, kind);
                self.log.push(row);
            }
        } else {
            c.tally.energy(self.t);
        }
        self.last_kind = Some(kind);
        self.last = r;
    }
}

impl TxTest for SsbTest {
    const ID: &'static str = ID;
    const NAME: &'static str = "ssb_test.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let front = Front::new(rate_hz, WORK_HZ, PASS_HZ, SKIP_S);
        self.rate = front.rate();
        self.front = Some(front);
        self.fft = Some(Fft::forward(WINDOW));
        self.window = Window::BlackmanHarris4.coefficients(WINDOW);
        self.buf = vec![Complex32::default(); WINDOW];
        self.p = vec![0.0; WINDOW];
        self.x.clear();
        self.lo = v.get(&TONE_MIN_HZ);
        self.hi = v.get(&TONE_MAX_HZ).max(self.lo);
        self.two_db = v.get(&TWO_TONE_DB);
        self.pep = v.get(&PEP_OFFSET_DB);
        self.t = self.t.max(SKIP_S);
        self.last = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let Some(mut front) = self.front.take() else {
            return;
        };
        for &z in front.process(x) {
            self.x.push(z);
            if self.x.len() == WINDOW {
                self.window(c);
                self.x.clear();
            }
        }
        self.front = Some(front);
    }
    fn clear(&mut self) {
        self.windows = 0;
        self.last_kind = None;
        self.last = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&SSB_TEST_EVENTS, p);
        let kind = self.last_kind.unwrap_or(SsbTestKind::NoTest);
        let r = self.last;
        p.put(&self.values(&r, kind));
        match r {
            Some(Reading {
                tone1,
                tone2: Some(t2),
                imd,
                ..
            }) => {
                let i3 = imd[0].map_or("none".to_string(), |v| format!("{:.1} dB", v.0));
                format!("two tones {tone1:.0} and {t2:.0} Hz, IMD3 {i3} under one tone")
            }
            Some(Reading {
                tone1,
                carrier: (db, under),
                ..
            }) => {
                if under {
                    format!("one tone {tone1:.0} Hz, no carrier seen, more than {db:.1} dB under")
                } else {
                    format!("one tone {tone1:.0} Hz, carrier {db:.1} dB under")
                }
            }
            None => "no clean test tone".to_string(),
        }
    }
}

const KEYS: [&str; 4] = ["tone_min_hz", "tone_max_hz", "two_tone_db", "pep_offset_db"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("one tone at 1000 Hz, carrier 50 dB under", o, 0.0);
    c.seconds = o.seconds.max(8.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = crate::instrument::number(r, SsbTestFields::TONE1_HZ.key);
        vec![CheckLine::new(
            SsbTestFields::TONE1_HZ.key,
            (f - 1000.0).abs() < 2.0,
            format!("{f:.1} Hz, 1000 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: SsbTest,
    settings: SsbTestSetting,
    keys: KEYS,
    name: "SSB transmitter test meter",
    blurb: "One tone: carrier leak, opposite sideband and harmonics; two tones: IMD3, IMD5 and IMD7 under one tone and PEP.",
    tuning: (0.1e6, 500e6, 48e3, 4e3, false),
    rate_text: "The SSB test meter needs at least 12 ksps",
    generator: |cfg| gen::SsbGenerator::new(cfg.rate_hz, &cfg.settings),
    verify: verify_cases
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fields of one window of flat noise with a tone at 1000 Hz and a carrier at the dial whose 7-bin sum is
    /// `ratio` times that of the noise.
    fn carrier_at(ratio: f64) -> SsbTestFields {
        let mut m = SsbTest::default();
        m.start(WORK_HZ, &Values::defaults(&DecoderSetting::schema()));
        // With every bin at 1 the measure takes the noise of a bin as 1 / ln 2.
        m.p.fill(1.0);
        let noise = 1.0 / std::f64::consts::LN_2;
        let k = m.bin(1000.0);
        m.p[k] = 1e6 * noise;
        m.p[0] = ratio * 7.0 * noise - 6.0;
        let r = m.measure();
        m.values(&r, SsbTestKind::OneTone)
    }

    #[test]
    fn a_carrier_not_found_reads_as_the_floor_of_its_search() {
        let f = carrier_at(3.0);
        assert_eq!(f.carrier_found, Some(false), "{f:?}");
        // The tone over 10 times the 7-bin sum of the noise: 1e6 / 70.
        let floor = 10.0 * (1e6f64 / 70.0).log10();
        assert!(
            f.carrier_db.is_some_and(|v| (v - floor).abs() < 0.1),
            "{f:?}"
        );
        assert_eq!(f.carrier_under, Some(true), "{f:?}");
        assert!(
            f.tone1_hz.is_some_and(|t| (t.0 - 1000.0).abs() < 1.0),
            "{f:?}"
        );
        let f = carrier_at(20.0);
        assert_eq!(f.carrier_found, Some(true), "{f:?}");
        // The tone over the 7-bin sum of the carrier: 1e6 / 140.
        let want = 10.0 * (1e6f64 / 140.0).log10();
        assert!(
            f.carrier_db.is_some_and(|v| (v - want).abs() < 0.1),
            "{f:?}"
        );
        assert_eq!(f.carrier_under, Some(false), "{f:?}");
    }
}
