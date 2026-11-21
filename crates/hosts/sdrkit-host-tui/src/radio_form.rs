//! The form that opens a radio: the user chooses the kind of radio, then fills the parts of its address (host, port).
//! The library declares the parts and builds the address: the `address` command of the object `driver.<key>`.

use crate::api::{Decl, Handle, Id, Type, Value};
use crate::find::Find;
use crate::format;
use crate::results::Results;
use ratatui::crossterm::event::KeyCode;

/// The key of the command that builds an address from its parts.
const ADDRESS_COMMAND: &str = "address";

/// One kind of radio, from the engine's `drivers` table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kind {
    /// The driver key, or empty for the line that takes a whole address.
    pub key: String,
    /// The name for people.
    pub name: String,
    /// The address forms that the driver opens.
    pub forms: String,
}

/// One part of an address.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// The declaration: label, type, default, range and help.
    pub decl: Decl,
    /// The text that the user typed, or the default. Empty leaves the part out.
    pub text: String,
    /// The row keys that the part chooses from, for a part that refers to a table.
    pub rows: Vec<String>,
}

/// The step of the form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The user chooses the kind of radio.
    Kind,
    /// The user fills the parts of the address.
    Parts,
}

/// How the parts make the address.
#[derive(Debug)]
enum Compose {
    /// The `address` command of the kind's object.
    Command { kind: Handle, id: Id },
    /// The driver key, a colon and the text of the one part.
    Prefix(String),
    /// The text of the one part.
    Whole,
}

/// What a key did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The form stays open.
    Stay,
    /// The user closed the form.
    Close,
    /// The user asked to open the radio at this address.
    Open(String),
}

/// The form.
#[derive(Debug)]
pub struct RadioForm {
    /// The step.
    pub step: Step,
    /// The kinds of radio.
    pub kinds: Vec<Kind>,
    /// The chosen kind.
    pub kind: usize,
    /// The parts of the address of the chosen kind.
    pub parts: Vec<Part>,
    /// The chosen part.
    pub part: usize,
    /// The address that the parts make, or the reason they make none.
    pub address: Result<String, String>,
    /// The error of the last attempt to open the radio, or empty.
    pub error: String,
    /// The text typed to search the kinds.
    pub find: Find,
    compose: Compose,
}

