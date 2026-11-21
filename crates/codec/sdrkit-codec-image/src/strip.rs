//! Strip pictures: a picture that a satellite decoder sends in strips of a few rows of one channel. A [`StripPicture`]
//! gathers the strips of one instrument, and [`render`] draws a [`View`] of it with an [`Enhance`] choice. Rows that never
//! arrive stay 0 (black).

use crate::lut::{Lut, Palette};
use crate::{ImageError, PixelFormat, Raster};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Some rows of one channel of a picture: the `picture.strip` graph item.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    /// The picture it belongs to. A new picture of the same instrument has a new number.
    pub picture: u32,
    /// The instrument channel, 1-based as the instrument documents name them.
    pub channel: u16,
    /// The first row (picture lines from the top).
    pub first_row: u32,
    /// The first column.
    pub x: u32,
    /// The number of rows.
    pub rows: u16,
    /// The width in pixels.
    pub width: u32,
    /// The 8-bit samples, row by row (`L008`). It holds `rows` x `width` bytes.
    pub samples: Arc<[u8]>,
    /// Which parts of the row arrived: bit k is the k-th part.
    pub mask: u64,
    /// The time on board, in seconds of the day, when known.
    pub onboard_s: Option<f64>,
    /// The instrument, for a source with more than one. It is empty for a source with one.
    pub instrument: &'static str,
}

impl sdrkit_graph::Item for Strip {
    const KIND: &'static str = "picture.strip";
}

impl Strip {
    /// A strip of `rows` x `width` samples of `channel` at (`x`, `first_row`), as part 0 of its row, of picture 0.
    pub fn new(
        channel: u16,
        first_row: u32,
        x: u32,
        rows: u16,
        width: u32,
        samples: Arc<[u8]>,
    ) -> Strip {
        Strip {
            picture: 0,
            channel,
            first_row,
            x,
            rows,
            width,
            samples,
            mask: 1,
            onboard_s: None,
            instrument: "",
        }
    }
}

/// One channel of a [`StripPicture`]: dense rows, with a flag for each row that arrived.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plane {
    data: Vec<u8>,
    present: Vec<bool>,
    /// The parts that arrived for each strip, keyed by the strip's first row.
    pub masks: BTreeMap<u32, u64>,
}

impl Plane {
    /// The samples of all rows held, row by row. Rows that did not arrive are 0.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    /// The rows held (the highest row that arrived, plus 1).
    pub fn len(&self) -> u32 {
        self.present.len() as u32
    }
    /// True when no row arrived.
    pub fn is_empty(&self) -> bool {
        self.present.is_empty()
    }
    /// True when row `y` arrived.
    pub fn has_row(&self, y: u32) -> bool {
        self.present.get(y as usize).copied().unwrap_or(false)
    }
    /// The flags of the rows that arrived, one for each row held.
    pub fn present(&self) -> &[bool] {
        &self.present
    }
}

/// The picture of one instrument, gathered from strips.
#[derive(Clone, Debug, PartialEq)]
pub struct StripPicture {
    /// Its number.
    pub id: u32,
    /// The width of every channel.
    pub width: u32,
    /// The most rows kept.
    pub max_rows: u32,
    /// The channels that arrived.
    pub planes: BTreeMap<u16, Plane>,
    /// The rows: the highest row that arrived in any channel, plus 1.
    pub height: u32,
    /// A number that grows with every change.
    pub version: u64,
    /// The strips added.
    pub strips: u64,
}

impl StripPicture {
    /// An empty picture numbered `id`, of `width` pixels and at most `max_rows` rows.
    pub fn new(id: u32, width: u32, max_rows: u32) -> StripPicture {
        StripPicture {
            id,
            width,
            max_rows,
            planes: BTreeMap::new(),
            height: 0,
            version: 0,
            strips: 0,
        }
    }

