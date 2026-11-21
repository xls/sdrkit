//! Transmit over TCI: audio for the modulator of the server, never IQ. The transmitter takes the real part of each complex
//! sample as one mono audio frame at 48 000 sps.

use crate::radio::{Conn, Shared, TxShared, TX_QUEUE};
use crate::{lock, TciError};
use sdrkit_core::net::NetError;
use sdrkit_core::{Complex32, Hz, Results, SettingSet, Values};
use sdrkit_driver_core::{
    tx_schema, Doing, DriverError, RateLimits, Transmitter, TxCapabilities, TxLimits, TxSetting,
    TxState, TxStats, TX_RATE as TX_RATE_SETTING,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The transmit audio rate.
pub const TX_RATE: Hz = 48_000.0;

/// The limits of the transmitter of the server `info`: the VFO range of the server, the one audio rate, no filter
/// width and no power setting.
pub(crate) fn limits(info: &crate::ServerInfo) -> TxLimits {
    let (min, max) = info
        .vfo_limits
        .unwrap_or((crate::DEFAULT_MIN_FREQ, crate::DEFAULT_MAX_FREQ));
    TxLimits {
        min_freq_hz: min,
        max_freq_hz: max,
        rates: RateLimits {
            min_hz: TX_RATE,
            max_hz: TX_RATE,
            ranges: vec![(TX_RATE, TX_RATE)],
            steady_hz: None,
        },
        bandwidth_hz: None,
        power_db: (0.0, 0.0, 0.0),
        start_power_db: None,
        common_rates_hz: Vec::new(),
        full_output_dbm: Vec::new(),
    }
}

pub(crate) type Current = Arc<Mutex<Option<Arc<Conn>>>>;

/// Why transmit cannot start now, found before anything is sent.
pub(crate) fn refusal(entry: &Shared, rx: Option<usize>) -> Result<(), DriverError> {
    let e = lock(entry);
    if e.info.is_receive_only() {
        return Err(DriverError::driver(TciError::ReceiveOnly));
    }
    if let Some(rx) = rx {
        if e.info.tx_enable[rx] == Some(false) {
            return Err(DriverError::driver(TciError::ReceiverMayNotTransmit {
                receiver: rx,
            }));
        }
    }
    Ok(())
}

fn dropped() -> DriverError {
    DriverError::Net(NetError::Closed)
}

/// A TCI source's transmitter: keys the server's transmitter (`trx`) and answers its chrono packets with audio.
pub(crate) struct TciTransmitter {
    tx: TxShared,
    current: Current,
    entry: Shared,
    state: TxState,
    gen: Option<u64>,
    caps: TxCapabilities,
    lines: Vec<String>,
}

impl TciTransmitter {
    pub(crate) fn new(
        tx: TxShared,
        current: Current,
        entry: Shared,
        label: &str,
    ) -> TciTransmitter {
        let limits = limits(&lock(&entry).info);
        TciTransmitter {
            tx,
            current,
            entry,
            state: TxState::Disarmed,
            gen: None,
            caps: TxCapabilities {
                name: format!("TCI transmit audio at {label}"),
                settings: tx_schema(&[], &limits),
                limits,
                outputs: Vec::new(),
                cyclic: false,
                full_duplex: false,
                buffer_samples: 0,
            },
            lines: Vec::new(),
        }
    }

    /// Queues mono audio frames (full scale 1.0, clamped), waiting up to `wait` for room. Returns how many were taken.
    fn push_audio(&mut self, audio: &[f32], wait: Duration) -> Result<usize, DriverError> {
        self.refresh();
        if self.state != TxState::Armed {
            return Err(DriverError::NotNow {
                state: Doing::Disarmed,
            });
        }
        let end = Instant::now() + wait;
        let mut taken = 0;
        loop {
            {
                let mut t = lock(&self.tx);
                if t.gen != self.gen {
                    return Err(dropped());
                }
                let room = TX_QUEUE
                    .saturating_sub(t.queue.len())
                    .min(audio.len() - taken);
                t.queue.extend(audio[taken..taken + room].iter().map(|v| {
                    if v.is_finite() {
                        v.clamp(-1.0, 1.0)
                    } else {
                        0.0
                    }
                }));
                taken += room;
            }
            if taken == audio.len() || Instant::now() >= end {
                return Ok(taken);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// A transmission that ended because its connection was lost is closed. A reconnect never resumes it.
    fn refresh(&mut self) {
        if self.state.transmitting() && lock(&self.tx).gen != self.gen {
            self.state = TxState::Closed;
            self.gen = None;
            self.lines
                .push("TCI: transmit ended: the connection to the server dropped".into());
        }
    }
}

impl Transmitter for TciTransmitter {
    fn capabilities(&self) -> &TxCapabilities {
        &self.caps
    }

    fn configure(&mut self, values: &Values) -> Result<(), DriverError> {
        let rate = values.get(&TX_RATE_SETTING);
        if rate != 0.0 && (rate - TX_RATE).abs() > 0.5 {
            return Err(DriverError::Refused {
                id: TxSetting::Rate.id(),
            });
        }
        Ok(())
    }

    fn arm(&mut self) -> Result<(), DriverError> {
        self.refresh();
        match self.state {
            TxState::Armed => return Ok(()),
            TxState::Closed => {
                return Err(DriverError::NotNow {
                    state: Doing::Closed,
                })
            }
            _ => {}
        }
        let conn = lock(&self.current).clone().ok_or_else(dropped)?;
        refusal(&self.entry, Some(conn.rx))?;
        let rx = conn.rx;
        conn.link.text("audio_samplerate:48000;")?;
        conn.link.text("audio_stream_sample_type:float32;")?;
        conn.link.text("audio_stream_channels:2;")?;
        conn.link.text(&format!("trx:{rx},true,tci;"))?;
        {
            let mut t = lock(&self.tx);
            t.gen = Some(conn.gen);
            t.queue.clear();
            t.lost = false;
            t.since = Some(Instant::now());
        }
        self.gen = Some(conn.gen);
        self.state = TxState::Armed;
        Ok(())
    }

    fn disarm(&mut self) {
        self.refresh();
        if !self.state.transmitting() {
            return;
        }
        let mine = {
            let mut t = lock(&self.tx);
            let mine = t.gen.is_some() && t.gen == self.gen;
            if mine {
                t.gen = None;
                t.queue.clear();
                t.since = None;
            }
            mine
        };
        if mine {
            if let Some(c) = lock(&self.current).clone() {
                if Some(c.gen) == self.gen {
                    let _ = c.link.text(&format!("trx:{},false;", c.rx));
                }
            }
        }
        self.gen = None;
        self.state = TxState::Disarmed;
    }

    fn push(&mut self, iq: &[Complex32], wait: Duration) -> Result<usize, DriverError> {
        // Audio as the real part; a small fixed scratch keeps this free of allocation.
        let mut done = 0;
        let mut scratch = [0f32; 1024];
        for chunk in iq.chunks(scratch.len()) {
            for (d, s) in scratch.iter_mut().zip(chunk) {
                *d = s.re;
            }
            let n = self.push_audio(&scratch[..chunk.len()], wait)?;
            done += n;
            if n < chunk.len() {
                break;
            }
        }
        Ok(done)
    }

    fn flush(&mut self) -> Result<(), DriverError> {
        Ok(())
    }

    fn play_cyclic(&mut self, _iq: &[Complex32]) -> Result<(), DriverError> {
        Err(DriverError::driver(TciError::NoCyclic))
    }

    fn state(&self) -> TxState {
        if self.state.transmitting() && lock(&self.tx).gen != self.gen {
            return TxState::Closed;
        }
        self.state
    }

    fn stats(&self) -> TxStats {
        let t = lock(&self.tx);
        TxStats {
            sent: t.frames,
            buffers: t.packets,
            queued: t.queue.len(),
            padded: 0,
            underruns: t.underruns,
            underrun_samples: t.underrun_frames,
            transmitting_since: if self.state.transmitting() && t.gen == self.gen {
                t.since
            } else {
                None
            },
        }
    }

    fn poll(&mut self, results: &mut Results) {
        self.refresh();
        for line in self.lines.drain(..) {
            results.log(line);
        }
    }

    fn close(&mut self) {
        self.disarm();
        self.state = TxState::Closed;
    }
}

impl Drop for TciTransmitter {
    fn drop(&mut self) {
        self.close();
    }
}
