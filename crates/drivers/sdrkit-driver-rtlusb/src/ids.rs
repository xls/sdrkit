//! The USB ids of RTL2832U dongles and the layout of their EEPROM (specification sections 1.1 and 1.5).

/// The vendor and product ids of the dongles the driver opens.
pub const DEVICES: [(u16, u16); 42] = [
    (0x0bda, 0x2832),
    (0x0bda, 0x2838),
    (0x0413, 0x6680),
    (0x0413, 0x6f0f),
    (0x0458, 0x707f),
    (0x0ccd, 0x00a9),
    (0x0ccd, 0x00b3),
    (0x0ccd, 0x00b4),
    (0x0ccd, 0x00b5),
    (0x0ccd, 0x00b7),
    (0x0ccd, 0x00b8),
    (0x0ccd, 0x00b9),
    (0x0ccd, 0x00c0),
    (0x0ccd, 0x00c6),
    (0x0ccd, 0x00d3),
    (0x0ccd, 0x00d7),
    (0x0ccd, 0x00e0),
    (0x1554, 0x5020),
    (0x15f4, 0x0131),
    (0x15f4, 0x0133),
    (0x185b, 0x0620),
    (0x185b, 0x0650),
    (0x185b, 0x0680),
    (0x1b80, 0xd393),
    (0x1b80, 0xd394),
    (0x1b80, 0xd395),
    (0x1b80, 0xd397),
    (0x1b80, 0xd398),
    (0x1b80, 0xd39d),
    (0x1b80, 0xd3a4),
    (0x1b80, 0xd3a8),
    (0x1b80, 0xd3af),
    (0x1b80, 0xd3b0),
    (0x1d19, 0x1101),
    (0x1d19, 0x1102),
    (0x1d19, 0x1103),
    (0x1d19, 0x1104),
    (0x1f4d, 0xa803),
    (0x1f4d, 0xb803),
    (0x1f4d, 0xc803),
    (0x1f4d, 0xd286),
    (0x1f4d, 0xd803),
];

/// True for the ids of an RTL2832U dongle.
pub fn is_rtl(vendor_id: u16, product_id: u16) -> bool {
    DEVICES.contains(&(vendor_id, product_id))
}

/// The 8-bit I2C address of the EEPROM.
pub const EEPROM_ADDR: u8 = 0xA0;
/// The size of the EEPROM in bytes.
pub const EEPROM_LEN: usize = 256;
/// The offset at which the IR configuration starts; the strings end before it.
const IR_OFFSET: usize = 78;

/// The fields of an EEPROM image.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Eeprom {
    /// The USB vendor id.
    pub vendor_id: u16,
    /// The USB product id.
    pub product_id: u16,
    /// The image says a serial number string is present.
    pub has_serial: bool,
    /// Configuration bit 0: remote wakeup.
    pub remote_wakeup: bool,
    /// Configuration bit 1: the IR endpoint.
    pub ir_endpoint: bool,
    /// The manufacturer string.
    pub manufacturer: String,
    /// The product string.
    pub product: String,
    /// The serial number string.
    pub serial: String,
}

/// Reads the fields of an EEPROM image: `None` without the signature 0x28 0x32 or with strings that do not end before
/// offset 78.
pub fn parse_eeprom(image: &[u8]) -> Option<Eeprom> {
    if image.len() < IR_OFFSET || image[0] != 0x28 || image[1] != 0x32 {
        return None;
    }
    let mut at = 9;
    let mut strings = [String::new(), String::new(), String::new()];
    for s in &mut strings {
        let len = usize::from(*image.get(at)?);
        if len < 2 || image.get(at + 1) != Some(&0x03) || at + len > IR_OFFSET {
            return None;
        }
        let units: Vec<u16> = image[at + 2..at + len]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        *s = String::from_utf16_lossy(&units);
        at += len;
    }
    let [manufacturer, product, serial] = strings;
    Some(Eeprom {
        vendor_id: u16::from_le_bytes([image[2], image[3]]),
        product_id: u16::from_le_bytes([image[4], image[5]]),
        has_serial: image[6] == 0xA5,
        remote_wakeup: image[7] & 0x01 != 0,
        ir_endpoint: image[7] & 0x02 != 0,
        manufacturer,
        product,
        serial,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(s: &str) -> Vec<u8> {
        let mut v = vec![(2 + 2 * s.len()) as u8, 0x03];
        for c in s.encode_utf16() {
            v.extend_from_slice(&c.to_le_bytes());
        }
        v
    }

    #[test]
    fn the_oem_image_reads_back() {
        let mut image = vec![0x28, 0x32, 0xda, 0x0b, 0x38, 0x28, 0xa5, 0x16, 0x02];
        image.extend(descriptor("Realtek"));
        image.extend(descriptor("RTL2838UHIDIR"));
        image.extend(descriptor("00000001"));
        image.resize(EEPROM_LEN, 0xff);
        image[78] = 0;
        let e = parse_eeprom(&image).unwrap();
        assert_eq!((e.vendor_id, e.product_id), (0x0bda, 0x2838));
        assert!(e.has_serial && !e.remote_wakeup && e.ir_endpoint);
        assert_eq!(e.manufacturer, "Realtek");
        assert_eq!(e.product, "RTL2838UHIDIR");
        assert_eq!(e.serial, "00000001");
        image[1] = 0;
        assert_eq!(parse_eeprom(&image), None);
    }

    #[test]
    fn the_table_has_the_oem_ids() {
        assert!(is_rtl(0x0bda, 0x2838));
        assert!(is_rtl(0x0bda, 0x2832));
        assert!(!is_rtl(0x1d50, 0x6089));
    }
}
