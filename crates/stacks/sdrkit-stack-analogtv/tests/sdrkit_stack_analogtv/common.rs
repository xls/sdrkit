//! Helpers shared by the analogue video tests: receivers made from setting texts, runs that read the typed results after each
//! piece, and picture measures.
#![allow(dead_code)]

use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::keys;
use sdrkit_core::{
    Choice, Complex32, Generator, InputInfo, Output, Picture, Receiver, Record, Results,
    SettingSet, StackConfig, StreamKind, Values,
};
use sdrkit_graph::StackGraph;
use sdrkit_proto_mpegts::publish::TELETEXT_PAGE_TABLE as PAGES_TABLE;
use sdrkit_stack_analogtv::card::{Planes, EBU_BARS};
use sdrkit_stack_analogtv::results as r;
use sdrkit_stack_analogtv::settings::AtvSetting;
use sdrkit_stack_analogtv::typed::{AtvFields, TeletextFields};
use sdrkit_stack_analogtv::{AtvGenParams, AtvGenerator};
use serde_json::{json, Map, Value as Json};
use std::sync::Arc;

/// The settings JSON form of `(key, text)` pairs: a number, yes or no, or the text (a choice key).
pub fn settings_json(settings: &[(&str, &str)]) -> Json {
    let mut m = Map::new();
    for (k, v) in settings {
        let j = match (*v, v.parse::<f64>()) {
            ("true", _) => Json::Bool(true),
            ("false", _) => Json::Bool(false),
            (_, Ok(x)) if !matches!(*k, "system" | "gen_system" | "channel_width") => {
                if x.fract() == 0.0 {
                    json!(x as i64)
                } else {
                    json!(x)
                }
            }
            _ => Json::String((*v).to_string()),
        };
        m.insert((*k).to_string(), j);
    }
    Json::Object(m)
}

/// The values of `schema` with `settings` (as [`settings_json`]) over the defaults.
pub fn values_of(schema: &Arc<sdrkit_core::Schema>, settings: &[(&str, &str)]) -> Values {
    let mut v = Values::defaults(schema);
    v.update_from_json(&settings_json(settings))
        .expect("the settings are valid");
    v
}

/// Runs command `key` of `rx` with the row key `row`.
pub fn invoke(rx: &mut StackGraph, key: &str, row: &str) -> Result<(), sdrkit_core::CommandError> {
    let id = rx.commands().id(key)?;
    let def = rx.commands().def(id).expect("a command");
    let mut args: Record = def.new_args();
    let key: sdrkit_core::publish::Field<sdrkit_core::publish::field::Text> =
        sdrkit_core::publish::Field::new("row", "Row", "");
    args.set(&key, row);
    owner::invoke(rx, id, &args).map(|_| ())
}

/// The repository's data folder.
pub fn data() -> sdrkit_presets::Data {
    sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
}

/// The text of the setting `channels` for the analogue TV plan `id` of the data folder.
pub fn atv_channels(id: &str) -> String {
    data()
        .atv_plan(id)
        .unwrap_or_else(|| panic!("no analogue TV plan {id}"))
        .text()
}

/// The text of the setting `channels` for the FPV channels of the data folder.
pub fn fpv_channels_text() -> String {
    data().fpv_channels().expect("the FPV channels").text()
}

/// An `atv` receiver at `rate` with these settings. The key `teletext_page` runs the `teletext.select` command.
pub fn atv(rate: f64, settings: &[(&str, &str)]) -> StackGraph {
    let page = settings
        .iter()
        .find(|(k, _)| *k == "teletext_page")
        .map(|p| p.1);
    let rest: Vec<(&str, &str)> = settings
        .iter()
        .copied()
        .filter(|(k, _)| *k != "teletext_page")
        .collect();
    let values = values_of(&AtvSetting::schema(), &rest);
    let mut rx = sdrkit_stack_analogtv::atv::graph::receiver(&StackConfig::new(values))
        .expect("the receiver builds");
    if let Some(p) = page {
        invoke(&mut rx, "teletext.select", p).expect("teletext.select");
    }
    rx.configure(&InputInfo::new(rate, 600e6, 600e6).recording())
        .expect("configure");
    rx
}

