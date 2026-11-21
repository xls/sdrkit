//! The meaning of Modbus frames: function and exception names, request or reply, a summary with the conventional register
//! numbers, and the values of a reply for the register map.
//!
//! Modbus Application Protocol Specification V1.1b3.

/// A Modbus frame: address, function and the data between the function and the check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pdu<'a> {
    /// The station address (0 is broadcast).
    pub address: u8,
    /// The function code (with bit 7 for an exception).
    pub function: u8,
    /// The data after the function, without the check.
    pub data: &'a [u8],
}

impl<'a> Pdu<'a> {
    /// The frame of an RTU frame with its 2 CRC bytes, or of the bytes of an ASCII frame with its LRC.
    pub fn of(frame: &'a [u8], check_len: usize) -> Option<Pdu<'a>> {
        if frame.len() < 2 + check_len {
            return None;
        }
        Some(Pdu {
            address: frame[0],
            function: frame[1],
            data: &frame[2..frame.len() - check_len],
        })
    }

    /// The function without the exception bit.
    pub fn base(&self) -> u8 {
        self.function & 0x7F
    }

    /// Whether it is an exception reply.
    pub fn is_exception(&self) -> bool {
        self.function & 0x80 != 0
    }

    fn u16_at(&self, at: usize) -> Option<u16> {
        Some(u16::from(*self.data.get(at)?) << 8 | u16::from(*self.data.get(at + 1)?))
    }

    /// Whether the frame has the form of a request of its function (5, 6, 8 and 22 have the same form both ways).
    pub fn looks_like_request(&self) -> bool {
        if self.is_exception() {
            return false;
        }
        let n = self.data.len();
        match self.function {
            1..=6 | 8 => n == 4,
            7 | 11 | 12 | 17 => n == 0,
            15 | 16 => n >= 5 && usize::from(self.data[4]) == n - 5,
            22 => n == 6,
            23 => n >= 9 && usize::from(self.data[8]) == n - 9,
            24 => n == 2,
            _ => true,
        }
    }
}

/// The name of a function.
pub fn function_name(f: u8) -> &'static str {
    match f & 0x7F {
        1 => "read coils",
        2 => "read discrete inputs",
        3 => "read holding registers",
        4 => "read input registers",
        5 => "write single coil",
        6 => "write single register",
        7 => "read exception status",
        8 => "diagnostics",
        11 => "get comm event counter",
        12 => "get comm event log",
        15 => "write multiple coils",
        16 => "write multiple registers",
        17 => "report server ID",
        20 => "read file record",
        21 => "write file record",
        22 => "mask write register",
        23 => "read/write multiple registers",
        24 => "read FIFO queue",
        43 => "encapsulated interface",
        _ => "unknown function",
    }
}

/// The name of an exception code.
pub fn exception_name(code: u8) -> &'static str {
    match code {
        1 => "illegal function",
        2 => "illegal data address",
        3 => "illegal data value",
        4 => "server device failure",
        5 => "acknowledge",
        6 => "server device busy",
        7 => "negative acknowledge",
        8 => "memory parity error",
        10 => "gateway path unavailable",
        11 => "gateway target failed to respond",
        _ => "unknown exception",
    }
}

/// The conventional number of item `index` of the table that function `f` reads or writes: coils 00001, discrete inputs
/// 10001, input registers 30001, holding registers 40001.
pub fn number(f: u8, index: u32) -> u32 {
    let base = match f & 0x7F {
        1 | 5 | 15 => 1,
        2 => 10_001,
        4 => 30_001,
        _ => 40_001,
    };
    base + index
}

fn num_text(f: u8, index: u32) -> String {
    format!("{:05}", number(f, index))
}

/// What a request asked for: the function, the first index and the quantity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asked {
    /// The function.
    pub function: u8,
    /// The first index.
    pub start: u16,
    /// The quantity.
    pub quantity: u16,
}

/// What a request asks for, for the reading of its reply.
pub fn asked(p: &Pdu) -> Asked {
    let start = p.u16_at(0).unwrap_or(0);
    let quantity = match p.function {
        1..=4 | 15 | 16 => p.u16_at(2).unwrap_or(0),
        23 => p.u16_at(2).unwrap_or(0),
        _ => 1,
    };
    Asked {
        function: p.function,
        start,
        quantity,
    }
}

