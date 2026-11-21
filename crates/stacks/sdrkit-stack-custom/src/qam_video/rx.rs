//! The frame receiver of `qam_video` at 4 samples a symbol: detection on the preamble, the header by a search of the residual
//! frequency and a Viterbi decoder, fine timing, a phase slope fit on the known symbols, removal of the sweep signature, and the
//! LDPC blocks with their CRC. When a block fails, recovery passes estimate the channel from the known symbols (then also from
//! the decoded data), equalise the frame with a decision-feedback equaliser, and keep the result when more blocks pass. The
//! demapping, the decoding and the recovery passes run on a helper thread behind a bounded queue.

use super::phy::{self, Header, Waveform, PREAMBLE, SPS};
use crate::family::fit::{parabola, phase_slope};
use crate::family::{self, qam};
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::interp::{Cubic, Reader};
use sdrkit_algo_fec::conv::codes::K7_171_133;
use sdrkit_algo_fec::conv::{Ends, Viterbi};
use sdrkit_algo_fec::ldpc::LdpcDecoder;
use sdrkit_algo_modem::mmse_dfe::{estimate_channel, MmseDfe};
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;
use std::sync::mpsc;

/// A frame received.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Its header.
    pub header: Header,
    /// The four cells, `None` for a block that failed.
    pub cells: Vec<Option<Vec<u8>>>,
    /// The sample of the first preamble symbol, absolute.
    pub at: f64,
    /// The carrier offset, cycles a symbol.
    pub carrier: f64,
    /// Es/N0 of the known symbols, dB.
    pub esn0_db: f64,
}

/// The frame receiver for one symbol rate.
pub struct FrameReceiver {
    /// Only frames of this symbol rate count.
    pub symbol_rate: f64,
    raw: Vec<Complex32>,
    base: u64,
    z: Vec<Complex32>,
    /// The next sample where the detection looks.
    scan: u64,
    vit: Viterbi,
    worker: Option<Worker>,
    pending: usize,
    /// The matched filter of the input, and the input samples it has taken.
    fir: Fir<Complex32>,
    fed: u64,
    mf_out: Vec<Complex32>,
    /// Frames whose header passed.
    pub frames: u64,
    /// The recovery passes with the decision-feedback equaliser on (the default).
    pub equaliser: bool,
}

const MARGIN: usize = 64;

impl FrameReceiver {
    /// A receiver of frames at `symbol_rate`.
    pub fn new(symbol_rate: f64) -> FrameReceiver {
        FrameReceiver {
            symbol_rate,
            raw: Vec::new(),
            base: 0,
            z: Vec::new(),
            scan: 0,
            vit: Viterbi::new(K7_171_133, 70).expect("header Viterbi"),
            worker: None,
            pending: 0,
            fir: Fir::new(&mf_taps()),
            fed: 0,
            mf_out: Vec::new(),
            frames: 0,
            equaliser: true,
        }
    }

    /// Forgets the stream.
    pub fn reset(&mut self) {
        self.raw.clear();
        self.z.clear();
        self.base = 0;
        self.scan = 0;
        self.fir.reset();
        self.fed = 0;
    }

    fn z_at(&self, t: i64) -> Complex32 {
        let i = t - self.base as i64;
        if i < 0 || i as usize >= self.z.len() {
            Complex32::new(0.0, 0.0)
        } else {
            self.z[i as usize]
        }
    }

    fn interp(z: &dyn Fn(i64) -> Complex32, pos: f64) -> Complex32 {
        let fl = pos.floor() as i64;
        let w = [z(fl - 1), z(fl), z(fl + 1), z(fl + 2)];
        Reader::new(Cubic).at(&w, 1.0 + (pos - fl as f64))
    }

    fn metric(&self, n: i64) -> (f64, Complex32, Complex32) {
        let p = phy::preamble();
        let (mut c1, mut c2, mut e) = (Complex32::new(0.0, 0.0), Complex32::new(0.0, 0.0), 0f32);
        for k in 0..PREAMBLE {
            let v = self.z_at(n + (SPS * k) as i64);
            e += v.norm_sqr();
            if k < 16 {
                c1 += v * p[k].conj();
            } else {
                c2 += v * p[k].conj();
            }
        }
        let m = (c1.norm() + c2.norm()).powi(2) / (32.0 * e.max(1e-20));
        (f64::from(m), c1, c2)
    }