/// The teletext state of a report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Teletext {
    /// Packets are received.
    pub present: bool,
    /// Packets received.
    pub packets: u64,
    /// Lines whose framing code or address failed.
    pub errors: u64,
    /// Characters whose parity failed.
    pub parity_errors: u64,
    /// The header text.
    pub header: String,
    /// The pages held.
    pub pages: Vec<String>,
    /// The page shown.
    pub page: String,
    /// Rows 1 to 24 of the page shown.
    pub rows: Vec<String>,
}

/// The NICAM state of a report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nicam {
    /// Frame locked.
    pub locked: bool,
    /// Sound valid.
    pub valid: bool,
    /// The mode label ("stereo", "dual mono").
    pub mode: String,
    /// Parity errors.
    pub parity_errors: u64,
}

/// The A2 state of a report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct A2 {
    /// The pilot is there.
    pub present: bool,
    /// The mode label ("stereo", "dual", "mono").
    pub mode: String,
}

/// What the `atv` results say at one moment, read from the typed fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Detail {
    /// Seconds of signal fed when it was read.
    pub t: f64,
    pub state: u8,
    pub data_valid: bool,
    pub system: String,
    pub colour_system: String,
    pub colour: bool,
    pub lines: u32,
    pub field_hz: f64,
    pub line_hz: f64,
    pub line_error_ppm: f64,
    pub detector: String,
    pub synchronous_detector: bool,
    pub vision_hz: f64,
    pub sound_hz: f64,
    pub sound_spacing_mhz: f64,
    pub carrier_offset_hz: f64,
    pub carrier_to_noise_db: f64,
    pub video_snr_db: f64,
    pub sync_depth_pct: f64,
    pub burst_level: f64,
    pub chroma_level: f64,
    pub secam_identification: String,
    pub sound_system: String,
    pub sound_present: bool,
    pub sound_deviation_khz: f64,
    pub sound_level_dbfs: f64,
    pub line_count: u64,
    pub field_count: u64,
    pub pictures: u64,
    pub snow_pictures: u64,
    pub fields_ok: u64,
    pub fields_bad: u64,
    pub fm_polarity: String,
    pub nbtv_line_locked: bool,
    pub nbtv_picture_locked: bool,
    pub nicam: Option<Nicam>,
    pub a2: Option<A2>,
    pub teletext: Option<Teletext>,
}

