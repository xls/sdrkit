//! The teletext decoder: it slices the teletext lines of a 625-line picture and returns each 42-byte packet to the page store of
//! `sdrkit-proto-mpegts`. It publishes the header, the pages held and the page shown, and sends each completed page as a
//! `teletext.page` item. The command `teletext.select` chooses the page shown.
//!
//! Standard: ITU-R BT.653 system B, ETS 300 706.

use super::{last, LineInfo, Tune};
use crate::results::teletext_published;
use crate::teletext::{is_teletext_line, BIT_RATE, FRAMING, ONE_LEVEL, START_US};
use crate::typed::TeletextFields;
use crate::util::{Interp8, Ring, RingRead};
use sdrkit_core::command::CommandSet;
use sdrkit_core::publish::Published;
use sdrkit_core::{CommandError, CommandId, Commands, ConfigError, Record, Results, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed};
use sdrkit_proto_mpegts::publish::{
    TeletextSelectArgs, TeletextTables, TELETEXT_PAGE_TABLE, TELETEXT_SELECT,
};
use sdrkit_proto_mpegts::{TeletextDecoder as PageDecoder, TeletextPage};
use sdrkit_util_text::teletext::hamming::parity;
use sdrkit_util_text::teletext::packet::{parse, Packet};
use std::collections::VecDeque;
use std::f64::consts::TAU;

sdrkit_core::commands! {
    /// The commands of the teletext decoder.
    pub enum TeletextCommand {
        /// Shows a page.
        TeletextSelect = TELETEXT_SELECT.command,
    }
}

/// The page shown first.
const FIRST_PAGE: u16 = 100;

/// The teletext decoder.
pub struct TeletextDecoder {
    fv: f64,
    ring: Ring<f32>,
    queue: VecDeque<LineInfo>,
    interp: Interp8,
    pages: PageDecoder,
    tables: TeletextTables,
    header: String,
    epoch: u32,
    shown: u16,
    lines: u64,
    recent: u32,
    seg: Vec<f32>,
    bits: Vec<u8>,
    published: Published,
}

impl Default for TeletextDecoder {
    fn default() -> Self {
        TeletextDecoder {
            fv: 1.0,
            ring: crate::util::ring(16),
            queue: VecDeque::with_capacity(64),
            interp: Interp8::new(0.95),
            pages: PageDecoder::new(),
            tables: TeletextTables::default(),
            header: String::new(),
            epoch: u32::MAX,
            shown: FIRST_PAGE,
            lines: 0,
            recent: 0,
            seg: Vec::with_capacity(1024),
            bits: Vec::with_capacity(400),
            published: teletext_published(),
        }
    }
}

impl TeletextDecoder {
    fn reset(&mut self) {
        self.queue.clear();
        self.pages.clear();
        self.header.clear();
        self.lines = 0;
        self.recent = 0;
    }

    fn us(&self, x: f64) -> f64 {
        x * 1e-6 * self.fv
    }

    /// Slices one line; returns the 42 bytes after the framing code.
    fn slice(&mut self, li: &LineInfo) -> Option<[u8; 42]> {
        let tb = self.fv / BIT_RATE;
        // Too few samples a bit to slice (the video band ends below the data's).
        if tb < 1.2 || li.gain == 0.0 {
            return None;
        }
        let a = (li.t0 + self.us(START_US - 2.0)).floor() as i64;
        let b = (li.t0 + self.us(START_US) + 361.0 * tb + 8.0).ceil() as i64;
        let (blank, pol, gain) = (li.blank, li.pol, li.gain);
        self.seg.clear();
        for k in a..b {
            self.seg.push((blank - pol * self.ring.at(k)) / gain);
        }
        let x0 = li.t0 + self.us(START_US) - a as f64;
        // The clock run-in: the phase comes from its fundamental at half the bit rate, the slicing level from
        // its mean.
        let w = TAU / (2.0 * tb);
        let (ra, rb) = ((x0 + 0.5 * tb).max(0.0) as usize, (x0 + 15.5 * tb) as usize);
        if rb >= self.seg.len() {
            return None;
        }
        let (mut re, mut im, mut mean) = (0.0f64, 0.0f64, 0.0f64);
        for k in ra..rb {
            let v = self.seg[k] as f64;
            re += v * (w * k as f64).cos();
            im -= v * (w * k as f64).sin();
            mean += v;
        }
        let n = (rb - ra) as f64;
        mean /= n;
        let amp = 2.0 * (re * re + im * im).sqrt() / n;
        // A run-in swings between black and 0.66 of white; its fundamental is about 0.4 of that.
        if amp < 0.25 * ONE_LEVEL * 0.6 || !(0.1..0.6).contains(&mean) {
            return None;
        }
        // The ones of the run-in are at the cosine's peaks: k = -arg / w + m 2 tb. The first bit's centre is nearest x0 + tb / 2.
        let peak = -im.atan2(re) / w;
        let want = x0 + 0.5 * tb;
        let m = ((want - peak) / (2.0 * tb)).round();
        let c0 = peak + m * 2.0 * tb;
        let th = mean as f32;
        let total = 16 + 8 + 42 * 8 + 8;
        self.bits.clear();
        for i in 0..total {
            let pos = c0 + (i as f64 - 4.0) * tb;
            let v = if pos >= 4.0 && pos < (self.seg.len() - 5) as f64 {
                self.interp.at_slice(&self.seg, pos)
            } else {
                0.0
            };
            self.bits.push((v > th) as u8);
        }
        // bits[4] is the first bit of the run-in at its place; the framing code is at 4 + 16, looked for two bits either side.
        let byte_at = |bits: &[u8], s: usize| -> u8 { (0..8).map(|i| bits[s + i] << i).sum() };
        let start = [20usize, 18, 22, 19, 21, 17, 23]
            .into_iter()
            .find(|&s| byte_at(&self.bits, s) == FRAMING)?;
        let mut out = [0u8; 42];
        for (k, o) in out.iter_mut().enumerate() {
            let s = start + 8 + 8 * k;
            if s + 8 > self.bits.len() {
                return None;
            }
            *o = byte_at(&self.bits, s);
        }
        Some(out)
    }

