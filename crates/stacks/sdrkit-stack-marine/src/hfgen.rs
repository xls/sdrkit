//! The marine HF and VHF test signal: NAVTEX, MF/HF DSC, HF weather fax of a synthetic chart, and VHF channel 70 DSC, as clean
//! complex baseband at unit power while it sends. The engine adds noise, offset, clock error and fading. Nothing is transmitted.

use crate::dsc::call::{self, Outgoing};
use crate::fax;
use crate::navtex;
use sdrkit_algo_sim::modulate::{Afsk, Cpm, Fm};
use sdrkit_algo_sim::SimError;
use sdrkit_codec_image::font::{draw_text, FIXED_5X7};
use sdrkit_codec_image::{PixelFormat, Raster};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};
use std::sync::Arc;

/// One piece of the programme.
#[derive(Clone, Debug)]
pub enum Segment {
    /// FSK of `bits` at `baud`, `shift` Hz between the tones, centred on the dial. A 1 is the higher tone when `one_high`.
    Fsk {
        /// The bits.
        bits: Arc<Vec<u8>>,
        /// Symbols a second.
        baud: f64,
        /// Hz between the tones.
        shift: f64,
        /// A 1 is the higher tone.
        one_high: bool,
    },
    /// A plain tone `hz` above the dial (USB), for `secs`.
    Tone {
        /// Hz above the dial.
        hz: f64,
        /// Seconds.
        secs: f64,
    },
    /// Fax phasing lines: black with a white pulse 5 % of the line wide centred on the line boundary.
    Phasing {
        /// Lines.
        lines: usize,
        /// Lines a minute.
        lpm: f64,
    },
    /// Fax picture rows (0 black to 255 white).
    Rows {
        /// The rows, all of one width.
        rows: Arc<Vec<Vec<u8>>>,
        /// Lines a minute.
        lpm: f64,
    },
    /// No carrier, for `secs`.
    Silence {
        /// Seconds.
        secs: f64,
    },
    /// FM with AFSK tones (1300 Hz for a 1, 2100 Hz for a 0) at `baud`, peak deviation `dev` Hz.
    Afsk {
        /// The bits.
        bits: Arc<Vec<u8>>,
        /// Symbols a second.
        baud: f64,
        /// Peak deviation, Hz.
        dev: f64,
    },
    /// An unmodulated carrier, for `secs`.
    Carrier {
        /// Seconds.
        secs: f64,
    },
}

impl Segment {
    fn secs(&self) -> f64 {
        match self {
            Segment::Fsk { bits, baud, .. } | Segment::Afsk { bits, baud, .. } => {
                bits.len() as f64 / baud
            }
            Segment::Tone { secs, .. } | Segment::Silence { secs } | Segment::Carrier { secs } => {
                *secs
            }
            Segment::Phasing { lines, lpm } => *lines as f64 * 60.0 / lpm,
            Segment::Rows { rows, lpm } => rows.len() as f64 * 60.0 / lpm,
        }
    }
}

/// Options of the generator.
#[derive(Clone, Debug)]
pub struct HfOptions {
    /// Samples a second.
    pub rate: f64,
    /// Play the programme once (false loops it).
    pub once: bool,
    /// What the signal is, for the engine's impairments.
    pub info: SignalInfo,
}

impl Default for HfOptions {
    fn default() -> Self {
        HfOptions {
            rate: 12e3,
            once: false,
            info: SignalInfo::new(fax::SNR_BANDWIDTH_HZ),
        }
    }
}

/// Bits rendered at a time.
const BITS_PER_STEP: usize = 64;
/// Samples rendered at a time for the tone and fax segments.
const SAMPLES_PER_STEP: usize = 4096;

/// The modulator of the segment being played.
enum Playing {
    Fsk {
        cpm: Cpm,
        next: usize,
    },
    Afsk {
        afsk: Afsk,
        next: usize,
    },
    /// Samples given by an instantaneous frequency (tones, fax), or silence.
    Wave {
        n: u64,
        len: u64,
    },
    Done,
}

