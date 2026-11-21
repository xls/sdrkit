//! Builds the plan view of a band group: its channels from numbered and listed channels, and its segments from the bands that
//! have no channels.

use crate::data::{Band, BandChannel, BandGroup, Naming, PlanKind, Raster, Service, Source};
use crate::model::{Channel, ChannelPlan, Review, Segment};
use crate::naming::icao_833_name;

/// The bands of `group` with the bands of the groups it includes first, in order. `find` returns a group by its id.
pub(crate) fn bands_of<'a>(
    group: &'a BandGroup,
    find: &dyn Fn(&str) -> Option<&'a BandGroup>,
) -> Vec<Band> {
    let mut out = Vec::new();
    let mut seen = vec![group.id.as_str()];
    collect(group, find, &mut seen, &mut out);
    out
}

fn collect<'a>(
    group: &'a BandGroup,
    find: &dyn Fn(&str) -> Option<&'a BandGroup>,
    seen: &mut Vec<&'a str>,
    out: &mut Vec<Band>,
) {
    for id in &group.include {
        if seen.contains(&id.as_str()) {
            continue;
        }
        seen.push(id);
        if let Some(g) = find(id) {
            collect(g, find, seen, out);
        }
    }
    out.extend(group.bands.iter().cloned());
}

/// The channels of one band in the plan view, in the order the band provides them (numbered channels first). A channel of a band
/// that is not in use is marked reallocated.
pub fn band_channels(b: &Band) -> Vec<Channel> {
    let mut out = Vec::new();
    let width = b.channel_width_hz.unwrap_or(0.0);
    let off_note = |note: &str| {
        if note.is_empty() {
            b.note.clone()
        } else {
            note.to_string()
        }
    };
    if let Some(n) = &b.numbering {
        for number in n.first..=n.last {
            let centre = crate::data::numbered_centre(n, number);
            let name = match n.names {
                Naming::Number => format!("{}{number}", n.prefix),
                Naming::Icao833 => icao_833_name(centre),
            };
            let mut c = Channel::new(name, centre, width);
            if !b.in_use {
                c.reallocated = true;
                c.note = b.note.clone();
            }
            out.push(c);
        }
    }
    for lc in &b.channels {
        let mut c = Channel::new(lc.name.clone(), lc.centre_hz, lc.width_hz.unwrap_or(width));
        c.pair_hz = lc.pair_hz;
        c.reallocated = !b.in_use || !lc.in_use;
        c.note = if c.reallocated {
            off_note(&lc.note)
        } else {
            lc.note.clone()
        };
        out.push(c);
    }
    out
}

/// The segment of a band without channels; `None` for a band with channels.
pub(crate) fn band_segment(b: &Band) -> Option<Segment> {
    if b.numbering.is_some() || !b.channels.is_empty() {
        return None;
    }
    let mut s = Segment::new(b.name.clone(), b.start_hz, b.end_hz)
        .modulation(b.modulation.clone())
        .note(b.note.clone());
    if let Some(r) = &b.raster {
        s = s.raster(r.first_hz, r.step_hz);
    }
    Some(s)
}

/// The plan view of a group whose bands are `bands` (with its includes already resolved). The countries and ITU regions are
/// left empty for the caller to fill.
pub(crate) fn plan_of(group: &BandGroup, bands: &[Band], user: bool) -> ChannelPlan {
    let review = if user {
        Review::User
    } else if group.verified {
        Review::Checked
    } else {
        Review::Unchecked
    };
    let mut p = ChannelPlan::new(
        &group.id,
        &group.name,
        group.kind,
        review,
        &group.source.cite(),
    );
    p.note = group.note.clone();
    p.analog = group.analog.clone();
    for b in bands {
        for m in &b.modes {
            if !p.modes.contains(m) {
                p.modes.push(m.clone());
            }
        }
        p.channels.extend(band_channels(b));
        p.segments.extend(band_segment(b));
    }
    p.channels
        .sort_by(|a, b| a.centre_hz.total_cmp(&b.centre_hz));
    p.segments.sort_by(|a, b| a.lo_hz.total_cmp(&b.lo_hz));
    p
}

/// The service a user plan's bands are given, from the plan's kind.
fn service_of(kind: PlanKind) -> Service {
    match kind {
        PlanKind::Fm | PlanKind::Dab | PlanKind::Tv | PlanKind::AnalogTv | PlanKind::Broadcast => {
            Service::Broadcasting
        }
        PlanKind::Cable => Service::Cable,
        PlanKind::Amateur => Service::Amateur,
        PlanKind::Marine => Service::MaritimeMobile,
        PlanKind::Aviation => Service::AeronauticalMobile,
        PlanKind::Personal | PlanKind::Other => Service::LandMobile,
        PlanKind::Ism => Service::Ism,
    }
}

/// A band group made from a plan in the plan view (a user plan from a CSV file or a JSON file): one band per segment,
/// and one band that holds every channel.
pub(crate) fn group_of(plan: &ChannelPlan) -> BandGroup {
    let service = service_of(plan.kind);
    let mut bands: Vec<Band> = plan
        .segments
        .iter()
        .map(|s| Band {
            name: s.name.clone(),
            service,
            start_hz: s.lo_hz,
            end_hz: s.hi_hz,
            modes: plan.modes.clone(),
            in_use: true,
            modulation: s.modulation.clone(),
            note: s.note.clone(),
            channel_width_hz: None,
            numbering: None,
            channels: Vec::new(),
            raster: s.step_hz.map(|step| Raster {
                first_hz: s.first_hz.unwrap_or(s.lo_hz),
                step_hz: step,
            }),
            cellular: None,
        })
        .collect();
    if let Some((lo, hi)) = plan
        .channels
        .iter()
        .map(|c| (c.lower_hz(), c.upper_hz()))
        .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
    {
        bands.push(Band {
            name: "Channels".into(),
            service,
            start_hz: lo,
            end_hz: hi,
            modes: plan.modes.clone(),
            in_use: true,
            modulation: String::new(),
            note: String::new(),
            channel_width_hz: None,
            numbering: None,
            channels: plan
                .channels
                .iter()
                .map(|c| BandChannel {
                    name: c.name.clone(),
                    centre_hz: c.centre_hz,
                    width_hz: Some(c.width_hz),
                    pair_hz: c.pair_hz,
                    note: c.note.clone(),
                    in_use: !c.reallocated,
                })
                .collect(),
            raster: None,
            cellular: None,
        });
    }
    BandGroup {
        id: plan.id.clone(),
        name: plan.name.clone(),
        kind: plan.kind,
        source: Source {
            publisher: "user".into(),
            title: if plan.source.trim().is_empty() {
                "user plan".into()
            } else {
                plan.source.clone()
            },
            number: String::new(),
            date: "user".into(),
            clause: String::new(),
            url: String::new(),
        },
        also: Vec::new(),
        verified: false,
        unverified_reason: "made by the user".into(),
        note: plan.note.clone(),
        ended: String::new(),
        analog: plan.analog.clone(),
        include: Vec::new(),
        bands,
    }
}
