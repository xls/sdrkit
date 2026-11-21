//! LBJ messages: the basic, detail and time messages, their fields, and the pairing of a basic message with
//! its detail message.

use sdrkit_proto_pager::pocsag::{Config, NumericSet, PocsagAssembler};
use sdrkit_proto_pager::{Page, ReadMode};
use sdrkit_core::publish::field;
use sdrkit_core::Setting;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The POCSAG settings of LBJ: numeric pages with the bracket characters as `)` and `(`, no voting, and pages that read on
/// past lost words (each lost word reads as five `?`).
pub fn pocsag_config() -> Config {
    Config {
        read: ReadMode::Numeric,
        numeric: NumericSet::Parentheses,
        combine: false,
        end_after_lost: 0,
        ..Config::default()
    }
}

/// Cuts the code words of a burst's batches (16 per batch, the sync words left out) into pages. `rel` holds 32 reliabilities
/// per code word (1.0 for a clean bit) for the soft repair of message words. Without them, only the hard repair runs.
pub fn pages(codewords: &[u32], rel: Option<&[f32]>) -> Vec<Page> {
    let mut out = Vec::new();
    let Ok(mut asm) = PocsagAssembler::new(pocsag_config()) else {
        return out;
    };
    for (i, &cw) in codewords.iter().enumerate() {
        let r = rel
            .and_then(|r| r.get(i * 32..i * 32 + 32))
            .and_then(|r| <&[f32; 32]>::try_from(r).ok());
        asm.push(i % 16, cw, r, 0.0, |p| out.push(p.clone()));
    }
    asm.end(0.0, |p| out.push(p.clone()));
    out
}

/// Basic message addresses.
pub const BASIC: [u32; 2] = [1234000, 1233999];
/// Detail message addresses.
pub const DETAIL: [u32; 2] = [1234002, 1234001];
/// The time message address.
pub const TIME: u32 = 1234008;

/// The setting of the class names of the locomotive type codes: the text of a list as [`Locomotives::parse`] reads it.
pub const LOCOMOTIVES: Setting<field::Text> = Setting::new("locomotives", "Locomotive classes", "", "").help(
    "The class names of the locomotive type codes, one code and its name a line (the data file \
     data/cell/locomotives.txt). Without it a locomotive shows its type code.",
);

/// The class names of locomotive type codes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Locomotives {
    classes: Vec<(String, String)>,
}

impl Locomotives {
    /// The list of `text`: one code, a space and the class name a line. Lines that start with `#` and lines without a
    /// space are left out.
    pub fn parse(text: &str) -> Locomotives {
        let classes = text
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once(' '))
            .map(|(a, b)| (a.to_owned(), b.trim().to_owned()))
            .collect();
        Locomotives { classes }
    }

    /// The class name of the locomotive type code `code` exactly as the list writes it (`"239"`, `"3073"`).
    pub fn class(&self, code: &str) -> Option<&str> {
        self.classes.iter().find(|(a, _)| a == code).map(|(_, b)| b.as_str())
    }

    /// The class name of an LBJ locomotive type code: the code whole, or the first three digits of a four-digit code
    /// whose last digit is 0.
    pub fn lbj_type(&self, code: &str) -> Option<&str> {
        self.class(code).or_else(|| {
            if code.len() == 4 && code.ends_with('0') {
                self.class(&code[..3])
            } else {
                None
            }
        })
    }

    /// The locomotive text "TYPE-NNNN" of an LBJ message (the raw code when the type is not known).
    pub fn lbj_text(&self, code: &str, number: &str) -> String {
        format!("{}-{}", self.lbj_type(code).unwrap_or(code), number)
    }
}

/// A position.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// Inside the checks of the message.
    pub valid: bool,
    /// WGS-84 degrees.
    pub latitude: f64,
    /// WGS-84 degrees.
    pub longitude: f64,
}

