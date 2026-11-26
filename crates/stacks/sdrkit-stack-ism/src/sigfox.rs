//! Sigfox uplinks (`sigfox`): the frame layer, the narrow-carrier finder, the D-BPSK demodulator, the joint decoder of the
//! repetitions and the message list. The authentication code is shown as received and never checked.
//!
//! Standard: Sigfox EP-SPECS Rev. 1.3 sections 2 and 3.

use crate::family::{at_least_value, hex, min_rate_message, CleanScene, REPORT_S};
use crate::typed::{
    SigfoxDemodFields, SigfoxFields, SigfoxFinderFields, SigfoxMessageEvent, SigfoxMessageRow,
    SIGFOX_MESSAGE, SIGFOX_MESSAGES, SIGFOX_WATERFALL,
};
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::design_by_taps;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_fec::conv::{ConvCode, Encoder, Ends, Viterbi};
use sdrkit_algo_fec::llr::from_hard;
use sdrkit_algo_modem::freq::PowerLine;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, ReadFields};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::AutoGain;
use sdrkit_core::SignalType;
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState, Receiver, Record,
    Results, ResultsDecl, SeriesId, Setting, SettingSet, SignalInfo, StackConfig, StackDescriptor,
    StateBlob, StateError, Status, Tuning, Values,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError, Io, Item, Kept,
    Kind, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_util_bits::crc::catalog::CRC16_GSM;
use sdrkit_util_bits::pack::{pack, unpack};
use sdrkit_util_bits::{BitOrder, Pad};
use sdrkit_util_objects::commands;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, Seen, Shift, Tables};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};
use std::fmt::Write as _;
use std::sync::Arc;

/// A Sigfox zone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    /// Name.
    pub name: &'static str,
    /// Uplink centre (Hz).
    pub uplink_hz: f64,
    /// Uplink bit rate.
    pub bit_rate: u32,
}

/// The zones RC1 to RC6 (EP-SPECS section 2).
pub const ZONES: [Zone; 6] = [
    Zone {
        name: "RC1",
        uplink_hz: 868.130e6,
        bit_rate: 100,
    },
    Zone {
        name: "RC2",
        uplink_hz: 902.200e6,
        bit_rate: 600,
    },
    Zone {
        name: "RC3",
        uplink_hz: 923.200e6,
        bit_rate: 100,
    },
    Zone {
        name: "RC4",
        uplink_hz: 920.800e6,
        bit_rate: 600,
    },
    Zone {
        name: "RC5",
        uplink_hz: 923.300e6,
        bit_rate: 100,
    },
    Zone {
        name: "RC6",
        uplink_hz: 865.200e6,
        bit_rate: 100,
    },
];

/// The zone the dial is on (within 150 kHz of its uplink centre), or `None`.
pub fn zone_of(dial_hz: f64) -> Option<Zone> {
    ZONES
        .iter()
        .copied()
        .find(|z| (z.uplink_hz - dial_hz).abs() <= 150e3)
}

/// The 19 preamble bits.
pub const PREAMBLE: [u8; 19] = [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1];

/// The frame types: (container bytes, the types of ranks 1 to 3, control message). EP-SPECS table 3-2.
const FT: [(usize, [u16; 3], bool); 6] = [
    (8, [0x006B, 0x06E0, 0x0034], false),
    (9, [0x008D, 0x00D2, 0x0302], false),
    (12, [0x035F, 0x0598, 0x05A3], false),
    (16, [0x0611, 0x06BF, 0x072C], false),
    (20, [0x094C, 0x0971, 0x0997], false),
    (16, [0x0F67, 0x0FC9, 0x11BE], true),
];

/// The generator of each rank's code, octal with the newest bit most significant in a 3-bit register: rank 1 sends the bits
/// as they are, rank 2 sends 1 + X + X², rank 3 sends 1 + X². Together they form one rate 1/3 code of constraint length 3.
pub const RANK_POLYS: [u32; 3] = [0o4, 0o7, 0o5];

/// The frame type of (container bytes, rank 1 to 3, control), or `None`.
pub fn frame_type(container: usize, rank: u8, control: bool) -> Option<u16> {
    if !(1..=3).contains(&rank) {
        return None;
    }
    FT.iter()
        .find(|f| f.0 == container && f.2 == control)
        .map(|f| f.1[(rank - 1) as usize])
}

/// Matches a received frame type: (container bytes, rank, control, exact type) when one entry is at most one bit away and no
/// other entry is as close.
pub fn match_frame_type(ft: u16) -> Option<(usize, u8, bool, u16)> {
    let mut best: Option<(u32, (usize, u8, bool, u16))> = None;
    let mut tie = false;
    for f in FT {
        for (r, &t) in f.1.iter().enumerate() {
            let d = ((t ^ ft) & 0x1FFF).count_ones();
            match best {
                Some((bd, _)) if d > bd => {}
                Some((bd, _)) if d == bd => tie = true,
                _ => {
                    best = Some((d, (f.0, r as u8 + 1, f.2, t)));
                    tie = false;
                }
            }
        }
    }
    match best {
        Some((d, v)) if d <= 1 && !tie => Some(v),
        _ => None,
    }
}

/// The payload length by container size and LI, or `None` when the pair is not valid. A container of 8 bytes with LI 2 or 3 is
/// a one-bit message with no payload byte. EP-SPECS table 3-3.
pub fn payload_len(container: usize, li: u8) -> Option<usize> {
    match (container, li) {
        (8, 0) | (8, 2) | (8, 3) => Some(0),
        (9, 0) => Some(1),
        (12, 0) => Some(4),
        (12, 1) => Some(3),
        (12, 2) => Some(2),
        (16, l) => Some(8 - l as usize),
        (20, l) => Some(12 - l as usize),
        _ => None,
    }
}

/// A decoded uplink frame (`sfx.uplink`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Uplink {
    /// Frame type as received (the matched table entry).
    pub frame_type: u16,
    /// Container bytes.
    pub container: usize,
    /// Rank 1 to 3.
    pub rank: u8,
    /// Control message.
    pub control: bool,
    /// LI.
    pub li: u8,
    /// Downlink request.
    pub bf: bool,
    /// REP.
    pub rep: bool,
    /// Message counter.
    pub counter: u16,
    /// Device id.
    pub device_id: u32,
    /// Payload bytes.
    pub payload: Vec<u8>,
    /// One-bit message value, when it is one.
    pub one_bit: Option<u8>,
    /// Authentication bytes (not checked).
    pub auth: Vec<u8>,
    /// CRC.
    pub crc: u16,
    /// Bit rate.
    pub bit_rate: u32,
    /// Frequency offset from the input centre (Hz).
    pub offset_hz: f64,
    /// Level (dBFS).
    pub level_db: f32,
    /// Signal time (s).
    pub time: f64,
    /// The frame decoded only together with other repetitions of the same message.
    pub joint: bool,
}

impl Item for Uplink {
    const KIND: &'static str = "sfx.uplink";
}

/// The coded bits of `bits` for `rank` (the coder starts in state 0). EP-SPECS section 3.4.
pub fn encode_rank(bits: &[u8], rank: u8) -> Vec<u8> {
    let poly = RANK_POLYS[(rank.clamp(1, 3) - 1) as usize];
    let mut out = Vec::with_capacity(bits.len());
    Encoder::new(ConvCode::new(3, &[poly])).encode(bits, &mut out);
    out
}

/// Decodes the coded bits of one message sent in several ranks: `frames` holds each rank (1 to 3) with the LLRs of its coded
/// bits (positive means 0), all of the same length. The ranks together form one rate 1/n code, decoded by the Viterbi
/// algorithm from state 0. Returns the decoded bits, or `None` for no frame, a bad rank, a rank given twice or unequal lengths.
pub fn decode_ranks(frames: &[(u8, &[i8])]) -> Option<Vec<u8>> {
    let len = frames.first()?.1.len();
    let mut polys = Vec::with_capacity(frames.len());
    for (k, (rank, llr)) in frames.iter().enumerate() {
        if !(1..=3).contains(rank) || llr.len() != len || frames[..k].iter().any(|f| f.0 == *rank) {
            return None;
        }
        polys.push(RANK_POLYS[(*rank - 1) as usize]);
    }
    let code = ConvCode::try_new(3, &polys).ok()?;
    let mut llr = Vec::with_capacity(len * frames.len());
    for i in 0..len {
        llr.extend(frames.iter().map(|f| f.1[i]));
    }
    let mut out = vec![0u8; len];
    Viterbi::new(code, len.max(1))
        .ok()?
        .decode(&llr, Ends::Start(0), &mut out)
        .ok()?;
    Some(out)
}

