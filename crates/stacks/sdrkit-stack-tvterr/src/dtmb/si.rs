//! `dtmb.si`: the system information. Per frame and carrier form it correlates the 36 equalised system-information symbols with
//! the 22 words, accumulates the scores over frames and accepts an index that leads clearly for three frames in a row. It then
//! tells `dtmb.body` which carrier form to equalise, tells `dtmb.track` whether the system information is known, and hands the
//! accepted form's frames to `dtmb.fec`.
//!
//! Standard: GB 20600-2006 clause 4.6.

use super::body::{BodyFrame, BodySetting, Equalised, Forms};
use super::pn::Header;
use super::profile::{si_chips, Carriers, Profile, SI, SI_INDICES};
use super::track::TrackSetting;
use sdrkit_core::{Choice, ConfigError, SettingSet, Value, Values};
use sdrkit_graph::{
    no_values, Command, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};

/// Words in the table.
const WORDS: usize = 22;
/// Frames in a row an index must lead before it is accepted.
const ACCEPT: u32 = 3;
/// Frames in a row another index or form must lead before it replaces an accepted one.
const REPLACE: u32 = 40;
/// Frames in a row with a weak score after which the system information counts as lost.
const LOST: u32 = 400;

/// A frame for `dtmb.fec`.
#[derive(Clone, Debug, Default)]
pub struct DataFrame {
    /// The frame's number in its grid.
    pub index: u64,
    /// The accepted profile; `None` removes the decoder.
    pub profile: Option<Profile>,
    /// The carrier form.
    pub carriers: Option<Carriers>,
    /// The header.
    pub header: Option<Header>,
    /// A new decoder starts with this frame.
    pub fresh: bool,
    /// Frames lost before this one, to be erased too.
    pub lost: u32,
    /// The equalised body; `None` for an erased frame.
    pub body: Option<Equalised>,
}

impl Item for DataFrame {
    const KIND: &'static str = "dtmb.data_frame";
}

/// What `dtmb.si` reports after each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SiState {
    /// The accepted index (3 to 24).
    pub index: Option<usize>,
    /// The accepted form.
    pub carriers: Option<Carriers>,
    /// The normalised score of the leading index (1 = perfect).
    pub corr: f64,
    /// Log lines since the last state.
    pub log: Vec<String>,
}

impl Item for SiState {
    const KIND: &'static str = "dtmb.si_state";
}

/// The scores of one carrier form.
#[derive(Clone, Debug, Default)]
struct Hyp {
    acc: [f32; WORDS],
    n: u32,
    leader: Option<usize>,
    run: u32,
    s: f32,
}

impl Hyp {
    /// Adds a frame: the weighted correlation of the 36 symbols with every word, accumulated 0.7 / 0.3.
    fn update(&mut self, words: &[[f32; SI]; WORDS], body: &Equalised) {
        let mut u = [0f32; SI];
        let mut w = [0f32; SI];
        for i in 0..SI {
            let s = body.cells[i];
            u[i] = (s.re + s.im) * std::f32::consts::FRAC_1_SQRT_2;
            w[i] = 1.0 / body.var[i].max(1e-6);
        }
        let ws: f32 = w.iter().sum();
        for (acc, c) in self.acc.iter_mut().zip(words) {
            let score: f32 = (0..SI).map(|i| w[i] * u[i] * c[i]).sum::<f32>() / ws;
            *acc = 0.7 * *acc + 0.3 * score;
        }
        self.n += 1;
        let norm = (1.0 - 0.7f32.powi(self.n.min(30) as i32)).max(0.3);
        let (mut best, mut second) = (0usize, f32::MIN);
        for i in 1..WORDS {
            if self.acc[i] > self.acc[best] {
                best = i;
            }
        }
        for (i, &a) in self.acc.iter().enumerate() {
            if i != best {
                second = second.max(a);
            }
        }
        self.s = self.acc[best] / norm;
        let margin = (self.acc[best] - second) / norm;
        if self.s > 0.45 && margin > 0.35 {
            if self.leader == Some(best) {
                self.run += 1;
            } else {
                self.leader = Some(best);
                self.run = 1;
            }
        } else {
            self.leader = None;
            self.run = 0;
        }
    }
}

/// `dtmb.si`. Input `in`: [`BodyFrame`] from `dtmb.body`. Outputs: `out` ([`DataFrame`] for `dtmb.fec`, once the system
/// information is accepted), `control` (`forms` for `dtmb.body`), `state` ([`SiState`] after each frame), `track` (`si_known`
/// for `dtmb.track`, when it changes).
pub struct Si {
    words: [[f32; SI]; WORDS],
    grid: u32,
    hyps: [Hyp; 2],
    accepted: Option<(usize, Carriers)>,
    weak: u32,
    /// What `dtmb.track` was told last.
    told: Option<bool>,
    state: SiState,
}

impl Default for Si {
    fn default() -> Self {
        let mut words = [[0f32; SI]; WORDS];
        for (w, x) in words.iter_mut().zip(SI_INDICES) {
            *w = si_chips(x).unwrap_or([0.0; SI]);
        }
        Si {
            words,
            grid: u32::MAX,
            hyps: Default::default(),
            accepted: None,
            weak: 0,
            told: None,
            state: SiState::default(),
        }
    }
}

impl Si {
    fn reset(&mut self) {
        self.hyps = Default::default();
        self.accepted = None;
        self.weak = 0;
        self.state.index = None;
        self.state.carriers = None;
        self.state.corr = 0.0;
    }

