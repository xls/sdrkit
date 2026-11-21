//! Streams: what flows along an edge of a graph.
//!
//! A stream has a [`Kind`] (what its items are) and a [`Format`] (fixed when the graph is configured: rate, centre, bandwidth ...).
//! Each call of a component sees one [`Buffer`] per port: the items of that call, the absolute index of the first one, and the
//! [`Tag`]s attached at item positions. A typed stream carries a Rust type that implements [`Item`].

use crate::media::MediaFormat;
pub use crate::media::Packet;
use sdrkit_core::{Complex32, Event, EventDef, Hz, Record};
use std::any::{Any, TypeId};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// The type of a typed stream: the Rust type of its items, its kind key and the event kind that its items leave the graph as.
/// Made once from the type with [`ItemType::of`].
#[derive(Clone, Copy)]
pub struct ItemType {
    id: TypeId,
    meta: fn() -> ItemMeta,
}

/// The properties [`ItemType`] holds for a type, read through its accessors.
#[derive(Clone, Copy)]
struct ItemMeta {
    rust: &'static str,
    key: &'static str,
    event: Option<&'static EventDef>,
}

fn meta_of<T: Item>() -> ItemMeta {
    ItemMeta {
        rust: std::any::type_name::<T>(),
        key: T::KIND,
        event: T::event_def(),
    }
}

impl ItemType {
    /// The item type of `T`.
    pub fn of<T: Item>() -> ItemType {
        debug_assert!(
            T::event_def().is_none_or(|d| d.kind == T::KIND),
            "the event kind of {} is not its KIND",
            std::any::type_name::<T>()
        );
        ItemType {
            id: TypeId::of::<T>(),
            meta: meta_of::<T>,
        }
    }
    /// The kind key (`"epg.event"`).
    pub fn key(&self) -> &'static str {
        (self.meta)().key
    }
    /// The name of the Rust type.
    pub fn rust_name(&self) -> &'static str {
        (self.meta)().rust
    }
    /// The Rust type.
    pub fn type_id(&self) -> TypeId {
        self.id
    }
    /// The event kind that an item of this type becomes when it leaves the graph. `None` keeps it inside the graph.
    pub fn event_def(&self) -> Option<&'static EventDef> {
        (self.meta)().event
    }
    /// Whether this is the item type of `T`.
    pub fn is<T: Item>(&self) -> bool {
        self.id == TypeId::of::<T>()
    }
}

impl PartialEq for ItemType {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ItemType {}

impl Hash for ItemType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl fmt::Debug for ItemType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.key(), self.rust_name())
    }
}

/// What a stream carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Complex samples (radio samples, a channel).
    Iq,
    /// Real samples (an FM multiplex, a demodulated signal).
    Real,
    /// Audio frames, [`Format::channels`] samples each.
    Audio,
    /// Complex points (a constellation).
    Symbols,
    /// Hard bits: one `u8` per bit, 0 or 1, the first sent first.
    HardBits,
    /// Soft bits: one `i8` log-likelihood ratio per bit, `L = ln P(0) / P(1)` scaled by [`Format::llr_scale`]. Positive means 0,
    /// 0 is an erasure, and the value saturates at ±127.
    Llr,
    /// Byte packets ([`Packet`]): transport stream packets, frames after FEC, compressed media.
    Bytes,
    /// Events ([`Event`]): decoded groups, messages, results.
    Messages,
    /// Frames of dB bins ([`Frames`]).
    Spectrum,
    /// A typed stream of native Rust values ([`Item`]).
    Item(ItemType),
}

impl Kind {
    /// The typed kind for the Rust type `T`.
    pub fn typed<T: Item>() -> Kind {
        Kind::Item(ItemType::of::<T>())
    }

    /// Whether items of this kind have a sample rate (one item per `1 / rate` seconds).
    pub fn is_sampled(&self) -> bool {
        matches!(
            self,
            Kind::Iq
                | Kind::Real
                | Kind::Audio
                | Kind::Symbols
                | Kind::HardBits
                | Kind::Llr
                | Kind::Spectrum
        )
    }

    /// The sample type that the graph holds items of this kind in. `None` for a kind without samples.
    pub fn sample_type(&self) -> Option<SampleType> {
        match self {
            Kind::Iq | Kind::Symbols => Some(SampleType::Cf32),
            Kind::Real | Kind::Audio | Kind::Spectrum => Some(SampleType::F32),
            Kind::HardBits => Some(SampleType::U8),
            Kind::Llr => Some(SampleType::I8),
            Kind::Bytes | Kind::Messages | Kind::Item(_) => None,
        }
    }

    /// An empty buffer of this kind.
    pub fn empty_data(&self) -> Data {
        match self {
            Kind::Iq => Data::Iq(Vec::new()),
            Kind::Real => Data::Real(Vec::new()),
            Kind::Audio => Data::Audio(AudioFrames::default()),
            Kind::Symbols => Data::Symbols(Vec::new()),
            Kind::HardBits => Data::HardBits(Vec::new()),
            Kind::Llr => Data::Llr(Vec::new()),
            Kind::Bytes => Data::Bytes(Vec::new()),
            Kind::Messages => Data::Messages(Vec::new()),
            Kind::Spectrum => Data::Spectrum(Frames::default()),
            Kind::Item(_) => Data::Items(None),
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Iq => f.write_str("iq"),
            Kind::Real => f.write_str("real"),
            Kind::Audio => f.write_str("audio"),
            Kind::Symbols => f.write_str("symbols"),
            Kind::HardBits => f.write_str("hard_bits"),
            Kind::Llr => f.write_str("llr"),
            Kind::Bytes => f.write_str("bytes"),
            Kind::Messages => f.write_str("messages"),
            Kind::Spectrum => f.write_str("spectrum"),
            Kind::Item(t) => f.write_str(t.key()),
        }
    }
}

