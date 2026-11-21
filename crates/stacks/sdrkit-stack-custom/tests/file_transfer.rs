//! `file_transfer` through its transmitter, a test channel and its receiver, and its session protocol over a simulated link
//! (Part G section 8). Es/N0 is set at 4 samples a symbol with unit-power samples; the noise is added at 4 samples a symbol,
//! the signal is then converted to 3.2 Msps, and the carrier offset is applied there.

// Test channels take many plain parameters; profiles are walked by index.
#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

mod common;

use common::Noise;
use sdrkit_core::Complex32;
use sdrkit_stack_custom::file_transfer::rx::{Receiver, RxFrame};
use sdrkit_stack_custom::file_transfer::session::{
    control_frame, join_message, safe_name, CarouselItem, Collector, DataBlock, Message, RxBurst, RxData,
    RxEvent, Sender, SenderConfig, SenderOut, SenderState, SessionReceiver,
};
use sdrkit_stack_custom::file_transfer::{
    frame, max_blocks, shape, FrameHeader, FrameKind, TxFrame, Width, BASE_RATE, PROFILES, TX_LEVEL, WIDTHS,
};
use sdrkit_algo_dsp::blackman_rational::BlackmanRational;

/// A uniform value in `[0, 1)` from a seeded xorshift.
struct Uni(u64);

impl Uni {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn random_bytes(n: usize, seed: u64) -> Vec<u8> {
    let mut u = Uni(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    (0..n).map(|_| (u.next() * 256.0) as u8).collect()
}

fn header(width: Width, profile: usize, kind: FrameKind, blocks: usize, frame_no: u16, index: u8, last: bool) -> FrameHeader {
    FrameHeader {
        width,
        profile: profile as u8,
        kind,
        session: 7,
        frame: frame_no,
        blocks: blocks as u8,
        index,
        last,
        callsign: "LY1TST".into(),
    }
}

/// A data frame of `blocks` random payloads.
fn data_frame(width: Width, profile: usize, blocks: usize, frame_no: u16, index: u8, last: bool, seed: u64) -> TxFrame {
    let p = PROFILES[profile].payload();
    TxFrame {
        header: header(width, profile, FrameKind::Data, blocks, frame_no, index, last),
        blocks: (0..blocks).map(|b| random_bytes(p, seed * 1000 + b as u64)).collect(),
    }
}

/// The frames back to back at 4 samples a symbol and unit power.
fn phy(frames: &[TxFrame]) -> Vec<Complex32> {
    let mut s = Vec::new();
    for f in frames {
        s.extend(frame(&f.header, &f.blocks));
    }
    shape(&s)
}

/// `lead` and `tail` zero symbols round `x4` (4 samples a symbol of `width`), noise of Es/N0 `esn0` at that rate, then 3.2
/// Msps and the carrier offset `hz`.
fn channel(x4: &[Complex32], width: Width, lead: usize, tail: usize, esn0: Option<f64>, hz: f64, seed: u64) -> Vec<Complex32> {
    let mut y = vec![Complex32::new(0.0, 0.0); 4 * lead];
    y.extend_from_slice(x4);
    y.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 4 * tail));
    if let Some(db) = esn0 {
        Noise::new(seed).add(&mut y, 4.0 / 10f64.powf(db / 10.0));
    }
    let mut z = if width.factor() == 1 { y } else { BlackmanRational::convert(&y, width.factor(), 1).unwrap() };
    common::shift(&mut z, hz, BASE_RATE);
    z
}

fn receive(y: &[Complex32], rate: f64, widths: &[Width], chunk: usize) -> Vec<RxFrame> {
    let mut rx = Receiver::new(rate, widths);
    let mut out = Vec::new();
    for c in y.chunks(chunk) {
        rx.process(c, &mut out);
    }
    rx.finish(&mut out);
    out
}

fn good(f: &RxFrame, sent: &TxFrame) -> usize {
    f.blocks.iter().zip(&sent.blocks).filter(|(a, b)| a.as_deref() == Some(&b[..])).count()
}

/// Section 8 test 5: every profile at every width, min(2, Nmax) blocks, Es/N0 55 dB, no offset.
#[test]
fn clean_loopback_every_profile_and_width() {
    for w in WIDTHS {
        for p in 0..10 {
            let nb = max_blocks(&PROFILES[p], w).min(2);
            let f = data_frame(w, p, nb, 0x21, 0, true, p as u64);
            let y = channel(&phy(std::slice::from_ref(&f)), w, 300, 300, Some(55.0), 0.0, 1 + p as u64);
            let got = receive(&y, BASE_RATE, &WIDTHS, 65_536);
            assert_eq!(got.len(), 1, "{w:?} P{p}: {} frames", got.len());
            assert_eq!(got[0].width, w);
            assert_eq!(got[0].header, f.header);
            assert_eq!(good(&got[0], &f), nb, "{w:?} P{p}");
        }
    }
}

/// Section 8 test 8: block error rate below 5 % at -200 Hz and +200 Hz.
#[test]
fn block_error_rate_with_carrier_offsets() {
    for (w, p, nb, frames, db) in [(Width::W200K, 0, 1, 40, 5.25), (Width::W500K, 4, 5, 8, 13.0), (Width::W1M, 9, 5, 8, 31.9)] {
        for hz in [-200.0, 200.0] {
            let (mut bad, mut total) = (0, 0);
            for t in 0..frames {
                let f = data_frame(w, p, nb, t as u16, 0, true, 50 + t as u64);
                let y = channel(&phy(std::slice::from_ref(&f)), w, 200 + 37 * (t % 5), 300, Some(db), hz, 900 + t as u64);
                let got = receive(&y, BASE_RATE, &[w], 65_536);
                total += nb;
                bad += nb - got.first().map_or(0, |g| good(g, &f));
            }
            let ber = bad as f64 / total as f64;
            assert!(ber < 0.05, "{w:?} P{p} at {hz} Hz: block error rate {ber}");
        }
    }
}

