//! What the driver learns from a server and publishes: the lists and limits of the server, the readings, the tables and the
//! waterfalls, and the backoff wait. One set per server address, kept across connections.

use crate::proto::{Band, Decode, Mode};
use crate::radio::Opts;
use crate::*;
use sdrkit_core::{
    Axis, ChoiceDecl, FieldDecl, Raster, SettingDecl, SettingId, StateBlob, StateError, Value,
};
use sdrkit_driver_core::RadioSetting;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// The most rows of a list of the server.
pub(crate) const MAX_LIST: usize = 2000;
/// The most decodes kept.
pub(crate) const MAX_DECODES: usize = 500;
/// The most digital voice values kept.
pub(crate) const MAX_METADATA: usize = 64;
/// The lines a waterfall keeps.
pub(crate) const WATERFALL_ROWS: usize = 64;

/// The waits of the backoff of a server.
#[derive(Debug, Default)]
pub(crate) struct Backoff {
    level: u32,
    until: Option<Instant>,
}

/// The last lines of a waterfall. The buffers of old lines are used again, so a line costs no allocation once the waterfall
/// is full.
#[derive(Debug, Default)]
pub(crate) struct Lines {
    rows: VecDeque<Vec<f32>>,
    /// The frequency of the first bin.
    start_hz: f64,
    /// The width of a bin.
    step_hz: f64,
    ver: u64,
}

impl Lines {
    /// Adds a line of `bins`, the first at `start_hz`, `step_hz` apart.
    pub fn push(&mut self, bins: &[f32], start_hz: f64, step_hz: f64) {
        let mut row = if self.rows.len() >= WATERFALL_ROWS {
            self.rows.pop_front().unwrap_or_default()
        } else {
            Vec::with_capacity(bins.len())
        };
        row.clear();
        row.extend_from_slice(bins);
        self.rows.push_back(row);
        (self.start_hz, self.step_hz) = (start_hz, step_hz);
        self.ver += 1;
    }

    fn write(&self, def: &RasterDef, r: &mut Results) {
        let Some(id) = r.decl().raster_id(def.key) else {
            return;
        };
        let width = self.rows.back().map_or(0, Vec::len);
        r.update_raster(id, |ras: &mut Raster| {
            ras.width = width;
            ras.height = self.rows.len();
            ras.x = Axis::new(self.start_hz, self.step_hz, def.x_unit);
            ras.y = Axis::new(0.0, 1.0, def.y_unit);
            def.unit.clone_into(&mut ras.unit);
            ras.values.clear();
            for line in &self.rows {
                let n = line.len().min(width);
                ras.values.extend_from_slice(&line[..n]);
                ras.values.resize(ras.values.len() + width - n, f32::NAN);
            }
            true
        });
    }
}

/// The readings of a server, as it last reported them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Readings {
    pub server: Option<String>,
    pub profile: Option<String>,
    pub profile_name: Option<String>,
    pub band: Option<Band>,
    pub listeners: Option<i64>,
    pub max_listeners: Option<i64>,
    pub smeter_db: Option<f64>,
    pub cpu_pct: Option<f64>,
    pub temperature_c: Option<f64>,
    pub receiver: Option<String>,
    pub location: Option<String>,
    pub height_m: Option<f64>,
    pub admin: Option<String>,
    pub position: Option<(f64, f64)>,
    pub decoder_line: Option<String>,
    pub server_error: Option<String>,
    pub kbit_s: Option<f64>,
    pub audio_sps: Option<f64>,
    pub lines_s: Option<f64>,
    pub bytes_in: Option<i64>,
    pub audio_adpcm: Option<bool>,
    pub audio_sync: Option<bool>,
    pub waterfall_adpcm: Option<bool>,
    pub fft_bins: Option<i64>,
    pub waterfall_min_db: Option<f64>,
    pub waterfall_max_db: Option<f64>,
}

/// One bookmark.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BookmarkRow {
    pub name: String,
    pub freq: f64,
    pub mode: String,
}

