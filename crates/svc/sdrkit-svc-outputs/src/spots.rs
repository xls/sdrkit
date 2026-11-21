//! Spot reports: decoded amateur stations, posted in batches under the user's own call sign and locator. An output of
//! this kind needs both.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sdrkit_core::publish::keys;
use sdrkit_svc_events::{FieldRef, KindSet, Kinds, Observation, Reading};
use serde::{Deserialize, Serialize};

use crate::http::post;
use crate::runner::{Item, Sink};
use crate::OutputError;
use sdrkit_core::net::Transport;

/// Spot reporting settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpotSettings {
    /// The user's call sign.
    pub callsign: String,
    /// The user's Maidenhead locator.
    pub locator: String,
    /// Where batches are posted (`http://` or `https://`).
    pub endpoint: String,
    /// Seconds between batches.
    pub batch_s: f64,
    /// The same station on the same band is reported at most once in this many seconds.
    pub dedupe_s: f64,
    /// The most spots per batch.
    pub max_batch: usize,
}

impl Default for SpotSettings {
    fn default() -> Self {
        SpotSettings {
            callsign: String::new(),
            locator: String::new(),
            endpoint: String::new(),
            batch_s: 300.0,
            dedupe_s: 900.0,
            max_batch: 500,
        }
    }
}

/// One spot.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Spot {
    /// The station heard.
    pub call: String,
    /// Its locator, when sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    /// Frequency, Hz.
    pub freq_hz: f64,
    /// Signal to noise, dB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snr_db: Option<f64>,
    /// Mode name.
    pub mode: String,
    /// Unix seconds.
    pub time: f64,
}

/// The amateur band of a frequency, as a whole number of metres (160 to 0, rounded), for the dedupe key.
fn band(freq_hz: f64) -> i64 {
    if freq_hz <= 0.0 {
        return 0;
    }
    (299_792_458.0 / freq_hz).round() as i64
}

/// The fields that a spot reads, resolved when the output is added.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SpotFields {
    pub call: Option<FieldRef>,
    pub locator: Option<FieldRef>,
    pub snr: Option<FieldRef>,
    pub mode: Option<FieldRef>,
}

impl SpotFields {
    /// The standard fields of a spot within the kinds `within`. A field that no kind declares is left out.
    pub fn resolve(k: &Kinds, within: Option<&KindSet>) -> SpotFields {
        let f = |name: &str| k.field(name, 0, within).ok();
        SpotFields {
            call: f(keys::CALLSIGN.key),
            locator: f(keys::LOCATOR.key),
            snr: f(keys::SNR_DB.key),
            mode: f(keys::SPOT_MODE.key),
        }
    }
}

/// The spot that `o` describes, if any: a call sign with a locator or a signal to noise ratio.
pub(crate) fn spot_of(o: &Observation<'_>, f: &SpotFields) -> Option<Spot> {
    let text = |r: &Option<FieldRef>| match r.as_ref().and_then(|r| o.read(r)) {
        Some(Reading::Text(t)) if !t.trim().is_empty() => Some(t.trim()),
        _ => None,
    };
    let call = text(&f.call)?.to_ascii_uppercase();
    if call.len() < 3
        || !call
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'/' || b == b'-')
        || !call.bytes().any(|b| b.is_ascii_digit())
    {
        return None;
    }
    let locator = text(&f.locator).map(str::to_owned);
    let snr_db = f.snr.as_ref().and_then(|r| o.read(r)?.number());
    if locator.is_none() && snr_db.is_none() {
        return None;
    }
    Some(Spot {
        call,
        locator,
        freq_hz: o.event.freq_hz,
        snr_db,
        mode: text(&f.mode).unwrap_or("").to_ascii_uppercase(),
        time: o.unix_time.floor(),
    })
}

/// The report that a batch posts.
#[derive(Serialize)]
struct Report<'a> {
    receiver: Receiver<'a>,
    spots: &'a [Spot],
}

/// The receiver of a report.
#[derive(Serialize)]
struct Receiver<'a> {
    callsign: String,
    locator: &'a str,
    software: &'static str,
}

/// The spot reporter.
pub struct SpotSink {
    s: SpotSettings,
    transport: Arc<dyn Transport>,
    batch: Vec<Spot>,
    since: Instant,
    recent: HashMap<(String, i64), f64>,
}

