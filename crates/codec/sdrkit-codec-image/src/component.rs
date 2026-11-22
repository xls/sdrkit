//! `image.pictures`: [`Strip`] items in, [`Picture`] items out.
//!
//! It keeps one [`StripPicture`] for each instrument through retunes, rate changes and demodulator resets. At each tick of the
//! graph's clock it draws the chosen view again when the picture changed, at most once every `frame_every` ticks. A change of
//! view or enhancement draws at once. [`Component::save_state`] returns the gathered strips for the host to keep.

use crate::raster::Raster;
use crate::strip::{render, Enhance, Strip, StripPicture, View};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Family, Fields, StackShape};
use sdrkit_core::media::{Picture, PictureIds, PictureState};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::SchemaError;
use sdrkit_core::{Choice, SettingRole};
use sdrkit_core::{
    ChoiceDecl, ConfigError, DeclareSetting, FieldDecl, Record, Results, Schema, Setting,
    SettingDecl, SettingError, SettingId, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// The result set of the picture component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Fields)]
pub struct PictureFields {
    /// The rows of the picture shown.
    #[field(label = "Rows")]
    pub rows: i64,
    /// The strips kept, over all instruments.
    #[field(label = "Strips")]
    pub strips: i64,
    /// The strips refused because their samples did not match their size.
    #[field(label = "Bad strips")]
    pub bad_strips: i64,
}

/// The shared interface of the crate: the result set of the picture component. A family whose stacks show it names
/// this interface with `Family::uses` and lists the set with `.fields::<PictureFields>()`.
pub fn interface() -> &'static Family {
    static SHARED: OnceLock<Family> = OnceLock::new();
    SHARED.get_or_init(|| {
        Family::shared("image", 1, 0).stack(StackShape::new("image").fields::<PictureFields>())
    })
}

const ENHANCE: Setting<field::Choice<Enhance>> =
    Setting::choice("enhance", "Enhancement", Enhance::None);
const FOLLOW: Setting<field::Bool> =
    Setting::new("follow", "Follow", "", true).help("Draw while the picture grows.");
const FRAME_EVERY: Setting<field::I64> = Setting::new("frame_every", "Frame interval", "ticks", 8)
    .range(1.0, 1000.0, 1.0)
    .help("The fewest clock ticks between two drawings of a growing picture.");

/// The settings of [`Pictures`], in the order of its schema. The choice lists of `instrument` and `view` come from the layouts
/// that the component was made with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicturesSetting {
    /// `instrument`: the instrument shown (one choice per layout).
    Instrument,
    /// `view`: the view shown. The choices are the view keys of every layout. A key that the instrument shown does not have
    /// is refused.
    View,
    /// `enhance`: the enhancement ([`Enhance`]).
    Enhance,
    /// `follow`: draw while the picture grows.
    Follow,
    /// `frame_every`: the fewest ticks between two drawings of a growing picture (1 to 1000).
    FrameEvery,
}

impl PicturesSetting {
    const ALL: [PicturesSetting; 5] = [
        PicturesSetting::Instrument,
        PicturesSetting::View,
        PicturesSetting::Enhance,
        PicturesSetting::Follow,
        PicturesSetting::FrameEvery,
    ];

    /// The id of the setting.
    pub fn id(self) -> SettingId {
        SettingId(self as u16)
    }

    /// The setting of an id, `None` outside the set.
    pub fn from_id(id: SettingId) -> Option<PicturesSetting> {
        Self::ALL.get(usize::from(id.0)).copied()
    }
}

const CLEAR: Command =
    Command::new("clear", "New picture").help("Forgets every picture and starts new ones.");

sdrkit_core::commands! {
    /// The commands of [`Pictures`].
    pub enum PicturesCommand {
        /// Starts new pictures.
        Clear = CLEAR,
    }
}

/// A view with its setting key and its label.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedView {
    /// The choice key of the `view` setting.
    pub value: Cow<'static, str>,
    /// What people see.
    pub label: Cow<'static, str>,
    /// What it shows.
    pub view: View,
}

impl NamedView {
    /// A named view.
    pub fn new(
        value: impl Into<Cow<'static, str>>,
        label: impl Into<Cow<'static, str>>,
        view: View,
    ) -> NamedView {
        NamedView {
            value: value.into(),
            label: label.into(),
            view,
        }
    }
}

