//! Decoder edge cases, allocation counts of the decode paths, setting refusals and saved state, one test each.
//!
//! The allocation tests count with an allocator that counts only on the thread that enables it, so other tests that run at the
//! same time do not change the count.

use crate::common::*;
use sdrkit_core::{owner, Complex32, InputInfo, Owner, Receiver, StackConfig, StateBlob, Value};
use sdrkit_dev_test::Harness;
use sdrkit_proto_pager::pocsag::{encode_batches, Content, NumericSet, PocsagTx, SYNC};
use sdrkit_stack_cell::gsm::codes::{encode_bcch, encode_sch, interleave, GsmDecoder, Sch};
use sdrkit_stack_cell::gsm::search::fcch_hits;
use sdrkit_stack_cell::lbj::message::pages;
use sdrkit_stack_cell::lte::phy::{
    append_crc16, append_crc24a, tbcc_encode, turbo_block, turbo_encode, Tbcc, TurboRx,
};
use sdrkit_stack_cell::nbiot::phy::{decode_mib_nb, encode_mib_nb};
use sdrkit_stack_cell::nr::phy::{encode_pbch, Mib, PbchDecoder};
use sdrkit_stack_cell::GSMR;
use sdrkit_util_bits::crc::catalog::{
    CRC10_GSM, CRC11_NR, CRC16_XMODEM, CRC24_LTE_A, CRC24_LTE_B, CRC24_NR_C, CRC40_GSM, CRC6_NR,
    CRC8_LTE,
};
use sdrkit_util_bits::pack::unpack;
use sdrkit_util_bits::BitOrder;
use serde_json::Value as J;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn note() {
    // `try_with` fails only while the thread ends; nothing is counted then.
    let _ = COUNTING.try_with(|on| {
        if on.get() {
            let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        }
    });
}

// SAFETY: every call goes to the system allocator unchanged. The counter only reads and writes thread-local cells.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        note();
        // SAFETY: the layout comes from the caller, as for the system allocator.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` was given by `alloc` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        note();
        // SAFETY: `p` was given by `alloc` with layout `l`.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The allocations that `f` makes on this thread.
fn allocations<T>(f: impl FnOnce() -> T) -> (T, usize) {
    ALLOCS.with(|n| n.set(0));
    COUNTING.with(|on| on.set(true));
    let r = f();
    COUNTING.with(|on| on.set(false));
    (r, ALLOCS.with(|n| n.get()))
}

/// Soft values of `bits` (positive means 0) at `level`, with every seventh value weakened.
fn soft_of(bits: &[u8], level: f32) -> Vec<f32> {
    bits.iter()
        .enumerate()
        .map(|(i, &b)| {
            let v = if b == 0 { level } else { -level };
            if i % 7 == 3 {
                0.3 * v
            } else {
                v
            }
        })
        .collect()
}

// The decoders of the crate refuse soft values that are not finite and return no result.
#[test]
fn viterbi_paths_refuse_nan_and_infinite_soft_values() {
    // LTE PBCH: 40 bits (MIB and CRC16) into 1920.
    let mut mib = vec![0u8; 24];
    mib[3] = 1;
    append_crc16(&mut mib, 0);
    let mut soft = soft_of(&tbcc_encode(&mib, 1920), 2.0);
    let mut tbcc = Tbcc::new();
    let mut out = [0u8; 40];
    assert!(tbcc.decode(&soft, 40, &mut out));
    assert_eq!(&out[..], &mib[..]);
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        soft[100] = bad;
        assert!(!tbcc.decode(&soft, 40, &mut out), "{bad}");
    }
    // NB-IoT MIB-NB through its decode function.
    let mut nb = soft_of(&encode_mib_nb(&[1; 34], 1), 2.0);
    assert!(decode_mib_nb(&mut tbcc, &nb, 1).is_some());
    nb[7] = f32::NAN;
    assert!(decode_mib_nb(&mut tbcc, &nb, 1).is_none());
    // GSM BCCH and SCH.
    let mut gsm = GsmDecoder::new();
    let msg = [0x2Bu8; 23];
    let mut bursts =
        interleave(&encode_bcch(&msg)).map(|b| b.map(|x| if x == 0 { 1.0f32 } else { -1.0 }));
    assert_eq!(gsm.decode_bcch(&bursts), Some(msg));
    bursts[2][50] = f32::NAN;
    assert_eq!(gsm.decode_bcch(&bursts), None);
    bursts[2][50] = f32::INFINITY;
    assert_eq!(gsm.decode_bcch(&bursts), None);
    let sch = Sch::from_fn(10, 52);
    let mut s = soft_of(&encode_sch(&sch), 1.0);
    assert_eq!(gsm.decode_sch(&s), Some(sch));
    s[0] = f32::NEG_INFINITY;
    assert_eq!(gsm.decode_sch(&s), None);
}

