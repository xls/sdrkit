//! `dvbt.tps`: the TPS bit of every symbol, frame synchronisation, the parameters with BCH correction, the cell id from two
//! frames, and the count of symbols lost in a resynchronisation.
//!
//! Standard: ETSI EN 300 744 clause 4.6.

use crate::items::{Carriers, Link, TpsMessage};
use crate::params::{Mode, TpsParams};
use crate::tables::Layout;
use crate::tps;
use crate::typed::TpsFields;
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::Published;
use sdrkit_core::{Complex32, ConfigError, Results, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed};
use sdrkit_graph::{Kept, SlicePool};
use std::collections::VecDeque;
use std::f64::consts::PI;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// TPS synchronisation state.
#[derive(Clone, Debug, Default)]
struct Lock {
    /// The count of the symbol carrying s0 of a frame.
    frame_start: i64,
    /// The frame number of that frame.
    frame: u8,
    params: TpsParams,
    failures: u32,
    good_frames: u64,
    cell_hi: Option<u8>,
    cell_lo: Option<u8>,
    cell_known: bool,
    after_resync: u8,
}

impl Default for DvbtTps {
    fn default() -> Self {
        DvbtTps::new(Link::new())
    }
}

/// `dvbt.tps`.
pub struct DvbtTps {
    link: Arc<Link>,
    restart_seen: u32,
    layouts: Vec<Layout>,
    prev: Option<Carriers>,
    bits: VecDeque<(i64, u8)>,
    lock: Option<Lock>,
    held: Vec<Carriers>,
    holding: bool,
    hold_bits: Vec<(i64, u8)>,
    zeros: SlicePool<Complex32>,
    p: Published,
    /// Frames decoded, failed.
    pub frames: (u64, u64),
    /// The time of the last good frame (signal seconds).
    pub last_good: f64,
    /// What was unsupported, when anything.
    pub unsupported: Option<&'static str>,
    message: Option<TpsMessage>,
    /// The word of the last decoded frame (DVB-SH reading), for the next symbol.
    word: Option<u128>,
}

impl DvbtTps {
    /// A TPS decoder sharing `link`.
    pub fn new(link: Arc<Link>) -> Self {
        DvbtTps {
            restart_seen: link.restart.load(Ordering::Relaxed),
            link,
            layouts: Vec::new(),
            prev: None,
            bits: VecDeque::new(),
            lock: None,
            held: Vec::new(),
            holding: false,
            hold_bits: Vec::new(),
            zeros: SlicePool::default(),
            p: Published::new(TpsFields::SHAPE.fields),
            frames: (0, 0),
            last_good: 0.0,
            unsupported: None,
            message: None,
            word: None,
        }
    }

    /// The parameters, once locked.
    pub fn params(&self) -> Option<TpsParams> {
        self.lock.as_ref().map(|l| l.params)
    }

    fn layout(&mut self, n: usize) -> &Layout {
        let m = Mode::from_fft(n).unwrap_or(Mode::K2);
        let i = match self.layouts.iter().position(|l| l.mode == m) {
            Some(i) => i,
            None => {
                self.layouts.push(Layout::new(m));
                self.layouts.len() - 1
            }
        };
        &self.layouts[i]
    }

    fn unlock(&mut self) {
        self.lock = None;
        self.word = None;
        self.bits.clear();
        self.holding = false;
        self.held.clear();
        self.link.tps_locked.store(false, Ordering::Relaxed);
        if self.link.state.load(Ordering::Relaxed) >= 2 {
            self.link.state.store(1, Ordering::Relaxed);
            self.link.detect.store(1, Ordering::Relaxed);
        }
    }

    /// The TPS bit of `c` against the previous symbol, and the differential point.
    fn bit(&mut self, c: &Carriers) -> Option<(u8, Complex32)> {
        let prev = self.prev.as_ref()?;
        if prev.count + 1 != c.count || prev.cells.len() != c.cells.len() {
            return None;
        }
        let n = c.fft as f64;
        let k = c.cells.len();
        let half = (k as i64 - 1) / 2;
        let d = c.moved as f64;
        let s = c.int_offset as i64;
        let (cells, pcells) = (c.cells.clone(), prev.cells.clone());
        let lay = self.layout(c.fft);
        let rot = |kk: usize| {
            let f = (kk as i64 - half + s) as f64;
            let a = 2.0 * PI * f * d / n;
            Complex32::new(a.cos() as f32, a.sin() as f32)
        };
        let mut cp = Complex32::default();
        for &kk in &lay.continual {
            cp += cells[kk] * (pcells[kk] * rot(kk)).conj();
        }
        let mut dd = Complex32::default();
        for &kk in &lay.tps {
            dd += cells[kk] * (pcells[kk] * rot(kk)).conj();
        }
        let turn = if cp.norm() > 0.0 {
            cp.conj() / cp.norm()
        } else {
            Complex32::new(1.0, 0.0)
        };
        let z = dd * turn;
        let pt = if dd.norm() > 0.0 { z / dd.norm() } else { z };
        Some(((z.re < 0.0) as u8, pt))
    }

