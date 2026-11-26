//! The presets part of the C API, built with the feature `presets`: the lookups of `sdrkit-presets` over a data folder.
//! The header `include/sdrkit_presets.h` is written by hand and declares what this file defines. A library built
//! without the feature holds none of it and links no data reader.

use crate::error::{
    Error, SDRKIT_E_BAD_ARGUMENT, SDRKIT_E_IO, SDRKIT_E_MALFORMED, SDRKIT_E_NOT_FOUND,
    SDRKIT_E_NO_SUCH_ID, SDRKIT_E_NO_SUCH_OBJECT, SDRKIT_E_OUT_OF_RANGE, SDRKIT_E_PANIC,
    SDRKIT_E_WRONG_HANDLE, SDRKIT_OK,
};
use crate::handle::{lock, sized_in, sized_structs, slice_in, str_in, Handle, Out};
use crate::value::{count, Arg, Str, Variant};
use sdrkit_data_plans::api::PlanFormat;
use sdrkit_data_plans::Place;
use sdrkit_presets::{
    lora::SlotRow, preset_settings, Data, DataError, FrequencyRow, Hit, HitKind, Options, PlanChannel, Preset,
    PresetError, PresetSetting, Region, RegionChoice, StartError,
};
use sdrkit_svc_freqdb::{Entry, Label, View};
use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

/// A plan.
pub const SDRKIT_PRESETS_HIT_PLAN: i32 = 0;
/// A channel of a plan.
pub const SDRKIT_PRESETS_HIT_CHANNEL: i32 = 1;
/// A segment of a plan.
pub const SDRKIT_PRESETS_HIT_SEGMENT: i32 = 2;
/// An entry of the frequency database.
pub const SDRKIT_PRESETS_HIT_ENTRY: i32 = 3;

/// The format of a plan file is taken from its text; an export writes CSV.
pub const SDRKIT_PRESETS_FORMAT_AUTO: i32 = 0;
/// CSV, one channel or segment in a row.
pub const SDRKIT_PRESETS_FORMAT_CSV: i32 = 1;
/// JSON.
pub const SDRKIT_PRESETS_FORMAT_JSON: i32 = 2;

/// One search result.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsHit {
    /// The number of bytes of the struct.
    pub size: u32,
    /// An `SDRKIT_PRESETS_HIT_*` constant.
    pub kind: i32,
    /// The plan id, `<plan>/<channel>` for a channel or a segment, or the id of the entry.
    pub id: Str,
    /// The name to show.
    pub name: Str,
    /// The name of the plan of a channel, the kind of a plan, the stack of an entry.
    pub detail: Str,
    /// The plan id of a plan, a channel or a segment. Empty for an entry.
    pub plan: Str,
    /// `frequency_hz` holds a value.
    pub has_frequency_hz: bool,
    /// The frequency, Hz.
    pub frequency_hz: f64,
    /// `width_hz` holds a value.
    pub has_width_hz: bool,
    /// The width, Hz.
    pub width_hz: f64,
    /// How well the result matches. The results are in the order of the score, best first.
    pub score: f64,
}

/// The span that labels are for.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsView {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The left edge, Hz.
    pub lo_hz: f64,
    /// The right edge, Hz.
    pub hi_hz: f64,
    /// The width, pixels.
    pub width_px: f64,
    /// The width of one character, pixels. 0 for 7.
    pub char_px: f64,
    /// The rows of labels. 0 for 3.
    pub rows: u32,
}

/// One placed label.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsLabel {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The label is a span (a band or a range). Otherwise it is one frequency.
    pub span: bool,
    /// The frequency, Hz: the lower edge of a span.
    pub hz: f64,
    /// `end_hz` holds a value.
    pub has_end_hz: bool,
    /// The upper edge of a span, Hz.
    pub end_hz: f64,
    /// The text.
    pub text: Str,
    /// The id of the entry of the label. Empty for a name of a plan.
    pub id: Str,
    /// The position of the frequency from the left edge, pixels.
    pub x_px: f64,
    /// The width of a span, pixels.
    pub span_px: f64,
    /// The row, 0 nearest the spectrum.
    pub row: u32,
}

/// A place in a plan: a channel, a raster point of a segment, or a frequency inside a segment.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsPlace {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The channel is reallocated.
    pub reallocated: bool,
    /// The plan id.
    pub plan: Str,
    /// The name to show.
    pub name: Str,
    /// The centre of the channel or the raster point, Hz.
    pub centre_hz: f64,
    /// `width_hz` holds a value.
    pub has_width_hz: bool,
    /// The width of the channel, Hz.
    pub width_hz: f64,
    /// The offset of the asked frequency from `centre_hz`, Hz.
    pub offset_hz: f64,
}

/// What a preset is asked for: a stack in a country, with the choices of the user.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsQuery {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The stack id.
    pub stack: *const c_char,
    /// The country, ISO 3166 alpha-2.
    pub country: *const c_char,
    /// The city of the country, for the known stations. NULL for none.
    pub city: *const c_char,
    /// The submode of the stack. NULL for its default.
    pub submode: *const c_char,
    /// `frequency_hz` holds a value.
    pub has_frequency_hz: bool,
    /// The frequency to start on, Hz, in place of that of the data.
    pub frequency_hz: f64,
}

/// One channel that the data has for a stack in a country.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsChannel {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The number or the name of the channel.
    pub label: Str,
    /// The centre frequency, Hz.
    pub frequency_hz: f64,
    /// `width_hz` holds a value.
    pub has_width_hz: bool,
    /// The width, Hz.
    pub width_hz: f64,
}

/// One supplied entry of the frequency database for a stack in a country.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsFrequency {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The stable id of the entry.
    pub id: Str,
    /// The name.
    pub name: Str,
    /// The frequency to tune, Hz.
    pub frequency_hz: f64,
    /// `bandwidth_hz` holds a value.
    pub has_bandwidth_hz: bool,
    /// The bandwidth, Hz.
    pub bandwidth_hz: f64,
    /// The notes of the entry. Empty if it has none.
    pub note: Str,
}

