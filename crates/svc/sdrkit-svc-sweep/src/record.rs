//! Which hits to record while a sweep runs, and how many the CPU allows. The engine does the recording.

sdrkit_core::choice_enum! {
    /// What a recording holds.
    pub enum RecordKind {
        /// Demodulated audio, as WAV.
        Audio = ("audio", "Audio"),
        /// The IQ samples of the channel, as SigMF.
        Iq = ("iq", "IQ"),
    }
}

/// The recording rules of a sweep.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RecordOptions {
    /// Records hits.
    pub enabled: bool,
    /// What to record.
    pub kind: RecordKind,
    /// The most channels at once.
    pub max_channels: usize,
    /// The longest stay on one hop while recording, s.
    pub max_park_s: f64,
}

/// A channel that the engine must record now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecordRequest {
    /// The row key of the channel in the findings table.
    pub id: u64,
    /// Its centre, Hz.
    pub hz: f64,
    /// Its width, Hz.
    pub width_hz: f64,
    /// The demodulator to record audio with (`"nfm"`, `"am"`, `"wfm"`, `"cw"`). Empty for IQ, or if none fits.
    pub demod: &'static str,
    /// What to record.
    pub kind: RecordKind,
}

/// An open hit on the hop that a sweep captures: its id, centre, width, SNR and demodulator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OpenHit {
    pub id: u64,
    pub hz: f64,
    pub width_hz: f64,
    pub snr_db: f32,
    pub demod: &'static str,
}

/// The most channels the CPU allows: `max_channels` while the load is under 60 %, falling linearly to one at 90 % (and none
/// from 95 %).
pub(crate) fn cpu_guard(max_channels: usize, cpu_load: f64) -> usize {
    if cpu_load >= 0.95 {
        0
    } else if cpu_load <= 0.6 {
        max_channels
    } else {
        let t = ((0.9 - cpu_load) / 0.3).clamp(0.0, 1.0);
        ((1.0 + t * (max_channels.saturating_sub(1)) as f64).floor() as usize).min(max_channels)
    }
}

/// Puts into `out` what to record from the open `hits` of the hop: the strongest first, up to the CPU guard. Sorts `hits`.
pub(crate) fn choose(
    opts: &RecordOptions,
    cpu_load: f64,
    hits: &mut [OpenHit],
    out: &mut Vec<RecordRequest>,
) {
    out.clear();
    if !opts.enabled {
        return;
    }
    let limit = cpu_guard(opts.max_channels, cpu_load);
    hits.sort_unstable_by(|a, b| b.snr_db.total_cmp(&a.snr_db));
    out.extend(hits.iter().take(limit).map(|h| RecordRequest {
        id: h.id,
        hz: h.hz,
        width_hz: h.width_hz,
        demod: if opts.kind == RecordKind::Iq {
            ""
        } else {
            h.demod
        },
        kind: opts.kind,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_and_choose() {
        assert_eq!(cpu_guard(8, 0.3), 8);
        assert_eq!(cpu_guard(8, 0.9), 1);
        assert_eq!(cpu_guard(8, 0.75), 4);
        assert_eq!(cpu_guard(8, 0.99), 0);
        let mut opts = RecordOptions {
            enabled: true,
            kind: RecordKind::Audio,
            max_channels: 2,
            max_park_s: 60.0,
        };
        let hit = |id, hz, snr_db, demod| OpenHit {
            id,
            hz,
            width_hz: 1e4,
            snr_db,
            demod,
        };
        let mut hits = [
            hit(1, 1e6, 10.0, "nfm"),
            hit(2, 2e6, 30.0, "am"),
            hit(3, 3e6, 20.0, "nfm"),
        ];
        let mut c = Vec::new();
        choose(&opts, 0.1, &mut hits, &mut c);
        assert_eq!(c.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2, 3]);
        assert_eq!(c[0].demod, "am");
        opts.enabled = false;
        choose(&opts, 0.1, &mut hits, &mut c);
        assert!(c.is_empty());
    }
}
