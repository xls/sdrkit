//! SigMF metadata of IQ recordings, written from typed fields and read back.
//!
//! Standard: SigMF specification v1.0.0, the global, captures and annotations objects.

use crate::error::{FormatError, SigmfField};
use serde::{Deserialize, Serialize};

/// The SigMF version that this crate writes.
const VERSION: &str = "1.0.0";

/// The metadata of one recording.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Meta {
    /// `core:datatype`.
    pub datatype: String,
    /// `core:sample_rate`, Hz.
    pub rate_hz: f64,
    /// `core:frequency` of the first capture, Hz.
    pub centre_hz: f64,
    /// `core:datetime` of the first capture, ISO 8601 UTC. Empty for none.
    pub datetime: String,
    /// `core:description`. Empty for none.
    pub description: String,
    /// One annotation over the samples from `start` for `count`, with a label. `None` for no annotation.
    pub annotation: Option<(u64, u64, String)>,
}

#[derive(Serialize, Deserialize)]
struct Doc {
    global: Global,
    #[serde(default)]
    captures: Vec<Capture>,
    #[serde(default)]
    annotations: Vec<Annotation>,
}

#[derive(Serialize, Deserialize)]
struct Global {
    #[serde(rename = "core:datatype", default)]
    datatype: String,
    #[serde(rename = "core:sample_rate", default)]
    sample_rate: Option<f64>,
    #[serde(rename = "core:version", default)]
    version: String,
    #[serde(
        rename = "core:recorder",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    recorder: String,
    #[serde(
        rename = "core:description",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    description: String,
}

#[derive(Serialize, Deserialize)]
struct Capture {
    #[serde(rename = "core:sample_start", default)]
    sample_start: u64,
    #[serde(
        rename = "core:frequency",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    frequency: Option<f64>,
    #[serde(
        rename = "core:datetime",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    datetime: String,
}

#[derive(Serialize, Deserialize)]
struct Annotation {
    #[serde(rename = "core:sample_start", default)]
    sample_start: u64,
    #[serde(rename = "core:sample_count", default)]
    sample_count: u64,
    #[serde(
        rename = "core:label",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    label: String,
}

/// The metadata file text of `m`.
pub(crate) fn write(m: &Meta) -> Vec<u8> {
    let doc = Doc {
        global: Global {
            datatype: m.datatype.clone(),
            sample_rate: Some(m.rate_hz),
            version: VERSION.into(),
            recorder: "sdrkit".into(),
            description: m.description.clone(),
        },
        captures: vec![Capture {
            sample_start: 0,
            frequency: (m.centre_hz > 0.0).then_some(m.centre_hz),
            datetime: m.datetime.clone(),
        }],
        annotations: m
            .annotation
            .iter()
            .map(|(start, count, label)| Annotation {
                sample_start: *start,
                sample_count: *count,
                label: label.clone(),
            })
            .collect(),
    };
    // Plain structs of strings and numbers always serialise.
    serde_json::to_vec_pretty(&doc).unwrap_or_default()
}

/// Reads metadata file text.
pub(crate) fn read(text: &[u8]) -> Result<Meta, FormatError> {
    let doc: Doc = serde_json::from_slice(text).map_err(|e| FormatError::SigmfJson {
        line: e.line(),
        column: e.column(),
    })?;
    if doc.global.datatype.is_empty() {
        return Err(FormatError::SigmfField(SigmfField::Datatype));
    }
    let rate_hz = doc
        .global
        .sample_rate
        .filter(|r| r.is_finite() && *r > 0.0)
        .ok_or(FormatError::SigmfField(SigmfField::SampleRate))?;
    let first = doc.captures.first();
    Ok(Meta {
        datatype: doc.global.datatype,
        rate_hz,
        centre_hz: first.and_then(|c| c.frequency).unwrap_or(0.0),
        datetime: first.map(|c| c.datetime.clone()).unwrap_or_default(),
        description: doc.global.description,
        annotation: doc
            .annotations
            .into_iter()
            .next()
            .map(|a| (a.sample_start, a.sample_count, a.label)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_reads_back() {
        let m = Meta {
            datatype: "ci8".into(),
            rate_hz: 48_000.0,
            centre_hz: 7.1e6,
            datetime: "2025-01-01T12:00:00Z".into(),
            description: String::new(),
            annotation: Some((100, 200, "button".into())),
        };
        assert_eq!(read(&write(&m)).unwrap(), m);
        assert_eq!(
            read(br#"{"global":{"core:datatype":"ci8"}}"#),
            Err(FormatError::SigmfField(SigmfField::SampleRate))
        );
        assert!(matches!(read(b"{"), Err(FormatError::SigmfJson { .. })));
    }
}
