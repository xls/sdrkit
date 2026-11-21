//! RDS (IEC 62106; RBDS, NRSC-4-B, shares the bit stream): the 57 kHz demodulator, the block layer, the group decoder and
//! the sender ([`encode`]).

pub mod block;
pub mod demod;
pub mod encode;
pub mod group;
pub mod tmc;
pub mod view;

pub use block::{BlockLayer, BlockStats, Group};
pub use demod::RdsDemod;
pub use group::GroupDecoder;
pub use view::RdsView;

use sdrkit_util_text::ebu::RdsCharset;

/// How much the block layer repairs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Correction {
    /// 0 (nothing), 1 (bursts of 1 or 2 bits) or 2 (also two weak symbols).
    pub level: u8,
    /// Voting over repeated copies (at level 2).
    pub voting: bool,
}

impl Default for Correction {
    fn default() -> Self {
        Correction {
            level: 2,
            voting: true,
        }
    }
}

/// The data layers of RDS: blocks and groups. Feed it bits (from [`RdsDemod`] or straight from a test).
#[derive(Clone, Debug, Default)]
pub struct RdsDecoder {
    /// The block layer.
    pub blocks: BlockLayer,
    /// The decoded fields.
    pub groups: GroupDecoder,
    /// Group monitor lines (`"PPPP BBBB CCCC DDDD"`) since they were last taken.
    pub monitor: Vec<String>,
    /// Keep monitor lines (off by default, so nothing grows unless requested).
    pub keep_monitor: bool,
}

impl RdsDecoder {
    /// A decoder with this correction and charset.
    pub fn new(correction: Correction, charset: RdsCharset) -> Self {
        let mut d = RdsDecoder {
            groups: GroupDecoder::new(charset),
            ..Default::default()
        };
        d.set_correction(correction);
        d
    }

    /// Changes the correction.
    pub fn set_correction(&mut self, c: Correction) {
        self.blocks.set_level(c.level);
        self.blocks.set_voting(c.voting && c.level >= 2);
    }

    /// Clears everything decoded and the sync, keeping the settings.
    pub fn reset(&mut self) {
        self.blocks.reset();
        self.groups.reset();
        self.monitor.clear();
    }

    /// One data bit with its reliability. Returns whether a group was accepted.
    pub fn push_bit(&mut self, bit: bool, rel: f32, wall_clock_s: f64) -> bool {
        match self.blocks.push(bit, rel) {
            Some(g) => {
                if self.keep_monitor {
                    if self.monitor.len() >= 256 {
                        self.monitor.remove(0);
                    }
                    self.monitor.push(g.hex());
                }
                self.groups.push(&g, wall_clock_s)
            }
            None => false,
        }
    }
}

