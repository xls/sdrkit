//! Hellschreiber: Feld Hell, Slow Hell, Hell 80, HellX5, HellX9 and the FSK Hell variants. The receiver draws what it hears:
//! the tone's envelope (or frequency) per pixel, cut into 14-pixel columns at the boundary the blank rows show, behind a level
//! gate. The generator draws text with the shared Hell font, or a TrueType font the host supplies.
//!
//! Standard: the Feld Hell Club's mode descriptions (7 x 7 font, each font pixel twice in height, 17.5 columns a second).

use crate::decoder::{self, flush_logs, put_text, Emit, HfGenerator, Modulator, GEN_MESSAGE};
use crate::finder::CentreFinder;
use crate::kinds::HellColumn;
use crate::mfsk::{CENTRE, FIND_SIGNAL};
use crate::picture::HfPicture;
use crate::typed::HellFields;
use crate::util::Tone;
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_codec_image::font::{self, ttf::TrueTypeFont, Glyph};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::Published;
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::{
    Choice as _, ChoiceDecl, Complex32, ConfigError, GenConfig, Generator, LockState, Schema,
    SettingError, SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Outbox, Port, RateFrom, Timed};
use std::collections::VecDeque;
use std::sync::Arc;

/// A Hell variant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HellVariant {
    /// Its name.
    pub name: &'static str,
    /// Columns per second.
    pub columns: f64,
    /// FSK deviation, Hz (0 for on/off keying).
    pub fsk: f64,
    /// Raised-cosine edges, seconds (0 for square keying).
    pub edge: f64,
}

impl HellVariant {
    /// Pixels per second.
    pub fn pixel_rate(&self) -> f64 {
        14.0 * self.columns
    }
}

/// Every variant.
pub const VARIANTS: [HellVariant; 7] = [
    HellVariant {
        name: "Feld Hell",
        columns: 17.5,
        fsk: 0.0,
        edge: 0.004,
    },
    HellVariant {
        name: "Slow Hell",
        columns: 2.1875,
        fsk: 0.0,
        edge: 0.004,
    },
    HellVariant {
        name: "Hell 80",
        columns: 35.0,
        fsk: 150.0,
        edge: 0.0,
    },
    HellVariant {
        name: "HellX5",
        columns: 87.5,
        fsk: 0.0,
        edge: 0.0,
    },
    HellVariant {
        name: "HellX9",
        columns: 157.5,
        fsk: 0.0,
        edge: 0.0,
    },
    HellVariant {
        name: "FSK Hell 245",
        columns: 17.5,
        fsk: 61.25,
        edge: 0.0,
    },
    HellVariant {
        name: "FSK Hell 105",
        columns: 17.5,
        fsk: 27.5,
        edge: 0.0,
    },
];

/// A variant by name.
pub fn variant(name: &str) -> Option<HellVariant> {
    VARIANTS
        .iter()
        .copied()
        .find(|v| v.name.eq_ignore_ascii_case(name))
}

/// The `variant` setting: an entry of [`VARIANTS`].
pub const VARIANT_SETTING: CatalogueSetting = CatalogueSetting {
    key: "variant",
    label: "Variant",
    help: "",
    choices: || {
        VARIANTS
            .iter()
            .map(|v| ChoiceDecl::new(v.name, v.name))
            .collect()
    },
    default: "Feld Hell",
    advanced: false,
    role: None,
};

sdrkit_core::settings! {
    /// The settings of [`HellDecoder`].
    pub enum HellSetting {
        /// The variant.
        Variant = VARIANT_SETTING,
        /// The audio centre.
        Centre = CENTRE,
        /// The centre follows the signal.
        FindSignal = FIND_SIGNAL,
    }
}

sdrkit_core::settings! {
    /// The test-signal settings of the Hell stack.
    pub enum HellGenSetting {
        /// The text drawn.
        Message = GEN_MESSAGE,
    }
}