    /// Takes samples at 4 samples a symbol and appends the frames decoded so far, in order. Frames are decoded on a helper
    /// thread, so a frame comes out some calls after its samples; [`FrameReceiver::finish`] waits for the rest.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Frame>) {
        self.raw.extend_from_slice(x);
        // The matched filter: the output after input sample m is centred on sample m - 24.
        self.mf_out.resize(x.len(), Complex32::new(0.0, 0.0));
        self.fir.process(x, &mut self.mf_out);
        let skip = 24u64.saturating_sub(self.fed).min(x.len() as u64) as usize;
        self.z.extend_from_slice(&self.mf_out[skip..]);
        self.fed += x.len() as u64;
        let longest = SPS * Waveform { profile: phy::Profile::A, robust: true }.frame_symbols() + 2 * MARGIN;
        loop {
            let zend = self.base + self.z.len() as u64;
            if self.scan + (SPS * (PREAMBLE + phy::HEADER) + MARGIN + 16) as u64 > zend {
                break;
            }
            let n = self.scan as i64;
            if self.metric(n).0 < 0.40 {
                self.scan += 1;
                continue;
            }
            let mut best = (0f64, n);
            for d in 0..=8 {
                let m = self.metric(n + d).0;
                if m > best.0 {
                    best = (m, n + d);
                }
            }
            let (mb, c1, c2) = self.metric(best.1);
            let off = parabola(self.metric(best.1 - 1).0, mb, self.metric(best.1 + 1).0);
            let start = best.1 as f64 + off;
            let w0 = f64::from((c2 * c1.conj()).arg()) / 16.0;
            match self.header(start, w0) {
                Some((h, w, ph)) => {
                    if (h.waveform.profile.symbol_rate() - self.symbol_rate).abs() > 1.0 {
                        self.scan = best.1 as u64 + 16;
                        continue;
                    }
                    let need = start as u64 + (SPS * h.waveform.frame_symbols() + MARGIN) as u64;
                    if need + 25 > zend {
                        // Wait for the rest of the frame.
                        self.scan = best.1 as u64;
                        break;
                    }
                    let f = self.frame(start, h, w, ph);
                    self.frames += 1;
                    self.scan = (start as u64) + (SPS * h.waveform.frame_symbols()) as u64 - 8;
                    self.submit(f, out);
                }
                None => self.scan = best.1 as u64 + 16,
            }
        }
        // Keep what the next detection and a whole frame need.
        let keep_from = self.scan.saturating_sub((longest + 200) as u64).max(self.base);
        let keep_from = keep_from.min(self.scan.saturating_sub(64)).max(self.base);
        let drop = (keep_from - self.base) as usize;
        if drop > 0 && drop <= self.z.len() {
            self.raw.drain(..drop.min(self.raw.len()));
            self.z.drain(..drop);
            self.base += drop as u64;
        }
        self.collect(out);
    }

    /// Decodes the header of a frame whose preamble starts at `start`: the header, the frequency and the phase found.
    fn header(&mut self, start: f64, w0: f64) -> Option<(Header, f64, f64)> {
        let p = phy::preamble();
        let zf = |t: i64| self.z_at(t);
        let y: Vec<Complex32> = (0..PREAMBLE + phy::HEADER)
            .map(|k| Self::interp(&zf, start + (SPS * k) as f64))
            .collect();
        let mut ws: Vec<f64> = vec![0.0];
        let mut k = 1;
        while k as f64 * 0.008 <= 0.1 + 1e-9 {
            ws.push(k as f64 * 0.008);
            ws.push(-(k as f64) * 0.008);
            k += 1;
        }
        for dw in ws {
            let w = w0 + dw;
            let rot = |k: usize| {
                let a = -w * k as f64;
                Complex32::new(a.cos() as f32, a.sin() as f32)
            };
            let c: Complex32 = (0..PREAMBLE).map(|k| y[k] * rot(k) * p[k].conj()).sum();
            // The common phase of the header from the squared BPSK symbols, tied to the preamble's.
            let sq: Complex32 = (PREAMBLE..PREAMBLE + phy::HEADER).map(|k| (y[k] * rot(k)).powi(2)).sum();
            let mut ph = f64::from(c.arg());
            let hp = f64::from(sq.arg()) / 2.0;
            // Of the two square roots, the one nearer the preamble phase.
            let cand = [hp, hp + std::f64::consts::PI];
            ph = *cand
                .iter()
                .min_by(|a, b| ang(**a - ph).abs().total_cmp(&ang(**b - ph).abs()))
                .unwrap_or(&ph);
            let r = Complex32::new((-ph).cos() as f32, (-ph).sin() as f32);
            let soft: Vec<f32> = (PREAMBLE..PREAMBLE + phy::HEADER).map(|k| (y[k] * rot(k) * r).re).collect();
            let mean = soft.iter().map(|v| v.abs()).sum::<f32>() / soft.len() as f32;
            let llr: Vec<i8> = soft.iter().map(|v| (v * 24.0 / mean.max(1e-12)).round().clamp(-127.0, 127.0) as i8).collect();
            let mut bits = [0u8; 64];
            if self.vit.decode(&llr, Ends::Zero, &mut bits).is_err() {
                continue;
            }
            if let Some(h) = phy::parse_header(&bits) {
                return Some((h, w, ph));
            }
        }
        None
    }

    /// Finds the timing and the carrier of the frame at `start` and takes its raw samples.
    fn frame(&mut self, start: f64, h: Header, w_hdr: f64, _ph: f64) -> Segment {
        let waveform = h.waveform;
        let fs = waveform.frame_symbols();
        let layout = phy::region_layout(waveform);
        let pil = phy::pilots();
        let p = phy::preamble();
        let header_syms: Vec<Complex32> = phy::header_coded(&h).iter().map(|&b| Complex32::new(if b == 0 { 1.0 } else { -1.0 }, 0.0)).collect();
        // The known symbols: preamble, header (now known) and pilots.
        let mut known: Vec<(usize, Complex32)> = (0..PREAMBLE).map(|k| (k, p[k])).collect();
        known.extend(header_syms.iter().enumerate().map(|(i, &v)| (PREAMBLE + i, v)));
        let r0 = PREAMBLE + phy::HEADER;
        for (i, s) in layout.iter().enumerate() {
            if let Some(n) = s {
                known.push((r0 + i, Complex32::new(pil[*n], 0.0)));
            }
        }
        // Fine timing: the shift that maximises the fit on the known symbols.
        let zf = |t: i64| self.z_at(t);
        let known_at = |tau: f64| -> Vec<(f64, Complex32)> {
            known
                .iter()
                .map(|&(k, s)| (k as f64, Self::interp(&zf, start + tau + (SPS * k) as f64) * s.conj()))
                .collect()
        };
        // The slope: the mean phase step from pilot to pilot (16 symbols, so unambiguous within +-0.19 rad a symbol), then a
        // search near it; the header's estimate when there are no pilot pairs.
        let k0 = known_at(0.0);
        let step: Complex32 = k0.windows(2).filter(|w| w[1].0 - w[0].0 == 16.0).map(|w| w[1].1 * w[0].1.conj()).sum();
        let w_pp = if step.norm() > 0.0 { f64::from(step.arg()) / 16.0 } else { w_hdr };
        let (w_all, _) = phase_slope(&k0, w_pp, 0.0025, 0.00025);
        // Fine timing: the trial shift whose known symbols line up best at that slope.
        let amp_at = |tau: f64| f64::from(phase_slope(&known_at(tau), w_all, 0.0, 1.0).1.norm());
        let mut amps = Vec::new();
        for i in -4..=4 {
            let tau = 0.1 * f64::from(i);
            amps.push((tau, amp_at(tau)));
        }
        let bi = amps.iter().enumerate().max_by(|a, b| a.1 .1.total_cmp(&b.1 .1)).map_or(4, |(i, _)| i);
        let mut tau = amps[bi].0;
        if bi > 0 && bi + 1 < amps.len() {
            tau += 0.1 * parabola(amps[bi - 1].1, amps[bi].1, amps[bi + 1].1);
        }
        let (w, sum) = phase_slope(&known_at(tau), w_all, 0.0005, 0.00005);
        let a0 = sum / known.len() as f32;
        // The raw samples of the frame go to the helper thread with the timing and the carrier found.
        let n0 = (start + tau).floor() as i64 - 30;
        let n1 = (start + tau) as i64 + (SPS * fs) as i64 + 30;
        let lo = (n0 - self.base as i64).max(0) as usize;
        let hi = ((n1 - self.base as i64).max(0) as usize).min(self.raw.len());
        let t0 = start + tau;
        Segment {
            header: h,
            raw: self.raw[lo..hi].to_vec(),
            n_first: lo as f64 + self.base as f64 - t0,
            w,
            a0,
            known,
            at: t0,
            equaliser: self.equaliser,
        }
    }
}

