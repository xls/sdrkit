//! The saved state of strip pictures: JSON with each channel as a base64 PNG of the rows held, the rows that arrived as a
//! base64 bit set, and the parts that arrived for each strip.

use crate::strip::StripPicture;
use crate::{PixelFormat, Raster};
use sdrkit_core::base64::{decode_into, encode_into, Alphabet};
use sdrkit_core::{StateBlob, StateError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The owner tag of the saved state.
const OWNER: &str = "image.pictures";
/// The version of the saved state. A state of another version is refused.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct SavedPlane {
    channel: u16,
    png: String,
    rows: String,
    masks: Vec<(u32, u64)>,
}

#[derive(Serialize, Deserialize)]
struct SavedPicture {
    instrument: String,
    id: u32,
    width: u32,
    max_rows: u32,
    strips: u64,
    planes: Vec<SavedPlane>,
}

fn b64(data: &[u8]) -> String {
    let mut s = String::new();
    encode_into(data, Alphabet::Standard, true, &mut s);
    s
}

fn unb64(text: &str) -> Result<Vec<u8>, StateError> {
    let mut v = Vec::new();
    decode_into(text, Alphabet::Standard, &mut v).map_err(|_| StateError::Damaged)?;
    Ok(v)
}

fn save_plane(width: u32, channel: u16, pl: &crate::strip::Plane) -> Option<SavedPlane> {
    let raster = Raster::from_vec(PixelFormat::Grey8, width, pl.len(), pl.data().to_vec()).ok()?;
    let png = crate::png::encode(&raster).ok()?;
    let mut bits = vec![0u8; pl.present().len().div_ceil(8)];
    for (y, &b) in pl.present().iter().enumerate() {
        if b {
            bits[y / 8] |= 0x80 >> (y % 8);
        }
    }
    Some(SavedPlane {
        channel,
        png: b64(&png),
        rows: b64(&bits),
        masks: pl.masks.iter().map(|(&r, &m)| (r, m)).collect(),
    })
}

/// The state of `pictures`, or `None` if it cannot be written.
pub(crate) fn save(pictures: &BTreeMap<&'static str, StripPicture>) -> Option<StateBlob> {
    let list: Vec<SavedPicture> = pictures
        .iter()
        .map(|(name, p)| SavedPicture {
            instrument: (*name).to_string(),
            id: p.id,
            width: p.width,
            max_rows: p.max_rows,
            strips: p.strips,
            planes: p
                .planes
                .iter()
                .filter_map(|(&ch, pl)| save_plane(p.width, ch, pl))
                .collect(),
        })
        .collect();
    Some(StateBlob::new(
        OWNER,
        VERSION,
        serde_json::to_vec(&list).ok()?,
    ))
}

/// The pictures of a state that [`save`] made, by instrument name.
///
/// # Errors
///
/// [`StateError`] for a state of another owner or version, or a damaged one.
pub(crate) fn load(state: &StateBlob) -> Result<Vec<(String, StripPicture)>, StateError> {
    let bytes = state.open(OWNER, VERSION)?;
    let list: Vec<SavedPicture> = serde_json::from_slice(bytes).map_err(|_| StateError::Damaged)?;
    let mut out = Vec::with_capacity(list.len());
    for p in list {
        let mut pic = StripPicture::new(p.id, p.width, p.max_rows);
        for pl in p.planes {
            let raster = crate::png::decode(&unb64(&pl.png)?).map_err(|_| StateError::Damaged)?;
            if raster.format() != PixelFormat::Grey8 || raster.width() != p.width {
                return Err(StateError::Damaged);
            }
            let bits = unb64(&pl.rows)?;
            let h = raster.height() as usize;
            if bits.len() < h.div_ceil(8) {
                return Err(StateError::Damaged);
            }
            let present = (0..h)
                .map(|y| bits[y / 8] & (0x80 >> (y % 8)) != 0)
                .collect();
            let masks = pl.masks.into_iter().collect();
            pic.insert_plane(pl.channel, raster.data().to_vec(), present, masks)
                .map_err(|_| StateError::Damaged)?;
        }
        pic.strips = p.strips;
        out.push((p.instrument, pic));
    }
    Ok(out)
}
