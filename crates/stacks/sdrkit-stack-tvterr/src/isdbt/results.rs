//! What the ISDB-T receiver publishes: the choices of its fields, the TMCC event, and the declarations of the report
//! and of the two stacks, made from the shapes of [`crate::typed`].

use super::tmcc::{LayerParams, Tmcc};
use crate::typed::{IsdbtFields, IsdbtNarrowFields, IsdbtTmccEvent, ISDBT_LAYERS, ISDBT_SERIES};
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::{Record, ResultsDecl};
use std::fmt::Write;

sdrkit_core::choice_enum! {
    /// The carrier modulation of a layer.
    pub enum Modulation {
        /// Differential QPSK.
        Dqpsk = ("dqpsk", "DQPSK"),
        /// QPSK.
        Qpsk = ("qpsk", "QPSK"),
        /// 16-QAM.
        Qam16 = ("16qam", "16QAM"),
        /// 64-QAM.
        Qam64 = ("64qam", "64QAM"),
    }
}

sdrkit_core::choice_enum! {
    /// The inner code rate of a layer.
    pub enum CodeRate {
        /// 1/2.
        R12 = ("1/2", "1/2"),
        /// 2/3.
        R23 = ("2/3", "2/3"),
        /// 3/4.
        R34 = ("3/4", "3/4"),
        /// 5/6.
        R56 = ("5/6", "5/6"),
        /// 7/8.
        R78 = ("7/8", "7/8"),
    }
}

sdrkit_core::choice_enum! {
    /// A hierarchical layer.
    pub enum LayerName {
        /// Layer A.
        A = ("a", "A"),
        /// Layer B.
        B = ("b", "B"),
        /// Layer C.
        C = ("c", "C"),
    }
}

impl LayerName {
    /// The layer of index 0, 1 or 2 (C for a larger index).
    pub fn of(index: usize) -> LayerName {
        match index {
            0 => LayerName::A,
            1 => LayerName::B,
            _ => LayerName::C,
        }
    }

    /// The row key of the layer in [`LAYERS`].
    pub fn key(self) -> &'static str {
        match self {
            LayerName::A => "A",
            LayerName::B => "B",
            LayerName::C => "C",
        }
    }
}

/// The modulation of a used layer (ARIB STD-B31 table 3-24 codes 0 to 3).
pub fn modulation_of(p: &LayerParams) -> Option<Modulation> {
    match p.modulation {
        0 => Some(Modulation::Dqpsk),
        1 => Some(Modulation::Qpsk),
        2 => Some(Modulation::Qam16),
        3 => Some(Modulation::Qam64),
        _ => None,
    }
}

/// The code rate of a used layer (ARIB STD-B31 table 3-25 codes 0 to 4).
pub fn code_rate_of(p: &LayerParams) -> Option<CodeRate> {
    match p.code_rate {
        0 => Some(CodeRate::R12),
        1 => Some(CodeRate::R23),
        2 => Some(CodeRate::R34),
        3 => Some(CodeRate::R56),
        4 => Some(CodeRate::R78),
        _ => None,
    }
}

/// Writes the fields of the event `isdbt.tmcc` for `t` into `r`. The text lists the layers in use
/// ("A 1 QPSK 2/3, B 12 64QAM 3/4").
pub fn write_tmcc(t: &Tmcc, r: &mut Record) {
    let mut text = String::new();
    for (li, l) in t
        .current
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.used())
    {
        if !text.is_empty() {
            text.push_str(", ");
        }
        let _ = write!(
            text,
            "{} {} {} {}",
            LayerName::of(li).key(),
            l.segments,
            l.modulation_name(),
            l.rate_name()
        );
    }
    IsdbtTmccEvent {
        system_id: i64::from(t.system),
        switch_countdown: i64::from(t.countdown),
        emergency_alarm: t.emergency,
        partial_reception: t.current.partial,
        text: &text,
        urgency: if t.emergency {
            Urgency::Emergency
        } else {
            Urgency::None
        },
        family: Family::Broadcast,
    }
    .write(r);
}

/// What `isdbt.report` publishes: the fields, the layer table and the plots.
pub fn report_declaration() -> ResultsDecl {
    ISDBT_SERIES.iter().fold(
        ResultsDecl::new()
            .with_fields(IsdbtFields::SHAPE.fields)
            .with_table(&ISDBT_LAYERS),
        ResultsDecl::with_series,
    )
}

/// The report of a narrow-band (ISDB-TSB) receiver: the ISDB-T report with the segment count in use and the note.
pub fn narrow_report_declaration() -> ResultsDecl {
    report_declaration().with_fields(IsdbtNarrowFields::SHAPE.fields)
}

/// What the ISDB-TSB stack publishes.
pub fn narrow_declaration() -> ResultsDecl {
    crate::typed::shape(crate::isdbtsb::ID).results_decl()
}

/// What the ISDB-T stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(super::ID).results_decl()
}
