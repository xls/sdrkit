//! `dtmb.body`: the frame body. Per frame it takes the PN phase of the headers from the tracker, fits the channel to each header
//! by least squares, removes the headers' leakage into the body, and equalises the body for C = 3780 (zero
//! forcing per carrier) or C = 1 (minimum mean square error over a 4096-point transform), then scales it by the median body gain.
//! PN595 is equalised both ways until `dtmb.si` names the carrier form.
//!
//! Standard: GB 20600-2006 clauses 4.4 to 4.6.

use super::estimate::{tracking, widest, Estimator, Fit};
use super::pn::Header;
use super::profile::{carriers, SI};
use super::track::{is_silent, FrameSyms};
use super::BODY;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    apply_commands, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port,
    RateFrom, SlicePool, Timed,
};
use std::collections::VecDeque;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The carrier forms `dtmb.body` equalises.
    pub enum Forms {
        /// Both (PN595 before the system information decides).
        Both = ("both", "Both"),
        /// C = 3780 only.
        Multi = ("3780", "C = 3780"),
        /// C = 1 only.
        Single = ("1", "C = 1"),
    }
}

/// The carrier forms to equalise. `dtmb.si` sets it once the system information names the form.
pub const FORMS: Setting<field::Choice<Forms>> =
    Setting::choice("forms", "Carrier forms", Forms::Both);

sdrkit_core::settings! {
    /// The settings of `dtmb.body`.
    pub enum BodySetting {
        /// The carrier forms.
        Forms = FORMS,
    }
}

/// One equalised body: the 36 system-information symbols then the 3744 data symbols in stream order, each with its complex
/// noise variance.
#[derive(Clone, Debug)]
pub struct Equalised {
    /// The symbols.
    pub cells: Arc<[Complex32]>,
    /// Their variances.
    pub var: Arc<[f32]>,
}

/// What `dtmb.body` gives per frame.
#[derive(Clone, Debug, Default)]
pub struct BodyFrame {
    /// The grid this frame belongs to (a new grid starts everything after it afresh).
    pub grid: u32,
    /// The frame's number in the grid.
    pub index: u64,
    /// The header.
    pub header: Option<Header>,
    /// The frame is to be erased: the tracker sent no symbols, or its closing header is silent or not usable.
    pub erase: bool,
    /// Frames lost before this one, to be erased too.
    pub lost: u32,
    /// The closing header was exact silence.
    pub silent: bool,
    /// The body as C = 3780.
    pub multi: Option<Equalised>,
    /// The body as C = 1.
    pub single: Option<Equalised>,
}

impl Item for BodyFrame {
    const KIND: &'static str = "dtmb.body_frame";
}

/// The equaliser work buffers and the transforms.
struct Work {
    fft3780: Fft,
    fft4096: Fft,
    ifft4096: Fft,
    b: Vec<Complex32>,
    x: Vec<Complex32>,
    h: Vec<Complex32>,
    chips_a: Vec<f32>,
    chips_b: Vec<f32>,
    out: Vec<Complex32>,
    var: Vec<f32>,
}

/// `dtmb.body`. Input `in`: [`FrameSyms`] from `dtmb.track`. Input `control`: `forms` from `dtmb.si`. Output `out`: one
/// [`BodyFrame`] per frame.
pub struct Body {
    forms: Forms,
    grid: u32,
    est: Option<Estimator>,
    window: (usize, usize),
    since_wide: u32,
    /// The number of the header whose fit `fit_next` holds (the one that opens the next frame).
    next_a: Option<u64>,
    fit_a: Fit,
    fit_b: Fit,
    fit_next: Fit,
    fit_wide: Fit,
    gains: [VecDeque<f32>; 2],
    work: Work,
    cells: SlicePool<Complex32>,
    vars: SlicePool<f32>,
}