/// The picture layout of one instrument.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The instrument name the strips carry ([`Strip::instrument`]). An empty name takes strips of every instrument.
    pub name: &'static str,
    /// The picture width. 0 takes the width of the first strip (its `x` plus its `width`).
    pub width: u32,
    /// The most rows kept.
    pub max_rows: u32,
    /// The mask of a whole strip (all its parts arrived).
    pub full_mask: u64,
    /// Its views. The first is the default. At least one.
    pub views: Vec<NamedView>,
}

impl Layout {
    /// A layout for any instrument: the width of the first strip, at most `max_rows` rows, and the views `auto` (the lowest
    /// channel in grey) and `rgb123` (channels 1, 2 and 3 as red, green and blue).
    pub fn any(max_rows: u32) -> Layout {
        Layout {
            name: "",
            width: 0,
            max_rows,
            full_mask: 1,
            views: vec![
                NamedView::new("auto", "Automatic", View::FirstChannel),
                NamedView::new(
                    "rgb123",
                    "Channels 1, 2, 3",
                    View::Rgb([(1, false), (2, false), (3, false)]),
                ),
            ],
        }
    }
}

/// The pictures component: one picture for each instrument, drawn in the chosen view at the ticks of the graph's clock.
pub struct Pictures {
    layouts: Vec<Layout>,
    schema: Arc<Schema>,
    inputs: usize,
    pictures: BTreeMap<&'static str, StripPicture>,
    next_id: u32,
    instrument: usize,
    view: usize,
    enhance: Enhance,
    follow: bool,
    frame_every: u32,
    since_frame: u32,
    // (picture id, version, instrument, view, enhance) of the last picture sent.
    last: Option<(u32, u64, usize, usize, Enhance)>,
    bad_strips: u64,
    published: Published,
    /// The numbers of the pictures sent, by the number of the strip picture and the view.
    ids: PictureIds<(u32, usize)>,
    /// The last states of pictures that ended between two ticks. The next tick sends them.
    ended: Vec<Picture>,
}

impl Default for Pictures {
    fn default() -> Self {
        Pictures::new(vec![Layout::any(20_000)])
            .expect("the layout for any instrument has a valid schema")
    }
}

impl Pictures {
    /// A component for these instrument layouts. An empty list, or a layout without views, takes [`Layout::any`] instead.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] if two layouts have the same instrument name.
    pub fn new(layouts: Vec<Layout>) -> Result<Pictures, SchemaError> {
        let mut layouts: Vec<Layout> = layouts
            .into_iter()
            .filter(|l| !l.views.is_empty())
            .collect();
        if layouts.is_empty() {
            layouts.push(Layout::any(20_000));
        }
        let mut view_keys: Vec<ChoiceDecl> = Vec::new();
        for v in layouts.iter().flat_map(|l| &l.views) {
            if !view_keys.iter().any(|c| c.key == v.value) {
                view_keys.push(ChoiceDecl {
                    key: v.value.clone(),
                    label: v.label.clone(),
                });
            }
        }
        let instruments: Vec<ChoiceDecl> = layouts
            .iter()
            .map(|l| ChoiceDecl {
                key: Cow::Borrowed(l.name),
                label: Cow::Borrowed(if l.name.is_empty() { "Any" } else { l.name }),
            })
            .collect();
        let schema = Arc::new(Schema::new(vec![
            SettingDecl::new(
                FieldDecl::with_choices("instrument", "Instrument", instruments),
                Value::Choice(0),
            )
            .with_role(SettingRole::PictureSource),
            SettingDecl::new(
                FieldDecl::with_choices("view", "View", view_keys),
                Value::Choice(0),
            )
            .with_role(SettingRole::PictureView),
            ENHANCE.decl(),
            FOLLOW.decl(),
            FRAME_EVERY.decl(),
        ])?);
        Ok(Pictures {
            layouts,
            schema,
            inputs: 1,
            pictures: BTreeMap::new(),
            next_id: 1,
            ids: PictureIds::new(),
            ended: Vec::new(),
            instrument: 0,
            view: 0,
            enhance: ENHANCE.default,
            follow: FOLLOW.default,
            frame_every: FRAME_EVERY.default as u32,
            since_frame: 0,
            last: None,
            bad_strips: 0,
            published: Published::new(PictureFields::SHAPE.fields),
        })
    }

