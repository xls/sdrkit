//! Checks of details of the standards that are easy to get wrong (the varicodes, the figures case, the ARDOP checks), and of
//! the allocation and the spot form of the receivers.

use crate::common::*;
use sdrkit_stack_hfkbd as hf;

/// The IZ8BLY varicode of MFSK16, which THOR also uses (N. Greenman and N. Porcino, MFSK16 description), for the lower-case
/// letters.
#[test]
fn mfsk_varicode_has_the_iz8bly_lower_case_codes() {
    let bits = |b: u8| -> String {
        hf::mfsk::varicode_bits(&[b])
            .iter()
            .map(|&x| (b'0' + x) as char)
            .collect()
    };
    for (b, s) in [
        (b'e', "1000"),
        (b't', "1100"),
        (b'a', "10100"),
        (b'o', "10000"),
        (b'i', "11000"),
        (b'n', "11100"),
        (b's', "101000"),
        (b'r', "100000"),
        (b' ', "100"),
    ] {
        assert_eq!(bits(b), s, "{}", b as char);
    }
    for b in b"lhbwyjqz".iter().copied().chain(b'a'..=b'z') {
        let want = sdrkit_util_text::varicode::mfsk::code(b).to_bit_string();
        assert_eq!(bits(b), want, "{}", b as char);
    }
}

/// The figures case of US teleprinters, the one of amateur practice: `"` on Z, `;` on V, `'` on J, `$` on D, `!`, `&` and `#`
/// on F, G and H. The ITU signs `+` and `=` are not in it.
#[test]
fn rtty_figures_follow_the_us_teleprinter_case() {
    let h = harness(&hf::RTTY)
        .set(
            &hf::decoder::GEN_MESSAGE,
            "RYRY 1$2;3 IT'S \"4/5\" #6 &7! \n",
        )
        .seconds(20.0);
    let r = run(&h);
    assert!(
        text(&r).contains("1$2;3 IT'S \"4/5\" #6 &7!"),
        "{:?}",
        text(&r)
    );
    let t = hf::rtty::TABLE;
    assert_eq!(t, sdrkit_util_text::ita2::US_TTY);
    assert_eq!(t.char_of(17, true), Some('"'));
    assert_eq!(t.char_of(30, true), Some(';'));
    assert_eq!(t.char_of(11, true), Some('\''));
    assert!(t.code_of('+').is_err() && t.code_of('=').is_err());
}

/// The varicodes the keyboard stacks send are the published tables of sdrkit-util-text.
#[test]
fn keyboard_modes_send_the_published_varicode_tables() {
    use sdrkit_util_text::varicode::fsq::Alphabet;
    for b in 0..=255u8 {
        for (kind, a) in [
            (hf::fsq::Kind::Fsq, Alphabet::Fsq),
            (hf::fsq::Kind::Ifkp, Alphabet::Ifkp),
        ] {
            let want = a
                .code(b)
                .ok()
                .map(|c| (c.first as u32, c.marker.map(u32::from)));
            assert_eq!(hf::fsq::code(kind, b), want);
        }
        assert_eq!(
            hf::mfsk::varicode_bits(&[b]),
            sdrkit_util_text::varicode::mfsk::code(b)
                .iter()
                .collect::<Vec<u8>>()
        );
    }
}

/// DominoEX sends the published nibble codes of every byte.
#[test]
fn dominoex_sends_the_published_nibble_codes() {
    for b in 0..=255u8 {
        assert_eq!(
            hf::domino::nibbles(b),
            sdrkit_util_text::varicode::dominoex::code(b).symbols()
        );
    }
}

/// The ARDOP type check is a 4-ary symbol: 1 XOR the four 2-bit groups of the type byte.
#[test]
fn ardop_type_check_is_a_4ary_symbol() {
    assert_eq!(hf::ardop::type_check(0x30), 2);
    assert_eq!(hf::ardop::type_check(0x00), 1);
    assert_eq!(hf::ardop::type_check(0xFF), 1);
    assert_eq!(hf::ardop::type_check(0x4A), 0);
    assert_eq!(hf::ardop::type_symbols(0x30, 0xFF)[4], 2);
}

/// The ARDOP frame check has the effective generator 0x18810: the check value of "123456789" is 0x0E59.
#[test]
fn ardop_frame_check_has_the_generator_0x18810() {
    assert_eq!(hf::ardop::frame_check(b"123456789"), 0x0E59);
    let d: Vec<u8> = (1..=12).collect();
    assert_eq!(hf::ardop::frame_check(&d), 0xD58C);
}

/// A word with more errors than the code corrects is never given back as corrected unless it is a code word.
#[test]
fn ardop_reed_solomon_never_accepts_a_wrong_word() {
    let mut rs = hf::ardop::RsCodes::new();
    let data: Vec<u8> = (1..=12).collect();
    let mut good = data.clone();
    good.extend(rs.parity(&data, 4));
    let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
    for _ in 0..2000 {
        let mut w = good.clone();
        for _ in 0..3 {
            let i = rng.below(w.len() as u64) as usize;
            w[i] ^= 1 + rng.below(255) as u8;
        }
        if rs.correct(&mut w, 4).is_some() {
            let p = rs.parity(&w[..12], 4);
            assert_eq!(&w[12..], &p[..], "a corrected word that is not a code word");
        }
    }
}