impl Default for Body {
    fn default() -> Self {
        Body {
            forms: Forms::Both,
            grid: u32::MAX,
            est: None,
            window: (24, 100),
            since_wide: 0,
            next_a: None,
            fit_a: Fit::default(),
            fit_b: Fit::default(),
            fit_next: Fit::default(),
            fit_wide: Fit::default(),
            gains: Default::default(),
            work: Work {
                fft3780: Fft::forward(BODY),
                fft4096: Fft::forward(4096),
                ifft4096: Fft::inverse(4096),
                b: Vec::with_capacity(BODY + 512),
                x: vec![Complex32::new(0.0, 0.0); 4096],
                h: vec![Complex32::new(0.0, 0.0); 4096],
                chips_a: vec![0.0; 945],
                chips_b: vec![0.0; 945],
                out: vec![Complex32::new(0.0, 0.0); BODY],
                var: vec![0.0; BODY],
            },
            cells: SlicePool::default(),
            vars: SlicePool::default(),
        }
    }
}

impl Body {
    /// Forgets everything of the last grid.
    fn reset(&mut self, grid: u32) {
        self.grid = grid;
        self.next_a = None;
        self.since_wide = 0;
        self.gains = Default::default();
        self.forms = Forms::Both;
    }
}

/// The tracking window after a fit of the widest window `wide`: the extent of the taps above `max(peak x 1e-4, 12 var)`.
fn adapt(kind: Header, window: (usize, usize), wide: &Fit) -> (usize, usize) {
    let peak = wide.taps.iter().map(|h| h.norm_sqr()).fold(0f32, f32::max);
    let floor = (peak * 1e-4).max(12.0 * wide.var);
    let mut lags = wide
        .taps
        .iter()
        .enumerate()
        .filter(|(_, h)| h.norm_sqr() > floor)
        .map(|(i, _)| i as isize - wide.pre as isize);
    let Some(first) = lags.next() else {
        return window;
    };
    let last = lags.next_back().unwrap_or(first);
    let (wpre, wpost, cap) = widest(kind);
    let pre = (((8 - first).max(0) as usize).div_ceil(8) * 8).clamp(8, wpre);
    let post = (((last + 24).max(0) as usize).div_ceil(16) * 16).clamp(32, wpost);
    // Growth is followed at once; a shrink only of 8 (pre) or 16 (post) or more.
    let (cpre, cpost) = window;
    let mut pre = if pre > cpre || pre + 8 <= cpre {
        pre
    } else {
        cpre
    };
    let mut post = if post > cpost || post + 16 <= cpost {
        post
    } else {
        cpost
    };
    if pre + post > cap {
        let s = cap as f64 / (pre + post) as f64;
        pre = (pre as f64 * s) as usize;
        post = (post as f64 * s) as usize;
    }
    (pre.max(1), post.max(1))
}

