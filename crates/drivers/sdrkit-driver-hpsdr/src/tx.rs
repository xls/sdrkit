//! The transmitter of an HPSDR radio. It rides on the receive stream: protocol 1 carries the transmit samples in the EP2
//! packets, protocol 2 sends DUC packets from the stream thread. The radio is keyed only while the transmitter is armed.

use crate::hub::{Stream, TxQueue};
use crate::p1;
use crate::p2;
use crate::plan;
use crate::profile::Profile;
use sdrkit_core::{Complex32, Results, SettingSet, Values};
use sdrkit_driver_core::{
    tx_schema, Doing, DriverError, RateLimits, Transmitter, TxCapabilities, TxLimits, TxSetting,
    TxState, TxStats, TX_FREQUENCY, TX_POWER, TX_RATE,
};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

/// The range of the drive level relative to full drive: (min, max, step) in dB.
pub const POWER_DB: (f64, f64, f64) = (-90.0, 0.0, 0.5);

/// The limits of the transmitter of a radio with the profile `p`: the tuning range of the radio, the one rate of its
/// protocol and the range of the drive level. The transmit filter cannot be set.
pub fn limits(p: &Profile) -> TxLimits {
    let rate = f64::from(if p.protocol == 1 {
        p1::TX_RATE
    } else {
        p2::TX_RATE
    });
    TxLimits {
        min_freq_hz: p.min_freq_hz,
        max_freq_hz: p.max_freq_hz,
        rates: RateLimits {
            min_hz: rate,
            max_hz: rate,
            ranges: vec![(rate, rate)],
            steady_hz: None,
        },
        bandwidth_hz: None,
        power_db: POWER_DB,
        start_power_db: None,
        common_rates_hz: Vec::new(),
        full_output_dbm: Vec::new(),
    }
}

/// The drive level for a power relative to full drive: 10^(power / 20) of full drive, as 0 to 255.
pub fn drive(power_db: f64) -> u8 {
    plan::drive_byte(100.0 * 10f64.powf(power_db / 20.0))
}

/// An HPSDR transmitter.
pub struct HpsdrTransmitter {
    p: Profile,
    stream: Weak<dyn Stream>,
    q: Arc<TxQueue>,
    caps: TxCapabilities,
    freq: u32,
    drive: u8,
}

impl HpsdrTransmitter {
    /// Opens the transmitter of the radio of `stream`, disarmed, with `values`.
    ///
    /// # Errors
    ///
    /// [`DriverError::NotNow`] if the stream is lost, [`DriverError::Refused`] if a value is refused,
    /// [`HpsdrError::TransmitterTaken`](crate::HpsdrError::TransmitterTaken) if another transmitter is open.
    pub fn open(
        p: Profile,
        label: &str,
        s: &Arc<dyn Stream>,
        values: &Values,
    ) -> Result<HpsdrTransmitter, DriverError> {
        let name = p.name;
        if s.lost() {
            return Err(DriverError::NotNow {
                state: Doing::Stopped,
            });
        }
        let q = Arc::new(TxQueue::default());
        s.claim(&q)?;
        let per = if p.protocol == 1 {
            p1::TX_PER_PACKET
        } else {
            p2::DUC_SAMPLES
        };
        let limits = limits(&p);
        let caps = TxCapabilities {
            name: format!("{name} TX {label}"),
            settings: tx_schema(&[], &limits),
            limits,
            outputs: vec!["tx1".into()],
            cyclic: true,
            full_duplex: true,
            buffer_samples: per,
        };
        let mut t = HpsdrTransmitter {
            p,
            stream: Arc::downgrade(s),
            q,
            caps,
            freq: 0,
            drive: 0,
        };
        if let Err(e) = t.configure(values) {
            t.close();
            return Err(e);
        }
        Ok(t)
    }

    /// The queue the stream thread takes the samples from.
    pub fn queue(&self) -> &Arc<TxQueue> {
        &self.q
    }