/// The columns of a text: each character's columns (14-pixel masks, bit 0 = the bottom pixel) and a blank column. A space is three
/// blank columns.
pub fn text_columns(text: &str, font: Option<&TrueTypeFont>) -> Vec<u16> {
    let mut out = vec![];
    for c in text.chars() {
        if c == ' ' || c == '\n' {
            out.extend([0, 0, 0]);
            continue;
        }
        if let Some(g) = glyph_pixels(c, font) {
            out.extend(g);
            out.push(0);
        }
    }
    out
}

/// A character's columns as 14-pixel masks (bit 0 the bottom pixel): the shared Hell glyph with each font pixel twice in
/// height, else the glyph the user font draws 14 pixels high, else `?`.
pub fn glyph_pixels(c: char, font: Option<&TrueTypeFont>) -> Option<Vec<u16>> {
    // A glyph shorter than the 7-row cell is centred in it (the 5-row font has a blank row above and below).
    let builtin = |g: Glyph| {
        let lift = 7u8.saturating_sub(g.height) / 2;
        (0..g.width)
            .map(|x| {
                let col = (g.column(x) as u16) << lift;
                let mut m = 0u16;
                for r in 0..7 {
                    if (col >> r) & 1 == 1 {
                        m |= 3 << (2 * r);
                    }
                }
                m
            })
            .collect::<Vec<u16>>()
    };
    if let Some(g) = font::hell::glyph(c) {
        return Some(builtin(g));
    }
    if let Some(m) = font.and_then(|f| f.render(c, 14)) {
        let cols = (0..m.width)
            .map(|x| {
                let mut v = 0u16;
                for k in 0..m.height.min(14) {
                    let y = m.height - 1 - k;
                    if m.data[(y * m.width + x) as usize] > 127 {
                        v |= 1 << k;
                    }
                }
                v
            })
            .collect();
        return Some(cols);
    }
    font::hell::glyph('?').map(builtin)
}

/// The Hell transmitter.
pub struct HellModulator {
    v: HellVariant,
    rate: f64,
    centre: f64,
    cols: Vec<u16>,
    t: f64,
    osc: Tone,
    env: f64,
}

impl HellModulator {
    /// Sends `text` repeated.
    pub fn new(
        v: HellVariant,
        rate: f64,
        centre: f64,
        text: &str,
        font: Option<&TrueTypeFont>,
    ) -> Self {
        HellModulator {
            v,
            rate,
            centre,
            cols: text_columns(text, font),
            t: 0.0,
            osc: Tone::new(rate),
            env: 0.0,
        }
    }
    fn pixel(&self, t: f64) -> bool {
        let p = (t * self.v.pixel_rate()).floor() as usize;
        let col = self.cols[(p / 14) % self.cols.len()];
        (col >> (p % 14)) & 1 == 1
    }
}

impl Modulator for HellModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        for o in out {
            // The keying edges are centred on the pixel boundaries.
            let on = self.pixel(self.t + 0.5 * self.v.edge);
            self.t += 1.0 / self.rate;
            if self.v.fsk > 0.0 {
                let f = self.centre + if on { self.v.fsk } else { -self.v.fsk };
                *o = self.osc.next(f, 1.0);
                continue;
            }
            let target = if on { 1.0 } else { 0.0 };
            if self.v.edge > 0.0 {
                let step = 1.0 / (self.v.edge * self.rate);
                self.env = if on {
                    (self.env + step).min(1.0)
                } else {
                    (self.env - step).max(0.0)
                };
                let e = 0.5 * (1.0 - (std::f64::consts::PI * self.env).cos());
                *o = self.osc.next(self.centre, e);
            } else {
                *o = self.osc.next(self.centre, target);
            }
        }
    }
}

const BINS: usize = 56;
const KEEP: usize = 3000;
/// AM: the columns in a row, each with contrast of its own, that open the gate.
const OPEN: usize = 3;
/// AM: the most columns held while the gate is closed, drawn when it opens.
const HOLD: usize = 16;
/// AM: the columns after the gate closes within which contrast opens it again at once.
const GRACE: usize = 16;