/// The streaming generator of a programme of segments.
pub struct HfGenerator {
    prog: Vec<Segment>,
    o: HfOptions,
    seg: usize,
    playing: Playing,
    /// FM by an instantaneous frequency in hertz (1 Hz per unit), phase continuous over the whole programme.
    fm: Fm,
    /// FM of the VHF AFSK audio.
    vhf: Fm,
    buf: Vec<Complex32>,
    audio: Vec<f32>,
    levels: Vec<f32>,
    pos: usize,
}

impl HfGenerator {
    /// A generator of `prog` with these options. An error for a rate that cannot carry the programme (below 8 kHz, or below twice
    /// the FM deviation of a VHF segment).
    pub fn new(prog: Vec<Segment>, o: HfOptions) -> Result<Self, SimError> {
        if !(o.rate >= 8000.0 && o.rate.is_finite()) {
            return Err(SimError::OutOfRange {
                param: sdrkit_algo_sim::Param::Rate,
                min: 8000.0,
                max: f64::INFINITY,
                got: o.rate,
            });
        }
        let dev = prog
            .iter()
            .map(|s| match s {
                Segment::Afsk { dev, .. } => *dev,
                _ => 0.0,
            })
            .fold(1.0, f64::max);
        let mut g = HfGenerator {
            fm: Fm::new(o.rate, 1.0)?,
            vhf: Fm::new(o.rate, dev)?,
            prog,
            o,
            seg: 0,
            playing: Playing::Done,
            buf: Vec::new(),
            audio: Vec::new(),
            levels: Vec::new(),
            pos: 0,
        };
        g.start(0)?;
        Ok(g)
    }

    /// The programme's length, seconds of signal.
    pub fn duration(&self) -> f64 {
        self.prog.iter().map(Segment::secs).sum()
    }

    fn start(&mut self, seg: usize) -> Result<(), SimError> {
        self.seg = seg;
        let rate = self.o.rate;
        self.playing = match self.prog.get(seg) {
            None => Playing::Done,
            Some(Segment::Fsk { baud, shift, .. }) => Playing::Fsk {
                cpm: Cpm::cpfsk(shift / baud, rate / baud)?,
                next: 0,
            },
            Some(Segment::Afsk { baud, .. }) => Playing::Afsk {
                afsk: Afsk::new(rate, *baud, 1300.0, 2100.0)?,
                next: 0,
            },
            Some(s) => Playing::Wave {
                n: 0,
                len: (s.secs() * rate).round() as u64,
            },
        };
        Ok(())
    }

    /// The instantaneous frequency (Hz above the dial) of a tone or fax segment at `t` seconds into it, or `None` for no carrier.
    fn wave_hz(seg: &Segment, t: f64) -> Option<f64> {
        match seg {
            Segment::Tone { hz, .. } => Some(*hz),
            Segment::Carrier { .. } => Some(0.0),
            Segment::Phasing { lpm, .. } => {
                let x = (t * lpm / 60.0).fract();
                let white = !(0.025..0.975).contains(&x);
                Some(if white { fax::WHITE_HZ } else { fax::BLACK_HZ })
            }
            Segment::Rows { rows, lpm } => {
                let lines = t * lpm / 60.0;
                let row = &rows[(lines as usize).min(rows.len().saturating_sub(1))];
                let x =
                    ((lines.fract() * row.len() as f64) as usize).min(row.len().saturating_sub(1));
                let v = f64::from(row.get(x).copied().unwrap_or(255));
                Some(fax::BLACK_HZ + (fax::WHITE_HZ - fax::BLACK_HZ) * v / 255.0)
            }
            _ => None,
        }
    }

