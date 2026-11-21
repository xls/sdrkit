//! ATSC 3.0 low-level signalling: LLS tables (the gzip comes from `sdrkit-proto-ipcast`) and the service list table (SLT). ROUTE,
//! the IP layers and fragmented MP4 are in `sdrkit-proto-ipcast`.
//!
//! Standard: ATSC A/331 clause 6.

use sdrkit_proto_ipcast::gzip;

/// The LLS address and port.
pub const LLS_ADDR: [u8; 4] = [224, 0, 23, 60];
/// The LLS port.
pub const LLS_PORT: u16 = 4937;
/// The largest LLS table taken after decompression (A/331 does not limit it. This keeps a broken table from using much memory).
pub const LLS_MAX: usize = 1 << 20;

/// Table id of the service list table.
pub const TABLE_SLT: u8 = 0x01;
/// Table id of the system time.
pub const TABLE_SYSTEM_TIME: u8 = 0x03;
/// The last table id read as one gzip member (0x01 SLT to 0x05 onscreen message notification).
pub const TABLE_LAST_PLAIN: u8 = 0x05;
/// Table id of the signed multi-table.
pub const TABLE_SIGNED_MULTI: u8 = 0xFE;

/// An LLS table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lls {
    /// Table id (1 = SLT).
    pub table_id: u8,
    /// Group id.
    pub group_id: u8,
    /// Groups in all, minus 1.
    pub group_count_minus1: u8,
    /// Version.
    pub version: u8,
    /// The XML (after gunzip).
    pub xml: String,
}

/// The tables of one LLS datagram.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LlsDatagram {
    /// The tables read, in the order they come.
    pub tables: Vec<Lls>,
    /// The datagram is a signed multi-table (id 0xFE). The signature is not verified.
    pub signed: bool,
    /// Tables refused: a broken gzip member or CRC, text that is not UTF-8, or a head or length that runs past the end.
    pub refused: usize,
}

/// One table body: a gzip member holding UTF-8 XML.
fn table(
    table_id: u8,
    group_id: u8,
    group_count_minus1: u8,
    version: u8,
    body: &[u8],
) -> Option<Lls> {
    let xml = String::from_utf8(gzip::decompress(body, LLS_MAX).ok()?).ok()?;
    Some(Lls {
        table_id,
        group_id,
        group_count_minus1,
        version,
        xml,
    })
}

/// Reads an LLS datagram (ATSC A/331): the 4-byte head (table id, group id, group count minus 1, version), then for ids
/// 0x01 to 0x05 one gzip member, for id 0xFE (signed multi-table) a payload count and per payload its id, version, 16-bit
/// length and gzip member, then the signature, which is not checked. The walk over the payloads stops at the first whose head
/// or length runs past the end; each payload is read as a plain table of its id with the group of the outer head. Other ids
/// and payloads of other ids are skipped. A datagram shorter than its head gives nothing.
pub fn parse_lls(b: &[u8]) -> LlsDatagram {
    let mut out = LlsDatagram::default();
    let Some(&[id, group, count, version]) = b.get(..4) else {
        out.refused += usize::from(!b.is_empty());
        return out;
    };
    let push = |out: &mut LlsDatagram, id: u8, version: u8, body: &[u8]| {
        if !(TABLE_SLT..=TABLE_LAST_PLAIN).contains(&id) {
            return;
        }
        match table(id, group, count, version, body) {
            Some(t) => out.tables.push(t),
            None => out.refused += 1,
        }
    };
    if id != TABLE_SIGNED_MULTI {
        push(&mut out, id, version, &b[4..]);
        return out;
    }
    out.signed = true;
    let Some(&n) = b.get(4) else {
        out.refused += 1;
        return out;
    };
    let mut at = 5;
    for _ in 0..n {
        let Some(&[pid, pver, l0, l1]) = b.get(at..at + 4) else {
            out.refused += 1;
            break;
        };
        let len = usize::from(u16::from_be_bytes([l0, l1]));
        let Some(body) = b.get(at + 4..at + 4 + len) else {
            out.refused += 1;
            break;
        };
        push(&mut out, pid, pver, body);
        at += 4 + len;
    }
    out
}

/// Builds an LLS table of one group.
pub fn build_lls(table_id: u8, group: u8, version: u8, xml: &str) -> Vec<u8> {
    let mut v = vec![table_id, group, 0, version];
    gzip::compress(xml.as_bytes(), 6, &mut v);
    v
}

