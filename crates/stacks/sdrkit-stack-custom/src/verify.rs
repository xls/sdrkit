//! The test-signal checks and the benchmarks of the stacks.

use crate::arq_packet::stack as ap;
use crate::file_transfer::stack as ft;
use crate::hf_chat::stack as hc;
use crate::picture_burst::stack as pb;
use crate::qam_video::stack as qv;
use crate::slotted_voice::stack as sv;
use crate::typed::{
    ArqPacketFields, FileTransferFields, HfChatFields, PictureBurstFields, QamVideoFields, SlottedVoiceFields,
};
use sdrkit_core::interface::Column;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    BenchResult, Benchmark, Complex32, GenConfig, InputInfo, Output, Results, StackConfig, StackDescriptor, Values,
};

/// Times the receiver of `desc` on `signal_s` seconds of its own test signal at its default rate.
pub fn bench_receiver(seconds: f64, desc: &'static StackDescriptor, signal_s: f64) -> BenchResult {
    bench_receiver_with(seconds, desc, signal_s, desc.tuning.rate_hz, &[])
}

/// Times the receiver of `desc` on `signal_s` seconds of its own test signal at `rate`, with the settings `set` (key and
/// choice key) applied to the generator and the receiver.
pub fn bench_receiver_with(seconds: f64, desc: &'static StackDescriptor, signal_s: f64, rate: f64, set: &[(&str, &str)]) -> BenchResult {
    let mut settings = Values::defaults(&(desc.settings)());
    for (key, choice) in set {
        let schema = settings.schema().clone();
        let Ok(id) = schema.id(key) else {
            return BenchResult::default();
        };
        let Some(i) = schema.setting(id).and_then(|d| d.field.choice_index(choice)) else {
            return BenchResult::default();
        };
        if settings.set(id, &sdrkit_core::Value::Choice(i)).is_err() {
            return BenchResult::default();
        }
    }
    let Some(make) = desc.make_generator else {
        return BenchResult::default();
    };
    let Ok(mut g) = make(&GenConfig::new(rate, settings.clone()).with_seed(1)) else {
        return BenchResult::default();
    };
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    let Ok(mut rx) = (desc.make_receiver)(&StackConfig::new(settings)) else {
        return BenchResult::default();
    };
    let _ = rx.configure(&InputInfo::new(rate, desc.tuning.default_hz, desc.tuning.default_hz).recording());
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for c in x.chunks(65_536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// The benchmarks of the crate's receivers on their own test signals (items are input samples).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "slotted_voice.receiver_240k",
            run: |s| bench_receiver(s, &sv::SLOTTED_VOICE, 6.0),
        },
        Benchmark {
            name: "qam_video.receiver_2m4",
            run: |s| bench_receiver(s, &qv::QAM_VIDEO, 2.0),
        },
        Benchmark {
            name: "qam_video.receiver_3m2_d",
            run: |s| bench_receiver_with(s, &qv::QAM_VIDEO, 2.0, 3.2e6, &[("profile", "d"), ("listen_profile", "cd")]),
        },
        Benchmark {
            name: "picture_burst.receiver_1m",
            run: |s| bench_receiver(s, &pb::PICTURE_BURST, 11.0),
        },
    ]
}

fn at_least(r: &Results, c: &Column<field::I64>, min: i64) -> CheckLine {
    let v = r.fields().at(c).unwrap_or(0);
    CheckLine::new(c.key, v >= min, format!("{v}, at least {min} expected"))
}

