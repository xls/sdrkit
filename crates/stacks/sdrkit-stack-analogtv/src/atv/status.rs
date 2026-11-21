//! The status of the `atv` stack: the results, the log, the scopes and the channel table. The supervisor sends the
//! snapshot of each report and its log lines ([`StatusItem`]); the counts of the picture decoder are added here, so the
//! component runs after the picture decoder.

use super::supervisor::{summary_line, AtvDetail};
use super::{Modulation, PictureReport};
use crate::channels::{Channel, ATV_CHANNELS_CAP};
use crate::results::{atv_published, publish_atv, series_id, write_channels, write_series};
use crate::settings::CHANNELS;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    ConfigError, Results, SeriesId, SettingError, SettingId, SettingSet, Status, Value, Values,
};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Item, Port};
use std::sync::Arc;

/// One report of the supervisor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AtvReport {
    /// The snapshot, without the counts of the picture decoder and without the summary.
    pub detail: AtvDetail,
    /// The log lines of the other components since the last report.
    pub log: Vec<String>,
    /// The line scope. An empty scope or spectrum is not written.
    pub line: Arc<[[f32; 2]]>,
    /// The field-blanking scope.
    pub vbi: Arc<[[f32; 2]]>,
    /// The spectrum.
    pub spectrum: Arc<[[f32; 2]]>,
}

/// What the supervisor sends to the status component.
#[derive(Clone, Debug, PartialEq)]
pub enum StatusItem {
    /// A log line.
    Log(String),
    /// A report.
    Report(Box<AtvReport>),
}

impl Item for StatusItem {
    const KIND: &'static str = "atv.status_item";
}

sdrkit_core::settings! {
    /// The settings of the status component.
    pub enum StatusSetting {
        /// The channels.
        Channels = CHANNELS,
    }
}

/// The counts and levels that the picture decoder measures.
#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    pictures: u64,
    snow_pictures: u64,
    fields_ok: u64,
    fields_bad: u64,
    white_peak: f64,
    chroma_level: f64,
}

/// The status component.
pub struct AtvStatus {
    /// The graph has a picture decoder: its counts go into each report.
    with_picture: bool,
    published: Published,
    /// The ids of the series `line`, `vbi` and `spectrum`.
    series: [Option<SeriesId>; 3],
    channels_text: String,
    channels: Vec<Channel>,
    counts: Counts,
}

impl AtvStatus {
    /// For a modulation.
    pub fn new(modulation: Modulation) -> Self {
        AtvStatus {
            with_picture: matches!(modulation, Modulation::Am | Modulation::FmVideo),
            published: atv_published(),
            series: [None; 3],
            channels_text: String::new(),
            channels: Vec::new(),
            counts: Counts::default(),
        }
    }

    /// Writes one report into the results.
    fn report(&mut self, r: &AtvReport) {
        let mut d = r.detail.clone();
        if self.with_picture {
            let c = self.counts;
            d.pictures = c.pictures;
            d.snow_pictures = c.snow_pictures;
            d.fields_ok = c.fields_ok;
            d.fields_bad = c.fields_bad;
            d.white_peak = c.white_peak;
            d.chroma_level = c.chroma_level;
        }
        for l in &r.log {
            self.published.results_mut().log(l.clone());
        }
        for (id, points) in self.series.into_iter().zip([&r.line, &r.vbi, &r.spectrum]) {
            if !points.is_empty() {
                write_series(&mut self.published, id, points);
            }
        }
        d.summary = summary_line(&d);
        publish_atv(&mut self.published, &d);
        self.published.results_mut().set_status(&Status {
            lock_state: d.state.lock_state(),
            snr_db: Some(d.video_snr_db as f32),
            level_dbfs: Some(d.carrier_dbfs as f32),
            offset_hz: Some(d.carrier_offset_hz),
            summary: d.summary,
        });
    }
}

impl Component for AtvStatus {
    fn info(&self) -> ComponentInfo {
        let mut inputs = vec![Port::typed::<StatusItem>("in")];
        if self.with_picture {
            inputs.push(Port::typed::<PictureReport>("picture"));
        }
        ComponentInfo::sink("atv.status", inputs).with_settings(StatusSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.published = atv_published();
        self.series = ["line", "vbi", "spectrum"].map(|k| series_id(&self.published, k));
        write_channels(&mut self.published, &self.channels);
        self.counts = Counts::default();
        Ok(Vec::new())
    }
    fn process(&mut self, io: &mut Io) {
        if self.with_picture {
            if let Some(p) = io.items::<PictureReport>(1).last() {
                let p = &p.value;
                self.counts = Counts {
                    pictures: p.pictures,
                    snow_pictures: p.snow_pictures,
                    fields_ok: p.fields_ok,
                    fields_bad: p.fields_bad,
                    white_peak: p.white_peak,
                    chroma_level: p.chroma_level,
                };
            }
        }
        for k in 0..io.items::<StatusItem>(0).len() {
            match &io.items::<StatusItem>(0)[k].value {
                StatusItem::Log(l) => self.published.results_mut().log(l.clone()),
                StatusItem::Report(r) => {
                    let r = r.clone();
                    self.report(&r);
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match StatusSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            StatusSetting::Channels => {
                let text = CHANNELS.read(id, v)?;
                self.channels = crate::channels::read(id, text, ATV_CHANNELS_CAP)?;
                self.channels_text = text.to_owned();
                write_channels(&mut self.published, &self.channels);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            StatusSetting::Channels,
            Value::Text(self.channels_text.clone()),
        )])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}
