//! Settings: what a stack, a component or a driver can be told, declared once with limits and a default, and the checked
//! values of a schema.

use crate::command::CommandError;
use crate::marks::SettingRole;
use crate::publish::{field, Field};
use crate::record::{DeclError, FieldDecl, FieldId, Kind, Record, RecordDecl};
use crate::stack::ConfigError;
use crate::station::StationItem;
use crate::storage::{StorageError, StorePath};
use crate::value::{Choice, ChoiceDecl, Value, ValueKind};
use serde::{Serialize, Serializer};
use serde_json::Value as Json;
use std::borrow::Cow;
use std::sync::Arc;

/// The index of a setting in its owner's [`Schema`].
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, serde::Deserialize,
)]
pub struct SettingId(pub u16);

impl SettingId {
    /// The slot index.
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
    fn field(self) -> FieldId {
        FieldId(self.0)
    }
}

/// The limits of a number setting. `step` is the grid a front end offers. [`Schema::check`] does not apply it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Range {
    /// The smallest value.
    pub min: f64,
    /// The largest value.
    pub max: f64,
    /// The grid a front end offers. 0 for none.
    pub step: f64,
}

/// The declaration of one setting: a field declaration plus its limits, default, section and help.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SettingDecl {
    /// Key, label, unit, kind and choice list.
    #[serde(flatten)]
    pub field: FieldDecl,
    /// The limits of a number setting. `None` for no limits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    /// The value in force until one is set. [`Value::None`] for a required command argument.
    pub default: Value,
    /// A command argument that has no default and must be given. A setting is never required.
    pub required: bool,
    /// A change takes effect only after the owner restarts.
    pub restart: bool,
    /// The setting cannot be changed (the stack or the graph fixes it).
    pub fixed: bool,
    /// The heading to group the setting under. Empty for none.
    pub section: Cow<'static, str>,
    /// The setting is for a person who changes how the owner works beyond its defaults. A front end shows the other
    /// settings first, and these on request. The owner works without any of them set.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub advanced: bool,
    /// One or two plain sentences for people. Empty for none.
    pub help: Cow<'static, str>,
    /// What the number 0 means when it is not a quantity: "Automatic". Empty when 0 is a number like any other. With
    /// a meaning, 0 is a valid value outside the limits, and a front end shows the meaning in place of the number.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub zero_means: Cow<'static, str>,
    /// The values of a number setting that people normally use. A front end offers them next to the range.
    #[serde(skip_serializing_if = "Common::is_empty")]
    pub common: Common,
    /// What the setting chooses, for a front end that puts its control next to what it acts on. `None` for a setting
    /// without a role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<SettingRole>,
}

/// The values of a number setting that people normally use: one list, or one list for each option of a choice setting of
/// the same schema. Read the list in force with [`Schema::common`].
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Common {
    /// The list when the values do not depend on another setting, or for an option that `sets` does not name.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<f64>,
    /// The key of the choice setting whose option selects the list. Empty when the values do not depend on one.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub by: Cow<'static, str>,
    /// The list for each option key of the setting `by`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sets: Vec<(Cow<'static, str>, Vec<f64>)>,
}

impl Common {
    /// True if no list has a value.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.sets.iter().all(|(_, v)| v.is_empty())
    }
}

/// The common values of a number setting for each option of a choice setting: (option key, values).
pub type CommonSets = &'static [(&'static str, &'static [f64])];

impl SettingDecl {
    /// A setting of `field` with this default, no limits, no section and no help.
    pub fn new(field: FieldDecl, default: Value) -> Self {
        SettingDecl {
            field,
            range: None,
            default,
            required: false,
            restart: false,
            fixed: false,
            advanced: false,
            section: Cow::Borrowed(""),
            help: Cow::Borrowed(""),
            zero_means: Cow::Borrowed(""),
            common: Common::default(),
            role: None,
        }
    }
    /// The same setting with the role `role`: what it chooses for a front end.
    #[must_use]
    pub fn with_role(mut self, role: SettingRole) -> Self {
        self.role = Some(role);
        self
    }
    /// A required command argument of `field`: it has no default and must be given.
    pub fn required(field: FieldDecl) -> Self {
        SettingDecl {
            required: true,
            ..SettingDecl::new(field, Value::None)
        }
    }
    /// The same setting, marked advanced ([`SettingDecl::advanced`]).
    #[must_use]
    pub fn with_advanced(mut self) -> Self {
        self.advanced = true;
        self
    }
    /// The same setting with these limits.
    #[must_use]
    pub fn with_range(mut self, min: f64, max: f64, step: f64) -> Self {
        self.range = Some(Range { min, max, step });
        self
    }
    /// The same setting, applied only after a restart.
    #[must_use]
    pub fn with_restart(mut self) -> Self {
        self.restart = true;
        self
    }
    /// The same setting, fixed.
    #[must_use]
    pub fn with_fixed(mut self) -> Self {
        self.fixed = true;
        self
    }
    /// The same setting under this heading.
    #[must_use]
    pub fn with_section(mut self, section: impl Into<Cow<'static, str>>) -> Self {
        self.section = section.into();
        self
    }
    /// The same setting with this help text.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<Cow<'static, str>>) -> Self {
        self.help = help.into();
        self
    }
    /// The same setting, where the number 0 has this meaning (see [`SettingDecl::zero_means`]).
    #[must_use]
    pub fn with_zero_means(mut self, meaning: impl Into<Cow<'static, str>>) -> Self {
        self.zero_means = meaning.into();
        self
    }
    /// The same setting with these common values ([`SettingDecl::common`]), in place of any list it had.
    #[must_use]
    pub fn with_common(mut self, values: impl Into<Vec<f64>>) -> Self {
        self.common.values = values.into();
        self
    }
    /// The same setting with common values that depend on the option of the choice setting `by` of the same schema: the
    /// values for each option key in `sets` ([`Setting::common_for`]).
    #[must_use]
    pub fn with_common_for(
        mut self,
        by: impl Into<Cow<'static, str>>,
        sets: Vec<(Cow<'static, str>, Vec<f64>)>,
    ) -> Self {
        self.common.by = by.into();
        self.common.sets = sets;
        self
    }
    /// Whether `x` is the number 0 with a declared meaning.
    fn is_meant_zero(&self, x: f64) -> bool {
        x == 0.0 && !self.zero_means.is_empty()
    }
}

/// A field kind that a setting can have: every kind except [`field::Bytes`].
pub trait SettingKind: Kind {
    /// The type of a default given in a constant.
    type Default: Copy + Send + Sync + 'static;
    /// The default as a value.
    fn default_value(d: Self::Default) -> Value;
    /// The default as a read result.
    fn default_out<'a>(d: Self::Default) -> Self::Out<'a>;
}

macro_rules! plain_setting_kind {
    ($marker:ty, $t:ty) => {
        impl SettingKind for $marker {
            type Default = $t;
            fn default_value(d: $t) -> Value {
                let mut v = Value::None;
                <$marker as Kind>::store(&mut v, d);
                v
            }
            fn default_out<'a>(d: $t) -> Self::Out<'a> {
                d
            }
        }
    };
}

plain_setting_kind!(field::F64, f64);
plain_setting_kind!(field::I64, i64);
plain_setting_kind!(field::Bool, bool);
plain_setting_kind!(field::Hz, f64);
plain_setting_kind!(field::Time, f64);
plain_setting_kind!(field::Duration, f64);
plain_setting_kind!(field::Position, (f64, f64));

impl SettingKind for field::Text {
    type Default = &'static str;
    fn default_value(d: &'static str) -> Value {
        Value::Text(d.to_owned())
    }
    fn default_out<'a>(d: &'static str) -> Self::Out<'a> {
        d
    }
}

