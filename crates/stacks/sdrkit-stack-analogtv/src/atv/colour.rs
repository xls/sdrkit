//! The colour lock: it finds the colour system of the picture signal, follows the burst, identifies the SECAM lines, runs
//! the colour killer and measures the sync compression. It orders the picture decoder line by line ([`LineJob`]): the
//! rows, the ends of the fields and the restarts. It also returns the line and field-blanking scopes.
//!
//! Standard: ITU-R BT.470-6 Table 2, ITU-R BT.1700.

use super::picture::raster_of;
use super::{last, Carrier, LineInfo, LineJob, PictureReport, RowJob, Tune};
use crate::settings::{ColourChoice, COLOUR, COLOUR_ON};
use crate::systems::{secam, Colour, Raster, System, SystemId, R625};
use crate::util::{Held, Interp8, RingRead};
use sdrkit_core::{
    Choice, Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};
use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

/// One candidate burst loop of the colour search.
#[derive(Clone, Debug)]
struct BurstLoop {
    colour: Colour,
    fsc: f64,
    nominal: f64,
    trim: f64,
    /// The phase of the burst loop (radians) at sample `anchor`.
    phase: f64,
    anchor: f64,
    lines: u32,
    amp: f64,
    err2: f64,
    prev: Complex32,
    prev2: Complex32,
    vre: [f64; 2],
    v_switch: bool,
    burst_start: f64,
    burst_len: f64,
    amp_nom: f64,
    shift: f64,
}

impl BurstLoop {
    fn new(colour: Colour, anchor: f64) -> Self {
        let p = colour.params().expect("a burst colour");
        BurstLoop {
            colour,
            fsc: p.fsc,
            nominal: p.fsc,
            trim: 0.0,
            phase: 0.0,
            anchor,
            lines: 0,
            amp: 0.0,
            err2: 3.0,
            prev: Complex32::default(),
            prev2: Complex32::default(),
            vre: [0.0; 2],
            v_switch: p.v_switch,
            burst_start: p.burst_start_us,
            burst_len: p.burst_cycles / p.fsc * 1e6,
            // The nominal burst amplitude of the system table is peak to peak.
            amp_nom: p.burst_amp,
            shift: 0.0,
        }
    }
    #[inline]
    fn phase_at(&self, k: f64, fv: f64) -> f64 {
        self.phase + TAU * self.fsc * (k - self.anchor) / fv
    }
    fn set_freq(&mut self, f: f64, at: f64, fv: f64) {
        self.phase = self.phase_at(at, fv).rem_euclid(TAU);
        self.anchor = at;
        self.fsc = f;
    }
}

/// SECAM identification state.
#[derive(Clone, Debug, Default)]
struct SecamIdent {
    lead_lines: u32,
    pairs: u32,
    alt_pairs: u32,
    id_pairs: u32,
    last_lead: Option<(u64, bool)>,
    last_id: Option<(u64, bool)>,
    /// Votes that odd-counter lines are D'R (positive) or D'B (negative), this field.
    votes: f64,
    prev_votes: f64,
}

/// Settings of the colour lock.
#[derive(Clone, Debug)]
struct Opts {
    colour: ColourChoice,
    colour_on: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            colour: COLOUR.default,
            colour_on: COLOUR_ON.default,
        }
    }
}

sdrkit_core::settings! {
    /// The settings of the colour lock.
    pub enum ColourSetting {
        /// The colour system.
        Colour = COLOUR,
        /// Colour on.
        ColourOn = COLOUR_ON,
    }
}