    /// The scattered-pilot phase (l mod 4) a symbol carries, when it is clear.
    fn pilot_phase(&mut self, c: &Carriers) -> Option<usize> {
        let cells = c.cells.clone();
        let lay = self.layout(c.fft);
        let mut scores = [0f32; 4];
        for (h, sc) in scores.iter_mut().enumerate() {
            let mut acc = Complex32::default();
            let mut mag = 0f32;
            let mut k = 3 * h;
            while k + 12 < lay.k {
                let a = cells[k] * lay.pilot[k].signum();
                let b = cells[k + 12] * lay.pilot[k + 12].signum();
                let z = a * b.conj();
                acc += z;
                mag += z.norm();
                k += 12;
            }
            *sc = if mag > 0.0 { acc.norm() / mag } else { 0.0 };
        }
        let mut idx: Vec<usize> = (0..4).collect();
        idx.sort_by(|a, b| scores[*b].total_cmp(&scores[*a]));
        (scores[idx[0]] >= 0.5 && scores[idx[0]] >= 1.5 * scores[idx[1]]).then_some(idx[0])
    }

    fn search(&mut self, c: &Carriers) -> bool {
        let n = self.bits.len();
        if n < 68 {
            return false;
        }
        let guard = c.guard;
        for back in 0..(n - 67).min(140) {
            let j = n - 68 - back;
            let c0 = self.bits[j].0;
            if self.bits[j + 67].0 != c0 + 67 {
                continue;
            }
            let mut b = [0u8; 68];
            for (i, x) in b.iter_mut().enumerate() {
                *x = self.bits[j + i].1;
            }
            match tps::decode_reading(&b, false, self.sh()) {
                Ok(blk)
                    if blk.params.mode.fft() == c.fft
                        && blk.params.guard.samples(c.fft) == guard =>
                {
                    let mut l = Lock {
                        frame_start: c0,
                        frame: blk.frame,
                        params: blk.params,
                        ..Default::default()
                    };
                    if blk.frame % 2 == 0 {
                        l.cell_hi = Some(blk.cell_half);
                    } else {
                        l.cell_lo = Some(blk.cell_half);
                    }
                    l.params.cell_id = 0;
                    // Move on to the frame that holds the current symbol.
                    let frames = (c.count as i64 - c0).div_euclid(68);
                    l.frame_start += 68 * frames;
                    l.frame = ((l.frame as i64 + frames).rem_euclid(4)) as u8;
                    self.lock = Some(l);
                    self.link.tps_locked.store(true, Ordering::Relaxed);
                    self.link.state.store(2, Ordering::Relaxed);
                    self.link.detect.store(2, Ordering::Relaxed);
                    self.unsupported = None;
                    self.p
                        .results_mut()
                        .log(format!("TPS locked: {}", blk.params.text()));
                    return true;
                }
                Err(tps::TpsError::Unsupported(w)) => self.unsupported = Some(w),
                _ => {}
            }
        }
        false
    }

