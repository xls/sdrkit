//! MFSK (MFSK4 to MFSK128L): the IZ8BLY varicode, the K = 7 code, the diagonal interleaver and Gray-coded tones. The receiver
//! takes a spectrum every sixteenth of a symbol, follows the symbol clock and the tone alignment, makes soft bits and decodes
//! them; it also receives the Shortwave Radiogram pictures. The generator sends the clean signal.
//!
//! Standard: N. Greenman ZL1BPU and N. Porcino IZ8BLY, "MFSK16 specification" (tones, Gray code, K = 7 code, interleaver,
//! varicode).

use crate::decoder::Emit;
use crate::decoder::HeardCommand;
use crate::decoder::{
    self, last_call, put_text, Heard, HfGenerator, Modulator, GEN_MESSAGE, GEN_SECTION,
};
use crate::fec::{SoftViterbi, MFSK_K7};
use crate::finder::CentreFinder;
use crate::picture::{self, HfPicture, PhaseRing, PicHeader, PictureReader};
use crate::typed::KeyboardFields;
use crate::util::Tone;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_codec_image::history::PictureHistory;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::Record;
use sdrkit_core::{
    Choice as _, ChoiceDecl, Complex32, ConfigError, GenConfig, Generator, LockState, Schema,
    Setting, SettingError, SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Outbox, Port};
use sdrkit_util_bits::gray::{gray, ungray};
use sdrkit_util_bits::interleave::DelayLanes;
use sdrkit_util_text::store::TextStore;
use sdrkit_util_text::varicode::mfsk as varicode;
use std::sync::Arc;

/// What a keyboard MFSK stack shows.
pub struct Keyboard<'a> {
    /// The variant's name.
    pub variant: &'a str,
    /// The audio centre, Hz.
    pub centre_hz: f64,
    /// The tuning error found, Hz.
    pub offset_hz: f64,
    /// The decoder's own SNR, dB.
    pub snr_db: f32,
    /// The text.
    pub store: &'a TextStore,
    /// The decision quality, 0 to 1.
    pub quality: f32,
}

/// Writes the keyboard fields, with the text through `heard`.
pub fn put_keyboard(res: &mut Published, k: &Keyboard, heard: &mut Heard) {
    heard.update(k.store);
    res.put(&KeyboardFields {
        variant: k.variant,
        centre_hz: Hertz(k.centre_hz),
        offset_hz: Hertz(k.offset_hz),
        snr_db: f64::from(k.snr_db),
        text: heard.text(),
        characters: k.store.total as i64,
        callsign: last_call(k.store),
        quality_pct: (f64::from(k.quality.clamp(0.0, 1.0)) * 100.0).round(),
    });
}

/// The IZ8BLY varicode bits of `text`, in the order sent.
pub fn varicode_bits(text: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    let _ = varicode::encode(text, &mut |b| v.push(b));
    v
}

/// The diagonal interleaver of `size` bits per symbol and `depth` stages: bit `i` is delayed by `depth i` symbols when sending
/// and `depth (size - 1 - i)` when receiving.
pub fn interleaver(size: usize, depth: usize, receive: bool) -> DelayLanes<f32> {
    DelayLanes::forney(size as u32, depth as u32, receive, 0.0)
}

/// An MFSK variant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MfskVariant {
    /// "MFSK16".
    pub name: &'static str,
    /// Its own sample rate.
    pub rate: f64,
    /// Samples per symbol.
    pub symlen: usize,
    /// Bits per symbol.
    pub bps: usize,
    /// Interleaver depth.
    pub depth: usize,
    /// Preamble symbols.
    pub preamble: usize,
    /// Sends pictures.
    pub pictures: bool,
}

impl MfskVariant {
    /// Tones.
    pub fn tones(&self) -> usize {
        1 << self.bps
    }
    /// Baud = tone spacing, Hz.
    pub fn spacing(&self) -> f64 {
        self.rate / self.symlen as f64
    }
    /// (tones - 1) x spacing.
    pub fn bandwidth(&self) -> f64 {
        (self.tones() - 1) as f64 * self.spacing()
    }
}

/// Every variant.
pub const VARIANTS: [MfskVariant; 11] = [
    MfskVariant {
        name: "MFSK4",
        rate: 8000.0,
        symlen: 2048,
        bps: 5,
        depth: 5,
        preamble: 107,
        pictures: false,
    },
    MfskVariant {
        name: "MFSK8",
        rate: 8000.0,
        symlen: 1024,
        bps: 5,
        depth: 5,
        preamble: 107,
        pictures: false,
    },
    MfskVariant {
        name: "MFSK11",
        rate: 11025.0,
        symlen: 1024,
        bps: 4,
        depth: 10,
        preamble: 107,
        pictures: false,
    },
    MfskVariant {
        name: "MFSK16",
        rate: 8000.0,
        symlen: 512,
        bps: 4,
        depth: 10,
        preamble: 107,
        pictures: true,
    },
    MfskVariant {
        name: "MFSK22",
        rate: 11025.0,
        symlen: 512,
        bps: 4,
        depth: 10,
        preamble: 107,
        pictures: false,
    },
    MfskVariant {
        name: "MFSK31",
        rate: 8000.0,
        symlen: 256,
        bps: 3,
        depth: 10,
        preamble: 107,
        pictures: false,
    },
    MfskVariant {
        name: "MFSK32",
        rate: 8000.0,
        symlen: 256,
        bps: 4,
        depth: 10,
        preamble: 107,
        pictures: true,
    },
    MfskVariant {
        name: "MFSK64",
        rate: 8000.0,
        symlen: 128,
        bps: 4,
        depth: 10,
        preamble: 180,
        pictures: true,
    },
    MfskVariant {
        name: "MFSK128",
        rate: 8000.0,
        symlen: 64,
        bps: 4,
        depth: 20,
        preamble: 214,
        pictures: true,
    },
    MfskVariant {
        name: "MFSK64L",
        rate: 8000.0,
        symlen: 128,
        bps: 4,
        depth: 400,
        preamble: 2500,
        pictures: false,
    },
    MfskVariant {
        name: "MFSK128L",
        rate: 8000.0,
        symlen: 64,
        bps: 4,
        depth: 800,
        preamble: 5000,
        pictures: false,
    },
];

