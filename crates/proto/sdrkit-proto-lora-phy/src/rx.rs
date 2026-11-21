//! Detection, synchronisation, demodulation and decoding of LoRa frames on the chip stream of one channel.

use crate::chips::ChipRings;
use crate::coding::{Codec, Header, Payload, SymbolSoft};
use crate::error::{LoraError, Param, Result};
use crate::params::{self, Ldro};
use crate::recover::Recovery;
use sdrkit_algo_modem::chirp::{Dechirp, Direction};
use sdrkit_algo_modem::fsk::ln_i0;
use sdrkit_core::Complex32;
use serde::{Deserialize, Serialize};

/// What one detector searches for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetectorConfig {
    /// The spreading factor, 5 to 12.
    pub sf: u8,
    /// The bandwidth, Hz.
    pub bw: f64,
    /// The sync words it accepts, 1 to [`MAX_SYNCS`]. One preamble search serves them all.
    pub syncs: Vec<u8>,
    /// Inverted IQ (gateway downlinks).
    pub invert: bool,
    /// The preamble length of the setting. The reported frame start comes from it.
    pub preamble: u16,
    /// The header agreed in advance for an implicit header. `None` for an explicit header.
    pub implicit: Option<Header>,
    /// Low data rate optimisation.
    pub ldro: Ldro,
    /// Soft-decision decoding.
    pub soft: bool,
    /// The search for lost samples is on.
    pub jump_search: bool,
    /// The usual jump of the radio, chips.
    pub jump_chips: f64,
    /// Doppler tracking, for satellites. Off for ground links.
    pub doppler: bool,
    /// A number the owner provides this detector. Each frame carries it.
    pub tag: u32,
    /// The absolute carrier frequency, Hz (0 if not known). The detector does not read it: the carrier loop measures the
    /// change of the carrier during a frame apart from the symbol clock.
    #[serde(default)]
    pub carrier_hz: f64,
}

impl DetectorConfig {
    /// A detector for `sf` and `bw` with one sync word, an explicit header, soft decoding and the jump search on.
    pub fn new(sf: u8, bw: f64, sync: u8) -> Self {
        DetectorConfig {
            sf,
            bw,
            syncs: vec![sync],
            invert: false,
            preamble: 8,
            implicit: None,
            ldro: Ldro::Auto,
            soft: true,
            jump_search: true,
            jump_chips: 11.7,
            doppler: false,
            tag: 0,
            carrier_hz: 0.0,
        }
    }

    /// Makes sure that the configuration is valid: SF 5 to 12, a finite positive bandwidth, at least one sync word, and a valid
    /// implicit header. If it is not valid, this returns an error.
    pub fn check(&self) -> Result<()> {
        if !(5..=12).contains(&self.sf) {
            return Err(LoraError::BadParameter(Param::SpreadingFactor));
        }
        if !(self.bw.is_finite() && self.bw > 0.0) {
            return Err(LoraError::BadParameter(Param::Bandwidth));
        }
        if self.syncs.is_empty() || self.syncs.len() > MAX_SYNCS {
            return Err(LoraError::BadParameter(Param::SyncWords));
        }
        if let Some(h) = self.implicit {
            if !(5..=8).contains(&h.cr) || h.len == 0 {
                return Err(LoraError::BadParameter(Param::ImplicitHeader));
            }
        }
        if !(self.jump_chips.is_finite() && self.carrier_hz.is_finite()) {
            return Err(LoraError::BadParameter(Param::JumpOrCarrier));
        }
        Ok(())
    }

    fn ldro_on(&self) -> bool {
        match self.ldro {
            Ldro::On => true,
            Ldro::Off => false,
            Ldro::Auto => params::auto_ldro(self.sf, self.bw),
        }
    }
}

/// A decoded LoRa frame (graph item `lora.frame`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LoraFrame {
    /// The payload bytes.
    pub payload: Vec<u8>,
    /// The spreading factor.
    pub sf: u8,
    /// The bandwidth, Hz.
    pub bw: f64,
    /// The coding rate 5 to 8, from the header (or the agreed header).
    pub cr: u8,
    /// The CRC is present.
    pub crc_present: bool,
    /// The CRC is right.
    pub crc_ok: bool,
    /// The frame has an implicit header.
    pub implicit: bool,
    /// The sync word heard.
    pub sync: u8,
    /// Inverted IQ.
    pub inverted: bool,
    /// The carrier offset from the channel centre at the first data symbol, Hz.
    pub cfo_hz: f64,
    /// The carrier offset at the end of the frame, Hz, from the carrier loop of the data symbols.
    #[serde(default)]
    pub cfo_end_hz: f64,
    /// The rate of change of the carrier during the frame, Hz/s.
    #[serde(default)]
    pub cfo_rate_hz_s: f64,
    /// The clock offset, ppm.
    pub ppm: f64,
    /// The SNR in the LoRa bandwidth, dB.
    pub snr_db: f64,
    /// The level at the channel output, dB relative to full scale.
    pub level_db: f64,
    /// The start: the first preamble up-chirp, as an input sample index (filter delay removed).
    pub start: u64,
    /// The end, as an input sample index.
    pub end: u64,
    /// The codewords corrected.
    pub corrected: u32,
    /// The list search repaired the frame.
    pub list_fixed: bool,
    /// The sample jumps undone.
    pub jumps: u32,
    /// The tag of the detector.
    pub tag: u32,
    /// The centre of the channel, Hz (absolute when the radio's centre is known).
    pub channel_hz: f64,
}