fn sv_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, version 3 at 16 kbit/s, +1200 Hz", o, 1200.0);
    c.seconds = o.seconds.max(12.0);
    c.expect_audio = Some(true);
    c.expect_lock = true;
    c.check = Some(|r| {
        let f = r.fields();
        let ok = f.at(&SlottedVoiceFields::FRAMES_OK).unwrap_or(0);
        let bad = f.at(&SlottedVoiceFields::FRAMES_BAD).unwrap_or(0);
        let off = f.at(&SlottedVoiceFields::OFFSET_HZ).unwrap_or(0.0);
        let call = f.at(&SlottedVoiceFields::CALLSIGN).unwrap_or_default().to_string();
        vec![
            at_least(r, &SlottedVoiceFields::FRAMES_OK, 200),
            CheckLine::new("frames_bad", bad * 20 < ok.max(1), format!("{bad} bad of {ok}")),
            CheckLine::new("offset_hz", (off - 1200.0).abs() < 150.0, format!("{off:.0} Hz, 1200 expected")),
            CheckLine::new("callsign", call == crate::family::DEMO_CALLSIGN, format!("{call:?}")),
        ]
    });
    vec![c]
}

/// The checks of `slotted_voice`.
pub static SLOTTED_VOICE_VERIFY: VerifyHooks = VerifyHooks {
    cases: sv_cases,
    vector_settings: None,
    check_vector: None,
};

fn qv_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, profile A, +3000 Hz", o, 3000.0);
    c.seconds = o.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.expect_lock = true;
    c.check = Some(|r| {
        let f = r.fields();
        let ok = f.at(&QamVideoFields::FRAMES_OK).unwrap_or(0);
        let bad = f.at(&QamVideoFields::FRAMES_BAD).unwrap_or(0);
        let off = f.at(&QamVideoFields::OFFSET_HZ).unwrap_or(0.0);
        vec![
            at_least(r, &QamVideoFields::FRAMES_OK, 10),
            CheckLine::new("frames_bad", bad * 10 <= ok, format!("{bad} bad of {ok}")),
            at_least(r, &QamVideoFields::PACKETS, 20),
            CheckLine::new("offset_hz", (off - 3000.0).abs() < 400.0, format!("{off:.0} Hz, 3000 expected")),
        ]
    });
    vec![c]
}

/// The checks of `qam_video`.
pub static QAM_VIDEO_VERIFY: VerifyHooks = VerifyHooks {
    cases: qv_cases,
    vector_settings: None,
    check_vector: None,
};

fn pb_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, profile 0, +500 Hz", o, 500.0);
    c.seconds = o.seconds.max(12.0);
    c.expect_audio = Some(false);
    c.expect_lock = true;
    c.check = Some(|r| {
        let f = r.fields();
        let n = f.at(&PictureBurstFields::LAYERS).unwrap_or(0);
        let sent = f.at(&PictureBurstFields::LAYERS_SENT).unwrap_or(-1);
        let off = f.at(&PictureBurstFields::OFFSET_HZ).unwrap_or(0.0);
        vec![
            CheckLine::new("layers", n == sent, format!("{n} of {sent} layers")),
            CheckLine::new("offset_hz", (off - 500.0).abs() < 150.0, format!("{off:.0} Hz, 500 expected")),
        ]
    });
    vec![c]
}

/// The checks of `picture_burst`.
pub static PICTURE_BURST_VERIFY: VerifyHooks = VerifyHooks {
    cases: pb_cases,
    vector_settings: None,
    check_vector: None,
};

fn ap_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, profile 2, +1000 Hz, 20 dB", o, 1000.0);
    c.seconds = o.seconds.max(20.0);
    c.snr_db = Some(20.0);
    c.expect_audio = Some(false);
    c.expect_lock = false;
    c.check = Some(|r| {
        let f = r.fields();
        let good = f.at(&ArqPacketFields::BLOCKS_GOOD).unwrap_or(0);
        let bad = f.at(&ArqPacketFields::BLOCKS_BAD).unwrap_or(0);
        let off = f.at(&ArqPacketFields::OFFSET_HZ).unwrap_or(0.0);
        let p = f.at(&ArqPacketFields::PROFILE).unwrap_or(-1);
        vec![
            at_least(r, &ArqPacketFields::MESSAGES, 3),
            CheckLine::new("blocks_bad", bad * 10 <= good && good > 0, format!("{bad} bad of {good}")),
            CheckLine::new("offset_hz", (off - 1000.0).abs() < 200.0, format!("{off:.0} Hz, 1000 expected")),
            CheckLine::new("profile", p == 2, format!("{p}, 2 expected")),
        ]
    });
    vec![c]
}