// The page reader of sdrkit-proto-pager repairs a message word with 3 weak wrong bits (a search over the least reliable bits).
#[test]
fn lbj_message_word_with_three_weak_wrong_bits_is_repaired() {
    let tx = PocsagTx {
        ric: 1_234_000,
        function: 1,
        content: Content::Numeric("19533"),
    };
    let mut words = Vec::new();
    encode_batches(&[tx], NumericSet::Parentheses, &mut words).unwrap();
    assert_eq!(words[0], SYNC);
    let mut cws: Vec<u32> = words[1..17].to_vec();
    // Slot 0 is the address (RIC 1234000 is frame 0); slot 1 is the message word.
    let mut rel = vec![1.0f32; cws.len() * 32];
    for bit in [3usize, 12, 25] {
        cws[1] ^= 1 << (31 - bit);
        rel[32 + bit] = 0.2;
    }
    let p = pages(&cws, Some(&rel));
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].address, 1_234_000);
    assert_eq!(p[0].text.trim_end(), "19533");
    assert_eq!(p[0].lost_words, 0);
    assert_eq!(p[0].bits_repaired, 3);
    // Without the reliabilities the hard repair alone cannot fix 3 bits.
    let hard = pages(&cws, None);
    assert_eq!(hard[0].lost_words, 1);
}

// The NR PBCH decoder allocates nothing after its first call.
#[test]
fn nr_pbch_decode_allocates_nothing_after_the_first_call() {
    let m = Mib {
        sfn: 517,
        hrf: 1,
        scs_common_30: true,
        k_ssb: 3,
        dmrs_pos3: false,
        coreset0: 12,
        search_space0: 2,
        barred: false,
        intra_freq_allowed: true,
    };
    let soft = soft_of(&encode_pbch(&m, 501, 5, 8), 2.0);
    let mut dec = PbchDecoder::new();
    assert_eq!(dec.decode(&soft, 501, 5), Some(m.clone()));
    let (got, n) = allocations(|| dec.decode(&soft, 501, 5));
    assert_eq!(got, Some(m));
    assert_eq!(n, 0, "allocations in a PBCH decode");
}

