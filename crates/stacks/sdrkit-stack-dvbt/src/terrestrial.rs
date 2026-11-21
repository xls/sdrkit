//! DVB-T2 and DVB-T on one frequency: the `dvb_terrestrial` stack tries a DVB-T2 receiver and the DVB-T receiver in turn
//! until one locks. The DVB-T2 receiver comes from its own crate through [`set_t2_factory`]. The stages of the stack are
//! those of the standard that decodes now: the host cuts and runs them as for that stack alone.

use crate::receiver::receiver_with;
use crate::results::Decoding;
use crate::settings::TerrestrialSetting;
use crate::typed::DvbTerrestrialFields;
use sdrkit_core::interface::Fields;
use sdrkit_core::owner::Owner;
use sdrkit_core::{
    CommandError, CommandId, Commands, Complex32, ConfigError, InputInfo, LockState, Output,
    Receiver, Record, Results, Schema, SettingError, SettingId, SettingSet, StackConfig, Staged,
    Station, Value, Values,
};
use sdrkit_graph::StackGraph;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "dvb_terrestrial";

/// Makes a DVB-T2 receiver.
pub type T2Factory = fn(&StackConfig) -> Result<Box<dyn Receiver>, ConfigError>;

static T2: OnceLock<T2Factory> = OnceLock::new();

/// Registers the DVB-T2 receiver that the stack alternates with. Returns false if one is registered already.
pub fn set_t2_factory(f: T2Factory) -> bool {
    T2.set(f).is_ok()
}

/// The slice of signal given to DVB-T2, s.
pub const T2_SLICE_S: f64 = 1.6;
/// The slice of signal given to DVB-T, s.
pub const T_SLICE_S: f64 = 1.3;

/// The DVB-T2 / DVB-T receiver.
pub struct TerrestrialReceiver {
    t: StackGraph,
    t2: Option<Box<dyn Receiver>>,
    which: Decoding,
    slice_left: f64,
    locked: Option<Decoding>,
    input: Option<InputInfo>,
    schema: Arc<Schema>,
    values: Values,
    results: Results,
    /// The declaration of the DVB-T results that `results` extends.
    mirrored: Arc<sdrkit_core::ResultsDecl>,
    /// The log lines of this stack, kept beside those of the receiver that decodes.
    notes: Vec<&'static str>,
}

fn slice(w: Decoding) -> f64 {
    match w {
        Decoding::Dvbt2 => T2_SLICE_S,
        Decoding::Dvbt => T_SLICE_S,
    }
}

impl TerrestrialReceiver {
    /// The receiver with the settings of `cfg`.
    ///
    /// # Errors
    ///
    /// As [`receiver_with`], and [`ConfigError`] if the DVB-T2 receiver refuses `cfg`.
    pub fn new(cfg: &StackConfig) -> Result<Self, ConfigError> {
        let schema = TerrestrialSetting::schema();
        let t = receiver_with(crate::typed::terrestrial_receiver(), cfg)?;
        let t2 = T2.get().map(|f| f(cfg)).transpose()?;
        let mirrored = Arc::clone(Owner::results(&t).decl());
        let results = Results::new(Arc::new(
            mirrored
                .as_ref()
                .clone()
                .with_fields(DvbTerrestrialFields::SHAPE.fields),
        ));
        if let Some(what) =
            crate::typed::shape(ID).difference(&schema, results.decl(), Owner::commands(&t))
        {
            return Err(sdrkit_graph::GraphError::Shape { stack: ID, what }.into());
        }
        let mut notes = Vec::new();
        if t2.is_none() {
            notes.push("no DVB-T2 stack in this build: DVB-T only");
        }
        let values = Values::defaults(&schema);
        let mut rx = TerrestrialReceiver {
            t,
            which: if t2.is_some() {
                Decoding::Dvbt2
            } else {
                Decoding::Dvbt
            },
            t2,
            slice_left: T2_SLICE_S,
            locked: None,
            input: None,
            schema,
            values,
            results,
            mirrored,
            notes,
        };
        for (i, _) in rx.schema.settings().iter().enumerate() {
            let id = SettingId(i as u16);
            if let Some(v) = cfg.settings.value(id) {
                // The values of `cfg` passed the check of this schema when the DVB-T receiver accepted them.
                let _ = rx.values.set(id, v);
            }
        }
        rx.mirror();
        Ok(rx)
    }