/// Section 8 test 6: a 500 kHz frame is found only by the 500 kHz receiver.
#[test]
fn width_separation() {
    let f = data_frame(Width::W500K, 1, 2, 3, 0, true, 6);
    let y = channel(&phy(std::slice::from_ref(&f)), Width::W500K, 400, 400, Some(20.0), 0.0, 6);
    assert!(receive(&y, BASE_RATE, &[Width::W1M], 65_536).is_empty());
    assert!(receive(&y, BASE_RATE, &[Width::W200K], 65_536).is_empty());
    let got = receive(&y, BASE_RATE, &[Width::W500K], 65_536);
    assert_eq!(got.len(), 1);
    assert_eq!(good(&got[0], &f), 2);
}

/// Section 8 test 7: width detection at Es/N0 7 dB, P0, one block, three noise realisations per width.
#[test]
fn width_detection_at_7_db() {
    for w in WIDTHS {
        for seed in 0..3u64 {
            let f = data_frame(w, 0, 1, 1, 0, true, 70 + seed);
            let y = channel(&phy(std::slice::from_ref(&f)), w, 500, 500, Some(7.0), 0.0, 700 + seed);
            let got = receive(&y, BASE_RATE, &WIDTHS, 65_536);
            assert_eq!(got.len(), 1, "{w:?} realisation {seed}");
            assert_eq!(got[0].width, w);
        }
    }
}

/// Section 8 test 9: a burst of 4 P3 frames at 1 MHz (3 blocks each), Es/N0 30 dB, +150 Hz.
#[test]
fn burst_of_four_frames() {
    let frames: Vec<TxFrame> = (0..4).map(|i| data_frame(Width::W1M, 3, 3, (9 << 4) | i, i as u8, i == 3, 90 + u64::from(i))).collect();
    let y = channel(&phy(&frames), Width::W1M, 700, 700, Some(30.0), 150.0, 9);
    let got = receive(&y, BASE_RATE, &WIDTHS, 65_536);
    assert_eq!(got.len(), 4);
    for (g, f) in got.iter().zip(&frames) {
        assert_eq!(g.header, f.header);
        assert_eq!(good(g, f), 3);
        assert!((g.offset_hz - 150.0).abs() < 30.0, "{}", g.offset_hz);
    }
}

/// Section 8 test 10: six P2 frames at 200 kHz (1 block each), Es/N0 25 dB, fed in pieces of 50 000 samples.
#[test]
fn streaming_in_pieces() {
    let mut x = Vec::new();
    let mut sent = Vec::new();
    for i in 0..6u16 {
        let f = data_frame(Width::W200K, 2, 1, i << 4, 0, true, 100 + u64::from(i));
        x.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 4 * (300 + 50 * usize::from(i))));
        x.extend(phy(std::slice::from_ref(&f)));
        sent.push(f);
    }
    let y = channel(&x, Width::W200K, 200, 400, Some(25.0), 0.0, 10);
    let got = receive(&y, BASE_RATE, &WIDTHS, 50_000);
    assert_eq!(got.len(), 6);
    for (g, f) in got.iter().zip(&sent) {
        assert_eq!(good(g, f), 1);
    }
}

/// Section 8 test 11: an input rate of 9 142 857 sps (a 1 MHz P1 frame of 3 blocks at 20 dB, cubic interpolation from 3.2
/// Msps): the receiver is built in under 2 s and decodes the 3 blocks.
#[test]
fn odd_input_rate() {
    let rate = 9_142_857.0;
    let f = data_frame(Width::W1M, 1, 3, 0x30, 0, true, 11);
    let y = channel(&phy(std::slice::from_ref(&f)), Width::W1M, 600, 600, Some(20.0), 0.0, 11);
    let rd = sdrkit_algo_dsp::interp::Reader::new(sdrkit_algo_dsp::interp::Cubic);
    let step = BASE_RATE / rate;
    let n = ((y.len() - 4) as f64 / step) as usize;
    let z: Vec<Complex32> = (0..n).map(|i| rd.at(&y, i as f64 * step)).collect();
    let t = std::time::Instant::now();
    let mut rx = Receiver::new(rate, &WIDTHS);
    assert!(t.elapsed().as_secs_f64() < 2.0, "built in {:?}", t.elapsed());
    let mut got = Vec::new();
    for c in z.chunks(65_536) {
        rx.process(c, &mut got);
    }
    rx.finish(&mut got);
    assert_eq!(got.len(), 1);
    assert_eq!(good(&got[0], &f), 3);
}

/// The receiver of a host: frames, bursts and the receiver side of the session, with the files it delivers.
struct Rig {
    rx: Receiver,
    rate: f64,
    n: u64,
    col: Collector,
    sess: SessionReceiver,
    files: Vec<(String, Vec<u8>)>,
    events: Vec<RxEvent>,
    replies: Vec<Message>,
    frames: usize,
    bad: usize,
}

