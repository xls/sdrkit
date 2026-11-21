//! The iiod context XML: devices, channels with their directions and scan elements, and context attributes.

use std::collections::BTreeMap;

/// A channel of a device.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    /// The id (`voltage0`, `altvoltage0`).
    pub id: String,
    /// The optional name (`RX_LO`).
    pub name: String,
    /// An output channel (false: input).
    pub output: bool,
    /// The scan index and format (`le:S12/16>>0`) of a stream channel.
    pub scan: Option<(u32, String)>,
    /// The channel's attribute names.
    pub attrs: Vec<String>,
}

/// A device of the context.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Device {
    /// The id (`iio:device1`).
    pub id: String,
    /// The name (`ad9361-phy`).
    pub name: String,
    /// Its channels.
    pub channels: Vec<Channel>,
    /// Its device attribute names, in the order of the XML (the binary protocol addresses them by position).
    pub attrs: Vec<String>,
    /// Its debug attribute names.
    pub debug_attrs: Vec<String>,
    /// Its buffer attribute names.
    pub buffer_attrs: Vec<String>,
}

impl Device {
    /// A channel by id and direction.
    pub fn channel(&self, id: &str, output: bool) -> Option<&Channel> {
        self.channels
            .iter()
            .find(|c| c.id == id && c.output == output)
    }

    /// The position of a channel in the device's list: the bit of the channel in a buffer mask.
    pub fn channel_index(&self, id: &str, output: bool) -> Option<usize> {
        self.channels
            .iter()
            .position(|c| c.id == id && c.output == output)
    }

    /// The device has this device attribute.
    pub fn has_attr(&self, name: &str) -> bool {
        self.attrs.iter().any(|a| a == name)
    }
}

impl Channel {
    /// The channel has this attribute.
    pub fn has(&self, attr: &str) -> bool {
        self.attrs.iter().any(|a| a == attr)
    }

    /// The parsed scan format of a stream channel.
    pub fn format(&self) -> Option<DataFormat> {
        self.scan.as_ref().and_then(|s| DataFormat::parse(&s.1))
    }
}

/// A stream channel's sample format, from a scan element's `format` (`le:S12/16>>0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataFormat {
    /// Big endian.
    pub big_endian: bool,
    /// Signed.
    pub signed: bool,
    /// Meaningful bits.
    pub bits: u32,
    /// Storage bits (8, 16, 32, 64).
    pub storage: u32,
    /// Right shift of the data in its storage.
    pub shift: u32,
    /// Values per sample (the `X<n>` repeat), usually 1.
    pub repeat: u32,
}

impl DataFormat {
    /// Parses `[lb]e:[sSuU]<bits>/<storage>[X<repeat>]>><shift>`.
    pub fn parse(s: &str) -> Option<DataFormat> {
        let (endian, rest) = s.split_once(':')?;
        let big_endian = match endian {
            "be" => true,
            "le" => false,
            _ => return None,
        };
        let mut chars = rest.chars();
        let signed = match chars.next()? {
            's' | 'S' => true,
            'u' | 'U' => false,
            _ => return None,
        };
        let rest = chars.as_str();
        let (bits, rest) = rest.split_once('/')?;
        let (storage, shift) = rest.split_once(">>")?;
        let (storage, repeat) = match storage.split_once('X') {
            Some((s, r)) => (s, r.parse().ok()?),
            None => (storage, 1),
        };
        let f = DataFormat {
            big_endian,
            signed,
            bits: bits.parse().ok()?,
            storage: storage.parse().ok()?,
            shift: shift.parse().ok()?,
            repeat,
        };
        (f.storage.is_multiple_of(8) && f.storage > 0 && f.bits <= f.storage).then_some(f)
    }

    /// Bytes per sample of this channel.
    pub fn bytes(&self) -> usize {
        (self.storage / 8 * self.repeat.max(1)) as usize
    }
}

/// What the context XML says.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    /// The devices.
    pub devices: Vec<Device>,
    /// Context attributes (`fw_version`, `hw_model`).
    pub attrs: BTreeMap<String, String>,
}

