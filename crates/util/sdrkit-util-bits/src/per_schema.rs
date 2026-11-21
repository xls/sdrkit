//! ASN.1 types described as data, and one walker that reads, writes and prints any of them in unaligned PER.
//!
//! A protocol module writes its types as static [`Ty`] tables (CHOICE, SEQUENCE, INTEGER range with a print scale, ENUMERATED
//! names, string sizes) and labels its components with [`Field`]. [`walk`] reads such a type into a [`Value`] tree with a
//! [`crate::per::PerReader`], or writes a tree with a [`crate::per::PerWriter`]: the same table serves both. [`print_field`]
//! writes a tree as indented "label: value" lines.
//!
//! Extension markers are not supported: every type here is a root-only type.
//!
//! ```
//! use sdrkit_util_bits::per::{PerReader, PerWriter};
//! use sdrkit_util_bits::per_schema::{walk, Field, Num, Ty, Value, print_field};
//! static LEVEL: Ty = Ty::Int { lo: 30, hi: 600, num: Num::PLAIN };
//! static FEET: Ty = Ty::Int { lo: 0, hi: 2500, num: Num::new(10, 0, "ft") };
//! static ALT: Ty = Ty::Choice(&[Field::new("Altitude", &FEET), Field::new("Flight level", &LEVEL)]);
//! let mut v = Value::Choice(1, Box::new(Value::Int(350)));
//! let mut out = Vec::new();
//! let mut w = PerWriter::new(&mut out);
//! walk(&mut w, &ALT, &mut v);
//! w.finish();
//! let mut r = PerReader::new(&out);
//! let mut back = Value::Null;
//! walk(&mut r, &ALT, &mut back);
//! let mut text = String::new();
//! print_field(&Field::new("", &ALT), &back, 0, &mut text);
//! assert_eq!(text, "Flight level: 350\n");
//! ```

use crate::per::PerIo;
use std::fmt::Write as _;

/// How a whole number is printed: the value times `mul`, shown with `dec` decimals, then the unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Num {
    /// Multiplier applied to the coded value.
    pub mul: i64,
    /// Decimals: the product is divided by 10^dec.
    pub dec: u8,
    /// Unit written after the number (none when empty).
    pub unit: &'static str,
}

impl Num {
    /// The number as coded, without a unit.
    pub const PLAIN: Num = Num::new(1, 0, "");

    /// A print scale.
    pub const fn new(mul: i64, dec: u8, unit: &'static str) -> Num {
        Num { mul, dec, unit }
    }

    /// Appends the printed form of `v` to `out`.
    pub fn write(&self, v: i64, out: &mut String) {
        let x = v.saturating_mul(self.mul);
        if self.dec == 0 {
            let _ = write!(out, "{x}");
        } else {
            let p = 10i64.pow(u32::from(self.dec));
            let sign = if x < 0 { "-" } else { "" };
            let a = x.unsigned_abs();
            let p = p as u64;
            let _ = write!(out, "{sign}{}.{:0w$}", a / p, a % p, w = usize::from(self.dec));
        }
        if !self.unit.is_empty() {
            out.push(' ');
            out.push_str(self.unit);
        }
    }
}

/// A component of a SEQUENCE or an alternative of a CHOICE: its label and type.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    /// Label in the printed form; empty prints the content without a label.
    pub label: &'static str,
    /// The type.
    pub ty: &'static Ty,
    /// OPTIONAL (SEQUENCE components only).
    pub optional: bool,
}

impl Field {
    /// A mandatory component or a CHOICE alternative.
    pub const fn new(label: &'static str, ty: &'static Ty) -> Field {
        Field {
            label,
            ty,
            optional: false,
        }
    }

    /// An OPTIONAL component.
    pub const fn opt(label: &'static str, ty: &'static Ty) -> Field {
        Field {
            label,
            ty,
            optional: true,
        }
    }
}

/// Prints a value on one line.
pub type ShowFn = fn(&Value, &mut String);

