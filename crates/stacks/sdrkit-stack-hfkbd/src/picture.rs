//! Pictures: the unwrapped-phase ring the picture modes read tones from, a picture being received with its results, the
//! Shortwave Radiogram picture header and reader (MFSK), and a test picture.

use crate::decoder::Emit;
use crate::typed::{PictureEvent, PictureFields};
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_codec_image::history::PictureHistory;
use sdrkit_codec_image::{PixelFormat, Raster};
use sdrkit_core::interface::EventKind;
use sdrkit_core::{Complex32, Event};
use sdrkit_graph::Outbox;
use std::f64::consts::TAU;

/// A picture of a picture mode, being received or finished: its pixels (BGRA) and what the results show of it.
#[derive(Clone, Debug)]
pub struct HfPicture {
    /// 1, 2, 3 ... for each stack, growing.
    pub id: u64,
    /// The name of the picture mode ("Robot 36", "MFSK32").
    pub mode: &'static str,
    /// Rows complete.
    pub rows: u32,
    /// All rows are in.
    pub complete: bool,
    /// Signal time of its start, seconds.
    pub start: f64,
    /// A colour picture.
    pub colour: bool,
    /// Samples done and in all (the progress).
    pub progress: [u64; 2],
    /// The pixels.
    pub raster: Raster,
}

impl HfPicture {
    /// A black picture of `width` by `height`.
    pub fn new(id: u64, mode: &'static str, width: u32, height: u32, start: f64) -> Self {
        HfPicture {
            id,
            mode,
            rows: 0,
            complete: false,
            start,
            colour: false,
            progress: [0, 0],
            raster: Raster::new(PixelFormat::Bgra, width.max(1), height.max(1))
                .expect("a BGRA picture of the sizes these stacks send"),
        }
    }
    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.raster.width()
    }
    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.raster.height()
    }
    /// Sets pixel (x, y) to (r, g, b).
    pub fn put(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8) {
        self.raster.put(x as i64, y as i64, &[b, g, r, 255]);
    }
    /// The standard picture item of it (BGRA pixels), named after the picture mode, in the state `state`. The number
    /// of the item is the number of the picture.
    pub fn to_picture(&self, state: sdrkit_graph::PictureState) -> sdrkit_graph::Picture {
        self.raster.to_picture(self.mode, self.id as u32, state)
    }
}

/// The event `kind` of picture `p` at signal time `time`, from a signal at `freq_hz`.
pub fn picture_event(
    kind: &'static EventKind<PictureEvent<'static>>,
    p: &HfPicture,
    time: f64,
    freq_hz: f64,
) -> Event {
    kind.event(
        time,
        freq_hz,
        &PictureEvent {
            picture_id: p.id as i64,
            picture_width: i64::from(p.width()),
            picture_height: i64::from(p.height()),
            picture_rows: i64::from(p.rows),
            picture_complete: p.complete,
            picture_mode: p.mode,
        },
    )
}

/// The picture fields: `current` (the one being received) or else the newest of `history`.
pub fn picture_fields<'a>(
    current: Option<&'a HfPicture>,
    history: &'a PictureHistory<HfPicture>,
) -> PictureFields<'a> {
    let p = current.or_else(|| history.latest());
    PictureFields {
        picture_id: p.map(|p| p.id as i64),
        picture_width: p.map(|p| i64::from(p.width())),
        picture_height: p.map(|p| i64::from(p.height())),
        picture_rows: p.map(|p| i64::from(p.rows)),
        picture_progress_pct: p.map(|p| {
            if p.progress[1] > 0 {
                (100.0 * p.progress[0] as f64 / p.progress[1] as f64).round()
            } else {
                0.0
            }
        }),
        picture_complete: p.map(|p| p.complete),
        picture_colour: p.map(|p| p.colour),
        picture_mode: p.map(|p| p.mode),
        pictures: history.finished() as i64,
    }
}

