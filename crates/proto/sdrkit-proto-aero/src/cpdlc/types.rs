//! The parameter types of FANS-1/A CPDLC (RTCA DO-258) as unaligned PER tables, with their labels and print scales.

use sdrkit_util_bits::per_schema::{Field, Num, Ty, Value};
use std::fmt::Write as _;

const fn int(lo: i64, hi: i64) -> Ty {
    Ty::Int {
        lo,
        hi,
        num: Num::PLAIN,
    }
}

const fn scaled(lo: i64, hi: i64, mul: i64, dec: u8, unit: &'static str) -> Ty {
    Ty::Int {
        lo,
        hi,
        num: Num::new(mul, dec, unit),
    }
}

const fn f(label: &'static str, ty: &'static Ty) -> Field {
    Field::new(label, ty)
}

const fn o(label: &'static str, ty: &'static Ty) -> Field {
    Field::opt(label, ty)
}

fn clock(v: &Value, out: &mut String) {
    if let Value::Seq(items) = v {
        for (i, x) in items.iter().enumerate() {
            if i > 0 {
                out.push(':');
            }
            let _ = write!(out, "{:02}", x.as_ref().and_then(Value::int).unwrap_or(0));
        }
    }
}

fn digits(v: &Value, out: &mut String) {
    if let Value::List(items) = v {
        for x in items {
            let _ = write!(out, "{}", x.int().unwrap_or(0));
        }
    }
}

fn coordinate(v: &Value, out: &mut String, width: usize, names: [&str; 2]) {
    let deg = v.get(0).and_then(Value::int).unwrap_or(0);
    let dir = match v.get(2) {
        Some(Value::Enum(i)) => names[(*i).min(1)],
        _ => "",
    };
    match v.get(1).and_then(Value::int) {
        Some(m) => {
            let _ = write!(out, "{deg:0width$} {:02}.{}' {dir}", m / 10, m % 10);
        }
        None => {
            let _ = write!(out, "{deg:0width$} deg {dir}");
        }
    }
}

fn latitude(v: &Value, out: &mut String) {
    coordinate(v, out, 2, ["north", "south"]);
}

fn longitude(v: &Value, out: &mut String) {
    coordinate(v, out, 3, ["east", "west"]);
}

// Altitude, time, position.

static QNH_FT: Ty = scaled(0, 2500, 10, 0, "ft");
static QNH_M: Ty = scaled(0, 16000, 1, 0, "m");
static QFE_FT: Ty = scaled(0, 2100, 10, 0, "ft");
static QFE_M: Ty = scaled(0, 7000, 1, 0, "m");
static GNSS_FT: Ty = scaled(0, 150_000, 1, 0, "ft");
static GNSS_M: Ty = scaled(0, 50_000, 1, 0, "m");
static FL: Ty = int(30, 600);
static FL_M: Ty = scaled(100, 2000, 10, 0, "m");
/// Altitude: CHOICE of 8.
pub static ALTITUDE: Ty = Ty::Choice(&[
    f("Altitude (QNH)", &QNH_FT),
    f("Altitude (QNH)", &QNH_M),
    f("Altitude (QFE)", &QFE_FT),
    f("Altitude (QFE)", &QFE_M),
    f("Altitude (GNSS)", &GNSS_FT),
    f("Altitude (GNSS)", &GNSS_M),
    f("Flight level", &FL),
    f("Flight level", &FL_M),
]);
static ALTITUDE2: Ty = Ty::List {
    lo: 2,
    hi: 2,
    item: &f("", &ALTITUDE),
};