// The system information path (rate dematching, combining and turbo decoding) and the DCI path allocate nothing after warm-up.
#[test]
fn sib_and_dci_decode_paths_allocate_nothing_after_warm_up() {
    let tbs = 208;
    let (k, f) = turbo_block(tbs).unwrap();
    let mut tb: Vec<u8> = (0..tbs).map(|i| ((i * 7 + 3) % 5 == 0) as u8).collect();
    append_crc24a(&mut tb);
    let mut block = vec![0u8; f];
    block.extend_from_slice(&tb);
    let e = 1200;
    let rv0 = soft_of(&turbo_encode(&block, f, 0, e), 2.0);
    let rv2 = soft_of(&turbo_encode(&block, f, 2, e), 2.0);
    let mut rx = TurboRx::default();
    let run = |rx: &mut TurboRx| {
        assert!(rx.begin(k, f));
        assert!(rx.add(&rv0, 0));
        assert!(rx.add(&rv2, 2));
        rx.decode(8).map(|d| d[f..f + tbs] == tb[..tbs])
    };
    assert_eq!(run(&mut rx), Some(true));
    let (ok, n) = allocations(|| run(&mut rx));
    assert_eq!(ok, Some(true));
    assert_eq!(n, 0, "allocations in a SIB decode");

    // A DCI on SI-RNTI: 43 bits with the CRC into 288.
    let mut dci: Vec<u8> = (0..27).map(|i| (i % 3 == 0) as u8).collect();
    append_crc16(&mut dci, 0xFFFF);
    let soft = soft_of(&tbcc_encode(&dci, 288), 2.0);
    let mut tbcc = Tbcc::new();
    let mut out = [0u8; 43];
    let mut re = [0u8; 288];
    assert!(tbcc.decode(&soft, 43, &mut out));
    let (ok, n) = allocations(|| tbcc.decode(&soft, 43, &mut out) && tbcc.encode(&out, &mut re));
    assert!(ok);
    assert_eq!(&out[..], &dci[..]);
    assert_eq!(n, 0, "allocations in a DCI decode and encode");
}

// The catalogue CRCs that the crate uses give the published check values of "123456789", over bytes and over bits.
#[test]
fn catalog_crcs_used_by_cell_give_published_check_values() {
    let data = b"123456789";
    let mut bits = Vec::new();
    unpack(data, BitOrder::Msb, &mut bits);
    assert_eq!(CRC24_LTE_A.checksum(data), 0xCDE703);
    assert_eq!(CRC24_LTE_B.checksum(data), 0x23EF52);
    assert_eq!(CRC24_NR_C.checksum(data), 0xF48279);
    assert_eq!(CRC8_LTE.checksum(data), 0xEA);
    assert_eq!(CRC11_NR.checksum(data), 0x5CA);
    assert_eq!(CRC6_NR.checksum(data), 0x15);
    assert_eq!(CRC16_XMODEM.checksum(data), 0x31C3);
    assert_eq!(CRC24_LTE_A.bits(&bits), 0xCDE703);
    assert_eq!(CRC24_LTE_B.bits(&bits), 0x23EF52);
    assert_eq!(CRC24_NR_C.bits(&bits), 0xF48279);
    assert_eq!(CRC8_LTE.bits(&bits), 0xEA);
    assert_eq!(CRC11_NR.bits(&bits), 0x5CA);
    assert_eq!(CRC6_NR.bits(&bits), 0x15);
    assert_eq!(CRC16_XMODEM.bits(&bits), 0x31C3);
    // The GSM codes of 45.003 (FIRE code and SCH parity, both sent inverted).
    assert_eq!(CRC40_GSM.bits(&bits), 0xD4164FC646);
    assert_eq!(CRC10_GSM.bits(&bits), 0x12A);
}

// The FCCH detector finds a weak burst after a long, strong signal.
#[test]
fn fcch_late_after_a_long_strong_prefix_is_found() {
    // 2 000 000 stream samples of a strong signal with random phases, then an FCCH burst at amplitude 1, then noise.
    let prefix = 2_000_000usize;
    let burst = 592usize;
    let mut s = 0x2545_F491_4F6C_DD1Du64;
    let mut phase = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64 * std::f64::consts::TAU
    };
    let mut y: Vec<Complex32> = (0..prefix)
        .map(|_| Complex32::from_polar(100.0, phase() as f32))
        .collect();
    // The FCCH phase advances a quarter turn per symbol, 4 samples (45.002 5.2.4).
    y.extend(
        (0..burst)
            .map(|n| Complex32::from_polar(1.0, (std::f64::consts::FRAC_PI_8 * n as f64) as f32)),
    );
    // Noise at the level of the burst after it, so only the windows inside the burst hold the tone.
    y.extend((0..2000).map(|_| Complex32::from_polar(1.0, phase() as f32)));
    let hits = fcch_hits(&y);
    assert_eq!(hits.len(), 1, "{hits:?}");
    let centre = prefix + burst / 2;
    assert!(hits[0].centre.abs_diff(centre) < 100, "{hits:?}");
    assert!(hits[0].err_hz.abs() < 50.0, "{hits:?}");
}

