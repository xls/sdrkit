//! The recordings of the record service on the sessions: the engine passes each recording to the thread of its session, which
//! feeds it the session's audio or samples. The rewind buffer takes the samples of the first session on a radio.

use crate::feed::lock;
use crate::services::ServiceKind;
use crate::session::{Session, TUNED};
use crate::{EngineError, Input, SessionId, Shared};
use sdrkit_core::{owner, Owner, Record, Value};
use sdrkit_svc_record::Feed;
use std::sync::Arc;

impl Shared {
    /// Hands the recordings that the record service started to their sessions. A rewind buffer waits for a session on a
    /// radio. A recording whose session is gone is dropped, and the record service shows why.
    pub(crate) fn give_records(&self) {
        let mut feeds = self.services.record.lock().take_feeds();
        feeds.append(&mut lock(&self.rewind_wait));
        for f in feeds {
            let key = match &f {
                Feed::Audio(a) => Some(a.session()),
                Feed::Iq(i) => Some(i.session()),
                Feed::Ts(t) => Some(t.session()),
                Feed::TimeShift(t) => Some(t.session()),
                Feed::Rewind(_) => None,
            };
            let session = match key {
                Some(k) => match self.session_by_key(k) {
                    Ok(id) => self.session(id),
                    Err(e) => {
                        self.services
                            .with(ServiceKind::Record, |o| o.problem(&e.to_string()));
                        continue;
                    }
                },
                None => self.first_on_radio(),
            };
            match session {
                Some(s) => give(&s, f),
                None => lock(&self.rewind_wait).push(f),
            }
        }
    }

    /// The oldest session that reads a radio.
    fn first_on_radio(&self) -> Option<Arc<Session>> {
        lock(&self.sessions)
            .iter()
            .map(|(_, s)| s)
            .find(|s| matches!(s.input, Input::Radio(_)))
            .cloned()
    }

    /// Runs command `key` of the record service with the arguments `args` (argument key and value).
    ///
    /// # Errors
    ///
    /// [`EngineError::Command`] if the service has no such command or refuses it.
    pub(crate) fn record_command(
        &self,
        key: &str,
        args: &[(&str, Value)],
    ) -> Result<(), EngineError> {
        self.services.with(ServiceKind::Record, |o| {
            let unknown = |key: &str| {
                let error = sdrkit_core::CommandError::Unknown {
                    key: key.to_owned(),
                };
                EngineError::Command {
                    text: error.to_string(),
                    error,
                }
            };
            let id = o.commands().id(key).map_err(|_| unknown(key))?;
            let def = o.commands().def(id).ok_or_else(|| unknown(key))?.clone();
            let mut given: Record = def.new_args();
            for (k, v) in args {
                let arg = def.args.id(k).map_err(|_| unknown(k))?;
                given
                    .set_value(sdrkit_core::FieldId(arg.0), v)
                    .map_err(|_| unknown(k))?;
            }
            owner::invoke(o, id, &given)
                .map(|_| ())
                .map_err(|e| EngineError::command(o, id, e))
        })
    }

    /// Records the audio or the samples of session `id` for `seconds` (0: until stopped), through the record service.
    pub(crate) fn record_session(
        &self,
        id: SessionId,
        what: sdrkit_svc_automation::events::RecordWhat,
        seconds: f64,
    ) -> Result<(), EngineError> {
        use sdrkit_svc_automation::events::RecordWhat;
        let session = Value::Text(crate::session_key(id));
        let seconds = Value::Duration(seconds.max(0.0));
        let args = [("session", session), ("seconds", seconds)];
        if matches!(what, RecordWhat::Audio | RecordWhat::Both) {
            self.record_command("audio.start", &args)?;
        }
        if matches!(what, RecordWhat::Iq | RecordWhat::Both) {
            self.record_command("iq.start", &args)?;
        }
        Ok(())
    }
}

/// Passes recording `f` to the thread of session `s`, with the stack and the frequency it names.
fn give(s: &Session, mut f: Feed) {
    let (stack, tuned) = {
        let c = s.owner();
        let stack = c.stack.map_or("", |d| d.id);
        (stack, c.values().map_or(0.0, |v| v.get(&TUNED)))
    };
    match &mut f {
        Feed::Audio(a) => a.set_tuning(stack, tuned),
        Feed::Iq(i) => i.set_stack(stack),
        Feed::Ts(_) | Feed::TimeShift(_) | Feed::Rewind(_) => {}
    }
    s.link.record(f, stack);
}