static HOURS: Ty = int(0, 23);
static MINUTES: Ty = int(0, 59);
static HM: Ty = Ty::Seq(&[f("Hours", &HOURS), f("Minutes", &MINUTES)]);
static HMS: Ty = Ty::Seq(&[
    f("Hours", &HOURS),
    f("Minutes", &MINUTES),
    f("Seconds", &MINUTES),
]);
/// Time: hours and minutes, printed HH:MM.
pub static TIME: Ty = Ty::Show(&HM, clock);
/// Timestamp: hours, minutes and seconds, printed HH:MM:SS.
pub static TIMESTAMP: Ty = Ty::Show(&HMS, clock);
static TIME2: Ty = Ty::List {
    lo: 2,
    hi: 2,
    item: &f("Time", &TIME),
};

static FIX: Ty = Ty::Ia5 { lo: 1, hi: 5 };
static NAVAID: Ty = Ty::Ia5 { lo: 1, hi: 4 };
static AIRPORT: Ty = Ty::Ia5 { lo: 4, hi: 4 };
static LAT_DEG: Ty = int(0, 90);
static LON_DEG: Ty = int(0, 180);
static TENTHS_MIN: Ty = int(0, 599);
static NS: Ty = Ty::Enum(&["north", "south"]);
static EW: Ty = Ty::Enum(&["east", "west"]);
static LAT_SEQ: Ty = Ty::Seq(&[
    f("Degrees", &LAT_DEG),
    o("Minutes", &TENTHS_MIN),
    f("Direction", &NS),
]);
static LON_SEQ: Ty = Ty::Seq(&[
    f("Degrees", &LON_DEG),
    o("Minutes", &TENTHS_MIN),
    f("Direction", &EW),
]);
static LATITUDE: Ty = Ty::Show(&LAT_SEQ, latitude);
static LONGITUDE: Ty = Ty::Show(&LON_SEQ, longitude);
static LATLON: Ty = Ty::Seq(&[f("Latitude", &LATITUDE), f("Longitude", &LONGITUDE)]);

static DEG: Ty = scaled(1, 360, 1, 0, "deg");
/// Degrees: magnetic or true.
pub static DEGREES: Ty = Ty::Choice(&[f("Degrees (magnetic)", &DEG), f("Degrees (true)", &DEG)]);
static DIST_NM: Ty = scaled(0, 9999, 1, 1, "nm");
static DIST_KM: Ty = scaled(1, 1024, 1, 0, "km");
static DISTANCE: Ty = Ty::Choice(&[f("Distance", &DIST_NM), f("Distance", &DIST_KM)]);
static PBD: Ty = Ty::Seq(&[
    f("Fix", &FIX),
    o("", &LATLON),
    f("", &DEGREES),
    f("", &DISTANCE),
]);
static PB: Ty = Ty::Seq(&[f("Fix", &FIX), o("", &LATLON), f("", &DEGREES)]);
static PBPB: Ty = Ty::List {
    lo: 2,
    hi: 2,
    item: &f("Place and bearing", &PB),
};
/// Position: CHOICE of 5.
pub static POSITION: Ty = Ty::Choice(&[
    f("Fix", &FIX),
    f("Navaid", &NAVAID),
    f("Airport", &AIRPORT),
    f("", &LATLON),
    f("Place, bearing and distance", &PBD),
]);
static POSITION2: Ty = Ty::List {
    lo: 2,
    hi: 2,
    item: &f("Position", &POSITION),
};

// Offsets, speeds, rates.

