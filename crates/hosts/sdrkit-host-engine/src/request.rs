//! The requests of receivers and services: the engine carries out each one in one place, or refuses it with a typed error
//! that the requesting owner shows. Each request is also an event and a row of the engine owner.

use crate::feed::lock;
use crate::services::{open_for, ServiceKind};
use crate::session::{By, Session};
use crate::{EngineError, Input, OwnerRef, SessionId, Shared};
use sdrkit_core::publish::{field, keys};
use sdrkit_core::{
    ConfigError, Event, EventDef, Field, HostRequest, Owner, OwnerSetting, SignalType,
    StackDescriptor, TableDef, TuneTarget, Value, Values,
};
use sdrkit_svc_memory::{Memory, OnSame};

sdrkit_core::choice_enum! {
    /// The kind of a request.
    pub enum RequestKind {
        /// Move the receiver's own frequency.
        Retune = ("retune", "Retune"),
        /// Visit a list of channels in turn.
        Hop = ("hop", "Hop"),
        /// Receive a second channel for a while.
        Follow = ("follow", "Follow"),
        /// Stop receiving a followed channel.
        Release = ("release", "Release"),
        /// Tune a session, and start the stack it needs.
        Tune = ("tune", "Tune"),
        /// Save a channel as a memory.
        StoreMemory = ("store_memory", "Store memory"),
        /// Count a hit on a memory.
        MemoryHit = ("memory_hit", "Memory hit"),
        /// Record a channel.
        Record = ("record", "Record"),
        /// Save the last seconds of the rewind buffer.
        Rewind = ("rewind", "Save rewind"),
        /// Run a script of the host.
        Script = ("script", "Run script"),
        /// Send an event to one output.
        Output = ("output", "Send to output"),
    }
}

impl RequestKind {
    /// The kind of `r`.
    pub fn of(r: &HostRequest) -> RequestKind {
        match r {
            HostRequest::Retune { .. } => RequestKind::Retune,
            HostRequest::Hop { .. } => RequestKind::Hop,
            HostRequest::Follow { .. } => RequestKind::Follow,
            HostRequest::Release { .. } => RequestKind::Release,
            HostRequest::Tune { .. } => RequestKind::Tune,
            HostRequest::StoreMemory { .. } => RequestKind::StoreMemory,
            HostRequest::MemoryHit { .. } => RequestKind::MemoryHit,
            HostRequest::Record { .. } => RequestKind::Record,
        }
    }
}

/// The owner that made a request.
pub const FROM: Field<field::Text> = Field::new("from", "From", "");
/// The kind of a request.
pub const KIND: Field<field::Choice<RequestKind>> = Field::new("kind", "Request", "");
/// The frequency of a request, if it names one.
pub const FREQ_HZ: Field<field::Hz> = Field::new("freq_hz", "Frequency", "Hz");
/// The engine carried out the request.
pub const DONE: Field<field::Bool> = Field::new("done", "Done", "");
/// Why the engine refused the request.
pub const REFUSED: Field<field::Text> = Field::new("refused", "Refused because", "");
/// The session that a request started or tuned.
pub const SESSION: Field<field::Text> =
    Field::new("session", "Session", "").refers_to_table("sessions");

/// A request reached the engine: carried out, or refused with the reason.
pub static REQUEST: EventDef = EventDef::new(
    "engine.request",
    "Request",
    &[
        &FROM,
        &KIND,
        &FREQ_HZ,
        &DONE,
        &REFUSED,
        &SESSION,
        &keys::FAMILY,
    ],
);

/// The requests, newest last. The row key is a number that counts the requests.
pub static REQUESTS: TableDef = TableDef {
    key: "requests",
    label: "Requests",
    columns: &[&FROM, &KIND, &FREQ_HZ, &DONE, &REFUSED, &SESSION],
    cap: 256,
    marks: sdrkit_core::RowMarks::NONE,
};

/// A request from a rule of the automation: its kind, the session whose event set it off, the automation's event, and that
/// event.
pub(crate) struct Action {
    pub(crate) kind: RequestKind,
    pub(crate) session: Option<SessionId>,
    pub(crate) event: Event,
    pub(crate) trigger: Option<Event>,
}

impl Action {
    /// The kind of request that the automation's event `e` carries, if any.
    pub(crate) fn kind_of(e: &Event) -> Option<RequestKind> {
        use sdrkit_svc_automation::events::{RECORD, REWIND, RUN_SCRIPT, SEND, TUNE};
        [
            (&TUNE, RequestKind::Tune),
            (&RECORD, RequestKind::Record),
            (&REWIND, RequestKind::Rewind),
            (&RUN_SCRIPT, RequestKind::Script),
            (&SEND, RequestKind::Output),
        ]
        .into_iter()
        .find(|(d, _)| e.is(d))
        .map(|(_, k)| k)
    }
}

/// A request for the handler: one of a receiver or a service, or an action of the automation.
pub(crate) enum Ask {
    /// A request of a receiver or a service.
    Host(HostRequest),
    /// An action of the automation.
    Action(Action),
}

/// Who made a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Requester {
    /// The receiver of a session.
    Session(SessionId),
    /// A service.
    Service(ServiceKind),
}

/// A session that follows a channel for another session.
pub(crate) struct Follower {
    from: SessionId,
    freq_hz: f64,
    pub(crate) session: SessionId,
    /// The wall time at which the engine stops it.
    until: f64,
}

/// What a carried out request started or tuned.
type Served = Option<SessionId>;

/// The frequency that request `r` names, if it names one.
fn freq_of(r: &HostRequest) -> Option<f64> {
    match r {
        HostRequest::Retune { tuned_hz, .. } => Some(*tuned_hz),
        HostRequest::Follow { freq_hz, .. }
        | HostRequest::Release { freq_hz }
        | HostRequest::StoreMemory { freq_hz, .. }
        | HostRequest::Record { freq_hz, .. } => Some(*freq_hz),
        HostRequest::Tune { freq_hz, .. } => *freq_hz,
        HostRequest::Hop { .. } | HostRequest::MemoryHit { .. } => None,
    }
}

