//! The channel scan for the stacks of this crate: what to ask of the radio, when a channel is occupied, when the receiver
//! counts as locked, and what the channel's result says. [`ChannelScan`] is the state machine that a host drives for one
//! channel.

use crate::dtmb::pn::Header;
use crate::isdbt::results::{CodeRate, LayerName, Modulation};
use crate::isdbt::Guard;
use crate::typed::{Atsc3Fields, AtscFields, DtmbFields, IsdbtFields};
use crate::{atsc, atsc3, dtmb, isdbt};
use sdrkit_algo_dsp::floor::DEFAULT_MIN_FLOOR;
use sdrkit_core::interface::Column;
use sdrkit_core::record::Kind;
use sdrkit_core::Results;

/// A standard the scan looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standard {
    /// ATSC 1.0.
    Atsc,
    /// ISDB-T.
    Isdbt,
    /// ATSC 3.0.
    Atsc3,
    /// DTMB.
    Dtmb,
}

impl Standard {
    /// The stack id.
    pub fn stack_id(self) -> &'static str {
        match self {
            Standard::Atsc => atsc::ID,
            Standard::Isdbt => isdbt::ID,
            Standard::Atsc3 => atsc3::ID,
            Standard::Dtmb => dtmb::ID,
        }
    }
}

/// The radio settings a scan for a standard requires.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// The standard.
    pub standard: Standard,
    /// Channel width, MHz.
    pub width_mhz: f64,
    /// The requested rate, samples/s.
    pub rate_hz: f64,
    /// The stack's minimum rate.
    pub min_rate_hz: f64,
}

/// The scan profile of a standard. `scan_width_mhz` is the scan's channel width (DTMB uses 6 MHz below 7).
pub fn profile(standard: Standard, scan_width_mhz: f64) -> Profile {
    let (w, rate, min) = match standard {
        Standard::Atsc => (6.0, 8e6, 6.5e6),
        Standard::Isdbt => (6.0, 8e6, 6e6),
        Standard::Atsc3 => (6.0, 8e6, 6.5e6),
        Standard::Dtmb => {
            if scan_width_mhz < 7.0 {
                (6.0, 10e6, 6e6)
            } else {
                (8.0, 10e6, 8e6)
            }
        }
    };
    Profile {
        standard,
        width_mhz: w,
        rate_hz: rate,
        min_rate_hz: min,
    }
}

/// Why a radio cannot scan.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum RateError {
    /// The radio's highest rate is below 1.15 times the channel width.
    #[error("the radio reaches {max_hz} samples a second, {need_hz} are needed")]
    RadioTooSlow {
        /// The radio's highest rate.
        max_hz: f64,
        /// The rate needed.
        need_hz: f64,
    },
    /// The radio runs below the stack's minimum rate.
    #[error("the radio runs at {rate_hz} samples a second, {need_hz} are needed")]
    RunningTooSlow {
        /// The rate the radio runs at.
        rate_hz: f64,
        /// The rate needed.
        need_hz: f64,
    },
}

/// Before the scan starts: the radio must reach 1.15 times the channel width in samples a second.
///
/// # Errors
///
/// [`RateError::RadioTooSlow`] if it does not.
pub fn check_radio(max_rate_hz: f64, width_mhz: f64) -> Result<(), RateError> {
    let need = 1.15 * width_mhz * 1e6;
    if max_rate_hz + 1.0 < need {
        Err(RateError::RadioTooSlow {
            max_hz: max_rate_hz,
            need_hz: need,
        })
    } else {
        Ok(())
    }
}

/// After the radio opens: a radio that runs below the stack's minimum rate stops the scan.
///
/// # Errors
///
/// [`RateError::RunningTooSlow`] if `rate_hz` is below the minimum of `p`.
pub fn check_running_rate(rate_hz: f64, p: &Profile) -> Result<(), RateError> {
    if rate_hz < p.min_rate_hz {
        Err(RateError::RunningTooSlow {
            rate_hz,
            need_hz: p.min_rate_hz,
        })
    } else {
        Ok(())
    }
}

/// The scan's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScanSettings {
    /// Occupied when the in-band level exceeds the floor by this, dB.
    pub occupancy_db: f64,
    /// How long to wait for a lock, s.
    pub lock_wait_s: f64,
    /// How long to wait for service names, s.
    pub names_wait_s: f64,
    /// Run the host's quick gain optimisation after tuning an occupied channel, before the lock wait.
    pub quick_gain: bool,
}