    fn line(&mut self, li: LineInfo) {
        if li.lines != 625 || !is_teletext_line(li.number) {
            return;
        }
        match self.slice(&li) {
            Some(bytes) => {
                self.lines += 1;
                let before = self.pages.store().stats.packets;
                if let Ok(Packet::Header(h)) = parse(&bytes) {
                    self.header.clear();
                    self.header.extend(h.text.iter().map(|&b| match parity(b) {
                        Some(c) if (0x20..0x7F).contains(&c) => c as char,
                        _ => ' ',
                    }));
                }
                self.pages.push_packet(&bytes);
                if self.pages.store().stats.packets > before {
                    self.recent = 50;
                }
            }
            None => {
                if self.recent > 0 {
                    self.recent -= 1;
                }
            }
        }
    }

    fn publish(&mut self) {
        let p = &mut self.published;
        let present = self.recent > 0;
        let st = self.pages.store().stats;
        let text = present
            .then(|| self.pages.page(self.shown, None))
            .flatten()
            .map(|pg| pg.text().join("\n"));
        p.put(&TeletextFields {
            teletext_header: present.then_some(self.header.trim()),
            teletext_pages: present.then_some(self.pages.stats.pages as i64),
            teletext_packets: present.then_some(st.packets as i64),
            teletext_errors: present.then_some(st.bad_address as i64),
            teletext_parity_errors: present.then_some(st.parity_errors as i64),
            teletext_page: i64::from(self.shown),
            page_text: text.as_deref(),
        });
        self.tables.write(&self.pages, p);
    }
}

impl Component for TeletextDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.teletext",
            vec![
                Port::real("video"),
                Port::typed::<LineInfo>("lines").optional("no line is sliced"),
                Port::typed::<Tune>("control"),
            ],
            vec![Port::typed::<TeletextPage>("pages").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fv = inputs[0].rate.max(1.0);
        // The line reports come up to a dozen lines after their lines.
        self.ring = crate::util::ring((self.us(64.0 * 24.0) as usize).max(64));
        self.reset();
        Ok(vec![Format::new(Kind::typed::<TeletextPage>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(t) = last::<Tune>(io, 2) {
            if t.epoch != self.epoch {
                self.epoch = t.epoch;
                self.reset();
            }
        }
        for li in io.items::<LineInfo>(1) {
            if self.queue.len() >= 128 {
                self.queue.pop_front();
            }
            self.queue.push_back(li.value);
        }
        let v = io.real(0);
        let mut i = 0;
        loop {
            let n = 256.min(v.len() - i);
            for &x in &v[i..i + n] {
                self.ring.push(x);
            }
            i += n;
            let end = self.ring.end() as f64;
            while let Some(li) = self.queue.front().copied() {
                if end < li.t0 + li.period + 16.0 {
                    break;
                }
                self.queue.pop_front();
                self.line(li);
            }
            if n == 0 || i >= v.len() {
                break;
            }
        }
        let t = io.input_format(0).time_of(io.input(0).first());
        let completed: Vec<(u16, u16)> = self.pages.drain_completed().collect();
        for (page, subcode) in completed {
            if let Some(pg) = self.pages.page(page, Some(subcode)) {
                io.items_out::<TeletextPage>(0)
                    .push(Timed { time: t, value: pg });
            }
        }
        if io.tick() {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        TeletextCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TeletextCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TeletextCommand::TeletextSelect => {
                let TeletextSelectArgs { row } = TELETEXT_SELECT.args(args)?;
                self.shown = row
                    .parse::<u16>()
                    .ok()
                    .filter(|n| (100..=899).contains(n))
                    .ok_or_else(|| CommandError::NoSuchRow {
                        table: crate::results::table_id(&self.published, &TELETEXT_PAGE_TABLE),
                        key: row.to_owned(),
                    })?;
                self.publish();
            }
        }
        Ok(None)
    }
}
