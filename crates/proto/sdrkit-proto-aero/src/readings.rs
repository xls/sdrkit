//! Fixed-format readings of ACARS texts: the OOOI reports of the Q labels and the media advisory (label SA).
//!
//! OOOI means "out, off, on, in": the times at which an aircraft leaves the gate, takes off, lands and arrives at the gate.
//! Standard: ARINC 620 (not freely available).
//!
//! The readers do not allocate. The `Display` forms give one line of text.
//!
//! ```
//! use sdrkit_proto_aero::readings::{oooi, media_advisory};
//! assert_eq!(oooi("QA", "OMDB1030").unwrap().to_string(), "from OMDB out 1030");
//! let m = media_advisory("0EV123456SH").unwrap();
//! assert_eq!(m.to_string(), "link established: VHF ACARS at 12:34:56, available: default SATCOM, HF");
//! ```

use std::fmt;

/// A time of day in hours and minutes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hhmm {
    /// 0 to 23.
    pub h: u8,
    /// 0 to 59.
    pub m: u8,
}

impl fmt::Display for Hhmm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}{:02}", self.h, self.m)
    }
}

/// An OOOI report. Each field holds a value when the label's layout has it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Oooi {
    /// The departure airport (ICAO code).
    pub from: Option<[u8; 4]>,
    /// The destination airport (ICAO code).
    pub to: Option<[u8; 4]>,
    /// Gate out time.
    pub out: Option<Hhmm>,
    /// Wheels off time.
    pub off: Option<Hhmm>,
    /// Wheels on time.
    pub on: Option<Hhmm>,
    /// Gate in time.
    pub in_: Option<Hhmm>,
    /// Estimated time of arrival.
    pub eta: Option<Hhmm>,
}

#[derive(Clone, Copy)]
enum F {
    From,
    To,
    Out,
    Off,
    On,
    In,
    Eta,
}

fn layout(label: &str) -> Option<&'static [(F, usize)]> {
    use F::*;
    Some(match label {
        "Q1" => &[(From, 0), (Out, 4), (Off, 8), (On, 12), (In, 16), (To, 24)],
        "Q2" => &[(From, 0), (Eta, 4)],
        "QA" | "QH" => &[(From, 0), (Out, 4)],
        "QB" => &[(From, 0), (Off, 4)],
        "QC" => &[(From, 0), (On, 4)],
        "QD" => &[(From, 0), (In, 4)],
        "QE" => &[(From, 0), (Out, 4), (To, 8)],
        "QF" => &[(From, 0), (Off, 4), (To, 8)],
        "QG" => &[(From, 0), (Out, 4), (In, 8)],
        "QK" => &[(From, 0), (On, 4), (To, 8)],
        "QL" => &[(To, 0), (In, 8), (From, 13)],
        "QM" => &[(To, 0), (From, 8)],
        "QN" => &[(To, 4), (Eta, 8)],
        "QP" => &[(From, 0), (To, 4), (Out, 8)],
        "QQ" => &[(From, 0), (To, 4), (Off, 8)],
        "QR" => &[(From, 0), (To, 4), (On, 8)],
        "QS" => &[(From, 0), (To, 4), (In, 8)],
        "QT" => &[(From, 0), (To, 4), (Out, 8), (In, 12)],
        _ => return None,
    })
}

/// The OOOI report in the text of a Q label. `None` when the label has no OOOI layout or a field does not fit. An airport must be
/// four capital letters, and a time must be HHMM.
pub fn oooi(label: &str, text: &str) -> Option<Oooi> {
    let b = text.as_bytes();
    let mut o = Oooi::default();
    for &(f, at) in layout(label)? {
        let s: [u8; 4] = b.get(at..at + 4)?.try_into().ok()?;
        match f {
            F::From | F::To => {
                if !s.iter().all(u8::is_ascii_uppercase) {
                    return None;
                }
                if matches!(f, F::From) {
                    o.from = Some(s);
                } else {
                    o.to = Some(s);
                }
            }
            _ => {
                if !s.iter().all(u8::is_ascii_digit) {
                    return None;
                }
                let h = (s[0] - b'0') * 10 + (s[1] - b'0');
                let m = (s[2] - b'0') * 10 + (s[3] - b'0');
                if h > 23 || m > 59 {
                    return None;
                }
                let t = Some(Hhmm { h, m });
                match f {
                    F::Out => o.out = t,
                    F::Off => o.off = t,
                    F::On => o.on = t,
                    F::In => o.in_ = t,
                    _ => o.eta = t,
                }
            }
        }
    }
    Some(o)
}

