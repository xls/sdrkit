//! The DVB-S2 sender of `dvbs` (`tx_standard` DVB-S2): from its lock on, the stack's own receiver returns the transport
//! stream bit for bit (PCR fields aside), null packets included, for every constellation, both frame sizes, with and
//! without pilots, at three symbol rates; the useful bit rate, the PLHEADER, the spectrum of each roll-off, the level and
//! the peaks, the end of a transmission, the refusals and the processor share. Nothing goes to a radio.

use crate::common::*;
use crate::dvbs_transmit::{call, host_stream, masked, pid, refusal, run, send_from, tx_fields, Host, DIAL};
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Choice, Clock, CommandError, Complex32, InputInfo, Receiver};
use sdrkit_graph::{RunContext, StackGraph};
use sdrkit_proto_dvbs2::modcod::ModCod;
use sdrkit_proto_dvbs2::settings::{
    FecFrame, GenRollOff, GenStandardChoice, TxModcod, PL_SCRAMBLING, TX_FRAME, TX_INVERSION,
    TX_MODCOD, TX_PILOTS, TX_PL_SCRAMBLING, TX_ROLLOFF, TX_STANDARD, TX_SYMBOL_RATE,
};
use sdrkit_proto_dvbs2::sync::Received;
use sdrkit_proto_dvbs2::typed::Dvbs2Fields as D;
use sdrkit_proto_dvbs2::FrameSize;
use sdrkit_proto_mpegts::packet::{NULL_PID, TS_SIZE};
use sdrkit_proto_mpegts::sendqueue::{QueueError, SendQueue, QUEUE_PACKETS};
use sdrkit_stack_dvbcs::dvbs::{self, DVBS};
use sdrkit_stack_dvbcs::dvbs2_transmit::{rms, S2Chain, S2Config, BLOCK};
use sdrkit_stack_dvbcs::dvbs_transmit::{TxError, RISE_S};
use std::sync::Arc;

/// The settings of MODCOD `id` at `symbol_rate`, the rest as the defaults: normal frames, no pilots, roll-off 0.35.
fn cfg(id: u8, symbol_rate: f64) -> S2Config {
    S2Config {
        symbol_rate,
        modcod: ModCod::new(id).unwrap(),
        ..S2Config::default()
    }
}

fn short(c: S2Config) -> S2Config {
    S2Config {
        frame: FecFrame::Short,
        ..c
    }
}

fn pilots(c: S2Config) -> S2Config {
    S2Config { pilots: true, ..c }
}

/// The `dvbs` stack with the DVB-S2 sender settings of `c` at `rate`, the transmitter on, and its host.
fn station(c: &S2Config, rate: f64) -> (StackGraph, Arc<Host>) {
    let cfg = config(
        &DVBS,
        &[
            CaseSetting::new(&TX_STANDARD, GenStandardChoice::Dvbs2),
            CaseSetting::new(&TX_SYMBOL_RATE, c.symbol_rate),
            CaseSetting::new(&TX_MODCOD, TxModcod::from_index(u16::from(c.modcod.id) - 1).unwrap()),
            CaseSetting::new(&TX_FRAME, c.frame),
            CaseSetting::new(&TX_PILOTS, c.pilots),
            CaseSetting::new(&TX_ROLLOFF, c.roll_off),
            CaseSetting::new(&TX_PL_SCRAMBLING, i64::from(c.pl_scrambling)),
            CaseSetting::new(&TX_INVERSION, c.inversion),
        ],
    );
    let mut st = dvbs::receiver(&cfg).expect("the stack takes the settings");
    st.configure(&InputInfo::new(rate, DIAL, DIAL).recording()).unwrap();
    let host = Arc::new(Host::default());
    let mut ctx: RunContext = st.graph().context().clone();
    ctx.streams = Some(host.clone());
    ctx.clock = Clock::fixed(1_800_000_000.0);
    st.set_context(ctx);
    assert!(st.transmits());
    st.set_transmit(true);
    (st, host)
}