/// A variant by name.
pub fn variant(name: &str) -> Option<MfskVariant> {
    VARIANTS
        .iter()
        .copied()
        .find(|v| v.name.eq_ignore_ascii_case(name))
}

/// The keyboard MFSK test text.
pub const TEST_TEXT: &str = "CQ CQ DE K0TST K0TST TEST MESSAGE 0123456789 K ";

/// How the code bits become a tone. The published description leaves the bit order open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conventions {
    /// The first code bit of a symbol is its most significant bit.
    pub msb_first: bool,
    /// The tone is the Gray code of the symbol (otherwise the symbol is the Gray code of the tone).
    pub tone_is_gray: bool,
    /// Bit i is delayed by depth x (size - 1 - i) on transmit (otherwise depth x i).
    pub reverse_delays: bool,
}

/// sdrkit's conventions.
pub const CONVENTIONS: Conventions = Conventions {
    msb_first: true,
    tone_is_gray: true,
    reverse_delays: false,
};

fn conventions() -> Conventions {
    CONVENTIONS
}

fn tx_interleaver(bps: usize, depth: usize, c: Conventions) -> DelayLanes<f32> {
    interleaver(bps, depth, c.reverse_delays)
}

fn rx_interleaver(bps: usize, depth: usize, c: Conventions) -> DelayLanes<f32> {
    interleaver(bps, depth, !c.reverse_delays)
}

/// The tone of a symbol.
pub fn tone_of(sym: u32, c: Conventions) -> u32 {
    if c.tone_is_gray {
        gray(sym)
    } else {
        ungray(sym)
    }
}

/// The symbol of a tone.
pub fn symbol_of(tone: u32, c: Conventions) -> u32 {
    if c.tone_is_gray {
        ungray(tone)
    } else {
        gray(tone)
    }
}

/// The coding of a variant from data bits to tones: the K = 7 code (the register starts at zero), words of `bps` code bits with
/// the first collected most significant, the interleaver (its delay lines start full of zeros) and the Gray code.
pub struct MfskCode {
    bps: usize,
    c: Conventions,
    enc: Encoder,
    il: DelayLanes<f32>,
    code: std::collections::VecDeque<u8>,
}

impl MfskCode {
    /// The coding of `v`.
    pub fn new(v: &MfskVariant) -> Self {
        let c = conventions();
        MfskCode {
            bps: v.bps,
            c,
            enc: Encoder::new(MFSK_K7),
            il: tx_interleaver(v.bps, v.depth, c),
            code: Default::default(),
        }
    }

    /// Adds one code bit; returns the tone of the word it completes.
    pub fn code_bit(&mut self, bit: u8) -> Option<u32> {
        self.code.push_back(bit);
        if self.code.len() < self.bps {
            return None;
        }
        let mut v = [0.0f32; 8];
        for (x, c) in v.iter_mut().zip(self.code.drain(..self.bps)) {
            *x = c as f32;
        }
        self.il.process(&mut v[..self.bps]);
        let mut sym = 0u32;
        for (i, &b) in v[..self.bps].iter().enumerate() {
            if b > 0.5 {
                let pos = if self.c.msb_first {
                    self.bps - 1 - i
                } else {
                    i
                };
                sym |= 1 << pos;
            }
        }
        Some(tone_of(sym, self.c))
    }

    /// Codes one data bit; returns the tone of the word that its two code bits complete (at most one: a word has 3 bits or
    /// more).
    pub fn data_bit(&mut self, bit: u8) -> Option<u32> {
        let p = self.enc.push_bit(bit);
        let a = self.code_bit(p & 1);
        let b = self.code_bit((p >> 1) & 1);
        a.or(b)
    }

    /// Code bits waiting for a word to complete.
    pub fn partial(&self) -> usize {
        self.code.len()
    }
}

/// The MFSK transmitter: varicode, K = 7 code, interleaver, Gray, one phase-continuous tone per symbol.
pub struct MfskModulator {
    v: MfskVariant,
    rate: f64,
    centre: f64,
    code: MfskCode,
    text: Vec<u8>,
    ti: usize,
    data: std::collections::VecDeque<u8>,
    preamble: usize,
    osc: Tone,
    tone: u32,
    left: f64,
    /// A picture to send after the text has been sent this many times (header, flush, prologue, pixels, flush).
    picture: Option<(usize, String, Vec<u8>, usize, usize, bool, usize)>,
    pic_phase: Vec<(f64, f64)>,
    pic_i: usize,
    rounds: usize,
}

impl MfskModulator {
    /// Sends `text` again and again at `centre` Hz.
    pub fn new(v: MfskVariant, rate: f64, centre: f64, text: &str) -> Self {
        let mut m = MfskModulator {
            v,
            rate,
            centre,
            code: MfskCode::new(&v),
            text: text.as_bytes().to_vec(),
            ti: 0,
            data: Default::default(),
            preamble: v.preamble,
            osc: Tone::new(rate),
            tone: 0,
            left: 0.0,
            picture: None,
            pic_phase: Vec::new(),
            pic_i: 0,
            rounds: 0,
        };
        m.tone = m.next_tone();
        m.left = m.symbol_samples();
        m
    }