/// The colour lock.
pub struct ColourLock {
    fv: f64,
    o: Opts,
    tune: Tune,
    epoch: u32,
    ring_v: Held<f32>,
    ring_b: Held<Complex32>,
    queue: VecDeque<LineInfo>,
    interp: Interp8,
    loops: Vec<BurstLoop>,
    search_lines: u32,
    decided: Option<Colour>,
    chosen: usize,
    locked_lines: u32,
    killer: bool,
    kill_count: u32,
    secam: SecamIdent,
    secam_how: &'static str,
    /// The settings changed the colour system: the picture decoder forgets its SECAM rows at the next pass.
    secam_restart: bool,
    burst_x: Vec<Complex32>,
    snaps: VecDeque<(u64, Vec<Complex32>)>,
    raster: Raster,
    cur_field: Option<u8>,
    compression: f64,
    sync_depth_squeeze: f64,
    video_snr: f64,
    log: Vec<String>,
    line_scope: Vec<[f32; 2]>,
    vbi_scope: Vec<[f32; 2]>,
    line_shared: Arc<[[f32; 2]]>,
    vbi_shared: Arc<[[f32; 2]]>,
    vbi_t0: Option<f64>,
}

impl Default for ColourLock {
    fn default() -> Self {
        ColourLock {
            fv: 1.0,
            o: Opts::default(),
            tune: Tune::default(),
            epoch: u32::MAX,
            ring_v: Held::new(1 << 15),
            ring_b: Held::new(1 << 15),
            queue: VecDeque::with_capacity(32),
            interp: Interp8::default(),
            loops: Vec::new(),
            search_lines: 0,
            decided: None,
            chosen: 0,
            locked_lines: 0,
            killer: false,
            kill_count: 0,
            secam: SecamIdent::default(),
            secam_how: "",
            secam_restart: false,
            burst_x: Vec::new(),
            snaps: VecDeque::new(),
            raster: R625,
            cur_field: None,
            compression: 0.0,
            sync_depth_squeeze: 0.0,
            video_snr: 0.0,
            log: Vec::new(),
            line_scope: Vec::new(),
            vbi_scope: Vec::new(),
            line_shared: Arc::from(&[][..]),
            vbi_shared: Arc::from(&[][..]),
            vbi_t0: None,
        }
    }
}

impl ColourLock {
    fn us(&self, x: f64) -> f64 {
        x * 1e-6 * self.fv
    }

    fn reset(&mut self) {
        self.snaps.clear();
        self.queue.clear();
        self.loops.clear();
        self.search_lines = 0;
        self.decided = None;
        self.locked_lines = 0;
        self.killer = false;
        self.kill_count = 0;
        self.secam = SecamIdent::default();
        self.secam_how = "";
        self.cur_field = None;
        self.vbi_t0 = None;
        self.compression = 0.0;
    }

    fn colour_capable(&self) -> bool {
        self.fv >= 9.5e6 && self.o.colour != ColourChoice::Mono
    }

    /// The candidate loops for a raster, or the forced one.
    fn start_search(&mut self, lines: u32, at: f64) {
        self.loops.clear();
        self.search_lines = 0;
        if !self.colour_capable() || lines == 405 || lines == 819 {
            return;
        }
        let positive = self
            .tune
            .system
            .map(|s| System::get(s).positive)
            .unwrap_or(false);
        let n_system = self.tune.system == Some(SystemId::N);
        let set: Vec<Colour> = match (self.o.colour, lines) {
            (ColourChoice::Pal, 525) => vec![Colour::PalM],
            (ColourChoice::Pal, _) => vec![if n_system { Colour::PalN } else { Colour::Pal }],
            (ColourChoice::Ntsc, 525) => vec![Colour::Ntsc],
            (ColourChoice::Ntsc, _) => vec![if n_system {
                Colour::NtscN
            } else {
                Colour::Ntsc443
            }],
            (ColourChoice::Secam, _) => vec![],
            (_, 525) => vec![Colour::Ntsc, Colour::PalM],
            _ => {
                if positive {
                    vec![Colour::Pal, Colour::PalN]
                } else {
                    vec![Colour::Pal, Colour::PalN, Colour::Ntsc443, Colour::NtscN]
                }
            }
        };
        self.loops = set.into_iter().map(|c| BurstLoop::new(c, at)).collect();
    }

    fn decide(&mut self, c: Colour, how: &str) {
        self.decided = Some(c);
        self.locked_lines = 0;
        self.killer = false;
        self.log.push(how.to_string());
    }