impl Body {
    /// One frame.
    fn frame(&mut self, fs: &FrameSyms) -> BodyFrame {
        if fs.grid != self.grid {
            self.reset(fs.grid);
        }
        let kind = fs.header;
        if self.est.as_ref().is_none_or(|e| e.kind != kind) {
            self.est = Some(Estimator::new(kind));
            self.window = tracking(kind);
            self.next_a = None;
        }
        let mut out = BodyFrame {
            grid: fs.grid,
            index: fs.index,
            header: Some(kind),
            erase: true,
            lost: fs.lost,
            ..Default::default()
        };
        let l = kind.len();
        let syms: &[Complex32] = &fs.syms;
        let (Some((pa, pb)), true) = (fs.phases, syms.len() >= 2 * l + BODY) else {
            self.next_a = None;
            return out;
        };
        let y_b = &syms[l + BODY..2 * l + BODY];
        if is_silent(y_b) {
            out.silent = true;
            self.next_a = None;
            return out;
        }
        let Some(est) = self.est.as_mut() else {
            return out;
        };
        let (pre, post) = self.window;
        est.fit(y_b, pb, pre, post, &mut self.fit_b);
        self.since_wide += 1;
        if self.since_wide >= 32 {
            self.since_wide = 0;
            let (wpre, wpost, _) = widest(kind);
            est.fit(y_b, pb, wpre, wpost, &mut self.fit_wide);
            self.window = adapt(kind, self.window, &self.fit_wide);
        }
        if self.next_a.take() == Some(fs.index) {
            std::mem::swap(&mut self.fit_a, &mut self.fit_next);
        } else {
            est.fit(&syms[..l], pa, pre, post, &mut self.fit_a);
        }
        self.fit_next.copy_from(&self.fit_b);
        self.next_a = Some(fs.index + 1);
        if !self.fit_b.usable {
            return out;
        }
        if !self.fit_a.usable {
            self.fit_a.copy_from(&self.fit_b);
        }
        let (fit_a, fit_b) = (&self.fit_a, &self.fit_b);
        let w = &mut self.work;
        est.seq.chips(pa, &mut w.chips_a[..l]);
        est.seq.chips(pb, &mut w.chips_b[..l]);
        let amp = est.amp;
        // The body with the headers' leakage removed, from -pre to 3780 + post.
        let bpre = fit_a.pre.max(fit_b.pre);
        let bpost = fit_a.post.max(fit_b.post);
        w.b.clear();
        w.b.extend_from_slice(&syms[l - bpre..l + BODY + bpost]);
        for (j, h) in fit_a.taps.iter().enumerate() {
            if h.norm_sqr() == 0.0 {
                continue;
            }
            let lag = j as isize - fit_a.pre as isize;
            let g = h * amp;
            for tau in -(bpre as isize)..lag {
                let c = w.chips_a[(tau + l as isize - lag) as usize];
                w.b[(tau + bpre as isize) as usize] -= g * c;
            }
        }
        for (j, h) in fit_b.taps.iter().enumerate() {
            if h.norm_sqr() == 0.0 {
                continue;
            }
            let lag = j as isize - fit_b.pre as isize;
            let g = h * amp;
            let start = (BODY as isize + lag).max(-(bpre as isize));
            for tau in start..(BODY + bpost) as isize {
                let i = tau - BODY as isize - lag;
                if i >= l as isize {
                    break;
                }
                w.b[(tau + bpre as isize) as usize] -= g * w.chips_b[i as usize];
            }
        }
        // The noise and the change of the channel between the two headers.
        let lo = -(bpre as isize);
        let hi = bpost as isize;
        let drift: f32 = (lo..hi)
            .map(|m| (fit_a.tap(m) - fit_b.tap(m)).norm_sqr())
            .sum();
        let sigma2 = 0.5 * (fit_a.noise + fit_b.noise) + 0.25 * drift;
        let want_multi = kind != Header::Pn595 || self.forms != Forms::Single;
        let want_single = kind == Header::Pn595 && self.forms != Forms::Multi;
        if want_multi {
            equalise_multi(w, fit_a, fit_b, bpre, bpost, sigma2);
            out.multi = Some(self.finish(0));
        }
        if want_single {
            equalise_single(
                &mut self.work,
                &self.fit_a,
                &self.fit_b,
                bpre,
                bpost,
                sigma2,
            );
            out.single = Some(self.finish(1));
        }
        out.erase = false;
        out
    }

    /// Divides the equalised body in `work` by the median body gain of the form's last 15 frames and hands it out.
    fn finish(&mut self, form: usize) -> Equalised {
        let w = &mut self.work;
        let data = &w.out[SI..BODY];
        let p: f32 = data.iter().map(|c| c.norm_sqr()).sum::<f32>() / data.len() as f32;
        let v: f32 = w.var[SI..BODY].iter().map(|&v| v.min(4.0)).sum::<f32>() / data.len() as f32;
        let g = (p - v).max(0.05).sqrt();
        let gains = &mut self.gains[form];
        gains.push_back(g);
        while gains.len() > 15 {
            gains.pop_front();
        }
        let mut sorted = [0f32; 15];
        let n = gains.len();
        for (s, &x) in sorted.iter_mut().zip(gains.iter()) {
            *s = x;
        }
        sorted[..n].sort_unstable_by(f32::total_cmp);
        let gain = sorted[n / 2];
        let (inv, inv2) = (1.0 / gain, 1.0 / (gain * gain));
        let out = &w.out;
        let var = &w.var;
        Equalised {
            cells: self.cells.exact(BODY, |c| {
                for (c, &o) in c.iter_mut().zip(out.iter()) {
                    *c = o * inv;
                }
            }),
            var: self.vars.exact(BODY, |c| {
                for (c, &v) in c.iter_mut().zip(var.iter()) {
                    *c = (v * inv2).max(1e-6);
                }
            }),
        }
    }
}