impl LoraFrame {
    /// True if the frame is good: its CRC is right, or it has no CRC.
    pub fn good(&self) -> bool {
        self.crc_ok || !self.crc_present
    }
}

/// The symbols of a frame (graph item `lora.symbols`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoraSymbols {
    /// The symbols with their soft values.
    pub symbols: Vec<SymbolSoft>,
    /// The mean noise power per bin.
    pub noise: f32,
}

/// A change of a detector's busy state: it found a preamble, or the frame it was in is over. The channel receiver and the bank
/// give these with input sample indices, so a transmitter can wait for a free channel at the time of each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BusyChange {
    /// The input sample (absolute, filter delay removed) up to which the detector had read when its state changed.
    pub at: u64,
    /// The detector's place: in [`crate::ChannelRx::detectors`] for a channel, in [`crate::Bank::states`] for a bank.
    pub detector: usize,
    /// The detector's tag.
    pub tag: u32,
    /// The centre of the detector's channel, Hz, absolute. It is 0 from a single channel receiver.
    pub channel_hz: f64,
    /// The new state: true when the detector is inside a frame.
    pub busy: bool,
}

/// The counters of a detector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorStats {
    /// Preambles found.
    pub preambles: u64,
    /// Preambles with no sync word or SFD after them.
    pub sync_failed: u64,
    /// Headers that failed.
    pub header_failed: u64,
    /// Frames decoded, good or not.
    pub frames: u64,
    /// Frames whose CRC failed.
    pub crc_failed: u64,
    /// Sample jumps undone.
    pub jumps_undone: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Search,
    Preamble,
    Sync2,
    Sfd,
    Data,
}

/// The analysis of one window.
#[derive(Clone, Copy, Debug, Default)]
struct Peak {
    bin: usize,
    power: f32,
    /// The mean power over all bins, the peak included.
    mean: f32,
    /// The offset of the true peak from `bin` (Candan's estimator).
    frac: f32,
    /// The timing error from the phase step at the frequency wrap, chips (positive: read late). `None` when the wrap is at an
    /// end of the symbol.
    timing: Option<f32>,
    /// The offset of the tone from `bin` inside the segments before and after the wrap, bins: timing error plus carrier error.
    freq: f32,
}

/// One detector, demodulator and decoder for one spreading factor, sync-word set and IQ direction. It finds a preamble in
/// the chip stream, checks the sync symbols, takes the carrier offset and timing from the start frame delimiter, reads the
/// data symbols with their soft values while a timing loop and a carrier loop follow the transmitter, and decodes the header
/// and the payload. During the data symbols it watches for the preamble of a stronger frame and changes to that frame. If the CRC fails, it runs the searches of
/// [`crate::recover`]. Between frames and per data symbol it allocates nothing.
pub struct Detector {
    cfg: DetectorConfig,
    n: usize,
    dech: Dechirp,
    codec: Codec,
    recovery: Recovery,
    payload: Payload,
    buf: Vec<Complex32>,
    state: State,
    /// The start of the next window, chips (absolute).
    pos: f64,
    hist: [(usize, f32); 2],
    hist_len: usize,
    pre_count: usize,
    ref_bin: usize,
    pre_fracs: Vec<f32>,
    pre_phase_acc: Complex32,
    pre_last: Option<Complex32>,
    pre_snr_acc: f64,
    pre_lvl_acc: f64,
    pre_snr_n: usize,
    /// The sync words whose first symbol was heard, each with the form (index into `sync_symbol_forms`) that matched.
    sync_cands: [(u8, u8); MAX_SYNCS],
    sync_cand_n: usize,
    sync_found: u8,
    sfd_pos: f64,
    data_start: f64,
    cfo_bins: f64,
    drift: f64,
    tau: f64,
    loop_drift: f64,
    /// What the carrier loop added to `cfo_bins` during the frame, bins.
    cfo_adj: f64,
    /// The watch for a stronger preamble during the data symbols: the start of its next window (chips), the bin of its last
    /// strong window, and how many strong windows in a row had that bin.
    watch_pos: f64,
    watch_bin: usize,
    watch_n: u8,
    /// The peak powers of the last eight symbols of the frame being read, as a ring. It starts with the level of the preamble.
    own_power: [f32; 8],
    /// A later pass over the data symbols runs: no watch.
    quiet: bool,
    nsym: usize,
    header: Option<Header>,
    syms: Vec<SymbolSoft>,
    syms_saved: Vec<SymbolSoft>,
    frame_start_chip: f64,
    noise_acc: f64,
    first_pass: bool,
    stats: DetectorStats,
    /// The busy state at the last check.
    was_busy: bool,
    /// The highest chip read so far (exclusive end), absolute.
    read_hi: f64,
    /// The busy changes of the last call of [`Detector::run`]: (chip, busy).
    changes: Vec<(f64, bool)>,
}

const PRE_FRACS: usize = 4096;