    /// The same component with `n` strip inputs (`in`, `in2`, `in3` ...), for a stack with several instrument decoders.
    pub fn with_inputs(mut self, n: usize) -> Pictures {
        self.inputs = n.max(1);
        self
    }

    fn layout(&self) -> &Layout {
        &self.layouts[self.instrument.min(self.layouts.len() - 1)]
    }

    /// The choices of `view`: every view key of every layout, once, in order.
    fn view_keys(&self) -> &[ChoiceDecl] {
        self.schema
            .setting(PicturesSetting::View.id())
            .map_or(&[], |s| &s.field.choices)
    }

    fn named_view(&self) -> &NamedView {
        let l = self.layout();
        &l.views[self.view.min(l.views.len() - 1)]
    }

    /// The picture of the instrument shown, if any strip arrived.
    pub fn current(&self) -> Option<&StripPicture> {
        self.pictures.get(self.layout().name)
    }

    /// The pictures of all instruments, by instrument name.
    pub fn pictures(&self) -> &BTreeMap<&'static str, StripPicture> {
        &self.pictures
    }

    /// Draws the chosen view of the instrument shown, with its name (`<instrument>-<view tag>`). The picture has the
    /// number of that view of that instrument, and it can grow.
    pub fn render_current(&mut self) -> Option<Picture> {
        let l = self.layout();
        let pic = self.pictures.get(l.name)?;
        let view = self.view.min(l.views.len() - 1);
        let (r, name) = Self::draw(l, pic, view, self.enhance)?;
        let id = self.ids.of(&(pic.id, view));
        Some(r.to_picture(&name, id, PictureState::Growing))
    }

    /// The view `view` of the picture `pic` of the layout `l`, and its name.
    fn draw(
        l: &Layout,
        pic: &StripPicture,
        view: usize,
        enhance: Enhance,
    ) -> Option<(Raster, String)> {
        let v = &l.views.get(view)?.view;
        let r = render(pic, v, enhance)?;
        let tag = v.resolve(pic).map_or_else(|| v.tag(), |v| v.tag());
        let name = if l.name.is_empty() {
            tag
        } else {
            format!("{}-{tag}", l.name)
        };
        Some((r, name))
    }

    /// Ends every picture sent: draws each once more as finished, for the next tick. The next picture of a view has a
    /// new number.
    fn end_sent(&mut self) {
        for ((pic_id, view), id) in self.ids.close_all() {
            let found = self.layouts.iter().find_map(|l| {
                let pic = self.pictures.get(l.name).filter(|p| p.id == pic_id)?;
                Self::draw(l, pic, view, self.enhance)
            });
            if let Some((r, name)) = found {
                self.ended
                    .push(r.to_picture(&name, id, PictureState::Finished));
            }
        }
    }

    /// Sends the last states of the pictures that ended.
    fn send_ended(&mut self, io: &mut Io) {
        for p in self.ended.drain(..) {
            io.items_out::<Picture>(0).push(Timed {
                time: 0.0,
                value: p,
            });
        }
    }

    fn add(&mut self, s: &Strip) {
        let Some(l) = self
            .layouts
            .iter()
            .find(|l| l.name == s.instrument || l.name.is_empty())
        else {
            return;
        };
        let (name, max) = (l.name, l.max_rows);
        let width = if l.width > 0 {
            l.width
        } else {
            s.x.saturating_add(s.width)
        };
        let id = self.next_id;
        let pic = self
            .pictures
            .entry(name)
            .or_insert_with(|| StripPicture::new(id, width, max));
        if pic.id == id {
            self.next_id += 1;
        }
        if pic.add(s).is_err() {
            self.bad_strips += 1;
        }
    }

    fn publish(&mut self) {
        let rows = self.current().map_or(0, |p| p.height) as i64;
        let strips = self.pictures.values().map(|p| p.strips).sum::<u64>() as i64;
        let bad = self.bad_strips as i64;
        self.published.put(&PictureFields {
            rows,
            strips,
            bad_strips: bad,
        });
    }

    fn input_name(i: usize) -> String {
        if i == 0 {
            "in".into()
        } else {
            format!("in{}", i + 1)
        }
    }

    fn choice(id: SettingId, v: &Value, len: usize) -> Result<usize, SettingError> {
        match v {
            Value::Choice(i) if usize::from(*i) < len => Ok(usize::from(*i)),
            Value::Choice(i) => Err(SettingError::NotAChoice { id, index: *i }),
            other => Err(match other.kind() {
                Some(got) => SettingError::WrongKind {
                    id,
                    expected: sdrkit_core::ValueKind::Choice,
                    got,
                },
                None => SettingError::Required { id },
            }),
        }
    }
}

