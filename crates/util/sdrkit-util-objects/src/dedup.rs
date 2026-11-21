//! De-duplication of repeats within a time window.

use crate::persist::Shift;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// Remembers the keys it saw in the last `window_s` seconds, so that a repeat of the same message (a relayed mesh packet, a frame
/// heard by two demodulators, a beacon sent three times) is reported once.
///
/// The window starts when a key is first seen. A repeat inside the window does not extend it. Thus a message sent again after the
/// window is reported again. The memory holds at most `cap` keys. If it is full, the oldest key leaves first. It is not emptied
/// all at once, so a burst of repeats just after a full memory is still caught.
///
/// Use [`fingerprint`] to make a `u64` key from the bytes of a message.
///
/// ```
/// use sdrkit_util_objects::{fingerprint, Dedup};
/// let mut d = Dedup::new(600.0, 4000);
/// let key = fingerprint(b"!a1b2c3d4 hello");
/// assert!(!d.seen(key, 10.0)); // new: report it
/// assert!(d.seen(key, 70.0)); // a repeat: drop it
/// assert!(!d.seen(key, 700.0)); // after the window: new again
/// ```
#[derive(Clone, Debug)]
pub struct Dedup<K> {
    times: HashMap<K, f64>,
    order: VecDeque<(K, f64)>,
    window_s: f64,
    cap: usize,
}

impl<K: Hash + Eq + Clone> Dedup<K> {
    /// A memory of at most `cap` keys (at least 1), each kept for `window_s` seconds of signal time.
    pub fn new(window_s: f64, cap: usize) -> Self {
        let cap = cap.max(1);
        Dedup {
            times: HashMap::with_capacity(cap.saturating_mul(2)),
            order: VecDeque::with_capacity(cap),
            window_s: if window_s.is_finite() {
                window_s.max(0.0)
            } else {
                0.0
            },
            cap,
        }
    }

    /// Returns true if `key` was seen in the window before signal time `now_s` (a repeat). Returns false and remembers the key
    /// if it is new. If `now_s` is not finite, the key counts as new and the memory does not change.
    pub fn seen(&mut self, key: K, now_s: f64) -> bool {
        if !now_s.is_finite() {
            return false;
        }
        self.forget_before(now_s);
        if let Some(&t) = self.times.get(&key) {
            // A key from a time after `now_s` (the signal clock went back) also counts as a repeat.
            if now_s - t <= self.window_s {
                return true;
            }
        }
        if self.order.len() >= self.cap {
            if let Some((old, _)) = self.order.pop_front() {
                self.times.remove(&old);
            }
        }
        self.times.insert(key.clone(), now_s);
        self.order.push_back((key, now_s));
        false
    }

    /// Returns true if `key` was seen in the window before `now_s`. It does not remember the key.
    pub fn contains(&self, key: &K, now_s: f64) -> bool {
        self.times
            .get(key)
            .is_some_and(|&t| now_s - t <= self.window_s)
    }

    /// Keys remembered now.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Returns true if no key is remembered.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// The window, seconds.
    pub fn window_s(&self) -> f64 {
        self.window_s
    }

    /// Forgets every key.
    pub fn clear(&mut self) {
        self.times.clear();
        self.order.clear();
    }

    fn forget_before(&mut self, now_s: f64) {
        while let Some((k, t)) = self.order.front() {
            if now_s - *t <= self.window_s {
                break;
            }
            self.times.remove(k);
            self.order.pop_front();
        }
    }
}

impl<K> Shift for Dedup<K> {
    fn shift_times(&mut self, by_s: f64) {
        self.times.values_mut().for_each(|t| *t += by_s);
        self.order.iter_mut().for_each(|(_, t)| *t += by_s);
    }
}

/// The 64-bit FNV-1a hash of `bytes`: a short key for a message, for [`Dedup`].
///
/// FNV-1a (Fowler, Noll and Vo) starts from the offset basis 0xcbf29ce484222325. For each byte it XORs the byte in, then
/// multiplies by the prime 0x100000001b3. The values below are from the published FNV test vectors.
///
/// ```
/// use sdrkit_util_objects::fingerprint;
/// assert_eq!(fingerprint(b""), 0xcbf2_9ce4_8422_2325);
/// assert_eq!(fingerprint(b"a"), 0xaf63_dc4c_8601_ec8c);
/// assert_eq!(fingerprint(b"foobar"), 0x8594_4171_f739_67e8);
/// ```
pub fn fingerprint(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