impl Default for ScanSettings {
    fn default() -> Self {
        ScanSettings {
            occupancy_db: 5.0,
            lock_wait_s: 5.0,
            names_wait_s: 9.0,
            quick_gain: false,
        }
    }
}

/// The occupancy of a channel from its spectrum.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Occupancy {
    /// Mean power inside 0.9 × half the width, dB.
    pub in_band_db: f64,
    /// Median power beyond 1.12 × half the width up to 0.94 of the Nyquist edge, dB.
    pub floor_db: f64,
    /// In-band over floor, dB.
    pub occupancy_db: f64,
    /// Above the threshold.
    pub occupied: bool,
}

/// Occupancy from a power spectrum in natural order (index 0 at −rate/2) of a channel tuned to its centre. Powers count as at
/// least [`DEFAULT_MIN_FLOOR`] (−120 dB of full scale), so digital silence beside the channel is a floor and not minus infinity.
pub fn occupancy(power: &[f64], rate_hz: f64, width_hz: f64, threshold_db: f64) -> Occupancy {
    let n = power.len();
    let hz = |i: usize| (i as f64 - n as f64 / 2.0) * rate_hz / n as f64;
    let half = width_hz / 2.0;
    let (mut ib, mut k) = (0.0, 0usize);
    let mut fl = Vec::new();
    for (i, &p) in power.iter().enumerate() {
        let f = hz(i).abs();
        if f <= 0.9 * half {
            ib += p;
            k += 1;
        } else if f >= 1.12 * half && f <= 0.94 * rate_hz / 2.0 {
            fl.push(p);
        }
    }
    let db = |p: f64| 10.0 * p.max(f64::from(DEFAULT_MIN_FLOOR)).log10();
    let in_band_db = db(ib / k.max(1) as f64);
    let floor_db = if fl.is_empty() {
        // The channel fills the sample band: no floor to compare with beside it.
        f64::NEG_INFINITY
    } else {
        fl.sort_by(f64::total_cmp);
        db(fl[fl.len() / 2])
    };
    let occ = in_band_db - floor_db;
    Occupancy {
        in_band_db,
        floor_db,
        occupancy_db: if occ.is_finite() { occ } else { 99.0 },
        occupied: occ > threshold_db,
    }
}

/// Where the receiver is, judged from its results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lock {
    /// Not locked yet.
    Waiting,
    /// Locked: worth waiting for the names.
    Locked,
    /// Locked and done (the transport stream flows, or a layer synchronised).
    Done,
}

/// The value of the result field of column `c`, `None` when absent.
fn at<'a, K: Kind>(r: &'a Results, c: &Column<K>) -> Option<K::Out<'a>> {
    r.fields().at(c)
}

/// Lock state of a receiver's results.
pub fn judge(standard: Standard, d: &Results) -> Lock {
    match standard {
        // Field sync and equaliser trained. Done when the transport stream flows.
        Standard::Atsc => {
            if at(d, &AtscFields::TS_FLOWING) == Some(true) {
                Lock::Done
            } else if at(d, &AtscFields::FIELD_SYNC) == Some(true)
                && at(d, &AtscFields::EQUALISER_TRAINED) == Some(true)
            {
                Lock::Locked
            } else {
                Lock::Waiting
            }
        }
        // TMCC decoded. Done when a layer synchronises.
        Standard::Isdbt => match at(d, &IsdbtFields::DETECTION).unwrap_or(0) {
            3 => Lock::Done,
            2 => Lock::Locked,
            _ => Lock::Waiting,
        },
        // A frame decoded.
        Standard::Atsc3 => {
            if at(d, &Atsc3Fields::FRAMES_OK).unwrap_or(0) > 0 {
                Lock::Done
            } else {
                Lock::Waiting
            }
        }
        // System information decoded. Done when the transport stream flows.
        Standard::Dtmb => {
            if at(d, &DtmbFields::TS_LOCK) == Some(true) {
                Lock::Done
            } else if at(d, &DtmbFields::MAPPING).is_some() {
                Lock::Locked
            } else {
                Lock::Waiting
            }
        }
    }
}

/// One layer of an ISDB-T channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerSummary {
    /// The layer.
    pub layer: LayerName,
    /// Its segments.
    pub segments: i64,
    /// Its modulation.
    pub modulation: Option<Modulation>,
    /// Its code rate.
    pub code_rate: Option<CodeRate>,
}

