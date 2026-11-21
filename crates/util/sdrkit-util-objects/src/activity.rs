//! Transmissions that end after a hang time.

/// What [`Activity::touch`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Touch {
    /// Nothing was active on the key: a transmission starts.
    Started,
    /// The transmission on the key goes on.
    Continued,
}

/// A transmission that ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Closed<K> {
    /// Its key (a channel, a slot, a talkgroup).
    pub key: K,
    /// Signal time of its first activity, seconds.
    pub start_s: f64,
    /// Signal time of its last activity, seconds.
    pub last_s: f64,
}

/// The transmissions in progress, keyed by channel. Each ends when it had no activity for the hang time (a call on a voice
/// channel that ends without its terminator, for example).
///
/// ```
/// use sdrkit_util_objects::{Activity, Touch};
/// let mut a = Activity::new(2.0, 8);
/// assert_eq!(a.touch(1u32, 10.0), Touch::Started);
/// assert_eq!(a.touch(1, 11.0), Touch::Continued);
/// let mut ended = Vec::new();
/// a.expire(14.0, |c| ended.push(c)); // silent for 3 s, longer than the 2 s hang time
/// assert_eq!((ended[0].start_s, ended[0].last_s), (10.0, 11.0));
/// ```
#[derive(Clone, Debug)]
pub struct Activity<K> {
    active: Vec<Closed<K>>,
    hang_s: f64,
}

impl<K: Eq + Copy> Activity<K> {
    /// No transmission in progress. A transmission ends after `hang_s` seconds without activity. Room is made for `channels`
    /// keys at once.
    pub fn new(hang_s: f64, channels: usize) -> Self {
        Activity {
            active: Vec::with_capacity(channels.max(1)),
            hang_s: if hang_s.is_finite() {
                hang_s.max(0.0)
            } else {
                0.0
            },
        }
    }

    /// The hang time, seconds.
    pub fn hang_s(&self) -> f64 {
        self.hang_s
    }

    /// Sets the hang time, seconds. A value that is not finite or below 0 is refused, and the hang time does not change.
    pub fn set_hang_s(&mut self, hang_s: f64) -> bool {
        if hang_s.is_finite() && hang_s >= 0.0 {
            self.hang_s = hang_s;
            true
        } else {
            false
        }
    }

    /// Activity on `key` at signal time `now_s`. A time that is not finite changes nothing and counts as a continuation.
    pub fn touch(&mut self, key: K, now_s: f64) -> Touch {
        if let Some(a) = self.active.iter_mut().find(|a| a.key == key) {
            if now_s.is_finite() && now_s > a.last_s {
                a.last_s = now_s;
            }
            return Touch::Continued;
        }
        if !now_s.is_finite() {
            return Touch::Continued;
        }
        self.active.push(Closed {
            key,
            start_s: now_s,
            last_s: now_s,
        });
        Touch::Started
    }

    /// The transmission in progress on `key`.
    pub fn get(&self, key: K) -> Option<&Closed<K>> {
        self.active.iter().find(|a| a.key == key)
    }

    /// Ends the transmission on `key` (its terminator came).
    pub fn close(&mut self, key: K) -> Option<Closed<K>> {
        let i = self.active.iter().position(|a| a.key == key)?;
        Some(self.active.swap_remove(i))
    }

    /// Ends every transmission silent for longer than the hang time at `now_s`, and passes each to `ended`. Returns how many ended.
    pub fn expire(&mut self, now_s: f64, mut ended: impl FnMut(Closed<K>)) -> usize {
        if !now_s.is_finite() {
            return 0;
        }
        let mut n = 0;
        let mut i = 0;
        while i < self.active.len() {
            if now_s - self.active[i].last_s > self.hang_s {
                ended(self.active.swap_remove(i));
                n += 1;
            } else {
                i += 1;
            }
        }
        n
    }

    /// Ends every transmission (a retune), and passes each to `ended`.
    pub fn close_all(&mut self, mut ended: impl FnMut(Closed<K>)) {
        self.active.drain(..).for_each(&mut ended);
    }

    /// The transmissions in progress.
    pub fn iter(&self) -> impl Iterator<Item = &Closed<K>> {
        self.active.iter()
    }

    /// Transmissions in progress now.
    pub fn len(&self) -> usize {
        self.active.len()
    }

    /// Returns true if no transmission is in progress.
    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }
}
