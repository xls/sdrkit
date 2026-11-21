//! The class of device: major class (bits 8 to 12), minor class (bits 2 to 7) and service classes (bits 13 to 23) as text.
//! Standard: Bluetooth Assigned Numbers, clause 2.8 (Class of Device).

/// The major class of `class`.
pub fn major(class: u32) -> u8 {
    (class >> 8 & 0x1F) as u8
}

/// The minor class field of `class`.
pub fn minor(class: u32) -> u8 {
    (class >> 2 & 0x3F) as u8
}

/// The name of major class `m`.
pub fn major_name(m: u8) -> &'static str {
    match m {
        0 => "Miscellaneous",
        1 => "Computer",
        2 => "Phone",
        3 => "LAN / network access point",
        4 => "Audio / video",
        5 => "Peripheral",
        6 => "Imaging",
        7 => "Wearable",
        8 => "Toy",
        9 => "Health",
        31 => "Uncategorised",
        _ => "Reserved",
    }
}

/// The name of minor class `n` of major class `m`, where the class lists it.
pub fn minor_name(m: u8, n: u8) -> Option<&'static str> {
    const COMPUTER: [&str; 8] = [
        "uncategorised",
        "desktop",
        "server",
        "laptop",
        "handheld PC",
        "palm-size PC",
        "wearable computer",
        "tablet",
    ];
    const PHONE: [&str; 6] = [
        "uncategorised",
        "cellular",
        "cordless",
        "smartphone",
        "wired modem or voice gateway",
        "common ISDN access",
    ];
    const AV: [&str; 19] = [
        "uncategorised",
        "wearable headset",
        "hands-free",
        "reserved",
        "microphone",
        "loudspeaker",
        "headphones",
        "portable audio",
        "car audio",
        "set-top box",
        "HiFi audio",
        "VCR",
        "video camera",
        "camcorder",
        "video monitor",
        "video display and loudspeaker",
        "video conferencing",
        "reserved",
        "gaming or toy",
    ];
    const WEARABLE: [&str; 6] = ["", "wristwatch", "pager", "jacket", "helmet", "glasses"];
    let n = usize::from(n);
    let s = match m {
        1 => COMPUTER.get(n).copied(),
        2 => PHONE.get(n).copied(),
        4 => AV.get(n).copied(),
        5 => match n >> 4 {
            1 => Some("keyboard"),
            2 => Some("pointing device"),
            3 => Some("keyboard and pointing device"),
            _ => None,
        },
        7 => WEARABLE.get(n).copied(),
        _ => None,
    };
    s.filter(|s| !s.is_empty())
}

/// The service classes of `class` by bit, in bit order. Bits 14 and 15 are not shown.
const SERVICES: [(u32, &str); 9] = [
    (13, "Limited discoverable"),
    (16, "Positioning"),
    (17, "Networking"),
    (18, "Rendering"),
    (19, "Capturing"),
    (20, "Object transfer"),
    (21, "Audio"),
    (22, "Telephony"),
    (23, "Information"),
];

/// The class of device as text: "Phone: smartphone (Object transfer, Telephony)".
///
/// ```
/// use sdrkit_proto_bt::cod::text;
/// let class = 1 << 20 | 1 << 22 | 2 << 8 | 3 << 2 | 1;
/// assert_eq!(text(class), "Phone: smartphone (Object transfer, Telephony)");
/// ```
pub fn text(class: u32) -> String {
    let m = major(class);
    let mut s = major_name(m).to_string();
    if let Some(n) = minor_name(m, minor(class)) {
        s.push_str(": ");
        s.push_str(n);
    }
    let services: Vec<&str> = SERVICES
        .iter()
        .filter(|(b, _)| class >> b & 1 == 1)
        .map(|&(_, n)| n)
        .collect();
    if !services.is_empty() {
        s.push_str(" (");
        s.push_str(&services.join(", "));
        s.push(')');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_as_text() {
        assert_eq!(
            text(1 << 21 | 4 << 8 | 1 << 2 | 1),
            "Audio / video: wearable headset (Audio)"
        );
        assert_eq!(text(5 << 8 | 0x10 << 2), "Peripheral: keyboard");
        assert_eq!(text(31 << 8), "Uncategorised");
        assert_eq!(text(12 << 8 | 1 << 13), "Reserved (Limited discoverable)");
        assert_eq!(major_name(major(9 << 8)), "Health");
    }
}