/// The parameters of a channel found.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ChannelDetail {
    /// Nothing found.
    #[default]
    None,
    /// ISDB-T.
    Isdbt {
        /// Mode 1, 2 or 3.
        mode: i64,
        /// The guard interval.
        guard: Option<Guard>,
        /// Segments of the layers in use.
        segments: i64,
        /// A one-segment layer is there.
        partial: bool,
        /// The layers in use, A to C.
        layers: Vec<LayerSummary>,
    },
    /// ATSC 1.0, 8-VSB in 6 MHz.
    Atsc {
        /// The field parity, 1 or 2.
        field_parity: i64,
    },
    /// ATSC 3.0.
    Atsc3,
    /// DTMB.
    Dtmb {
        /// The frame header.
        header: Option<Header>,
    },
}

/// What the scan says of a channel beside its parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanNote {
    /// The channel is empty.
    Empty,
    /// The radio refused the tune.
    CouldNotTune,
    /// A signal is there, the receiver did not lock.
    SignalNoLock,
    /// DTMB frame headers found, the system information cannot be decoded (a weak signal).
    DtmbFramesOnly,
    /// ATSC 3.0 bootstraps found, no frame decoded.
    Atsc3BootstrapsOnly,
    /// An ISDB-T signal found, the TMCC not decoded.
    IsdbtNoTmcc {
        /// Mode 1, 2 or 3.
        mode: i64,
        /// The guard interval.
        guard: Option<Guard>,
    },
    /// The ATSC pilot found, no field sync.
    AtscPilotOnly,
    /// The ATSC segment sync found, no field sync.
    AtscSegmentSyncOnly,
}

/// The result of one channel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelResult {
    /// Frequency, Hz.
    pub freq_hz: f64,
    /// Width, MHz.
    pub width_mhz: f64,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// Occupancy, dB.
    pub occupancy_db: f64,
    /// The standard found (`None` when nothing was found).
    pub standard: Option<Standard>,
    /// Its parameters.
    pub detail: ChannelDetail,
    /// Why nothing was found, or what was found of the standard without a lock.
    pub notes: Vec<ScanNote>,
    /// SNR, dB (none for ATSC 3.0).
    pub snr_db: Option<f64>,
    /// Share of good FEC blocks, 0 to 1.
    pub fec_good: Option<f64>,
    /// Service names as the transport stream lists them.
    pub services: Vec<String>,
    /// Multiplex rate, Mbit/s.
    pub rate_mbps: Option<f64>,
    /// Locked.
    pub locked: bool,
    /// Listed as seen (a signal of the standard without a lock).
    pub seen: bool,
}

/// Fills `r` from the results of a locked (or seen) receiver.
pub fn describe(standard: Standard, d: &Results, r: &mut ChannelResult) {
    let f = d.fields();
    r.standard = Some(standard);
    match standard {
        Standard::Isdbt => {
            use crate::typed::{IsdbtLayerRow, ISDBT_LAYERS};
            let mut segments = 0i64;
            let mut layers = Vec::new();
            let table = d
                .decl()
                .table_id(ISDBT_LAYERS.key)
                .and_then(|id| d.table(id));
            if let Some(t) = table {
                for name in [LayerName::A, LayerName::B, LayerName::C] {
                    let Some(row) = t.row(name.key()) else {
                        continue;
                    };
                    let n = row.at(&IsdbtLayerRow::SEGMENTS).unwrap_or(0);
                    segments += n;
                    layers.push(LayerSummary {
                        layer: name,
                        segments: n,
                        modulation: row.at(&IsdbtLayerRow::MODULATION),
                        code_rate: row.at(&IsdbtLayerRow::CODE_RATE),
                    });
                }
            }
            r.detail = ChannelDetail::Isdbt {
                mode: f.at(&IsdbtFields::MODE).unwrap_or(0),
                guard: f.at(&IsdbtFields::GUARD),
                segments,
                partial: f.at(&IsdbtFields::PARTIAL_RECEPTION).unwrap_or(false),
                layers,
            };
            r.snr_db = f.at(&IsdbtFields::SNR_DB);
            r.fec_good = f.at(&IsdbtFields::PACKETS_OK_PCT).map(|p| p / 100.0);
            r.rate_mbps = f.at(&IsdbtFields::NET_RATE_BIT_S).map(|b| b / 1e6);
        }
        Standard::Atsc => {
            r.detail = ChannelDetail::Atsc {
                field_parity: f.at(&AtscFields::FIELD_PARITY).unwrap_or(0),
            };
            r.snr_db = f.at(&AtscFields::SNR_DB);
            let (c, x, bad) = (
                at(d, &AtscFields::RS_CLEAN).unwrap_or(0),
                at(d, &AtscFields::RS_CORRECTED).unwrap_or(0),
                at(d, &AtscFields::RS_FAILED).unwrap_or(0),
            );
            if c + x + bad > 0 {
                r.fec_good = Some((c + x) as f64 / (c + x + bad) as f64);
            }
            r.rate_mbps = Some(atsc::gen::BIT_RATE / 1e6);
        }
        Standard::Atsc3 => {
            r.detail = ChannelDetail::Atsc3;
            r.snr_db = None;
        }
        Standard::Dtmb => {
            r.detail = ChannelDetail::Dtmb {
                header: f.at(&DtmbFields::HEADER),
            };
            r.snr_db = f.at(&DtmbFields::CN_DB);
            let (ok, bad) = (
                at(d, &DtmbFields::LDPC_OK).unwrap_or(0),
                at(d, &DtmbFields::LDPC_BAD).unwrap_or(0),
            );
            if ok + bad > 0 {
                r.fec_good = Some(ok as f64 / (ok + bad) as f64);
            }
            r.rate_mbps = f
                .at(&DtmbFields::PAYLOAD_BPS)
                .filter(|b| *b > 0.0)
                .map(|b| b / 1e6);
        }
    }
}