    fn stream(&self) -> Result<Arc<dyn Stream>, DriverError> {
        self.stream
            .upgrade()
            .filter(|s| !s.lost())
            .ok_or(DriverError::NotNow {
                state: Doing::Stopped,
            })
    }

    fn open_or_closed(&self) -> Result<(), DriverError> {
        if self.q.state() == TxState::Closed {
            return Err(DriverError::NotNow {
                state: Doing::Closed,
            });
        }
        Ok(())
    }
}

impl Transmitter for HpsdrTransmitter {
    fn capabilities(&self) -> &TxCapabilities {
        &self.caps
    }

    fn configure(&mut self, values: &Values) -> Result<(), DriverError> {
        self.open_or_closed()?;
        let s = self.stream()?;
        let rate = self.caps.limits.rates.max_hz;
        let asked = values.get(&TX_RATE);
        if asked > 0.0 && (asked - rate).abs() > 1.0 {
            return Err(DriverError::Refused {
                id: TxSetting::Rate.id(),
            });
        }
        let refused = DriverError::Refused {
            id: TxSetting::Frequency.id(),
        };
        let f = plan::freq_sent(values.get(&TX_FREQUENCY), s.ppm()).ok_or(refused.clone())?;
        if f64::from(f) < self.p.min_freq_hz || f64::from(f) > self.p.max_freq_hz + 1.0 {
            return Err(refused);
        }
        self.freq = f;
        self.drive = drive(values.get(&TX_POWER));
        if self.q.state().transmitting() {
            s.tx_set(self.freq, self.drive);
        }
        Ok(())
    }

    fn arm(&mut self) -> Result<(), DriverError> {
        self.open_or_closed()?;
        self.stream()?.tx_arm(&self.q, self.freq, self.drive)
    }

    fn disarm(&mut self) {
        if let Some(s) = self.stream.upgrade() {
            if self.q.state().transmitting() {
                s.tx_end("");
            }
        }
        self.q.set_state(TxState::Disarmed);
    }

    fn push(&mut self, iq: &[Complex32], wait: Duration) -> Result<usize, DriverError> {
        let state = match self.q.state() {
            TxState::Armed => None,
            TxState::Closed => Some(Doing::Closed),
            TxState::Cyclic => Some(Doing::Cyclic),
            TxState::Disarmed => Some(Doing::Disarmed),
        };
        if let Some(state) = state {
            return Err(DriverError::NotNow { state });
        }
        let end = Instant::now() + wait;
        let mut done = 0;
        loop {
            done += self.q.push(&iq[done..]);
            if done == iq.len() || Instant::now() >= end || self.q.state() != TxState::Armed {
                return Ok(done);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn flush(&mut self) -> Result<(), DriverError> {
        self.q.flush();
        Ok(())
    }

    fn play_cyclic(&mut self, iq: &[Complex32]) -> Result<(), DriverError> {
        if !self.q.state().transmitting() {
            return Err(DriverError::NotNow {
                state: Doing::Disarmed,
            });
        }
        self.q.cyclic(iq);
        Ok(())
    }

    fn state(&self) -> TxState {
        if self.q.state() != TxState::Closed && self.stream.upgrade().is_none_or_lost() {
            return TxState::Closed;
        }
        self.q.state()
    }

    fn stats(&self) -> TxStats {
        self.q.stats()
    }

    fn poll(&mut self, results: &mut Results) {
        for line in self.q.take_notes() {
            results.log(line);
        }
    }

    fn close(&mut self) {
        self.disarm();
        self.q.set_state(TxState::Closed);
    }
}

impl Drop for HpsdrTransmitter {
    fn drop(&mut self) {
        self.close();
    }
}

trait LostCheck {
    fn is_none_or_lost(&self) -> bool;
}

impl LostCheck for Option<Arc<dyn Stream>> {
    fn is_none_or_lost(&self) -> bool {
        self.as_ref().map(|s| s.lost()).unwrap_or(true)
    }
}