/// How one sample is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SampleType {
    /// 32-bit float.
    F32,
    /// Signed 8-bit integer.
    I8,
    /// Signed 16-bit integer.
    I16,
    /// Signed 32-bit integer.
    I32,
    /// Unsigned 8-bit integer.
    U8,
    /// Complex, two 32-bit floats.
    Cf32,
    /// Complex, two signed 16-bit integers.
    Cs16,
    /// Complex, two signed 32-bit integers (24-bit samples in 32-bit words, for example).
    Cs32,
    /// Complex, two signed 8-bit integers.
    Cs8,
    /// Complex, two unsigned 8-bit integers with an offset of 128.
    Cu8,
}

impl SampleType {
    /// The bytes of one value: of one number, so of half a complex sample.
    pub fn value_bytes(self) -> usize {
        match self {
            SampleType::I8 | SampleType::U8 | SampleType::Cs8 | SampleType::Cu8 => 1,
            SampleType::I16 | SampleType::Cs16 => 2,
            SampleType::I32 | SampleType::F32 | SampleType::Cf32 | SampleType::Cs32 => 4,
        }
    }
}

/// The order of the bits of a byte in a bit stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BitOrder {
    /// The most significant bit is sent first.
    MsbFirst,
    /// The least significant bit is sent first.
    LsbFirst,
}

/// A native Rust value carried by a typed stream.
///
/// ```
/// use sdrkit_graph::Item;
/// #[derive(Clone, Debug)]
/// struct EpgEvent { title: String, start: i64 }
/// impl Item for EpgEvent {
///     const KIND: &'static str = "epg.event";
/// }
/// ```
///
/// Values stay native inside the graph. An item type with an event kind ([`Item::event_def`]) leaves the graph as an [`Event`]
/// of that kind.
pub trait Item: Clone + Send + Sync + 'static {
    /// The kind key. If the type has an event kind, the key is the kind key of that event.
    const KIND: &'static str;
    /// The event kind that an item becomes when it leaves the graph. `None` (the default) keeps it inside the graph.
    fn event_def() -> Option<&'static EventDef> {
        None
    }
    /// Writes the item into `fields`, a record of the fields of [`Item::event_def`]. The default writes nothing.
    fn write_fields(&self, fields: &mut Record) {
        let _ = fields;
    }
}

impl Item for sdrkit_core::HostRequest {
    const KIND: &'static str = "host.request";
}

/// A value of a registered item type, carried by a tag ([`TagKind::Typed`]): a stack's own marker, such as the timing of a frame.
#[derive(Clone)]
pub struct TypedTag {
    ty: ItemType,
    value: Arc<dyn Any + Send + Sync>,
}

impl TypedTag {
    /// The item type of the value.
    pub fn item_type(&self) -> ItemType {
        self.ty
    }
    /// The value, if it is a `T`.
    pub fn get<T: Item>(&self) -> Option<&T> {
        self.value.downcast_ref::<T>()
    }
}

/// Makes the typed tags of one item type for one producer. It keeps the values of its tags in memory and reuses a value once no
/// tag holds it, so it allocates only until it holds as many values as tags are alive at one time.
///
/// ```
/// use sdrkit_graph::{Item, TagSource};
/// #[derive(Clone, Copy)]
/// struct Recentred(f64);
/// impl Item for Recentred {
///     const KIND: &'static str = "dab.recentred";
/// }
/// let mut tags = TagSource::<Recentred>::new();
/// let t = tags.tag(Recentred(250.0));
/// assert_eq!(t.get::<Recentred>().map(|r| r.0), Some(250.0));
/// ```
pub struct TagSource<T: Item> {
    values: Vec<Arc<T>>,
}

impl<T: Item> Default for TagSource<T> {
    fn default() -> Self {
        TagSource { values: Vec::new() }
    }
}

impl<T: Item> TagSource<T> {
    /// A source that holds no value yet.
    pub fn new() -> Self {
        Self::default()
    }
    /// A tag that carries `value`.
    pub fn tag(&mut self, value: T) -> TypedTag {
        let free = self
            .values
            .iter_mut()
            .position(|a| Arc::get_mut(a).is_some());
        let slot = match free {
            Some(i) => {
                let a = &mut self.values[i];
                if let Some(v) = Arc::get_mut(a) {
                    *v = value;
                }
                &self.values[i]
            }
            None => {
                self.values.push(Arc::new(value));
                &self.values[self.values.len() - 1]
            }
        };
        let value: Arc<dyn Any + Send + Sync> = Arc::clone(slot) as _;
        TypedTag {
            ty: ItemType::of::<T>(),
            value,
        }
    }
}

/// Two typed tags are equal if they share one value.
impl PartialEq for TypedTag {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
}

impl fmt::Debug for TypedTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypedTag({:?})", self.ty)
    }
}