impl Rig {
    fn new(rate: f64) -> Rig {
        Rig {
            rx: Receiver::new(rate, &WIDTHS),
            rate,
            n: 0,
            col: Collector::default(),
            sess: SessionReceiver::default(),
            files: Vec::new(),
            events: Vec::new(),
            replies: Vec::new(),
            frames: 0,
            bad: 0,
        }
    }

    fn bursts(&mut self, b: Vec<RxBurst>) {
        for x in b {
            self.sess.on_burst(&x, &mut self.replies);
        }
    }

    fn take(&mut self, frames: Vec<RxFrame>) {
        let now = self.n as f64 / self.rate;
        for f in frames {
            self.frames += 1;
            self.bad += f.blocks.iter().filter(|b| b.is_none()).count();
            let h = &f.header;
            let mut done = Vec::new();
            if h.kind == FrameKind::Data {
                let d = RxData { index: h.index, last: h.last, profile: h.profile, blocks: f.blocks.clone(), esn0_db: f.esn0_db };
                self.col.take(h.session, h.frame, d, now, &mut done);
                self.bursts(done);
                continue;
            }
            if let Some(m) = join_message(&f.blocks) {
                if matches!(m, Message::Finish { .. }) {
                    self.col.flush(&mut done);
                    self.bursts(done);
                }
                let mut ev = Vec::new();
                self.sess.on_message(&m, Some(f.esn0_db), &mut self.replies, &mut ev);
                for e in &ev {
                    if let RxEvent::Delivered { name, bytes, .. } = e {
                        self.files.push((name.clone(), bytes.clone()));
                    }
                }
                self.events.extend(ev);
            }
        }
        let mut done = Vec::new();
        self.col.tick(now, &mut done);
        self.bursts(done);
    }

    fn feed(&mut self, x: &[Complex32]) {
        for c in x.chunks(65_536) {
            self.n += c.len() as u64;
            let mut out = Vec::new();
            self.rx.process(c, &mut out);
            self.take(out);
        }
    }

    fn finish(&mut self) {
        let mut out = Vec::new();
        self.rx.finish(&mut out);
        self.take(out);
    }
}

/// The default transmitter's samples for `payload` with Es/N0 `esn0` at 4 samples a symbol, converted to `rate` and moved by
/// `hz`.
fn carousel(payload: &[u8], name: &str, session: u16, profile: usize, width: Width, passes: usize, rate: f64, esn0: Option<f64>, hz: f64, seed: u64) -> Vec<Complex32> {
    use sdrkit_stack_custom::file_transfer::stack::carousel_samples;
    let mut x = carousel_samples(payload, name, "LY1TST", session, profile, width, passes, width.rate());
    for v in &mut x {
        *v /= TX_LEVEL;
    }
    if let Some(db) = esn0 {
        Noise::new(seed).add(&mut x, 4.0 / 10f64.powf(db / 10.0));
    }
    let mut y = sdrkit_stack_custom::file_transfer::to_rate(&x, width, rate);
    common::shift(&mut y, hz, rate);
    y
}

fn deliver(y: &[Complex32], rate: f64) -> Rig {
    let mut r = Rig::new(rate);
    r.feed(y);
    r.finish();
    r
}

/// Section 8 test 12: 200 kHz, P2, 3000 random bytes, 2 passes at 1.25 Msps, Es/N0 6 dB, four carrier offsets.
#[test]
fn default_transmitter_200_khz_with_offsets() {
    let file = random_bytes(3000, 12);
    for (i, hz) in [-200.0, 150.0, 2000.0, -2100.0].into_iter().enumerate() {
        let y = carousel(&file, "data.bin", 12, 2, Width::W200K, 2, 1.25e6, Some(6.0), hz, 120 + i as u64);
        let r = deliver(&y, 1.25e6);
        assert_eq!(r.files.len(), 1, "{hz} Hz");
        assert_eq!(r.files[0].1, file, "{hz} Hz");
    }
}

/// Section 8 test 13: 1 MHz, P2, 6000 bytes, 2 passes at 2.4 Msps, Es/N0 12 dB, three carrier offsets.
#[test]
fn default_transmitter_1_mhz_with_offsets() {
    let file = random_bytes(6000, 13);
    for (i, hz) in [5000.0, -5000.0, 3000.0].into_iter().enumerate() {
        let y = carousel(&file, "data.bin", 13, 2, Width::W1M, 2, 2.4e6, Some(12.0), hz, 130 + i as u64);
        let r = deliver(&y, 2.4e6);
        assert_eq!(r.files.len(), 1, "{hz} Hz");
        assert_eq!(r.files[0].1, file, "{hz} Hz");
    }
}

/// Section 8 test 15: clean files at each width, 2 passes: one file (the repeat is ignored), the same bytes and name, no bad
/// block.
#[test]
fn clean_files_at_each_width() {
    for (w, p, rate, n) in [(Width::W200K, 2, 1.25e6, 3000), (Width::W500K, 4, 2.4e6, 20_000), (Width::W1M, 3, 2.4e6, 20_000)] {
        let file = random_bytes(n, 15 + p as u64);
        let name = sdrkit_stack_custom::file_transfer::stack::tx_name("dir/clean.bin");
        let y = carousel(&file, &name, 15, p, w, 2, rate, None, 0.0, 0);
        let r = deliver(&y, rate);
        assert_eq!(r.files.len(), 1, "{w:?}");
        assert_eq!(r.files[0].0, "clean.bin", "{w:?}");
        assert!(r.files[0].1 == file, "{w:?}");
        assert_eq!(r.bad, 0, "{w:?}");
    }
}

