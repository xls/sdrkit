//! Composite video for the generators and the senders: the test card or a picture of the host, line by line at 12 Msps (24 Msps
//! for 819 lines). It has the sync pulses of the half-line grid, raised-cosine edges, the colour burst, and PAL, NTSC or SECAM
//! chrominance. A picture of the host is taken at the start of each field: the last one given before it.
//!
//! Standard: ITU-R BT.470-6.

use crate::card::{Card, Pattern, Planes, MULTIBURST};
use crate::systems::{secam, Colour, ColourParams, Pulse, Raster, System, SystemId};
use crate::util::rc_step;
use sdrkit_algo_dsp::fir::{lowpass, Convolver};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::Complex32;
use sdrkit_codec_image::colour::{KU, KV};
use std::f64::consts::TAU;

sdrkit_core::choice_enum! {
    /// Where SECAM is identified.
    pub enum SecamIdent {
        /// The lead-in on the back porch and the identification lines of the field blanking.
        Both = ("both", "Both"),
        /// The lead-in only.
        BackPorch = ("porch", "Back porch only"),
        /// The identification lines only.
        FieldLines = ("lines", "Field lines only"),
    }
}

/// Options of the composite video.
#[derive(Clone, Debug)]
pub struct VideoOptions {
    /// The system.
    pub system: SystemId,
    /// The colour.
    pub colour: Colour,
    /// What it shows.
    pub pattern: Pattern,
    /// The ident text.
    pub ident: String,
    /// 7.5 IRE setup on 525 lines.
    pub setup: bool,
    /// SECAM line identification.
    pub secam_ident: SecamIdent,
    /// Chrominance sent 170 ns early (the transmitter's group-delay pre-correction).
    pub group_delay: bool,
    /// The clock at the first sample, seconds.
    pub start_s: f64,
    /// Teletext in the field blanking (625 lines only).
    pub teletext: bool,
}

impl Default for VideoOptions {
    fn default() -> Self {
        VideoOptions {
            system: SystemId::G,
            colour: Colour::Pal,
            pattern: Pattern::Card,
            ident: "TEST CARD".into(),
            setup: true,
            secam_ident: SecamIdent::Both,
            group_delay: true,
            start_s: 0.0,
            teletext: false,
        }
    }
}

/// Makes composite video at its internal rate.
pub struct VideoGen {
    sys: System,
    r: Raster,
    colour: Colour,
    cp: Option<ColourParams>,
    rate: f64,
    nf: u64,
    lines: u64,
    /// Global internal sample index of the next sample.
    n: u64,
    card: Card,
    /// The picture of the host in use; `None` shows the test card.
    host: Option<Planes>,
    /// A change of the picture that waits for the start of the next field.
    next: Option<Option<Planes>>,
    /// Pictures of the host taken.
    taken: u64,
    /// The first line of field 2.
    field2_line: u64,
    /// The pictures of field 2 of this frame are taken.
    field2_done: bool,
    shown: Option<Planes>,
    luma: Vec<f32>,
    u: Vec<f32>,
    v: Vec<f32>,
    dr: Vec<f32>,
    db: Vec<f32>,
    /// The luma and the chroma low-passes, on a line with its held ends ([`filter_line`]).
    ylp: Convolver<f32>,
    clp: Convolver<f32>,
    /// The pixel-held Y, U and V of one line, reused for each line.
    line_raw: [Vec<f32>; 3],
    /// The band-limited Y, U and V of one line, reused for each line.
    line_filtered: [Vec<f32>; 3],
    /// The line with its held ends that [`filter_line`] reads, reused for each line.
    line_held: Vec<f32>,
    /// The filter's output over the held line, reused for each line.
    line_full: Vec<f32>,
    /// The shapes of the kinds of line drawn so far.
    shapes: Vec<LineShape>,
    /// For each picture row: the row changed since the last frame. Reused for each frame.
    changed: Vec<bool>,
    /// For each line (line `l` at `l - 1`): `None` when it is drawn, or `Some(card)` when it waits to be drawn from
    /// [`VideoGen::shown`], with `card` true when the test card was on the air as it was marked.
    dirty: Vec<Option<bool>>,
    setup: f64,
    gd: usize,
    secam_ident: SecamIdent,
    secam_phase: f64,
    /// The colour subcarrier at the next sample, and its values over the line being sent.
    subcarrier: Nco,
    sub_line: Vec<Complex32>,
    frame: u64,
    built: bool,
    pattern: Pattern,
    teletext: Option<crate::teletext::Service>,
}