/// The Hell receiver.
pub struct HellDecoder {
    v: HellVariant,
    centre: f64,
    find: bool,
    nco: Nco,
    lpf: Fir<Complex32>,
    prev: Complex32,
    env: SampleRing<f32>,
    n: u64,
    energy: [f32; BINS],
    col_start: f64,
    next_col: f64,
    floor: f32,
    peak: f32,
    gate: bool,
    /// AM, gate closed: the columns in a row with contrast of their own, and the columns since the contrast began (the
    /// drawn column, the sample where it starts and its pixel levels).
    run: usize,
    held: VecDeque<([u8; 14], f64, [f32; 14])>,
    /// AM: the columns since the gate was last open.
    closed_for: usize,
    cols: VecDeque<[u8; 14]>,
    total: u64,
    new_cols: Vec<HellColumn>,
    logs: Vec<String>,
    status: Status,
    finder: CentreFinder,
    snr: f32,
    pic_dirty: bool,
    /// A state of the tape went out.
    pic_sent: bool,
    since_pic: usize,
}

impl HellDecoder {
    /// A receiver with these settings.
    pub fn new() -> Self {
        let var = VARIANTS[0];
        let mut d = HellDecoder {
            v: var,
            centre: 1500.0,
            find: true,
            nco: Nco::new(8000.0, -1500.0),
            lpf: Fir::new(&[1.0]),
            prev: Complex32::new(1.0, 0.0),
            env: SampleRing::new(2048).expect("a ring of at least one sample"),
            n: 0,
            energy: [0.0; BINS],
            col_start: 0.0,
            next_col: 0.0,
            floor: 0.0,
            peak: 0.0,
            gate: false,
            run: 0,
            held: VecDeque::with_capacity(HOLD),
            closed_for: usize::MAX,
            cols: VecDeque::new(),
            total: 0,
            new_cols: Vec::new(),
            logs: Vec::new(),
            status: Status::default(),
            finder: CentreFinder::new(var.pixel_rate() as f32 + 2.0 * var.fsk as f32),
            snr: 0.0,
            pic_dirty: false,
            pic_sent: false,
            since_pic: 0,
        };
        d.rebuild();
        d
    }

    fn rebuild(&mut self) {
        let pr = self.v.pixel_rate();
        let cut = (0.75 * pr).max(2.0 * self.v.fsk + 0.6 * pr).min(3800.0);
        self.lpf = Fir::new(
            &lowpass(8000.0, cut, cut * 1.6 + 40.0, 50.0).expect("the filter settings are valid"),
        );
        self.nco = Nco::new(8000.0, -self.centre);
        self.energy = [0.0; BINS];
        self.env.reset();
        self.n = 0;
        self.col_start = 0.0;
        self.next_col = 0.0;
        self.close();
        self.closed_for = usize::MAX;
    }

    /// Closes the gate and forgets the columns held.
    fn close(&mut self) {
        self.gate = false;
        self.run = 0;
        self.held.clear();
    }

    fn col_len(&self) -> f64 {
        8000.0 / self.v.columns
    }

    /// The picture of the kept columns: 14 rows, the top row first.
    pub fn picture(&self) -> HfPicture {
        let w = self.cols.len().max(1) as u32;
        let mut p = HfPicture::new(1, self.v.name, w, 14, 0.0);
        for (x, col) in self.cols.iter().enumerate() {
            for (k, &v) in col.iter().enumerate() {
                p.put(x as u32, 13 - k as u32, v, v, v);
            }
        }
        p.rows = 14;
        p
    }

    /// The columns kept, oldest first.
    pub fn columns(&self) -> impl Iterator<Item = &[u8; 14]> {
        self.cols.iter()
    }