    /// Renders the next part of the programme into `buf`. Returns false at the end.
    fn render(&mut self) -> bool {
        self.buf.clear();
        self.pos = 0;
        loop {
            let Some(seg) = self.prog.get(self.seg) else {
                if self.o.once || self.prog.is_empty() || self.start(0).is_err() {
                    return false;
                }
                continue;
            };
            let rate = self.o.rate;
            let finished = match (&mut self.playing, seg) {
                (Playing::Fsk { cpm, next }, Segment::Fsk { bits, one_high, .. }) => {
                    if *next >= bits.len() {
                        cpm.flush(&mut self.buf);
                        true
                    } else {
                        let end = (*next + BITS_PER_STEP).min(bits.len());
                        self.levels.clear();
                        self.levels.extend(bits[*next..end].iter().map(|&b| {
                            if (b != 0) == *one_high {
                                1.0
                            } else {
                                -1.0
                            }
                        }));
                        cpm.push(&self.levels, &mut self.buf);
                        *next = end;
                        false
                    }
                }
                (Playing::Afsk { afsk, next }, Segment::Afsk { bits, .. }) => {
                    self.audio.clear();
                    let done = *next >= bits.len();
                    if done {
                        afsk.flush(&mut self.audio);
                    } else {
                        let end = (*next + BITS_PER_STEP).min(bits.len());
                        afsk.push(&bits[*next..end], &mut self.audio);
                        *next = end;
                    }
                    let start = self.buf.len();
                    self.buf
                        .resize(start + self.audio.len(), Complex32::default());
                    if self
                        .vhf
                        .process(&self.audio, &mut self.buf[start..])
                        .is_err()
                    {
                        return false;
                    }
                    done
                }
                (Playing::Wave { n, len }, s) => {
                    let k = (*len - *n).min(SAMPLES_PER_STEP as u64) as usize;
                    self.audio.clear();
                    let mut on = true;
                    for i in 0..k {
                        let t = (*n + i as u64) as f64 / rate;
                        match Self::wave_hz(s, t) {
                            Some(f) => self.audio.push(f as f32),
                            None => {
                                on = false;
                                self.audio.push(0.0);
                            }
                        }
                    }
                    *n += k as u64;
                    let start = self.buf.len();
                    self.buf.resize(start + k, Complex32::default());
                    if on
                        && self
                            .fm
                            .process(&self.audio, &mut self.buf[start..])
                            .is_err()
                    {
                        return false;
                    }
                    *n >= *len
                }
                _ => true,
            };
            if finished {
                let next = self.seg + 1;
                if self.start(next).is_err() {
                    return false;
                }
            }
            if !self.buf.is_empty() {
                return true;
            }
        }
    }
}

impl Generator for HfGenerator {
    /// The station is made at 0 Hz; the framework places it on the tuned frequency.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut written = 0;
        while written < out.len() {
            if self.pos >= self.buf.len() && !self.render() {
                break;
            }
            let n = (self.buf.len() - self.pos).min(out.len() - written);
            out[written..written + n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            written += n;
        }
        written
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.o.info)
    }
}

/// What a NAVTEX transmission is: 304 Hz necessary bandwidth (RR Appendix 1, F1B at 100 Bd and 170 Hz shift), 100 Bd, and five
/// information bits in every fourteen sent.
pub fn navtex_info() -> SignalInfo {
    SignalInfo::new(304.0)
        .with_symbol_rate(100.0)
        .with_bit_rate(100.0 * 5.0 / 14.0)
}

/// What an MF/HF DSC call is: 304 Hz necessary bandwidth, 100 Bd, and seven information bits in every twenty sent.
pub fn dsc_info() -> SignalInfo {
    SignalInfo::new(304.0)
        .with_symbol_rate(100.0)
        .with_bit_rate(100.0 * 7.0 / 20.0)
}

/// What a VHF DSC call is: 16 kHz (emission 16K0G2B), 1200 Bd, and seven information bits in every twenty sent.
pub fn dsc_vhf_info() -> SignalInfo {
    SignalInfo::new(16_000.0)
        .with_symbol_rate(1200.0)
        .with_bit_rate(1200.0 * 7.0 / 20.0)
}

/// What a weather fax is: the 3 kHz voice channel it is sent in, no symbols.
pub fn fax_info() -> SignalInfo {
    SignalInfo::new(fax::SNR_BANDWIDTH_HZ)
}