/// Section 8 test 16: a blanked part of the first pass is filled by the second; with one pass blanked, no file.
#[test]
fn holes_are_filled_by_the_second_pass() {
    let file = random_bytes(12_000, 16);
    let rate = 1.25e6;
    let mut y = carousel(&file, "holes.bin", 16, 2, Width::W200K, 2, rate, Some(20.0), 0.0, 16);
    let n = y.len();
    y[n * 22 / 100..n * 34 / 100].fill(Complex32::new(0.0, 0.0));
    let r = deliver(&y, rate);
    assert_eq!(r.files.len(), 1);
    assert_eq!(r.files[0].1, file);
    let mut half = y[..n / 2].to_vec();
    half[n * 20 / 100..n * 35 / 100].fill(Complex32::new(0.0, 0.0));
    let r = deliver(&half, rate);
    assert!(r.files.is_empty());
}

/// Section 8 test 17: a session cut at 55 %, then a new session of the same file: the bytes held are reported and the file
/// arrives intact.
#[test]
fn resume_in_a_new_session() {
    let file = random_bytes(40_000, 17);
    let rate = 1.25e6;
    let a = carousel(&file, "big.bin", 5, 3, Width::W200K, 1, rate, Some(20.0), 0.0, 17);
    let b = carousel(&file, "big.bin", 6, 3, Width::W200K, 1, rate, Some(20.0), 0.0, 18);
    let mut r = Rig::new(rate);
    r.feed(&a[..a.len() * 55 / 100]);
    r.feed(&vec![Complex32::new(0.0, 0.0); 100_000]);
    assert!(r.files.is_empty());
    r.feed(&b);
    r.finish();
    let held = r.events.iter().find_map(|e| match e {
        RxEvent::Opened { session: 6, held, .. } => Some(*held),
        _ => None,
    });
    assert!(held.is_some_and(|h| h > 10_000), "{held:?}");
    assert_eq!(r.files.len(), 1);
    assert_eq!(r.files[0].1, file);
}

/// The sample files of Part G section 9 made again: session 7, LY1TST, two bursts of four frames (frame number 4 b + f, the
/// last flag on frame 3), the most blocks a frame, block n at offset n D, 4 samples a symbol, 0.1 s of silence first and
/// 0.05 s after each burst, then noise at an SNR over the signal samples.
#[test]
fn sample_files_made_again() {
    let sentence = b"The quick brown fox jumps over the lazy dog while the file link sends its test sentence. ";
    for (w, p, snr, blocks) in [
        (Width::W200K, 1, 12.0, 24),
        (Width::W500K, 4, 24.0, 120),
        (Width::W1M, 1, 12.0, 56),
        (Width::W1M, 4, 24.0, 120),
        (Width::W1M, 8, 36.0, 312),
    ] {
        let nb = max_blocks(&PROFILES[p], w);
        let d = PROFILES[p].data();
        let fs = w.rate();
        let mut y = vec![Complex32::new(0.0, 0.0); (0.1 * fs) as usize];
        let mut n = 0usize;
        let mut sent = Vec::new();
        for b in 0..2u16 {
            let frames: Vec<TxFrame> = (0..4u16)
                .map(|f| {
                    let blocks = (0..nb)
                        .map(|_| {
                            let data: Vec<u8> = sentence.iter().copied().cycle().skip(n * d % sentence.len()).take(d).collect();
                            let blk = DataBlock { offset: (n * d) as u32, data }.encode();
                            n += 1;
                            blk
                        })
                        .collect();
                    TxFrame { header: header(w, p, FrameKind::Data, nb, 4 * b + f, f as u8, f == 3), blocks }
                })
                .collect();
            y.extend(phy(&frames));
            y.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), (0.05 * fs) as usize));
            sent.extend(frames);
        }
        let ps = common::power(&y.iter().copied().filter(|v| v.norm_sqr() > 0.0).collect::<Vec<_>>());
        Noise::new(p as u64).add(&mut y, ps / 10f64.powf(snr / 10.0));
        let got = receive(&y, fs, &WIDTHS, 65_536);
        assert_eq!(got.len(), 8, "{w:?} P{p}");
        let total: usize = got.iter().zip(&sent).map(|(g, s)| good(g, s)).sum();
        assert_eq!(total, blocks, "{w:?} P{p}");
    }
}

