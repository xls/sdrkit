//! Building AIS messages (for the generators and the tests): the inverse of [`super::message`] for the types a station sends.

use sdrkit_util_bits::pack::push_u64;
use sdrkit_util_text::sixbit::ais_code;

/// Message bits being written, one per byte, most significant bit first (a thin writer over `sdrkit_util_bits::pack::push_u64` and
/// the AIS 6-bit code).
#[derive(Clone, Debug, Default)]
pub(crate) struct BitWriter {
    /// The bits so far.
    pub bits: Vec<u8>,
}

impl BitWriter {
    /// No bits yet.
    pub fn new() -> Self {
        Self::default()
    }
    /// The low `len` bits of `v` (two's complement for a negative value).
    pub fn put(&mut self, v: i64, len: usize) -> &mut Self {
        push_u64(&mut self.bits, v as u64, len as u32);
        self
    }
    /// A flag.
    pub fn flag(&mut self, b: bool) -> &mut Self {
        self.put(i64::from(b), 1)
    }
    /// `chars` characters of `s` in the AIS 6-bit code, padded with `@`. A character outside the code becomes `@`.
    pub fn text(&mut self, s: &str, chars: usize) -> &mut Self {
        let mut it = s.chars();
        for _ in 0..chars {
            let v = it.next().map_or(0, |c| ais_code(c).unwrap_or(0));
            self.put(i64::from(v), 6);
        }
        self
    }
    /// Bits as they are.
    pub fn raw(&mut self, bits: &[u8]) -> &mut Self {
        self.bits.extend_from_slice(bits);
        self
    }
    /// The number of bits so far.
    pub fn len(&self) -> usize {
        self.bits.len()
    }
    /// Zeros up to `len` bits.
    #[cfg(test)]
    pub fn pad_to(&mut self, len: usize) -> &mut Self {
        self.bits.resize(self.bits.len().max(len), 0);
        self
    }
    /// The bits.
    pub fn finish(&self) -> Vec<u8> {
        self.bits.clone()
    }
}

fn header(w: &mut BitWriter, t: u8, mmsi: u32) {
    w.put(i64::from(t), 6).put(0, 2).put(i64::from(mmsi), 30);
}

fn lonlat(
    w: &mut BitWriter,
    lat: Option<f64>,
    lon: Option<f64>,
    lon_bits: usize,
    lat_bits: usize,
    scale: f64,
) {
    let lo = lon.map_or(181.0 * scale, |v| (v * scale).round());
    let la = lat.map_or(91.0 * scale, |v| (v * scale).round());
    w.put(lo as i64, lon_bits).put(la as i64, lat_bits);
}

/// A position report of class A (types 1, 2, 3).
pub struct PositionA {
    pub msg_type: u8,
    pub mmsi: u32,
    pub status: u8,
    /// Raw rate of turn (-128 not available).
    pub rot_raw: i8,
    pub sog_kn: Option<f64>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub cog_deg: Option<f64>,
    pub heading: Option<u16>,
    pub second: u8,
}

/// Type 1, 2 or 3 (168 bits).
pub fn position_a(p: &PositionA) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, p.msg_type, p.mmsi);
    w.put(i64::from(p.status), 4).put(i64::from(p.rot_raw), 8);
    w.put(
        p.sog_kn
            .map_or(1023, |v| (v * 10.0).round().min(1022.0) as i64),
        10,
    );
    w.flag(true);
    lonlat(&mut w, p.lat, p.lon, 28, 27, 600_000.0);
    w.put(
        p.cog_deg.map_or(3600, |v| (v * 10.0).round() as i64 % 3600),
        12,
    );
    w.put(i64::from(p.heading.unwrap_or(511)), 9);
    w.put(i64::from(p.second), 6)
        .put(0, 2)
        .put(0, 3)
        .flag(false)
        .put(0, 19);
    w.finish()
}

