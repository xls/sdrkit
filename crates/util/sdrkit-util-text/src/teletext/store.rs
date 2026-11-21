//! Page and subpage assembly (ETS 300 706 clause 9 and Annex B): packets are attached to the page whose header last came in the
//! same magazine (or in any magazine when C11 says the magazines are sent serially), a page ends at the next header of its magazine,
//! and every page is kept per subcode, with the last complete copy beside the one being received.

use super::hamming::parity;
use super::packet::{parse, pes_packets, Link, Packet, PageNo};
use super::page::{render, Page, RawPage};
use std::collections::{BTreeMap, VecDeque};

/// Counters of a [`PageStore`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Packets taken.
    pub packets: u64,
    /// Packets dropped for an address that failed Hamming 8/4.
    pub bad_address: u64,
    /// Characters with a parity error.
    pub parity_errors: u64,
    /// Pages completed.
    pub pages: u64,
}

struct Entry {
    live: RawPage,
    complete: RawPage,
    has_complete: bool,
}

/// The page store.
pub struct PageStore {
    pages: BTreeMap<(PageNo, u16), Box<Entry>>,
    open: [Option<(PageNo, u16)>; 9],
    magazine_designation: [Option<u8>; 9],
    completed: VecDeque<(PageNo, u16)>,
    cap: usize,
    version: u64,
    /// Counters.
    pub stats: Stats,
}

impl Default for PageStore {
    fn default() -> Self {
        Self::new(2048)
    }
}

/// How many completed pages are queued for [`PageStore::take_completed`].
const COMPLETED_CAP: usize = 256;

impl PageStore {
    /// A store keeping at most `cap` page and subpage entries (the least recently changed are dropped first).
    pub fn new(cap: usize) -> Self {
        PageStore {
            pages: BTreeMap::new(),
            open: [None; 9],
            magazine_designation: [None; 9],
            completed: VecDeque::with_capacity(COMPLETED_CAP),
            cap: cap.max(1),
            version: 0,
            stats: Stats::default(),
        }
    }

    /// Forgets everything (a new service).
    pub fn clear(&mut self) {
        let cap = self.cap;
        *self = Self::new(cap);
    }

    /// The packets of a teletext PES payload (EN 300 472).
    pub fn push_pes(&mut self, payload: &[u8]) {
        for p in pes_packets(payload) {
            self.push_packet(&p);
        }
    }

    /// One packet of 42 bytes (address and data, first bit sent in bit 0).
    pub fn push_packet(&mut self, b: &[u8; 42]) {
        let Ok(p) = parse(b) else {
            self.stats.bad_address += 1;
            return;
        };
        self.stats.packets += 1;
        match p {
            Packet::Header(h) => self.header(&h),
            Packet::Row {
                magazine,
                row,
                data,
            } => {
                // X/25 is not a display row at Level 1.
                if row > 24 {
                    return;
                }
                let (mut errors, mut changed) = (0, false);
                if let Some(page) = self.open_page(magazine) {
                    for (d, &x) in page.rows[row as usize].iter_mut().zip(data.iter()) {
                        match parity(x) {
                            Some(v) if *d != v => {
                                *d = v;
                                changed = true;
                            }
                            Some(_) => {}
                            None => errors += 1,
                        }
                    }
                    page.received |= 1 << row;
                }
                if changed {
                    self.bump(magazine);
                }
                self.stats.parity_errors += errors;
            }
            Packet::Triplets {
                magazine,
                row,
                designation,
                triplets,
            } => match row {
                26 => {
                    if let Some(page) = self.open_page(magazine) {
                        page.enhancements[designation as usize] =
                            triplets.map(|t| t.unwrap_or(u32::MAX));
                        page.enhanced |= 1 << designation;
                        self.bump(magazine);
                    }
                }
                28 | 29 if designation == 0 || designation == 4 => {
                    // Format 1 with page function 0 (a basic Level 1 page): triplet 1 bits 8 to 14 designate the default sets.
                    let Some(t1) = triplets[0] else { return };
                    if t1 & 15 != 0 {
                        return;
                    }
                    let d = ((t1 >> 7) & 0x7F) as u8;
                    if row == 29 {
                        self.magazine_designation[magazine as usize] = Some(d);
                    } else if let Some(page) = self.open_page(magazine) {
                        page.designation = Some(d);
                        self.bump(magazine);
                    }
                }
                _ => {}
            },
            Packet::Links {
                magazine,
                designation: 0,
                links,
            } => {
                if let Some(page) = self.open_page(magazine) {
                    // A link that failed Hamming 8/4 keeps the one already held.
                    for (dst, l) in page.links.iter_mut().zip(links) {
                        if l.is_some() {
                            *dst = l;
                        }
                    }
                    self.bump(magazine);
                }
            }
            _ => {}
        }
    }