    /// The frame check at symbol 67.
    fn frame_check(&mut self, time: f64) {
        let Some(l) = self.lock.as_ref() else { return };
        let start = l.frame_start;
        let mut b = [0u8; 68];
        let mut have = 0;
        for &(cnt, bit) in self.bits.iter() {
            if cnt >= start && cnt < start + 68 {
                b[(cnt - start) as usize] = bit;
                have += 1;
            }
        }
        let ok = if have == 68 {
            tps::decode_reading(&b, true, self.sh()).ok()
        } else {
            None
        };
        let Some(l) = self.lock.as_mut() else { return };
        match ok {
            Some(blk) => {
                self.frames.0 += 1;
                self.last_good = time;
                l.failures = 0;
                l.after_resync = 0;
                l.good_frames += 1;
                let mut p = blk.params;
                if self.link.sh.load(Ordering::Relaxed) {
                    self.word = Some(blk.word);
                }
                if blk.frame % 2 == 0 {
                    l.cell_hi = Some(blk.cell_half);
                } else {
                    l.cell_lo = Some(blk.cell_half);
                }
                if let (Some(h), Some(lo)) = (l.cell_hi, l.cell_lo) {
                    p.cell_id = (h as u16) << 8 | lo as u16;
                    l.cell_known = true;
                } else {
                    p.cell_id = l.params.cell_id;
                }
                if p != l.params {
                    self.p.results_mut().log(format!("TPS: {}", p.text()));
                }
                l.params = p;
                l.frame = blk.frame;
            }
            None => {
                self.frames.1 += 1;
                l.failures += 1;
                if l.after_resync > 0 {
                    l.after_resync += 1;
                    if l.after_resync >= 3 {
                        // The second frame after a resynchronisation failed too: TPS synchronisation starts again.
                        self.p
                            .results_mut()
                            .log("TPS: frame sync lost after a resynchronisation".to_string());
                        self.unlock();
                        return;
                    }
                }
                if l.failures >= 3 {
                    self.p
                        .results_mut()
                        .log("TPS: three frames failed, mode detection again".to_string());
                    self.unlock();
                    self.link.request_restart();
                    return;
                }
            }
        }
        let Some(l) = self.lock.as_mut() else { return };
        l.frame_start += 68;
        l.frame = (l.frame + 1) & 3;
        self.message = Some(TpsMessage {
            params: l.params,
            frame: l.frame,
            symbol: 0,
            cell_id_known: l.cell_known,
        });
    }

    fn erased(&mut self, like: &Carriers, l: usize, frame: u8, params: TpsParams) -> Carriers {
        let k = like.cells.len();
        let cells = self.zeros.exact(k, |v| v.fill(Complex32::default()));
        Carriers {
            cells,
            l,
            frame,
            params: Some(params),
            erased: true,
            collapsed: true,
            tps_word: None,
            ..like.clone()
        }
    }

    fn place(&self, count: u64) -> (usize, u8) {
        let lk = self
            .lock
            .as_ref()
            .expect("a symbol is placed only while the TPS lock is held");
        let d = count as i64 - lk.frame_start;
        let l = d.rem_euclid(68) as usize;
        let frames = d.div_euclid(68);
        (l, ((lk.frame as i64 + frames).rem_euclid(4)) as u8)
    }

    /// Whether TPS is read with the DVB-SH fields.
    fn sh(&self) -> bool {
        self.link.sh.load(Ordering::Relaxed)
    }

    /// Estimates losses of more than three symbols from the TPS bits since the resynchronisation: the extra loss (a multiple of 4).
    /// `None` in the DVB-SH reading, whose interleaver bits are not part of the parameters.
    fn tps_loss(&self) -> Option<i64> {
        if self.sh() {
            return None;
        }
        let lk = self.lock.as_ref()?;
        let mut best: Vec<(usize, i64)> = Vec::new();
        for extra in (0..=64).step_by(4) {
            let mut mism = 0;
            for &(cnt, bit) in self.hold_bits.iter().skip(1) {
                let d = cnt + extra - lk.frame_start;
                let l = d.rem_euclid(68) as usize;
                if l == 0 {
                    continue;
                }
                let frame = ((lk.frame as i64 + d.div_euclid(68)).rem_euclid(4)) as u8;
                let exp = tps::encode(&lk.params, frame);
                if exp[l] != bit {
                    mism += 1;
                }
            }
            best.push((mism, extra));
        }
        best.sort();
        let (m0, x0) = best[0];
        let m1 = best[1].0;
        (x0 != 0 && m0 <= 2 && m1 >= m0 + 4).then_some(x0)
    }
}