/// The gains of the timing loop of the data symbols, per chip of timing error: on the window position and on its rate.
const TIMING_KP: f64 = 0.1;
const TIMING_KI: f64 = 0.004;
/// The gain of the carrier loop of the data symbols, per bin of carrier error.
const CARRIER_K: f64 = 0.15;

/// The most sync words one detector accepts.
pub const MAX_SYNCS: usize = 16;

impl Detector {
    /// A detector for `cfg` that searches from chip `start`. If the configuration is not valid, this returns an error.
    pub fn new(cfg: DetectorConfig, start: u64) -> Result<Self> {
        cfg.check()?;
        let n = 1usize << cfg.sf;
        let dech = Dechirp::new(cfg.sf, 1).map_err(LoraError::Modem)?;
        let codec = Codec::new(cfg.sf)?;
        let most = params::data_symbols(cfg.sf, 8, 255, true, false, true)
            .max(params::data_symbols(cfg.sf, 8, 255, true, false, false));
        let mut payload = Payload::default();
        payload.bytes.reserve(255);
        payload.block_clean.reserve(most);
        Ok(Detector {
            n,
            dech,
            codec,
            recovery: Recovery::new(),
            payload,
            buf: vec![Complex32::default(); n],
            state: State::Search,
            pos: start as f64,
            hist: [(0, 0.0); 2],
            hist_len: 0,
            pre_count: 0,
            ref_bin: 0,
            pre_fracs: Vec::with_capacity(PRE_FRACS),
            pre_phase_acc: Complex32::default(),
            pre_last: None,
            pre_snr_acc: 0.0,
            pre_lvl_acc: 0.0,
            pre_snr_n: 0,
            sync_cands: [(0, 0); MAX_SYNCS],
            sync_cand_n: 0,
            sync_found: 0,
            sfd_pos: 0.0,
            data_start: 0.0,
            cfo_bins: 0.0,
            drift: 0.0,
            tau: 0.0,
            loop_drift: 0.0,
            cfo_adj: 0.0,
            watch_pos: 0.0,
            watch_bin: 0,
            watch_n: 0,
            own_power: [0.0; 8],
            quiet: false,
            nsym: 0,
            header: None,
            syms: Vec::with_capacity(most + 1),
            syms_saved: Vec::with_capacity(most + 1),
            frame_start_chip: 0.0,
            noise_acc: 0.0,
            first_pass: true,
            stats: DetectorStats::default(),
            was_busy: false,
            read_hi: start as f64,
            changes: Vec::with_capacity(8),
            cfg,
        })
    }

    /// The configuration.
    pub fn config(&self) -> &DetectorConfig {
        &self.cfg
    }

    /// The counters.
    pub fn stats(&self) -> &DetectorStats {
        &self.stats
    }

    /// Sets the counters (to keep them over a reset).
    pub fn set_stats(&mut self, stats: DetectorStats) {
        self.stats = stats;
    }

    /// True inside a frame: a preamble was found and its frame is not over.
    pub fn busy(&self) -> bool {
        self.state != State::Search
    }

    /// Starts the search again from chip `at`.
    pub fn restart(&mut self, at: u64) {
        self.state = State::Search;
        self.pos = at as f64;
        self.hist_len = 0;
    }

    /// The earliest chip this detector can still read.
    pub fn needs_from(&self) -> f64 {
        let n = self.n as f64;
        match self.state {
            State::Data => self.data_start.min(self.pos) - n,
            State::Search => self.pos,
            _ => self.pos - 14.0 * n,
        }
    }

    /// Dechirps the window in `self.buf` (up-chirps or down-chirps), removes `cfo` bins, and finds the peak.
    fn analyse(&mut self, dir: Direction, cfo: f64) -> Peak {
        match self.dech.dechirp(&self.buf, dir, cfo) {
            Ok(p) => {
                let n = self.n as f32;
                Peak {
                    bin: p.bin,
                    power: p.power,
                    mean: (p.power + (n - 1.0) * p.noise) / n,
                    frac: p.frac as f32,
                    timing: p.timing.map(|t| t as f32),
                    freq: p.freq as f32,
                }
            }
            Err(_) => Peak::default(),
        }
    }

    /// Bin 0 of the last window's spectrum (the preamble phase).
    fn bin0(&self) -> Complex32 {
        self.dech.spectrum()[0]
    }

    /// The mean power of the bins far from `peak` (N/8 away or more).
    fn far_noise(&self, peak: usize) -> f32 {
        let n = self.n;
        let pow = self.dech.bins();
        let gap = (n / 8).max(4);
        let (mut s, mut c) = (0.0f32, 0usize);
        for k in gap..=(n - gap) {
            s += pow[(peak + k) % n];
            c += 1;
        }
        (s / c.max(1) as f32).max(1e-30)
    }

    fn circ(&self, a: usize, b: usize) -> usize {
        let d = (a as isize - b as isize).rem_euclid(self.n as isize) as usize;
        d.min(self.n - d)
    }

    fn signed(&self, b: f64) -> f64 {
        let n = self.n as f64;
        let mut v = b.rem_euclid(n);
        if v > n / 2.0 {
            v -= n;
        }
        v
    }