/// One step of [`walk_tags`]: a run of items, or a tag just before the item it is attached to.
#[derive(Clone, Copy, Debug)]
pub enum TagStep<'t> {
    /// Items `a .. b` of this call (offsets from its first item).
    Items(usize, usize),
    /// A tag at offset `at` (`at` may equal the number of items: the tag stands before the next call's first item).
    Tag(usize, &'t Tag),
}

/// Cuts the `n` items of a call whose first item has absolute index `first` at the positions of `tags`: `f` sees the runs of items
/// between tags, and each tag just before the item it is attached to, in order. For a component that must act at a tag's exact
/// item (reset a loop at a retune, restart a descrambler at a frame start). Allocates nothing.
///
/// ```
/// use sdrkit_graph::{walk_tags, Tag, TagKind, TagStep};
/// let tags = [Tag { at: 12, kind: TagKind::EndOfStream }];
/// let mut seen = Vec::new();
/// walk_tags(10, 5, &tags, |s| match s {
///     TagStep::Items(a, b) => seen.push((a, b)),
///     TagStep::Tag(at, _) => seen.push((at, usize::MAX)),
/// });
/// assert_eq!(seen, [(0, 2), (2, usize::MAX), (2, 5)]);
/// ```
pub fn walk_tags<'t>(first: u64, n: usize, tags: &'t [Tag], mut f: impl FnMut(TagStep<'t>)) {
    let mut pos = 0usize;
    for t in tags {
        let at = (t.at.saturating_sub(first) as usize).min(n);
        if at > pos {
            f(TagStep::Items(pos, at));
            pos = at;
        }
        f(TagStep::Tag(at, t));
    }
    if pos < n {
        f(TagStep::Items(pos, n));
    }
}

/// Reusable shared slices (`Arc<[T]>`) for items that carry arrays (packets, chirp bins, OFDM cells, bursts): a slice is reused
/// once every holder downstream has dropped it, so once the pool holds as many slices as are in flight at once, making
/// one allocates nothing. At most [`SlicePool::MAX`] slices are kept; beyond that (slices held downstream for a long time) a slice
/// gets its own allocation.
///
/// ```
/// use sdrkit_graph::SlicePool;
/// let mut pool = SlicePool::<f32>::default();
/// let a = pool.exact(3, |s| s.copy_from_slice(&[1.0, 2.0, 3.0]));
/// assert_eq!(&*a, &[1.0, 2.0, 3.0]);
/// drop(a);
/// let b = pool.exact(3, |s| s.fill(0.5)); // the same slice, reused
/// assert_eq!(&*b, &[0.5; 3]);
/// ```
#[derive(Debug)]
pub struct SlicePool<T> {
    slots: Vec<Arc<[T]>>,
    next: usize,
}

impl<T> Default for SlicePool<T> {
    fn default() -> Self {
        SlicePool {
            slots: Vec::new(),
            next: 0,
        }
    }
}

impl<T: Copy + Default> SlicePool<T> {
    /// The most slices kept for reuse.
    pub const MAX: usize = 1024;

    /// A pool with `ready` slices of `len` values made now (so the first ones cost nothing later).
    pub fn with_ready(len: usize, ready: usize) -> Self {
        SlicePool {
            slots: (0..ready.min(Self::MAX))
                .map(|_| Arc::from(vec![T::default(); len]))
                .collect(),
            next: 0,
        }
    }

    /// A free slice of at least `len` values with its first `len` written by `fill` (the rest is left as it was): for items that
    /// carry their own length, such as a [`Packet`]. A slice is picked round robin from where the last one was taken, so the search
    /// is short.
    pub fn at_least(&mut self, len: usize, fill: impl FnOnce(&mut [T])) -> Arc<[T]> {
        self.take(len, |n| n >= len, len.max(64), fill)
    }

    /// A free slice of exactly `len` values, written by `fill`.
    pub fn exact(&mut self, len: usize, fill: impl FnOnce(&mut [T])) -> Arc<[T]> {
        self.take(len, |n| n == len, len, fill)
    }

    fn take(
        &mut self,
        len: usize,
        fits: impl Fn(usize) -> bool,
        new_len: usize,
        fill: impl FnOnce(&mut [T]),
    ) -> Arc<[T]> {
        let n = self.slots.len();
        let mut free_other = None;
        for i in 0..n {
            let j = (self.next + i) % n;
            let ok = fits(self.slots[j].len());
            if let Some(buf) = Arc::get_mut(&mut self.slots[j]) {
                if ok {
                    fill(&mut buf[..len]);
                    self.next = (j + 1) % n;
                    return self.slots[j].clone();
                }
                free_other.get_or_insert(j);
            }
        }
        // None fits: replace a free slice of another size, or add one while under the cap.
        let mut fresh: Arc<[T]> = Arc::from(vec![T::default(); new_len]);
        if let Some(buf) = Arc::get_mut(&mut fresh) {
            fill(&mut buf[..len]);
        }
        match free_other {
            Some(j) => self.slots[j] = fresh.clone(),
            None if n < Self::MAX => self.slots.push(fresh.clone()),
            None => {}
        }
        fresh
    }
}

/// Reusable packet buffers for a component that makes new bytes (frames from bits, decoded or descrambled packets): a
/// [`SlicePool`] of bytes, each packet a view of the first bytes of a free buffer, so a running graph allocates nothing per packet.
///
/// ```
/// use sdrkit_graph::PacketPool;
/// let mut pool = PacketPool::default();
/// let a = pool.make(4, 0.0, |b| b.copy_from_slice(&[1, 2, 3, 4]));
/// assert_eq!(a.bytes(), &[1, 2, 3, 4]);
/// drop(a);
/// let b = pool.make(3, 0.1, |b| b.fill(7)); // the first buffer, reused
/// assert_eq!(b.bytes(), &[7, 7, 7]);
/// ```
#[derive(Debug, Default)]
pub struct PacketPool(SlicePool<u8>);