/// What sets the shape of a line: where its first sample falls after its 0H (in units of 1 / lines of a sample), its
/// length, its pulses and whether it carries picture.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LineKey {
    start: u64,
    len: u64,
    pulses: [Pulse; 3],
    picture: bool,
}

/// The parts of a line that do not depend on the picture, for each of its samples: the time after 0H (µs), the sync, and
/// on a picture line the blanking window and the pixel of the picture row.
#[derive(Clone, Debug, Default)]
struct LineShape {
    key: Option<LineKey>,
    t: Vec<f64>,
    sync: Vec<f64>,
    window: Vec<f64>,
    pixel: Vec<usize>,
}

/// The most line shapes a generator keeps.
const MAX_SHAPES: usize = 32;

/// Filters one line `x` into `y` with `taps`, a [`line_filter`], holding the end samples beyond each end of the line:
/// `y[i] = sum_j h[j] x[i + j - len / 2]` for the design `h`. `held` (the line with its held ends) and `full` (the output
/// over it) are scratch.
fn filter_line(
    x: &[f32],
    taps: &Convolver<f32>,
    held: &mut Vec<f32>,
    full: &mut Vec<f32>,
    y: &mut Vec<f32>,
) {
    let n = x.len();
    y.clear();
    let (Some(&first), Some(&last)) = (x.first(), x.last()) else {
        return;
    };
    let len = taps.len();
    held.clear();
    held.resize(len / 2, first);
    held.extend_from_slice(x);
    held.resize(n + len - 1, last);
    full.clear();
    full.resize(held.len(), 0.0);
    taps.centred(held, full)
        .expect("the output is as long as the input");
    // Output `m` of the convolver with the reversed design is `sum_j h[j] held[m - (len - 1 - centre) + j]`.
    let o = len - 1 - taps.centre();
    y.extend_from_slice(&full[o..o + n]);
}

/// The convolver of [`filter_line`] for the design `h`: its taps reversed.
fn line_filter(h: &[f32]) -> Convolver<f32> {
    let rev: Vec<f32> = h.iter().rev().copied().collect();
    Convolver::new(&rev).expect("a design has finite taps")
}

/// The picture on the air: the host's, or the test card.
fn source<'a>(host: &'a Option<Planes>, card: &'a Card) -> &'a Planes {
    host.as_ref().unwrap_or(&card.planes)
}

/// Whether row `row` of `a` and `b` (pictures of one size) differs.
fn row_differs(a: &Planes, b: &Planes, row: usize) -> bool {
    let r = row * a.w..(row + 1) * a.w;
    a.y[r.clone()] != b.y[r.clone()] || a.u[r.clone()] != b.u[r.clone()] || a.v[r.clone()] != b.v[r]
}

/// Copies row `row` of `from` into `to` (pictures of one size).
fn copy_row(from: &Planes, to: &mut Planes, row: usize) {
    let r = row * from.w..(row + 1) * from.w;
    to.y[r.clone()].copy_from_slice(&from.y[r.clone()]);
    to.u[r.clone()].copy_from_slice(&from.u[r.clone()]);
    to.v[r.clone()].copy_from_slice(&from.v[r]);
}

/// The internal rate for a system.
pub fn internal_rate(id: SystemId) -> f64 {
    if id == SystemId::E {
        24e6
    } else {
        12e6
    }
}