    /// Adds a strip. It returns `Ok(false)` when the strip starts at or below the row limit. If the strip holds a number of
    /// samples other than `rows` x `width`, it returns an error and the picture does not change. Columns beyond the picture
    /// width are cut.
    pub fn add(&mut self, s: &Strip) -> Result<bool, ImageError> {
        let need = s.rows as usize * s.width as usize;
        if s.samples.len() != need {
            return Err(ImageError::WrongLength {
                expected: need,
                got: s.samples.len(),
            });
        }
        if s.first_row >= self.max_rows || s.rows == 0 {
            return Ok(false);
        }
        let w = self.width as usize;
        let end = (s.first_row + s.rows as u32).min(self.max_rows);
        let plane = self.planes.entry(s.channel).or_default();
        if plane.present.len() < end as usize {
            plane.present.resize(end as usize, false);
            plane.data.resize(end as usize * w, 0);
        }
        let sw = s.width as usize;
        let x0 = (s.x as usize).min(w);
        let n = sw.min(w - x0);
        for (k, y) in (s.first_row..end).enumerate() {
            let y = y as usize;
            plane.data[y * w + x0..y * w + x0 + n].copy_from_slice(&s.samples[k * sw..k * sw + n]);
            plane.present[y] = true;
        }
        *plane.masks.entry(s.first_row).or_default() |= s.mask;
        self.height = self.height.max(end);
        self.version += 1;
        self.strips += 1;
        Ok(true)
    }

    /// The strips of `channel` whose parts all arrived (`full` is the mask of a whole strip).
    pub fn complete_strips(&self, channel: u16, full: u64) -> usize {
        self.planes.get(&channel).map_or(0, |p| {
            p.masks.values().filter(|&&m| m & full == full).count()
        })
    }

    /// The samples of channel `ch` as one buffer of [`StripPicture::height`] rows. Missing rows are 0. It copies when the
    /// channel holds fewer rows than the picture.
    pub fn channel(&self, ch: u16) -> Option<Vec<u8>> {
        let p = self.planes.get(&ch)?;
        let mut out = vec![0u8; self.width as usize * self.height as usize];
        out[..p.data.len()].copy_from_slice(&p.data);
        Some(out)
    }

    /// Makes a plane from saved parts: `data` holds `present.len()` rows of [`StripPicture::width`] samples. It returns an error
    /// when the sizes do not match or the rows pass the row limit.
    pub fn insert_plane(
        &mut self,
        channel: u16,
        data: Vec<u8>,
        present: Vec<bool>,
        masks: BTreeMap<u32, u64>,
    ) -> Result<(), ImageError> {
        let need = present.len() * self.width as usize;
        if data.len() != need {
            return Err(ImageError::WrongLength {
                expected: need,
                got: data.len(),
            });
        }
        if present.len() > self.max_rows as usize {
            return Err(ImageError::Full);
        }
        self.height = self.height.max(present.len() as u32);
        self.planes.insert(
            channel,
            Plane {
                data,
                present,
                masks,
            },
        );
        self.version += 1;
        Ok(())
    }
}

/// What a picture shows: one channel in grey, three channels as red, green and blue, or the first of a list that can be drawn.
#[derive(Clone, Debug, PartialEq)]
pub enum View {
    /// One channel.
    Grey {
        /// The channel.
        channel: u16,
        /// Shown inverted, with 0 kept as "no data" (an infrared channel with cold clouds white).
        invert: bool,
    },
    /// A colour composite: (channel, inverted) for red, green and blue.
    Rgb([(u16, bool); 3]),
    /// The lowest channel that arrived, in grey.
    FirstChannel,
    /// The first view of the list whose channels all arrived. Meteor's automatic colour, for example, is
    /// `FirstOf([Rgb 1 2 3, Rgb 1 2 5, FirstChannel])`.
    FirstOf(Vec<View>),
}

impl View {
    fn drawable(&self, pic: &StripPicture) -> bool {
        let has = |c: &u16| pic.planes.contains_key(c);
        match self {
            View::Grey { channel, .. } => has(channel),
            View::Rgb(c) => c.iter().all(|(ch, _)| has(ch)),
            View::FirstChannel => !pic.planes.is_empty(),
            View::FirstOf(list) => list.iter().any(|v| v.drawable(pic)),
        }
    }

    /// The plain view ([`View::Grey`] or [`View::Rgb`]) to draw with the channels that arrived, or `None` when nothing can be
    /// drawn.
    pub fn resolve(&self, pic: &StripPicture) -> Option<View> {
        match self {
            View::Grey { .. } => self.drawable(pic).then(|| self.clone()),
            // A composite with some channels missing still draws: a missing channel is black.
            View::Rgb(c) => c
                .iter()
                .any(|(ch, _)| pic.planes.contains_key(ch))
                .then(|| self.clone()),
            View::FirstChannel => pic.planes.keys().next().map(|&channel| View::Grey {
                channel,
                invert: false,
            }),
            View::FirstOf(list) => list
                .iter()
                .find(|v| v.drawable(pic))
                .and_then(|v| v.resolve(pic)),
        }
    }