    fn column(&mut self, end: f64) {
        let p = self.col_len() / 14.0;
        let start = end - self.col_len();
        let now = self.n as f64;
        let mut vals = [0f32; 14];
        for (k, v) in vals.iter_mut().enumerate() {
            let a = start + k as f64 * p;
            let b = a + p;
            let (ia, ib) = (a.ceil() as i64, b.floor() as i64);
            let mut s = 0.0;
            let mut c = 0;
            for i in ia..ib.max(ia + 1) {
                let back = (now - 1.0 - i as f64) as i64;
                if back >= 0 && (back as usize) < self.env.capacity() {
                    s += self.env.back(back as usize).unwrap_or(0.0);
                    c += 1;
                }
            }
            *v = if c > 0 { s / c as f32 } else { 0.0 };
        }
        let fsk = self.v.fsk > 0.0;
        // Level tracking: the floor follows the lows, the peak the highs.
        for &v in &vals {
            if !fsk {
                if v < self.floor || self.floor == 0.0 {
                    self.floor += 0.2 * (v - self.floor);
                } else {
                    self.floor += 0.002 * (v - self.floor);
                }
                if v > self.peak {
                    self.peak += 0.3 * (v - self.peak);
                } else {
                    self.peak += 0.003 * (v - self.peak);
                }
            }
        }
        let contrast = if fsk {
            // FSK: the tone must stand clear of the noise in its band.
            self.peak > 0.0 && self.snr > 10.0
        } else {
            self.peak > 6.0 * self.floor.max(1e-12)
        };
        if !fsk {
            self.snr = 20.0 * (self.peak / self.floor.max(1e-12)).max(1e-6).log10();
        }
        if self.gate {
            self.closed_for = 0;
        } else {
            self.closed_for = self.closed_for.saturating_add(1);
        }
        if !contrast {
            self.close();
            return;
        }
        let mut col = [0u8; 14];
        for (k, &v) in vals.iter().enumerate() {
            let x = if fsk {
                (v / (2.0 * self.v.fsk as f32) + 0.5).clamp(0.0, 1.0)
            } else {
                ((v - self.floor) / (self.peak - self.floor).max(1e-12)).clamp(0.0, 1.0)
            };
            col[k] = (x * 255.0) as u8;
        }
        if fsk || self.gate {
            self.gate = true;
            self.emit(col);
            return;
        }
        self.open_am(col, start, vals, p);
    }