/// Where a stack starts in a country.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsDialInfo {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The frequency to start on, Hz.
    pub frequency_hz: f64,
    /// `width_hz` holds a value.
    pub has_width_hz: bool,
    /// The width of the channel at that frequency, Hz.
    pub width_hz: f64,
    /// The id of the plan or the band group of the channels. Empty if there is none.
    pub plan: Str,
    /// The submode to start in. Empty for the default of the stack.
    pub submode: Str,
    /// The network of the known station at the frequency. Empty if the frequency is not of a known station.
    pub network: Str,
}

/// One country that the data has plans for.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsCountry {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The ISO 3166 alpha-2 code, upper case.
    pub code: Str,
    /// The name of the country in the plan files. Empty if they give none.
    pub name: Str,
}

/// One setting of a stack for a country, ready to set: its key, and its id and value as an argument of a start.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsSetting {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The key of the setting.
    pub key: Str,
    /// The id of the setting of the stack type and its value, of the type of the setting.
    pub arg: Arg,
}

/// What the Meshtastic slots are asked for: a region, a preset and the channel settings.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsSlotQuery {
    /// The number of bytes of the struct.
    pub size: u32,
    /// A Meshtastic region ("ANZ"), a band plan of the mesh stacks ("EU 868") or a country, ISO 3166 alpha-2.
    pub region: *const c_char,
    /// The Meshtastic preset. NULL for LongFast.
    pub preset: *const c_char,
    /// The name of the primary channel. NULL for the name of the preset.
    pub channel_name: *const c_char,
    /// The channel number, from 1. 0 takes the slot from the channel name.
    pub channel_num: u32,
}

/// One Meshtastic frequency slot, as a row of the table `slots` of the stack gives it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsSlot {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The slot number, from 1.
    pub slot: u32,
    /// The centre of the slot, Hz.
    pub frequency_hz: f64,
    /// The stack listens on the slot with the channel settings of the query.
    pub current: bool,
    /// The slot of the default channel name of the preset.
    pub preset_default: bool,
}

/// Why a stack offers no channels, or fewer, in a country: the mode, or a part of it, is out of service there.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresetsServiceInfo {
    /// The number of bytes of the struct.
    pub size: u32,
    /// The mode, or a part of it, is out of service in the country. False: the rest of the struct is empty.
    pub out_of_service: bool,
    /// Only a part of the mode ended; the channels of the part that remains stay in the lists.
    pub partial: bool,
    /// `year` holds a value.
    pub has_year: bool,
    /// The year the service ended.
    pub year: i32,
    /// The date the service ended as the data gives it ("2024", "2024-10" or "2024-10-28"). Empty if not known.
    pub ended: Str,
    /// One sentence for a person: "3G (UMTS) was switched off in Australia in 2024".
    pub sentence: Str,
}

sized_structs!(
    PresetsHit,
    PresetsView,
    PresetsLabel,
    PresetsPlace,
    PresetsQuery,
    PresetsChannel,
    PresetsFrequency,
    PresetsDialInfo,
    PresetsCountry,
    PresetsSetting,
    PresetsSlotQuery,
    PresetsSlot,
    PresetsServiceInfo,
);

/// What one presets handle holds: the data folder and the buffers behind the views of its calls.
struct State {
    data: Data,
    entries: Option<Vec<Entry>>,
    ids: Vec<String>,
    hits: Vec<Hit>,
    labels: Vec<(bool, Label)>,
    place: (String, String),
    channels: Vec<PlanChannel>,
    frequencies: Vec<FrequencyRow>,
    countries: Vec<RegionChoice>,
    dial: (String, String, String),
    settings: Vec<PresetSetting>,
    slots: Vec<SlotRow>,
    service_end: (String, String),
    message: String,
}

/// The mark of a presets handle, so that no handle of an engine is one.
const MARK: Handle = 1 << 63;

/// One slot of the table of open handles: its generation, which a close raises, and its state.
type Slot = (u32, Option<Arc<Mutex<State>>>);

static OPEN: Mutex<Vec<Slot>> = Mutex::new(Vec::new());

/// The handle of slot `index` in generation `generation`.
fn handle_of(index: usize, generation: u32) -> Handle {
    MARK | (Handle::from(generation & 0x7FFF_FFFF) << 32) | (index as Handle + 1)
}

/// The slot index and the generation of the presets handle `h`.
fn split(h: Handle) -> Option<(usize, u32)> {
    if h & MARK == 0 {
        return None;
    }
    let index = usize::try_from(h & 0xFFFF_FFFF).ok()?.checked_sub(1)?;
    Some((index, ((h & !MARK) >> 32) as u32))
}
/// The message of the last `sdrkit_presets_open` that failed.
static OPEN_MESSAGE: Mutex<String> = Mutex::new(String::new());

/// An error of a presets call: its code and its sentence.
struct Fault(i32, String);

impl From<Error> for Fault {
    fn from(e: Error) -> Fault {
        Fault(e.code(), e.to_string())
    }
}

impl From<DataError> for Fault {
    fn from(e: DataError) -> Fault {
        let code = match &e {
            DataError::Missing { .. } => SDRKIT_E_NOT_FOUND,
            DataError::Read { .. } => SDRKIT_E_IO,
            DataError::Format { .. } => SDRKIT_E_MALFORMED,
        };
        Fault(code, e.to_string())
    }
}

impl From<PresetError> for Fault {
    fn from(e: PresetError) -> Fault {
        match e {
            PresetError::NoData { .. } => Fault(SDRKIT_E_NOT_FOUND, e.to_string()),
            PresetError::Data(d) => d.into(),
            PresetError::ChannelNumTooHigh(_) => Fault(SDRKIT_E_OUT_OF_RANGE, e.to_string()),
        }
    }
}