/// The result for a channel with a signal but no lock: whether it is listed as seen, and the notes.
///
/// Only DTMB is listed as seen (frame headers whose system information does not decode). An ISDB-T cyclic prefix or an ATSC 3.0 bootstrap
/// without a decoded frame is reported in the notes.
pub fn not_locked(standard: Standard, d: &Results) -> (bool, Vec<ScanNote>) {
    let f = d.fields();
    match standard {
        Standard::Dtmb
            if at(d, &DtmbFields::TRACKING) == Some(true)
                || at(d, &DtmbFields::FRAMES).unwrap_or(0) > 0 =>
        {
            (true, vec![ScanNote::DtmbFramesOnly])
        }
        _ => {
            let note = match standard {
                Standard::Atsc3 if at(d, &Atsc3Fields::BOOTSTRAPS).unwrap_or(0) > 0 => {
                    Some(ScanNote::Atsc3BootstrapsOnly)
                }
                Standard::Isdbt if at(d, &IsdbtFields::DETECTION).unwrap_or(0) >= 1 => {
                    Some(ScanNote::IsdbtNoTmcc {
                        mode: f.at(&IsdbtFields::MODE).unwrap_or(0),
                        guard: f.at(&IsdbtFields::GUARD),
                    })
                }
                Standard::Atsc if at(d, &AtscFields::DETECTION).unwrap_or(0) >= 2 => {
                    Some(ScanNote::AtscSegmentSyncOnly)
                }
                Standard::Atsc if at(d, &AtscFields::DETECTION).unwrap_or(0) >= 1 => {
                    Some(ScanNote::AtscPilotOnly)
                }
                _ => None,
            };
            let mut notes = vec![ScanNote::SignalNoLock];
            notes.extend(note);
            (false, notes)
        }
    }
}

/// The phase of one channel's scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Measuring the spectrum.
    Occupancy,
    /// Waiting for the receiver to lock.
    Lock,
    /// Waiting for the service names.
    Names,
    /// Finished.
    Done,
}

/// One channel's scan.
#[derive(Clone, Debug)]
pub struct ChannelScan {
    standard: Standard,
    settings: ScanSettings,
    profile: Profile,
    phase: Phase,
    t_lock: f64,
    t_names: f64,
    first_name: Option<f64>,
    stale_seq: Option<u64>,
    last: Results,
    result: ChannelResult,
}

impl ChannelScan {
    /// A scan of the channel at `freq_hz` for `standard`.
    pub fn new(
        standard: Standard,
        freq_hz: f64,
        scan_width_mhz: f64,
        settings: ScanSettings,
    ) -> ChannelScan {
        let profile = profile(standard, scan_width_mhz);
        ChannelScan {
            standard,
            settings,
            profile,
            phase: Phase::Occupancy,
            t_lock: 0.0,
            t_names: 0.0,
            first_name: None,
            stale_seq: None,
            last: Results::default(),
            result: ChannelResult {
                freq_hz,
                width_mhz: profile.width_mhz,
                ..Default::default()
            },
        }
    }