// A GSM setting value of the wrong kind or out of range is refused, and the setting keeps its value.
#[test]
fn gsmr_bad_setting_values_are_refused() {
    let mut rx = stack_graph(&GSMR, &[]);
    let schema = Owner::schema(&rx).clone();
    let id = |k: &str| schema.id(k).unwrap();
    for (key, v) in [
        ("max_carriers", Value::I64(0)),
        ("max_carriers", Value::I64(25)),
        ("max_carriers", Value::F64(f64::NAN)),
        ("max_carriers", Value::Text("many".into())),
        ("gsm_band", Value::Choice(2)),
        ("gsm_band", Value::Bool(true)),
    ] {
        assert!(owner::apply(&mut rx, id(key), &v).is_err(), "{key} = {v:?}");
    }
    assert!(owner::apply(&mut rx, id("max_carriers"), &Value::I64(3)).is_ok());
    assert!(owner::apply(&mut rx, id("gsm_band"), &Value::Choice(1)).is_ok());
    let v = Owner::values(&rx).expect("the values");
    assert_eq!(v.value(id("max_carriers")), Some(&Value::I64(3)));
    assert_eq!(v.value(id("gsm_band")), Some(&Value::Choice(1)));
}

// The GSM cell table leaves as saved state, which a new receiver restores. Ages are kept, clear empties the table, and a
// state of another stack is refused.
#[test]
fn gsmr_saved_state_round_trips_the_cell_table() {
    let x = Harness::new(&GSMR)
        .seconds(2.3)
        .generate()
        .expect("the test signal");
    let input = InputInfo::new(5e6, 922.8e6, 922.8e6).recording();
    let mut rx = stack_graph(&GSMR, &[]);
    let saved = run_graph(&mut rx, input, &x, &[]);
    let before = cells(&saved);
    assert_eq!(before.len(), 2);
    let text = rx.save_state().unwrap();

    let mut rx2 = stack_graph(&GSMR, &[]);
    rx2.configure(&input).unwrap();
    rx2.restore_state(&text).unwrap();
    let after = cells(&state(&rx2));
    assert_eq!(after.len(), 2);
    let age = |s: &J, c: &J| s["clock"].as_f64().unwrap() - c["seen"]["last_s"].as_f64().unwrap();
    let saved2 = state(&rx2);
    let (h1, h2) = (held(&saved), held(&saved2));
    for (a, b) in before.iter().zip(&after) {
        let (mut a2, mut b2) = (a.clone(), b.clone());
        a2.as_object_mut().unwrap().remove("seen");
        b2.as_object_mut().unwrap().remove("seen");
        assert_eq!(a2, b2);
        assert_eq!(a["seen"]["count"], b["seen"]["count"]);
        let (ta, tb) = (
            age(&saved["components"]["search"], a),
            age(&saved2["components"]["search"], b),
        );
        assert!((ta - tb).abs() < 1e-9, "{ta} {tb}");
    }
    let texts = |h: &J| -> Vec<J> {
        h["log"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["text"].clone())
            .collect()
    };
    assert!(!texts(h1).is_empty());
    assert_eq!(texts(h1), texts(h2));
    assert_eq!(h1["decoded"], h2["decoded"]);
    assert_eq!(table_len(rx2.results(), "cells"), 2);

    clear(&mut rx2);
    assert!(cells(&state(&rx2)).is_empty());
    rx2.restore_state(&text).unwrap();
    assert_eq!(cells(&state(&rx2)).len(), 2);

    let other = StateBlob::new("cell", text.version(), text.bytes().to_vec());
    clear(&mut rx2);
    assert!(rx2.restore_state(&other).is_err());
    assert!(cells(&state(&rx2)).is_empty());
}

