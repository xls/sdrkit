//! Conditions: a small expression language over the fields of an observation, resolved against the event kinds when the
//! rule is set.
//!
//! Names are standard values (`kind`, `family`, `urgency`, `urgent`, `source`, `text`, `freq_hz`, `freq_mhz`, `lat`, `lon`,
//! `distance_km`, `bearing_deg`, `unix_time`, `signal_time`) or fields that the rule's kinds declare. Literals are numbers,
//! `'text'` or `"text"`, `true`, `false`, `null` and lists `[a, b]`. Comparisons are `==`, `!=`, `<`, `<=`, `>`, `>=`, `~`
//! (contains), `^=` (starts with), `$=` (ends with) and `in`. Text compares without regard to case. Logic is `and`, `or`,
//! `not` and parentheses. Functions are `exists(name)`, `lower(x)`, `upper(x)`, `len(x)`, `abs(x)` and
//! `within(lat, lon, km)`.

use std::borrow::Cow;
use std::cmp::Ordering;

use sdrkit_svc_events::text::{cmp_text, contains, ends, starts};
use sdrkit_svc_events::{FieldRef, KindSet, Kinds, NameError, Observation, Position, Reading};

/// What is wrong with the text of a condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syntax {
    /// A text literal has no closing quote.
    TextNotClosed,
    /// A number does not parse.
    NotANumber,
    /// A character that the language does not use.
    UnexpectedCharacter,
    /// A closing parenthesis is missing.
    NoClosingParenthesis,
    /// A comma or a closing bracket is missing in a list.
    NoListEnd,
    /// A value is missing.
    NoValue,
    /// The function is not one of the language.
    UnknownFunction,
    /// The function has the wrong number of values.
    WrongCount,
    /// `exists` takes a name.
    ExistsTakesAName,
    /// Text follows a complete condition.
    TextAfterEnd,
}

/// Why a condition does not parse.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ExprError {
    /// The text is not a condition.
    #[error("condition, at {at}: {problem:?}")]
    Syntax {
        /// The byte position.
        at: usize,
        /// What is wrong.
        problem: Syntax,
    },
    /// A name does not resolve.
    #[error("condition: {0}")]
    Name(#[from] NameError),
}

impl ExprError {
    /// The byte position of the problem in the text.
    pub fn at(&self) -> usize {
        match self {
            ExprError::Syntax { at, .. } => *at,
            ExprError::Name(
                NameError::UnknownField { at, .. }
                | NameError::UnknownKind { at, .. }
                | NameError::UnknownFamily { at, .. },
            ) => *at,
        }
    }
}

/// A value during evaluation. Text borrows from the observation or the condition where it can.
#[derive(Clone, Debug, PartialEq)]
enum Val<'a> {
    Null,
    Bool(bool),
    Num(f64),
    Str(Cow<'a, str>),
}

impl Val<'_> {
    fn truthy(&self) -> bool {
        match self {
            Val::Null => false,
            Val::Bool(b) => *b,
            Val::Num(x) => *x != 0.0 && !x.is_nan(),
            Val::Str(s) => !s.is_empty(),
        }
    }

    fn num(&self) -> Option<f64> {
        match self {
            Val::Num(x) => Some(*x),
            Val::Str(s) => s.trim().parse().ok(),
            Val::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Val::Null => None,
        }
    }
}

impl std::fmt::Display for Val<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Val::Null => Ok(()),
            Val::Bool(b) => write!(f, "{b}"),
            Val::Num(x) => Reading::Number(*x).fmt(f),
            Val::Str(s) => f.write_str(s),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
    Starts,
    Ends,
    In,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Func {
    Lower,
    Upper,
    Len,
    Abs,
    Within,
}

#[derive(Clone, Debug, PartialEq)]
enum Lit {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
}

/// A condition, resolved against the event kinds.
#[derive(Clone, Debug, PartialEq)]
pub struct Expr(Node);

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Lit(Lit),
    Field(FieldRef),
    List(Vec<Node>),
    Not(Box<Node>),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    Cmp(Box<Node>, Op, Box<Node>),
    Exists(FieldRef),
    Call(Func, Vec<Node>),
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Op(Op),
    And,
    Or,
    Not,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
}

