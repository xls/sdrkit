//! The tests of sdrkit-proto-pager, in one program. Known answers come from ITU-R M.584-2 (annex 1 for POCSAG, annex 2 for the FLEX
//! code word, checksum and interleaving) and ARIB RCR STD-43A (FLEX field layouts, address ranges and the message checks,
//! with its worked examples); other FLEX parts are tested by round trip.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ARMED.try_with(|a| {
            if a.get() {
                let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            }
        });
        // SAFETY: forwards the caller's request to the system allocator unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `alloc` above with the same layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Counts the allocations that this thread makes in `f`.
fn allocations(f: impl FnOnce()) -> usize {
    COUNT.with(|c| c.set(0));
    ARMED.with(|a| a.set(true));
    f();
    ARMED.with(|a| a.set(false));
    COUNT.with(|c| c.get())
}

/// A small seeded generator for test patterns.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u32) -> u32 {
        (self.next() % n as u64) as u32
    }
}

mod word_tests {
    use super::Rng;
    use sdrkit_proto_pager::pocsag::{IDLE, IDLE_AS_PRINTED, SYNC};
    use sdrkit_proto_pager::word::{self, Checked, SoftRepair};

    #[test]
    fn sync_and_idle_are_reproduced_from_their_information_bits() {
        // M.584-2 annex 1 Tables 1 and 2: the encoder must give the printed check and parity bits, not only a code word.
        assert_eq!(SYNC, 0b0111_1100_1101_0010_0001_0101_1101_1000);
        assert_eq!(word::encode(word::data(SYNC)), SYNC);
        assert_eq!(word::encode(word::data(IDLE)), IDLE);
        // Table 2 prints 0x7AC9C197, which fails the clause 1.4 code.
        assert!(!word::is_valid(IDLE_AS_PRINTED));
        assert_eq!((IDLE ^ IDLE_AS_PRINTED).count_ones(), 1);
    }

    #[test]
    fn check_bits_follow_the_clause_1_4_division() {
        // Information 1 is x^10. x^10 mod (x^10 + x^9 + x^8 + x^6 + x^5 + x^3 + 1) = x^9 + x^8 + x^6 + x^5 + x^3 + 1 (0x369).
        // The word has 7 ones before the parity bit, so the parity bit is 1.
        assert_eq!(word::encode(1), (1 << 11) | (0x369 << 1) | 1);
        assert_eq!(word::encode(0), 0);
        assert_eq!(word::code().radius(), 2);
    }

    /// A direct implementation of the repair rule, to compare with the shared code.
    fn trunk_rule(cw: u32, sent: u32) -> Option<(u32, u8)> {
        let errors = (cw ^ sent).count_ones() as u8;
        (errors <= 2).then_some((sent, errors))
    }

    #[test]
    fn hard_repair_matches_the_copies_it_replaces() {
        let mut r = Rng(1);
        for _ in 0..200 {
            let cw = word::encode(r.next() as u32);
            assert!(word::is_valid(cw));
            assert_eq!(word::repair(cw), Some((cw, 0)));
            for i in 0..32 {
                assert_eq!(word::repair(cw ^ (1 << i)), trunk_rule(cw ^ (1 << i), cw));
                for j in i + 1..32 {
                    let bad = cw ^ (1 << i) ^ (1 << j);
                    assert_eq!(word::repair(bad), trunk_rule(bad, cw));
                }
            }
            for _ in 0..40 {
                let a = r.below(32);
                let mut b = r.below(32);
                while b == a {
                    b = r.below(32);
                }
                let mut c = r.below(32);
                while c == a || c == b {
                    c = r.below(32);
                }
                assert_eq!(word::repair(cw ^ (1 << a) ^ (1 << b) ^ (1 << c)), None);
            }
        }
    }

    #[test]
    fn chase_repairs_three_weak_bits_and_never_a_confident_one() {
        let cw = word::encode(0x12345);
        let bad = cw ^ (1 << 3) ^ (1 << 17) ^ (1 << 29);
        assert_eq!(word::repair(bad), None);
        let mut rel = [1.0f32; 32];
        for b in [3, 17, 29] {
            rel[31 - b] = 0.1;
        }
        rel[5] = 0.2;
        rel[6] = 0.3;
        rel[7] = 0.3;
        assert_eq!(
            word::repair_soft(bad, &rel, SoftRepair::MESSAGE),
            Some((cw, 3))
        );
        let mut rel2 = rel;
        rel2[31 - 29] = 2.0;
        assert_ne!(
            word::repair_soft(bad, &rel2, SoftRepair::MESSAGE).map(|r| r.0),
            Some(cw)
        );
        // The header limit is lower.
        assert_eq!(
            word::repair_soft(bad, &rel, SoftRepair::HEADER),
            Some((cw, 3))
        );
        let mut rel3 = rel;
        rel3[31 - 3] = 0.8;
        assert_eq!(word::repair_soft(bad, &rel3, SoftRepair::HEADER), None);
        assert!(Checked::soft(bad, Some(&rel), SoftRepair::MESSAGE).valid);
    }

    #[test]
    fn llrs_give_hard_bits_and_reliabilities() {
        let mut llr = [2.0f32; 32];
        llr[0] = -1.5;
        llr[31] = f32::NAN;
        let (w, rel) = word::from_llrs(&llr);
        assert_eq!(w, 0x8000_0000);
        assert_eq!((rel[0], rel[31]), (1.5, 0.0));
        assert_eq!(word::rev21(1), 1 << 20);
        assert_eq!(word::rev21(word::rev21(0x12345)), 0x12345);
    }
}

mod vote_tests {
    use super::Rng;
    use sdrkit_proto_pager::vote::vote;
    use sdrkit_proto_pager::word::{self, Checked};

    fn wreck(r: &mut Rng, w: u32, n: u32) -> u32 {
        let mut x = w;
        let mut done = 0;
        while done < n {
            let b = r.below(32);
            if (x ^ w) & (1 << b) == 0 {
                x ^= 1 << b;
                done += 1;
            }
        }
        x
    }

    #[test]
    fn copies_give_back_a_lost_word() {
        let mut r = Rng(7);
        let w = word::encode(0x1ABCDE);
        assert_eq!(vote(&[Checked::hard(w)]).unwrap().cw, w);
        let copies = [
            Checked::hard(wreck(&mut r, w, 5)),
            Checked::hard(w),
            Checked::hard(wreck(&mut r, w, 6)),
        ];
        assert_eq!(vote(&copies).unwrap().cw, w);
        let mut ok3 = 0;
        for _ in 0..200 {
            let c: Vec<Checked> = (0..3).map(|_| Checked::hard(wreck(&mut r, w, 3))).collect();
            if vote(&c).map(|x| x.cw) == Some(w) {
                ok3 += 1;
            }
        }
        assert!(ok3 >= 100, "{ok3}");
        let (mut ok2, mut wrong2) = (0, 0);
        for _ in 0..300 {
            let c: Vec<Checked> = (0..2).map(|_| Checked::hard(wreck(&mut r, w, 3))).collect();
            match vote(&c).map(|x| x.cw) {
                Some(x) if x == w => ok2 += 1,
                Some(_) => wrong2 += 1,
                None => {}
            }
        }
        assert!(ok2 >= 20, "{ok2}");
        assert!(wrong2 * 20 <= ok2.max(1), "{wrong2} wrong of {ok2}");
        assert_eq!(vote(&[]), None);
    }
}