/// The `atv` fields of `res` at `t` seconds.
pub fn detail(res: &Results, t: f64) -> Detail {
    let f = res.fields();
    let num = |x: Option<f64>| x.unwrap_or(0.0);
    let int = |x: Option<i64>| x.unwrap_or(0).max(0) as u64;
    let state = f.at(&AtvFields::STATE).map_or(0, |s| s.index() as u8);
    let teletext = f.at(&TeletextFields::TELETEXT_PACKETS).map(|packets| {
        let pages = res
            .decl()
            .table_id(PAGES_TABLE.key)
            .and_then(|id| res.table(id))
            .map(|t| {
                let mut v: Vec<String> =
                    t.rows_newest_first().map(|(k, _)| k.to_string()).collect();
                v.sort();
                v
            })
            .unwrap_or_default();
        Teletext {
            present: true,
            packets: packets.max(0) as u64,
            errors: int(f.at(&TeletextFields::TELETEXT_ERRORS)),
            parity_errors: int(f.at(&TeletextFields::TELETEXT_PARITY_ERRORS)),
            header: f
                .at(&TeletextFields::TELETEXT_HEADER)
                .unwrap_or("")
                .to_string(),
            pages,
            page: f
                .at(&TeletextFields::TELETEXT_PAGE)
                .unwrap_or(0)
                .to_string(),
            rows: f
                .at(&TeletextFields::PAGE_TEXT)
                .map(|t| t.lines().skip(1).map(str::to_string).collect())
                .unwrap_or_default(),
        }
    });
    Detail {
        t,
        state,
        data_valid: state == 2,
        system: f.at(&AtvFields::SYSTEM).unwrap_or("").to_string(),
        colour_system: f
            .at(&AtvFields::COLOUR_SYSTEM)
            .map(|e| e.label().to_string())
            .unwrap_or_default(),
        colour: f.at(&AtvFields::COLOUR).unwrap_or(false),
        lines: int(f.at(&AtvFields::LINES)) as u32,
        field_hz: num(f.at(&AtvFields::FIELD_RATE)),
        line_hz: num(f.at(&AtvFields::LINE_RATE)),
        line_error_ppm: num(f.at(&AtvFields::LINE_ERROR_PPM)),
        detector: f
            .at(&AtvFields::DETECTOR)
            .map(|e| e.label().to_string())
            .unwrap_or_default(),
        synchronous_detector: f.at(&AtvFields::DETECTOR) == Some(r::DetectorUsed::Synchronous),
        vision_hz: num(f.at(&AtvFields::VISION_CARRIER)),
        sound_hz: num(f.at(&AtvFields::SOUND_CARRIER)),
        sound_spacing_mhz: num(f.at(&AtvFields::SOUND_SPACING)) / 1e6,
        carrier_offset_hz: num(f.get(&keys::OFFSET_HZ)),
        carrier_to_noise_db: num(f.at(&AtvFields::CARRIER_TO_NOISE_DB)),
        video_snr_db: num(f.get(&keys::SNR_DB)),
        sync_depth_pct: num(f.at(&AtvFields::SYNC_DEPTH_PCT)),
        burst_level: num(f.at(&AtvFields::BURST_PCT)) / 100.0,
        chroma_level: num(f.at(&AtvFields::CHROMA_LEVEL_PCT)) / 100.0,
        secam_identification: f
            .at(&AtvFields::SECAM_IDENTIFICATION)
            .unwrap_or("")
            .to_string(),
        sound_system: f
            .at(&AtvFields::SOUND_SYSTEM)
            .map(|e| e.label().to_string())
            .unwrap_or_default(),
        sound_present: f.at(&AtvFields::SOUND_PRESENT).unwrap_or(false),
        sound_deviation_khz: num(f.at(&AtvFields::SOUND_DEVIATION)) / 1e3,
        sound_level_dbfs: num(f.at(&AtvFields::AUDIO_LEVEL_DBFS)),
        line_count: int(f.at(&AtvFields::LINE_COUNT)),
        field_count: int(f.at(&AtvFields::FIELD_COUNT)),
        pictures: int(f.at(&AtvFields::PICTURES)),
        snow_pictures: int(f.at(&AtvFields::SNOW_PICTURES)),
        fields_ok: int(f.at(&AtvFields::FIELDS_OK)),
        fields_bad: int(f.at(&AtvFields::FIELDS_BAD)),
        fm_polarity: f
            .at(&AtvFields::FM_POLARITY)
            .map(|e| e.label().to_string())
            .unwrap_or_default(),
        nbtv_line_locked: f.at(&AtvFields::NBTV_LINE_LOCKED).unwrap_or(false),
        nbtv_picture_locked: f.at(&AtvFields::NBTV_PICTURE_LOCKED).unwrap_or(false),
        nicam: f.at(&AtvFields::NICAM_LOCKED).map(|locked| Nicam {
            locked,
            valid: f.at(&AtvFields::NICAM_VALID).unwrap_or(false),
            mode: f
                .at(&AtvFields::NICAM_MODE)
                .map(|e| e.label().to_string())
                .unwrap_or_default(),
            parity_errors: int(f.at(&AtvFields::NICAM_PARITY_ERRORS)),
        }),
        a2: Some(A2 {
            present: f.at(&AtvFields::A2_MODE).is_some(),
            mode: f
                .at(&AtvFields::A2_MODE)
                .map(|e| e.label().to_string())
                .unwrap_or_default(),
        }),
        teletext,
    }
}

/// A picture from the stack's picture stream, with what the results said when it came.
#[derive(Clone, Debug, PartialEq)]
pub struct AtvPicture {
    /// The picture.
    pub picture: Picture,
    /// The picture had colour.
    pub colour: bool,
}