    /// Also sends a grey or colour picture (`pixels`: grey W x H, or RGB W x H x 3) after `after_chars` characters, with
    /// `spp` samples per pixel at 8 kHz.
    pub fn with_picture(
        mut self,
        after_chars: usize,
        w: usize,
        h: usize,
        colour: bool,
        pixels: Vec<u8>,
        spp: usize,
    ) -> Self {
        let header = format!(
            "\nSending Pic:{w}x{h}{}{};",
            if colour { "C" } else { "" },
            if spp != 8 {
                format!("p{spp}")
            } else {
                String::new()
            }
        );
        self.picture = Some((after_chars, header, pixels, w, h, colour, spp));
        self
    }

    fn symbol_samples(&self) -> f64 {
        self.v.symlen as f64 / self.v.rate * self.rate
    }

    fn flush_bits(&self) -> usize {
        6 + (self.v.depth * (self.v.bps - 1) * self.v.bps).div_ceil(2) + 2
    }

    fn refill_data(&mut self) {
        if self.preamble > 0 {
            // The preamble: zero bits, so the encoder sends tone 0.
            let n = self.preamble * self.v.bps / 2;
            self.data.extend(std::iter::repeat_n(0, n.max(1)));
            self.preamble = 0;
            return;
        }
        if let Some((after, header, pixels, w, h, colour, spp)) = self.picture.clone() {
            if self.ti == after && self.pic_phase.is_empty() && self.rounds == 0 {
                self.data.extend(varicode_bits(header.as_bytes()));
                // The flush, then the picture follows from the symbol stream (see `next_tone`).
                self.data.push_back(1);
                self.data.extend(std::iter::repeat_n(0, self.flush_bits()));
                self.pic_phase = picture_tones(&self.v, self.centre, w, h, colour, &pixels, spp);
                self.pic_i = usize::MAX;
                self.ti += 1;
                return;
            }
        }
        let b = self.text[self.ti % self.text.len()];
        self.ti += 1;
        if self.ti.is_multiple_of(self.text.len()) {
            self.rounds += 1;
        }
        self.data.extend(varicode_bits(&[b]));
    }

    fn next_tone(&mut self) -> u32 {
        loop {
            if self.data.is_empty() {
                self.refill_data();
            }
            let bit = self.data.pop_front().unwrap_or(0);
            if let Some(t) = self.code.data_bit(bit) {
                return t;
            }
        }
    }
}

/// The tones of a picture at 8 kHz: the prologue (352 samples of the lowest tone), then each pixel's tone for `spp` samples. As
/// (frequency, seconds).
fn picture_tones(
    v: &MfskVariant,
    centre: f64,
    w: usize,
    h: usize,
    colour: bool,
    px: &[u8],
    spp: usize,
) -> Vec<(f64, f64)> {
    let lo = centre - v.bandwidth() / 2.0;
    let mut out = vec![(lo, 352.0 / 8000.0)];
    let d = spp as f64 / 8000.0;
    for y in 0..h {
        if colour {
            for c in 0..3 {
                for x in 0..w {
                    let p = px[(y * w + x) * 3 + c] as f64;
                    out.push((lo + p / 255.0 * v.bandwidth(), d));
                }
            }
        } else {
            for x in 0..w {
                out.push((lo + px[y * w + x] as f64 / 255.0 * v.bandwidth(), d));
            }
        }
    }
    out
}

impl Modulator for MfskModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        let lo = self.centre - (self.v.tones() - 1) as f64 / 2.0 * self.v.spacing();
        for o in out {
            if !self.pic_phase.is_empty() && self.pic_i != usize::MAX {
                // Sending the picture.
                let (f, d) = self.pic_phase[self.pic_i];
                *o = self.osc.next(f, 1.0);
                self.left -= 1.0;
                if self.left <= 0.0 {
                    self.pic_i += 1;
                    if self.pic_i >= self.pic_phase.len() {
                        self.pic_phase.clear();
                        self.pic_i = 0;
                        // The flush after the picture.
                        self.data.push_back(1);
                        let n = self.flush_bits();
                        self.data.extend(std::iter::repeat_n(0, n));
                        self.tone = self.next_tone();
                        self.left += self.symbol_samples();
                    } else {
                        self.left += self.pic_phase[self.pic_i].1 * self.rate;
                    }
                }
                let _ = d;
                continue;
            }
            let f = lo + self.tone as f64 * self.v.spacing();
            *o = self.osc.next(f, 1.0);
            self.left -= 1.0;
            if self.left <= 0.0 {
                if !self.pic_phase.is_empty()
                    && self.pic_i == usize::MAX
                    && self.data.is_empty()
                    && self.code.partial() == 0
                {
                    // The flush is out: the picture starts.
                    self.pic_i = 0;
                    self.left += self.pic_phase[0].1 * self.rate;
                    continue;
                }
                self.tone = self.next_tone();
                self.left += self.symbol_samples();
            }
        }
    }
}

/// The `centre` setting of the keyboard MFSK family.
pub const CENTRE: Setting<field::Hz> =
    Setting::new("centre", "Audio centre", "Hz", 1500.0).range(300.0, 3500.0, 1.0);
/// The `find_signal` setting of the keyboard MFSK family.
pub const FIND_SIGNAL: Setting<field::Bool> = Setting::new("find_signal", "Find signal", "", true)
    .help("The centre follows the signal while nothing is decoding.");

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
    default: "MFSK16",
    advanced: false,
    role: None,
};

sdrkit_core::settings! {
    /// The settings of [`MfskDecoder`].
    pub enum MfskSetting {
        /// The variant.
        Variant = VARIANT_SETTING,
        /// The audio centre.
        Centre = CENTRE,
        /// The centre follows the signal.
        FindSignal = FIND_SIGNAL,
    }
}

/// The width of the grey test picture, 0 for none.
pub const GEN_PICTURE_WIDTH: Setting<field::I64> =
    Setting::new("gen_picture_width", "Picture width", "px", 0)
        .range(0.0, 320.0, 1.0)
        .section(GEN_SECTION)
        .help("A grey test picture of this width follows 20 characters. 0 sends none.")
        .advanced();