/// Builds the bits of a frame on air: the preamble, the frame type, then the container and its CRC coded for `rank`.
/// The authentication bytes are filler from `auth_seed` (a test signal cannot compute real ones).
#[allow(clippy::too_many_arguments)]
pub fn build_frame(
    device_id: u32,
    counter: u16,
    payload: &[u8],
    one_bit: Option<u8>,
    bf: bool,
    rank: u8,
    auth_seed: u64,
) -> Vec<u8> {
    let (container, li): (usize, u8) = if let Some(b) = one_bit {
        (8, 2 + (b & 1))
    } else {
        match payload.len() {
            0 => (8, 0),
            1 => (9, 0),
            2..=4 => (12, (4 - payload.len()) as u8),
            5..=8 => (16, (8 - payload.len()) as u8),
            _ => (20, (12 - payload.len().min(12)) as u8),
        }
    };
    let mut c = vec![
        (li << 6) | ((bf as u8) << 5) | ((counter >> 8) as u8 & 0xF),
        counter as u8,
    ];
    c.extend_from_slice(&device_id.to_le_bytes());
    c.extend_from_slice(&payload[..payload.len().min(12)]);
    let fill = container.saturating_sub(c.len());
    let mut auth = vec![0u8; fill];
    Rng::new(auth_seed).fill_bytes(&mut auth);
    c.extend_from_slice(&auth);
    let crc = CRC16_GSM.checksum(&c);
    c.extend_from_slice(&crc.to_be_bytes());
    let mut plain = Vec::with_capacity(8 * c.len());
    unpack(&c, BitOrder::Msb, &mut plain);
    let ft = frame_type(container, rank, false).unwrap_or(0);
    let mut bits = PREAMBLE.to_vec();
    for i in (0..13).rev() {
        bits.push(((ft >> i) & 1) as u8);
    }
    bits.extend(encode_rank(&plain, rank));
    bits
}

/// The uplink of the decoded container and CRC bits of a frame, or `None` when the CRC is wrong or the LI is not valid.
fn uplink_of(container: usize, rank: u8, control: bool, exact: u16, dec: &[u8]) -> Option<Uplink> {
    let mut bytes = Vec::with_capacity(container + 2);
    pack(dec, BitOrder::Msb, Pad::Zero, &mut bytes);
    if bytes.len() < container + 2 {
        return None;
    }
    let c = &bytes[..container];
    let crc = u16::from_be_bytes([bytes[container], bytes[container + 1]]);
    if CRC16_GSM.checksum(c) != crc {
        return None;
    }
    let li = c[0] >> 6;
    let pl = payload_len(container, li)?;
    if 6 + pl > container {
        return None;
    }
    let one_bit = (container == 8 && li >= 2).then(|| li - 2);
    Some(Uplink {
        frame_type: exact,
        container,
        rank,
        control,
        li,
        bf: c[0] & 0x20 != 0,
        rep: c[0] & 0x10 != 0,
        counter: ((c[0] as u16 & 0xF) << 8) | c[1] as u16,
        device_id: u32::from_le_bytes([c[2], c[3], c[4], c[5]]),
        payload: c[6..6 + pl].to_vec(),
        one_bit,
        auth: c[6 + pl..].to_vec(),
        crc,
        ..Default::default()
    })
}

/// The 13 frame-type bits at the start of `bits` as a number, or `None` when there are fewer.
fn ft_of(bits: &[u8]) -> Option<u16> {
    (bits.len() >= 13).then(|| {
        bits[..13]
            .iter()
            .fold(0u16, |a, &b| (a << 1) | (b & 1) as u16)
    })
}

/// Parses frame bits after the preamble (frame type first). Returns the uplink and the number of bits used, or `None`.
pub fn parse_frame(bits: &[u8]) -> Option<(Uplink, usize)> {
    let (container, rank, control, exact) = match_frame_type(ft_of(bits)?)?;
    let n = 8 * container + 16;
    if bits.len() < 13 + n {
        return None;
    }
    let llr: Vec<i8> = bits[13..13 + n].iter().map(|&b| from_hard(b)).collect();
    let dec = decode_ranks(&[(rank, &llr)])?;
    Some((uplink_of(container, rank, control, exact, &dec)?, 13 + n))
}

/// Whether the Sigfox chain runs (`sfx.state`): sent by `sfx.front` once after each configure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SigfoxState {
    /// The chain runs: the rate is high enough, and the dial is on a Sigfox zone when that is required.
    pub active: bool,
}

impl Item for SigfoxState {
    const KIND: &'static str = "sfx.state";
}

/// `sfx.front`: mixes the dial to 0 Hz, filters to 108 kHz and decimates to about 400 ksps.
pub struct Front {
    /// Whether it runs (inside `ism` only on a Sigfox zone).
    pub enabled: bool,
    /// Only on a zone (the `ism` rule).
    pub only_on_zone: bool,
    nco: Option<Nco>,
    decim: Option<FirDecimator<Complex32>>,
    mixed: Vec<Complex32>,
    state_due: bool,
}