impl fmt::Display for Oooi {
    /// "from OMDB out 0714 off 0722 on 1451 in 1459 to EGLL". The destination comes last when the report has a departure airport
    /// and a time, and second otherwise.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = |c: &[u8; 4]| std::str::from_utf8(c).unwrap_or("????").to_owned();
        let times = [
            ("out", self.out),
            ("off", self.off),
            ("on", self.on),
            ("in", self.in_),
            ("eta", self.eta),
        ];
        let any_time = times.iter().any(|t| t.1.is_some());
        let to_last = self.from.is_some() && any_time;
        let mut sep = "";
        let mut word = |f: &mut fmt::Formatter<'_>, name: &str, v: &dyn fmt::Display| {
            let r = write!(f, "{sep}{name} {v}");
            sep = " ";
            r
        };
        if let Some(c) = &self.from {
            word(f, "from", &code(c))?;
        }
        if let (Some(c), false) = (&self.to, to_last) {
            word(f, "to", &code(c))?;
        }
        for (name, t) in times {
            if let Some(t) = t {
                word(f, name, &t)?;
            }
        }
        if let (Some(c), true) = (&self.to, to_last) {
            word(f, "to", &code(c))?;
        }
        Ok(())
    }
}

/// A link named in a media advisory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// `V`: VHF ACARS.
    VhfAcars,
    /// `S`: the default satellite link.
    DefaultSatcom,
    /// `H`: HF data link.
    Hf,
    /// `G`: Globalstar.
    Globalstar,
    /// `C`: ICO satellite link.
    Ico,
    /// `2`: VDL Mode 2.
    Vdl2,
    /// `X`: Inmarsat Aero.
    InmarsatAero,
    /// `I`: Iridium.
    Iridium,
}

impl Link {
    /// The link of a media advisory character.
    pub fn from_char(c: u8) -> Option<Link> {
        Some(match c {
            b'V' => Link::VhfAcars,
            b'S' => Link::DefaultSatcom,
            b'H' => Link::Hf,
            b'G' => Link::Globalstar,
            b'C' => Link::Ico,
            b'2' => Link::Vdl2,
            b'X' => Link::InmarsatAero,
            b'I' => Link::Iridium,
            _ => return None,
        })
    }

    /// The name shown.
    pub fn name(self) -> &'static str {
        match self {
            Link::VhfAcars => "VHF ACARS",
            Link::DefaultSatcom => "default SATCOM",
            Link::Hf => "HF",
            Link::Globalstar => "Globalstar",
            Link::Ico => "ICO SATCOM",
            Link::Vdl2 => "VDL2",
            Link::InmarsatAero => "Inmarsat Aero",
            Link::Iridium => "Iridium",
        }
    }
}

/// The most links a media advisory lists as available.
pub const MAX_AVAILABLE: usize = 8;

/// A media advisory (label SA): the link the aircraft uses, since when, and the other links available to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaAdvisory {
    /// True when the link is established, false when it is lost.
    pub established: bool,
    /// The link that changed.
    pub link: Link,
    /// The time of the change: hours, minutes, seconds.
    pub time: (u8, u8, u8),
    available: [Option<Link>; MAX_AVAILABLE],
}

impl MediaAdvisory {
    /// The other links available.
    pub fn available(&self) -> impl Iterator<Item = Link> + '_ {
        self.available.iter().map_while(|x| *x)
    }
}

/// The media advisory of a label SA text. The text holds version `0`, then `E` (established) or `L` (lost), the link
/// character and HHMMSS. The available links follow, up to an optional `/` and free text. `None` when the text does not fit.
pub fn media_advisory(text: &str) -> Option<MediaAdvisory> {
    let b = text.as_bytes();
    if b.len() < 9 || b[0] != b'0' || !matches!(b[1], b'E' | b'L') {
        return None;
    }
    let link = Link::from_char(b[2])?;
    if !b[3..9].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let two = |i: usize| (b[i] - b'0') * 10 + (b[i + 1] - b'0');
    let (h, m, s) = (two(3), two(5), two(7));
    if h > 23 || m > 59 || s > 59 {
        return None;
    }
    let mut available = [None; MAX_AVAILABLE];
    for (k, &c) in b[9..].iter().take_while(|&&c| c != b'/').enumerate() {
        *available.get_mut(k)? = Some(Link::from_char(c)?);
    }
    Some(MediaAdvisory {
        established: b[1] == b'E',
        link,
        time: (h, m, s),
        available,
    })
}

impl fmt::Display for MediaAdvisory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (h, m, s) = self.time;
        write!(
            f,
            "link {}: {} at {h:02}:{m:02}:{s:02}",
            if self.established {
                "established"
            } else {
                "lost"
            },
            self.link.name()
        )?;
        for (i, l) in self.available().enumerate() {
            write!(
                f,
                "{}{}",
                if i == 0 { ", available: " } else { ", " },
                l.name()
            )?;
        }
        Ok(())
    }
}