impl VideoGen {
    /// A generator for these options.
    pub fn new(o: &VideoOptions) -> Self {
        let sys = System::get(o.system);
        let r = sys.raster;
        let rate = internal_rate(o.system);
        let frame_rate = r.line_hz / r.lines as f64;
        let nf = (rate / frame_rate).round() as u64;
        let video = sys.video_mhz * 1e6;
        let ypass = (video - 0.4e6).min(0.45 * rate);
        let ystop = (video + 0.6e6).min(0.5 * rate);
        let ylp = lowpass(rate, ypass, ystop, 40.0).expect("the filter settings are valid");
        let clp = lowpass(rate, 1.3e6, 2.6e6, 40.0).expect("the filter settings are valid");
        let colour_name = if o.colour == Colour::Mono {
            "MONO".to_string()
        } else {
            o.colour.name().to_string()
        };
        let label = format!("{} {}", colour_name, sys.letter);
        let card = Card::new(
            r.width as usize,
            r.height as usize,
            o.pattern,
            &o.ident,
            &label,
            r.label().replace("59.94", "60").as_str(),
        );
        let n = (o.start_s * rate).round().max(0.0) as u64;
        let cp = o.colour.params();
        VideoGen {
            sys,
            r,
            colour: o.colour,
            cp,
            rate,
            nf,
            lines: r.lines as u64,
            n,
            card,
            host: None,
            next: None,
            taken: 0,
            field2_line: r.lines as u64 / 2 + 1,
            field2_done: false,
            shown: None,
            luma: vec![0.0; nf as usize],
            u: vec![0.0; nf as usize],
            v: vec![0.0; nf as usize],
            dr: if o.colour == Colour::Secam {
                vec![0.0; nf as usize]
            } else {
                Vec::new()
            },
            db: if o.colour == Colour::Secam {
                vec![0.0; nf as usize]
            } else {
                Vec::new()
            },
            ylp: line_filter(&ylp),
            clp: line_filter(&clp),
            line_raw: Default::default(),
            line_filtered: Default::default(),
            line_held: Vec::new(),
            line_full: Vec::new(),
            shapes: Vec::new(),
            changed: Vec::new(),
            dirty: vec![None; r.lines as usize],
            setup: if r.lines == 525 && o.setup {
                r.setup
            } else {
                0.0
            },
            gd: if o.group_delay && cp.is_some() {
                (0.17e-6 * rate).round() as usize
            } else {
                0
            },
            secam_ident: o.secam_ident,
            secam_phase: 0.0,
            subcarrier: {
                let fsc = cp.map_or(0.0, |c| c.fsc);
                let mut nco = Nco::new(rate, fsc);
                nco.set_phase_cycles((n as f64 * fsc / rate).fract());
                nco
            },
            sub_line: Vec::new(),
            frame: u64::MAX,
            built: false,
            pattern: o.pattern,
            teletext: (o.teletext && r.lines == 625).then(crate::teletext::Service::default),
        }
    }

    /// The internal rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }
    /// The system.
    pub fn system(&self) -> &System {
        &self.sys
    }
    /// The raster.
    pub fn raster(&self) -> &Raster {
        &self.r
    }
    /// The picture last drawn (the test card as sent).
    pub fn card(&self) -> &Planes {
        &self.card.planes
    }

    /// Shows `picture` from the start of the next field on, or the test card again with `None`. The picture has the size of
    /// the raster ([`Raster::width`] x [`Raster::height`]); a picture of another size is ignored.
    pub fn show(&mut self, picture: Option<Planes>) {
        let fits = picture
            .as_ref()
            .is_none_or(|p| p.w == self.r.width as usize && p.h == self.r.height as usize);
        if fits {
            self.next = Some(picture);
        }
    }

    /// The pictures of the host taken so far: one for each field that started with a new one.
    pub fn pictures_taken(&self) -> u64 {
        self.taken
    }

    /// Takes the change of the picture that waits, if any.
    fn take_next(&mut self) -> bool {
        match self.next.take() {
            Some(p) => {
                self.taken += u64::from(p.is_some());
                self.host = p;
                true
            }
            None => false,
        }
    }

    /// Time in µs after 0H of line `l` (1-based) of frame position `k`.
    #[inline]
    fn t_us(&self, l: u64, k: u64) -> f64 {
        (k as f64 * self.lines as f64 - ((l - 1) * self.nf) as f64)
            / (self.lines as f64 * self.rate)
            * 1e6
    }

    fn line_range(&self, l: u64) -> (u64, u64) {
        let s0 = ((l - 1) * self.nf).div_ceil(self.lines);
        let s1 = (l * self.nf).div_ceil(self.lines);
        (s0, s1)
    }

