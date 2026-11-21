//! The ensemble scan: tunes through a list of channels and records the ensembles found. It decides from the receiver's results
//! when to move on.

use crate::channels::Channel;
use crate::typed::{DataFields, EnsembleFields, ServiceRow, SignalFields, SyncFields, SERVICES};
use sdrkit_core::{LockState, Results};

/// What the scan reads from the receiver at each look.
#[derive(Clone, Debug, Default)]
pub struct Observation {
    /// Seconds since the tune.
    pub since_tune_s: f64,
    /// Locked, and for how long.
    pub locked_s: Option<f64>,
    /// The ensemble is valid, labelled, and every service labelled.
    pub complete: bool,
    /// The ensemble is valid.
    pub valid: bool,
    /// The quiet time in ms (section 4.1).
    pub quiet_ms: f64,
    /// Null symbols analysed for transmitter identification (when collected).
    pub tii_frames: Option<u32>,
    /// The ensemble label, SNR and service labels (for the record).
    pub label: String,
    /// SNR measure.
    pub snr_db: f32,
    /// Service labels ("?" for an unlabelled one).
    pub services: Vec<String>,
}

impl Observation {
    /// What the scan reads from the receiver's results, `since_tune_s` after the tune and `locked_s` after the lock (`None`
    /// while not locked). With `collect_tii` the scan also waits for 40 null symbols analysed (or 5 s of lock), so the
    /// transmitters are known.
    pub fn from_results(
        r: &Results,
        since_tune_s: f64,
        locked_s: Option<f64>,
        collect_tii: bool,
    ) -> Self {
        let f = r.fields();
        let services: Vec<String> = SERVICES
            .rows::<ServiceRow>(r)
            .map(|(_, row)| match row.label.map(str::trim) {
                Some(l) if !l.is_empty() => l.to_string(),
                _ => "?".to_string(),
            })
            .collect();
        let valid = f.at(&EnsembleFields::ENSEMBLE_VALID).unwrap_or(false);
        let label = f
            .at(&EnsembleFields::ENSEMBLE_LABEL)
            .map_or(String::new(), |l| l.trim().to_string());
        Observation {
            since_tune_s,
            locked_s: if r.status().lock_state == LockState::Locked {
                locked_s
            } else {
                None
            },
            complete: valid
                && !label.is_empty()
                && !services.is_empty()
                && services.iter().all(|s| s != "?"),
            valid,
            quiet_ms: f.at(&SyncFields::QUIET).unwrap_or(0.0) * 1000.0,
            tii_frames: collect_tii.then(|| f.at(&DataFields::TII_ANALYSED).unwrap_or(0) as u32),
            label,
            snr_db: f.at(&SignalFields::SNR_DB).unwrap_or(0.0) as f32,
            services,
        }
    }
}

/// One ensemble found.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// Channel name.
    pub channel: &'static str,
    /// Frequency in Hz.
    pub hz: f64,
    /// Ensemble label.
    pub label: String,
    /// SNR measure.
    pub snr_db: f32,
    /// Service labels joined with ", ".
    pub stations: String,
}

/// What the host does next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Keep listening.
    Wait,
    /// Tune here.
    Tune(f64),
    /// The scan is over: tune back here.
    Done(f64),
}

/// The scan.
#[derive(Clone, Debug)]
pub struct EnsembleScan {
    table: Vec<Channel>,
    index: usize,
    return_hz: f64,
    /// The ensembles found.
    pub found: Vec<Found>,
}

impl EnsembleScan {
    /// A scan of the channels of `table` in order, returning to `return_hz` at the end.
    pub fn new(table: Vec<Channel>, return_hz: f64) -> Self {
        EnsembleScan {
            table,
            index: 0,
            return_hz,
            found: Vec::new(),
        }
    }

    /// The first channel to tune.
    pub fn start(&mut self) -> Step {
        self.index = 0;
        match self.table.first() {
            Some(c) => Step::Tune(c.hz),
            None => Step::Done(self.return_hz),
        }
    }

    /// The channel being scanned.
    pub fn current(&self) -> Option<Channel> {
        self.table.get(self.index).copied()
    }

    /// Whether to leave the channel now (section 15's conditions).
    pub fn leave(o: &Observation) -> bool {
        match o.locked_s {
            Some(l) => {
                let tii_done = o.tii_frames.is_none_or(|n| n >= 40 || l > 5.0);
                (o.complete && l > 0.6 && tii_done) || l > 7.0
            }
            None => {
                o.since_tune_s > 2.2
                    || (o.since_tune_s > 0.5
                        && o.quiet_ms > 700.0
                        && o.quiet_ms <= o.since_tune_s * 1000.0 + 100.0)
            }
        }
    }

