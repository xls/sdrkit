//! DCS codes in the listening chain and the tone decoder, and the position of a Maidenhead locator.

use sdrkit_core::{SettingSet, Value};
use sdrkit_proto_tones::dcs::{DcsConfig, DcsDecoder, DcsEncoder, DcsEventKind};
use sdrkit_stack_general::bands::locator_position;
use sdrkit_stack_general::listen::{ChainSettings, ChainSquelch, ListenSetting, ListeningChain};
use sdrkit_util_ids::tones::dcs::{self, Dcs};

const RATE: f64 = 48_000.0;

fn text(
    s: &mut ChainSettings,
    setting: ListenSetting,
    v: &str,
) -> Result<(), sdrkit_core::SettingError> {
    s.apply(setting.id(), &Value::Text(v.into()))
}

/// The DCS code of the chain is octal: the written code "023" is the nine-bit value 0o23 (19), its value keeps the three digits,
/// and the tone squelch opens on the audio of that code.
#[test]
fn dcs_code_is_held_as_octal_and_written_with_three_digits() {
    let mut s = ChainSettings::default();
    s.control.on = true;
    s.control.squelch = ChainSquelch::Dcs;
    s.control.squelch_hang_ms = 250.0;
    text(&mut s, ListenSetting::Dcs, "023").unwrap();
    assert_eq!(s.control.dcs_code, Dcs::from_octal("023").unwrap());
    assert_eq!(s.control.dcs_code.value(), 19);
    let v = s.values();
    let id = ListenSetting::schema().id("dcs").unwrap();
    assert_eq!(v.value(id), Some(&Value::Text("023".into())));
    // A code without the leading zero reads as the same code.
    let mut short = ChainSettings::default();
    text(&mut short, ListenSetting::Dcs, "23").unwrap();
    assert_eq!(short.control.dcs_code.value(), 0o23);
    // A digit 8 or 9 is not octal: the value is refused.
    let mut bad = ChainSettings::default();
    assert!(text(&mut bad, ListenSetting::Dcs, "089").is_err());
    assert_eq!(
        bad.control.dcs_code,
        ChainSettings::default().control.dcs_code
    );
    // The tone squelch opens on D023N.
    let mut chain = ListeningChain::new(RATE, s);
    let mut enc = DcsEncoder::new(
        RATE,
        Dcs::from_octal("023").unwrap(),
        false,
        0.2,
        3.0,
        false,
    )
    .unwrap();
    let mut audio = vec![0f32; enc.len() as usize];
    enc.fill(&mut audio);
    let mut opened = false;
    for b in audio.chunks(480) {
        let mut st: Vec<[f32; 2]> = b.iter().map(|&v| [v, v]).collect();
        chain.process(&mut st);
        opened |= chain.control().squelch_open();
    }
    assert!(opened, "the DCS squelch opens on 023");
    assert_eq!(chain.settings().control.squelch, ChainSquelch::Dcs);
}

/// The DCS reader repairs errors by the majority of three repeated words: one wrong bit in one word still reads, and noise words
/// almost never read as a code.
#[test]
fn dcs_reading_repairs_by_majority_and_noise_does_not_read_as_a_code() {
    // Noise words: count the random 23-bit words that read as a code with no correction.
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    let n = 20_000;
    let reads = (0..n)
        .filter(|_| dcs::decode(rng.next_u32() & 0x7F_FFFF).is_some())
        .count();
    assert!(reads * 100 < n, "{reads} of {n} noise words read as a code");
    // A bit stream of D023N with one bit inverted in every third word: the decoder still locks on D023N.
    let word = Dcs::from_octal("023").unwrap().word();
    let spb = RATE / dcs::BIT_RATE as f64;
    let bits = (3.0 * dcs::BIT_RATE as f64) as usize;
    let mut audio = Vec::new();
    for k in 0..bits {
        let mut b = (word >> (k % 23)) & 1;
        if k % 69 == 50 {
            b ^= 1;
        }
        let level = if b == 1 { 0.2 } else { -0.2 };
        let end = ((k + 1) as f64 * spb).round() as usize;
        audio.resize(end, level);
    }
    let mut d = DcsDecoder::new(RATE, DcsConfig::default()).unwrap();
    let mut locked = None;
    d.process(&audio, |e| {
        if e.kind == DcsEventKind::Locked {
            locked = Some(e.reading.name());
        }
    });
    assert_eq!(locked.as_deref(), Some("D023N"));
}

/// The centre of a locator is latitude first.
#[test]
fn locator_position_has_latitude_first() {
    let (lat, lon) = locator_position("JN58td").unwrap();
    assert!(
        (lat - 48.146).abs() < 0.01 && (lon - 11.625).abs() < 0.01,
        "{lat} {lon}"
    );
}

/// An 8-character locator names a cell 24 times smaller than its 6-character square, and its centre is the centre of that cell.
#[test]
fn locator_position_reads_all_eight_characters() {
    let six = locator_position("JN58td").unwrap();
    let eight = locator_position("JN58td25").unwrap();
    assert_ne!(six, eight);
    let want = sdrkit_util_geo::maidenhead::decode("JN58td25")
        .unwrap()
        .centre();
    assert!((eight.0 - want.lat_deg).abs() < 1e-9 && (eight.1 - want.lon_deg).abs() < 1e-9);
}
