//! The end of a pass. A strip decoder that places no line for [`PASS_GAP`] seconds of signal starts a new picture and sends
//! [`PassEnd`]. [`PassPictures`] then finishes the pictures of the pass, and the next strip starts new pictures with new
//! numbers and a name of their own. A shorter dropout stays in the picture.

use sdrkit_codec_image::component::{Layout, Pictures, PicturesCommand};
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::command::{CommandError, CommandId, CommandSet as _, Commands};
use sdrkit_core::media::Picture;
use sdrkit_core::publish::field;
use sdrkit_core::settings::SchemaError;
use sdrkit_core::time::Clock;
use sdrkit_core::{
    ConfigError, Record, Results, Setting, SettingError, SettingId, StateBlob, StateError, Value,
    Values,
};
use sdrkit_graph::{
    no_values, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Latency, Port,
    RunContext, Timed,
};
use std::collections::VecDeque;

/// The seconds of signal without a line after which a pass ends.
pub const PASS_GAP: Setting<field::F64> = Setting::new("pass_gap", "Pass end", "s", 60.0)
    .range(1.0, 3600.0, 1.0)
    .help(
        "After this many seconds of signal without a picture line the pass ends: its picture is finished, and the next \
         line starts a new picture. A shorter dropout stays in the picture.",
    )
    .advanced();

sdrkit_core::settings! {
    /// The setting of a strip decoder that ends a pass.
    pub enum PassSetting {
        /// The seconds of signal without a line after which a pass ends.
        PassGap = PASS_GAP,
    }
}

/// The end of a pass, from a strip decoder to its [`PassPictures`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PassEnd;

impl Item for PassEnd {
    const KIND: &'static str = "satimg.pass_end";
}

/// The output port of [`PassEnd`] items of a strip decoder.
pub fn pass_port() -> Port {
    super::link::item_port::<PassEnd>("pass")
}

/// The format of the [`pass_port`] output.
pub fn pass_format() -> Format {
    super::link::item_format::<PassEnd>()
}

/// The pass clock of a strip decoder: the signal time since its last line.
#[derive(Clone, Debug, PartialEq)]
pub struct PassTimer {
    gap_s: f64,
    quiet_s: f64,
    open: bool,
}

impl Default for PassTimer {
    fn default() -> Self {
        PassTimer {
            gap_s: PASS_GAP.default,
            quiet_s: 0.0,
            open: false,
        }
    }
}

impl PassTimer {
    /// Counts one graph pass of `seconds` of signal, in which the decoder placed a line when `lines` is true. It returns
    /// true once, when a pass with lines has had no line for the gap: the decoder then starts a new picture and sends
    /// [`PassEnd`] with [`PassTimer::send`].
    pub fn step(&mut self, seconds: f64, lines: bool) -> bool {
        if lines {
            self.open = true;
            self.quiet_s = 0.0;
            return false;
        }
        if !self.open {
            return false;
        }
        self.quiet_s += seconds;
        if self.quiet_s < self.gap_s {
            return false;
        }
        self.open = false;
        self.quiet_s = 0.0;
        true
    }

    /// [`PassTimer::step`] for the graph pass of `io`. At the end of a pass it also sends [`PassEnd`] on output `o`, and the
    /// caller starts a new picture.
    pub fn after(&mut self, io: &mut Io, lines: bool, o: usize) -> bool {
        let end = self.step(io.pass_seconds(), lines);
        if end {
            Self::send(io, o);
        }
        end
    }

    /// Sends [`PassEnd`] on output `o`.
    pub fn send(io: &mut Io, o: usize) {
        io.items_out::<PassEnd>(o).push(Timed {
            time: 0.0,
            value: PassEnd,
        });
    }

    /// The gap, in seconds of signal.
    pub fn gap_s(&self) -> f64 {
        self.gap_s
    }

    /// Applies [`PASS_GAP`] with id `id`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] for a value of another kind.
    pub fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.gap_s = PASS_GAP.read(id, v)?;
        Ok(Applied::Live)
    }
}

/// The pictures component of a stack whose pictures come in passes: [`Pictures`], whose pictures end at a [`PassEnd`]. Each
/// strip input (`in`, `in2` ...) has an input of [`PassEnd`] items (`pass`, `pass2` ...).
/// The name of each picture is the name that [`Pictures`] gives, after `source` and before the number of its pass, and
/// the UTC minute of its first strip when the host's clock gives the time of the signal.
pub struct PassPictures {
    inner: Pictures,
    strips: usize,
    source: &'static str,
    wall: bool,
    clock: Clock,
    /// A strip of the pass in progress arrived.
    open: bool,
    passes: u32,
    label: String,
    /// The pass labels of the newest picture numbers.
    labels: VecDeque<(u32, String)>,
}

/// The picture numbers whose pass label is kept.
const LABELS: usize = 64;

impl PassPictures {
    /// A component for these instrument layouts, with one strip input, whose names begin with `source` (a satellite or
    /// a satellite family).
    ///
    /// # Errors
    ///
    /// The errors of [`Pictures::new`].
    pub fn new(layouts: Vec<Layout>, source: &'static str) -> Result<PassPictures, SchemaError> {
        Ok(PassPictures {
            inner: Pictures::new(layouts)?,
            strips: 1,
            source,
            wall: false,
            clock: Clock::fixed(0.0),
            open: false,
            passes: 0,
            label: String::new(),
            labels: VecDeque::new(),
        })
    }