impl Context {
    /// Parses the XML that `PRINT` returns.
    ///
    /// # Errors
    ///
    /// [`roxmltree::Error`] if the text is not XML.
    pub fn parse(xml: &str) -> Result<Context, roxmltree::Error> {
        let doc = roxmltree::Document::parse_with_options(
            xml,
            roxmltree::ParsingOptions {
                allow_dtd: true,
                ..Default::default()
            },
        )?;
        let root = doc.root_element();
        let mut ctx = Context::default();
        for n in root.children().filter(|n| n.is_element()) {
            match n.tag_name().name() {
                "context-attribute" => {
                    if let (Some(k), Some(v)) = (n.attribute("name"), n.attribute("value")) {
                        ctx.attrs.insert(k.into(), v.into());
                    }
                }
                "device" => {
                    let mut d = Device {
                        id: n.attribute("id").unwrap_or("").into(),
                        name: n.attribute("name").unwrap_or("").into(),
                        channels: Vec::new(),
                        attrs: Vec::new(),
                        debug_attrs: Vec::new(),
                        buffer_attrs: Vec::new(),
                    };
                    for c in n.children().filter(|c| c.is_element()) {
                        match c.tag_name().name() {
                            "channel" => {
                                let mut ch = Channel {
                                    id: c.attribute("id").unwrap_or("").into(),
                                    name: c.attribute("name").unwrap_or("").into(),
                                    output: c.attribute("type") == Some("output"),
                                    scan: None,
                                    attrs: Vec::new(),
                                };
                                for a in c.children().filter(|a| a.is_element()) {
                                    match a.tag_name().name() {
                                        "scan-element" => {
                                            let idx =
                                                a.attribute("index").and_then(|i| i.parse().ok());
                                            if let Some(i) = idx {
                                                ch.scan = Some((
                                                    i,
                                                    a.attribute("format").unwrap_or("").into(),
                                                ));
                                            }
                                        }
                                        "attribute" => {
                                            ch.attrs.extend(a.attribute("name").map(String::from))
                                        }
                                        _ => {}
                                    }
                                }
                                d.channels.push(ch);
                            }
                            "attribute" => d.attrs.extend(c.attribute("name").map(String::from)),
                            "debug-attribute" => {
                                d.debug_attrs.extend(c.attribute("name").map(String::from))
                            }
                            "buffer-attribute" => {
                                d.buffer_attrs.extend(c.attribute("name").map(String::from))
                            }
                            _ => {}
                        }
                    }
                    ctx.devices.push(d);
                }
                _ => {}
            }
        }
        Ok(ctx)
    }

    /// The first device with this name.
    pub fn device(&self, name: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.name == name)
    }

    /// The position of a device (by id) in the context: its number in the binary protocol.
    pub fn device_index(&self, id: &str) -> Option<usize> {
        self.devices.iter().position(|d| d.id == id)
    }
}

/// Reads a range attribute in the form `[first step last]` as (first, step, last).
pub fn parse_range(s: &str) -> Option<(f64, f64, f64)> {
    let v: Vec<f64> = s
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split_whitespace()
        .filter_map(|x| x.parse().ok())
        .collect();
    match v[..] {
        [a, b, c] => Some((a, b, c)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(
            parse_range("[70000000 1 6000000000]"),
            Some((70e6, 1.0, 6e9))
        );
        assert_eq!(parse_range("[-1 1 73]"), Some((-1.0, 1.0, 73.0)));
        assert_eq!(parse_range("nonsense"), None);
    }

    #[test]
    fn a_context_with_a_dtd_parses() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><!DOCTYPE context [<!ELEMENT context (device | context-attribute)*><!ATTLIST context name CDATA #REQUIRED>]><context name="network"><context-attribute name="fw_version" value="v0.39" /><device id="iio:device4" name="cf-ad9361-lpc"><channel id="voltage0" type="input"><scan-element index="0" format="le:S12/16&gt;&gt;0" scale="1.000000" /><attribute name="calibscale" filename="in_voltage0_calibscale" /></channel><attribute name="sync_start_enable" /></device></context>"#;
        let c = Context::parse(xml).unwrap();
        assert_eq!(c.attrs["fw_version"], "v0.39");
        let d = c.device("cf-ad9361-lpc").unwrap();
        assert_eq!(d.id, "iio:device4");
        assert_eq!(
            d.channel("voltage0", false).unwrap().scan,
            Some((0, "le:S12/16>>0".into()))
        );
        assert!(d.channel("voltage0", true).is_none());
        assert_eq!(d.attrs, ["sync_start_enable"]);
        assert_eq!(c.device_index("iio:device4"), Some(0));
        assert_eq!(d.channel_index("voltage0", false), Some(0));
    }

    #[test]
    fn scan_formats() {
        let f = DataFormat::parse("le:S12/16>>0").unwrap();
        assert_eq!(
            (
                f.big_endian,
                f.signed,
                f.bits,
                f.storage,
                f.shift,
                f.bytes()
            ),
            (false, true, 12, 16, 0, 2)
        );
        let f = DataFormat::parse("be:u24/32X2>>8").unwrap();
        assert_eq!(
            (
                f.big_endian,
                f.signed,
                f.bits,
                f.storage,
                f.repeat,
                f.shift,
                f.bytes()
            ),
            (true, false, 24, 32, 2, 8, 8)
        );
        assert_eq!(DataFormat::parse("xx:S12/16>>0"), None);
        assert_eq!(DataFormat::parse("le:S20/16>>0"), None);
    }
}