mod pocsag_tests {
    use super::{allocations, Rng};
    use sdrkit_proto_pager::charset::NumericSet;
    use sdrkit_proto_pager::page::{looks_like_text, Reason};
    use sdrkit_proto_pager::pocsag::{
        self, address_of, address_word, encode_batches, message_word, transmission_bits, Config,
        Content, PocsagAssembler, PocsagDecoder, PocsagTx, IDLE, SYNC,
    };
    use sdrkit_proto_pager::word;
    use sdrkit_proto_pager::{Error, Kind, Page, ReadMode};

    fn decode(words: &[u32], cfg: Config) -> Vec<Page> {
        let mut bits = Vec::new();
        transmission_bits(words, pocsag::PREAMBLE_BITS, &mut bits);
        bits.extend(std::iter::repeat_n(0, 64));
        let mut rx = PocsagDecoder::new(cfg).unwrap();
        let mut pages = Vec::new();
        for &b in &bits {
            rx.push_bit(b, 1.0, |p| pages.push(p.clone()));
        }
        rx.finish(2.0, |p| pages.push(p.clone()));
        pages
    }

    fn assemble(words: &[u32], asm: &mut PocsagAssembler) -> Vec<Page> {
        let mut pages = Vec::new();
        let mut slot = 0;
        for &w in words {
            if w == SYNC {
                slot = 0;
                continue;
            }
            asm.push(slot, w, None, 0.0, |p| pages.push(p.clone()));
            slot += 1;
        }
        asm.end(0.0, |p| pages.push(p.clone()));
        pages
    }

    #[test]
    fn address_word_follows_clause_1_3_2() {
        // RIC 1234567 = 0x12D687: the 18 high bits 0x25AD0 after a 0 flag, then function 3. Frame 7 = slots 14 and 15.
        let cw = address_word(1_234_567, 3).unwrap();
        assert_eq!(word::data(cw), (0x25AD0 << 2) | 3);
        assert_eq!(cw >> 31, 0);
        assert_eq!(address_of(cw, 14), (1_234_567, 3));
        assert_eq!(address_of(cw, 15), (1_234_567, 3));
        assert_eq!(address_word(1 << 21, 0), Err(Error::RicTooLarge(1 << 21)));
        assert_eq!(address_word(5, 4), Err(Error::BadFunction(4)));
    }

    #[test]
    fn numeric_and_text_bits_follow_clauses_2_1_and_2_2() {
        // "12345": each character's bit 1 (least significant) first, 5 characters per word.
        // 1 = 1000, 2 = 0100, 3 = 1100, 4 = 0010, 5 = 1010.
        let data = 0b1000_0100_1100_0010_1010;
        let mut words = Vec::new();
        encode_batches(
            &[PocsagTx {
                ric: 8,
                function: 0,
                content: Content::Numeric("12345"),
            }],
            NumericSet::Itu,
            &mut words,
        )
        .unwrap();
        assert_eq!(words[0], SYNC);
        assert_eq!(words[1], address_word(8, 0).unwrap());
        assert_eq!(words[2], message_word(data));
        // A text "A" (0x41 = 1000001 sent 1,0,0,0,0,0,1), then EOT (0x04 sent 0,0,1,0,0,0,0), then zero fill.
        let mut words = Vec::new();
        encode_batches(
            &[PocsagTx {
                ric: 8,
                function: 3,
                content: Content::Alpha("A"),
            }],
            NumericSet::Itu,
            &mut words,
        )
        .unwrap();
        #[allow(clippy::unusual_byte_groupings)] // grouped by character: A, EOT, fill
        let want = 0b1000001_0010000_000000;
        assert_eq!(words[2], message_word(want));
        // At least one idle word after a message before the next address (clause 1.2).
        assert_eq!(words[3], IDLE);
    }

    #[test]
    fn three_pages_and_one_repaired_bit() {
        let tx = [
            PocsagTx {
                ric: 1_234_567,
                function: 3,
                content: Content::Alpha("Hello world"),
            },
            PocsagTx {
                ric: 1_234_560,
                function: 0,
                content: Content::Numeric("0123-456"),
            },
            PocsagTx {
                ric: 7,
                function: 1,
                content: Content::Tone,
            },
        ];
        let mut w = Vec::new();
        encode_batches(&tx, NumericSet::Itu, &mut w).unwrap();
        let k = w
            .iter()
            .position(|&x| x != SYNC && x != IDLE && x >> 31 == 1)
            .unwrap();
        w[k] ^= 1 << 9;
        let pages = decode(&w, Config::default());
        assert_eq!(pages.len(), 3);
        assert_eq!(
            (pages[0].address, pages[0].function, pages[0].kind),
            (1_234_567, Some(3), Kind::Alpha)
        );
        assert_eq!(pages[0].text, "Hello world");
        assert_eq!(pages[0].bits_repaired, 1);
        assert_eq!(
            (pages[1].address, pages[1].kind),
            (1_234_560, Kind::Numeric)
        );
        assert_eq!(pages[1].text, "0123-456");
        assert_eq!(
            (pages[2].address, pages[2].function, pages[2].kind),
            (7, Some(1), Kind::Tone)
        );
        assert!(pages.iter().all(|p| !p.damaged && p.rate == "POCSAG 1200"));
    }

