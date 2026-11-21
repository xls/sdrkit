//! The application data of binary messages (types 6, 8, 25, 26): the IMO international function messages (DAC 1, IMO
//! SN.1/Circ.289 and ITU-R M.1371 Annex 5) decoded into fields. Every other DAC / FI pair is shown as hex only. A pair is decoded
//! only when the data's length fits its layout. "Not available" codes are left out.

use super::message::{hex, Fields};
use serde_json::{json, Map, Value};

fn put(m: &mut Map<String, Value>, k: &str, v: impl Into<Value>) {
    m.insert(k.into(), v.into());
}

/// Decodes the data of DAC / FI (the bits after the application identifier). `addressed`: from an addressed message.
pub fn decode(dac: u16, fi: u8, addressed: bool, data: &[u8]) -> Option<Value> {
    if dac != 1 {
        return None;
    }
    let r = Fields::new(data);
    let n = data.len();
    let mut m = Map::new();
    match (fi, addressed) {
        (0, _) if n >= 12 => {
            put(&mut m, "kind", "Text telegram");
            put(&mut m, "ack_required", r.b(0));
            put(&mut m, "text_seq", r.u(1, 11));
            put(&mut m, "text", r.text_to_end(12));
        }
        (2, true) if n >= 16 => {
            put(
                &mut m,
                "kind",
                "Interrogation for a specific function message",
            );
            put(&mut m, "requested_dac", r.u(0, 10));
            put(&mut m, "requested_fi", r.u(10, 6));
        }
        (3, true) if n >= 10 => {
            put(&mut m, "kind", "Capability interrogation");
            put(&mut m, "requested_dac", r.u(0, 10));
        }
        (4, true) if n >= 10 + 128 => {
            put(&mut m, "kind", "Capability reply");
            put(&mut m, "dac", r.u(0, 10));
            let fis: Vec<u64> = (0..64)
                .filter(|k| r.b(10 + 2 * k))
                .map(|k| k as u64)
                .collect();
            put(&mut m, "available_fi", fis);
        }
        (5, true) if n >= 31 => {
            put(&mut m, "kind", "Application acknowledgement");
            put(&mut m, "dac", r.u(0, 10));
            put(&mut m, "fi", r.u(10, 6));
            put(&mut m, "text_seq", r.u(16, 11));
            put(&mut m, "ai_available", r.b(27));
            put(&mut m, "ai_response", r.u(28, 3));
        }
        (31, false) if n >= 304 => met_hydro(&r, &mut m),
        (21, false) if n >= 360 - 56 => weather_obs(&r, &mut m),
        (22, _) if n >= 56 => area_notice(&r, &mut m),
        (24, _) if n >= 46 => {
            put(&mut m, "kind", "Extended ship static and voyage data");
            put(&mut m, "linkage_id", r.u(0, 10));
            let ad = r.u(10, 13);
            if ad != 0 {
                put(&mut m, "air_draught_m", ad as f64 / 10.0);
            }
            put(&mut m, "last_port", r.text(23, 5));
            if n >= 58 {
                put(
                    &mut m,
                    "next_ports",
                    vec![r.text(53, 5), r.text_raw_safe(83, 5)],
                );
            }
        }
        (25, _) if n >= 12 => {
            put(&mut m, "kind", "Dangerous cargo indication");
            put(&mut m, "unit", r.u(0, 2));
            put(&mut m, "amount", r.u(2, 10));
            let mut cargos = Vec::new();
            let mut at = 12;
            while at + 17 <= n {
                cargos.push(json!({"code_type": r.u(at, 4), "code": r.u(at + 4, 13)}));
                at += 17;
            }
            put(&mut m, "cargos", cargos);
        }
        (26, _) if n >= 112 => {
            put(&mut m, "kind", "Environmental sensor report");
            let mut reps = Vec::new();
            let mut at = 0;
            while at + 112 <= n {
                reps.push(json!({"report_type": r.u(at, 4), "day": r.u(at + 4, 5), "hour": r.u(at + 9, 5), "minute": r.u(at + 14, 6), "site_id": r.u(at + 20, 7), "payload_hex": hex(&data[at + 27..at + 112])}));
                at += 112;
            }
            put(&mut m, "reports", reps);
        }
        (23, _) if n >= 46 => {
            put(&mut m, "kind", "Environmental (site location)");
            put(&mut m, "header_hex", hex(&data[..46.min(n)]));
        }
        (27, _) if n >= 58 => route(&r, &mut m),
        (29, _) if n >= 10 => {
            put(&mut m, "kind", "Text description");
            put(&mut m, "linkage_id", r.u(0, 10));
            put(&mut m, "text", r.text_to_end(10));
        }
        _ => return None,
    }
    Some(Value::Object(m))
}