    /// AM, gate closed, a column `col` (pixel levels `vals`, from sample `start`, pixels of `p` samples) with contrast
    /// against the floor and the peak. The floor and the peak follow slowly, so after a step of the level (the start of the
    /// stream, noise after silence, the gain control settling) they differ by more than noise does. The gate opens after
    /// OPEN columns in a row whose own highest pixel is six times their lowest, as ink against blank rows is and noise is
    /// not, once a pixel near the floor came after one near the peak: blank rows come back after ink, the low level before
    /// a step up does not. Within GRACE columns of an open gate it opens at once. The columns held are drawn when it opens.
    fn open_am(&mut self, col: [u8; 14], start: f64, vals: [f32; 14], p: f64) {
        let (lo, hi) = vals
            .iter()
            .fold((f32::MAX, 0.0f32), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        self.run = if hi > 0.0 && hi > 6.0 * lo {
            self.run + 1
        } else {
            0
        };
        if self.held.len() == HOLD {
            self.held.pop_front();
        }
        self.held.push_back((col, start, vals));
        if self.closed_for <= GRACE || (self.run >= OPEN && self.low_after_high(p)) {
            self.gate = true;
            while let Some((c, _, _)) = self.held.pop_front() {
                self.emit(c);
            }
        }
    }

    /// AM: whether, in the columns held, a pixel at most twice the floor comes after one at least half the peak, in the order
    /// of the pixels' times (columns can overlap). `p`: the pixel length in samples.
    fn low_after_high(&self, p: f64) -> bool {
        let mut first_high = f64::INFINITY;
        for (_, start, vals) in &self.held {
            for (k, &v) in vals.iter().enumerate() {
                if v >= 0.5 * self.peak {
                    first_high = first_high.min(start + k as f64 * p);
                }
            }
        }
        self.held.iter().any(|(_, start, vals)| {
            vals.iter()
                .enumerate()
                .any(|(k, &v)| v <= 2.0 * self.floor && start + k as f64 * p > first_high)
        })
    }

    /// Keeps and sends one drawn column.
    fn emit(&mut self, col: [u8; 14]) {
        self.cols.push_back(col);
        while self.cols.len() > KEEP {
            self.cols.pop_front();
        }
        self.total += 1;
        self.new_cols.push(HellColumn { pixels: col });
        self.pic_dirty = true;
    }
}

impl Default for HellDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl sdrkit_graph::Decoder for HellDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.hell"
    }
    fn outputs(&self) -> Vec<Port> {
        let mut o = decoder::outputs();
        o.push(Port::typed::<HellColumn>("columns").rate_from(RateFrom::Fixed(0.0)));
        o
    }
    fn settings(&self) -> Arc<Schema> {
        HellSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match HellSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HellSetting::Variant => {
                let i = VARIANT_SETTING.read(id, value)?;
                let v = VARIANTS.get(i).copied().ok_or(SettingError::NotAChoice {
                    id,
                    index: i as u16,
                })?;
                if v != self.v {
                    self.v = v;
                    self.finder = CentreFinder::new(v.pixel_rate() as f32 + 2.0 * v.fsk as f32);
                    self.rebuild();
                }
            }
            HellSetting::Centre => {
                self.centre = CENTRE.read(id, value)?;
                self.rebuild();
            }
            HellSetting::FindSignal => self.find = FIND_SIGNAL.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let i = VARIANTS.iter().position(|v| *v == self.v).unwrap_or(0);
        values_of([
            (HellSetting::Variant, Value::Choice(i as u16)),
            (HellSetting::Centre, Value::Hz(self.centre)),
            (HellSetting::FindSignal, Value::Bool(self.find)),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("hell")
    }
    fn configure(&mut self, _input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.rebuild();
        let mut f = decoder::formats();
        f.push(Format::new(sdrkit_graph::Kind::typed::<HellColumn>(), 0.0));
        Ok(f)
    }
    fn retune(&mut self, _input: &Format) {
        self.rebuild();
    }
    fn finish(&mut self, out: &mut Outbox) {
        self.end_picture(out);
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        // In pieces that end where the signal finder decides, so the result does not depend on how the input is cut.
        let mut rest = x;
        while !rest.is_empty() {
            let n = rest.len().min(self.finder.until_frame());
            let (piece, r) = rest.split_at(n);
            rest = r;
            self.piece(piece, out);
        }
        let o = out.items::<HellColumn>(decoder::EXTRA);
        let time = self.n as f64 / 8000.0;
        for c in self.new_cols.drain(..) {
            o.push(Timed { time, value: c });
        }
    }
    fn report(&mut self, res: &mut Published) {
        let st = &mut self.status;
        st.lock_state = if self.gate {
            LockState::Locked
        } else {
            LockState::Searching
        };
        st.snr_db = Some(self.snr);
        st.offset_hz = Some(0.0);
        if self.gate {
            put_text(
                &mut st.summary,
                format_args!("{}: {} columns", self.v.name, self.total),
            );
        } else {
            put_text(
                &mut st.summary,
                format_args!("no picture yet: the tone shows here as lines of dots"),
            );
        }
        res.results_mut().set_status(st);
        res.put(&HellFields {
            variant: self.v.name,
            centre_hz: Hertz(self.centre),
            columns: self.total as i64,
            snr_db: f64::from(self.snr),
        });
        flush_logs(&mut self.logs, res);
    }
    fn clear(&mut self) {
        self.cols.clear();
    }
    fn forget(&mut self, _kept: &Kept) {
        self.cols.clear();
        self.total = 0;
    }
}

impl HellDecoder {
    fn piece(&mut self, x: &[Complex32], out: &mut Outbox) {
        let cl = self.col_len();
        let fsk = self.v.fsk > 0.0;
        for &s in x {
            let m = s * self.nco.next_sample();
            let y = self.lpf.process_sample(m);
            let e = if fsk {
                let d = (y * self.prev.conj()).arg() * (8000.0 / std::f32::consts::TAU);
                self.prev = y;
                d
            } else {
                y.norm()
            };
            self.env.push(e);
            // Energy by position in the column, for the column boundary.
            let pos = (self.n as f64 % cl) / cl;
            let b = ((pos * BINS as f64) as usize).min(BINS - 1);
            let ink = if fsk { e.max(0.0) } else { e * e };
            self.energy[b] += ink;
            self.n += 1;
            if self.n as f64 >= self.next_col {
                // Once a column: choose the boundary where four pixels (the blank rows) hold the least energy.
                let w = BINS * 4 / 14;
                let mut best = (f32::MAX, 0usize);
                for start in 0..BINS {
                    let s: f32 = (0..w).map(|k| self.energy[(start + k) % BINS]).sum();
                    if s < best.0 {
                        best = (s, start);
                    }
                }
                for v in &mut self.energy {
                    *v *= 0.97;
                }
                // The column starts half a pixel before the middle of the blank window: each pixel slot is then centred on the
                // middle of a sent pixel's energy as the filters spread it.
                let start_bin = (best.1 + w / 2 + BINS - BINS / 28) % BINS;
                let phase = start_bin as f64 / BINS as f64 * cl;
                let base = (self.n as f64 / cl).floor() * cl + phase;
                let end = if base <= self.n as f64 {
                    base
                } else {
                    base - cl
                };
                if end > self.col_start + 0.5 * cl && end >= cl {
                    self.column(end);
                    self.col_start = end;
                }
                self.next_col = self.n as f64 + cl / 2.0;
            }
        }
        if fsk {
            // FSK: the SNR from the spread of the frequency against the deviation.
            let last = self.env.last(512);
            let mean_abs = last.iter().map(|v| v.abs()).sum::<f32>() / last.len() as f32;
            let var = last
                .iter()
                .map(|v| (v.abs() - mean_abs).powi(2))
                .sum::<f32>()
                / last.len() as f32;
            let r = mean_abs / var.sqrt().max(1e-6);
            self.snr = 20.0 * r.max(1e-6).log10() + 6.0;
            self.peak = mean_abs;
        }
        self.since_pic += x.len();
        if self.since_pic >= 4000 {
            self.since_pic = 0;
            self.take_picture(out);
        }
        // The signal finder decides at the end of the piece, after the decoder has seen it.
        if self.find && !self.gate {
            if let Some(c) = self.finder.feed(x, self.centre as f32, true) {
                self.logs.push(format!("signal found at {c:.0} Hz"));
                self.centre = c as f64;
                self.rebuild();
            }
        }
    }
}

impl HellDecoder {
    /// Emits the picture of the kept columns to the outputs when it changed (called at each report). The picture is
    /// one tape that grows, with one number.
    pub fn take_picture(&mut self, out: &mut Outbox) {
        if self.pic_dirty {
            self.pic_dirty = false;
            self.pic_sent = true;
            out.picture(
                self.n as f64 / 8000.0,
                self.picture()
                    .to_picture(sdrkit_graph::PictureState::Growing),
            );
        }
    }
    /// Emits the tape once more, finished, at the end of the input.
    fn end_picture(&mut self, out: &mut Outbox) {
        if self.pic_dirty || self.pic_sent {
            (self.pic_dirty, self.pic_sent) = (false, false);
            out.picture(
                self.n as f64 / 8000.0,
                self.picture()
                    .to_picture(sdrkit_graph::PictureState::Finished),
            );
        }
    }
}

/// The test text of Hell.
pub const TEST_TEXT: &str = "K0TST TEST 73 ";

/// The generator. It reads the settings `variant`, `centre` and `gen_message`.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = VARIANTS[usize::from(cfg.settings.entry(&VARIANT_SETTING)).min(VARIANTS.len() - 1)];
    let centre = cfg.settings.get(&CENTRE);
    let m = HellModulator::new(
        v,
        cfg.rate_hz,
        centre,
        &decoder::message(cfg, TEST_TEXT),
        None,
    );
    let bw = if v.fsk > 0.0 {
        2.0 * v.fsk + v.pixel_rate()
    } else {
        v.pixel_rate()
    };
    // The tone is on for less than half the pixels of a text: the mean power while sending is about 0.4.
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        m,
        SignalInfo::new(bw)
            .with_symbol_rate(v.pixel_rate())
            .with_power(0.4),
    ))
}