// The POCSAG idle word is 0x7A89C197. ITU-R M.584-2 table 2 prints 0x7AC9C197, which is not a code word of its own
// BCH(31,21) code. The LBJ generator pads with the word on the air.
#[test]
fn pocsag_idle_word_is_the_one_on_the_air() {
    use sdrkit_proto_pager::pocsag::{IDLE, PREAMBLE_BITS};
    use sdrkit_stack_cell::lbj::gen::{burst_bits, demo_bursts};
    assert_eq!(IDLE, 0x7A89_C197);
    let bits = burst_bits(&demo_bursts()[0]);
    let words: Vec<u32> = bits[PREAMBLE_BITS..]
        .chunks(32)
        .map(|w| w.iter().fold(0u32, |a, &b| (a << 1) | u32::from(b)))
        .collect();
    assert_eq!(words[0], SYNC);
    assert!(words.contains(&0x7A89_C197), "{words:08X?}");
    assert!(!words.contains(&0x7AC9_C197));
}

// A later NR PSS peak of one N_ID2 must reach the larger of the threshold and 12 dB, as in the LTE PSS search.
#[test]
fn nr_second_pss_peaks_need_12_db() {
    use sdrkit_stack_cell::nr::search::{pick_peaks, SECOND_PEAK_DB};
    assert_eq!(SECOND_PEAK_DB, 12.0);
    let mut fold = vec![vec![1.0f32; 2000]];
    let mean = (2000.0 - 3.0 + 110.0 + 21.3 + 13.4) / 2000.0;
    fold[0][100] = 110.0; // 20 dB over the mean
    fold[0][700] = 21.3; // 13 dB: a second cell
    fold[0][1500] = 13.4; // 11 dB: over the 10 dB threshold, under 12 dB
    let db = |v: f32| 10.0 * (v / mean as f32).log10();
    assert!(db(13.4) > 10.0 && db(13.4) < 12.0 && db(21.3) > 12.0);
    let mut peaks = Vec::new();
    pick_peaks(&mut fold, 0, 0, 30, 10.0, &mut peaks);
    let at: Vec<usize> = peaks.iter().map(|p| p.pos).collect();
    assert_eq!(at, vec![100, 700]);
}

// LTE channel estimation keeps its buffers: the second subframe allocates nothing.
#[test]
fn lte_channel_estimation_allocates_nothing_after_the_first_subframe() {
    use sdrkit_stack_cell::lte::search::{Chan, ChanEstimator, Grid};
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 40) as f32 / (1u64 << 24) as f32 - 0.5
    };
    let g: Grid = (0..14)
        .map(|_| (0..300).map(|_| Complex32::new(next(), next())).collect())
        .collect();
    let mut est = ChanEstimator::default();
    let mut ch = Chan::default();
    for sf in [0, 5] {
        est.estimate(&g, 301, 25, sf, 4, false, 3, &mut ch);
    }
    let (_, n) = allocations(|| est.estimate(&g, 301, 25, 1, 4, false, 3, &mut ch));
    assert_eq!(n, 0, "allocations in a channel estimate");
    assert_eq!(ch.h.len(), 4);
    assert!(ch.noise.is_finite() && ch.rsrp > 0.0);
}

