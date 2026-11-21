//! Announcements (EN 300 401 clause 8.1.6): which active announcement applies to the service being heard.

use crate::ensemble::{Announcement, Ensemble};

/// An announcement that applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Applies {
    /// Cluster.
    pub cluster: u8,
    /// The announcement type (bit number, 0 = alarm).
    pub ty: u8,
    /// The sub-channel to play.
    pub subch: u8,
}

/// Whether one of the active announcements applies to a listener of sub-channel `playing`, with the types in `enabled` (bit mask).
pub fn applicable(db: &Ensemble, playing: u8, enabled: u16) -> Option<Applies> {
    let listeners: Vec<_> = db
        .services
        .values()
        .filter(|s| s.components.iter().any(|c| c.subch == Some(playing)))
        .collect();
    let mut best: Option<Applies> = None;
    for a in db.announcements.values() {
        let Announcement {
            cluster,
            asw,
            subch,
            ..
        } = *a;
        if subch == playing {
            continue;
        }
        let alarm_cluster = cluster == 0xFF || cluster == 0xFE;
        let mut types = 0u16;
        for s in &listeners {
            if s.clusters.contains(&cluster) {
                types |= asw & s.asu;
            }
        }
        if alarm_cluster {
            types |= asw & 1;
        }
        types &= enabled;
        if types == 0 {
            continue;
        }
        // The announcement's sub-channel must be in the table with a size and carry an audio service.
        let ok = db.subchannels.get(&subch).is_some_and(|s| s.size_cu > 0)
            && db
                .services
                .values()
                .any(|s| s.audio().and_then(|c| c.subch) == Some(subch));
        if !ok {
            continue;
        }
        let ty = types.trailing_zeros() as u8;
        if best.is_none_or(|b| ty < b.ty) {
            best = Some(Applies { cluster, ty, subch });
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ensemble::{Component, Service, SubChannel};

    fn db() -> Ensemble {
        let mut e = Ensemble::default();
        for sc in 1..=3u8 {
            e.subchannels.insert(
                sc,
                SubChannel {
                    id: sc,
                    start_cu: 0,
                    size_cu: 36,
                    protection: None,
                    bitrate: 48,
                    fec: 0,
                    language: None,
                },
            );
            let comp = Component {
                tmid: 0,
                ty: 63,
                subch: Some(sc),
                scid: None,
                packet_address: None,
                data_groups: false,
                primary: true,
                ca: false,
                scids: Some(0),
            };
            let mut s = Service {
                sid: 0xC100 + sc as u32,
                components: vec![comp],
                ..Default::default()
            };
            if sc == 1 {
                s.asu = 0x0003;
                s.clusters = vec![5];
            }
            e.services.insert(s.sid, s);
        }
        e
    }

    #[test]
    fn p9_rules() {
        let mut e = db();
        e.announcements.insert(
            5,
            Announcement {
                cluster: 5,
                asw: 0x0002,
                subch: 3,
                new: true,
                region: None,
                last_frame: 0,
            },
        );
        assert_eq!(
            applicable(&e, 1, 0x3F),
            Some(Applies {
                cluster: 5,
                ty: 1,
                subch: 3
            })
        );
        assert_eq!(applicable(&e, 1, 0x3F & !2), None);
        // The announcing service itself (no support of its own), and a listener already on it.
        assert_eq!(applicable(&e, 2, 0x3F), None);
        assert_eq!(applicable(&e, 3, 0x3F), None);
        // A type not announced.
        e.announcements.insert(
            5,
            Announcement {
                cluster: 5,
                asw: 0x0004,
                subch: 3,
                new: true,
                region: None,
                last_frame: 0,
            },
        );
        assert_eq!(applicable(&e, 1, 0x3F), None);
        // The alarm cluster reaches a service without support, and alarm wins over traffic.
        e.announcements.insert(
            5,
            Announcement {
                cluster: 5,
                asw: 0x0002,
                subch: 3,
                new: true,
                region: None,
                last_frame: 0,
            },
        );
        e.announcements.insert(
            0xFF,
            Announcement {
                cluster: 0xFF,
                asw: 0x0001,
                subch: 2,
                new: true,
                region: None,
                last_frame: 0,
            },
        );
        assert_eq!(
            applicable(&e, 1, 0x3F),
            Some(Applies {
                cluster: 0xFF,
                ty: 0,
                subch: 2
            })
        );
        assert_eq!(
            applicable(&e, 3, 0x3F),
            Some(Applies {
                cluster: 0xFF,
                ty: 0,
                subch: 2
            })
        );
    }
}