impl<E: Choice + Send + Sync> SettingKind for field::Choice<E> {
    type Default = E;
    fn default_value(d: E) -> Value {
        Value::Choice(d.index())
    }
    fn default_out<'a>(d: E) -> Self::Out<'a> {
        d
    }
}

/// One setting, named once as a `const`: its field, default and limits. Reads through it are typed: `values.get(&DEEMPH)`
/// returns an `f64`.
pub struct Setting<K: SettingKind> {
    /// Key, label, unit and kind.
    pub field: Field<K>,
    /// The value in force until one is set.
    pub default: K::Default,
    /// The limits of a number setting.
    pub range: Option<Range>,
    /// A change takes effect only after the owner restarts.
    pub restart: bool,
    /// The setting cannot be changed.
    pub fixed: bool,
    /// The heading to group the setting under.
    pub section: &'static str,
    /// The setting is advanced ([`SettingDecl::advanced`]).
    pub advanced: bool,
    /// One or two plain sentences for people.
    pub help: &'static str,
    /// What the number 0 means when it is not a quantity, or empty (see [`SettingDecl::zero_means`]).
    pub zero_means: &'static str,
    /// The sample rate that the owner would like for a value of the setting ([`Setting::sets_rate`]).
    pub rate_of: Option<RateOf>,
    /// The values that people normally use ([`Setting::common`]).
    pub common: &'static [f64],
    /// The key of the choice setting that selects the common values, and the values for each of its options
    /// ([`Setting::common_for`]). Empty key for none.
    pub common_for: (&'static str, CommonSets),
    /// What the setting chooses ([`SettingDecl::role`]).
    pub role: Option<SettingRole>,
}

/// The sample rate in Hz that a stack would like for a value of one of its settings. 0 for the rate of its table
/// ([`Tuning::rate_hz`](crate::Tuning)).
pub type RateOf = fn(&Value) -> f64;

impl<K: SettingKind> Clone for Setting<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: SettingKind> Copy for Setting<K> {}
impl<K: SettingKind> std::fmt::Debug for Setting<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Setting")
            .field("key", &self.field.key)
            .finish()
    }
}

impl<K: SettingKind> Setting<K> {
    /// A setting `key` shown as `label` in `unit`, with this default.
    pub const fn new(
        key: &'static str,
        label: &'static str,
        unit: &'static str,
        default: K::Default,
    ) -> Self {
        Setting {
            field: Field::new(key, label, unit),
            default,
            range: None,
            restart: false,
            fixed: false,
            section: "",
            advanced: false,
            help: "",
            zero_means: "",
            rate_of: None,
            common: &[],
            common_for: ("", &[]),
            role: None,
        }
    }
    /// The same setting with the role `role`: `VIEW.role(SettingRole::PictureView)`. A front end finds the setting by
    /// its role and not by its key.
    #[must_use]
    pub const fn role(mut self, role: SettingRole) -> Self {
        self.role = Some(role);
        self
    }
    /// The same setting, whose value can be taken from column `column` of a row of table `table` of the same owner:
    /// the addressee of the texts that the stack sends, from the row of a station. Other values stay valid. An empty
    /// `column` is the row key.
    #[must_use]
    pub const fn refers(mut self, table: &'static str, column: &'static str) -> Self {
        self.field = self.field.refers_to_column(table, column);
        self
    }
    /// The same setting with the values that people normally use: `RATE.common(&[1.024e6, 2.048e6])`. Each value must
    /// pass the checks of the setting.
    #[must_use]
    pub const fn common(mut self, values: &'static [f64]) -> Self {
        self.common = values;
        self
    }
    /// The same setting with common values that depend on the option of the choice setting `by` of the same schema:
    /// `WIDTH.common_for("demod", &[("cw", &[250.0, 500.0]), ("usb", &[2400.0, 2700.0])])`. An option that `sets`
    /// does not name has the values of [`Setting::common`].
    #[must_use]
    pub const fn common_for(mut self, by: &'static str, sets: CommonSets) -> Self {
        self.common_for = (by, sets);
        self
    }
    /// The same setting, whose value gives the sample rate that the stack would like: a channel width, for example.
    /// The host asks the radio for `rate_of(value)` when the session starts and when the value changes
    /// ([`StackDescriptor::rate_hz`](crate::StackDescriptor::rate_hz)). At most one setting of a schema has this.
    #[must_use]
    pub const fn sets_rate(mut self, rate_of: RateOf) -> Self {
        self.rate_of = Some(rate_of);
        self
    }
    /// The same setting with these limits.
    #[must_use]
    pub const fn range(mut self, min: f64, max: f64, step: f64) -> Self {
        self.range = Some(Range { min, max, step });
        self
    }
    /// The same setting, applied only after a restart.
    #[must_use]
    pub const fn restart(mut self) -> Self {
        self.restart = true;
        self
    }
    /// The same setting, fixed.
    #[must_use]
    pub const fn fixed(mut self) -> Self {
        self.fixed = true;
        self
    }
    /// The same setting, marked advanced ([`SettingDecl::advanced`]).
    #[must_use]
    pub const fn advanced(mut self) -> Self {
        self.advanced = true;
        self
    }
    /// The same setting under this heading.
    #[must_use]
    pub const fn section(mut self, section: &'static str) -> Self {
        self.section = section;
        self
    }
    /// The same setting with this help text.
    #[must_use]
    pub const fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }
    /// The same setting, where the number 0 has this meaning: `RATE.zero_means("Automatic")`.
    #[must_use]
    pub const fn zero_means(mut self, meaning: &'static str) -> Self {
        self.zero_means = meaning;
        self
    }
    /// The key.
    pub const fn key(&self) -> &'static str {
        self.field.key
    }
    /// The value `v` given to setting `id`, as this setting's type.
    ///
    /// # Errors
    ///
    /// [`SettingError::WrongKind`] or [`SettingError::Required`] if `v` is not a value of this kind.
    pub fn read<'v>(&self, id: SettingId, v: &'v Value) -> Result<K::Out<'v>, SettingError> {
        K::load(v).ok_or(match v.kind() {
            Some(got) => SettingError::WrongKind {
                id,
                expected: K::VALUE_KIND,
                got,
            },
            None => SettingError::Required { id },
        })
    }
}

/// A setting of any kind, for lists of declarations (`&[&dyn DeclareSetting]`).
pub trait DeclareSetting: Sync {
    /// The key.
    fn key(&self) -> &'static str;
    /// The declaration.
    fn decl(&self) -> SettingDecl;
    /// The sample rate that the owner would like for a value of the setting. `None` (the default) for a setting that
    /// does not give the rate.
    fn rate_of(&self) -> Option<RateOf> {
        None
    }
}

impl<K: SettingKind> DeclareSetting for Setting<K> {
    fn key(&self) -> &'static str {
        self.field.key
    }
    fn decl(&self) -> SettingDecl {
        SettingDecl {
            field: self.field.decl(),
            range: self.range,
            default: K::default_value(self.default),
            required: false,
            restart: self.restart,
            fixed: self.fixed,
            advanced: self.advanced,
            section: Cow::Borrowed(self.section),
            help: Cow::Borrowed(self.help),
            zero_means: Cow::Borrowed(self.zero_means),
            common: Common {
                values: self.common.to_vec(),
                by: Cow::Borrowed(self.common_for.0),
                sets: self
                    .common_for
                    .1
                    .iter()
                    .map(|(o, v)| (Cow::Borrowed(*o), v.to_vec()))
                    .collect(),
            },
            role: self.role,
        }
    }
    fn rate_of(&self) -> Option<RateOf> {
        self.rate_of
    }
}