/// C = 3780: the linear convolution made circular by folding the samples after and before the body onto it, a transform of
/// 3780 points, zero forcing by the transform of the mean taps. The result goes to `w.out` and `w.var` in stream order.
fn equalise_multi(w: &mut Work, a: &Fit, b: &Fit, pre: usize, post: usize, sigma2: f32) {
    let x = &mut w.x[..BODY];
    x.copy_from_slice(&w.b[pre..pre + BODY]);
    for n in 0..post {
        x[n] += w.b[pre + BODY + n];
    }
    for n in 0..pre {
        x[BODY - pre + n] += w.b[n];
    }
    w.fft3780.process(x);
    let s = 1.0 / (BODY as f32).sqrt();
    let h = &mut w.h[..BODY];
    h.fill(Complex32::new(0.0, 0.0));
    for m in -(pre as isize)..post as isize {
        h[m.rem_euclid(BODY as isize) as usize] = 0.5 * (a.tap(m) + b.tap(m));
    }
    w.fft3780.process(h);
    let map = carriers();
    let mut put = |k: usize, i: usize| {
        let hk = h[k];
        let p = hk.norm_sqr().max(1e-9);
        w.out[i] = x[k] * s * hk.conj() / p;
        w.var[i] = sigma2 / p;
    };
    for (i, &k) in map.si.iter().enumerate() {
        put(k as usize, i);
    }
    for (i, &k) in map.data.iter().enumerate() {
        put(k as usize, SI + i);
    }
}

/// C = 1: the samples from -pre to 3780 + post - 2 placed at `t mod 4096`, a 4096-point transform, minimum mean square error
/// by the transform of the mean taps, the bias removed. The result goes to `w.out` and `w.var`.
fn equalise_single(w: &mut Work, a: &Fit, b: &Fit, pre: usize, post: usize, sigma2: f32) {
    const N: usize = 4096;
    w.x.fill(Complex32::new(0.0, 0.0));
    let span = BODY + pre + post - 1;
    for i in 0..span.min(w.b.len()) {
        let t = i as isize - pre as isize;
        w.x[t.rem_euclid(N as isize) as usize] = w.b[i];
    }
    w.fft4096.process(&mut w.x);
    w.h.fill(Complex32::new(0.0, 0.0));
    for m in -(pre as isize)..post as isize {
        w.h[m.rem_euclid(N as isize) as usize] = 0.5 * (a.tap(m) + b.tap(m));
    }
    w.fft4096.process(&mut w.h);
    let lambda = (sigma2 * span as f32 / BODY as f32).max(1e-9);
    let mut g = 0f32;
    for (x, h) in w.x.iter_mut().zip(&w.h) {
        let p = h.norm_sqr();
        *x *= h.conj() / (p + lambda);
        g += p / (p + lambda);
    }
    let g = (g / N as f32).max(1e-3);
    w.ifft4096.process(&mut w.x);
    let s = 1.0 / (N as f32 * g);
    for (o, x) in w.out.iter_mut().zip(&w.x[..BODY]) {
        *o = x * s;
    }
    w.var.fill((1.0 - g) / g);
}