/// A NAVTEX programme: each message after `idle_s` of phasing.
pub fn navtex_programme(messages: &[String], idle_s: f64) -> Vec<Segment> {
    messages
        .iter()
        .map(|m| {
            let words = navtex::message_words(m, idle_s);
            let mut bits = Vec::with_capacity(words.len() * 7);
            for w in words {
                bits.extend_from_slice(&w.bits());
            }
            // Y (1) is the lower tone.
            Segment::Fsk {
                bits: Arc::new(bits),
                baud: 100.0,
                shift: 170.0,
                one_high: false,
            }
        })
        .collect()
}

/// A DSC MF/HF programme: each call with `gap_s` of silence after.
pub fn dsc_programme(calls: &[Outgoing], gap_s: f64) -> Vec<Segment> {
    let mut v = Vec::new();
    for c in calls {
        let mut bits = c.bits();
        bits.extend((0..10).map(|i| (i % 2) as u8));
        v.push(Segment::Fsk {
            bits: Arc::new(bits),
            baud: 100.0,
            shift: 170.0,
            one_high: false,
        });
        v.push(Segment::Silence { secs: gap_s });
    }
    v
}

/// A VHF channel 70 programme (FM, AFSK 1300 / 2100 Hz, 1200 Bd).
pub fn dsc_vhf_programme(calls: &[Outgoing], gap_s: f64) -> Vec<Segment> {
    let mut v = Vec::new();
    for c in calls {
        v.push(Segment::Carrier { secs: 0.05 });
        let mut bits = c.bits();
        bits.extend((0..10).map(|i| (i % 2) as u8));
        v.push(Segment::Afsk {
            bits: Arc::new(bits),
            baud: 1200.0,
            dev: 3000.0,
        });
        v.push(Segment::Silence { secs: gap_s });
    }
    v
}

/// A weather fax programme: start tone 5 s, phasing, one white line, the picture, stop tone 5 s, black 10 s, then a pause.
pub fn fax_programme(
    rows: Vec<Vec<u8>>,
    lpm: f64,
    ioc: u32,
    phasing_s: f64,
    pause_s: f64,
) -> Vec<Segment> {
    let w = rows.first().map_or(fax::width_for_ioc(ioc), |r| r.len());
    let mut all = vec![vec![255u8; w]];
    all.extend(rows);
    let p = 60.0 / lpm;
    vec![
        Segment::Tone {
            hz: if ioc == 288 { 675.0 } else { 300.0 },
            secs: 5.0,
        },
        Segment::Phasing {
            lines: (phasing_s / p).round() as usize,
            lpm,
        },
        Segment::Rows {
            rows: Arc::new(all),
            lpm,
        },
        Segment::Tone {
            hz: 450.0,
            secs: 5.0,
        },
        Segment::Tone {
            hz: fax::BLACK_HZ,
            secs: 10.0,
        },
        Segment::Silence { secs: pause_s },
    ]
}