    /// The identification lines of SECAM (7 to 15 and 320 to 328).
    fn secam_id_line(&self, l: u64) -> bool {
        self.colour == Colour::Secam
            && self.r.lines == 625
            && ((7..=15).contains(&l) || (320..=328).contains(&l))
            && self.secam_ident != SecamIdent::BackPorch
    }

    /// The shape of line `l` from [`VideoGen::shapes`], made the first time a line of its kind is drawn.
    fn line_shape(&mut self, l: u64) -> usize {
        let r = self.r;
        let (s0, s1) = self.line_range(l);
        let half = r.line_us() / 2.0;
        // The pulses of the two half-lines of the line, and the first of the next line, whose leading edge (centred on its
        // 0H) starts in this line.
        let pulses = [
            r.half_line_pulse((2 * (l - 1)) as u32),
            r.half_line_pulse((2 * (l - 1) + 1) as u32),
            r.half_line_pulse((2 * l) as u32),
        ];
        let picture = r.row_of(l as u32).is_some();
        // A picture line whose second half carries a pulse (the last line of field 2 in 625 lines) is a half line: its
        // picture ends a front porch before the half-line.
        let active_end = if pulses[1] == Pulse::None {
            r.blank_end_us + r.active_us
        } else {
            half - r.front_porch_us
        };
        let key = LineKey {
            start: s0 * self.lines - (l - 1) * self.nf,
            len: s1 - s0,
            pulses,
            picture,
        };
        if let Some(i) = self.shapes.iter().position(|sh| sh.key == Some(key)) {
            return i;
        }
        // Past the bound (819 lines have a timing for nearly every line), the last place is made again for each line.
        let at = self.shapes.len().min(MAX_SHAPES - 1);
        if at == self.shapes.len() {
            self.shapes.push(LineShape::default());
        }
        let mut sh = std::mem::take(&mut self.shapes[at]);
        sh.key = Some(key);
        sh.t.clear();
        sh.sync.clear();
        sh.window.clear();
        sh.pixel.clear();
        let (w, wpx) = (r.active_us, r.width as usize);
        for k in s0..s1 {
            let t = self.t_us(l, k);
            let mut s = 0.0f64;
            for (hh, p) in pulses.iter().enumerate() {
                let th = t - hh as f64 * half;
                let width = match p {
                    Pulse::Line => r.sync_us,
                    Pulse::Equalising => r.eq_us,
                    Pulse::Broad => r.broad_us,
                    Pulse::None => continue,
                };
                if th > -1.0 && th < width + 1.0 {
                    s += r.sync_level
                        * (rc_step(th, r.sync_edge_us) - rc_step(th - width, r.sync_edge_us));
                }
            }
            sh.t.push(t);
            sh.sync.push(s);
            if picture {
                sh.window.push(
                    rc_step(t - r.blank_end_us, r.blank_edge_us)
                        - rc_step(t - active_end, r.blank_edge_us),
                );
                let x = ((t - r.blank_end_us) / w * wpx as f64).floor();
                sh.pixel.push(x.clamp(0.0, (wpx - 1) as f64) as usize);
            }
        }
        self.shapes[at] = sh;
        at
    }

    /// Marks line `l` to be drawn from the picture now shown.
    fn mark(&mut self, l: u64) {
        self.dirty[(l - 1) as usize] = Some(self.host.is_none());
    }

    /// Draws line `l` if it waits to be drawn.
    fn draw(&mut self, l: u64) {
        if let Some(card) = self.dirty[(l - 1) as usize].take() {
            self.render_line(l, card);
        }
    }