/// The height of the grey test picture.
pub const GEN_PICTURE_HEIGHT: Setting<field::I64> =
    Setting::new("gen_picture_height", "Picture height", "px", 30)
        .range(1.0, 256.0, 1.0)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the MFSK stack.
    pub enum MfskGenSetting {
        /// The text sent.
        Message = GEN_MESSAGE,
        /// The picture width.
        PictureWidth = GEN_PICTURE_WIDTH,
        /// The picture height.
        PictureHeight = GEN_PICTURE_HEIGHT,
    }
}

const HOPS: usize = 16;

/// The MFSK receiver.
pub struct MfskDecoder {
    v: MfskVariant,
    c: Conventions,
    centre: f64,
    find: bool,
    dial: f64,
    rs: Option<Resampler<Complex32>>,
    rsbuf: Vec<Complex32>,
    nco: Nco,
    mixed: Vec<Complex32>,
    hist: SampleRing<Complex32>,
    spec: WindowedFft,
    hop: usize,
    hop_n: u64,
    last_dec: u64,
    pos: Vec<i32>,
    energy: Vec<f32>,
    peak_avg: [f32; HOPS],
    phase: usize,
    align: i32,
    symbols: u64,
    mags: Vec<f32>,
    deint: DelayLanes<f32>,
    vit: [SoftViterbi; 2],
    vmetric: [f32; 2],
    ncode: u64,
    prev_code: f32,
    vari: varicode::Decoder,
    quality: f32,
    locked: bool,
    replay: Vec<u8>,
    snr: f32,
    store: TextStore,
    heard: Heard,
    status: Status,
    finder: CentreFinder,
    t_in: u64,
    // pictures
    reader: Option<PictureReader>,
    pics: PictureHistory<HfPicture>,
    /// A picture that a reset of the signal cut off. The next block sends it, finished.
    cut: Option<HfPicture>,
    pic_wait: Option<PicHeader>,
    ring: Option<PhaseRing>,
}

impl MfskDecoder {
    /// A receiver of MFSK16 at the default settings.
    pub fn new() -> Self {
        Self::with_variant(VARIANTS[3])
    }

    /// A receiver of one variant.
    pub fn with_variant(v: MfskVariant) -> Self {
        let c = conventions();
        let t = v.tones() as i32;
        let pos: Vec<i32> = (-(t / 2 + 4) * 4..=(t / 2 + 4) * 4).collect();
        let n = pos.len();
        MfskDecoder {
            v,
            c,
            centre: 1500.0,
            find: true,
            dial: 0.0,
            rs: (v.rate != 8000.0).then(|| {
                Resampler::new(8000.0, v.rate).expect("the variant has a fixed positive rate")
            }),
            rsbuf: Vec::new(),
            nco: Nco::new(v.rate, -1500.0),
            mixed: Vec::new(),
            hist: SampleRing::new(v.symlen).expect("a ring of at least one sample"),
            spec: WindowedFft::new(4 * v.symlen, v.symlen, Window::Rectangular)
                .expect("a window that fits the transform"),
            hop: 0,
            hop_n: 0,
            last_dec: 0,
            pos,
            energy: vec![0.0; n],
            peak_avg: [0.0; HOPS],
            phase: 0,
            align: 0,
            symbols: 0,
            mags: vec![0.0; v.tones()],
            deint: rx_interleaver(v.bps, v.depth, c),
            vit: [SoftViterbi::new(MFSK_K7, 45), SoftViterbi::new(MFSK_K7, 45)],
            vmetric: [0.0; 2],
            ncode: 0,
            prev_code: 0.0,
            vari: varicode::Decoder::new(),
            quality: 0.0,
            locked: false,
            replay: Vec::new(),
            snr: 0.0,
            store: decoder::store(8000, 0),
            heard: Heard::new(v.name),
            status: Status::default(),
            finder: CentreFinder::new(v.bandwidth() as f32 + v.spacing() as f32),
            t_in: 0,
            reader: None,
            pics: PictureHistory::new(8),
            cut: None,
            pic_wait: None,
            ring: None,
        }
    }

    fn make_ring(&mut self) {
        self.ring = self.v.pictures.then(|| {
            let half = self.v.bandwidth() / 2.0 + (self.v.bandwidth() / 2.0).max(60.0);
            PhaseRing::new(8000.0, self.centre + self.offset_hz(), half, 6.0)
        });
    }

    fn reset_signal(&mut self) {
        let keep_store = std::mem::replace(&mut self.store, decoder::store(1, 1));
        let keep_heard = std::mem::replace(&mut self.heard, Heard::new(self.v.name));
        let pics = std::mem::replace(&mut self.pics, PictureHistory::new(8));
        let (centre, find, dial, t_in) = (self.centre, self.find, self.dial, self.t_in);
        let reader = self.reader.take();
        *self = Self::with_variant(self.v);
        self.store = keep_store;
        self.heard = keep_heard;
        self.heard.set_stack(self.v.name);
        self.pics = pics;
        self.centre = centre;
        self.find = find;
        self.dial = dial;
        self.t_in = t_in;
        self.nco = Nco::new(self.v.rate, -centre);
        self.make_ring();
        if let Some(r) = reader {
            // An MFSK picture being received is ended where it is.
            let p = r.picture();
            self.cut = (p.rows > 0).then(|| p.clone());
            self.pics.finish(p.id, p);
        }
    }