/// The font file of the sender.
pub const FONT: sdrkit_core::Setting<sdrkit_core::publish::field::Text> =
    sdrkit_core::Setting::new("font", "Font file", "", "")
        .help("A TrueType font file (or OpenType with TrueType outlines) that the host names. The characters that the Hell glyphs do not have are sent with it, 14 pixels high; ASCII keeps the Hell glyphs. Empty, or a file that cannot be read: the Hell glyphs alone, and ? for the rest.")
        .section(crate::sender::TX_SECTION);

sdrkit_core::settings! {
    /// The settings of the Hell sender.
    pub enum HellTxSetting {
        /// The variant.
        Variant = VARIANT_SETTING,
        /// The audio centre.
        Centre = CENTRE,
        /// USB or LSB.
        Demod = crate::front::DEMOD,
        /// The level.
        Level = crate::sender::TX_LEVEL,
        /// The tune time.
        TuneLimit = crate::sender::TX_TUNE_LIMIT,
        /// The font file.
        Font = FONT,
    }
}

/// The blank columns of a space sent.
pub const TX_SPACE_COLUMNS: usize = 5;
/// The text at the start of every burst.
pub const TX_START: &str = "...";

/// The columns the sender sends for character `c` (14-pixel masks, bit 0 the bottom pixel): a blank column, the glyph's
/// columns from the first column of its cell to its last inked one, a blank column. A space, CR and LF are five blank
/// columns. The glyph is that of the shared Hell font, else that of `font` 14 pixels high, else `?` ([`glyph_pixels`]).
pub fn tx_columns(c: char, font: Option<&TrueTypeFont>, out: &mut Vec<u16>) {
    if matches!(c, ' ' | '\n' | '\r') {
        out.extend([0; TX_SPACE_COLUMNS]);
        return;
    }
    let g = glyph_pixels(c, font).unwrap_or_default();
    let end = g.iter().rposition(|&m| m != 0).map_or(0, |i| i + 1);
    out.push(0);
    out.extend_from_slice(&g[..end]);
    out.push(0);
}