/// The summary of a request.
pub fn request_summary(p: &Pdu) -> String {
    let f = p.function;
    let a = asked(p);
    match f {
        1 | 2 => format!(
            "start {} ({}), quantity {}",
            a.start,
            num_text(f, u32::from(a.start)),
            a.quantity
        ),
        3 | 4 => format!(
            "start {} (register {}), quantity {}",
            a.start,
            num_text(f, u32::from(a.start)),
            a.quantity
        ),
        5 => format!(
            "coil {} = {}",
            num_text(f, u32::from(a.start)),
            match p.u16_at(2) {
                Some(0xFF00) => "ON",
                Some(0x0000) => "OFF",
                _ => "a bad value",
            }
        ),
        6 => format!(
            "register {} = {}",
            num_text(f, u32::from(a.start)),
            p.u16_at(2).unwrap_or(0)
        ),
        8 => format!(
            "sub-function {}, data {}",
            a.start,
            hex(p.data.get(2..).unwrap_or(&[]))
        ),
        15 => format!(
            "coils from {}, quantity {}",
            num_text(f, u32::from(a.start)),
            a.quantity
        ),
        16 => {
            let values: Vec<String> = p
                .data
                .get(5..)
                .unwrap_or(&[])
                .chunks(2)
                .filter(|c| c.len() == 2)
                .map(|c| (u16::from(c[0]) << 8 | u16::from(c[1])).to_string())
                .collect();
            format!(
                "registers from {}, quantity {}: {}",
                num_text(f, u32::from(a.start)),
                a.quantity,
                values.join(", ")
            )
        }
        22 => format!(
            "register {}, AND mask {:04X}, OR mask {:04X}",
            num_text(f, u32::from(a.start)),
            p.u16_at(2).unwrap_or(0),
            p.u16_at(4).unwrap_or(0)
        ),
        23 => format!(
            "read {} from {}, write {} from {}",
            a.quantity,
            num_text(3, u32::from(a.start)),
            p.u16_at(6).unwrap_or(0),
            num_text(16, u32::from(p.u16_at(4).unwrap_or(0)))
        ),
        24 => format!("queue at {}", num_text(3, u32::from(a.start))),
        43 => format!("MEI type {:02X}", p.data.first().copied().unwrap_or(0)),
        7 | 11 | 12 | 17 => String::new(),
        _ => format!("{} bytes", p.data.len()),
    }
}

