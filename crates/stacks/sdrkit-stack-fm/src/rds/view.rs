//! What the RDS decoder holds at a report: the decoded fields and the block counters, copied in place so that a steady signal
//! allocates nothing.

use super::block::BlockStats;
use super::group::{ClockTime, EonEntry, Oda, Pin};
use super::tmc::{TmcMessage, TmcService};
use super::RdsDecoder;
use sdrkit_proto_bcdata::tags::TagSet;

/// The RDS fields at one report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RdsView {
    /// Block sync.
    pub sync: bool,
    /// The usable-block percentage (0 without sync).
    pub ok_pct: f32,
    /// The block counters.
    pub blocks: BlockStats,
    /// Accepted groups since reset.
    pub groups: u64,
    /// PI (0 until a group has been accepted).
    pub pi: u16,
    /// Programme type code, `None` before the first accepted group.
    pub pty: Option<u8>,
    /// Traffic programme.
    pub tp: bool,
    /// Traffic announcement.
    pub ta: bool,
    /// Music (true) or speech.
    pub music: bool,
    /// The published PS (8 characters, trailing spaces kept), or empty.
    pub station: String,
    /// The name being collected (`_` for missing segments).
    pub station_partial: String,
    /// The RadioText as shown.
    pub text: String,
    /// The RadioText is complete.
    pub text_complete: bool,
    /// Alternative frequency codes, ascending (frequency = (875 + code) / 10 MHz).
    pub af_codes: Vec<u8>,
    /// Programme type name.
    pub ptyn: String,
    /// Extended country code.
    pub ecc: Option<u8>,
    /// Language code.
    pub language: Option<u16>,
    /// Programme item number.
    pub pin: Option<Pin>,
    /// Clock time.
    pub clock: Option<ClockTime>,
    /// Open data applications.
    pub oda: Vec<Oda>,
    /// The RadioText+ tags and the item bits.
    pub tags: TagSet,
    /// Enhanced RadioText.
    pub ert: String,
    /// Other networks.
    pub eon: Vec<EonEntry>,
    /// The TMC service.
    pub tmc_service: TmcService,
    /// TMC messages, newest first.
    pub tmc: Vec<TmcMessage>,
}

/// Copies `src` into `dst` when they differ (the lists change seldom, so most reports copy nothing).
fn copy<T: Clone + PartialEq>(dst: &mut Vec<T>, src: &[T]) {
    if dst.as_slice() != src {
        dst.clear();
        dst.extend_from_slice(src);
    }
}

fn copy_str(dst: &mut String, src: &str) {
    if dst != src {
        dst.clear();
        dst.push_str(src);
    }
}

impl RdsView {
    /// Takes the decoder's fields, in place.
    pub fn fill(&mut self, d: &RdsDecoder) {
        let b = &d.blocks;
        let g = &d.groups;
        self.sync = b.synced();
        self.ok_pct = b.quality();
        self.blocks = b.stats();
        self.groups = g.groups;
        self.pi = g.pi;
        self.pty = g.pty;
        self.tp = g.tp;
        self.ta = g.ta;
        self.music = g.music;
        copy_str(&mut self.station, &g.ps);
        copy_str(&mut self.station_partial, &g.ps_partial);
        copy_str(&mut self.text, &g.rt);
        self.text_complete = g.rt_complete;
        copy(&mut self.af_codes, g.af_codes());
        copy_str(&mut self.ptyn, &g.ptyn);
        self.ecc = g.ecc;
        self.language = g.language;
        self.pin = g.pin;
        self.clock = g.clock;
        copy(&mut self.oda, &g.oda);
        if self.tags != g.tags {
            self.tags.clone_from(&g.tags);
        }
        copy_str(&mut self.ert, &g.ert);
        copy(&mut self.eon, &g.eon);
        self.tmc_service = g.tmc.service;
        if !self.tmc.iter().eq(g.tmc.messages.iter()) {
            self.tmc.clear();
            self.tmc.extend(g.tmc.messages.iter().cloned());
        }
    }

    /// The alternative frequencies, Hz.
    pub fn af_hz(&self) -> impl Iterator<Item = f64> + '_ {
        self.af_codes
            .iter()
            .map(|&c| (875 + c as u32) as f64 * 100e3)
    }
}