/// The checks of `arq_packet`.
pub static ARQ_PACKET_VERIFY: VerifyHooks = VerifyHooks {
    cases: ap_cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmark of `arq_packet`: its receiver on its test signal at 250 ksps.
pub fn arq_packet_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "arq_packet.receiver_250k",
        run: |s| bench_receiver(s, &ap::ARQ_PACKET, 8.0),
    }]
}

fn ft_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, 200 kHz, profile 2, +1500 Hz, 20 dB", o, 1500.0);
    c.seconds = o.seconds.max(4.0);
    c.snr_db = c.snr_db.map(|_| 20.0);
    c.expect_audio = Some(false);
    c.expect_lock = false;
    c.check = Some(|r| {
        let f = r.fields();
        let good = f.at(&FileTransferFields::BLOCKS_GOOD).unwrap_or(0);
        let bad = f.at(&FileTransferFields::BLOCKS_BAD).unwrap_or(0);
        let off = f.at(&FileTransferFields::OFFSET_HZ).unwrap_or(0.0);
        let hash = f.at(&FileTransferFields::HASH_OK);
        let width = f.at(&FileTransferFields::WIDTH).unwrap_or_default().to_string();
        vec![
            at_least(r, &FileTransferFields::FILES, 1),
            CheckLine::new("hash_ok", hash == Some(true), format!("{hash:?}")),
            CheckLine::new("blocks_bad", bad * 10 <= good && good > 0, format!("{bad} bad of {good}")),
            CheckLine::new("offset_hz", (off - 1500.0).abs() < 300.0, format!("{off:.0} Hz, 1500 expected")),
            CheckLine::new("width", width == "200 kHz", width),
        ]
    });
    vec![c]
}

/// The checks of `file_transfer`.
pub static FILE_TRANSFER_VERIFY: VerifyHooks = VerifyHooks {
    cases: ft_cases,
    vector_settings: None,
    check_vector: None,
};

/// Times the `file_transfer` receiver on `signal_s` seconds of a one-way transfer that fills them at profile 9 in the width
/// of choice `tx` (0 1 MHz, 1 500 kHz, 2 200 kHz) at 2.4 Msps, with white noise for an Es/N0 of 35 dB, searching the widths of
/// choice `rx` of the `widths` setting.
fn bench_ft(seconds: f64, tx: u8, rx: u16, signal_s: f64) -> BenchResult {
    use crate::file_transfer::{stack::carousel_samples, Width, TX_LEVEL};
    let d = &ft::FILE_TRANSFER;
    let mut settings = Values::defaults(&(d.settings)());
    let Ok(ws) = settings.schema().id("widths") else {
        return BenchResult::default();
    };
    if settings.set(ws, &sdrkit_core::Value::Choice(rx)).is_err() {
        return BenchResult::default();
    }
    let Some(width) = Width::of(tx) else {
        return BenchResult::default();
    };
    let rate = d.tuning.rate_hz;
    let n = (rate * signal_s) as usize;
    // About 0.9 of the coded rate of profile 9 for the whole time.
    let bytes = (width.symbol_rate() * 10.0 * 0.9 * 0.8 / 8.0 * signal_s) as usize;
    let payload: Vec<u8> = (0..bytes).map(|i| (i * 7 + i / 251) as u8).collect();
    let mut x = carousel_samples(&payload, "bench.bin", "TEST1", 1, 9, width, 1, rate);
    x.resize(n, Complex32::new(0.0, 0.0));
    // Unit power at 4 samples a symbol; the noise has the density of Es/N0 35 dB in the width.
    let var = 4.0 / 10f64.powf(3.5) * width.rate() / rate;
    let mut s = 0x2545_F491_4F6C_DD1Du64;
    let mut uni = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    };
    for v in &mut x {
        let r = (-var * uni().ln()).sqrt();
        let a = std::f64::consts::TAU * uni();
        *v = *v / TX_LEVEL + Complex32::new((r * a.cos()) as f32, (r * a.sin()) as f32);
    }
    let Ok(mut rx) = (d.make_receiver)(&StackConfig::new(settings)) else {
        return BenchResult::default();
    };
    let _ = rx.configure(&InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording());
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for c in x.chunks(65_536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// The benchmarks of `file_transfer`: its receiver at 2.4 Msps on 3 s of frames at profile 9 with noise, one width searched
/// (the width of the signal), and all three widths searched on the 1 MHz signal.
pub fn file_transfer_benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark { name: "file_transfer.receiver_1m", run: |s| bench_ft(s, 0, 1, 3.0) },
        Benchmark { name: "file_transfer.receiver_500k", run: |s| bench_ft(s, 1, 2, 3.0) },
        Benchmark { name: "file_transfer.receiver_200k", run: |s| bench_ft(s, 2, 3, 3.0) },
        Benchmark { name: "file_transfer.receiver_all", run: |s| bench_ft(s, 0, 0, 3.0) },
    ]
}