/// The raw samples of a frame and what the receiving thread found: the position of the first sample relative to symbol 0
/// (samples), the phase slope and the gain of the known symbols, the known symbols.
struct Segment {
    header: Header,
    raw: Vec<Complex32>,
    n_first: f64,
    w: f64,
    a0: Complex32,
    known: Vec<(usize, Complex32)>,
    at: f64,
    equaliser: bool,
}

/// Removes the carrier and the sweep signature from the raw samples of a frame, filters them and takes its symbols, with
/// the gain, the phase slope and the noise variance of its known symbols.
fn symbols(sg: Segment) -> Symbols {
    let Segment { header: h, raw, n_first, w, a0, known, at, equaliser } = sg;
    let fs = h.waveform.frame_symbols();
    // The carrier ramp by a running product, anchored again every 1024 samples.
    let mut seg: Vec<Complex32> = Vec::with_capacity(raw.len());
    let mut rot = Complex32::new(1.0, 0.0);
    let step = Complex32::new((-w / SPS as f64).cos() as f32, (-w / SPS as f64).sin() as f32);
    for (i, &v) in raw.iter().enumerate() {
        if i % 1024 == 0 {
            let n = n_first + i as f64;
            let ang = -(w * n / SPS as f64) - f64::from(a0.arg());
            rot = Complex32::new(ang.cos() as f32, ang.sin() as f32);
        }
        seg.push(v * rot);
        rot *= step;
    }
        let period = (h.waveform.sweep_frames() * fs) as f64;
        let su = f64::from(h.sweep) * fs as f64;
        // The signature: zero outside the frame's 4 fs samples.
        let mut c0 = vec![Complex32::new(0.0, 0.0); seg.len()];
        let i0 = ((-n_first).ceil().max(0.0) as usize).min(seg.len());
        let i1 = (((SPS * fs) as f64 - n_first).ceil().max(0.0) as usize).min(seg.len());
        if i1 > i0 {
            phy::chirp_fill(su + (n_first + i0 as f64) / SPS as f64, 1.0 / SPS as f64, period, &mut c0[i0..i1]);
        }
        // A small timing shift dt turns each product of sample and signature by 2 pi f dt / 4, f its frequency in cycles a
        // symbol; the trials use the series of that turn to the fourth power, so the sums of f^m times the products serve
        // every trial.
        let mut mom = [Complex32::new(0.0, 0.0); 5];
        let mut e0 = 0f32;
        let mut freq = vec![0f32; seg.len()];
        for (i, (v, c)) in seg.iter().zip(&c0).enumerate() {
            let e = c.norm_sqr();
            if e == 0.0 {
                continue;
            }
            e0 += e;
            let u = su + (n_first + i as f64) / SPS as f64;
            let u = if (0.0..period).contains(&u) { u } else { u.rem_euclid(period) };
            let f = if u < period / 2.0 { 0.45 * (1.0 - 4.0 * u / period) } else { 0.45 * (-1.0 + 4.0 * (u - period / 2.0) / period) } as f32;
            freq[i] = f;
            let w = v * c.conj();
            let mut fk = 1f32;
            for m in &mut mom {
                *m += w * fk;
                fk *= f;
            }
        }
        let e0 = e0.max(1e-20);
        let amp_of = |dt: f64| {
            let a = (TAU * dt / SPS as f64) as f32;
            let (a2, a3) = (a * a, a * a * a);
            (mom[0] + mom[1] * Complex32::new(0.0, a) - mom[2] * (a2 / 2.0) - mom[3] * Complex32::new(0.0, a3 / 6.0) + mom[4] * (a2 * a2 / 24.0)) / e0
        };
        let mut tries = Vec::new();
        for i in -5..=5 {
            let dt = 0.1 * f64::from(i);
            tries.push((dt, amp_of(dt)));
        }
        let bi = tries.iter().enumerate().max_by(|a, b| a.1 .1.norm().total_cmp(&b.1 .1.norm())).map_or(5, |(i, _)| i);
        let mut dt = tries[bi].0;
        if bi > 0 && bi + 1 < tries.len() {
            dt += 0.1
                * parabola(
                    f64::from(tries[bi - 1].1.norm()),
                    f64::from(tries[bi].1.norm()),
                    f64::from(tries[bi + 1].1.norm()),
                );
        }
        let ca = amp_of(dt);
        // Remove it only when it is there (a frame sent without the signature keeps its samples).
        if ca.norm() > 0.05 * a0.norm() {
            // The signature delayed by dt: each value turned back by 2 pi f dt / 4, by the same series.
            let a = (TAU * dt / SPS as f64) as f32;
            for ((v, c), &f) in seg.iter_mut().zip(&c0).zip(&freq) {
                let x = a * f;
                let x2 = x * x;
                *v -= ca * c * Complex32::new(1.0 - x2 / 2.0 + x2 * x2 / 24.0, -(x - x2 * x / 6.0));
            }
        }
        // Matched filter of the cleaned segment, and the symbols.
        let mut zseg = vec![Complex32::new(0.0, 0.0); seg.len()];
        Fir::new(&mf_taps()).process(&seg, &mut zseg);
        // The filter output centred on sample t of the segment.
        let zs = |t: i64| {
            let j = t + 24;
            if j >= 48 && (j as usize) < zseg.len() {
                zseg[j as usize]
            } else {
                Complex32::new(0.0, 0.0)
            }
        };
        let y: Vec<Complex32> = (0..fs).map(|k| FrameReceiver::interp(&zs, -n_first + (SPS * k) as f64)).collect();
        let v: Vec<(f64, Complex32)> = known.iter().map(|&(k, s)| (k as f64, y[k] * s.conj())).collect();
        let (w2c, _) = phase_slope(&v, 0.0, 0.002, 0.0004);
        let (w2, s2) = phase_slope(&v, w2c, 0.0006, 0.00005);
        let g = s2 / known.len() as f32;
        let gk = |k: usize| g * Complex32::new((w2 * k as f64).cos() as f32, (w2 * k as f64).sin() as f32);
        let resid: f64 = known.iter().map(|&(k, s)| f64::from((y[k] - gk(k) * s).norm_sqr())).sum::<f64>()
            / known.len() as f64
            / f64::from(g.norm_sqr()).max(1e-20);
        let nv = resid.max(1e-5) as f32;
        Symbols {
            header: h,
            y,
            known,
            w2,
            g,
            nv,
            at,
            carrier: (w + w2) / TAU,
            equaliser,
        }
}