/// Type 18 (168 bits).
pub fn position_b(
    mmsi: u32,
    sog_kn: Option<f64>,
    lat: Option<f64>,
    lon: Option<f64>,
    cog: Option<f64>,
    heading: Option<u16>,
    second: u8,
) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 18, mmsi);
    w.put(0, 8);
    w.put(
        sog_kn.map_or(1023, |v| (v * 10.0).round().min(1022.0) as i64),
        10,
    );
    w.flag(false);
    lonlat(&mut w, lat, lon, 28, 27, 600_000.0);
    w.put(cog.map_or(3600, |v| (v * 10.0).round() as i64 % 3600), 12);
    w.put(i64::from(heading.unwrap_or(511)), 9);
    w.put(i64::from(second), 6).put(0, 2);
    w.flag(true)
        .flag(false)
        .flag(true)
        .flag(true)
        .flag(true)
        .flag(false)
        .flag(false);
    w.put(0b1100000000000000110, 20);
    w.finish()
}

/// Static and voyage data, type 5 (424 bits).
#[derive(Clone, Debug, Default)]
pub struct Static5 {
    pub mmsi: u32,
    pub imo: u32,
    pub callsign: String,
    pub name: String,
    pub ship_type: u8,
    pub dim: [u16; 4],
    pub eta: [u8; 4],
    pub draught_m: f64,
    pub destination: String,
}

/// Type 5.
pub fn static_voyage(s: &Static5) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 5, s.mmsi);
    w.put(0, 2)
        .put(i64::from(s.imo), 30)
        .text(&s.callsign, 7)
        .text(&s.name, 20);
    w.put(i64::from(s.ship_type), 8);
    w.put(i64::from(s.dim[0]), 9)
        .put(i64::from(s.dim[1]), 9)
        .put(i64::from(s.dim[2]), 6)
        .put(i64::from(s.dim[3]), 6);
    w.put(1, 4);
    for (v, n) in s.eta.iter().zip([4usize, 5, 5, 6]) {
        w.put(i64::from(*v), n);
    }
    w.put((s.draught_m * 10.0).round() as i64, 8)
        .text(&s.destination, 20)
        .flag(false)
        .put(0, 1);
    w.finish()
}

/// Base station report, type 4 (168 bits). `utc`: year, month, day, hour, minute, second.
pub fn base_station(mmsi: u32, utc: [u32; 6], lat: f64, lon: f64) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 4, mmsi);
    for (v, n) in utc.iter().zip([14usize, 4, 5, 5, 6, 6]) {
        w.put(i64::from(*v), n);
    }
    w.flag(true);
    lonlat(&mut w, Some(lat), Some(lon), 28, 27, 600_000.0);
    w.put(7, 4).put(0, 10).flag(false).put(0, 19);
    w.finish()
}

/// SAR aircraft, type 9 (168 bits).
pub fn sar_aircraft(
    mmsi: u32,
    altitude_m: u16,
    sog_kn: u16,
    lat: f64,
    lon: f64,
    cog: f64,
    second: u8,
) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 9, mmsi);
    w.put(i64::from(altitude_m), 12)
        .put(i64::from(sog_kn), 10)
        .flag(false);
    lonlat(&mut w, Some(lat), Some(lon), 28, 27, 600_000.0);
    w.put((cog * 10.0).round() as i64 % 3600, 12)
        .put(i64::from(second), 6)
        .put(0, 8)
        .flag(true)
        .put(0, 3)
        .flag(false)
        .flag(false)
        .put(0, 20);
    w.finish()
}

/// Safety broadcast, type 14.
pub fn safety_broadcast(mmsi: u32, text: &str) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 14, mmsi);
    w.put(0, 2).text(text, text.chars().count());
    w.finish()
}

/// Broadcast binary, type 8.
pub fn broadcast_binary(mmsi: u32, dac: u16, fi: u8, data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 8, mmsi);
    w.put(0, 2)
        .put(i64::from(dac), 10)
        .put(i64::from(fi), 6)
        .raw(data);
    w.finish()
}