    fn read_chips(&mut self, rings: &ChipRings, start: u64) -> bool {
        let ok = rings.chips(start, self.n, self.cfg.invert, &mut self.buf);
        if ok {
            self.read_hi = self.read_hi.max((start + self.n as u64) as f64);
        }
        ok
    }

    fn read_interp(&mut self, rings: &ChipRings, start: f64, step: f64) -> bool {
        let ok = rings.interpolate(start, step, self.n, self.cfg.invert, &mut self.buf);
        if ok {
            self.read_hi = self.read_hi.max(start + step * self.n as f64);
        }
        ok
    }

    /// The changes of [`Detector::busy`] during the last call of [`Detector::run`], oldest first. Each is the chip (absolute)
    /// that the detector had read up to when the state changed, and the new state.
    pub fn busy_changes(&self) -> &[(f64, bool)] {
        &self.changes
    }

    /// Records a change of the busy state at the highest chip read.
    fn note_busy(&mut self) {
        let b = self.busy();
        if b != self.was_busy {
            self.was_busy = b;
            self.changes.push((self.read_hi, b));
        }
    }

    /// Runs on what the rings hold. Decoded frames go to `out`. The busy changes it makes are in [`Detector::busy_changes`].
    pub fn run(&mut self, rings: &ChipRings, out: &mut Vec<(LoraFrame, LoraSymbols)>) {
        self.changes.clear();
        loop {
            self.note_busy();
            if !self.step(rings, out) {
                break;
            }
        }
        self.note_busy();
    }

    /// One step of [`Detector::run`]: one window in the search, the SFD, or one data symbol. False when the rings do not
    /// hold enough chips yet.
    fn step(&mut self, rings: &ChipRings, out: &mut Vec<(LoraFrame, LoraSymbols)>) -> bool {
        let n = self.n;
        match self.state {
            State::Search | State::Preamble | State::Sync2 => {
                if self.pos < rings.x1_first as f64 {
                    // The history was dropped: skip ahead.
                    self.state = State::Search;
                    self.pos = rings.x1_first as f64;
                    self.hist_len = 0;
                }
                if !self.read_chips(rings, self.pos.round() as u64) {
                    return false;
                }
                let pk = self.analyse(Direction::Up, 0.0);
                self.pos += n as f64;
                self.step_upchirp_window(pk);
            }
            State::Sfd => {
                // The two whole down-chirps: the first SFD window and the next.
                let p0 = self.sfd_pos;
                if !rings.has(p0, 1.0, 2 * n + 2) {
                    if (p0 as u64) < rings.x1_first {
                        self.state = State::Search;
                    }
                    return false;
                }
                self.read_chips(rings, p0.round() as u64);
                let a = self.analyse(Direction::Down, 0.0);
                self.read_chips(rings, p0.round() as u64 + n as u64);
                let c = self.analyse(Direction::Down, 0.0);
                if !self.start_frame(a, c, rings) {
                    self.stats.sync_failed += 1;
                    self.state = State::Search;
                    self.pos = p0 + n as f64;
                    self.hist_len = 0;
                }
            }
            State::Data => {
                if !self.data_step(rings, out) {
                    return false;
                }
            }
        }
        true
    }