static OFF_NM: Ty = scaled(1, 128, 1, 0, "nm");
static OFF_KM: Ty = scaled(1, 256, 1, 0, "km");
static DISTANCE_OFFSET: Ty = Ty::Choice(&[f("Offset", &OFF_NM), f("Offset", &OFF_KM)]);
static DIRECTION: Ty = Ty::Enum(&[
    "left",
    "right",
    "either side",
    "north",
    "south",
    "east",
    "west",
    "north-east",
    "north-west",
    "south-east",
    "south-west",
]);
static IAS: Ty = scaled(7, 38, 10, 0, "kts");
static IAS_M: Ty = scaled(10, 137, 10, 0, "km/h");
static TAS: Ty = scaled(7, 70, 10, 0, "kts");
static TAS_M: Ty = scaled(10, 137, 10, 0, "km/h");
static GS: Ty = scaled(7, 70, 10, 0, "kts");
static GS_M: Ty = scaled(10, 265, 10, 0, "km/h");
static MACH: Ty = scaled(61, 92, 1, 2, "");
static MACH_L: Ty = scaled(93, 604, 1, 2, "");
/// Speed: CHOICE of 8.
pub static SPEED: Ty = Ty::Choice(&[
    f("Indicated airspeed", &IAS),
    f("Indicated airspeed", &IAS_M),
    f("True airspeed", &TAS),
    f("True airspeed", &TAS_M),
    f("Ground speed", &GS),
    f("Ground speed", &GS_M),
    f("Mach number", &MACH),
    f("Mach number", &MACH_L),
]);
static SPEED2: Ty = Ty::List {
    lo: 2,
    hi: 2,
    item: &f("", &SPEED),
};
static VR_FT: Ty = scaled(0, 60, 100, 0, "ft/min");
static VR_M: Ty = scaled(0, 200, 10, 0, "m/min");
static VERTICAL_RATE: Ty = Ty::Choice(&[f("Vertical rate", &VR_FT), f("Vertical rate", &VR_M)]);

// Frequencies and facilities.

static HF: Ty = scaled(2850, 28000, 1, 0, "kHz");
static VHF: Ty = scaled(117_000, 138_000, 1, 3, "MHz");
static UHF: Ty = scaled(225_000, 399_975, 1, 3, "MHz");
static SATCH: Ty = Ty::Digits { lo: 12, hi: 12 };
/// Frequency: CHOICE of 4.
pub static FREQUENCY: Ty = Ty::Choice(&[
    f("HF", &HF),
    f("VHF", &VHF),
    f("UHF", &UHF),
    f("Satcom channel", &SATCH),
]);
static FACILITY_NAME: Ty = Ty::Ia5 { lo: 3, hi: 18 };
static FACILITY_ID: Ty = Ty::Choice(&[
    f("Facility designation", &AIRPORT),
    f("Facility Name", &FACILITY_NAME),
]);
static FACILITY_FUNCTION: Ty = Ty::Enum(&[
    "center",
    "approach",
    "tower",
    "final",
    "ground control",
    "clearance delivery",
    "departure",
    "control",
]);
static UNIT_NAME: Ty = Ty::Seq(&[
    f("", &FACILITY_ID),
    f("Facility function", &FACILITY_FUNCTION),
]);
static TP4: Ty = Ty::Enum(&["labelA", "labelB"]);
static OCTAL: Ty = int(0, 7);
static BEACON_LIST: Ty = Ty::List {
    lo: 4,
    hi: 4,
    item: &f("", &OCTAL),
};
static BEACON: Ty = Ty::Show(&BEACON_LIST, digits);
static ATIS: Ty = Ty::Ia5 { lo: 1, hi: 1 };
static ERROR_INFO: Ty = Ty::Enum(&[
    "application error",
    "duplicate message identification number",
    "unrecognized message reference number",
    "end service with pending messages",
    "end service with no valid response",
    "insufficient message storage capacity",
    "no available message identification number",
    "commanded termination",
    "insufficient data",
    "unexpected data",
    "invalid data",
    "reserved 1",
    "reserved 2",
    "reserved 3",
    "reserved 4",
    "reserved 5",
    "reserved 6",
]);
/// Free text: IA5String SIZE (1..256).
pub static FREE_TEXT: Ty = Ty::Ia5 { lo: 1, hi: 256 };
static TO_FROM: Ty = Ty::Enum(&["to", "from"]);
static ALTIM_IN: Ty = scaled(2200, 3200, 1, 2, "inHg");
static ALTIM_HPA: Ty = scaled(7500, 12500, 1, 1, "hPa");
static ALTIMETER: Ty = Ty::Choice(&[f("Altimeter", &ALTIM_IN), f("Altimeter", &ALTIM_HPA)]);