    fn open_page(&mut self, magazine: u8) -> Option<&mut RawPage> {
        let key = self.open[magazine as usize]?;
        self.pages.get_mut(&key).map(|e| &mut e.live)
    }

    fn bump(&mut self, magazine: u8) {
        self.version += 1;
        let v = self.version;
        if let Some(p) = self.open_page(magazine) {
            p.version = v;
        }
    }

    fn close(&mut self, magazine: u8) {
        if let Some(key) = self.open[magazine as usize].take() {
            if let Some(e) = self.pages.get_mut(&key) {
                e.complete.clone_from(&e.live);
                e.has_complete = true;
                self.stats.pages += 1;
                if self.completed.len() == COMPLETED_CAP {
                    self.completed.pop_front();
                }
                self.completed.push_back(key);
            }
        }
    }

    fn header(&mut self, h: &super::packet::Header) {
        if h.control.serial {
            for m in 1..=8 {
                self.close(m);
            }
        } else {
            self.close(h.magazine);
        }
        let Some(page) = h.page() else { return };
        if page.decimal().is_none() {
            return; // a time filler or a data page: closes, opens nothing
        }
        let key = (page, h.subcode);
        if !self.pages.contains_key(&key) {
            if self.pages.len() >= self.cap {
                self.evict();
            }
            self.pages.insert(
                key,
                Box::new(Entry {
                    live: RawPage::new(page, h.subcode),
                    complete: RawPage::new(page, h.subcode),
                    has_complete: false,
                }),
            );
        }
        let mag_designation = self.magazine_designation[h.magazine as usize];
        let mut errors = 0;
        if let Some(e) = self.pages.get_mut(&key) {
            let p = &mut e.live;
            if h.control.erase {
                p.erase();
            }
            p.control = h.control;
            p.national = h.national;
            if p.designation.is_none() {
                p.designation = mag_designation;
            }
            for (d, &x) in p.rows[0][8..].iter_mut().zip(h.text.iter()) {
                match parity(x) {
                    Some(v) => *d = v,
                    None => errors += 1,
                }
            }
            p.received |= 1;
        }
        self.stats.parity_errors += errors;
        self.open[h.magazine as usize] = Some(key);
        self.bump(h.magazine);
    }

    fn evict(&mut self) {
        // Drop the least recently changed entry that is not open.
        let victim = self
            .pages
            .iter()
            .filter(|(k, _)| !self.open.contains(&Some(**k)))
            .min_by_key(|(_, e)| e.live.version)
            .map(|(k, _)| *k);
        if let Some(k) = victim {
            self.pages.remove(&k);
        }
    }

    /// The next page completed since the last call.
    pub fn take_completed(&mut self) -> Option<(PageNo, u16)> {
        self.completed.pop_front()
    }