    #[test]
    fn inverted_and_soft_input() {
        let tx = [PocsagTx {
            ric: 4242,
            function: 3,
            content: Content::Alpha("Inverted and soft"),
        }];
        let mut w = Vec::new();
        encode_batches(&tx, NumericSet::Itu, &mut w).unwrap();
        let mut bits = Vec::new();
        transmission_bits(&w, 576, &mut bits);
        // Inverted polarity (a bit 1 sent as a positive LLR), with three weak wrong bits in one message word.
        let mut llr: Vec<f32> = bits
            .iter()
            .map(|&b| if b == 1 { 1.0 } else { -1.0 })
            .collect();
        let msg_index = w
            .iter()
            .position(|&x| x != SYNC && x != IDLE && x >> 31 == 1)
            .unwrap();
        let base = 576 + msg_index * 32;
        for b in [2, 9, 20] {
            llr[base + b] = -llr[base + b] * 0.1;
        }
        let mut rx = PocsagDecoder::new(Config::default()).unwrap();
        let mut pages = Vec::new();
        for &l in &llr {
            rx.push_llr(l, 0.0, |p| pages.push(p.clone()));
        }
        rx.finish(0.0, |p| pages.push(p.clone()));
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].text, "Inverted and soft");
        assert_eq!(pages[0].bits_repaired, 3);
    }

    #[test]
    fn two_damaged_copies_make_one_good_page() {
        let text = "Ambulance to 12 Main Street, urgent";
        let tx = [PocsagTx {
            ric: 4242,
            function: 3,
            content: Content::Alpha(text),
        }];
        let mut w = Vec::new();
        encode_batches(&tx, NumericSet::Itu, &mut w).unwrap();
        let msg: Vec<usize> = (0..w.len())
            .filter(|&i| w[i] >> 31 == 1 && w[i] != IDLE && w[i] != SYNC)
            .collect();
        let mut asm = PocsagAssembler::new(Config::default()).unwrap();
        let mut a = w.clone();
        a[msg[1]] ^= 0b10101;
        let p1 = assemble(&a, &mut asm);
        assert!(p1[0].damaged);
        assert_eq!(p1[0].lost_words, 1);
        assert!(p1[0].text.contains('?'), "{}", p1[0].text);
        let mut b = w.clone();
        b[msg[3]] ^= 0b11100;
        let p2 = assemble(&b, &mut asm);
        assert!(!p2[0].damaged, "{:?}", p2[0]);
        assert!(p2[0].combined);
        assert_eq!(p2[0].copies, 2);
        assert_eq!(p2[0].text, text);
        // Another RIC is never combined.
        let mut other = Vec::new();
        encode_batches(
            &[PocsagTx {
                ric: 4250,
                function: 3,
                content: Content::Alpha(text),
            }],
            NumericSet::Itu,
            &mut other,
        )
        .unwrap();
        let m2: Vec<usize> = (0..other.len())
            .filter(|&i| other[i] >> 31 == 1 && other[i] != IDLE && other[i] != SYNC)
            .collect();
        other[m2[1]] ^= 0b10101;
        let p3 = assemble(&other, &mut asm);
        assert!(p3[0].damaged && !p3[0].combined);
    }

    #[test]
    fn a_lost_word_is_marked_not_read_as_zeros() {
        // The trunk copy filled a lost word with zero bits, which reads as "00000" in a numeric page.
        let tx = [PocsagTx {
            ric: 1_234_000,
            function: 1,
            content: Content::Numeric("19533  38212345"),
        }];
        let mut w = Vec::new();
        encode_batches(&tx, NumericSet::Parentheses, &mut w).unwrap();
        let msg: Vec<usize> = (0..w.len())
            .filter(|&i| w[i] >> 31 == 1 && w[i] != IDLE && w[i] != SYNC)
            .collect();
        w[msg[1]] ^= 0b111;
        let cfg = Config {
            read: ReadMode::Numeric,
            numeric: NumericSet::Parentheses,
            ..Config::default()
        };
        let pages = decode(&w, cfg);
        assert_eq!(pages[0].text, "19533?????12345");
        assert_eq!(pages[0].reason, Reason::Forced);
    }

    #[test]
    fn two_lost_words_in_a_row_end_the_page() {
        // M.584-2 annex 1 clause 2.5.1.
        let tx = [PocsagTx {
            ric: 16,
            function: 0,
            content: Content::Numeric("11111222223333344444"),
        }];
        let mut w = Vec::new();
        encode_batches(&tx, NumericSet::Itu, &mut w).unwrap();
        let msg: Vec<usize> = (0..w.len())
            .filter(|&i| w[i] >> 31 == 1 && w[i] != IDLE && w[i] != SYNC)
            .collect();
        w[msg[1]] ^= 0b111;
        w[msg[2]] ^= 0b111;
        let cfg = Config {
            read: ReadMode::Numeric,
            combine: false,
            ..Config::default()
        };
        let pages = decode(&w, cfg.clone());
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].text, "11111");
        assert!(pages[0].damaged);
        assert_eq!(pages[0].lost_words, 2);
        let pages = decode(
            &w,
            Config {
                end_after_lost: 0,
                ..cfg
            },
        );
        assert_eq!(pages[0].text, "11111??????????44444");
    }

    #[test]
    fn a_word_near_the_idle_word_ends_the_page() {
        let tx = [PocsagTx {
            ric: 16,
            function: 0,
            content: Content::Numeric("12345"),
        }];
        let mut w = Vec::new();
        encode_batches(&tx, NumericSet::Itu, &mut w).unwrap();
        let cfg = Config {
            read: ReadMode::Numeric,
            combine: false,
            ..Config::default()
        };
        // The idle word after the message with 4 wrong bits (5 when no repair passes) is still the idle word.
        let at = (0..w.len())
            .find(|&i| w[i] == IDLE && w[i - 1] >> 31 == 1)
            .unwrap();
        for flips in [0b1111u32, 0x8000_000F] {
            let mut bad = w.clone();
            bad[at] ^= flips;
            let wrong = bad[at];
            let mut asm = PocsagAssembler::new(cfg.clone()).unwrap();
            let pages = assemble(&bad, &mut asm);
            assert_eq!(pages.len(), 1, "{wrong:08x}");
            assert_eq!(pages[0].text, "12345");
        }
    }

    #[test]
    fn an_address_with_two_repaired_bits_needs_a_message_word() {
        let tx = [
            PocsagTx {
                ric: 16,
                function: 0,
                content: Content::Numeric("12345"),
            },
            PocsagTx {
                ric: 1234,
                function: 2,
                content: Content::Tone,
            },
        ];
        let mut w = Vec::new();
        encode_batches(&tx, NumericSet::Itu, &mut w).unwrap();
        let addr = |ric| {
            let a = address_word(ric, if ric == 16 { 0 } else { 2 }).unwrap();
            w.iter().position(|&x| x == a).unwrap()
        };
        let (numeric, tone) = (addr(16), addr(1234));
        let cfg = Config {
            read: ReadMode::Numeric,
            combine: false,
            ..Config::default()
        };
        // Eight bits of each word are weaker than the bits changed below, and the others are too sure for a soft repair.
        let mut rel = [2.0f32; 32];
        rel[..8].fill(0.3);
        let run = |w: &[u32]| {
            let mut asm = PocsagAssembler::new(cfg.clone()).unwrap();
            let mut pages = 0;
            let mut slot = 0;
            for &x in w {
                if x == SYNC {
                    slot = 0;
                    continue;
                }
                asm.push(slot, x, Some(&rel), 0.0, |_| pages += 1);
                slot += 1;
            }
            asm.end(0.0, |_| pages += 1);
            (pages, asm.refusals())
        };
        assert_eq!(run(&w).0, 2);
        // With hard input the tone page stays: no bit is known to be weak.
        let mut bad = w.clone();
        bad[tone] ^= 0x0001_0100;
        let mut asm = PocsagAssembler::new(cfg.clone()).unwrap();
        assert_eq!(assemble(&bad, &mut asm).len(), 2);
        // Two repaired bits in each address word: the numeric page has a message word that passed; the tone page has none,
        // and its repaired bits are not among the weakest.
        let mut bad = w.clone();
        bad[numeric] ^= 0x0001_0100;
        bad[tone] ^= 0x0001_0100;
        let (n, r) = run(&bad);
        assert_eq!(n, 1);
        assert_eq!((r.total, r.unconfirmed), (1, 1));
        // A lost first message word after an address with two repaired bits: refused too.
        let mut bad = w.clone();
        bad[numeric] ^= 0x0001_0100;
        bad[numeric + 1] ^= 0b111;
        assert_eq!(run(&bad).1.total, 1);
        // One repaired bit: both pages.
        let mut bad = w.clone();
        bad[numeric] ^= 0x100;
        bad[tone] ^= 0x100;
        assert_eq!(run(&bad), (2, Default::default()));
    }

    #[test]
    fn the_encoder_refuses_what_it_cannot_send() {
        let mut out = Vec::new();
        let bad = |content| {
            [PocsagTx {
                ric: 1,
                function: 0,
                content,
            }]
        };
        assert_eq!(
            encode_batches(&bad(Content::Numeric("12A")), NumericSet::Itu, &mut out),
            Err(Error::BadCharacter('A'))
        );
        assert_eq!(
            encode_batches(&bad(Content::Alpha("Grüße")), NumericSet::Itu, &mut out),
            Err(Error::BadCharacter('ü'))
        );
        assert_eq!(
            encode_batches(
                &[PocsagTx {
                    ric: 1 << 21,
                    function: 0,
                    content: Content::Tone
                }],
                NumericSet::Itu,
                &mut out
            ),
            Err(Error::RicTooLarge(1 << 21))
        );
        assert!(out.is_empty());
        // The RIC that the trunk encoder sent as a message word (bit 20 set by an oversized RIC).
        assert_eq!(address_word(pocsag::MAX_RIC, 3).unwrap() >> 31, 0);
    }

    #[test]
    fn readings_and_read_modes() {
        assert!(looks_like_text(b"QD4639 Segment01 Segment02"));
        assert!(looks_like_text(b"Test page"));
        assert!(!looks_like_text(b"x}Q~w|"));
        use sdrkit_proto_pager::page::choose_reading;
        assert_eq!(
            choose_reading(ReadMode::Auto, 0, "123", false, None),
            (false, Reason::Auto)
        );
        assert_eq!(
            choose_reading(ReadMode::Auto, 3, "1*U]", false, None),
            (true, Reason::Auto)
        );
        assert_eq!(
            choose_reading(ReadMode::Function, 0, "1", true, None),
            (false, Reason::Function)
        );
        assert_eq!(
            choose_reading(ReadMode::Numeric, 3, "1", true, Some(true)),
            (true, Reason::Address)
        );
        assert_eq!(ReadMode::by_name("function"), Some(ReadMode::Function));
        assert_eq!(ReadMode::by_name("bogus"), None);
        assert_eq!(
            NumericSet::by_name("parentheses"),
            Some(NumericSet::Parentheses)
        );
    }

    #[test]
    fn all_rates_and_false_syncs() {
        for rate in pocsag::Rate::ALL {
            let tx = [PocsagTx {
                ric: 99,
                function: 3,
                content: Content::Alpha("rate"),
            }];
            let mut w = Vec::new();
            encode_batches(&tx, NumericSet::Itu, &mut w).unwrap();
            let pages = decode(
                &w,
                Config {
                    rate,
                    ..Config::default()
                },
            );
            assert_eq!(pages[0].rate, rate.name());
        }
        // A sync word followed by noise returns no page and no false transmission.
        let mut r = Rng(3);
        let mut bits = Vec::new();
        transmission_bits(&[SYNC], 0, &mut bits);
        bits.extend((0..32 * 40).map(|_| (r.next() & 1) as u8));
        let mut rx = PocsagDecoder::new(Config::default()).unwrap();
        let mut n = 0;
        for &b in &bits {
            rx.push_bit(b, 0.0, |_| n += 1);
        }
        assert_eq!(n, 0);
    }

    #[test]
    fn soft_sync_finds_a_sync_word_with_weak_errors() {
        // Four weak sign errors in the sync word: beyond the 2 hard errors allowed, inside the soft correlation.
        let tx = [PocsagTx {
            ric: 4242,
            function: 3,
            content: Content::Alpha("weak sync"),
        }];
        let mut w = Vec::new();
        encode_batches(&tx, NumericSet::Itu, &mut w).unwrap();
        let mut bits = Vec::new();
        transmission_bits(&w, 576, &mut bits);
        let mut llr: Vec<f32> = bits
            .iter()
            .map(|&b| if b == 1 { -1.0 } else { 1.0 })
            .collect();
        for k in [3, 11, 19, 27] {
            llr[576 + k] *= -0.1;
        }
        let hard: Vec<u8> = llr.iter().map(|&l| (l < 0.0) as u8).collect();
        let mut got = 0;
        let mut rx = PocsagDecoder::new(Config::default()).unwrap();
        for &b in &hard {
            rx.push_bit(b, 0.0, |_| got += 1);
        }
        rx.finish(0.0, |_| got += 1);
        assert_eq!(got, 0);
        let mut texts = Vec::new();
        let mut rx = PocsagDecoder::new(Config::default()).unwrap();
        for &l in &llr {
            rx.push_llr(l, 0.0, |p| texts.push(p.text.clone()));
        }
        rx.finish(0.0, |p| texts.push(p.text.clone()));
        assert_eq!(texts, ["weak sync"]);
    }

    #[test]
    fn bad_settings_are_refused() {
        for cfg in [
            Config {
                sync_errors: 16,
                ..Config::default()
            },
            Config {
                first_batch_valid: 17,
                ..Config::default()
            },
            Config {
                soft_sync_score: 1.5,
                ..Config::default()
            },
            Config {
                max_words: 0,
                ..Config::default()
            },
            Config {
                max_age_s: f64::NAN,
                ..Config::default()
            },
        ] {
            assert!(matches!(PocsagDecoder::new(cfg), Err(Error::BadSetting(_))));
        }
    }

    #[test]
    fn steady_state_decoding_does_not_allocate() {
        let tx = [
            PocsagTx {
                ric: 1_234_567,
                function: 3,
                content: Content::Alpha("Steady state test message"),
            },
            PocsagTx {
                ric: 222,
                function: 0,
                content: Content::Numeric("0123456789"),
            },
        ];
        let mut w = Vec::new();
        encode_batches(&tx, NumericSet::Itu, &mut w).unwrap();
        w[5] ^= 0b11; // a repaired word
        let mut bits = Vec::new();
        transmission_bits(&w, 576, &mut bits);
        bits.extend(std::iter::repeat_n(0, 64));
        let llr: Vec<f32> = bits
            .iter()
            .map(|&b| if b == 1 { -1.0 } else { 1.0 })
            .collect();
        let mut rx = PocsagDecoder::new(Config::default()).unwrap();
        // Warm up: the history fills, the strings grow to their size.
        for _ in 0..40 {
            for &l in &llr {
                rx.push_llr(l, 0.0, |_| {});
            }
        }
        let mut pages = 0;
        let n = allocations(|| {
            for _ in 0..25 {
                for &l in &llr {
                    rx.push_llr(l, 0.0, |_| pages += 1);
                }
            }
        });
        assert_eq!(pages, 50);
        assert_eq!(n, 0);
    }
}