/// The synthetic chart, the same for the same size: a white margin and a frame, land, a grid, isobars every 4 hPa around a low and
/// a high with H and L at the centres, a title in the 5 x 7 font, and a grey wedge of eight flat steps from black to white.
pub fn chart(width: usize, height: usize) -> Vec<Vec<u8>> {
    let (w32, h32) = (width as u32, height as u32);
    let Ok(mut r) = Raster::new(PixelFormat::Grey8, w32, h32) else {
        return vec![vec![255u8; width]; height];
    };
    r.data_mut().fill(255);
    let m = width / 100;
    let put = |r: &mut Raster, x: usize, y: usize, v: u8| r.put(x as i64, y as i64, &[v]);
    // Frame: dark columns about 1 % of the width from each edge, and rows near the top and bottom.
    for y in 0..height {
        for x in m..m + 3 {
            put(&mut r, x, y, 0);
            put(&mut r, width - 1 - x, y, 0);
        }
    }
    for y in [m.min(height - 1), height.saturating_sub(m + 1)] {
        for x in m..width - m {
            put(&mut r, x, y, 0);
        }
    }
    let w = width;
    let at = |r: &Raster, x: usize, y: usize| r.data()[y * w + x];
    // Land: a smooth blob at the left, light grey.
    for y in 0..height {
        let edge = (width as f64 * (0.18 + 0.05 * ((y as f64) / 37.0).sin())) as usize;
        for x in (m + 3)..edge.min(width) {
            if at(&r, x, y) == 255 {
                put(&mut r, x, y, 200);
            }
        }
    }
    // Grid every 150 pixels.
    for y in 0..height {
        for x in 0..width {
            if (x % 150 == 0 || y % 150 == 0)
                && x > m + 3
                && x < width - m - 3
                && at(&r, x, y) > 100
            {
                put(&mut r, x, y, 128);
            }
        }
    }
    // Isobars: rings around a low and a high.
    let centres = [
        (width as f64 * 0.4, height as f64 * 0.5, "L"),
        (width as f64 * 0.75, height as f64 * 0.4, "H"),
    ];
    for (cx, cy, _) in centres {
        for y in 0..height {
            for x in 0..width {
                let d = ((x as f64 - cx).powi(2) + ((y as f64 - cy) * 3.0).powi(2)).sqrt();
                if d > 30.0 && (d % 60.0) < 2.5 && x > m + 3 && x < width - m - 3 {
                    put(&mut r, x, y, 0);
                }
            }
        }
    }
    for (cx, cy, c) in centres {
        if (cy as usize) + 21 < height {
            draw_text(&mut r, &FIXED_5X7, c, cx as i64, cy as i64 - 10, 3, &[0]);
        }
    }
    // Title.
    let x0 = width as i64 - 400;
    draw_text(
        &mut r,
        &FIXED_5X7,
        "SDRKIT FAX 1200",
        x0,
        3.min(height as i64 - 15),
        2,
        &[0],
    );
    // The grey wedge: eight flat steps from black to white, at the bottom right.
    let wedge_w = 40;
    let y0 = height.saturating_sub(12);
    for y in y0..height.saturating_sub(2) {
        for k in 0..8 {
            let v = (255 * k / 7) as u8;
            for x in width - 400 + k * wedge_w..width - 400 + (k + 1) * wedge_w {
                put(&mut r, x, y, v);
            }
        }
    }
    r.data().chunks_exact(width).map(<[u8]>::to_vec).collect()
}

/// A generator that produces no samples, for a programme the rate cannot carry.
struct Nothing(f64);

impl Generator for Nothing {
    fn rate_hz(&self) -> f64 {
        self.0
    }
    fn generate(&mut self, _: &mut [Complex32]) -> usize {
        0
    }
}

sdrkit_core::choice_enum! {
    /// What the marine HF test signal sends.
    pub enum GenService {
        /// NAVTEX messages.
        Navtex = ("navtex", "NAVTEX"),
        /// MF/HF DSC calls.
        Dsc = ("dsc", "DSC"),
        /// A weather fax chart.
        Wefax = ("wefax", "Weather fax"),
        /// VHF channel 70 DSC calls.
        DscVhf = ("dsc_vhf", "DSC on VHF"),
    }
}

const GEN_SERVICE: Setting<field::Choice<GenService>> =
    Setting::choice("gen_service", "Test signal", GenService::Navtex)
        .help("What the test signal sends when no submode names it.")
        .section("Test signal")
        .advanced();
const GEN_IDLE_S: Setting<field::Duration> = Setting::new("gen_idle_s", "NAVTEX phasing", "s", 6.0)
    .range(0.0, 60.0, 0.0)
    .section("Test signal")
    .advanced();
const GEN_IOC: Setting<field::I64> = Setting::new("gen_ioc", "Fax IOC", "", 576)
    .range(100.0, 1000.0, 1.0)
    .section("Test signal")
    .advanced();
const GEN_LINES: Setting<field::I64> = Setting::new("gen_lines", "Fax lines", "", 800)
    .range(1.0, fax::MAX_LINES as f64, 1.0)
    .section("Test signal")
    .advanced();