// Procedures, holds, reports.

static PROC_TYPE: Ty = Ty::Enum(&["arrival", "approach", "departure"]);
static PROC_NAME: Ty = Ty::Ia5 { lo: 1, hi: 6 };
static PROCEDURE: Ty = Ty::Seq(&[
    f("Procedure type", &PROC_TYPE),
    f("Procedure name", &PROC_NAME),
    o("Procedure transition", &FIX),
]);
static LEG_NM: Ty = scaled(1, 999, 1, 1, "nm");
static LEG_KM: Ty = scaled(1, 128, 1, 0, "km");
static LEG_DISTANCE: Ty = Ty::Choice(&[f("Leg distance", &LEG_NM), f("Leg distance", &LEG_KM)]);
static LEG_TIME: Ty = scaled(1, 99, 1, 1, "min");
static LEG_TYPE: Ty = Ty::Choice(&[f("", &LEG_DISTANCE), f("Leg time", &LEG_TIME)]);
static HOLD: Ty = Ty::Seq(&[
    f("", &POSITION),
    f("", &ALTITUDE),
    f("", &DEGREES),
    f("Direction", &DIRECTION),
    o("", &LEG_TYPE),
]);
static FUEL: Ty = Ty::Show(&HM, clock);
static SOULS: Ty = int(1, 1024);
static VERSION: Ty = int(0, 15);
static TEMP_C: Ty = scaled(-80, 47, 1, 0, "C");
static TEMP_F: Ty = scaled(-105, 150, 1, 0, "F");
static TEMPERATURE: Ty = Ty::Choice(&[f("Temperature", &TEMP_C), f("Temperature", &TEMP_F)]);
static WIND_KT: Ty = scaled(0, 255, 1, 0, "kts");
static WIND_KMH: Ty = scaled(0, 511, 1, 0, "km/h");
static WIND_SPEED: Ty = Ty::Choice(&[f("Wind speed", &WIND_KT), f("Wind speed", &WIND_KMH)]);
static WINDS: Ty = Ty::Seq(&[f("Wind direction", &DEG), f("", &WIND_SPEED)]);
static TURBULENCE: Ty = Ty::Enum(&["light", "moderate", "severe"]);
static ICING: Ty = Ty::Enum(&["trace", "light", "moderate", "severe"]);
static UP_DOWN: Ty = Ty::Enum(&["up", "down"]);
static VERTICAL_CHANGE: Ty = Ty::Seq(&[
    f("Vertical direction", &UP_DOWN),
    f("", &VERTICAL_RATE),
]);
static POSITION_REPORT: Ty = Ty::Seq(&[
    f("Current position", &POSITION),
    f("Time at current position", &TIME),
    f("", &ALTITUDE),
    o("Next fix", &POSITION),
    o("ETA at next fix", &TIME),
    o("Next+1 fix", &POSITION),
    o("ETA at destination", &TIME),
    o("Remaining fuel", &FUEL),
    o("", &TEMPERATURE),
    o("", &WINDS),
    o("Turbulence", &TURBULENCE),
    o("Icing", &ICING),
    o("", &SPEED),
    o("Ground speed", &GS),
    o("", &VERTICAL_CHANGE),
    o("Track angle", &DEGREES),
    o("True heading", &DEGREES),
    o("", &DISTANCE),
    o("Supplementary information", &FREE_TEXT),
    o("Reported waypoint position", &POSITION),
    o("Reported waypoint time", &TIME),
    o("Reported waypoint altitude", &ALTITUDE),
]);

// Route clearances.