impl Component for Pictures {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "image.pictures",
            (0..self.inputs)
                .map(|i| Port::typed::<Strip>(&Self::input_name(i)))
                .collect(),
            vec![Port::typed::<Picture>("picture").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(Arc::clone(&self.schema))
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // The pictures stay through a reconfigure (a retune or a new rate). The next tick draws again.
        self.last = None;
        Ok(vec![Format::new(Kind::typed::<Picture>(), 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        for i in 0..self.inputs.min(io.num_inputs()) {
            for s in io.items::<Strip>(i) {
                self.add(&s.value);
            }
        }
        if !io.tick() {
            return;
        }
        self.send_ended(io);
        self.since_frame = self.since_frame.saturating_add(1);
        let state = self
            .current()
            .map(|p| (p.id, p.version, self.instrument, self.view, self.enhance));
        let changed_view = match (self.last, state) {
            (Some(a), Some(b)) => (a.0, a.2, a.3, a.4) != (b.0, b.2, b.3, b.4),
            (None, Some(_)) => true,
            _ => false,
        };
        let grew = self.follow
            && self.since_frame >= self.frame_every
            && state.map(|s| s.1) != self.last.map(|s| s.1);
        if changed_view || grew {
            if let Some(p) = self.render_current() {
                io.items_out::<Picture>(0).push(Timed {
                    time: 0.0,
                    value: p,
                });
                self.last = state;
                self.since_frame = 0;
            }
        }
        self.publish();
    }

    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        self.end_sent();
        self.send_ended(io);
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PicturesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PicturesSetting::Instrument => {
                self.instrument = Self::choice(id, v, self.layouts.len())?;
                self.view = 0;
            }
            PicturesSetting::View => {
                let keys = self.view_keys();
                let key = &keys[Self::choice(id, v, keys.len())?].key;
                self.view = self
                    .layout()
                    .views
                    .iter()
                    .position(|x| x.value == *key)
                    .ok_or(SettingError::NotPermitted { id })?;
            }
            PicturesSetting::Enhance => self.enhance = ENHANCE.read(id, v)?,
            PicturesSetting::Follow => self.follow = FOLLOW.read(id, v)?,
            PicturesSetting::FrameEvery => self.frame_every = FRAME_EVERY.read(id, v)? as u32,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        let mut v = Values::defaults(&self.schema);
        let view_key = &self.named_view().value;
        let view = self
            .view_keys()
            .iter()
            .position(|c| c.key == *view_key)
            .unwrap_or(0) as u16;
        let present = [
            (
                PicturesSetting::Instrument,
                Value::Choice(self.instrument as u16),
            ),
            (PicturesSetting::View, Value::Choice(view)),
            (
                PicturesSetting::Enhance,
                Value::Choice(self.enhance.index()),
            ),
            (PicturesSetting::Follow, Value::Bool(self.follow)),
            (
                PicturesSetting::FrameEvery,
                Value::I64(i64::from(self.frame_every)),
            ),
        ];
        for (s, value) in present {
            if v.set(s.id(), &value).is_err() {
                sdrkit_core::record::refuse_write(
                    self.schema.settings()[usize::from(s.id().0)]
                        .field
                        .key
                        .as_ref(),
                    "value outside the schema",
                );
            }
        }
        v
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn commands(&self) -> &Commands {
        PicturesCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match PicturesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            PicturesCommand::Clear => {
                self.end_sent();
                self.pictures.clear();
                self.last = None;
                self.publish();
            }
        }
        Ok(None)
    }

    fn save_state(&self) -> Option<StateBlob> {
        if self.pictures.is_empty() {
            return None;
        }
        crate::state::save(&self.pictures)
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let pics = crate::state::load(state)?;
        let mut restored = BTreeMap::new();
        for p in pics {
            let Some(l) = self.layouts.iter().find(|l| l.name == p.0) else {
                continue;
            };
            if l.width > 0 && l.width != p.1.width {
                continue;
            }
            self.next_id = self.next_id.max(p.1.id.saturating_add(1));
            restored.insert(l.name, p.1);
        }
        self.end_sent();
        self.pictures = restored;
        self.last = None;
        Ok(())
    }
}