/// The value of the `bandwidth` setting `field` of a stack for a channel `hz` wide: the choice whose key is the width
/// as a number in MHz, kHz or Hz, or the width for a number in Hz. `None` if the setting has no such value.
fn width_value(field: &sdrkit_core::FieldDecl, hz: f64) -> Option<Value> {
    use sdrkit_core::ValueKind;
    match field.kind {
        ValueKind::Choice => {
            let fits = |key: &str| {
                key.parse::<f64>()
                    .is_ok_and(|k| [1e6, 1e3, 1.0].iter().any(|unit| k * unit == hz))
            };
            let i = field.choices.iter().position(|c| fits(&c.key))?;
            u16::try_from(i).ok().map(Value::Choice)
        }
        ValueKind::Hz if field.unit == "Hz" => Some(Value::Hz(hz)),
        ValueKind::F64 if field.unit == "Hz" => Some(Value::F64(hz)),
        _ => None,
    }
}

impl Shared {
    /// Carries out `ask` of `from`, or refuses it. Either way the engine raises a [`REQUEST`] event and writes a row of
    /// [`REQUESTS`]. A refusal also shows in the requesting owner: in a session's field [`crate::REQUEST_PROBLEM`], in a
    /// service through [`Owner::problem`].
    pub(crate) fn handle(&self, from: Requester, ask: Ask) {
        let (kind, freq) = match &ask {
            Ask::Host(r) => (RequestKind::of(r), freq_of(r)),
            Ask::Action(a) => (a.kind, Some(a.event.freq_hz).filter(|f| *f > 0.0)),
        };
        let outcome = match ask {
            Ask::Host(r) => self.serve(from, r),
            Ask::Action(a) => self.serve_action(a),
        };
        let from_key = match from {
            Requester::Session(id) => crate::session_key(id),
            Requester::Service(s) => s.key().to_owned(),
        };
        let refused = outcome.as_ref().err().map(ToString::to_string);
        match from {
            Requester::Session(id) => {
                if let Some(s) = self.session(id) {
                    let mut c = lock(&s.control);
                    match &refused {
                        Some(text) => c.problem(text),
                        None => c.request_served(),
                    }
                }
            }
            Requester::Service(s) => {
                if let Some(text) = &refused {
                    self.services.with(s, |o| o.problem(text));
                }
            }
        }
        let session = outcome.ok().flatten().map(crate::session_key);
        let mut e = self.engine.lock();
        let now = self.facilities.clock.now();
        e.note_request(
            now,
            &from_key,
            kind,
            freq,
            refused.as_deref(),
            session.as_deref(),
        );
    }

    fn serve(&self, from: Requester, req: HostRequest) -> Result<Served, EngineError> {
        req.validate()?;
        self.allowed()?;
        let kind = RequestKind::of(&req);
        let own = || match from {
            Requester::Session(id) => Ok(id),
            Requester::Service(_) => Err(EngineError::Unserved { kind }),
        };
        match req {
            HostRequest::Retune { tuned_hz, .. } => {
                let id = own()?;
                // A fixed frequency ends a hop of the session.
                self.end_hop_by(id, || self.tune_to(id, tuned_hz))?;
                Ok(Some(id))
            }
            HostRequest::Hop {
                channels,
                dwell_s,
                stop_on_lock,
            } => {
                let id = own()?;
                self.start_hop(id, channels, dwell_s, stop_on_lock)?;
                Ok(Some(id))
            }
            HostRequest::Follow {
                freq_hz,
                signal,
                max_s,
                settings,
                ..
            } => {
                let id = own()?;
                let (stack, submode) = match signal {
                    Some(sig) => self.open_signal(sig)?,
                    None => (
                        self.stack_of(id)?.ok_or(EngineError::Unserved { kind })?,
                        "",
                    ),
                };
                self.follow(id, stack, submode, (freq_hz, max_s), &settings)
                    .map(Some)
            }
            HostRequest::Release { freq_hz } => {
                let id = own()?;
                self.release(id, freq_hz)?;
                Ok(None)
            }
            HostRequest::Tune {
                session,
                freq_hz,
                target,
                submode,
                rate_hz,
                bandwidth_hz,
                settings,
            } => {
                // A session names itself with an empty key.
                let id = match (from, session.is_empty()) {
                    (Requester::Session(id), true) => id,
                    _ => self.session_by_key(&session)?,
                };
                // Only the stack of a session and the scan service state the rate of the radio. The rate of the scan
                // is the wide rate of one look: the next tune without a rate brings back the rate of the stack.
                let scan = from == Requester::Service(ServiceKind::Scan);
                let rate_hz = match (rate_hz, from) {
                    (Some(hz), Requester::Session(own)) if own == id => Some(hz),
                    (Some(hz), _) if scan => Some(hz),
                    (Some(_), _) => return Err(EngineError::Unserved { kind }),
                    (None, _) => None,
                };
                // A tune with a frequency ends a hop of the session.
                if freq_hz.is_some() {
                    self.end_hop(id);
                }
                // A visit of a scan starts with no result of the channel before.
                self.serve_tune(
                    id,
                    &target,
                    &submode,
                    &settings,
                    bandwidth_hz,
                    (freq_hz, scan),
                )?;
                if let Some(hz) = rate_hz {
                    self.take_stack_rate(id, hz)?;
                    // A wide look of the scan is a block of the radio: its centre is the frequency of the tune.
                    if let (true, Some(at)) = (scan, freq_hz) {
                        self.centre_radio(id, at)?;
                    }
                }
                Ok(Some(id))
            }
            HostRequest::StoreMemory {
                name,
                freq_hz,
                target,
                bandwidth_hz,
                settings,
                group,
            } => {
                let stack = match target {
                    TuneTarget::Running => match from {
                        Requester::Session(id) => {
                            self.stack_of(id)?.map(|d| d.id).unwrap_or_default()
                        }
                        Requester::Service(_) => "",
                    },
                    TuneTarget::Stack(ref s) => self.stack(s)?.id,
                    TuneTarget::Signal(sig) => self.open_signal(sig)?.0.id,
                };
                let mut m = Memory::new(&name, freq_hz, stack);
                m.bandwidth_hz = bandwidth_hz;
                m.settings = settings;
                m.group = group;
                self.store_memory(m)?;
                Ok(None)
            }
            HostRequest::MemoryHit { memory, at_s, tone } => {
                let id = memory
                    .parse()
                    .map_err(|_| EngineError::NoSuchKey { key: memory })?;
                self.services
                    .memory()
                    .lock()
                    .update_store(|s| s.record_hit(id, at_s, &tone))??;
                Ok(None)
            }
            HostRequest::Record { .. } => Err(EngineError::Unserved { kind }),
        }
    }