    /// One hop: the spectrum of the last symbol length.
    fn hop(&mut self, out: &mut Outbox) {
        self.spec.run(self.hist.last(self.v.symlen));
        let bins = self.spec.bins();
        let t = self.v.tones();
        let nb = 4 * self.v.symlen as i32;
        let t2 = 2 * (t as i32 - 1);
        let mut sum = 0.0;
        let mut max = 0.0f32;
        for k in 0..t {
            let q = 4 * k as i32 - t2 + self.align;
            let m = bins[q.rem_euclid(nb) as usize].norm();
            self.mags[k] = m;
            sum += m;
            max = max.max(m);
        }
        let peak = if sum > 0.0 { max / sum } else { 0.0 };
        let h = (self.hop_n % HOPS as u64) as usize;
        let a = if self.symbols < 10 {
            1.0 / (self.symbols as f32 + 1.0)
        } else {
            0.1
        };
        self.peak_avg[h] += a * (peak - self.peak_avg[h]);
        if h == self.phase && self.hop_n >= self.last_dec + HOPS as u64 / 2 {
            self.last_dec = self.hop_n;
            // Energy per position for the alignment, as a share of the symbol's energy over the positions: every symbol
            // weighs the same, so a loud one (the AGC overshoot at the start of a burst after silence, a static crash) does not
            // decide the shift. A symbol without energy (digital silence) adds nothing.
            let total: f32 = self
                .pos
                .iter()
                .map(|&q| bins[q.rem_euclid(nb) as usize].norm_sqr())
                .sum();
            if total > 0.0 {
                for (i, &q) in self.pos.iter().enumerate() {
                    let m = bins[q.rem_euclid(nb) as usize].norm_sqr() / total;
                    self.energy[i] += 0.05 * (m - self.energy[i]);
                }
            }
            self.symbol(out);
            // Move the decision one hop towards the best phase.
            let best = (0..HOPS)
                .max_by(|&x, &y| self.peak_avg[x].total_cmp(&self.peak_avg[y]))
                .expect("the range is not empty");
            if best != self.phase && self.peak_avg[best] > 1.03 * self.peak_avg[self.phase] {
                let d = (best + HOPS - self.phase) % HOPS;
                self.phase = if d <= HOPS / 2 {
                    (self.phase + 1) % HOPS
                } else {
                    (self.phase + HOPS - 1) % HOPS
                };
            }
        }
        self.hop_n += 1;
    }

    fn align(&mut self) {
        let t = self.v.tones() as i32;
        let base = self.pos[0];
        let range = if self.symbols >= 64 { 12 } else { 2 };
        let mut scores = [(0.0f32, 0i32); 25];
        let mut top = f32::MIN;
        for (n, a) in (-range..=range).enumerate() {
            let mut inside = 0.0;
            for k in 0..t {
                let q = 4 * k - 2 * (t - 1) + a;
                inside += self.energy[(q - base) as usize];
            }
            // Just outside the tone window.
            let lo = -2 * (t - 1) + a - 4;
            let hi = 4 * (t - 1) - 2 * (t - 1) + a + 4;
            let outside = self.energy[(lo - base) as usize] + self.energy[(hi - base) as usize];
            let s = inside - outside * t as f32 / 2.0;
            scores[n] = (s, a);
            top = top.max(s);
        }
        // Of the shifts that fit about as well as the best (a tone the text never uses leaves a tie), the one whose lowest tone
        // is in use (the preamble and idle send the lowest tone), then the one nearest the centre.
        let mut best = (f32::MIN, self.align);
        let mut key = (f32::MIN, i32::MIN);
        for &(s, a) in &scores[..(2 * range + 1) as usize] {
            if s >= top - 0.02 * top.abs() {
                let low = self.energy[(-2 * (t - 1) + a - base) as usize];
                let k = (
                    (low / top.abs().max(1e-20) * t as f32 * 4.0).min(1.0),
                    -a.abs(),
                );
                if k.0 > key.0 + 0.1 || ((k.0 - key.0).abs() <= 0.1 && k.1 > key.1) {
                    key = k;
                    best = (s, a);
                }
            }
        }
        self.align = best.1;
    }

    fn symbol(&mut self, out: &mut Outbox) {
        self.symbols += 1;
        if self.symbols.is_multiple_of(8) {
            self.align();
        }
        let t = self.v.tones();
        let bps = self.v.bps;
        let sum: f32 = self.mags.iter().sum();
        let (mut best, mut bi) = (0.0f32, 0);
        for (k, &m) in self.mags.iter().enumerate() {
            if m > best {
                best = m;
                bi = k;
            }
        }
        let others = (sum - best) / (t - 1) as f32;
        let snr = 20.0 * (best / others.max(1e-12)).max(1e-6).log10();
        self.snr += 0.1 * (snr - self.snr);
        let mut soft = [0.0f32; 8];
        let mean = sum / t as f32;
        let erased = self.mags.iter().all(|&m| m > 0.5 * mean) && best < 2.0 * mean;
        if sum > 0.0 && !erased {
            for (tone, &m) in self.mags.iter().enumerate() {
                let sym = symbol_of(tone as u32, self.c);
                for (i, s) in soft.iter_mut().enumerate().take(bps) {
                    let pos = if self.c.msb_first { bps - 1 - i } else { i };
                    *s += if (sym >> pos) & 1 == 1 { m } else { -m };
                }
            }
            for s in soft.iter_mut().take(bps) {
                *s /= sum;
            }
        }
        let _ = bi;
        self.deint.process(&mut soft[..bps]);
        // Two pairings when the bits per symbol are odd: decoder 0 pairs code bits (0, 1), (2, 3) .... Decoder 1 pairs (1, 2),
        // (3, 4) .... The one with the better smoothed metric speaks.
        let odd = bps % 2 == 1;
        for k in 0..bps {
            let cur = soft[k];
            let n = self.ncode;
            self.ncode += 1;
            let prev = self.prev_code;
            self.prev_code = cur;
            if n == 0 {
                continue;
            }
            let which = if n % 2 == 1 { 0 } else { 1 };
            if which == 1 && !odd {
                continue;
            }
            let r = self.vit[which].push(prev, cur);
            let g = self.vit[which].gain / (prev.abs() + cur.abs()).max(1e-6);
            self.vmetric[which] += 0.02 * (g - self.vmetric[which]);
            let better = if !odd || self.vmetric[0] >= self.vmetric[1] {
                0
            } else {
                1
            };
            if let Some(bit) = r {
                if which == better {
                    self.bit(bit, out);
                }
            }
        }
        let best = self.vmetric[0].max(self.vmetric[1]);
        self.quality = best;
    }