impl FrameReceiver {

    /// Hands the samples of a frame to the decoding thread and takes the frames it has finished.
    fn submit(&mut self, s: Segment, out: &mut Vec<Frame>) {
        let w = self.worker.get_or_insert_with(Worker::spawn);
        if w.jobs.send(s).is_ok() {
            self.pending += 1;
        }
        self.collect(out);
    }

    /// Takes the frames the decoding thread has finished, in order.
    fn collect(&mut self, out: &mut Vec<Frame>) {
        if let Some(w) = &self.worker {
            while let Ok(f) = w.done.try_recv() {
                self.pending -= 1;
                out.push(f);
            }
        }
    }

    /// Waits for the decoding thread and appends every frame it still holds.
    pub fn finish(&mut self, out: &mut Vec<Frame>) {
        if let Some(w) = &self.worker {
            while self.pending > 0 {
                match w.done.recv() {
                    Ok(f) => {
                        self.pending -= 1;
                        out.push(f);
                    }
                    Err(_) => {
                        self.pending = 0;
                    }
                }
            }
        }
    }
}

/// The symbols of a frame, ready to decode: the frame's symbols, its known symbols, the gain and phase slope fitted to them,
/// and the noise variance after the gain.
struct Symbols {
    header: Header,
    y: Vec<Complex32>,
    known: Vec<(usize, Complex32)>,
    w2: f64,
    g: Complex32,
    nv: f32,
    at: f64,
    carrier: f64,
    equaliser: bool,
}