/// Builds a signed multi-table of one group from `(table id, version, XML)` payloads and the bytes of a signature.
pub fn build_signed_lls(
    group: u8,
    version: u8,
    tables: &[(u8, u8, &str)],
    signature: &[u8],
) -> Vec<u8> {
    let mut v = vec![TABLE_SIGNED_MULTI, group, 0, version, tables.len() as u8];
    let mut z = Vec::new();
    for &(id, ver, xml) in tables {
        z.clear();
        gzip::compress(xml.as_bytes(), 6, &mut z);
        v.extend_from_slice(&[id, ver]);
        v.extend_from_slice(&(z.len() as u16).to_be_bytes());
        v.extend_from_slice(&z);
    }
    v.extend_from_slice(&(signature.len() as u16).to_be_bytes());
    v.extend_from_slice(signature);
    v
}

/// The system time table (A/331 clause 6.4, table 6.7): the leap seconds between TAI and UTC, the local offset from UTC, the
/// leap second and daylight saving announcements.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemTime {
    /// `currentUtcOffset` (required): TAI minus UTC, seconds.
    pub utc_offset_s: i32,
    /// `utcLocalOffset` (required): the local time minus UTC as an XML duration (`-PT5H`). `None` when the table lacks it.
    pub local_offset: Option<String>,
    /// `ptpPrepend`: the upper 16 bits of the 48-bit PTP seconds count, 0 when absent.
    pub ptp_prepend: u16,
    /// `leap59`: the last minute of the UTC day has 59 seconds (false when absent).
    pub leap59: bool,
    /// `leap61`: the last minute of the UTC day has 61 seconds (false when absent).
    pub leap61: bool,
    /// `dsStatus`: daylight saving time is in effect (false when absent).
    pub ds_status: bool,
    /// `dsDayOfMonth`: the local day of the month (1 to 31) of the next daylight saving transition, when one is announced.
    pub ds_day_of_month: Option<u8>,
    /// `dsHour`: the local hour (0 to 23) of that transition.
    pub ds_hour: Option<u8>,
    /// What the table breaks of A/331: a required attribute missing, a value out of range or of the wrong type. Such a value
    /// reads as absent.
    pub notes: Vec<String>,
}

/// Parses the SystemTime XML. `None` without a `SystemTime` element or its `currentUtcOffset`. A missing `utcLocalOffset` or a
/// bad optional attribute still gives the other fields, with a note.
pub fn parse_system_time(xml: &str) -> Option<SystemTime> {
    let root = xml.find("<SystemTime")?;
    let end = xml[root..].find('>')? + root;
    let head = &xml[root..end];
    let mut t = SystemTime {
        utc_offset_s: attr(head, "currentUtcOffset")?.parse().ok()?,
        local_offset: attr(head, "utcLocalOffset").map(String::from),
        ..Default::default()
    };
    if t.local_offset.is_none() {
        t.notes.push("utcLocalOffset is missing (required)".into());
    }
    let mut notes = Vec::new();
    let mut number = |name: &str, max: u16, min: u16| -> Option<u16> {
        let v = attr(head, name)?;
        match v.trim().parse::<u16>() {
            Ok(n) if (min..=max).contains(&n) => Some(n),
            _ => {
                notes.push(format!(
                    "{name} \"{v}\" is not a number from {min} to {max}"
                ));
                None
            }
        }
    };
    t.ptp_prepend = number("ptpPrepend", u16::MAX, 0).unwrap_or(0);
    t.ds_day_of_month = number("dsDayOfMonth", 31, 1).map(|n| n as u8);
    t.ds_hour = number("dsHour", 23, 0).map(|n| n as u8);
    let mut flag = |name: &str| match attr(head, name).map(str::trim) {
        None => false,
        Some("true" | "1") => true,
        Some("false" | "0") => false,
        Some(v) => {
            notes.push(format!("{name} \"{v}\" is not a boolean"));
            false
        }
    };
    t.leap59 = flag("leap59");
    t.leap61 = flag("leap61");
    t.ds_status = flag("dsStatus");
    if t.ds_day_of_month.is_some() != t.ds_hour.is_some() {
        notes.push("dsDayOfMonth and dsHour come only together".into());
    }
    t.notes.append(&mut notes);
    Some(t)
}

/// A service of the SLT.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SltService {
    /// serviceId.
    pub id: u16,
    /// Major channel.
    pub major: u16,
    /// Minor channel.
    pub minor: u16,
    /// serviceCategory (1 TV, 2 audio, 3 app, 4 guide, 5 alerts, 6 DRM).
    pub category: u8,
    /// shortServiceName.
    pub short_name: String,
    /// globalServiceID.
    pub global_id: String,
    /// hidden.
    pub hidden: bool,
    /// protected.
    pub protected: bool,
    /// slsProtocol (1 ROUTE, 2 MMTP).
    pub sls_protocol: u8,
    /// SLS destination address.
    pub sls_dst: String,
    /// SLS source address.
    pub sls_src: String,
    /// SLS destination port.
    pub sls_port: u16,
    /// sltSvcSeqNum.
    pub seq: u8,
}