impl From<StartError> for Fault {
    fn from(e: StartError) -> Fault {
        match e {
            StartError::Preset(p) => p.into(),
            StartError::Setting { .. } => Fault(SDRKIT_E_MALFORMED, e.to_string()),
            StartError::Engine(e) => Error::Engine(*e).into(),
        }
    }
}

fn fault(code: i32, message: &str) -> Fault {
    Fault(code, message.to_owned())
}

/// The state of the presets handle `h`.
fn state_of(h: Handle) -> Option<Arc<Mutex<State>>> {
    let (i, generation) = split(h)?;
    match lock(&OPEN).get(i)? {
        (g, state) if *g & 0x7FFF_FFFF == generation => state.clone(),
        _ => None,
    }
}

/// Runs the body of a call on the presets handle `h`, and keeps the sentence of an error in the handle.
fn run(h: Handle, f: impl FnOnce(&mut State) -> Result<(), Fault>) -> i32 {
    let Some(state) = state_of(h) else {
        return SDRKIT_E_NO_SUCH_OBJECT;
    };
    let mut st = lock(&state);
    match catch_unwind(AssertUnwindSafe(|| f(&mut st))) {
        Ok(Ok(())) => SDRKIT_OK,
        Ok(Err(Fault(code, message))) => {
            st.message = message;
            code
        }
        Err(_) => {
            st.message = "the library failed inside".to_owned();
            SDRKIT_E_PANIC
        }
    }
}

/// The string at `p`, or `None` for NULL.
unsafe fn str_or_null<'a>(p: *const c_char) -> Result<Option<&'a str>, Error> {
    if p.is_null() {
        Ok(None)
    } else {
        // SAFETY: the caller's contract.
        unsafe { str_in(p) }.map(Some)
    }
}

/// The preset that the data of `st` has for the query at `query`.
unsafe fn preset_of(st: &State, query: *const PresetsQuery) -> Result<Preset, Fault> {
    // SAFETY: the caller's contract.
    let q = unsafe { sized_in(query) }?;
    // SAFETY: the caller's contract: the strings of the query are NUL-terminated or NULL.
    let (stack, country, city, submode) = unsafe {
        (str_in(q.stack)?, str_in(q.country)?, str_or_null(q.city)?, str_or_null(q.submode)?)
    };
    let region = Region::new(country)
        .map_err(|e| fault(SDRKIT_E_BAD_ARGUMENT, &e.to_string()))?
        .with_city(city.unwrap_or(""));
    let options = Options {
        frequency_hz: q.has_frequency_hz.then_some(q.frequency_hz),
        submode: submode.map(str::to_owned),
        settings: Vec::new(),
    };
    Ok(st.data.preset(stack, &region, &options)?)
}

/// The `n` plan ids at `plans`.
unsafe fn ids_in<'a>(plans: *const *const c_char, n: usize) -> Result<Vec<&'a str>, Error> {
    // SAFETY: the caller's contract.
    let list = unsafe { slice_in(plans, n) }?;
    // SAFETY: the caller's contract.
    list.iter().map(|p| unsafe { str_in(*p) }).collect()
}

fn format_of(format: i32) -> Result<PlanFormat, Fault> {
    match format {
        SDRKIT_PRESETS_FORMAT_AUTO => Ok(PlanFormat::Auto),
        SDRKIT_PRESETS_FORMAT_CSV => Ok(PlanFormat::Csv),
        SDRKIT_PRESETS_FORMAT_JSON => Ok(PlanFormat::Json),
        _ => Err(fault(SDRKIT_E_BAD_ARGUMENT, "the format is not one of its constants")),
    }
}

/// The entry of the engine handle `engine`, or of a handle of an object of the engine.
fn engine_of(engine: Handle) -> Result<Arc<crate::handle::Entry>, Fault> {
    let e = crate::handle::get(engine)
        .ok_or_else(|| fault(SDRKIT_E_NO_SUCH_OBJECT, "the engine handle is not open"))?;
    if matches!(e.what, crate::handle::What::Graph) {
        return Err(fault(SDRKIT_E_WRONG_HANDLE, "the handle is not of an engine"));
    }
    Ok(e)
}

impl State {
    /// The frequency entries of the folder, read once. A folder without the file has none.
    fn entries(&mut self) -> Result<&[Entry], Fault> {
        if self.entries.is_none() {
            self.entries = Some(match self.data.frequency_entries() {
                Ok(v) => v,
                Err(DataError::Missing { .. }) => Vec::new(),
                Err(e) => return Err(e.into()),
            });
        }
        Ok(self.entries.as_deref().unwrap_or_default())
    }

    /// Keeps `place` and returns its view.
    fn place(&mut self, place: Place) -> PresetsPlace {
        self.place = (place.plan, place.name);
        PresetsPlace {
            reallocated: place.reallocated,
            plan: Str::of(&self.place.0),
            name: Str::of(&self.place.1),
            centre_hz: place.centre_hz,
            has_width_hz: place.width_hz.is_some(),
            width_hz: place.width_hz.unwrap_or(0.0),
            offset_hz: place.offset_hz,
            ..PresetsPlace::default()
        }
    }
}

/// Opens the data folder `folder` of the computer, reads its plan files, and writes a handle to `presets`. Free it
/// with `sdrkit_presets_close`. Returns `SDRKIT_E_NOT_FOUND` if the folder or its folder `plans` does not exist,
/// `SDRKIT_E_IO` if a file cannot be read, and `SDRKIT_E_MALFORMED` for a plan file that does not have its format;
/// `sdrkit_presets_error_message` with handle 0 then returns the sentence.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_open(folder: *const c_char, presets: *mut Handle) -> i32 {
    // SAFETY: the caller's contract.
    let (folder, out) = unsafe { (str_in(folder), Out::new(presets)) };
    let opened = catch_unwind(AssertUnwindSafe(|| -> Result<(), Fault> {
        let (folder, out) = (folder?, out?);
        let state = State {
            data: Data::open(folder)?,
            entries: None,
            ids: Vec::new(),
            hits: Vec::new(),
            labels: Vec::new(),
            place: Default::default(),
            channels: Vec::new(),
            frequencies: Vec::new(),
            countries: Vec::new(),
            dial: Default::default(),
            settings: Vec::new(),
            slots: Vec::new(),
            service_end: Default::default(),
            message: String::new(),
        };
        let state = Some(Arc::new(Mutex::new(state)));
        let mut open = lock(&OPEN);
        let i = match open.iter().position(|slot| slot.1.is_none()) {
            Some(i) => {
                open[i].1 = state;
                i
            }
            None => {
                open.push((0, state));
                open.len() - 1
            }
        };
        out.put(handle_of(i, open[i].0));
        Ok(())
    }));
    match opened {
        Ok(Ok(())) => SDRKIT_OK,
        Ok(Err(Fault(code, message))) => {
            *lock(&OPEN_MESSAGE) = message;
            code
        }
        Err(_) => SDRKIT_E_PANIC,
    }
}