    fn bit(&mut self, bit: u8, out: &mut Outbox) {
        let lock_now = self.quality > 0.62;
        if lock_now && !self.locked {
            self.locked = true;
            self.heard
                .logs
                .push(format!("{} locked at {:.0} Hz", self.v.name, self.centre));
            // Replay the bits decoded just before the lock.
            let bits = std::mem::take(&mut self.replay);
            for b in bits {
                self.vari_bit(b, out);
            }
        } else if self.quality < 0.45 && self.locked {
            self.locked = false;
            self.vari.reset();
        }
        if self.locked {
            self.vari_bit(bit, out);
        } else {
            self.replay.push(bit);
            if self.replay.len() > 64 {
                self.replay.remove(0);
            }
        }
    }

    fn make_ring_if_moved(&mut self) {
        let want = self.centre + self.offset_hz();
        if self
            .ring
            .as_ref()
            .is_some_and(|r| (r.centre() - want).abs() > self.v.spacing() / 2.0)
        {
            // The band moved with the alignment: the reader needs the history, so it is kept. Only a large move rebuilds it.
        }
    }

    fn vari_bit(&mut self, bit: u8, out: &mut Outbox) {
        if self.reader.is_some() {
            return;
        }
        if let Some(Ok(varicode::Symbol::Primary(b))) = self.vari.push(bit) {
            if b == 0 {
                return;
            }
            let time = self.t_in as f64 / 8000.0;
            let snr = self.snr;
            self.heard.char(
                &mut self.store,
                out,
                b,
                time,
                100,
                0,
                self.dial + self.centre,
                snr,
            );
            if self.v.pictures {
                let mut tail = [0u8; 40];
                let n = tail_bytes(&self.store, &mut tail);
                if let Some(h) = PicHeader::find(&tail[..n]) {
                    if self.pic_wait.is_none() && self.reader.is_none() {
                        self.pic_wait = Some(h);
                    }
                }
            }
        }
    }

    /// Tuning error found, Hz.
    pub fn offset_hz(&self) -> f64 {
        self.align as f64 * self.v.spacing() / 4.0
    }
}

impl Default for MfskDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// The newest characters of a store as bytes (at most `out.len()`), oldest first. Returns how many.
pub fn tail_bytes(store: &TextStore, out: &mut [u8]) -> usize {
    let total = store.text().count();
    let skip = total.saturating_sub(out.len());
    let mut n = 0;
    for c in store.text().skip(skip) {
        out[n] = if c.is_ascii() { c as u8 } else { b'?' };
        n += 1;
    }
    n
}