    fn step_upchirp_window(&mut self, pk: Peak) {
        let n = self.n;
        let ratio = pk.power / pk.mean.max(1e-30);
        match self.state {
            State::Search => {
                if ratio < 6.0 {
                    self.hist_len = 0;
                    return;
                }
                // A preamble read half a chip off, with its wrap inside the window, has two equal peaks one bin at each side
                // of its place, so the bins of three windows can differ by two.
                let matches = |h: &(usize, f32)| self.circ(h.0, pk.bin) <= 2;
                let ok = self.hist_len >= 2
                    && matches(&self.hist[0])
                    && matches(&self.hist[1])
                    && self.circ(self.hist[0].0, self.hist[1].0) <= 2;
                if ok {
                    self.stats.preambles += 1;
                    // Align the windows so the preamble peaks at bin 0.
                    let shift = (n - pk.bin) % n;
                    self.pos += shift as f64;
                    self.state = State::Preamble;
                    self.pre_count = 3;
                    self.ref_bin = 0;
                    self.pre_fracs.clear();
                    self.pre_phase_acc = Complex32::default();
                    self.pre_last = None;
                    self.pre_snr_acc = 0.0;
                    self.pre_lvl_acc = 0.0;
                    self.pre_snr_n = 0;
                    self.hist_len = 0;
                } else {
                    self.hist[0] = self.hist[1];
                    self.hist[1] = (pk.bin, pk.power);
                    self.hist_len = (self.hist_len + 1).min(2);
                }
            }
            State::Preamble => {
                // A preamble chirp is known by its bin, at any level: the gain of the radio can change during the preamble.
                if self.circ(pk.bin, self.ref_bin) <= 1 && ratio >= 6.0 {
                    self.pre_count += 1;
                    self.ref_bin = pk.bin;
                    // The fractional position relative to bin 0.
                    let pos = self.signed(pk.bin as f64) as f32 + pk.frac;
                    if self.pre_fracs.len() < PRE_FRACS {
                        self.pre_fracs.push(pos);
                    }
                    let x0 = self.bin0();
                    if let Some(l) = self.pre_last {
                        self.pre_phase_acc += x0 * l.conj();
                    }
                    self.pre_last = Some(x0);
                    let noise = ((pk.mean * n as f32 - pk.power) / (n as f32 - 1.0)).max(1e-30);
                    self.pre_snr_acc += ((pk.power - noise).max(1e-30) / (n as f32 * noise)) as f64;
                    self.pre_lvl_acc += (pk.power / (n as f32 * n as f32)) as f64;
                    self.pre_snr_n += 1;
                    if self.pre_count > self.cfg.preamble as usize * 2 + 64 {
                        // A steady chirp that never becomes a frame.
                        self.state = State::Search;
                    }
                    return;
                }
                if self.pre_count < 4 || self.pre_fracs.is_empty() {
                    self.state = State::Search;
                    return;
                }
                self.sync_cand_n = 0;
                for i in 0..self.cfg.syncs.len() {
                    let s = self.cfg.syncs[i];
                    for (form, syms) in params::sync_symbol_forms(s, self.cfg.sf).iter().enumerate()
                    {
                        let e = (self.ref_bin + syms[0] as usize) % n;
                        if self.circ(pk.bin, e) <= 1 && self.sync_cand_n < self.sync_cands.len() {
                            self.sync_cands[self.sync_cand_n] = (s, form as u8);
                            self.sync_cand_n += 1;
                        }
                    }
                }
                if self.sync_cand_n == 0 {
                    self.stats.sync_failed += 1;
                    self.state = State::Search;
                    return;
                }
                self.state = State::Sync2;
            }
            State::Sync2 => {
                let found =
                    self.sync_cands[..self.sync_cand_n]
                        .iter()
                        .copied()
                        .find(|&(s, form)| {
                            let syms = params::sync_symbol_forms(s, self.cfg.sf)[form as usize];
                            let e = (self.ref_bin + syms[1] as usize) % n;
                            self.circ(pk.bin, e) <= 1
                        });
                match found {
                    Some((s, _)) => {
                        self.sync_found = s;
                        self.sfd_pos = self.pos;
                        self.state = State::Sfd;
                    }
                    None => {
                        self.stats.sync_failed += 1;
                        self.state = State::Search;
                    }
                }
            }
            _ => {}
        }
    }

    /// From the two SFD windows: the carrier offset, the timing and the data start. If the SFD is not there, this returns
    /// false.
    fn start_frame(&mut self, a: Peak, c: Peak, rings: &ChipRings) -> bool {
        let n = self.n as f64;
        let thr = 10.0 + self.cfg.sf as f32;
        let ra = a.power / a.mean.max(1e-30);
        let rc = c.power / c.mean.max(1e-30);
        if ra < thr && rc < thr {
            return false;
        }
        let pick = |p: &Peak| self.signed(p.bin as f64) + p.frac as f64;
        let d = if ra >= thr && rc >= thr && self.circ(a.bin, c.bin) <= 1 {
            (pick(&a) + pick(&c)) / 2.0
        } else if rc >= ra {
            pick(&c)
        } else {
            pick(&a)
        };
        let m = self.pre_fracs.len() as f64;
        let u = self.pre_fracs.iter().map(|&v| v as f64).sum::<f64>() / m;
        // The clock offset is estimated from the slope of the preamble peak.
        let mut drift = 0.0;
        if self.pre_fracs.len() >= 4 {
            let mx = (m - 1.0) / 2.0;
            let (mut sxy, mut sxx) = (0.0, 0.0);
            for (k, &v) in self.pre_fracs.iter().enumerate() {
                let dx = k as f64 - mx;
                sxy += dx * (v as f64 - u);
                sxx += dx * dx;
            }
            drift = (-(sxy / sxx) / n).clamp(-250e-6, 250e-6);
        }
        // The up-chirp position carried forward to the SFD window (a clock error moves it window by window).
        let u = u - drift * n * ((m - 1.0) / 2.0 + 3.0);
        let mut cfo = (u + d) / 2.0;
        if self.pre_fracs.len() >= 2 && self.pre_phase_acc.norm_sqr() > 0.0 {
            let fp = self.pre_phase_acc.arg() as f64 / std::f64::consts::TAU;
            cfo = (cfo - fp).round() + fp;
        }
        // The up-chirps peak at u = CFO - tau: the symbols start tau chips after the window grid.
        let tau = cfo - u;
        let extra = if self.cfg.sf < 7 { 2.0 } else { 0.0 };
        let base_start = self.sfd_pos + tau + (2.25 + extra) * n;
        // The carrier offset is known only modulo N/2. Try the other value too, on the first two data symbols.
        let alt_cfo = cfo - n / 2.0 * cfo.signum();
        let alt_start = self.sfd_pos + (alt_cfo - u) + (2.25 + extra) * n;
        let (start, cfo) =
            if cfo.abs() > n / 8.0 && rings.has(alt_start.max(base_start), 1.0, 2 * self.n + 4) {
                let s1 = self.score(rings, base_start, cfo, drift);
                let s2 = self.score(rings, alt_start, alt_cfo, drift);
                if s2 > s1 * 1.5 {
                    (alt_start, alt_cfo)
                } else {
                    (base_start, cfo)
                }
            } else {
                (base_start, cfo)
            };
        let (start, cfo) = self.fine(rings, start, cfo, drift);
        self.cfo_bins = cfo;
        self.drift = drift;
        self.data_start = start;
        self.frame_start_chip = start - (self.cfg.preamble as f64 + 4.25 + extra) * n;
        self.tau = 0.0;
        self.loop_drift = 0.0;
        self.cfo_adj = 0.0;
        self.syms.clear();
        self.noise_acc = 0.0;
        self.header = self.cfg.implicit;
        self.nsym = match self.cfg.implicit {
            Some(h) => params::data_symbols(
                self.cfg.sf,
                h.cr,
                h.len as usize,
                h.crc,
                true,
                self.cfg.ldro_on(),
            ),
            None => 8,
        };
        self.first_pass = true;
        self.state = State::Data;
        self.pos = start;
        self.watch_pos = start.ceil();
        self.watch_n = 0;
        self.own_power = [if self.pre_snr_n > 0 {
            (self.pre_lvl_acc / self.pre_snr_n as f64) as f32 * (self.n * self.n) as f32
        } else {
            f32::INFINITY
        }; 8];
        true
    }