/// The symbols of a PLFRAME of `c`, and the symbols that one transport packet takes.
fn frame_and_packet_symbols(c: &S2Config) -> (f64, f64) {
    let frame = c.modcod.frame_symbols(c.short(), c.pilots) as f64;
    let data_bits = (c.modcod.code(c.short()).unwrap().kbch - 80) as f64;
    (frame, frame * 1504.0 / data_bits)
}

/// The symbols that the receiver may take to lock at `rate` samples per second on a carrier of `c`: one survey of its
/// front end (48 spectra of 4096 samples or 4 ms each), 10 000 symbols for the symbol clock, and [`LOCK_FRAMES`] frames
/// for the frame search and the first headers.
fn lock_bound(c: &S2Config, rate: f64) -> f64 {
    let survey = 48.0 * (0.004 * rate).max(4096.0);
    survey * c.symbol_rate / rate + 10_000.0 + LOCK_FRAMES * frame_and_packet_symbols(c).0
}

/// The PLFRAMEs after the survey in which the receiver locks.
const LOCK_FRAMES: f64 = 4.0;

/// A transmission of the sender: its settings, the sample rate, the packets of the host and the samples.
struct Sent {
    c: S2Config,
    rate: f64,
    /// The packets that the receiver must return, at least.
    n: usize,
    hp: Vec<[u8; TS_SIZE]>,
    x: Vec<Complex32>,
    what: String,
}

/// The transmission of `c` at `rate` with `n` packets of the host after those of the lock bound. The sender counts its
/// packets, keeps the bit rate of the mode and stays under full scale.
fn send(c: &S2Config, rate: f64, n: usize) -> Sent {
    let what = format!(
        "{} {:?} pilots {} inverted {} roll-off {} at {} sym/s, {rate} samples/s",
        c.modcod.name(),
        c.frame,
        c.pilots,
        c.inversion,
        c.roll_off.value(),
        c.symbol_rate
    );
    let (_, packet_symbols) = frame_and_packet_symbols(c);
    let warm = (lock_bound(c, rate) / packet_symbols).ceil() as usize;
    let hp = host_stream(c.bit_rate(), warm + n);
    let (x, ends, f) = send_from(station(c, rate), rate, &hp, 0.05, 0.2);
    assert_eq!(ends, 1, "{what}: one end mark");
    assert_eq!(f.tx_packets as usize, hp.len(), "{what}");
    assert!(f.tx_nulls > 0, "{what}: {f:?}");
    assert!((f.tx_bitrate_bps - c.bit_rate()).abs() < 1e-6, "{what}: {f:?}");
    if c.bit_rate() > 300e3 {
        assert!(f.tx_pcr_restamped > 0, "{what}: {f:?}");
    }
    let peak = x.iter().map(|z| z.norm()).fold(0f32, f32::max);
    assert!(peak < 0.93, "{what}: peak {peak}");
    Sent {
        c: *c,
        rate,
        n,
        hp,
        x,
        what,
    }
}

