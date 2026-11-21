//! The frames, flags and counters of [`WordSync`] against a record on fixed bit streams: noise before the first frame,
//! wrong bits in sync words, a bit slip, a change of polarity, a sync word that the next frame does not confirm and a
//! loss, for several rules and for the input in pieces of several lengths.

use sdrkit_proto_ccsds::wordsync::{WordSync, WordSyncConfig, WordSyncStats};

/// A linear congruential generator: the same numbers on every target.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 32) as u32
    }
    fn bits(&mut self, n: usize, out: &mut Vec<u8>) {
        // Bit 1 set too: the synchroniser reads bit 0 only.
        out.extend((0..n).map(|_| ((self.next() >> 9) & 3) as u8));
    }
}

/// Appends `count` frames whose sync words have `wrong` bits flipped, the whole frame inverted with `invert`.
fn frames(c: &WordSyncConfig, g: &mut Lcg, count: usize, wrong: u32, invert: bool, out: &mut Vec<u8>) {
    for _ in 0..count {
        let start = out.len();
        for i in (0..c.word_bits).rev() {
            out.push(((c.word >> i) & 1) as u8);
        }
        for k in 0..wrong {
            let at = start + ((g.next() >> 4) as usize + 5 * k as usize) % c.word_bits as usize;
            out[at] ^= 1;
        }
        g.bits(c.frame_bits - c.word_bits as usize, out);
        if invert {
            out[start..].iter_mut().for_each(|b| *b ^= 1);
        }
    }
}

/// The stream of one set of rules.
fn stream(c: &WordSyncConfig, seed: u64) -> Vec<u8> {
    let mut g = Lcg(seed);
    let mut x = Vec::new();
    g.bits(777, &mut x);
    frames(c, &mut g, 5, 0, false, &mut x);
    frames(c, &mut g, 2, c.acquire_errors, false, &mut x);
    frames(c, &mut g, 2, c.track_errors, false, &mut x);
    frames(c, &mut g, 1, c.track_errors + 1, false, &mut x);
    frames(c, &mut g, 3, 0, false, &mut x);
    // A change of polarity, then back.
    frames(c, &mut g, 4, 1.min(c.track_errors), true, &mut x);
    frames(c, &mut g, 3, 0, false, &mut x);
    // A slip of three bits: misses, a loss and a new lock.
    g.bits(3, &mut x);
    frames(c, &mut g, c.max_misses as usize + 6, 0, false, &mut x);
    // Noise, one frame alone (its lock is not confirmed), noise, and a sync word that starts inside the word read for
    // a confirmation.
    g.bits((c.max_misses as usize + 2) * c.frame_bits + 13, &mut x);
    frames(c, &mut g, 1, 0, false, &mut x);
    g.bits(c.word_bits as usize / 2, &mut x);
    frames(c, &mut g, 4, 0, true, &mut x);
    g.bits((c.max_misses as usize + 3) * c.frame_bits, &mut x);
    frames(c, &mut g, 3, c.acquire_errors, false, &mut x);
    x
}

/// A digest of the frames (bytes and flags, in order) and the counters, for the input in pieces of `cut` bits.
fn run(c: WordSyncConfig, x: &[u8], cut: usize) -> (u64, u64, WordSyncStats, bool, bool) {
    let mut s = WordSync::new(c).unwrap();
    let (mut h, mut n) = (0xCBF2_9CE4_8422_2325u64, 0u64);
    let mut eat = |b: u8| h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3);
    for piece in x.chunks(cut) {
        s.push_bits(piece, |f| {
            n += 1;
            f.bytes.iter().for_each(|&b| eat(b));
            eat(u8::from(f.inverted));
            eat(f.word_errors as u8);
            eat(u8::from(f.missed));
        });
    }
    (h, n, *s.stats(), s.locked(), s.inverted())
}

fn rules() -> Vec<WordSyncConfig> {
    let base = WordSyncConfig {
        word: 0x1ACF_FC1D,
        word_bits: 32,
        frame_bits: 1024,
        acquire_errors: 2,
        track_errors: 8,
        complement: true,
        max_misses: 3,
    };
    vec![
        base,
        WordSyncConfig { complement: false, frame_bits: 333, ..base },
        WordSyncConfig { word: 0xB5, word_bits: 8, frame_bits: 20, acquire_errors: 0, track_errors: 1, ..base },
        WordSyncConfig { word: 0x0347_76C7_2728_95B0, word_bits: 64, frame_bits: 500, max_misses: 0, ..base },
        WordSyncConfig { word: 0x2_C7E5, word_bits: 18, frame_bits: 97, acquire_errors: 1, track_errors: 3, ..base },
    ]
}

/// The record: the digest of the frames, their count and the counters (sync hits, frames, confirmed, unconfirmed,
/// misses, losses) of each set of rules.
const RECORD: [(u64, u64, [u64; 6]); 5] = [
    (0x39AA_4AF1_94C9_348E, 41, [33, 41, 4, 1, 12, 3]),
    (0x3655_2101_726D_B598, 33, [25, 33, 4, 1, 12, 3]),
    (0xD031_7E46_2E7A_97BC, 41, [33, 41, 4, 2, 13, 3]),
    (0xDCC2_16E9_B865_17DB, 31, [32, 31, 5, 1, 4, 4]),
    (0x27F8_4523_BC64_1AD0, 41, [33, 41, 4, 2, 13, 3]),
];

#[test]
fn the_frames_are_those_of_the_record() {
    for (k, c) in rules().into_iter().enumerate() {
        let x = stream(&c, 40 + k as u64);
        let whole = run(c, &x, x.len());
        let st = whole.2;
        let counters = [st.sync_hits, st.frames, st.confirmed, st.unconfirmed, st.misses, st.losses];
        assert_eq!((whole.0, whole.1, counters), RECORD[k], "rules {k}");
        for cut in [1, 2, 7, 31, 64, 1000, 4099] {
            assert_eq!(run(c, &x, cut), whole, "rules {k} cut {cut}");
        }
    }
}