    /// Carries out an action of the automation. A tune tunes the session whose event set the rule off, and a record records
    /// it through the record service. A rewind saves the rewind buffer for the event's seconds. An output gets the event that
    /// set the rule off. A script runs in the host: the automation's event is the host's instruction, so the engine has
    /// nothing more to do.
    fn serve_action(&self, a: Action) -> Result<Served, EngineError> {
        self.allowed()?;
        match a.kind {
            RequestKind::Tune => {
                let id = a.session.ok_or(EngineError::NoSession)?;
                let target = match a.event.get(&sdrkit_svc_automation::events::STACK) {
                    Some(s) if !s.is_empty() => TuneTarget::Stack(s.to_owned()),
                    _ => TuneTarget::Running,
                };
                let freq = Some(a.event.freq_hz).filter(|f| *f > 0.0);
                self.serve_tune(id, &target, "", &[], None, (freq, false))?;
                Ok(Some(id))
            }
            RequestKind::Output => {
                let row = a
                    .event
                    .get(&sdrkit_svc_automation::events::OUTPUT)
                    .unwrap_or_default();
                let trigger = a.trigger.as_ref().ok_or(EngineError::NoSession)?;
                let ctx = sdrkit_svc_events::Context {
                    unix_time: self.facilities.clock.now(),
                    receiver: self.engine.lock().position(),
                };
                self.services.send_to(row, trigger, &ctx)?;
                Ok(None)
            }
            RequestKind::Record => {
                use sdrkit_svc_automation::events::{RecordWhat, SECONDS, WHAT};
                let id = a.session.ok_or(EngineError::NoSession)?;
                let what = a.event.get(&WHAT).unwrap_or(RecordWhat::Audio);
                self.record_session(id, what, a.event.get(&SECONDS).unwrap_or(0.0))?;
                Ok(Some(id))
            }
            RequestKind::Rewind => {
                let seconds = a
                    .event
                    .get(&sdrkit_svc_automation::events::SECONDS)
                    .filter(|s| *s > 0.0);
                let args: Vec<(&str, Value)> = seconds
                    .map(|s| ("pre_s", Value::Duration(s)))
                    .into_iter()
                    .chain([("reason", Value::Text("automation".into()))])
                    .collect();
                self.record_command("rewind.save", &args)?;
                Ok(None)
            }
            RequestKind::Script => Ok(None),
            kind => Err(EngineError::Unserved { kind }),
        }
    }

    /// Refuses with [`EngineError::RequestsOff`] when the engine's setting `requests_allowed` is off.
    fn allowed(&self) -> Result<(), EngineError> {
        if self.engine.lock().requests_allowed() {
            Ok(())
        } else {
            Err(EngineError::RequestsOff)
        }
    }

    /// The stack that receives `signal`, with its submode for it (empty for the default).
    pub(crate) fn open_signal(
        &self,
        signal: SignalType,
    ) -> Result<(&'static StackDescriptor, &'static str), EngineError> {
        open_for(&self.keys.stacks, signal).ok_or(EngineError::NoStackFor { signal })
    }

    /// Tunes session `id`: puts the stack of `target` in it, applies `settings` and the bandwidth, then tunes it to
    /// the frequency of `to`: (the frequency, fresh). With fresh, the session starts with no result of the frequency
    /// before ([`Shared::tune`]).
    fn serve_tune(
        &self,
        id: SessionId,
        target: &TuneTarget,
        submode: &str,
        settings: &[OwnerSetting],
        bandwidth_hz: Option<f64>,
        to: (Option<f64>, bool),
    ) -> Result<(), EngineError> {
        let put = self.retarget(id, target, submode)?;
        self.apply_settings(id, settings, bandwidth_hz)?;
        if put {
            // The operator of the new stack demands its rate from the start: the radio has that of the stack before.
            if let Some(s) = self.session(id) {
                self.set_stack_rate(&s, true)?;
            }
        }
        if let (Some(hz), fresh) = to {
            self.tune(id, hz, fresh)?;
        }
        Ok(())
    }

    /// Saves `m` as a memory. A memory of the same channel (the same stack, the frequency within the memory service's
    /// tolerance) is updated in place of a second one.
    fn store_memory(&self, m: Memory) -> Result<(), EngineError> {
        let now = self.facilities.clock.now();
        let mut svc = self.services.memory().lock();
        let id = sdrkit_core::SettingSet::id(sdrkit_svc_memory::MemorySetting::SameChannel);
        let tolerance = match svc.values().and_then(|v| v.value(id)) {
            Some(Value::Hz(hz)) => *hz,
            _ => 0.0,
        };
        svc.update_store(|s| s.store_current(m, OnSame::Update, now, tolerance))??;
        Ok(())
    }

