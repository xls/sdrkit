//! Character repertoires and glyph cells of the stacks that send text as dot pictures.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`gb2312`] | the 7445 characters of GB 2312 as Unicode, by their EUC-CN codes |
//! | [`cell`] | 16 × 16 dot cells from an HZK16 file, a TrueType font, or the shared Hell glyphs, and the box of a missing character |

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod cell;
pub mod gb2312;