/// A choice setting whose list is a catalogue that is read when the schema is made (the codes or sequences of an algorithm
/// crate). The value is the index of an entry in the list that `choices` returns.
#[derive(Clone, Copy, Debug)]
pub struct CatalogueSetting {
    /// The key.
    pub key: &'static str,
    /// What people see.
    pub label: &'static str,
    /// One or two plain sentences for people.
    pub help: &'static str,
    /// The entries of the catalogue, in order.
    pub choices: fn() -> Vec<ChoiceDecl>,
    /// The key of the default entry.
    pub default: &'static str,
    /// The setting is advanced ([`SettingDecl::advanced`]).
    pub advanced: bool,
    /// What the setting chooses ([`SettingDecl::role`]).
    pub role: Option<SettingRole>,
}

impl CatalogueSetting {
    /// The index of the default entry.
    ///
    /// # Panics
    ///
    /// If the default key is not in the catalogue, or the catalogue has more than 65 536 entries. Both are faults of the
    /// declaration.
    pub fn default_index(&self) -> u16 {
        Self::index_in(&(self.choices)(), self.default)
    }
    fn index_in(choices: &[ChoiceDecl], key: &str) -> u16 {
        let i = choices
            .iter()
            .position(|c| c.key == key)
            .expect("the default key is in the catalogue");
        u16::try_from(i).expect("a catalogue has at most 65 536 entries")
    }
    /// The catalogue index that `v` returns to setting `id`.
    ///
    /// # Errors
    ///
    /// [`SettingError::WrongKind`] or [`SettingError::Required`] if `v` is not a choice.
    pub fn read(&self, id: SettingId, v: &Value) -> Result<usize, SettingError> {
        match v {
            Value::Choice(i) => Ok(usize::from(*i)),
            other => Err(match other.kind() {
                Some(got) => SettingError::WrongKind {
                    id,
                    expected: ValueKind::Choice,
                    got,
                },
                None => SettingError::Required { id },
            }),
        }
    }
}

impl DeclareSetting for CatalogueSetting {
    fn key(&self) -> &'static str {
        self.key
    }
    fn decl(&self) -> SettingDecl {
        let choices = (self.choices)();
        let default = Self::index_in(&choices, self.default);
        let d = SettingDecl::new(
            FieldDecl::with_choices(self.key, self.label, choices),
            Value::Choice(default),
        )
        .with_help(self.help);
        let d = match self.role {
            Some(r) => d.with_role(r),
            None => d,
        };
        if self.advanced {
            d.with_advanced()
        } else {
            d
        }
    }
}

/// A required command argument of any kind, [`field::Bytes`] included, named once as a `const`. It has no default:
/// [`Commands::check`](crate::command::Commands::check) refuses a call that leaves it out. Read it with [`Values::arg`].
pub struct Argument<K: Kind> {
    /// Key, label, unit and kind.
    pub field: Field<K>,
    /// One or two plain sentences for people.
    pub help: &'static str,
}

impl<K: Kind> Clone for Argument<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: Kind> Copy for Argument<K> {}
impl<K: Kind> std::fmt::Debug for Argument<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Argument")
            .field("key", &self.field.key)
            .finish()
    }
}

impl<K: Kind> Argument<K> {
    /// A required argument of `field`.
    pub const fn new(field: Field<K>) -> Self {
        Argument { field, help: "" }
    }
    /// The same argument with this help text.
    #[must_use]
    pub const fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }
}

impl<K: Kind> DeclareSetting for Argument<K> {
    fn key(&self) -> &'static str {
        self.field.key
    }
    fn decl(&self) -> SettingDecl {
        SettingDecl::required(self.field.decl()).with_help(self.help)
    }
}

/// The settings of one owner as an enum with one variant per setting. Implement it with [`settings!`](crate::settings!).
pub trait SettingSet: Copy + Sized + 'static {
    /// The settings, in variant order.
    const SETTINGS: &'static [&'static dyn DeclareSetting];
    /// The schema of the settings. It is built once and shared.
    fn schema() -> Arc<Schema>;
    /// The id of this setting.
    fn id(self) -> SettingId;
    /// The setting with this id. `None` if the id is outside the set.
    fn from_id(id: SettingId) -> Option<Self>;
    /// Makes sure that `value` fits this setting, for a constructor argument that the schema limits.
    ///
    /// # Errors
    ///
    /// What [`Schema::check`] returns.
    fn check(self, value: &Value) -> Result<(), SettingError> {
        Self::schema().check(self.id(), value)
    }
}

/// Declares an owner's settings as an enum and implements [`SettingSet`] for it. Each variant names its [`Setting`] constant.
///
/// ```
/// use sdrkit_core::publish::field;
/// use sdrkit_core::settings::{Setting, SettingError, SettingId, SettingSet, Values};
/// use sdrkit_core::Value;
///
/// const DEEMPH: Setting<field::F64> = Setting::new("deemph_us", "De-emphasis", "us", 50.0).range(0.0, 75.0, 25.0);
/// const RDS: Setting<field::Bool> = Setting::new("rds", "RDS", "", true);
///
/// sdrkit_core::settings! {
///     /// The settings of a broadcast FM receiver.
///     pub enum FmSetting {
///         Deemph = DEEMPH,
///         Rds = RDS,
///     }
/// }
///
/// struct Fm {
///     deemph_us: f64,
///     rds: bool,
/// }
///
/// impl Fm {
///     fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
///         match FmSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
///             FmSetting::Deemph => self.deemph_us = DEEMPH.read(id, v)?,
///             FmSetting::Rds => self.rds = RDS.read(id, v)?,
///         }
///         Ok(())
///     }
/// }
///
/// let values = Values::defaults(&FmSetting::schema());
/// let mut fm = Fm { deemph_us: values.get(&DEEMPH), rds: values.get(&RDS) };
/// fm.apply(FmSetting::Deemph.id(), &Value::F64(75.0)).unwrap();
/// assert_eq!(fm.deemph_us, 75.0);
/// ```
#[macro_export]
macro_rules! settings {
    ($(#[$m:meta])* $vis:vis enum $name:ident {
        $($(#[$vm:meta])* $var:ident = $setting:path),+ $(,)?
    }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u16)]
        $vis enum $name {
            $($(#[$vm])* $var),+
        }
        impl $crate::settings::SettingSet for $name {
            const SETTINGS: &'static [&'static dyn $crate::settings::DeclareSetting] = &[$(&$setting),+];
            fn schema() -> ::std::sync::Arc<$crate::settings::Schema> {
                static SCHEMA: ::std::sync::OnceLock<::std::sync::Arc<$crate::settings::Schema>> =
                    ::std::sync::OnceLock::new();
                ::std::sync::Arc::clone(SCHEMA.get_or_init(|| {
                    ::std::sync::Arc::new($crate::settings::Schema::of(Self::SETTINGS))
                }))
            }
            fn id(self) -> $crate::settings::SettingId {
                $crate::settings::SettingId(self as u16)
            }
            fn from_id(id: $crate::settings::SettingId) -> Option<Self> {
                const ALL: &[$name] = &[$($name::$var),+];
                ALL.get(id.index()).copied()
            }
        }
    };
}

/// Why a list of setting declarations is not a schema.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SchemaError {
    /// The keys are not unique, or there are too many settings.
    #[error(transparent)]
    Decl(#[from] DeclError),
    /// A setting has the kind [`ValueKind::Bytes`] or [`ValueKind::Any`], which no setting can have.
    #[error("setting number {} has a kind no setting can have", .id.0)]
    Kind {
        /// The setting.
        id: SettingId,
    },
    /// A setting is required. Only a command argument can be required, every setting has a default.
    #[error("setting number {} has no default", .id.0)]
    NoDefault {
        /// The setting.
        id: SettingId,
    },
    /// The default of a setting does not pass [`Schema::check`].
    #[error("the default of setting number {} is refused: {source}", .id.0)]
    Default {
        /// The setting.
        id: SettingId,
        /// Why the default is refused.
        source: SettingError,
    },
}

/// The settings of one owner, in the order a front end shows them. [`SettingId`] is the index in this list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schema {
    settings: Vec<SettingDecl>,
    record: Arc<RecordDecl>,
    rate: Option<RateRule>,
}

