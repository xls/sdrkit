//! Helpers of the tests: a stack's harness at 12 ksps, noise given as an SNR in 2.5 kHz, typed result reads and the recordings.
#![allow(dead_code)]

pub use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_CN0_DBHZ, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::verify::CaseSetting;
pub use sdrkit_core::Choice;
use sdrkit_core::{Record, Results, StackDescriptor, Value};
use sdrkit_dev_test::{Harness, Run};
use serde_json::Value as Json;

/// Catalogue setting `s` at the entry whose key is `entry`.
pub fn pick(s: &CatalogueSetting, entry: &str) -> CaseSetting {
    let i = (s.choices)()
        .iter()
        .position(|c| c.key == entry)
        .unwrap_or_else(|| panic!("{} has no entry {entry}", s.key));
    CaseSetting {
        key: s.key,
        value: Value::Choice(u16::try_from(i).unwrap()),
    }
}

/// The rate of the test signals.
pub const RATE: f64 = 12_000.0;

/// A stack at 12 ksps, the dial at 14.07 MHz, fed in pieces of 800 samples.
pub fn harness(desc: &'static StackDescriptor) -> Harness {
    Harness::new(desc).rate(RATE).freq(14.07e6).piece(800)
}

/// White noise for an SNR in 2.5 kHz, the bandwidth HF reports use, set as the C/N0 that corresponds to it.
pub fn snr_2500(h: Harness, snr_db: f64) -> Harness {
    h.set(&GEN_NOISE, NoiseKind::Cn0)
        .set(&GEN_CN0_DBHZ, snr_db + 10.0 * 2500f64.log10())
}

/// Runs the harness, failing on an error.
pub fn run(h: &Harness) -> Run {
    h.run().unwrap_or_else(|e| panic!("{e}"))
}

/// A number of a value, `NaN` when it is not a number.
pub fn number(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x)) => *x,
        Some(Value::I64(i)) => *i as f64,
        _ => f64::NAN,
    }
}

/// A number of the results, `NaN` when it is missing or has no value.
pub fn num(r: &Run, key: &str) -> f64 {
    number(r.field(key))
}

/// A field of `res` as text (a choice as its label), empty when it is missing.
pub fn field_text(res: &Results, key: &str) -> String {
    res.decl()
        .field_id(key)
        .and_then(|id| res.fields().text(id))
        .map(|t| t.to_string())
        .unwrap_or_default()
}

/// A number field of `res`, `NaN` when it is missing.
pub fn field_num(res: &Results, key: &str) -> f64 {
    number(
        res.decl()
            .field_id(key)
            .and_then(|id| res.fields().value(id)),
    )
}

/// A text of the results, empty when it is missing.
pub fn text_field(r: &Run, key: &str) -> String {
    field_text(&r.results, key)
}

/// The decoded text.
pub fn text(r: &Run) -> String {
    text_field(r, "text")
}

/// Feeds the recording `name` of this family to the harness. A missing recording fails the test.
pub fn recording(h: &Harness, name: &str) -> Run {
    h.run_vector("hfkbd", name)
        .unwrap_or_else(|e| panic!("{e}"))
        .unwrap_or_else(|| panic!("the recording hfkbd/{name} is missing"))
}

/// The expectations of a recording from `expected.json`.
pub fn expected(name: &str) -> Json {
    sdrkit_dev_test::sigmf::read_expected("hfkbd").unwrap()["recordings"][name].clone()
}

/// Whether `v` is in the inclusive range `[r[0], r[1]]` of `expected.json`.
pub fn in_range(v: f64, r: &Json) -> bool {
    v >= r[0].as_f64().unwrap() && v <= r[1].as_f64().unwrap()
}

/// `seconds` of white noise alone at the harness rate, as loud as noise 0 dB below a unit-power signal in 2.5 kHz.
pub fn noise_only(seconds: f64, seed: u64) -> Vec<sdrkit_core::Complex32> {
    let power = RATE / 2500.0;
    let mut x = vec![sdrkit_core::Complex32::default(); (seconds * RATE) as usize];
    sdrkit_algo_sim::noise::Awgn::with_power(power, seed)
        .unwrap()
        .process(&mut x);
    x
}

/// Feeds samples to the harness's receiver.
pub fn feed(h: &Harness, x: &[sdrkit_core::Complex32]) -> Run {
    h.feed(x).unwrap_or_else(|e| panic!("{e}"))
}

/// A switch of the results. Panics when it is missing.
pub fn flag(r: &Run, key: &str) -> bool {
    match r.field(key) {
        Some(Value::Bool(b)) => *b,
        v => panic!("{key} is not a switch: {v:?}"),
    }
}

/// The rows of a table of the results, newest first.
pub fn rows(r: &Run, table: &str) -> Vec<Record> {
    let res = &r.results;
    let Some(t) = res.decl().table_id(table).and_then(|id| res.table(id)) else {
        return Vec::new();
    };
    t.rows_newest_first().map(|(_, row)| row.clone()).collect()
}

