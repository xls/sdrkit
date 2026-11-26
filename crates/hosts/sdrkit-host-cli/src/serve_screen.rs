//! The live screen of `serve`: what the radio does, what goes out, each client and its last requests. [`lines`] makes
//! the text of one screen from a [`Screen`], with no terminal; `serve` draws it in place.

use crate::text;
use sdrkit_driver_core::{arrive_text, fmt_m};

/// The radio of the share.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Radio {
    /// The name of the radio, with the end of its serial number where its driver gives one.
    pub name: String,
    /// The driver.
    pub driver: String,
    /// Who steers the radio, in words.
    pub control: String,
    /// The centre frequency, Hz.
    pub centre_hz: f64,
    /// The sample rate, Hz.
    pub rate_hz: f64,
    /// The settings that the screen shows (gain and its stages, automatic gain, bandwidth, antenna, bias tee), as
    /// label and value with its unit.
    pub settings: Vec<(String, String)>,
    /// The mean power of the signal, dBFS.
    pub level_dbfs: Option<f32>,
    /// The peak power of the last block, dBFS.
    pub peak_dbfs: Option<f32>,
    /// Samples that the radio lost.
    pub lost: i64,
}

/// One client.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Client {
    /// The number of the client in the share.
    pub id: u64,
    /// Its address.
    pub address: String,
    /// The protocol and what carries its stream: "rtl_tcp", "SoapyRemote TCP".
    pub protocol: String,
    /// The sample format on the wire: "cs8".
    pub format: String,
    /// The rate it asked for last, Hz.
    pub asked_hz: Option<f64>,
    /// The rate it gets, Hz.
    pub gets_hz: f64,
    /// The rate of the radio, Hz: above `gets_hz` when the share cuts the rate down for this client.
    pub capture_hz: f64,
    /// Megabits a second written to it, measured from the bytes.
    pub mbit_s: f64,
    /// The part of its samples that reached it over the last window, if one was counted.
    pub part: Option<f64>,
    /// How long it is connected, s.
    pub connected_s: f64,
}

/// One request of a client, in words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Note {
    /// How long ago, s.
    pub age_s: f64,
    /// The client: "client 2"; the table of the clients has its address.
    pub who: String,
    /// What it asked for and what came of it: "frequency 106.5 MHz", "not carried out: bandwidth 5 MHz (follow mode)".
    pub text: String,
}

/// What one screen shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Screen {
    /// The radio.
    pub radio: Radio,
    /// Bytes a second written to every client, measured.
    pub bytes_per_s: f64,
    /// How long the share runs, s.
    pub uptime_s: f64,
    /// The clients.
    pub clients: Vec<Client>,
    /// The last requests, the newest first.
    pub notes: Vec<Note>,
    /// Where clients connect: "192.168.10.20:55140 (SoapyRemote)".
    pub addresses: Vec<String>,
    /// Why this computer falls behind, when it does: the sender found no free buffer, or the ring of the radio fills.
    pub behind: Option<String>,
}

/// The requests that the screen shows.
pub const NOTES: usize = 6;
/// A peak at or above this is clipping, dBFS.
const CLIP_DBFS: f32 = -0.5;