/// The setting of a schema that gives the sample rate, and the rate for its value.
#[derive(Clone, Copy)]
struct RateRule {
    id: SettingId,
    of: RateOf,
}

impl PartialEq for RateRule {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && std::ptr::fn_addr_eq(self.of, other.of)
    }
}

impl std::fmt::Debug for RateRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateRule").field("id", &self.id).finish()
    }
}

impl Schema {
    /// A schema of these settings, for a list built at run time.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] if two keys are the same, a kind is [`ValueKind::Bytes`], a setting is required, or a default does
    /// not pass the check.
    pub fn new(settings: Vec<SettingDecl>) -> Result<Self, SchemaError> {
        Self::build(settings, false)
    }

    /// A schema of command arguments. An argument can be required ([`SettingDecl::required`]) and can be of kind
    /// [`ValueKind::Bytes`].
    ///
    /// # Errors
    ///
    /// [`SchemaError`] if two keys are the same or the default of an argument that is not required does not pass the check.
    pub fn arguments(args: Vec<SettingDecl>) -> Result<Self, SchemaError> {
        Self::build(args, true)
    }

    fn build(settings: Vec<SettingDecl>, arguments: bool) -> Result<Self, SchemaError> {
        let record = Arc::new(RecordDecl::new(
            settings.iter().map(|s| s.field.clone()).collect(),
        )?);
        let schema = Schema {
            settings,
            record,
            rate: None,
        };
        for (i, s) in schema.settings.iter().enumerate() {
            let id = SettingId(i as u16);
            if !arguments && matches!(s.field.kind, ValueKind::Bytes | ValueKind::Any) {
                return Err(SchemaError::Kind { id });
            }
            if s.required {
                if !arguments {
                    return Err(SchemaError::NoDefault { id });
                }
                continue;
            }
            schema
                .check_value(id, s, &s.default)
                .map_err(|source| SchemaError::Default { id, source })?;
        }
        Ok(schema)
    }

    /// A schema of a constant list.
    ///
    /// # Panics
    ///
    /// If [`Schema::new`] refuses the list. A constant list that it refuses is a programming error.
    pub fn of(settings: &[&dyn DeclareSetting]) -> Self {
        let mut schema = match Self::new(settings.iter().map(|s| s.decl()).collect()) {
            Ok(s) => s,
            Err(e) => panic!("{e}"),
        };
        schema.rate = settings.iter().enumerate().find_map(|(i, s)| {
            Some(RateRule {
                id: SettingId(i as u16),
                of: s.rate_of()?,
            })
        });
        if let Err(e) = schema.check_common() {
            panic!("{e}");
        }
        schema
    }

    /// Checks the common values of every setting: each is a value that the setting accepts, and a list that depends on
    /// another setting names a choice setting of the schema and options of it.
    fn check_common(&self) -> Result<(), String> {
        for (i, s) in self.settings.iter().enumerate() {
            let id = SettingId(i as u16);
            let c = &s.common;
            let key = &s.field.key;
            for &x in c.values.iter().chain(c.sets.iter().flat_map(|(_, v)| v)) {
                let v = number_value(s.field.kind, x)
                    .ok_or_else(|| format!("setting {key} has common values but is not a number"))?;
                self.check_value(id, s, &v)
                    .map_err(|e| format!("common value {x} of setting {key}: {e}"))?;
            }
            if c.by.is_empty() {
                if !c.sets.is_empty() {
                    return Err(format!("setting {key} has common values by option but no setting that selects them"));
                }
                continue;
            }
            let by = self
                .id(&c.by)
                .ok()
                .and_then(|b| self.setting(b))
                .filter(|b| b.field.kind == ValueKind::Choice)
                .ok_or_else(|| format!("setting {key} takes its common values from {}, which is not a choice setting of the schema", c.by))?;
            if let Some((o, _)) = c
                .sets
                .iter()
                .find(|(o, _)| !by.field.choices.iter().any(|ch| ch.key == *o))
            {
                return Err(format!("setting {key} has common values for {o}, which is not an option of {}", c.by));
            }
        }
        Ok(())
    }

    /// The common values of setting `id` ([`SettingDecl::common`]). A list that depends on a choice setting is the one for
    /// the option of that setting in `values`, or for its default without `values`. `values` must be of this schema.
    /// Empty for a setting without common values, an id outside the schema, or a choice setting that the schema does not
    /// hold (a declaration copied into another schema under other keys).
    pub fn common(&self, id: SettingId, values: Option<&Values>) -> &[f64] {
        let Some(s) = self.setting(id) else {
            return &[];
        };
        let c = &s.common;
        if c.by.is_empty() {
            return &c.values;
        }
        let Ok(by) = self.id(&c.by) else {
            return &[];
        };
        let Some(by_decl) = self.setting(by) else {
            return &[];
        };
        let value = values
            .and_then(|v| v.value(by))
            .unwrap_or(&by_decl.default);
        let option = match value {
            Value::Choice(i) => by_decl.field.choices.get(usize::from(*i)),
            _ => None,
        };
        option
            .and_then(|o| c.sets.iter().find(|(k, _)| *k == o.key))
            .map_or(&c.values, |(_, v)| v)
    }

    /// True if a setting of the schema gives the sample rate ([`Setting::sets_rate`]).
    pub fn sets_rate(&self) -> bool {
        self.rate.is_some()
    }

    /// The sample rate that the setting of the schema that gives the rate states for its value in `values`. `values`
    /// may be of another schema that has the setting under the same key. `None` if no setting gives the rate, if
    /// `values` does not hold the setting, or if the rate for its value is 0: the rate of the table then counts.
    pub fn rate_hz(&self, values: &Values) -> Option<f64> {
        let rule = self.rate?;
        let key = &self.settings.get(rule.id.index())?.field.key;
        let id = values.schema().id(key).ok()?;
        let hz = (rule.of)(values.value(id)?);
        (hz > 0.0).then_some(hz)
    }

    /// A schema of a constant list of command arguments.
    ///
    /// # Panics
    ///
    /// If [`Schema::arguments`] refuses the list. A constant list that it refuses is a programming error.
    pub fn of_arguments(args: &[&dyn DeclareSetting]) -> Self {
        match Self::arguments(args.iter().map(|s| s.decl()).collect()) {
            Ok(s) => s,
            Err(e) => panic!("{e}"),
        }
    }

    /// The id of the setting with key `key`. This is the only lookup by text. Do it once and keep the id.
    ///
    /// # Errors
    ///
    /// [`SettingError::Unknown`] if the schema has no such key.
    pub fn id(&self, key: &str) -> Result<SettingId, SettingError> {
        self.record
            .id(key)
            .map(|f| SettingId(f.0))
            .ok_or_else(|| SettingError::Unknown { key: key.into() })
    }

    /// The declaration of setting `id`.
    pub fn setting(&self, id: SettingId) -> Option<&SettingDecl> {
        self.settings.get(id.index())
    }

    /// The declarations, in order.
    pub fn settings(&self) -> &[SettingDecl] {
        &self.settings
    }