impl PacketPool {
    /// A packet of `len` bytes at signal time `time`, its bytes written by `fill` (which gets exactly `len` bytes).
    pub fn make(&mut self, len: usize, time: f64, fill: impl FnOnce(&mut [u8])) -> Packet {
        let buf = self.0.at_least(len, fill);
        Packet::slice(&buf, 0, len, time)
    }
}

/// An item of a typed stream with its time, so it stays aligned with the samples it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Timed<T> {
    /// Seconds in the signal's own time: the index of the sample it came from divided by that stream's rate
    /// ([`Format::time_of`]).
    pub time: f64,
    /// The value.
    pub value: T,
}

/// Audio, stored interleaved: `channels` samples per frame, frame after frame. One channel is mono, two are left then right.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioFrames {
    /// Samples per frame, 1 or more. The graph sets it from the format of the stream.
    pub channels: u16,
    /// The samples.
    pub samples: Vec<f32>,
}

impl Default for AudioFrames {
    fn default() -> Self {
        AudioFrames {
            channels: 1,
            samples: Vec::new(),
        }
    }
}

impl AudioFrames {
    /// The number of whole frames.
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }
    /// The frames as arrays of `N` samples.
    ///
    /// # Panics
    ///
    /// If `N` is not the channel count. Reading another count is a programming error.
    pub fn frames_of<const N: usize>(&self) -> &[[f32; N]] {
        assert_eq!(
            usize::from(self.channels),
            N,
            "audio of {} channels",
            self.channels
        );
        self.samples.as_chunks::<N>().0
    }
    /// The samples of mono audio. Panics as [`AudioFrames::frames_of`].
    pub fn mono(&self) -> &[f32] {
        self.frames_of::<1>().as_flattened()
    }
    /// The frames of stereo audio, left then right. Panics as [`AudioFrames::frames_of`].
    pub fn stereo(&self) -> &[[f32; 2]] {
        self.frames_of::<2>()
    }
    /// Appends one frame, which must hold [`AudioFrames::channels`] samples.
    pub fn push_frame(&mut self, frame: &[f32]) {
        debug_assert_eq!(frame.len(), usize::from(self.channels));
        self.samples.extend_from_slice(frame);
    }
    /// Appends one stereo frame.
    pub fn push_stereo(&mut self, left: f32, right: f32) {
        debug_assert_eq!(self.channels, 2);
        self.samples.extend_from_slice(&[left, right]);
    }
}

/// Spectrum frames, stored flat: `bins` values per frame, in dBFS, from the lowest frequency to the highest.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frames {
    /// Bins per frame (the format's [`Format::frame_len`]).
    pub bins: usize,
    /// All frames, one after the other.
    pub values: Vec<f32>,
}

impl Frames {
    /// Number of frames.
    pub fn len(&self) -> usize {
        self.values.len().checked_div(self.bins).unwrap_or(0)
    }
    /// No frames.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Frame `i`.
    pub fn frame(&self, i: usize) -> &[f32] {
        &self.values[i * self.bins..(i + 1) * self.bins]
    }
    /// Appends a frame (`frame.len()` must be `bins`).
    pub fn push(&mut self, frame: &[f32]) {
        debug_assert_eq!(frame.len(), self.bins);
        self.values.extend_from_slice(frame);
    }
}

/// The type-erased storage of a typed stream. Implemented for `Vec<Timed<T>>` for every [`Item`]; component authors never implement
/// it.
pub trait ItemVec: Send + Sync {
    /// Number of items.
    fn len(&self) -> usize;
    /// No items.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Removes all items, keeping the allocation.
    fn clear(&mut self);
    /// For downcasting.
    fn as_any(&self) -> &dyn Any;
    /// For downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// The type of the items.
    fn item_type(&self) -> ItemType;
    /// Bytes of memory held by the vector (capacity times item size).
    fn bytes(&self) -> usize;
    /// Bytes of the items (length times item size).
    fn len_bytes(&self) -> usize {
        0
    }
    /// Keeps the first `n` items.
    fn truncate(&mut self, n: usize);
    /// Copies the items into `dst` (reusing it when it holds the same type).
    fn copy_into(&self, dst: &mut Option<Box<dyn ItemVec>>);
    /// Copies the items to the end of `dst` (created on first use). `false` when `dst` holds another type.
    fn extend_into(&self, dst: &mut Option<Box<dyn ItemVec>>) -> bool;
    /// Item `i` as an event on `freq_hz`, if the type has an event kind ([`Item::event_def`]).
    fn event(&self, i: usize, freq_hz: f64) -> Option<Event>;
    /// Moves every item to the end of `dst` (created on first use), leaving `self` empty. `false` when `dst` holds another type.
    fn append_to(&mut self, dst: &mut Option<Box<dyn ItemVec>>) -> bool;
    /// Makes room for `n` items in all.
    fn reserve_total(&mut self, n: usize);
}