/// The carrier phase (radians) and the carrier offset (as a share of the symbol rate, up to 0.3 % each way) of `seed`.
fn phase_and_offset(seed: u64) -> (f64, f64) {
    let mut v = (seed + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut unit = || {
        v ^= v << 13;
        v ^= v >> 7;
        v ^= v << 17;
        (v >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    };
    (std::f64::consts::PI * unit(), 0.003 * unit())
}

/// The loop of `sent` with the noise, the carrier phase and the carrier offset of `seed`, at 30 dB in the band of the
/// carrier: the receiver reports the mode, and from its lock to the end of the transmission it returns every packet
/// bit for bit (PCR fields aside), the null packets too, none in error; the fall after `stop` cuts no packet. Returns
/// the symbols before the lock.
fn receive(sent: &Sent, seed: u64) -> f64 {
    let Sent { c, rate, n, hp, x, what } = sent;
    let (rate, n) = (*rate, *n);
    let what = format!("{what}, seed {seed}");
    let (_, packet_symbols) = frame_and_packet_symbols(c);
    let bound = lock_bound(c, rate);
    let (phase, offset) = phase_and_offset(seed);
    let turn = 2.0 * std::f64::consts::PI * offset * c.symbol_rate / rate;
    let mut noisy: Vec<Complex32> = x
        .iter()
        .enumerate()
        .map(|(k, z)| {
            let (s, co) = (phase + turn * k as f64).sin_cos();
            z * Complex32::new(co as f32, s as f32)
        })
        .collect();
    let p = x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / x.len() as f64;
    let band = c.symbol_rate * (1.0 + c.roll_off.value());
    sdrkit_dev_test::Awgn::new(p, 30.0, band, rate, seed).process(&mut noisy);
    let settings = [CaseSetting::new(&PL_SCRAMBLING, i64::from(c.pl_scrambling))];
    let mut rx = Rx::new(&DVBS, &settings, &recording(rate, DIAL));
    rx.feed(&noisy, 16384);
    rx.finish();
    assert_eq!(rx.at(&D::STANDARD), Some(Received::Dvbs2), "{what}");
    assert_eq!(rx.at(&D::MODCOD), Some(i64::from(c.modcod.id)), "{what}");
    let frame = if c.short() { FrameSize::Short } else { FrameSize::Normal };
    assert_eq!(rx.at(&D::FRAME), Some(frame), "{what}");
    assert_eq!(rx.at(&D::PILOTS), Some(c.pilots), "{what}");
    assert_eq!(rx.at(&D::INVERTED), Some(c.inversion), "{what}");
    assert_eq!(rx.at(&D::SINGLE_STREAM), Some(true), "{what}");
    assert_eq!(rx.at(&D::BCH_BAD), Some(0), "{what}");
    assert!((rx.num("roll_off") - c.roll_off.value()).abs() < 1e-6, "{what}: roll-off {}", rx.num("roll_off"));

    let all = &rx.packets;
    assert!(all.iter().all(|p| p[1] & 0x80 == 0), "{what}: a packet in error after the lock");
    let sent = x.len() as f64 / rate * c.symbol_rate;
    let lock = sent - all.len() as f64 * packet_symbols;
    eprintln!("{what}: lock in {lock:.0} symbols of {bound:.0}");
    assert!(lock <= bound, "{what}: lock in {lock:.0} symbols, bound {bound:.0}");
    let null = sdrkit_proto_mpegts::source::null_packet();
    let (nulls, got): (Vec<[u8; TS_SIZE]>, Vec<[u8; TS_SIZE]>) =
        all.iter().partition(|p| pid(&p[..]) == NULL_PID);
    assert!(nulls.iter().all(|p| *p == null), "{what}: a null packet differs");
    let got: Vec<_> = got.iter().map(masked).collect();
    let want: Vec<_> = hp.iter().map(masked).collect();
    assert!(got.len() <= want.len(), "{what}: {} packets returned of {}", got.len(), want.len());
    assert!(got[..] == want[want.len() - got.len()..], "{what}: the packets differ");
    assert!(got.len() >= n, "{what}: {} packets returned of {}", got.len(), want.len());
    lock
}

/// [`send`] and [`receive`] with one seed.
fn loop_test(c: &S2Config, rate: f64, n: usize) -> f64 {
    receive(&send(c, rate, n), 5)
}

/// The seeds of the loops that run over seeds.
const SEEDS: u64 = 10;

/// [`send`], then [`receive`] with each of [`SEEDS`] seeds: all lock.
fn loop_seeds(c: &S2Config, rate: f64, n: usize) {
    let sent = send(c, rate, n);
    for seed in 0..SEEDS {
        receive(&sent, seed);
    }
}

fn inverted(c: S2Config) -> S2Config {
    S2Config { inversion: true, ..c }
}

fn roll_off(c: S2Config, roll_off: GenRollOff) -> S2Config {
    S2Config { roll_off, ..c }
}

#[test]
fn loop_qpsk() {
    loop_test(&short(cfg(4, 125e3)), 2e6, 100);
    loop_test(&pilots(cfg(7, 1e6)), 2e6, 100);
    loop_test(&cfg(11, 2e6), 4e6, 100);
    // Roll-off 0.20, with a PL scrambling code that the receiver has set.
    let c = S2Config {
        pl_scrambling: 131_070,
        ..roll_off(pilots(short(cfg(6, 1e6))), GenRollOff::R20)
    };
    loop_test(&c, 2e6, 100);
}

#[test]
fn loop_8psk() {
    loop_test(&pilots(short(cfg(12, 125e3))), 2e6, 100);
    loop_test(&roll_off(cfg(14, 1e6), GenRollOff::R25), 2e6, 100);
    loop_test(&short(cfg(13, 1e6)), 2e6, 100);
    loop_test(&pilots(cfg(16, 2e6)), 4e6, 100);
}

/// Normal frames without pilots, over seeds: the carrier comes from the headers and the data alone.
#[test]
fn loop_qpsk_and_8psk_normal_frames_without_pilots() {
    loop_seeds(&cfg(7, 1e6), 2e6, 100);
    loop_seeds(&cfg(13, 1e6), 2e6, 100);
}

#[test]
fn loop_16apsk() {
    loop_seeds(&pilots(cfg(18, 125e3)), 2e6, 100);
    loop_seeds(&short(cfg(22, 125e3)), 2e6, 100);
    loop_seeds(&pilots(short(cfg(19, 1e6))), 2e6, 100);
    loop_seeds(&short(cfg(21, 1e6)), 2e6, 100);
    loop_seeds(&pilots(cfg(23, 2e6)), 4e6, 100);
}

#[test]
fn loop_16apsk_normal_frames_without_pilots() {
    loop_seeds(&cfg(18, 1e6), 2e6, 100);
    loop_seeds(&roll_off(cfg(23, 2e6), GenRollOff::R20), 4e6, 100);
}

#[test]
fn loop_32apsk() {
    loop_seeds(&pilots(short(cfg(24, 125e3))), 2e6, 100);
    loop_seeds(&pilots(cfg(27, 125e3)), 2e6, 100);
    loop_seeds(&short(cfg(25, 1e6)), 2e6, 100);
    loop_seeds(&pilots(cfg(28, 1e6)), 2e6, 100);
    loop_seeds(&short(cfg(26, 2e6)), 4e6, 100);
}

#[test]
fn loop_32apsk_normal_frames_without_pilots() {
    loop_seeds(&cfg(24, 1e6), 2e6, 100);
    loop_seeds(&roll_off(cfg(28, 2e6), GenRollOff::R25), 4e6, 100);
}

/// An inverted spectrum, over seeds: the receiver reports the sense and returns the packets, for every constellation,
/// with and without pilots.
#[test]
fn loop_inverted() {
    loop_seeds(&inverted(cfg(6, 1e6)), 2e6, 100);
    loop_seeds(&inverted(pilots(short(cfg(14, 1e6)))), 2e6, 100);
    loop_seeds(&inverted(cfg(19, 1e6)), 2e6, 100);
    loop_seeds(&inverted(pilots(cfg(25, 2e6))), 4e6, 100);
}

/// Spectral inversion negates Q of every sample.
#[test]
fn inversion_mirrors_the_signal() {
    let c = pilots(short(cfg(19, 1e6)));
    let inverted = S2Config { inversion: true, ..c };
    let (mut a, mut b) = (S2Chain::new(&c, 2e6).unwrap(), S2Chain::new(&inverted, 2e6).unwrap());
    let (mut qa, mut qb) = (SendQueue::new(c.bit_rate()), SendQueue::new(c.bit_rate()));
    let (mut xa, mut xb) = (Vec::new(), Vec::new());
    for _ in 0..40 {
        a.block(&mut qa, &mut xa);
        b.block(&mut qb, &mut xb);
        assert!(!xa.is_empty() && xa.len() == xb.len());
        assert!(xa.iter().zip(&xb).all(|(p, q)| (p.conj() - q).norm() < 1e-6));
    }
}

#[test]
fn the_useful_bit_rate_follows_the_standard() {
    // The data field of a frame, K_bch − 80 bits, over the symbols of the PLFRAME.
    // QPSK 2/3 normal at 1 Msym/s: 43 040 − 80 = 42 960 bits in 32 400 + 90 symbols.
    let a = cfg(6, 1e6);
    assert!((a.bit_rate() - 42_960.0 / 32_490.0 * 1e6).abs() < 1e-6);
    assert!((a.bit_rate() - 1.3222e6).abs() < 100.0, "{}", a.bit_rate());
    // With pilots: 22 blocks of 36 symbols more, 33 282 symbols.
    assert!((pilots(a).bit_rate() - 1.2908e6).abs() < 100.0);
    // 16APSK 3/4 short with pilots at 2 Msym/s: 11 712 − 80 bits in 45 slots, the header and 2 pilot blocks,
    // 4050 + 90 + 72 = 4212 symbols.
    let b = pilots(short(cfg(19, 2e6)));
    assert!((b.bit_rate() - 2e6 * 11_632.0 / 4212.0).abs() < 1e-6, "{}", b.bit_rate());
    // 8PSK 9/10 normal at 125 ksym/s: 58 192 − 80 bits in 21 600 + 90 symbols, 334.9 kbit/s.
    let c = cfg(17, 125e3);
    assert!((c.bit_rate() - 125e3 * 58_112.0 / 21_690.0).abs() < 1e-6);
    assert!((c.bit_rate() - 334_901.0).abs() < 1.0, "{}", c.bit_rate());
    // The stack states the rate of its settings before it sends, and the queue time at that rate.
    let (mut st, _host) = station(&b, 4e6);
    let f = tx_fields(&st);
    assert!(!f.transmitting && f.tx_freq_hz.is_none());
    assert!((f.tx_bitrate_bps - b.bit_rate()).abs() < 1e-6, "{f:?}");
    let ten: Vec<u8> = host_stream(1e6, 10).into_iter().flatten().collect();
    call(&mut st, "append", Some(&ten)).unwrap();
    let f = tx_fields(&st);
    assert!((f.tx_queue_s.0 - 10.0 * 1504.0 / b.bit_rate()).abs() < 1e-9, "{f:?}");
}

/// The 64 bits of the PLS code of the 7 signalling bits `b` (first bit first), from the standard: the (32, 6) code of
/// the first six bits by its generator matrix, each bit of it followed by itself or, with the seventh bit set, by its
/// complement, then the scrambling sequence (EN 302 307-1 clause 5.5.2.4).
fn pls_of_the_standard(b: [u8; 7]) -> [u8; 64] {
    const G: [u32; 6] = [
        0b0101_0101_0101_0101_0101_0101_0101_0101,
        0b0011_0011_0011_0011_0011_0011_0011_0011,
        0b0000_1111_0000_1111_0000_1111_0000_1111,
        0b0000_0000_1111_1111_0000_0000_1111_1111,
        0b0000_0000_0000_0000_1111_1111_1111_1111,
        0b1111_1111_1111_1111_1111_1111_1111_1111,
    ];
    const SCRAMBLE: u64 = 0b0111_0001_1001_1101_1000_0011_1100_1001_0101_0011_0100_0010_0010_1101_1111_1010;
    let mut out = [0u8; 64];
    for i in 0..32 {
        let y = (0..6).fold(0u8, |a, k| a ^ (b[k] & ((G[k] >> (31 - i)) & 1) as u8));
        out[2 * i] = y;
        out[2 * i + 1] = y ^ b[6];
    }
    for (i, o) in out.iter_mut().enumerate() {
        *o ^= ((SCRAMBLE >> (63 - i)) & 1) as u8;
    }
    out
}

/// The 26 bits of the start of frame, 18D2E82 hexadecimal, first bit first.
fn sof_of_the_standard() -> Vec<u8> {
    (0..26).map(|i| ((0x18D_2E82u32 >> (25 - i)) & 1) as u8).collect()
}

/// The bits of the 90 π/2-BPSK symbols from symbol `first` of the samples `x` (4 samples per symbol, roll-off `ro`),
/// symbol `k` centred `delay` samples after sample `4 k`: a matched filter at each symbol, then the sign on the axis of
/// the symbol (odd symbols a quarter turn on from even ones; 0 is the point (1 + j)/√2).
fn header_bits(x: &[Complex32], first: usize, delay: usize, ro: f64) -> Vec<u8> {
    (0..90)
        .map(|i| {
            let centre = 4 * (first + i) + delay;
            let mut z = (0f64, 0f64);
            for m in -32i64..=32 {
                let h = sdrkit_algo_dsp::pulse::rrc(m as f64 / 4.0, ro);
                let s = x[(centre as i64 + m) as usize];
                z = (z.0 + h * f64::from(s.re), z.1 + h * f64::from(s.im));
            }
            let on_axis = if i % 2 == 0 { z.0 + z.1 } else { z.1 - z.0 };
            u8::from(on_axis < 0.0)
        })
        .collect()
}

#[test]
fn the_plheader_carries_the_sof_and_the_pls_code_of_the_standard() {
    // MODCOD 6 (QPSK 2/3), normal frames, no pilots: 00110 0 0 (worked example 2 of the specification).
    // MODCOD 19 (16APSK 3/4), short frames, pilots: 10011 1 1.
    for (c, bits) in [
        (cfg(6, 500e3), [0, 0, 1, 1, 0, 0, 0]),
        (pilots(short(cfg(19, 500e3))), [1, 0, 0, 1, 1, 1, 1]),
    ] {
        let rate = 2e6;
        let frame = c.modcod.frame_symbols(c.short(), c.pilots);
        let (x, _, _) = send_from(station(&c, rate), rate, &[], 3.2 * frame as f64 / c.symbol_rate, 0.0);
        let sof = sof_of_the_standard();
        // The delay of the pulse shaper: the one at which the second frame starts with the start of frame.
        let delay = (0..=200)
            .find(|&d| header_bits(&x, frame, d, 0.35)[..26] == sof[..])
            .expect("the start of frame at the start of the second PLFRAME");
        for k in 1..3 {
            let h = header_bits(&x, k * frame, delay, 0.35);
            assert_eq!(h[..26], sof[..], "frame {k}");
            assert_eq!(h[26..], pls_of_the_standard(bits)[..], "frame {k}: {}", c.modcod.name());
        }
    }
}

/// The steady part of a transmission of null packets: after the rise and before the fall.
fn steady(c: &S2Config, rate: f64, seconds: f64) -> Vec<Complex32> {
    let (x, _, _) = send_from(station(c, rate), rate, &[], seconds, 0.0);
    x[(0.02 * rate) as usize..((seconds - 0.02) * rate) as usize].to_vec()
}

#[test]
fn the_spectrum_has_the_shape_of_each_roll_off() {
    let (rs, rate) = (1e6, 8e6);
    for ro in [GenRollOff::R35, GenRollOff::R25, GenRollOff::R20] {
        let c = S2Config {
            roll_off: ro,
            ..cfg(13, rs)
        };
        let x = steady(&c, rate, 0.5);
        let n = 4096;
        let mut psd = vec![0f32; n];
        welch(&x, n, Window::BlackmanHarris4, 0.5, Scaling::Power, &mut psd).unwrap();
        let bin = |f: f64| ((f / rate * n as f64).round() as isize + n as isize / 2) as usize;
        let band = |a: f64, b: f64| {
            let v: Vec<f64> = (bin(a)..=bin(b)).map(|i| f64::from(psd[i])).collect();
            v.iter().sum::<f64>() / v.len() as f64
        };
        let centre = band(-0.25 * rs, 0.25 * rs);
        let db = |p: f64| 10.0 * (p.max(1e-30) / centre).log10();
        let both = |a: f64, b: f64| db(band(a, b).max(band(-b, -a)));
        // The raised cosine of the roll-off: 0.7 dB, 3.0 dB and 8.3 dB down at a quarter, a half and three quarters of
        // the roll-off region, which lies from (1 − roll-off) / 2 to (1 + roll-off) / 2 of the symbol rate.
        let a = ro.value();
        for (part, want) in [(0.25, -0.69), (0.5, -3.01), (0.75, -8.34)] {
            let f = rs * ((1.0 - a) / 2.0 + part * a);
            let got = both(f - 0.004 * rs, f + 0.004 * rs);
            assert!((got - want).abs() < 0.6, "roll-off {a} at {f} Hz: {got:.2} dB, want {want}");
        }
        // Beyond the occupied band, and the shoulder at twice the symbol rate (specification 7.8 and 0.6).
        let edge = rs * (1.0 + a) / 2.0;
        let beyond = both(1.1 * edge, 3.0 * rs);
        let shoulder = both(1.95 * rs, 2.05 * rs);
        eprintln!("roll-off {a}: beyond 1.1 × the band edge {beyond:.1} dB, shoulder at 2 Rs {shoulder:.1} dB");
        assert!(beyond < -50.0 && shoulder < -50.0, "roll-off {a}: {beyond} {shoulder}");
    }
}

#[test]
fn level_and_peaks_of_each_constellation() {
    // The peak over the mean power of the steady signal, dB, with roll-off 0.35 and 0.20.
    for (id, ro, low, high) in [
        (7, GenRollOff::R35, 3.4, 4.3),
        (14, GenRollOff::R35, 3.4, 4.3),
        (19, GenRollOff::R35, 4.2, 5.1),
        (25, GenRollOff::R35, 5.1, 6.0),
        (19, GenRollOff::R20, 5.6, 6.6),
        (25, GenRollOff::R20, 6.4, 7.4),
    ] {
        let c = S2Config {
            roll_off: ro,
            ..cfg(id, 1e6)
        };
        let x = steady(&c, 4e6, 0.6);
        let mean = (x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / x.len() as f64).sqrt();
        let peak = x.iter().map(|z| z.norm()).fold(0f32, f32::max);
        let papr = 20.0 * (f64::from(peak) / mean).log10();
        eprintln!("{} roll-off {}: rms {mean:.4}, peak {peak:.3}, peak to mean {papr:.2} dB", c.modcod.name(), ro.value());
        assert!((mean - f64::from(rms(c.modcod.modulation, ro))).abs() < 0.01, "{mean}");
        assert!(peak < 0.9, "{}: peak {peak} reaches the limiter", c.modcod.name());
        assert!((low..high).contains(&papr), "{}: {papr}", c.modcod.name());
    }
}

#[test]
fn stop_ends_with_a_fall_after_the_plframe_on_air() {
    let c = short(cfg(6, 1e6));
    let rate = 2e6;
    let (mut st, host) = station(&c, rate);
    call(&mut st, "start", None).unwrap();
    run(&mut st, rate, 0.2, |_| {});
    call(&mut st, "stop", None).unwrap();
    assert!(matches!(
        refusal(&call(&mut st, "start", None).unwrap_err()),
        Some(TxError::Ending)
    ));
    run(&mut st, rate, 0.2, |_| {});
    assert!(!tx_fields(&st).transmitting);
    let (x, ends) = host.sent();
    assert_eq!(ends, 1);
    // The fall is a raised cosine of 5 ms to zero.
    let fall = (RISE_S * rate) as usize;
    let tail = &x[x.len() - fall..];
    let level = |s: &[Complex32]| (s.iter().map(|z| z.norm_sqr()).sum::<f32>() / s.len() as f32).sqrt();
    let before = level(&x[x.len() - 2 * fall..x.len() - fall]);
    assert!((before - c.rms()).abs() < 0.05, "{before}");
    assert!(level(&tail[..fall / 8]) > 0.8 * c.rms(), "the fall starts high");
    assert!(level(&tail[fall - fall / 16..]) < 0.02, "the fall ends at zero");
    // The fall starts where a PLFRAME ends: the symbols before it are whole frames, less the delay of the pulse
    // shaper (12 symbols) and the samples of one shaping step.
    let sps = rate / c.symbol_rate;
    let frame = c.modcod.frame_symbols(true, false) as f64;
    let frames = ((x.len() - fall) as f64 / sps + 12.0) / frame;
    eprintln!("the fall starts after {frames:.4} PLFRAMEs");
    assert!((frames - frames.round()).abs() * frame <= 13.0, "{frames}");
    // It starts at the first frame end after the stop.
    let most = 0.2 * rate + 0.05 * rate + frame * sps + fall as f64;
    assert!((x.len() as f64) < most, "{} samples, at most {most}", x.len());
    // After the end the stack starts again.
    call(&mut st, "start", None).expect("a new transmission");
}

#[test]
fn start_needs_the_transmitter_a_frame_of_the_mode_and_a_rate_for_the_symbol_rate() {
    let (mut st, _host) = station(&cfg(6, 1e6), 2e6);
    st.set_transmit(false);
    assert!(matches!(
        call(&mut st, "start", None),
        Err(CommandError::TransmitOff)
    ));
    // Code rate 9/10 has no short frame.
    let (mut st, _host) = station(&short(cfg(11, 1e6)), 2e6);
    assert_eq!(tx_fields(&st).tx_bitrate_bps, 0.0);
    assert_eq!(
        refusal(&call(&mut st, "start", None).unwrap_err()),
        Some(TxError::NoShortFrame)
    );
    // 1.25 Msym/s at 2 Msps: roll-off 0.35 needs 2.19 Msps, roll-off 0.20 needs 1.95 Msps.
    let (mut st, _host) = station(&cfg(6, 1.25e6), 2e6);
    assert!(matches!(
        refusal(&call(&mut st, "start", None).unwrap_err()),
        Some(TxError::RateTooLow { .. })
    ));
    let c = S2Config {
        roll_off: GenRollOff::R20,
        ..cfg(6, 1.25e6)
    };
    let (mut st, _host) = station(&c, 2e6);
    call(&mut st, "start", None).expect("the rate is enough for roll-off 0.20");
    assert!(tx_fields(&st).transmitting);
}

#[test]
fn append_takes_whole_packets_with_a_sync_byte_while_the_queue_has_room() {
    let (mut st, _host) = station(&cfg(6, 1e6), 2e6);
    let full: Vec<u8> = host_stream(1e6, QUEUE_PACKETS).into_iter().flatten().collect();
    let e = call(&mut st, "append", Some(&full[..100])).unwrap_err();
    assert_eq!(
        refusal(&e),
        Some(TxError::Queue(QueueError::NotWhole { bytes: 100 }))
    );
    let mut bad = full[..2 * TS_SIZE].to_vec();
    bad[TS_SIZE] = 0x48;
    let e = call(&mut st, "append", Some(&bad)).unwrap_err();
    assert_eq!(
        refusal(&e),
        Some(TxError::Queue(QueueError::NoSync { packet: 1 }))
    );
    assert_eq!(tx_fields(&st).tx_queue, 0, "all or none");
    call(&mut st, "append", Some(&full)).expect("the queue takes 6000 packets");
    let e = call(&mut st, "append", Some(&full[..TS_SIZE])).unwrap_err();
    assert_eq!(
        refusal(&e),
        Some(TxError::Queue(QueueError::Full {
            packets: 1,
            room: 0
        }))
    );
}

#[test]
fn the_sender_runs_faster_than_real_time() {
    for b in sdrkit_stack_dvbcs::benchmarks()
        .iter()
        .filter(|b| b.name.contains("dvbs2_sender"))
    {
        let r = (b.run)(0.3);
        eprintln!("{}: {:.1} % of one core", b.name, 100.0 * r.cpu_fraction_of_realtime);
        assert!(r.cpu_fraction_of_realtime > 0.0 && r.cpu_fraction_of_realtime < 1.0, "{r:?}");
    }
    // The longest step: one that codes a normal frame (BCH and LDPC of 64 800 bits) and shapes its first block.
    for c in [sdrkit_stack_dvbcs::dvbs2_transmit::bench_qpsk(), sdrkit_stack_dvbcs::dvbs2_transmit::bench_16apsk()] {
        let rate = 2e6;
        let mut chain = S2Chain::new(&c, rate).unwrap();
        let mut queue = SendQueue::new(c.bit_rate());
        let mut out = Vec::new();
        let mut times = Vec::new();
        for _ in 0..2000 {
            let t = std::time::Instant::now();
            chain.block(&mut queue, &mut out);
            times.push(t.elapsed().as_secs_f64());
        }
        // The first step codes a frame; so does one step in 64 (QPSK) or in 32 (16APSK).
        let first = times[0];
        times.sort_by(f64::total_cmp);
        eprintln!(
            "{}: a step that codes a frame takes {:.3} ms, the median step {:.3} ms, the slowest {:.3} ms, each for {:.3} ms of signal",
            c.modcod.name(),
            1e3 * first,
            1e3 * times[times.len() / 2],
            1e3 * times[times.len() - 1],
            1e3 * BLOCK as f64 / c.symbol_rate
        );
        assert!(chain.frames() > 20);
    }
}