sdrkit_core::choice_enum! {
    /// The direction of a train: odd train numbers run down, even ones up.
    #[derive(Serialize, Deserialize)]
    pub enum Direction {
        /// Up.
        #[serde(rename = "up")]
        Up = ("up", "Up"),
        /// Down.
        #[serde(rename = "down")]
        Down = ("down", "Down"),
    }
}

/// One LBJ message as shown.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LbjMessage {
    /// Running id (a merged message replaces the row of this id).
    pub id: u64,
    /// Signal time of the burst, seconds.
    pub t: f64,
    /// Wall time, seconds since 1970, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// The page's address.
    pub address: u32,
    /// Train as shown (prefix + digits for a paired detail).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train: Option<String>,
    /// Train prefix ("K", "T" ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Train digits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_digits: Option<String>,
    /// The direction of the train.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<Direction>,
    /// Speed (km/h), empty when blank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_kmh: Option<String>,
    /// Kilometre post with one decimal, empty when blank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub km_post: Option<String>,
    /// "HXD3C-0931".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locomotive: Option<String>,
    /// The type code ("2390").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loco_code: Option<String>,
    /// The locomotive number ("0931").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loco_serial: Option<String>,
    /// Cab end, raw.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cab_end_raw: Option<String>,
    /// Route name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// Tail characters, raw.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail_raw: Option<String>,
    /// Position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    /// "08:12".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_time: Option<String>,
    /// A detail message merged with its basic message.
    pub paired: bool,
    /// Carries the detail fields.
    pub detailed: bool,
    /// No missing codeword, no '?', at most 2 corrected bits.
    pub clean: bool,
    /// Bits corrected.
    pub corrected_bits: u32,
    /// Level of the burst, dBFS.
    pub level_dbfs: f32,
    /// Carrier offset of the burst, Hz.
    pub carrier_offset_hz: f32,
    #[serde(skip)]
    missing: u32,
}

impl sdrkit_util_objects::Shift for LbjMessage {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The fields decoded from the text of a basic message.
#[derive(Clone, Debug, PartialEq)]
pub enum Basic {
    /// An empty frame.
    Blank,
    /// Rejected.
    Bad,
    /// Train digits, speed, kilometre post.
    Fields(String, String, String),
}

fn strip_zeros(s: &str) -> String {
    let t = s.trim_start_matches('0');
    if t.is_empty() {
        "0".into()
    } else {
        t.into()
    }
}

/// Parses a basic message's text.
pub fn parse_basic(page_text: &str) -> Basic {
    let chars: Vec<char> = page_text.chars().collect();
    let text: String = if chars.len() >= 65 {
        chars[..15].iter().collect()
    } else {
        page_text.to_string()
    };
    if text.chars().all(|c| matches!(c, ' ' | '-' | '*' | '?')) {
        return Basic::Blank;
    }
    let tc: Vec<char> = text.chars().collect();
    let (train, speed, km) = if tc.len() == 15 {
        let f = |a: usize, b: usize| tc[a..b].iter().collect::<String>().trim().to_string();
        (f(0, 5), f(5, 10), f(10, 15))
    } else {
        let toks: Vec<&str> = text.split_whitespace().collect();
        if toks.len() != 3 {
            return Basic::Bad;
        }
        (toks[0].into(), toks[1].into(), toks[2].into())
    };
    if train.is_empty() || train.len() > 5 || !train.chars().all(|c| c.is_ascii_digit()) {
        return Basic::Bad;
    }
    let speed = if speed.is_empty() {
        String::new()
    } else if speed.len() <= 3 && speed.chars().all(|c| c.is_ascii_digit()) {
        let v: u32 = speed.parse().unwrap_or(999);
        if v > 400 {
            return Basic::Bad;
        }
        v.to_string()
    } else {
        return Basic::Bad;
    };
    let km = if km.is_empty() {
        String::new()
    } else {
        let (neg, d) = match km.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, km.as_str()),
        };
        if d.is_empty() || d.len() > 5 || !d.chars().all(|c| c.is_ascii_digit()) {
            return Basic::Bad;
        }
        let v: u32 = d.parse().unwrap_or(0);
        format!("{}{}.{}", if neg { "-" } else { "" }, v / 10, v % 10)
    };
    Basic::Fields(strip_zeros(&train), speed, km)
}