impl<T: Item> ItemVec for Vec<Timed<T>> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn clear(&mut self) {
        Vec::clear(self)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn bytes(&self) -> usize {
        self.capacity() * std::mem::size_of::<Timed<T>>()
    }
    fn len_bytes(&self) -> usize {
        Vec::len(self) * std::mem::size_of::<Timed<T>>()
    }
    fn truncate(&mut self, n: usize) {
        Vec::truncate(self, n)
    }
    fn item_type(&self) -> ItemType {
        ItemType::of::<T>()
    }
    fn copy_into(&self, dst: &mut Option<Box<dyn ItemVec>>) {
        if let Some(v) = dst
            .as_mut()
            .and_then(|d| d.as_any_mut().downcast_mut::<Vec<Timed<T>>>())
        {
            v.clone_from(self);
            return;
        }
        *dst = Some(Box::new(self.clone()));
    }
    fn extend_into(&self, dst: &mut Option<Box<dyn ItemVec>>) -> bool {
        if dst.is_none() {
            *dst = Some(Box::new(Vec::<Timed<T>>::with_capacity(self.len())));
        }
        match dst
            .as_mut()
            .and_then(|d| d.as_any_mut().downcast_mut::<Vec<Timed<T>>>())
        {
            Some(v) => {
                v.extend_from_slice(self);
                true
            }
            None => false,
        }
    }
    fn event(&self, i: usize, freq_hz: f64) -> Option<Event> {
        let t = self.get(i)?;
        let mut e = T::event_def()?.event(t.time, freq_hz);
        t.value.write_fields(&mut e.fields);
        Some(e)
    }
    fn reserve_total(&mut self, n: usize) {
        self.reserve(n.saturating_sub(self.len()));
    }
    fn append_to(&mut self, dst: &mut Option<Box<dyn ItemVec>>) -> bool {
        if dst.is_none() {
            *dst = Some(Box::new(Vec::<Timed<T>>::with_capacity(self.len())));
        }
        match dst
            .as_mut()
            .and_then(|d| d.as_any_mut().downcast_mut::<Vec<Timed<T>>>())
        {
            Some(v) => {
                v.append(self);
                true
            }
            None => false,
        }
    }
}

/// The items of one buffer.
pub enum Data {
    /// See [`Kind::Iq`].
    Iq(Vec<Complex32>),
    /// See [`Kind::Real`].
    Real(Vec<f32>),
    /// See [`Kind::Audio`].
    Audio(AudioFrames),
    /// See [`Kind::Symbols`].
    Symbols(Vec<Complex32>),
    /// See [`Kind::HardBits`].
    HardBits(Vec<u8>),
    /// See [`Kind::Llr`].
    Llr(Vec<i8>),
    /// See [`Kind::Bytes`].
    Bytes(Vec<Packet>),
    /// See [`Kind::Messages`].
    Messages(Vec<Event>),
    /// See [`Kind::Spectrum`].
    Spectrum(Frames),
    /// See [`Kind::Item`]. Made on the first write, then reused.
    Items(Option<Box<dyn ItemVec>>),
}

impl Default for Data {
    fn default() -> Self {
        Data::Real(Vec::new())
    }
}

impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({} items)", self.kind_name(), self.len())
    }
}