/// Frames waiting for the decoding thread; a full queue holds the receiving thread back.
const JOBS: usize = 16;

/// The thread that takes the symbols of the frames from their samples, demaps and decodes them, and runs the recovery passes.
struct Worker {
    jobs: mpsc::SyncSender<Segment>,
    done: mpsc::Receiver<Frame>,
}

impl Worker {
    fn spawn() -> Worker {
        let (jobs, job_rx) = mpsc::sync_channel::<Segment>(JOBS);
        let (done_tx, done) = mpsc::channel();
        std::thread::spawn(move || {
            let mut d = Decoding::default();
            for s in job_rx {
                if done_tx.send(d.frame(symbols(s))).is_err() {
                    break;
                }
            }
        });
        Worker { jobs, done }
    }
}

/// The decoders of the frame decoding, one for each code rate met.
#[derive(Default)]
struct Decoding {
    decs: Vec<(usize, LdpcDecoder)>,
}

impl Decoding {
    fn decoder(&mut self, rate: usize) -> &mut LdpcDecoder {
        if let Some(i) = self.decs.iter().position(|d| d.0 == rate) {
            return &mut self.decs[i].1;
        }
        let c = family::ldpc::code(phy::N, rate).expect("family code");
        self.decs.push((rate, c.decoder()));
        &mut self.decs.last_mut().expect("pushed").1
    }