fn nibble(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some(c as u8 - b'0'),
        '*' | '.' => Some(10),
        'U' => Some(11),
        ' ' => Some(12),
        '-' => Some(13),
        ')' | ']' => Some(14),
        '(' | '[' => Some(15),
        _ => None,
    }
}

fn hex_bytes(s: &[char]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for p in s.chunks(2) {
        let hi = nibble(p[0])?;
        let lo = nibble(*p.get(1)?)?;
        out.push(hi << 4 | lo);
    }
    Some(out)
}

/// The detail fields of a detail message's text (at least 47 characters).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Detail {
    /// Train prefix.
    pub prefix: String,
    /// Type code.
    pub loco_code: String,
    /// Number.
    pub loco_serial: String,
    /// Cab end.
    pub cab_end: String,
    /// Route (UTF-8).
    pub route: String,
    /// Position.
    pub position: Position,
    /// Tail.
    pub tail: String,
}

/// Parses a detail message's text; none when it is shorter than 47 characters.
pub fn parse_detail(text: &str) -> Option<Detail> {
    let c: Vec<char> = text.chars().collect();
    if c.len() < 47 {
        return None;
    }
    let s = |a: usize, b: usize| c[a..b.min(c.len())].iter().collect::<String>();
    let prefix = hex_bytes(&c[0..4])
        .map(|b| {
            let b: Vec<u8> = b.into_iter().filter(|&x| x != 0).collect();
            if b.iter().all(|x| (0x20..0x7F).contains(x)) {
                String::from_utf8(b).unwrap_or_default().trim().to_string()
            } else {
                String::new()
            }
        })
        .unwrap_or_default();
    let route = hex_bytes(&c[14..30])
        .map(|mut b| {
            while b.last().is_some_and(|&x| x == 0 || x == 0xFF) {
                b.pop();
            }
            match encoding_rs::GBK.decode_without_bom_handling_and_without_replacement(&b) {
                Some(s) if !s.chars().any(|ch| ch.is_control()) => s.trim().to_string(),
                _ => String::new(),
            }
        })
        .unwrap_or_default();
    let pos = s(30, 47);
    let position = if pos.chars().all(|ch| ch.is_ascii_digit()) && pos.len() == 17 {
        let n = |a: usize, b: usize| pos[a..b].parse::<f64>().unwrap_or(-1.0);
        let (lod, lom) = (n(0, 3), n(3, 9) / 1e4);
        let (lad, lam) = (n(9, 11), n(11, 17) / 1e4);
        let valid =
            lom < 60.0 && lam < 60.0 && (70.0..=140.0).contains(&lod) && lad > 0.0 && lad <= 60.0;
        Position {
            valid,
            latitude: lad + lam / 60.0,
            longitude: lod + lom / 60.0,
        }
    } else {
        Position::default()
    };
    Some(Detail {
        prefix,
        loco_code: s(4, 8),
        loco_serial: s(8, 12),
        cab_end: s(12, 14),
        route,
        position,
        tail: s(47, 50),
    })
}

/// The time message's "HH:MM", when valid.
pub fn parse_time(text: &str) -> Option<String> {
    let c: Vec<char> = text.chars().collect();
    if c.len() < 5 {
        return None;
    }
    let hh: String = c[1..3].iter().collect();
    let mm: String = c[3..5].iter().collect();
    let h: u32 = hh.parse().ok()?;
    let m: u32 = mm.parse().ok()?;
    (h < 24 && m < 60).then(|| format!("{hh}:{mm}"))
}

/// What the message stage does with a burst's pages.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Show a new message.
    Show(LbjMessage),
    /// Replace the row of this id with a merged message.
    Replace(u64, LbjMessage),
}

