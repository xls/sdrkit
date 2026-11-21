//! Helpers for the DAB tests: runs of the receiver, the test signal through the shared impairments of `sdrkit-algo-sim`, and audio
//! checks.
#![allow(dead_code)]

use sdrkit_algo_sim::fading::{Doppler, Tap, Tdl};
use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_core::interface::Column;
use sdrkit_core::publish::TableDef;
use sdrkit_core::record::Kind;
use sdrkit_core::{
    owner, Complex32, Elementary, Generator, InputInfo, LockState, Output, Owner, Receiver, Record,
    SettingSet, StackConfig, StreamKind, Value, Values,
};
use sdrkit_graph::StackGraph;
use sdrkit_stack_dab::generator::{DabGenerator, GenParams};
use sdrkit_stack_dab::typed::*;
use sdrkit_stack_dab::DabSetting;

/// The mean power of the test signal as the tests use it: -14 dBFS, so 8-bit samples hold its peaks.
pub const POWER: f64 = 0.04;

/// What the air adds to the test signal: noise as C/N over 2.048 MHz (`None`: none), a carrier offset and a sample clock error.
#[derive(Clone, Copy, Debug)]
pub struct Air {
    /// C/N in dB over 2.048 MHz.
    pub cn_db: Option<f64>,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Sample clock error, ppm.
    pub clock_ppm: f64,
    /// The noise seed.
    pub seed: u64,
}

impl Default for Air {
    fn default() -> Self {
        Air {
            cn_db: None,
            offset_hz: 0.0,
            clock_ppm: 0.0,
            seed: 1,
        }
    }
}

impl Air {
    /// Noise at `cn_db` and nothing else.
    pub fn cn(cn_db: f64) -> Air {
        Air {
            cn_db: Some(cn_db),
            ..Air::default()
        }
    }
    /// The shared impairment chain for this air.
    pub fn chain(&self) -> sdrkit_algo_sim::chain::ChainConfig {
        sdrkit_algo_sim::chain::ChainConfig {
            seed: self.seed,
            level_dbfs: 10.0 * POWER.log10(),
            offset_hz: self.offset_hz,
            clock_ppm: self.clock_ppm,
            noise: self
                .cn_db
                .map_or(NoiseLevel::Off, |snr_db| NoiseLevel::SnrDb {
                    snr_db,
                    bandwidth_hz: 2.048e6,
                }),
            ..Default::default()
        }
    }
}

/// `seconds` of a generator's signal through the air, at the generator's rate.
pub fn through_air(g: DabGenerator, air: Air, seconds: f64) -> Vec<Complex32> {
    let rate = g.rate_hz();
    let sig = sdrkit_algo_sim::chain::SignalInfo::new(rate);
    let mut g = sdrkit_algo_sim::chain::Impaired::new(g, &air.chain(), sig).expect("a valid chain");
    let mut v = vec![Complex32::default(); (seconds * rate) as usize];
    g.generate(&mut v);
    v
}

/// White noise added to `x` (signal power `power`, C/N `cn_db` in `bandwidth_hz`, samples at `rate_hz`).
pub fn awgn(
    x: &mut [Complex32],
    power: f64,
    cn_db: f64,
    bandwidth_hz: f64,
    rate_hz: f64,
    seed: u64,
) {
    let level = NoiseLevel::SnrDb {
        snr_db: cn_db,
        bandwidth_hz,
    };
    Awgn::complex(level, power, rate_hz, seed)
        .expect("a valid noise level")
        .process(x);
}

/// `x` moved up by `hz` at `rate_hz`.
pub fn offset(x: &mut [Complex32], rate_hz: f64, hz: f64) {
    sdrkit_algo_sim::offset::FrequencyOffset::new(rate_hz, hz, 0.0)
        .expect("a valid offset")
        .process(x);
}

/// `x` as a receiver samples it when its clock runs `ppm` fast.
pub fn clock(x: &[Complex32], ppm: f64) -> Vec<Complex32> {
    let mut y = Vec::with_capacity(x.len() + x.len() / 1000);
    sdrkit_algo_sim::clock::ClockError::new(ppm)
        .expect("a valid clock error")
        .process(x, &mut y);
    y
}