    /// The same component with `n` strip inputs ([`Pictures::with_inputs`]) and as many inputs of [`PassEnd`].
    pub fn with_inputs(mut self, n: usize) -> PassPictures {
        self.inner = self.inner.with_inputs(n);
        self.strips = n.max(1);
        self
    }

    /// The component inside.
    pub fn pictures(&self) -> &Pictures {
        &self.inner
    }

    /// Starts the label of a new pass.
    fn start_pass(&mut self) {
        self.open = true;
        self.passes += 1;
        self.label = if self.wall {
            format!("pass {}, {}", self.passes, utc_label(self.clock.now()))
        } else {
            format!("pass {}", self.passes)
        };
    }

    /// Ends the pictures of the pass in progress: [`Pictures`] sends their last state, finished, at the next tick.
    fn end_pass(&mut self) {
        self.open = false;
        // The clear command of `Pictures` takes no arguments and does not fail.
        let _ = self.inner.invoke(PicturesCommand::Clear.id(), &no_values());
    }

    /// Gives the pictures from item `from` of the picture output their names.
    fn name(&mut self, io: &mut Io, from: usize) {
        for p in io.items_out::<Picture>(0).iter_mut().skip(from) {
            let p = &mut p.value;
            let label = match self.labels.iter().find(|(id, _)| *id == p.id) {
                Some((_, l)) => l.clone(),
                None => {
                    if self.labels.len() == LABELS {
                        self.labels.pop_front();
                    }
                    self.labels.push_back((p.id, self.label.clone()));
                    self.label.clone()
                }
            };
            let base = p.name.as_deref().unwrap_or_default();
            p.name = Some(format!("{} {base}, {label}", self.source));
        }
    }

    /// One pass of the graph: the end of a pass, the start of the next, then [`Pictures`]. `last` is the end of the input.
    fn run(&mut self, io: &mut Io, last: bool) {
        let ended = (self.strips..2 * self.strips).any(|i| !io.items::<PassEnd>(i).is_empty());
        let arrived = (0..self.strips).any(|i| !io.items::<Strip>(i).is_empty());
        if ended && self.open {
            self.end_pass();
        }
        if arrived && !self.open {
            self.start_pass();
        }
        let from = io.items_out::<Picture>(0).len();
        if last {
            self.inner.finish(io);
        } else {
            self.inner.process(io);
        }
        self.name(io, from);
    }
}

/// The UTC minute of Unix time `t`: `2026-11-23 10:15 UTC`.
fn utc_label(t: f64) -> String {
    match sdrkit_util_geo::time::Civil::from_unix_f64(t) {
        Ok(c) => format!(
            "{:04}-{:02}-{:02} {:02}:{:02} UTC",
            c.year, c.month, c.day, c.hour, c.minute
        ),
        Err(_) => String::from("time unknown"),
    }
}

impl Component for PassPictures {
    fn info(&self) -> ComponentInfo {
        let mut info = self.inner.info();
        info.name = "satimg.pass_pictures".into();
        for i in 0..self.strips {
            let name = match i {
                0 => String::from("pass"),
                _ => format!("pass{}", i + 1),
            };
            info.inputs.push(
                Port::typed::<PassEnd>(&name)
                    .optional("the pictures end only at the end of the input"),
            );
        }
        info
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.inner
            .configure(&inputs[..inputs.len().min(self.strips)])
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io, false);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io, true);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.inner.apply(id, v)
    }
    fn values(&self) -> Values {
        self.inner.values()
    }
    fn latency(&self) -> Latency {
        self.inner.latency()
    }
    fn results(&self) -> Option<&Results> {
        self.inner.results()
    }
    fn commands(&self) -> &Commands {
        self.inner.commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        // A new picture by command also starts a new pass.
        self.open = false;
        self.inner.invoke(id, args)
    }
    fn forget(&mut self, kept: &Kept) {
        self.inner.forget(kept);
    }
    fn retuned(&mut self, centre_hz: f64, tuned_hz: f64) {
        self.inner.retuned(centre_hz, tuned_hz);
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.wall = ctx.wall_clock();
        self.clock = ctx.clock.clone();
        self.inner.attach(ctx);
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.inner.save_state()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.open = false;
        self.inner.restore_state(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pass_ends_once_after_the_gap_and_not_before() {
        let mut t = PassTimer::default();
        assert!(!t.step(100.0, false), "no pass before the first line");
        assert!(!t.step(0.1, true));
        assert!(!t.step(59.0, false));
        assert!(!t.step(0.5, true), "a short dropout keeps the pass");
        assert!(!t.step(59.9, false));
        assert!(t.step(0.2, false));
        assert!(!t.step(100.0, false), "the end comes once");
    }

    #[test]
    fn a_utc_label_has_the_minute() {
        assert_eq!(utc_label(1_780_272_000.0 + 3_725.0), "2026-06-01 01:02 UTC");
    }
}
