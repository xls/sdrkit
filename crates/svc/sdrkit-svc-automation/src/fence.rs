//! Named areas and the closest approach: an object that crosses the edge of an area returns `geo.enter` or `geo.exit`, and an
//! object that moves towards the receiver returns `geo.cpa` with the distance and the time of its closest approach.

use std::collections::HashMap;

use sdrkit_core::publish::keys;
use sdrkit_svc_events::{FamilySet, Kinds, Observation, Position};
use sdrkit_util_geo::position::{closest_approach, Fence, Track};
use sdrkit_util_geo::KNOT_M_S;
use serde::{Deserialize, Serialize};

use crate::error::{AutomationError, TextError};

sdrkit_core::choice_enum! {
    /// The shape of a named area.
    pub enum Shape {
        /// A circle around a centre.
        Circle = ("circle", "Circle"),
        /// A polygon of three or more corners.
        Polygon = ("polygon", "Polygon"),
    }
}

/// A named area as the user set it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FenceText {
    pub shape: String,
    pub lat: f64,
    pub lon: f64,
    pub radius_km: f64,
    pub corners: String,
    pub families: String,
}

/// A named area, resolved.
#[derive(Clone, Debug)]
pub(crate) struct NamedFence {
    pub fence: Fence,
    pub families: FamilySet,
}

/// The corners of a list such as `"-33.8 151.2; -33.9 151.3; -33.9 151.1"`.
fn corners(text: &str) -> Result<Vec<Position>, TextError> {
    let mut out = Vec::new();
    let mut start = 0;
    for part in text.split(';') {
        let at = start + part.len() - part.trim_start().len();
        start += part.len() + 1;
        if part.trim().is_empty() {
            continue;
        }
        let mut n = part.split_whitespace().map(str::parse::<f64>);
        let p = match (n.next(), n.next(), n.next()) {
            (Some(Ok(lat)), Some(Ok(lon)), None) => Position::checked(lat, lon, 0.0).ok(),
            _ => None,
        };
        out.push(p.ok_or(TextError::Corner { at })?);
    }
    if out.len() < 3 {
        return Err(TextError::Corner { at: text.len() });
    }
    Ok(out)
}

impl NamedFence {
    /// The area of `text` with shape `shape`.
    ///
    /// # Errors
    ///
    /// [`AutomationError::Text`] if a corner or a family does not parse.
    pub fn resolve(text: FenceText, shape: Shape) -> Result<NamedFence, AutomationError> {
        let err = |arg| move |error| AutomationError::Text { arg, error };
        let fence = match shape {
            Shape::Circle => Fence::Circle {
                centre: Position::new(text.lat, text.lon),
                radius_m: text.radius_km * 1000.0,
            },
            Shape::Polygon => Fence::Polygon {
                corners: corners(&text.corners).map_err(err("corners"))?,
            },
        };
        let families =
            Kinds::family_list(&text.families, 0).map_err(|e| err("families")(e.into()))?;
        Ok(NamedFence { fence, families })
    }
}

/// The closest approach warning, resolved from the settings.
#[derive(Clone, Debug, Default)]
pub(crate) struct Cpa {
    pub max_km: f64,
    pub max_tcpa_s: f64,
    pub families: FamilySet,
    pub repeat_s: f64,
}

/// A crossing or an approach that the watch found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GeoChange {
    Enter(usize),
    Exit(usize),
    Approach { cpa_km: f64, tcpa_s: f64 },
}

/// What the watch keeps between events.
#[derive(Debug, Default)]
pub(crate) struct GeoWatch {
    /// Per identity, one bit per area: inside.
    inside: HashMap<String, u64>,
    cpa_last: HashMap<String, f64>,
}

const MAX_TRACKED: usize = 100_000;

impl GeoWatch {
    /// Forgets which objects are inside which area.
    pub fn reset(&mut self) {
        self.inside.clear();
    }

    /// The crossings and the approach of `o`, given the areas, the warning and the receiver. Nothing is allocated for an
    /// object that is seen before and crosses nothing.
    pub fn observe(
        &mut self,
        o: &Observation<'_>,
        fences: &[NamedFence],
        cpa: Option<&Cpa>,
        receiver: Option<&Position>,
        out: &mut Vec<GeoChange>,
    ) {
        let (Some(p), Some(id)) = (o.position, o.identity) else {
            return;
        };
        if !fences.is_empty() {
            let mut bits = 0u64;
            let mut mask = 0u64;
            for (i, f) in fences.iter().enumerate().take(64) {
                if f.families.admits(o.family) {
                    mask |= 1 << i;
                    if f.fence.contains(&p) {
                        bits |= 1 << i;
                    }
                }
            }
            let was = match self.inside.get_mut(id) {
                Some(w) => {
                    let old = *w;
                    *w = (old & !mask) | bits;
                    old
                }
                None => {
                    if self.inside.len() >= MAX_TRACKED {
                        self.inside.clear();
                    }
                    self.inside.insert(id.to_owned(), bits);
                    0
                }
            };
            for i in 0..fences.len().min(64) {
                let b = 1u64 << i;
                match (was & b != 0, bits & b != 0, mask & b != 0) {
                    (false, true, true) => out.push(GeoChange::Enter(i)),
                    (true, false, true) => out.push(GeoChange::Exit(i)),
                    _ => {}
                }
            }
        }
        let (Some(cpa), Some(rx)) = (cpa, receiver) else {
            return;
        };
        if !cpa.families.admits(o.family) {
            return;
        }
        let (Some(speed), Some(course)) =
            (o.event.get(&keys::SPEED_KT), o.event.get(&keys::COURSE_DEG))
        else {
            return;
        };
        if !(0.0..102.2).contains(&speed) || !(0.0..360.0).contains(&course) {
            return;
        }
        let own = Track {
            position: *rx,
            speed_m_s: 0.0,
            course_deg: 0.0,
        };
        let other = Track {
            position: p,
            speed_m_s: speed * KNOT_M_S,
            course_deg: course,
        };
        let a = closest_approach(&own, &other);
        let cpa_km = a.distance_m / 1000.0;
        if a.time_s <= 0.0 || a.time_s > cpa.max_tcpa_s || cpa_km > cpa.max_km {
            return;
        }
        if self
            .cpa_last
            .get(id)
            .is_some_and(|t| o.unix_time - t < cpa.repeat_s)
        {
            return;
        }
        if self.cpa_last.len() >= MAX_TRACKED {
            self.cpa_last.clear();
        }
        self.cpa_last.insert(id.to_owned(), o.unix_time);
        out.push(GeoChange::Approach {
            cpa_km,
            tcpa_s: a.time_s,
        });
    }
}