mod flex_tests {
    use super::allocations;
    use sdrkit_proto_pager::flex::{
        self, address, address_words, data_symbols, encode_phase, fragment_check, group_sum,
        header_bits, interleave, long_capcode, numeric_check, signature, split_phases, Address,
        Biw, Config, Fiw, FlexDecoder, FlexTx, Header, Special, Speed, Sync1Search, Vector,
        PHASE_BITS, PHASE_WORDS,
    };
    use sdrkit_proto_pager::{Error, Kind, Page};
    use sdrkit_util_bits::checksum::flex_check_ok;

    fn decode(words: &[u32; PHASE_WORDS], dec: &mut FlexDecoder) -> Vec<Page> {
        let mut out = Vec::new();
        dec.decode_words(Speed::F1600_2, Fiw::default(), 0, words, None, 0.0, |p| {
            out.push(p.clone())
        });
        out
    }

    #[test]
    fn checksum_follows_annex_2_clause_1_4_1() {
        // The binary sum of the nibbles a to d and bit e0, ones complemented, low 4 bits. Cycle 0, frame 40: value 0x2800,
        // nibbles 0, 8, 2, 0 and e0 = 0 sum to 10, so the checksum is 0xF - 10 = 5.
        let f = Fiw {
            cycle: 0,
            frame: 40,
            rest: 0,
        };
        assert_eq!(f.value(), 0x2805);
        assert!(flex_check_ok(f.value()));
        assert_eq!(Fiw::from_value(0x2805), Some(f));
        assert_eq!(Fiw::from_value(0x2804), None);
        let w = flex::word(f.value());
        assert_eq!(Fiw::from_word(w ^ 0x41, None, Default::default()), Some(f));
    }