/// The information the driver holds about one server.
#[derive(Debug, Default)]
pub(crate) struct View {
    /// The profiles of the server: (id, name).
    pub profiles: Vec<(String, String)>,
    /// The band of each profile the driver has seen.
    pub bands: BTreeMap<String, Band>,
    /// The mode list of the server, once it sent one.
    pub modes: Option<Vec<Mode>>,
    /// The features of the server, once it sent them.
    pub features: Option<BTreeMap<String, bool>>,
    /// Modes whose demodulator failed on the server.
    pub unavailable: BTreeSet<String>,
    /// The readings.
    pub r: Readings,
    /// The bookmarks.
    pub bookmarks: Vec<BookmarkRow>,
    /// The width B of the `iq` delivery when it is in use, for the tune column of the bookmarks. 0 in the `audio` delivery.
    pub iq_width: f64,
    /// The dial frequencies: (mode, frequency).
    pub dials: Vec<(String, f64)>,
    /// The last decodes: (number, time, decode).
    pub decodes: VecDeque<(u64, f64, Decode)>,
    /// The number of the last decode.
    pub decode_seq: u64,
    /// The digital voice values.
    pub metadata: Vec<(String, String)>,
    /// The waterfall of the band.
    pub waterfall: Lines,
    /// The spectrum of the decoder.
    pub decoder_fft: Lines,
    /// The last frequency the driver tuned.
    pub last_f: Option<f64>,
    /// A frequency that a retune was refused for.
    pub refused_f: Option<f64>,
    bookmarks_ver: u64,
    dials_ver: u64,
    metadata_ver: u64,
    profiles_ver: u64,
    modes_ver: u64,
}

/// The parameters the driver requests from the server's demodulator.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ModePlan {
    /// The demodulator.
    pub modulation: String,
    /// The decoder.
    pub secondary: Option<String>,
    /// The passband of the mode in the list of the server.
    pub bandpass: Option<(f64, f64)>,
}

fn refused(s: RunTime) -> DriverError {
    DriverError::Refused { id: run_time_id(s) }
}

/// The owner tag of the saved state of an OpenWebRX radio.
const STATE_OWNER: &str = "openwebrx";
/// The format version of the saved state.
const STATE_VERSION: u32 = 1;

/// The saved state: the profiles (id, name) and the band (id, centre, width) of each profile seen.
#[derive(serde::Serialize, serde::Deserialize)]
struct Saved {
    profiles: Vec<(String, String)>,
    bands: Vec<(String, f64, f64)>,
}

/// How the driver chooses the profile of the server.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Profile {
    /// The driver never changes the profile.
    Keep,
    /// The driver switches to a profile that covers the frequency.
    Follow,
    /// The driver selects this profile, `<sdr id>|<profile id>`.
    Named(String),
}

/// The settings that an open radio adds, in their order. The profile comes first because the server can send its mode list
/// later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunTime {
    /// The profile rule, or a named profile.
    Profile,
    /// The demodulator of the server.
    ServerMode,
    /// The decoder of the server.
    Secondary,
}

/// The id in a radio's schema of run-time setting `s`.
pub(crate) fn run_time_id(s: RunTime) -> SettingId {
    DESCRIPTOR.run_time_id(s as usize)
}

/// The lists of a server that the run-time settings choose from: (id, name) pairs in the order of the server.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Lists {
    /// The profiles.
    pub profiles: Vec<(String, String)>,
    /// The demodulators, `None` before the server sent its mode list.
    pub demods: Option<Vec<(String, String)>>,
    /// The decoders.
    pub decoders: Vec<(String, String)>,
}

/// The demodulator that a radio starts with.
const START_MODE: &str = "usb";

fn choices<'a>(
    fixed: &[(&'static str, &'static str)],
    list: impl Iterator<Item = &'a (String, String)>,
) -> Vec<ChoiceDecl> {
    fixed
        .iter()
        .map(|(k, l)| ChoiceDecl::new(k, l))
        .chain(list.map(|(k, l)| ChoiceDecl {
            key: Cow::Owned(k.clone()),
            label: Cow::Owned(l.clone()),
        }))
        .collect()
}

