//! FIS-B in UAT uplink information frames: the APDU header, DLAC text, the text products, the text and graphic records (TWGO) of
//! NOTAM, AIRMET, SIGMET, G-AIRMET, CWA, SUA and TFR, the grid blocks of the gridded products, and the joining of segments.
//!
//! Standard: RTCA DO-358B

use serde::{Deserialize, Serialize};

/// The APDU header.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Apdu {
    /// Application method flag.
    pub a_flag: bool,
    /// Geographic flag.
    pub g_flag: bool,
    /// Product file flag.
    pub p_flag: bool,
    /// Segmentation flag.
    pub s_flag: bool,
    /// Product id (11 bits).
    pub product_id: u16,
    /// Time format 0 to 3.
    pub time_format: u8,
    /// Month and day (formats 2 and 3).
    pub month_day: Option<(u8, u8)>,
    /// Hours and minutes.
    pub hours: u8,
    /// Minutes.
    pub minutes: u8,
    /// Seconds (formats 1 and 3).
    pub seconds: Option<u8>,
    /// Segmentation: product file id, file length, APDU number (when the segmentation flag is set).
    pub segment: Option<(u16, u16, u16)>,
    /// Where the product data starts in the information frame.
    pub data_offset: usize,
}

/// Decodes the APDU header of an information frame (at least 4 bytes).
pub fn decode_apdu(d: &[u8]) -> Option<Apdu> {
    if d.len() < 4 {
        return None;
    }
    let mut a = Apdu {
        a_flag: d[0] & 0x80 != 0,
        g_flag: d[0] & 0x40 != 0,
        p_flag: d[0] & 0x20 != 0,
        product_id: ((d[0] & 0x1F) as u16) << 6 | (d[1] >> 2) as u16,
        s_flag: d[1] & 0x02 != 0,
        time_format: (d[1] & 1) << 1 | d[2] >> 7,
        ..Default::default()
    };
    match a.time_format {
        0 => {
            a.hours = (d[2] & 0x7C) >> 2;
            a.minutes = (d[2] & 3) << 4 | d[3] >> 4;
            a.data_offset = 4;
        }
        1 => {
            if d.len() < 5 {
                return None;
            }
            a.hours = (d[2] & 0x7C) >> 2;
            a.minutes = (d[2] & 3) << 4 | d[3] >> 4;
            a.seconds = Some((d[3] & 0x0F) << 2 | d[4] >> 6);
            a.data_offset = 5;
        }
        2 => {
            if d.len() < 5 {
                return None;
            }
            a.month_day = Some(((d[2] & 0x78) >> 3, (d[2] & 7) << 2 | d[3] >> 6));
            a.hours = (d[3] & 0x3E) >> 1;
            a.minutes = (d[3] & 1) << 5 | d[4] >> 3;
            a.data_offset = 5;
        }
        _ => {
            if d.len() < 6 {
                return None;
            }
            a.month_day = Some(((d[2] & 0x78) >> 3, (d[2] & 7) << 2 | d[3] >> 6));
            a.hours = (d[3] & 0x3E) >> 1;
            a.minutes = (d[3] & 1) << 5 | d[4] >> 3;
            a.seconds = Some((d[4] & 7) << 3 | d[5] >> 5);
            a.data_offset = 6;
        }
    }
    if a.s_flag {
        // Product file id (10 bits), product file length (9 bits), APDU number (9 bits).
        let o = a.data_offset;
        if d.len() >= o + 4 {
            let v = (d[o] as u32) << 24
                | (d[o + 1] as u32) << 16
                | (d[o + 2] as u32) << 8
                | d[o + 3] as u32;
            a.segment = Some((
                (v >> 22) as u16,
                ((v >> 13) & 0x1FF) as u16,
                ((v >> 4) & 0x1FF) as u16,
            ));
        }
    }
    Some(a)
}

/// Encodes an APDU header (time format 0 to 3).
#[allow(clippy::too_many_arguments)]
pub fn encode_apdu(
    product_id: u16,
    a_flag: bool,
    s_flag: bool,
    time_format: u8,
    month: u8,
    day: u8,
    h: u8,
    m: u8,
    s: u8,
) -> Vec<u8> {
    let mut v = vec![0u8; 6];
    v[0] = (a_flag as u8) << 7 | ((product_id >> 6) as u8 & 0x1F);
    v[1] = ((product_id & 0x3F) as u8) << 2 | (s_flag as u8) << 1 | (time_format >> 1);
    v[2] = (time_format & 1) << 7;
    match time_format {
        0 | 1 => {
            v[2] |= (h & 0x1F) << 2 | (m >> 4) & 3;
            v[3] = (m & 15) << 4;
            if time_format == 1 {
                v[3] |= (s >> 2) & 15;
                v[4] = (s & 3) << 6;
            }
        }
        _ => {
            v[2] |= (month & 15) << 3 | (day >> 2) & 7;
            v[3] = (day & 3) << 6 | (h & 0x1F) << 1 | (m >> 5) & 1;
            v[4] = (m & 0x1F) << 3;
            if time_format == 3 {
                v[4] |= (s >> 3) & 7;
                v[5] = (s & 7) << 5;
            }
        }
    }
    v.truncate([4, 5, 5, 6][time_format as usize & 3]);
    v
}

