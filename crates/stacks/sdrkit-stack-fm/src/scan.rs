//! The band scan: it finds the stations of a band and their names. The engine drives it: it tunes where the scan requests,
//! passes it wide spectrum frames to find candidates, then the receiver's reports on each candidate. Times are seconds on any
//! clock that runs with the signal.

use crate::results::Reading;

/// What the scanner requests from its driver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScanCommand {
    /// Tune to this frequency for the spectrum sweep, with a sample rate of at least 2 MHz, and send spectrum frames
    /// ([`BandScan::on_spectrum`]).
    Survey {
        /// The centre frequency, Hz.
        centre_hz: f64,
    },
    /// Tune the FM receiver to this frequency (a retune: its results start empty) and send its reports ([`BandScan::on_report`]).
    Check {
        /// The candidate's frequency, Hz.
        hz: f64,
    },
    /// The scan is over: tune back to this frequency and unmute.
    Done {
        /// The frequency the scan started from, Hz.
        return_hz: f64,
    },
}

/// A station the scan found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanStation {
    /// Frequency, MHz.
    pub mhz: f64,
    /// The trimmed PS (a scrolling name's windows joined), or "Station" when it has none.
    pub name: String,
    /// The RadioText, or the PTY name when there is no text.
    pub note: String,
    /// PTY name ("None" left out).
    pub pty: String,
    /// The trimmed RadioText.
    pub text: String,
    /// The RadioText came in whole; when false `text` is the part received so far (missing segments blank).
    pub text_complete: bool,
    /// PI (0 = unknown).
    pub pi: u16,
    /// Traffic programme.
    pub tp: bool,
    /// SNR measure, dB.
    pub snr_db: f32,
    /// Stereo.
    pub stereo: bool,
    /// RDS sync was seen.
    pub rds: bool,
}

#[derive(Clone, Debug)]
enum Phase {
    Idle,
    Survey {
        hop: usize,
        since: f64,
    },
    Check {
        idx: usize,
        since: f64,
        track: Box<Track>,
    },
    Done,
}

#[derive(Clone, Debug, Default)]
struct Track {
    station: bool,
    rds_seen: bool,
    names: Vec<String>,
    name_since: f64,
    come_round: bool,
    last: Option<Reading>,
}

/// The band scanner.
#[derive(Clone, Debug)]
pub struct BandScan {
    low_hz: f64,
    high_hz: f64,
    full_rds: bool,
    return_hz: f64,
    centres: Vec<f64>,
    candidates: Vec<f64>,
    phase: Phase,
    /// The stations found so far, lowest first.
    pub stations: Vec<ScanStation>,
}

/// Two finds with one PI this close are one station heard twice (a strong carrier also shows 0.1 MHz off), Hz.
pub const SAME_STATION_HZ: f64 = 200e3;