    fn forms_command(forms: Forms) -> Command {
        Command::new(BodySetting::Forms.id(), Value::Choice(forms.index()))
    }

    /// One frame; returns the frame for the decoder and a command for `dtmb.body`.
    fn frame(&mut self, f: &BodyFrame) -> (Option<DataFrame>, Option<Command>) {
        let mut cmd = None;
        let mut out = None;
        if f.grid != self.grid {
            self.grid = f.grid;
            let had = self.accepted.is_some();
            self.reset();
            cmd = Some(Self::forms_command(Forms::Both));
            if had {
                out = Some(DataFrame::default());
            }
        }
        let forms = [(Carriers::Multi, &f.multi), (Carriers::Single, &f.single)];
        for (h, (_, body)) in self.hyps.iter_mut().zip(forms.iter()) {
            if let Some(b) = body {
                h.update(&self.words, b);
            }
        }
        let lead = |h: &Hyp, run: u32| h.leader.filter(|_| h.run >= run).map(|i| (i, h.s));
        let pick = |a: Option<(usize, f32)>, b: Option<(usize, f32)>| match (a, b) {
            (Some(x), Some(y)) if y.1 > x.1 => Some((y.0, Carriers::Single)),
            (Some(x), _) => Some((x.0, Carriers::Multi)),
            (None, Some(y)) => Some((y.0, Carriers::Single)),
            _ => None,
        };
        let mut fresh = false;
        match self.accepted {
            None => {
                if let Some((i, c)) = pick(lead(&self.hyps[0], ACCEPT), lead(&self.hyps[1], ACCEPT))
                {
                    self.accept(i, c, f.header);
                    fresh = true;
                    if f.header == Some(Header::Pn595) {
                        cmd = Some(Self::forms_command(match c {
                            Carriers::Multi => Forms::Multi,
                            Carriers::Single => Forms::Single,
                        }));
                    }
                }
            }
            Some((idx, form)) => {
                let k = usize::from(form == Carriers::Single);
                let h = &self.hyps[k];
                if let Some((i, _)) = lead(h, REPLACE) {
                    if i + *SI_INDICES.start() != idx {
                        self.accept(i, form, f.header);
                        fresh = true;
                    }
                }
                if f.multi.is_some() || f.single.is_some() {
                    self.weak = if self.hyps[k].s < 0.25 {
                        self.weak + 1
                    } else {
                        0
                    };
                }
                if self.weak >= LOST {
                    self.state.log.push("DTMB: system information lost".into());
                    self.reset();
                    return (
                        Some(DataFrame::default()),
                        Some(Self::forms_command(Forms::Both)),
                    );
                }
            }
        }
        self.state.corr = self.hyps.iter().map(|h| h.s).fold(0f32, f32::max) as f64;
        if let Some((idx, form)) = self.accepted {
            let body = match form {
                Carriers::Multi => &f.multi,
                Carriers::Single => &f.single,
            };
            out = Some(DataFrame {
                index: f.index,
                profile: Profile::from_index(idx),
                carriers: Some(form),
                header: f.header,
                fresh,
                lost: f.lost,
                body: if f.erase { None } else { body.clone() },
            });
        }
        (out, cmd)
    }

    fn accept(&mut self, word: usize, form: Carriers, header: Option<Header>) {
        let idx = word + *SI_INDICES.start();
        self.accepted = Some((idx, form));
        self.weak = 0;
        self.state.index = Some(idx);
        self.state.carriers = Some(form);
        if let Some(p) = Profile::from_index(idx) {
            let carriers = match form {
                Carriers::Multi => "multi-carrier",
                Carriers::Single => "single carrier",
            };
            let name = header.map(|h| h.name()).unwrap_or("");
            self.state.log.push(format!(
                "DTMB: system information: {} (index {idx}), {carriers}, {name}",
                p.describe()
            ));
        }
    }
}

impl Component for Si {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dtmb.si",
            vec![Port::typed::<BodyFrame>("in")],
            vec![
                Port::typed::<DataFrame>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::command("control").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<SiState>("state").rate_from(RateFrom::Fixed(0.0)),
                Port::command("track").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.grid = u32::MAX;
        self.told = None;
        self.reset();
        Ok(vec![
            Format::new(Kind::typed::<DataFrame>(), 0.0),
            Format::new(Kind::typed::<Command>(), 0.0),
            Format::new(Kind::typed::<SiState>(), 0.0),
            Format::new(Kind::typed::<Command>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.items::<BodyFrame>(0).len();
        for i in 0..n {
            let (time, f) = {
                let it = &io.items::<BodyFrame>(0)[i];
                (it.time, it.value.clone())
            };
            let (out, cmd) = self.frame(&f);
            if let Some(value) = out {
                io.items_out::<DataFrame>(0).push(Timed { time, value });
            }
            if let Some(value) = cmd {
                io.items_out::<Command>(1).push(Timed { time, value });
            }
            let known = self.accepted.is_some();
            if self.told != Some(known) {
                self.told = Some(known);
                let value = Command::new(TrackSetting::SiKnown.id(), Value::Bool(known));
                io.items_out::<Command>(3).push(Timed { time, value });
            }
            let value = self.state.clone();
            self.state.log.clear();
            io.items_out::<SiState>(2).push(Timed { time, value });
        }
    }
}