    /// The standard that decodes now.
    pub fn decoding(&self) -> Decoding {
        self.which
    }

    /// Whether one of the standards has locked.
    pub fn locked(&self) -> bool {
        self.locked.is_some()
    }

    fn is_locked(&self, w: Decoding) -> bool {
        match w {
            Decoding::Dvbt => Owner::results(&self.t).status().lock_state == LockState::Locked,
            Decoding::Dvbt2 => self
                .t2
                .as_ref()
                .is_some_and(|r| r.results().status().lock_state == LockState::Locked),
        }
    }

    /// The receive chain of the standard that decodes now.
    fn active(&mut self) -> &mut dyn Receiver {
        match (self.which, self.t2.as_mut()) {
            (Decoding::Dvbt2, Some(r)) => r.as_mut(),
            _ => &mut self.t,
        }
    }

    /// Takes the decision at the end of a slice. With DVB-T2 present the stages of the standard that decodes come back to
    /// this thread and its results are merged first, so the decision falls on the same sample for every placement of its
    /// stages. A standard that is not locked at the end of its slice gives way; one that lost its lock keeps one more slice.
    fn decide(&mut self, out: &mut Output) {
        if self.t2.is_some() {
            let rx = self.active();
            if let Some(st) = rx.staged() {
                st.gather();
            }
            if let Some(g) = rx
                .as_any_mut()
                .and_then(|a| a.downcast_mut::<sdrkit_graph::Graph>())
            {
                g.refresh_results();
            }
        }
        self.slice_left = slice(self.which);
        if self.is_locked(self.which) {
            self.locked = Some(self.which);
        } else if self.locked.take().is_none() {
            self.switch(Some(out));
        }
    }

    /// Ends the placement of the stages of the standard that decodes: its stage threads return, and the output they
    /// finished goes to `out`, or is dropped without one.
    fn settle(&mut self, out: Option<&mut Output>) {
        if let Some(st) = self.active().staged() {
            match out {
                Some(out) => st.drain(out),
                None => st.drain(&mut Output::default()),
            }
            drop(st.cut(0));
        }
    }

    fn switch(&mut self, out: Option<&mut Output>) {
        if self.t2.is_none() {
            return;
        }
        self.settle(out);
        self.which = match self.which {
            Decoding::Dvbt2 => Decoding::Dvbt,
            Decoding::Dvbt => Decoding::Dvbt2,
        };
        self.slice_left = slice(self.which);
        self.log_trying();
        if let Some(i) = self.input {
            // A receiver that refuses the input shows why in its own results.
            let _ = match self.which {
                Decoding::Dvbt => self.t.configure(&i),
                Decoding::Dvbt2 => self.t2.as_mut().map_or(Ok(()), |r| r.configure(&i)),
            };
        }
    }

    fn log_trying(&mut self) {
        let line = match self.which {
            Decoding::Dvbt2 => "trying DVB-T2",
            Decoding::Dvbt => "trying DVB-T",
        };
        self.notes.retain(|n| !n.starts_with("trying "));
        self.notes.push(line);
    }

