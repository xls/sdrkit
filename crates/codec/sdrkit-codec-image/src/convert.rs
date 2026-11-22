//! The picture adapters that the graph inserts: `image.decode` (a compressed picture to raw pixels) and `image.pixels` (raw
//! pixels to another packed layout).

use crate::raster::bytes_per_pixel;
use crate::{decode, Raster, DECODES};
use sdrkit_core::media::{Codec, MediaFormat, Packaging, Picture, PixelFormat};
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{Choice, ConfigError, Results, Setting, SettingError, SettingId, SettingSet};
use sdrkit_core::{Value, Values};
use sdrkit_graph::{
    no_values, values_of, Adapter, AdapterClass, Applied, Component, ComponentInfo, Format,
    FormatConstraint, Io, Kind, Port, RateFrom, Registry, Timed,
};

sdrkit_core::choice_enum! {
    /// A pixel format that a [`Raster`] holds.
    pub enum PackedPixels {
        /// 8-bit grey.
        Grey8 = ("grey8", "8-bit grey"),
        /// BGRA, 8 bits each.
        Bgra = ("bgra", "BGRA"),
    }
}

impl PackedPixels {
    /// The pixel format.
    pub const fn format(self) -> PixelFormat {
        match self {
            PackedPixels::Grey8 => PixelFormat::Grey8,
            PackedPixels::Bgra => PixelFormat::Bgra,
        }
    }

    /// The choice of a packed pixel format, `None` for a format that a [`Raster`] does not hold.
    pub const fn of(format: PixelFormat) -> Option<PackedPixels> {
        match format {
            PixelFormat::Grey8 => Some(PackedPixels::Grey8),
            PixelFormat::Bgra => Some(PackedPixels::Bgra),
            PixelFormat::Nv12
            | PixelFormat::Nv12Full
            | PixelFormat::I420
            | PixelFormat::Yuy2
            | PixelFormat::Rgb8 => None,
        }
    }
}

/// The format of a picture stream, with the codec of its pictures when every picture has the same one.
fn stream_format(media: Option<Codec>) -> Format {
    let mut f = Format::new(Kind::typed::<Picture>(), 0.0);
    f.media = media.map(|c| MediaFormat::new(c, Packaging::Raw));
    f
}

/// The pixel codecs that an input constraint accepts, in its order.
fn wanted_pixels(want: &FormatConstraint) -> impl Iterator<Item = PixelFormat> + '_ {
    want.media.iter().filter_map(|m| match m.codec {
        Codec::Pixels(p) => Some(p),
        _ => None,
    })
}

/// `image.decode`: decodes each compressed picture ([`DECODES`]) to raw pixels with [`decode`]. A raw picture passes as it
/// is. A picture that does not decode is dropped and counted in `frames_bad`.
#[derive(Debug)]
pub struct Decode {
    bad: u64,
    published: Published,
}

impl Default for Decode {
    fn default() -> Self {
        Decode {
            bad: 0,
            published: Published::new(&[&keys::FRAMES_BAD]),
        }
    }
}

impl Component for Decode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "image.decode",
            vec![Port::typed::<Picture>("in")],
            vec![Port::typed::<Picture>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![stream_format(None)])
    }

    fn process(&mut self, io: &mut Io) {
        let before = self.bad;
        for i in 0..io.items::<Picture>(0).len() {
            let it = &io.items::<Picture>(0)[i];
            let (time, p) = (it.time, &it.value);
            let out = match p.codec {
                Codec::Pixels(_) => Some(p.clone()),
                codec => decode(codec, &p.data)
                    .ok()
                    .map(|r| r.to_picture(p.name.as_deref().unwrap_or(""), p.id, p.state)),
            };
            match out {
                Some(value) => io.items_out::<Picture>(0).push(Timed { time, value }),
                None => self.bad += 1,
            }
        }
        if self.bad != before {
            self.published.set(&keys::FRAMES_BAD, self.bad as i64);
        }
    }

    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

const TO: Setting<field::Choice<PackedPixels>> =
    Setting::choice("to", "Pixel format", PackedPixels::Bgra);

