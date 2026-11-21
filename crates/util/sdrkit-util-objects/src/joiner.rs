//! Joining messages sent in several parts.

/// A message body that parts can be added to.
pub trait Joinable {
    /// The size of the body, in the unit [`JoinRules::max_len`] uses (bytes of text, for example).
    fn joined_len(&self) -> usize;
    /// Adds the body of the next part to the end of this one.
    fn append(&mut self, next: Self);
}

impl Joinable for Vec<u8> {
    fn joined_len(&self) -> usize {
        self.len()
    }
    fn append(&mut self, next: Self) {
        self.extend_from_slice(&next);
    }
}

impl Joinable for String {
    fn joined_len(&self) -> usize {
        self.len()
    }
    fn append(&mut self, next: Self) {
        self.push_str(&next);
    }
}

/// How a part says where it goes in its message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Numbering {
    /// A running number (0, 1, 2 ... or the letters A, B, C as 0, 1, 2). The same number again is a repeat. A number that is
    /// not the next one shows a missing part, and the message is then marked incomplete.
    Seq(u32),
    /// A label that only shows repeats: the same label as the last part is a repeat. Missing parts cannot be seen.
    Tag(u32),
    /// No number. Every part is new.
    None,
}

/// One part of a message.
#[derive(Clone, Debug, PartialEq)]
pub struct Part<M> {
    /// Signal time, seconds.
    pub t: f64,
    /// The number of the whole message. If it differs from the message open under the same key, the open message is given out
    /// incomplete and this part starts a new one. Use 0 where the signal provides no such number.
    pub message: u32,
    /// Where the part goes.
    pub number: Numbering,
    /// True for the last part of its message.
    pub last: bool,
    /// The body.
    pub body: M,
}

/// A message given out by the [`Joiner`].
#[derive(Clone, Debug, PartialEq)]
pub struct Joined<K, M> {
    /// The key it was joined under.
    pub key: K,
    /// The joined body.
    pub body: M,
    /// Parts joined (repeats not counted).
    pub parts: u32,
    /// True if every part came, in order, up to a last part. False if a part was missing, the message timed out, was cut off by a
    /// new message number or was pushed out by the cap on open messages.
    pub complete: bool,
    /// True if a part was not added because it made the body longer than [`JoinRules::max_len`].
    pub cut: bool,
    /// Signal time of the first part.
    pub first_t: f64,
    /// Signal time of the last part.
    pub last_t: f64,
}

/// The rules of a [`Joiner`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JoinRules {
    /// If no part came for this many seconds, the open message is given out incomplete.
    pub timeout_s: f64,
    /// The most messages open at a time. If a new message needs room, the oldest open one is given out incomplete.
    pub max_open: usize,
    /// The largest joined body ([`Joinable::joined_len`]). A part that makes the body longer is not added, and the message
    /// is marked cut and incomplete.
    pub max_len: usize,
}

impl Default for JoinRules {
    /// 45 s timeout, 64 open messages, 8192 units of body.
    fn default() -> Self {
        JoinRules {
            timeout_s: 45.0,
            max_open: 64,
            max_len: 8192,
        }
    }
}

struct Open<K, M> {
    key: K,
    message: u32,
    body: M,
    parts: u32,
    complete: bool,
    cut: bool,
    first_t: f64,
    last_t: f64,
    last_number: Numbering,
}

impl<K, M> Open<K, M> {
    fn done(self, complete: bool) -> Joined<K, M> {
        Joined {
            key: self.key,
            body: self.body,
            parts: self.parts,
            complete: self.complete && complete,
            cut: self.cut,
            first_t: self.first_t,
            last_t: self.last_t,
        }
    }
}

/// Joins the parts of messages again, keyed by what identifies a message (sender, direction, label ...).
///
/// The rules: a repeated part is dropped. A part out of order marks its message incomplete. A new message number under the same
/// key emits the open message incomplete. The last part emits the joined message. Timeouts, a cap on open messages and a
/// cap on the body length give out incomplete messages. A message of one part (its first part is also its last) is given out at
/// once. If the signal clock goes back by more than one second, open messages are given out incomplete.
///
/// ```
/// use sdrkit_util_objects::{JoinRules, Joiner, Numbering, Part};
///
/// let mut j: Joiner<&str, String> = Joiner::new(JoinRules::default());
/// let mut out = Vec::new();
/// let part = |t, n, last, s: &str| Part { t, message: 1, number: Numbering::Seq(n), last, body: s.to_string() };
/// j.push("N123|H1", part(1.0, 0, false, "WX OMDB "), &mut out);
/// j.push("N123|H1", part(1.5, 0, false, "WX OMDB "), &mut out); // a repeat
/// j.push("N123|H1", part(2.0, 1, true, "1200Z"), &mut out);
/// assert_eq!(out[0].body, "WX OMDB 1200Z");
/// assert!(out[0].complete);
/// assert_eq!(out[0].parts, 2);
/// ```
pub struct Joiner<K, M> {
    open: Vec<Open<K, M>>,
    rules: JoinRules,
    enabled: bool,
}