fn text_cell(r: &Results, table: usize, row: usize, column: Option<usize>) -> String {
    match column.and_then(|c| r.tables[table].rows[row].cells.get(c)) {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn address_part(help: &str) -> Part {
    Part {
        decl: Decl {
            key: "address".into(),
            label: "Address".into(),
            help: help.into(),
            ty: Type::String,
            required: true,
            ..Decl::default()
        },
        text: String::new(),
        rows: Vec::new(),
    }
}

impl RadioForm {
    /// Makes the form with the kinds of radio of `engine`, the results of the engine.
    pub fn new(engine: &Results) -> RadioForm {
        let mut kinds = Vec::new();
        if let Some(t) = engine.table_id("drivers") {
            let name = engine.column_id(t, "name");
            let forms = engine.column_id(t, "address");
            for (i, row) in engine.tables[t].rows.iter().enumerate() {
                kinds.push(Kind {
                    key: row.key.clone(),
                    name: text_cell(engine, t, i, name),
                    forms: text_cell(engine, t, i, forms),
                });
            }
        }
        kinds.push(Kind {
            key: String::new(),
            name: "Other: type the whole address".into(),
            forms: "<driver>:<address>".into(),
        });
        RadioForm {
            step: Step::Kind,
            kinds,
            kind: 0,
            parts: Vec::new(),
            part: 0,
            address: Err(String::new()),
            error: String::new(),
            find: Find::default(),
            compose: Compose::Whole,
        }
    }

    /// Returns the text of each kind for the search: the driver key, then the name.
    pub fn kind_lines(&self) -> Vec<String> {
        let line = |k: &Kind| format!("{} {}", k.key, k.name);
        self.kinds.iter().map(line).collect()
    }

    /// Reads the parts of the chosen kind from its object and shows them. A kind whose object declares no parts gets
    /// one part, the address after the driver key.
    fn choose_kind(&mut self, engine: &Handle, results: &Results) {
        let Some(k) = self.kinds.get(self.kind) else {
            return;
        };
        self.step = Step::Parts;
        self.part = 0;
        self.error.clear();
        if k.key.is_empty() {
            self.parts = vec![address_part(
                "The driver key, a colon and the address: iiod:192.168.2.1",
            )];
            self.compose = Compose::Whole;
            self.address = self.composed();
            return;
        }
        let declared = engine.find(&format!("driver.{}", k.key)).and_then(|h| {
            let c = h.commands()?;
            Ok((h, c))
        });
        let command = declared.ok().and_then(|(h, commands)| {
            let c = commands.into_iter().find(|c| c.key == ADDRESS_COMMAND)?;
            Some((h, c))
        });
        match command {
            Some((kind, c)) => {
                self.parts = c
                    .args
                    .into_iter()
                    .map(|decl| {
                        let text = match &decl.default {
                            Value::None => String::new(),
                            v => format::edit_text(v, &decl),
                        };
                        let rows = if decl.refers_table.is_empty() {
                            Vec::new()
                        } else {
                            results
                                .table_id(&decl.refers_table)
                                .map(|t| {
                                    let rows = &results.tables[t].rows;
                                    rows.iter().map(|r| r.key.clone()).collect()
                                })
                                .unwrap_or_default()
                        };
                        Part { decl, text, rows }
                    })
                    .collect();
                self.compose = Compose::Command { kind, id: c.id };
            }
            None => {
                self.parts = vec![address_part(&format!("One of: {}", k.forms))];
                self.compose = Compose::Prefix(k.key.clone());
            }
        }
        self.address = self.composed();
    }

    /// Returns the address that the parts make.
    fn composed(&self) -> Result<String, String> {
        let one = || {
            let t = self.parts.first().map_or("", |p| p.text.trim());
            if t.is_empty() {
                Err("Address: required".to_owned())
            } else {
                Ok(t.to_owned())
            }
        };
        match &self.compose {
            Compose::Whole => one(),
            Compose::Prefix(key) => one().map(|t| format!("{key}:{t}")),
            Compose::Command { kind, id } => {
                let mut args = Vec::new();
                for (i, p) in (0..).zip(&self.parts) {
                    if p.text.trim().is_empty() {
                        if p.decl.required {
                            return Err(format!("{}: required", p.decl.label));
                        }
                        continue;
                    }
                    let v = format::parse(p.text.trim(), &p.decl)
                        .map_err(|e| format!("{}: {e}", p.decl.label))?;
                    args.push((i, v));
                }
                let reply = kind.invoke(*id, &args).map_err(|e| e.to_string())?;
                match reply.into_iter().next() {
                    Some(Value::String(s)) if !s.is_empty() => Ok(s),
                    _ => Err("the library returned no address".to_owned()),
                }
            }
        }
    }

    /// Sets the chosen part to the option, the row or the switch state `step` places after the current one. Returns
    /// false for a part that is plain text.
    fn cycle(&mut self, step: isize) -> bool {
        let Some(p) = self.parts.get_mut(self.part) else {
            return false;
        };
        let options: Vec<&str> = if p.decl.ty == Type::Bool {
            vec!["no", "yes"]
        } else if p.decl.ty == Type::Enum {
            p.decl.options.iter().map(|o| o.key.as_str()).collect()
        } else {
            p.rows.iter().map(String::as_str).collect()
        };
        if options.is_empty() {
            return false;
        }
        let n = options.len() as isize;
        let next = match options.iter().position(|o| *o == p.text) {
            Some(i) => (i as isize + step).rem_euclid(n),
            None if step < 0 => n - 1,
            None => 0,
        };
        p.text = options[next as usize].to_owned();
        true
    }

    /// Carries out key `code`. `engine` and `results` are the engine's handle and results.
    pub fn key(&mut self, code: KeyCode, engine: &Handle, results: &Results) -> Outcome {
        match self.step {
            Step::Kind => match code {
                KeyCode::Esc => return Outcome::Close,
                KeyCode::Enter | KeyCode::Right => self.choose_kind(engine, results),
                code if Find::takes(code) => {
                    let line = self.find.key(code, &self.kind_lines());
                    self.kind = line.unwrap_or(self.kind);
                }
                c => {
                    self.find.clear();
                    crate::app::move_sel(&mut self.kind, self.kinds.len(), c);
                }
            },
            Step::Parts => {
                let n = self.parts.len();
                match code {
                    KeyCode::Esc => {
                        self.step = Step::Kind;
                        self.error.clear();
                        self.find.clear();
                        return Outcome::Stay;
                    }
                    KeyCode::Enter => match &self.address {
                        Ok(a) => return Outcome::Open(a.clone()),
                        Err(e) => self.error = e.clone(),
                    },
                    KeyCode::Up | KeyCode::BackTab => self.part = (self.part + n - 1) % n.max(1),
                    KeyCode::Down | KeyCode::Tab => self.part = (self.part + 1) % n.max(1),
                    KeyCode::Left => {
                        self.cycle(-1);
                    }
                    KeyCode::Right => {
                        self.cycle(1);
                    }
                    KeyCode::Char(' ') if self.cycle(1) => {}
                    KeyCode::Char(c) => {
                        // A switch or an option changes with the arrows; only a text or a number is typed.
                        let typed = |p: &&mut Part| !matches!(p.decl.ty, Type::Bool | Type::Enum);
                        if let Some(p) = self.parts.get_mut(self.part).filter(typed) {
                            p.text.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        if let Some(p) = self.parts.get_mut(self.part) {
                            p.text.pop();
                        }
                    }
                    KeyCode::Delete => {
                        if let Some(p) = self.parts.get_mut(self.part) {
                            p.text.clear();
                        }
                    }
                    _ => {}
                }
                if !matches!(code, KeyCode::Enter) {
                    self.error.clear();
                    self.address = self.composed();
                }
            }
        }
        Outcome::Stay
    }
}