impl sdrkit_graph::Decoder for MfskDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.mfsk"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        MfskSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match MfskSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MfskSetting::Variant => {
                let i = VARIANT_SETTING.read(id, value)?;
                let v = VARIANTS.get(i).copied().ok_or(SettingError::NotAChoice {
                    id,
                    index: i as u16,
                })?;
                if v != self.v {
                    self.v = v;
                    self.finder = CentreFinder::new(v.bandwidth() as f32 + v.spacing() as f32);
                    self.reset_signal();
                }
            }
            MfskSetting::Centre => {
                self.centre = CENTRE.read(id, value)?;
                self.reset_signal();
            }
            MfskSetting::FindSignal => self.find = FIND_SIGNAL.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let i = VARIANTS.iter().position(|v| *v == self.v).unwrap_or(3);
        values_of([
            (MfskSetting::Variant, Value::Choice(i as u16)),
            (MfskSetting::Centre, Value::Hz(self.centre)),
            (MfskSetting::FindSignal, Value::Bool(self.find)),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("mfsk")
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.reset_signal();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.reset_signal();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        if let Some(p) = self.cut.take() {
            out.picture(
                self.t_in as f64 / 8000.0,
                p.to_picture(sdrkit_graph::PictureState::Finished),
            );
        }
        // In pieces that end where the signal finder decides, so the result does not depend on how the input is cut.
        let mut rest = x;
        while !rest.is_empty() {
            let n = rest.len().min(self.finder.until_frame());
            let (piece, r) = rest.split_at(n);
            rest = r;
            self.piece(piece, out);
        }
    }
    fn report(&mut self, res: &mut Published) {
        self.report_inner(res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        HeardCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let p = self.results();
        self.heard.invoke(id, args, &p)
    }
    fn clear(&mut self) {
        self.store.clear();
        self.heard.clear();
        self.pics.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset_signal();
        if !kept.field("text") {
            self.store.clear();
        }
        self.heard.forget(kept);
        if !kept.field("pictures") {
            self.pics.clear();
        }
    }
}

impl MfskDecoder {
    fn piece(&mut self, x: &[Complex32], out: &mut Outbox) {
        // The picture reader works on the 8 kHz audio itself. The text decoder keeps its timing but rests meanwhile.
        if let Some(ring) = &mut self.ring {
            ring.push(x);
        }
        if let Some(h) = self.pic_wait.take() {
            let lo = self.centre + self.offset_hz() - self.v.bandwidth() / 2.0;
            let latency = (self.v.depth * (self.v.bps - 1) + 45 / self.v.bps + 4) as f64
                * self.v.symlen as f64
                / self.v.rate;
            self.make_ring_if_moved();
            let symlen8 = self.v.symlen as f64 * 8000.0 / self.v.rate;
            let id = self.pics.next_id();
            self.reader = Some(PictureReader::new(
                h,
                id,
                self.t_in,
                lo,
                self.v.bandwidth(),
                latency,
                self.v.name,
                self.v.spacing(),
                symlen8,
            ));
        }
        let freq = self.dial + self.centre;
        if let (Some(r), Some(ring)) = (&mut self.reader, &self.ring) {
            if let Some(done) = r.feed(ring, out, freq) {
                self.heard.logs.push(format!(
                    "{}: picture {}x{} {}",
                    self.v.name,
                    done.width(),
                    done.height(),
                    if done.colour { "colour" } else { "grey" }
                ));
                out.picture(
                    self.t_in as f64 / 8000.0,
                    done.to_picture(sdrkit_graph::PictureState::Finished),
                );
                self.pics.finish(done.id, done);
                self.reader = None;
                self.vari.reset();
                self.replay.clear();
            }
        }
        let input: &[Complex32] = match &mut self.rs {
            Some(r) => {
                self.rsbuf.clear();
                r.process(x, &mut self.rsbuf);
                &self.rsbuf
            }
            None => x,
        };
        self.mixed.clear();
        self.mixed.resize(input.len(), Complex32::default());
        self.nco.mix(input, &mut self.mixed);
        let hop = self.v.symlen / HOPS;
        let mixed = std::mem::take(&mut self.mixed);
        for &s in &mixed {
            self.hist.push(s);
            self.hop += 1;
            if self.hop == hop {
                self.hop = 0;
                self.hop(out);
            }
        }
        self.mixed = mixed;
        self.t_in += x.len() as u64;
        // The signal finder decides at the end of the piece, after the decoder has seen it.
        let busy = self.locked && self.store.plausible()
            || self.reader.is_some()
            || self.pic_wait.is_some();
        if self.find {
            if let Some(c) = self.finder.feed(x, self.centre as f32, !busy) {
                self.heard.logs.push(format!("signal found at {c:.0} Hz"));
                self.centre = c as f64;
                self.reset_signal();
            }
        }
    }
    fn report_inner(&mut self, res: &mut Published) {
        let offset = self.offset_hz();
        let st = &mut self.status;
        st.lock_state = if self.locked {
            LockState::Locked
        } else {
            LockState::Searching
        };
        st.snr_db = Some(self.snr);
        st.offset_hz = Some(offset);
        let what = if self.locked { "locked" } else { "searching" };
        put_text(&mut st.summary, format_args!("{} {what}", self.v.name));
        res.results_mut().set_status(st);
        put_keyboard(
            res,
            &Keyboard {
                variant: self.v.name,
                centre_hz: self.centre,
                offset_hz: self.offset_hz(),
                snr_db: self.snr,
                store: &self.store,
                quality: self.quality,
            },
            &mut self.heard,
        );
        let current = self.reader.as_ref().map(PictureReader::picture);
        res.put(&picture::picture_fields(current.as_ref(), &self.pics));
        self.heard.report(res);
    }
}

impl MfskDecoder {
    /// The finished pictures.
    pub fn pictures(&self) -> Vec<&HfPicture> {
        self.pics.iter().map(|(_, p)| p).collect()
    }
}

/// The generator. It reads the settings `variant`, `centre`, `gen_message`, `gen_picture_width` and `gen_picture_height` (a
/// grey test picture after 20 characters).
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let v = VARIANTS[usize::from(s.entry(&VARIANT_SETTING)).min(VARIANTS.len() - 1)];
    let centre = s.get(&CENTRE);
    let mut m = MfskModulator::new(v, cfg.rate_hz, centre, &decoder::message(cfg, TEST_TEXT));
    let (w, h) = (s.get(&GEN_PICTURE_WIDTH), s.get(&GEN_PICTURE_HEIGHT));
    if w > 0 {
        let (w, h) = (w as usize, h as usize);
        m = m.with_picture(20, w, h, false, picture::test_grey(w, h), 8);
    }
    let baud = v.rate / v.symlen as f64;
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        m,
        SignalInfo::new(v.bandwidth() + v.spacing())
            .with_symbol_rate(baud)
            .with_bit_rate(baud * v.bps as f64 / 2.0),
    ))
}

sdrkit_core::settings! {
    /// The settings of the MFSK sender.
    pub enum MfskTxSetting {
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
    }
}

/// The characters after the start of a burst: CR, STX, CR.
const TX_START: [u8; 3] = [b'\r', 2, b'\r'];
/// MFSK16: the symbols of the lowest tone at the start of a burst.
const MFSK16_IDLE: usize = 8;
/// Zero data bits of the flush beyond the span of the interleaver: the code memory and the receiver's decision delay.
const FLUSH_MARGIN: usize = 70;

impl MfskVariant {
    /// The start of a burst: zero code bits, then zero data bits. MFSK16: 8 symbols of the lowest tone. The others: (2 P) mod
    /// S code bits and P / 3 data bits (P the preamble count, S the bits per symbol); the L variants send no data bits.
    pub fn tx_preamble(&self) -> (usize, usize) {
        if self.name == "MFSK16" {
            return (0, MFSK16_IDLE * self.bps / 2);
        }
        let code = (2 * self.preamble) % self.bps;
        let data = if self.name.ends_with('L') {
            0
        } else {
            self.preamble / 3
        };
        (code, data)
    }

    /// The zero data bits after the closing 1 of a burst: the span of the interleaver, (S - 1) depth S / 2 bits, and 70.
    pub fn tx_flush(&self) -> usize {
        (self.bps - 1) * self.depth * self.bps / 2 + FLUSH_MARGIN
    }

    /// The audio frequency of tone 0 at `centre`, Hz.
    pub fn low_hz(&self, centre: f64) -> f64 {
        centre - (self.tones() - 1) as f64 * self.spacing() / 2.0
    }
}

/// The MFSK coder. One phase-continuous tone per symbol; tone k at the lowest tone + k spacing. A burst is the variant's start
/// ([`MfskVariant::tx_preamble`]), CR, STX, CR, the text as UTF-8 bytes in the varicode, a 1, the flush of
/// [`MfskVariant::tx_flush`] zero bits, then zero bits until the last word is complete.
pub struct MfskCoder {
    v: MfskVariant,
    low: f64,
    code: MfskCode,
}