    /// The baseband at blanking: its mean over the front porch (no subcarrier there in any system).
    fn base_dc(&self, li: &LineInfo) -> Complex32 {
        let a = (li.t0 - self.us(1.15)).round() as i64;
        let b = (li.t0 - self.us(0.35)).round() as i64;
        self.ring_b.mean_of(a, b)
    }

    /// The blanking after the sync (5 to 10 µs after 0H), sampled on the line's own grid, with the line-locked part (the carrier
    /// at blanking, the sync edge's ringing through the filters and its echoes) taken out: the mean of the same span over the last
    /// eight burst lines, over which the subcarrier's phase turns full circles (PAL and NTSC alike), so the burst cancels in it.
    /// Before eight lines are in, the carrier at blanking (the front porch's mean) is taken out instead.
    fn snapshot(&mut self, li: &LineInfo) {
        let n = self.us(5.0).round() as usize;
        let t = li.t0 + self.us(5.0);
        let oldest = if self.snaps.len() >= 8 {
            self.snaps.pop_front()
        } else {
            None
        };
        let mut snap = oldest.map_or_else(|| Vec::with_capacity(n), |s| s.1);
        snap.clear();
        for j in 0..n {
            snap.push(self.interp.at(&self.ring_b, t + j as f64));
        }
        if self.snaps.back().is_some_and(|s| s.0 + 1 != li.counter) {
            self.snaps.clear();
        }
        self.snaps.push_back((li.counter, snap));
        self.burst_x.clear();
        let cur = &self
            .snaps
            .back()
            .expect("the snapshot of this line is at the back")
            .1;
        if self.snaps.len() == 8 {
            for j in 0..n {
                let mut m = Complex32::default();
                for s in &self.snaps {
                    m += s.1[j];
                }
                self.burst_x.push(cur[j] - m / 8.0);
            }
        } else {
            let dc = self.base_dc(li);
            for &c in cur.iter() {
                self.burst_x.push(c - dc);
            }
        }
    }

    /// Measures one burst against a loop's phase: (normalised complex burst, noise power, mean shift for the compression test).
    fn burst(&self, li: &LineInfo, l: &BurstLoop) -> (Complex32, f64, f64) {
        let blank_v = li.pol * li.blank;
        let norm = -li.pol * li.gain;
        if norm == 0.0 || self.burst_x.is_empty() {
            return (Complex32::default(), 0.0, 0.0);
        }
        let t5 = li.t0 + self.us(5.0);
        let idx = |us: f64| {
            ((li.t0 + self.us(us) - t5).round().max(0.0) as usize).min(self.burst_x.len())
        };
        let mix = |a: usize, b: usize| -> Complex32 {
            let (si, co) = l.phase_at(t5 + a as f64, self.fv).sin_cos();
            let mut rot = Complex32::new(co as f32, -si as f32);
            let w = TAU * l.fsc / self.fv;
            let step = Complex32::new(w.cos() as f32, -w.sin() as f32);
            let mut s = Complex32::default();
            for j in a..b {
                s += self.burst_x[j] * rot;
                rot *= step;
            }
            s / (b - a).max(1) as f32
        };
        let (a, b) = (idx(l.burst_start + 0.45), idx(l.burst_start + 1.95));
        let s = mix(a, b);
        let (na, nb) = (
            idx(l.burst_start + l.burst_len + 0.3),
            idx(l.burst_start + l.burst_len + 1.8),
        );
        let n = mix(na, nb);
        let (ka, kb) = (
            (t5 + a as f64).round() as i64,
            (t5 + b as f64).round() as i64,
        );
        let mut hann_dc = 0.0f64;
        let mut hw = 0.0f64;
        for k in ka..kb {
            let w = 0.5 - 0.5 * (TAU * (k - ka) as f64 / (kb - ka).max(1) as f64).cos();
            hann_dc += w * (self.ring_v.at(k) - blank_v) as f64;
            hw += w;
        }
        let shift = if hw > 0.0 {
            -(hann_dc / hw) / (li.pol * li.gain) as f64
        } else {
            0.0
        };
        // The interpolator's gain at the subcarrier depends on the fraction of the grid; it is divided out.
        let g = self.interp.gain(t5.fract(), TAU * l.fsc / self.fv);
        let g = if g.norm() > 0.2 {
            g
        } else {
            Complex32::new(1.0, 0.0)
        };
        (s / g / norm, (n / g / norm).norm_sqr() as f64, shift)
    }