/// The five recorded files of section 9, in both sample forms: 8 frames of session 7 from LY1TST with frame numbers 0 to 7,
/// every block good, block n at offset n D, and the data of each block one 99-byte sentence repeated.
#[test]
fn the_recorded_vectors() {
    let Some(root) = common::recordings() else { return };
    let dir = common::folder_with(&root, "_200_kHz_P1_low_cf32.sigmf-data");
    for (stem, w, p, blocks) in [
        ("_200_kHz_P1_low", Width::W200K, 1, 24),
        ("_500_kHz_P4_mid", Width::W500K, 4, 120),
        ("_1_MHz_P1_low", Width::W1M, 1, 56),
        ("_1_MHz_P4_mid", Width::W1M, 4, 120),
        ("_1_MHz_P8_high", Width::W1M, 8, 312),
    ] {
        for (name, rate, y) in common::both_forms(&dir, stem) {
            assert_eq!(rate, w.rate(), "{name}");
            let got = receive(&y, rate, &WIDTHS, 65_536);
            assert_eq!(got.iter().map(|f| f.header.frame).collect::<Vec<_>>(), (0..8).collect::<Vec<u16>>(), "{name}");
            let d = PROFILES[p].data();
            let mut data = Vec::new();
            for f in &got {
                assert_eq!((f.width, usize::from(f.header.profile), f.header.session, f.header.callsign.as_str()), (w, p, 7, "LY1TST"), "{name}");
                for b in &f.blocks {
                    let blk = DataBlock::decode(b.as_deref().unwrap_or_else(|| panic!("{name} frame {}: a block failed", f.header.frame))).expect("block");
                    assert_eq!(blk.offset as usize, data.len(), "{name}");
                    assert_eq!(blk.data.len(), d, "{name}");
                    assert!(blk.data.iter().zip(blk.data.iter().skip(99)).all(|(a, b)| a == b), "{name}: not a repeated 99-byte sentence");
                    data.extend(blk.data);
                }
            }
            assert_eq!(data.len(), blocks * d, "{name}");
            assert!(data.iter().all(|c| c.is_ascii_graphic() || *c == b' '), "{name}: not text");
        }
    }
}

/// A link between a two-way sender and a receiver, carried as messages and block payloads with their CRC-32.
struct Link {
    u: Uni,
    /// Probability that a frame header is lost.
    frame_loss: f64,
    /// Probability that a block fails its CRC.
    block_loss: f64,
    /// Probability that a reply is lost.
    reply_loss: f64,
    /// Lose the first ACCEPT.
    lose_accept: bool,
    /// Change one byte of one data block, with a CRC that passes, once.
    forge: bool,
    /// Frames and blocks with a CRC that failed, as the channel made them.
    crc_failures: usize,
}

impl Link {
    fn new(seed: u64) -> Link {
        Link { u: Uni(seed | 1), frame_loss: 0.0, block_loss: 0.0, reply_loss: 0.0, lose_accept: false, forge: false, crc_failures: 0 }
    }

    /// A block content through the channel: the payload and its CRC-32, a bit flipped when the block is lost, and the CRC
    /// checked as the receiver checks it.
    fn block(&mut self, content: &[u8], payload: usize, data: bool) -> Option<Vec<u8>> {
        let mut b = content.to_vec();
        b.resize(payload, 0);
        if self.forge && data && content.len() > 10 {
            self.forge = false;
            b[8] ^= 0x55;
        }
        let crc = sdrkit_stack_custom::family::crc32(&b);
        let mut word = b.clone();
        word.extend_from_slice(&crc.to_le_bytes());
        if self.u.next() < self.block_loss {
            let k = (self.u.next() * word.len() as f64) as usize;
            word[k] ^= 1 << (k % 8);
        }
        let (body, tail) = word.split_at(payload);
        let ok = sdrkit_stack_custom::family::crc32(body) == u32::from_le_bytes(tail.try_into().unwrap());
        if !ok {
            self.crc_failures += 1;
        }
        ok.then(|| body.to_vec())
    }

    /// Runs a session until the sender is done or failed, or `stop` says so; returns the files delivered.
    fn run(&mut self, s: &mut Sender, r: &mut SessionReceiver, stop: impl Fn(&Sender) -> bool) -> Vec<Vec<u8>> {
        let mut files = Vec::new();
        let mut outs = vec![s.start()];
        let mut col = Collector::default();
        for _ in 0..200_000 {
            if matches!(s.state(), SenderState::Done | SenderState::Failed(_)) {
                break;
            }
            if stop(s) {
                // What was not put on the air yet does not count as sent.
                for o in &outs {
                    if let SenderOut::Burst(b) = o {
                        s.data_sent -= b.data_bytes() as u64;
                    }
                }
                break;
            }
            let mut replies = Vec::new();
            for o in outs.drain(..) {
                match o {
                    SenderOut::Control(m) => {
                        let f = control_frame(&m, Width::W1M, "LY1TST", 0).unwrap();
                        let p = PROFILES[1].payload();
                        let blocks: Vec<Option<Vec<u8>>> = f.blocks.iter().map(|b| self.block(b, p, false)).collect();
                        let Some(m) = join_message(&blocks) else { continue };
                        let mut ev = Vec::new();
                        r.on_message(&m, Some(40.0), &mut replies, &mut ev);
                        for e in ev {
                            if let RxEvent::Delivered { bytes, .. } = e {
                                files.push(bytes);
                            }
                        }
                    }
                    SenderOut::Burst(b) => {
                        let p = PROFILES[usize::from(b.profile)].payload();
                        let mut done = Vec::new();
                        for f in b.tx_frames(s.config().session, Width::W1M, "LY1TST") {
                            if self.u.next() < self.frame_loss {
                                continue;
                            }
                            let blocks = f.blocks.iter().map(|c| self.block(c, p, true)).collect();
                            let h = f.header;
                            let d = RxData { index: h.index, last: h.last, profile: h.profile, blocks, esn0_db: 40.0 };
                            col.take(h.session, h.frame, d, 0.0, &mut done);
                        }
                        col.flush(&mut done);
                        for x in done {
                            r.on_burst(&x, &mut replies);
                        }
                    }
                }
            }
            let mut next = Vec::new();
            for m in replies {
                if matches!(m, Message::Accept { .. }) && std::mem::take(&mut self.lose_accept) {
                    continue;
                }
                if self.u.next() < self.reply_loss {
                    continue;
                }
                next.extend(s.on_message(&m));
            }
            if next.is_empty() && !matches!(s.state(), SenderState::Done | SenderState::Failed(_)) {
                match s.on_timeout() {
                    Some(o) => next.push(o),
                    None => break,
                }
            }
            outs = next;
        }
        files
    }
}