static RUNWAY_DIR: Ty = int(1, 36);
static RUNWAY_CONF: Ty = Ty::Enum(&["left", "right", "center", "none"]);
static RUNWAY: Ty = Ty::Seq(&[
    f("Runway direction", &RUNWAY_DIR),
    f("Runway configuration", &RUNWAY_CONF),
]);
static PUBLISHED_ID: Ty = Ty::Seq(&[f("Fix", &FIX), o("", &LATLON)]);
static TRACK_NAME: Ty = Ty::Ia5 { lo: 3, hi: 6 };
static LATLON_LIST: Ty = Ty::List {
    lo: 1,
    hi: 128,
    item: &f("", &LATLON),
};
static TRACK_DETAIL: Ty = Ty::Seq(&[
    f("Track name", &TRACK_NAME),
    f("Coordinate list", &LATLON_LIST),
]);
static ROUTE_INFO: Ty = Ty::Choice(&[
    f("Published identifier", &PUBLISHED_ID),
    f("", &LATLON),
    f("", &PBPB),
    f("Place, bearing and distance", &PBD),
    f("Airway ID", &FIX),
    f("Track detail", &TRACK_DETAIL),
]);
static ROUTE: Ty = Ty::List {
    lo: 1,
    hi: 128,
    item: &f("", &ROUTE_INFO),
};
static PLUS_MINUS: Ty = Ty::Enum(&["plus", "minus"]);
static ATW_DISTANCE: Ty = Ty::Seq(&[
    f("ATW distance tolerance", &PLUS_MINUS),
    f("", &DISTANCE),
]);
static AT_ABOVE_BELOW: Ty = Ty::Enum(&["at", "at or above", "at or below"]);
static ATW_ALTITUDE: Ty = Ty::Seq(&[
    f("ATW altitude tolerance", &AT_ABOVE_BELOW),
    f("", &ALTITUDE),
]);
static ATW_ALTITUDES: Ty = Ty::List {
    lo: 1,
    hi: 2,
    item: &f("", &ATW_ALTITUDE),
};
static ATW: Ty = Ty::Seq(&[
    f("", &POSITION),
    f("", &ATW_DISTANCE),
    o("", &SPEED),
    o("", &ATW_ALTITUDES),
]);
static ATW_LIST: Ty = Ty::List {
    lo: 1,
    hi: 8,
    item: &f("Along-track waypoint", &ATW),
};
static RP_LAT: Ty = Ty::Seq(&[f("Latitude direction", &NS), f("Latitude", &LAT_DEG)]);
static RP_LON: Ty = Ty::Seq(&[f("Longitude direction", &EW), f("Longitude", &LON_DEG)]);
static RP_CHOICE: Ty = Ty::Choice(&[f("", &RP_LAT), f("", &RP_LON)]);
static DEG_INCREMENT: Ty = scaled(1, 20, 1, 0, "deg");
static REPORTING_POINTS: Ty = Ty::Seq(&[
    f("", &RP_CHOICE),
    o("Degree increment", &DEG_INCREMENT),
]);
static INTERCEPT_FROM: Ty = Ty::Choice(&[
    f("Published identifier", &PUBLISHED_ID),
    f("", &LATLON),
    f("", &PBPB),
    f("Place, bearing and distance", &PBD),
]);
static INTERCEPT: Ty = Ty::Seq(&[f("", &INTERCEPT_FROM), f("", &DEGREES)]);
static INTERCEPT_LIST: Ty = Ty::List {
    lo: 1,
    hi: 4,
    item: &f("Intercept course", &INTERCEPT),
};
static HOLD_AT: Ty = Ty::Seq(&[
    f("", &POSITION),
    o("Holding speed (min)", &SPEED),
    o("", &ATW_ALTITUDE),
    o("Holding speed (max)", &SPEED),
    o("Direction", &DIRECTION),
    o("", &DEGREES),
    o("Expect further clearance at", &TIME),
    o("", &LEG_TYPE),
]);
static HOLD_LIST: Ty = Ty::List {
    lo: 1,
    hi: 8,
    item: &f("Holding point", &HOLD_AT),
};
static WSA: Ty = Ty::Seq(&[f("", &POSITION), o("", &SPEED), o("", &ATW_ALTITUDES)]);
static WSA_LIST: Ty = Ty::List {
    lo: 1,
    hi: 32,
    item: &f("Waypoint", &WSA),
};
static AT_AFTER_BEFORE: Ty = Ty::Enum(&["at", "at or after", "at or before"]);
static RTA_TIME: Ty = Ty::Seq(&[f("Time", &TIME), f("Time tolerance", &AT_AFTER_BEFORE)]);
static RTA_TOLERANCE: Ty = scaled(1, 150, 1, 1, "min");
static RTA: Ty = Ty::Seq(&[
    f("", &POSITION),
    f("", &RTA_TIME),
    o("RTA tolerance", &RTA_TOLERANCE),
]);
static RTA_LIST: Ty = Ty::List {
    lo: 1,
    hi: 32,
    item: &f("Required arrival time", &RTA),
};
static ROUTE_ADDITIONAL: Ty = Ty::Seq(&[
    o("Along-track waypoints", &ATW_LIST),
    o("Reporting points", &REPORTING_POINTS),
    o("Intercept courses", &INTERCEPT_LIST),
    o("Holding points", &HOLD_LIST),
    o("Waypoints, speeds and altitudes", &WSA_LIST),
    o("Required arrival times", &RTA_LIST),
]);
static ROUTE_CLEARANCE: Ty = Ty::Seq(&[
    o("Departure airport", &AIRPORT),
    o("Destination airport", &AIRPORT),
    o("Departure runway", &RUNWAY),
    o("Departure procedure", &PROCEDURE),
    o("Arrival runway", &RUNWAY),
    o("Approach procedure", &PROCEDURE),
    o("Arrival procedure", &PROCEDURE),
    o("Airway intercept", &FIX),
    o("Route", &ROUTE),
    o("", &ROUTE_ADDITIONAL),
]);