impl Lists {
    /// The run-time settings: the profile, then the demodulator and the decoder once the mode list is known.
    pub fn settings(&self) -> Vec<SettingDecl> {
        let profile = FieldDecl::with_choices(
            "profile",
            "Profile",
            choices(
                &[
                    ("keep", "Keep the server's"),
                    ("follow", "Follow the tuning"),
                ],
                self.profiles.iter(),
            ),
        );
        let mut out = vec![SettingDecl::new(profile, Value::Choice(0)).with_help(
            "Keep the server's never changes it. Follow the tuning switches to a profile that covers the frequency. A named \
             profile is selected. A change of profile changes the band for every listener of the receiver.",
        )];
        if let Some(demods) = &self.demods {
            let at = demods
                .iter()
                .position(|(id, _)| id == START_MODE)
                .unwrap_or(0);
            let field = FieldDecl::with_choices(
                "servermode",
                "Server demodulator",
                choices(&[], demods.iter()),
            );
            out.push(SettingDecl::new(field, Value::Choice(at as u16)).with_help(
                "The demodulator of the server for Server demodulates. A demodulator that the server cannot run now is refused.",
            ));
            let field = FieldDecl::with_choices(
                "secondary",
                "Server decoder",
                choices(&[("", "Off")], self.decoders.iter()),
            );
            out.push(SettingDecl::new(field, Value::Choice(0)).with_help(
                "The server decodes FT8, WSPR, PSK and others itself, and sdrkit shows its decodes. A decoder that needs a \
                 demodulator other than the one in use is refused.",
            ));
        }
        out
    }

    fn index(v: &Values, s: RunTime) -> Option<usize> {
        match v.value(run_time_id(s)) {
            Some(Value::Choice(i)) => Some(usize::from(*i)),
            _ => None,
        }
    }

    /// The profile rule of `v`. [`Profile::Keep`] when `v` has no profile setting.
    pub fn profile(&self, v: &Values) -> Profile {
        match Self::index(v, RunTime::Profile) {
            Some(1) => Profile::Follow,
            Some(i) if i >= 2 => self
                .profiles
                .get(i - 2)
                .map_or(Profile::Keep, |(id, _)| Profile::Named(id.clone())),
            _ => Profile::Keep,
        }
    }

    /// The demodulator of `v`. The start demodulator when `v` has none.
    pub fn servermode(&self, v: &Values) -> String {
        Self::index(v, RunTime::ServerMode)
            .and_then(|i| self.demods.as_ref()?.get(i))
            .map_or_else(|| START_MODE.to_string(), |(id, _)| id.clone())
    }

    /// The decoder of `v`. `None` for off.
    pub fn secondary(&self, v: &Values) -> Option<String> {
        let i = Self::index(v, RunTime::Secondary)?;
        self.decoders
            .get(i.checked_sub(1)?)
            .map(|(id, _)| id.clone())
    }
}

impl View {
    /// What the driver learnt about the server that a new radio of the same address needs: the profiles and their bands.
    pub fn save(&self) -> Option<StateBlob> {
        let saved = Saved {
            profiles: self.profiles.clone(),
            bands: self
                .bands
                .iter()
                .map(|(id, b)| (id.clone(), b.centre, b.rate))
                .collect(),
        };
        Some(StateBlob::new(
            STATE_OWNER,
            STATE_VERSION,
            serde_json::to_vec(&saved).ok()?,
        ))
    }

    /// Takes what [`View::save`] kept. The profiles and bands that the server sends later replace them.
    ///
    /// # Errors
    ///
    /// [`StateError`] if the state is not of this driver, or does not decode.
    pub fn restore(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let saved: Saved = serde_json::from_slice(state.open(STATE_OWNER, STATE_VERSION)?)
            .map_err(|_| StateError::Damaged)?;
        self.set_profiles(saved.profiles);
        for (id, centre, rate) in saved.bands {
            self.set_band(id, Band { centre, rate });
        }
        Ok(())
    }

    /// The lists that the run-time settings choose from.
    pub fn lists(&self) -> Lists {
        let pairs = |decoder: bool| -> Vec<(String, String)> {
            self.modes
                .iter()
                .flatten()
                .filter(|m| m.is_decoder() == decoder)
                .map(|m| (m.id.clone(), m.name.clone()))
                .collect()
        };
        Lists {
            profiles: self.profiles.clone(),
            demods: self.modes.as_ref().map(|_| pairs(false)),
            decoders: pairs(true),
        }
    }

    /// The server can run `m` now.
    pub fn available(&self, m: &Mode) -> bool {
        m.available(self.features.as_ref()) && !self.unavailable.contains(&m.id)
    }

    /// The name of profile `id`.
    pub fn profile_name(&self, id: &str) -> String {
        self.profiles
            .iter()
            .find(|(i, _)| i == id)
            .map_or_else(|| id.to_string(), |(_, n)| n.clone())
    }