/// A time as `h:mm:ss`.
fn clock(s: f64) -> String {
    let s = s.max(0.0) as u64;
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// A rate in Msps.
fn msps(hz: f64) -> String {
    format!("{} Msps", fmt_m(hz))
}

/// `s` cut to `width` characters.
fn fit(s: &str, width: usize) -> String {
    s.chars().take(width).collect()
}

/// The cells of `row` padded to `widths`, with two spaces between them. A width of 0 drops the column.
fn row(cells: &[String], widths: &[usize]) -> String {
    let mut out = String::new();
    for (c, w) in cells.iter().zip(widths) {
        if *w == 0 {
            continue;
        }
        if !out.is_empty() {
            out.push_str("  ");
        }
        let c = fit(c, *w);
        out.push_str(&format!("{c:<w$}"));
    }
    out.trim_end().to_owned()
}

/// How the samples of the client arrive: "all arrive", or "2.1 of 2.4 Msps arrive (88 %)".
fn arrive(c: &Client) -> String {
    match c.part {
        Some(p) if p < 0.97 => arrive_text(p * c.gets_hz, c.gets_hz),
        Some(_) => "all arrive".to_owned(),
        None => "-".to_owned(),
    }
}

/// The lines of screen `s` on a terminal `width` columns wide. No line is longer than `width`. Below 80 columns the
/// table of the clients drops the rate asked for and the time connected.
pub fn lines(s: &Screen, width: usize) -> Vec<String> {
    let r = &s.radio;
    let mut out = Vec::new();
    out.push(format!(
        "sdrkit serve, up {}. Keys: f follow, r remote, s slice; q or Ctrl-C stops.",
        clock(s.uptime_s)
    ));
    out.push(String::new());
    out.push(format!("Radio: {} ({})", r.name, r.driver));
    out.push(format!("  {}", r.control));
    out.push(format!(
        "  centre {}, rate {}",
        text::hz(r.centre_hz),
        msps(r.rate_hz)
    ));
    let settings: Vec<String> = r.settings.iter().map(|(k, v)| format!("{k} {v}")).collect();
    // The settings fill lines of the width.
    let mut line = String::from(" ");
    for part in settings {
        if line.len() > 2 && line.len() + part.len() + 2 > width {
            out.push(line);
            line = String::from(" ");
        }
        line.push(' ');
        line.push_str(&part);
        line.push(',');
    }
    if line.len() > 2 {
        line.pop();
        out.push(line);
    }
    let level = match (r.level_dbfs, r.peak_dbfs) {
        (Some(l), Some(p)) => {
            let clip = if p >= CLIP_DBFS {
                ": CLIPPING, lower the gain"
            } else {
                ""
            };
            format!("level {l:.1} dBFS, peak {p:.1} dBFS{clip}")
        }
        _ => "level: no samples yet".to_owned(),
    };
    out.push(format!("  {level}, {} samples lost at the radio", r.lost));
    if let Some(b) = &s.behind {
        out.push(format!("  THIS COMPUTER FALLS BEHIND: {b}"));
    }
    out.push(String::new());
    let n = s.clients.len();
    out.push(format!(
        "Sent: {:.2} MB/s, {:.1} Mbit/s, to {n} {}",
        s.bytes_per_s / 1e6,
        s.bytes_per_s * 8.0 / 1e6,
        if n == 1 { "client" } else { "clients" }
    ));
    // One line of figures for each client, and one line of words under it. Below 80 columns the rate asked for goes.
    let narrow = width < 80;
    let widths: [usize; 7] = [2, 21, 15, 6, if narrow { 0 } else { 9 }, 9, 6];
    if n > 0 {
        out.push(row(
            &[
                "#", "address", "protocol", "format", "asks", "gets", "Mbit/s",
            ]
            .map(String::from),
            &widths,
        ));
    }
    for c in &s.clients {
        out.push(row(
            &[
                c.id.to_string(),
                c.address.clone(),
                c.protocol.clone(),
                c.format.clone(),
                c.asked_hz.map_or("-".to_owned(), msps),
                msps(c.gets_hz),
                format!("{:.1}", c.mbit_s),
            ],
            &widths,
        ));
        let cut = if c.capture_hz > c.gets_hz * 1.001 {
            format!(", cut from {}", msps(c.capture_hz))
        } else {
            String::new()
        };
        out.push(format!(
            "    {}, connected {}{cut}",
            arrive(c),
            clock(c.connected_s)
        ));
    }
    out.push(String::new());
    out.push("Requests, the newest first, with how long ago:".to_owned());
    if s.notes.is_empty() {
        out.push("  none yet".to_owned());
    }
    for note in s.notes.iter().take(NOTES) {
        out.push(format!(
            "  {}  {}: {}",
            clock(note.age_s),
            note.who,
            note.text
        ));
    }
    out.push(String::new());
    out.push(format!("Connect to: {}", s.addresses.join(", ")));
    out.into_iter().map(|l| fit(&l, width)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Screen {
        Screen {
            radio: Radio {
                name: "RTL2832U R820T Generic".into(),
                driver: "rtlusb".into(),
                control: "the first client steers the radio: client 2".into(),
                centre_hz: 104.9e6,
                rate_hz: 2.4e6,
                settings: vec![
                    ("gain".into(), "36 dB".into()),
                    ("automatic gain".into(), "off".into()),
                    ("bias tee".into(), "off".into()),
                ],
                level_dbfs: Some(-23.14),
                peak_dbfs: Some(-0.2),
                lost: 0,
            },
            bytes_per_s: 4.8e6,
            uptime_s: 3725.0,
            clients: vec![Client {
                id: 2,
                address: "192.168.10.155:61742".into(),
                protocol: "SoapyRemote TCP".into(),
                format: "cs8".into(),
                asked_hz: Some(0.5e6),
                gets_hz: 0.5e6,
                capture_hz: 2.4e6,
                mbit_s: 8.04,
                part: Some(0.88),
                connected_s: 65.0,
            }],
            notes: vec![Note {
                age_s: 3.0,
                who: "client 2".into(),
                text: "not carried out: bandwidth 5 MHz (follow mode)".into(),
            }],
            addresses: vec!["192.168.10.20:55140 (SoapyRemote)".into()],
            behind: Some("12 blocks found no buffer".into()),
        }
    }

    #[test]
    fn a_screen_at_80_columns() {
        let l = lines(&screen(), 80);
        assert!(l.iter().all(|x| x.chars().count() <= 80), "{l:#?}");
        assert_eq!(
            l[0],
            "sdrkit serve, up 1:02:05. Keys: f follow, r remote, s slice; q or Ctrl-C stops."
        );
        assert!(
            l.contains(&"  THIS COMPUTER FALLS BEHIND: 12 blocks found no buffer".to_owned()),
            "{l:#?}"
        );
        assert!(
            l.contains(&"  centre 104.9 MHz, rate 2.4 Msps".to_owned()),
            "{l:#?}"
        );
        assert!(
            l.contains(&"  gain 36 dB, automatic gain off, bias tee off".to_owned()),
            "{l:#?}"
        );
        assert!(l
            .iter()
            .any(|x| x.contains("level -23.1 dBFS, peak -0.2 dBFS: CLIPPING")));
        assert!(
            l.contains(&"Sent: 4.80 MB/s, 38.4 Mbit/s, to 1 client".to_owned()),
            "{l:#?}"
        );
        let i = l.iter().position(|x| x.starts_with("2 ")).unwrap();
        for part in [
            "192.168.10.155:61742",
            "SoapyRemote TCP",
            "cs8",
            "0.5 Msps",
            "8.0",
        ] {
            assert!(l[i].contains(part), "{part} in {}", l[i]);
        }
        assert_eq!(
            l[i + 1],
            "    0.44 of 0.5 Msps arrive (88 %), connected 0:01:05, cut from 2.4 Msps"
        );
        assert!(l.contains(
            &"  0:00:03  client 2: not carried out: bandwidth 5 MHz (follow mode)".to_owned()
        ));
        assert_eq!(
            l.last().unwrap(),
            "Connect to: 192.168.10.20:55140 (SoapyRemote)"
        );
    }

    #[test]
    fn a_narrow_screen_drops_columns_and_cuts_lines() {
        let l = lines(&screen(), 50);
        assert!(l.iter().all(|x| x.chars().count() <= 50), "{l:#?}");
        let head = l.iter().find(|x| x.starts_with("# ")).unwrap();
        assert!(!head.contains("asks"), "{head}");
    }
}
