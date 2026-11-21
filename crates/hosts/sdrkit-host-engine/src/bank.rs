//! The sessions of the channel bank. One filterbank session reads the bank's radio once and cuts every channel from it. Each
//! channel's session reads the samples of its channel through a feed of its own. The filterbank session mixes the audio of
//! the channel sessions into its own audio, which the host drains.

use crate::feed::{lock, Feed};
use crate::mix::MixIn;
use crate::services::ServiceKind;
use crate::session::By;
use crate::{EngineError, Input, RadioId, SessionId, Shared};
use sdrkit_core::{ConfigError, Owner, Values};
use sdrkit_graph::components::NullSink;
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Graph, GraphBuilder, Io, Port};
use sdrkit_svc_bank::{CarrierScan, ChannelChange};
use std::sync::Arc;

/// The id of the carrier scan in the graph of the filterbank session.
const SCAN: &str = "scan";

/// What the engine runs for the bank: the radio it reads, the filterbank session, and the feed of each channel.
#[derive(Default)]
pub(crate) struct BankRun {
    radio: Option<RadioId>,
    /// The rate and centre that the bank was last given.
    input: (f64, f64),
    session: Option<SessionId>,
    /// The session and the feed of each channel, by the channel's row key.
    feeds: Vec<(String, SessionId, Arc<Feed>)>,
    /// The channels or the input changed since the filterbank graph was built.
    layout: bool,
}

/// The end of one output of the channelizer: it passes each piece of the channel to the feed of the channel's session.
struct ChannelSink {
    feed: Arc<Feed>,
    rate_hz: f64,
    centre_hz: f64,
}

impl Component for ChannelSink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("engine.channel", vec![Port::iq("in")])
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate_hz = inputs[0].rate;
        self.centre_hz = inputs[0].centre_hz;
        Ok(Vec::new())
    }

    fn process(&mut self, io: &mut Io) {
        let iq = io.iq(0);
        if iq.is_empty() {
            return;
        }
        // On a live radio a channel session that is behind loses the piece, and counts it as dropped. On another radio
        // the bank waits for the channel session.
        let _ = self
            .feed
            .deliver(self.rate_hz, self.centre_hz, false, |buf| {
                buf.clear();
                buf.extend_from_slice(iq);
                iq.len()
            });
    }
}

impl Shared {
    /// Runs the channel bank at wall time `now`: starts and ends the sessions of its channels, builds the filterbank session
    /// again when the channels change, passes the bank the rate and centre of its radio, and passes it the levels of its
    /// carrier scan. A problem shows on the bank service.
    pub(crate) fn step_bank(&self, now: f64) {
        let mut run = std::mem::take(&mut *lock(&self.bank));
        let changes = self.services.bank.lock().take_changes();
        for c in changes {
            match c {
                ChannelChange::Start {
                    channel,
                    radio,
                    stack,
                    submode,
                    ..
                } => {
                    let started = self.start_channel(&mut run, &channel, &radio, &stack, &submode);
                    let attached = started.and_then(|id| {
                        let row = crate::session_key(id);
                        Ok(self.services.bank.lock().attached(&channel, &row)?)
                    });
                    if let Err(e) = attached {
                        self.bank_problem(&format!("{channel}: {e}"));
                    }
                    run.layout = true;
                }
                ChannelChange::Stop { channel, session } => {
                    run.feeds.retain(|(k, _, _)| *k != channel);
                    let id = session.and_then(|k| crate::parse_session_key(&k));
                    if let Some(id) = id {
                        // A session that the host closed already is gone.
                        let _ = self.end_session(id);
                    }
                    run.layout = true;
                }
                ChannelChange::Layout { radio } => {
                    if let Some(radio) = radio {
                        if let Err(e) = self.bank_radio(&mut run, &radio) {
                            self.bank_problem(&format!("span: {e}"));
                        }
                    }
                    run.layout = true;
                }
            }
        }
        if let Some(radio) = run.radio.and_then(|r| self.radio(r)) {
            let (rate, centre) = radio.feed.tuning();
            if rate > 0.0 && run.input != (rate, centre) {
                run.input = (rate, centre);
                self.services.bank.lock().input(rate, centre);
                run.layout = true;
            }
        }
        if std::mem::take(&mut run.layout) {
            if let Err(e) = self.build_bank(&mut run) {
                self.bank_problem(&e.to_string());
            }
        }
        self.bank_levels(&run, now);
        *lock(&self.bank) = run;
    }