    fn run_loops(&mut self, li: &LineInfo) {
        self.snapshot(li);
        let fh = self.fv / li.period;
        let searching = self.decided.is_none();
        for idx in 0..self.loops.len() {
            if !searching && idx != self.chosen {
                continue;
            }
            let l = self.loops[idx].clone();
            let (d, noise, shift) = self.burst(li, &l);
            let l = &mut self.loops[idx];
            let amp = ((d.norm_sqr() as f64 - noise).max(0.0)).sqrt() / l.amp_nom;
            let early = l.lines < 40;
            let (kp, kf) = if early { (0.5, 0.04) } else { (0.1, 0.004) };
            let target = Complex32::new(0.0, 1.0);
            let steer = if l.v_switch { d + l.prev } else { d };
            let err = (steer * target.conj()).arg() as f64;
            // The scatter is judged on the bursts of this line and of the line two before (the same V-switch phase in PAL), which
            // halves the noise in it, so a burst at C/N 20 dB still counts while a loop following noise stays far off.
            let dd = if l.lines >= 2 { d + l.prev2 } else { d };
            let line_err = if l.v_switch {
                let e = (dd * target.conj()).arg() as f64;
                e.abs() - PI / 4.0
            } else {
                (dd * target.conj()).arg() as f64
            };
            if amp >= 0.3 && (!l.v_switch || l.lines > 0) {
                let at = li.t0;
                l.set_freq(l.fsc, at, self.fv);
                l.phase += kp * err;
                let dtrim = kf * err * fh / TAU;
                l.trim = (l.trim + dtrim).clamp(-3e3, 3e3);
                let f = l.nominal + l.trim;
                l.set_freq(f, at, self.fv);
            }
            let w = if l.lines < 8 {
                0.4
            } else if l.v_switch {
                0.06
            } else {
                0.1
            };
            l.amp += w * (amp - l.amp);
            // A line without a burst (the field blanking before the line numbers are known) says nothing about the phase.
            if amp >= 0.3 {
                l.err2 += w * (line_err * line_err - l.err2);
            }
            l.shift += 0.1 * (shift - l.shift);
            let par = li.odd as usize;
            l.vre[par] += 0.1 * (d.re as f64 / l.amp_nom - l.vre[par]);
            l.prev2 = l.prev;
            l.prev = d;
            l.lines += 1;
        }
    }