/// Frees the presets handle `presets`. Every view of its calls ends.
#[no_mangle]
pub extern "C" fn sdrkit_presets_close(presets: Handle) -> i32 {
    let mut open = lock(&OPEN);
    match split(presets).and_then(|(i, g)| open.get_mut(i).filter(|s| s.0 & 0x7FFF_FFFF == g)) {
        Some(slot) if slot.1.is_some() => {
            // A later handle of the slot is of another generation, so this handle stays closed.
            *slot = (slot.0.wrapping_add(1), None);
            SDRKIT_OK
        }
        _ => SDRKIT_E_NO_SUCH_OBJECT,
    }
}

/// Writes the sentence of the last error of `presets` to `message`; with handle 0, that of the last
/// `sdrkit_presets_open` that failed. The string is a view, valid until the next call on `presets`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_error_message(presets: Handle, message: *mut Str) -> i32 {
    // SAFETY: the caller's contract.
    let Ok(out) = (unsafe { Out::new(message) }) else {
        return crate::error::SDRKIT_E_NULL_POINTER;
    };
    if presets == 0 {
        thread_local! {
            static KEPT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        }
        KEPT.with(|k| {
            *k.borrow_mut() = lock(&OPEN_MESSAGE).clone();
            out.put(Str {
                ptr: k.borrow().as_ptr().cast(),
                len: k.borrow().len(),
            });
        });
        return SDRKIT_OK;
    }
    let Some(state) = state_of(presets) else {
        return SDRKIT_E_NO_SUCH_OBJECT;
    };
    out.put(Str::of(&lock(&state).message));
    SDRKIT_OK
}

/// Sets the file `file` of the computer as the file of the user's plans of `presets`, adds its plans if it exists,
/// and writes their number to `count`. `sdrkit_presets_plan_import` and `sdrkit_presets_plan_remove` then write the
/// file. Returns `SDRKIT_E_MALFORMED` for a file that is not a plan file.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_personal_plans(
    presets: Handle,
    file: *const c_char,
    count_out: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (file, out) = unsafe { (str_in(file), Out::new(count_out)) };
    run(presets, |st| {
        let out = out?;
        out.put(count(st.data.open_personal_plans(file?)?));
        Ok(())
    })
}

/// Lists the countries that have at least one plan, by code, and writes their number to `count`.
/// `sdrkit_presets_country_read` reads one.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_countries(presets: Handle, count_out: *mut u32) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count_out) };
    run(presets, |st| {
        let out = out?;
        st.countries = st.data.regions();
        out.put(count(st.countries.len()));
        Ok(())
    })
}

/// Writes country `index` of the last `sdrkit_presets_countries` to `country`. Its strings are views, valid until the
/// next `sdrkit_presets_countries` on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_country_read(
    presets: Handle,
    index: u32,
    country: *mut PresetsCountry,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(country) };
    run(presets, |st| {
        let out = out?;
        let c = st
            .countries
            .get(index as usize)
            .ok_or_else(|| fault(SDRKIT_E_NO_SUCH_ID, "the index is outside the list"))?;
        out.put(PresetsCountry {
            code: Str::of(&c.code),
            name: Str::of(&c.name),
            ..PresetsCountry::default()
        });
        Ok(())
    })
}

/// Lists the plans that the country `country` (ISO 3166 alpha-2) uses, the plan to start with first in each kind, and
/// writes their number to `count`. `sdrkit_presets_plan_id` reads one. A country that the data does not name has none.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_plan_ids(
    presets: Handle,
    country: *const c_char,
    count_out: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (country, out) = unsafe { (str_in(country), Out::new(count_out)) };
    run(presets, |st| {
        let out = out?;
        st.ids = st.data.plan_ids(country?);
        out.put(count(st.ids.len()));
        Ok(())
    })
}

/// Writes plan id `index` of the last `sdrkit_presets_plan_ids` to `id`. The string is a view, valid until the next
/// `sdrkit_presets_plan_ids` on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_plan_id(presets: Handle, index: u32, id: *mut Str) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(id) };
    run(presets, |st| {
        let out = out?;
        let id = st
            .ids
            .get(index as usize)
            .ok_or_else(|| fault(SDRKIT_E_NO_SUCH_ID, "the index is outside the list"))?;
        out.put(Str::of(id));
        Ok(())
    })
}

/// Gives the scan service of the engine `engine` the channels of the `count` plans `plans`: for each stack that a
/// plan names, the name and the frequency of each channel. Writes the number of stacks that have channels to
/// `stacks`, which can be NULL. The list replaces the one before. An id that no plan has is skipped.
/// `engine` is the handle of the engine or of an object of it.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_scan_channels_set(
    presets: Handle,
    engine: Handle,
    plans: *const *const c_char,
    count_in: usize,
    stacks: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (plans, out) = unsafe { (ids_in(plans, count_in), Out::new(stacks)) };
    run(presets, |st| {
        let out = crate::handle::optional(out)?;
        let engine = engine_of(engine)?;
        let n = st.data.load_scan_channels(&engine.host.engine, &plans?);
        if let Some(out) = out {
            out.put(count(n));
        }
        Ok(())
    })
}

