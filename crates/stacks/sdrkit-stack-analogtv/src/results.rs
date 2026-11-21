//! The option types of the results of the three stacks, and the writers of the status snapshots of `atv` and `fpv`.

use crate::atv::supervisor::AtvDetail;
use crate::fpv::FpvDetail;
use crate::typed::{
    AtvChannelRow, AtvFields, FpvChannelRow, FpvFields, TeletextFields, ATV_CHANNELS, ATV_SERIES,
    FPV_CHANNELS, FPV_SERIES,
};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::{Published, TableDef};
use sdrkit_core::{ResultsDecl, Series, SeriesId, TableId};
use sdrkit_proto_mpegts::publish::{TELETEXT_PAGE_TABLE, TELETEXT_SUBPAGE_TABLE};
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// How far a receiver has come.
    #[derive(Default)]
    pub enum State {
        /// No signal found.
        #[default]
        Searching = ("searching", "Searching"),
        /// A signal and its line sync, or a transmitter, found.
        Sync = ("sync", "Synchronising"),
        /// Locked and decoding.
        Locked = ("locked", "Locked"),
    }
}

impl State {
    /// The state of a number 0, 1 or 2.
    pub fn of(n: u8) -> State {
        match n {
            0 => State::Searching,
            1 => State::Sync,
            _ => State::Locked,
        }
    }
    /// The lock state of the state.
    pub fn lock_state(self) -> sdrkit_core::LockState {
        match self {
            State::Searching => sdrkit_core::LockState::Searching,
            State::Sync => sdrkit_core::LockState::Tracking,
            State::Locked => sdrkit_core::LockState::Locked,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The vision detector in use.
    #[derive(Default)]
    pub enum DetectorUsed {
        /// The synchronous detector.
        Synchronous = ("synchronous", "synchronous"),
        /// The envelope.
        #[default]
        Envelope = ("envelope", "envelope"),
        /// The FM discriminator.
        FmDiscriminator = ("fm", "FM discriminator"),
    }
}

sdrkit_core::choice_enum! {
    /// The sound that plays.
    #[derive(Default)]
    pub enum SoundPlaying {
        /// The FM carrier.
        #[default]
        Fm = ("fm", "FM"),
        /// The AM carrier.
        Am = ("am", "AM"),
        /// The FM carrier, with the A2 pilot saying mono.
        A2Mono = ("a2_mono", "FM mono (A2 pilot)"),
        /// A2 stereo.
        A2Stereo = ("a2_stereo", "A2 stereo"),
        /// A2 two languages, the first.
        A2DualA = ("a2_dual_a", "A2 dual, sound A"),
        /// A2 two languages, the second.
        A2DualB = ("a2_dual_b", "A2 dual, sound B"),
        /// NICAM stereo.
        NicamStereo = ("nicam_stereo", "NICAM stereo"),
        /// NICAM two mono channels, the first.
        NicamDual1 = ("nicam_dual_1", "NICAM dual mono, channel 1"),
        /// NICAM two mono channels, the second.
        NicamDual2 = ("nicam_dual_2", "NICAM dual mono, channel 2"),
        /// NICAM mono.
        NicamMono = ("nicam_mono", "NICAM mono"),
        /// No sound carrier found.
        NoneFound = ("none_found", "none found"),
        /// The sound is not decoded (FM video).
        NotDecoded = ("not_decoded", "not decoded"),
        /// NBTV: the audio is the picture.
        Picture = ("picture", "none (the audio is the picture)"),
    }
}

sdrkit_core::choice_enum! {
    /// The polarity of FM video.
    #[derive(Default)]
    pub enum FmPolarity {
        /// Not known yet.
        #[default]
        Trying = ("trying", "trying"),
        /// The sync tip is the lowest frequency.
        TipLow = ("tip_low", "sync tip is the lowest frequency"),
        /// The sync tip is the highest frequency.
        TipHigh = ("tip_high", "sync tip is the highest frequency"),
    }
}

sdrkit_core::choice_enum! {
    /// What the A2 pilot says.
    pub enum A2Mode {
        /// Mono.
        Mono = ("mono", "mono"),
        /// Stereo.
        Stereo = ("stereo", "stereo"),
        /// Two languages.
        Dual = ("dual", "dual"),
    }
}

sdrkit_core::choice_enum! {
    /// The picture's aspect ratio.
    pub enum Aspect {
        /// 4:3.
        Normal = ("4:3", "4:3"),
        /// 16:9.
        Wide = ("16:9", "16:9"),
    }
}

sdrkit_core::choice_enum! {
    /// The conditional access system of a MAC channel.
    pub enum CaSystem {
        /// EuroCrypt.
        EuroCrypt = ("eurocrypt", "EuroCrypt"),
        /// Another system.
        Other = ("other", "conditional access"),
    }
}

sdrkit_core::choice_enum! {
    /// How the MAC sound is coded.
    #[derive(Default)]
    pub enum SoundCoding {
        /// Companded 14/10 bits at 32 kHz with first-level protection.
        #[default]
        Companded = ("companded", "companded 14/10, 32 kHz, first-level protection"),
        /// Scrambled, so not played.
        Scrambled = ("scrambled", "scrambled (not played)"),
    }
}

/// The writer of the supervisor: the `atv` fields, the channel table and the series.
pub fn atv_published() -> Published {
    let mut d = ResultsDecl::new()
        .with_fields(AtvFields::SHAPE.fields)
        .with_table(&ATV_CHANNELS);
    for s in &ATV_SERIES {
        d = d.with_series(s);
    }
    Published::from_decl(Arc::new(d))
}

/// The writer of the teletext decoder: its fields and the teletext tables.
pub fn teletext_published() -> Published {
    Published::from_decl(Arc::new(
        ResultsDecl::new()
            .with_fields(TeletextFields::SHAPE.fields)
            .with_table(&TELETEXT_PAGE_TABLE)
            .with_table(&TELETEXT_SUBPAGE_TABLE),
    ))
}

/// Writes the table `channels` from `channels`, in place of its rows.
pub fn write_channels(p: &mut Published, channels: &[crate::channels::Channel]) {
    p.clear_rows(ATV_CHANNELS.def());
    for c in channels {
        let row = AtvChannelRow {
            label: &c.label,
            vision: Hertz(c.hz),
        };
        ATV_CHANNELS.put(p, &c.label, &row);
    }
}

/// The id of table `t`, which `p` declares.
///
/// # Panics
///
/// If `p` does not declare `t`: the writers of this crate declare every table they name.
pub fn table_id(p: &Published, t: &TableDef) -> TableId {
    p.results()
        .decl()
        .table_id(t.key)
        .expect("the writer declares its tables")
}

/// The id of series `key` of `p`.
pub fn series_id(p: &Published, key: &str) -> Option<SeriesId> {
    p.results().decl().series_id(key)
}

/// Writes `points` as series `id` of `p`, a line, if they changed.
pub fn write_series(p: &mut Published, id: Option<SeriesId>, points: &[[f32; 2]]) {
    if let Some(id) = id {
        p.results_mut().update_series(id, |s: &mut Series| {
            let changed = !s.line || s.points != points;
            if changed {
                s.points.clear();
                s.points.extend_from_slice(points);
                s.line = true;
            }
            changed
        });
    }
}

/// Writes the `atv` snapshot `d` as its fields.
pub fn publish_atv(p: &mut Published, d: &AtvDetail) {
    let nicam = d.nicam.as_ref();
    p.put(&AtvFields {
        state: d.state,
        system: (!d.system.is_empty()).then_some(d.system.as_str()),
        colour_system: d.colour_system,
        colour: d.colour,
        colour_killer: d.colour_killer,
        lines: i64::from(d.lines),
        field_rate: Hertz(d.field_hz),
        line_rate: Hertz(d.line_hz),
        line_error_ppm: d.line_error_ppm,
        detector: d.detector,
        modulation: d.modulation,
        vision_carrier: Hertz(d.vision_hz),
        sound_carrier: (d.sound_hz != 0.0).then_some(Hertz(d.sound_hz)),
        sound_spacing: Hertz(d.sound_spacing_mhz * 1e6),
        offset_hz: Hertz(d.carrier_offset_hz),
        level_dbfs: d.carrier_dbfs,
        carrier_to_noise_db: d.carrier_to_noise_db,
        snr_db: d.video_snr_db,
        sound_present: d.sound_present,
        sound_deviation: Hertz(d.sound_deviation_khz * 1e3),
        audio_level_dbfs: d.sound_level_dbfs,
        sound_system: d.sound_system,
        sync_found_pct: 100.0 * d.sync_quality,
        line_count: d.line_count as i64,
        field_count: d.field_count as i64,
        pictures: d.pictures as i64,
        snow_pictures: d.snow_pictures as i64,
        last_field: i64::from(d.last_field),
        fields_ok: d.fields_ok as i64,
        fields_bad: d.fields_bad as i64,
        sync_depth_pct: d.sync_depth_pct,
        sync_compression_pct: d.sync_compression_pct,
        white_peak: d.white_peak,
        burst_pct: 100.0 * d.burst_level,
        burst_phase_error_deg: d.burst_phase_error_deg,
        chroma_level_pct: 100.0 * d.chroma_level,
        secam_identification: (!d.secam_identification.is_empty())
            .then_some(d.secam_identification),
        fm_polarity: d.fm_polarity,
        nbtv_line_locked: d.nbtv_line_locked,
        nbtv_picture_locked: d.nbtv_picture_locked,
        nicam_locked: nicam.map(|n| n.locked),
        nicam_valid: nicam.map(|n| n.valid),
        nicam_mode: nicam.and_then(|n| n.mode),
        nicam_parity_errors: nicam.map(|n| n.parity_errors as i64),
        nicam_error_pct: nicam.map(|n| 100.0 * n.error_rate),
        a2_mode: d.a2.as_ref().and_then(|a| a.mode),
    });
}

/// The writer of the `fpv` display.
pub fn fpv_published() -> Published {
    let mut d = ResultsDecl::new()
        .with_fields(FpvFields::SHAPE.fields)
        .with_table(&FPV_CHANNELS);
    for s in &FPV_SERIES {
        d = d.with_series(s);
    }
    Published::from_decl(Arc::new(d))
}

/// Writes the `fpv` snapshot `d` as its fields and its channel table.
pub fn publish_fpv(p: &mut Published, d: &FpvDetail) {
    let m = &d.meter;
    p.put(&FpvFields {
        state: State::of(d.state),
        channel: (!d.names.is_empty()).then_some(d.names.as_str()),
        level_dbfs: m.level_dbfs,
        over_noise_db: m.over_db,
        noise_dbfs: m.floor_dbfs,
        offset_hz: Hertz(m.offset_hz),
        system: (!d.system.is_empty()).then_some(d.system.as_str()),
        colour_system: d.colour_system,
        lines: i64::from(d.lines),
        field_rate: Hertz(d.field_hz),
        sync_found_pct: 100.0 * d.sync_quality,
        pictures: d.pictures as i64,
        fields_ok: d.fields_ok as i64,
        fields_bad: d.fields_bad as i64,
        polarity: d.polarity,
        snr_db: d.video_snr_db,
        sound_present: d.sound.present,
        sound_subcarrier: d
            .sound
            .present
            .then_some(Hertz(d.sound.subcarrier_mhz * 1e6)),
        subcarrier_60_db: d.sound.s60_db,
        subcarrier_65_db: d.sound.s65_db,
    });
    for row in &m.table {
        let r = FpvChannelRow {
            name: &row.name,
            band: &row.band,
            number: i64::from(row.number),
            freq: Hertz(row.mhz * 1e6),
            level_dbfs: row.level_dbfs,
            over_noise_db: row.over_db,
            present: row.present,
            inside: row.inside,
            heard: row.heard_s.map(Seconds),
        };
        FPV_CHANNELS.put(p, &row.name, &r);
    }
}