    /// The session with the row key `key` of the `sessions` table.
    pub(crate) fn session_by_key(&self, key: &str) -> Result<SessionId, EngineError> {
        let none = || EngineError::NoSuchSession {
            key: key.to_owned(),
        };
        let id = crate::parse_session_key(key).ok_or_else(none)?;
        self.session(id).ok_or_else(none)?;
        Ok(id)
    }

    /// The stack that session `id` runs. `None` for a graph of the host.
    fn stack_of(&self, id: SessionId) -> Result<Option<&'static StackDescriptor>, EngineError> {
        let s = self
            .session(id)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?;
        let stack = lock(&s.control).stack;
        Ok(stack)
    }

    /// Tunes session `id` to `hz`: its operator listens there, the session follows, and the radio does what the operator
    /// then demands ([`Shared::move_radio`]).
    pub(crate) fn tune_to(&self, id: SessionId, hz: f64) -> Result<(), EngineError> {
        self.tune(id, hz, false)
    }

    /// [`Shared::tune_to`]. With `fresh`, the stack forgets the results of the frequency before and the session shows
    /// none of them until the stack has taken the change, also when `hz` is the frequency that the session has.
    fn tune(&self, id: SessionId, hz: f64, fresh: bool) -> Result<(), EngineError> {
        let s = self
            .session(id)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?;
        {
            let mut c = lock(&s.control);
            c.operator.set_frequency(hz);
            if fresh {
                c.tune_fresh(hz)?;
            } else {
                c.tune(hz)?;
            }
        }
        self.drop_stack_rate(&s)?;
        self.move_radio(id, &s)
    }

    /// Carries out what the operator of session `s` demands of the radio that the session reads, while the radio streams.
    /// This is the place that moves a streaming radio for a session; only the wide look of a scan sets the centre
    /// itself ([`Shared::centre_radio`]). The transmitter goes to the frequency that the
    /// operator sends on. The centre of the receiver moves to the frequency that the operator listens to, plus its offset,
    /// if that frequency is more than 40 % of the sample rate from the centre; else it stays. Does nothing for a session
    /// that reads no radio and for an operator that demands no frequency. While another session hops the radio, the
    /// centre stays where the hop put it ([`crate::hop`]).
    pub(crate) fn move_radio(&self, id: SessionId, s: &Session) -> Result<(), EngineError> {
        let Input::Radio(r) = s.input else {
            return Ok(());
        };
        let radio = self
            .radio(r)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(r)))?;
        let (demand, tuning) = {
            let c = lock(&s.control);
            (c.operator.demand(), c.stack.map(|d| d.tuning.clone()))
        };
        if let Some(tx) = demand.transmit {
            // A frequency that the transmitter does not have stays out of the radio's settings; a burst there is refused.
            let _ = radio.owner.lock().tx_tune(tx.centre_hz, None);
        }
        let Some(rx) = demand.receive else {
            return Ok(());
        };
        if rx.tuned_hz <= 0.0 || self.hopped_by_other(r, id) {
            return Ok(());
        }
        let (rate_hz, centre_hz) = radio.feed.tuning();
        if rate_hz <= 0.0 || (rx.tuned_hz - centre_hz).abs() <= 0.4 * rate_hz {
            return Ok(());
        }
        let mut o = radio.owner.lock();
        let c = EngineError::id_of(o.schema(), sdrkit_driver_core::CENTRE.field.key)?;
        let offset = match &tuning {
            Some(t) if o.capabilities().realtime => radio_offset(t, o.centre_clean(), rate_hz),
            _ => rx.tune_offset_hz,
        };
        let centre = sdrkit_core::Value::Hz(rx.tuned_hz + offset);
        EngineError::apply(&mut *o, c, &centre)?;
        Ok(())
    }

    /// Moves the centre of the radio that session `id` reads to `hz`, however near the centre is. Does nothing for a
    /// session that reads no radio.
    fn centre_radio(&self, id: SessionId, hz: f64) -> Result<(), EngineError> {
        let Some(Input::Radio(r)) = self.session(id).map(|s| s.input) else {
            return Ok(());
        };
        let radio = self
            .radio(r)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(r)))?;
        if radio.feed.tuning().1 == hz {
            return Ok(());
        }
        let mut o = radio.owner.lock();
        let c = EngineError::id_of(o.schema(), sdrkit_driver_core::CENTRE.field.key)?;
        EngineError::apply(&mut *o, c, &sdrkit_core::Value::Hz(hz))
    }

    /// Returns true if radio `r` tunes, false if it has one fixed frequency (a recording).
    pub(crate) fn radio_tunes(&self, r: crate::RadioId) -> Result<bool, EngineError> {
        let radio = self
            .radio(r)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(r)))?;
        let (lo, hi) = radio.owner.lock().freq_range();
        Ok(hi > lo)
    }

    /// Carries out `demand` on radio `r` before a session of the operator that made it starts. This is the one place that
    /// sets up a radio for a session. The transmitter goes to the frequency that the operator sends on. If no session reads
    /// the radio, the receiver takes the rate for the rate of the demand ([`Shared::set_radio_rate`]; `least_hz` is the
    /// least rate that the stack runs at), and its centre goes to the frequency that the operator listens to, plus its
    /// offset. The centre stays for an operator with no frequency: the session then listens the offset below it. A radio
    /// that a session reads keeps its rate and its centre.
    ///
    /// # Errors
    ///
    /// [`EngineError::Config`] with [`ConfigError::RateTooLow`] if no rate of the radio, or the rate that the host
    /// set, is enough for the stack; [`EngineError::LinkTooSlow`] if the link to the radio carries less than the least rate
    /// of `stack`. The radio is left as it was then.
    pub(crate) fn prepare_radio(
        &self,
        r: crate::RadioId,
        demand: &sdrkit_core::Demand,
        tuning: &sdrkit_core::Tuning,
        stack: &'static str,
    ) -> Result<(), EngineError> {
        let least_hz = tuning.min_rate_hz;
        let radio = self
            .radio(r)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(r)))?;
        let mut o = radio.owner.lock();
        if let Some(tx) = demand.transmit {
            // A frequency that the transmitter does not have stays out of the radio's settings; a burst there is refused.
            let _ = o.tx_tune(tx.centre_hz, None);
        }
        let Some(rx) = demand.receive else {
            return Ok(());
        };
        if radio.feed.readers() > 0 {
            return Ok(());
        }
        if rx.rate_hz > 0.0 {
            if let Some(note) = Self::set_radio_rate(&mut o, rx.rate_hz, least_hz, stack)? {
                self.log(note);
            }
        }
        if rx.tuned_hz <= 0.0 {
            return Ok(());
        }
        let rate_hz = match radio.feed.tuning().0 {
            r if r > 0.0 => r,
            _ => rx.rate_hz,
        };
        // A test signal or a recording has no converter: it keeps the offset that its station was made at.
        let offset = if o.capabilities().realtime {
            radio_offset(tuning, o.centre_clean(), rate_hz)
        } else {
            rx.tune_offset_hz
        };
        let centre = rx.tuned_hz + offset;
        let id = EngineError::id_of(o.schema(), sdrkit_driver_core::CENTRE.field.key)?;
        EngineError::apply(&mut *o, id, &sdrkit_core::Value::Hz(centre))?;
        Ok(())
    }

    /// Gives the radio `o` the rate for `stack`, which would like `want_hz` and runs at `least_hz` or more. This is the
    /// one rule for the rate of every radio. A radio with the rate on Automatic takes the rate of [`automatic_rate`],
    /// held to what its link carries ([`sdrkit_driver_core::RateLimits::steady_hz`], see [`link_rate`]). A rate that the
    /// host set stays; one above what the link carries is warned about once. Returns a note for the log when the rate
    /// is held below what the stack would like, or above the link.
    ///
    /// # Errors
    ///
    /// [`EngineError::Config`] with [`ConfigError::RateTooLow`]: the least rate of the stack, and the rate that the
    /// host set or the highest rate of the radio. [`EngineError::LinkTooSlow`]: the link carries less than the least rate
    /// of the stack; nothing is written to the radio.
    fn set_radio_rate(
        o: &mut crate::radio::RadioOwner,
        want_hz: f64,
        least_hz: f64,
        stack: &'static str,
    ) -> Result<Option<String>, EngineError> {
        let rate = &sdrkit_driver_core::RATE;
        let now = sdrkit_core::Owner::values(o).map(|v| v.get(rate));
        let link = o.rates().steady_hz.filter(|c| *c > 0.0);
        if !o.rate_is_automatic() {
            return match now {
                Some(got) if got < least_hz => Err(ConfigError::RateTooLow {
                    need: least_hz,
                    got,
                }
                .into()),
                _ => Ok(None),
            };
        }
        // A radio that does not list its rates has those of its rate setting.
        let schema = sdrkit_core::Owner::schema(o);
        let id = EngineError::id_of(schema, rate.field.key)?;
        let limits = schema.setting(id).and_then(|s| s.range);
        let listed = &o.rates().ranges;
        let ranges: Vec<(f64, f64)> = match (listed.is_empty(), limits) {
            (false, _) => listed.clone(),
            (true, Some(r)) => vec![(r.min, r.max)],
            (true, None) => Vec::new(),
        };
        let mut resolved = if ranges.is_empty() {
            want_hz
        } else {
            automatic_rate(&ranges, want_hz, least_hz)?
        };
        let mut note = None;
        if let Some(cap) = link {
            if resolved > cap * 1.01 {
                // The link figure is a live reading: it chooses a narrower rate where the stack allows one, and it
                // never refuses a start.
                let text = match link_rate(&ranges, cap, least_hz) {
                    Ok(fit) => {
                        let text = format!(
                            "the link to this radio carries about {} Msps now: {stack} runs at {} Msps, a narrower capture \
                             than the {} Msps it prefers",
                            sdrkit_driver_core::fmt_m(cap),
                            sdrkit_driver_core::fmt_m(fit),
                            sdrkit_driver_core::fmt_m(resolved)
                        );
                        resolved = fit;
                        text
                    }
                    // The stack needs more than the link carries now: it runs at its rate, and the radio's own
                    // status says what arrives.
                    Err(()) => String::new(),
                };
                if !text.is_empty() {
                    o.note(&text);
                    note = Some(text);
                }
            }
        }
        if now != Some(resolved) {
            o.set_resolved_rate(resolved)?;
        }
        Ok(note)
    }

    /// Gives the operator of session `s` the sample rate that its stack would like with its present settings. If that
    /// is another rate than the operator demanded before, the radio is set to it, unless another session reads the
    /// radio too. Does nothing for a stack whose settings do not give the rate ([`sdrkit_core::Setting::sets_rate`]).
    pub(crate) fn follow_rate(&self, s: &Session) -> Result<(), EngineError> {
        self.set_stack_rate(s, false)
    }

    /// The stack of session `id` asks for the sample rate `hz`: the operator demands it, and the radio is set to it
    /// unless another session reads the radio too. The rate counts until the settings of the stack give another one,
    /// the frequency of the session changes or another stack comes into the session.
    fn take_stack_rate(&self, id: SessionId, hz: f64) -> Result<(), EngineError> {
        let s = self
            .session(id)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?;
        {
            let mut c = s.owner();
            let Some(desc) = c.stack else {
                return Ok(());
            };
            let Some(stated) = sdrkit_core::Owner::values(&*c).map(|v| desc.rate_hz(v)) else {
                return Ok(());
            };
            c.asked_rate = Some((stated, hz));
        }
        self.set_stack_rate(&s, false)
    }

    /// The frequency of session `s` changed: a rate that its stack asked for on the frequency before counts no more.
    pub(crate) fn drop_stack_rate(&self, s: &Session) -> Result<(), EngineError> {
        if lock(&s.control).asked_rate.take().is_none() {
            return Ok(());
        }
        self.set_stack_rate(s, false)
    }

    /// [`Shared::follow_rate`]. With `always`, the radio is set to the rate also if the operator demanded it before.
    /// The radio takes the rate by the rule of [`Shared::set_radio_rate`].
    fn set_stack_rate(&self, s: &Session, always: bool) -> Result<(), EngineError> {
        let Some(desc) = lock(&s.control).stack else {
            return Ok(());
        };
        // The session holds the settings of its stack under the keys of the stack.
        let mut c = s.owner();
        if !always && !(desc.settings)().sets_rate() && c.asked_rate.is_none() {
            return Ok(());
        }
        let Some(stated) = sdrkit_core::Owner::values(&*c).map(|v| desc.rate_hz(v)) else {
            return Ok(());
        };
        // A rate that the stack asked for counts while its settings give the rate that they gave then.
        let rate_hz = match c.asked_rate {
            Some((then, asked)) if then.to_bits() == stated.to_bits() => asked,
            _ => {
                c.asked_rate = None;
                stated
            }
        };
        let demanded = c.operator.demand().receive.map_or(0.0, |r| r.rate_hz);
        if rate_hz.to_bits() == demanded.to_bits() && !always {
            return Ok(());
        }
        c.operator.set_rate(rate_hz);
        drop(c);
        let Input::Radio(r) = s.input else {
            return Ok(());
        };
        // A recording keeps its rate, as it does when a session starts on it.
        if !self.radio_tunes(r)? {
            return Ok(());
        }
        let radio = self
            .radio(r)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(r)))?;
        if rate_hz <= 0.0 || radio.feed.readers() > 1 {
            return Ok(());
        }
        let mut o = radio.owner.lock();
        let r = Self::set_radio_rate(&mut o, rate_hz, desc.tuning.min_rate_hz, desc.name);
        drop(o);
        if let Ok(Some(note)) = &r {
            self.log(note.clone());
        }
        r.map(|_| ())
    }

    /// The host gave the rate of radio `r` back to the engine (Automatic): the radio takes the rate for the stack of
    /// the one session that reads it. With no session or more than one, the radio keeps its rate until a stack starts.
    pub(crate) fn resolve_rate_again(&self, r: crate::RadioId) -> Result<(), EngineError> {
        let readers: Vec<_> = lock(&self.sessions)
            .iter()
            .filter(|(_, s)| s.input == Input::Radio(r) && s.by != crate::session::By::Share)
            .map(|(_, s)| std::sync::Arc::clone(s))
            .collect();
        match readers.as_slice() {
            [s] => self.set_stack_rate(s, true),
            _ => Ok(()),
        }
    }

    /// Puts the stack of `target` (in `submode`) in session `id`, if it does not run it already in that submode. True if it
    /// put a stack.
    fn retarget(
        &self,
        id: SessionId,
        target: &TuneTarget,
        submode: &str,
    ) -> Result<bool, EngineError> {
        let running = self.stack_of(id)?;
        let wanted = match target {
            TuneTarget::Running => {
                if submode.is_empty() {
                    return Ok(false);
                }
                running.ok_or(EngineError::Unserved {
                    kind: RequestKind::Tune,
                })?
            }
            TuneTarget::Stack(s) => self.stack(s)?,
            TuneTarget::Signal(sig) => {
                let (d, sub) = self.open_signal(*sig)?;
                if submode.is_empty() {
                    return self.put_stack(id, running, d, sub);
                }
                d
            }
        };
        self.put_stack(id, running, wanted, submode)
    }

    /// Puts stack `wanted` in `submode` (empty for the default) in session `id`, which runs `running`. Nothing changes if
    /// it runs `wanted` already, in `submode` when one is given. True if it put the stack.
    fn put_stack(
        &self,
        id: SessionId,
        running: Option<&'static StackDescriptor>,
        wanted: &'static StackDescriptor,
        submode: &str,
    ) -> Result<bool, EngineError> {
        if running.is_some_and(|r| r.id == wanted.id) {
            let s = self
                .session(id)
                .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?;
            let same = submode.is_empty() || lock(&s.control).submode.as_deref() == Some(submode);
            if same {
                return Ok(false);
            }
        }
        let values = Values::defaults(&(wanted.settings)());
        let sub = (!submode.is_empty()).then_some(submode);
        let rx = self.make_receiver(wanted, sub, &values, &self.station())?;
        let s = self
            .session(id)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?;
        s.replace(rx, Some(wanted), sub)?;
        self.set_session_stack(id, Some(wanted.id));
        self.tell_scan(id);
        Ok(true)
    }

    /// Applies `settings` to the receiver of session `id`, and the bandwidth if its receiver has a `bandwidth` setting
    /// with that width ([`width_value`]; a stack without one keeps its own bandwidth). A setting of an owner that the session does not run is refused.
    fn apply_settings(
        &self,
        id: SessionId,
        settings: &[OwnerSetting],
        bandwidth_hz: Option<f64>,
    ) -> Result<(), EngineError> {
        let s = self
            .session(id)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(id)))?;
        let mut c = lock(&s.control);
        let stack_key = c.stack.map(|d| format!("stack.{}", d.id));
        for o in settings {
            let refused = || EngineError::NotInSession {
                owner: o.owner.as_str().to_owned(),
                key: o.key.clone(),
            };
            if stack_key.as_deref() != Some(o.owner.as_str()) {
                return Err(refused());
            }
            let sid = c.receiver_setting(&o.key).ok_or_else(refused)?;
            EngineError::apply(&mut *c, sid, &o.value)?;
        }
        if let (Some(bw), Some(sid)) = (bandwidth_hz, c.receiver_setting("bandwidth")) {
            let width = sdrkit_core::Owner::schema(&*c)
                .setting(sid)
                .and_then(|decl| width_value(&decl.field, bw));
            if let Some(v) = width {
                EngineError::apply(&mut *c, sid, &v)?;
            }
        }
        drop(c);
        self.follow_rate(&s)
    }

    /// Starts a session of `stack` in `submode` (empty for the default) on the radio of session `from`, tuned to `freq_hz`,
    /// for `max_s` seconds, with the stack settings `settings` that the stack has and takes.
    fn follow(
        &self,
        from: SessionId,
        stack: &'static StackDescriptor,
        submode: &str,
        (freq_hz, max_s): (f64, f64),
        settings: &[(String, Value)],
    ) -> Result<SessionId, EngineError> {
        let s = self
            .session(from)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Session(from)))?;
        let Input::Radio(radio) = s.input else {
            return Err(EngineError::NoRadio);
        };
        let mut values = Values::defaults(&(stack.settings)());
        for (key, v) in settings {
            if let Ok(id) = values.schema().id(key) {
                // A value that the stack refuses keeps the default.
                let _ = values.set(id, v);
            }
        }
        let sub = (!submode.is_empty()).then_some(submode);
        let held = self.check_signal(Input::Radio(radio), stack, &values)?;
        let rx = self.make_receiver(stack, sub, &values, &self.station())?;
        let id = self.start(None, By::Engine, rx, Input::Radio(radio), Some(stack), sub)?;
        if let Some(s) = self.session(id) {
            crate::lock(&s.control).keep_signal_settings(&values);
        }
        self.tune_to(id, freq_hz)?;
        held.keep();
        let until = self.facilities.clock.now() + max_s;
        lock(&self.followers).push(Follower {
            from,
            freq_hz,
            session: id,
            until,
        });
        Ok(id)
    }

    /// Stops the session that follows `freq_hz` for session `from`.
    fn release(&self, from: SessionId, freq_hz: f64) -> Result<(), EngineError> {
        let f = {
            let mut fs = lock(&self.followers);
            let i = fs
                .iter()
                .position(|f| f.from == from && f.freq_hz == freq_hz)
                .ok_or(EngineError::NotFollowed { freq_hz })?;
            fs.swap_remove(i)
        };
        self.end_session(f.session)
    }

    /// Stops the followers whose time ran out at wall time `now`, and those whose session is gone.
    pub(crate) fn expire_followers(&self, now: f64) {
        let ended: Vec<SessionId> = {
            let mut fs = lock(&self.followers);
            let mut out = Vec::new();
            fs.retain(|f| {
                let keep = f.until > now && self.session(f.from).is_some();
                if !keep {
                    out.push(f.session);
                }
                keep
            });
            out
        };
        for s in ended {
            // A follower that the host closed already is gone.
            let _ = self.end_session(s);
        }
    }
}