/// `x` plus one echo `delay` samples late at `level_db`, in phase with it.
pub fn echo(x: &mut [Complex32], delay: usize, level_db: f64) {
    let g = 10f32.powf(level_db as f32 / 20.0);
    let mut late = vec![Complex32::default(); x.len()];
    for (l, v) in late[delay.min(x.len())..].iter_mut().zip(x.iter()) {
        *l = *v * g;
    }
    sdrkit_algo_sim::front::add_into(x, &late);
}

/// An ADC of `bits` bits clipping at `full_scale`.
pub fn quantise(x: &mut [Complex32], bits: u32, full_scale: f32) {
    let q = sdrkit_algo_sim::front::Quantiser::new(bits).expect("2 to 24 bits");
    sdrkit_algo_sim::front::scale(x, 1.0 / full_scale);
    q.process(x);
    sdrkit_algo_sim::front::scale(x, full_scale);
}

/// What a run produced.
pub struct Run {
    pub rx: StackGraph,
    pub media: Vec<Elementary>,
    pub audio: Vec<f32>,
    pub events: Vec<sdrkit_core::Event>,
}

impl Run {
    /// The value of result field `c`.
    pub fn at<K: Kind>(&self, c: &Column<K>) -> Option<K::Out<'_>> {
        self.rx.results().fields().at(c)
    }
    /// A count of result field `c`, 0 when it has none.
    pub fn count(&self, c: &Column<sdrkit_core::publish::field::I64>) -> i64 {
        self.at(c).unwrap_or(0)
    }
    /// The rows of table `t`, newest first.
    pub fn rows(&self, t: &TableDef) -> Vec<(&str, &Record)> {
        rows(self.rx.results(), t)
    }
    /// The status lock state.
    pub fn lock_state(&self) -> LockState {
        self.rx.results().status().lock_state
    }
    pub fn units(&self, kind: StreamKind) -> Vec<&Elementary> {
        self.media.iter().filter(|m| m.kind == kind).collect()
    }
}

/// The rows of table `t` of `r`, newest first.
pub fn rows<'a>(r: &'a sdrkit_core::Results, t: &TableDef) -> Vec<(&'a str, &'a Record)> {
    r.decl()
        .table_id(t.key)
        .and_then(|id| r.table(id))
        .map(|t| t.rows_newest_first().collect())
        .unwrap_or_default()
}

/// The SId of the test ensemble's service on sub-channel `subch`: the default ensemble numbers its services C101 to C104 on
/// sub-channels 1 to 4.
pub fn sid_of(subch: i32) -> String {
    format!("{:04X}", 0xC100 + subch)
}

/// Plays the service whose `services` row key is `row`.
pub fn select(rx: &mut StackGraph, row: &str) {
    let id = rx.commands().id("service.select").expect("the command");
    let mut args = rx.commands().def(id).expect("declared").new_args();
    args.set(&sdrkit_proto_mpegts::publish::SERVICE_ROW.field, row);
    owner::invoke(rx, id, &args).expect("selected");
}

/// A receiver with these settings, configured for `rate`. A switch is given as 0 or 1. `service` is a sub-channel of the
/// test ensemble (see [`sid_of`]).
pub fn receiver(settings: &[(&str, f64)], rate: f64) -> StackGraph {
    let schema = DabSetting::schema();
    let mut values = Values::defaults(&schema);
    let mut service = None;
    for &(k, v) in settings {
        if k == "service" {
            service = Some(v as i32);
            continue;
        }
        let id = schema.id(k).expect("a setting");
        let value = match schema.setting(id).map(|s| s.field.kind) {
            Some(sdrkit_core::ValueKind::Bool) => Value::Bool(v != 0.0),
            Some(sdrkit_core::ValueKind::I64) => Value::I64(v as i64),
            Some(sdrkit_core::ValueKind::Choice) => Value::Choice(v as u16),
            _ => Value::F64(v),
        };
        values.set(id, &value).expect("a valid value");
    }
    let mut rx = sdrkit_stack_dab::receiver::receiver(&StackConfig::new(values)).expect("builds");
    if let Some(s) = service.filter(|&s| s >= 0) {
        select(&mut rx, &sid_of(s));
    }
    rx.configure(&InputInfo::new(rate, 225.648e6, 225.648e6).recording())
        .expect("rate");
    rx
}