/// The SLT.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Slt {
    /// BSIDs.
    pub bsids: Vec<u16>,
    /// Services.
    pub services: Vec<SltService>,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let pat = format!("{name}=\"");
    let i = tag.find(&pat)? + pat.len();
    let j = tag[i..].find('"')? + i;
    Some(&tag[i..j])
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Parses the SLT XML (attributes as A/331 Table 6.2 names them).
pub fn parse_slt(xml: &str) -> Option<Slt> {
    let root = xml.find("<SLT")?;
    let end = xml[root..].find('>')? + root;
    let head = &xml[root..end];
    let bsids = attr(head, "bsid")
        .map(|s| {
            s.split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    let mut services = Vec::new();
    let mut rest = &xml[end..];
    while let Some(i) = rest.find("<Service ") {
        let r = &rest[i..];
        let close = r
            .find("</Service>")
            .or_else(|| r.find("/>"))
            .unwrap_or(r.len());
        let body = &r[..close];
        let tag_end = body.find('>').unwrap_or(body.len());
        let tag = &body[..tag_end];
        let num = |n: &str| {
            attr(tag, n)
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(0)
        };
        let mut s = SltService {
            id: num("serviceId"),
            major: num("majorChannelNo"),
            minor: num("minorChannelNo"),
            category: num("serviceCategory") as u8,
            short_name: attr(tag, "shortServiceName")
                .map(unescape)
                .unwrap_or_default(),
            global_id: attr(tag, "globalServiceID").unwrap_or("").into(),
            hidden: attr(tag, "hidden") == Some("true"),
            protected: attr(tag, "protected") == Some("true"),
            seq: num("sltSvcSeqNum") as u8,
            ..Default::default()
        };
        if let Some(j) = body.find("<BroadcastSvcSignaling") {
            let bt = &body[j..];
            let bt = &bt[..bt.find('>').unwrap_or(bt.len())];
            s.sls_protocol = attr(bt, "slsProtocol")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            s.sls_dst = attr(bt, "slsDestinationIpAddress").unwrap_or("").into();
            s.sls_src = attr(bt, "slsSourceIpAddress").unwrap_or("").into();
            s.sls_port = attr(bt, "slsDestinationUdpPort")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        }
        services.push(s);
        rest = &r[close.min(r.len())..];
        if close == 0 {
            break;
        }
    }
    Some(Slt { bsids, services })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lls_and_slt() {
        let xml = r#"<?xml version="1.0"?><SLT xmlns="tag:atsc.org,2016:XMLSchemas/ATSC3/Delivery/SLT/1.0/" bsid="1234 77">
<Service serviceId="1001" globalServiceID="urn:x" sltSvcSeqNum="0" majorChannelNo="5" minorChannelNo="1" serviceCategory="1" shortServiceName="TEST-HD"><BroadcastSvcSignaling slsProtocol="1" slsDestinationIpAddress="239.255.10.1" slsDestinationUdpPort="3000" slsSourceIpAddress="10.0.0.1"/></Service>
<Service serviceId="1002" sltSvcSeqNum="3" majorChannelNo="5" minorChannelNo="2" serviceCategory="2" shortServiceName="Radio &amp; Co" hidden="true"><BroadcastSvcSignaling slsProtocol="2" slsDestinationIpAddress="239.255.10.2" slsDestinationUdpPort="3001" slsSourceIpAddress="10.0.0.1"/></Service></SLT>"#;
        let lls = build_lls(1, 0, 3, xml);
        let d = parse_lls(&lls);
        assert!(!d.signed && d.refused == 0 && d.tables.len() == 1);
        let l = &d.tables[0];
        assert_eq!((l.table_id, l.group_id, l.version), (1, 0, 3));
        let slt = parse_slt(&l.xml).unwrap();
        assert_eq!(slt.bsids, vec![1234, 77]);
        let a = &slt.services[0];
        assert_eq!(
            (
                a.id,
                a.major,
                a.minor,
                a.category,
                a.short_name.as_str(),
                a.sls_protocol
            ),
            (1001, 5, 1, 1, "TEST-HD", 1)
        );
        let b = &slt.services[1];
        assert_eq!(
            (
                b.minor,
                b.short_name.as_str(),
                b.hidden,
                b.sls_protocol,
                b.sls_port,
                b.seq
            ),
            (2, "Radio & Co", true, 2, 3001, 3)
        );
        assert!(parse_lls(&lls[..3]).tables.is_empty());
        let mut bad = lls.clone();
        let k = bad.len() - 10;
        bad[k] ^= 0xFF;
        let d = parse_lls(&bad);
        assert!(
            d.tables.is_empty() && d.refused == 1,
            "a corrupted table is refused"
        );
    }

    const SLT: &str = r#"<SLT bsid="50"><Service serviceId="5" majorChannelNo="9" minorChannelNo="1" serviceCategory="1" shortServiceName="S"><BroadcastSvcSignaling slsProtocol="1" slsDestinationIpAddress="239.255.1.1" slsDestinationUdpPort="5000"/></Service></SLT>"#;
    const TIME: &str = r#"<SystemTime xmlns="tag:atsc.org,2016:XMLSchemas/ATSC3/Delivery/SYSTIME/1.0/" currentUtcOffset="37" utcLocalOffset="-PT5H"/>"#;

    #[test]
    fn signed_multi_table_gives_every_table() {
        // A dummy signature: the reader does not check it.
        let sig = [0xA5u8; 40];
        let b = build_signed_lls(
            2,
            7,
            &[(TABLE_SLT, 4, SLT), (TABLE_SYSTEM_TIME, 1, TIME)],
            &sig,
        );
        let d = parse_lls(&b);
        assert!(d.signed && d.refused == 0, "{d:?}");
        assert_eq!(d.tables.len(), 2);
        let (slt, time) = (&d.tables[0], &d.tables[1]);
        assert_eq!((slt.table_id, slt.group_id, slt.version), (TABLE_SLT, 2, 4));
        assert_eq!(parse_slt(&slt.xml).unwrap().services[0].id, 5);
        assert_eq!((time.table_id, time.version), (TABLE_SYSTEM_TIME, 1));
        assert_eq!(
            parse_system_time(&time.xml).unwrap(),
            SystemTime {
                utc_offset_s: 37,
                local_offset: Some("-PT5H".into()),
                ..Default::default()
            }
        );
        // Every attribute of A/331 table 6.7.
        let full = parse_system_time(
            r#"<SystemTime currentUtcOffset="37" utcLocalOffset="-PT4H30M" ptpPrepend="1" leap59="false" leap61="true" dsStatus="1" dsDayOfMonth="31" dsHour="2"/>"#,
        )
        .unwrap();
        assert_eq!(
            full,
            SystemTime {
                utc_offset_s: 37,
                local_offset: Some("-PT4H30M".into()),
                ptp_prepend: 1,
                leap59: false,
                leap61: true,
                ds_status: true,
                ds_day_of_month: Some(31),
                ds_hour: Some(2),
                notes: Vec::new(),
            }
        );
        // utcLocalOffset is required: without it the other fields still come, with a note. Bad values read as absent.
        let partial = parse_system_time(
            r#"<SystemTime currentUtcOffset="37" leap59="yes" dsDayOfMonth="32" dsHour="5"/>"#,
        )
        .unwrap();
        assert_eq!(
            (partial.utc_offset_s, &partial.local_offset, partial.leap59),
            (37, &None, false)
        );
        assert_eq!((partial.ds_day_of_month, partial.ds_hour), (None, Some(5)));
        assert_eq!(partial.notes.len(), 4, "{:?}", partial.notes);
        assert!(partial.notes[0].contains("utcLocalOffset"));
        assert!(parse_system_time(r#"<SystemTime utcLocalOffset="-PT5H"/>"#).is_none());
        // Cut at every 37th byte: no panic, the tables wholly before the cut and nothing past it.
        let slt_end = 5 + 4 + u16::from_be_bytes([b[7], b[8]]) as usize;
        let time_end = slt_end + 4 + u16::from_be_bytes([b[slt_end + 2], b[slt_end + 3]]) as usize;
        for cut in (0..b.len()).step_by(37) {
            let d = parse_lls(&b[..cut]);
            let want = usize::from(cut >= slt_end) + usize::from(cut >= time_end);
            assert_eq!(d.tables.len(), want, "cut at {cut}");
            assert!(d
                .tables
                .iter()
                .all(|t| t.table_id == TABLE_SLT || cut >= time_end));
        }
        // A payload length that runs past the end stops the walk there: the SLT before it is kept.
        let mut long = b.clone();
        long[slt_end + 2..slt_end + 4].copy_from_slice(&u16::MAX.to_be_bytes());
        let d = parse_lls(&long);
        assert_eq!((d.tables.len(), d.refused), (1, 1));
        assert_eq!(d.tables[0].table_id, TABLE_SLT);
        // A payload of an id that is not decoded is skipped and the walk goes on.
        let b = build_signed_lls(0, 0, &[(0x80, 0, "x"), (TABLE_SLT, 0, SLT)], &sig);
        let d = parse_lls(&b);
        assert_eq!(d.tables.len(), 1);
        assert_eq!(d.tables[0].table_id, TABLE_SLT);
    }
}