/// The name of a product.
pub fn product_name(id: u16) -> String {
    match id {
        0 => "METAR".into(),
        1 => "TAF".into(),
        2 => "NEXRAD".into(),
        8 => "AIRMET".into(),
        10 => "SIGMET".into(),
        11 => "AIRMET (TWGO)".into(),
        12 => "G-AIRMET".into(),
        13 => "SUA status".into(),
        14 => "G-AIRMET (TWGO)".into(),
        15 => "CWA".into(),
        16 => "NOTAM-TFR".into(),
        17 => "PIREP".into(),
        63 => "NEXRAD regional".into(),
        64 => "NEXRAD CONUS".into(),
        70 => "icing, low".into(),
        71 => "icing, high".into(),
        84 => "cloud tops".into(),
        90 => "turbulence, low".into(),
        91 => "turbulence, high".into(),
        103 => "lightning".into(),
        413 => "winds aloft".into(),
        n => format!("FIS-B {n}"),
    }
}

/// The DLAC alphabet (6 bits): 0 end of text, 27 substitute, 28 tab (the next code counts spaces), 29 record separator, 30 line
/// feed, 31 a change-of-cipher mark, then space to '?'.
const DLAC: &[u8; 64] =
    b"\x03ABCDEFGHIJKLMNOPQRSTUVWXYZ\x1A\t\x1E\n| !\"#$%&'()*+,-./0123456789:;<=>?";

/// Decodes DLAC text (four 6-bit characters in every three bytes).
pub fn dlac_decode(d: &[u8]) -> String {
    let mut s = String::with_capacity(d.len() * 4 / 3);
    let mut tab = false;
    let nbits = d.len() * 8;
    let mut bit = 0;
    while bit + 6 <= nbits {
        let mut c = 0u8;
        for i in 0..6 {
            let b = bit + i;
            c = c << 1 | (d[b / 8] >> (7 - b % 8)) & 1;
        }
        bit += 6;
        if tab {
            for _ in 0..c {
                s.push(' ');
            }
            tab = false;
            continue;
        }
        match c {
            0 => break,
            28 => tab = true,
            27 => {}
            29 => s.push('\u{1E}'),
            30 => s.push('\n'),
            31 => {}
            _ => s.push(DLAC[c as usize] as char),
        }
    }
    s
}

/// Encodes text as DLAC (upper case; characters outside the alphabet become spaces; ended with the end-of-text code).
pub fn dlac_encode(text: &str) -> Vec<u8> {
    let mut codes: Vec<u8> = Vec::with_capacity(text.len() + 1);
    for ch in text.chars() {
        let ch = ch.to_ascii_uppercase();
        let c = match ch {
            '\n' => 30,
            '\u{1E}' => 29,
            _ => DLAC
                .iter()
                .skip(32)
                .position(|&b| b as char == ch)
                .map(|p| p as u8 + 32)
                .or_else(|| {
                    DLAC.iter()
                        .take(27)
                        .skip(1)
                        .position(|&b| b as char == ch)
                        .map(|p| p as u8 + 1)
                })
                .unwrap_or(32),
        };
        codes.push(c);
    }
    codes.push(0);
    let mut out = vec![0u8; (codes.len() * 6).div_ceil(8)];
    for (i, &c) in codes.iter().enumerate() {
        for k in 0..6 {
            if c >> (5 - k) & 1 == 1 {
                let b = i * 6 + k;
                out[b / 8] |= 0x80 >> (b % 8);
            }
        }
    }
    out
}

/// One block of a gridded product (the global block representation).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridBlock {
    /// Block number (20 bits).
    pub block: u32,
    /// Southern hemisphere.
    pub south: bool,
    /// Scale factor 0 (high resolution), 1 (medium), 2 (low).
    pub scale: u8,
    /// North-west corner and size, degrees: (lat north, lon west, lat size, lon size).
    pub area: (f64, f64, f64, f64),
    /// 128 bins (32 x 4, rows from north), or `None` for an empty block.
    pub bins: Option<Vec<u8>>,
}

/// Where a block lies: (northern latitude, western longitude, latitude size, longitude size), degrees.
pub fn block_area(block: u32, south: bool, scale: u8) -> (f64, f64, f64, f64) {
    let k = match scale {
        1 => 5.0,
        2 => 9.0,
        _ => 1.0,
    };
    let (lat_size, lon_size) = if block >= 405_000 {
        (4.0 / 60.0 * k, 96.0 / 60.0 * k)
    } else {
        (4.0 / 60.0 * k, 48.0 / 60.0 * k)
    };
    let row = block / 450;
    let col = block % 450;
    let mut lat = row as f64 * 4.0 / 60.0;
    let mut lon = col as f64
        * if block >= 405_000 {
            96.0 / 60.0
        } else {
            48.0 / 60.0
        };
    if lon > 180.0 {
        lon -= 360.0;
    }
    lat += lat_size;
    if south {
        lat = -(lat - lat_size);
    }
    (lat, lon, lat_size, lon_size)
}