    fn secam_measure(&mut self, li: &LineInfo) {
        let r = raster_of(li.lines);
        let g = (li.pol * li.gain).abs().max(1e-9);
        let dc = self.base_dc(li);
        let freq = |a_us: f64, b_us: f64, s: &Self| -> (f64, f64) {
            let a = (li.t0 + s.us(a_us)).round() as i64;
            let b = (li.t0 + s.us(b_us)).round() as i64;
            let w = TAU * secam::F_BELL / s.fv;
            let mut prev = Complex32::default();
            let mut acc = Complex32::default();
            let mut amp = 0.0f64;
            let mut box4 = [Complex32::default(); 8];
            for k in a..b {
                let x = s.ring_b.at(k) - dc;
                let (si, co) = (w * k as f64).sin_cos();
                let z = x * Complex32::new(co as f32, -si as f32);
                box4[(k & 7) as usize] = z;
                let m = box4.iter().fold(Complex32::default(), |s, v| s + v) * 0.125;
                if k >= a + 8 {
                    acc += m * prev.conj();
                    amp += m.norm() as f64;
                }
                prev = m;
            }
            let n = (b - a - 8).max(1) as f64;
            let f = secam::F_BELL + (acc.im as f64).atan2(acc.re as f64) * s.fv / TAU;
            (f, amp / n / g as f64)
        };
        let id_line = matches!(li.number, 7..=15 | 320..=328) && li.lines == 625;
        if id_line {
            let (f, a) = freq(
                r.blank_end_us + 6.0,
                r.blank_end_us + r.active_us - 6.0,
                self,
            );
            if a > 0.05 {
                let is_r = (f - (secam::F0R + 350e3)).abs() < 120e3;
                let is_b = (f - (secam::F0B - 350e3)).abs() < 120e3;
                if is_r || is_b {
                    let vote = if is_r == li.odd { 4.0 } else { -4.0 };
                    self.secam.votes += vote;
                    if let Some((c, rr)) = self.secam.last_id {
                        if c + 1 == li.counter && rr != is_r {
                            self.secam.id_pairs += 1;
                        }
                    }
                    self.secam.last_id = Some((li.counter, is_r));
                }
            }
        } else if r.row_of(li.number).is_some() || li.number == 0 {
            let (f, a) = freq(6.1, r.blank_end_us - 0.6, self);
            if a > 0.05 {
                let is_r = (f - secam::F0R).abs() < 45e3;
                let is_b = (f - secam::F0B).abs() < 45e3;
                if is_r || is_b {
                    self.secam.lead_lines += 1;
                    self.secam.votes += if is_r == li.odd { 1.0 } else { -1.0 };
                    if let Some((c, rr)) = self.secam.last_lead {
                        if c + 1 == li.counter {
                            self.secam.pairs += 1;
                            if rr != is_r {
                                self.secam.alt_pairs += 1;
                            }
                        }
                    }
                    self.secam.last_lead = Some((li.counter, is_r));
                }
            }
        }
    }