/// The pictures of `out`.
pub fn pictures(out: &Output) -> impl Iterator<Item = Picture> + '_ {
    out.media
        .iter()
        .filter(|e| e.kind == StreamKind::Picture)
        .map(|e| Picture {
            codec: e.format.codec,
            layout: e.format.pixels.clone(),
            name: None,
            data: Arc::from(e.packet.bytes()),
        })
}

/// The results of a run.
pub struct Run {
    /// Interleaved stereo at 48 kHz.
    pub audio: Vec<f32>,
    /// The results after each piece that changed them.
    pub reports: Vec<Detail>,
    /// The last picture (not snow).
    pub picture: Option<AtvPicture>,
    /// Pictures decoded.
    pub pictures: u64,
    /// Snow pictures seen.
    pub snow: u64,
}

impl Run {
    pub fn last(&self) -> Detail {
        self.reports.last().cloned().unwrap_or_default()
    }
    /// The first time (s) at which `f` holds.
    pub fn first(&self, f: impl Fn(&Detail) -> bool) -> Option<f64> {
        self.reports.iter().find(|d| f(d)).map(|d| d.t)
    }
    /// Left and right of the last `seconds` of audio.
    pub fn tail(&self, seconds: f64) -> (Vec<f32>, Vec<f32>) {
        let frames = (seconds * 48_000.0) as usize;
        let n = self.audio.len() / 2;
        let from = n.saturating_sub(frames);
        sdrkit_dev_test::measure::deinterleave(&self.audio[2 * from..])
    }
}

/// Feeds `seconds` from `source` through `rx` in pieces of `piece`.
pub fn run(
    rx: &mut StackGraph,
    rate: f64,
    seconds: f64,
    piece: usize,
    mut source: impl FnMut(&mut [Complex32]),
) -> Run {
    let total = (seconds * rate) as usize;
    let mut buf = vec![Complex32::default(); piece];
    let mut out = Output::default();
    let mut r = Run {
        audio: Vec::new(),
        reports: Vec::new(),
        picture: None,
        pictures: 0,
        snow: 0,
    };
    let mut done = 0;
    let mut counter = Owner::results(rx).change_counter();
    while done < total {
        let n = piece.min(total - done);
        source(&mut buf[..n]);
        out.clear();
        rx.feed(&buf[..n], &mut out);
        done += n;
        r.audio.extend_from_slice(out.audio());
        let res = Owner::results(rx);
        let colour = res.fields().at(&AtvFields::COLOUR).unwrap_or(false);
        for p in pictures(&out) {
            r.picture = Some(AtvPicture { picture: p, colour });
        }
        if res.change_counter() != counter {
            counter = res.change_counter();
            r.reports.push(detail(res, done as f64 / rate));
        }
    }
    let last = r.last();
    r.pictures = last.pictures;
    r.snow = last.snow_pictures;
    r
}

/// Runs the generator with `p`, at 40 dB carrier to noise, into a fresh receiver.
pub fn gen_run(p: AtvGenParams, seconds: f64, settings: &[(&str, &str)]) -> (Run, AtvGenerator) {
    imp_run(p, Imp::cnr(40.0), seconds, settings)
}

/// The mean B, G, R over the middle half of each EBU bar (rows 40 to 172, columns 46 to 721 at 768 x 576; scaled for other sizes).
pub fn bars(p: &AtvPicture) -> Vec<[f64; 3]> {
    let l = p.picture.layout.as_ref().unwrap();
    let (w, h) = (l.width as usize, l.height as usize);
    let data = &p.picture.data;
    let (x0, x1) = (
        (0.06 * w as f64).round() as usize,
        (0.94 * w as f64).round() as usize,
    );
    let (y0, y1) = (
        (0.07 * h as f64).round() as usize,
        (0.30 * h as f64).round() as usize,
    );
    (0..8)
        .map(|k| {
            let a = x0 + (x1 - x0) * k / 8;
            let b = x0 + (x1 - x0) * (k + 1) / 8;
            let (ca, cb) = (a + (b - a) / 4, b - (b - a) / 4);
            let (ra, rb) = (y0 + (y1 - y0) / 4, y1 - (y1 - y0) / 4);
            let mut s = [0.0f64; 3];
            let mut n = 0.0;
            for r in ra..rb {
                for c in ca..cb {
                    let i = (r * w + c) * 4;
                    s[0] += data[i + 2] as f64;
                    s[1] += data[i + 1] as f64;
                    s[2] += data[i] as f64;
                    n += 1.0;
                }
            }
            [s[0] / n, s[1] / n, s[2] / n]
        })
        .collect()
}