/// An ASN.1 type, unaligned PER, no extension markers.
#[derive(Debug)]
pub enum Ty {
    /// NULL: no bits.
    Null,
    /// BOOLEAN: one bit; printed "yes" or "no".
    Bool,
    /// INTEGER (lo..hi), printed with `num`.
    Int {
        /// Lower bound.
        lo: i64,
        /// Upper bound.
        hi: i64,
        /// Print scale.
        num: Num,
    },
    /// ENUMERATED: the names of the values in order.
    Enum(&'static [&'static str]),
    /// CHOICE: the alternatives in order.
    Choice(&'static [Field]),
    /// SEQUENCE: the components in order.
    Seq(&'static [Field]),
    /// SEQUENCE SIZE (lo..hi) OF the item.
    List {
        /// Fewest items.
        lo: usize,
        /// Most items.
        hi: usize,
        /// The item, with its label.
        item: &'static Field,
    },
    /// IA5String SIZE (lo..hi): 7 bits a character.
    Ia5 {
        /// Shortest.
        lo: usize,
        /// Longest.
        hi: usize,
    },
    /// NumericString SIZE (lo..hi): 4 bits a character, alphabet space and 0 to 9.
    Digits {
        /// Shortest.
        lo: usize,
        /// Longest.
        hi: usize,
    },
    /// A type printed on one line by a function (a time, a coordinate, a code of several digits).
    Show(&'static Ty, ShowFn),
}

/// A decoded value. Its shape follows the [`Ty`] it was read with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Value {
    /// NULL, or a value not yet read.
    #[default]
    Null,
    /// BOOLEAN.
    Bool(bool),
    /// INTEGER (the coded value, without the print scale).
    Int(i64),
    /// ENUMERATED index.
    Enum(usize),
    /// CHOICE: the alternative and its value.
    Choice(usize, Box<Value>),
    /// SEQUENCE: one entry a component, `None` for an absent OPTIONAL.
    Seq(Vec<Option<Value>>),
    /// SEQUENCE OF.
    List(Vec<Value>),
    /// A string.
    Text(String),
}

impl Value {
    /// The whole number, when this is one.
    pub fn int(&self) -> Option<i64> {
        match self {
            Value::Int(x) => Some(*x),
            _ => None,
        }
    }

    /// The component `i` of a SEQUENCE, when present.
    pub fn get(&self, i: usize) -> Option<&Value> {
        match self {
            Value::Seq(v) => v.get(i).and_then(Option::as_ref),
            _ => None,
        }
    }
}

const NUMERIC: &[u8; 11] = b" 0123456789";

fn string<P: PerIo>(p: &mut P, v: &mut Value, lo: usize, hi: usize, digits: bool) {
    let s = match std::mem::take(v) {
        Value::Text(s) => s,
        _ => String::new(),
    };
    let mut n = s.len();
    p.count(&mut n, lo, hi);
    let width = if digits { 4 } else { 7 };
    if p.reading() {
        let mut out = String::with_capacity(n);
        for _ in 0..n {
            if p.failed() {
                break;
            }
            let mut c = 0;
            p.bits(&mut c, width);
            if digits {
                match NUMERIC.get(c as usize) {
                    Some(&d) => out.push(char::from(d)),
                    None => p.fail(),
                }
            } else {
                out.push(char::from(c as u8));
            }
        }
        *v = Value::Text(out);
    } else {
        for b in s.bytes() {
            let mut c = if digits {
                match NUMERIC.iter().position(|&d| d == b) {
                    Some(i) => i as u64,
                    None => {
                        p.fail();
                        0
                    }
                }
            } else {
                if !b.is_ascii() {
                    p.fail();
                }
                u64::from(b & 0x7F)
            };
            p.bits(&mut c, width);
        }
        *v = Value::Text(s);
    }
}

/// Reads (with a reader) or writes (with a writer) `v` as type `ty`. A reader replaces `v`; after a failure it stops reading and
/// leaves the rest as [`Value::Null`]. A writer writes the lowest value of a part whose value does not fit its type, and marks
/// the writer as failed.
pub fn walk<P: PerIo>(p: &mut P, ty: &Ty, v: &mut Value) {
    if p.reading() && p.failed() {
        return;
    }
    let reading = p.reading();
    match ty {
        Ty::Null => {
            if reading {
                *v = Value::Null;
            }
        }
        Ty::Bool => {
            let mut b = matches!(v, Value::Bool(true));
            p.flag(&mut b);
            *v = Value::Bool(b);
        }
        Ty::Int { lo, hi, .. } => {
            let mut x = match v {
                Value::Int(x) if !reading => *x,
                _ => *lo,
            };
            if !reading && !(*lo..=*hi).contains(&x) {
                p.fail();
            }
            p.int(&mut x, *lo, *hi);
            *v = Value::Int(x);
        }
        Ty::Enum(names) => {
            let mut i = match v {
                Value::Enum(i) if !reading => *i,
                _ => 0,
            };
            if !reading && i >= names.len() {
                p.fail();
                i = 0;
            }
            p.enumerated(&mut i, names.len(), false);
            *v = Value::Enum(i);
        }
        Ty::Choice(alts) => {
            let (mut i, mut inner) = match std::mem::take(v) {
                Value::Choice(i, b) if !reading => (i, *b),
                _ => (0, Value::Null),
            };
            if !reading && i >= alts.len() {
                p.fail();
                i = 0;
            }
            p.choice(&mut i, alts.len(), false);
            match alts.get(i) {
                Some(a) => walk(p, a.ty, &mut inner),
                None => p.fail(),
            }
            *v = Value::Choice(i, Box::new(inner));
        }
        Ty::Seq(fields) => {
            let mut items = match std::mem::take(v) {
                Value::Seq(x) if !reading => x,
                _ => Vec::new(),
            };
            items.resize_with(fields.len(), || None);
            for (f, it) in fields.iter().zip(items.iter_mut()) {
                if f.optional {
                    let mut present = it.is_some();
                    p.flag(&mut present);
                    if reading {
                        *it = present.then_some(Value::Null);
                    }
                } else if it.is_none() {
                    *it = Some(Value::Null);
                }
            }
            for (f, it) in fields.iter().zip(items.iter_mut()) {
                if let Some(x) = it {
                    walk(p, f.ty, x);
                }
            }
            *v = Value::Seq(items);
        }
        Ty::List { lo, hi, item } => {
            let mut items = match std::mem::take(v) {
                Value::List(x) if !reading => x,
                _ => Vec::new(),
            };
            let mut n = items.len();
            if !reading && !(*lo..=*hi).contains(&n) {
                p.fail();
            }
            p.count(&mut n, *lo, *hi);
            if reading {
                items.reserve(n);
                for _ in 0..n {
                    if p.failed() {
                        break;
                    }
                    let mut x = Value::Null;
                    walk(p, item.ty, &mut x);
                    items.push(x);
                }
            } else {
                for x in &mut items {
                    walk(p, item.ty, x);
                }
            }
            *v = Value::List(items);
        }
        Ty::Ia5 { lo, hi } => string(p, v, *lo, *hi, false),
        Ty::Digits { lo, hi } => string(p, v, *lo, *hi, true),
        Ty::Show(inner, _) => walk(p, inner, v),
    }
}

fn indent(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push(' ');
    }
}