    fn colour_step(&mut self, li: &LineInfo) {
        if !self.colour_capable() || li.lines == 405 || li.lines == 819 {
            if self.decided.is_none() && li.lines != 0 {
                self.decide(Colour::Mono, "no colour subcarrier (monochrome)");
            }
            return;
        }
        // No burst in the field blanking, nor on the burst-blanked lines next to it (BT.470-6; the widest of the systems' ranges).
        let blanked = match li.lines {
            625 => matches!(li.number, 1..=6 | 310..=319 | 622..=625),
            525 => matches!(li.number, 1..=7 | 260..=270 | 522..=525),
            _ => false,
        };
        let vbi = li.number != 0 && (blanked || raster_of(li.lines).row_of(li.number).is_none());
        if self.loops.is_empty() && self.decided.is_none() && self.o.colour != ColourChoice::Secam {
            self.start_search(li.lines, li.t0);
        }
        if (!vbi || li.number == 0) && !self.loops.is_empty() {
            self.run_loops(li);
        }
        if self.decided.is_none() || self.decided == Some(Colour::Secam) {
            self.secam_measure(li);
        }
        if self.decided.is_none() {
            self.search_lines += 1;
            // The candidates are judged after 160 lines, and again every 20 lines while the search goes on (a weak burst's
            // scatter settles once the loops run on their slow gains).
            if self.search_lines >= 160
                && (self.search_lines - 160).is_multiple_of(20)
                && !self.loops.is_empty()
            {
                let mut text = String::from("colour search:");
                for l in &self.loops {
                    text.push_str(&format!(
                        " [{:.0} Hz: burst {:.0}%, {} lines, phase error {:.1} deg]",
                        l.fsc,
                        l.amp * 100.0,
                        l.lines,
                        l.err2.max(0.0).sqrt().to_degrees()
                    ));
                }
                if self.search_lines == 160 {
                    self.log.push(text);
                }
                let best = self
                    .loops
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| {
                        l.amp >= 0.5
                            && l.lines >= 50
                            && l.err2 < 0.4
                            && (!l.v_switch
                                || (l.vre[0].signum() != l.vre[1].signum()
                                    && l.vre[0].abs() >= 0.3 * 0.707
                                    && l.vre[1].abs() >= 0.3 * 0.707))
                    })
                    .min_by(|a, b| a.1.err2.total_cmp(&b.1.err2))
                    .map(|(i, l)| (i, l.colour));
                if let Some((i, c)) = best {
                    self.chosen = i;
                    let how = match c {
                        Colour::Pal => "PAL burst",
                        Colour::PalN => "PAL-N burst",
                        Colour::NtscN => "NTSC-N burst (3.58 MHz)",
                        Colour::Ntsc443 => "NTSC 4.43 burst",
                        Colour::PalM => "PAL-M burst",
                        _ => "NTSC burst",
                    };
                    self.decide(c, how);
                    return;
                }
            }
            let s = &self.secam;
            let by_lead =
                s.lead_lines >= 30 && s.pairs >= 20 && s.alt_pairs as f64 >= 0.85 * s.pairs as f64;
            let by_id = s.id_pairs >= 6;
            let forced_secam = self.o.colour == ColourChoice::Secam;
            let can_secam = self.search_lines >= 160 || forced_secam || self.loops.is_empty();
            if (by_lead || by_id) && can_secam {
                let how = match (by_lead, by_id) {
                    (true, true) => "lead-in on the lines and identification lines",
                    (true, false) => "lead-in on the lines",
                    _ => "identification lines of the field blanking",
                };
                self.secam_how = how;
                let text = format!(
                    "SECAM ({how}; lead-in found on {} lines, {} of {} pairs alternate, {} identification line pairs)",
                    s.lead_lines, s.alt_pairs, s.pairs, s.id_pairs
                );
                self.decide(Colour::Secam, &text);
                return;
            }
            // A weak SECAM signal that has shown some of its identification gets two frames to show the rest.
            let some_secam = s.id_pairs > 0 || s.lead_lines >= 30;
            let limit = if some_secam { 1300 } else { 700 };
            if self.search_lines >= limit && !forced_secam {
                self.decide(Colour::Mono, "no colour subcarrier (monochrome)");
            }
            return;
        }
        // Lock and killer, judged on lines with a burst.
        if vbi {
            return;
        }
        match self.decided {
            Some(Colour::Secam) => {
                self.locked_lines += 1;
                self.killer = false;
            }
            Some(Colour::Mono) | None => {}
            Some(_) => {
                if let Some(l) = self.loops.get(self.chosen) {
                    if l.amp < 0.2 || l.err2 > 1.0 {
                        self.kill_count += 1;
                        if self.kill_count > 12 && !self.killer {
                            self.killer = true;
                            self.log.push("burst too weak".into());
                        }
                    } else {
                        self.kill_count = 0;
                    }
                    if l.amp > 0.3 && l.err2 < 0.5 {
                        self.killer = false;
                    }
                    if !self.killer {
                        self.locked_lines += 1;
                    }
                }
            }
        }
    }

    fn colour_on(&self) -> bool {
        self.o.colour_on
            && self.o.colour != ColourChoice::Mono
            && matches!(self.decided, Some(c) if c != Colour::Mono)
            && !self.killer
            && self.locked_lines >= 30
    }

    fn compression_gain(&self) -> f64 {
        if self.compression > 0.0 {
            1.0 / (1.0 - self.compression)
        } else {
            1.0
        }
    }

    /// Which colour difference the odd lines carry, by the votes of this field or of the last one.
    fn odd_is_r(&self) -> Option<bool> {
        if self.secam.votes != 0.0 {
            Some(self.secam.votes > 0.0)
        } else if self.secam.prev_votes != 0.0 {
            Some(self.secam.prev_votes > 0.0)
        } else {
            None
        }
    }

    fn scopes(&mut self, li: &LineInfo) {
        let r = raster_of(li.lines);
        let norm = |s: &Self, k: f64| -> f32 {
            let v = s.ring_v.at(k.round() as i64);
            (li.blank - li.pol * v) / li.gain
        };
        if li.number == 100 || (li.number == 0 && self.line_scope.is_empty()) {
            self.line_scope.clear();
            let start = li.t0 - self.us(2.0);
            for i in 0..512 {
                let k = start + li.period * i as f64 / 512.0;
                self.line_scope
                    .push([(i as f64 / 512.0 * r.line_us() - 2.0) as f32, norm(self, k)]);
            }
            self.line_shared = Arc::from(&self.line_scope[..]);
        }
        // The vertical interval: 15 lines from 3 lines before the field sync.
        if li.number == r.lines - 2 {
            self.vbi_t0 = Some(li.t0);
        }
        if li.number == 12 {
            if let Some(t0) = self.vbi_t0.take() {
                let span = 15.0 * li.period;
                self.vbi_scope.clear();
                for i in 0..512 {
                    let a = t0 + span * i as f64 / 512.0;
                    let b = t0 + span * (i + 1) as f64 / 512.0;
                    let mut s = 0.0;
                    let mut n = 0;
                    let mut k = a.round();
                    while k < b {
                        s += norm(self, k);
                        n += 1;
                        k += 1.0;
                    }
                    self.vbi_scope
                        .push([(15.0 * i as f64 / 512.0) as f32, s / n.max(1) as f32]);
                }
                self.vbi_shared = Arc::from(&self.vbi_scope[..]);
            }
        }
    }

    /// Follows the colour on one line and orders its row, or the end of the field, from the picture decoder.
    fn process_line(&mut self, li: LineInfo, io: &mut Io) {
        if li.lines == 0 || li.gain == 0.0 {
            return;
        }
        let t = li.t0 / self.fv;
        let order = |io: &mut Io, value: LineJob| {
            io.items_out::<LineJob>(0).push(Timed { time: t, value });
        };
        let r = raster_of(li.lines);
        if r.lines != self.raster.lines {
            self.raster = r;
            self.cur_field = None;
            self.decided = None;
            self.loops.clear();
            order(io, LineJob::Raster(r.lines));
        }
        self.colour_step(&li);
        self.scopes(&li);
        if li.number == 0 {
            return;
        }
        let video_snr = self.video_snr;
        let end = |field: u8| LineJob::EndField {
            field,
            time: t,
            video_snr,
        };
        match r.row_of(li.number) {
            Some((row, f2)) => {
                let field = f2 as u8;
                if let Some(cf) = self.cur_field {
                    if cf != field {
                        order(io, end(cf));
                        if field == 0 {
                            self.secam.prev_votes = self.secam.votes;
                        }
                    }
                }
                self.cur_field = Some(field);
                let job = RowJob {
                    line: li,
                    row,
                    field,
                    decided: self.decided,
                    colour: self.colour_on(),
                    gain: self.compression_gain(),
                    carrier: self.loops.get(self.chosen).map(|l| Carrier {
                        phase: l.phase,
                        anchor: l.anchor,
                        fsc: l.fsc,
                        amp: l.amp,
                        v_switch: l.v_switch,
                        vre: l.vre,
                    }),
                    odd_is_r: self.odd_is_r(),
                    dc: self.base_dc(&li),
                    held: self.ring_v.end(),
                };
                order(io, LineJob::Row(job));
            }
            None => {
                if let Some(cf) = self.cur_field.take() {
                    order(io, end(cf));
                    self.secam.prev_votes = if self.secam.votes != 0.0 {
                        self.secam.votes
                    } else {
                        self.secam.prev_votes
                    };
                    // A field's votes are its own; the last decision stays as a prior.
                    self.secam.votes = 0.0;
                }
            }
        }
        // Sync compression from the chosen burst loop.
        if let (Some(c), Some(l)) = (self.decided, self.loops.get(self.chosen)) {
            if c != Colour::Secam && c != Colour::Mono {
                let est = l.amp + PI / 2.0 * l.shift / l.amp_nom;
                if est > 1.12 && self.sync_depth_squeeze > 0.12 {
                    self.compression = 1.0 - 1.0 / est;
                } else {
                    self.compression = 0.0;
                }
            }
        }
    }
}

