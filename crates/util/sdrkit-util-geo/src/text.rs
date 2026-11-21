//! [`Text`]: a string with a fixed capacity, held inline, so the formatters in this crate do not allocate.

use crate::Error;
use std::fmt;

/// A UTF-8 string of at most `N` bytes, stored inline.
///
/// The formatters of this crate (locators, coordinates, ISO 8601 times, NMEA sentences) return it. It can be used wherever a `str` is expected, and
/// [`fmt::Write`] into it fails (rather than truncating) when the text would not fit.
///
/// ```
/// use std::fmt::Write;
/// let mut t = sdrkit_util_geo::Text::<8>::new();
/// write!(t, "{}-{}", 12, 34).unwrap();
/// assert_eq!(t.as_str(), "12-34");
/// assert!(write!(t, "too long").is_err());
/// ```
#[derive(Clone, Copy)]
pub struct Text<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> Text<N> {
    /// The capacity in bytes.
    pub const CAPACITY: usize = N;

    /// An empty text.
    pub const fn new() -> Self {
        Text {
            buf: [0; N],
            len: 0,
        }
    }

    /// A text holding `s`, or [`Error::TooLong`].
    pub fn try_from_str(s: &str) -> Result<Self, Error> {
        let mut t = Self::new();
        t.push_str(s)?;
        Ok(t)
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        // Only whole `str`s and `char`s are ever copied in, so the bytes are always valid UTF-8.
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    /// The text as bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// Its length in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether it is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Empties it.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Appends a string, or returns [`Error::TooLong`] and leaves the text unchanged.
    pub fn push_str(&mut self, s: &str) -> Result<(), Error> {
        let end = self.len + s.len();
        if end > N {
            return Err(Error::TooLong);
        }
        self.buf[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }

    /// Appends a character, or returns [`Error::TooLong`].
    pub fn push(&mut self, c: char) -> Result<(), Error> {
        let mut b = [0u8; 4];
        self.push_str(c.encode_utf8(&mut b))
    }

    /// Shortens it to `len` bytes (no change when it is already shorter). `len` must fall on a character boundary, which it always
    /// does for the ASCII texts this crate makes; otherwise the text is left unchanged.
    pub fn truncate(&mut self, len: usize) {
        if len < self.len && self.as_str().is_char_boundary(len) {
            self.len = len;
        }
    }
}

impl<const N: usize> Default for Text<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> std::ops::Deref for Text<N> {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> AsRef<str> for Text<N> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> fmt::Write for Text<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s).map_err(|_| fmt::Error)
    }
}

impl<const N: usize> fmt::Display for Text<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<const N: usize> fmt::Debug for Text<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl<const N: usize, const M: usize> PartialEq<Text<M>> for Text<N> {
    fn eq(&self, other: &Text<M>) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<const N: usize> Eq for Text<N> {}

impl<const N: usize> PartialEq<str> for Text<N> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<const N: usize> PartialEq<&str> for Text<N> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl<const N: usize> std::hash::Hash for Text<N> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    #[test]
    fn capacity_is_enforced() {
        let mut t = Text::<4>::new();
        t.push_str("ab").unwrap();
        assert_eq!(t.push_str("abc"), Err(Error::TooLong));
        assert_eq!(t, "ab");
        t.push('é').unwrap();
        assert_eq!(t.len(), 4);
        assert!(t.push('x').is_err());
        t.truncate(3); // not a boundary: unchanged
        assert_eq!(t.as_str(), "abé");
        t.truncate(1);
        assert_eq!(t.as_str(), "a");
        t.clear();
        assert!(t.is_empty());
        assert!(write!(t, "{}", 12345).is_err());
        assert_eq!(Text::<3>::try_from_str("abcd"), Err(Error::TooLong));
    }
}