    /// The server lists profile `id`, or has not sent its list.
    pub fn has_profile(&self, id: &str) -> bool {
        self.profiles.is_empty() || self.profiles.iter().any(|(i, _)| i == id)
    }

    /// The demodulator and the decoder for `o`. A mode is checked only against a list the server sent.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] of `servermode` for a demodulator that the server does not list or cannot run, and of
    /// `secondary` for such a decoder, or one that needs another demodulator.
    pub fn plan(&self, o: &Opts) -> Result<ModePlan, DriverError> {
        let modulation = if o.delivery == Delivery::Audio {
            o.servermode.clone()
        } else {
            "usb".to_string()
        };
        let find = |id: &str| self.modes.as_ref().map(|l| l.iter().find(|m| m.id == id));
        if o.delivery == Delivery::Audio {
            if let Some(found) = find(&modulation) {
                if !found.is_some_and(|m| !m.is_decoder() && self.available(m)) {
                    return Err(refused(RunTime::ServerMode));
                }
            }
        }
        let secondary = o.secondary.clone();
        if let Some(s) = &secondary {
            if let Some(found) = find(s) {
                match found {
                    Some(m) if m.is_decoder() && self.available(m) => {
                        if !m.underlying.contains(&modulation) {
                            return Err(refused(RunTime::Secondary));
                        }
                    }
                    _ => return Err(refused(RunTime::Secondary)),
                }
            }
        }
        let bandpass = find(&modulation).flatten().and_then(|m| m.bandpass);
        Ok(ModePlan {
            modulation,
            secondary,
            bandpass,
        })
    }

    /// Refuses settings that the server is known not to take.
    ///
    /// # Errors
    ///
    /// What [`View::plan`] returns; [`DriverError::Refused`] of `profile` for a named profile the server does not list,
    /// of `rate` for an exact rate other than the audio rate, and of `bias_tee` when it is on.
    pub fn check(&self, o: &Opts) -> Result<(), DriverError> {
        self.plan(o)?;
        if matches!(&o.profile, Profile::Named(p) if !self.has_profile(p)) {
            return Err(refused(RunTime::Profile));
        }
        o.check()
    }

    /// Takes the profile list of the server. Returns whether it changed.
    pub fn set_profiles(&mut self, list: Vec<(String, String)>) -> bool {
        if list == self.profiles {
            return false;
        }
        self.profiles = list;
        if !self.profiles.is_empty() {
            let p = &self.profiles;
            self.bands.retain(|id, _| p.iter().any(|(i, _)| i == id));
        }
        self.profiles_ver += 1;
        true
    }

    /// Keeps the band of profile `id`.
    pub fn set_band(&mut self, id: String, band: Band) {
        if self.bands.get(&id) != Some(&band) {
            self.bands.insert(id, band);
            self.profiles_ver += 1;
        }
    }

    /// Takes the mode list of the server.
    pub fn set_modes(&mut self, modes: Vec<Mode>) {
        self.modes = Some(modes);
        self.modes_ver += 1;
    }

    /// Takes the feature list of the server.
    pub fn set_features(&mut self, f: BTreeMap<String, bool>) {
        self.features = Some(f);
        self.modes_ver += 1;
    }

    /// Marks modes as not available.
    pub fn mark_unavailable(&mut self, ids: impl IntoIterator<Item = String>) {
        self.unavailable.extend(ids);
        self.modes_ver += 1;
    }

    /// Takes the bookmark list.
    pub fn set_bookmarks(&mut self, b: Vec<BookmarkRow>) {
        self.bookmarks = b;
        self.bookmarks_ver += 1;
    }

    /// Takes the dial frequency list.
    pub fn set_dials(&mut self, d: Vec<(String, f64)>) {
        self.dials = d;
        self.dials_ver += 1;
    }

    /// Takes the digital voice values.
    pub fn set_metadata(&mut self, m: Vec<(String, String)>) {
        self.metadata = m;
        self.metadata_ver += 1;
    }

    /// Adds a decode that came at `time`.
    pub fn add_decode(&mut self, time: f64, d: Decode) {
        self.decode_seq += 1;
        if self.decodes.len() >= MAX_DECODES {
            self.decodes.pop_front();
        }
        self.decodes.push_back((self.decode_seq, time, d));
    }
}

/// What a source wrote into its results last: the version of each part.
#[derive(Debug, Default)]
pub(crate) struct Written {
    bookmarks: u64,
    dials: u64,
    decode: u64,
    metadata: u64,
    profiles: u64,
    modes: u64,
    waterfall: u64,
    decoder_fft: u64,
    iq_width: f64,
}