const GEN_LPM: Setting<field::F64> = Setting::new("gen_lpm", "Fax lines per minute", "", 120.0)
    .range(30.0, 480.0, 0.0)
    .section("Test signal")
    .advanced();
const GEN_PHASING_S: Setting<field::Duration> =
    Setting::new("gen_phasing_s", "Fax phasing", "s", 10.0)
        .range(0.0, 60.0, 0.0)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the marine HF stack.
    pub enum MarineGenSetting {
        /// What the test signal sends.
        Service = GEN_SERVICE,
        /// The NAVTEX phasing before each message.
        IdleS = GEN_IDLE_S,
        /// The fax index of cooperation.
        Ioc = GEN_IOC,
        /// The fax lines.
        Lines = GEN_LINES,
        /// The fax lines per minute.
        Lpm = GEN_LPM,
        /// The fax phasing.
        PhasingS = GEN_PHASING_S,
    }
}

/// The generator for the `marine` stack: the service from the submode (navtex, dsc, wefax), else the `gen_service` setting, with
/// the other test-signal settings. A rate below 8 kHz produces no samples.
pub fn from_config(cfg: &GenConfig) -> Box<dyn Generator> {
    let s = &cfg.settings;
    let service = match cfg.submode.as_deref() {
        Some("dsc") => GenService::Dsc,
        Some("wefax") => GenService::Wefax,
        Some("navtex") => GenService::Navtex,
        _ => s.get(&GEN_SERVICE),
    };
    let (prog, info) = match service {
        GenService::Dsc => (dsc_programme(&call::test_calls(false), 2.0), dsc_info()),
        GenService::DscVhf => (
            dsc_vhf_programme(&call::test_calls(true), 0.3),
            dsc_vhf_info(),
        ),
        GenService::Wefax => {
            let ioc = s.get(&GEN_IOC) as u32;
            let lines = s.get(&GEN_LINES) as usize;
            let prog = fax_programme(
                chart(fax::width_for_ioc(ioc), lines),
                s.get(&GEN_LPM),
                ioc,
                s.get(&GEN_PHASING_S),
                10.0,
            );
            (prog, fax_info())
        }
        GenService::Navtex => (
            navtex_programme(&navtex::demo_messages(), s.get(&GEN_IDLE_S)),
            navtex_info(),
        ),
    };
    let o = HfOptions {
        rate: cfg.rate_hz,
        once: false,
        info,
    };
    match HfGenerator::new(prog, o) {
        Ok(g) => Box::new(g),
        Err(_) => Box::new(Nothing(cfg.rate_hz)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signal_has_unit_power_and_the_programme_length() {
        let rate = 12e3;
        let prog = dsc_programme(&call::test_calls(false)[..1], 0.0);
        let secs: f64 = prog.iter().map(Segment::secs).sum();
        let mut g = HfGenerator::new(
            prog,
            HfOptions {
                rate,
                once: true,
                info: dsc_info(),
            },
        )
        .unwrap();
        let mut x = vec![Complex32::default(); (secs * rate) as usize + 1000];
        let n = g.generate(&mut x);
        assert!((n as f64 - secs * rate).abs() <= 2.0, "{n}");
        let p = x[..n].iter().map(|v| v.norm_sqr()).sum::<f32>() / n as f32;
        assert!((p - 1.0).abs() < 1e-3, "{p}");
        assert_eq!(g.generate(&mut x), 0);
        assert!(HfGenerator::new(
            Vec::new(),
            HfOptions {
                rate: 100.0,
                ..Default::default()
            }
        )
        .is_err());
    }

    #[test]
    fn the_chart_has_its_frame_and_wedge() {
        let c = chart(1809, 60);
        assert_eq!((c.len(), c[0].len()), (60, 1809));
        assert_eq!(c[30][18], 0);
        assert_eq!(c[50][1809 - 400], 0);
        assert_eq!(c[50][1809 - 41], 255);
    }
}