impl Front {
    /// A front end. With `only_on_zone` it runs only when the dial is on a zone (or the frequency is not known).
    pub fn new(only_on_zone: bool) -> Front {
        Front {
            enabled: true,
            only_on_zone,
            nco: None,
            decim: None,
            mixed: Vec::new(),
            state_due: false,
        }
    }
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "sfx.front",
            vec![Port::iq("in")],
            vec![
                Port::iq("out"),
                Port::typed::<SigfoxState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let fs = f.rate;
        let d1 = ((fs / 400e3).floor() as usize).max(1);
        let fs1 = fs / d1 as f64;
        let n = (3.3 * fs / (50e3f64).max(fs1 - 216e3)).max(15.0).ceil() as usize | 1;
        let taps = design_by_taps(n, (108e3 / fs).min(0.5), Window::Hamming)?;
        self.decim = Some(FirDecimator::new(&taps, d1));
        self.nco = Some(Nco::new(fs, -f.signal_offset_hz()));
        let on_zone = f.tuned_hz <= 0.0 || zone_of(f.tuned_hz).is_some();
        self.enabled = fs >= 400e3 && (!self.only_on_zone || on_zone);
        self.state_due = true;
        let mut o = f.with_rate(fs1);
        o.centre_hz = f.tuned_hz;
        Ok(vec![o, Format::new(Kind::typed::<SigfoxState>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if self.state_due {
            self.state_due = false;
            io.items_out::<SigfoxState>(1).push(Timed {
                time: 0.0,
                value: SigfoxState {
                    active: self.enabled,
                },
            });
        }
        if !self.enabled {
            return;
        }
        let x = io.iq(0);
        let (Some(nco), Some(decim)) = (self.nco.as_mut(), self.decim.as_mut()) else {
            return;
        };
        self.mixed.resize(x.len(), Complex32::default());
        nco.mix(x, &mut self.mixed);
        decim.process(&self.mixed, io.iq_out(0));
    }
}

/// A carrier segment for the demodulator, with its samples.
#[derive(Clone, Debug, Default)]
pub struct Segment {
    /// Samples at `rate`.
    pub samples: Vec<Complex32>,
    /// Rate.
    pub rate: f64,
    /// The track frequency (Hz from the dial).
    pub freq_hz: f64,
    /// Index of the first sample (at `rate`).
    pub start: u64,
}

impl Item for Segment {
    const KIND: &'static str = "sfx.segment";
}

#[derive(Clone, Copy, Debug)]
struct Track {
    freq: f64,
    peak: f32,
    start: u64,
    last: u64,
    fed_frame: u64,
}

/// Frames the finder averages.
const AVG_FRAMES: usize = 8;
/// The least power per sample that the finder takes as signal (-100 dBFS, below the noise of any radio). Without it, digital
/// silence starts tracks on rounding errors, and such a track can hold a later carrier.
const MIN_SAMPLE_POWER: f32 = 1e-10;
/// Peaks closer than this to a stronger peak of the same frame are its sidelobes, Hz.
const SIDELOBE_HZ: f64 = 1500.0;
/// The longest track the finder passes on, seconds: the longest uplink at 100 bit/s (about 2.6 s) with the lead and the quiet
/// time that ends a track. A longer track is a steady carrier.
const MAX_TRACK_S: f64 = 4.0;
/// The weakest peak the finder takes, as a share of the strongest bin of the frame (50 dB).
const DYNAMIC_RANGE: f32 = 1e-5;
/// Columns of the waterfall.
const WF_COLUMNS: usize = 192;
/// Rows of the waterfall kept.
const WF_ROWS: usize = 160;

/// `sfx.finder`: a fine spectrogram, peaks and carrier tracks. An ended track goes to the demodulator with its samples.
pub struct Finder {
    rate: f64,
    n: usize,
    fft: Option<Fft>,
    win: Vec<f32>,
    min_power: f32,
    buf: Vec<Complex32>,
    fill: usize,
    work: Vec<Complex32>,
    frames: Vec<f32>,
    held: usize,
    next: usize,
    sum: Vec<f32>,
    band: Vec<usize>,
    vals: Vec<f32>,
    scratch: Vec<f32>,
    peaks: Vec<(f64, f32)>,
    tracks: Vec<Track>,
    ended: Vec<Track>,
    hist: Option<SampleRing<Complex32>>,
    pos: u64,
    frame: u64,
    /// Carriers passed to the demodulator.
    pub carriers: u64,
    out: Vec<Segment>,
    wf_acc: Vec<f32>,
    wf_frames: usize,
    /// The waterfall rows (192 columns each, newest last).
    pub waterfall: VecDeque<Vec<u8>>,
    /// Rows made.
    pub rows: u64,
    published: Published,
    waterfall_id: Option<SeriesId>,
}

impl Default for Finder {
    fn default() -> Self {
        let published = Published::from_decl(Arc::new(
            ResultsDecl::new()
                .with_fields(SigfoxFinderFields::SHAPE.fields)
                .with_series(&SIGFOX_WATERFALL),
        ));
        let waterfall_id = published.results().decl().series_id(SIGFOX_WATERFALL.key);
        Finder {
            rate: 0.0,
            n: 0,
            fft: None,
            win: vec![],
            min_power: 0.0,
            buf: vec![],
            fill: 0,
            work: vec![],
            frames: vec![],
            held: 0,
            next: 0,
            sum: vec![],
            band: vec![],
            vals: vec![],
            scratch: vec![],
            peaks: vec![],
            tracks: vec![],
            ended: vec![],
            hist: None,
            pos: 0,
            frame: 0,
            carriers: 0,
            out: vec![],
            wf_acc: vec![0.0; WF_COLUMNS],
            wf_frames: 0,
            waterfall: VecDeque::new(),
            rows: 0,
            published,
            waterfall_id,
        }
    }
}

impl Finder {
    fn bin_hz(&self, k: usize) -> f64 {
        let n = self.n as i64;
        let k = k as i64;
        let s = if k < n / 2 { k } else { k - n };
        s as f64 * self.rate / n as f64
    }

    /// One spectrogram frame of `buf`: the averaged spectrum, the waterfall, the peaks, the tracks and their endings.
    fn analyse(&mut self) {
        let n = self.n;
        let hop = n / 2;
        self.frame += 1;
        for i in 0..n {
            self.work[i] = self.buf[i] * self.win[i];
        }
        if let Some(f) = self.fft.as_mut() {
            f.process(&mut self.work);
        }
        let slot = self.next * n;
        for i in 0..n {
            self.frames[slot + i] = self.work[i].norm_sqr();
        }
        self.next = (self.next + 1) % AVG_FRAMES;
        self.held = (self.held + 1).min(AVG_FRAMES);
        // The sum is made again from the held frames. A running sum keeps rounding errors after a strong carrier ends, and they
        // look like a weak carrier in silence.
        self.sum.iter_mut().for_each(|v| *v = 0.0);
        for frame in self.frames.chunks_exact(n).take(self.held) {
            for (v, &p) in self.sum.iter_mut().zip(frame) {
                *v += p;
            }
        }
        let k = self.held as f32;
        self.vals.clear();
        for &i in &self.band {
            self.vals.push(self.sum[i] / k);
        }
        let noise = percentile(&self.vals, 0.5, &mut self.scratch).max(1e-30);
        // A peak must also be within DYNAMIC_RANGE of the strongest bin, so the sidelobes of a strong carrier over little noise
        // do not start tracks.
        let strongest = self.vals.iter().copied().fold(0.0f32, f32::max);
        let thr = (3.1 * noise)
            .max(strongest * DYNAMIC_RANGE)
            .max(self.min_power);
        let now = self.pos;
        for &i in &self.band {
            let col = (((self.bin_hz(i) + 96e3) / 192e3) * WF_COLUMNS as f64).floor();
            if (0.0..WF_COLUMNS as f64).contains(&col) {
                let c = col as usize;
                self.wf_acc[c] = self.wf_acc[c].max(self.sum[i] / k / noise);
            }
        }
        self.wf_frames += 1;
        if self.wf_frames as f64 * hop as f64 / self.rate >= 0.1 {
            let row: Vec<u8> = self
                .wf_acc
                .iter()
                .map(|&x| (10.0 * x.max(1e-9).log10() * 6.0).clamp(0.0, 255.0) as u8)
                .collect();
            self.waterfall.push_back(row);
            if self.waterfall.len() > WF_ROWS {
                self.waterfall.pop_front();
            }
            self.rows += 1;
            self.wf_acc.iter_mut().for_each(|v| *v = 0.0);
            self.wf_frames = 0;
        }
        let bin_w = self.rate / n as f64;
        self.peaks.clear();
        for &i in &self.band {
            let v = self.sum[i] / k;
            if v <= thr {
                continue;
            }
            let l = self.sum[(i + n - 1) % n] / k;
            let r = self.sum[(i + 1) % n] / k;
            if v < l || v < r {
                continue;
            }
            let d = log_parabolic(l as f64, v as f64, r as f64).offset;
            self.peaks.push((self.bin_hz(i) + d * bin_w, v));
        }
        // Only the strongest peak within SIDELOBE_HZ is a carrier: the others are its sidelobes.
        self.peaks.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        let mut kept = 0;
        for i in 0..self.peaks.len() {
            let p = self.peaks[i];
            if self.peaks[..kept]
                .iter()
                .all(|q| (q.0 - p.0).abs() > SIDELOBE_HZ)
            {
                self.peaks[kept] = p;
                kept += 1;
            }
        }
        self.peaks.truncate(kept);
        for t in self.tracks.iter_mut() {
            t.peak *= 0.995;
        }
        for &(f, v) in &self.peaks {
            if let Some(t) = self.tracks.iter_mut().find(|t| (t.freq - f).abs() <= 350.0) {
                if v >= 0.7 * t.peak {
                    t.freq += (f - t.freq) / 2.0;
                }
                t.peak = t.peak.max(v);
                t.last = now;
                t.fed_frame = self.frame;
            } else {
                self.tracks.push(Track {
                    freq: f,
                    peak: v,
                    start: now.saturating_sub((n + 10 * hop) as u64),
                    last: now,
                    fed_frame: self.frame,
                });
            }
        }
        let rate = self.rate;
        let frame = self.frame;
        self.ended.clear();
        let ended = &mut self.ended;
        self.tracks.retain(|t| {
            let lasted = (now - t.start) as f64 / rate;
            let quiet = (frame - t.fed_frame) as f64;
            let quiet_s = quiet * hop as f64 / rate;
            let end = if lasted >= 0.3 {
                quiet >= 4.0 && quiet_s >= 0.15
            } else {
                quiet >= 4.0
            };
            if lasted > MAX_TRACK_S && !end {
                // A dead track stays until it ends, and is never decoded.
                return true;
            }
            if end && (0.1..=MAX_TRACK_S).contains(&lasted) {
                ended.push(*t);
            }
            !end
        });
        let Some(hist) = self.hist.as_ref() else {
            return;
        };
        for t in &self.ended {
            self.carriers += 1;
            let pad = (0.06 * rate) as u64;
            let from = t.start.saturating_sub(pad).max(hist.start());
            let to = (t.last + pad).min(self.pos);
            if to <= from + 1000 {
                continue;
            }
            let mut samples = vec![Complex32::default(); (to - from) as usize];
            hist.copy(from, &mut samples);
            self.out.push(Segment {
                samples,
                rate,
                freq_hz: t.freq,
                start: from,
            });
        }
    }
}

impl Finder {
    /// Writes the count of carriers.
    fn put_carriers(&mut self) {
        self.published.put(&SigfoxFinderFields {
            carriers: self.carriers as i64,
        });
    }
}

impl Component for Finder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "sfx.finder",
            vec![Port::iq("in")],
            vec![Port::typed::<Segment>("segments").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        let mut n = 256;
        while self.rate / (2.0 * n as f64) >= 80.0 {
            n *= 2;
        }
        self.n = n;
        self.fft = Some(Fft::forward(n));
        self.win = vec![0.0; n];
        Window::Hann.fill_periodic(&mut self.win);
        self.min_power = MIN_SAMPLE_POWER * self.win.iter().map(|w| w * w).sum::<f32>();
        self.buf = vec![Complex32::default(); n];
        self.fill = 0;
        self.work = vec![Complex32::default(); n];
        self.frames = vec![0.0; AVG_FRAMES * n];
        self.held = 0;
        self.next = 0;
        self.sum = vec![0.0; n];
        self.band = (0..n).filter(|&i| self.bin_hz(i).abs() <= 98e3).collect();
        self.vals = Vec::with_capacity(self.band.len());
        self.tracks.clear();
        self.hist = SampleRing::new((3.8 * self.rate).max(1.0) as usize).ok();
        self.pos = 0;
        self.frame = 0;
        self.put_carriers();
        let mut o = f.with_kind(Kind::typed::<Segment>());
        o.rate = 0.0;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        if self.n == 0 {
            return;
        }
        let x = io.iq(0);
        let hop = self.n / 2;
        for &s in x {
            if let Some(h) = self.hist.as_mut() {
                h.push(s);
            }
            self.pos += 1;
            self.buf[self.fill] = s;
            self.fill += 1;
            if self.fill == self.n {
                self.analyse();
                self.buf.copy_within(hop.., 0);
                self.fill = self.n - hop;
            }
        }
        if !self.out.is_empty() {
            let rate = self.rate;
            let segs = self.out.drain(..).map(|s| Timed {
                time: s.start as f64 / rate,
                value: s,
            });
            io.items_out::<Segment>(0).extend(segs);
        }
        if io.tick() {
            self.put_carriers();
            let wf = &self.waterfall;
            let pts = || {
                wf.iter().enumerate().flat_map(|(r, row)| {
                    row.iter()
                        .enumerate()
                        .filter(|(_, &v)| v > 0)
                        .map(move |(c, &v)| [c as f32 + r as f32 * 1000.0, f32::from(v)])
                })
            };
            if let Some(id) = self.waterfall_id {
                self.published.results_mut().update_series(id, |s| {
                    if !s.line && s.points.iter().copied().eq(pts()) {
                        return false;
                    }
                    s.line = false;
                    s.points.clear();
                    s.points.extend(pts());
                    true
                });
            }
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn forget(&mut self, kept: &Kept) {
        self.tracks.clear();
        self.ended.clear();
        self.out.clear();
        self.held = 0;
        self.next = 0;
        self.fill = 0;
        self.sum.iter_mut().for_each(|v| *v = 0.0);
        self.wf_acc.iter_mut().for_each(|v| *v = 0.0);
        self.wf_frames = 0;
        self.waterfall.clear();
        if let Some(id) = self.waterfall_id {
            self.published.results_mut().update_series(id, |s| {
                let had = !s.points.is_empty();
                s.points.clear();
                had
            });
        }
        if !kept.field("carriers") {
            self.carriers = 0;
        }
        self.put_carriers();
    }
}

/// The largest carrier drift the demodulator searches, Hz per second (either sign).
const MAX_DRIFT_HZ_S: f64 = 60.0;
/// The step of the coarse drift search, Hz per second.
const DRIFT_STEP_HZ_S: f64 = 2.0;
/// The least SNR of the squared-signal line that is accepted for a frequency estimate, dB.
const MIN_LINE_SNR_DB: f32 = 10.0;

/// Samples averaged before the squaring: fewer noise per sample, and carriers within `rate / 6` remain in range.
const LINE_AVERAGE: usize = 3;

/// The carrier frequency (Hz) and drift (Hz per second) of a BPSK signal `x` at `rate`, measured at the middle of `x`: the squared
/// signal is a line at twice the carrier, so each candidate drift takes its chirp off the squared signal, and the candidate with
/// the strongest line wins (a coarse search, then a fine one around it). Returns `None` when no line stands
/// [`MIN_LINE_SNR_DB`] over the noise.
fn carrier_and_drift(x: &[Complex32], rate: f64) -> Option<(f64, f64)> {
    let x: Vec<Complex32> = x
        .as_chunks::<LINE_AVERAGE>()
        .0
        .iter()
        .map(|c| c.iter().sum::<Complex32>() / LINE_AVERAGE as f32)
        .collect();
    let rate = rate / LINE_AVERAGE as f64;
    let m = x.len();
    if m < 64 {
        return None;
    }
    let nfft = (2 * m).next_power_of_two().max(1024);
    let mut line = PowerLine::new(1, nfft).ok()?;
    let tc = m as f64 / 2.0 / rate;
    let mut sq = vec![Complex32::default(); m];
    let mut try_drift = |drift: f64| -> Option<(f32, f64)> {
        for (i, (y, v)) in sq.iter_mut().zip(&x).enumerate() {
            let dt = i as f64 / rate - tc;
            let (sn, cs) = approx::sin_cos(-TAU * drift * dt * dt);
            *y = v * v * Complex32::new(cs, sn);
        }
        let l = line.estimate(&sq).ok()?;
        Some((l.snr_db, l.freq * rate / 2.0))
    };
    let mut best: Option<(f32, f64, f64)> = None;
    let mut search = |from: f64, to: f64, step: f64, best: &mut Option<(f32, f64, f64)>| {
        let mut d = from;
        while d <= to + 1e-9 {
            if let Some((snr, f)) = try_drift(d) {
                if best.is_none_or(|b| snr > b.0) {
                    *best = Some((snr, f, d));
                }
            }
            d += step;
        }
    };
    search(-MAX_DRIFT_HZ_S, MAX_DRIFT_HZ_S, DRIFT_STEP_HZ_S, &mut best);
    let (_, _, coarse) = best?;
    search(
        coarse - DRIFT_STEP_HZ_S,
        coarse + DRIFT_STEP_HZ_S,
        DRIFT_STEP_HZ_S / 8.0,
        &mut best,
    );
    let (snr, f, drift) = best?;
    (snr >= MIN_LINE_SNR_DB).then_some((f, drift))
}

/// A frame whose CRC failed alone: its coded bits as LLRs, kept to decode with other ranks of the same message.
#[derive(Clone, Debug, PartialEq)]
pub struct SoftFrame {
    /// Container bytes.
    pub container: usize,
    /// Rank 1 to 3.
    pub rank: u8,
    /// Control message.
    pub control: bool,
    /// The frame type of the table entry.
    pub frame_type: u16,
    /// The LLRs of the coded container and CRC bits (positive means 0).
    pub llr: Vec<i8>,
    /// Bit rate.
    pub bit_rate: u32,
    /// Frequency offset from the input centre (Hz).
    pub offset_hz: f64,
    /// Level (dBFS).
    pub level_db: f32,
    /// Signal time (s).
    pub time: f64,
}

/// The result of a carrier segment.
#[derive(Clone, Debug, PartialEq)]
pub enum Demodulated {
    /// A frame with a good CRC.
    Uplink(Uplink),
    /// A frame with a known frame type whose CRC failed.
    Soft(SoftFrame),
    /// Nothing.
    Nothing,
}

/// The LLR scale: the mean decision magnitude of a frame becomes this value.
const LLR_MEAN: f32 = 24.0;

/// Demodulates one carrier segment at `bit_rate`.
pub fn demod_segment(seg: &Segment, bit_rate: u32) -> Demodulated {
    let r1 = 20.0 * bit_rate as f64;
    let step = seg.rate / r1;
    let n_out = (seg.samples.len() as f64 / step).floor() as usize;
    if n_out < 800 {
        return Demodulated::Nothing;
    }
    // 1. Mix and integrate by an exact moving average (cumulative sum with fractional edges).
    let mut nco = Nco::new(seg.rate, -seg.freq_hz);
    nco.set_phase_cycles(-seg.freq_hz * seg.start as f64 / seg.rate);
    let mut mixed = vec![Complex32::default(); seg.samples.len()];
    nco.mix(&seg.samples, &mut mixed);
    let mut cs = Vec::with_capacity(mixed.len() + 1);
    let mut acc = (0f64, 0f64);
    cs.push(acc);
    for y in &mixed {
        acc.0 += y.re as f64;
        acc.1 += y.im as f64;
        cs.push(acc);
    }
    let at = |t: f64| -> (f64, f64) {
        let i = (t.floor() as usize).min(cs.len() - 2);
        let fr = t - i as f64;
        let a = cs[i];
        let b = cs[i + 1];
        (a.0 + (b.0 - a.0) * fr, a.1 + (b.1 - a.1) * fr)
    };
    let mut x: Vec<Complex32> = (0..n_out)
        .map(|k| {
            let a = at(k as f64 * step);
            let b = at((k + 1) as f64 * step);
            Complex32::new(((b.0 - a.0) / step) as f32, ((b.1 - a.1) / step) as f32)
        })
        .collect();
    // 2. Fine frequency and drift from the squared signal.
    let m = x.len();
    let mut fine_hz = 0.0;
    if let Some((fc, slope)) = carrier_and_drift(&x, r1) {
        fine_hz = fc;
        let tc = m as f64 / 2.0 / r1;
        for (i, s) in x.iter_mut().enumerate() {
            let t = i as f64 / r1;
            let p = -TAU * (fc * t + 0.5 * slope * ((t - tc).powi(2) - tc * tc));
            let (sn, cs) = approx::sin_cos(p);
            *s *= Complex32::new(cs, sn);
        }
    }
    // 3. Symbol timing: the alignment with the most energy in 20-sample sums.
    let mut best = (0usize, -1f64);
    for a in 0..20 {
        let mut e = 0f64;
        let mut i = a;
        while i + 20 <= m {
            let s: Complex32 = x[i..i + 20].iter().sum();
            e += s.norm_sqr() as f64;
            i += 20;
        }
        if e > best.1 {
            best = (a, e);
        }
    }
    let syms: Vec<Complex32> = x[best.0..]
        .as_chunks::<20>()
        .0
        .iter()
        .map(|c| c.iter().sum::<Complex32>() / 20.0)
        .collect();
    // 4. Burst edges.
    let pw: Vec<f32> = syms.iter().map(|s| s.norm_sqr()).collect();
    let gate = 0.3 * percentile(&pw, 0.6, &mut Vec::new());
    let edge = |i: &usize| pw[*i] > gate && pw[*i + 1] > gate;
    let Some(start) = (0..pw.len().saturating_sub(1)).find(edge) else {
        return Demodulated::Nothing;
    };
    let Some(end) = (0..pw.len().saturating_sub(1))
        .rev()
        .find(edge)
        .map(|e| e + 1)
    else {
        return Demodulated::Nothing;
    };
    if end < start + 20 {
        return Demodulated::Nothing;
    }
    // 5. Differential decisions with the drift turn removed.
    let d: Vec<Complex32> = (0..syms.len() - 1)
        .map(|i| syms[i + 1] * syms[i].conj())
        .collect();
    let free: Vec<Complex32> = d.iter().map(|v| v * v / v.norm().max(1e-30)).collect();
    let mut soft = Vec::with_capacity(d.len());
    for i in 0..d.len() {
        let lo = i.saturating_sub(15);
        let hi = (i + 16).min(d.len());
        let s: Complex32 = free[lo..hi].iter().sum();
        let turn = approx::atan2(s.im, s.re) / 2.0;
        let (sn, cs) = approx::sin_cos(-turn as f64);
        soft.push((d[i] * Complex32::new(cs, sn)).re);
    }
    let bits: Vec<u8> = soft.iter().map(|&v| (v > 0.0) as u8).collect();
    let level = pw[start..=end.min(pw.len() - 1)]
        .iter()
        .map(|&v| v as f64)
        .sum::<f64>()
        / (end - start + 1) as f64;
    let level_db = (10.0 * level.max(1e-20).log10()) as f32;
    let offset_hz = seg.freq_hz + fine_hz;
    let time = seg.start as f64 / seg.rate;
    // 6. Preamble and frame, both polarities. The first frame type that matches is kept for the joint decoder.
    let mut kept: Option<SoftFrame> = None;
    for pol in [0u8, 1u8] {
        for off in -2i64..=5 {
            let s0 = start as i64 + off;
            if s0 < 0 {
                continue;
            }
            let s0 = s0 as usize;
            if s0 + 19 + 13 > bits.len() {
                continue;
            }
            let ok = (0..19)
                .filter(|&i| bits[s0 + i] ^ pol == PREAMBLE[i])
                .count();
            if ok < 17 {
                continue;
            }
            let b: Vec<u8> = bits[s0 + 19..].iter().map(|&v| v ^ pol).collect();
            if let Some((mut up, used)) = parse_frame(&b) {
                if s0 + 19 + used > end + 5 {
                    continue;
                }
                up.bit_rate = bit_rate;
                up.offset_hz = offset_hz;
                up.level_db = level_db;
                up.time = time;
                return Demodulated::Uplink(up);
            }
            if kept.is_some() {
                continue;
            }
            let Some((container, rank, control, exact)) = ft_of(&b).and_then(match_frame_type)
            else {
                continue;
            };
            let n = 8 * container + 16;
            let from = s0 + 19 + 13;
            if from + n > soft.len() || from + n > end + 5 {
                continue;
            }
            let v = &soft[from..from + n];
            let mean = v.iter().map(|x| x.abs()).sum::<f32>() / n as f32;
            let scale = if mean > 0.0 { LLR_MEAN / mean } else { 0.0 };
            let sign = if pol == 0 { -1.0 } else { 1.0 };
            let llr = v
                .iter()
                .map(|&x| (sign * x * scale).round().clamp(-127.0, 127.0) as i8)
                .collect();
            kept = Some(SoftFrame {
                container,
                rank,
                control,
                frame_type: exact,
                llr,
                bit_rate,
                offset_hz,
                level_db,
                time,
            });
        }
    }
    kept.map_or(Demodulated::Nothing, Demodulated::Soft)
}

/// Frames that failed alone, kept for the joint decoder.
const PENDING_CAP: usize = 16;
/// The longest time between the first and the last rank of one message, seconds.
const JOINT_WINDOW_S: f64 = 8.0;

/// The joint decoder of the repetitions: frames whose CRC failed alone are kept, and each new one is decoded together with the
/// kept frames of the other ranks of a message of the same size and bit rate.
#[derive(Default)]
pub struct Joint {
    pending: VecDeque<SoftFrame>,
}

impl Joint {
    /// Takes a frame that failed alone. Returns the uplinks of every frame of a message that now decodes (one for each rank).
    pub fn push(&mut self, f: SoftFrame) -> Vec<Uplink> {
        self.pending
            .retain(|p| (f.time - p.time).abs() <= JOINT_WINDOW_S);
        let same = |p: &SoftFrame| {
            p.container == f.container
                && p.control == f.control
                && p.bit_rate == f.bit_rate
                && p.rank != f.rank
        };
        // The newest kept frame of each other rank.
        let mut others: Vec<usize> = Vec::new();
        for (i, p) in self.pending.iter().enumerate().rev() {
            if same(p) && !others.iter().any(|&j| self.pending[j].rank == p.rank) {
                others.push(i);
            }
        }
        let mut tries: Vec<Vec<usize>> = Vec::new();
        if others.len() == 2 {
            tries.push(others.clone());
        }
        for &o in &others {
            tries.push(vec![o]);
        }
        for t in tries {
            let mut frames: Vec<(u8, &[i8])> = vec![(f.rank, &f.llr)];
            frames.extend(
                t.iter()
                    .map(|&i| (self.pending[i].rank, &self.pending[i].llr[..])),
            );
            let Some(dec) = decode_ranks(&frames) else {
                continue;
            };
            let Some(base) = uplink_of(f.container, f.rank, f.control, f.frame_type, &dec) else {
                continue;
            };
            let mut used: Vec<SoftFrame> = vec![f.clone()];
            let mut idx = t.clone();
            idx.sort_unstable_by(|a, b| b.cmp(a));
            for i in idx {
                if let Some(p) = self.pending.remove(i) {
                    used.push(p);
                }
            }
            return used
                .into_iter()
                .map(|s| Uplink {
                    frame_type: s.frame_type,
                    rank: s.rank,
                    bit_rate: s.bit_rate,
                    offset_hz: s.offset_hz,
                    level_db: s.level_db,
                    time: s.time,
                    joint: true,
                    ..base.clone()
                })
                .collect();
        }
        self.pending.push_back(f);
        while self.pending.len() > PENDING_CAP {
            self.pending.pop_front();
        }
        Vec::new()
    }
}

/// `sfx.demod`: segments in, uplinks out (100 bit/s first, then 600). Frames that fail alone go to the joint decoder.
#[derive(Default)]
pub struct Demod {
    /// Segments that did not decode.
    pub bad: u64,
    joint: Joint,
    published: Option<Published>,
}

impl Demod {
    /// A demodulator that also publishes the count of carriers that did not decode (for the `sigfox` stack).
    pub fn reporting() -> Demod {
        Demod {
            published: Some(Published::new(SigfoxDemodFields::SHAPE.fields)),
            ..Default::default()
        }
    }
}

impl Component for Demod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "sfx.demod",
            vec![Port::typed::<Segment>("in")],
            vec![Port::typed::<Uplink>("uplinks").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.put_bad();
        Ok(vec![inputs[0].with_kind(Kind::typed::<Uplink>())])
    }
    fn process(&mut self, io: &mut Io) {
        let segs = io.items::<Segment>(0);
        if segs.is_empty() {
            return;
        }
        let mut out = Vec::new();
        for s in segs {
            let r = match demod_segment(&s.value, 100) {
                Demodulated::Uplink(u) => Demodulated::Uplink(u),
                first => match (demod_segment(&s.value, 600), first) {
                    (Demodulated::Uplink(u), _) => Demodulated::Uplink(u),
                    (_, Demodulated::Soft(f)) | (Demodulated::Soft(f), _) => Demodulated::Soft(f),
                    _ => Demodulated::Nothing,
                },
            };
            match r {
                Demodulated::Uplink(u) => out.push(Timed {
                    time: s.time,
                    value: u,
                }),
                Demodulated::Soft(f) => {
                    let got = self.joint.push(f);
                    if got.is_empty() {
                        self.bad += 1;
                    }
                    out.extend(got.into_iter().map(|u| Timed {
                        time: u.time,
                        value: u,
                    }));
                }
                Demodulated::Nothing => self.bad += 1,
            }
        }
        self.put_bad();
        io.items_out::<Uplink>(0).extend(out);
    }
    fn results(&self) -> Option<&Results> {
        self.published.as_ref().map(|p| p.results())
    }
    fn forget(&mut self, kept: &Kept) {
        self.joint = Joint::default();
        if !kept.field("carriers_bad") {
            self.bad = 0;
        }
        self.put_bad();
    }
}

impl Demod {
    /// Writes the count of carriers that did not decode, if the demodulator reports.
    fn put_bad(&mut self) {
        if let Some(p) = self.published.as_mut() {
            p.put(&SigfoxDemodFields {
                carriers_bad: self.bad as i64,
            });
        }
    }
}

/// One merged message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// The row key: device id, counter and the signal time of the first frame when it was heard.
    #[serde(default)]
    pub key: String,
    /// First time (s of signal).
    pub first: f64,
    /// Last time.
    pub last: f64,
    /// Device id (8 upper-case hex).
    pub device_id: String,
    /// Counter.
    pub counter: u16,
    /// Payload text.
    pub payload: String,
    /// Payload bytes.
    pub payload_bytes: usize,
    /// Authentication (hex, not checked).
    pub auth: String,
    /// CRC (hex, checked).
    pub crc: String,
    /// Frame type of the first repetition.
    pub frame_type: String,
    /// LI.
    pub li: u8,
    /// REP.
    pub rep: bool,
    /// Downlink request.
    pub downlink: bool,
    /// Control flag.
    pub control: bool,
    /// Container bytes.
    pub container: usize,
    /// Ranks heard (bit 0 rank 1 ...).
    pub ranks: u8,
    /// Bit rate.
    pub bit_rate: u32,
    /// Repetitions.
    pub reps: u32,
    /// Frequencies (Hz from the dial).
    pub freqs: Vec<f64>,
    /// Level (dBFS), the strongest repetition's.
    pub level_db: f32,
    raw_payload: Vec<u8>,
    /// Each repetition's (time, frequency), for the same-transmission rule.
    seen: Vec<(f64, f64)>,
}

impl Shift for Message {
    fn shift_times(&mut self, by_s: f64) {
        self.first += by_s;
        self.last += by_s;
        for s in &mut self.seen {
            s.0 += by_s;
        }
    }
}

/// Messages kept.
pub const MESSAGE_CAP: usize = 300;

/// The message list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Messages {
    /// Newest first, up to [`MESSAGE_CAP`].
    pub list: VecDeque<Message>,
    /// Frames decoded (every repetition).
    pub frames: u64,
}

impl Messages {
    /// Adds an uplink at `dial` Hz from the input centre. Returns the log line of a new message.
    pub fn add(&mut self, u: &Uplink, dial: f64) -> Option<String> {
        self.frames += 1;
        let f = u.offset_hz - dial;
        let id = format!("{:08X}", u.device_id);
        if let Some(m) = self.list.iter_mut().find(|m| {
            m.device_id == id
                && m.counter == u.counter
                && m.raw_payload == u.payload
                && u.time - m.last <= 20.0
        }) {
            if m.seen
                .iter()
                .any(|&(t, fr)| (fr - f).abs() <= 2e3 && (u.time - t).abs() <= 0.5)
            {
                return None;
            }
            m.reps += 1;
            m.freqs.push(f);
            m.seen.push((u.time, f));
            m.ranks |= 1 << (u.rank.clamp(1, 3) - 1);
            m.level_db = m.level_db.max(u.level_db);
            m.last = m.last.max(u.time);
            m.first = m.first.min(u.time);
            return None;
        }
        let payload = if let Some(b) = u.one_bit {
            format!("bit {b}")
        } else if u.payload.is_empty() {
            "(no payload)".into()
        } else {
            hex(&u.payload)
        };
        self.list.push_front(Message {
            key: format!("{id}:{}:{:.3}", u.counter, u.time),
            first: u.time,
            last: u.time,
            device_id: id.clone(),
            counter: u.counter,
            payload: payload.clone(),
            payload_bytes: u.payload.len(),
            auth: hex(&u.auth),
            crc: format!("{:04X}", u.crc),
            frame_type: format!("{:04X}", u.frame_type),
            li: u.li,
            rep: u.rep,
            downlink: u.bf,
            control: u.control,
            container: u.container,
            ranks: 1 << (u.rank.clamp(1, 3) - 1),
            bit_rate: u.bit_rate,
            reps: 1,
            freqs: vec![f],
            level_db: u.level_db,
            raw_payload: u.payload.clone(),
            seen: vec![(u.time, f)],
        });
        self.list.truncate(MESSAGE_CAP);
        Some(format!(
            "Sigfox {id} #{} {payload} ({} bit/s)",
            u.counter, u.bit_rate
        ))
    }
    /// Forgets the messages.
    pub fn clear(&mut self) {
        self.list.clear();
        self.frames = 0;
    }
}

impl Shift for Messages {
    fn shift_times(&mut self, by_s: f64) {
        self.list.iter_mut().for_each(|m| m.shift_times(by_s));
    }
}

impl Tables for Messages {
    const FORMAT: StateFormat = StateFormat::new("ism.sigfox", 2);
    fn clear(&mut self) {
        Messages::clear(self);
    }
    fn adopt(&mut self, saved: Self) {
        self.list = saved.list;
        self.list.truncate(MESSAGE_CAP);
        self.frames = saved.frames;
    }
}

/// The event of a new message from uplink `u`, heard at `time` and `freq_hz`, with the log line `line`.
pub fn message_event(u: &Uplink, time: f64, freq_hz: f64, line: &str) -> Event {
    let id = format!("{:08X}", u.device_id);
    SIGFOX_MESSAGE.event(
        time,
        freq_hz,
        &SigfoxMessageEvent {
            source: &id,
            text: line,
            family: Family::Sensor,
            device_id: &id,
            seq: i64::from(u.counter),
            payload: &hex(&u.payload),
        },
    )
}

/// A message with the input centre for its frequency.
struct Located<'a> {
    message: &'a Message,
    centre_hz: f64,
}