/// What the driver keeps per server address.
#[derive(Debug, Default)]
pub(crate) struct AddrState {
    pub backoff: Mutex<Backoff>,
    pub view: Mutex<View>,
    lost: AtomicU64,
    lost_events: AtomicU64,
}

fn table<'a>(r: &'a mut Results, def: &TableDef) -> Option<sdrkit_core::TableMut<'a>> {
    let id = r.decl().table_id(def.key)?;
    r.table_mut(id)
}

impl AddrState {
    /// Waits out the backoff in steps of at most 100 ms. Returns false when the abort fired.
    pub fn wait_backoff(&self, abort: &sdrkit_driver_core::net::Abort) -> bool {
        loop {
            let until = lock(&self.backoff).until;
            let Some(u) = until else { return true };
            let now = Instant::now();
            if now >= u {
                return true;
            }
            if abort.fired() {
                return false;
            }
            std::thread::sleep((u - now).min(Duration::from_millis(100)));
        }
    }

    /// One more busy answer in a row: the wait starts at the base and doubles up to the longest wait.
    pub fn back_off(&self, t: &Timing) -> Duration {
        let mut b = lock(&self.backoff);
        b.level += 1;
        let wait = t
            .backoff_base
            .saturating_mul(1u32 << (b.level - 1).min(20))
            .min(t.backoff_max);
        b.until = Some(Instant::now() + wait);
        wait
    }

    /// A connection got its first `config`: the next wait starts at the base again.
    pub fn reset_backoff(&self) {
        *lock(&self.backoff) = Backoff::default();
    }

    /// Counts `samples` lost to a resync of the audio.
    pub fn add_lost(&self, samples: u64) {
        self.lost.fetch_add(samples, Ordering::Relaxed);
        self.lost_events.fetch_add(1, Ordering::Relaxed);
    }

    /// The samples lost to resyncs, and how many resyncs.
    pub fn resync_lost(&self) -> (u64, u64) {
        (
            self.lost.load(Ordering::Relaxed),
            self.lost_events.load(Ordering::Relaxed),
        )
    }