/// Gives the frequency database of the engine `engine` the entries of the file `freqdb/builtin.csv` of the folder,
/// and writes their number to `count`, which can be NULL. The list replaces the one before. Returns
/// `SDRKIT_E_NOT_FOUND` if the folder has no such file.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_frequency_entries_set(
    presets: Handle,
    engine: Handle,
    count_out: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count_out) };
    run(presets, |st| {
        let out = crate::handle::optional(out)?;
        let engine = engine_of(engine)?;
        let n = st.data.load_frequency_entries(&engine.host.engine)?;
        if let Some(out) = out {
            out.put(count(n));
        }
        Ok(())
    })
}

/// Searches every plan, the channels and segments of the `count` plans `plans`, and the frequency entries of the
/// folder for `text`: a name, a frequency ("474", "14074 kHz") or a channel name ("21", "ch 16"). Writes the number
/// of results to `found`, best first. `sdrkit_presets_hit_read` reads one.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_search(
    presets: Handle,
    text: *const c_char,
    plans: *const *const c_char,
    count_in: usize,
    found: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (text, plans, out) = unsafe { (str_in(text), ids_in(plans, count_in), Out::new(found)) };
    run(presets, |st| {
        let out = out?;
        st.entries()?;
        st.hits = st
            .data
            .search(text?, &plans?, st.entries.as_deref().unwrap_or_default());
        out.put(count(st.hits.len()));
        Ok(())
    })
}

/// Writes result `index` of the last `sdrkit_presets_search` to `hit`. Its strings are views, valid until the next
/// `sdrkit_presets_search` on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_hit_read(presets: Handle, index: u32, hit: *mut PresetsHit) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(hit) };
    run(presets, |st| {
        let out = out?;
        let h = st
            .hits
            .get(index as usize)
            .ok_or_else(|| fault(SDRKIT_E_NO_SUCH_ID, "the index is outside the list"))?;
        out.put(PresetsHit {
            kind: match h.kind {
                HitKind::Plan => SDRKIT_PRESETS_HIT_PLAN,
                HitKind::Channel => SDRKIT_PRESETS_HIT_CHANNEL,
                HitKind::Segment => SDRKIT_PRESETS_HIT_SEGMENT,
                HitKind::Entry => SDRKIT_PRESETS_HIT_ENTRY,
            },
            id: Str::of(&h.id),
            name: Str::of(&h.name),
            detail: Str::of(&h.detail),
            plan: Str::of(h.plan.as_deref().unwrap_or("")),
            has_frequency_hz: h.frequency_hz.is_some(),
            frequency_hz: h.frequency_hz.unwrap_or(0.0),
            has_width_hz: h.width_hz.is_some(),
            width_hz: h.width_hz.unwrap_or(0.0),
            score: h.score,
            ..PresetsHit::default()
        });
        Ok(())
    })
}

/// Places the labels of the span `view`: the names of the frequency entries of the folder that apply in the country
/// `country` (NULL for every entry) and the channels and segments of the `count` plans `plans`. Writes the number of
/// labels to `found`. `sdrkit_presets_label_read` reads one. Returns `SDRKIT_E_BAD_ARGUMENT` for a view without a width.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_labels(
    presets: Handle,
    plans: *const *const c_char,
    count_in: usize,
    country: *const c_char,
    view: *const PresetsView,
    found: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (plans, view, out) = unsafe { (ids_in(plans, count_in), sized_in(view), Out::new(found)) };
    // SAFETY: the caller's contract.
    let country = (!country.is_null()).then(|| unsafe { str_in(country) });
    run(presets, |st| {
        let (out, v) = (out?, view?);
        if !(v.hi_hz > v.lo_hz && v.width_px > 0.0) {
            return Err(fault(SDRKIT_E_BAD_ARGUMENT, "the view has no width"));
        }
        let view = View {
            lo_hz: v.lo_hz,
            hi_hz: v.hi_hz,
            width_px: v.width_px,
            char_px: if v.char_px > 0.0 { v.char_px } else { 7.0 },
            rows: if v.rows > 0 { v.rows as usize } else { 3 },
        };
        let scope = match country.transpose()? {
            Some(c) => st.data.frequency_scope(c),
            None => Vec::new(),
        };
        st.entries()?;
        let l = st.data.labels(
            st.entries.as_deref().unwrap_or_default(),
            &plans?,
            &scope,
            &view,
        );
        st.labels = l
            .points
            .into_iter()
            .map(|p| (false, p))
            .chain(l.spans.into_iter().map(|s| (true, s)))
            .collect();
        out.put(count(st.labels.len()));
        Ok(())
    })
}

/// Writes label `index` of the last `sdrkit_presets_labels` to `label`: the labels of one frequency first, then the
/// spans. Its strings are views, valid until the next `sdrkit_presets_labels` on `presets`. Returns
/// `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_label_read(
    presets: Handle,
    index: u32,
    label: *mut PresetsLabel,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(label) };
    run(presets, |st| {
        let out = out?;
        let (span, l) = st
            .labels
            .get(index as usize)
            .ok_or_else(|| fault(SDRKIT_E_NO_SUCH_ID, "the index is outside the list"))?;
        out.put(PresetsLabel {
            span: *span,
            hz: l.hz,
            has_end_hz: l.end_hz.is_some(),
            end_hz: l.end_hz.unwrap_or(0.0),
            text: Str::of(&l.text),
            id: Str::of(&l.id),
            x_px: l.x_px,
            span_px: l.span_px,
            row: count(l.row),
            ..PresetsLabel::default()
        });
        Ok(())
    })
}

