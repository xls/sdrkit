//! `fec.viterbi`: the Viterbi decoder of `sdrkit-algo-fec` on a soft-bit stream, sliding window or whole frames, with bit-error
//! estimation by re-encoding.

use super::common::Framer;
use sdrkit_algo_fec::conv::{codes, reencode_errors, ConvCode, Ends, Viterbi};
use sdrkit_core::publish::{field, keys, Field, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Every, Format, Io, Port, TagCarry, TagKind};

/// The best path's metric growth per trellis step: about `n × mean |LLR|` on a clean signal, much less on noise.
pub const MEAN_GAIN: Field<field::F64> = Field::new("mean_gain", "Path metric gain", "");

sdrkit_core::choice_enum! {
    /// Every named convolutional code of `sdrkit_algo_fec::conv::codes` the Viterbi decoder runs (`K` up to 16), by the constant's name in
    /// snake case.
    pub enum ConvCodeName {
        /// `ccsds_k7`.
        CcsdsK7 = ("ccsds_k7", "ccsds_k7"),
        /// `k7_171_133`.
        K7171133 = ("k7_171_133", "k7_171_133"),
        /// `dvb_k7`.
        DvbK7 = ("dvb_k7", "dvb_k7"),
        /// `isdb_t_k7`.
        IsdbTK7 = ("isdb_t_k7", "isdb_t_k7"),
        /// `isdb_s_k7`.
        IsdbSK7 = ("isdb_s_k7", "isdb_s_k7"),
        /// `galileo_k7`.
        GalileoK7 = ("galileo_k7", "galileo_k7"),
        /// `wifi_k7`.
        WifiK7 = ("wifi_k7", "wifi_k7"),
        /// `inmarsat_k7`.
        InmarsatK7 = ("inmarsat_k7", "inmarsat_k7"),
        /// `hfdl_k7`.
        HfdlK7 = ("hfdl_k7", "hfdl_k7"),
        /// `dab_k7_r4`.
        DabK7R4 = ("dab_k7_r4", "dab_k7_r4"),
        /// `drm_k7_r6`.
        DrmK7R6 = ("drm_k7_r6", "drm_k7_r6"),
        /// `lte_k7_r3`.
        LteK7R3 = ("lte_k7_r3", "lte_k7_r3"),
        /// `hd_fm_k7_r3`.
        HdFmK7R3 = ("hd_fm_k7_r3", "hd_fm_k7_r3"),
        /// `hd_fm_k7_r4`.
        HdFmK7R4 = ("hd_fm_k7_r4", "hd_fm_k7_r4"),
        /// `hd_am_e1`.
        HdAmE1 = ("hd_am_e1", "hd_am_e1"),
        /// `hd_am_e2`.
        HdAmE2 = ("hd_am_e2", "hd_am_e2"),
        /// `gsm_k5`.
        GsmK5 = ("gsm_k5", "gsm_k5"),
        /// `nxdn_k5`.
        NxdnK5 = ("nxdn_k5", "nxdn_k5"),
        /// `m17_k5`.
        M17K5 = ("m17_k5", "m17_k5"),
        /// `tetra_k5_r4`.
        TetraK5R4 = ("tetra_k5_r4", "tetra_k5_r4"),
        /// `qpsk31_k5`.
        Qpsk31K5 = ("qpsk31_k5", "qpsk31_k5"),
        /// `dstar_k3`.
        DstarK3 = ("dstar_k3", "dstar_k3"),
        /// `thor_k15`.
        ThorK15 = ("thor_k15", "thor_k15"),
    }
}