    /// The peak-to-mean ratio of the first two data symbols from `start` with the carrier offset `cfo`.
    fn score(&mut self, rings: &ChipRings, start: f64, cfo: f64, drift: f64) -> f32 {
        let n = self.n as f64;
        let mut s = 0.0;
        for k in 0..2 {
            let st = start + k as f64 * n * (1.0 + drift);
            if !self.read_interp(rings, st, 1.0 + drift) {
                return 0.0;
            }
            let p = self.analyse(Direction::Up, cfo);
            s += p.power / p.mean.max(1e-30);
        }
        s
    }

    /// The fine estimates on interpolated preamble windows: the fractional carrier offset from the phase advance, the timing
    /// from the peak position, and the SNR and the level. Returns the corrected data start and carrier offset.
    fn fine(&mut self, rings: &ChipRings, mut start: f64, mut cfo: f64, drift: f64) -> (f64, f64) {
        let n = self.n as f64;
        let extra = if self.cfg.sf < 7 { 2.0 } else { 0.0 };
        let m = (self.pre_count.saturating_sub(1)).clamp(1, 8);
        for pass in 0..3 {
            let mut acc = Complex32::default();
            let mut last: Option<Complex32> = None;
            let (mut fsum, mut snr, mut lvl, mut cnt) = (0.0f64, 0.0f64, 0.0f64, 0usize);
            for j in (1..=m).rev() {
                let ws = start - (4.25 + extra + j as f64) * n * (1.0 + drift);
                if !self.read_interp(rings, ws, 1.0 + drift) {
                    continue;
                }
                let pk = self.analyse(Direction::Up, cfo);
                if self.circ(pk.bin, 0) > 1 {
                    continue;
                }
                let x0 = self.bin0();
                if let Some(l) = last {
                    acc += x0 * l.conj();
                }
                last = Some(x0);
                fsum += self.signed(pk.bin as f64) + pk.frac as f64;
                let noise = self.far_noise(pk.bin);
                snr += ((pk.power - noise).max(1e-30) / (n as f32 * noise)) as f64;
                lvl += (pk.power / (n as f32 * n as f32)) as f64;
                cnt += 1;
            }
            if cnt == 0 {
                break;
            }
            if pass == 2 {
                self.pre_snr_acc = snr;
                self.pre_lvl_acc = lvl;
                self.pre_snr_n = cnt;
                break;
            }
            if acc.norm_sqr() > 0.0 {
                // The carrier advances by the whole offset (in bins) per window. Its fraction is computed from the phase step.
                let fp = acc.arg() as f64 / std::f64::consts::TAU / (1.0 + drift);
                cfo = (cfo - fp).round() + fp;
            } else {
                // A window early by e shows the peak at -e (after the carrier is removed).
                start -= fsum / cnt as f64;
                continue;
            }
            if pass == 1 {
                start -= fsum / cnt as f64;
            }
        }
        (start, cfo)
    }

    /// Searches the chips before `upto` for the preamble of a second frame that is at least 3 dB stronger than the frame
    /// being read: three windows in a row with the same strong bin. When it finds one, it gives up the frame and starts the
    /// search at the first of the three windows, and returns true.
    fn stronger_preamble(&mut self, rings: &ChipRings, upto: f64) -> bool {
        let n = self.n;
        // The level of the frame being read: the mean over the older half of its last eight symbols. It follows a change of
        // the gain of the radio, and a second frame that begins does not raise it before the three windows are over.
        let k = self.syms.len();
        let own = (0..4).map(|j| self.own_power[(k + j) % 8]).sum::<f32>() / 4.0;
        while self.watch_pos + n as f64 <= upto && self.read_chips(rings, self.watch_pos as u64) {
            let pk = self.analyse(Direction::Up, 0.0);
            self.watch_pos += n as f64;
            // The peak and two bins at each side: a chirp read between two chips, with its wrap inside the window, has its
            // power in the bins next to its place.
            let bins = self.dech.bins();
            let around: f32 = (n - 2..=n + 2).map(|j| bins[(pk.bin + j) % n]).sum();
            let strong = pk.power >= 6.0 * pk.mean && around >= 2.0 * own;
            if strong && self.watch_n > 0 && self.circ(pk.bin, self.watch_bin) <= 2 {
                self.watch_n += 1;
            } else {
                self.watch_n = strong as u8;
            }
            self.watch_bin = pk.bin;
            if self.watch_n >= 3 {
                self.state = State::Search;
                self.pos = self.watch_pos - 3.0 * n as f64;
                self.hist_len = 0;
                return true;
            }
        }
        false
    }