fn syntax(at: usize, problem: Syntax) -> ExprError {
    ExprError::Syntax { at, problem }
}

fn lex(src: &str) -> Result<Vec<(usize, Tok)>, ExprError> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        let start = i;
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let two = match src.get(i..i + 2).unwrap_or("") {
            "==" => Some(Tok::Op(Op::Eq)),
            "!=" => Some(Tok::Op(Op::Ne)),
            "<=" => Some(Tok::Op(Op::Le)),
            ">=" => Some(Tok::Op(Op::Ge)),
            "^=" => Some(Tok::Op(Op::Starts)),
            "$=" => Some(Tok::Op(Op::Ends)),
            "&&" => Some(Tok::And),
            "||" => Some(Tok::Or),
            _ => None,
        };
        if let Some(t) = two {
            out.push((start, t));
            i += 2;
            continue;
        }
        let one = match c {
            b'<' => Some(Tok::Op(Op::Lt)),
            b'>' => Some(Tok::Op(Op::Gt)),
            b'=' => Some(Tok::Op(Op::Eq)),
            b'~' => Some(Tok::Op(Op::Contains)),
            b'!' => Some(Tok::Not),
            b'(' => Some(Tok::LParen),
            b')' => Some(Tok::RParen),
            b'[' => Some(Tok::LBracket),
            b']' => Some(Tok::RBracket),
            b',' => Some(Tok::Comma),
            _ => None,
        };
        if let Some(t) = one {
            out.push((start, t));
            i += 1;
            continue;
        }
        if c == b'\'' || c == b'"' {
            let mut s = String::new();
            i += 1;
            loop {
                let Some(ch) = src[i..].chars().next() else {
                    return Err(syntax(start, Syntax::TextNotClosed));
                };
                i += ch.len_utf8();
                if ch as u32 == u32::from(c) {
                    break;
                }
                if ch == '\\' {
                    let Some(n) = src[i..].chars().next() else {
                        return Err(syntax(start, Syntax::TextNotClosed));
                    };
                    i += n.len_utf8();
                    s.push(n);
                } else {
                    s.push(ch);
                }
            }
            out.push((start, Tok::Str(s)));
            continue;
        }
        if c.is_ascii_digit()
            || (matches!(c, b'-' | b'.') && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            i += 1;
            while i < b.len()
                && (b[i].is_ascii_digit()
                    || matches!(b[i], b'.' | b'e' | b'E')
                    || (matches!(b[i], b'-' | b'+') && matches!(b[i - 1], b'e' | b'E')))
            {
                i += 1;
            }
            let n: f64 = src[start..i]
                .parse()
                .map_err(|_| syntax(start, Syntax::NotANumber))?;
            out.push((start, Tok::Num(n)));
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'_' | b'.')) {
                i += 1;
            }
            let word = &src[start..i];
            out.push((
                start,
                match word.to_ascii_lowercase().as_str() {
                    "and" => Tok::And,
                    "or" => Tok::Or,
                    "not" => Tok::Not,
                    "in" => Tok::Op(Op::In),
                    _ => Tok::Ident(word.to_owned()),
                },
            ));
            continue;
        }
        return Err(syntax(start, Syntax::UnexpectedCharacter));
    }
    Ok(out)
}