fn sender(session: u16, file: &[u8], profile: usize) -> Sender {
    Sender::new(
        SenderConfig {
            session,
            callsign: "LY1TST".into(),
            recipient: "CQ".into(),
            width: Width::W1M,
            name: "file.bin".into(),
            profile,
            fixed_profile: false,
            profiles: (0, 9),
        },
        file.to_vec(),
    )
}

/// A two-ended session over a channel that loses frame headers, fails block CRCs and loses replies, with one block whose
/// data was changed under a CRC that passes: the whole-file hash fails once, the receiver drops the bytes, the sender sends
/// the file again, and the file then arrives intact.
#[test]
fn two_ended_session_with_lost_frames_bad_crcs_and_a_wrong_hash() {
    let file = random_bytes(60_000, 31);
    let mut link = Link::new(31);
    link.frame_loss = 0.1;
    link.block_loss = 0.1;
    link.reply_loss = 0.05;
    link.forge = true;
    let mut s = sender(31, &file, 4);
    let mut r = SessionReceiver::default();
    let files = link.run(&mut s, &mut r, |_| false);
    assert_eq!(s.state(), &SenderState::Done);
    assert_eq!(files, vec![file.clone()]);
    assert!(link.crc_failures > 5);
    // The hash failure made the sender send the file twice at least.
    assert!(s.data_sent >= 2 * file.len() as u64, "{}", s.data_sent);
}

/// Section 8 test 18: 2 MiB at 1 MHz from P4, block loss 0, 5, 20 and 50 %, reply loss 1 %, cut at 40 %; a new session of
/// the same file completes it.
#[test]
fn two_way_two_mebibytes_with_a_resume() {
    let file = random_bytes(2 << 20, 18);
    let size = file.len() as u64;
    for (i, loss) in [0.0, 0.05, 0.2, 0.5].into_iter().enumerate() {
        let mut link = Link::new(180 + i as u64);
        link.block_loss = loss;
        link.reply_loss = 0.01;
        let mut r = SessionReceiver::default();
        let mut a = sender(1, &file, 4);
        let got = link.run(&mut a, &mut r, |s| s.acked_bytes() * 10 >= size * 4);
        assert!(got.is_empty());
        let mut b = sender(2, &file, 4);
        let got = link.run(&mut b, &mut r, |_| false);
        assert_eq!(b.state(), &SenderState::Done, "loss {loss}");
        assert_eq!(got.len(), 1);
        assert!(got[0] == file, "loss {loss}");
        assert!(b.data_sent <= 3 * size + 1_000_000, "loss {loss}: {}", b.data_sent);
        if loss <= 0.2 {
            let eff = size as f64 / (a.data_sent + b.data_sent) as f64;
            assert!(eff >= 0.9 * (1.0 - loss), "loss {loss}: efficiency {eff}");
        }
    }
}

/// Section 8 test 19: a lost ACCEPT brings OPEN again; a small and an empty file go through; a receiver without the
/// compression codec answers ABORT and the sender fails.
#[test]
fn lost_accept_small_and_empty_files_and_compression() {
    for file in [b"tiny".to_vec(), Vec::new()] {
        let mut link = Link::new(19);
        link.lose_accept = true;
        let mut s = sender(19, &file, 2);
        let mut r = SessionReceiver::default();
        let files = link.run(&mut s, &mut r, |_| false);
        assert_eq!(s.state(), &SenderState::Done);
        assert_eq!(files, vec![file]);
    }
    let mut s = sender(20, b"abc", 2);
    let SenderOut::Control(Message::Open(mut o)) = s.start() else { panic!("OPEN first") };
    o.compression = 1;
    let mut r = SessionReceiver::default();
    let (mut replies, mut ev) = (Vec::new(), Vec::new());
    r.on_message(&Message::Open(o), None, &mut replies, &mut ev);
    assert_eq!(replies, vec![Message::Abort { session: 20, reason: 2 }]);
    assert!(s.on_message(&replies[0]).is_empty());
    assert!(matches!(s.state(), SenderState::Failed(_)));
    // A sender that hears nothing gives up after 6 retries.
    let mut s = sender(21, b"abc", 2);
    s.start();
    let mut n = 0;
    while s.on_timeout().is_some() {
        n += 1;
    }
    assert_eq!(n, 6);
    assert!(matches!(s.state(), SenderState::Failed(_)));
}

/// A receiver with a callsign takes a file for CQ or for itself, not for another station; an announced size above its
/// limit is refused.
#[test]
fn receiver_callsign_and_size_limit() {
    let s = sender(40, b"abc", 2);
    let SenderOut::Control(Message::Open(o)) = s.clone().start() else { panic!("OPEN first") };
    let mut r = SessionReceiver::default();
    r.callsign = "DL2XYZ".into();
    for (to, answered) in [("CQ", true), ("dl2xyz", true), ("G0ABC", false)] {
        let (mut replies, mut ev) = (Vec::new(), Vec::new());
        r.on_message(&Message::Open(sdrkit_stack_custom::file_transfer::session::Open { receiver: to.into(), ..o.clone() }), None, &mut replies, &mut ev);
        assert_eq!(!replies.is_empty(), answered, "{to}");
    }
    let mut r = SessionReceiver::with_limit(2);
    let (mut replies, mut ev) = (Vec::new(), Vec::new());
    r.on_message(&Message::Open(o), None, &mut replies, &mut ev);
    assert_eq!(replies, vec![Message::Abort { session: 40, reason: 3 }]);
    assert_eq!(safe_name("../x/../../y.txt"), "y.txt");
}