impl SpotSink {
    /// A reporter. Refused without the user's call sign, locator and endpoint.
    pub fn new(s: SpotSettings, transport: Arc<dyn Transport>) -> Result<Self, OutputError> {
        if s.callsign.trim().len() < 3
            || sdrkit_util_geo::maidenhead::decode(&s.locator).is_err()
            || s.endpoint.is_empty()
        {
            return Err(OutputError::Setting(crate::BadSetting::Callsign));
        }
        crate::net::Url::parse(&s.endpoint)?;
        Ok(SpotSink {
            s,
            transport,
            batch: Vec::new(),
            since: Instant::now(),
            recent: HashMap::new(),
        })
    }

    /// The spots waiting for the next batch.
    pub fn waiting(&self) -> &[Spot] {
        &self.batch
    }

    /// Posts the batch now.
    pub fn flush(&mut self) -> Result<(), OutputError> {
        self.since = Instant::now();
        if self.batch.is_empty() {
            return Ok(());
        }
        let doc = serde_json::to_vec(&Report {
            receiver: Receiver {
                callsign: self.s.callsign.to_ascii_uppercase(),
                locator: &self.s.locator,
                software: concat!("sdrkit ", env!("CARGO_PKG_VERSION")),
            },
            spots: &self.batch,
        })?;
        post(
            &*self.transport,
            &self.s.endpoint,
            "application/json",
            &[],
            &doc,
            Duration::from_secs(15),
        )?;
        self.batch.clear();
        Ok(())
    }
}

impl Sink<Item> for SpotSink {
    fn deliver(&mut self, item: &Item) -> Result<(), OutputError> {
        if let Item::Spot(sp) = item {
            let sp = sp.clone();
            let key = (sp.call.clone(), band(sp.freq_hz));
            if !self
                .recent
                .get(&key)
                .is_some_and(|t| sp.time - t < self.s.dedupe_s)
            {
                self.recent.insert(key, sp.time);
                if self.batch.len() < self.s.max_batch.max(1) {
                    self.batch.push(sp);
                }
            }
        }
        if self.since.elapsed().as_secs_f64() >= self.s.batch_s
            || self.batch.len() >= self.s.max_batch.max(1)
        {
            self.flush()?;
        }
        Ok(())
    }

    fn idle(&mut self) -> Result<(), OutputError> {
        if self.since.elapsed().as_secs_f64() >= self.s.batch_s {
            self.flush()?;
        }
        Ok(())
    }

    fn close(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::{EventCatalog, EventDef};
    use sdrkit_svc_events::Context;

    static DECODE: EventDef = EventDef::new(
        "test.decode",
        "Decode",
        &[
            &keys::CALLSIGN,
            &keys::LOCATOR,
            &keys::SNR_DB,
            &keys::SPOT_MODE,
        ],
    );

    #[test]
    fn spots_from_observations() {
        let k = Kinds::new(Arc::new(EventCatalog::new(&[&DECODE]).unwrap()));
        let f = SpotFields::resolve(&k, None);
        let ctx = Context {
            unix_time: 100.5,
            receiver: None,
        };
        let spot = |call: &str, grid: Option<&str>, snr: Option<f64>| {
            let mut e = DECODE
                .event(0.0, 14.074512e6)
                .set(&keys::CALLSIGN, call)
                .opt(&keys::LOCATOR, grid)
                .opt(&keys::SNR_DB, snr)
                .set(&keys::SPOT_MODE, "ft8");
            k.catalog().stamp(&mut e);
            spot_of(&k.observe(&e, &ctx).unwrap(), &f)
        };
        let s = spot("ja1xyz", Some("PM95"), Some(-12.0)).unwrap();
        assert_eq!(
            (
                s.call.as_str(),
                s.mode.as_str(),
                s.locator.as_deref(),
                s.time
            ),
            ("JA1XYZ", "FT8", Some("PM95"), 100.0)
        );
        assert!(
            spot("VK2ABC", None, None).is_none(),
            "no report, no locator"
        );
        assert!(spot("CQ", None, Some(1.0)).is_none());
        assert_eq!(band(14.074e6), 21);
    }

    #[test]
    fn opt_in_needs_own_details() {
        let t: Arc<dyn Transport> = Arc::new(sdrkit_core::net::StdNet);
        assert!(SpotSink::new(SpotSettings::default(), t.clone()).is_err());
        let ok = SpotSettings {
            callsign: "VK2ABC".into(),
            locator: "QF56od".into(),
            endpoint: "http://127.0.0.1:9/spots".into(),
            ..SpotSettings::default()
        };
        assert!(SpotSink::new(ok, t).is_ok());
    }
}