sdrkit_core::settings! {
    /// The settings of [`PixelConvert`].
    pub enum PixelConvertSetting {
        /// The pixel format it returns.
        To = TO,
    }
}

/// `image.pixels`: converts each raw picture to the packed pixel format `to` ([`Raster::convert`]). A picture already in
/// that format passes and shares its bytes. A compressed picture, or one that does not convert, is dropped and counted in
/// `frames_bad`.
#[derive(Debug)]
pub struct PixelConvert {
    to: PackedPixels,
    bad: u64,
    published: Published,
}

impl Default for PixelConvert {
    fn default() -> Self {
        PixelConvert {
            to: TO.default,
            bad: 0,
            published: Published::new(&[&keys::FRAMES_BAD]),
        }
    }
}

impl PixelConvert {
    /// A converter to `to`.
    pub fn new(to: PackedPixels) -> Self {
        PixelConvert {
            to,
            ..Self::default()
        }
    }
}

impl Component for PixelConvert {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "image.pixels",
            vec![Port::typed::<Picture>("in")],
            vec![Port::typed::<Picture>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(PixelConvertSetting::schema())
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![stream_format(Some(Codec::Pixels(self.to.format())))])
    }

    fn process(&mut self, io: &mut Io) {
        let before = self.bad;
        let to = self.to.format();
        for i in 0..io.items::<Picture>(0).len() {
            let it = &io.items::<Picture>(0)[i];
            let (time, p) = (it.time, &it.value);
            let out = if p.codec == Codec::Pixels(to) {
                Some(p.clone())
            } else {
                Raster::from_picture(p)
                    .and_then(|r| r.convert(to))
                    .ok()
                    .map(|r| r.to_picture(p.name.as_deref().unwrap_or(""), p.id, p.state))
            };
            match out {
                Some(value) => io.items_out::<Picture>(0).push(Timed { time, value }),
                None => self.bad += 1,
            }
        }
        if self.bad != before {
            self.published.set(&keys::FRAMES_BAD, self.bad as i64);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PixelConvertSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PixelConvertSetting::To => self.to = TO.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([(PixelConvertSetting::To, Value::Choice(self.to.index()))])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

/// The settings of `image.decode` when the stream carries compressed pictures and the input requires raw pixels.
fn decode_step(input: &Format, want: &FormatConstraint) -> Option<Values> {
    let codec = input.media.as_ref()?.codec;
    (DECODES.contains(&codec) && wanted_pixels(want).next().is_some()).then(no_values)
}

/// The settings of `image.pixels` for a stream of raw pixels in a layout that the input does not want, when the change adds
/// colour channels (`lossless` true) or drops them (`lossless` false).
fn pixel_step(input: &Format, want: &FormatConstraint, lossless: bool) -> Option<Values> {
    let Codec::Pixels(from) = input.media.as_ref()?.codec else {
        return None;
    };
    bytes_per_pixel(from)?;
    if wanted_pixels(want).any(|p| p == from) {
        return None;
    }
    let to = wanted_pixels(want).find_map(PackedPixels::of)?;
    if (from == PixelFormat::Grey8) != lossless {
        return None;
    }
    Some(values_of([(
        PixelConvertSetting::To,
        Value::Choice(to.index()),
    )]))
}

/// Adds `image.decode` and `image.pixels` to `r`, as components and as adapters of picture streams.
pub(crate) fn register(r: &mut Registry) {
    let pictures = Kind::typed::<Picture>();
    r.add_adapter(Adapter {
        kind: "image.decode",
        from: pictures,
        to: pictures,
        class: AdapterClass::Lossless,
        cost: 8,
        settings: decode_step,
        make: || Box::new(Decode::default()),
    });
    r.add_adapter(Adapter {
        kind: "image.pixels",
        from: pictures,
        to: pictures,
        class: AdapterClass::Lossless,
        cost: 2,
        settings: |f, want| pixel_step(f, want, true),
        make: || Box::new(PixelConvert::default()),
    });
    r.add_adapter(Adapter {
        kind: "image.pixels",
        from: pictures,
        to: pictures,
        class: AdapterClass::DeclaredLossy,
        cost: 2,
        settings: |f, want| pixel_step(f, want, false),
        make: || Box::new(PixelConvert::default()),
    });
}