    /// Draws line `l` from the rows of [`VideoGen::shown`]; `card` is true when the test card was on the air as the line
    /// was marked.
    fn render_line(&mut self, l: u64, card: bool) {
        let r = self.r;
        let (s0, s1) = self.line_range(l);
        let row = r.row_of(l as u32);
        let w = r.active_us;
        let shape = self.line_shape(l);
        let sh = &self.shapes[shape];
        // Picture: pixel-held card values at the internal samples, then band-limited.
        let len = (s1 - s0) as usize;
        let mut raw = std::mem::take(&mut self.line_raw);
        for c in &mut raw {
            c.clear();
            c.resize(len, 0.0);
        }
        if let Some((row, _)) = row {
            let pl = self
                .shown
                .as_ref()
                .expect("a picture line is marked after the picture is shown");
            let base = row as usize * pl.w;
            for (i, &x) in sh.pixel.iter().enumerate() {
                raw[0][i] = pl.y[base + x];
                raw[1][i] = pl.u[base + x];
                raw[2][i] = pl.v[base + x];
            }
        }
        let mut filtered = std::mem::take(&mut self.line_filtered);
        let (yf, uf, vf) = if row.is_some() {
            let (held, full) = (&mut self.line_held, &mut self.line_full);
            filter_line(&raw[0], &self.ylp, held, full, &mut filtered[0]);
            filter_line(&raw[1], &self.clp, held, full, &mut filtered[1]);
            filter_line(&raw[2], &self.clp, held, full, &mut filtered[2]);
            (&filtered[0], &filtered[1], &filtered[2])
        } else {
            (&raw[0], &raw[1], &raw[2])
        };
        let mb = row.map(|(rw, _)| {
            let h = r.height as f64;
            (rw as f64) >= (MULTIBURST[2] * h).round() && (rw as f64) < (MULTIBURST[3] * h).round()
        }) == Some(true)
            && self.pattern == Pattern::Card
            && card;
        let packets: [f64; 6] = if self.sys.id == SystemId::A {
            [0.25, 0.5, 1.0, 1.5, 2.0, 2.75]
        } else if matches!(self.sys.id, SystemId::M | SystemId::N) {
            [0.5, 1.0, 2.0, 3.0, 3.58, 4.2]
        } else {
            [0.5, 1.0, 2.0, 3.0, 4.0, 4.8]
        };
        let mbx0 = r.blank_end_us + MULTIBURST[0] * w;
        let mbx1 = r.blank_end_us + MULTIBURST[1] * w;
        let id_line = self.secam_id_line(l);
        let at = s0 as usize..s1 as usize;
        if row.is_none() {
            for (y, &s) in self.luma[at.clone()].iter_mut().zip(&sh.sync) {
                *y = s as f32;
            }
            self.u[at.clone()].fill(0.0);
            self.v[at].fill(0.0);
        } else {
            let (luma, u, v) = (
                &mut self.luma[at.clone()],
                &mut self.u[at.clone()],
                &mut self.v[at],
            );
            for i in 0..len {
                let (t, win) = (sh.t[i], sh.window[i]);
                let mut y = yf[i] as f64;
                if mb && t >= mbx0 && t < mbx1 {
                    let pk = ((t - mbx0) / (mbx1 - mbx0) * 6.0).floor() as usize;
                    let p0 = mbx0 + pk as f64 * (mbx1 - mbx0) / 6.0;
                    let pw = (mbx1 - mbx0) / 6.0;
                    let env = rc_step(t - p0 - 0.15 * pw, 0.3) - rc_step(t - p0 - 0.85 * pw, 0.3);
                    y += 0.4 * env * (TAU * packets[pk.min(5)] * (t - p0)).sin();
                }
                let c = win * (self.setup + (1.0 - self.setup) * y);
                luma[i] = (sh.sync[i] + c) as f32;
                u[i] = (win * uf[i] as f64) as f32;
                v[i] = (win * vf[i] as f64) as f32;
            }
        }
        self.line_raw = raw;
        self.line_filtered = filtered;
        if self.colour == Colour::Secam {
            self.render_secam_line(l, s0, s1, row.is_some(), id_line);
        }
    }