/// The values of a reply of function 1 to 4 or 23 (number and value), with the request it answers if known.
pub fn reply_values(p: &Pdu, req: Option<&Asked>) -> Vec<(Option<u32>, u16)> {
    let f = p.function;
    let Some(&n) = p.data.first() else {
        return Vec::new();
    };
    let body = p.data.get(1..1 + usize::from(n)).unwrap_or(&[]);
    let start = req.filter(|r| r.function == f).map(|r| u32::from(r.start));
    match f {
        1 | 2 => {
            let q = req
                .filter(|r| r.function == f)
                .map_or(body.len() * 8, |r| usize::from(r.quantity));
            (0..q.min(body.len() * 8))
                .map(|k| {
                    let v = u16::from(body[k / 8] >> (k % 8) & 1);
                    (start.map(|s| number(f, s + k as u32)), v)
                })
                .collect()
        }
        3 | 4 | 23 => body
            .chunks(2)
            .filter(|c| c.len() == 2)
            .enumerate()
            .map(|(k, c)| {
                let v = u16::from(c[0]) << 8 | u16::from(c[1]);
                (
                    start.map(|s| number(if f == 23 { 3 } else { f }, s + k as u32)),
                    v,
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The summary of a reply, with the request it answers if known.
pub fn reply_summary(p: &Pdu, req: Option<&Asked>) -> String {
    if p.is_exception() {
        let c = p.data.first().copied().unwrap_or(0);
        return format!("exception {c}: {}", exception_name(c));
    }
    let f = p.function;
    match f {
        1..=4 | 23 => {
            let n = p.data.first().copied().unwrap_or(0);
            let values: Vec<String> = reply_values(p, req)
                .iter()
                .map(|(num, v)| match num {
                    Some(k) => format!("{k:05} = {v}"),
                    None => v.to_string(),
                })
                .collect();
            format!("{n} bytes: {}", values.join(", "))
        }
        5 | 6 | 8 | 22 => format!("echo: {}", request_summary(p)),
        7 => format!("status {:02X}", p.data.first().copied().unwrap_or(0)),
        11 => format!(
            "status {:04X}, event count {}",
            p.u16_at(0).unwrap_or(0),
            p.u16_at(2).unwrap_or(0)
        ),
        15 | 16 => {
            let a = asked(p);
            let what = if f == 15 { "coils" } else { "registers" };
            format!(
                "{what} from {}, quantity {}",
                num_text(f, u32::from(a.start)),
                a.quantity
            )
        }
        24 => format!("{} registers", p.u16_at(2).unwrap_or(0)),
        _ => format!("{} bytes", p.data.len()),
    }
}

/// Bytes as hex pairs with spaces.
pub fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(3 * b.len());
    for (k, v) in b.iter().enumerate() {
        if k > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{v:02X}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pdu(b: &[u8]) -> Pdu<'_> {
        Pdu::of(b, 2).unwrap()
    }

    #[test]
    fn the_worked_frames_mean_what_they_say() {
        let req = [0x11, 0x03, 0x00, 0x6B, 0x00, 0x03, 0x76, 0x87];
        let p = pdu(&req);
        assert!(p.looks_like_request());
        assert_eq!(function_name(p.function), "read holding registers");
        assert_eq!(
            request_summary(&p),
            "start 107 (register 40108), quantity 3"
        );
        let rep = [
            0x11, 0x03, 0x06, 0x02, 0x2B, 0x00, 0x00, 0x00, 0x64, 0xC8, 0xBA,
        ];
        let r = pdu(&rep);
        assert!(!r.looks_like_request());
        let a = asked(&p);
        assert_eq!(
            reply_summary(&r, Some(&a)),
            "6 bytes: 40108 = 555, 40109 = 0, 40110 = 100"
        );
        assert_eq!(reply_summary(&r, None), "6 bytes: 555, 0, 100");
        let ex = [0x01, 0x83, 0x02, 0xC0, 0xF1];
        let e = pdu(&ex);
        assert!(e.is_exception());
        assert_eq!(reply_summary(&e, None), "exception 2: illegal data address");
        let r3 = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A, 0xC5, 0xCD];
        assert_eq!(
            request_summary(&pdu(&r3)),
            "start 0 (register 40001), quantity 10"
        );
    }

    #[test]
    fn writes_coils_and_diagnostics() {
        let p = pdu(&[0x11, 0x05, 0x00, 0xAC, 0xFF, 0x00, 0, 0]);
        assert_eq!(request_summary(&p), "coil 00173 = ON");
        let p = pdu(&[0x11, 0x06, 0x00, 0x01, 0x00, 0x03, 0, 0]);
        assert_eq!(request_summary(&p), "register 40002 = 3");
        let p = pdu(&[
            0x11, 0x10, 0x00, 0x01, 0x00, 0x02, 0x04, 0x00, 0x0A, 0x01, 0x02, 0, 0,
        ]);
        assert!(p.looks_like_request());
        assert_eq!(
            request_summary(&p),
            "registers from 40002, quantity 2: 10, 258"
        );
        let p = pdu(&[0x11, 0x08, 0x00, 0x00, 0xA5, 0x37, 0, 0]);
        assert_eq!(request_summary(&p), "sub-function 0, data A5 37");
        // An 8-byte coil reply: 3 bytes of coils.
        let req = pdu(&[0x11, 0x01, 0x00, 0x13, 0x00, 0x13, 0, 0]);
        let rep = pdu(&[0x11, 0x01, 0x03, 0xCD, 0x6B, 0x05, 0, 0]);
        let v = reply_values(&rep, Some(&asked(&req)));
        assert_eq!(v.len(), 19);
        assert_eq!(v[0], (Some(20), 1));
        assert_eq!(v[1], (Some(21), 0));
    }
}