    /// The result of a channel whose tune the radio refused.
    pub fn refused(standard: Standard, freq_hz: f64, scan_width_mhz: f64) -> ChannelResult {
        ChannelResult {
            freq_hz,
            width_mhz: profile(standard, scan_width_mhz).width_mhz,
            notes: vec![ScanNote::CouldNotTune],
            ..Default::default()
        }
    }

    /// The profile in use.
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// The phase.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The spectrum after tuning (natural order), the input level, and the time. An empty channel finishes here.
    pub fn on_spectrum(&mut self, power: &[f64], rate_hz: f64, level_dbfs: f64, t: f64) {
        if self.phase != Phase::Occupancy {
            return;
        }
        let o = occupancy(
            power,
            rate_hz,
            self.profile.width_mhz * 1e6,
            self.settings.occupancy_db,
        );
        self.result.level_dbfs = level_dbfs;
        self.result.occupancy_db = o.occupancy_db;
        if o.occupied {
            self.phase = Phase::Lock;
            self.t_lock = t;
        } else {
            self.result.notes.push(ScanNote::Empty);
            self.phase = Phase::Done;
        }
    }

    /// The change counter of the receiver's results before the retune: results whose counter is not above it describe the
    /// previous channel and are ignored.
    pub fn set_retune_seq(&mut self, seq: u64) {
        self.stale_seq = Some(seq);
    }

    /// The receiver's results at time `t`, with their change counter (only results newer than the retune count, see
    /// [`ChannelScan::set_retune_seq`]).
    pub fn on_results(&mut self, results: &Results, seq: Option<u64>, t: f64) {
        if self.phase == Phase::Occupancy || self.phase == Phase::Done {
            return;
        }
        if let (Some(stale), Some(seq)) = (self.stale_seq, seq) {
            if seq <= stale {
                self.tick(t);
                return;
            }
        }
        self.last.clone_from(results);
        let lock = judge(self.standard, results);
        if self.phase == Phase::Lock {
            let near_end = t - self.t_lock >= self.settings.lock_wait_s - 0.8;
            match lock {
                Lock::Done => self.enter_names(t),
                Lock::Locked if near_end => self.enter_names(t),
                Lock::Locked | Lock::Waiting => {}
            }
        }
        self.tick(t);
    }

    /// Services named so far, and whether the network name is known. A service without a name yet starts with `[`.
    pub fn on_services(&mut self, names: &[String], network_known: bool, t: f64) {
        if self.phase != Phase::Names {
            return;
        }
        self.result.services = names.to_vec();
        if !names.is_empty() && self.first_name.is_none() {
            self.first_name = Some(t);
        }
        let all_named =
            !names.is_empty() && names.iter().all(|n| !n.starts_with('[') && !n.is_empty());
        if all_named && network_known {
            self.finish();
        }
        self.tick(t);
    }

    /// Time passes: the lock and names waits end.
    pub fn tick(&mut self, t: f64) {
        match self.phase {
            Phase::Lock if t - self.t_lock >= self.settings.lock_wait_s => {
                if judge(self.standard, &self.last) != Lock::Waiting {
                    self.enter_names(t);
                } else {
                    self.finish();
                }
            }
            Phase::Names => {
                let first_plus = self.first_name.is_some_and(|f| t - f >= 3.0);
                if t - self.t_names >= self.settings.names_wait_s || first_plus {
                    self.finish();
                }
            }
            _ => {}
        }
    }

    fn enter_names(&mut self, t: f64) {
        self.phase = Phase::Names;
        self.t_names = t;
    }

    fn finish(&mut self) {
        let d = &self.last;
        if judge(self.standard, d) != Lock::Waiting {
            self.result.locked = true;
            describe(self.standard, d, &mut self.result);
        } else {
            let (seen, notes) = not_locked(self.standard, d);
            self.result.seen = seen;
            self.result.notes.extend(notes);
            if seen {
                describe(self.standard, d, &mut self.result);
            }
        }
        self.phase = Phase::Done;
    }