    /// The SECAM colour differences, pre-emphasised along the line (D'R and D'B both, the line parity picks one when sent).
    fn render_secam_line(&mut self, l: u64, s0: u64, s1: u64, picture: bool, id_line: bool) {
        let r = self.r;
        let w1 = TAU * secam::F1;
        // Bilinear transform of (1 + s / w1) / (1 + s / (3 w1)).
        let k = 2.0 * self.rate;
        let (b0, b1) = (1.0 + k / w1, 1.0 - k / w1);
        let (a0, a1) = (1.0 + k / (3.0 * w1), 1.0 - k / (3.0 * w1));
        let (mut xr, mut yr, mut xb, mut yb) = (0.0, 0.0, 0.0, 0.0);
        for k_ in s0..s1 {
            let t = self.t_us(l, k_);
            let p = k_ as usize;
            let (dr, db) = if id_line {
                // Trapezoids of BT.470-6 note 24.
                let a = r.blank_end_us;
                let ramp = ((t - a) / 5.0).clamp(0.0, 1.0)
                    * ((r.blank_end_us + r.active_us - t) / 3.0).clamp(0.0, 1.0);
                (1.25 * ramp, -1.52 * ramp)
            } else if picture && t >= r.blank_end_us - 0.5 {
                let ry = self.v[p] as f64 / KV as f64;
                let by = self.u[p] as f64 / KU as f64;
                (secam::KR * ry, secam::KB * by)
            } else {
                (0.0, 0.0)
            };
            if id_line {
                self.dr[p] = dr as f32;
                self.db[p] = db as f32;
                continue;
            }
            let or = (b0 * dr + b1 * xr - a1 * yr) / a0;
            xr = dr;
            yr = or;
            let ob = (b0 * db + b1 * xb - a1 * yb) / a0;
            xb = db;
            yb = ob;
            self.dr[p] = or.clamp(-2.0, 2.0) as f32;
            self.db[p] = ob.clamp(-2.0, 2.0) as f32;
        }
    }

    fn new_frame(&mut self) {
        let frame = self.n / self.nf;
        let t = self.n as f64 / self.rate;
        self.take_next();
        if self.host.is_none() {
            self.card.update(t, frame);
        }
        let mut changed = std::mem::take(&mut self.changed);
        changed.clear();
        let now = source(&self.host, &self.card);
        match &self.shown {
            Some(prev) if self.built => changed
                .extend((0..self.r.height as usize).map(|row| row_differs(prev, now, row))),
            _ => changed.resize(self.r.height as usize, true),
        }
        match &mut self.shown {
            Some(s) if (s.w, s.h) == (now.w, now.h) => {
                for row in (0..now.h).filter(|&row| changed[row]) {
                    copy_row(now, s, row);
                }
            }
            _ => self.shown = Some(now.clone()),
        }
        // The lines are drawn as the signal reaches them. A line stays marked until it is drawn, and draws the rows it
        // was marked with, so the output does not depend on when it is drawn.
        for l in 1..=self.lines {
            let render = match self.r.row_of(l as u32) {
                Some((row, _)) => changed[row as usize],
                None => !self.built,
            };
            if render {
                self.mark(l);
            }
        }
        self.built = true;
        self.changed = changed;
        self.frame = frame;
        self.field2_done = false;
        if self.teletext.is_some() {
            for l in 1..=self.lines {
                if crate::teletext::is_teletext_line(l as u32) && !self.secam_id_line(l) {
                    self.mark(l);
                    self.draw(l);
                    self.teletext_line(l, t);
                }
            }
        }
    }

    /// At the start of field 2: takes the change of the picture that waits and marks the lines of field 2 whose rows
    /// changed.
    fn new_field2(&mut self) {
        if !self.take_next() {
            return;
        }
        for l in self.field2_line..=self.lines {
            let Some((row, _)) = self.r.row_of(l as u32) else {
                continue;
            };
            let row = row as usize;
            let now = source(&self.host, &self.card);
            let Some(shown) = &mut self.shown else {
                continue;
            };
            if !row_differs(shown, now, row) {
                continue;
            }
            copy_row(now, shown, row);
            self.mark(l);
        }
    }

    /// Adds the next teletext packet to line `l` (BT.653 system B: NRZ at 6.9375 Mbit/s, a one at 0.66 of white).
    fn teletext_line(&mut self, l: u64, clock_s: f64) {
        use crate::teletext::{line_bits, BIT_RATE, LINE_BITS, ONE_LEVEL, START_US};
        let Some(svc) = &mut self.teletext else {
            return;
        };
        let bits = line_bits(&svc.next_packet(clock_s));
        let tb = 1e6 / BIT_RATE;
        let rise = 0.7 * tb;
        let (s0, s1) = self.line_range(l);
        for k in s0..s1 {
            let t = self.t_us(l, k);
            let x = (t - START_US) / tb;
            if x < -1.0 || x > LINE_BITS as f64 + 1.0 {
                continue;
            }
            let i = x.floor() as i64;
            let mut v = 0.0;
            for j in (i - 1).max(0)..=(i + 1).min(LINE_BITS as i64 - 1) {
                if bits[j as usize] == 1 {
                    let a = START_US + j as f64 * tb;
                    v += rc_step(t - a, rise) - rc_step(t - a - tb, rise);
                }
            }
            self.luma[k as usize] += (ONE_LEVEL * v) as f32;
        }
    }