    /// Demodulates the next data symbol. If its samples are not there yet, this returns false.
    fn data_step(&mut self, rings: &ChipRings, out: &mut Vec<(LoraFrame, LoraSymbols)>) -> bool {
        let n = self.n;
        let k = self.syms.len();
        let wide = k < 40;
        let doppler = self.cfg.doppler;
        let step = if doppler { 1.0 } else { 1.0 + self.drift };
        let start = self.data_start + k as f64 * n as f64 * step + self.tau;
        if !rings.has(start, step, n) {
            if (start as u64) < rings.x1_first {
                self.state = State::Search;
                self.pos = rings.x1_first as f64;
                return true;
            }
            return false;
        }
        if !self.quiet && !doppler && self.stronger_preamble(rings, start + n as f64 * step) {
            return true;
        }
        self.read_interp(rings, start, step);
        let cfo = if doppler {
            self.cfo_bins + self.loop_drift + self.drift * n as f64 * k as f64
        } else {
            self.cfo_bins + self.cfo_adj
        };
        let pk = self.analyse(Direction::Up, cfo);
        if !self.quiet {
            self.own_power[k % 8] = pk.power;
        }
        // Soft values: the four strongest bins.
        let mut top = [(0usize, -1.0f32); 4];
        for (i, &p) in self.dech.bins().iter().enumerate() {
            if p > top[3].1 {
                let mut j = 3;
                while j > 0 && p > top[j - 1].1 {
                    top[j] = top[j - 1];
                    j -= 1;
                }
                top[j] = (i, p);
            }
        }
        let total: f32 = pk.mean * n as f32;
        let sig4: f32 = top.iter().map(|t| t.1.max(0.0)).sum();
        let noise = ((total - sig4) / (n as f32 - 4.0)).max(1e-30);
        self.noise_acc += noise as f64;
        let amp = (top[0].1 - noise).max(noise * 1e-3).sqrt();
        let mut s = SymbolSoft::hard(top[0].0 as u16);
        if self.cfg.soft {
            s.count = 4;
            for (j, t) in top.iter().enumerate() {
                s.bins[j] = t.0 as u16;
                s.ll[j] = ln_i0(2.0 * amp * t.1.max(0.0).sqrt() / noise);
            }
            s.floor = ln_i0(2.0 * amp * noise.sqrt() / noise);
        }
        self.syms.push(s);
        let ratio = pk.power / pk.mean.max(1e-30);
        if doppler {
            // The carrier loop on the fractional position of the peak.
            if ratio > 8.0 {
                let kp = if wide { 0.3 } else { 0.08 };
                self.loop_drift += kp * pk.frac as f64;
            }
        } else {
            if ratio > 8.0 {
                // The phase step at the wrap is the timing error alone. It is certain when the wrap is near the middle of the
                // symbol, and says nothing when the wrap is at an end.
                let w = (n - top[0].0) % n;
                let certain = 4.0 * (w * (n - w)) as f64 / (n * n) as f64;
                let late = pk.timing.map_or(0.0, |t| t as f64) * certain;
                self.tau -= TIMING_KP * late;
                self.loop_drift -= TIMING_KI * late;
                // What stays of the tone offset is the carrier error.
                self.cfo_adj += CARRIER_K * (pk.freq as f64 - late);
            }
            self.tau += self.loop_drift;
        }
        let k = self.syms.len();
        if k == 8 && self.header.is_none() {
            match self.codec.decode_header(&self.syms, self.cfg.soft) {
                Some(h) => {
                    self.header = Some(h);
                    self.nsym = params::data_symbols(
                        self.cfg.sf,
                        h.cr,
                        h.len as usize,
                        h.crc,
                        false,
                        self.cfg.ldro_on(),
                    );
                }
                None => {
                    self.stats.header_failed += 1;
                    self.state = State::Search;
                    self.pos = start + n as f64;
                    self.hist_len = 0;
                    return true;
                }
            }
        }
        if k >= self.nsym {
            self.finish(rings, start + n as f64, out);
        }
        true
    }

    /// Demodulates all data symbols again with the current drift and loop settings. If they are not all there, this returns
    /// false.
    fn redo_symbols(&mut self, rings: &ChipRings) -> bool {
        self.tau = 0.0;
        self.loop_drift = 0.0;
        self.cfo_adj = 0.0;
        self.syms.clear();
        while self.syms.len() < self.nsym {
            let k0 = self.syms.len();
            if !self.data_step_quiet(rings) || self.syms.len() == k0 {
                return false;
            }
        }
        true
    }