    #[test]
    fn interleaving_puts_successive_bits_of_a_word_5_ms_apart() {
        // Annex 2 clause 1.3: 8 words a block, 5 ms between successive bits of a word (8 bits at 1600 bit/s).
        let mut words = [0u32; PHASE_WORDS];
        words[9] = 0xC000_0001; // word 1 of block 1: bits 0, 1 and 31
        let mut bits = vec![0u8; PHASE_BITS];
        interleave(&words, &mut bits).unwrap();
        let ones: Vec<usize> = (0..PHASE_BITS).filter(|&i| bits[i] == 1).collect();
        assert_eq!(ones, [256 + 1, 256 + 9, 256 + 31 * 8 + 1]);
    }

    #[test]
    fn speeds_and_sync_1() {
        assert_eq!(Speed::from_code(0x870C, 3), Some(Speed::F1600_2));
        assert_eq!(Speed::from_code(0x870C ^ 0x0101, 3), Some(Speed::F1600_2));
        assert_eq!(Speed::from_code(0x870C ^ 0x0FFF, 3), None);
        assert_eq!(Speed::from_code(0x4C7C, 0), Some(Speed::F6400_4));
        // The sync part is 115 ms: sync 1 and the FIW are 144 bits at 1600 bit/s, sync 2 is 25 ms (annex 2 Figure 3).
        let mut bits = Vec::new();
        header_bits(Speed::F3200_4, Fiw::default(), &mut bits);
        assert_eq!(bits.len(), 144);
        assert_eq!(Speed::F6400_4.data_symbols(), 5632);
        assert_eq!(Speed::F1600_2.data_symbols(), 2816);
        assert_eq!(Speed::F3200_2.sync2_symbols(), 80);
        for speed in Speed::ALL {
            for inverted in [false, true] {
                let mut bits = Vec::new();
                header_bits(
                    speed,
                    Fiw {
                        cycle: 3,
                        frame: 9,
                        rest: 0,
                    },
                    &mut bits,
                );
                let mut s = Sync1Search::new(4, 0.8).unwrap();
                let mut hit = None;
                for (i, &b) in bits.iter().enumerate() {
                    let b = if i == 40 { b ^ 1 } else { b };
                    if let Some(h) = s.push(b ^ inverted as u8) {
                        hit = Some((i + 1, h));
                    }
                }
                let (end, h) = hit.unwrap();
                assert_eq!(
                    (end, h.speed, h.inverted, h.errors),
                    (112, speed, inverted, 1)
                );
                let mut w = 0u32;
                for &b in &bits[112..144] {
                    w = (w << 1) | b as u32;
                }
                assert_eq!(
                    Fiw::from_word(w, None, Default::default()),
                    Some(Fiw {
                        cycle: 3,
                        frame: 9,
                        rest: 0
                    })
                );
            }
        }
    }

    #[test]
    fn soft_sync_1_and_bad_settings() {
        let mut bits = Vec::new();
        header_bits(Speed::F6400_4, Fiw::default(), &mut bits);
        let mut llr: Vec<f32> = bits
            .iter()
            .map(|&b| if b == 1 { 1.0 } else { -1.0 })
            .collect(); // inverted
        for k in (40..100).step_by(9) {
            llr[k] *= -0.05;
        }
        let mut s = Sync1Search::new(4, 0.8).unwrap();
        let hits: Vec<_> = llr
            .iter()
            .enumerate()
            .filter_map(|(i, &l)| s.push_llr(l).map(|h| (i + 1, h)))
            .collect();
        assert_eq!(hits.len(), 1);
        let (end, h) = hits[0];
        assert_eq!(
            (end, h.speed, h.inverted, h.errors),
            (112, Speed::F6400_4, true, 7)
        );
        assert!(Sync1Search::new(40, 0.8).is_err());
        assert!(Sync1Search::new(4, 1.5).is_err());
        assert!(FlexDecoder::new(Config {
            fragment_age_s: -1.0,
            ..Config::default()
        })
        .is_err());
    }

    #[test]
    fn addresses() {
        assert_eq!(address(0x8001), Address::Short(1));
        assert_eq!(address(0x1E_0000), Address::Short(flex::MAX_SHORT_CAPCODE));
        assert_eq!(address(0x8000), Address::LongFirst(0x8000));
        assert_eq!(address(0x1F_FFFF), Address::Idle);
        // RCR STD-43A table 3.8.1-1: the ranges of the address word.
        assert_eq!(address(0x1F_7FFF), Address::LongFirst(0x1F_7FFF));
        assert_eq!(address(0x1E_0001), Address::LongSecond(0x1E_0001));
        assert_eq!(address(0x1F_0000), Address::LongSecond(0x1F_0000));
        assert_eq!(address(0x1F_0001), Address::Reserved(0x1F_0001));
        assert_eq!(address(0x1F_7820), Address::Reserved(0x1F_7820));
        assert_eq!(
            address(0x1F_2800),
            Address::Special(Special::InformationService, 0x1F_2800)
        );
        assert_eq!(
            address(0x1F_6800),
            Address::Special(Special::Network, 0x1F_6800)
        );
        assert_eq!(
            address(0x1F_7800),
            Address::Special(Special::Temporary, 0x1F_7800)
        );
        assert_eq!(
            address(0x1F_781F),
            Address::Special(Special::OperatorMessaging, 0x1F_781F)
        );
        // The ends of the short range and of the long-address sets 1-2, 1-3 and 1-4, and 2-3 (TI SPRA193 2.9.4).
        for (cap, words) in [
            (1u32, (0x8001, None)),
            (1_933_312, (0x1E_0000, None)),
            (2_101_249, (1, Some(0x1F_FFFE))),
            (1_075_843_072, (0x8000, Some(0x1F_7FFF))),
            (1_075_843_073, (1, Some(0x1E_0001))),
            (3_223_326_720, (0x8000, Some(0x1F_0000))),
            (3_223_326_721, (0x1F_8001, Some(0x1E_0001))),
            (u32::MAX, (2_093_055, Some(1_998_784))),
        ] {
            assert_eq!(address_words(cap), Ok(words), "{cap}");
        }
        for cap in [
            2_101_249u32,
            2_200_000,
            2_068_480 + 32_768 * 5,
            1_075_843_073,
            2_000_000_000,
            3_300_000_000,
            u32::MAX,
        ] {
            let (w1, w2) = address_words(cap).unwrap();
            assert_eq!(long_capcode(w1, w2.unwrap()), Some(cap), "{cap}");
        }
        // Only the pairs 1-2, 1-3, 1-4 and 2-3 make a capcode; 0x1FFFFF is an idle word, not a second word.
        assert_eq!(long_capcode(5, 0x0F_FFFF), None);
        assert_eq!(long_capcode(5, 0x1F_FFFF), None);
        assert_eq!(long_capcode(5, 5), None);
        assert_eq!(long_capcode(0x1F_8000, 0x1F_8000), None);
        assert_eq!(long_capcode(0x1F_8000, 0x1E_8001), None);
        for cap in [
            0,
            flex::MAX_SHORT_CAPCODE + 1,
            2_000_000,
            2_068_481,
            flex::MIN_LONG_CAPCODE - 1,
        ] {
            assert_eq!(address_words(cap), Err(Error::BadCapcode(cap)));
        }
    }

