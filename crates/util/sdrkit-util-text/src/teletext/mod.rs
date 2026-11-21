//! Teletext (ETS 300 706, carried in DVB by ETSI EN 300 472 and in the vertical blanking of 625-line television by ITU-R BT.653
//! system B): the protection codes, packets, page and subpage assembly, the Latin character sets with the national option sub-sets
//! and the G2 set, and the page model a 40 × 25 renderer draws, at presentation Level 1.5.
//!
//! The same code serves a DVB stream ([`PageStore::push_pes`]) and an analogue slicer ([`PageStore::push_packet`] with the 42
//! bytes after the framing code).
//!
//! ```
//! use sdrkit_util_text::teletext::{packet, Page, PageNo, PageStore};
//!
//! let p100 = PageNo::from_decimal(100).unwrap();
//! let mut store = PageStore::default();
//! store.push_packet(&packet::build_header(p100, 0, Default::default(), 0, b"SDRKIT"));
//! store.push_packet(&packet::build_row(1, 1, b"Hello"));
//! let mut page = Page::default();
//! assert!(store.render(p100, None, &mut page));
//! let mut row = String::new();
//! page.row_text(1, &mut row);
//! assert!(row.starts_with("Hello"));
//! ```
//!
//! Not covered: the Cyrillic, Greek, Arabic and Hebrew G0 and G2 sets (their designation is recognised by
//! [`charset::designation`], the text is shown with the Latin set), Level 2.5 and 3.5 enhancements (objects, DRCS, colour tables,
//! side panels), and the second G0 set reached with ESC.

pub mod charset;
pub mod hamming;
pub mod packet;
pub mod page;
pub mod store;

pub use packet::{Link, PageNo};
pub use page::{Cell, Glyph, Page, RawPage, Size};
pub use store::PageStore;