    /// The number of settings.
    pub fn len(&self) -> usize {
        self.settings.len()
    }

    /// True if the schema has no settings.
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }

    /// The record declaration that [`Values`] are built on.
    pub fn record_decl(&self) -> &Arc<RecordDecl> {
        &self.record
    }

    /// Checks `value` for setting `id`: the setting exists and is not fixed, the kind is right, a number is finite and inside
    /// its range, and a choice index is inside the list. Nothing is clamped or snapped.
    ///
    /// # Errors
    ///
    /// [`SettingError::NoSuchId`], [`SettingError::Fixed`], [`SettingError::Required`] (for [`Value::None`]),
    /// [`SettingError::WrongKind`], [`SettingError::NotFinite`], [`SettingError::OutOfRange`] or
    /// [`SettingError::NotAChoice`].
    pub fn check(&self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let s = self.setting(id).ok_or(SettingError::NoSuchId { id })?;
        if s.fixed {
            return Err(SettingError::Fixed { id });
        }
        self.check_value(id, s, value)
    }

    fn check_value(
        &self,
        id: SettingId,
        s: &SettingDecl,
        value: &Value,
    ) -> Result<(), SettingError> {
        let Some(got) = value.kind() else {
            return Err(SettingError::Required { id });
        };
        if s.field.kind == ValueKind::Any {
            return Ok(());
        }
        if got != s.field.kind {
            return Err(SettingError::WrongKind {
                id,
                expected: s.field.kind,
                got,
            });
        }
        let in_range = |x: f64, min: f64, max: f64| {
            if !x.is_finite() {
                Err(SettingError::NotFinite { id })
            } else if x < min || x > max {
                Err(SettingError::OutOfRange {
                    id,
                    min,
                    max,
                    got: x,
                })
            } else {
                Ok(())
            }
        };
        let (min, max) = s
            .range
            .map_or((f64::NEG_INFINITY, f64::INFINITY), |r| (r.min, r.max));
        match value {
            Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => {
                if s.is_meant_zero(*x) {
                    return Ok(());
                }
                in_range(*x, min, max)
            }
            Value::I64(0) if s.is_meant_zero(0.0) => Ok(()),
            Value::I64(x) => in_range(*x as f64, min, max),
            Value::Position { lat, lon } => {
                in_range(*lat, -90.0, 90.0)?;
                in_range(*lon, -180.0, 180.0)
            }
            Value::Choice(index) if usize::from(*index) >= s.field.choices.len() => {
                Err(SettingError::NotAChoice { id, index: *index })
            }
            _ => Ok(()),
        }
    }
}

/// The number `x` as a value of `kind`. `None` for a kind that is not a number.
fn number_value(kind: ValueKind, x: f64) -> Option<Value> {
    match kind {
        ValueKind::F64 => Some(Value::F64(x)),
        ValueKind::Hz => Some(Value::Hz(x)),
        ValueKind::Time => Some(Value::Time(x)),
        ValueKind::Duration => Some(Value::Duration(x)),
        ValueKind::I64 if x.fract() == 0.0 => Some(Value::I64(x as i64)),
        _ => None,
    }
}

/// A schema serialises as the list of its setting declarations.
impl Serialize for Schema {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.settings.serialize(s)
    }
}

/// A complete, checked set of values for a schema: one slot per setting.
#[derive(Clone, Debug, PartialEq)]
pub struct Values {
    schema: Arc<Schema>,
    record: Record,
}

impl Values {
    /// Every setting of `schema` at its default.
    pub fn defaults(schema: &Arc<Schema>) -> Self {
        let mut record = Record::new(Arc::clone(&schema.record));
        for (i, s) in schema.settings.iter().enumerate() {
            // A schema has made sure that every default fits its setting.
            let _ = record.set_value(FieldId(i as u16), &s.default);
        }
        Values {
            schema: Arc::clone(schema),
            record,
        }
    }

    /// The schema.
    pub fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }

    /// The value of setting `s`.
    ///
    /// # Panics
    ///
    /// In debug builds, if the schema does not declare `s` with its kind. In release builds the default of `s` is returned.
    pub fn get<K: SettingKind>(&self, s: &Setting<K>) -> K::Out<'_> {
        match self.record.get(&s.field) {
            Some(v) => v,
            None => {
                crate::record::refuse_write(
                    s.field.key,
                    "a setting that the schema does not declare",
                );
                K::default_out(s.default)
            }
        }
    }

    /// The catalogue index of setting `s`.
    ///
    /// # Panics
    ///
    /// In debug builds, if the schema does not declare `s` as a choice. In release builds the default index of `s` is
    /// returned.
    pub fn entry(&self, s: &CatalogueSetting) -> u16 {
        let held = self.schema.id(s.key).ok().and_then(|id| {
            match self.record.value(crate::record::FieldId(id.0))? {
                Value::Choice(i) => Some(*i),
                _ => None,
            }
        });
        match held {
            Some(i) => i,
            None => {
                crate::record::refuse_write(s.key, "a setting that the schema does not declare");
                s.default_index()
            }
        }
    }

    /// The value of the required command argument `a`.
    ///
    /// # Errors
    ///
    /// [`CommandError::NotAnArgument`] if the schema does not declare `a` with its kind. [`CommandError::MissingArgument`]
    /// if it holds no value. After [`Commands::check`](crate::command::Commands::check), an argument that the command declares
    /// always holds one.
    pub fn arg<K: Kind>(&self, a: &Argument<K>) -> Result<K::Out<'_>, CommandError> {
        let not_an_argument = CommandError::NotAnArgument { key: a.field.key };
        let arg = self
            .schema
            .id(a.field.key)
            .map_err(|_| not_an_argument.clone())?;
        if self.record.value(arg.field()).is_none_or(Value::is_none) {
            return Err(CommandError::MissingArgument { arg });
        }
        self.record.get(&a.field).ok_or(not_an_argument)
    }

    /// The value of setting `id`.
    pub fn value(&self, id: SettingId) -> Option<&Value> {
        self.record.value(id.field())
    }

    /// The values as a record.
    pub fn record(&self) -> &Record {
        &self.record
    }

    /// Sets setting `id` after [`Schema::check`].
    ///
    /// # Errors
    ///
    /// What [`Schema::check`] returns. The values do not change.
    pub fn set(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        self.schema.check(id, value)?;
        self.record
            .set_value(id.field(), value)
            .map(|_| ())
            .map_err(|_| SettingError::NoSuchId { id })
    }

    /// The values as a JSON object by key, without the fixed settings. Numbers are JSON numbers, yes/no settings are JSON
    /// booleans, texts are JSON strings, a choice is its key and a position is `{"lat", "lon"}`.
    pub fn to_json(&self) -> Json {
        let mut m = serde_json::Map::new();
        for (s, (_, v)) in self.schema.settings.iter().zip(self.record.iter()) {
            if s.fixed {
                continue;
            }
            let j = match v {
                Value::None => Json::Null,
                Value::Bool(b) => Json::Bool(*b),
                Value::I64(x) => Json::from(*x),
                Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => {
                    Json::from(*x)
                }
                Value::Text(t) => Json::String(t.clone()),
                Value::Position { lat, lon } => serde_json::json!({ "lat": lat, "lon": lon }),
                Value::Choice(i) => match s.field.choices.get(usize::from(*i)) {
                    Some(c) => Json::String(c.key.to_string()),
                    None => Json::from(*i),
                },
                Value::Bytes(b) => Json::from(b.to_vec()),
            };
            m.insert(s.field.key.to_string(), j);
        }
        Json::Object(m)
    }

    /// Values of `schema` read from a JSON object of the form [`Values::to_json`] returns. A setting that the object leaves out
    /// keeps its default.
    ///
    /// # Errors
    ///
    /// [`ValuesError`] for the first entry that is refused.
    pub fn from_json(schema: &Arc<Schema>, json: &Json) -> Result<Self, ValuesError> {
        let mut v = Values::defaults(schema);
        v.update_from_json(json)?;
        Ok(v)
    }

    /// Sets every setting that a JSON object of the form [`Values::to_json`] returns. All or nothing: if one entry is refused,
    /// no value changes.
    ///
    /// # Errors
    ///
    /// [`ValuesError`] for the first entry that is refused: an unknown key, a fixed setting, a wrong JSON type, an unknown
    /// choice key, or a value that [`Schema::check`] refuses.
    pub fn update_from_json(&mut self, json: &Json) -> Result<(), ValuesError> {
        let Json::Object(map) = json else {
            return Err(ValuesError::NotAnObject);
        };
        let mut next = self.record.clone();
        for (key, j) in map {
            let id = self.schema.id(key)?;
            let s = &self.schema.settings[id.index()];
            let v = from_json(id, s, j)?;
            self.schema.check(id, &v)?;
            next.set_value(id.field(), &v)
                .map_err(|_| SettingError::NoSuchId { id })?;
        }
        self.record = next;
        Ok(())
    }
}

