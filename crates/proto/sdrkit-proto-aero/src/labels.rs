//! ARINC 620 label meanings.
//!
//! The label is the two characters after the acknowledgement character. It identifies the kind of message a block holds. ARINC 620
//! ("Datalink Ground System Standard and Interface Specification") defines the labels. ARINC 620 is not freely available;
//! [`Label::verified`] is true for an entry confirmed by the standard.
//!
//! Where sources differ, [`Label::alternative`] holds the second meaning. A display can show both.
//!
//! ```
//! use sdrkit_proto_aero::labels;
//! assert_eq!(labels::meaning("B6"), Some("ADS-C downlink"));
//! assert_eq!(labels::find("10").unwrap().alternative, Some("Arrival report"));
//! assert!(labels::find("ZZ").is_none());
//! ```

/// One label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label {
    /// The two characters, with `_d` for `_` DEL.
    pub code: &'static str,
    /// The meaning.
    pub meaning: &'static str,
    /// A second meaning, when the source lists disagree.
    pub alternative: Option<&'static str>,
    /// Whether the entry is confirmed by the ARINC 620 text.
    pub verified: bool,
}

const fn l(code: &'static str, meaning: &'static str, alternative: Option<&'static str>) -> Label {
    Label {
        code,
        meaning,
        alternative,
        verified: false,
    }
}

/// Every label, sorted by code (byte order).
pub static LABELS: &[Label] = &[
    l("00", "Emergency situation report", None),
    l("10", "Free text (aircraft)", Some("Arrival report")),
    l("11", "Weather request", None),
    l("12", "Weather report", None),
    l("13", "Out-off-on-in report", None),
    l("14", "Out report", None),
    l("15", "Off report", Some("Flight status report")),
    l("16", "Position report (weather)", None),
    l("17", "In report", Some("ETA report")),
    l("20", "Position report (delay)", None),
    l("21", "Position report", None),
    l("22", "Ground position report", None),
    l("23", "On report", None),
    l("2N", "Takeoff report", None),
    l("2Z", "Progress report", None),
    l("30", "Fuel report", None),
    l("33", "Arrival information", None),
    l("36", "Diversion report", None),
    l("37", "Delay report", None),
    l("38", "Out report (resend)", None),
    l("39", "Off report (resend)", None),
    l("41", "In report (resend)", None),
    l(
        "44",
        "Position report (downlink)",
        Some("Position or ETA report"),
    ),
    l("45", "Out report (with fuel)", None),
    l("48", "Delay message", None),
    l("4M", "Cargo information", None),
    l("52", "Ground UTC request", None),
    l("54", "Voice contact request", None),
    l("57", "Pilot report", None),
    l("5U", "Weather request", None),
    l("5Z", "Airline designated downlink", None),
    l("7A", "Engine data", None),
    l("7B", "Engine data (extended)", None),
    l("80", "Airline defined (3 character header)", None),
    l("83", "Fuel and position report", None),
    l("8S", "Airline designated", None),
    l("A0", "ATIS facility notification", None),
    l("A1", "Oceanic clearance", None),
    l("A2", "Unassigned", None),
    l("A3", "Departure clearance response", None),
    l("A4", "Flight systems message", None),
    l("A5", "Request position report", None),
    l("A6", "ADS-C uplink", None),
    l("A7", "Free text to aircraft", None),
    l("A8", "Deliver departure slot", None),
    l("A9", "ATIS report", None),
    l("AA", "ATC communications (uplink)", None),
    l("AB", "TWIP report", None),
    l("AC", "Pushback clearance", None),
    l("AD", "Expected taxi clearance", None),
    l("AE", "Unassigned", None),
    l("AF", "CPC command response", None),
    l("B0", "ATS facility notification", None),
    l("B1", "Oceanic clearance request", None),
    l("B2", "Oceanic clearance readback", None),
    l("B3", "Departure clearance request", None),
    l("B4", "Departure clearance readback", None),
    l("B5", "Unassigned", None),
    l("B6", "ADS-C downlink", None),
    l("B7", "Unassigned", None),
    l("B8", "Request departure slot", None),
    l("B9", "ATIS request", None),
    l("BA", "ATC communications (downlink)", None),
    l("BB", "TWIP request", None),
    l("BC", "Pushback clearance request", None),
    l("BD", "Expected taxi clearance request", None),
    l("BE", "CPC logon / logoff request", None),
    l("BF", "CPC WILCO / UNABLE response", None),
    l("C0", "Uplink message to all cockpit printers", None),
    l("C1", "Uplink message to cockpit printer 1", None),
    l("C2", "Uplink message to cockpit printer 2", None),
    l("C3", "Uplink message to cockpit printer 3", None),
    l("CA", "Printer error", None),
    l("CB", "Printer busy", None),
    l("CC", "Printer local / remote", None),
    l("CD", "Printer no paper", None),
    l("CE", "Printer test", None),
    l("CF", "Printer recovery", None),
    l("F3", "Dedicated transceiver advisory", None),
    l("H1", "Message to / from terminal", None),
    l("H2", "Meteorological report", None),
    l("H3", "Icing report", None),
    l("H4", "Flight management computer", None),
    l("HX", "Undelineated terminal message", None),
    l("M1", "IATA departure message", None),
    l("M2", "IATA arrival message", None),
    l("Q0", "Link test", None),
    l(
        "Q1",
        "ETA departure / arrival report",
        Some("OOOI report (out, off, on, in)"),
    ),
    l("Q2", "ETA report", None),
    l("Q3", "Clock update", None),
    l("Q4", "Voice circuit busy", None),
    l("Q5", "Unable to process uplink", None),
    l("Q6", "Voice to ACARS change-over", None),
    l("Q7", "Delay message", None),
    l("QA", "Out / fuel report", None),
    l("QB", "Off report", None),
    l("QC", "On report", None),
    l("QD", "In / fuel report", None),
    l("QE", "Out / fuel / destination report", None),
    l("QF", "Off / destination report", None),
    l("QG", "Out / return in report", None),
    l("QH", "Out report", None),
    l("QK", "Landing report", None),
    l(
        "QL",
        "Arrival report",
        Some("Gate in, origin and destination"),
    ),
    l(
        "QM",
        "Arrival information report",
        Some("Origin and destination"),
    ),
    l("QN", "Diversion report", Some("Destination and ETA")),
    l("QP", "Out report", None),
    l("QQ", "Off report", None),
    l("QR", "On report", None),
    l("QS", "In report", None),
    l("QT", "Out / return in report", None),
    l("RA", "Command aircraft terminal to transmit data", None),
    l("RB", "Response of aircraft terminal", None),
    l("S1", "Network statistics", None),
    l("SA", "Media advisory", None),
    l("SQ", "Squitter", None),
    l("X1", "Service provider defined", None),
    l("_d", "General response", None),
];

/// The entry of a label, or `None` when the table does not hold it.
pub fn find(code: &str) -> Option<&'static Label> {
    LABELS
        .binary_search_by(|x| x.code.cmp(code))
        .ok()
        .map(|i| &LABELS[i])
}

/// The meaning of a label, or `None` when the table does not hold it.
pub fn meaning(code: &str) -> Option<&'static str> {
    find(code).map(|x| x.meaning)
}