// Predeparture clearance.

static FLIGHT_ID: Ty = Ty::Ia5 { lo: 2, hi: 7 };
static AIRCRAFT_TYPE: Ty = Ty::Ia5 { lo: 2, hi: 5 };
static EQUIPMENT_STATUS: Ty = Ty::Enum(&[
    "Loran A",
    "Loran C",
    "DME",
    "Decca",
    "ADF",
    "GNSS",
    "HF RTF",
    "inertial navigation",
    "ILS",
    "Omega",
    "VOR",
    "Doppler",
    "RNAV route equipment",
    "TACAN",
    "UHF RTF",
    "VHF RTF",
]);
static EQUIPMENT_LIST: Ty = Ty::List {
    lo: 1,
    hi: 16,
    item: &f("COMM/NAV equipment status", &EQUIPMENT_STATUS),
};
static EQUIPMENT: Ty = Ty::Seq(&[
    f("COMM/NAV/Approach equipment available", &Ty::Bool),
    o("", &EQUIPMENT_LIST),
]);
static REVISION: Ty = int(1, 16);
static PDC: Ty = Ty::Seq(&[
    f("Flight ID", &FLIGHT_ID),
    o("Aircraft type", &AIRCRAFT_TYPE),
    o("Aircraft equipment", &EQUIPMENT),
    f("Estimated departure time", &TIME),
    f("", &ROUTE_CLEARANCE),
    o("Altitude restriction", &ALTITUDE),
    f("Departure frequency", &VHF),
    f("Code", &BEACON),
    f("Revision number", &REVISION),
]);

// The parameters of the elements: SEQUENCEs of the types above, in the order of their names.