    /// The result once done.
    pub fn result(&self) -> Option<&ChannelResult> {
        (self.phase == Phase::Done).then_some(&self.result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::{Quality, StepColour};
    use sdrkit_core::interface::Hertz;
    use sdrkit_core::publish::Published;
    use std::sync::Arc;

    fn results_of(decl: sdrkit_core::ResultsDecl) -> Published {
        Published::from_decl(Arc::new(decl))
    }

    /// The fields of an ISDB-T report at a detection level, in mode 3 with the guard 1/8.
    fn isdbt_fields(detection: i64) -> IsdbtFields {
        IsdbtFields {
            detection,
            width: Hertz(6e6),
            width_auto: false,
            ifft_rate: Hertz(isdbt::ifft_rate(6.0)),
            mode: Some(3),
            guard: Some(Guard::G8),
            fft_size: None,
            carriers: None,
            carrier_spacing: None,
            frame_length: None,
            offset_hz: None,
            clock_ppm: None,
            snr_db: None,
            frames_ok: 0,
            frames_bad: 0,
            carrier_shift: None,
            frame_symbol: None,
            since_tmcc: None,
            mirrored: false,
            system_id: None,
            switch_countdown: None,
            emergency_alarm: None,
            partial_reception: None,
            packets_ok_pct: 0.0,
            net_rate_bit_s: 0.0,
            quality_pct: 0.0,
            quality: crate::status::Quality::NoLock,
            step_mode: crate::status::StepColour::Off,
            step_tmcc: crate::status::StepColour::Off,
            step_layers: crate::status::StepColour::Off,
            step_ts: crate::status::StepColour::Off,
        }
    }

    #[test]
    fn profiles_checks_and_occupancy() {
        let p = profile(Standard::Dtmb, 6.0);
        assert_eq!((p.width_mhz, p.rate_hz), (6.0, 10e6));
        assert_eq!(profile(Standard::Dtmb, 8.0).width_mhz, 8.0);
        assert_eq!(profile(Standard::Isdbt, 8.0).rate_hz, 8e6);
        assert!(check_radio(6.0e6, 6.0).is_err());
        assert!(check_radio(6.9e6, 6.0).is_ok());
        let e = check_running_rate(5e6, &profile(Standard::Atsc, 6.0)).unwrap_err();
        assert_eq!(
            e,
            RateError::RunningTooSlow {
                rate_hz: 5e6,
                need_hz: 6.5e6
            }
        );
        // A flat channel 6 MHz wide 20 dB over the floor, at 8 Msps.
        let n = 1024;
        let p: Vec<f64> = (0..n)
            .map(|i| {
                let f = (i as f64 - 512.0) * 8e6 / 1024.0;
                if f.abs() < 2.9e6 {
                    100.0
                } else {
                    1.0
                }
            })
            .collect();
        let o = occupancy(&p, 8e6, 6e6, 5.0);
        assert!(o.occupied && (o.occupancy_db - 20.0).abs() < 0.5, "{o:?}");
        let flat = vec![1.0; n];
        assert!(!occupancy(&flat, 8e6, 6e6, 5.0).occupied);
        // Digital silence beside the channel: the floor is the least floor, not minus infinity.
        let silent: Vec<f64> = p
            .iter()
            .map(|&v| if v < 50.0 { 0.0 } else { 1e-6 })
            .collect();
        let o = occupancy(&silent, 8e6, 6e6, 5.0);
        assert!(o.occupied && (o.floor_db + 120.0).abs() < 0.01, "{o:?}");
        assert!((o.occupancy_db - 60.0).abs() < 0.5, "{o:?}");
        let o = occupancy(&vec![0.0; n], 8e6, 6e6, 5.0);
        assert!(!o.occupied && o.occupancy_db == 0.0, "{o:?}");
    }

    #[test]
    fn a_channel_scan_runs_its_phases() {
        use isdbt::results as i;
        let mut s = ChannelScan::new(Standard::Isdbt, 473e6, 6.0, ScanSettings::default());
        s.on_spectrum(&vec![1.0; 1024], 8e6, -40.0, 0.0);
        assert_eq!(s.result().unwrap().notes, vec![ScanNote::Empty]);

        let mut s = ChannelScan::new(Standard::Isdbt, 473e6, 6.0, ScanSettings::default());
        let p: Vec<f64> = (0..1024)
            .map(|i| if (i - 512i32).abs() < 360 { 50.0 } else { 1.0 })
            .collect();
        s.on_spectrum(&p, 8e6, -30.0, 0.0);
        assert_eq!(s.phase(), Phase::Lock);
        let mut d = results_of(i::report_declaration());
        d.put(&IsdbtFields {
            snr_db: Some(25.0),
            packets_ok_pct: 100.0,
            net_rate_bit_s: 17.3e6,
            partial_reception: Some(true),
            ..isdbt_fields(3)
        });
        for (name, n, m, c) in [
            (LayerName::A, 1, Modulation::Qpsk, CodeRate::R23),
            (LayerName::B, 12, Modulation::Qam64, CodeRate::R34),
        ] {
            crate::typed::ISDBT_LAYERS.put(
                &mut d,
                name.key(),
                &crate::typed::IsdbtLayerRow {
                    layer: name,
                    partial: false,
                    segments: n,
                    modulation: Some(m),
                    code_rate: Some(c),
                    interleave_length: 0,
                    rate_bit_s: 0.0,
                    packets: 0,
                    rs_clean: 0,
                    rs_corrected: 0,
                    rs_failed: 0,
                    rs_locked: false,
                    viterbi_margin: 0.0,
                },
            );
        }
        s.on_results(d.results(), None, 1.0);
        assert_eq!(s.phase(), Phase::Names);
        s.on_services(&["TEST [TV]".to_string()], true, 1.5);
        let r = s.result().unwrap();
        assert!(r.locked);
        assert_eq!(
            r.detail,
            ChannelDetail::Isdbt {
                mode: 3,
                guard: Some(Guard::G8),
                segments: 13,
                partial: true,
                layers: vec![
                    LayerSummary {
                        layer: LayerName::A,
                        segments: 1,
                        modulation: Some(Modulation::Qpsk),
                        code_rate: Some(CodeRate::R23),
                    },
                    LayerSummary {
                        layer: LayerName::B,
                        segments: 12,
                        modulation: Some(Modulation::Qam64),
                        code_rate: Some(CodeRate::R34),
                    },
                ],
            }
        );

        // DTMB with headers but no system information: listed as seen.
        let mut s = ChannelScan::new(Standard::Dtmb, 530e6, 8.0, ScanSettings::default());
        let p: Vec<f64> = (0..1024)
            .map(|i| if (i - 512i32).abs() < 390 { 50.0 } else { 1.0 })
            .collect();
        s.on_spectrum(&p, 10e6, -30.0, 0.0);
        let mut d = results_of(dtmb::results::declaration());
        let mut fields = DtmbFields {
            detection: 1,
            tracking: true,
            header: None,
            pn_phase_rotates: None,
            width: Hertz(8e6),
            frame_length: None,
            cn_db: Some(20.0),
            echo_span: None,
            frames: 100,
            frames_erased: 0,
            no_header_pct: 0.0,
            rate_ok: true,
            step_pn_sync: StepColour::Amber,
            si_index: None,
            si_corr: None,
            mapping: None,
            code_rate: None,
            interleaver: None,
            carriers: None,
            mer_db: None,
            ldpc_ok: 0,
            ldpc_bad: 0,
            ldpc_iterations: 0.0,
            bch_corrected: 0,
            codewords_dropped: 0,
            codewords_skipped: 0,
            packets: 0,
            ts_lock: false,
            payload_bps: None,
            note: "",
            offset_hz: None,
            clock_ppm: None,
            level_dbfs: -30.0,
            snr_db: None,
        };
        d.put(&fields);
        s.on_results(d.results(), None, 1.0);
        s.tick(5.1);
        let r = s.result().unwrap();
        assert!(r.seen && !r.locked);
        assert_eq!(r.notes, vec![ScanNote::DtmbFramesOnly]);
        assert_eq!(r.snr_db, Some(20.0));

        // DTMB decoded to the transport stream: locked, with the share of good code words and the payload rate.
        let mut s = ChannelScan::new(Standard::Dtmb, 530e6, 8.0, ScanSettings::default());
        s.on_spectrum(&p, 10e6, -30.0, 0.0);
        fields.header = Some(Header::Pn420);
        fields.mapping = Some(dtmb::profile::Mapping::Qam16);
        d.put(&fields);
        s.on_results(d.results(), None, 1.0);
        assert_eq!(judge(Standard::Dtmb, d.results()), Lock::Locked);
        assert_eq!(s.phase(), Phase::Lock);
        fields.ldpc_ok = 990;
        fields.ldpc_bad = 10;
        fields.payload_bps = Some(16.2432e6);
        fields.ts_lock = true;
        d.put(&fields);
        s.on_results(d.results(), None, 1.2);
        assert_eq!(s.phase(), Phase::Names);
        s.on_services(&["TEST [TV]".to_string()], true, 1.5);
        let r = s.result().unwrap();
        assert!(r.locked && r.notes.is_empty(), "{r:?}");
        assert_eq!(
            r.detail,
            ChannelDetail::Dtmb {
                header: Some(Header::Pn420)
            }
        );
        assert_eq!(r.fec_good, Some(0.99));
        assert_eq!(r.rate_mbps, Some(16.2432));
        assert_eq!(
            ChannelScan::refused(Standard::Atsc, 1e6, 6.0).notes,
            vec![ScanNote::CouldNotTune]
        );
    }

    #[test]
    fn not_locked_notes_and_stale_reports() {
        let mut d = results_of(isdbt::results::report_declaration());
        d.put(&isdbt_fields(1));
        let (seen, notes) = not_locked(Standard::Isdbt, d.results());
        assert!(!seen);
        assert_eq!(
            notes,
            vec![
                ScanNote::SignalNoLock,
                ScanNote::IsdbtNoTmcc {
                    mode: 3,
                    guard: Some(Guard::G8)
                }
            ]
        );
        let mut b = results_of(atsc3::results::declaration());
        b.put(&crate::typed::Atsc3Fields {
            bootstrap_seen: true,
            bootstraps: 4,
            frames_taken: 0,
            frames_ok: 0,
            offset_hz: None,
            minor_version: None,
            system_bandwidth: None,
            preamble_structure: None,
            min_time_next: None,
            post_rate: None,
            ea_wake_up: None,
            frame_length: None,
            lls_tables: 0,
            signed_signalling: false,
            leap_seconds: None,
            service: None,
            service_received: None,
            route_objects: 0,
            media_samples: 0,
            step_bootstrap: StepColour::Green,
            step_l1: StepColour::Amber,
            step_service: StepColour::Off,
        });
        let (seen, notes) = not_locked(Standard::Atsc3, b.results());
        assert!(!seen);
        assert_eq!(notes[1], ScanNote::Atsc3BootstrapsOnly);
        let empty = Results::default();
        assert_eq!(
            not_locked(Standard::Atsc, &empty).1,
            vec![ScanNote::SignalNoLock]
        );
        assert_eq!(
            not_locked(Standard::Dtmb, &empty).1,
            vec![ScanNote::SignalNoLock]
        );
        assert_eq!(
            ChannelScan::refused(Standard::Dtmb, 1e6, 6.0).width_mhz,
            6.0
        );

        // A locked report of the previous channel (old change counter) must not lock the new one.
        let mut s = ChannelScan::new(Standard::Atsc, 539e6, 6.0, ScanSettings::default());
        let p: Vec<f64> = (0..1024)
            .map(|i| if (i - 512i32).abs() < 360 { 50.0 } else { 1.0 })
            .collect();
        s.on_spectrum(&p, 8e6, -30.0, 0.0);
        s.set_retune_seq(10);
        let mut old = results_of(atsc::results::declaration());
        old.put(&AtscFields {
            detection: 4,
            pilot_locked: true,
            segment_sync: true,
            field_sync: true,
            equaliser_trained: true,
            ts_flowing: true,
            offset_hz: None,
            clock_ppm: None,
            snr_db: None,
            frames_ok: 0,
            frames_bad: 0,
            segment_correlation: 1.0,
            field_parity: None,
            data_snr_db: None,
            pilot_db: None,
            fields: 0,
            rs_clean: 0,
            rs_corrected: 0,
            rs_failed: 0,
            packets: 0,
            field_sync_misses: 0,
            lock_losses: 0,
            quality_pct: 0.0,
            quality: Quality::Excellent,
            step_pilot: StepColour::Green,
            step_segment: StepColour::Green,
            step_field: StepColour::Green,
            step_equaliser: StepColour::Green,
            step_trellis: StepColour::Green,
            step_rs: StepColour::Green,
            step_ts: StepColour::Green,
        });
        s.on_results(old.results(), Some(10), 0.5);
        assert_eq!(s.phase(), Phase::Lock);
        s.tick(5.0);
        let r = s.result().unwrap();
        assert!(!r.locked);
        assert_eq!(r.notes, vec![ScanNote::SignalNoLock]);
    }
}