/// A two-ended session through the physical layer at 200 kHz: frames and replies are real signals with noise, and the
/// first data burst loses one of its frames.
#[test]
fn two_ended_session_over_the_air() {
    use sdrkit_stack_custom::file_transfer::stack::item_samples;
    let file = random_bytes(5000, 41);
    let w = Width::W200K;
    let fs = w.rate();
    let mut s = Sender::new(
        SenderConfig {
            session: 41,
            callsign: "LY1TST".into(),
            recipient: "CQ".into(),
            width: w,
            name: "air.bin".into(),
            profile: 1,
            fixed_profile: true,
            profiles: (0, 9),
        },
        file.clone(),
    );
    let mut far = Rig::new(fs);
    let mut near = Receiver::new(fs, &[w]);
    let mut frame_no = 0u16;
    let mut outs = vec![s.start()];
    let mut seed = 400;
    let mut blanked = false;
    for _ in 0..60 {
        if matches!(s.state(), SenderState::Done | SenderState::Failed(_)) {
            break;
        }
        for o in outs.drain(..) {
            let item = match o {
                SenderOut::Control(m) => CarouselItem::Control(m),
                SenderOut::Burst(b) => CarouselItem::Burst(b),
            };
            let mut x = item_samples(&item, 41, w, "LY1TST", &mut frame_no, fs);
            if let (CarouselItem::Burst(b), false) = (&item, blanked) {
                if b.frames.len() > 1 {
                    // The second frame of the first burst is lost.
                    blanked = true;
                    let per = x.len() / b.frames.len();
                    x[per..2 * per].fill(Complex32::new(0.0, 0.0));
                }
            }
            for v in &mut x {
                *v /= TX_LEVEL;
            }
            seed += 1;
            Noise::new(seed).add(&mut x, 4.0 / 10f64.powf(0.8));
            far.feed(&x);
            far.feed(&vec![Complex32::new(0.0, 0.0); (0.3 * fs) as usize]);
        }
        let mut next = Vec::new();
        for m in std::mem::take(&mut far.replies) {
            let mut y = item_samples(&CarouselItem::Control(m), 41, w, "DL2XYZ", &mut frame_no, fs);
            for v in &mut y {
                *v /= TX_LEVEL;
            }
            seed += 1;
            Noise::new(seed).add(&mut y, 4.0 / 10f64.powf(0.8));
            y.extend(vec![Complex32::new(0.0, 0.0); 20_000]);
            let mut got = Vec::new();
            near.process(&y, &mut got);
            for f in got {
                if let Some(m) = join_message(&f.blocks) {
                    next.extend(s.on_message(&m));
                }
            }
        }
        if next.is_empty() {
            next.extend(s.on_timeout());
        }
        outs = next;
    }
    assert_eq!(s.state(), &SenderState::Done);
    assert!(blanked);
    assert_eq!(far.files, vec![("air.bin".to_string(), file)]);
}

/// The test signal through the stack's graph at 2.4 Msps, moved by `hz`, with noise of Es/N0 `db` (4 samples a symbol of
/// the 200 kHz width at unit power), for `seconds`.
fn engine(hz: f64, db: f64, seconds: f64) -> sdrkit_core::Results {
    use sdrkit_core::{GenConfig, InputInfo, Output, StackConfig, Values};
    let d = &sdrkit_stack_custom::file_transfer::FILE_TRANSFER;
    let rate = d.tuning.rate_hz;
    let s = Values::defaults(&(d.settings)());
    let mut g = (d.make_generator.unwrap())(&GenConfig::new(rate, s.clone()).with_seed(1)).unwrap();
    let mut rx = (d.make_receiver)(&StackConfig::new(s)).unwrap();
    rx.configure(&InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording()).unwrap();
    let mut out = Output::default();
    let mut x = vec![Complex32::new(0.0, 0.0); 60_000];
    let mut n = 0usize;
    let mut noise = Noise::new(14);
    // Noise of variance 4 / Es/N0 at 640 ksps is that times 640 / 2400 at the input rate's bandwidth per sample.
    let var = 4.0 / 10f64.powf(db / 10.0) * Width::W200K.rate() / rate;
    for _ in 0..(rate * seconds) as usize / x.len() {
        g.generate(&mut x);
        for (i, v) in x.iter_mut().enumerate() {
            let a = std::f64::consts::TAU * (hz * (n + i) as f64 / rate).fract();
            *v = *v * Complex32::new(a.cos() as f32, a.sin() as f32) + noise.sample(var);
        }
        n += x.len();
        rx.feed(&x, &mut out);
        out.clear();
    }
    rx.finish(&mut out);
    rx.results().clone()
}