/// The coder of Hell. Each column is 14 pixels, the bottom pixel first, at 14 times the column rate. On-off variants: an
/// inked pixel is the tone, a blank one silence (the phase runs on); in Feld Hell and Slow Hell a run of inked pixels rises and
/// falls by a raised cosine of min(4 ms, a pixel) inside its first and last pixel, HellX5 and HellX9 are keyed square. FSK
/// variants: the tone is always on, an inked pixel at centre + shift / 2, a blank one at centre - shift / 2. A burst is three
/// full stops, the text and one space. The glyphs are those of [`tx_columns`] with the coder's font.
pub struct HellCoder {
    v: HellVariant,
    centre: f64,
    font: Option<Arc<TrueTypeFont>>,
    cols: Vec<u16>,
}

impl HellCoder {
    /// A coder of variant `v` at the audio centre `centre`, with the shared Hell font alone.
    pub fn new(v: HellVariant, centre: f64) -> Self {
        HellCoder {
            v,
            centre,
            font: None,
            cols: Vec::new(),
        }
    }
    /// The same coder with `font` for the characters the shared Hell font does not have.
    #[must_use]
    pub fn with_font(mut self, font: Option<Arc<TrueTypeFont>>) -> Self {
        self.font = font;
        self
    }
    /// The elements of the columns of one character: a run of ink never crosses the blank columns at its ends.
    fn send(&mut self, c: char, out: &mut Vec<crate::sender::Element>) {
        self.cols.clear();
        tx_columns(c, self.font.as_deref(), &mut self.cols);
        let px = 1.0 / self.v.pixel_rate();
        let ink = |cols: &[u16], i: usize| (cols[i / 14] >> (i % 14)) & 1 == 1;
        let n = 14 * self.cols.len();
        for i in 0..n {
            let on = ink(&self.cols, i);
            let e = if self.v.fsk > 0.0 {
                let hz = self.centre + if on { self.v.fsk } else { -self.v.fsk };
                crate::sender::Element::tone(hz, px)
            } else if on {
                let edge = self.v.edge.min(px);
                let first = i == 0 || !ink(&self.cols, i - 1);
                let last = i + 1 == n || !ink(&self.cols, i + 1);
                crate::sender::Element {
                    hz: self.centre,
                    s: px,
                    env: crate::sender::Envelope::Keyed {
                        rise: if first { edge } else { 0.0 },
                        fall: if last { edge } else { 0.0 },
                    },
                }
            } else {
                crate::sender::Element::silence(self.centre, px)
            };
            out.push(e);
        }
    }
}