impl Shaped for Located<'_> {
    type Row = SigfoxMessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let m = self.message;
        out.put(&SigfoxMessageRow {
            time_s: m.first,
            device_id: &m.device_id,
            seq: i64::from(m.counter),
            payload: &m.payload,
            payload_bytes: m.payload_bytes as i64,
            frequency: Hertz(self.centre_hz + m.freqs.first().copied().unwrap_or(0.0)),
            level_dbfs: f64::from(m.level_db),
            reps: i64::from(m.reps),
            ranks: i64::from(m.ranks),
            bit_rate: Hertz(f64::from(m.bit_rate)),
            auth: &m.auth,
            crc: &m.crc,
            frame_type: &m.frame_type,
            li: i64::from(m.li),
            container_bytes: m.container as i64,
            downlink: m.downlink,
            control: m.control,
        });
    }
}

/// `sfx.messages`: the message list and the results of the `sigfox` stack.
pub struct MessagesSink {
    msgs: Messages,
    published: Published,
    centre: f64,
    offset_hz: Option<f64>,
    last_decode: f64,
    now: f64,
    events: Vec<Event>,
}

impl Default for MessagesSink {
    fn default() -> Self {
        MessagesSink {
            msgs: Messages::default(),
            published: Published::from_decl(Arc::new(
                ResultsDecl::new()
                    .with_fields(SigfoxFields::SHAPE.fields)
                    .with_table(&SIGFOX_MESSAGES),
            )),
            centre: 0.0,
            offset_hz: None,
            last_decode: f64::NEG_INFINITY,
            now: 0.0,
            events: vec![],
        }
    }
}