/// Counts and actions of one burst.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// What to show.
    pub actions: Vec<Action>,
    /// Empty frames.
    pub blank: u32,
    /// Messages rejected by the checks.
    pub failed: u32,
}

/// The pairing state.
#[derive(Debug, Default)]
pub struct Pairing {
    waiting_basic: Option<LbjMessage>,
    waiting_detail: Option<LbjMessage>,
    next_id: u64,
    locomotives: Arc<Locomotives>,
}

fn merge(detail: &LbjMessage, basic: &LbjMessage) -> LbjMessage {
    let mut m = detail.clone();
    let digits = basic.train_digits.clone().unwrap_or_default();
    m.train_digits = Some(digits.clone());
    m.train = Some(format!(
        "{}{}",
        m.prefix.clone().unwrap_or_default(),
        digits
    ));
    m.speed_kmh = basic.speed_kmh.clone();
    m.km_post = basic.km_post.clone();
    m.direction = basic.direction;
    m.paired = true;
    m.corrected_bits = detail.corrected_bits + basic.corrected_bits;
    m.clean = detail.clean && basic.clean;
    m.t = detail.t.max(basic.t);
    m
}

fn clean(text: &str, missing: u32, corrected: u32) -> bool {
    missing == 0 && !text.contains('?') && corrected <= 2
}

impl Pairing {
    /// Makes the next message ids follow `last` (after a restored state, so a new message never takes the id of a kept one).
    pub fn continue_after(&mut self, last: u64) {
        self.next_id = self.next_id.max(last);
    }

    /// Names the locomotive classes of the messages that follow from `list`.
    pub fn set_locomotives(&mut self, list: Arc<Locomotives>) {
        self.locomotives = list;
    }

    /// Forgets the remembered messages.
    pub fn reset(&mut self) {
        self.waiting_basic = None;
        self.waiting_detail = None;
    }
    fn new_msg(
        &mut self,
        address: u32,
        t: f64,
        corrected: u32,
        missing: u32,
        text: &str,
    ) -> LbjMessage {
        self.next_id += 1;
        LbjMessage {
            id: self.next_id,
            t,
            address,
            clean: clean(text, missing, corrected),
            corrected_bits: corrected,
            missing,
            ..Default::default()
        }
    }

    /// Handles one page of a burst at signal time `t`.
    pub fn page(
        &mut self,
        ric: u32,
        text: &str,
        missing: u32,
        corrected: u32,
        t: f64,
        out: &mut Outcome,
    ) {
        if BASIC.contains(&ric) {
            match parse_basic(text) {
                Basic::Blank => out.blank += 1,
                Basic::Bad => {
                    out.failed += 1;
                    self.reset();
                }
                Basic::Fields(train, speed, km) => {
                    let mut m = self.new_msg(ric, t, corrected, missing, text);
                    let odd = train
                        .chars()
                        .last()
                        .and_then(|c| c.to_digit(10))
                        .unwrap_or(0)
                        % 2
                        == 1;
                    m.direction = Some(if odd { Direction::Down } else { Direction::Up });
                    m.train = Some(train.clone());
                    m.train_digits = Some(train);
                    m.speed_kmh = Some(speed);
                    m.km_post = Some(km);
                    out.actions.push(Action::Show(m.clone()));
                    match self.waiting_detail.take() {
                        Some(d) if (0.0..=5.0).contains(&(m.t - d.t)) => {
                            let merged = merge(&d, &m);
                            out.actions.push(Action::Replace(d.id, merged));
                        }
                        _ => self.waiting_basic = Some(m),
                    }
                }
            }
            let chars: Vec<char> = text.chars().collect();
            if chars.len() >= 65 {
                let tail: String = chars[chars.len() - 50..].iter().collect();
                self.detail_page(DETAIL[0], &tail, missing, 0, t, out);
            }
        } else if DETAIL.contains(&ric) {
            self.detail_page(ric, text, missing, corrected, t, out);
        } else if ric == TIME {
            if let Some(hhmm) = parse_time(text) {
                let mut m = self.new_msg(ric, t, corrected, missing, text);
                m.network_time = Some(hhmm);
                out.actions.push(Action::Show(m));
            } else {
                out.failed += 1;
            }
        } else {
            self.reset();
        }
    }