impl MfskCoder {
    /// A coder of `v` around the audio centre `centre`.
    pub fn new(v: MfskVariant, centre: f64) -> Self {
        MfskCoder {
            v,
            low: v.low_hz(centre),
            code: MfskCode::new(&v),
        }
    }
    fn tone(&self, t: Option<u32>, out: &mut Vec<crate::sender::Element>) {
        if let Some(t) = t {
            out.push(crate::sender::Element::tone(
                self.low + f64::from(t) * self.v.spacing(),
                self.v.symlen as f64 / self.v.rate,
            ));
        }
    }
    fn data_bit(&mut self, bit: u8, out: &mut Vec<crate::sender::Element>) {
        let t = self.code.data_bit(bit);
        self.tone(t, out);
    }
    fn byte(&mut self, b: u8, out: &mut Vec<crate::sender::Element>) {
        for bit in varicode::code(b).iter() {
            self.data_bit(bit, out);
        }
    }
}

impl crate::sender::Coder for MfskCoder {
    fn begin(&mut self, out: &mut Vec<crate::sender::Element>) {
        let (code, data) = self.v.tx_preamble();
        for _ in 0..code {
            let t = self.code.code_bit(0);
            self.tone(t, out);
        }
        for _ in 0..data {
            self.data_bit(0, out);
        }
        for b in TX_START {
            self.byte(b, out);
        }
    }
    fn char(&mut self, c: char, out: &mut Vec<crate::sender::Element>) {
        let mut buf = [0u8; 4];
        for &b in c.encode_utf8(&mut buf).as_bytes() {
            self.byte(b, out);
        }
    }
    fn end(&mut self, out: &mut Vec<crate::sender::Element>) {
        self.data_bit(1, out);
        for _ in 0..self.v.tx_flush() {
            self.data_bit(0, out);
        }
        while self.code.partial() > 0 {
            self.data_bit(0, out);
        }
    }
}

/// The MFSK side of the sender.
#[derive(Clone, Debug)]
pub struct MfskModem {
    v: MfskVariant,
    centre: f64,
    demod: crate::front::Demod,
    common: crate::sender::Common,
}

impl Default for MfskModem {
    fn default() -> Self {
        MfskModem {
            v: variant(VARIANT_SETTING.default).unwrap_or(VARIANTS[3]),
            centre: CENTRE.default,
            demod: crate::front::DEMOD.default,
            common: crate::sender::Common::default(),
        }
    }
}

impl crate::sender::Modem for MfskModem {
    const NAME: &'static str = "hfkbd.mfsktx";
    fn schema() -> Arc<Schema> {
        MfskTxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match MfskTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MfskTxSetting::Variant => {
                let i = VARIANT_SETTING.read(id, v)?;
                self.v = VARIANTS.get(i).copied().ok_or(SettingError::NotAChoice {
                    id,
                    index: i as u16,
                })?;
            }
            MfskTxSetting::Centre => self.centre = CENTRE.read(id, v)?,
            MfskTxSetting::Demod => self.demod = crate::front::DEMOD.read(id, v)?,
            MfskTxSetting::Level => self.common.level = crate::sender::TX_LEVEL.read(id, v)?,
            MfskTxSetting::TuneLimit => {
                self.common.tune_s = crate::sender::TX_TUNE_LIMIT.read(id, v)?;
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let i = VARIANTS.iter().position(|x| *x == self.v).unwrap_or(0) as u16;
        values_of([
            (MfskTxSetting::Variant, Value::Choice(i)),
            (MfskTxSetting::Centre, Value::Hz(self.centre)),
            (MfskTxSetting::Demod, Value::Choice(self.demod.index())),
            (MfskTxSetting::Level, Value::F64(self.common.level)),
            (
                MfskTxSetting::TuneLimit,
                Value::Duration(self.common.tune_s),
            ),
        ])
    }
    fn common(&self) -> crate::sender::Common {
        self.common
    }
    /// Every character, as its UTF-8 bytes.
    fn accept(&self, text: &str, out: &mut Vec<char>) -> Result<(), crate::sender::TxError> {
        out.extend(text.chars());
        Ok(())
    }
    fn coder(&self) -> Result<crate::sender::Coding, crate::sender::TxError> {
        crate::sender::lower_of(self.demod)?;
        Ok(crate::sender::Coding::Elements(Box::new(MfskCoder::new(
            self.v,
            self.centre,
        ))))
    }
    fn centre_hz(&self) -> f64 {
        self.centre
    }
    /// The tones and half a spacing each side.
    fn band(&self) -> (f64, f64) {
        let lo = self.v.low_hz(self.centre) - self.v.spacing() / 2.0;
        (lo, lo + self.v.tones() as f64 * self.v.spacing())
    }
    fn lower(&self) -> bool {
        crate::sender::lower_of(self.demod).unwrap_or(false)
    }
}

/// The MFSK sender component.
pub fn sender() -> Box<dyn sdrkit_graph::Component> {
    Box::new(crate::sender::TextSender::<MfskModem>::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tones_are_used() {
        for v in VARIANTS.iter().filter(|v| v.preamble < 1000) {
            let mut m = MfskModulator::new(*v, 8000.0, 1500.0, TEST_TEXT);
            let mut seen = vec![0u32; v.tones()];
            for _ in 0..3000 {
                seen[m.next_tone() as usize] += 1;
            }
            // With an odd number of bits per symbol every symbol holds bits of one polynomial only, so a short text can leave
            // one tone unused.
            let used = seen.iter().filter(|&&c| c > 0).count();
            assert!(used + 1 >= v.tones(), "{}: {seen:?}", v.name);
        }
    }
}