impl MessagesSink {
    /// The messages heard.
    pub fn messages(&self) -> &Messages {
        &self.msgs
    }

    fn publish(&mut self) {
        let state = if self.now.max(self.last_decode) - self.last_decode < 300.0 {
            2
        } else {
            1
        };
        let zone = zone_of(self.centre).map(|z| z.name);
        let p = &mut self.published;
        let mut summary = String::new();
        let _ = match (state, self.msgs.list.front()) {
            (2, Some(m)) => write!(
                summary,
                "Sigfox {} #{} {}",
                m.device_id, m.counter, m.payload
            ),
            _ => write!(
                summary,
                "Sigfox: listening on {}",
                zone.unwrap_or("the dial")
            ),
        };
        p.results_mut().set_status(&Status {
            lock_state: LockState::from_level(state),
            snr_db: None,
            level_dbfs: None,
            offset_hz: self.offset_hz,
            summary,
        });
        p.put(&SigfoxFields {
            frames_ok: self.msgs.frames as i64,
            messages: self.msgs.list.len() as i64,
            zone,
        });
        let located: Vec<Located> = self
            .msgs
            .list
            .iter()
            .rev()
            .map(|message| Located {
                message,
                centre_hz: self.centre,
            })
            .collect();
        shaped::rows_of(
            &located,
            |l, w| w.write_str(&l.message.key),
            &SIGFOX_MESSAGES,
            self.now,
            p,
        );
    }
}

