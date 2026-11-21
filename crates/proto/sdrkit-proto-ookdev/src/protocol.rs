//! Device protocols and the list that offers each train to them.
//!
//! A device protocol is a [`Protocol`]: its name, its modulation, its [`Timing`], an optional despike width, and a decoder that
//! makes [`Reading`]s from [`BitRows`]. A stack keeps its own device decoders. It writes each one as a type that implements
//! [`Protocol`], or as a [`ProtocolDef`] (a `const` table entry with a decoder function).
//!
//! A [`ProtocolList`] holds the protocols of one receiver. Any crate can add protocols to it. [`ProtocolList::decode`] does
//! this for each train:
//!
//! 1. It refuses a train with a length that is not finite or that is negative.
//! 2. It offers the train to every protocol with the same modulation, in the order the protocols were added.
//! 3. If the protocol has a despike width, it merges short runs first ([`Train::despike`]) on a copy it keeps.
//! 4. The protocol's slicer makes the rows, and the decoder makes readings from each set of rows.
//! 5. It copies the signal measurements of the train into each reading. It keeps a reading once when two protocols or two sets
//!    of rows give the same reading ([`Reading::same_as`]).
//!
//! [`Ctx`] is the state of one train that a decoder can use. For example, a decoder can accept a code only on its second copy.
//!
//! ```
//! use sdrkit_proto_ookdev::{BitRows, Check, Coding, Ctx, Modulation, ProtocolDef, ProtocolList, Reading, Timing, Train};
//!
//! /// A test device: 8 bits of PWM, the id is the byte, sent twice.
//! fn decode(rows: &BitRows, ctx: &mut Ctx, out: &mut Vec<Reading>) {
//!     for row in rows.iter() {
//!         if row.len == 8 && !ctx.first_copy(row.byte(0) as u64) {
//!             out.push(Reading::new("Test", "remote", row.byte(0).to_string(), Check::Repeat));
//!         }
//!     }
//! }
//! static TEST: ProtocolDef = ProtocolDef {
//!     name: "Test remote",
//!     modulation: Modulation::Ook,
//!     timing: Timing::new(Coding::Pwm, 400.0, 1200.0, 5000.0).with_gap(2000.0),
//!     despike_us: 0.0,
//!     decode,
//! };
//! let mut list = ProtocolList::new();
//! list.add_static(&TEST);
//! // 0xA5 twice: a short mark is 1, a long mark is 0. A 3000 µs space starts the second row.
//! let mut pairs = Vec::new();
//! for _ in 0..2 {
//!     for k in 0..8 {
//!         let one = (0xA5u8 >> (7 - k)) & 1 == 1;
//!         pairs.push(if one { (400.0, 1200.0) } else { (1200.0, 400.0) });
//!     }
//!     pairs.last_mut().unwrap().1 = 3000.0;
//! }
//! let train = Train::from_us(Modulation::Ook, 1e6, &pairs);
//! let mut out = Vec::new();
//! list.decode(&train, &mut out).unwrap();
//! assert_eq!(out.len(), 1);
//! assert_eq!(out[0].id, "165");
//! ```

use crate::reading::Reading;
use crate::rows::BitRows;
use crate::slicer::{Slicer, Timing};
use crate::train::{Modulation, Train};
use crate::Error;
use std::sync::Arc;

/// The most codes a [`Ctx`] remembers in one train.
pub const CTX_CODES: usize = 64;

/// The state of one train that a decoder can use (module documentation).
///
/// It remembers up to [`CTX_CODES`] codes per train, each tagged with the protocol that saw it. The list clears it before each
/// train. It allocates nothing.
#[derive(Clone, Debug)]
pub struct Ctx {
    protocol: usize,
    codes: [(usize, u64); CTX_CODES],
    n: usize,
}

impl Default for Ctx {
    fn default() -> Self {
        Ctx {
            protocol: 0,
            codes: [(0, 0); CTX_CODES],
            n: 0,
        }
    }
}

impl Ctx {
    /// Forgets every code.
    pub fn clear(&mut self) {
        self.n = 0;
    }

    /// The index of the protocol that the list calls now.
    pub fn protocol(&self) -> usize {
        self.protocol
    }

    /// Returns true the first time this protocol reports `code` in this train, and false after that. If the memory is full, it
    /// returns true and does not remember the code.
    pub fn first_copy(&mut self, code: u64) -> bool {
        let key = (self.protocol, code);
        if self.codes[..self.n].contains(&key) {
            return false;
        }
        if self.n < CTX_CODES {
            self.codes[self.n] = key;
            self.n += 1;
        }
        true
    }
}

/// One device protocol (module documentation).
pub trait Protocol: Send + Sync {
    /// The devices it decodes, for people ("Nexus, FreeTec NC-7345").
    fn name(&self) -> &str;
    /// OOK or FSK. The list offers a protocol only trains of its modulation.
    fn modulation(&self) -> Modulation;
    /// The coding and its timing.
    fn timing(&self) -> &Timing;
    /// Runs shorter than this are merged before slicing, µs. 0 means none.
    fn despike_us(&self) -> f32 {
        0.0
    }
    /// Appends the readings in one set of rows to `out`. A decoder adds a reading only when a frame test passed.
    fn decode(&self, rows: &BitRows, ctx: &mut Ctx, out: &mut Vec<Reading>);
}

/// A decoder function for a [`ProtocolDef`].
pub type DecodeFn = fn(&BitRows, &mut Ctx, &mut Vec<Reading>);

