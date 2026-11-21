//! [`PictureHistory`]: the last pictures that a stack finished, newest first, with growing numbers.

use std::collections::VecDeque;

/// The last `cap` finished pictures, newest first. Each picture gets a number from [`PictureHistory::next_id`]. The numbers
/// keep growing after [`PictureHistory::clear`] and past numbers seen in a restored state.
///
/// ```
/// use sdrkit_codec_image::history::PictureHistory;
///
/// let mut h = PictureHistory::new(2);
/// for name in ["a", "b", "c"] {
///     let id = h.next_id();
///     h.finish(id, name);
/// }
/// let ids: Vec<u64> = h.iter().map(|(id, _)| id).collect();
/// assert_eq!(ids, [3, 2]); // newest first, the oldest dropped
/// ```
#[derive(Clone, Debug)]
pub struct PictureHistory<T> {
    cap: usize,
    list: VecDeque<(u64, T)>,
    next: u64,
    finished: u64,
}

impl<T> PictureHistory<T> {
    /// A history of at most `cap` pictures (at least 1).
    pub fn new(cap: usize) -> PictureHistory<T> {
        PictureHistory {
            cap: cap.max(1),
            list: VecDeque::with_capacity(cap.max(1) + 1),
            next: 1,
            finished: 0,
        }
    }

    /// The number for the next picture (1, 2, 3 ...).
    pub fn next_id(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }

    /// Makes later numbers larger than `id` (after a restore).
    pub fn seen_id(&mut self, id: u64) {
        self.next = self.next.max(id.saturating_add(1));
    }

    /// Adds a finished picture. The oldest goes when the history is full.
    pub fn finish(&mut self, id: u64, picture: T) {
        self.seen_id(id);
        self.list.push_front((id, picture));
        self.list.truncate(self.cap);
        self.finished += 1;
    }

    /// The newest picture.
    pub fn latest(&self) -> Option<&T> {
        self.list.front().map(|e| &e.1)
    }

    /// The picture numbered `id`.
    pub fn get(&self, id: u64) -> Option<&T> {
        self.list.iter().find(|e| e.0 == id).map(|e| &e.1)
    }

    /// The pictures with their numbers, newest first.
    pub fn iter(&self) -> impl Iterator<Item = (u64, &T)> {
        self.list.iter().map(|e| (e.0, &e.1))
    }

    /// The pictures held.
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// True when no picture is held.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The number of pictures finished since the start, including those already dropped.
    pub fn finished(&self) -> u64 {
        self.finished
    }

    /// Removes every picture. The numbers keep growing.
    pub fn clear(&mut self) {
        self.list.clear();
    }
}