/// No parameter.
pub static NONE: Ty = Ty::Null;
/// Time.
pub static P_TIME: Ty = Ty::Seq(&[f("Time", &TIME)]);
/// TimeAltitude.
pub static TIME_ALT: Ty = Ty::Seq(&[f("Time", &TIME), f("", &ALTITUDE)]);
/// PositionAltitude.
pub static POS_ALT: Ty = Ty::Seq(&[f("", &POSITION), f("", &ALTITUDE)]);
/// AltitudeTime.
pub static ALT_TIME: Ty = Ty::Seq(&[f("", &ALTITUDE), f("Time", &TIME)]);
/// AltitudePosition.
pub static ALT_POS: Ty = Ty::Seq(&[f("", &ALTITUDE), f("", &POSITION)]);
/// AltitudeAltitude.
pub static ALT_ALT: Ty = Ty::Seq(&[f("", &ALTITUDE2)]);
/// PositionAltitudeAltitude.
pub static POS_ALT_ALT: Ty = Ty::Seq(&[f("", &POSITION), f("", &ALTITUDE2)]);
/// PositionTime.
pub static POS_TIME: Ty = Ty::Seq(&[f("", &POSITION), f("Time", &TIME)]);
/// PositionTimeTime.
pub static POS_TIME_TIME: Ty = Ty::Seq(&[f("", &POSITION), f("", &TIME2)]);
/// PositionSpeed.
pub static POS_SPEED: Ty = Ty::Seq(&[f("", &POSITION), f("", &SPEED)]);
/// PositionTimeAltitude.
pub static POS_TIME_ALT: Ty = Ty::Seq(&[f("", &POSITION), f("Time", &TIME), f("", &ALTITUDE)]);
/// PositionAltitudeSpeed.
pub static POS_ALT_SPEED: Ty = Ty::Seq(&[f("", &POSITION), f("", &ALTITUDE), f("", &SPEED)]);
/// TimePositionAltitude.
pub static TIME_POS_ALT: Ty = Ty::Seq(&[f("Time", &TIME), f("", &POSITION), f("", &ALTITUDE)]);
/// TimePositionAltitudeSpeed.
pub static TIME_POS_ALT_SPEED: Ty = Ty::Seq(&[
    f("Time", &TIME),
    f("", &POSITION),
    f("", &ALTITUDE),
    f("", &SPEED),
]);
/// DistanceOffsetDirection.
pub static OFFSET_DIR: Ty = Ty::Seq(&[f("", &DISTANCE_OFFSET), f("Direction", &DIRECTION)]);
/// PositionDistanceOffsetDirection.
pub static POS_OFFSET_DIR: Ty = Ty::Seq(&[
    f("", &POSITION),
    f("", &DISTANCE_OFFSET),
    f("Direction", &DIRECTION),
]);
/// TimeDistanceOffsetDirection.
pub static TIME_OFFSET_DIR: Ty = Ty::Seq(&[
    f("Time", &TIME),
    f("", &DISTANCE_OFFSET),
    f("Direction", &DIRECTION),
]);
/// PredepartureClearance.
pub static P_PDC: Ty = Ty::Seq(&[f("", &PDC)]);
/// TimePosition.
pub static TIME_POS: Ty = Ty::Seq(&[f("Time", &TIME), f("", &POSITION)]);
/// PositionPosition.
pub static POS_POS: Ty = Ty::Seq(&[f("", &POSITION2)]);
/// PositionRouteClearance.
pub static POS_ROUTE: Ty = Ty::Seq(&[f("", &POSITION), f("", &ROUTE_CLEARANCE)]);
/// RouteClearance.
pub static P_ROUTE: Ty = Ty::Seq(&[f("", &ROUTE_CLEARANCE)]);
/// ProcedureName.
pub static P_PROCEDURE: Ty = Ty::Seq(&[f("", &PROCEDURE)]);
/// PositionProcedureName.
pub static POS_PROCEDURE: Ty = Ty::Seq(&[f("", &POSITION), f("", &PROCEDURE)]);
/// HoldClearance.
pub static P_HOLD: Ty = Ty::Seq(&[f("", &HOLD)]);
/// DirectionDegrees.
pub static DIR_DEG: Ty = Ty::Seq(&[f("Direction", &DIRECTION), f("", &DEGREES)]);
/// PositionDegrees.
pub static POS_DEG: Ty = Ty::Seq(&[f("", &POSITION), f("", &DEGREES)]);
/// TimeSpeed.
pub static TIME_SPEED: Ty = Ty::Seq(&[f("Time", &TIME), f("", &SPEED)]);
/// AltitudeSpeed.
pub static ALT_SPEED: Ty = Ty::Seq(&[f("", &ALTITUDE), f("", &SPEED)]);
/// TimeSpeedSpeed.
pub static TIME_SPEED2: Ty = Ty::Seq(&[f("Time", &TIME), f("", &SPEED2)]);
/// PositionSpeedSpeed.
pub static POS_SPEED2: Ty = Ty::Seq(&[f("", &POSITION), f("", &SPEED2)]);
/// AltitudeSpeedSpeed.
pub static ALT_SPEED2: Ty = Ty::Seq(&[f("", &ALTITUDE), f("", &SPEED2)]);
/// SpeedSpeed.
pub static SPEED_SPEED: Ty = Ty::Seq(&[f("", &SPEED2)]);
/// ICAOUnitNameFrequency.
pub static UNIT_FREQ: Ty = Ty::Seq(&[f("", &UNIT_NAME), f("", &FREQUENCY)]);
/// PositionICAOUnitNameFrequency.
pub static POS_UNIT_FREQ: Ty = Ty::Seq(&[f("", &POSITION), f("", &UNIT_NAME), f("", &FREQUENCY)]);
/// TimeICAOunitnameFrequency.
pub static TIME_UNIT_FREQ: Ty = Ty::Seq(&[f("Time", &TIME), f("", &UNIT_NAME), f("", &FREQUENCY)]);
/// BeaconCode.
pub static P_BEACON: Ty = Ty::Seq(&[f("Code", &BEACON)]);
/// ATISCode.
pub static P_ATIS: Ty = Ty::Seq(&[f("ATIS code", &ATIS)]);
/// ErrorInformation.
pub static P_ERROR: Ty = Ty::Seq(&[f("Error information", &ERROR_INFO)]);
/// ICAOfacilityDesignation.
pub static P_FACILITY: Ty = Ty::Seq(&[f("Facility designation", &AIRPORT)]);
/// ICAOFacilityDesignationTp4Table.
pub static P_FACILITY_TP4: Ty = Ty::Seq(&[
    f("Facility designation", &AIRPORT),
    f("TP4 table", &TP4),
]);
/// FreeText.
pub static P_FREE_TEXT: Ty = Ty::Seq(&[f("", &FREE_TEXT)]);
/// VerticalRate.
pub static P_VERTICAL_RATE: Ty = Ty::Seq(&[f("", &VERTICAL_RATE)]);
/// ToFromPosition.
pub static TO_FROM_POS: Ty = Ty::Seq(&[f("To/From", &TO_FROM), f("", &POSITION)]);
/// Altimeter.
pub static P_ALTIMETER: Ty = Ty::Seq(&[f("", &ALTIMETER)]);
/// PositionReport.
pub static P_POSITION_REPORT: Ty = Ty::Seq(&[f("", &POSITION_REPORT)]);
/// RemainingFuelRemainingSouls.
pub static FUEL_SOULS: Ty = Ty::Seq(&[f("Remaining fuel", &FUEL), f("Persons on board", &SOULS)]);
/// VersionNumber.
pub static P_VERSION: Ty = Ty::Seq(&[f("Version number", &VERSION)]);
/// TimeDistanceToFromPosition.
pub static TIME_DIST_TOFROM_POS: Ty = Ty::Seq(&[
    f("Time", &TIME),
    f("", &DISTANCE),
    f("To/From", &TO_FROM),
    f("", &POSITION),
]);

// Headers.

static MSG_NUMBER: Ty = int(0, 63);
/// The message header.
pub static HEADER: Ty = Ty::Seq(&[
    f("Msg ID", &MSG_NUMBER),
    o("Msg Ref", &MSG_NUMBER),
    o("Timestamp", &TIMESTAMP),
]);