/// Writes the name of the channel or the segment at `hz` in the `count` plans `plans` to `place`, the most specific
/// one. Its strings are views, valid until the next call on `presets` that writes a place. Returns
/// `SDRKIT_E_NOT_FOUND` if no plan of the list holds the frequency.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_name_of(
    presets: Handle,
    hz: f64,
    plans: *const *const c_char,
    count_in: usize,
    place: *mut PresetsPlace,
) -> i32 {
    // SAFETY: the caller's contract.
    let (plans, out) = unsafe { (ids_in(plans, count_in), Out::new(place)) };
    run(presets, |st| {
        let out = out?;
        let found = st
            .data
            .name_of(hz, &plans?)
            .ok_or_else(|| fault(SDRKIT_E_NOT_FOUND, "no plan of the list holds the frequency"))?;
        out.put(st.place(found));
        Ok(())
    })
}

/// Writes the channel that the typed name `name` ("21", "9A", "ch 16") has in the first of the `count` plans `plans`
/// that has it to `place`. The views are as those of `sdrkit_presets_name_of`. Returns `SDRKIT_E_NOT_FOUND` if no plan
/// of the list has the name.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_channel_named(
    presets: Handle,
    name: *const c_char,
    plans: *const *const c_char,
    count_in: usize,
    place: *mut PresetsPlace,
) -> i32 {
    // SAFETY: the caller's contract.
    let (name, plans, out) = unsafe { (str_in(name), ids_in(plans, count_in), Out::new(place)) };
    run(presets, |st| {
        let out = out?;
        let found = st
            .data
            .channel_named(name?, &plans?)
            .ok_or_else(|| fault(SDRKIT_E_NOT_FOUND, "no plan of the list has the name"))?;
        out.put(st.place(found));
        Ok(())
    })
}

/// Writes the next channel of the plan `plan` above `hz` (`up`) or below it to `place`. A reallocated channel counts
/// with `include_reallocated`. The views are as those of `sdrkit_presets_name_of`. Returns `SDRKIT_E_NOT_FOUND` if no
/// plan has the id or the plan has no channel there.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_step(
    presets: Handle,
    plan: *const c_char,
    hz: f64,
    up: bool,
    include_reallocated: bool,
    place: *mut PresetsPlace,
) -> i32 {
    // SAFETY: the caller's contract.
    let (plan, out) = unsafe { (str_in(plan), Out::new(place)) };
    run(presets, |st| {
        let (plan, out) = (plan?, out?);
        let (next, name) = st
            .data
            .step(plan, hz, up, include_reallocated)
            .ok_or_else(|| fault(SDRKIT_E_NOT_FOUND, "the plan has no channel there"))?;
        let found = st.data.name_of(next, &[plan]).unwrap_or(Place {
            plan: plan.to_owned(),
            name,
            centre_hz: next,
            width_hz: None,
            segment: None,
            modulation: String::new(),
            reallocated: false,
            vision_hz: None,
            sound_hz: None,
            pair_hz: None,
            offset_hz: 0.0,
        });
        out.put(st.place(found));
        Ok(())
    })
}

/// Adds the plans of the plan file `file` of the computer, in the format `format` (an `SDRKIT_PRESETS_FORMAT_*`
/// constant), to the user's plans of `presets`, and writes their number to `added`, which can be NULL. A CSV file
/// that gives no id or name takes `id` and `name`. A plan with the id of a plan of the folder replaces it. All or
/// nothing. Returns `SDRKIT_E_NOT_FOUND` if the file does not exist and `SDRKIT_E_MALFORMED` if it is not a valid
/// plan file.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_plan_import(
    presets: Handle,
    file: *const c_char,
    format: i32,
    id: *const c_char,
    name: *const c_char,
    added: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (file, id, name, out) = unsafe { (str_in(file), str_in(id), str_in(name), Out::new(added)) };
    run(presets, |st| {
        let out = crate::handle::optional(out)?;
        let file = std::path::Path::new(file?);
        let text = std::fs::read_to_string(file).map_err(|e| {
            let code = if e.kind() == std::io::ErrorKind::NotFound {
                SDRKIT_E_NOT_FOUND
            } else {
                SDRKIT_E_IO
            };
            Fault(code, format!("{}: {e}", file.display()))
        })?;
        let ids = st.data.import_plan(&text, format_of(format)?, id?, name?)?;
        if let Some(out) = out {
            out.put(count(ids.len()));
        }
        Ok(())
    })
}

/// Writes the plan `plan` to the file `file` of the computer in the format `format` (an `SDRKIT_PRESETS_FORMAT_*`
/// constant). Returns `SDRKIT_E_NOT_FOUND` if no plan has the id and `SDRKIT_E_IO` if the write fails.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_plan_export(
    presets: Handle,
    plan: *const c_char,
    format: i32,
    file: *const c_char,
) -> i32 {
    // SAFETY: the caller's contract.
    let (plan, file) = unsafe { (str_in(plan), str_in(file)) };
    run(presets, |st| {
        let text = st
            .data
            .export_plan(plan?, format_of(format)?)
            .ok_or_else(|| fault(SDRKIT_E_NOT_FOUND, "no plan has the id"))?;
        let file = file?;
        std::fs::write(file, text).map_err(|e| Fault(SDRKIT_E_IO, format!("{file}: {e}")))
    })
}

/// Removes the user's plan `plan`. A plan of the folder that it replaced is back. Returns `SDRKIT_E_NOT_FOUND` if the
/// user has no plan of the id.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_plan_remove(presets: Handle, plan: *const c_char) -> i32 {
    // SAFETY: the caller's contract.
    let plan = unsafe { str_in(plan) };
    run(presets, |st| {
        if st.data.remove_plan(plan?)? {
            Ok(())
        } else {
            Err(fault(SDRKIT_E_NOT_FOUND, "the user has no plan of the id"))
        }
    })
}