impl ConvCodeName {
    /// The code.
    pub fn code(self) -> ConvCode {
        match self {
            ConvCodeName::CcsdsK7 => codes::CCSDS_K7,
            ConvCodeName::K7171133 => codes::K7_171_133,
            ConvCodeName::DvbK7 => codes::DVB_K7,
            ConvCodeName::IsdbTK7 => codes::ISDB_T_K7,
            ConvCodeName::IsdbSK7 => codes::ISDB_S_K7,
            ConvCodeName::GalileoK7 => codes::GALILEO_K7,
            ConvCodeName::WifiK7 => codes::WIFI_K7,
            ConvCodeName::InmarsatK7 => codes::INMARSAT_K7,
            ConvCodeName::HfdlK7 => codes::HFDL_K7,
            ConvCodeName::DabK7R4 => codes::DAB_K7_R4,
            ConvCodeName::DrmK7R6 => codes::DRM_K7_R6,
            ConvCodeName::LteK7R3 => codes::LTE_K7_R3,
            ConvCodeName::HdFmK7R3 => codes::HD_FM_K7_R3,
            ConvCodeName::HdFmK7R4 => codes::HD_FM_K7_R4,
            ConvCodeName::HdAmE1 => codes::HD_AM_E1,
            ConvCodeName::HdAmE2 => codes::HD_AM_E2,
            ConvCodeName::GsmK5 => codes::GSM_K5,
            ConvCodeName::NxdnK5 => codes::NXDN_K5,
            ConvCodeName::M17K5 => codes::M17_K5,
            ConvCodeName::TetraK5R4 => codes::TETRA_K5_R4,
            ConvCodeName::Qpsk31K5 => codes::QPSK31_K5,
            ConvCodeName::DstarK3 => codes::DSTAR_K3,
            ConvCodeName::ThorK15 => codes::THOR_K15,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The trellis state that [`ViterbiDecode`] assumes at the start and the end of the stream or of each frame.
    pub enum TrellisEnds {
        /// Terminated (start and end in state 0).
        Zero = ("zero", "Terminated (start and end in state 0)"),
        /// Known start (state 0), free end.
        Start = ("start", "Known start (state 0), free end"),
        /// Nothing known.
        Open = ("open", "Nothing known"),
        /// Tail-biting.
        TailBiting = ("tail_biting", "Tail-biting"),
    }
}

const CODE: Setting<field::Choice<ConvCodeName>> =
    Setting::choice("code", "Code", ConvCodeName::DvbK7);
const ENDS: Setting<field::Choice<TrellisEnds>> =
    Setting::choice("ends", "Ends", TrellisEnds::Start)
        .help("What is known about the start and the end of the stream or of each frame.");
const TRACEBACK: Setting<field::I64> = Setting::new("traceback", "Traceback", "", 0).range(0.0, 65_536.0, 1.0).help("Steps decoded after a bit before it is released; 0 for 16 (K - 1). Punctured high rates want more.");
const FRAME: Setting<field::I64> = Setting::new("frame", "Frame length", "", 0).range(0.0, 1_048_576.0, 1.0).help("Trellis steps (tail included) per frame decoded as a whole; 0 decodes a continuous stream.");

sdrkit_core::settings! {
    /// The settings of [`ViterbiDecode`].
    pub enum ViterbiDecodeSetting {
        /// Code.
        Code = CODE,
        /// Ends.
        Ends = ENDS,
        /// Traceback.
        Traceback = TRACEBACK,
        /// Frame length.
        Frame = FRAME,
    }
}

/// Decodes a convolutional code: `Llr` in (`n` values per input bit, punctured places 0, after `fec.depuncture`), `HardBits`
/// out.
///
/// - **Streaming** (`frame` 0): a sliding window ([`Viterbi::push`]) releases bits once `traceback` further steps have been
///   decoded after them; any cut of the stream produces the same bits. `ends` `zero` or `start` start the decoder in state 0;
///   `open` with every state equally likely. An `EndOfStream` tag releases every bit still held (dropping the tail with
///   `zero`).
/// - **Frames** (`frame` above 0): every `frame` trellis steps (`frame × n` values) are decoded as one whole frame with the
///   chosen `ends` ([`Viterbi::decode`]); a `FrameStart` tag starts a new frame at its value (a partial frame before it is
///   dropped). Each frame's first output bit carries a `FrameStart` tag. With `zero` the `K − 1` tail bits are not output.
///
/// Results: `bits_corrected` (received hard decisions the decoder changed, counted by re-encoding the output), `ber` (the same as a
/// fraction of the non-erased values, over the last quarter second) and `mean_gain`.
#[derive(Debug)]
pub struct ViterbiDecode {
    name: ConvCodeName,
    code: ConvCode,
    ends: TrellisEnds,
    traceback: usize,
    frame: usize,
    dec: Option<Viterbi>,
    // Streaming: the values whose bits are not released yet, and the re-encoder's state.
    hist: Vec<i8>,
    state: u32,
    skip: usize,
    piece: usize,
    // Frames.
    framer: Framer<i8>,
    bits: Vec<u8>,
    starts: Vec<usize>,
    carry: TagCarry,
    corrected: u64,
    errs: u64,
    counted: u64,
    every: Every,
    published: Published,
}

impl Default for ViterbiDecode {
    fn default() -> Self {
        ViterbiDecode {
            name: CODE.default,
            code: CODE.default.code(),
            ends: ENDS.default,
            traceback: TRACEBACK.default as _,
            frame: FRAME.default as _,
            dec: None,
            hist: Vec::new(),
            state: 0,
            skip: 0,
            piece: 1,
            framer: Framer::default(),
            bits: Vec::new(),
            starts: Vec::new(),
            carry: TagCarry::default(),
            corrected: 0,
            errs: 0,
            counted: 0,
            every: Every::default(),
            published: Published::new(&[&keys::BITS_CORRECTED, &keys::BER, &MEAN_GAIN]),
        }
    }
}

fn ends_of(ends: TrellisEnds) -> Ends {
    match ends {
        TrellisEnds::Zero => Ends::Zero,
        TrellisEnds::Open => Ends::Open,
        TrellisEnds::TailBiting => Ends::TailBiting,
        TrellisEnds::Start => Ends::Start(0),
    }
}

impl ViterbiDecode {
    /// Counts the re-encoding disagreements of `bits`, just released, against the oldest values held, and drops those values.
    fn count_released(&mut self, bits: &[u8]) {
        let n = self.code.n();
        let k = self.code.k();
        let used = (bits.len() * n).min(self.hist.len());
        let skip = self.skip.min(bits.len());
        // The first K − 1 bits of an open start only set the re-encoder's state.
        let (head, tail) = bits.split_at(skip);
        let state0 = advance(self.state, head, k);
        if !tail.is_empty() {
            let seg = &self.hist[skip * n..used];
            if let Ok(e) = reencode_errors(&self.code, state0, tail, seg) {
                self.errs += e as u64;
                self.corrected += e as u64;
                self.counted += seg.iter().filter(|&&v| v != 0).count() as u64;
            }
        }
        self.state = advance(state0, tail, k);
        self.skip -= skip;
        self.hist.drain(..used);
    }
}

/// The encoder state after `bits` from `state` (the newest bit in bit `K − 2`).
fn advance(mut state: u32, bits: &[u8], k: u32) -> u32 {
    for &b in bits {
        state = (state >> 1) | ((b as u32 & 1) << (k - 2));
    }
    state
}

impl Component for ViterbiDecode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fec.viterbi",
            vec![Port::llr("in")],
            vec![Port::hard_bits("out")],
        )
        .manual_tags()
        .with_settings(ViterbiDecodeSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let n = self.code.n();
        let k = self.code.k() as usize;
        let ends = ends_of(self.ends);
        let streaming = self.frame == 0;
        if streaming && ends == Ends::TailBiting {
            return Err(SettingError::Required {
                id: ViterbiDecodeSetting::Frame.id(),
            }
            .into());
        }
        if !streaming && self.frame < k {
            return Err(SettingError::Conflict {
                id: ViterbiDecodeSetting::Frame.id(),
                with: ViterbiDecodeSetting::Code.id(),
            }
            .into());
        }
        let mut dec = Viterbi::new(self.code, self.frame).map_err(crate::stage)?;
        if self.traceback > 0 {
            dec.set_traceback(self.traceback, self.traceback)
                .map_err(crate::stage)?;
        }
        let (depth, chunk) = dec.window();
        match ends {
            Ends::Zero | Ends::Start(_) => dec.reset_to(0),
            _ => dec.reset(),
        }
        self.state = 0;
        self.skip = if matches!(ends, Ends::Open) { k - 1 } else { 0 };
        self.piece = chunk * n;
        self.hist.clear();
        self.hist.reserve((depth + 2 * chunk + 2) * n);
        self.framer.reset(self.frame * n);
        self.bits.clear();
        self.bits.resize(self.frame, 0);
        self.starts.clear();
        self.starts.reserve(64);
        self.dec = Some(dec);
        (self.corrected, self.errs, self.counted) = (0, 0, 0);
        let in_rate = inputs[0].rate;
        let out_rate = if streaming {
            in_rate / n as f64
        } else {
            let info = if ends == Ends::Zero {
                self.frame - (k - 1)
            } else {
                self.frame
            };
            in_rate / n as f64 * info as f64 / self.frame as f64
        };
        self.carry.reset(in_rate, out_rate);
        self.every = Every::new(in_rate, 0.25);
        Ok(vec![Format::hard_bits(out_rate)])
    }