impl Component for MessagesSink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "sfx.messages",
            vec![Port::typed::<Uplink>("in")],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.centre = inputs[0].centre_hz;
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for u in io.items::<Uplink>(0) {
            if let Some(line) = self.msgs.add(&u.value, 0.0) {
                self.events.push(message_event(
                    &u.value,
                    u.value.time,
                    self.centre + u.value.offset_hz,
                    &line,
                ));
                self.offset_hz = Some(u.value.offset_hz);
            }
            self.last_decode = u.value.time;
            self.now = self.now.max(u.value.time);
            new = true;
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        if io.tick() {
            self.now += REPORT_S;
        }
        if io.tick() || new {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        MessagesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match MessagesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            MessagesCommand::Clear => {
                self.msgs.clear();
                self.published.clear_rows(&SIGFOX_MESSAGES);
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(SIGFOX_MESSAGES.key) {
            self.msgs.list.clear();
            self.published.clear_rows(&SIGFOX_MESSAGES);
        }
        if !kept.field("frames_ok") {
            self.msgs.frames = 0;
        }
        self.last_decode = f64::NEG_INFINITY;
        self.offset_hz = None;
        self.publish();
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.msgs.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.msgs.restore_state(state, self.now)?;
        self.publish();
        Ok(())
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

/// D-BPSK samples of frame bits at unit amplitude: one first symbol that sets the phase, then the same phase for 1 and an inverted phase for 0,
/// each change a raised-cosine transition over the first 10 % of the symbol. The carrier is at 0 Hz.
pub fn modulate(bits: &[u8], bit_rate: f64, rate: f64) -> Vec<Complex32> {
    let sps = rate / bit_rate;
    let nsym = bits.len() + 1;
    let n = (nsym as f64 * sps).ceil() as usize;
    let mut sym_phase = vec![0f64; nsym];
    for (i, &b) in bits.iter().enumerate() {
        sym_phase[i + 1] = sym_phase[i] + if b == 1 { 0.0 } else { PI };
    }
    (0..n)
        .map(|k| {
            let t = k as f64 / sps;
            let s = (t.floor() as usize).min(nsym - 1);
            let frac = t - s as f64;
            let prev = if s == 0 {
                sym_phase[0]
            } else {
                sym_phase[s - 1]
            };
            let target = sym_phase[s];
            let ph = if frac < 0.1 && s > 0 {
                let a = 0.5 - 0.5 * approx::sin_cos(PI * frac / 0.1).1 as f64;
                prev + (target - prev) * a
            } else {
                target
            };
            let (sn, cs) = approx::sin_cos(ph);
            Complex32::new(cs, sn)
        })
        .collect()
}

/// A test device of the generator.
#[derive(Clone, Debug, PartialEq)]
pub struct TestDevice {
    /// Device id.
    pub id: u32,
    /// Payload bytes.
    pub payload: Vec<u8>,
    /// The value of a one-bit message.
    pub one_bit: Option<u8>,
    /// The device requests a downlink.
    pub downlink: bool,
}

/// The generator's test devices, sent in turn.
pub fn test_devices() -> Vec<TestDevice> {
    let d = |id, payload: &[u8], one_bit, downlink| TestDevice {
        id,
        payload: payload.to_vec(),
        one_bit,
        downlink,
    };
    vec![
        d(0x00C0FFEE, &[0x1F, 0x3A, 0x00, 0x7C], None, false),
        d(0x0BADF00D, &[], Some(1), false),
        d(0x00ABCDEF, b"HELLO WORLD!", None, false),
        d(0x01234567, &[0x0A, 0x0B, 0x0C], None, true),
    ]
}

/// The test signal options.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// Bit rate (100 or 600).
    pub bit_rate: u32,
    /// Repetitions (3 or 1).
    pub reps: u8,
    /// Gap between messages (s).
    pub gap_s: f64,
    /// Seed of the frequencies and the authentication filler.
    pub seed: u64,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            bit_rate: 100,
            reps: 3,
            gap_s: 1.0,
            seed: 1,
        }
    }
}