/// Lists the channels that the data has for the stack of `query` in its country, lowest first, and writes their
/// number to `count`: those of the plan of the stack, else the scan channels of the plans of the country, else the
/// dials of every band plan that a mesh stack offers in the country, else the downlinks of the stack, else its
/// starting dials. The channels of the user's plans that list the stack, and the
/// country or no country, come in addition. `sdrkit_presets_channel_read` reads one. Returns `SDRKIT_E_BAD_ARGUMENT`
/// for a country that is not a code of two letters.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_channels(
    presets: Handle,
    query: *const PresetsQuery,
    count_out: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (q, out) = unsafe { (sized_in(query), Out::new(count_out)) };
    run(presets, |st| {
        let (q, out) = (q?, out?);
        // SAFETY: the caller's contract: the strings of the query are NUL-terminated or NULL.
        let (stack, country, city) = unsafe { (str_in(q.stack)?, str_in(q.country)?, str_or_null(q.city)?) };
        let region = Region::new(country)
            .map_err(|e| fault(SDRKIT_E_BAD_ARGUMENT, &e.to_string()))?
            .with_city(city.unwrap_or(""));
        st.channels = st.data.stack_channels(stack, &region)?;
        out.put(count(st.channels.len()));
        Ok(())
    })
}

/// Lists the dials that the data has for the stack of `query`, lowest first, and writes their number to `count`: the
/// rows of the dial file of the submode of the query or of the stack, each labelled by its band; if neither has a row
/// and the data gives the stack the setting `dials`, every row, labelled by its stack and band. The city and the
/// frequency of the query are not used. `sdrkit_presets_channel_read` reads one. Returns `SDRKIT_E_BAD_ARGUMENT` for a
/// country that is not a code of two letters.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_dials(
    presets: Handle,
    query: *const PresetsQuery,
    count_out: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (q, out) = unsafe { (sized_in(query), Out::new(count_out)) };
    run(presets, |st| {
        let (q, out) = (q?, out?);
        // SAFETY: the caller's contract: the strings of the query are NUL-terminated or NULL.
        let (stack, country, submode) = unsafe { (str_in(q.stack)?, str_in(q.country)?, str_or_null(q.submode)?) };
        let region = Region::new(country).map_err(|e| fault(SDRKIT_E_BAD_ARGUMENT, &e.to_string()))?;
        st.channels = st.data.dials(stack, &region, submode)?;
        out.put(count(st.channels.len()));
        Ok(())
    })
}

/// Writes channel `index` of the last `sdrkit_presets_channels` or `sdrkit_presets_dials` to `channel`. Its label is a
/// view, valid until the next of these calls on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the
/// list.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_channel_read(
    presets: Handle,
    index: u32,
    channel: *mut PresetsChannel,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(channel) };
    run(presets, |st| {
        let out = out?;
        let c = st
            .channels
            .get(index as usize)
            .ok_or_else(|| fault(SDRKIT_E_NO_SUCH_ID, "the index is outside the list"))?;
        out.put(PresetsChannel {
            label: Str::of(&c.label),
            frequency_hz: c.frequency_hz,
            has_width_hz: c.width_hz.is_some(),
            width_hz: c.width_hz.unwrap_or(0.0),
            ..PresetsChannel::default()
        });
        Ok(())
    })
}

/// Lists the supplied entries of the frequency database (`freqdb/builtin.csv`) for the stack of `query` in its country,
/// lowest first, and writes their number to `count`: the entries whose stack is the stack, and the entries of the
/// general receiver or of no stack that have the stack id as a tag; each applies everywhere, in the country or in its
/// ITU region. The city, the submode and the frequency of the query are not used. `sdrkit_presets_frequency_read`
/// reads one. Returns `SDRKIT_E_BAD_ARGUMENT` for a country that is not a code of two letters.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_frequencies(
    presets: Handle,
    query: *const PresetsQuery,
    count_out: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (q, out) = unsafe { (sized_in(query), Out::new(count_out)) };
    run(presets, |st| {
        let (q, out) = (q?, out?);
        // SAFETY: the caller's contract: the strings of the query are NUL-terminated or NULL.
        let (stack, country) = unsafe { (str_in(q.stack)?, str_in(q.country)?) };
        let region = Region::new(country).map_err(|e| fault(SDRKIT_E_BAD_ARGUMENT, &e.to_string()))?;
        st.frequencies = st.data.frequency_rows(stack, &region)?;
        out.put(count(st.frequencies.len()));
        Ok(())
    })
}

/// Writes entry `index` of the last `sdrkit_presets_frequencies` to `frequency`. Its strings are views, valid until the
/// next `sdrkit_presets_frequencies` on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_frequency_read(
    presets: Handle,
    index: u32,
    frequency: *mut PresetsFrequency,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(frequency) };
    run(presets, |st| {
        let out = out?;
        let f = st
            .frequencies
            .get(index as usize)
            .ok_or_else(|| fault(SDRKIT_E_NO_SUCH_ID, "the index is outside the list"))?;
        out.put(PresetsFrequency {
            id: Str::of(&f.id),
            name: Str::of(&f.name),
            frequency_hz: f.frequency_hz,
            has_bandwidth_hz: f.bandwidth_hz.is_some(),
            bandwidth_hz: f.bandwidth_hz.unwrap_or(0.0),
            note: Str::of(&f.note),
            ..PresetsFrequency::default()
        });
        Ok(())
    })
}

/// Writes where the stack of `query` starts in its country to `dial`: the strongest known station of the city, else the
/// first channel of its plan, else its first downlink, else its starting dial; the frequency of the query replaces it.
/// Its strings are views, valid until the next `sdrkit_presets_dial` on `presets`. Returns `SDRKIT_E_NOT_FOUND` if the
/// data has nothing for the stack in the country, `SDRKIT_E_BAD_ARGUMENT` for a country that is not a code.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_dial(
    presets: Handle,
    query: *const PresetsQuery,
    dial: *mut PresetsDialInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(dial) };
    run(presets, |st| {
        let out = out?;
        // SAFETY: the caller's contract.
        let p = unsafe { preset_of(st, query) }?;
        let network = p.station.as_ref().map_or_else(String::new, |s| s.network.clone());
        st.dial = (p.plan, p.submode.unwrap_or_default(), network);
        out.put(PresetsDialInfo {
            frequency_hz: p.frequency_hz,
            has_width_hz: p.width_hz.is_some(),
            width_hz: p.width_hz.unwrap_or(0.0),
            plan: Str::of(&st.dial.0),
            submode: Str::of(&st.dial.1),
            network: Str::of(&st.dial.2),
            ..PresetsDialInfo::default()
        });
        Ok(())
    })
}