    /// The worked examples of the checks: the documents' own and those computed from their rules, bit for bit.
    #[test]
    fn check_sums_follow_rcr_std_43a() {
        // RCR STD-43A p.32: FIW of cycle 3, frame 60, two transmissions: check A, the five groups sum to 21.
        let f = Fiw {
            cycle: 3,
            frame: 60,
            rest: 0b000110,
        };
        assert_eq!(f.value(), 0x3_3C3A);
        assert_eq!(Fiw::from_value(0x3_3C3A), Some(f));
        // Cycle 15 is not a cycle.
        let c15 = Fiw {
            cycle: 15,
            ..Fiw::default()
        };
        assert_eq!(Fiw::from_value(c15.value()), None);
        // RCR STD-43A p.45: numeric vector, b = 7, n = 4, K = E, check 8.
        let v = Vector::Numeric {
            kind: 3,
            start: 7,
            count: 4,
            check: 0xE,
        };
        assert_eq!(v.value() & 0xF, 8);
        assert_eq!(Vector::from_value(v.value()), Some(v));
        // RCR STD-43A p.54: message 32808590; groups 8C, 20, 00, 2B, 81, 19 sum to 0x171; K = 0x0D.
        let words = [0x00_208C, 0x19_812B];
        assert_eq!(group_sum(words[0]) + group_sum(words[1]), 0x171);
        assert_eq!(numeric_check(&words), 0x0D);
        let p = decode(
            &phase_with(
                100,
                Vector::Numeric {
                    kind: 3,
                    start: 3,
                    count: 2,
                    check: 0xD,
                },
                &words,
            ),
            &mut FlexDecoder::new(Config::default()).unwrap(),
        );
        assert_eq!(p[0].text, "32808590");
        // Text "HELLO", short address: header 001A22, signature 0B.
        let msg = [0x00_1800, 0x11_640B, 0x13_E64C];
        assert_eq!(fragment_check(&msg, 10), 0x222);
        assert_eq!(signature(*b"HELLO"), 0x0B);
        // Numeric "1234", short address: K = 0x2C, message word 010C86, vector word 1801BA.
        assert_eq!(numeric_check(&[0x01_0C84]), 0x2C);
        let v = Vector::Numeric {
            kind: 3,
            start: 3,
            count: 1,
            check: 0xC,
        };
        assert_eq!(v.value(), 0x18_01BA);
        let p = decode(
            &phase_with(100, v, &[0x01_0C86]),
            &mut FlexDecoder::new(Config::default()).unwrap(),
        );
        assert_eq!(p[0].text, "1234");
        // Text "Hi!" on a long address, N = 5, R = 1: header 08B932 in the second vector word; the vector word is not in
        // the sum and the header is; ETX is not in the signature.
        let msg = [0x08_B800, 0x1A_642D, 0x00_C1A1];
        assert_eq!(fragment_check(&msg, 10), 0x132);
        assert_eq!(
            fragment_check(&[0x08_B800, 0x00_D459, 0x1A_642D, 0x00_C1A1], 10),
            0x005
        );
        assert_eq!(!(group_sum(msg[1]) + group_sum(msg[2])) & 0x3FF, 0x1F2);
        assert_eq!(signature([0x48, 0x69, 0x21, 0x03, 0x03]), 0x2D);
        assert_eq!(Vector::Alpha { start: 40, len: 3 }.value(), 0x00_D459);
        let h = Header::from_value(0x08_B932);
        assert_eq!(
            (h.check, h.fragment, h.number, h.retrieval, h.continued),
            (0x132, 3, 5, true, false)
        );
        // Binary first fragment, data A5 3C, N = 9, R = 1, B = 0001: header 04ED66, signature 1E.
        assert_eq!(
            fragment_check(&[0x04_E000, 0x03_C011, 0x00_3CA5], 12),
            0xD66
        );
    }