/// The host's wall clock in seconds since 1970 (it stamps the clock-time groups).
pub fn wall_clock_s() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::block::{encode_block, OFFSET_A, OFFSET_B, OFFSET_C, OFFSET_CP, OFFSET_D};
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    /// The test groups: the PS "SDRKIT  " in four 0A groups, then "Hello world\r" in three 2A groups.
    fn test_groups() -> Vec<[u16; 4]> {
        let pi = 0x4A21;
        let mut v = Vec::new();
        let ps = b"SDRKIT  ";
        for s in 0..4u16 {
            let b = 0x0400 | (10 << 5) | 0x08 | s;
            v.push([
                pi,
                b,
                0xCDCD,
                u16::from_be_bytes([ps[2 * s as usize], ps[2 * s as usize + 1]]),
            ]);
        }
        let rt = b"Hello world\r";
        for (i, c) in rt.chunks(4).enumerate() {
            let b = 0x2000 | 0x0400 | (10 << 5) | i as u16;
            v.push([
                pi,
                b,
                u16::from_be_bytes([c[0], c[1]]),
                u16::from_be_bytes([c[2], c[3]]),
            ]);
        }
        v
    }

    fn words(g: &[u16; 4]) -> [u32; 4] {
        let c_off = if g[1] & 0x0800 != 0 {
            OFFSET_CP
        } else {
            OFFSET_C
        };
        [
            encode_block(g[0], OFFSET_A),
            encode_block(g[1], OFFSET_B),
            encode_block(g[2], c_off),
            encode_block(g[3], OFFSET_D),
        ]
    }

    struct Run {
        groups: u64,
        wrong_ps: bool,
        wrong_rt: bool,
        ps: String,
        rt: String,
    }

    /// Feeds groups; `damage` can change a group's 104 bits and reliabilities.
    fn run(
        dec: &mut RdsDecoder,
        n: usize,
        list: &[[u16; 4]],
        mut damage: impl FnMut(usize, &mut [bool; 104], &mut [f32; 104]),
    ) -> Run {
        let mut wrong_ps = false;
        let mut wrong_rt = false;
        for k in 0..n {
            let g = &list[k % list.len()];
            let w = words(g);
            let mut bits = [false; 104];
            let mut rel = [1.0f32; 104];
            for (bi, word) in w.iter().enumerate() {
                for i in 0..26 {
                    bits[bi * 26 + i] = word >> (25 - i) & 1 == 1;
                }
            }
            damage(k, &mut bits, &mut rel);
            for i in 0..104 {
                dec.push_bit(bits[i], rel[i], 0.0);
                let ps = &dec.groups.ps;
                if !ps.is_empty() && ps != "SDRKIT  " {
                    wrong_ps = true;
                }
                let rt = &dec.groups.rt;
                if rt.len() > 11
                    || rt
                        .chars()
                        .zip("Hello world".chars())
                        .any(|(a, b)| a != b && a != ' ')
                {
                    if !wrong_rt {
                        eprintln!("wrong RT {rt:?} at group {k}: {:?}", dec.monitor.last());
                    }
                    wrong_rt = true;
                }
            }
        }
        Run {
            groups: dec.groups.groups,
            wrong_ps,
            wrong_rt,
            ps: dec.groups.ps.clone(),
            rt: dec.groups.rt.clone(),
        }
    }

    #[test]
    fn one_error_per_group_at_each_level() {
        let list = test_groups();
        for case in ['a', 'b', 'c'] {
            for level in 0..=2u8 {
                let mut dec = RdsDecoder::new(
                    Correction {
                        level,
                        voting: false,
                    },
                    RdsCharset::Auto,
                );
                dec.keep_monitor = true;
                let mut rng = Rng::new(11 + level as u64);
                let r = run(&mut dec, 400, &list, |k, bits, rel| {
                    if k < 20 {
                        return;
                    }
                    let block = rng.below(4) as usize;
                    let base = block * 26;
                    match case {
                        'a' => {
                            let i = rng.below(26) as usize;
                            bits[base + i] ^= true;
                        }
                        'b' => {
                            let i = rng.below(25) as usize;
                            bits[base + i] ^= true;
                            bits[base + i + 1] ^= true;
                        }
                        _ => {
                            let i = rng.below(14) as usize;
                            let j = i + 2 + 5 + rng.below(4) as usize; // 5 to 8 bits apart
                            for (p, r) in [(i, 0.1f32), (j, 0.15)] {
                                bits[base + p] ^= true;
                                bits[base + p + 1] ^= true;
                                rel[base + p] = r;
                                rel[base + p + 1] = r;
                            }
                        }
                    }
                });
                assert!(!r.wrong_ps, "case {case} level {level}: wrong PS shown");
                assert!(!r.wrong_rt, "case {case} level {level}: wrong RT shown");
                let repairable = match case {
                    'a' | 'b' => level >= 1,
                    _ => level >= 2,
                };
                if repairable {
                    let need = if case == 'c' { 350 } else { 390 };
                    assert!(
                        r.groups >= need,
                        "case {case} level {level}: {} groups",
                        r.groups
                    );
                    assert_eq!(r.ps, "SDRKIT  ");
                    assert_eq!(r.rt, "Hello world");
                } else {
                    assert!(
                        r.groups < 300,
                        "case {case} level {level}: {} groups",
                        r.groups
                    );
                }
            }
        }
    }

    #[test]
    fn random_bits_give_no_text() {
        let mut dec = RdsDecoder::default();
        let mut rng = Rng::new(5);
        for _ in 0..200_000 {
            dec.push_bit(rng.next_u64() & 1 == 1, 0.2 + rng.uniform() as f32, 0.0);
        }
        assert_eq!(dec.groups.ps, "");
        assert_eq!(dec.groups.rt, "");
    }

    /// Soft random errors: a wrong bit mostly has a low reliability.
    fn soft_errors(rng: &mut Rng, ber: f64, bits: &mut [bool; 104], rel: &mut [f32; 104]) {
        for i in 0..104 {
            if rng.uniform() < ber {
                bits[i] ^= true;
                rel[i] = (0.05 + 0.4 * rng.uniform()) as f32;
            } else {
                rel[i] = (0.3 + 1.2 * rng.uniform()) as f32;
            }
        }
    }

    #[test]
    fn voting() {
        let list = test_groups();
        let count_wrong = |dec: &RdsDecoder| {
            let ps = dec.groups.ps.as_bytes();
            ps.iter().zip(b"SDRKIT  ").filter(|(a, b)| a != b).count()
        };
        for ber in [0.03, 0.05, 0.08] {
            let mut results = Vec::new();
            for voting in [false, true] {
                let mut dec = RdsDecoder::new(Correction { level: 2, voting }, RdsCharset::Auto);
                let mut rng = Rng::new((ber * 1000.0) as u64);
                let mut wrong_positions = 0usize;
                let mut wrong_ps = false;
                for k in 0..1400 {
                    let w = words(&list[k % list.len()]);
                    let mut bits = [false; 104];
                    let mut rel = [1.0f32; 104];
                    for (bi, word) in w.iter().enumerate() {
                        for i in 0..26 {
                            bits[bi * 26 + i] = word >> (25 - i) & 1 == 1;
                        }
                    }
                    soft_errors(&mut rng, ber, &mut bits, &mut rel);
                    for i in 0..104 {
                        dec.push_bit(bits[i], rel[i], 0.0);
                    }
                    let wrong = count_wrong(&dec);
                    wrong_positions += if dec.groups.ps.is_empty() { 0 } else { wrong };
                    wrong_ps |= wrong > 0 && !dec.groups.ps.is_empty();
                }
                results.push((
                    dec.groups.groups,
                    dec.blocks.stats().voted,
                    wrong_positions,
                    wrong_ps,
                    dec.groups.ps.clone(),
                    dec.groups.rt.clone(),
                ));
            }
            let (g_off, v_off, w_off, _, _, _) = results[0].clone();
            let (g_on, _, w_on, wrong_on, ps_on, rt_on) = results[1].clone();
            assert_eq!(v_off, 0, "voting off must not vote");
            assert!(
                g_on >= g_off,
                "BER {ber}: voting lost groups ({g_on} < {g_off})"
            );
            if ber >= 0.05 {
                assert!(
                    g_on as f64 > g_off as f64 * 1.05,
                    "BER {ber}: voting gained too little ({g_on} vs {g_off})"
                );
            }
            if ber <= 0.05 {
                assert!(!wrong_on, "BER {ber}: wrong PS with voting");
                assert_eq!(ps_on, "SDRKIT  ", "BER {ber}");
                assert_eq!(rt_on, "Hello world", "BER {ber}");
            }
            assert!(
                w_on as f64 <= w_off as f64 * 1.2 + 50.0,
                "BER {ber}: {w_on} wrong positions against {w_off}"
            );
        }
        // Random bits: at most one voted block and no text.
        let mut dec = RdsDecoder::default();
        let mut rng = Rng::new(99);
        for _ in 0..2_000_000 {
            dec.push_bit(rng.next_u64() & 1 == 1, (0.05 + rng.uniform()) as f32, 0.0);
        }
        assert!(
            dec.blocks.stats().voted <= 1,
            "{} voted blocks",
            dec.blocks.stats().voted
        );
        assert_eq!(dec.groups.ps, "");
        assert_eq!(dec.groups.rt, "");
    }

    #[test]
    fn sync_is_lost_after_24_bad_blocks_and_data_is_kept() {
        let list = test_groups();
        let mut dec = RdsDecoder::new(
            Correction {
                level: 0,
                voting: false,
            },
            RdsCharset::Auto,
        );
        run(&mut dec, 14, &list, |_, _, _| {});
        assert!(dec.blocks.synced());
        assert_eq!(dec.groups.ps, "SDRKIT  ");
        let mut rng = Rng::new(3);
        for _ in 0..26 * 24 {
            dec.push_bit(rng.next_u64() & 1 == 1, 1.0, 0.0);
        }
        assert!(!dec.blocks.synced());
        assert_eq!(dec.blocks.quality(), 0.0);
        assert_eq!(dec.groups.ps, "SDRKIT  ");
    }
}