/// Lists the settings that the data has for the stack of `query` in its country, as settings of the stack type in the
/// engine `engine` (the handle of the engine or of an object of it), and writes their number to `count`.
/// `sdrkit_presets_setting_read` reads one; its `arg` goes as it is into the settings of `sdrkit_stack_start`. Returns
/// `SDRKIT_E_NOT_FOUND` if the data has nothing for the stack in the country, `SDRKIT_E_NO_SUCH_KEY` if the engine has
/// no such stack or the stack no setting of a key, and `SDRKIT_E_MALFORMED` if a value of the data does not fit its
/// setting.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_settings(
    presets: Handle,
    engine: Handle,
    query: *const PresetsQuery,
    count_out: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count_out) };
    run(presets, |st| {
        let out = out?;
        let engine = engine_of(engine)?;
        // SAFETY: the caller's contract.
        let p = unsafe { preset_of(st, query) }?;
        st.settings = preset_settings(&engine.host.engine, &p)?;
        out.put(count(st.settings.len()));
        Ok(())
    })
}

/// Writes setting `index` of the last `sdrkit_presets_settings` to `setting`. Its strings are views, valid until the
/// next `sdrkit_presets_settings` on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_setting_read(
    presets: Handle,
    index: u32,
    setting: *mut PresetsSetting,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(setting) };
    run(presets, |st| {
        let out = out?;
        let s = st
            .settings
            .get(index as usize)
            .ok_or_else(|| fault(SDRKIT_E_NO_SUCH_ID, "the index is outside the list"))?;
        out.put(PresetsSetting {
            key: Str::of(&s.key),
            arg: Arg {
                id: u32::from(s.id.0),
                value: Variant::of(&s.value),
            },
            ..PresetsSetting::default()
        });
        Ok(())
    })
}

/// Lists the Meshtastic frequency slots of the preset of `query` in its region, lowest first, as the table `slots` of a
/// running stack `meshtastic` lists them, and writes their number to `count`. The slot of the channel number of the
/// query, else of its channel name, is current; a preset with its own frequency in the region makes the slot of that
/// frequency current, if it is one. `sdrkit_presets_meshtastic_slot_read` reads one. Returns `SDRKIT_E_NOT_FOUND` for a
/// region or a preset that the data does not have, and `SDRKIT_E_OUT_OF_RANGE` for a channel number above the slots of
/// the band.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_meshtastic_slots(
    presets: Handle,
    query: *const PresetsSlotQuery,
    count_out: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (q, out) = unsafe { (sized_in(query), Out::new(count_out)) };
    run(presets, |st| {
        let (q, out) = (q?, out?);
        // SAFETY: the caller's contract: the strings of the query are NUL-terminated or NULL.
        let (region, preset, name) =
            unsafe { (str_in(q.region)?, str_or_null(q.preset)?, str_or_null(q.channel_name)?) };
        st.slots = st.data.meshtastic_slots(
            region,
            preset.unwrap_or(""),
            name.unwrap_or(""),
            q.channel_num,
        )?;
        out.put(count(st.slots.len()));
        Ok(())
    })
}

/// Writes slot `index` of the last `sdrkit_presets_meshtastic_slots` to `slot`. Returns `SDRKIT_E_NO_SUCH_ID` for an
/// index outside the list.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_meshtastic_slot_read(
    presets: Handle,
    index: u32,
    slot: *mut PresetsSlot,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(slot) };
    run(presets, |st| {
        let out = out?;
        let r = st
            .slots
            .get(index as usize)
            .ok_or_else(|| fault(SDRKIT_E_NO_SUCH_ID, "the index is outside the list"))?;
        out.put(PresetsSlot {
            slot: r.slot,
            frequency_hz: r.freq_hz,
            current: r.current,
            preset_default: r.preset_default,
            ..PresetsSlot::default()
        });
        Ok(())
    })
}

/// Writes to `end` whether the stack of `query`, or a part of it, is out of service in its country, the year it ended
/// and one sentence for a person ("3G (UMTS) was switched off in Australia in 2024"): why `sdrkit_presets_channels`
/// lists no channels, or only those of the part that remains. The sentence names the country as the data does and the
/// service by the generation word of the stack's name. The city, the submode and the frequency of the query are not
/// used. Its strings are views, valid until the next `sdrkit_presets_service_end` on `presets`. Returns
/// `SDRKIT_E_BAD_ARGUMENT` for a country that is not a code of two letters.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_presets_service_end(
    presets: Handle,
    query: *const PresetsQuery,
    end: *mut PresetsServiceInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let (q, out) = unsafe { (sized_in(query), Out::new(end)) };
    run(presets, |st| {
        let (q, out) = (q?, out?);
        // SAFETY: the caller's contract: the strings of the query are NUL-terminated or NULL.
        let (stack, country) = unsafe { (str_in(q.stack)?, str_in(q.country)?) };
        let region = Region::new(country).map_err(|e| fault(SDRKIT_E_BAD_ARGUMENT, &e.to_string()))?;
        let name = sdrkit_host_catalog::all()
            .into_iter()
            .find(|d| d.id == stack)
            .map_or(stack, |d| d.name);
        let Some((state, sentence)) = st.data.service_end_why(stack, name, &region) else {
            st.service_end = Default::default();
            out.put(PresetsServiceInfo::default());
            return Ok(());
        };
        st.service_end = (state.ended.clone(), sentence);
        out.put(PresetsServiceInfo {
            out_of_service: true,
            partial: state.remains.is_some(),
            has_year: state.year().is_some(),
            year: state.year().unwrap_or(0),
            ended: Str::of(&st.service_end.0),
            sentence: Str::of(&st.service_end.1),
            ..PresetsServiceInfo::default()
        });
        Ok(())
    })
}