impl crate::sender::Coder for HellCoder {
    fn begin(&mut self, out: &mut Vec<crate::sender::Element>) {
        for c in TX_START.chars() {
            self.send(c, out);
        }
    }
    fn char(&mut self, c: char, out: &mut Vec<crate::sender::Element>) {
        self.send(c, out);
    }
    fn end(&mut self, out: &mut Vec<crate::sender::Element>) {
        self.send(' ', out);
    }
}

/// The Hell side of the sender.
#[derive(Clone, Debug)]
pub struct HellModem {
    v: HellVariant,
    centre: f64,
    demod: crate::front::Demod,
    common: crate::sender::Common,
    font_file: String,
    font: Option<Arc<TrueTypeFont>>,
}

impl Default for HellModem {
    fn default() -> Self {
        HellModem {
            v: VARIANTS[0],
            centre: CENTRE.default,
            demod: crate::front::DEMOD.default,
            common: crate::sender::Common::default(),
            font_file: String::new(),
            font: None,
        }
    }
}

impl crate::sender::Modem for HellModem {
    const NAME: &'static str = "hfkbd.helltx";
    fn schema() -> Arc<Schema> {
        HellTxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match HellTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HellTxSetting::Variant => {
                let i = VARIANT_SETTING.read(id, v)?;
                self.v = VARIANTS.get(i).copied().ok_or(SettingError::NotAChoice {
                    id,
                    index: i as u16,
                })?;
            }
            HellTxSetting::Centre => self.centre = CENTRE.read(id, v)?,
            HellTxSetting::Demod => self.demod = crate::front::DEMOD.read(id, v)?,
            HellTxSetting::Level => self.common.level = crate::sender::TX_LEVEL.read(id, v)?,
            HellTxSetting::TuneLimit => {
                self.common.tune_s = crate::sender::TX_TUNE_LIMIT.read(id, v)?;
            }
            HellTxSetting::Font => {
                let f = FONT.read(id, v)?;
                if f != self.font_file {
                    self.font = if f.is_empty() {
                        None
                    } else {
                        std::fs::read(f)
                            .ok()
                            .and_then(|b| TrueTypeFont::parse(b).ok())
                            .map(Arc::new)
                    };
                    f.clone_into(&mut self.font_file);
                }
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let i = VARIANTS.iter().position(|x| *x == self.v).unwrap_or(0);
        values_of([
            (HellTxSetting::Variant, Value::Choice(i as u16)),
            (HellTxSetting::Centre, Value::Hz(self.centre)),
            (HellTxSetting::Demod, Value::Choice(self.demod.index())),
            (HellTxSetting::Level, Value::F64(self.common.level)),
            (
                HellTxSetting::TuneLimit,
                Value::Duration(self.common.tune_s),
            ),
            (HellTxSetting::Font, Value::Text(self.font_file.clone())),
        ])
    }
    fn common(&self) -> crate::sender::Common {
        self.common
    }
    /// Every character is sent: one without a glyph in the Hell font or the font file as `?`, a line end as a space.
    fn accept(&self, text: &str, out: &mut Vec<char>) -> Result<(), crate::sender::TxError> {
        out.extend(text.chars());
        Ok(())
    }
    fn coder(&self) -> Result<crate::sender::Coding, crate::sender::TxError> {
        crate::sender::lower_of(self.demod)?;
        Ok(crate::sender::Coding::Elements(Box::new(
            HellCoder::new(self.v, self.centre).with_font(self.font.clone()),
        )))
    }
    fn centre_hz(&self) -> f64 {
        self.centre
    }
    /// The tones and half the pixel rate each side: the rate of alternate inked and blank pixels.
    fn band(&self) -> (f64, f64) {
        let half = self.v.fsk + self.v.pixel_rate() / 2.0;
        (self.centre - half, self.centre + half)
    }
    fn lower(&self) -> bool {
        crate::sender::lower_of(self.demod).unwrap_or(false)
    }
}

/// The Hell sender component.
pub fn sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<HellModem>::default())
}