    fn process(&mut self, io: &mut Io) {
        let Some(mut dec) = self.dec.take() else {
            return;
        };
        let input = io.input(0);
        let x = io.llrs(0);
        let mut out = std::mem::take(io.hard_bits_out(0));
        let out_first = io.output(0).first();
        let ends = ends_of(self.ends);
        if self.frame == 0 {
            let eos = input.tags().iter().any(|t| t.kind == TagKind::EndOfStream);
            for piece in x.chunks(self.piece.max(1)) {
                self.hist.extend_from_slice(piece);
                let before = out.len();
                dec.push(piece, &mut out);
                let released = std::mem::take(&mut out);
                self.count_released(&released[before..]);
                out = released;
            }
            if eos {
                let before = out.len();
                if ends == Ends::Zero {
                    dec.flush_terminated(&mut out);
                } else {
                    dec.flush(&mut out);
                }
                let released = std::mem::take(&mut out);
                self.count_released(&released[before..]);
                out = released;
                // A new stream may follow: start as at configure.
                let k = self.code.k() as usize;
                if ends == Ends::Open {
                    dec.reset();
                    self.skip = k - 1;
                } else {
                    dec.reset_to(0);
                    self.skip = 0;
                }
                self.state = 0;
                self.hist.clear();
            }
            self.carry.forward(input, io.output(0));
        } else {
            let Self {
                framer,
                bits,
                starts,
                corrected,
                errs,
                counted,
                ..
            } = self;
            starts.clear();
            framer.feed(x, input.first(), input.tags(), |frame, _| {
                if let Ok(r) = dec.decode(frame, ends, bits) {
                    let info = if ends == Ends::Zero {
                        bits.len() + 1 - dec.code().k() as usize
                    } else {
                        bits.len()
                    };
                    starts.push(out.len());
                    out.extend_from_slice(&bits[..info]);
                    *corrected += r.corrected as u64;
                    *errs += r.corrected as u64;
                    *counted += frame.iter().filter(|&&v| v != 0).count() as u64;
                }
            });
            for &s in starts.iter() {
                self.carry.queue(
                    out_first + s as u64,
                    TagKind::FrameStart {
                        inverted: false,
                        errors: 0,
                    },
                );
            }
            *io.hard_bits_out(0) = out;
            self.carry.forward_if(input, io.output(0), |t| {
                !matches!(t.kind, TagKind::FrameStart { .. })
            });
            out = std::mem::take(io.hard_bits_out(0));
        }
        *io.hard_bits_out(0) = out;
        if self.every.tick(x.len()) {
            self.published
                .set(&keys::BITS_CORRECTED, self.corrected as i64);
            // With nothing decoded since the last update (a frame not complete yet) the last values stay.
            if self.counted > 0 {
                self.published
                    .set(&keys::BER, self.errs as f64 / self.counted as f64);
                self.published.set(&MEAN_GAIN, dec.mean_gain() as f64);
                (self.errs, self.counted) = (0, 0);
            }
        }
        self.dec = Some(dec);
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ViterbiDecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ViterbiDecodeSetting::Code => {
                self.name = CODE.read(id, v)?;
                self.code = self.name.code();
            }
            ViterbiDecodeSetting::Ends => self.ends = ENDS.read(id, v)?,
            ViterbiDecodeSetting::Traceback => self.traceback = TRACEBACK.read(id, v)? as usize,
            ViterbiDecodeSetting::Frame => self.frame = FRAME.read(id, v)? as usize,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (ViterbiDecodeSetting::Code, Value::Choice(self.name.index())),
            (ViterbiDecodeSetting::Ends, Value::Choice(self.ends.index())),
            (
                ViterbiDecodeSetting::Traceback,
                Value::I64(self.traceback as i64),
            ),
            (ViterbiDecodeSetting::Frame, Value::I64(self.frame as i64)),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}