/// The values serialise as the JSON object of [`Values::to_json`].
impl Serialize for Values {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(ser)
    }
}

/// The type of a JSON value, for [`ValuesError::WrongJson`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonKind {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool,
    /// A number.
    Number,
    /// A string.
    String,
    /// An array.
    Array,
    /// An object.
    Object,
}

impl JsonKind {
    fn of(j: &Json) -> Self {
        match j {
            Json::Null => JsonKind::Null,
            Json::Bool(_) => JsonKind::Bool,
            Json::Number(_) => JsonKind::Number,
            Json::String(_) => JsonKind::String,
            Json::Array(_) => JsonKind::Array,
            Json::Object(_) => JsonKind::Object,
        }
    }
}

fn from_json(id: SettingId, s: &SettingDecl, j: &Json) -> Result<Value, ValuesError> {
    let wrong = || ValuesError::WrongJson {
        id,
        expected: s.field.kind,
        got: JsonKind::of(j),
    };
    let num = || j.as_f64().ok_or_else(wrong);
    Ok(match s.field.kind {
        ValueKind::Bool => Value::Bool(j.as_bool().ok_or_else(wrong)?),
        ValueKind::I64 => Value::I64(j.as_i64().ok_or_else(wrong)?),
        ValueKind::F64 => Value::F64(num()?),
        ValueKind::Hz => Value::Hz(num()?),
        ValueKind::Time => Value::Time(num()?),
        ValueKind::Duration => Value::Duration(num()?),
        ValueKind::Text => Value::Text(j.as_str().ok_or_else(wrong)?.to_owned()),
        ValueKind::Choice => {
            let key = j.as_str().ok_or_else(wrong)?;
            Value::Choice(
                s.field
                    .choice_index(key)
                    .ok_or_else(|| ValuesError::UnknownChoice {
                        id,
                        key: key.to_owned(),
                    })?,
            )
        }
        ValueKind::Position => {
            let lat = j.get("lat").and_then(Json::as_f64).ok_or_else(wrong)?;
            let lon = j.get("lon").and_then(Json::as_f64).ok_or_else(wrong)?;
            Value::Position { lat, lon }
        }
        ValueKind::Bytes | ValueKind::Any => return Err(wrong()),
    })
}

/// Why a JSON object of settings was refused.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ValuesError {
    /// The JSON is not an object.
    #[error("the settings are not a JSON object")]
    NotAnObject,
    /// The JSON type does not fit the kind of the setting.
    #[error("setting number {} takes {expected:?}, not a JSON {got:?}", .id.0)]
    WrongJson {
        /// The setting.
        id: SettingId,
        /// The kind of the setting.
        expected: ValueKind,
        /// The JSON type given.
        got: JsonKind,
    },
    /// A choice setting was given a key that its list does not have.
    #[error("setting number {} has no choice {key}", .id.0)]
    UnknownChoice {
        /// The setting.
        id: SettingId,
        /// The key given.
        key: String,
    },
    /// The key is unknown, or the value is refused by [`Schema::check`].
    #[error(transparent)]
    Setting(#[from] SettingError),
}

/// Why a setting or a value for it was refused.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SettingError {
    /// No setting has this key. Only a lookup by key returns it.
    #[error("there is no setting {key}")]
    Unknown {
        /// The key.
        key: String,
    },
    /// No setting has this id.
    #[error("there is no setting number {}", .id.0)]
    NoSuchId {
        /// The id.
        id: SettingId,
    },
    /// The value is of another kind than the setting.
    #[error("setting number {} takes {expected:?}, not {got:?}", .id.0)]
    WrongKind {
        /// The setting.
        id: SettingId,
        /// The kind of the setting.
        expected: ValueKind,
        /// The kind of the value.
        got: ValueKind,
    },
    /// The setting needs a value: [`Value::None`], or an empty text where the owner needs one.
    #[error("setting number {} needs a value", .id.0)]
    Required {
        /// The setting.
        id: SettingId,
    },
    /// The number is outside the range of the setting.
    #[error("{got} is outside {min} to {max} for setting number {}", .id.0)]
    OutOfRange {
        /// The setting.
        id: SettingId,
        /// The smallest value.
        min: f64,
        /// The largest value.
        max: f64,
        /// The value given.
        got: f64,
    },
    /// The number is NaN or infinite.
    #[error("setting number {} needs a finite number", .id.0)]
    NotFinite {
        /// The setting.
        id: SettingId,
    },
    /// The choice index is outside the list of the setting, or matches no entry of a list that is loaded at run time.
    #[error("setting number {} has no choice {index}", .id.0)]
    NotAChoice {
        /// The setting.
        id: SettingId,
        /// The index given.
        index: u16,
    },
    /// The number is inside the range, but the standard allows only some values there.
    #[error("setting number {} does not allow this value", .id.0)]
    NotPermitted {
        /// The setting.
        id: SettingId,
    },
    /// The text does not parse as the setting needs (a list of channels, a range of groups, hex digits, a time).
    #[error("the text of setting number {} does not parse", .id.0)]
    Malformed {
        /// The setting.
        id: SettingId,
        /// The byte offset of the first part that does not parse, if the parser reports it.
        at: Option<usize>,
    },
    /// The setting is fixed by the stack or the graph.
    #[error("setting number {} is fixed", .id.0)]
    Fixed {
        /// The setting.
        id: SettingId,
    },
    /// The value does not agree with the value of another setting.
    #[error("setting number {} does not agree with setting number {}", .id.0, .with.0)]
    Conflict {
        /// The setting.
        id: SettingId,
        /// The other setting.
        with: SettingId,
    },
    /// The owner does not use this setting in its present configuration.
    #[error("setting number {} is not used now", .id.0)]
    Unsupported {
        /// The setting.
        id: SettingId,
    },
    /// The value needs a part of the station context that is not set.
    #[error("setting number {} needs the station's {item:?}", .id.0)]
    NeedsStation {
        /// The setting.
        id: SettingId,
        /// The missing part.
        item: StationItem,
    },
    /// The value names a stored file that cannot be read or written.
    #[error("setting number {}: {path}: {source}", .id.0)]
    Storage {
        /// The setting.
        id: SettingId,
        /// The file.
        path: StorePath,
        /// What went wrong.
        source: StorageError,
    },
    /// The value is valid, but the owner cannot configure itself with it.
    #[error("setting number {}: {source}", .id.0)]
    Config {
        /// The setting.
        id: SettingId,
        /// Why the configuration failed.
        source: Box<ConfigError>,
    },
}