    /// Makes `radio` (its row key in the `radios` table) the radio of the bank, if the bank has none.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchKey`] if no such radio is open. [`EngineError::BankRadio`] if the bank reads another radio.
    fn bank_radio(&self, run: &mut BankRun, radio: &str) -> Result<RadioId, EngineError> {
        let r = crate::parse_radio_key(radio)
            .filter(|r| self.radio(*r).is_some())
            .ok_or_else(|| EngineError::NoSuchKey {
                key: radio.to_owned(),
            })?;
        if run.radio.is_some_and(|x| x != r) {
            return Err(EngineError::BankRadio {
                radio: radio.to_owned(),
            });
        }
        run.radio = Some(r);
        Ok(r)
    }

    /// Starts the session of channel `channel` on radio `radio` (its row key in the `radios` table), fed by a feed of its own.
    fn start_channel(
        &self,
        run: &mut BankRun,
        channel: &str,
        radio: &str,
        stack: &str,
        submode: &str,
    ) -> Result<SessionId, EngineError> {
        let r = self.bank_radio(run, radio)?;
        let live = self.radio(r).is_some_and(|x| x.feed.live());
        let d = self.stack(stack)?;
        let sub = (!submode.is_empty()).then_some(submode);
        let rx = self.make_receiver(d, sub, &Values::defaults(&(d.settings)()), &self.station())?;
        let feed = Arc::new(Feed::new(
            live,
            Arc::clone(self.components.catalog()),
            self.context(),
        ));
        let id = self.start_on(
            None,
            By::Engine,
            rx,
            (Input::Radio(r), Arc::clone(&feed)),
            Some(d),
            sub,
            Arc::new(crate::streams::Streams::new(crate::AUDIO_CAP, 1)),
        )?;
        run.feeds.push((channel.to_owned(), id, feed));
        Ok(id)
    }

    /// Builds the graph of the filterbank session from the bank's channels and carriers: the channelizer, one sink per
    /// channel and the carrier scan. Starts the session, passes it the new graph, or ends it when nothing is left to cut.
    fn build_bank(&self, run: &mut BankRun) -> Result<(), EngineError> {
        let (split, keys, scan) = {
            let bank = self.services.bank.lock();
            let (split, keys) = bank.channelizer();
            let scan = (!bank.watched().is_empty()).then(|| bank.carrier_scan());
            (split, keys, scan)
        };
        let Some(radio) = run.radio else {
            return Ok(());
        };
        if keys.is_empty() && scan.is_none() {
            if let Some(id) = run.session.take() {
                // A session that the host closed already is gone.
                let _ = self.end_session(id);
            }
            return Ok(());
        }
        let mut b = GraphBuilder::new()
            .catalog(self.components.catalog())
            .add("split", split)
            .input("split");
        for (i, key) in keys.iter().enumerate() {
            let out = format!("out{i}");
            let feed = run.feeds.iter().find(|(k, _, _)| k == key);
            b = match feed {
                Some((_, _, f)) => b.add(
                    &out,
                    ChannelSink {
                        feed: Arc::clone(f),
                        rate_hz: 0.0,
                        centre_hz: 0.0,
                    },
                ),
                None => b.add(&out, NullSink::default()),
            };
            b = b.connect(&format!("split.ch{i}"), &out);
        }
        if let Some(scan) = scan {
            b = b.add(SCAN, scan).input(SCAN);
        }
        let mut graph = b.build()?;
        graph.set_context(self.context());
        let rx = Box::new(graph);
        match run.session.and_then(|id| self.session(id)) {
            Some(s) => s.replace(rx, None, None)?,
            None => {
                run.session =
                    Some(self.start(None, By::Engine, rx, Input::Radio(radio), None, None)?)
            }
        }
        let inputs = run
            .feeds
            .iter()
            .filter_map(|(_, id, _)| self.session(*id))
            .map(|s| MixIn::new(Arc::clone(&s.link.streams), Arc::clone(&s.link.gains)))
            .collect();
        if let Some(s) = run.session.and_then(|id| self.session(id)) {
            s.link.mix(inputs);
        }
        Ok(())
    }

    /// Passes the bank the readings of the carrier scan of the filterbank session at wall time `now`. If the session thread
    /// holds its graph, the readings wait for the next tick.
    fn bank_levels(&self, run: &BankRun, now: f64) {
        let Some(s) = run.session.and_then(|id| self.session(id)) else {
            return;
        };
        let readings = {
            let Some(mut r) = s.link.try_run() else {
                return;
            };
            let graph = r.rx.as_any_mut().and_then(|a| a.downcast_mut::<Graph>());
            match graph.and_then(|g| g.component_as::<CarrierScan>(SCAN)) {
                Some(scan) => scan.readings().to_vec(),
                None => return,
            }
        };
        let mut bank = self.services.bank.lock();
        // The scan holds no readings before its first frame.
        if readings.len() != bank.watched().len() {
            return;
        }
        if let Err(e) = bank.levels(now, &readings) {
            bank.problem(&e.to_string());
        }
    }

    fn bank_problem(&self, text: &str) {
        self.services.with(ServiceKind::Bank, |o| o.problem(text));
    }
}