    /// Brings the results of the receiver that decodes now into this stack's results. The DVB-T results copy whole. Of the
    /// DVB-T2 receiver, whose results have another declaration, only the status shows.
    fn mirror(&mut self) {
        let src = Owner::results(&self.t);
        if !Arc::ptr_eq(src.decl(), &self.mirrored) {
            self.mirrored = Arc::clone(src.decl());
            let decl = self
                .mirrored
                .as_ref()
                .clone()
                .with_fields(DvbTerrestrialFields::SHAPE.fields);
            self.results = Results::new(Arc::new(decl));
        }
        match self.which {
            Decoding::Dvbt => self.results.copy_from(Owner::results(&self.t)),
            Decoding::Dvbt2 => {
                if let Some(r) = self.t2.as_ref() {
                    let st = r.results().status().clone();
                    self.results.set_status(&st);
                }
            }
        }
        DvbTerrestrialFields {
            standard: self.which,
        }
        .write(&mut self.results.fields_mut());
        for n in &self.notes {
            if !self.results.log_lines().iter().any(|l| l == n) {
                self.results.log(*n);
            }
        }
    }
}

impl Owner for TerrestrialReceiver {
    fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }

    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        Owner::apply(&mut self.t, id, value)?;
        if let (Some(t2), Some(s)) = (self.t2.as_mut(), self.schema.setting(id)) {
            if let Ok(id2) = t2.schema().id(&s.field.key) {
                // A setting that the DVB-T2 receiver refuses leaves it as it was: the setting is for DVB-T.
                let _ = sdrkit_core::owner::apply(t2.as_mut(), id2, value);
            }
        }
        let _ = self.values.set(id, value);
        Ok(())
    }

    fn results_decl(&self) -> &Arc<sdrkit_core::ResultsDecl> {
        self.results.decl()
    }

    fn results(&self) -> &Results {
        &self.results
    }

    fn commands(&self) -> &Commands {
        Owner::commands(&self.t)
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let r = Owner::invoke(&mut self.t, id, args);
        let key = self.t.commands().def(id).map(|d| d.key.clone());
        if let (Some(t2), Some(key)) = (self.t2.as_mut(), key) {
            if let Ok(id2) = t2.commands().id(&key) {
                // The command is for both receivers. The DVB-T reply and error are the ones that count.
                let _ = sdrkit_core::owner::invoke(t2.as_mut(), id2, args.record());
            }
        }
        self.mirror();
        r
    }
}

impl Receiver for TerrestrialReceiver {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        self.input = Some(*input);
        self.locked = None;
        self.slice_left = slice(self.which);
        if let Some(r) = self.t2.as_mut() {
            // A DVB-T2 receiver that refuses the input shows why in its own results.
            let _ = r.configure(input);
        }
        let r = self.t.configure(input);
        self.mirror();
        r
    }

    fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
        let Some(input) = self.input else { return };
        self.active().feed(iq, out);
        self.slice_left -= iq.len() as f64 / input.rate_hz;
        if self.slice_left <= 0.0 {
            self.decide(out);
        }
        self.mirror();
    }

    fn finish(&mut self, out: &mut Output) {
        self.active().finish(out);
        self.mirror();
    }

    fn retuned(&mut self, input: &InputInfo) {
        self.input = Some(*input);
        self.locked = None;
        // A new channel starts the alternation again, DVB-T2 first.
        if self.t2.is_some() && self.which != Decoding::Dvbt2 {
            self.settle(None);
            self.which = Decoding::Dvbt2;
            self.log_trying();
        }
        self.slice_left = slice(self.which);
        self.t.retuned(input);
        if let Some(r) = self.t2.as_mut() {
            r.retuned(input);
        }
        self.mirror();
    }

    fn forget(&mut self) {
        self.t.forget();
        if let Some(r) = self.t2.as_mut() {
            r.forget();
        }
        self.mirror();
    }

    fn set_station(&mut self, station: &Station) {
        self.t.set_station(station);
        if let Some(r) = self.t2.as_mut() {
            r.set_station(station);
        }
    }

    /// The stages of the standard that decodes now. Only its receive chain is ever cut: a hand-over ends the placement
    /// of the one that stops.
    fn staged(&mut self) -> Option<&mut dyn Staged> {
        self.active().staged()
    }
}