/// Feeds `x` in pieces of `piece` to a receiver playing the service on sub-channel `service` (see [`sid_of`]).
pub fn run(x: &[Complex32], rate: f64, service: i32, piece: usize) -> Run {
    run_with(x, rate, &[("service", service as f64)], piece)
}

/// Feeds `x` in pieces of `piece` to a receiver playing the service whose row key is `sid`.
pub fn run_sid(x: &[Complex32], rate: f64, sid: &str, piece: usize) -> Run {
    let mut rx = receiver(&[], rate);
    select(&mut rx, sid);
    feed(rx, x, piece)
}

pub fn run_with(x: &[Complex32], rate: f64, settings: &[(&str, f64)], piece: usize) -> Run {
    feed(receiver(settings, rate), x, piece)
}

/// Writes the stack setting `key` through the checked path.
pub fn set(rx: &mut StackGraph, key: &str, v: Value) {
    let id = rx.schema().id(key).expect("a setting");
    owner::apply(rx, id, &v).expect("taken");
}

/// Feeds `x` in pieces of `piece` to `rx`.
pub fn feed(mut rx: StackGraph, x: &[Complex32], piece: usize) -> Run {
    let mut out = Output::default();
    let mut media = Vec::new();
    let mut audio = Vec::new();
    let mut events = Vec::new();
    for c in x.chunks(piece) {
        out.clear();
        rx.feed(c, &mut out);
        media.append(&mut out.media);
        audio.extend_from_slice(out.audio());
        events.append(&mut out.events);
    }
    Run {
        rx,
        media,
        audio,
        events,
    }
}

/// Seconds of the generator's clean signal at its rate, at the tests' level.
pub fn generate(p: GenParams, seconds: f64) -> Vec<Complex32> {
    generate_air(p, Air::default(), seconds)
}

/// Seconds of the generator's signal at its rate, through the air.
pub fn generate_air(p: GenParams, air: Air, seconds: f64) -> Vec<Complex32> {
    through_air(DabGenerator::new(p), air, seconds)
}

/// Rounds to 8 bits (steps of 1/127), as a dongle delivers.
pub fn quantize8(x: &mut [Complex32]) {
    for v in x.iter_mut() {
        v.re = (v.re * 127.0).round().clamp(-128.0, 127.0) / 127.0;
        v.im = (v.im * 127.0).round().clamp(-128.0, 127.0) / 127.0;
    }
}

/// The amplitude of a tone by a Hann-windowed single-bin DFT.
pub fn tone(x: &[f32], rate: f64, f: f64) -> f64 {
    let n = x.len();
    let (mut re, mut im, mut w) = (0.0, 0.0, 0.0);
    for (i, &v) in x.iter().enumerate() {
        let h = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
        let p = std::f64::consts::TAU * f * i as f64 / rate;
        re += v as f64 * h * p.cos();
        im += v as f64 * h * p.sin();
        w += h;
    }
    2.0 * (re * re + im * im).sqrt() / w
}