    /// Whether line `l` carries a burst.
    fn burst_line(&self, l: u64) -> bool {
        if self.r.lines == 525 {
            !(l >= 522 || l <= 7 || (260..=270).contains(&l))
        } else {
            let i = self.sys.id == SystemId::I;
            let (a, b, c, d) = if i {
                (623, 6, 311, 319)
            } else {
                (622, 5, 310, 318)
            };
            !(l >= a || l <= b || (c..=d).contains(&l))
        }
    }

    /// Fills `out` with composite video.
    pub fn fill(&mut self, out: &mut [f32]) {
        let r = self.r;
        let mut i = 0;
        while i < out.len() {
            if self.frame != self.n / self.nf {
                self.new_frame();
            }
            let fpos = self.n % self.nf;
            if !self.field2_done && fpos >= self.line_range(self.field2_line).0 {
                self.field2_done = true;
                self.new_field2();
            }
            // The line holding fpos.
            let l = fpos * self.lines / self.nf + 1;
            let l = if self.line_range(l).0 > fpos {
                l - 1
            } else {
                l
            };
            // The line, and the next, whose first chroma samples the end of this line reads.
            self.draw(l);
            if l < self.lines {
                self.draw(l + 1);
            }
            let (_, s1) = self.line_range(l);
            let n_line = ((s1 - fpos) as usize).min(out.len() - i);
            let global_line = (self.n / self.nf) * self.lines + (l - 1);
            let odd = global_line % 2 == 1;
            match (self.colour, self.cp) {
                (Colour::Secam, _) => {
                    let id_line = self.secam_id_line(l);
                    let picture = r.row_of(l as u32).is_some();
                    let start = if self.secam_ident == SecamIdent::FieldLines && !id_line {
                        r.blank_end_us
                    } else {
                        5.6
                    };
                    let end = if r.half_line_pulse((2 * l - 1) as u32) == Pulse::None {
                        r.blank_end_us + r.active_us
                    } else {
                        r.line_us() / 2.0 - r.front_porch_us
                    };
                    let (f0, dev, arr) = if odd {
                        (secam::F0B, secam::DEV_B, &self.db)
                    } else {
                        (secam::F0R, secam::DEV_R, &self.dr)
                    };
                    for j in 0..n_line {
                        let k = fpos + j as u64;
                        let t = self.t_us(l, k);
                        let mut v = self.luma[k as usize] as f64;
                        if (picture || id_line) && t > start - 0.5 && t < end + 0.5 {
                            let d = arr[k as usize] as f64;
                            let f = f0 + dev * d;
                            self.secam_phase = (self.secam_phase + f / self.rate).fract();
                            let ff = f / secam::F_BELL - secam::F_BELL / f;
                            let g = num_complex::Complex64::new(1.0, secam::BELL_Q * ff)
                                / num_complex::Complex64::new(1.0, 1.26 * ff);
                            let env = rc_step(t - start, 0.3) - rc_step(t - end, 0.3);
                            v += env
                                * secam::M0
                                * g.norm()
                                * (TAU * self.secam_phase + g.arg()).cos();
                        } else {
                            self.secam_phase = (self.secam_phase + f0 / self.rate).fract();
                        }
                        out[i + j] = v as f32;
                    }
                }
                (_, Some(cp)) => {
                    let vs = if cp.v_switch && odd { -1.0 } else { 1.0 };
                    let burst = self.burst_line(l);
                    let blen = cp.burst_cycles / cp.fsc * 1e6;
                    let (bu, bv) = if cp.v_switch {
                        (
                            -std::f64::consts::FRAC_1_SQRT_2,
                            std::f64::consts::FRAC_1_SQRT_2 * vs,
                        )
                    } else {
                        (-1.0, 0.0)
                    };
                    let mut sub = std::mem::take(&mut self.sub_line);
                    sub.resize(n_line, Complex32::default());
                    self.subcarrier.fill(&mut sub);
                    let subcarrier = |j: usize| (sub[j].im, sub[j].re);
                    let last = self.nf as usize - 1;
                    for (j, o) in out[i..i + n_line].iter_mut().enumerate() {
                        let k = fpos as usize + j;
                        let kk = (k + self.gd).min(last);
                        let (s, c) = subcarrier(j);
                        *o = self.luma[k] + self.u[kk] * s + vs as f32 * self.v[kk] * c;
                    }
                    if burst {
                        let (b0, b1) = (cp.burst_start_us - 0.3, cp.burst_start_us + blen + 0.3);
                        // The samples near the burst; the exact test below picks those inside it.
                        let near = |t: f64| {
                            let k = (t * 1e-6 * self.lines as f64 * self.rate
                                + ((l - 1) * self.nf) as f64)
                                / self.lines as f64;
                            (k.max(0.0) as u64).saturating_sub(fpos)
                        };
                        let j0 = (near(b0).saturating_sub(2) as usize).min(n_line);
                        let j1 = ((near(b1) + 2) as usize).min(n_line);
                        for j in j0..j1 {
                            let t = self.t_us(l, fpos + j as u64);
                            if t > b0 && t < b1 {
                                let (s, c) = subcarrier(j);
                                let env = rc_step(t - cp.burst_start_us, 0.3)
                                    - rc_step(t - cp.burst_start_us - blen, 0.3);
                                // The table's burst amplitude is the peak (3/14 of white is 300 mV peak to peak).
                                out[i + j] +=
                                    (cp.burst_amp * env) as f32 * (bu as f32 * s + bv as f32 * c);
                            }
                        }
                    }
                    self.sub_line = sub;
                }
                _ => {
                    out[i..i + n_line]
                        .copy_from_slice(&self.luma[fpos as usize..fpos as usize + n_line]);
                }
            }
            self.n += n_line as u64;
            i += n_line;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(o: &VideoOptions) -> (VideoGen, Vec<f32>) {
        let mut g = VideoGen::new(o);
        let mut v = vec![0f32; g.nf as usize];
        g.fill(&mut v);
        (g, v)
    }

    #[test]
    fn line_sync_width_and_levels_are_right() {
        let (g, v) = frame(&VideoOptions {
            colour: Colour::Mono,
            ..Default::default()
        });
        let rate = g.rate;
        // Line 100 starts at 99 * 768.
        let s = 99 * 768;
        let th = (-3.0 / 7.0) / 2.0;
        let lead = (0..60).find(|&i| v[s - 30 + i] < th as f32).unwrap() as f64 - 30.0;
        let trail = (0..120).find(|&i| v[s + 20 + i] > th as f32).unwrap() as f64 + 20.0;
        let width_us = (trail - lead) / rate * 1e6;
        assert!((width_us - 4.7).abs() < 0.1, "{width_us}");
        assert!((v[s + 30] as f64 + 3.0 / 7.0).abs() < 0.002);
        assert!(v[s - 6].abs() < 0.002, "front porch {}", v[s - 6]);
    }

    #[test]
    fn the_burst_alternates_and_the_secam_lead_in_is_at_rest() {
        let (_, v) = frame(&VideoOptions::default());
        let s = 99 * 768;
        let burst: f32 = (0..20).map(|i| v[s + 75 + i].abs()).fold(0.0, f32::max);
        assert!((burst as f64 - 3.0 / 14.0).abs() < 0.02, "{burst}");
        let (g, v) = frame(&VideoOptions {
            system: SystemId::L,
            colour: Colour::Secam,
            ..Default::default()
        });
        let s = 99 * 768;
        // Zero crossings over the lead-in (6.5 .. 9.5 µs).
        let a = s + (6.5e-6 * g.rate) as usize;
        let b = s + (9.5e-6 * g.rate) as usize;
        let mut z = 0;
        for i in a..b {
            if (v[i] >= 0.0) != (v[i + 1] >= 0.0) {
                z += 1;
            }
        }
        let f = z as f64 / 2.0 / ((b - a) as f64 / g.rate);
        assert!(
            (f - secam::F0R).abs() < 200e3 || (f - secam::F0B).abs() < 200e3,
            "{f}"
        );
        assert!(v[s + 24].abs() < 0.5);
    }
}