/// The worst channel difference (0..1) of the bars against the EBU 100/0/75/0 values.
pub fn bars_error(p: &AtvPicture) -> f64 {
    let b = bars(p);
    let mut worst = 0.0f64;
    for (k, m) in b.iter().enumerate() {
        for c in 0..3 {
            worst = worst.max((m[c] / 255.0 - EBU_BARS[k][c]).abs());
        }
    }
    worst
}

/// The mean absolute luminance difference (0 to 255) of the picture against the card. The card is band-limited to 5 MHz like a
/// video chain (a windowed sinc along the rows). The best horizontal alignment within 2 pixels is used (the position tolerance of
/// 18.1). The patterns made of detail inside the colour band are left out.
pub fn luma_difference(p: &AtvPicture, card: &Planes) -> f64 {
    let l = p.picture.layout.as_ref().unwrap();
    let (w, h) = (l.width as usize, l.height as usize);
    if w != card.w || h != card.h {
        return 255.0;
    }
    // 768 pixels in 52 µs: Nyquist 7.38 MHz; 5 MHz is 0.68 of it.
    let cut = 0.68f64;
    let taps: Vec<f64> = (-12i64..=12)
        .map(|k| {
            let x = k as f64;
            let s = if k == 0 {
                cut
            } else {
                (std::f64::consts::PI * cut * x).sin() / (std::f64::consts::PI * x)
            };
            s * (0.54 + 0.46 * (std::f64::consts::PI * x / 13.0).cos())
        })
        .collect();
    let norm: f64 = taps.iter().sum();
    let d = &p.picture.data;
    let mut best = f64::MAX;
    for dx in -2i64..=2 {
        let mut s = 0.0;
        let mut n = 0.0;
        for r in (h / 20)..(h * 9 / 10) {
            for c in (w / 20)..(w * 19 / 20) {
                // Detail finer than a colour decoder's luminance band (the multiburst, the wedge, the checkerboard) is left out.
                let (fx, fy) = (c as f64 / w as f64, r as f64 / h as f64);
                // So are the moving box and the clock, which change from frame to frame.
                if ((0.33..0.47).contains(&fy) && fx >= 0.53)
                    || ((0.60..0.92).contains(&fy) && (fx < 0.31 || fx >= 0.69))
                    || fy >= 0.93
                {
                    continue;
                }
                let i = (r * w + c) * 4;
                let y = 0.299 * d[i + 2] as f64 + 0.587 * d[i + 1] as f64 + 0.114 * d[i] as f64;
                let cc = c as i64 + dx;
                let mut want = 0.0;
                for (j, t) in taps.iter().enumerate() {
                    let x = (cc + j as i64 - 12).clamp(0, w as i64 - 1) as usize;
                    want += t * card.y[r * w + x] as f64;
                }
                s += (y - want / norm * 255.0).abs();
                n += 1.0;
            }
        }
        best = best.min(s / n);
    }
    best
}

/// The worst luminance difference (0..1) of the bars against the EBU bars' luminance.
pub fn luma_error(p: &AtvPicture) -> f64 {
    bars(p)
        .iter()
        .zip(EBU_BARS.iter())
        .map(|(m, e)| {
            let y = (0.299 * m[0] + 0.587 * m[1] + 0.114 * m[2]) / 255.0;
            let w = 0.299 * e[0] + 0.587 * e[1] + 0.114 * e[2];
            (y - w).abs()
        })
        .fold(0.0, f64::max)
}

/// Whether the picture is grey (R - B small everywhere on the bars).
pub fn grey(p: &AtvPicture) -> f64 {
    bars(p)
        .iter()
        .map(|b| (b[0] - b[2]).abs() / 255.0)
        .fold(0.0, f64::max)
}