    /// Decodes the frame of `s`: the one-tap path, then, when a block failed and the equaliser is on, the recovery passes.
    fn frame(&mut self, s: Symbols) -> Frame {
        let Symbols { header: h, y, known, w2, g, nv, at, carrier, equaliser } = s;
        let waveform = h.waveform;
        let fs = waveform.frame_symbols();
        let layout = phy::region_layout(waveform);
        let r0 = PREAMBLE + phy::HEADER;
        let gk = |k: usize| g * Complex32::new((w2 * k as f64).cos() as f32, (w2 * k as f64).sin() as f32);
        let data_at: Vec<usize> = layout.iter().enumerate().filter(|(_, s)| s.is_none()).map(|(i, _)| r0 + i).collect();
        let data: Vec<Complex32> = data_at.iter().map(|&k| y[k] / gk(k)).collect();
        let c = qam::constellation(waveform.bits()).expect("family map");
        let (mut cells, mut words) = self.decode_symbols(waveform, &c, &data, nv);
        let mut esn0 = -10.0 * f64::from(nv).log10();
        if equaliser && cells.iter().any(Option::is_none) {
            // The symbols with the slope taken out; the channel keeps the gain.
            let ys: Vec<Complex32> = (0..fs).map(|k| y[k] * Complex32::new((w2 * k as f64).cos() as f32, -(w2 * k as f64).sin() as f32)).collect();
            let mut train: Vec<Option<Complex32>> = vec![None; fs];
            for &(k, s) in &known {
                train[k] = Some(s);
            }
            let known_only = train.clone();
            let set = qam::point_set(waveform.bits()).expect("family map");
            for pass in 0..2 {
                if pass > 0 {
                    // The second pass also trains on the data the last decode gave.
                    let tx = phy::transmit_bits(&words);
                    let mut syms = Vec::with_capacity(data_at.len());
                    qam::map(&set, waveform.bits(), &tx, &mut syms);
                    for (&k, &s) in data_at.iter().zip(&syms) {
                        train[k] = Some(s);
                    }
                }
                let Some((eq, enoise)) = equalise(&ys, &train, &known_only, &c) else {
                    break;
                };
                let edata: Vec<Complex32> = data_at.iter().map(|&k| eq[k]).collect();
                let (c2, w2s) = self.decode_symbols(waveform, &c, &edata, enoise);
                if c2.iter().flatten().count() > cells.iter().flatten().count() {
                    cells = c2;
                    words = w2s;
                    esn0 = -10.0 * f64::from(enoise).log10();
                }
                if cells.iter().all(Option::is_some) {
                    break;
                }
            }
        }
        Frame {
            header: h,
            cells,
            at,
            carrier,
            esn0_db: esn0,
        }
    }