impl Data {
    /// Number of items.
    pub fn len(&self) -> usize {
        match self {
            Data::Iq(v) | Data::Symbols(v) => v.len(),
            Data::Real(v) => v.len(),
            Data::Audio(a) => a.frames(),
            Data::HardBits(v) => v.len(),
            Data::Llr(v) => v.len(),
            Data::Bytes(v) => v.len(),
            Data::Messages(v) => v.len(),
            Data::Spectrum(f) => f.len(),
            Data::Items(c) => c.as_ref().map_or(0, |c| c.len()),
        }
    }
    /// No items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Removes all items, keeping the allocation.
    pub fn clear(&mut self) {
        match self {
            Data::Iq(v) | Data::Symbols(v) => v.clear(),
            Data::Real(v) => v.clear(),
            Data::Audio(a) => a.samples.clear(),
            Data::HardBits(v) => v.clear(),
            Data::Llr(v) => v.clear(),
            Data::Bytes(v) => v.clear(),
            Data::Messages(v) => v.clear(),
            Data::Spectrum(f) => f.values.clear(),
            Data::Items(c) => {
                if let Some(c) = c {
                    c.clear()
                }
            }
        }
    }
    fn kind_name(&self) -> &'static str {
        match self {
            Data::Iq(_) => "iq",
            Data::Real(_) => "real",
            Data::Audio(_) => "audio",
            Data::Symbols(_) => "symbols",
            Data::HardBits(_) => "hard_bits",
            Data::Llr(_) => "llr",
            Data::Bytes(_) => "bytes",
            Data::Messages(_) => "messages",
            Data::Spectrum(_) => "spectrum",
            Data::Items(_) => "items",
        }
    }
    /// Bytes of the items (length, not capacity). A packet counts its own bytes.
    pub fn len_bytes(&self) -> usize {
        use std::mem::size_of;
        match self {
            Data::Iq(v) | Data::Symbols(v) => v.len() * size_of::<Complex32>(),
            Data::Real(v) => v.len() * size_of::<f32>(),
            Data::Audio(a) => a.samples.len() * size_of::<f32>(),
            Data::HardBits(v) => v.len(),
            Data::Llr(v) => v.len(),
            Data::Bytes(v) => v.iter().map(|p| p.len).sum(),
            Data::Messages(v) => v.len() * size_of::<Event>(),
            Data::Spectrum(f) => f.values.len() * size_of::<f32>(),
            Data::Items(c) => c.as_ref().map_or(0, |c| c.len_bytes()),
        }
    }
    /// Bytes of memory held (capacity, not length). Shared packet bytes and typed items' own heap data are not counted.
    pub fn bytes(&self) -> usize {
        use std::mem::size_of;
        match self {
            Data::Iq(v) | Data::Symbols(v) => v.capacity() * size_of::<Complex32>(),
            Data::Real(v) => v.capacity() * size_of::<f32>(),
            Data::Audio(a) => a.samples.capacity() * size_of::<f32>(),
            Data::HardBits(v) => v.capacity(),
            Data::Llr(v) => v.capacity(),
            Data::Bytes(v) => v.capacity() * size_of::<Packet>(),
            Data::Messages(v) => v.capacity() * size_of::<Event>(),
            Data::Spectrum(f) => f.values.capacity() * size_of::<f32>(),
            Data::Items(c) => c.as_ref().map_or(0, |c| c.bytes()),
        }
    }
    /// Keeps the first `n` items, dropping the rest.
    pub fn truncate(&mut self, n: usize) {
        match self {
            Data::Iq(v) | Data::Symbols(v) => v.truncate(n),
            Data::Real(v) => v.truncate(n),
            Data::Audio(a) => a.samples.truncate(n * usize::from(a.channels.max(1))),
            Data::HardBits(v) => v.truncate(n),
            Data::Llr(v) => v.truncate(n),
            Data::Bytes(v) => v.truncate(n),
            Data::Messages(v) => v.truncate(n),
            Data::Spectrum(f) => f.values.truncate(n * f.bins),
            Data::Items(c) => {
                if let Some(c) = c {
                    c.truncate(n)
                }
            }
        }
    }
    /// Moves every item of `src` to the end of `self` (both of the same kind), leaving `src` empty with its allocation. Returns
    /// `false` and changes nothing when the kinds differ.
    pub fn append(&mut self, src: &mut Data) -> bool {
        match (self, src) {
            (Data::Iq(d), Data::Iq(s)) | (Data::Symbols(d), Data::Symbols(s)) => d.append(s),
            (Data::Real(d), Data::Real(s)) => d.append(s),
            (Data::Audio(d), Data::Audio(s)) => {
                d.channels = s.channels;
                d.samples.append(&mut s.samples)
            }
            (Data::HardBits(d), Data::HardBits(s)) => d.append(s),
            (Data::Llr(d), Data::Llr(s)) => d.append(s),
            (Data::Bytes(d), Data::Bytes(s)) => d.append(s),
            (Data::Messages(d), Data::Messages(s)) => d.append(s),
            (Data::Spectrum(d), Data::Spectrum(s)) => {
                d.bins = s.bins;
                d.values.append(&mut s.values)
            }
            (Data::Items(d), Data::Items(s)) => {
                if let Some(s) = s {
                    return s.append_to(d);
                }
            }
            _ => return false,
        }
        true
    }
    /// Copies every item of `src` to the end of `self` (both of the same kind). Returns `false` and changes nothing when the
    /// kinds differ.
    pub fn extend_copy(&mut self, src: &Data) -> bool {
        match (self, src) {
            (Data::Iq(d), Data::Iq(s)) | (Data::Symbols(d), Data::Symbols(s)) => {
                d.extend_from_slice(s)
            }
            (Data::Real(d), Data::Real(s)) => d.extend_from_slice(s),
            (Data::Audio(d), Data::Audio(s)) => {
                d.channels = s.channels;
                d.samples.extend_from_slice(&s.samples)
            }
            (Data::HardBits(d), Data::HardBits(s)) => d.extend_from_slice(s),
            (Data::Llr(d), Data::Llr(s)) => d.extend_from_slice(s),
            (Data::Bytes(d), Data::Bytes(s)) => d.extend_from_slice(s),
            (Data::Messages(d), Data::Messages(s)) => d.extend_from_slice(s),
            (Data::Spectrum(d), Data::Spectrum(s)) => {
                d.bins = s.bins;
                d.values.extend_from_slice(&s.values)
            }
            (Data::Items(d), Data::Items(s)) => {
                if let Some(s) = s {
                    return s.extend_into(d);
                }
            }
            _ => return false,
        }
        true
    }
    /// Copies `src` into `self`, reusing `self`'s allocation when it is of the same kind.
    pub fn copy_from(&mut self, src: &Data) {
        match (self, src) {
            (Data::Iq(d), Data::Iq(s)) | (Data::Symbols(d), Data::Symbols(s)) => d.clone_from(s),
            (Data::Real(d), Data::Real(s)) => d.clone_from(s),
            (Data::Audio(d), Data::Audio(s)) => {
                d.channels = s.channels;
                d.samples.clone_from(&s.samples)
            }
            (Data::HardBits(d), Data::HardBits(s)) => d.clone_from(s),
            (Data::Llr(d), Data::Llr(s)) => d.clone_from(s),
            (Data::Bytes(d), Data::Bytes(s)) => d.clone_from(s),
            (Data::Messages(d), Data::Messages(s)) => d.clone_from(s),
            (Data::Spectrum(d), Data::Spectrum(s)) => {
                d.bins = s.bins;
                d.values.clone_from(&s.values);
            }
            (Data::Items(d), Data::Items(s)) => match s {
                Some(s) => s.copy_into(d),
                None => {
                    if let Some(d) = d {
                        d.clear()
                    }
                }
            },
            (d, s) => {
                *d = match s {
                    Data::Iq(v) => Data::Iq(v.clone()),
                    Data::Real(v) => Data::Real(v.clone()),
                    Data::Audio(v) => Data::Audio(v.clone()),
                    Data::Symbols(v) => Data::Symbols(v.clone()),
                    Data::HardBits(v) => Data::HardBits(v.clone()),
                    Data::Llr(v) => Data::Llr(v.clone()),
                    Data::Bytes(v) => Data::Bytes(v.clone()),
                    Data::Messages(v) => Data::Messages(v.clone()),
                    Data::Spectrum(v) => Data::Spectrum(v.clone()),
                    Data::Items(s) => {
                        let mut d = None;
                        if let Some(s) = s {
                            s.copy_into(&mut d)
                        }
                        Data::Items(d)
                    }
                }
            }
        }
    }
}