/// Section 8 test 14: the test signal (200 kHz, P2, the demonstration file, session 4242) at +1500 Hz with a little noise:
/// the file arrives with a good hash, bad blocks at most 10 % of good blocks, the offset within 300 Hz, the width 200 kHz.
#[test]
fn synthetic_loop_through_the_graph() {
    use sdrkit_core::publish::keys;
    use sdrkit_stack_custom::typed::FileTransferFields as Ft;
    let r = engine(1500.0, 20.0, 4.0);
    let f = r.fields();
    let good = f.at(&Ft::BLOCKS_GOOD).unwrap_or(0);
    assert!(f.at(&Ft::FILES).unwrap_or(0) >= 1);
    assert_eq!(f.at(&Ft::HASH_OK), Some(true));
    assert!(f.at(&Ft::BLOCKS_BAD).unwrap_or(0) * 10 <= good && good > 0);
    assert!((f.get(&keys::OFFSET_HZ).unwrap() - 1500.0).abs() < 300.0);
    assert_eq!(f.at(&Ft::WIDTH), Some("200 kHz"));
    assert_eq!(f.at(&Ft::SESSION), Some(4242));
}

/// Section 8 test 21: noise, silence, a constant and full-scale random input in pieces of 1, 7, 96 and 4801 samples, at 48
/// kHz and at radio rates: no crash, no frame.
#[test]
fn odd_input_gives_nothing() {
    for rate in [48_000.0, 1.25e6, 2.4e6, 3.2e6] {
        let mut rx = Receiver::new(rate, &WIDTHS);
        let mut noise = Noise::new(21);
        let mut u = Uni(21);
        let mut out = Vec::new();
        for kind in 0..4 {
            for piece in [1usize, 7, 96, 4801] {
                let n = if piece == 1 { 3000 } else { 50_000 };
                let x: Vec<Complex32> = (0..n)
                    .map(|_| match kind {
                        0 => noise.sample(1.0),
                        1 => Complex32::new(0.0, 0.0),
                        2 => Complex32::new(0.7, -0.3),
                        _ => Complex32::new((2.0 * u.next() - 1.0) as f32, (2.0 * u.next() - 1.0) as f32),
                    })
                    .collect();
                for c in x.chunks(piece) {
                    rx.process(c, &mut out);
                }
            }
        }
        rx.finish(&mut out);
        assert!(out.is_empty(), "{rate}: {} frames", out.len());
    }
}

/// The samples a graph sends to its transmitter.
#[derive(Default)]
struct Air(std::sync::Mutex<Vec<Complex32>>);

impl sdrkit_graph::components::HostStreams for Air {
    fn write(&self, port: sdrkit_graph::components::HostPort, data: sdrkit_graph::components::HostData<'_>) {
        if let (sdrkit_graph::components::HostPort::Transmit, sdrkit_graph::components::HostData::Iq { samples, .. }) = (port, data) {
            self.0.lock().unwrap().extend_from_slice(samples);
        }
    }
}

/// The stack's graph at `rate` with `settings` (key and value), its host storage and a capture of its transmitter.
fn ft_graph(rate: f64, settings: &[(&str, sdrkit_core::Value)], storage: std::sync::Arc<sdrkit_core::MemStorage>, air: std::sync::Arc<Air>) -> sdrkit_graph::StackGraph {
    use sdrkit_core::{InputInfo, Receiver as _, StackConfig, Values};
    let d = &sdrkit_stack_custom::file_transfer::FILE_TRANSFER;
    let mut s = Values::defaults(&(d.settings)());
    let schema = s.schema().clone();
    for (k, v) in settings {
        s.set(schema.id(k).unwrap(), v).unwrap();
    }
    let mut g = sdrkit_stack_custom::file_transfer::stack::graph(&StackConfig::new(s)).unwrap();
    g.set_context(sdrkit_graph::RunContext { transmit: true, storage, streams: Some(air), ..Default::default() });
    g.configure(&InputInfo::new(rate, 0.0, 0.0)).unwrap();
    g
}

/// The sender component reads its file from the host's storage, and the receiver component writes the file it receives into
/// its own host storage, in its folder, under the sanitised name that came over the air.
#[test]
fn files_from_and_to_host_storage() {
    use sdrkit_core::{Output, Owner, Receiver as _, Storage, StorePath, Value};
    use std::sync::Arc;
    let rate = 1.25e6;
    let file = random_bytes(3000, 77);
    let outbox = Arc::new(sdrkit_core::MemStorage::new());
    outbox.write(&StorePath::new("outbox/notes #2 (draft)*.bin").unwrap(), &file).unwrap();
    let air = Arc::new(Air::default());
    let mut tx = ft_graph(
        rate,
        &[("callsign", Value::Text("LY1TST".into())), ("file", Value::Text("outbox/notes #2 (draft)*.bin".into())), ("passes", Value::I64(1))],
        outbox,
        air.clone(),
    );
    let send = tx.commands().id("send").unwrap();
    let args = tx.commands().check(send, &tx.commands().def(send).unwrap().new_args()).unwrap();
    tx.invoke(send, &args).unwrap();
    let inbox = Arc::new(sdrkit_core::MemStorage::new());
    let mut rx = ft_graph(rate, &[], inbox.clone(), Arc::new(Air::default()));
    let mut out = Output::default();
    let quiet = vec![Complex32::new(0.0, 0.0); 62_500];
    let saved = StorePath::new("file_transfer/notes _2 (draft)_.bin").unwrap();
    for _ in 0..200 {
        tx.feed(&quiet, &mut out);
        out.clear();
        let sent = std::mem::take(&mut *air.0.lock().unwrap());
        let mut y = sent;
        y.resize(quiet.len().max(y.len()), Complex32::new(0.0, 0.0));
        rx.feed(&y, &mut out);
        out.clear();
        if inbox.read(&saved).is_ok() {
            break;
        }
    }
    assert_eq!(inbox.read(&saved).expect("the received file in storage"), file);
}