struct Parser<'k> {
    toks: Vec<(usize, Tok)>,
    at: usize,
    len: usize,
    kinds: &'k Kinds,
    within: Option<&'k KindSet>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at).map(|t| &t.1)
    }
    fn pos(&self) -> usize {
        self.toks.get(self.at).map_or(self.len, |t| t.0)
    }
    fn next(&mut self) -> Option<(usize, Tok)> {
        let t = self.toks.get(self.at).cloned();
        self.at += 1;
        t
    }
    fn or(&mut self) -> Result<Node, ExprError> {
        let mut l = self.and()?;
        while self.peek() == Some(&Tok::Or) {
            self.at += 1;
            l = Node::Or(Box::new(l), Box::new(self.and()?));
        }
        Ok(l)
    }
    fn and(&mut self) -> Result<Node, ExprError> {
        let mut l = self.not()?;
        while self.peek() == Some(&Tok::And) {
            self.at += 1;
            l = Node::And(Box::new(l), Box::new(self.not()?));
        }
        Ok(l)
    }
    fn not(&mut self) -> Result<Node, ExprError> {
        if self.peek() == Some(&Tok::Not) {
            self.at += 1;
            return Ok(Node::Not(Box::new(self.not()?)));
        }
        let l = self.value()?;
        if let Some(Tok::Op(op)) = self.peek().cloned() {
            self.at += 1;
            let r = self.value()?;
            return Ok(Node::Cmp(Box::new(l), op, Box::new(r)));
        }
        Ok(l)
    }
    fn list(&mut self, close: &Tok) -> Result<Vec<Node>, ExprError> {
        let mut items = Vec::new();
        if self.peek() == Some(close) {
            self.at += 1;
            return Ok(items);
        }
        loop {
            items.push(self.or()?);
            match self.next() {
                Some((_, Tok::Comma)) => continue,
                Some((_, t)) if t == *close => return Ok(items),
                _ => {
                    self.at -= 1;
                    return Err(syntax(self.pos(), Syntax::NoListEnd));
                }
            }
        }
    }
    fn value(&mut self) -> Result<Node, ExprError> {
        let Some((at, tok)) = self.next() else {
            return Err(syntax(self.len, Syntax::NoValue));
        };
        match tok {
            Tok::Num(n) => Ok(Node::Lit(Lit::Num(n))),
            Tok::Str(s) => Ok(Node::Lit(Lit::Str(s))),
            Tok::LParen => {
                let e = self.or()?;
                if self.peek() != Some(&Tok::RParen) {
                    return Err(syntax(self.pos(), Syntax::NoClosingParenthesis));
                }
                self.at += 1;
                Ok(e)
            }
            Tok::LBracket => Ok(Node::List(self.list(&Tok::RBracket)?)),
            Tok::Ident(w) => match w.to_ascii_lowercase().as_str() {
                "true" => Ok(Node::Lit(Lit::Bool(true))),
                "false" => Ok(Node::Lit(Lit::Bool(false))),
                "null" => Ok(Node::Lit(Lit::Null)),
                f if self.peek() == Some(&Tok::LParen) => {
                    self.at += 1;
                    let (func, want) = match f {
                        "exists" => (None, 1),
                        "lower" => (Some(Func::Lower), 1),
                        "upper" => (Some(Func::Upper), 1),
                        "len" => (Some(Func::Len), 1),
                        "abs" => (Some(Func::Abs), 1),
                        "within" => (Some(Func::Within), 3),
                        _ => return Err(syntax(at, Syntax::UnknownFunction)),
                    };
                    let args = self.list(&Tok::RParen)?;
                    if args.len() != want {
                        return Err(syntax(at, Syntax::WrongCount));
                    }
                    match func {
                        Some(func) => Ok(Node::Call(func, args)),
                        None => match args.into_iter().next() {
                            Some(Node::Field(f)) => Ok(Node::Exists(f)),
                            _ => Err(syntax(at, Syntax::ExistsTakesAName)),
                        },
                    }
                }
                _ => Ok(Node::Field(self.kinds.field(&w, at, self.within)?)),
            },
            _ => {
                self.at -= 1;
                Err(syntax(at, Syntax::NoValue))
            }
        }
    }
}

impl Expr {
    /// The condition of `src`, its names resolved against the kinds of `within` (every kind if `None`). Empty text is always
    /// true.
    ///
    /// # Errors
    ///
    /// [`ExprError`] with the byte position if the text does not parse or a name does not resolve.
    pub fn parse(src: &str, kinds: &Kinds, within: Option<&KindSet>) -> Result<Expr, ExprError> {
        if src.trim().is_empty() {
            return Ok(Expr(Node::Lit(Lit::Bool(true))));
        }
        let mut p = Parser {
            toks: lex(src)?,
            at: 0,
            len: src.len(),
            kinds,
            within,
        };
        let e = p.or()?;
        if p.at < p.toks.len() {
            return Err(syntax(p.pos(), Syntax::TextAfterEnd));
        }
        Ok(Expr(e))
    }

    /// True if the condition holds for `o`.
    pub fn holds(&self, o: &Observation<'_>) -> bool {
        eval(&self.0, o).truthy()
    }
}