impl SettingError {
    /// The same error with each setting id changed by `f`. An owner that forwards a setting to a part (a graph to a component)
    /// uses it to give the error with its own ids.
    #[must_use]
    pub fn map_ids(self, f: impl Fn(SettingId) -> SettingId) -> SettingError {
        use SettingError as E;
        match self {
            E::Unknown { key } => E::Unknown { key },
            E::NoSuchId { id } => E::NoSuchId { id: f(id) },
            E::WrongKind { id, expected, got } => E::WrongKind {
                id: f(id),
                expected,
                got,
            },
            E::Required { id } => E::Required { id: f(id) },
            E::OutOfRange { id, min, max, got } => E::OutOfRange {
                id: f(id),
                min,
                max,
                got,
            },
            E::NotFinite { id } => E::NotFinite { id: f(id) },
            E::NotAChoice { id, index } => E::NotAChoice { id: f(id), index },
            E::NotPermitted { id } => E::NotPermitted { id: f(id) },
            E::Malformed { id, at } => E::Malformed { id: f(id), at },
            E::Fixed { id } => E::Fixed { id: f(id) },
            E::Conflict { id, with } => E::Conflict {
                id: f(id),
                with: f(with),
            },
            E::Unsupported { id } => E::Unsupported { id: f(id) },
            E::NeedsStation { id, item } => E::NeedsStation { id: f(id), item },
            E::Storage { id, path, source } => E::Storage {
                id: f(id),
                path,
                source,
            },
            E::Config { id, source } => E::Config { id: f(id), source },
        }
    }
}