impl<K: Eq, M: Joinable> Joiner<K, M> {
    /// A joiner with these rules (at least one open message).
    pub fn new(rules: JoinRules) -> Self {
        let rules = JoinRules {
            max_open: rules.max_open.max(1),
            ..rules
        };
        Joiner {
            open: Vec::with_capacity(rules.max_open),
            rules,
            enabled: true,
        }
    }

    /// The rules.
    pub fn rules(&self) -> JoinRules {
        self.rules
    }

    /// Turns joining on or off. If it is off, every part is given out as a message of its own. Turning it off emits the open
    /// messages first.
    pub fn set_enabled(&mut self, on: bool, out: &mut Vec<Joined<K, M>>) {
        if !on {
            self.flush(out);
        }
        self.enabled = on;
    }

    /// Messages open now.
    pub fn open_count(&self) -> usize {
        self.open.len()
    }

    /// Forgets the open messages (a reset or a retune).
    pub fn clear(&mut self) {
        self.open.clear();
    }

    /// Emits every open message, incomplete.
    pub fn flush(&mut self, out: &mut Vec<Joined<K, M>>) {
        out.extend(self.open.drain(..).map(|o| o.done(false)));
    }

    /// Emits the open messages that timed out at signal time `now_s`.
    pub fn expire(&mut self, now_s: f64, out: &mut Vec<Joined<K, M>>) {
        if !now_s.is_finite() {
            return;
        }
        let mut i = 0;
        while i < self.open.len() {
            let t = self.open[i].last_t;
            if now_s - t > self.rules.timeout_s || t - now_s > 1.0 {
                out.push(self.open.remove(i).done(false));
            } else {
                i += 1;
            }
        }
    }

    /// Takes one part of the message `key`. Adds to `out` the messages it completes, and those it ends incomplete. A part whose
    /// time is not finite is given out as a message of its own, marked incomplete.
    pub fn push(&mut self, key: K, part: Part<M>, out: &mut Vec<Joined<K, M>>) {
        if !part.t.is_finite() {
            out.push(single(key, part, false));
            return;
        }
        if !self.enabled {
            out.push(single(key, part, true));
            return;
        }
        self.expire(part.t, out);
        if let Some(i) = self.open.iter().position(|o| o.key == key) {
            if self.open[i].message != part.message {
                out.push(self.open.remove(i).done(false));
            } else {
                let rules = self.rules;
                let o = &mut self.open[i];
                if is_repeat(o.last_number, part.number) {
                    o.last_t = part.t;
                    return;
                }
                if let (Numbering::Seq(last), Numbering::Seq(n)) = (o.last_number, part.number) {
                    if n != last.wrapping_add(1) {
                        o.complete = false;
                    }
                }
                if o.body.joined_len() + part.body.joined_len() > rules.max_len {
                    o.complete = false;
                    o.cut = true;
                } else {
                    o.body.append(part.body);
                }
                o.parts += 1;
                o.last_number = part.number;
                o.last_t = part.t;
                if part.last {
                    out.push(self.open.remove(i).done(true));
                }
                return;
            }
        }
        if part.last {
            let first_is_start = !matches!(part.number, Numbering::Seq(n) if n != 0);
            out.push(single(key, part, first_is_start));
            return;
        }
        if self.open.len() >= self.rules.max_open {
            out.push(self.open.remove(0).done(false));
        }
        let complete = !matches!(part.number, Numbering::Seq(n) if n != 0);
        let cut = part.body.joined_len() > self.rules.max_len;
        self.open.push(Open {
            key,
            message: part.message,
            body: part.body,
            parts: 1,
            complete: complete && !cut,
            cut,
            first_t: part.t,
            last_t: part.t,
            last_number: part.number,
        });
    }
}

fn is_repeat(last: Numbering, now: Numbering) -> bool {
    match (last, now) {
        (Numbering::Seq(a), Numbering::Seq(b)) => a == b,
        (Numbering::Tag(a), Numbering::Tag(b)) => a == b,
        _ => false,
    }
}

fn single<K, M>(key: K, part: Part<M>, complete: bool) -> Joined<K, M> {
    Joined {
        key,
        body: part.body,
        parts: 1,
        complete,
        cut: false,
        first_t: part.t,
        last_t: part.t,
    }
}