    /// Demaps the data symbols of a frame with noise variance `nv`, descrambles and deinterleaves, and decodes the four blocks:
    /// the cells (`None` where a block failed) and the hard code words.
    fn decode_symbols(&mut self, waveform: Waveform, c: &Constellation, data: &[Complex32], nv: f32) -> (Vec<Option<Vec<u8>>>, Vec<u8>) {
        let mut llr = vec![0f32; data.len() * waveform.bits()];
        let _ = c.demap_block(data, nv.max(1e-5), Demap::MaxLog, &mut llr);
        let perm = phy::permutation();
        let scr = phy::scrambler();
        let mut word = vec![0f32; phy::FRAME_BITS];
        for i in 0..phy::FRAME_BITS {
            let pi = perm[i] as usize;
            word[i] = if scr[pi] == 1 { -llr[pi] } else { llr[pi] };
        }
        let cell = waveform.cell();
        let k = waveform.k();
        let mut cells = Vec::with_capacity(phy::BLOCKS);
        let mut words = vec![0u8; phy::FRAME_BITS];
        for b in 0..phy::BLOCKS {
            let w = &word[b * phy::N..(b + 1) * phy::N];
            let mean = w.iter().map(|v| v.abs()).sum::<f32>() / w.len() as f32;
            let scale = 12.0 / mean.max(1e-12);
            let l: Vec<i8> = w.iter().map(|v| (v * scale).round().clamp(-127.0, 127.0) as i8).collect();
            let out = &mut words[b * phy::N..(b + 1) * phy::N];
            let ok = self.decoder(waveform.rate()).decode(&l, 50, out).is_ok();
            let bytes = family::pack(&out[..k - 16]);
            let crc = family::read_bits(out, k - 16, 16) as u16;
            cells.push((ok && family::crc16(&bytes) == crc).then(|| bytes[..cell].to_vec()));
        }
        (cells, words)
    }
}

/// The matched filter: the RRC divided by 2, so the cascade has unit gain at the symbol instant.
fn mf_taps() -> Vec<f32> {
    phy::rrc().iter().map(|v| v * 0.5).collect()
}

/// Equalises the symbols `ys` of a frame: the channel (lags -2 to 3) by least squares on the `train` symbols, a decision-feedback
/// equaliser that feeds back the `known` symbols and its own decisions elsewhere. Returns the output and its noise variance,
/// measured on the known symbols; `None` when the channel cannot be estimated.
fn equalise(ys: &[Complex32], train: &[Option<Complex32>], known: &[Option<Complex32>], c: &Constellation) -> Option<(Vec<Complex32>, f32)> {
    let (h, nv) = estimate_channel(ys, train, -2, 6).ok()?;
    let d = MmseDfe::design(&h, -2, -3, 7, nv.max(1e-5)).ok()?;
    let mut out = Vec::new();
    d.run(ys, known, |z| c.nearest(z), &mut out);
    let (mut e, mut n) = (0f64, 0usize);
    for (z, a) in out.iter().zip(known).skip(8) {
        if let Some(a) = a {
            e += f64::from((z - a).norm_sqr());
            n += 1;
        }
    }
    (n > 0).then(|| (out, ((e / n as f64) as f32).max(1e-5)))
}

fn ang(a: f64) -> f64 {
    (a + std::f64::consts::PI).rem_euclid(TAU) - std::f64::consts::PI
}