    /// The encoder sends the worked examples word for word, and a long address decodes from its second vector word.
    #[test]
    fn the_encoder_sends_the_worked_examples() {
        let mut w = [0u32; PHASE_WORDS];
        let one = |kind, text| {
            [FlexTx {
                capcode: 100,
                kind,
                text,
            }]
        };
        encode_phase(&one(Kind::Alpha, "HELLO"), &mut w).unwrap();
        assert_eq!(
            w[3..6].iter().map(|&x| flex::value(x)).collect::<Vec<_>>(),
            [0x00_1A22, 0x11_640B, 0x13_E64C]
        );
        encode_phase(&one(Kind::Numeric, "1234"), &mut w).unwrap();
        assert_eq!(
            w[2..4].iter().map(|&x| flex::value(x)).collect::<Vec<_>>(),
            [0x18_01BA, 0x01_0C86]
        );
        // "Hi!" on a long address: vector 00D459, header 08B932 in the second vector word, text at words 40 and 41.
        let (w1, w2) = address_words(2_201_234).unwrap();
        let mut v: [u32; PHASE_WORDS] = std::array::from_fn(|i| flex::IDLE_VALUES[i % 2]);
        v[0] = Biw {
            count: 1,
            vector_start: 3,
            ..Biw::default()
        }
        .value();
        (v[1], v[2], v[3], v[4]) = (w1, w2.unwrap(), 0x00_D459, 0x08_B932);
        (v[40], v[41]) = (0x1A_642D, 0x00_C1A1);
        let p = decode(
            &v.map(flex::word),
            &mut FlexDecoder::new(Config::default()).unwrap(),
        );
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].address, p[0].text.as_str()), (2_201_234, "Hi!"));
        assert_eq!(p[0].flex.unwrap().message_number, 5);
    }

    /// In a frame whose number disagrees with the frame one frame time before it, a page with no message check is refused and a
    /// checked page is given.
    #[test]
    fn a_frame_out_of_sequence_keeps_only_checked_pages() {
        let tx = [
            FlexTx {
                capcode: 100,
                kind: Kind::Tone,
                text: "",
            },
            FlexTx {
                capcode: 101,
                kind: Kind::Alpha,
                text: "Hi",
            },
        ];
        let mut w = [0u32; PHASE_WORDS];
        encode_phase(&tx, &mut w).unwrap();
        let mut dec = FlexDecoder::new(Config::default()).unwrap();
        let mut at = |frame: u8, time: f64| {
            let mut got = Vec::new();
            let fiw = Fiw {
                cycle: 2,
                frame,
                rest: 0,
            };
            dec.decode_words(Speed::F1600_2, fiw, 0, &w, None, time, |p| {
                got.push(p.address)
            });
            got
        };
        assert_eq!(at(10, 0.0), [100, 101]);
        assert_eq!(at(11, flex::FRAME_S), [100, 101]);
        assert_eq!(at(15, 2.0 * flex::FRAME_S), [101]);
        // Two frame times later, or after a gap of many frames, the number is not compared.
        assert_eq!(at(17, 4.0 * flex::FRAME_S), [100, 101]);
        assert_eq!(at(90, 100.0), [100, 101]);
        assert_eq!((dec.refusals().total, dec.refusals().unconfirmed), (1, 1));
    }

    /// A page with one character changed and its code word encoded again passes the BCH code and fails its message check.
    #[test]
    fn a_changed_character_is_refused() {
        let tx = [
            FlexTx {
                capcode: 1_234_567,
                kind: Kind::Alpha,
                text: "Changed text",
            },
            FlexTx {
                capcode: 1_234_568,
                kind: Kind::Numeric,
                text: "0123456789",
            },
            FlexTx {
                capcode: 2_201_234,
                kind: Kind::Numeric,
                text: "12",
            },
        ];
        let mut w = [0u32; PHASE_WORDS];
        encode_phase(&tx, &mut w).unwrap();
        let mut dec = FlexDecoder::new(Config::default()).unwrap();
        assert_eq!(decode(&w, &mut dec).len(), 3);
        assert_eq!(dec.refusals().total, 0);
        // Four address words: the vectors at 5 to 8 (8 is the long address's message), the others from word 9 (text: 6
        // words, numeric: 2).
        for (at, gone) in [(10, 1_234_567), (15, 1_234_568), (8, 2_201_234)] {
            let mut bad = w;
            bad[at] = flex::word(flex::value(bad[at]) ^ 0x100);
            let mut dec = FlexDecoder::new(Config::default()).unwrap();
            let p = decode(&bad, &mut dec);
            assert_eq!(p.len(), 2, "word {at}");
            assert!(p.iter().all(|p| p.address != gone));
            assert_eq!(dec.refusals().total, 1);
            assert_eq!(dec.refusals().unconfirmed, 0);
        }
    }

    #[test]
    fn a_phase_round_trips() {
        let tx = [
            FlexTx {
                capcode: 1_234_567,
                kind: Kind::Alpha,
                text: "Test page 1600/2: hello",
            },
            FlexTx {
                capcode: 1_234_568,
                kind: Kind::Numeric,
                text: "555-0123 U 45",
            },
            FlexTx {
                capcode: 1_234_569,
                kind: Kind::Tone,
                text: "",
            },
            FlexTx {
                capcode: 2_201_234,
                kind: Kind::Alpha,
                text: "Long address 1600/2",
            },
            FlexTx {
                capcode: 2_201_235,
                kind: Kind::Numeric,
                text: "0042",
            },
        ];
        let mut w = [0u32; PHASE_WORDS];
        encode_phase(&tx, &mut w).unwrap();
        let mut dec = FlexDecoder::new(Config::default()).unwrap();
        let p = decode(&w, &mut dec);
        assert_eq!(p.len(), tx.len());
        for (a, b) in p.iter().zip(&tx) {
            assert_eq!(
                (a.address, a.kind, a.text.as_str()),
                (b.capcode, b.kind, b.text),
                "{a:?}"
            );
            assert!(!a.damaged);
        }
        assert_eq!(dec.counts().lost, 0);
        // An idle phase returns nothing.
        let idle: [u32; PHASE_WORDS] =
            std::array::from_fn(|i| if i % 2 == 0 { 0 } else { u32::MAX });
        assert!(decode(&idle, &mut dec).is_empty());
        // A vector word with a wrong checksum drops that page only.
        let mut bad = w;
        // Seven address words (two long), so the vector field starts at word 8.
        bad[8] = flex::word(flex::value(bad[8]) ^ 0x10);
        assert_eq!(
            decode(&bad, &mut FlexDecoder::new(Config::default()).unwrap()).len(),
            tx.len() - 1
        );
        // A block information word with a wrong checksum: nothing.
        let mut bad = w;
        bad[0] = flex::word(flex::value(bad[0]) ^ 0x10);
        assert!(decode(&bad, &mut FlexDecoder::new(Config::default()).unwrap()).is_empty());
        // The encoder refuses what it cannot send.
        assert_eq!(
            encode_phase(
                &[FlexTx {
                    capcode: 2_000_000,
                    kind: Kind::Tone,
                    text: ""
                }],
                &mut w
            ),
            Err(Error::BadCapcode(2_000_000))
        );
        assert_eq!(
            encode_phase(
                &[FlexTx {
                    capcode: 5,
                    kind: Kind::Numeric,
                    text: "12A"
                }],
                &mut w
            ),
            Err(Error::BadCharacter('A'))
        );
        let long = "x".repeat(400);
        assert_eq!(
            encode_phase(
                &[FlexTx {
                    capcode: 5,
                    kind: Kind::Alpha,
                    text: &long
                }],
                &mut w
            ),
            Err(Error::PhaseFull)
        );
        assert_eq!(
            encode_phase(
                &[FlexTx {
                    capcode: 5,
                    kind: Kind::Binary,
                    text: ""
                }],
                &mut w
            ),
            Err(Error::BadKind(Kind::Binary))
        );
    }

    /// Builds a phase by hand: one short address and its vector, then message words.
    fn phase_with(capcode: u32, vector: Vector, message: &[u32]) -> [u32; PHASE_WORDS] {
        let mut v = [0u32; PHASE_WORDS];
        for (i, x) in v.iter_mut().enumerate() {
            *x = flex::IDLE_VALUES[i % 2];
        }
        v[0] = Biw {
            count: 1,
            vector_start: 2,
            ..Biw::default()
        }
        .value();
        v[1] = capcode + 0x8000;
        v[2] = vector.value();
        v[3..3 + message.len()].copy_from_slice(message);
        v.map(flex::word)
    }

    /// The words of a fragment with the fragment check of `bits` bits in its header.
    fn with_check(words: &[u32], bits: u32) -> Vec<u32> {
        let mut w = words.to_vec();
        w[0] = (w[0] & !((1 << bits) - 1)) | u32::from(fragment_check(&w, bits));
        w
    }

    #[test]
    fn every_vector_type() {
        let mut dec = FlexDecoder::new(Config::default()).unwrap();
        let secure = with_check(&[0x1234, 0x5678], 10);
        let p = decode(
            &phase_with(100, Vector::Secure { start: 3, len: 2 }, &secure),
            &mut dec,
        );
        assert_eq!(
            (p[0].kind, p[0].data.as_slice()),
            (Kind::Secure, &secure[..])
        );
        let p = decode(
            &phase_with(
                100,
                Vector::Binary { start: 3, len: 3 },
                &with_check(&[0, 0xABCDE, 0x1F], 12),
            ),
            &mut dec,
        );
        assert_eq!((p[0].kind, p[0].data.len()), (Kind::Binary, 3));
        let instr = Vector::Instruction(0x1230 << 7);
        let p = decode(&phase_with(100, instr, &[]), &mut dec);
        assert_eq!(p[0].kind, Kind::Instruction);
        assert_eq!(p[0].data[0], instr.value());
        // A short message of three digits: 7, 1, 2 at bits 9, 13, 17.
        let p = decode(
            &phase_with(
                100,
                Vector::Short {
                    sub: 0,
                    data: 7 | 1 << 4 | 2 << 8,
                },
                &[],
            ),
            &mut dec,
        );
        assert_eq!((p[0].kind, p[0].text.as_str()), (Kind::Numeric, "712"));
        let p = decode(
            &phase_with(100, Vector::Short { sub: 1, data: 0 }, &[]),
            &mut dec,
        );
        assert_eq!(p[0].kind, Kind::Tone);
        // A numbered numeric message skips 10 bits.
        let mut bits = vec![0u8; 10];
        for d in [4u8, 2] {
            bits.extend((0..4).map(|i| (d >> i) & 1));
        }
        while bits.len() < 21 {
            bits.extend((0..4).map(|i| (12u8 >> i) & 1));
        }
        bits.truncate(21);
        let v = bits
            .iter()
            .enumerate()
            .fold(0u32, |a, (i, &b)| a | (b as u32) << i);
        let k = numeric_check(&[v]);
        let p = decode(
            &phase_with(
                100,
                Vector::Numeric {
                    kind: 7,
                    start: 3,
                    count: 1,
                    check: k & 0xF,
                },
                &[v | u32::from(k >> 4)],
            ),
            &mut dec,
        );
        assert_eq!(p[0].text, "42");
    }

    /// A text fragment with its fragment check; a first fragment carries the signature of `whole`, the whole message.
    fn alpha_phase(capcode: u32, header: Header, text: &str, whole: &str) -> [u32; PHASE_WORDS] {
        let mut chars: Vec<u8> = Vec::new();
        if header.fragment == 3 {
            chars.push(signature(whole.bytes()));
        }
        chars.extend(text.bytes());
        while !chars.len().is_multiple_of(3) {
            chars.push(3);
        }
        let mut msg = vec![header.value()];
        msg.extend(
            chars
                .chunks(3)
                .map(|c| c[0] as u32 | (c[1] as u32) << 7 | (c[2] as u32) << 14),
        );
        let msg = with_check(&msg, 10);
        phase_with(
            capcode,
            Vector::Alpha {
                start: 3,
                len: msg.len() as u8,
            },
            &msg,
        )
    }

    #[test]
    fn fragments_join_across_frames() {
        let mut dec = FlexDecoder::new(Config::default()).unwrap();
        let h1 = Header {
            continued: true,
            fragment: 3,
            number: 9,
            ..Header::default()
        };
        let h2 = Header {
            continued: true,
            fragment: 0,
            number: 9,
            ..Header::default()
        };
        let h3 = Header {
            continued: false,
            fragment: 1,
            number: 9,
            ..Header::default()
        };
        let mut got = Vec::new();
        for (t, (h, s)) in [(h1, "first part, "), (h2, "second part, "), (h3, "end")]
            .into_iter()
            .enumerate()
        {
            let w = alpha_phase(321, h, s, "first part, second part, end");
            dec.decode_words(Speed::F1600_2, Fiw::default(), 0, &w, None, t as f64, |p| {
                got.push(p.clone())
            });
        }
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "first part, second part, end");
        assert_eq!(got[0].flex.unwrap().fragments, 3);
        // A fragment that never ends comes out damaged after its age.
        let mut got = Vec::new();
        let w = alpha_phase(77, h1, "alone", "alone");
        dec.decode_words(Speed::F1600_2, Fiw::default(), 0, &w, None, 0.0, |p| {
            got.push(p.clone())
        });
        assert!(got.is_empty());
        let idle = phase_with(1, Vector::Short { sub: 1, data: 0 }, &[]);
        dec.decode_words(
            Speed::F1600_2,
            Fiw::default(),
            0,
            &idle,
            None,
            1000.0,
            |p| got.push(p.clone()),
        );
        assert_eq!(got.iter().filter(|p| p.address == 77).count(), 1);
        let p = got.iter().find(|p| p.address == 77).unwrap();
        assert!(p.damaged && p.text == "alone");
        // A last fragment with no first fragment before it is marked damaged.
        let mut got = Vec::new();
        let w = alpha_phase(78, h3, "tail only", "");
        dec.decode_words(Speed::F1600_2, Fiw::default(), 0, &w, None, 1001.0, |p| {
            got.push(p.clone())
        });
        assert_eq!(got.len(), 1);
        assert!(got[0].damaged && got[0].text == "tail only");
    }

    #[test]
    fn every_speed_round_trips_through_symbols_and_llrs() {
        for speed in Speed::ALL {
            let np = speed.phases();
            let mut phases = vec![[0u32; PHASE_WORDS]; np];
            for (i, ph) in phases.iter_mut().enumerate() {
                let text = format!("{} phase {}", speed.name(), speed.phase_letters()[i]);
                let tx = [FlexTx {
                    capcode: 1000 + i as u32,
                    kind: Kind::Alpha,
                    text: &text,
                }];
                encode_phase(&tx, ph).unwrap();
            }
            let mut sym = Vec::new();
            data_symbols(speed, &phases, &mut sym).unwrap();
            let data = &sym[speed.sync2_symbols()..];
            assert_eq!(data.len(), speed.data_symbols());
            let y: Vec<f32> = data.iter().map(|&s| s as f32 / 3.0 * 0.8 + 0.03).collect();
            let mut bufs = vec![vec![0f32; PHASE_BITS]; np];
            {
                let mut refs: Vec<&mut [f32]> = bufs.iter_mut().map(|b| b.as_mut_slice()).collect();
                split_phases(speed, &y, &mut refs).unwrap();
            }
            let mut dec = FlexDecoder::new(Config::default()).unwrap();
            for (i, b) in bufs.iter().enumerate() {
                let mut got = Vec::new();
                dec.decode_llrs(speed, Fiw::default(), i as u8, b, 0.0, |p| {
                    got.push(p.clone())
                })
                .unwrap();
                let want = format!("{} phase {}", speed.name(), speed.phase_letters()[i]);
                assert_eq!(got.len(), 1, "{speed:?} {i}");
                assert_eq!(got[0].text, want);
                assert_eq!(got[0].rate, speed.name());
            }
        }
        assert_eq!(
            split_phases(Speed::F1600_2, &[0.0; 10], &mut []),
            Err(Error::BadLength(2816))
        );
    }

    #[test]
    fn a_lost_word_comes_back_from_an_earlier_copy() {
        let tx = [FlexTx {
            capcode: 4242,
            kind: Kind::Alpha,
            text: "Repeated message for the vote",
        }];
        let mut w = [0u32; PHASE_WORDS];
        encode_phase(&tx, &mut w).unwrap();
        let mut dec = FlexDecoder::new(Config::default()).unwrap();
        let first = decode(&w, &mut dec);
        assert!(!first[0].combined);
        let mut bad = w;
        bad[6] ^= 0b10101;
        let second = decode(&bad, &mut dec);
        assert_eq!(second[0].text, tx[0].text);
        assert!(second[0].combined);
        let mut bad2 = w;
        bad2[6] ^= 0b10101;
        let alone = decode(
            &bad2,
            &mut FlexDecoder::new(Config {
                combine: false,
                ..Config::default()
            })
            .unwrap(),
        );
        assert!(alone[0].damaged);
    }

    #[test]
    fn steady_state_decoding_does_not_allocate() {
        let tx = [
            FlexTx {
                capcode: 1_234_567,
                kind: Kind::Alpha,
                text: "No allocation in steady state",
            },
            FlexTx {
                capcode: 2_201_234,
                kind: Kind::Numeric,
                text: "0123456789",
            },
        ];
        let mut w = [0u32; PHASE_WORDS];
        encode_phase(&tx, &mut w).unwrap();
        w[7] ^= 0b11;
        let mut bits = vec![0u8; PHASE_BITS];
        interleave(&w, &mut bits).unwrap();
        let llr: Vec<f32> = bits
            .iter()
            .map(|&b| if b == 1 { -1.0 } else { 1.0 })
            .collect();
        let mut dec = FlexDecoder::new(Config::default()).unwrap();
        for _ in 0..60 {
            dec.decode_llrs(Speed::F1600_2, Fiw::default(), 0, &llr, 0.0, |_| {})
                .unwrap();
        }
        let mut pages = 0;
        let n = allocations(|| {
            for _ in 0..200 {
                dec.decode_llrs(Speed::F1600_2, Fiw::default(), 0, &llr, 0.0, |_| pages += 1)
                    .unwrap();
                dec.decode_bits(Speed::F1600_2, Fiw::default(), 0, &bits, 0.0, |_| {
                    pages += 1
                })
                .unwrap();
            }
        });
        assert_eq!(pages, 800);
        assert_eq!(n, 0);
    }
}