fn hc_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, profile 3, +30 Hz", o, 30.0);
    c.seconds = o.seconds.max(25.0);
    c.expect_audio = Some(false);
    c.expect_lock = false;
    c.check = Some(|r| {
        let f = r.fields();
        let good = f.at(&HfChatFields::BLOCKS_GOOD).unwrap_or(0);
        let bad = f.at(&HfChatFields::BLOCKS_BAD).unwrap_or(0);
        let off = f.at(&HfChatFields::OFFSET_HZ).unwrap_or(0.0);
        let p = f.at(&HfChatFields::PROFILE).unwrap_or(-1);
        let call = f.at(&HfChatFields::CALLSIGN).unwrap_or_default().to_string();
        vec![
            at_least(r, &HfChatFields::MESSAGES, 1),
            CheckLine::new("blocks_bad", bad == 0 && good > 0, format!("{bad} bad of {good}")),
            CheckLine::new("offset_hz", (off - 30.0).abs() < 20.0, format!("{off:.1} Hz, 30 expected")),
            CheckLine::new("profile", p == 3, format!("{p}, 3 expected")),
            CheckLine::new("callsign", call == crate::family::DEMO_CALLSIGN, format!("{call:?}")),
        ]
    });
    vec![c]
}

/// The checks of `hf_chat`.
pub static HF_CHAT_VERIFY: VerifyHooks = VerifyHooks {
    cases: hc_cases,
    vector_settings: None,
    check_vector: None,
};

/// Times the `hf_chat` receiver, searching all eight profiles, on `signal_s` seconds of its test signal at `rate`, the radio
/// tuned where the test signal puts it (20 kHz above the dial).
fn bench_hc(seconds: f64, rate: f64, signal_s: f64) -> BenchResult {
    let d = &hc::HF_CHAT;
    let settings = Values::defaults(&(d.settings)());
    let Some(make) = d.make_generator else {
        return BenchResult::default();
    };
    let Ok(mut g) = make(&GenConfig::new(rate, settings.clone()).with_seed(1)) else {
        return BenchResult::default();
    };
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * signal_s) as usize];
    g.generate(&mut x);
    let Ok(mut rx) = (d.make_receiver)(&StackConfig::new(settings)) else {
        return BenchResult::default();
    };
    let dial = d.tuning.default_hz;
    let centre = dial - g.station_offset_hz(d.tuning.tune_offset_hz);
    let _ = rx.configure(&InputInfo::new(rate, centre, dial).recording());
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), signal_s, || {
        for c in x.chunks(65_536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// The benchmark of `hf_chat`: its receiver, searching all eight profiles, on 25 s of its test signal at 250 ksps.
pub fn hf_chat_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "hf_chat.receiver_250k",
        run: |s| bench_hc(s, 250_000.0, 25.0),
    }]
}
