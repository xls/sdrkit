//! The automatic `atv` receiver: the AM broadcast graph and the FM video graph, of which one decodes at a time. While the
//! one that decodes has no lock, the [`ModulationProbe`] looks at the input; when two readings in a row name the other
//! modulation, that graph takes over. Noise names neither, so the receiver stays where it is.

use super::graph::receiver_of;
use super::probe::ModulationProbe;
use super::Modulation;
use sdrkit_core::owner::Owner;
use sdrkit_core::{
    CommandError, CommandId, Commands, Complex32, ConfigError, Facilities, InputInfo, LockState,
    Output, Receiver, Record, Results, ResultsDecl, Schema, SettingError, SettingId, Staged,
    StateBlob, StateError, Station, Value, Values,
};
use sdrkit_graph::{GraphError, StackGraph};
use std::sync::Arc;

/// Readings in a row that name the other modulation before it takes over.
const VOTES: u32 = 2;

/// The automatic receiver.
pub struct AutoReceiver {
    am: StackGraph,
    fm: StackGraph,
    which: Modulation,
    probe: ModulationProbe,
    votes: u32,
    input: Option<InputInfo>,
    results: Results,
    mirrored: Arc<ResultsDecl>,
    note: Option<&'static str>,
}

impl AutoReceiver {
    /// The receiver with the settings of `cfg`.
    ///
    /// # Errors
    ///
    /// [`GraphError`] if a graph does not build or `cfg` holds a value that a component refuses.
    pub fn new(cfg: &sdrkit_core::StackConfig) -> Result<Self, GraphError> {
        let am = receiver_of(cfg, Modulation::Am)?;
        let fm = receiver_of(cfg, Modulation::FmVideo)?;
        let mirrored = Arc::clone(Owner::results(&am).decl());
        let mut rx = AutoReceiver {
            am,
            fm,
            which: Modulation::Am,
            probe: ModulationProbe::new(1.0),
            votes: 0,
            input: None,
            results: Results::new(Arc::clone(&mirrored)),
            mirrored,
            note: None,
        };
        rx.mirror();
        Ok(rx)
    }

    /// The modulation of the graph that decodes now.
    pub fn modulation(&self) -> Modulation {
        self.which
    }

    fn active(&mut self) -> &mut StackGraph {
        match self.which {
            Modulation::FmVideo => &mut self.fm,
            _ => &mut self.am,
        }
    }

    fn active_ref(&self) -> &StackGraph {
        match self.which {
            Modulation::FmVideo => &self.fm,
            _ => &self.am,
        }
    }

    fn locked(&self) -> bool {
        Owner::results(self.active_ref()).status().lock_state == LockState::Locked
    }

    /// Ends the placement of the stages of the graph that decodes (its finished output goes to `out`), and starts the
    /// graph of `to` on the input.
    fn switch(&mut self, to: Modulation, out: &mut Output) {
        if let Some(st) = self.active().staged() {
            st.drain(out);
            drop(st.cut(0));
        }
        self.which = to;
        self.votes = 0;
        if let Some(i) = self.input {
            // A graph that refuses the input shows why in its own results.
            let _ = self.active().configure(&i);
        }
        self.note = Some(match to {
            Modulation::FmVideo => "the signal is FM video: decoding it as FM video",
            _ => "the signal is AM television: decoding it as AM",
        });
    }

    /// Brings the results of the graph that decodes into this receiver's results.
    fn mirror(&mut self) {
        let src = Owner::results(match self.which {
            Modulation::FmVideo => &self.fm,
            _ => &self.am,
        });
        // Both graphs declare the same results: a new declaration only comes with a change of a graph's own.
        if !Arc::ptr_eq(src.decl(), &self.mirrored) {
            if **src.decl() != *self.mirrored {
                self.results = Results::new(Arc::clone(src.decl()));
            }
            self.mirrored = Arc::clone(src.decl());
        }
        self.results.copy_from(src);
        if let Some(n) = self.note {
            if !self.results.log_lines().iter().any(|l| l == n) {
                self.results.log(n);
            }
        }
    }
}

impl Owner for AutoReceiver {
    fn schema(&self) -> &Arc<Schema> {
        Owner::schema(&self.am)
    }

    fn values(&self) -> Option<&Values> {
        Owner::values(self.active_ref())
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let r = match self.which {
            Modulation::FmVideo => {
                let r = Owner::apply(&mut self.fm, id, value);
                // The graph that waits takes what it can; it is set again when it decodes.
                let _ = Owner::apply(&mut self.am, id, value);
                r
            }
            _ => {
                let r = Owner::apply(&mut self.am, id, value);
                let _ = Owner::apply(&mut self.fm, id, value);
                r
            }
        };
        self.mirror();
        r
    }

    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.decl()
    }

    fn results(&self) -> &Results {
        &self.results
    }

    fn commands(&self) -> &Commands {
        Owner::commands(self.active_ref())
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let r = Owner::invoke(self.active(), id, args);
        self.mirror();
        r
    }

    fn problem(&mut self, text: &str) {
        Owner::problem(self.active(), text);
        self.mirror();
    }
}

impl Receiver for AutoReceiver {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        self.input = Some(*input);
        self.probe = ModulationProbe::new(input.rate_hz);
        self.votes = 0;
        let r = self.active().configure(input);
        self.mirror();
        r
    }

    fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
        if self.input.is_some() && !self.locked() {
            if let Some(c) = self.probe.feed(iq) {
                match c.modulation() {
                    Some(m) if m != self.which => {
                        self.votes += 1;
                        if self.votes >= VOTES {
                            self.switch(m, out);
                        }
                    }
                    _ => self.votes = 0,
                }
            }
        }
        self.active().feed(iq, out);
        self.mirror();
    }

    fn finish(&mut self, out: &mut Output) {
        self.active().finish(out);
        self.mirror();
    }

    fn retuned(&mut self, input: &InputInfo) {
        self.input = Some(*input);
        self.probe.reset();
        self.votes = 0;
        self.active().retuned(input);
        self.mirror();
    }

    fn forget(&mut self) {
        self.am.forget();
        self.fm.forget();
        self.mirror();
    }

    fn set_station(&mut self, station: &Station) {
        self.am.set_station(station);
        self.fm.set_station(station);
    }

    fn transmits(&self) -> bool {
        self.am.transmits()
    }

    fn set_transmit(&mut self, on: bool) {
        self.am.set_transmit(on);
        self.fm.set_transmit(on);
    }

    fn set_facilities(&mut self, facilities: &Facilities) {
        self.am.set_facilities(facilities);
        self.fm.set_facilities(facilities);
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.active_ref().save_state()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let r = self.active().restore_state(state);
        self.mirror();
        r
    }

    fn discontinuity(&mut self, lost: u64) {
        self.active().discontinuity(lost);
    }

    /// The stages of the graph that decodes. A change of graph ends the placement of the one that stops.
    fn staged(&mut self) -> Option<&mut dyn Staged> {
        self.active().staged()
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        self.active().as_any_mut()
    }
}