impl Component for Body {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dtmb.body",
            vec![Port::typed::<FrameSyms>("in"), Port::command("control")],
            vec![Port::typed::<BodyFrame>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(BodySetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(BodySetting::Forms, Value::Choice(self.forms.index()))])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.est = None;
        self.reset(u32::MAX);
        Ok(vec![Format::new(Kind::typed::<BodyFrame>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let _ = apply_commands(self, io, 1);
        let n = io.items::<FrameSyms>(0).len();
        for i in 0..n {
            let (time, fs) = {
                let it = &io.items::<FrameSyms>(0)[i];
                (it.time, it.value.clone())
            };
            let value = self.frame(&fs);
            io.items_out::<BodyFrame>(0).push(Timed { time, value });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BodySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BodySetting::Forms => self.forms = FORMS.read(id, v)?,
        }
        Ok(Applied::Live)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtmb::gen::{DtmbGenParams, DtmbGenerator};
    use crate::dtmb::pn::HeaderSeq;
    use crate::dtmb::profile::{si_chips, CodeRate, Interleaver, Mapping, SI_INDICES};
    use sdrkit_algo_sim::rng::Rng;

    /// The MER (dB) of frames 3 to 6 of a generator's signal at one sample per symbol through a channel of paths
    /// `(gain, delay in symbols)` (the first path at delay 0 has gain 1) and noise at `cn` dB, equalised with the window
    /// `(pre, post)`; and whether the single-frame system-information correlation picks the right index in each.
    fn mer(
        p: &DtmbGenParams,
        cn: Option<f64>,
        echoes: &[(f32, isize)],
        window: (usize, usize),
    ) -> (f64, bool) {
        let mut g = DtmbGenerator::new(p.clone()).unwrap();
        let header = p.header_sent();
        let l = header.len();
        let mut stream = Vec::new();
        let mut sent = Vec::new();
        for _ in 0..9 {
            stream.extend(g.frame_symbols());
            sent.push(g.data().to_vec());
        }
        let mut rx = stream.clone();
        for &(gain, d) in echoes {
            for (i, r) in rx.iter_mut().enumerate() {
                let j = i as isize - d;
                if j >= 0 && (j as usize) < stream.len() {
                    *r += stream[j as usize] * gain;
                }
            }
        }
        // "No noise" is 45 dB: a header with no noise at all has one magnitude and reads as silence.
        {
            let cn = cn.unwrap_or(45.0);
            let mut rng = Rng::new(3);
            let s = (10f64.powf(-cn / 10.0) / 2.0).sqrt() as f32;
            for r in rx.iter_mut() {
                *r += Complex32::new(s * rng.normal_f32(), s * rng.normal_f32());
            }
        }
        let mut body = Body {
            grid: 0,
            est: Some(Estimator::new(header)),
            window,
            forms: if p.single_carrier {
                Forms::Single
            } else {
                Forms::Multi
            },
            ..Default::default()
        };
        let words: Vec<[f32; SI]> = SI_INDICES.map(|x| si_chips(x).unwrap()).collect();
        let seq = HeaderSeq::new(header, p.fixed_phase);
        let want = p.profile().index() - 3;
        let (mut err, mut pow, mut right) = (0f64, 0f64, true);
        for k in 0..8 {
            let at = k * (l + BODY);
            let fs = FrameSyms {
                grid: 0,
                index: k as u64,
                header,
                phases: Some((seq.phase(k), seq.phase(k + 1))),
                lost: 0,
                syms: rx[at..at + 2 * l + BODY].to_vec().into(),
            };
            let out = body.frame(&fs);
            if !(3..=6).contains(&k) {
                continue;
            }
            let e = if p.single_carrier {
                out.single
            } else {
                out.multi
            };
            let e = e.unwrap_or_else(|| {
                panic!(
                    "frame {k}: erase {} silent {} phases {:?} window {:?}",
                    out.erase, out.silent, fs.phases, body.window
                )
            });
            for (r, t) in e.cells[SI..].iter().zip(&sent[k]) {
                err += (r - t).norm_sqr() as f64;
                pow += t.norm_sqr() as f64;
            }
            let score = |w: &[f32; SI]| -> f32 {
                (0..SI)
                    .map(|i| (e.cells[i].re + e.cells[i].im) * w[i])
                    .sum()
            };
            let best =
                (0..words.len()).max_by(|&a, &b| score(&words[a]).total_cmp(&score(&words[b])));
            right &= best == Some(want);
        }
        (10.0 * (pow / err.max(1e-30)).log10(), right)
    }

    /// Header, fixed phase, mapping, rate, interleaver, C/N, echoes, window and the MER to exceed.
    type MultiCase = (
        Header,
        bool,
        Mapping,
        CodeRate,
        Interleaver,
        Option<f64>,
        &'static [(f32, isize)],
        (usize, usize),
        f64,
    );
    /// C/N, echoes, window and the MER to exceed.
    type SingleCase = (Option<f64>, &'static [(f32, isize)], (usize, usize), f64);

    fn params(
        header: Header,
        m: Mapping,
        r: CodeRate,
        il: Interleaver,
        fixed: bool,
    ) -> DtmbGenParams {
        DtmbGenParams {
            header,
            mapping: m,
            code_rate: r,
            interleaver: il,
            fixed_phase: fixed,
            ..Default::default()
        }
    }

    #[test]
    fn multi_carrier_equaliser_against_the_sent_carriers() {
        use Header::*;
        // (header, fixed, mapping, rate, interleaver, C/N, echoes, window, MER above). Delays are in symbols (7.56 Msym/s).
        let cases: [MultiCase; 9] = [
            (
                Pn945,
                false,
                Mapping::Qam64,
                CodeRate::R06,
                Interleaver::Mode1,
                None,
                &[],
                (16, 64),
                38.0,
            ),
            (
                Pn420,
                false,
                Mapping::Qam16,
                CodeRate::R04,
                Interleaver::Mode2,
                None,
                &[],
                (16, 64),
                38.0,
            ),
            (
                Pn595,
                true,
                Mapping::Qam64,
                CodeRate::R08,
                Interleaver::Mode1,
                None,
                &[],
                (16, 64),
                38.0,
            ),
            (
                Pn945,
                false,
                Mapping::Qam64,
                CodeRate::R06,
                Interleaver::Mode1,
                Some(30.0),
                &[],
                (16, 64),
                27.0,
            ),
            (
                Pn420,
                false,
                Mapping::Qam64,
                CodeRate::R06,
                Interleaver::Mode1,
                Some(25.0),
                &[],
                (16, 64),
                22.0,
            ),
            (
                Pn945,
                false,
                Mapping::Qam64,
                CodeRate::R06,
                Interleaver::Mode1,
                Some(30.0),
                &[(0.5, 30)],
                (16, 80),
                26.0,
            ),
            (
                Pn420,
                false,
                Mapping::Qam16,
                CodeRate::R06,
                Interleaver::Mode2,
                Some(30.0),
                &[(0.316, 42)],
                (16, 80),
                26.0,
            ),
            (
                Pn945,
                false,
                Mapping::Qam64,
                CodeRate::R06,
                Interleaver::Mode1,
                Some(30.0),
                &[(0.708, 144)],
                (16, 240),
                22.0,
            ),
            (
                Pn595,
                false,
                Mapping::Qam64,
                CodeRate::R06,
                Interleaver::Mode1,
                Some(28.0),
                &[(0.5, 45)],
                (16, 90),
                24.0,
            ),
        ];
        for (h, fixed, m, r, il, cn, echoes, w, want) in cases {
            let (got, right) = mer(&params(h, m, r, il, fixed), cn, echoes, w);
            assert!(
                got > want && right,
                "{h:?} {m:?} {cn:?} {echoes:?}: MER {got:.1} dB, index right {right}"
            );
        }
    }

    #[test]
    fn single_carrier_equaliser_against_the_sent_symbols() {
        // PN595, 16QAM 0.6: (C/N, echoes in symbols, window, MER above).
        let cases: [SingleCase; 5] = [
            (None, &[], (24, 100), 38.0),
            (Some(30.0), &[], (24, 100), 27.0),
            (Some(30.0), &[(0.5, 30)], (24, 100), 23.0),
            (Some(30.0), &[(0.708, -38)], (60, 90), 20.0),
            (Some(25.0), &[(0.316, 91)], (24, 150), 20.0),
        ];
        for (cn, echoes, w, want) in cases {
            let p = DtmbGenParams {
                single_carrier: true,
                ..params(
                    Header::Pn595,
                    Mapping::Qam16,
                    CodeRate::R06,
                    Interleaver::Mode1,
                    true,
                )
            };
            let (got, right) = mer(&p, cn, echoes, w);
            assert!(
                got > want && right,
                "C = 1 {cn:?} {echoes:?}: MER {got:.1} dB, index right {right}"
            );
        }
        // The transmitter: the system-information symbols equal the word, the body has unit power, the header power 1.
        let p = DtmbGenParams {
            single_carrier: true,
            ..params(
                Header::Pn595,
                Mapping::Qam16,
                CodeRate::R06,
                Interleaver::Mode1,
                true,
            )
        };
        let mut g = DtmbGenerator::new(p.clone()).unwrap();
        let mut body_p = 0f64;
        let mut head_p = 0f64;
        for _ in 0..4 {
            let f = g.frame_symbols();
            let chips = si_chips(p.profile().index()).unwrap();
            for (s, c) in f[595..595 + SI].iter().zip(chips) {
                assert!((s.re - c * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
            }
            body_p += f[595..].iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / BODY as f64 / 4.0;
            head_p += f[..595].iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / 595.0 / 4.0;
        }
        assert!(
            (body_p - 1.0).abs() < 0.05 && (head_p - 1.0).abs() < 1e-5,
            "{body_p} {head_p}"
        );
    }
}