    fn decode(&mut self, h: Header) {
        let implicit = self.cfg.implicit.is_some();
        self.codec.decode_payload(
            &self.syms,
            self.cfg.ldro_on(),
            implicit,
            h,
            self.cfg.soft,
            &mut self.payload,
        );
    }

    fn finish(
        &mut self,
        rings: &ChipRings,
        end_chip: f64,
        out: &mut Vec<(LoraFrame, LoraSymbols)>,
    ) {
        let Some(h) = self.header else {
            self.state = State::Search;
            return;
        };
        let sf = self.cfg.sf;
        let ldro = self.cfg.ldro_on();
        let implicit = self.cfg.implicit.is_some();
        self.decode(h);
        let mut list_fixed = false;
        let mut jumps = 0;
        if h.crc && !self.payload.crc_ok && !self.cfg.doppler && self.first_pass {
            // More passes: one that starts with the clock error the timing loop learnt, and one without the clock
            // estimate of the preamble.
            self.first_pass = false;
            let drift0 = self.drift;
            let learnt = drift0 + self.loop_drift / self.n as f64;
            let span = (self.nsym * self.n) as f64;
            std::mem::swap(&mut self.syms, &mut self.syms_saved);
            let mut found = false;
            for drift in [learnt, 0.0] {
                if (drift - drift0).abs() * span <= 0.3 {
                    continue;
                }
                self.drift = drift;
                if self.redo_symbols(rings) {
                    self.decode(h);
                    if self.payload.crc_ok {
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                std::mem::swap(&mut self.syms, &mut self.syms_saved);
                self.drift = drift0;
                self.decode(h);
            }
        }
        if h.crc && !self.payload.crc_ok {
            let soft = self.cfg.soft;
            if self.recovery.list_decode(
                &self.codec,
                &self.syms,
                ldro,
                implicit,
                h,
                soft,
                &mut self.payload,
            ) {
                list_fixed = true;
            }
        }
        if h.crc && !self.payload.crc_ok && !implicit && self.cfg.jump_search {
            if let Some(j) = self.recovery.jump_search(
                &self.codec,
                &self.syms,
                ldro,
                h,
                self.cfg.jump_chips,
                &mut self.payload,
            ) {
                jumps = j;
                self.stats.jumps_undone += j as u64;
            }
        }
        self.stats.frames += 1;
        if h.crc && !self.payload.crc_ok {
            self.stats.crc_failed += 1;
        }
        let n = self.n as f64;
        let snr = if self.pre_snr_n > 0 {
            10.0 * (self.pre_snr_acc / self.pre_snr_n as f64)
                .max(1e-12)
                .log10()
        } else {
            -99.0
        };
        let lvl = if self.pre_snr_n > 0 {
            10.0 * (self.pre_lvl_acc / self.pre_snr_n as f64)
                .max(1e-20)
                .log10()
        } else {
            -200.0
        };
        let cfo_hz = self.cfo_bins * self.cfg.bw / n;
        let ppm = (self.drift + self.loop_drift / n) * 1e6;
        let k_last = self.syms.len().saturating_sub(1) as f64;
        // The carrier at the end: what the loop of the frame followed.
        let cfo_end_hz = if self.cfg.doppler {
            (self.cfo_bins + self.loop_drift + self.drift * n * k_last) * self.cfg.bw / n
        } else {
            (self.cfo_bins + self.cfo_adj) * self.cfg.bw / n
        };
        let dur_s = (k_last * n / self.cfg.bw).max(1e-9);
        let cfo_rate_hz_s = if k_last > 0.0 {
            (cfo_end_hz - cfo_hz) / dur_s
        } else {
            0.0
        };
        let frame = LoraFrame {
            payload: self.payload.bytes.clone(),
            sf,
            bw: self.cfg.bw,
            cr: self.payload.cr,
            crc_present: self.payload.crc_present,
            crc_ok: self.payload.crc_ok,
            implicit,
            sync: self.sync_found,
            inverted: self.cfg.invert,
            cfo_hz,
            cfo_end_hz,
            cfo_rate_hz_s,
            ppm,
            snr_db: snr,
            level_db: lvl,
            // Chip positions. The channel receiver turns them into input indices.
            start: self.frame_start_chip.max(0.0).round() as u64,
            end: end_chip.max(0.0).round() as u64,
            corrected: self.payload.corrected,
            list_fixed,
            jumps,
            tag: self.cfg.tag,
            channel_hz: 0.0,
        };
        let symbols = LoraSymbols {
            symbols: self.syms.clone(),
            noise: (self.noise_acc / self.syms.len().max(1) as f64) as f32,
        };
        out.push((frame, symbols));
        self.state = State::Search;
        self.pos = end_chip;
        self.hist_len = 0;
    }

    /// One data symbol of a later pass (no header decoding, no finishing).
    fn data_step_quiet(&mut self, rings: &ChipRings) -> bool {
        let saved_state = self.state;
        let nsym = self.nsym;
        let header = self.header;
        // Ask for more symbols than the frame has, so the frame does not finish.
        self.nsym = usize::MAX;
        self.quiet = true;
        let mut none = Vec::new();
        let ok = self.data_step(rings, &mut none);
        self.quiet = false;
        self.nsym = nsym;
        self.header = header;
        self.state = saved_state;
        ok
    }
}