// A setting value of the wrong kind or out of range is refused in every stack, and a band list that does not parse is
// refused when the receiver is built. A test signal setting out of range is refused before it reaches the generator.
#[test]
fn bad_setting_values_are_refused_in_every_stack() {
    use sdrkit_stack_cell::{CELL, EBICAB, LBJ, NBIOT, NR};
    for (desc, key, bad) in [
        (&CELL, "max_cells", Value::I64(25)),
        (&CELL, "hide_bands", Value::Text("B20".into())),
        (&CELL, "min_rsrp", Value::F64(-130.0)),
        (&NR, "max_cells", Value::I64(13)),
        (&NR, "mib_only", Value::Text("perhaps".into())),
        (&NBIOT, "min_npss", Value::F64(11.0)),
        (&NBIOT, "freq_correction_ppm", Value::F64(f64::NAN)),
        (&GSMR, "max_carriers", Value::I64(0)),
    ] {
        let mut rx = stack_graph(desc, &[]);
        let id = Owner::schema(&rx).id(key).unwrap();
        assert!(
            owner::apply(&mut rx, id, &bad).is_err(),
            "{} {key} = {bad:?}",
            desc.id
        );
    }
    // `clear` is a command, not a setting.
    for desc in [&LBJ, &EBICAB] {
        assert!((desc.settings)().id("clear").is_err(), "{}", desc.id);
        let rx = stack_graph(desc, &[]);
        assert!(rx.commands().id("clear").is_ok(), "{}", desc.id);
    }
    // A band list that does not parse fails the build.
    let schema = (CELL.settings)();
    let mut values = sdrkit_core::Values::defaults(&schema);
    values
        .set(schema.id("hide_bands").unwrap(), &Value::Text("B20".into()))
        .unwrap();
    assert!(sdrkit_stack_cell::lte::build(&StackConfig::new(values)).is_err());
    // Test signal values out of range never reach the generator.
    for (desc, key, bad) in [
        (&CELL, "gen_scene", Value::Choice(9)),
        (&NR, "gen_scene", Value::Choice(9)),
        (&NBIOT, "gen_load", Value::Text("maybe".into())),
        (&GSMR, "gen_centre_hz", Value::Hz(1e9)),
        (&LBJ, "gen_deviation_hz", Value::Hz(99e3)),
        (&EBICAB, "gen_burst_ms", Value::F64(0.0)),
    ] {
        let schema = (desc.settings)();
        let mut values = sdrkit_core::Values::defaults(&schema);
        assert!(
            values.set(schema.id(key).unwrap(), &bad).is_err(),
            "{} {key}",
            desc.id
        );
    }
}

// The tables of every stack leave as saved state, which a new receiver restores, and a state of another stack is refused.
#[test]
fn every_stack_saves_and_restores_its_tables() {
    use sdrkit_stack_cell::{CELL, EBICAB, LBJ, NBIOT, NR};
    let mut saved = Vec::new();
    for (desc, seconds, comp) in [
        (&CELL, 0.45, "search"),
        (&NR, 0.09, "search"),
        (&NBIOT, 1.05, "search"),
        (&LBJ, 3.0, "messages"),
        (&EBICAB, 0.1, "telegrams"),
    ] {
        let x = Harness::new(desc)
            .seconds(seconds)
            .generate()
            .expect("the test signal");
        let f = start_hz(desc);
        let input =
            InputInfo::new(desc.tuning.rate_hz, f + desc.tuning.tune_offset_hz, f).recording();
        let mut rx = stack_graph(desc, &[]);
        let s = run_graph(&mut rx, input, &x, &[]);
        let before = without_times(data(&s, comp).clone());
        assert_ne!(before, J::Null, "{}", desc.id);
        let text = rx.save_state().expect("a saved state");
        let mut rx2 = stack_graph(desc, &[]);
        rx2.configure(&input).unwrap();
        rx2.restore_state(&text).unwrap();
        let after = without_times(data(&state(&rx2), comp).clone());
        if let Some(d) = first_difference(&before, &after, desc.id) {
            panic!("restored state differs: {d}");
        }
        saved.push(text);
    }
    // The state of one stack is refused by the next.
    let mut rx = stack_graph(&NR, &[]);
    rx.configure(&InputInfo::new(7.68e6, 3541.44e6, 3541.44e6).recording())
        .unwrap();
    assert!(rx.restore_state(&saved[0]).is_err());
}

/// `j` without wall-clock times and signal times: a restored state moves every signal time onto the new clock.
fn without_times(j: J) -> J {
    fn walk(j: &mut J) {
        match j {
            J::Object(m) => {
                for k in ["first_s", "last_s", "t", "last_burst_s", "last_telegram_s"] {
                    m.remove(k);
                }
                m.values_mut().for_each(walk);
            }
            J::Array(a) => a.iter_mut().for_each(walk),
            _ => {}
        }
    }
    let mut j = without_wall(j);
    walk(&mut j);
    j
}