/// The RTTY tone integrators sum again from time to time, so a long signal leaves nothing behind once it stops.
#[test]
fn rtty_integrators_do_not_drift() {
    let mut d = hf::rtty::FskDemod::new(8000.0, 45.45);
    d.set_tones([1000.0, 1170.0]);
    let mut tone = hf::util::Tone::new(8000.0);
    for _ in 0..4_000_000 {
        d.step(tone.next(1000.0, 1000.0));
    }
    let mut last = 1.0;
    for _ in 0..2000 {
        last = d.step(sdrkit_core::Complex32::default());
    }
    assert_eq!(last, 0.0);
}

/// The FSQ receiver allocates nothing for each sample once it runs.
#[test]
fn fsq_allocates_nothing_per_sample() {
    use sdrkit_graph::Decoder;
    let x = noise_only(4.0, 9);
    let mut d = hf::fsq::FsqDecoder::new(hf::fsq::Kind::Fsq);
    let f = d
        .configure(&sdrkit_graph::Format::iq(8000.0, 14.1e6, 14.1e6))
        .unwrap();
    let kinds: Vec<sdrkit_graph::Kind> = f.iter().map(|f| f.kind).collect();
    let mut out = sdrkit_graph::Outbox::new(&kinds);
    let mut feed = |d: &mut hf::fsq::FsqDecoder, a: usize, b: usize| {
        for (k, c) in x[a..b].chunks(800).enumerate() {
            d.process(c, (a + 800 * k) as u64, &mut out);
        }
    };
    feed(&mut d, 0, 16000);
    let n = crate::alloc_count::count(|| feed(&mut d, 16000, x.len()));
    assert_eq!(n, 0, "{n} allocations");
}

/// The FSQ callsign check is CRC-8/SMBUS of the catalogue (check value 0xF4).
#[test]
fn fsq_callsign_check_is_crc8_smbus() {
    assert_eq!(hf::fsq::callsign_check(b"123456789"), 0xF4);
}

/// A spot carries the standard spot fields: callsign, snr_db, freq_hz and mode, and the callsign as its source.
#[test]
fn spots_use_the_standard_spot_fields() {
    use sdrkit_core::publish::keys;
    let r = run(&harness(&hf::PSK).seconds(20.0));
    let spots: Vec<_> = r.events_of(hf::typed::SPOT.kind).collect();
    assert!(!spots.is_empty());
    for e in spots {
        let call = e.get(&keys::CALLSIGN).unwrap();
        assert_eq!(e.get(&keys::SOURCE), Some(call));
        assert!(e.get(&keys::SNR_DB).is_some());
        assert!(e.get(&keys::SPOT_MODE).is_some());
        assert!(e.get(&keys::FREQ_HZ).unwrap() > 0.0);
    }
}

/// Each test signal states where its dial lies in its samples. Moved from there to 20 kHz below the centre, where a session
/// without a frequency listens, it decodes.
#[test]
fn test_signals_state_where_their_dial_lies() {
    use hf::typed::{KeyboardFields, WinmorFields};
    use sdrkit_core::{GenConfig, InputInfo, Output};
    let rate = 96_000.0;
    let centre = 14.1e6;
    for desc in [&hf::RTTY, &hf::PSK, &hf::DOMINOEX, &hf::OLIVIA, &hf::WINMOR] {
        let h = snr_2500(harness(desc), 20.0).rate(rate).seconds(20.0);
        let make = desc.make_generator.expect("a generator");
        let at = make(&GenConfig::new(rate, h.values().unwrap()))
            .unwrap()
            .station_offset_hz(hf::decoder::TUNE_OFFSET_HZ);
        let mut x = h.generate().unwrap();
        sdrkit_algo_sim::offset::FrequencyOffset::new(rate, -hf::decoder::TUNE_OFFSET_HZ - at, 0.0)
            .unwrap()
            .process(&mut x);
        let mut rx = h.receiver().unwrap();
        rx.configure(&InputInfo::new(
            rate,
            centre,
            centre - hf::decoder::TUNE_OFFSET_HZ,
        ))
        .unwrap();
        let mut out = Output::default();
        for c in x.chunks(9600) {
            rx.feed(c, &mut out);
            out.clear();
        }
        let f = rx.results().fields();
        let n = if desc.id == "winmor" {
            f.at(&WinmorFields::PILOTS).unwrap_or(0)
        } else {
            f.at(&KeyboardFields::CHARACTERS).unwrap_or(0)
        };
        assert!(n > 0, "{}: station at {at} Hz, {n}", desc.id);
    }
}