impl Fields<'_> {
    fn text_raw_safe(&self, start: usize, chars: usize) -> String {
        if start + chars * 6 <= self.len() {
            self.text(start, chars)
        } else {
            String::new()
        }
    }
}

fn met_hydro(r: &Fields, m: &mut Map<String, Value>) {
    put(m, "kind", "Meteorological and hydrographic data");
    let lon = r.i(0, 25) as f64 / 60_000.0;
    let lat = r.i(25, 24) as f64 / 60_000.0;
    if lon.abs() <= 180.0 && lat.abs() <= 90.0 {
        put(m, "lon", lon);
        put(m, "lat", lat);
    }
    put(m, "accuracy", r.b(49));
    let (day, hour, minute) = (r.u(50, 5), r.u(55, 5), r.u(60, 6));
    if day > 0 {
        put(m, "day", day);
    }
    if hour < 24 {
        put(m, "hour", hour);
    }
    if minute < 60 {
        put(m, "minute", minute);
    }
    let opt = |m: &mut Map<String, Value>, k: &str, v: u64, na: u64, f: &dyn Fn(u64) -> Value| {
        if v != na {
            m.insert(k.into(), f(v));
        }
    };
    let int = |v: u64| json!(v);
    let tenth = |v: u64| json!(v as f64 / 10.0);
    opt(m, "wind_speed_kn", r.u(66, 7), 127, &int);
    opt(m, "wind_gust_kn", r.u(73, 7), 127, &int);
    opt(m, "wind_dir_deg", r.u(80, 9), 360, &int);
    opt(m, "wind_gust_dir_deg", r.u(89, 9), 360, &int);
    let at = r.i(98, 11);
    if at != -1024 {
        put(m, "air_temp_c", at as f64 / 10.0);
    }
    opt(m, "humidity_pct", r.u(109, 7), 101, &int);
    let dp = r.i(116, 10);
    if dp != 501 {
        put(m, "dew_point_c", dp as f64 / 10.0);
    }
    opt(m, "pressure_hpa", r.u(126, 9), 511, &|v| json!(v + 799));
    opt(m, "pressure_tendency", r.u(135, 2), 3, &int);
    put(m, "visibility_greater", r.b(137));
    opt(m, "visibility_nm", r.u(138, 7), 127, &tenth);
    opt(m, "water_level_m", r.u(145, 12), 4001, &|v| {
        json!(v as f64 / 100.0 - 10.0)
    });
    opt(m, "level_trend", r.u(157, 2), 3, &int);
    opt(m, "current_kn", r.u(159, 8), 255, &tenth);
    opt(m, "current_dir_deg", r.u(167, 9), 360, &int);
    opt(m, "current2_kn", r.u(176, 8), 255, &tenth);
    opt(m, "current2_dir_deg", r.u(184, 9), 360, &int);
    opt(m, "current2_depth_m", r.u(193, 5), 31, &int);
    opt(m, "current3_kn", r.u(198, 8), 255, &tenth);
    opt(m, "current3_dir_deg", r.u(206, 9), 360, &int);
    opt(m, "current3_depth_m", r.u(215, 5), 31, &int);
    opt(m, "wave_height_m", r.u(220, 8), 255, &tenth);
    opt(m, "wave_period_s", r.u(228, 6), 63, &int);
    opt(m, "wave_dir_deg", r.u(234, 9), 360, &int);
    opt(m, "swell_height_m", r.u(243, 8), 255, &tenth);
    opt(m, "swell_period_s", r.u(251, 6), 63, &int);
    opt(m, "swell_dir_deg", r.u(257, 9), 360, &int);
    opt(m, "sea_state", r.u(266, 4), 13, &int);
    let wt = r.i(270, 10);
    if wt != 501 {
        put(m, "water_temp_c", wt as f64 / 10.0);
    }
    opt(m, "precipitation", r.u(280, 3), 7, &int);
    let sal = r.u(283, 9);
    if sal < 510 {
        put(m, "salinity_permille", sal as f64 / 10.0);
    }
    opt(m, "ice", r.u(292, 2), 3, &int);
}