    /// The pages held, in order (each once).
    pub fn pages(&self) -> impl Iterator<Item = PageNo> + '_ {
        let mut last = None;
        self.pages.keys().filter_map(move |&(p, _)| {
            let new = last != Some(p);
            last = Some(p);
            new.then_some(p)
        })
    }

    /// The subcodes held for a page, in order.
    pub fn subcodes(&self, page: PageNo) -> impl Iterator<Item = u16> + '_ {
        self.pages
            .range((page, 0)..=(page, u16::MAX))
            .map(|(&(_, s), _)| s)
    }

    /// A page: the last complete copy (or the copy being received when none has completed) of `subcode`, or of the most recently
    /// changed subpage. Links come from the copy being received.
    pub fn raw(
        &self,
        page: PageNo,
        subcode: Option<u16>,
    ) -> Option<(&RawPage, &[Option<Link>; 6])> {
        let e = match subcode {
            Some(s) => self.pages.get(&(page, s))?,
            None => {
                self.pages
                    .range((page, 0)..=(page, u16::MAX))
                    .max_by_key(|(_, e)| e.live.version)?
                    .1
            }
        };
        let shown = if e.has_complete { &e.complete } else { &e.live };
        Some((shown, &e.live.links))
    }

    /// Renders a page (see [`PageStore::raw`]) into `out`; `false` when the page is not held.
    pub fn render(&self, page: PageNo, subcode: Option<u16>, out: &mut Page) -> bool {
        match self.raw(page, subcode) {
            Some((p, links)) => {
                render(p, out);
                out.links = *links;
                true
            }
            None => false,
        }
    }

    /// The page offered for subtitles when the stream names none: 888, or 777 when only that is sent.
    pub fn default_subtitle_page(&self) -> PageNo {
        let p = |n| PageNo::from_decimal(n).unwrap_or_default();
        let has = |n| self.pages().any(|x| x == p(n));
        if has(888) || !has(777) {
            p(888)
        } else {
            p(777)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::packet::{
        build_filler, build_header, build_links, build_pes, build_row, build_triplets, triplet,
        Control,
    };
    use super::*;

    fn pn(s: &str) -> PageNo {
        PageNo::parse(s).unwrap()
    }

    fn feed(store: &mut PageStore, pk: &[[u8; 42]]) {
        let mut pes = vec![];
        build_pes(pk, &mut pes);
        store.push_pes(&pes);
    }

    fn row_text(store: &PageStore, page: &str, sub: Option<u16>, row: usize) -> String {
        let mut out = Page::default();
        assert!(store.render(pn(page), sub, &mut out));
        let mut s = String::new();
        out.row_text(row, &mut s);
        s
    }

    #[test]
    fn pages_assemble_and_complete_at_the_next_header() {
        let mut st = PageStore::default();
        let mut row1 = vec![0x02];
        row1.extend_from_slice(b"Hello");
        feed(
            &mut st,
            &[
                build_header(
                    pn("123"),
                    0,
                    Control::default(),
                    0,
                    b"HEADER TEXT 0123456789 abcdefghi",
                ),
                build_row(1, 1, &row1),
                build_header(pn("124"), 0, Control::default(), 0, b""),
            ],
        );
        assert_eq!(st.take_completed(), Some((pn("123"), 0)));
        assert_eq!(st.take_completed(), None);
        let t = row_text(&st, "123", None, 0);
        assert_eq!(&t[..8], "        ");
        assert_eq!(&t[8..], "HEADER TEXT 0123456789 abcdefghi");
        assert_eq!(&row_text(&st, "123", None, 1)[1..6], "Hello");
        assert_eq!(st.pages().collect::<Vec<_>>(), [pn("123"), pn("124")]);
    }

    #[test]
    fn subpages_links_and_fillers() {
        let mut st = PageStore::default();
        let l = |s: &str| {
            Some(Link {
                page: pn(s),
                subcode: 0x3F7F,
            })
        };
        feed(
            &mut st,
            &[
                build_header(pn("234"), 1, Control::default(), 0, b""),
                build_links(2, [l("234"), l("100"), None, l("899"), None, l("200")]),
                build_header(pn("234"), 2, Control::default(), 0, b""),
                build_row(2, 1, b"two"),
                build_filler(2),
            ],
        );
        assert_eq!(st.subcodes(pn("234")).collect::<Vec<_>>(), [1, 2]);
        let mut out = Page::default();
        st.render(pn("234"), Some(1), &mut out);
        assert_eq!(out.links[0].unwrap().page, pn("234"));
        assert_eq!(out.links[3].unwrap().page, pn("899"));
        assert_eq!(out.links[5].unwrap().page, pn("200"));
        st.render(pn("234"), None, &mut out);
        assert_eq!(out.subcode, 2);
        assert_eq!(st.pages().count(), 1);
        assert!(!st.render(pn("777"), None, &mut out));
        assert_eq!(st.default_subtitle_page(), pn("888"));
    }

    #[test]
    fn erase_and_parity_repair() {
        let mut st = PageStore::default();
        let mut bad = build_row(1, 1, b"Hello");
        bad[3] ^= 0x01; // 'e' with a parity error
        feed(
            &mut st,
            &[
                build_header(pn("100"), 0, Control::default(), 0, b""),
                bad,
                build_filler(1),
            ],
        );
        assert_eq!(&row_text(&st, "100", None, 1)[..5], "H llo");
        assert_eq!(st.stats.parity_errors, 1);
        feed(
            &mut st,
            &[
                build_header(pn("100"), 0, Control::default(), 0, b""),
                build_row(1, 1, b"Hello"),
                build_filler(1),
            ],
        );
        assert_eq!(&row_text(&st, "100", None, 1)[..5], "Hello");
        let erase = Control {
            erase: true,
            ..Control::default()
        };
        feed(
            &mut st,
            &[build_header(pn("100"), 0, erase, 0, b""), build_filler(1)],
        );
        assert!(row_text(&st, "100", None, 1).trim().is_empty());
        // A bad address is counted.
        let mut b = build_row(1, 1, b"x");
        b[0] ^= 3;
        st.push_packet(&b);
        assert_eq!(st.stats.bad_address, 1);
    }

    #[test]
    fn serial_mode_and_magazines_in_parallel() {
        let mut st = PageStore::default();
        // Parallel: rows of magazine 2 go to page 200 even with page 100 open.
        feed(
            &mut st,
            &[
                build_header(pn("100"), 0, Control::default(), 0, b""),
                build_header(pn("200"), 0, Control::default(), 0, b""),
                build_row(1, 1, b"one"),
                build_row(2, 1, b"two"),
            ],
        );
        assert!(row_text(&st, "100", None, 1).starts_with("one"));
        assert!(row_text(&st, "200", None, 1).starts_with("two"));
        // Serial: a header closes every magazine.
        let serial = Control {
            serial: true,
            ..Control::default()
        };
        feed(&mut st, &[build_header(pn("300"), 0, serial, 0, b"")]);
        let mut done = vec![];
        while let Some(p) = st.take_completed() {
            done.push(p.0.to_string());
        }
        assert_eq!(done, ["100", "200"]);
    }

    #[test]
    fn x26_and_x28_reach_the_page() {
        let mut st = PageStore::default();
        // X/28/0 format 1: page function 0, designation group 0001 with C12-C14 000: Polish.
        let t1 = 0b000_1000_u32 << 7;
        feed(
            &mut st,
            &[
                build_header(pn("150"), 0, Control::default(), 0, b""),
                build_triplets(1, 28, 0, &[t1]),
                build_row(1, 3, &[0x7B, b'a']),
                build_triplets(1, 26, 0, &[triplet(43, 0x04, 1), triplet(1, 0x1E, b'a')]),
                build_filler(1),
            ],
        );
        let t = row_text(&st, "150", None, 3);
        // 0x7B is ż in the Polish set; mode 0x1E is ogonek on 'a'.
        assert!(t.starts_with("żą"), "{t}");
    }

    #[test]
    fn the_cap_drops_the_oldest() {
        let mut st = PageStore::new(3);
        for n in [100u16, 101, 102, 103] {
            feed(
                &mut st,
                &[build_header(
                    PageNo::from_decimal(n).unwrap(),
                    0,
                    Control::default(),
                    0,
                    b"",
                )],
            );
        }
        assert_eq!(
            st.pages().map(|p| p.to_string()).collect::<Vec<_>>(),
            ["101", "102", "103"]
        );
    }
}