/// What a tag says.
#[derive(Clone, Debug, PartialEq)]
pub enum TagKind {
    /// The radio (or a shift) moved: from this item on, the centre and the tuned frequency are these.
    Retuned {
        /// The frequency at 0 Hz in the stream.
        centre_hz: Hz,
        /// Where the user tuned.
        tuned_hz: Hz,
    },
    /// The gain in front of this item changed.
    GainChanged {
        /// The new gain in dB.
        db: f64,
    },
    /// Items were lost just before this one.
    Discontinuity {
        /// How many (0 when unknown).
        lost: u64,
    },
    /// This item was taken at this wall-clock time.
    Timestamp {
        /// Seconds since 1970-01-01 UTC.
        unix_s: f64,
    },
    /// Nothing follows this position.
    EndOfStream,
    /// This item is the first of a burst for a transmitter, and must leave the antenna at this time. A burst without
    /// this tag goes out as it comes.
    BurstStart {
        /// Seconds since 1970-01-01 UTC.
        unix_s: f64,
    },
    /// This item is the last of a burst for a transmitter. The transmitter ends the burst after it.
    BurstEnd,
    /// The decoder configuration changed (new SPS/PPS, AudioSpecificConfig) from this packet on.
    ConfigChanged {
        /// The new configuration bytes.
        config: Arc<[u8]>,
    },
    /// A synchroniser gained (`locked: true`) or lost lock at this item (symbol sync, carrier sync, a squelch opening).
    Lock {
        /// Locked from here on.
        locked: bool,
    },
    /// A frame starts at this item: a sync word was found here.
    FrameStart {
        /// The sync word was found inverted (the bits that follow have been inverted back).
        inverted: bool,
        /// Bit errors in the sync word.
        errors: u32,
    },
    /// A marker that a stack defines, as a value of a registered item type.
    Typed(TypedTag),
}

/// A tag at an item position.
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    /// The absolute index of the item it is attached to (counted since the start of the stream).
    pub at: u64,
    /// What it says.
    pub kind: TagKind,
}

/// The items of one port for one call, with the index of the first one and the tags.
///
/// The graph owns every buffer and reuses it from call to call, so nothing is allocated once it has grown. A fan-out passes the
/// same buffer to every consumer.
#[derive(Debug, Default)]
pub struct Buffer {
    pub(crate) first: u64,
    /// The items.
    pub data: Data,
    pub(crate) tags: Vec<Tag>,
    /// Items moved out to a component's output for in-place work (they still count for the indices).
    pub(crate) moved: usize,
}

impl Buffer {
    /// An empty buffer of `kind`, starting at index 0.
    pub fn new(kind: &Kind) -> Buffer {
        Buffer {
            first: 0,
            data: kind.empty_data(),
            tags: Vec::new(),
            moved: 0,
        }
    }
    /// The absolute index of the first item.
    pub fn first(&self) -> u64 {
        self.first
    }
    /// Number of items.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    /// No items.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    /// The absolute index just after the last item (the first index of the next call).
    pub fn end(&self) -> u64 {
        self.first + (self.len() + self.moved) as u64
    }
    /// The tags, in order of position, all within `first() ..= end()`.
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }
    /// The tags, to change in place (a component rewriting what it passes on).
    pub fn tags_mut(&mut self) -> &mut [Tag] {
        &mut self.tags
    }
    /// Attaches a tag at the absolute position `at`, keeping the tags in order.
    pub fn push_tag(&mut self, at: u64, kind: TagKind) {
        let i = self
            .tags
            .iter()
            .rposition(|t| t.at <= at)
            .map_or(0, |i| i + 1);
        self.tags.insert(i, Tag { at, kind });
    }
    /// Attaches a tag at `offset` items from the start of this buffer.
    pub fn tag_here(&mut self, offset: usize, kind: TagKind) {
        self.push_tag(self.first + offset as u64, kind)
    }
    /// Clears items and tags and moves `first` to the end: the state before a component appends a new call's items.
    pub(crate) fn advance(&mut self) {
        self.first = self.end();
        self.data.clear();
        self.tags.clear();
        self.moved = 0;
    }
    /// Bytes of memory held by the items and tags (capacity).
    pub fn bytes(&self) -> usize {
        self.data.bytes() + self.tags.capacity() * std::mem::size_of::<Tag>()
    }
    /// Copies the graph's input (its index and tags from `src`, its samples from `iq`) into this buffer, reusing allocations.
    pub(crate) fn copy_input(&mut self, src: &Buffer, iq: &[Complex32]) {
        self.first = src.first;
        match &mut self.data {
            Data::Iq(v) => {
                v.clear();
                v.extend_from_slice(iq);
            }
            d => *d = Data::Iq(iq.to_vec()),
        }
        self.tags.clone_from(&src.tags);
        self.moved = 0;
    }

    /// Copies another buffer (index, items, tags) into this one, reusing allocations.
    pub fn copy_from(&mut self, src: &Buffer) {
        self.first = src.first;
        self.data.copy_from(&src.data);
        self.tags.clone_from(&src.tags);
        self.moved = 0;
    }
}