fn reading(r: Reading<'_>) -> Val<'_> {
    match r {
        Reading::Number(x) => Val::Num(x),
        Reading::Text(t) => Val::Str(Cow::Borrowed(t)),
        Reading::Bool(b) => Val::Bool(b),
        Reading::Position(..) | Reading::Bytes(_) => Val::Str(Cow::Owned(r.to_string())),
    }
}

fn eval<'a>(n: &'a Node, o: &Observation<'a>) -> Val<'a> {
    match n {
        Node::Lit(Lit::Null) | Node::List(_) => Val::Null,
        Node::Lit(Lit::Bool(b)) => Val::Bool(*b),
        Node::Lit(Lit::Num(x)) => Val::Num(*x),
        Node::Lit(Lit::Str(s)) => Val::Str(Cow::Borrowed(s)),
        Node::Field(f) => o.read(f).map_or(Val::Null, reading),
        Node::Exists(f) => Val::Bool(o.read(f).is_some()),
        Node::Not(e) => Val::Bool(!eval(e, o).truthy()),
        Node::And(a, b) => Val::Bool(eval(a, o).truthy() && eval(b, o).truthy()),
        Node::Or(a, b) => Val::Bool(eval(a, o).truthy() || eval(b, o).truthy()),
        Node::Cmp(a, Op::In, b) => {
            let a = eval(a, o);
            Val::Bool(match &**b {
                Node::List(items) => items.iter().any(|i| compare(&a, Op::Eq, &eval(i, o))),
                other => match eval(other, o) {
                    Val::Str(s) if !matches!(a, Val::Null) => contains(&s, &a.to_string()),
                    _ => false,
                },
            })
        }
        Node::Cmp(a, op, b) => Val::Bool(compare(&eval(a, o), *op, &eval(b, o))),
        Node::Call(f, args) => {
            let arg = |i: usize| args.get(i).map_or(Val::Null, |a| eval(a, o));
            match f {
                Func::Lower => Val::Str(Cow::Owned(arg(0).to_string().to_lowercase())),
                Func::Upper => Val::Str(Cow::Owned(arg(0).to_string().to_uppercase())),
                Func::Len => match arg(0) {
                    Val::Null => Val::Num(0.0),
                    Val::Str(s) => Val::Num(s.chars().count() as f64),
                    v => Val::Num(v.to_string().chars().count() as f64),
                },
                Func::Abs => arg(0).num().map_or(Val::Null, |x| Val::Num(x.abs())),
                Func::Within => match (o.position, arg(0).num(), arg(1).num(), arg(2).num()) {
                    (Some(p), Some(lat), Some(lon), Some(km)) => {
                        Val::Bool(Position::new(lat, lon).distance_m(&p) <= km * 1000.0)
                    }
                    _ => Val::Bool(false),
                },
            }
        }
    }
}

fn compare(a: &Val<'_>, op: Op, b: &Val<'_>) -> bool {
    if matches!(a, Val::Null) || matches!(b, Val::Null) {
        let both = matches!((a, b), (Val::Null, Val::Null));
        return match op {
            Op::Eq => both,
            Op::Ne => !both,
            _ => false,
        };
    }
    let nums = match (a, b) {
        (Val::Num(_), _) | (_, Val::Num(_)) => a.num().zip(b.num()),
        _ => None,
    };
    let text = |f: fn(&str, &str) -> bool| match (a, b) {
        (Val::Str(x), Val::Str(y)) => f(x, y),
        _ => f(&a.to_string(), &b.to_string()),
    };
    let order = || match nums {
        Some((x, y)) => x.partial_cmp(&y),
        None => Some(match (a, b) {
            (Val::Str(x), Val::Str(y)) => cmp_text(x, y),
            _ => cmp_text(&a.to_string(), &b.to_string()),
        }),
    };
    match op {
        Op::Eq => order() == Some(Ordering::Equal),
        Op::Ne => order() != Some(Ordering::Equal),
        Op::Lt => order() == Some(Ordering::Less),
        Op::Le => matches!(order(), Some(Ordering::Less | Ordering::Equal)),
        Op::Gt => order() == Some(Ordering::Greater),
        Op::Ge => matches!(order(), Some(Ordering::Greater | Ordering::Equal)),
        Op::Contains => text(contains),
        Op::Starts => text(starts),
        Op::Ends => text(ends),
        Op::In => false,
    }
}