/// The instantaneous phase of a band of the 8 kHz audio, unwrapped and kept for a while, so the mean frequency over any stretch
/// (fractional positions allowed) is a phase difference.
pub struct PhaseRing {
    rate: f64,
    centre: f64,
    nco: Nco,
    filt: Fir<Complex32>,
    prev: Complex32,
    acc: f64,
    ring: Vec<f64>,
    /// Samples pushed since the start (the index of the next one).
    pub count: u64,
    /// The filter's delay in samples.
    pub delay: f64,
}

impl PhaseRing {
    /// A ring of `seconds` at `rate`, for the band `centre ± half` Hz.
    pub fn new(rate: f64, centre: f64, half: f64, seconds: f64) -> Self {
        let taps = lowpass(rate, half, half + (0.25 * half).max(80.0), 50.0)
            .expect("the filter settings are valid");
        let delay = (taps.len() - 1) as f64 / 2.0;
        PhaseRing {
            rate,
            centre,
            nco: Nco::new(rate, -centre),
            filt: Fir::new(&taps),
            prev: Complex32::new(1.0, 0.0),
            acc: 0.0,
            ring: vec![0.0; (seconds * rate) as usize],
            count: 0,
            delay,
        }
    }
    /// The band's centre.
    pub fn centre(&self) -> f64 {
        self.centre
    }
    /// Pushes audio.
    pub fn push(&mut self, x: &[Complex32]) {
        for &s in x {
            let m = s * self.nco.next_sample();
            let y = self.filt.process_sample(m);
            let d = (y * self.prev.conj()).arg() as f64;
            self.prev = y;
            self.acc += d;
            let n = self.ring.len();
            self.ring[(self.count % n as u64) as usize] = self.acc;
            self.count += 1;
        }
    }
    /// The unwrapped phase at sample index `i` (must be within the ring).
    pub fn phase(&self, i: u64) -> f64 {
        self.ring[(i % self.ring.len() as u64) as usize]
    }
    /// Whether sample `i` is still held.
    pub fn holds(&self, i: u64) -> bool {
        i < self.count && i + (self.ring.len() as u64) > self.count
    }
    fn phase_at(&self, t: f64) -> f64 {
        let i = t.floor();
        let f = t - i;
        let a = self.phase(i as u64);
        let b = self.phase(i as u64 + 1);
        a + f * (b - a)
    }
    /// The mean frequency (Hz, absolute) from position `a` to `b` (sample indices of the input, fractional. The filter delay is
    /// added here).
    pub fn mean_freq(&self, a: f64, b: f64) -> f64 {
        let (a, b) = (a + self.delay, b + self.delay);
        if b <= a {
            return self.centre;
        }
        (self.phase_at(b) - self.phase_at(a)) / (TAU * (b - a)) * self.rate + self.centre
    }
}

/// `Pic:<W>x<H>[C][p<N>];` in a text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PicHeader {
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
    /// Colour (each row sends red, green and blue planes).
    pub colour: bool,
    /// Samples per pixel at 8 kHz (8, 4 or 2).
    pub spp: u32,
}

impl PicHeader {
    /// The header ending at the end of `tail`.
    pub fn find(tail: &[u8]) -> Option<PicHeader> {
        let s = std::str::from_utf8(tail).ok()?;
        if !s.ends_with(';') {
            return None;
        }
        let start = s.rfind("Pic:")?;
        let body = &s[start + 4..s.len() - 1];
        let (w, rest) = body.split_once('x')?;
        let w: u32 = w.parse().ok()?;
        let digits = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
        let h: u32 = rest[..digits].parse().ok()?;
        let mut rest = &rest[digits..];
        let colour = rest.starts_with('C');
        if colour {
            rest = &rest[1..];
        }
        let spp = match rest {
            "" => 8,
            "p2" => 2,
            "p4" => 4,
            "p8" => 8,
            _ => return None,
        };
        if !(1..=4095).contains(&w) || !(1..=4095).contains(&h) || w as u64 * h as u64 > 4_000_000 {
            return None;
        }
        Some(PicHeader { w, h, colour, spp })
    }
}