/// Decodes compressed audio with ffmpeg when it is installed (`None` otherwise): interleaved stereo float at the stream's rate.
pub fn ffmpeg_decode(units: &[&Elementary]) -> Option<(Vec<f32>, u32)> {
    let first = units.first()?;
    let f = &first.format;
    let dir = std::env::temp_dir().join(format!("sdrkit-dab-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let (input, fmt) = match f.codec {
        sdrkit_core::media::Codec::Mp2 => {
            let mut b = Vec::new();
            for u in units {
                b.extend_from_slice(u.packet.bytes());
            }
            let p = dir.join("a.mp2");
            std::fs::write(&p, b).ok()?;
            (p, "mp3")
        }
        _ => {
            let mut b = Vec::new();
            for u in units {
                let asc = sdrkit_proto_bcdata::aac::AudioSpecificConfig::parse(&u.format.config)
                    .expect("an ASC");
                let mut el = Vec::new();
                sdrkit_proto_bcdata::aac::write_latm(&asc, true, u.packet.bytes(), &mut el)
                    .expect("LATM");
                b.push(0x56);
                b.push(0xE0 | ((el.len() >> 8) as u8 & 0x1F));
                b.push(el.len() as u8);
                b.extend(el);
            }
            let p = dir.join("a.loas");
            std::fs::write(&p, b).ok()?;
            (p, "loas")
        }
    };
    let out = dir.join("a.raw");
    let st = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", fmt, "-i"])
        .arg(&input)
        .args(["-ac", "2", "-f", "f32le", "-acodec", "pcm_f32le"])
        .arg(&out)
        .status()
        .ok()?;
    if !st.success() {
        return None;
    }
    let raw = std::fs::read(&out).ok()?;
    let _ = std::fs::remove_dir_all(&dir);
    let rate = f.audio.as_ref().map_or(48_000, |a| a.sample_rate);
    Some((
        raw.as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
        rate,
    ))
}

pub fn left(x: &[f32]) -> Vec<f32> {
    x.iter().step_by(2).copied().collect()
}
pub fn right(x: &[f32]) -> Vec<f32> {
    x.iter().skip(1).step_by(2).copied().collect()
}

/// The default ensemble's coded loop, made once per test binary.
pub fn default_loop() -> std::sync::Arc<sdrkit_stack_dab::generator::CodedLoop> {
    static L: std::sync::OnceLock<std::sync::Arc<sdrkit_stack_dab::generator::CodedLoop>> =
        std::sync::OnceLock::new();
    L.get_or_init(|| {
        std::sync::Arc::new(sdrkit_stack_dab::generator::code_loop(&GenParams::default()))
    })
    .clone()
}

/// Seconds of the default ensemble with these output options (rate, TII …) through the air.
pub fn default_signal(p: GenParams, air: Air, seconds: f64) -> Vec<Complex32> {
    through_air(DabGenerator::from_loop(p, default_loop()), air, seconds)
}

/// The share of good FIBs.
pub fn fib_share(r: &Run) -> f64 {
    let ok = r.count(&SyncFields::FIB_OK) as f64;
    let bad = r.count(&SyncFields::FIB_BAD) as f64;
    if ok + bad == 0.0 {
        0.0
    } else {
        ok / (ok + bad)
    }
}

/// A good decode: locked, the FIB share at least `share`, 4 services, more than 50 good access units with at most 1 bad in
/// 20.
pub fn assert_good(r: &Run, share: f64, what: &str) {
    assert_eq!(r.lock_state(), LockState::Locked, "{what}: not locked");
    assert!(fib_share(r) >= share, "{what}: FIB share {}", fib_share(r));
    assert_eq!(r.rows(&SERVICES).len(), 4, "{what}: services");
    let ok = r.count(&AudioFields::AUDIO_UNITS_OK) as f64;
    let bad = r.count(&AudioFields::AUDIO_UNITS_BAD) as f64;
    assert!(ok > 50.0, "{what}: {ok} access units");
    assert!(bad * 20.0 <= ok, "{what}: {bad} bad of {ok}");
}

/// A multipath channel at 2.048 Msps through the shared delay line: paths of (delay samples, level dB), Rayleigh with `fd` Hz
/// Doppler, or static with `fd` 0. The output keeps the total power of the paths (the delay line itself has unit gain).
pub fn fading(x: &[Complex32], paths: &[(usize, f64)], fd: f64, seed: u64) -> Vec<Complex32> {
    let doppler = if fd > 0.0 {
        Doppler::Jakes { max_hz: fd }
    } else {
        Doppler::Static
    };
    let taps: Vec<Tap> = paths
        .iter()
        .map(|&(d, db)| Tap::new(d as f64 / 2.048e6, db, doppler))
        .collect();
    let total: f64 = paths.iter().map(|p| 10f64.powf(p.1 / 10.0)).sum();
    let mut tdl = Tdl::new(&taps, 2.048e6, seed).expect("a valid delay line");
    let mut y = x.to_vec();
    tdl.process(&mut y);
    sdrkit_algo_sim::front::scale(&mut y, total.sqrt() as f32);
    y
}