/// A protocol as a `const` table entry: the fields of [`Protocol`] and a decoder function.
#[derive(Clone, Copy, Debug)]
pub struct ProtocolDef {
    /// The devices it decodes.
    pub name: &'static str,
    /// OOK or FSK.
    pub modulation: Modulation,
    /// The coding and its timing.
    pub timing: Timing,
    /// Despike width, µs (0: none).
    pub despike_us: f32,
    /// The decoder.
    pub decode: DecodeFn,
}

impl Protocol for ProtocolDef {
    fn name(&self) -> &str {
        self.name
    }
    fn modulation(&self) -> Modulation {
        self.modulation
    }
    fn timing(&self) -> &Timing {
        &self.timing
    }
    fn despike_us(&self) -> f32 {
        self.despike_us
    }
    fn decode(&self, rows: &BitRows, ctx: &mut Ctx, out: &mut Vec<Reading>) {
        (self.decode)(rows, ctx, out)
    }
}

impl Protocol for &'static ProtocolDef {
    fn name(&self) -> &str {
        self.name
    }
    fn modulation(&self) -> Modulation {
        self.modulation
    }
    fn timing(&self) -> &Timing {
        &self.timing
    }
    fn despike_us(&self) -> f32 {
        self.despike_us
    }
    fn decode(&self, rows: &BitRows, ctx: &mut Ctx, out: &mut Vec<Reading>) {
        (self.decode)(rows, ctx, out)
    }
}

/// The protocols of one receiver (module documentation). Clone it cheaply: the protocols are shared.
#[derive(Clone, Default)]
pub struct ProtocolList {
    protocols: Vec<Arc<dyn Protocol>>,
}

impl std::fmt::Debug for ProtocolList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.protocols.iter().map(|p| p.name()))
            .finish()
    }
}

impl ProtocolList {
    /// An empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a protocol and returns its index.
    pub fn add(&mut self, p: impl Protocol + 'static) -> usize {
        self.protocols.push(Arc::new(p));
        self.protocols.len() - 1
    }

    /// Adds a protocol from a static table entry and returns its index.
    pub fn add_static(&mut self, p: &'static ProtocolDef) -> usize {
        self.add(p)
    }

    /// Adds every entry of a static table and returns the index of the first.
    pub fn add_table(&mut self, table: &'static [ProtocolDef]) -> usize {
        let first = self.protocols.len();
        for p in table {
            self.add(p);
        }
        first
    }

    /// The number of protocols.
    pub fn len(&self) -> usize {
        self.protocols.len()
    }

    /// Whether the list has no protocols.
    pub fn is_empty(&self) -> bool {
        self.protocols.is_empty()
    }

    /// Protocol `i`.
    pub fn get(&self, i: usize) -> Option<&dyn Protocol> {
        self.protocols.get(i).map(|p| p.as_ref())
    }

    /// Every protocol in order.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Protocol> + '_ {
        self.protocols.iter().map(|p| p.as_ref())
    }

    /// Decodes one train with a new [`Decoding`] state. A receiver that decodes many trains keeps one [`Decoding`] and calls
    /// [`Decoding::run`], which allocates nothing once its buffers have grown.
    pub fn decode(&self, train: &Train, out: &mut Vec<Reading>) -> Result<usize, Error> {
        Decoding::new().run(self, train, out)
    }
}

/// The working state of [`ProtocolList::decode`]: the slicer, the despiked copy of the train and the decoder context. Keep one
/// per receiver.
#[derive(Clone, Debug, Default)]
pub struct Decoding {
    slicer: Slicer,
    copy: Train,
    ctx: Ctx,
    found: Vec<Reading>,
}

impl Decoding {
    /// New working state with rows of the default capacity.
    pub fn new() -> Self {
        Self::default()
    }

    /// Offers `train` to every protocol of `list` with its modulation. It appends the readings that are not the same as one
    /// already appended for this train to `out`, and returns how many it appended. Errors for a train with a length that is not
    /// finite or is negative.
    pub fn run(
        &mut self,
        list: &ProtocolList,
        train: &Train,
        out: &mut Vec<Reading>,
    ) -> Result<usize, Error> {
        if !train.is_valid() {
            return Err(Error::BadTrain);
        }
        self.ctx.clear();
        let first = out.len();
        for (i, p) in list.protocols.iter().enumerate() {
            if p.modulation() != train.modulation || !p.timing().is_valid() {
                continue;
            }
            let t = if p.despike_us() > 0.0 {
                copy_train(&mut self.copy, train);
                self.copy.despike(train.samples(p.despike_us()));
                &self.copy
            } else {
                train
            };
            self.ctx.protocol = i;
            self.found.clear();
            let (ctx, found) = (&mut self.ctx, &mut self.found);
            self.slicer
                .slice(p.timing(), t, |rows| p.decode(rows, ctx, found))?;
            for mut r in self.found.drain(..) {
                r.protocol = i;
                r.offset_hz = train.offset_hz;
                r.level_dbfs = train.level_dbfs;
                r.snr_db = train.snr_db;
                r.time_s = train.time_s();
                if !out[first..].iter().any(|o| o.same_as(&r)) {
                    out.push(r);
                }
            }
        }
        Ok(out.len() - first)
    }
}

/// Copies `from` into `to`, reusing the pulse buffer of `to`.
fn copy_train(to: &mut Train, from: &Train) {
    let mut pulses = std::mem::take(&mut to.pulses);
    pulses.clear();
    pulses.extend_from_slice(&from.pulses);
    *to = Train { pulses, ..*from };
}