/// Reads one MFSK picture from the audio after its header: finds the end of the prologue, then takes each pixel's mean frequency.
pub struct PictureReader {
    h: PicHeader,
    id: u64,
    lo: f64,
    bw: f64,
    /// Where to search for the prologue from.
    from: u64,
    start: Option<f64>,
    px: Vec<u8>,
    done_px: usize,
    rows_sent: u32,
    t0: u64,
    mode: &'static str,
    spacing: f64,
    symlen: f64,
}

impl PictureReader {
    /// A reader for `h`, the header decoded at sample `now`. The lowest tone at `lo` Hz, the band `bw` Hz wide. The text decoder's
    /// delay in seconds.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        h: PicHeader,
        id: u64,
        now: u64,
        lo: f64,
        bw: f64,
        latency_s: f64,
        mode: &'static str,
        spacing: f64,
        symlen: f64,
    ) -> Self {
        let back = ((latency_s + 0.8) * 8000.0) as u64;
        PictureReader {
            h,
            id,
            lo,
            bw,
            from: now.saturating_sub(back),
            start: None,
            px: vec![0; (h.w * h.h * if h.colour { 3 } else { 1 }) as usize],
            done_px: 0,
            rows_sent: 0,
            t0: now,
            mode,
            spacing,
            symlen,
        }
    }

    fn total(&self) -> usize {
        self.px.len()
    }

    /// The picture so far.
    pub fn picture(&self) -> HfPicture {
        let (w, hh) = (self.h.w, self.h.h);
        let mut p = HfPicture::new(self.id, self.mode, w, hh, self.t0 as f64 / 8000.0);
        let wu = w as usize;
        for y in 0..hh as usize {
            for x in 0..wu {
                let (r, g, b) = if self.h.colour {
                    let base = y * wu * 3;
                    (
                        self.px[base + x],
                        self.px[base + wu + x],
                        self.px[base + 2 * wu + x],
                    )
                } else {
                    let v = self.px[y * wu + x];
                    (v, v, v)
                };
                p.put(x as u32, y as u32, r, g, b);
            }
        }
        let planes = if self.h.colour { 3 } else { 1 };
        let spp = self.h.spp as u64;
        p.rows = (self.done_px / (wu * planes)) as u32;
        p.complete = self.done_px >= self.total();
        p.colour = self.h.colour;
        p.progress = [self.done_px as u64 * spp, self.total() as u64 * spp];
        p
    }

    /// Reads what the ring holds. A finished picture (or one given up) is returned. Each new row goes to the picture output, and
    /// a finished picture emits the event [`crate::typed::MFSK_PICTURE`] with the frequency `freq_hz` of the signal.
    pub fn feed(&mut self, ring: &PhaseRing, out: &mut Outbox, freq_hz: f64) -> Option<HfPicture> {
        let now = ring.count;
        if self.start.is_none() {
            // The prologue: a run of the lowest tone of at least 300 samples, ended by the first pixel.
            const W: u64 = 64;
            let tol = (0.45 * self.spacing).min(0.1 * self.bw);
            let mut i = self
                .from
                .max(now.saturating_sub(ring.ring.len() as u64 - 2 * W));
            let step = 8u64;
            let mut run_start: Option<u64> = None;
            let end = now.saturating_sub(ring.delay as u64 + W + 2);
            while i + step < end {
                let f = ring.mean_freq(i as f64, (i + W) as f64);
                let low = (f - self.lo).abs() < tol;
                match (low, run_start) {
                    (true, None) => run_start = Some(i),
                    (false, Some(s)) => {
                        // The run ends somewhere in this window.
                        let e = self.jump(ring, i, i + W, false);
                        let s = self.jump(ring, s.saturating_sub(W), s + W, true);
                        // A flush symbol after a run of the lowest tone is one steady tone on the grid. Pixels are not.
                        let l = self.symlen;
                        if e + l + ring.delay + 2.0 > now as f64 {
                            break;
                        }
                        let f1 = ring.mean_freq(e + 4.0, e + l / 2.0);
                        let f2 = ring.mean_freq(e + l / 2.0, e + l - 4.0);
                        let idx = (f1 - self.lo) / self.spacing;
                        let flush = (f1 - f2).abs() < tol && (idx - idx.round()).abs() < 0.2;
                        if e - s >= 300.0 && !flush {
                            // The prologue starts at a symbol boundary. The flush can end with symbols of the lowest tone.
                            let l = self.symlen;
                            let k = ((e - s - 352.0) / l).round().max(0.0);
                            let p = s + 352.0 + k * l;
                            self.start = Some(if (p - e).abs() < 12.0 {
                                e
                            } else {
                                s + 352.0 + ((e - s - 352.0) / l).floor().max(0.0) * l
                            });
                            break;
                        }
                        run_start = None;
                    }
                    _ => {}
                }
                i += step;
            }
            self.from = run_start.unwrap_or(i).saturating_sub(W);
            if self.start.is_none() {
                // Give up a search that found nothing for 12 s.
                if now > self.t0 + 12 * 8000 {
                    return Some(self.picture());
                }
                return None;
            }
        }
        let s = self.start?;
        let spp = self.h.spp as f64;
        let w = self.h.w as usize * if self.h.colour { 3 } else { 1 };
        while self.done_px < self.total() {
            let a = s + self.done_px as f64 * spp;
            let b = a + spp;
            if b + ring.delay + 2.0 > now as f64 {
                break;
            }
            let f = ring.mean_freq(a, b);
            let v = ((f - self.lo) / self.bw * 255.0).round().clamp(0.0, 255.0) as u8;
            self.px[self.done_px] = v;
            self.done_px += 1;
            if self.done_px.is_multiple_of(w) {
                self.rows_sent += 1;
                out.picture(
                    now as f64 / 8000.0,
                    self.picture()
                        .to_picture(sdrkit_graph::PictureState::Growing),
                );
            }
        }
        if self.done_px >= self.total() {
            let p = self.picture();
            out.event(picture_event(
                &crate::typed::MFSK_PICTURE,
                &p,
                now as f64 / 8000.0,
                freq_hz,
            ));
            return Some(p);
        }
        None
    }

    /// The sample in `a..b` where the frequency steps most (`into_low`: from another tone onto the lowest. Otherwise off it).
    fn jump(&self, ring: &PhaseRing, a: u64, b: u64, into_low: bool) -> f64 {
        const H: f64 = 24.0;
        let mut best = (f64::MIN, (a + b) as f64 / 2.0);
        for p in a.max(H as u64)..b {
            let p = p as f64;
            let before = ring.mean_freq(p - H, p);
            let after = ring.mean_freq(p, p + H);
            let d = if into_low {
                before - after
            } else {
                after - before
            };
            if d > best.0 {
                best = (d, p);
            }
        }
        best.1
    }
}

/// A grey test picture: blocks of 5 pixels, two levels in a checkerboard.
pub fn test_grey(w: usize, h: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            v.push(if ((x / 5) + (y / 5)) % 2 == 0 {
                25
            } else {
                230
            });
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers() {
        assert_eq!(
            PicHeader::find(b"Sending Pic:40x30;"),
            Some(PicHeader {
                w: 40,
                h: 30,
                colour: false,
                spp: 8
            })
        );
        assert_eq!(
            PicHeader::find(b"Pic:320x240Cp4;"),
            Some(PicHeader {
                w: 320,
                h: 240,
                colour: true,
                spp: 4
            })
        );
        assert_eq!(PicHeader::find(b"Pic:0x30;"), None);
        assert_eq!(PicHeader::find(b"Pic:40x30"), None);
        assert_eq!(PicHeader::find(b"Pic:4000x4000;"), None);
    }
}