/// The amplitude of a tone in mono audio.
pub fn tone(x: &[f32], f: f64) -> f64 {
    sdrkit_dev_test::measure::tone_amplitude(x, 48_000.0, f)
}

/// Default generator parameters at a rate.
pub fn params(rate: f64) -> AtvGenParams {
    AtvGenParams {
        rate_hz: rate,
        ..Default::default()
    }
}

/// Runs a generator (any) into a receiver (any) and returns the receiver's results.
pub fn feed_any(
    g: &mut dyn Generator,
    rx: &mut dyn Receiver,
    seconds: f64,
    piece: usize,
) -> Vec<f32> {
    let rate = g.rate_hz();
    let total = (seconds * rate) as usize;
    let mut buf = vec![Complex32::default(); piece];
    let mut out = Output::default();
    let mut audio = Vec::new();
    let mut done = 0;
    while done < total {
        let n = piece.min(total - done);
        g.generate(&mut buf[..n]);
        out.clear();
        rx.feed(&buf[..n], &mut out);
        audio.extend_from_slice(out.audio());
        done += n;
    }
    audio
}

/// Impairments applied after a clean generator, through the framework's chain (`sdrkit_components::sim::impaired` with the
/// standard `gen_*` settings).
#[derive(Clone, Copy, Debug, Default)]
pub struct Imp {
    /// Carrier to noise: the peak vision carrier against the noise in 5 MHz (exactly 0: no noise).
    pub cnr_db: f64,
    /// Frequency offset, Hz.
    pub offset_hz: f64,
    /// Sample clock error, ppm.
    pub clock_ppm: f64,
    /// Echo: (delay in seconds, level dB).
    pub echo: Option<(f64, f64)>,
    /// An 8-bit converter, with the peak carrier this many dB below its full scale.
    pub bits8_headroom_db: Option<f64>,
    /// The spectrum inverted.
    pub inverted: bool,
}

impl Imp {
    /// Noise only.
    pub fn cnr(cnr_db: f64) -> Imp {
        Imp {
            cnr_db,
            ..Default::default()
        }
    }

    /// The `gen_*` settings of these impairments.
    pub fn values(&self) -> Values {
        let mut j = json!({
            "gen_seed": 7,
            "gen_offset_hz": self.offset_hz,
            "gen_clock_ppm": self.clock_ppm,
            "gen_inverted": self.inverted,
        });
        if self.cnr_db != 0.0 {
            j["gen_noise"] = json!("snr");
            j["gen_snr_db"] = json!(self.cnr_db);
        }
        if let Some((delay, db)) = self.echo {
            j["gen_fading"] = json!("echo");
            j["gen_echo_delay_s"] = json!(delay);
            j["gen_echo_db"] = json!(db);
        }
        if let Some(h) = self.bits8_headroom_db {
            j["gen_level_dbfs"] = json!(-h);
            j["gen_adc_bits"] = json!(8);
        }
        let mut v = Values::defaults(&sdrkit_components::sim::GenSetting::schema());
        v.update_from_json(&j).expect("the impairments are valid");
        v
    }
}

/// A generator through the impairments.
pub fn impaired(g: Box<dyn Generator>, imp: Imp) -> Box<dyn Generator> {
    sdrkit_components::sim::impaired(g, &imp.values()).expect("the impairments are valid")
}

/// Runs a clean generator through the impairments into a fresh receiver.
pub fn imp_run(
    p: AtvGenParams,
    imp: Imp,
    seconds: f64,
    settings: &[(&str, &str)],
) -> (Run, AtvGenerator) {
    let rate = p.rate_hz;
    let mut g = impaired(Box::new(AtvGenerator::new(p.clone())), imp);
    let mut rx = atv(rate, settings);
    let r = run(&mut rx, rate, seconds, 50_000, |b| {
        g.generate(b);
    });
    (r, AtvGenerator::new(p))
}

/// White noise with `sigma` on each of I and Q (`sdrkit_algo_sim::noise::Awgn`).
pub fn noise(sigma: f64, seed: u64) -> sdrkit_algo_sim::noise::Awgn {
    sdrkit_algo_sim::noise::Awgn::with_power(2.0 * sigma * sigma, seed).expect("a valid power")
}
