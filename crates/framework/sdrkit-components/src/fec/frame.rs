//! `fec.bch_decode`, `fec.ldpc_decode` and `fec.turbo_decode`: decoders of whole code words cut from a soft-bit stream, sharing
//! one frame shell (framing at fixed lengths and `FrameStart` tags, outputs, tags, results).

use super::common::{snake, FrameStatus, Framer, PacketPool};
use sdrkit_algo_fec::bch::{codes, Bch, BchCode};
use sdrkit_algo_fec::ldpc::tables::{dvb_s2, dvb_s2x, dvb_t2, wsjt, DvbTable};
use sdrkit_algo_fec::ldpc::{BpDecoder, InfoAt, LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::turbo::Turbo;
use sdrkit_algo_fec::FecError;
use sdrkit_core::publish::{field, keys, Field, Published};
use sdrkit_core::{CatalogueSetting, ChoiceDecl};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{
    Applied, Component, ComponentInfo, Every, Format, Io, Kind, Port, RateFrom, TagCarry, TagKind,
    Timed,
};
use sdrkit_util_bits::crc::catalog::{CRC24_LTE_A, CRC24_LTE_B};
use sdrkit_util_bits::pack::pack_into;
use sdrkit_util_bits::{BitOrder, Pad};

/// Mean iterations per frame since the last update.
pub const ITERATIONS: Field<field::F64> = Field::new("iterations", "Iterations", "");

/// What one frame decode returned: `Ok((corrected, iterations))`, or `Err(iterations)` when the frame could not be decoded (the
/// information bits then hold the decoder's best guess).
type FrameResult = Result<(u32, u32), u32>;

/// The part every frame decoder shares: frames of `n` items in, `k` information bits out on three ports (`out`: one packet of
/// the bits packed most significant first, the last byte padded with zeros; `bits`: the bits; `status`: one [`FrameStatus`]
/// per frame).
#[derive(Debug)]
struct Shell<T> {
    n: usize,
    k: usize,
    drop_failed: bool,
    framer: Framer<T>,
    info: Vec<u8>,
    starts: Vec<usize>,
    pool: PacketPool,
    carry: TagCarry,
    ok: u64,
    bad: u64,
    corrected: u64,
    iters: u64,
    frames: u64,
    every: Every,
    published: Published,
}

impl<T: Copy> Shell<T> {
    fn new() -> Self {
        Shell {
            n: 1,
            k: 1,
            drop_failed: false,
            framer: Framer::default(),
            info: Vec::new(),
            starts: Vec::new(),
            pool: PacketPool::default(),
            carry: TagCarry::default(),
            ok: 0,
            bad: 0,
            corrected: 0,
            iters: 0,
            frames: 0,
            every: Every::default(),
            published: Published::new(&[
                &keys::FRAMES_OK,
                &keys::FRAMES_BAD,
                &keys::BITS_CORRECTED,
                &ITERATIONS,
            ]),
        }
    }

    fn ports(name: &str, input: Port) -> ComponentInfo {
        ComponentInfo::processor(
            name,
            vec![input],
            vec![
                Port::bytes("out").rate_from(RateFrom::Fixed(0.0)),
                Port::hard_bits("bits"),
                Port::typed::<FrameStatus>("status").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .manual_tags()
    }

    /// Frames of `n` items giving `k` bits, from an input of `fmt`.
    fn configure(&mut self, n: usize, k: usize, fmt: &Format) -> Vec<Format> {
        (self.n, self.k) = (n, k);
        self.framer.reset(n);
        self.info.clear();
        self.info.resize(k, 0);
        self.starts.clear();
        self.starts.reserve(64);
        (self.ok, self.bad, self.corrected, self.iters, self.frames) = (0, 0, 0, 0, 0);
        let out_rate = fmt.rate * k as f64 / n as f64;
        self.carry.reset(fmt.rate, out_rate);
        self.every = Every::new(fmt.rate, 0.25);
        vec![
            Format::new(Kind::Bytes, 0.0),
            Format::hard_bits(out_rate),
            Format::new(Kind::typed::<FrameStatus>(), 0.0),
        ]
    }

    /// Runs `items` (from input 0, which holds them) through the framer, decoding each frame with `decode`.
    fn process(
        &mut self,
        io: &mut Io,
        items: &[T],
        mut decode: impl FnMut(&[T], &mut [u8]) -> FrameResult,
    ) {
        let input = io.input(0);
        let fmt = io.input_format(0);
        let bits_first = io.output(1).first();
        let mut packets = std::mem::take(io.bytes_out(0));
        let mut bits = std::mem::take(io.hard_bits_out(1));
        let mut status = std::mem::take(io.items_out::<FrameStatus>(2));
        let Shell {
            framer,
            info,
            starts,
            pool,
            ok,
            bad,
            corrected,
            iters,
            frames,
            drop_failed,
            ..
        } = self;
        starts.clear();
        framer.feed(items, input.first(), input.tags(), |frame, at| {
            let r = decode(frame, info);
            let time = fmt.time_of(at);
            let (good, fixed, used) = match r {
                Ok((c, i)) => (true, c, i),
                Err(i) => (false, 0, i),
            };
            *frames += 1;
            *iters += used as u64;
            if good {
                *ok += 1;
                *corrected += fixed as u64;
            } else {
                *bad += 1;
            }
            status.push(Timed {
                time,
                value: FrameStatus {
                    ok: good,
                    corrected: fixed,
                    iterations: used,
                    failed_words: (!good) as u32,
                },
            });
            if good || !*drop_failed {
                let info = &info[..];
                packets.push(pool.make(info.len().div_ceil(8), time, |b| {
                    // The packet has exactly the bytes these bits fill, so this cannot fail.
                    let _ = pack_into(info, BitOrder::Msb, Pad::Zero, b);
                }));
                starts.push(bits.len());
                bits.extend_from_slice(info);
            }
        });
        for &s in self.starts.iter() {
            self.carry.queue(
                bits_first + s as u64,
                TagKind::FrameStart {
                    inverted: false,
                    errors: 0,
                },
            );
        }
        *io.bytes_out(0) = packets;
        *io.hard_bits_out(1) = bits;
        *io.items_out::<FrameStatus>(2) = status;
        self.carry.forward_if(input, io.output(1), |t| {
            !matches!(t.kind, TagKind::FrameStart { .. })
        });
        if self.every.tick(items.len()) {
            self.published.set(&keys::FRAMES_OK, self.ok as i64);
            self.published.set(&keys::FRAMES_BAD, self.bad as i64);
            self.published
                .set(&keys::BITS_CORRECTED, self.corrected as i64);
            if self.frames > 0 {
                self.published
                    .set(&ITERATIONS, self.iters as f64 / self.frames as f64);
            }
            (self.iters, self.frames) = (0, 0);
        }
    }
}

const DROP_FAILED_HELP: &str =
    "Drop frames that could not be decoded instead of passing on the decoder's best guess.";

// ------------------------------------------------------------------------------------------------------------------------ BCH

/// Every named BCH code this component decodes: the catalogue of `sdrkit_algo_fec::bch::codes` (the DVB-S2 and S2X outer codes
/// `dvb_s2_normal_1_2`, `dvb_s2x_short_11_45` ..., Cospas-Sarsat, POCSAG, P25 NID, DVB-T TPS, BeiDou ...).
pub fn bch_codes() -> &'static [BchCode] {
    codes::ALL
}

/// The keys of [`ldpc_codes`].
fn ldpc_keys() -> Vec<&'static str> {
    static KEYS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    KEYS.get_or_init(|| ldpc_codes().into_iter().map(|(n, _)| n).collect())
        .iter()
        .map(String::as_str)
        .collect()
}

const BCH_CODE: CatalogueSetting = CatalogueSetting {
    key: "code",
    label: "Code",
    help: "",
    choices: || {
        bch_codes()
            .iter()
            .map(|c| ChoiceDecl::new(c.key, c.key))
            .collect()
    },
    default: "dvb_s2_normal_1_2",
    advanced: false,
    role: None,
};
const DROP_FAILED: Setting<field::Bool> =
    Setting::new("drop_failed", "Drop failed frames", "", false).help(DROP_FAILED_HELP);

sdrkit_core::settings! {
    /// The settings of [`BchDecode`].
    pub enum BchDecodeSetting {
        /// The code.
        Code = BCH_CODE,
        /// Drop failed frames.
        DropFailed = DROP_FAILED,
    }
}

/// Decodes a binary BCH code ([`Bch`]): `n`-bit code words from `HardBits` (the graph makes the hard decisions of LLRs), the `k`
/// information bits out. Algebraic decoding corrects up to `t` errors; a word with more is passed on as received (or dropped with
/// `drop_failed`) and counted in `frames_bad`. A `FrameStart` tag starts a new code word at its bit.
#[derive(Debug)]
pub struct BchDecode {
    code: &'static BchCode,
    index: u16,
    shell: Shell<u8>,
    bch: Option<Bch>,
    word: Vec<u8>,
}

impl Default for BchDecode {
    fn default() -> Self {
        let index = BCH_CODE.default_index();
        BchDecode {
            code: &bch_codes()[usize::from(index)],
            index,
            shell: Shell::new(),
            bch: None,
            word: Vec::new(),
        }
    }
}

impl Component for BchDecode {
    fn info(&self) -> ComponentInfo {
        Shell::<u8>::ports("fec.bch_decode", Port::hard_bits("in").lossy())
            .with_settings(BchDecodeSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let bch = Bch::from_code(self.code).map_err(crate::stage)?;
        let (n, k) = (bch.n(), bch.k());
        self.bch = Some(bch);
        self.word.clear();
        self.word.resize(n, 0);
        Ok(self.shell.configure(n, k, &inputs[0]))
    }

    fn process(&mut self, io: &mut Io) {
        let Some(bch) = self.bch.as_mut() else {
            return;
        };
        let items = io.hard_bits(0);
        let word = &mut self.word;
        self.shell.process(io, items, |frame, info| {
            word.copy_from_slice(frame);
            let r = bch.decode(word);
            info.copy_from_slice(&word[..info.len()]);
            match r {
                Ok(d) => Ok((d.corrected, 1)),
                Err(_) => Err(1),
            }
        });
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BchDecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BchDecodeSetting::Code => {
                let i = BCH_CODE.read(id, v)?;
                self.code = bch_codes().get(i).ok_or(SettingError::NotAChoice {
                    id,
                    index: i as u16,
                })?;
                self.index = i as u16;
            }
            BchDecodeSetting::DropFailed => self.shell.drop_failed = DROP_FAILED.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (BchDecodeSetting::Code, Value::Choice(self.index)),
            (
                BchDecodeSetting::DropFailed,
                Value::Bool(self.shell.drop_failed),
            ),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.shell.published.results())
    }
}

// ----------------------------------------------------------------------------------------------------------------------- LDPC

/// Which LDPC code a name stands for.
#[derive(Clone, Copy, Debug)]
pub(crate) enum LdpcChoice {
    /// A DVB-S2, S2X or T2 code by its address table.
    Dvb(&'static DvbTable),
    /// The FT8 / FT4 (174, 91) code.
    Ft8,
    /// The JS8 (174, 87) code.
    Js8,
    /// A 5G NR base graph (1 or 2), lifted by the `lifting` setting.
    Nr(u8),
}

/// Every named LDPC code: the DVB-S2, S2X and T2 tables (their names in snake case: `dvb_s2_short_1_2`), `ft8`, `js8`, `nr_bg1`
/// and `nr_bg2`.
pub(crate) fn ldpc_codes() -> Vec<(String, LdpcChoice)> {
    let mut v: Vec<(String, LdpcChoice)> = dvb_s2::ALL
        .iter()
        .chain(dvb_s2x::ALL)
        .chain(dvb_t2::ALL)
        .map(|t| (snake(t.name), LdpcChoice::Dvb(t)))
        .collect();
    v.push(("ft8".into(), LdpcChoice::Ft8));
    v.push(("js8".into(), LdpcChoice::Js8));
    v.push(("nr_bg1".into(), LdpcChoice::Nr(1)));
    v.push(("nr_bg2".into(), LdpcChoice::Nr(2)));
    v
}

sdrkit_core::choice_enum! {
    /// The algorithm of [`LdpcDecode`].
    pub enum LdpcAlgo {
        /// Layered min-sum (i8).
        MinSum = ("min_sum", "Layered min-sum (i8)"),
        /// Sum-product (f32).
        SumProduct = ("sum_product", "Sum-product (f32)"),
    }
}

#[derive(Debug)]
enum LdpcEngine {
    MinSum(Box<LdpcDecoder>),
    SumProduct {
        dec: BpDecoder,
        soft: Vec<f32>,
        scale: f32,
    },
}

const LDPC_CODE: CatalogueSetting = CatalogueSetting {
    key: "code",
    label: "Code",
    help: "",
    choices: || {
        ldpc_keys()
            .into_iter()
            .map(|k| ChoiceDecl::new(k, k))
            .collect()
    },
    default: "dvb_s2_short_1_2",
    advanced: false,
    role: None,
};
const LIFTING: Setting<field::I64> = Setting::new("lifting", "Lifting size", "", 384)
    .range(2.0, 384.0, 1.0)
    .help("For nr_bg1 and nr_bg2: the lifting size z (TS 38.212 table 5.3.2-1).");
const MAX_ITER: Setting<field::I64> =
    Setting::new("max_iter", "Most iterations", "", 50).range(1.0, 200.0, 1.0);
const ALGO: Setting<field::Choice<LdpcAlgo>> =
    Setting::choice("algo", "Algorithm", LdpcAlgo::MinSum);
const MIN_SUM_SCALE: Setting<field::F64> =
    Setting::new("min_sum_scale", "Min-sum scale", "", 0.8125)
        .range(0.25, 1.0, 0.0625)
        .help("Check-node scale of the min-sum decoder, in sixteenths.");

sdrkit_core::settings! {
    /// The settings of [`LdpcDecode`].
    pub enum LdpcDecodeSetting {
        /// Code.
        Code = LDPC_CODE,
        /// Lifting size.
        Lifting = LIFTING,
        /// Most iterations.
        MaxIter = MAX_ITER,
        /// Algorithm.
        Algo = ALGO,
        /// Min-sum scale.
        MinSumScale = MIN_SUM_SCALE,
        /// Drop failed frames.
        DropFailed = DROP_FAILED,
    }
}

/// Decodes an LDPC code: code words of `n` LLRs (`Llr`, positive means 0) in, the `k` information bits out.
///
/// `algo` `min_sum` runs the layered min-sum decoder on the `i8` values directly (`min_sum_scale` is the check-node scale);
/// `sum_product` runs belief propagation in `f32`, which needs the input's `llr_scale` to turn the values back into natural-log
/// LLRs. Decoding stops when every check is satisfied or after `max_iter` iterations; a frame still failing then is passed on
/// with the decoder's last decisions (or dropped with `drop_failed`) and counted in `frames_bad`. For `nr_bg1` and `nr_bg2` a
/// frame is the `n − 2 z` values sent (the first `2 z` bits are never sent). A `FrameStart` tag starts a new code word.
#[derive(Debug)]
pub struct LdpcDecode {
    index: u16,
    choice: LdpcChoice,
    lifting: usize,
    max_iter: u32,
    algo: LdpcAlgo,
    scale_q4: u8,
    shell: Shell<i8>,
    engine: Option<LdpcEngine>,
    skip: usize,
    range: std::ops::Range<usize>,
    full: Vec<i8>,
    cw: Vec<u8>,
}

impl Default for LdpcDecode {
    fn default() -> Self {
        let index = LDPC_CODE.default_index();
        LdpcDecode {
            index,
            choice: ldpc_codes().swap_remove(usize::from(index)).1,
            lifting: LIFTING.default as _,
            max_iter: MAX_ITER.default as _,
            algo: LdpcAlgo::MinSum,
            scale_q4: MinSum::default().scale_q4,
            shell: Shell::new(),
            engine: None,
            skip: 0,
            range: 0..0,
            full: Vec::new(),
            cw: Vec::new(),
        }
    }
}

impl LdpcDecode {
    fn code(&self) -> Result<LdpcCode, FecError> {
        match self.choice {
            LdpcChoice::Dvb(t) => LdpcCode::from_dvb(t),
            LdpcChoice::Ft8 => {
                LdpcCode::from_checks("ft8", 174, 91, &wsjt::FT8_CHECKS, InfoAt::Start)
            }
            LdpcChoice::Js8 => {
                LdpcCode::from_checks("js8", 174, 87, &wsjt::JS8_CHECKS, InfoAt::End)
            }
            LdpcChoice::Nr(bg) => LdpcCode::nr(bg, self.lifting),
        }
    }
}

impl Component for LdpcDecode {
    fn info(&self) -> ComponentInfo {
        Shell::<i8>::ports("fec.ldpc_decode", Port::llr("in"))
            .with_settings(LdpcDecodeSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let code = self.code().map_err(crate::stage)?;
        let (n, k) = (code.n(), code.k());
        self.skip = match self.choice {
            LdpcChoice::Nr(_) => 2 * code.z(),
            LdpcChoice::Dvb(_) | LdpcChoice::Ft8 | LdpcChoice::Js8 => 0,
        };
        self.range = code.info_range();
        self.full.clear();
        self.full.resize(n, 0);
        self.cw.clear();
        self.cw.resize(n, 0);
        self.engine = Some(match self.algo {
            LdpcAlgo::SumProduct => {
                let scale = inputs[0]
                    .llr_scale
                    .filter(|s| s.is_finite() && *s > 0.0)
                    .ok_or(ConfigError::Setting(SettingError::NotPermitted {
                        id: LdpcDecodeSetting::Algo.id(),
                    }))?;
                LdpcEngine::SumProduct {
                    dec: BpDecoder::new(&code),
                    soft: vec![0.0; n],
                    scale,
                }
            }
            LdpcAlgo::MinSum => LdpcEngine::MinSum(Box::new(LdpcDecoder::new(
                &code,
                MinSum {
                    scale_q4: self.scale_q4,
                    offset: 0,
                },
            ))),
        });
        Ok(self.shell.configure(n - self.skip, k, &inputs[0]))
    }

    fn process(&mut self, io: &mut Io) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        let (full, cw, skip, range, max_iter) = (
            &mut self.full,
            &mut self.cw,
            self.skip,
            self.range.clone(),
            self.max_iter,
        );
        let items = io.llrs(0);
        self.shell.process(io, items, |frame, info| {
            full[skip..].copy_from_slice(frame);
            let r = match engine {
                LdpcEngine::MinSum(d) => d.decode(full, max_iter, cw),
                LdpcEngine::SumProduct { dec, soft, scale } => {
                    for (s, &l) in soft.iter_mut().zip(full.iter()) {
                        *s = l as f32 / *scale;
                    }
                    dec.decode(soft, max_iter, cw)
                }
            };
            info.copy_from_slice(&cw[range.clone()]);
            match r {
                Ok(d) => Ok((d.corrected, d.iterations)),
                Err(_) => Err(max_iter),
            }
        });
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match LdpcDecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            LdpcDecodeSetting::Code => {
                let i = LDPC_CODE.read(id, v)?;
                let (_, choice) =
                    ldpc_codes()
                        .into_iter()
                        .nth(i)
                        .ok_or(SettingError::NotAChoice {
                            id,
                            index: i as u16,
                        })?;
                (self.index, self.choice) = (i as u16, choice);
            }
            LdpcDecodeSetting::Lifting => self.lifting = LIFTING.read(id, v)? as usize,
            LdpcDecodeSetting::MaxIter => self.max_iter = MAX_ITER.read(id, v)? as u32,
            LdpcDecodeSetting::Algo => self.algo = ALGO.read(id, v)?,
            LdpcDecodeSetting::MinSumScale => {
                let q = MIN_SUM_SCALE.read(id, v)? * 16.0;
                if q.fract() != 0.0 {
                    return Err(SettingError::NotPermitted { id });
                }
                self.scale_q4 = q as u8;
            }
            LdpcDecodeSetting::DropFailed => self.shell.drop_failed = DROP_FAILED.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (LdpcDecodeSetting::Code, Value::Choice(self.index)),
            (LdpcDecodeSetting::Lifting, Value::I64(self.lifting as i64)),
            (LdpcDecodeSetting::MaxIter, Value::I64(self.max_iter as i64)),
            (LdpcDecodeSetting::Algo, Value::Choice(self.algo.index())),
            (
                LdpcDecodeSetting::MinSumScale,
                Value::F64(self.scale_q4 as f64 / 16.0),
            ),
            (
                LdpcDecodeSetting::DropFailed,
                Value::Bool(self.shell.drop_failed),
            ),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.shell.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------- turbo

sdrkit_core::choice_enum! {
    /// When [`TurboDecode`] stops iterating.
    pub enum TurboStop {
        /// CRC-24/LTE-A passes (transport block).
        Crc24a = ("crc24a", "CRC-24/LTE-A passes (transport block)"),
        /// CRC-24/LTE-B passes (code block).
        Crc24b = ("crc24b", "CRC-24/LTE-B passes (code block)"),
        /// Decisions unchanged by an iteration.
        Stable = ("stable", "Decisions unchanged by an iteration"),
        /// Always max_iter iterations.
        None = ("none", "Always max_iter iterations"),
    }
}

const K: Setting<field::I64> = Setting::new("k", "Block size", "", 6144)
    .range(40.0, 6144.0, 8.0)
    .help("Bits per code block, one of the sizes of TS 36.212 table 5.1.3-3.");
const TURBO_MAX_ITER: Setting<field::I64> =
    Setting::new("max_iter", "Most iterations", "", 8).range(1.0, 32.0, 1.0);
const STOP: Setting<field::Choice<TurboStop>> =
    Setting::choice("stop", "Stop when", TurboStop::Crc24b);

sdrkit_core::settings! {
    /// The settings of [`TurboDecode`].
    pub enum TurboDecodeSetting {
        /// Block size.
        K = K,
        /// Most iterations.
        MaxIter = TURBO_MAX_ITER,
        /// Stop when.
        Stop = STOP,
        /// Drop failed frames.
        DropFailed = DROP_FAILED,
    }
}

/// Decodes the LTE turbo code ([`Turbo`]) of block size `k` (one of the 188 sizes of TS 36.212 table 5.1.3-3): frames of
/// `3 (k + 4)` LLRs (the streams `d0 | d1 | d2` after rate dematching, unsent bits 0) in, the `k` bits (CRC included) out.
///
/// `stop` says when the iterations end: when the block's CRC-24A or CRC-24B passes (a frame whose CRC never passes is failed),
/// when an iteration leaves the decisions unchanged, or never before `max_iter` (`none`: every frame counts as decoded).
#[derive(Debug)]
pub struct TurboDecode {
    k: usize,
    max_iter: u32,
    stop: TurboStop,
    shell: Shell<i8>,
    turbo: Option<Turbo>,
    prev: Vec<u8>,
}

impl Default for TurboDecode {
    fn default() -> Self {
        TurboDecode {
            k: K.default as _,
            max_iter: TURBO_MAX_ITER.default as _,
            stop: TurboStop::Crc24b,
            shell: Shell::new(),
            turbo: None,
            prev: Vec::new(),
        }
    }
}

impl Component for TurboDecode {
    fn info(&self) -> ComponentInfo {
        Shell::<i8>::ports("fec.turbo_decode", Port::llr("in"))
            .with_settings(TurboDecodeSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let t = Turbo::new(self.k).map_err(crate::stage)?;
        self.turbo = Some(t);
        self.prev.clear();
        self.prev.resize(self.k, 0);
        Ok(self.shell.configure(3 * (self.k + 4), self.k, &inputs[0]))
    }

    fn process(&mut self, io: &mut Io) {
        let Some(turbo) = self.turbo.as_mut() else {
            return;
        };
        let (prev, stop, max_iter) = (&mut self.prev, self.stop, self.max_iter);
        let items = io.llrs(0);
        self.shell.process(io, items, |frame, info| {
            let mut have_prev = false;
            let r = turbo.decode(
                frame,
                max_iter,
                |b| match stop {
                    TurboStop::Crc24a => CRC24_LTE_A.verify_bits(b),
                    TurboStop::Crc24b => CRC24_LTE_B.verify_bits(b),
                    TurboStop::Stable => {
                        let same = have_prev && prev[..] == *b;
                        prev.copy_from_slice(b);
                        have_prev = true;
                        same
                    }
                    TurboStop::None => false,
                },
                info,
            );
            match r {
                Ok(d) => Ok((d.corrected, d.iterations)),
                Err(_) if stop == TurboStop::None => Ok((0, max_iter)),
                Err(_) => Err(max_iter),
            }
        });
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TurboDecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TurboDecodeSetting::K => {
                let k = K.read(id, v)? as usize;
                if !sdrkit_algo_fec::turbo::sizes().any(|s| s == k) {
                    return Err(SettingError::NotPermitted { id });
                }
                self.k = k;
            }
            TurboDecodeSetting::MaxIter => self.max_iter = TURBO_MAX_ITER.read(id, v)? as u32,
            TurboDecodeSetting::Stop => self.stop = STOP.read(id, v)?,
            TurboDecodeSetting::DropFailed => self.shell.drop_failed = DROP_FAILED.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (TurboDecodeSetting::K, Value::I64(self.k as i64)),
            (
                TurboDecodeSetting::MaxIter,
                Value::I64(self.max_iter as i64),
            ),
            (TurboDecodeSetting::Stop, Value::Choice(self.stop.index())),
            (
                TurboDecodeSetting::DropFailed,
                Value::Bool(self.shell.drop_failed),
            ),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.shell.published.results())
    }
}