    fn detail_page(
        &mut self,
        ric: u32,
        text: &str,
        missing: u32,
        corrected: u32,
        t: f64,
        out: &mut Outcome,
    ) {
        let d = if missing == 0 {
            parse_detail(text)
        } else {
            None
        };
        let Some(d) = d else {
            out.failed += 1;
            self.reset();
            return;
        };
        let mut m = self.new_msg(ric, t, corrected, missing, text);
        m.detailed = true;
        m.prefix = Some(d.prefix.clone());
        m.train = Some(d.prefix.clone());
        m.locomotive = Some(self.locomotives.lbj_text(&d.loco_code, &d.loco_serial));
        m.loco_code = Some(d.loco_code);
        m.loco_serial = Some(d.loco_serial);
        m.cab_end_raw = Some(d.cab_end);
        m.route = Some(d.route);
        m.tail_raw = Some(d.tail);
        m.position = Some(d.position);
        match self.waiting_basic.take() {
            Some(b) if (0.0..=5.0).contains(&(m.t - b.t)) => {
                out.actions.push(Action::Show(merge(&m, &b)));
            }
            _ => {
                out.actions.push(Action::Show(m.clone()));
                self.waiting_detail = Some(m);
            }
        }
    }
}

/// The detail test text.
pub const DETAIL_TEST_TEXT: &str = "4U002390093130U-(2 9U- (-(000011720927939053465000";

#[cfg(test)]
mod tests {
    use super::*;

    fn f(t: &str) -> Basic {
        parse_basic(t)
    }

    #[test]
    fn basic_fields() {
        assert_eq!(
            f("19533  67   382"),
            Basic::Fields("19533".into(), "67".into(), "38.2".into())
        );
        assert_eq!(
            f("123    23   123"),
            Basic::Fields("123".into(), "23".into(), "12.3".into())
        );
        assert_eq!(
            f("10482 112 12046"),
            Basic::Fields("10482".into(), "112".into(), "1204.6".into())
        );
        assert_eq!(
            f("23017   0    50"),
            Basic::Fields("23017".into(), "0".into(), "5.0".into())
        );
        assert_eq!(f("----- --- -----"), Basic::Blank);
        assert_eq!(f("19533 401   382"), Basic::Bad);
        assert_eq!(f("1A533  67   382"), Basic::Bad);
        assert_eq!(
            f("77 12 -3"),
            Basic::Fields("77".into(), "12".into(), "-0.3".into())
        );
    }

    #[test]
    fn detail_fields() {
        let d = parse_detail(DETAIL_TEST_TEXT).unwrap();
        assert_eq!(d.prefix, "K");
        assert_eq!(d.loco_code, "2390");
        assert_eq!(d.loco_serial, "0931");
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/cell/locomotives.txt");
        let locos = Locomotives::parse(&std::fs::read_to_string(path).unwrap());
        assert_eq!(locos.lbj_text(&d.loco_code, &d.loco_serial), "HXD3C-0931");
        assert_eq!(d.cab_end, "30");
        assert_eq!(d.route, "津山线");
        assert!(d.position.valid);
        assert!((d.position.longitude - 117.3487983).abs() < 1e-6);
        assert!((d.position.latitude - 39.0891083).abs() < 1e-6);
        assert_eq!(d.tail, "000");
        // Position checks.
        let with = |pos: &str| {
            let mut t = DETAIL_TEST_TEXT.to_string();
            t.replace_range(30..47, pos);
            parse_detail(&t).unwrap().position.valid
        };
        assert!(!with("11760927939053465"));
        assert!(!with("06920927939053465"));
        assert!(!with("14120927939053465"));
        assert!(!with("11720927900053465"));
        assert!(!with("11720927961053465"));
        // Route bytes that are not GBK.
        let mut t = DETAIL_TEST_TEXT.to_string();
        t.replace_range(14..30, "(((((((((((((((("); // FF FF ... (all dropped) -> empty but valid
        assert_eq!(parse_detail(&t).unwrap().route, "");
        let mut t = DETAIL_TEST_TEXT.to_string();
        t.replace_range(14..30, "8(000000(0000000"); // 0x8F 0x00 ... 0xF0: not GBK
        assert_eq!(parse_detail(&t).unwrap().route, "");
        assert_eq!(locos.lbj_text("2391", "0931"), "2391-0931");
        assert_eq!(Locomotives::default().lbj_text("2390", "0931"), "2390-0931");
        assert_eq!(parse_time(" 0812"), Some("08:12".into()));
        assert_eq!(parse_time(" 2512"), None);
    }