/// The format of a stream, fixed when the graph is configured.
#[derive(Clone, Debug, PartialEq)]
pub struct Format {
    /// What the stream carries.
    pub kind: Kind,
    /// Items per second (0 for a stream without a rate: bytes, messages, typed items). For spectrum frames: frames per second.
    pub rate: Hz,
    /// Samples per audio frame (channels): 1 for mono audio and for every other kind, 2 for stereo audio.
    pub channels: u16,
    /// How one sample is stored. Inside a graph it follows from the kind ([`Kind::sample_type`]). `None` for a kind without
    /// samples.
    pub sample_type: Option<SampleType>,
    /// The bit order of a bit or byte stream, if the producer declares it.
    pub bit_order: Option<BitOrder>,
    /// The radio frequency at 0 Hz in the stream (`Iq`, `Symbols`, `Spectrum`); 0 when it has no meaning.
    pub centre_hz: Hz,
    /// Where the user tuned (the signal is at `tuned_hz - centre_hz` in an `Iq` stream).
    pub tuned_hz: Hz,
    /// The two-sided width of the meaningful part, in Hz (for real and audio streams, the highest meaningful frequency).
    pub bandwidth_hz: Hz,
    /// Values per item where an item is a frame: bins per spectrum frame; 0 otherwise.
    pub frame_len: usize,
    /// For compressed media and raw pixels: the codec, its packaging, configuration and properties.
    pub media: Option<MediaFormat>,
    /// For an [`Kind::Llr`] stream: `i8` units per unit of natural-log LLR (`i8 = round(llr_scale * L)`, saturated at ±127);
    /// `None` when the producer does not know it (hard decisions mapped to ±127, for example). Other kinds: `None`.
    pub llr_scale: Option<f32>,
    /// For a [`Kind::Spectrum`] stream: dB or linear power. Other kinds: `None`.
    pub scale: Option<crate::components::SpectrumScale>,
    /// For a [`Kind::Spectrum`] stream: the frames hold the bins from 0 Hz up to half the rate of the source (the spectrum of
    /// a real stream), and `bandwidth_hz` is half that rate. Other kinds: `false`.
    pub one_sided: bool,
    /// The group delay, in seconds, that the adapters which the graph inserted on the way to this stream add to it.
    pub delay_s: f64,
}

impl Format {
    /// A format of `kind` at `rate` with nothing else known.
    pub fn new(kind: Kind, rate: Hz) -> Format {
        Format {
            kind,
            rate,
            channels: 1,
            sample_type: kind.sample_type(),
            bit_order: None,
            centre_hz: 0.0,
            tuned_hz: 0.0,
            bandwidth_hz: rate,
            frame_len: 0,
            media: None,
            llr_scale: None,
            scale: None,
            one_sided: false,
            delay_s: 0.0,
        }
    }
    /// An audio stream at `rate` frames per second, of `channels` channels.
    pub fn audio(rate: Hz, channels: u16) -> Format {
        Format {
            channels,
            ..Format::new(Kind::Audio, rate)
        }
    }
    /// A [`Kind::HardBits`] stream at `rate` bits per second (0 for bits without a rate).
    pub fn hard_bits(rate: Hz) -> Format {
        Format::new(Kind::HardBits, rate)
    }
    /// A [`Kind::Llr`] stream at `rate` bits per second, with its scale (see [`Format::llr_scale`]).
    pub fn llr(rate: Hz, llr_scale: Option<f32>) -> Format {
        Format {
            llr_scale,
            ..Format::new(Kind::Llr, rate)
        }
    }
    /// An `Iq` stream at `rate` with its centre and tuned frequency.
    pub fn iq(rate: Hz, centre_hz: Hz, tuned_hz: Hz) -> Format {
        Format {
            centre_hz,
            tuned_hz,
            ..Format::new(Kind::Iq, rate)
        }
    }
    /// The same format with another kind (a component that changes only what the items are). The channel count (1) and the sample
    /// type follow the new kind.
    pub fn with_kind(&self, kind: Kind) -> Format {
        Format {
            kind,
            channels: 1,
            sample_type: kind.sample_type(),
            ..self.clone()
        }
    }
    /// The same format at another rate (a resampler).
    pub fn with_rate(&self, rate: Hz) -> Format {
        Format {
            rate,
            ..self.clone()
        }
    }
    /// The signal's offset from the centre (`tuned_hz - centre_hz`).
    pub fn signal_offset_hz(&self) -> Hz {
        self.tuned_hz - self.centre_hz
    }
    /// The signal time of the item at absolute index `at`, in seconds (0 for a stream without a rate).
    pub fn time_of(&self, at: u64) -> f64 {
        if self.rate > 0.0 {
            at as f64 / self.rate
        } else {
            0.0
        }
    }
}