impl Component for DvbtTps {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt.tps",
            vec![Port::typed::<Carriers>("in")],
            vec![
                Port::typed::<Carriers>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<TpsMessage>("tps").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.prev = None;
        self.unlock();
        Ok(vec![
            Format::new(Kind::typed::<Carriers>(), 0.0),
            Format::new(Kind::typed::<TpsMessage>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        // A new mode detection was requested: TPS synchronisation restarts with it.
        let r = self.link.restart.load(Ordering::Relaxed);
        if r != self.restart_seen {
            self.restart_seen = r;
            if self.lock.is_some() {
                self.unlock();
            }
            self.prev = None;
        }
        let mut out = std::mem::take(io.items_out::<Carriers>(0));
        for item in io.items::<Carriers>(0) {
            let c = &item.value;
            if c.fresh {
                self.unlock();
                self.prev = None;
            }
            let bit = self.bit(c);
            let mut cc = c.clone();
            if let Some((b, pt)) = bit {
                cc.tps_point = pt;
                self.bits.push_back((c.count as i64, b));
                if self.bits.len() > 4000 {
                    self.bits.drain(..2000);
                }
                if self.holding {
                    self.hold_bits.push((c.count as i64, b));
                }
            }
            if self.lock.is_none() {
                if bit.is_some() && self.search(c) {
                    // Locked: the symbols from this one on go out.
                } else {
                    self.prev = Some(c.clone());
                    continue;
                }
            }
            // A resynchronisation: count lost symbols from the scattered pilots.
            if c.resynced {
                let (l_exp, _) = self.place(c.count);
                if let Some(h) = self.pilot_phase(c) {
                    let lost = (h as i64 - (l_exp % 4) as i64).rem_euclid(4);
                    if lost > 0 {
                        let lk = self
                            .lock
                            .as_mut()
                            .expect("a symbol is placed only while the TPS lock is held");
                        lk.frame_start -= lost;
                        let (l0, _) = self.place(c.count);
                        for i in (1..=lost).rev() {
                            let pl = (l0 as i64 - i).rem_euclid(68) as usize;
                            let (_, fr) = self.place(c.count);
                            let params = self
                                .lock
                                .as_ref()
                                .expect("a symbol is placed only while the TPS lock is held")
                                .params;
                            let e = self.erased(c, pl, fr, params);
                            out.push(Timed {
                                time: item.time,
                                value: e,
                            });
                        }
                    }
                }
                if let Some(lk) = self.lock.as_mut() {
                    lk.after_resync = 1;
                }
                self.holding = true;
                self.hold_bits.clear();
                self.held.clear();
            }
            let (l, frame) = self.place(c.count);
            let params = self
                .lock
                .as_ref()
                .expect("a symbol is placed only while the TPS lock is held")
                .params;
            cc.l = l;
            cc.frame = frame;
            cc.params = Some(params);
            cc.tps_word = self.word.take();
            if self.holding {
                self.held.push(cc);
                if self.held.len() >= 24 {
                    // More than three symbols lost: the TPS bits tell how many (in multiples of four).
                    let extra = self.tps_loss();
                    self.holding = false;
                    let mut held = std::mem::take(&mut self.held);
                    if let Some(x) = extra {
                        let first = held[0].clone();
                        self.lock
                            .as_mut()
                            .expect("a symbol is placed only while the TPS lock is held")
                            .frame_start -= x;
                        let (l0, _) = self.place(first.count);
                        for i in (1..=x).rev() {
                            let pl = (l0 as i64 - i).rem_euclid(68) as usize;
                            let e = self.erased(&first, pl, frame, params);
                            out.push(Timed {
                                time: item.time,
                                value: e,
                            });
                        }
                        for h in held.iter_mut() {
                            let (l, fr) = self.place(h.count);
                            h.l = l;
                            h.frame = fr;
                        }
                        self.p.results_mut().log(format!("{x} more symbols lost"));
                    }
                    for h in held {
                        let t = h.time;
                        out.push(Timed { time: t, value: h });
                    }
                }
            } else {
                out.push(Timed {
                    time: item.time,
                    value: cc,
                });
            }
            if l == 67 {
                self.frame_check(c.time);
                if self.lock.is_none() {
                    self.prev = Some(c.clone());
                    continue;
                }
            }
            self.prev = Some(c.clone());
        }
        // The held symbols keep their computed frame position (later frame checks do not move them).
        *io.items_out::<Carriers>(0) = out;
        if let Some(m) = self.message.take() {
            io.items_out::<TpsMessage>(1).push(Timed {
                time: 0.0,
                value: m,
            });
        }
        if let Ok(mut t) = self.link.telemetry.lock() {
            t.tps = self.lock.as_ref().map(|l| {
                let (sl, fr) = self
                    .prev
                    .as_ref()
                    .map(|p| {
                        let d = p.count as i64 - l.frame_start;
                        (
                            d.rem_euclid(68) as usize,
                            ((l.frame as i64 + d.div_euclid(68)).rem_euclid(4)) as u8,
                        )
                    })
                    .unwrap_or((0, l.frame));
                (l.params, fr, sl)
            });
            t.cell_known = self.lock.as_ref().is_some_and(|l| l.cell_known);
            t.tps_frames = self.frames;
            t.tps_good_time = self.last_good;
            t.unsupported = self.unsupported;
        }
        if self.lock.is_some() {
            self.p.put(&TpsFields {
                tps_frames_ok: self.frames.0 as i64,
                tps_frames_failed: self.frames.1 as i64,
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    /// The counts are of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.frames = (0, 0);
        self.p.results_mut().fields_mut().clear();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}
