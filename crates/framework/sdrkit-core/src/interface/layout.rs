//! The fixed layout of the C structs of a family: the type, the offset and the size of each member. The header writer
//! and the C library both take the layout from here, so a struct of a header is the struct that the library fills.
//!
//! A struct starts with `uint32_t size`. A row struct then holds the row key, an event struct the time and the
//! frequency. The members follow in the order of [`Family::layout_order`], each with a `bool has_<key>` before it if
//! the field is optional.

use super::{ArgShape, Family, Shape};
use crate::value::ValueKind;
use std::mem::{align_of, size_of};

/// The C type of a member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CType {
    /// `bool`.
    Bool,
    /// `int32_t`: the index of an option.
    Enum,
    /// `int64_t`.
    I64,
    /// `double`.
    F64,
    /// `sdrkit_string`.
    Str,
    /// `sdrkit_bytes`.
    Bytes,
    /// `sdrkit_position`.
    Position,
    /// `sdrkit_variant`: a value that carries its own type.
    Variant,
}

impl CType {
    /// The C type of a field of `kind`.
    pub fn of(kind: ValueKind) -> CType {
        match kind {
            ValueKind::Bool => CType::Bool,
            ValueKind::Choice => CType::Enum,
            ValueKind::I64 => CType::I64,
            ValueKind::F64 | ValueKind::Hz | ValueKind::Time | ValueKind::Duration => CType::F64,
            ValueKind::Text => CType::Str,
            ValueKind::Bytes => CType::Bytes,
            ValueKind::Position => CType::Position,
            ValueKind::Any => CType::Variant,
        }
    }
    /// The size and the alignment in bytes.
    pub fn size_align(self) -> (usize, usize) {
        self.size_align_const()
    }
    /// As [`CType::size_align`], for a constant.
    pub const fn size_align_const(self) -> (usize, usize) {
        match self {
            CType::Bool => (1, 1),
            CType::Enum => (size_of::<i32>(), align_of::<i32>()),
            CType::I64 => (size_of::<i64>(), align_of::<i64>()),
            CType::F64 => (size_of::<f64>(), align_of::<f64>()),
            // A pointer and a `size_t`.
            CType::Str | CType::Bytes => (2 * size_of::<usize>(), align_of::<usize>()),
            CType::Position => (size_of::<Position>(), align_of::<Position>()),
            CType::Variant => (size_of::<Variant>(), align_of::<Variant>()),
        }
    }
}

/// A place on the Earth: `sdrkit_position` of the base header.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    /// Degrees north.
    pub lat: f64,
    /// Degrees east.
    pub lon: f64,
    /// Metres above the WGS 84 ellipsoid. It holds a value only if `has_alt` is true.
    pub alt_m: f64,
    /// True if `alt_m` holds a value.
    pub has_alt: bool,
}

/// The layout of `sdrkit_variant` of the base header: the type, the members of the plain types, then a string and
/// bytes, each a pointer and a `size_t`.
#[repr(C)]
struct Variant {
    r#type: i32,
    flag: bool,
    enum_index: u32,
    integer: i64,
    number: f64,
    lat: f64,
    lon: f64,
    string: [usize; 2],
    bytes: [usize; 2],
}

/// One member of a struct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    /// The key of the field or argument. The member has this name, without the key of the family and a dot before it.
    pub key: String,
    /// The declared kind.
    pub kind: ValueKind,
    /// The C type.
    pub ctype: CType,
    /// The index of the field in its shape, or of the argument in its list.
    pub index: usize,
    /// The offset of the `has_<key>` member of an optional field.
    pub has_at: Option<usize>,
    /// The offset.
    pub at: usize,
}

/// What a struct holds between `size` and its members.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    /// Nothing: a result set, the arguments or the reply of a command.
    Plain,
    /// `sdrkit_string key`: a table row.
    Row,
    /// `double time` and `double freq_hz`: an event.
    Event,
}

/// The layout of one struct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The offset of the row key ([`Head::Row`]) or of the time ([`Head::Event`], the frequency is 8 bytes after it).
    pub head_at: usize,
    /// The members, in the order of the struct.
    pub members: Vec<Member>,
    /// The size of the struct in bytes.
    pub size: usize,
}

struct Placer {
    at: usize,
    align: usize,
}

impl Placer {
    /// After `uint32_t size`.
    fn new() -> Self {
        Placer {
            at: size_of::<u32>(),
            align: align_of::<u32>(),
        }
    }
    fn place(&mut self, (size, align): (usize, usize)) -> usize {
        let start = self.at.next_multiple_of(align);
        self.at = start + size;
        self.align = self.align.max(align);
        start
    }
    fn head(&mut self, head: Head) -> usize {
        match head {
            Head::Plain => 0,
            Head::Row => self.place(CType::Str.size_align()),
            Head::Event => {
                let at = self.place(CType::F64.size_align());
                self.place(CType::F64.size_align());
                at
            }
        }
    }
    fn member(&mut self, key: &str, kind: ValueKind, index: usize, optional: bool) -> Member {
        let ctype = CType::of(kind);
        let has_at = optional.then(|| self.place(CType::Bool.size_align()));
        Member {
            key: key.to_owned(),
            kind,
            ctype,
            index,
            has_at,
            at: self.place(ctype.size_align()),
        }
    }
    fn finish(self, head_at: usize, members: Vec<Member>) -> Layout {
        Layout {
            head_at,
            members,
            size: self.at.next_multiple_of(self.align),
        }
    }
}

/// The layout of the struct of `shape` in `family`.
pub fn of_shape(family: &Family, shape: &Shape, head: Head) -> Layout {
    let mut p = Placer::new();
    let head_at = p.head(head);
    let mut members = Vec::with_capacity(shape.fields.len());
    for i in family.layout_order(shape) {
        let f = shape.fields[i];
        members.push(p.member(f.key(), f.kind(), i, shape.notes[i].optional));
    }
    p.finish(head_at, members)
}

/// The layout of the struct that holds the counts of dropped items of the decoder components `decoders` of a stack
/// ([`StackShape::decoders`](super::StackShape::decoders)): one optional whole number for each decoder, in order. The
/// key of a member is the key of the field in the results of the stack, `<decoder>.dropped`.
pub fn of_dropped(decoders: &[&str]) -> Layout {
    let mut p = Placer::new();
    let members = decoders
        .iter()
        .enumerate()
        .map(|(i, d)| p.member(&format!("{d}.dropped"), ValueKind::I64, i, true))
        .collect();
    p.finish(0, members)
}

/// The layout of the struct of the arguments `args`: the arguments in their order. An optional argument has a
/// `has_<key>` member before it.
///
/// # Errors
///
/// A sentence, if an argument is of [`ValueKind::Any`]: an argument of a typed command has one type.
pub fn of_args(args: &ArgShape) -> Result<Layout, String> {
    let mut p = Placer::new();
    let mut members = Vec::with_capacity(args.args.len());
    for (i, a) in args.args.iter().enumerate() {
        let decl = a.decl();
        if decl.field.kind == ValueKind::Any {
            return Err(format!(
                "the argument {} of {} is of any kind",
                a.key(),
                args.name
            ));
        }
        members.push(p.member(a.key(), decl.field.kind, i, !decl.required));
    }
    Ok(p.finish(0, members))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_position_struct_is_three_numbers_and_a_flag() {
        assert_eq!(CType::Position.size_align(), (32, 8));
    }
}