impl<E: Choice + Send + Sync> Setting<field::Choice<E>> {
    /// A choice setting `key` shown as `label`, with this default.
    pub const fn choice(key: &'static str, label: &'static str, default: E) -> Self {
        Setting::new(key, label, "", default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice_enum;

    choice_enum! {
        /// A test region.
        pub enum Region {
            One = ("r1", "Region 1"),
            Two = ("r2", "Region 2"),
        }
    }

    const GAIN: Setting<field::F64> =
        Setting::new("gain_db", "Gain", "dB", 10.0).range(0.0, 40.0, 1.0);
    const AGC: Setting<field::Bool> = Setting::new("agc", "AGC", "", true);
    const REGION: Setting<field::Choice<Region>> = Setting::choice("region", "Region", Region::One);
    const NAME: Setting<field::Text> = Setting::new("name", "Name", "", "");
    const LOCK: Setting<field::I64> = Setting::new("lock", "Lock", "", 3).fixed();

    crate::settings! {
        /// Test settings.
        enum Test {
            Gain = GAIN,
            Agc = AGC,
            Region = REGION,
            Name = NAME,
            Lock = LOCK,
        }
    }

    #[test]
    fn check_refuses_each_case_with_its_variant() {
        let s = Test::schema();
        let g = Test::Gain.id();
        assert_eq!(s.check(g, &Value::F64(20.0)), Ok(()));
        assert_eq!(
            s.check(g, &Value::Bool(true)),
            Err(SettingError::WrongKind {
                id: g,
                expected: ValueKind::F64,
                got: ValueKind::Bool
            })
        );
        assert_eq!(
            s.check(g, &Value::F64(41.0)),
            Err(SettingError::OutOfRange {
                id: g,
                min: 0.0,
                max: 40.0,
                got: 41.0
            })
        );
        assert_eq!(
            s.check(g, &Value::F64(f64::NAN)),
            Err(SettingError::NotFinite { id: g })
        );
        assert_eq!(
            s.check(g, &Value::None),
            Err(SettingError::Required { id: g })
        );
        let r = Test::Region.id();
        assert_eq!(s.check(r, &Value::Choice(1)), Ok(()));
        assert_eq!(
            s.check(r, &Value::Choice(2)),
            Err(SettingError::NotAChoice { id: r, index: 2 })
        );
        let l = Test::Lock.id();
        assert_eq!(
            s.check(l, &Value::I64(3)),
            Err(SettingError::Fixed { id: l })
        );
        assert_eq!(
            s.check(SettingId(9), &Value::I64(3)),
            Err(SettingError::NoSuchId { id: SettingId(9) })
        );
        assert_eq!(
            s.id("volume"),
            Err(SettingError::Unknown {
                key: "volume".into()
            })
        );
    }

    #[test]
    fn reads_are_typed_and_ids_round_trip() {
        let s = Test::schema();
        let mut v = Values::defaults(&s);
        assert_eq!(v.get(&GAIN), 10.0);
        assert!(v.get(&AGC));
        assert_eq!(v.get(&REGION), Region::One);
        v.set(Test::Region.id(), &Value::Choice(1)).unwrap();
        assert_eq!(v.get(&REGION), Region::Two);
        for t in [Test::Gain, Test::Agc, Test::Region, Test::Name, Test::Lock] {
            assert_eq!(Test::from_id(t.id()), Some(t));
        }
        assert_eq!(Test::from_id(SettingId(5)), None);
        assert_eq!(s.id("region"), Ok(Test::Region.id()));
        assert_eq!(GAIN.read(Test::Gain.id(), &Value::F64(3.0)), Ok(3.0));
    }

    #[test]
    fn a_runtime_schema_takes_an_owned_choice_list() {
        let choices = vec![
            crate::ChoiceDecl::new("a", "A"),
            crate::ChoiceDecl::new("b", "B"),
        ];
        let f = FieldDecl::with_choices("plan", "Plan", choices);
        let s = Arc::new(Schema::new(vec![SettingDecl::new(f, Value::Choice(1))]).unwrap());
        assert_eq!(
            Values::defaults(&s).to_json(),
            serde_json::json!({ "plan": "b" })
        );
        let bad = SettingDecl::new(
            FieldDecl::new("x", "X", "", ValueKind::F64),
            Value::F64(5.0),
        )
        .with_range(0.0, 1.0, 0.0);
        assert!(matches!(
            Schema::new(vec![bad]),
            Err(SchemaError::Default { .. })
        ));
    }

    #[test]
    fn json_round_trips_and_a_strict_read_changes_nothing_on_error() {
        let s = Test::schema();
        let mut v = Values::defaults(&s);
        v.update_from_json(&serde_json::json!({ "gain_db": 12, "region": "r2", "name": "x" }))
            .unwrap();
        let j = v.to_json();
        assert_eq!(
            j,
            serde_json::json!({ "gain_db": 12.0, "agc": true, "region": "r2", "name": "x" })
        );
        assert_eq!(Values::from_json(&s, &j).unwrap(), v);

        let before = v.clone();
        let unknown = serde_json::json!({ "gain_db": 5, "volume": 1 });
        assert_eq!(
            v.update_from_json(&unknown),
            Err(ValuesError::Setting(SettingError::Unknown {
                key: "volume".into()
            }))
        );
        let wrong = serde_json::json!({ "gain_db": 5, "agc": "yes" });
        assert!(matches!(
            v.update_from_json(&wrong),
            Err(ValuesError::WrongJson { .. })
        ));
        let range = serde_json::json!({ "agc": false, "gain_db": 99 });
        assert!(matches!(
            v.update_from_json(&range),
            Err(ValuesError::Setting(SettingError::OutOfRange { .. }))
        ));
        let choice = serde_json::json!({ "region": "r9" });
        assert!(matches!(
            v.update_from_json(&choice),
            Err(ValuesError::UnknownChoice { .. })
        ));
        let fixed = serde_json::json!({ "lock": 4 });
        assert!(matches!(
            v.update_from_json(&fixed),
            Err(ValuesError::Setting(SettingError::Fixed { .. }))
        ));
        assert_eq!(v, before);
    }

    #[test]
    fn a_catalogue_setting_and_field_read_and_write_by_entry() {
        use crate::publish::{CatalogueField, Published};
        use crate::value::{Choice, ChoiceDecl};

        fn codes() -> Vec<ChoiceDecl> {
            vec![
                ChoiceDecl::new("a", "Code A"),
                ChoiceDecl::new("b", "Code B"),
            ]
        }
        const CODE: CatalogueSetting = CatalogueSetting {
            key: "code",
            label: "Code",
            help: "",
            choices: codes,
            default: "b",
            advanced: false,
            role: None,
        };
        const LEVEL: Setting<field::F64> = Setting::new("level", "Level", "", 1.0);
        crate::settings! {
            enum CodeSetting {
                Level = LEVEL,
                Code = CODE,
            }
        }
        let mut v = Values::defaults(&CodeSetting::schema());
        assert_eq!(v.entry(&CODE), 1);
        v.set(CodeSetting::Code.id(), &Value::Choice(0)).unwrap();
        assert_eq!(v.entry(&CODE), 0);
        assert_eq!(CodeSetting::from_id(SettingId(1)), Some(CodeSetting::Code));

        const SEEN: CatalogueField = CatalogueField {
            key: "seen",
            label: "Seen code",
            choices: codes,
        };
        let mut p = Published::new(&[&SEEN]);
        p.set_entry(&SEEN, 1);
        assert_eq!(p.results().fields().entry(&SEEN), Some(1));
        let decl = p
            .results()
            .decl()
            .fields()
            .field(crate::record::FieldId(0))
            .unwrap();
        assert_eq!(decl.choices[1].label, "Code B");

        crate::choice_enum! {
            enum Side {
                Left = ("left", "Left side"),
                Right = ("right", "Right side"),
            }
        }
        assert_eq!(Side::Right.label(), "Right side");
        assert_eq!(Side::Left.key(), "left");
        assert_eq!(Side::from_key("right"), Some(Side::Right));
        assert_eq!(Side::from_key("up"), None);
    }

    #[test]
    fn a_setting_that_sets_the_rate_gives_the_rate_for_its_value() {
        fn of(width: &Value) -> f64 {
            match width {
                Value::F64(hz) => hz * 8.0 / 7.0,
                _ => 0.0,
            }
        }
        const WIDTH: Setting<field::F64> = Setting::new("width", "Width", "Hz", 0.0).sets_rate(of);
        const PLAIN_WIDTH: Setting<field::F64> = Setting::new("width", "Width", "Hz", 0.0);
        const LEVEL: Setting<field::F64> = Setting::new("level", "Level", "", 1.0);
        crate::settings! {
            enum WithRate {
                Level = LEVEL,
                Width = WIDTH,
            }
        }
        crate::settings! {
            enum Other {
                Width = PLAIN_WIDTH,
            }
        }
        let schema = WithRate::schema();
        assert!(schema.sets_rate() && !Other::schema().sets_rate());
        let mut v = Values::defaults(&schema);
        // The value 0 states no rate.
        assert_eq!(schema.rate_hz(&v), None);
        v.set(WithRate::Width.id(), &Value::F64(7e6)).unwrap();
        assert_eq!(schema.rate_hz(&v), Some(8e6));
        // The values of another owner that holds the setting under the same key.
        let mut other = Values::defaults(&Other::schema());
        other.set(Other::Width.id(), &Value::F64(6e6)).unwrap();
        assert_eq!(schema.rate_hz(&other), Some(6e6 * 8.0 / 7.0));
        assert_eq!(Other::schema().rate_hz(&other), None);
        assert_eq!(schema.rate_hz(&Values::defaults(&Arc::default())), None);
    }

    const STEP: Setting<field::Hz> = Setting::new("step", "Step", "Hz", 1000.0)
        .range(1.0, 1e6, 0.0)
        .common(&[100.0, 1000.0]);
    const WIDTH: Setting<field::Hz> = Setting::new("width", "Width", "Hz", 0.0)
        .range(0.0, 10_000.0, 0.0)
        .zero_means("Automatic")
        .common(&[0.0, 5000.0])
        .common_for("region", &[("r2", &[200.0, 2000.0])]);

    crate::settings! {
        /// Settings with common values.
        enum Usual {
            Region = REGION,
            Step = STEP,
            Width = WIDTH,
        }
    }

    #[test]
    fn common_values_follow_the_option_of_the_setting_that_selects_them() {
        let schema = Usual::schema();
        let width = Usual::Width.id();
        assert_eq!(schema.common(Usual::Step.id(), None), &[100.0, 1000.0]);
        assert_eq!(schema.common(Usual::Region.id(), None), &[] as &[f64]);
        // Without values, the default option r1 has no list of its own: the plain list holds.
        assert_eq!(schema.common(width, None), &[0.0, 5000.0]);
        let mut values = Values::defaults(&schema);
        assert_eq!(schema.common(width, Some(&values)), &[0.0, 5000.0]);
        values.set(Usual::Region.id(), &Value::Choice(1)).unwrap();
        assert_eq!(schema.common(width, Some(&values)), &[200.0, 2000.0]);
        // A copy under other keys has no setting that selects the list.
        let renamed: Vec<SettingDecl> = schema
            .settings()
            .iter()
            .map(|d| {
                let mut d = d.clone();
                d.field.key = format!("x.{}", d.field.key).into();
                d
            })
            .collect();
        let renamed = Schema::new(renamed).unwrap();
        assert_eq!(renamed.common(width, None), &[] as &[f64]);
        assert_eq!(renamed.common(Usual::Step.id(), None), &[100.0, 1000.0]);
        assert_eq!(schema.common(SettingId(9), None), &[] as &[f64]);
    }

    #[test]
    fn common_values_must_fit_their_setting_and_name_real_options() {
        let bad = [
            SettingDecl::new(FieldDecl::new("g", "G", "dB", ValueKind::F64), Value::F64(1.0))
                .with_range(0.0, 10.0, 0.0)
                .with_common(vec![20.0]),
        ];
        let schema = Schema::new(bad.to_vec()).unwrap();
        assert!(schema.check_common().unwrap_err().contains("common value 20"));
        let by_text = WIDTH
            .decl()
            .with_common_for("name", vec![("r2".into(), vec![200.0])]);
        let schema = Schema::new(vec![NAME.decl(), by_text]).unwrap();
        assert!(schema.check_common().unwrap_err().contains("not a choice setting"));
        let mut wrong = WIDTH.decl();
        wrong.common.sets[0].0 = "r9".into();
        let schema = Schema::new(vec![REGION.decl(), wrong]).unwrap();
        assert!(schema.check_common().unwrap_err().contains("r9"));
        let text = SettingDecl::new(FieldDecl::new("t", "T", "", ValueKind::Text), Value::Text(String::new()))
            .with_common(vec![1.0]);
        let schema = Schema::new(vec![text]).unwrap();
        assert!(schema.check_common().unwrap_err().contains("not a number"));
        assert_eq!(Usual::schema().check_common(), Ok(()));
    }

    #[test]
    fn common_values_serialise_only_when_present() {
        let json = serde_json::to_value(Usual::schema().settings()).unwrap();
        assert_eq!(json[0].get("common"), None);
        assert_eq!(json[1]["common"]["values"], serde_json::json!([100.0, 1000.0]));
        assert_eq!(json[2]["common"]["by"], "region");
        assert_eq!(json[2]["common"]["sets"][0][0], "r2");
    }
}