    /// Writes the readings, and the tables and rasters that changed since `w`, into `r`.
    pub fn write(&self, r: &mut Results, w: &mut Written) {
        let v = lock(&self.view);
        let x = &v.r;
        let lost = i64::try_from(self.lost.load(Ordering::Relaxed)).unwrap_or(i64::MAX);
        {
            let mut f = r.fields_mut();
            f.opt(&SERVER, x.server.as_deref());
            f.opt(&PROFILE_IN_USE, x.profile.as_deref());
            f.opt(&PROFILE_NAME, x.profile_name.as_deref());
            f.opt(&BAND_LOW, x.band.map(|b| b.lo()));
            f.opt(&BAND_HIGH, x.band.map(|b| b.hi()));
            f.opt(&LISTENERS, x.listeners);
            f.opt(&MAX_LISTENERS, x.max_listeners);
            f.opt(&SMETER, x.smeter_db);
            f.opt(&CPU, x.cpu_pct);
            f.opt(&TEMPERATURE, x.temperature_c);
            f.opt(&RECEIVER, x.receiver.as_deref());
            f.opt(&LOCATION, x.location.as_deref());
            f.opt(&HEIGHT, x.height_m);
            f.opt(&ADMIN, x.admin.as_deref());
            f.opt(&DECODER_LINE, x.decoder_line.as_deref());
            f.opt(&SERVER_ERROR, x.server_error.as_deref());
            f.opt(&KBIT_S, x.kbit_s);
            f.opt(&AUDIO_SPS, x.audio_sps);
            f.opt(&LINES_S, x.lines_s);
            f.opt(&BYTES_IN, x.bytes_in);
            f.opt(&AUDIO_ADPCM, x.audio_adpcm);
            f.opt(&AUDIO_SYNC, x.audio_sync);
            f.opt(&WATERFALL_ADPCM, x.waterfall_adpcm);
            f.opt(&FFT_BINS, x.fft_bins);
            f.opt(&WATERFALL_MIN, x.waterfall_min_db);
            f.opt(&WATERFALL_MAX, x.waterfall_max_db);
            f.set(&RESYNC_LOST, lost);
        }
        if let Some((lat, lon)) = x.position {
            let label = x.receiver.as_deref().unwrap_or("Receiver");
            r.update_point(RECEIVER_POINT, lat, lon, label, None);
        }
        let level = x.smeter_db.map(|d| d as f32);
        r.update_status(|s| {
            let changed = s.level_dbfs != level;
            s.level_dbfs = level;
            changed
        });
        if w.bookmarks != v.bookmarks_ver || w.iq_width != v.iq_width {
            (w.bookmarks, w.iq_width) = (v.bookmarks_ver, v.iq_width);
            if let Some(mut t) = table(r, &BOOKMARKS) {
                t.clear();
                for b in &v.bookmarks {
                    let mut row = t.row_mut(&format!("{}|{}", proto::json_num(b.freq), b.name));
                    row.set(&B_NAME, &b.name);
                    row.set(&B_FREQ, b.freq);
                    row.set(&B_MODE, &b.mode);
                    row.set(&B_TUNE, b.freq + v.iq_width / 2.0);
                }
            }
        }
        if w.dials != v.dials_ver {
            w.dials = v.dials_ver;
            if let Some(mut t) = table(r, &DIAL_FREQUENCIES) {
                t.clear();
                for (m, f) in &v.dials {
                    let mut row = t.row_mut(&format!("{m}|{}", proto::json_num(*f)));
                    row.set(&D_MODE, m);
                    row.set(&D_FREQ, *f);
                }
            }
        }
        if w.decode != v.decode_seq {
            let from = w.decode;
            w.decode = v.decode_seq;
            if let Some(mut t) = table(r, &DECODES) {
                for (n, time, d) in v.decodes.iter().filter(|(n, ..)| *n > from) {
                    let mut row = t.row_mut(&format!("{n:012}"));
                    row.set(&C_TIME, *time);
                    row.set(&C_MODE, &d.mode);
                    row.opt(&C_FREQ, d.freq);
                    row.opt(&C_LEVEL, d.db);
                    row.opt(&C_OFFSET, d.dt);
                    row.set(&C_MESSAGE, &d.message);
                    row.set(&C_REST, &d.rest);
                }
            }
        }
        if w.metadata != v.metadata_ver {
            w.metadata = v.metadata_ver;
            if let Some(mut t) = table(r, &METADATA) {
                t.clear();
                for (k, x) in &v.metadata {
                    let mut row = t.row_mut(k);
                    row.set(&V_NAME, k);
                    row.set(&V_VALUE, x);
                }
            }
        }
        if w.profiles != v.profiles_ver {
            w.profiles = v.profiles_ver;
            if let Some(mut t) = table(r, &PROFILES) {
                t.clear();
                let listed = v.profiles.iter().map(|(i, n)| (i, n.as_str()));
                let seen = v
                    .bands
                    .keys()
                    .filter(|k| !v.profiles.iter().any(|(i, _)| i == *k))
                    .map(|k| (k, k.as_str()));
                for (id, name) in listed.chain(seen) {
                    let band = v.bands.get(id);
                    let mut row = t.row_mut(id);
                    row.set(&P_ID, id);
                    row.set(&P_NAME, name);
                    row.opt(&P_LOW, band.map(Band::lo));
                    row.opt(&P_HIGH, band.map(Band::hi));
                }
            }
        }
        if w.modes != v.modes_ver {
            w.modes = v.modes_ver;
            if let Some(mut t) = table(r, &MODES) {
                t.clear();
                for m in v.modes.iter().flatten() {
                    let mut row = t.row_mut(&m.id);
                    row.set(&M_ID, &m.id);
                    row.set(&M_NAME, &m.name);
                    row.set(&M_DECODER, m.is_decoder());
                    row.set(&M_UNDERLYING, &m.underlying.join(","));
                    row.set(&M_AVAILABLE, v.available(m));
                }
            }
        }
        if w.waterfall != v.waterfall.ver {
            w.waterfall = v.waterfall.ver;
            v.waterfall.write(&WATERFALL, r);
        }
        if w.decoder_fft != v.decoder_fft.ver {
            w.decoder_fft = v.decoder_fft.ver;
            v.decoder_fft.write(&DECODER_WATERFALL, r);
        }
    }
}

/// A refusal of the standard setting `s`.
pub(crate) fn standard(s: RadioSetting) -> DriverError {
    DriverError::Refused { id: s.id() }
}