fn weather_obs(r: &Fields, m: &mut Map<String, Value>) {
    put(m, "kind", "Weather observation from ship");
    put(m, "location", r.text(1, 20));
    let lon = r.i(121, 25) as f64 / 60_000.0;
    let lat = r.i(146, 24) as f64 / 60_000.0;
    if lon.abs() <= 180.0 && lat.abs() <= 90.0 {
        put(m, "lon", lon);
        put(m, "lat", lat);
    }
    put(m, "day", r.u(170, 5));
    put(m, "hour", r.u(175, 5));
    put(m, "minute", r.u(180, 6));
    put(m, "present_weather", r.u(186, 4));
    put(m, "visibility_greater", r.b(190));
    let v = r.u(191, 7);
    if v != 127 {
        put(m, "visibility_nm", v as f64 / 10.0);
    }
    let h = r.u(198, 7);
    if h != 101 {
        put(m, "humidity_pct", h);
    }
    let w = r.u(205, 7);
    if w != 127 {
        put(m, "wind_speed_kn", w);
    }
    let wd = r.u(212, 9);
    if wd < 360 {
        put(m, "wind_dir_deg", wd);
    }
    let p = r.u(221, 9);
    if p != 511 {
        put(m, "pressure_hpa", p + 799);
    }
    put(m, "pressure_tendency", r.u(230, 4));
    let at = r.i(234, 11);
    if at != -1024 {
        put(m, "air_temp_c", at as f64 / 10.0);
    }
    let wt = r.i(245, 10);
    if wt != 501 {
        put(m, "water_temp_c", wt as f64 / 10.0);
    }
}

fn area_notice(r: &Fields, m: &mut Map<String, Value>) {
    put(m, "kind", "Area notice");
    put(m, "linkage_id", r.u(0, 10));
    put(m, "notice", r.u(10, 7));
    put(m, "month", r.u(17, 4));
    put(m, "day", r.u(21, 5));
    put(m, "hour", r.u(26, 5));
    put(m, "minute", r.u(31, 6));
    put(m, "duration_min", r.u(37, 18));
    let mut subs = Vec::new();
    let mut at = 55;
    while at + 87 <= r.len() {
        subs.push(json!({"shape": r.u(at, 3), "data_hex": hex(&(at + 3..at + 87).map(|i| r.u(i, 1) as u8).collect::<Vec<_>>())}));
        at += 87;
    }
    put(m, "subareas", subs);
}

fn route(r: &Fields, m: &mut Map<String, Value>) {
    put(m, "kind", "Route information");
    put(m, "linkage_id", r.u(0, 10));
    put(m, "sender", r.u(10, 3));
    put(m, "route_type", r.u(13, 5));
    put(m, "month", r.u(18, 4));
    put(m, "day", r.u(22, 5));
    put(m, "hour", r.u(27, 5));
    put(m, "minute", r.u(32, 6));
    put(m, "duration_min", r.u(38, 18));
    let count = r.u(56, 5) as usize;
    let mut wps = Vec::new();
    for k in 0..count {
        let at = 61 + 55 * k;
        if at + 55 > r.len() {
            break;
        }
        wps.push(json!({"lon": r.i(at, 28) as f64 / 600_000.0, "lat": r.i(at + 28, 27) as f64 / 600_000.0}));
    }
    put(m, "waypoints", wps);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ais::encode::BitWriter;

    #[test]
    fn met_hydro_fields() {
        let mut w = BitWriter::new();
        w.put((5.25 * 60_000.0) as i64, 25)
            .put((52.5 * 60_000.0) as i64, 24)
            .flag(true)
            .put(14, 5)
            .put(12, 5)
            .put(30, 6)
            .put(18, 7)
            .put(25, 7)
            .put(270, 9)
            .put(360, 9)
            .put(125, 11)
            .put(80, 7);
        w.pad_to(304);
        let v = decode(1, 31, false, &w.bits).unwrap();
        assert_eq!(v["wind_speed_kn"], 18);
        assert_eq!(v["air_temp_c"], 12.5);
        assert!(v.get("wind_gust_dir_deg").is_none());
        assert_eq!(v["humidity_pct"], 80);
        assert!(decode(1, 31, false, &w.bits[..200]).is_none());
        assert!(decode(366, 56, false, &w.bits).is_none());
    }
}
