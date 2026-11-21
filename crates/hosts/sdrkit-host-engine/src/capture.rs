//! The capture of the identify service as a session: a graph of one sink that keeps the samples the armed capture requires.
//! The engine passes them to the service and runs the analysis on a thread of its own.

use crate::feed::lock;
use crate::services::ServiceKind;
use crate::session::By;
use crate::{EngineError, Input, RadioId, SessionId, Shared};
use sdrkit_core::{Complex32, ConfigError, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, GraphBuilder, Io, Port};
use std::sync::{Arc, Mutex};

/// The samples of a capture, with the rate and centre they were received at.
struct Held {
    iq: Vec<Complex32>,
    want: usize,
    rate_hz: f64,
    centre_hz: f64,
}

/// A capture that runs: its session and what it holds.
pub(crate) struct CaptureRun {
    session: SessionId,
    held: Arc<Mutex<Held>>,
}

/// What the engine keeps of the identify capture between ticks.
#[derive(Default)]
pub(crate) struct CaptureState {
    run: Option<CaptureRun>,
    /// The capture was refused, and the service was told. It is not told again until a capture starts.
    refused: bool,
}

/// The sink of the capture session. It keeps samples until the capture is full. A new rate or centre starts
/// the capture again.
struct CaptureSink {
    held: Arc<Mutex<Held>>,
}

impl Component for CaptureSink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("engine.capture", vec![Port::iq("in")])
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let mut h = lock(&self.held);
        h.rate_hz = inputs[0].rate;
        h.centre_hz = inputs[0].centre_hz;
        h.iq.clear();
        Ok(Vec::new())
    }

    fn process(&mut self, io: &mut Io) {
        let iq = io.iq(0);
        // The engine may be reading a full capture. It takes no more samples then.
        if let Ok(mut h) = self.held.try_lock() {
            let take = h.want.saturating_sub(h.iq.len()).min(iq.len());
            h.iq.extend_from_slice(&iq[..take]);
        }
    }
}

impl Shared {
    /// Runs the identify service's capture at a tick: starts a capture session on the radio when the service arms a
    /// capture, passes the samples to the service once the capture is full, ends the session, and starts the analysis that the
    /// service queued on a thread of its own. A problem shows on the identify service.
    pub(crate) fn step_identify(&self) {
        let mut st = std::mem::take(&mut *lock(&self.capture));
        if let Err(e) = self.run_capture(&mut st) {
            if !std::mem::replace(&mut st.refused, true) {
                self.services
                    .with(ServiceKind::Identify, |o| o.problem(&e.to_string()));
            }
        }
        *lock(&self.capture) = st;
        let job = self.services.identify.lock().take_job();
        if let Some(j) = job {
            let ran = std::thread::Builder::new()
                .name("sdrkit-identify".into())
                .spawn(move || {
                    // The service takes the outcome through the job's control at its next tick.
                    let _ = (j.job)(&j.control);
                });
            if let Err(e) = ran {
                let text = EngineError::thread(&e).to_string();
                self.services
                    .with(ServiceKind::Identify, |o| o.problem(&text));
            }
        }
    }

    fn run_capture(&self, st: &mut CaptureState) -> Result<(), EngineError> {
        if let Some(c) = &st.run {
            let gone = self.session(c.session).is_none();
            let armed = {
                let h = lock(&c.held);
                self.services.identify.lock().wanted(h.rate_hz).is_some()
            };
            if gone || !armed {
                self.end_capture(st);
                return Ok(());
            }
            let full = match c.held.try_lock() {
                Ok(h) if h.want > 0 && h.iq.len() >= h.want => Some(
                    self.services
                        .identify
                        .lock()
                        .deliver(&h.iq, h.rate_hz, h.centre_hz)
                        .map_err(EngineError::from),
                ),
                _ => None,
            };
            if let Some(delivered) = full {
                self.end_capture(st);
                delivered?;
            }
            return Ok(());
        }
        let radio = self.capture_radio();
        let rate = radio
            .as_ref()
            .ok()
            .and_then(|r| self.radio(*r))
            .map_or(0.0, |r| r.feed.tuning().0);
        let Some(want) = self.services.identify.lock().wanted(rate) else {
            st.refused = false;
            return Ok(());
        };
        let radio = radio?;
        if rate <= 0.0 {
            return Ok(());
        }
        let held = Arc::new(Mutex::new(Held {
            iq: Vec::with_capacity(want),
            want,
            rate_hz: rate,
            centre_hz: 0.0,
        }));
        let mut graph = GraphBuilder::new()
            .catalog(self.components.catalog())
            .add(
                "capture",
                CaptureSink {
                    held: Arc::clone(&held),
                },
            )
            .input("capture")
            .build()?;
        graph.set_context(self.context());
        let session = self.start(
            None,
            By::Engine,
            Box::new(graph),
            Input::Radio(radio),
            None,
            None,
        )?;
        st.run = Some(CaptureRun { session, held });
        st.refused = false;
        Ok(())
    }

    fn end_capture(&self, st: &mut CaptureState) {
        if let Some(c) = st.run.take() {
            // A session that the host closed already is gone.
            let _ = self.end_session(c.session);
        }
    }

    /// The radio that a capture reads: the radio of the session that the command `arm` named, else the one open radio.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchSession`] if the named session is gone. [`EngineError::CaptureRadio`] if the named session reads
    /// no radio, or if no session is named and no radio or more than one radio is open.
    fn capture_radio(&self) -> Result<RadioId, EngineError> {
        let named = self
            .services
            .identify
            .lock()
            .armed_session()
            .map(str::to_owned);
        if let Some(key) = named {
            let id = self.session_by_key(&key)?;
            return match self.session(id).map(|s| s.input) {
                Some(Input::Radio(r)) => Ok(r),
                _ => Err(EngineError::CaptureRadio { radios: 0 }),
            };
        }
        let radios = lock(&self.radios);
        let mut open = radios.iter().map(|(h, _)| RadioId::new(h));
        match (open.next(), open.next()) {
            (Some(r), None) => Ok(r),
            _ => Err(EngineError::CaptureRadio {
                radios: radios.iter().count(),
            }),
        }
    }
}