/// Decodes the blocks of a gridded product (after the APDU header).
pub fn decode_grid(d: &[u8]) -> Vec<GridBlock> {
    let mut out = Vec::new();
    if d.len() < 3 {
        return out;
    }
    let rle = d[0] & 0x80 != 0;
    let south = d[0] & 0x40 != 0;
    let scale = (d[0] & 0x30) >> 4;
    let block = ((d[0] & 0x0F) as u32) << 16 | (d[1] as u32) << 8 | d[2] as u32;
    if rle {
        let mut bins = Vec::with_capacity(128);
        for &b in &d[3..] {
            let v = b & 7;
            let run = (b >> 3) as usize + 1;
            for _ in 0..run {
                if bins.len() < 128 {
                    bins.push(v);
                }
            }
        }
        bins.resize(128, 0);
        out.push(GridBlock {
            block,
            south,
            scale,
            area: block_area(block, south, scale),
            bins: Some(bins),
        });
    } else {
        // Empty blocks: the first, then a bitmap of the following ones.
        out.push(GridBlock {
            block,
            south,
            scale,
            area: block_area(block, south, scale),
            bins: None,
        });
        if d.len() > 3 {
            let l = (d[3] & 0x0F) as usize;
            let bits = (d[3] & 0xF0) >> 4;
            for i in 0..4 {
                if bits & (1 << i) != 0 {
                    let b = block + i as u32 + 1;
                    out.push(GridBlock {
                        block: b,
                        south,
                        scale,
                        area: block_area(b, south, scale),
                        bins: None,
                    });
                }
            }
            for j in 0..l {
                if let Some(&bm) = d.get(4 + j) {
                    for i in 0..8 {
                        if bm & (1 << i) != 0 {
                            let b = block + 5 + (j * 8 + i) as u32;
                            out.push(GridBlock {
                                block: b,
                                south,
                                scale,
                                area: block_area(b, south, scale),
                                bins: None,
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

/// Encodes one run-length block (128 bins of 0 to 7).
pub fn encode_grid_block(block: u32, south: bool, scale: u8, bins: &[u8]) -> Vec<u8> {
    let mut v = vec![
        0x80 | (south as u8) << 6 | (scale & 3) << 4 | ((block >> 16) & 0x0F) as u8,
        (block >> 8) as u8,
        block as u8,
    ];
    let mut i = 0;
    while i < bins.len().min(128) {
        let val = bins[i] & 7;
        let mut run = 1;
        while i + run < bins.len().min(128) && bins[i + run] & 7 == val && run < 32 {
            run += 1;
        }
        v.push(((run - 1) as u8) << 3 | val);
        i += run;
    }
    v
}

/// A geometry of a graphical overlay record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Geometry {
    /// A polygon (closed) or polyline of (lat, lon, altitude ft) vertices.
    Path {
        /// Closed.
        polygon: bool,
        /// Vertices.
        vertices: Vec<(f64, f64, i32)>,
    },
    /// A circle or prism: centre, radius (NM) and altitudes.
    Circle {
        /// Centre.
        centre: (f64, f64),
        /// Radius, NM.
        radius_nm: f64,
        /// Bottom and top, ft.
        alt: (i32, i32),
    },
    /// A point.
    Point((f64, f64, i32)),
    /// Another overlay option, kept as its number.
    Other(u8),
}

/// One record of a TWGO product.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TwgoRecord {
    /// A text record.
    Text {
        /// Report number.
        report: u16,
        /// Report year.
        year: u16,
        /// Active (report status 1).
        active: bool,
        /// The text.
        text: String,
    },
    /// A graphical overlay record.
    Graphic {
        /// Report number.
        report: u16,
        /// Overlay record identifier.
        overlay: u8,
        /// Label (a number or text).
        label: String,
        /// Start and end of the record's validity, when the record carries them.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        times: Option<(TwgoTime, TwgoTime)>,
        /// The geometry.
        geometry: Geometry,
    },
}

/// A start or end time of a graphical overlay record. The date/time format of the record says which parts are sent: 1 month, day,
/// hours and minutes; 2 day, hours and minutes; 3 hours and minutes (one byte each).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TwgoTime {
    /// Month (format 1).
    pub month: Option<u8>,
    /// Day (formats 1 and 2).
    pub day: Option<u8>,
    /// Hours.
    pub hours: u8,
    /// Minutes.
    pub minutes: u8,
}

/// Bytes of one time of a graphical overlay record for each date/time format.
fn twgo_time_len(format: u8) -> usize {
    match format {
        1 => 4,
        2 => 3,
        3 => 2,
        _ => 0,
    }
}

fn decode_twgo_time(format: u8, d: &[u8]) -> TwgoTime {
    match format {
        1 => TwgoTime {
            month: Some(d[0]),
            day: Some(d[1]),
            hours: d[2],
            minutes: d[3],
        },
        2 => TwgoTime {
            month: None,
            day: Some(d[0]),
            hours: d[1],
            minutes: d[2],
        },
        _ => TwgoTime {
            month: None,
            day: None,
            hours: d[0],
            minutes: d[1],
        },
    }
}

fn encode_twgo_time(format: u8, t: &TwgoTime, out: &mut Vec<u8>) {
    match format {
        1 => out.extend([t.month.unwrap_or(0), t.day.unwrap_or(0), t.hours, t.minutes]),
        2 => out.extend([t.day.unwrap_or(0), t.hours, t.minutes]),
        3 => out.extend([t.hours, t.minutes]),
        _ => {}
    }
}

/// The records of a TWGO product (after the APDU header).
pub fn decode_twgo(d: &[u8]) -> Option<(String, Vec<TwgoRecord>)> {
    if d.len() < 6 {
        return None;
    }
    let format = d[0] >> 4;
    let count = (d[1] >> 4) as usize;
    let location = dlac_decode(&d[2..5]);
    let mut out = Vec::new();
    let mut i = 6;
    for _ in 0..count.max(1) {
        if i + 2 > d.len() {
            break;
        }
        match format {
            2 => {
                if i + 5 > d.len() {
                    break;
                }
                let len = (d[i] as usize) << 8 | d[i + 1] as usize;
                let report = (d[i + 2] as u16) << 6 | (d[i + 3] >> 2) as u16;
                let year = ((d[i + 3] & 3) as u16) << 5 | (d[i + 4] >> 3) as u16;
                let active = d[i + 4] & 0x04 != 0;
                let end = (i + len).min(d.len());
                let text = dlac_decode(&d[(i + 5).min(end)..end]);
                out.push(TwgoRecord::Text {
                    report,
                    year,
                    active,
                    text,
                });
                if len < 5 {
                    break;
                }
                i += len;
            }
            8 => {
                if i + 8 > d.len() {
                    break;
                }
                let len = (d[i] as usize) << 2 | (d[i + 1] >> 6) as usize;
                let report = ((d[i + 1] & 0x3F) as u16) << 8 | d[i + 2] as u16;
                let overlay = ((d[i + 5] & 0x1E) >> 1) + 1;
                let numeric = d[i + 5] & 1 == 0;
                let (label, mut o) = if numeric {
                    (
                        ((d[i + 6] as u16) << 8 | d[i + 7] as u16).to_string(),
                        i + 8,
                    )
                } else {
                    (dlac_decode(&d[i + 6..(i + 15).min(d.len())]), i + 15)
                };
                let end = (i + len).min(d.len());
                let mut geometry = Geometry::Other(0);
                let mut times = None;
                if o + 2 <= end {
                    let opts = d[o] & 0x0F;
                    let dt_format = (d[o] & 0x30) >> 4;
                    let nverts = (d[o + 1] & 0x3F) as usize + 1;
                    o += 2;
                    let tl = twgo_time_len(dt_format);
                    if tl > 0 && o + 2 * tl <= end {
                        times = Some((
                            decode_twgo_time(dt_format, &d[o..o + tl]),
                            decode_twgo_time(dt_format, &d[o + tl..o + 2 * tl]),
                        ));
                    }
                    o += 2 * tl;
                    let k = 360.0 / 524_288.0;
                    let wrap = |lat: f64, lon: f64| {
                        (
                            if lat > 90.0 { lat - 180.0 } else { lat },
                            if lon > 180.0 { lon - 360.0 } else { lon },
                        )
                    };
                    geometry = match opts {
                        3 | 4 => {
                            let mut v = Vec::new();
                            for _ in 0..nverts {
                                if o + 6 > end {
                                    break;
                                }
                                let lon = ((d[o] as u32) << 11
                                    | (d[o + 1] as u32) << 3
                                    | (d[o + 2] as u32) >> 5)
                                    as f64
                                    * k;
                                let lat = (((d[o + 2] & 0x1F) as u32) << 14
                                    | (d[o + 3] as u32) << 6
                                    | (d[o + 4] as u32) >> 2)
                                    as f64
                                    * k;
                                let alt = (((d[o + 4] & 3) as i32) << 8 | d[o + 5] as i32) * 100;
                                let (la, lo) = wrap(lat, lon);
                                v.push((la, lo, alt));
                                o += 6;
                            }
                            Geometry::Path {
                                polygon: opts == 3,
                                vertices: v,
                            }
                        }
                        7 | 8 if o + 14 <= end => {
                            let lon = ((d[o] as u32) << 10
                                | (d[o + 1] as u32) << 2
                                | (d[o + 2] as u32) >> 6)
                                as f64
                                * 360.0
                                / 262_144.0;
                            let lat = (((d[o + 2] & 0x3F) as u32) << 12
                                | (d[o + 3] as u32) << 4
                                | (d[o + 4] as u32) >> 4)
                                as f64
                                * 360.0
                                / 262_144.0;
                            let (la, lo) = wrap(lat, lon);
                            let bottom =
                                (((d[o + 9] & 0x0F) as i32) << 3 | (d[o + 10] >> 5) as i32) * 500;
                            let top =
                                (((d[o + 10] & 0x1F) as i32) << 2 | (d[o + 11] >> 6) as i32) * 500;
                            let r = (((d[o + 11] & 0x3F) as u32) << 3 | (d[o + 12] >> 5) as u32)
                                as f64
                                * 0.2;
                            Geometry::Circle {
                                centre: (la, lo),
                                radius_nm: r,
                                alt: (bottom, top),
                            }
                        }
                        9 | 10 if o + 6 <= end => {
                            let lon = ((d[o] as u32) << 11
                                | (d[o + 1] as u32) << 3
                                | (d[o + 2] as u32) >> 5)
                                as f64
                                * k;
                            let lat = (((d[o + 2] & 0x1F) as u32) << 14
                                | (d[o + 3] as u32) << 6
                                | (d[o + 4] as u32) >> 2)
                                as f64
                                * k;
                            let alt = (((d[o + 4] & 3) as i32) << 8 | d[o + 5] as i32) * 100;
                            let (la, lo) = wrap(lat, lon);
                            Geometry::Point((la, lo, alt))
                        }
                        n => Geometry::Other(n),
                    };
                }
                out.push(TwgoRecord::Graphic {
                    report,
                    overlay,
                    label,
                    times,
                    geometry,
                });
                if len < 8 {
                    break;
                }
                i += len;
            }
            _ => break,
        }
    }
    Some((location, out))
}

/// Encodes a TWGO text product with one record.
pub fn encode_twgo_text(location: &str, report: u16, year: u16, text: &str) -> Vec<u8> {
    let mut v = vec![2 << 4, 1 << 4];
    let mut loc = dlac_encode(location);
    loc.resize(3, 0);
    v.extend_from_slice(&loc[..3]);
    v.push(0);
    let t = dlac_encode(text);
    let len = 5 + t.len();
    v.push((len >> 8) as u8);
    v.push(len as u8);
    v.push((report >> 6) as u8);
    v.push(((report & 0x3F) as u8) << 2 | ((year >> 5) & 3) as u8);
    v.push(((year & 0x1F) as u8) << 3 | 0x04);
    v.extend(t);
    v
}

/// Encodes a TWGO graphics product with one polygon record (vertices lat, lon, altitude ft).
pub fn encode_twgo_polygon(
    location: &str,
    report: u16,
    label: u16,
    vertices: &[(f64, f64, i32)],
) -> Vec<u8> {
    encode_twgo_graphic(
        location,
        report,
        label,
        None,
        &Geometry::Path {
            polygon: true,
            vertices: vertices.to_vec(),
        },
    )
}

fn push_lon_lat_19(r: &mut Vec<u8>, lat: f64, lon: f64, alt_100ft: u32) {
    let k = 524_288.0 / 360.0;
    let lo = ((if lon < 0.0 { lon + 360.0 } else { lon }) * k).round() as u32 & 0x7FFFF;
    let la = ((if lat < 0.0 { lat + 180.0 } else { lat }) * k).round() as u32 & 0x7FFFF;
    let a = alt_100ft.min(1023);
    r.push((lo >> 11) as u8);
    r.push((lo >> 3) as u8);
    r.push(((lo & 7) << 5) as u8 | (la >> 14) as u8);
    r.push((la >> 6) as u8);
    r.push(((la & 0x3F) << 2) as u8 | (a >> 8) as u8);
    r.push(a as u8);
}

/// Encodes a TWGO graphics product with one graphical overlay record: a polygon or polyline (overlay options 3 and 4), a circle
/// (option 7) or a point (option 9), with the record's start and end times when given (date/time format 1 to 3 from the parts set:
/// month and day give 1, day alone 2, neither 3).
pub fn encode_twgo_graphic(
    location: &str,
    report: u16,
    label: u16,
    times: Option<(TwgoTime, TwgoTime)>,
    geometry: &Geometry,
) -> Vec<u8> {
    let mut v = vec![8 << 4, 1 << 4];
    let mut loc = dlac_encode(location);
    loc.resize(3, 0);
    v.extend_from_slice(&loc[..3]);
    v.push(0);
    let mut r = vec![0u8; 8];
    r[1] = ((report >> 8) & 0x3F) as u8;
    r[2] = report as u8;
    r[5] = 0; // overlay record id 1, numeric label
    r[6] = (label >> 8) as u8;
    r[7] = label as u8;
    let dt_format = match times {
        None => 0u8,
        Some((t, _)) if t.month.is_some() => 1,
        Some((t, _)) if t.day.is_some() => 2,
        Some(_) => 3,
    };
    let (opts, n) = match geometry {
        Geometry::Path { polygon, vertices } => (if *polygon { 3 } else { 4 }, vertices.len()),
        Geometry::Circle { .. } => (7, 1),
        Geometry::Point(_) => (9, 1),
        Geometry::Other(o) => (*o & 0x0F, 1),
    };
    r.push(dt_format << 4 | opts);
    r.push((n.saturating_sub(1) & 0x3F) as u8);
    if let Some((a, b)) = &times {
        encode_twgo_time(dt_format, a, &mut r);
        encode_twgo_time(dt_format, b, &mut r);
    }
    match geometry {
        Geometry::Path { vertices, .. } => {
            for &(lat, lon, alt) in vertices {
                push_lon_lat_19(&mut r, lat, lon, (alt / 100).max(0) as u32);
            }
        }
        Geometry::Point((lat, lon, alt)) => {
            push_lon_lat_19(&mut r, *lat, *lon, (*alt / 100).max(0) as u32)
        }
        Geometry::Circle {
            centre: (lat, lon),
            radius_nm,
            alt: (bottom, top),
        } => {
            let k = 262_144.0 / 360.0;
            let lo = ((if *lon < 0.0 { lon + 360.0 } else { *lon }) * k).round() as u32 & 0x3FFFF;
            let la = ((if *lat < 0.0 { lat + 180.0 } else { *lat }) * k).round() as u32 & 0x3FFFF;
            let mut c = [0u8; 14];
            c[0] = (lo >> 10) as u8;
            c[1] = (lo >> 2) as u8;
            c[2] = ((lo & 3) << 6) as u8 | (la >> 12) as u8;
            c[3] = (la >> 4) as u8;
            c[4] = ((la & 15) << 4) as u8;
            let b = ((*bottom / 500).clamp(0, 127)) as u32;
            let t = ((*top / 500).clamp(0, 127)) as u32;
            let rr = ((radius_nm / 0.2).round() as u32).min(511);
            c[9] = (b >> 3) as u8;
            c[10] = ((b & 7) << 5) as u8 | (t >> 2) as u8;
            c[11] = ((t & 3) << 6) as u8 | (rr >> 3) as u8;
            c[12] = ((rr & 7) << 5) as u8;
            r.extend(c);
        }
        Geometry::Other(_) => {}
    }
    let len = r.len();
    r[0] = (len >> 2) as u8;
    r[1] |= ((len & 3) << 6) as u8;
    v.extend(r);
    v
}

/// A decoded FIS-B product.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Product {
    /// The APDU header.
    pub apdu: Apdu,
    /// The name.
    pub name: String,
    /// The text (text products and text records), joined.
    pub text: String,
    /// TWGO records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub records: Vec<TwgoRecord>,
    /// Blocks of a gridded product.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<GridBlock>,
}

/// Decodes the product of an information frame of type 0. For a segment of a segmented product file (segmentation flag set),
/// returns the header only; [`Reassembler`] joins the segments and decodes the whole file.
pub fn decode_product(frame: &[u8]) -> Option<Product> {
    let apdu = decode_apdu(frame)?;
    if apdu.s_flag {
        return Some(Product {
            name: product_name(apdu.product_id),
            text: String::new(),
            records: Vec::new(),
            blocks: Vec::new(),
            apdu,
        });
    }
    let data = frame.get(apdu.data_offset..).unwrap_or(&[]);
    Some(decode_body(apdu, data))
}

/// Where the data of a segment starts: after the 28 bits of segmentation fields (product file id, file length, APDU number), which
/// take 4 bytes with the spare bits.
const SEGMENT_HEADER: usize = 4;

/// Joins the segments of segmented FIS-B product files (DO-358B): the segments of one file have the same product id and product
/// file id, the file length is the number of segments and the APDU number (1 to the file length) is a segment's place. A file is
/// decoded once all its segments are held; files not completed within `timeout_s` of their first segment are dropped, and at most
/// 64 files are pending.
#[derive(Clone, Debug)]
pub struct Reassembler {
    files: Vec<PendingFile>,
    /// Seconds after which an incomplete file is dropped.
    pub timeout_s: f64,
}

#[derive(Clone, Debug)]
struct PendingFile {
    product_id: u16,
    file_id: u16,
    first: f64,
    apdu: Apdu,
    parts: Vec<Option<Vec<u8>>>,
}

impl Default for Reassembler {
    fn default() -> Self {
        Reassembler {
            files: Vec::new(),
            timeout_s: 600.0,
        }
    }
}

impl Reassembler {
    /// Takes one information frame (type 0) heard at `time` (s). Returns the whole product when this frame completed a segmented
    /// file. Frames without segmentation give `None` (decode them with [`decode_product`]).
    pub fn push(&mut self, time: f64, frame: &[u8]) -> Option<Product> {
        let apdu = decode_apdu(frame)?;
        let (file_id, len, n) = apdu.segment?;
        self.files.retain(|f| time - f.first <= self.timeout_s);
        if len == 0 || n == 0 || n > len {
            return None;
        }
        let data = frame.get(apdu.data_offset + SEGMENT_HEADER..)?.to_vec();
        let pid = apdu.product_id;
        let i = match self.files.iter().position(|f| {
            f.product_id == pid && f.file_id == file_id && f.parts.len() == len as usize
        }) {
            Some(i) => i,
            None => {
                if self.files.len() == 64 {
                    self.files.remove(0);
                }
                self.files.push(PendingFile {
                    product_id: pid,
                    file_id,
                    first: time,
                    apdu: apdu.clone(),
                    parts: vec![None; len as usize],
                });
                self.files.len() - 1
            }
        };
        let f = &mut self.files[i];
        if n == 1 {
            f.apdu = apdu;
        }
        f.parts[n as usize - 1] = Some(data);
        if f.parts.iter().any(|p| p.is_none()) {
            return None;
        }
        let f = self.files.remove(i);
        let mut apdu = f.apdu;
        let mut data = Vec::new();
        for p in f.parts.into_iter().flatten() {
            data.extend(p);
        }
        apdu.s_flag = false;
        apdu.segment = None;
        Some(decode_body(apdu, &data))
    }
    /// Files waiting for segments.
    pub fn pending(&self) -> usize {
        self.files.len()
    }
    /// Drops every pending file.
    pub fn clear(&mut self) {
        self.files.clear();
    }
}

/// Splits a product (APDU header then `data`) into segmented information frames of at most `max_data` data bytes each.
pub fn encode_segments(
    product_id: u16,
    file_id: u16,
    time_format: u8,
    (h, m, sec): (u8, u8, u8),
    data: &[u8],
    max_data: usize,
) -> Vec<Vec<u8>> {
    let parts: Vec<&[u8]> = data.chunks(max_data.max(1)).collect();
    let len = parts.len() as u32;
    parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut f = encode_apdu(product_id, false, true, time_format, 0, 0, h, m, sec);
            let v = (file_id as u32 & 0x3FF) << 22
                | (len & 0x1FF) << 13
                | ((i as u32 + 1) & 0x1FF) << 4;
            f.extend(v.to_be_bytes());
            f.extend_from_slice(p);
            f
        })
        .collect()
}

/// Decodes the product data that follows an APDU header.
fn decode_body(apdu: Apdu, data: &[u8]) -> Product {
    let id = apdu.product_id;
    let mut p = Product {
        name: product_name(id),
        text: String::new(),
        records: Vec::new(),
        blocks: Vec::new(),
        apdu,
    };
    match id {
        413 | 0 | 1 => {
            p.text = dlac_decode(data).replace('\u{1E}', "\n");
            let first = p.text.split_whitespace().next().unwrap_or("");
            match first {
                "METAR" | "SPECI" => p.name = "METAR".into(),
                "TAF" | "TAF.AMD" => p.name = "TAF".into(),
                "PIREP" => p.name = "PIREP".into(),
                "WINDS" => p.name = "winds aloft".into(),
                _ => {}
            }
        }
        8 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 => {
            if let Some((loc, records)) = decode_twgo(data) {
                let mut t = Vec::new();
                for r in &records {
                    if let TwgoRecord::Text { text, .. } = r {
                        t.push(text.clone());
                    }
                }
                p.text = if t.is_empty() { loc } else { t.join("\n") };
                p.records = records;
            }
        }
        63 | 64 | 70 | 71 | 84 | 90 | 91 | 103 => {
            p.blocks = decode_grid(data);
            p.text = format!("{} blocks", p.blocks.len());
        }
        _ => {}
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apdu_time_formats() {
        for (tf, off) in [(0u8, 4usize), (1, 5), (2, 5), (3, 6)] {
            let mut v = encode_apdu(8, true, true, tf, 11, 23, 14, 35, 42);
            v.extend([0u8; 6]);
            let a = decode_apdu(&v).unwrap();
            assert_eq!(
                (
                    a.product_id,
                    a.a_flag,
                    a.s_flag,
                    a.time_format,
                    a.data_offset
                ),
                (8, true, true, tf, off)
            );
            assert_eq!((a.hours, a.minutes), (14, 35));
            if tf >= 2 {
                assert_eq!(a.month_day, Some((11, 23)));
            }
            if tf == 1 || tf == 3 {
                assert_eq!(a.seconds, Some(42));
            }
        }
        assert_eq!(product_name(8), "AIRMET");
        assert_eq!(product_name(999), "FIS-B 999");
    }

    #[test]
    fn dlac_and_products() {
        let t = "METAR KSEA 081853Z 18010KT 10SM FEW040 12/06 A3001=";
        assert_eq!(dlac_decode(&dlac_encode(t)), t);
        let mut f = encode_apdu(413, false, false, 0, 0, 0, 18, 53, 0);
        f.extend(dlac_encode(t));
        let p = decode_product(&f).unwrap();
        assert_eq!((p.name.as_str(), p.text.as_str()), ("METAR", t));
        let bins: Vec<u8> = (0..128).map(|i| (i / 20) as u8).collect();
        let mut f = encode_apdu(63, false, false, 0, 0, 0, 1, 2, 0);
        f.extend(encode_grid_block(123_456, false, 0, &bins));
        let p = decode_product(&f).unwrap();
        assert_eq!(p.blocks.len(), 1);
        assert_eq!(p.blocks[0].bins.as_deref(), Some(&bins[..]));
        let (la, lo, _, _) = p.blocks[0].area;
        assert!(
            (la - (274.0 * 4.0 / 60.0 + 4.0 / 60.0)).abs() < 1e-9
                && (lo - 156.0 * 0.8).abs() < 1e-9
        );
        let mut f = encode_apdu(11, false, false, 0, 0, 0, 1, 2, 0);
        f.extend(encode_twgo_text("KSEA", 1234, 26, "AIRMET SIERRA FOR IFR"));
        let p = decode_product(&f).unwrap();
        assert_eq!(p.text, "AIRMET SIERRA FOR IFR");
        let verts = vec![
            (47.5, -122.25, 10000),
            (48.0, -121.5, 10000),
            (47.0, -121.0, 12000),
        ];
        let mut f = encode_apdu(14, false, false, 0, 0, 0, 1, 2, 0);
        f.extend(encode_twgo_polygon("KSEA", 77, 5, &verts));
        let p = decode_product(&f).unwrap();
        match &p.records[0] {
            TwgoRecord::Graphic {
                report,
                label,
                geometry: Geometry::Path { polygon, vertices },
                ..
            } => {
                assert_eq!(
                    (*report, label.as_str(), *polygon, vertices.len()),
                    (77, "5", true, 3)
                );
                assert!(
                    (vertices[0].0 - 47.5).abs() < 1e-3
                        && (vertices[0].1 + 122.25).abs() < 1e-3
                        && vertices[2].2 == 12000
                );
            }
            r => panic!("{r:?}"),
        }
    }

    #[test]
    fn segmented_files_are_joined() {
        let text =
            "TAF KSEA 081720Z 0818/0918 18010KT P6SM BKN040 FM090200 20012KT 5SM -RA OVC025=";
        let data = dlac_encode(text);
        let segs = encode_segments(413, 37, 0, (17, 20, 0), &data, 20);
        assert!(segs.len() >= 3);
        // Every segment alone is the header only.
        for (i, f) in segs.iter().enumerate() {
            let p = decode_product(f).unwrap();
            assert!(p.apdu.s_flag && p.text.is_empty());
            let (id, len, n) = p.apdu.segment.unwrap();
            assert_eq!((id, len as usize, n as usize), (37, segs.len(), i + 1));
        }
        // Out of order, with a repeat and a segment of another file in between.
        let mut r = Reassembler::default();
        let other = encode_segments(413, 38, 0, (17, 20, 0), &data, 20);
        let mut order: Vec<usize> = (0..segs.len()).rev().collect();
        order.insert(1, order[0]);
        let mut whole = None;
        for (k, &i) in order.iter().enumerate() {
            if k == 1 {
                assert!(r.push(1.0, &other[0]).is_none());
            }
            let got = r.push(1.0 + k as f64, &segs[i]);
            if k + 1 < order.len() {
                assert!(got.is_none(), "complete too early at {k}");
            } else {
                whole = got;
            }
        }
        let p = whole.expect("the file is complete");
        assert_eq!((p.name.as_str(), p.text.as_str()), ("TAF", text));
        assert!(!p.apdu.s_flag && p.apdu.segment.is_none());
        assert_eq!((p.apdu.hours, p.apdu.minutes), (17, 20));
        assert_eq!(r.pending(), 1);
        // A file whose segments come too far apart is dropped.
        let mut r = Reassembler {
            timeout_s: 10.0,
            ..Default::default()
        };
        assert!(r.push(0.0, &segs[0]).is_none());
        for (i, f) in segs.iter().enumerate().skip(1) {
            assert!(r.push(20.0 + i as f64, f).is_none());
        }
        // Bad segment numbers are ignored.
        let mut bad = segs[0].clone();
        bad[4..8].copy_from_slice(&((37u32 << 22) | (2 << 13) | (3 << 4)).to_be_bytes());
        assert!(Reassembler::default().push(0.0, &bad).is_none());
    }

    #[test]
    fn graphic_records_and_their_times() {
        let t0 = TwgoTime {
            month: Some(11),
            day: Some(8),
            hours: 14,
            minutes: 0,
        };
        let t1 = TwgoTime {
            month: Some(11),
            day: Some(8),
            hours: 20,
            minutes: 30,
        };
        let line = Geometry::Path {
            polygon: false,
            vertices: vec![(40.0, -105.0, 18000), (41.5, -104.25, 18000)],
        };
        let circle = Geometry::Circle {
            centre: (-33.9, 151.2),
            radius_nm: 30.0,
            alt: (0, 18000),
        };
        let point = Geometry::Point((25.25, 55.36, 3000));
        for (g, times) in [
            (line, Some((t0, t1))),
            (
                circle,
                Some((
                    TwgoTime { month: None, ..t0 },
                    TwgoTime { month: None, ..t1 },
                )),
            ),
            (
                point,
                Some((
                    TwgoTime {
                        hours: 9,
                        minutes: 15,
                        ..Default::default()
                    },
                    TwgoTime {
                        hours: 10,
                        minutes: 45,
                        ..Default::default()
                    },
                )),
            ),
            (
                Geometry::Path {
                    polygon: true,
                    vertices: vec![(1.0, 2.0, 0), (1.5, 2.0, 0), (1.5, 2.5, 0)],
                },
                None,
            ),
        ] {
            let mut f = encode_apdu(16, false, false, 0, 0, 0, 1, 2, 0);
            f.extend(encode_twgo_graphic("KDEN", 501, 9, times, &g));
            let p = decode_product(&f).unwrap();
            let TwgoRecord::Graphic {
                report,
                label,
                times: got_times,
                geometry,
                ..
            } = &p.records[0]
            else {
                panic!("{:?}", p.records);
            };
            assert_eq!((*report, label.as_str()), (501, "9"));
            assert_eq!(*got_times, times);
            match (&g, geometry) {
                (
                    Geometry::Path {
                        polygon: a,
                        vertices: va,
                    },
                    Geometry::Path {
                        polygon: b,
                        vertices: vb,
                    },
                ) => {
                    assert_eq!((a, va.len()), (b, vb.len()));
                    for (x, y) in va.iter().zip(vb) {
                        assert!((x.0 - y.0).abs() < 1e-3 && (x.1 - y.1).abs() < 1e-3 && x.2 == y.2);
                    }
                }
                (
                    Geometry::Circle {
                        centre: ca,
                        radius_nm: ra,
                        alt: aa,
                    },
                    Geometry::Circle {
                        centre: cb,
                        radius_nm: rb,
                        alt: ab,
                    },
                ) => {
                    assert!((ca.0 - cb.0).abs() < 2e-3 && (ca.1 - cb.1).abs() < 2e-3);
                    assert!((ra - rb).abs() < 0.11 && aa == ab, "{rb} {ab:?}");
                }
                (Geometry::Point(a), Geometry::Point(b)) => {
                    assert!((a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3 && a.2 == b.2);
                }
                (a, b) => panic!("{a:?} became {b:?}"),
            }
        }
    }

    #[test]
    fn empty_blocks_of_a_grid() {
        // Block 1000 empty, then 1001 and 1003 from the first nibble, 1005 and 1006 from the bitmap.
        let mut f = encode_apdu(64, false, false, 0, 0, 0, 1, 2, 0);
        f.extend([0x00, 0x03, 0xE8, 0b0101_0001, 0b0000_0011]);
        let p = decode_product(&f).unwrap();
        let ids: Vec<u32> = p.blocks.iter().map(|b| b.block).collect();
        assert_eq!(ids, vec![1000, 1001, 1003, 1005, 1006]);
        assert!(p.blocks.iter().all(|b| b.bins.is_none()));
        // A southern block mirrors the latitude; scale 1 is five times larger.
        let (la, _, ls, _) = block_area(450, true, 1);
        assert!((la + 4.0 / 60.0).abs() < 1e-12 && (ls - 5.0 * 4.0 / 60.0).abs() < 1e-12);
    }
}