/// The Sigfox generator: the test devices in turn, each message in `reps` ranks 0.5 s apart, each frame on a random 100 Hz
/// grid frequency within 80 kHz of the dial and at least 6 kHz from the one before. Clean, at unit amplitude.
///
/// # Errors
///
/// [`ConfigError::RateTooLow`] below [`MIN_RATE_HZ`], and the errors of the scene.
pub fn generator(rate: f64, p: GenParams) -> Result<CleanScene, ConfigError> {
    if !(rate >= MIN_RATE_HZ && rate.is_finite()) {
        return Err(ConfigError::RateTooLow {
            need: MIN_RATE_HZ,
            got: rate,
        });
    }
    let scene = Scene::new(rate)?;
    let reps = p.reps.clamp(1, 3) as u64;
    let bit_rate = p.bit_rate as f64;
    let devs = test_devices();
    let mut rng = Rng::new(p.seed ^ 0x5F0);
    let mut t = 0.2f64;
    let mut prev_f = 0.0f64;
    let mut k = 0u64;
    let gap = p.gap_s;
    let mut g = CleanScene::new(
        scene,
        rate,
        SignalInfo::new(bit_rate)
            .with_symbol_rate(bit_rate)
            .with_bit_rate(bit_rate),
    );
    g.scene_mut().add_source(Box::new(move || {
        let idx = (k / reps) as usize;
        let rank = (k % reps) as u8 + 1;
        k += 1;
        let d = &devs[idx % devs.len()];
        let counter = (0x123 + (idx / devs.len()) as u16) & 0xFFF;
        let bits = build_frame(
            d.id, counter, &d.payload, d.one_bit, d.downlink, rank, idx as u64,
        );
        let mut f;
        loop {
            f = ((rng.uniform() * 1600.0).floor() - 800.0) * 100.0;
            if (f - prev_f).abs() >= 6e3 {
                break;
            }
        }
        prev_f = f;
        let samples = modulate(&bits, bit_rate, rate);
        let start_s = t;
        t += samples.len() as f64 / rate + if (rank as u64) < reps { 0.5 } else { gap };
        Some(Burst {
            start_s,
            offset_hz: f,
            level_db: 0.0,
            samples,
        })
    }));
    Ok(g)
}