impl Component for ColourLock {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.colour",
            vec![
                Port::real("video"),
                Port::iq("base"),
                Port::typed::<LineInfo>("lines"),
                Port::typed::<Tune>("control"),
                Port::typed::<super::SyncReport>("sync"),
            ],
            vec![
                Port::typed::<LineJob>("jobs").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<PictureReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(ColourSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fv = inputs[0].rate;
        let n = ((25.0 * 100e-6 * self.fv) as usize).next_power_of_two();
        let n = n.max(16);
        self.ring_v = Held::new(n);
        self.ring_b = Held::new(n);
        self.epoch = u32::MAX;
        self.secam_restart = false;
        self.reset();
        Ok(vec![
            Format::new(Kind::typed::<LineJob>(), 0.0),
            Format::new(Kind::typed::<PictureReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let now = io.input_format(0).time_of(io.input(0).first());
        if std::mem::take(&mut self.secam_restart) {
            io.items_out::<LineJob>(0).push(Timed {
                time: now,
                value: LineJob::SecamRestart,
            });
        }
        if let Some(t) = last::<Tune>(io, 3) {
            if t.epoch != self.epoch {
                self.epoch = t.epoch;
                self.reset();
                io.items_out::<LineJob>(0).push(Timed {
                    time: now,
                    value: LineJob::Restart,
                });
            }
            if t.system != self.tune.system && self.decided.is_none() {
                self.loops.clear();
            }
            self.tune = t;
        }
        if let Some(s) = last::<super::SyncReport>(io, 4) {
            if s.gain > 0.0 && s.noise_var > 0.0 {
                let sigma = s.noise_var.sqrt() / s.gain;
                self.video_snr = (20.0 * (1.0 / sigma).log10()).clamp(0.0, 60.0);
            }
            // Sync depth against the nominal, for the compression test.
            if s.tip > 0.0 && !s.positive {
                let depth = (s.tip - s.blank) / s.tip;
                let nominal = self
                    .tune
                    .system
                    .map(|id| {
                        let sys = System::get(id);
                        let m = 1.0 - sys.peak_white;
                        let sl = -sys.raster.sync_level;
                        m * sl / (1.0 + sl)
                    })
                    .unwrap_or(0.2625);
                self.sync_depth_squeeze = (1.0 - depth / nominal).max(0.0);
            }
        }
        for li in io.items::<LineInfo>(2) {
            if self.queue.len() >= 1024 {
                self.queue.pop_front();
            }
            self.queue.push_back(li.value);
        }
        let v = io.real(0);
        let b = io.iq(1);
        let step = 256;
        let mut i = 0;
        while i < v.len() || i == 0 {
            let n = step.min(v.len() - i);
            self.ring_v.extend(&v[i..i + n]);
            self.ring_b.extend(&b[i..i + n]);
            i += n;
            let end = self.ring_v.end() as f64;
            while let Some(li) = self.queue.front().copied() {
                let need = li.t0 + li.period + self.us(6.5) + 48.0;
                if end < need {
                    break;
                }
                self.queue.pop_front();
                self.process_line(li, io);
            }
            if n == 0 {
                break;
            }
        }
        if io.tick() {
            let l = self.loops.get(self.chosen);
            let secam = self.decided == Some(Colour::Secam);
            let rep = PictureReport {
                colour: self.decided,
                colour_on: self.colour_on(),
                killer: self.killer,
                burst: if secam { 0.0 } else { l.map_or(0.0, |l| l.amp) },
                burst_phase_deg: l.map_or(0.0, |l| l.err2.max(0.0).sqrt().to_degrees()),
                compression_pct: 100.0 * self.compression,
                secam_how: self.secam_how,
                log: std::mem::take(&mut self.log),
                line: Arc::clone(&self.line_shared),
                vbi: Arc::clone(&self.vbi_shared),
                video_snr_db: self.video_snr,
                ..PictureReport::default()
            };
            io.items_out::<PictureReport>(1).push(Timed {
                time: now,
                value: rep,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ColourSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ColourSetting::Colour => {
                self.o.colour = COLOUR.read(id, v)?;
                self.decided = None;
                self.loops.clear();
                self.search_lines = 0;
                self.secam = SecamIdent::default();
                self.secam_restart = true;
            }
            ColourSetting::ColourOn => self.o.colour_on = COLOUR_ON.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ColourSetting::Colour, Value::Choice(self.o.colour.index())),
            (ColourSetting::ColourOn, Value::Bool(self.o.colour_on)),
        ])
    }
}