    fn shown(o: &Outcome) -> Vec<LbjMessage> {
        o.actions
            .iter()
            .map(|a| match a {
                Action::Show(m) | Action::Replace(_, m) => m.clone(),
            })
            .collect()
    }

    #[test]
    fn pairing() {
        // Basic then detail within 5 s.
        let mut p = Pairing::default();
        let mut o = Outcome::default();
        p.page(1234000, "123    23   123", 0, 0, 10.0, &mut o);
        p.page(1234002, DETAIL_TEST_TEXT, 0, 0, 10.0, &mut o);
        let s = shown(&o);
        assert_eq!(s.len(), 2);
        assert!(!s[0].paired);
        assert!(s[1].paired);
        assert_eq!(s[1].train.as_deref(), Some("K123"));
        assert_eq!(s[1].km_post.as_deref(), Some("12.3"));
        // Detail then basic: detail shown, then replaced by the merged one.
        let mut p = Pairing::default();
        let mut o = Outcome::default();
        p.page(1234002, DETAIL_TEST_TEXT, 0, 0, 10.0, &mut o);
        p.page(1234000, "123    23   123", 0, 0, 12.0, &mut o);
        assert!(matches!(o.actions[0], Action::Show(_)));
        let id = match &o.actions[0] {
            Action::Show(m) => m.id,
            _ => 0,
        };
        assert!(matches!(&o.actions[2], Action::Replace(i, m) if *i == id && m.paired));
        // 6 s apart: not paired.
        let mut p = Pairing::default();
        let mut o = Outcome::default();
        p.page(1234000, "123    23   123", 0, 0, 10.0, &mut o);
        p.page(1234002, DETAIL_TEST_TEXT, 0, 0, 16.0, &mut o);
        assert!(shown(&o).iter().all(|m| !m.paired));
        // Another address between them breaks the pairing.
        let mut p = Pairing::default();
        let mut o = Outcome::default();
        p.page(1234000, "123    23   123", 0, 0, 10.0, &mut o);
        p.page(555, "12345", 0, 0, 10.0, &mut o);
        p.page(1234002, DETAIL_TEST_TEXT, 0, 0, 10.0, &mut o);
        assert!(shown(&o).iter().all(|m| !m.paired));
        // One basic pairs with one detail only.
        let mut p = Pairing::default();
        let mut o = Outcome::default();
        p.page(1234000, "123    23   123", 0, 0, 10.0, &mut o);
        p.page(1234002, DETAIL_TEST_TEXT, 0, 0, 10.0, &mut o);
        p.page(1234002, DETAIL_TEST_TEXT, 0, 0, 11.0, &mut o);
        assert_eq!(shown(&o).iter().filter(|m| m.paired).count(), 1);
        // A 65-character basic carries its detail.
        let mut p = Pairing::default();
        let mut o = Outcome::default();
        let long = format!("123    23   123{DETAIL_TEST_TEXT}");
        p.page(1234000, &long, 0, 0, 10.0, &mut o);
        let s = shown(&o);
        assert_eq!(s.len(), 2);
        assert!(s[1].paired && s[1].detailed);
    }
}