    /// The channels it shows, as file names carry them: `ch5` for a grey view, `123` for a composite.
    pub fn tag(&self) -> String {
        match self {
            View::Grey { channel, .. } => format!("ch{channel}"),
            View::Rgb(c) => c.iter().map(|(ch, _)| ch.to_string()).collect(),
            View::FirstChannel => "first".into(),
            View::FirstOf(_) => "auto".into(),
        }
    }
}

sdrkit_core::choice_enum! {
    /// How a view is enhanced.
    pub enum Enhance {
        /// As received.
        None = ("none", "As received"),
        /// The 2 % and 98 % points of each channel to 1 and 255 ([`Lut::stretch`]).
        Stretch = ("stretch", "Contrast stretch"),
        /// The stretch, then a gamma of 0.7.
        Brighten = ("brighten", "Stretch and brighten"),
        /// The thermal palette ([`Palette::thermal`]) on a grey view. A composite is drawn as received.
        Thermal = ("thermal", "Thermal colours"),
        /// The satellite preset of [`crate::enhance::satellite`].
        Satellite = ("satellite", "Satellite preset"),
    }
}

impl Enhance {
    fn apply(self, p: &mut Vec<u8>, w: usize, h: usize) {
        match self {
            Enhance::Stretch => Lut::stretch(p, None).apply(p),
            Enhance::Brighten => Lut::stretch(p, Some(0.7)).apply(p),
            Enhance::Satellite => {
                if let Ok(out) = crate::enhance::satellite(p, w, h) {
                    *p = out;
                }
            }
            Enhance::None | Enhance::Thermal => {}
        }
    }
}

/// Draws `view` of `pic` with `enhance`: grey ([`PixelFormat::Grey8`]) for a grey view, BGRA for a composite or the thermal
/// palette. It returns `None` when the view cannot be drawn with the channels that arrived.
///
/// ```
/// use sdrkit_codec_image::strip::{render, Enhance, Strip, StripPicture, View};
/// use std::sync::Arc;
///
/// let mut pic = StripPicture::new(0, 16, 100);
/// let strip = Strip::new(1, 0, 0, 8, 16, Arc::from(vec![100u8; 8 * 16]));
/// pic.add(&strip).unwrap();
/// let grey = render(&pic, &View::Grey { channel: 1, invert: false }, Enhance::None).unwrap();
/// assert_eq!(grey.data()[0], 100);
/// ```
pub fn render(pic: &StripPicture, view: &View, enhance: Enhance) -> Option<Raster> {
    let v = view.resolve(pic)?;
    let (w, h) = (pic.width, pic.height.max(1));
    let n = w as usize * h as usize;
    let plane = |c: u16, inv: bool| -> Vec<u8> {
        let mut p = vec![0u8; n];
        if let Some(pl) = pic.planes.get(&c) {
            let len = pl.data.len().min(n);
            p[..len].copy_from_slice(&pl.data[..len]);
        }
        if inv {
            Lut::invert().apply(&mut p);
        }
        p
    };
    match v {
        View::Grey { channel, invert } => {
            let mut p = plane(channel, invert);
            if enhance == Enhance::Thermal {
                let mut r = Raster::new(PixelFormat::Bgra, w, h).ok()?;
                Palette::thermal().to_bgra(&p, r.data_mut());
                return Some(r);
            }
            enhance.apply(&mut p, w as usize, h as usize);
            Raster::from_vec(PixelFormat::Grey8, w, h, p).ok()
        }
        View::Rgb(chs) => {
            let mut planes: [Vec<u8>; 3] = std::array::from_fn(|k| plane(chs[k].0, chs[k].1));
            for p in planes.iter_mut() {
                enhance.apply(p, w as usize, h as usize);
            }
            let mut r = Raster::new(PixelFormat::Bgra, w, h).ok()?;
            for (i, d) in r.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
                *d = [planes[2][i], planes[1][i], planes[0][i], 255];
            }
            Some(r)
        }
        View::FirstChannel | View::FirstOf(_) => None,
    }
}