/// The lowest sample rate.
pub const MIN_RATE_HZ: f64 = 400e3;

sdrkit_core::commands! {
    /// The commands of [`MessagesSink`].
    pub enum MessagesCommand {
        /// Forgets the messages.
        Clear = commands::CLEAR,
    }
}

/// Builds the `sigfox` graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn build_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .catalog(catalog)
        .add("front", Front::new(false))
        .add("finder", Finder::default())
        .add("demod", Demod::reporting())
        .add("messages", MessagesSink::default())
        .input("front")
        .connect("front.out", "finder")
        .connect("finder", "demod")
        .connect("demod", "messages")
        .output("messages.events")
        .clock(REPORT_S)
        .status_from("messages")
        .build()
}

sdrkit_core::choice_enum! {
    /// The bit rate of the test signal.
    pub enum GenBitRate {
        /// 100 bit/s.
        B100 = ("100", "100 bit/s"),
        /// 600 bit/s.
        B600 = ("600", "600 bit/s"),
    }
}

sdrkit_core::choice_enum! {
    /// The repetitions of each message of the test signal.
    pub enum GenReps {
        /// Three frames.
        Three = ("3", "3 frames"),
        /// One frame.
        One = ("1", "1 frame"),
    }
}

/// The bit rate of the test signal.
pub const GEN_BIT_RATE: Setting<field::Choice<GenBitRate>> =
    Setting::choice("gen_bit_rate", "Test signal bit rate", GenBitRate::B100)
        .section("Test signal")
        .advanced();
/// The repetitions of each message of the test signal.
pub const GEN_REPS: Setting<field::Choice<GenReps>> =
    Setting::choice("gen_reps", "Repetitions", GenReps::Three)
        .section("Test signal")
        .advanced();
/// The gap between the messages of the test signal.
pub const GEN_GAP: Setting<field::Duration> =
    Setting::new("gen_gap", "Gap between messages", "s", 1.0)
        .range(0.1, 10.0, 0.1)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `sigfox` stack.
    pub enum SigfoxSetting {
        /// The bit rate of the test signal.
        GenBitRate = GEN_BIT_RATE,
        /// The repetitions of the test signal.
        GenReps = GEN_REPS,
        /// The gap between messages of the test signal.
        GenGap = GEN_GAP,
    }
}

/// The `sigfox` receiver.
///
/// # Errors
///
/// The errors of [`build_graph`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among them a
/// refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("sigfox"),
        build_graph(&Catalog::of(cfg)?)?,
    )
    .min_rate(MIN_RATE_HZ, &min_rate_message("Sigfox", MIN_RATE_HZ))
    .command(
        "clear",
        sdrkit_graph::CommandTarget::of("messages", "clear"),
    )
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The test signal options from settings that follow the schema of the stack.
pub fn gen_params(cfg: &GenConfig) -> GenParams {
    let s = &cfg.settings;
    GenParams {
        bit_rate: match s.get(&GEN_BIT_RATE) {
            GenBitRate::B100 => 100,
            GenBitRate::B600 => 600,
        },
        reps: match s.get(&GEN_REPS) {
            GenReps::Three => 3,
            GenReps::One => 1,
        },
        gap_s: s.get(&GEN_GAP),
        seed: cfg.seed,
    }
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator(cfg.rate_hz, gen_params(cfg))?))
}

/// Sigfox uplinks.
pub static SIGFOX: StackDescriptor = StackDescriptor {
    id: "sigfox",
    name: "Sigfox",
    group: "IoT, 868 / 902 MHz",
    blurb: "Sigfox uplinks: device ids, message counters and payloads (authentication codes shown, never checked).",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 862e6,
        max_hz: 928e6,
        tune_offset_hz: 250e3,
        ..Tuning::anywhere(868.130e6, 1.024e6, MIN_RATE_HZ, 0.2e6)
    },
    submodes: &[],
    settings: SigfoxSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Sigfox],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, RC1 uplinks", o, 0.0);
    c.seconds = o.seconds.max(10.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = SigfoxFields::read(r.fields());
        vec![
            at_least_value("frames_ok", f.as_ref().map_or(0, |f| f.frames_ok), 3),
            at_least_value("messages", f.as_ref().map_or(0, |f| f.messages), 1),
        ]
    });
    vec![c]
}

/// The test-signal cases of `sigfox`.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn hexbits(h: &str) -> Vec<u8> {
        let mut b = Vec::new();
        unpack(
            &crate::family::from_hex(h).expect("hex"),
            BitOrder::Msb,
            &mut b,
        );
        b
    }

    #[test]
    fn public_example_frame() {
        let b = hexbits("aaaaa35f01a0db334d00abad1deadc32758a");
        assert_eq!(&b[..19], &PREAMBLE);
        let (u, _) = parse_frame(&b[19..]).expect("parses");
        assert_eq!(u.frame_type, 0x035F);
        assert_eq!(u.container, 12);
        assert_eq!(u.rank, 1);
        assert_eq!(u.device_id, 0x004D33DB);
        assert_eq!(u.counter, 0x1A0);
        assert_eq!(hex(&u.payload), "ABAD1DEA");
        assert_eq!(hex(&u.auth), "DC32");
        assert_eq!(u.crc, 0x758A);
        assert!(!u.bf);
    }

    #[test]
    fn the_rank_codes_have_their_impulse_responses() {
        let imp = [1u8, 0, 0, 0, 0];
        assert_eq!(encode_rank(&imp, 1), vec![1, 0, 0, 0, 0]);
        assert_eq!(encode_rank(&imp, 2), vec![1, 1, 1, 0, 0]);
        assert_eq!(encode_rank(&imp, 3), vec![1, 0, 1, 0, 0]);
    }
}