    /// Looks at the receiver; returns the next step.
    pub fn look(&mut self, o: &Observation) -> Step {
        if !Self::leave(o) {
            return Step::Wait;
        }
        if o.locked_s.is_some() && o.valid {
            let c = self.table[self.index];
            self.found.push(Found {
                channel: c.name,
                hz: c.hz,
                label: o.label.clone(),
                snr_db: o.snr_db,
                stations: o.services.join(", "),
            });
        }
        self.index += 1;
        match self.table.get(self.index) {
            Some(c) => Step::Tune(c.hz),
            None => Step::Done(self.return_hz),
        }
    }

    /// A channel picked from the results: the scan ends there.
    pub fn pick(&mut self, hz: f64) -> Step {
        self.index = self.table.len();
        Step::Done(hz)
    }

    /// The channels left to scan, the current one included.
    pub fn remaining(&self) -> usize {
        self.table.len().saturating_sub(self.index)
    }

    /// The results as an export: a header line and one line per ensemble (time, frequency_hz, stack, channel, ensemble, snr_db,
    /// stations), comma separated, text fields quoted. `time` is the host's time stamp for the scan.
    pub fn export(&self, time: &str) -> String {
        let q = |t: &str| format!("\"{}\"", t.replace('"', "\"\""));
        let mut s = String::from("time,frequency_hz,stack,channel,ensemble,snr_db,stations\n");
        for f in &self.found {
            s.push_str(&format!(
                "{},{:.0},dab,{},{},{:.1},{}\n",
                q(time),
                f.hz,
                f.channel,
                q(&f.label),
                f.snr_db,
                q(&f.stations)
            ));
        }
        s
    }