/// The rate of a radio with the rates `ranges` (inclusive, a single rate is a range from it to itself) for a stack that
/// would like `want_hz` and runs at `least_hz` or more: the least rate of the radio at or above `want_hz`. If the radio
/// has none, its highest rate, and the session resamples.
///
/// # Errors
///
/// [`ConfigError::RateTooLow`] with `least_hz` and the highest rate of the radio, if that rate is below `least_hz`.
/// The highest rate of the radio's `ranges` (none: any rate) at or below `cap_hz`, the rate its link carries, and at or
/// above `least_hz`. `Err(())` when no such rate exists.
pub(crate) fn link_rate(ranges: &[(f64, f64)], cap_hz: f64, least_hz: f64) -> Result<f64, ()> {
    let best = if ranges.is_empty() {
        cap_hz
    } else {
        ranges
            .iter()
            .filter(|(lo, _)| *lo <= cap_hz)
            .map(|(_, hi)| hi.min(cap_hz))
            .fold(f64::NEG_INFINITY, f64::max)
    };
    if best.is_finite() && best >= least_hz && best > 0.0 {
        Ok(best)
    } else {
        Err(())
    }
}

pub(crate) fn automatic_rate(
    ranges: &[(f64, f64)],
    want_hz: f64,
    least_hz: f64,
) -> Result<f64, ConfigError> {
    let fit = ranges
        .iter()
        .filter(|(_, hi)| want_hz <= *hi)
        .map(|(lo, _)| lo.max(want_hz))
        .fold(f64::INFINITY, f64::min);
    if fit.is_finite() {
        return Ok(fit);
    }
    let highest = ranges.iter().map(|r| r.1).fold(0.0, f64::max);
    if highest >= least_hz {
        Ok(highest)
    } else {
        Err(ConfigError::RateTooLow {
            need: least_hz,
            got: highest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_holds_the_rate_down_or_refuses_it() {
        let pluto = [(2.1e6, 61.44e6)];
        // A link of 4 Msps: a stack that runs at 2 Msps or more gets 4.
        assert_eq!(link_rate(&pluto, 4e6, 2e6), Ok(4e6));
        // Wi-Fi needs 20.
        assert_eq!(link_rate(&pluto, 4e6, 20e6), Err(()));
        // Below the radio's lowest rate.
        assert_eq!(link_rate(&pluto, 1e6, 0.0), Err(()));
        let steps = [(1e6, 1e6), (2e6, 2e6), (4e6, 4e6)];
        assert_eq!(link_rate(&steps, 3e6, 1e6), Ok(2e6));
        assert_eq!(link_rate(&[], 3e6, 1e6), Ok(3e6));
    }

    #[test]
    fn automatic_takes_the_least_rate_of_the_radio_that_is_enough() {
        let steps = [(1e6, 1e6), (2e6, 2e6), (4e6, 4e6)];
        assert_eq!(automatic_rate(&steps, 3e6, 1e6), Ok(4e6));
        assert_eq!(automatic_rate(&steps, 2e6, 1e6), Ok(2e6));
        assert_eq!(automatic_rate(&steps, 48e3, 1e3), Ok(1e6));
        let ranges = [(225_001.0, 300_000.0), (900_001.0, 2.56e6)];
        assert_eq!(automatic_rate(&ranges, 250e3, 1e3), Ok(250e3));
        assert_eq!(automatic_rate(&ranges, 500e3, 1e3), Ok(900_001.0));
    }

    #[test]
    fn automatic_takes_the_highest_rate_while_the_stack_runs_at_it() {
        let ranges = [(225_001.0, 300_000.0), (900_001.0, 2.56e6)];
        assert_eq!(automatic_rate(&ranges, 8e6, 2e6), Ok(2.56e6));
        let refused = ConfigError::RateTooLow {
            need: 5.6e6,
            got: 2.56e6,
        };
        assert_eq!(automatic_rate(&ranges, 8e6, 5.6e6), Err(refused));
    }
}

/// The part of the sample rate, either side of the centre, that a channel stays inside: the filters of the radio roll off
/// beyond it.
const CAPTURE_SHARE: f64 = 0.4;
/// The distance from the centre of the radio to the centre of the channel, in half channel widths: a quarter of a half
/// width beyond the edge of the channel.
const OFFSET_MARGIN: f64 = 1.25;

/// How far above the station the engine tunes a radio with a clean centre or not (`clean`, see
/// [`sdrkit_driver_core::form::centre_is_clean`]) at `rate_hz`, for a stack of `tuning`. The offset keeps the carrier of
/// the radio's converter out of the channel:
///
/// - 0 for a stack that must not be offset-tuned, and on a clean centre, unless the signal has a carrier at its centre
///   that the DC removal of the radio would take;
/// - else the larger of the stack's own offset and [`OFFSET_MARGIN`] half channel widths, rounded up to 1, 2 or 5 times
///   a power of ten;
/// - 0 when the channel would then reach beyond [`CAPTURE_SHARE`] of the rate from the centre.
pub(crate) fn radio_offset(t: &sdrkit_core::Tuning, clean: bool, rate_hz: f64) -> f64 {
    if t.no_offset_tuning || (clean && !t.carrier_at_centre) {
        return 0.0;
    }
    let half = t.bandwidth_hz.max(0.0) / 2.0;
    let offset = round_up(half * OFFSET_MARGIN).max(t.tune_offset_hz);
    if rate_hz > 0.0 && offset + half > CAPTURE_SHARE * rate_hz {
        return 0.0;
    }
    offset
}

/// `x` rounded up to 1, 2 or 5 times a power of ten. 0 for 0.
fn round_up(x: f64) -> f64 {
    if x <= 0.0 || !x.is_finite() {
        return 0.0;
    }
    let p = 10f64.powf(x.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * p)
        .find(|v| *v >= x * (1.0 - 1e-12))
        .unwrap_or(10.0 * p)
}

#[cfg(test)]
mod offset_tests {
    use super::*;

    fn narrow() -> sdrkit_core::Tuning {
        // 12.5 kHz voice, which asks for 20 kHz itself.
        sdrkit_core::Tuning {
            tune_offset_hz: 20e3,
            ..sdrkit_core::Tuning::anywhere(446e6, 250e3, 48e3, 12.5e3)
        }
    }

    #[test]
    fn a_narrow_stack_on_a_clean_radio_is_tuned_onto_its_station() {
        assert_eq!(radio_offset(&narrow(), true, 2e6), 0.0);
    }

    #[test]
    fn a_narrow_stack_on_a_radio_without_correction_keeps_the_carrier_beyond_its_channel() {
        let off = radio_offset(&narrow(), false, 2e6);
        // Beyond the channel, and at least the 20 kHz the stack asks for.
        assert_eq!(off, 20e3);
        // An AM signal keeps its carrier off the centre even on a clean radio.
        let am = sdrkit_core::Tuning {
            carrier_at_centre: true,
            ..sdrkit_core::Tuning::anywhere(1e6, 250e3, 48e3, 10e3)
        };
        assert_eq!(radio_offset(&am, true, 2e6), 10e3);
    }

    #[test]
    fn a_wide_stack_is_on_its_centre_or_beyond_its_channel() {
        let fm = sdrkit_core::Tuning::anywhere(104.9e6, 2.4e6, 1e6, 200e3);
        assert_eq!(radio_offset(&fm, true, 2.4e6), 0.0);
        let off = radio_offset(&fm, false, 2.4e6);
        assert_eq!(off, 200e3);
        assert!(off - 100e3 > 0.0);
    }

    #[test]
    fn a_capture_too_narrow_for_the_offset_leaves_the_centre_on_the_station() {
        let fm = sdrkit_core::Tuning::anywhere(104.9e6, 2.4e6, 1e6, 200e3);
        assert_eq!(radio_offset(&fm, false, 0.5e6), 0.0);
        let held = sdrkit_core::Tuning {
            no_offset_tuning: true,
            ..narrow()
        };
        assert_eq!(radio_offset(&held, false, 2e6), 0.0);
    }

    #[test]
    fn offsets_are_round() {
        assert_eq!(round_up(7812.5), 10e3);
        assert_eq!(round_up(125e3), 200e3);
        assert_eq!(round_up(20e3), 20e3);
    }
}