/// The cell `key` of a row.
pub fn cell<'a>(row: &'a Record, key: &str) -> Option<&'a Value> {
    row.decl().id(key).and_then(|id| row.value(id))
}

/// A number of a cell, `NaN` when it has none.
pub fn cell_num(row: &Record, key: &str) -> f64 {
    number(cell(row, key))
}

/// A text of a cell (a choice as its label), empty when it has none.
pub fn cell_text(row: &Record, key: &str) -> String {
    row.decl()
        .id(key)
        .and_then(|id| row.text(id))
        .map(|t| t.to_string())
        .unwrap_or_default()
}

/// A switch of a cell.
pub fn cell_flag(row: &Record, key: &str) -> Option<bool> {
    match cell(row, key) {
        Some(Value::Bool(b)) => Some(*b),
        _ => None,
    }
}

/// A receiver of the harness's stack, configured for samples at `rate` with the dial at 0 Hz, and a tap on its pictures.
pub fn receiver_with_pictures(
    h: &Harness,
    rate: f64,
) -> (Box<dyn sdrkit_core::Receiver>, sdrkit_graph::TapReader) {
    let mut rx = h.receiver().unwrap();
    rx.configure(&sdrkit_core::InputInfo::new(rate, 0.0, 0.0).recording())
        .unwrap();
    let g = rx
        .as_any_mut()
        .unwrap()
        .downcast_mut::<sdrkit_graph::Graph>()
        .unwrap();
    let tap = g.tap("dec.pictures", 1 << 16).unwrap();
    (rx, tap)
}

/// Feeds samples in pieces of `piece`.
pub fn feed_rx(
    rx: &mut dyn sdrkit_core::Receiver,
    x: &[sdrkit_core::Complex32],
    piece: usize,
) -> sdrkit_core::Output {
    let mut all = sdrkit_core::Output::default();
    let mut out = sdrkit_core::Output::default();
    for c in x.chunks(piece.max(1)) {
        out.clear();
        rx.feed(c, &mut out);
        all.events.append(&mut out.events);
        for p in out.audio_runs() {
            all.push_audio(p.samples, p.rate_hz, p.channels);
        }
    }
    all
}

/// The pictures waiting in a tap, as rasters.
pub fn pictures(tap: &mut sdrkit_graph::TapReader) -> Vec<sdrkit_codec_image::Raster> {
    use sdrkit_graph::{Buffer, Kind, Picture, Timed};
    let mut out = vec![];
    let mut b = Buffer::new(&Kind::typed::<Picture>());
    while tap.pop(&mut b) {
        if let sdrkit_graph::Data::Items(Some(items)) = &b.data {
            if let Some(v) = items.as_any().downcast_ref::<Vec<Timed<Picture>>>() {
                out.extend(
                    v.iter()
                        .map(|t| sdrkit_codec_image::Raster::from_picture(&t.value).unwrap()),
                );
            }
        }
    }
    out
}

/// The grey level (mean of the colours) of pixel (x, y) of a BGRA raster.
pub fn grey(r: &sdrkit_codec_image::Raster, x: u32, y: u32) -> u8 {
    let p = &r.row(y).unwrap()[4 * x as usize..4 * x as usize + 3];
    ((p[0] as u32 + p[1] as u32 + p[2] as u32) / 3) as u8
}

/// Reads a PNG of this family's recordings as RGB rows: (width, height, rgb bytes). A missing file fails the test.
pub fn read_png(name: &str) -> (u32, u32, Vec<u8>) {
    let p = sdrkit_dev_test::recording(&format!("hfkbd/{name}"))
        .unwrap_or_else(|| panic!("the picture hfkbd/{name} is missing"));
    let r = sdrkit_codec_image::png::decode(&std::fs::read(p).unwrap()).unwrap();
    (r.width(), r.height(), r.rgb().flatten().collect())
}

/// Runs a keyboard stack's recording in pieces of two sizes and checks its text, callsign and lock. Returns the last run.
pub fn keyboard_check(desc: &'static StackDescriptor, name: &str, settings: &[CaseSetting]) -> Run {
    let e = expected(name);
    let mut last = None;
    for piece in [800, 4093] {
        let h = harness(desc).piece(piece).with_settings(settings);
        let r = recording(&h, name);
        let t = text(&r);
        let want: Vec<&str> = match &e["expect"]["contains"] {
            Json::String(s) => vec![s.as_str()],
            Json::Array(a) => a.iter().map(|v| v.as_str().unwrap()).collect(),
            _ => vec![],
        };
        for w in want {
            assert!(t.contains(w), "{name}: {w:?} not in {t:?}");
        }
        if let Some(c) = e["expect"]["callsign"].as_str() {
            assert_eq!(text_field(&r, "callsign"), c, "{name}");
        }
        if e["expect"]["state"] == "locked" {
            assert_eq!(
                r.results.status().lock_state,
                sdrkit_core::LockState::Locked,
                "{name}: not locked"
            );
        }
        last = Some(r);
    }
    last.unwrap()
}
