//! Text templates in user text: `{name}` is the value of a field, `{name|fallback}` returns the fallback (a template too) when
//! the field has no value, and `{{` and `}}` are single braces. Names resolve against the event kinds when the template is
//! set.

use std::fmt::Write as _;

use crate::kinds::{FieldRef, KindSet, Kinds, NameError};
use crate::observation::{Observation, Reading};

/// One part of a template.
#[derive(Clone, Debug, PartialEq)]
enum Part {
    Text(String),
    Field(FieldRef, Template),
}

/// A template, resolved against the event kinds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Template {
    parts: Vec<Part>,
}

/// The position of the brace that closes the one `s` starts with, counting nested pairs.
fn closing(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

impl Template {
    /// The template of `src`, its names resolved against the kinds of `within` (every kind if `None`). `at` is the byte
    /// position of `src` in the user's text.
    ///
    /// # Errors
    ///
    /// [`NameError::UnknownField`] with the position of the first name that no kind declares.
    pub fn parse(
        src: &str,
        at: usize,
        kinds: &Kinds,
        within: Option<&KindSet>,
    ) -> Result<Template, NameError> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut i = 0;
        while let Some(j) = src[i..].find(['{', '}']).map(|j| i + j) {
            text.push_str(&src[i..j]);
            let tail = &src[j..];
            if tail.starts_with("{{") || tail.starts_with("}}") {
                text.push_str(&tail[..1]);
                i = j + 2;
                continue;
            }
            if tail.starts_with('}') {
                text.push('}');
                i = j + 1;
                continue;
            }
            let Some(end) = closing(tail) else {
                text.push_str(tail);
                i = src.len();
                break;
            };
            let inner = &tail[1..end];
            let (name, fallback) = inner.split_once('|').unwrap_or((inner, ""));
            let name_at = at + j + 1 + (name.len() - name.trim_start().len());
            let field = kinds.field(name.trim(), name_at, within)?;
            let fallback_at = at + j + 2 + name.len();
            let fallback = Template::parse(fallback, fallback_at, kinds, within)?;
            if !text.is_empty() {
                parts.push(Part::Text(std::mem::take(&mut text)));
            }
            parts.push(Part::Field(field, fallback));
            i = j + end + 1;
        }
        text.push_str(&src[i..]);
        if !text.is_empty() {
            parts.push(Part::Text(text));
        }
        Ok(Template { parts })
    }

    /// True if the template is empty.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Writes the template filled from `o` at the end of `out`.
    pub fn fill_into(&self, o: &Observation<'_>, out: &mut String) {
        for p in &self.parts {
            match p {
                Part::Text(t) => out.push_str(t),
                Part::Field(f, fallback) => match o.read(f) {
                    Some(r) => {
                        let _ = write!(out, "{r}");
                    }
                    None => fallback.fill_into(o, out),
                },
            }
        }
    }

    /// Writes the template filled from `o` at the end of `out`, each field value through `value`. `value` takes `None` for
    /// a field with no value and no fallback.
    pub fn fill_mapped(
        &self,
        o: &Observation<'_>,
        out: &mut String,
        value: &dyn Fn(Option<Reading<'_>>, &mut String),
    ) {
        for p in &self.parts {
            match p {
                Part::Text(t) => out.push_str(t),
                Part::Field(f, fallback) => match o.read(f) {
                    Some(r) => value(Some(r), out),
                    None if !fallback.is_empty() => fallback.fill_mapped(o, out, value),
                    None => value(None, out),
                },
            }
        }
    }

    /// The template filled from `o`.
    pub fn fill(&self, o: &Observation<'_>) -> String {
        let mut out = String::new();
        self.fill_into(o, &mut out);
        out
    }
}
