//! Saved state: what a receiver gathered, as an opaque blob that the host keeps and returns.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;

/// An opaque, versioned blob of saved state: the owner's tag, the owner's format version and the bytes. The host stores it and
/// never reads the bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateBlob {
    owner: Cow<'static, str>,
    version: u32,
    #[serde(with = "bytes")]
    bytes: Arc<[u8]>,
}

impl StateBlob {
    /// A blob of `owner` (a stack id) in format `version`.
    pub fn new(
        owner: impl Into<Cow<'static, str>>,
        version: u32,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Self {
        StateBlob {
            owner: owner.into(),
            version,
            bytes: bytes.into(),
        }
    }

    /// The owner's tag.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The format version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// The bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The bytes, if the blob is of `owner` in `version`.
    ///
    /// # Errors
    ///
    /// [`StateError::WrongOwner`] or [`StateError::WrongVersion`].
    pub fn open(&self, owner: &str, version: u32) -> Result<&[u8], StateError> {
        if self.owner != owner {
            return Err(StateError::WrongOwner {
                found: self.owner.to_string(),
            });
        }
        if self.version != version {
            return Err(StateError::WrongVersion {
                expected: version,
                found: self.version,
            });
        }
        Ok(&self.bytes)
    }
}

/// Why saved state was not restored. Nothing changed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StateError {
    /// The receiver keeps no state.
    #[error("this receiver keeps no state")]
    NoState,
    /// The blob belongs to another owner.
    #[error("the state belongs to {found}")]
    WrongOwner {
        /// The owner of the blob.
        found: String,
    },
    /// The blob is of another format version.
    #[error("the state is version {found}, version {expected} is needed")]
    WrongVersion {
        /// The version the receiver reads.
        expected: u32,
        /// The version of the blob.
        found: u32,
    },
    /// The bytes do not decode.
    #[error("the state is damaged")]
    Damaged,
}

mod bytes {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub fn serialize<S: Serializer>(b: &Arc<[u8]>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(b)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Arc<[u8]>, D::Error> {
        Vec::<u8>::deserialize(d).map(Arc::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blob_opens_only_for_its_owner_and_version() {
        let b = StateBlob::new("adsb", 2, vec![1u8, 2, 3]);
        assert_eq!(b.open("adsb", 2), Ok(&[1u8, 2, 3][..]));
        assert_eq!(
            b.open("ais", 2),
            Err(StateError::WrongOwner {
                found: "adsb".into()
            })
        );
        assert_eq!(
            b.open("adsb", 3),
            Err(StateError::WrongVersion {
                expected: 3,
                found: 2
            })
        );
    }
}