/// Extended class B report, type 19 (312 bits).
#[allow(clippy::too_many_arguments)]
pub fn position_b_ext(
    mmsi: u32,
    sog_kn: f64,
    lat: f64,
    lon: f64,
    cog: f64,
    heading: Option<u16>,
    second: u8,
    name: &str,
    ship_type: u8,
    dim: [u16; 4],
) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 19, mmsi);
    w.put(0, 8)
        .put((sog_kn * 10.0).round() as i64, 10)
        .flag(false);
    lonlat(&mut w, Some(lat), Some(lon), 28, 27, 600_000.0);
    w.put((cog * 10.0).round() as i64 % 3600, 12)
        .put(i64::from(heading.unwrap_or(511)), 9)
        .put(i64::from(second), 6)
        .put(0, 4);
    w.text(name, 20).put(i64::from(ship_type), 8);
    w.put(i64::from(dim[0]), 9)
        .put(i64::from(dim[1]), 9)
        .put(i64::from(dim[2]), 6)
        .put(i64::from(dim[3]), 6);
    w.put(1, 4).flag(false).flag(true).flag(false).put(0, 4);
    w.finish()
}

/// Aid to navigation, type 21 (272 bits plus the name extension).
pub fn aid_to_navigation(
    mmsi: u32,
    aid_type: u8,
    name: &str,
    lat: f64,
    lon: f64,
    virtual_aid: bool,
) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 21, mmsi);
    let first: String = name.chars().take(20).collect();
    let ext: String = name.chars().skip(20).take(14).collect();
    w.put(i64::from(aid_type), 5).text(&first, 20).flag(false);
    lonlat(&mut w, Some(lat), Some(lon), 28, 27, 600_000.0);
    w.put(0, 9)
        .put(0, 9)
        .put(0, 6)
        .put(0, 6)
        .put(7, 4)
        .put(30, 6)
        .flag(false)
        .put(0, 8)
        .flag(false)
        .flag(virtual_aid)
        .flag(false)
        .put(0, 1);
    if !ext.is_empty() {
        let n = ext.chars().count();
        w.text(&ext, n);
        let pad = (8 - w.len() % 8) % 8;
        w.put(0, pad);
    }
    w.finish()
}

/// Static data B part A, type 24 (160 bits).
pub fn static_b_a(mmsi: u32, name: &str) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 24, mmsi);
    w.put(0, 2).text(name, 20);
    w.finish()
}

/// Static data B part B, type 24 (168 bits).
pub fn static_b_b(
    mmsi: u32,
    ship_type: u8,
    vendor: &str,
    callsign: &str,
    dim: [u16; 4],
) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 24, mmsi);
    w.put(1, 2)
        .put(i64::from(ship_type), 8)
        .text(vendor, 3)
        .put(1, 4)
        .put(12345, 20)
        .text(callsign, 7);
    w.put(i64::from(dim[0]), 9)
        .put(i64::from(dim[1]), 9)
        .put(i64::from(dim[2]), 6)
        .put(i64::from(dim[3]), 6)
        .put(0, 6);
    w.finish()
}

/// Long-range position, type 27 (96 bits). Out-of-range SOG and COG become 63 and 511.
pub fn long_range(
    mmsi: u32,
    status: u8,
    lat: Option<f64>,
    lon: Option<f64>,
    sog_kn: Option<f64>,
    cog_deg: Option<f64>,
    latency: bool,
) -> Vec<u8> {
    let mut w = BitWriter::new();
    header(&mut w, 27, mmsi);
    w.flag(false).flag(false).put(i64::from(status), 4);
    lonlat(&mut w, lat, lon, 18, 17, 600.0);
    let sog = sog_kn.map_or(63, |v| {
        if (0.0..63.0).contains(&v.round()) {
            v.round() as i64
        } else {
            63
        }
    });
    let cog = cog_deg.map_or(511, |v| {
        if (0.0..360.0).contains(&v.round()) {
            v.round() as i64
        } else {
            511
        }
    });
    w.put(sog, 6).put(cog, 9).flag(latency).put(0, 1);
    w.finish()
}