    /// Stops the scan: tune back.
    pub fn stop(&mut self) -> Step {
        self.index = self.table.len();
        Step::Done(self.return_hz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels;

    #[test]
    fn leaves_an_empty_channel_quickly_and_records_ensembles() {
        let mut s = EnsembleScan::new(channels::band_iii().collect(), 100e6);
        assert_eq!(s.start(), Step::Tune(174.928e6));
        let empty = Observation {
            since_tune_s: 0.7,
            quiet_ms: 720.0,
            ..Default::default()
        };
        assert!(matches!(s.look(&empty), Step::Tune(_)));
        let wait = Observation {
            since_tune_s: 0.3,
            locked_s: Some(0.1),
            ..Default::default()
        };
        assert_eq!(s.look(&wait), Step::Wait);
        let ok = Observation {
            since_tune_s: 1.0,
            locked_s: Some(0.8),
            complete: true,
            valid: true,
            label: "Test".into(),
            services: vec!["A".into(), "?".into()],
            ..Default::default()
        };
        assert!(matches!(s.look(&ok), Step::Tune(_)));
        assert_eq!(s.found[0].stations, "A, ?");
        assert_eq!(s.found[0].channel, "5B");
        let csv = s.export("2026-11-08T12:00:00Z");
        assert_eq!(
            csv,
            "time,frequency_hz,stack,channel,ensemble,snr_db,stations\n\"2026-11-08T12:00:00Z\",176640000,dab,5B,\"Test\",0.0,\"A, ?\"\n"
        );
        assert_eq!(s.remaining(), 36);
        assert_eq!(s.pick(225.648e6), Step::Done(225.648e6));
        assert_eq!(s.remaining(), 0);
    }

    #[test]
    fn every_leave_condition() {
        // Locked and complete, but only after 0.6 s of lock.
        let mut o = Observation {
            since_tune_s: 0.5,
            locked_s: Some(0.5),
            complete: true,
            valid: true,
            ..Default::default()
        };
        assert!(!EnsembleScan::leave(&o));
        o.locked_s = Some(0.61);
        assert!(EnsembleScan::leave(&o));
        // Transmitter identification collected: 40 nulls or 5 s.
        o.tii_frames = Some(10);
        assert!(!EnsembleScan::leave(&o));
        o.tii_frames = Some(40);
        assert!(EnsembleScan::leave(&o));
        o.tii_frames = Some(10);
        o.locked_s = Some(5.1);
        assert!(EnsembleScan::leave(&o));
        // Locked but incomplete: left after 7 s.
        o.complete = false;
        o.tii_frames = None;
        assert!(!EnsembleScan::leave(&o));
        o.locked_s = Some(7.1);
        assert!(EnsembleScan::leave(&o));
        // Not locked: 2.2 s.
        let mut n = Observation {
            since_tune_s: 2.1,
            ..Default::default()
        };
        assert!(!EnsembleScan::leave(&n));
        n.since_tune_s = 2.3;
        assert!(EnsembleScan::leave(&n));
        // Quiet: more than 0.5 s since the tune and more than 700 ms quiet, not older than the tune (+100 ms).
        n.since_tune_s = 0.45;
        n.quiet_ms = 800.0;
        assert!(!EnsembleScan::leave(&n));
        n.since_tune_s = 0.75;
        assert!(EnsembleScan::leave(&n));
        n.quiet_ms = 900.0;
        assert!(
            !EnsembleScan::leave(&n),
            "a quiet time older than the tune belongs to the channel before"
        );
        n.quiet_ms = 650.0;
        assert!(!EnsembleScan::leave(&n));
    }

    #[test]
    fn a_full_band_iii_pass_returns_to_the_start() {
        let mut s = EnsembleScan::new(channels::band_iii().collect(), 100e6);
        let mut tunes = vec![];
        let mut step = s.start();
        let quiet = Observation {
            since_tune_s: 0.8,
            quiet_ms: 750.0,
            ..Default::default()
        };
        loop {
            match step {
                Step::Tune(hz) => tunes.push(hz),
                Step::Done(hz) => {
                    assert_eq!(hz, 100e6);
                    break;
                }
                Step::Wait => unreachable!(),
            }
            step = s.look(&quiet);
        }
        assert_eq!(tunes.len(), 38);
        assert_eq!(tunes[37], channels::band_iii().nth(37).unwrap().hz);
        assert!(s.found.is_empty());
        assert_eq!(EnsembleScan::new(channels::blocks(), 0.0).remaining(), 61);
        assert_eq!(EnsembleScan::new(Vec::new(), 7.0).start(), Step::Done(7.0));
    }

    #[test]
    fn observation_from_the_results() {
        let service = |sid: i64, label: Option<&'static str>| ServiceRow {
            sid,
            label,
            short_label: None,
            kind: crate::results::ServiceKind::DabPlus,
            subchannel: None,
            bitrate_kbps: None,
            protection: None,
            component_type: None,
            pty: None,
            pty_name: None,
            pty_static: true,
            language: None,
            language_name: None,
            scrambled: false,
            announcement_support: 0,
            now_playing: None,
            playing: false,
        };
        let mut p = crate::results::store();
        p.results_mut()
            .set_status(&sdrkit_core::Status::new(LockState::Locked));
        p.put(&SignalFields {
            snr_db: Some(21.3),
            offset_hz: None,
            clock_ppm: None,
            level_dbfs: -20.0,
        });
        p.put(&DataFields {
            fic_records: 0,
            spi_objects: None,
            journaline_received: None,
            tii_analysed: Some(12),
        });
        p.put(&EnsembleFields {
            ensemble_label: Some("Test Ensemble   "),
            ensemble_short_label: None,
            eid: None,
            ecc: None,
            country: None,
            utc: None,
            local_time_offset: None,
            ensemble_valid: true,
            service_count: 2,
        });
        SERVICES.put(&mut p, "C101", &service(0xC101, Some("One")));
        SERVICES.put(&mut p, "C102", &service(0xC102, None));
        let o = Observation::from_results(p.results(), 1.0, Some(0.8), true);
        assert_eq!(o.label, "Test Ensemble");
        let mut names = o.services.clone();
        names.sort();
        assert_eq!(names, vec!["?".to_string(), "One".to_string()]);
        assert!(o.valid && !o.complete);
        assert_eq!(o.tii_frames, Some(12));
        SERVICES.put(&mut p, "C102", &service(0xC102, Some("Two")));
        let o = Observation::from_results(p.results(), 1.0, Some(0.8), false);
        assert!(o.complete);
        assert_eq!(o.tii_frames, None);
        // Not locked: the lock time is ignored.
        p.results_mut()
            .set_status(&sdrkit_core::Status::new(LockState::Searching));
        assert_eq!(
            Observation::from_results(p.results(), 1.0, Some(0.8), false).locked_s,
            None
        );
    }
}