/// A partial radio text with each run of blanks (the segments not yet received) made one space.
pub fn collapse_gaps(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Joins the windows of a scrolling name ("RADIO 1 ", "ADIO 1 H", ...) into one name.
pub fn join_windows(windows: &[String]) -> String {
    let mut out = String::new();
    for w in windows.iter().map(|w| w.trim_end()) {
        if out.is_empty() {
            out.push_str(w);
            continue;
        }
        if out.contains(w) {
            continue;
        }
        let a: Vec<char> = out.chars().collect();
        let b: Vec<char> = w.chars().collect();
        let k = (1..=a.len().min(b.len()))
            .rev()
            .find(|&k| a[a.len() - k..] == b[..k])
            .unwrap_or(0);
        if k == 0 {
            out.push(' ');
        }
        out.extend(&b[k..]);
    }
    out.trim().to_string()
}

impl BandScan {
    /// A scan of `low_hz` to `high_hz` (held to 76 to 108 MHz, at least 0.5 MHz), started at `return_hz`.
    pub fn new(low_hz: f64, high_hz: f64, full_rds: bool, return_hz: f64) -> Self {
        let low = low_hz.clamp(76e6, 107.5e6);
        let high = high_hz.clamp(low + 0.5e6, 108e6);
        let mut centres = Vec::new();
        let mut c = low + 1e6;
        while c - 1e6 < high {
            centres.push(c);
            c += 2e6;
        }
        BandScan {
            low_hz: low,
            high_hz: high,
            full_rds,
            return_hz,
            centres,
            candidates: Vec::new(),
            phase: Phase::Idle,
            stations: Vec::new(),
        }
    }

    /// Starts: the first command.
    pub fn start(&mut self, now_s: f64) -> ScanCommand {
        self.phase = Phase::Survey {
            hop: 0,
            since: now_s,
        };
        ScanCommand::Survey {
            centre_hz: self.centres[0],
        }
    }

    /// Whether the scan is over.
    pub fn done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    /// The candidates the spectrum sweep found so far, Hz.
    pub fn candidates(&self) -> &[f64] {
        &self.candidates
    }

    fn next_survey(&mut self, hop: usize, now_s: f64) -> ScanCommand {
        if hop + 1 < self.centres.len() {
            self.phase = Phase::Survey {
                hop: hop + 1,
                since: now_s,
            };
            return ScanCommand::Survey {
                centre_hz: self.centres[hop + 1],
            };
        }
        self.candidates.sort_by(f64::total_cmp);
        self.candidates.dedup_by(|a, b| (*a - *b).abs() < 1e3);
        self.next_check(0, now_s)
    }

    fn next_check(&mut self, idx: usize, now_s: f64) -> ScanCommand {
        match self.candidates.get(idx) {
            Some(&hz) => {
                self.phase = Phase::Check {
                    idx,
                    since: now_s,
                    track: Box::default(),
                };
                ScanCommand::Check { hz }
            }
            None => {
                self.phase = Phase::Done;
                ScanCommand::Done {
                    return_hz: self.return_hz,
                }
            }
        }
    }

    /// Adds a station, unless it is one already found on the next channel (the same PI within [`SAME_STATION_HZ`]): then the
    /// stronger of the two stays.
    fn add_station(&mut self, found: ScanStation) {
        let same = self.stations.iter().position(|s| {
            found.pi != 0
                && s.pi == found.pi
                && ((s.mhz - found.mhz) * 1e6).abs() <= SAME_STATION_HZ + 1.0
        });
        match same {
            Some(i) if found.snr_db > self.stations[i].snr_db => self.stations[i] = found,
            Some(_) => {}
            None => self.stations.push(found),
        }
    }

    /// A spectrum frame during the spectrum sweep: `power_db` covers `centre_hz - rate/2 .. centre_hz + rate/2`, lowest frequency first, with
    /// bin `n/2` at the centre (the DSP crate's spectrum analyser order).
    /// Returns the next command once the hop is measured.
    pub fn on_spectrum(
        &mut self,
        now_s: f64,
        centre_hz: f64,
        rate_hz: f64,
        power_db: &[f32],
    ) -> Option<ScanCommand> {
        let Phase::Survey { hop, since } = self.phase else {
            return None;
        };
        if (centre_hz - self.centres[hop]).abs() > 1.0 || now_s - since < 0.7 || power_db.is_empty()
        {
            return None;
        }
        let n = power_db.len();
        let bin = rate_hz / n as f64;
        let mut ch = [f64::MIN; 20];
        for (k, c) in ch.iter_mut().enumerate() {
            let off = -1.0e6 + k as f64 * 0.1e6;
            let (mut sum, mut cnt) = (0.0, 0);
            for (i, &p) in power_db.iter().enumerate() {
                let f = (i as f64 - (n / 2) as f64) * bin;
                if (f - off).abs() <= 50e3 && f.abs() >= 20e3 {
                    sum += 10f64.powf(p as f64 / 10.0);
                    cnt += 1;
                }
            }
            if cnt > 0 {
                *c = 10.0 * (sum / cnt as f64).log10();
            }
        }
        let mut sorted = ch;
        sorted.sort_by(f64::total_cmp);
        let floor = sorted[4];
        for k in 0..20 {
            let left = if k > 0 { ch[k - 1] } else { f64::MIN };
            let right = if k < 19 { ch[k + 1] } else { f64::MIN };
            let hz = centre_hz - 1.0e6 + k as f64 * 0.1e6;
            if ch[k] > floor + 9.0
                && ch[k] >= left
                && ch[k] >= right
                && hz >= self.low_hz
                && hz <= self.high_hz
            {
                self.candidates.push(hz);
            }
        }
        Some(self.next_survey(hop, now_s))
    }

    /// Time passing (call it now and then): skips a sweep hop that got no frame in 4 s.
    pub fn on_tick(&mut self, now_s: f64) -> Option<ScanCommand> {
        match self.phase {
            Phase::Survey { hop, since } if now_s - since > 4.0 => {
                Some(self.next_survey(hop, now_s))
            }
            _ => None,
        }
    }

    /// A report of the FM receiver tuned to the current candidate, read with [`Reading::from_results`]. Returns the next command
    /// when the candidate is finished.
    pub fn on_report(&mut self, now_s: f64, d: &Reading) -> Option<ScanCommand> {
        let full = self.full_rds;
        let Phase::Check { idx, since, track } = &mut self.phase else {
            return None;
        };
        let (idx, since) = (*idx, *since);
        let t = now_s - since;
        if t <= 0.9 {
            return None;
        }
        track.station |= d.carrier && d.state >= 1;
        track.rds_seen |= d.rds;
        if !d.station.is_empty() && track.names.last() != Some(&d.station) {
            if track.names.len() > 1 && track.names[0] == d.station {
                track.come_round = true;
            }
            if !track.come_round {
                track.names.push(d.station.clone());
            }
            track.name_since = t;
        }
        track.last = Some(d.clone());
        let named = !d.station.is_empty();
        let finished = if !track.station {
            t > 1.4
        } else if !full {
            (named && t > 1.6) || t >= 5.0
        } else {
            let scrolling = track.names.len() > 1;
            let name_round = if scrolling {
                track.come_round
            } else {
                named && t - track.name_since >= 4.0
            };
            let text_done = d.text_complete || t >= 6.0;
            let all = name_round && text_done && d.pi != 0;
            all || (!track.rds_seen && t >= 2.5)
                || (t >= 12.0 && !(scrolling && !track.come_round))
                || t >= 30.0
        };
        if !finished {
            return None;
        }
        if track.station {
            let last = track.last.clone().unwrap_or_default();
            let name = if track.names.len() > 1 {
                join_windows(&track.names)
            } else {
                last.station.trim().to_string()
            };
            let pty = if last.programme == "None" {
                String::new()
            } else {
                last.programme.clone()
            };
            let text = last.text.trim().to_string();
            let hz = self.candidates[idx];
            let complete = last.text_complete;
            let found = ScanStation {
                mhz: (hz / 1e3).round() / 1e3,
                name: if name.is_empty() {
                    "Station".into()
                } else {
                    name
                },
                note: if text.is_empty() {
                    pty.clone()
                } else if complete {
                    text.clone()
                } else {
                    format!("{} (partial)", collapse_gaps(&text))
                },
                pty,
                text,
                text_complete: complete,
                pi: last.pi,
                tp: last.traffic,
                snr_db: last.snr_db as f32,
                stereo: last.stereo,
                rds: track.rds_seen,
            };
            self.add_station(found);
        }
        Some(self.next_check(idx + 1, now_s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_join() {
        let w: Vec<String> = ["RADIO 1 ", "ADIO 1 H", "DIO 1 HI", "IO 1 HIT", "S       "]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(join_windows(&w), "RADIO 1 HIT S");
        assert_eq!(join_windows(&["NEWS".into(), "NEWS".into()]), "NEWS");
    }

    #[test]
    fn survey_finds_peaks_and_the_check_records_stations() {
        let mut s = BandScan::new(88e6, 90e6, false, 100e6);
        assert_eq!(s.start(0.0), ScanCommand::Survey { centre_hz: 89e6 });
        // A flat -100 dB spectrum at 2 MHz with stations at 88.5 and 89.3 MHz.
        let n = 2048;
        let spec: Vec<f32> = (0..n)
            .map(|i| {
                let f = 89e6 + (i as f64 - (n / 2) as f64) * 2e6 / n as f64;
                if (f - 88.5e6).abs() < 80e3 || (f - 89.3e6).abs() < 80e3 {
                    -60.0
                } else {
                    -100.0
                }
            })
            .collect();
        assert!(s.on_spectrum(0.5, 89e6, 2e6, &spec).is_none(), "too early");
        let cmd = s.on_spectrum(0.8, 89e6, 2e6, &spec).unwrap();
        assert_eq!(cmd, ScanCommand::Check { hz: 88.5e6 });
        assert_eq!(s.candidates(), &[88.5e6, 89.3e6]);
        // First candidate: a station with a name.
        let d = Reading {
            carrier: true,
            state: 2,
            station: "ONE     ".into(),
            programme: "News".into(),
            pi: 0x1111,
            ..Default::default()
        };
        assert!(s.on_report(1.0, &d).is_none());
        assert_eq!(
            s.on_report(2.6, &d),
            Some(ScanCommand::Check { hz: 89.3e6 })
        );
        // Second: nothing there.
        let none = Reading::default();
        assert!(s.on_report(3.8, &none).is_none());
        assert_eq!(
            s.on_report(4.3, &none),
            Some(ScanCommand::Done { return_hz: 100e6 })
        );
        assert!(s.done());
        assert_eq!(s.stations.len(), 1);
        assert_eq!(
            (
                s.stations[0].mhz,
                s.stations[0].name.as_str(),
                s.stations[0].note.as_str()
            ),
            (88.5, "ONE", "News")
        );
    }

    #[test]
    fn one_station_on_two_channels_is_found_once_and_a_partial_text_is_marked() {
        let mut s = BandScan::new(104e6, 106e6, false, 100e6);
        s.start(0.0);
        let n = 2048;
        let spec: Vec<f32> = (0..n)
            .map(|i| {
                let f = 105e6 + (i as f64 - (n / 2) as f64) * 2e6 / n as f64;
                if (f - 104.7e6).abs() < 30e3 || (f - 104.8e6).abs() < 30e3 {
                    -60.0
                } else {
                    -100.0
                }
            })
            .collect();
        assert_eq!(
            s.on_spectrum(0.8, 105e6, 2e6, &spec),
            Some(ScanCommand::Check { hz: 104.7e6 })
        );
        let station = |snr: f32, complete: bool| Reading {
            carrier: true,
            state: 2,
            station: "TRIPLE M".into(),
            programme: "Rock".into(),
            text: "     FOR     GREATEST SONG EVER ON".into(),
            text_complete: complete,
            pi: 0x2049,
            snr_db: snr as f64,
            ..Default::default()
        };
        // 104.7 MHz strong, 104.8 MHz weaker (the same PI): one station at 104.7.
        assert_eq!(
            s.on_report(2.6, &station(25.0, false)),
            Some(ScanCommand::Check { hz: 104.8e6 })
        );
        assert_eq!(
            s.on_report(4.5, &station(12.0, true)),
            Some(ScanCommand::Done { return_hz: 100e6 })
        );
        assert_eq!(s.stations.len(), 1);
        assert_eq!(s.stations[0].mhz, 104.7);
        assert!(!s.stations[0].text_complete);
        assert_eq!(s.stations[0].note, "FOR GREATEST SONG EVER ON (partial)");
        // A different PI next door is another station.
        let mut other = station(30.0, true);
        other.pi = 0x2050;
        s.add_station(ScanStation {
            mhz: 104.8,
            pi: other.pi,
            ..Default::default()
        });
        assert_eq!(s.stations.len(), 2);
    }

    #[test]
    fn a_hop_without_frames_is_skipped() {
        let mut s = BandScan::new(87e6, 91e6, true, 95e6);
        s.start(0.0);
        assert_eq!(
            s.on_tick(4.5),
            Some(ScanCommand::Survey { centre_hz: 90e6 })
        );
        assert_eq!(s.on_tick(9.0), Some(ScanCommand::Done { return_hz: 95e6 }));
    }
}