/// True for a type printed on one line.
pub fn is_leaf(ty: &Ty) -> bool {
    !matches!(ty, Ty::Choice(_) | Ty::Seq(_) | Ty::List { .. } | Ty::Null)
}

/// Appends the one-line form of a leaf value: the scaled number and unit, the ENUMERATED name, the string, "yes" or "no", or
/// what the [`Ty::Show`] function writes. Nothing for a container.
pub fn write_leaf(ty: &Ty, v: &Value, out: &mut String) {
    match (ty, v) {
        (Ty::Show(_, f), v) => f(v, out),
        (Ty::Int { num, .. }, Value::Int(x)) => num.write(*x, out),
        (Ty::Enum(names), Value::Enum(i)) => out.push_str(names.get(*i).copied().unwrap_or("?")),
        (Ty::Bool, Value::Bool(b)) => out.push_str(if *b { "yes" } else { "no" }),
        (Ty::Ia5 { .. } | Ty::Digits { .. }, Value::Text(s)) => out.push_str(s),
        _ => {}
    }
}

/// Appends `v`, read as the type of `f`, as indented lines (one space a level, starting at `depth`). A leaf prints
/// "label: value" (only the value when the label is empty). A labelled SEQUENCE, SEQUENCE OF or CHOICE prints "label:" and its
/// content one level deeper; an unlabelled one prints its content at the same level. A CHOICE prints only its alternative; an
/// absent OPTIONAL prints nothing; NULL prints its label alone, or nothing.
pub fn print_field(f: &Field, v: &Value, depth: usize, out: &mut String) {
    if is_leaf(f.ty) {
        indent(depth, out);
        if !f.label.is_empty() {
            out.push_str(f.label);
            out.push_str(": ");
        }
        write_leaf(f.ty, v, out);
        out.push('\n');
        return;
    }
    if matches!(f.ty, Ty::Null) {
        if !f.label.is_empty() {
            indent(depth, out);
            out.push_str(f.label);
            out.push('\n');
        }
        return;
    }
    let inner = if f.label.is_empty() {
        depth
    } else {
        indent(depth, out);
        out.push_str(f.label);
        out.push_str(":\n");
        depth + 1
    };
    match (f.ty, v) {
        (Ty::Choice(alts), Value::Choice(i, x)) => {
            if let Some(a) = alts.get(*i) {
                print_field(a, x, inner, out);
            }
        }
        (Ty::Seq(fields), Value::Seq(items)) => {
            for (c, x) in fields.iter().zip(items) {
                if let Some(x) = x {
                    print_field(c, x, inner, out);
                }
            }
        }
        (Ty::List { item, .. }, Value::List(items)) => {
            for x in items {
                print_field(item, x, inner, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::per::{PerReader, PerWriter};

    static NAME: Ty = Ty::Ia5 { lo: 1, hi: 5 };
    static CH: Ty = Ty::Digits { lo: 3, hi: 3 };
    static DIR: Ty = Ty::Enum(&["left", "right"]);
    static SEQ: Ty = Ty::Seq(&[
        Field::new("Name", &NAME),
        Field::opt("Channel", &CH),
        Field::opt("Direction", &DIR),
    ]);
    static LIST: Ty = Ty::List {
        lo: 1,
        hi: 4,
        item: &Field::new("Item", &SEQ),
    };

    #[test]
    fn round_trip_strings_lists_and_optionals() {
        let one = |n: &str, ch: Option<&str>| {
            Value::Seq(vec![
                Some(Value::Text(n.into())),
                ch.map(|c| Value::Text(c.into())),
                Some(Value::Enum(1)),
            ])
        };
        let mut v = Value::List(vec![one("AB", Some("123")), one("XYZ", None)]);
        let mut out = Vec::new();
        let mut w = PerWriter::new(&mut out);
        walk(&mut w, &LIST, &mut v);
        assert!(!w.failed());
        // 2 (count) + 2 * (2 presence + 3 length + 1 direction) + 2 * 7 + 3 * 4 + 3 * 7 bits.
        assert_eq!(w.bits_written(), 2 + 12 + 14 + 12 + 21);
        w.finish();
        let mut r = PerReader::new(&out);
        let mut back = Value::Null;
        walk(&mut r, &LIST, &mut back);
        assert!(!r.failed());
        assert_eq!(back, v);
        let mut text = String::new();
        print_field(&Field::new("List", &LIST), &back, 0, &mut text);
        assert!(text.contains(" Item:\n  Name: AB\n  Channel: 123\n  Direction: right\n"));
        // Every truncation fails and stops.
        for n in 0..out.len() - 1 {
            let mut r = PerReader::new(&out[..n]);
            let mut x = Value::Null;
            walk(&mut r, &LIST, &mut x);
            assert!(r.failed(), "{n}");
        }
    }

    #[test]
    fn numbers_print_with_their_scale() {
        let mut s = String::new();
        Num::new(1, 3, "MHz").write(121_500, &mut s);
        s.push(' ');
        Num::new(1, 1, "nm").write(5, &mut s);
        s.push(' ');
        Num::new(1, 0, "C").write(-56, &mut s);
        assert_eq!(s, "121.500 MHz 0.5 nm -56 C");
    }
}
