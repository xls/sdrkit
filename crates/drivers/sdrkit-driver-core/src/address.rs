//! The address form of a kind of radio: the parts of its address as the arguments of the `address` command, and the address
//! text composed from them.

use sdrkit_core::publish::field;
use sdrkit_core::storage::PathError;
use sdrkit_core::{
    CommandDef, CommandError, Field, RecordDecl, Schema, SettingDecl, SettingError, SettingId,
    StorePath, Value, Values,
};
use std::borrow::Cow;
use std::net::Ipv6Addr;
use std::sync::Arc;

/// The key of the command of every kind that composes an address.
pub const ADDRESS_COMMAND: &str = "address";

/// The reply of the `address` command: the address text, with the driver key as its scheme.
pub const ADDRESS: Field<field::Text> = Field::new("address", "Address", "");

/// The rule that the text of a part follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syntax {
    /// A host name or an IPv4 address: letters, digits, `.`, `-` and `_`. If `v6` is set, also an IPv6 address, with or
    /// without brackets, which is written in brackets.
    Host {
        /// The kind opens IPv6 addresses.
        v6: bool,
    },
    /// A name without spaces and without `/`.
    Name,
    /// A path in the storage folder ([`StorePath`]).
    StorePath,
    /// The path of a URL without spaces, `?` and `#`. A leading `/` is dropped.
    UrlPath,
}

/// The value of a part and the condition under which it is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartValue {
    /// A text. If `required` is false, the default is the empty text and an empty text is not written.
    Text {
        /// The rule the text follows.
        syntax: Syntax,
        /// The text must be given and must not be empty.
        required: bool,
    },
    /// An integer from `min` to `max`. Without a default, it must be given.
    Integer {
        /// The smallest value.
        min: i64,
        /// The largest value.
        max: i64,
        /// The default. `None` for a required part.
        default: Option<i64>,
        /// The part is not written when the value is the default.
        omit_default: bool,
    },
    /// A switch, off by default. It has no value to write: when it is on, [`AddressPart::before`] is written at the start
    /// of the address.
    Switch,
}

/// One part of the address of a kind of radio: one argument of the `address` command, and how its value is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressPart {
    /// The key of the argument.
    pub key: &'static str,
    /// What people see.
    pub label: &'static str,
    /// One sentence for people.
    pub help: &'static str,
    /// The text written before the value: `""` for the first part, `":"` before a port.
    pub before: &'static str,
    /// The value and when it is written.
    pub value: PartValue,
    /// The owner and the table whose row keys a text part holds. `None` for most parts.
    pub rows: Option<(&'static str, &'static str)>,
}

impl AddressPart {
    /// The part `host`: the host name or the IP address of the radio. It is required and written first. `v6` allows an
    /// IPv6 address.
    pub const fn host(v6: bool) -> Self {
        Self::text("host", "Host", "", Syntax::Host { v6 })
            .required()
            .help("The host name or the IP address of the radio or its server.")
    }

    /// The part `port`: a port from 1 to 65535, written after `:`. With `default`, the part is always written. Without
    /// one, it is required.
    pub const fn port(default: Option<u16>) -> Self {
        AddressPart {
            key: "port",
            label: "Port",
            help: "The port of the radio or its server.",
            before: ":",
            value: PartValue::Integer {
                min: 1,
                max: 65535,
                default: match default {
                    Some(p) => Some(p as i64),
                    None => None,
                },
                omit_default: false,
            },
            rows: None,
        }
    }

    /// The part `receiver`: a receiver of the radio, from 1 to `max`, written after `/rx`. Receiver 1 is the default
    /// and is not written.
    pub const fn receiver(max: u8) -> Self {
        AddressPart {
            key: "receiver",
            label: "Receiver",
            help: "The receiver of the radio to open. The first is 1.",
            before: "/rx",
            value: PartValue::Integer {
                min: 1,
                max: max as i64,
                default: Some(1),
                omit_default: true,
            },
            rows: None,
        }
    }

    /// An optional text part `key` shown as `label`, written after `before`.
    pub const fn text(
        key: &'static str,
        label: &'static str,
        before: &'static str,
        syntax: Syntax,
    ) -> Self {
        AddressPart {
            key,
            label,
            help: "",
            before,
            value: PartValue::Text {
                syntax,
                required: false,
            },
            rows: None,
        }
    }

    /// A switch `key` shown as `label`. When it is on, `on` is written at the start of the address.
    pub const fn switch(key: &'static str, label: &'static str, on: &'static str) -> Self {
        AddressPart {
            key,
            label,
            help: "",
            before: on,
            value: PartValue::Switch,
            rows: None,
        }
    }

    /// The same text part, required.
    #[must_use]
    pub const fn required(mut self) -> Self {
        if let PartValue::Text { syntax, .. } = self.value {
            self.value = PartValue::Text {
                syntax,
                required: true,
            };
        }
        self
    }

    /// The same part with this help text.
    #[must_use]
    pub const fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }

    /// The same part, holding row keys of table `table` of owner `owner`.
    ///
    /// # Panics
    ///
    /// If `owner` is not a valid owner key. In a `const`, this stops the build.
    #[must_use]
    pub const fn rows(mut self, owner: &'static str, table: &'static str) -> Self {
        assert!(
            sdrkit_core::OwnerKey::is_valid(owner),
            "not a valid owner key"
        );
        self.rows = Some((owner, table));
        self
    }

    /// The declaration of the argument that holds the part.
    pub fn decl(&self) -> SettingDecl {
        let decl = match self.value {
            PartValue::Text { required, .. } => {
                let mut f: Field<field::Text> = Field::new(self.key, self.label, "");
                if let Some((owner, table)) = self.rows {
                    f = f.refers_to(owner, table);
                }
                if required {
                    SettingDecl::required(f.decl())
                } else {
                    SettingDecl::new(f.decl(), Value::Text(String::new()))
                }
            }
            PartValue::Integer {
                min, max, default, ..
            } => {
                let f: Field<field::I64> = Field::new(self.key, self.label, "");
                match default {
                    Some(d) => SettingDecl::new(f.decl(), Value::I64(d)),
                    None => SettingDecl::required(f.decl()),
                }
                .with_range(min as f64, max as f64, 1.0)
            }
            PartValue::Switch => {
                let f: Field<field::Bool> = Field::new(self.key, self.label, "");
                SettingDecl::new(f.decl(), Value::Bool(false))
            }
        };
        decl.with_help(self.help)
    }
}

/// The declaration of the `address` command of a kind with the address parts `parts`: one argument per part, in order,
/// and the reply [`ADDRESS`].
///
/// # Panics
///
/// If two parts share a key or a default is outside its range. That is a fault of the descriptor.
pub fn command(parts: &[AddressPart]) -> CommandDef {
    let args = match Schema::arguments(parts.iter().map(AddressPart::decl).collect()) {
        Ok(s) => s,
        Err(e) => panic!("{e}"),
    };
    CommandDef {
        key: Cow::Borrowed(ADDRESS_COMMAND),
        label: Cow::Borrowed("Build an address"),
        help: Cow::Borrowed(
            "Returns the address of a radio of this kind, made from its parts. It connects to nothing.",
        ),
        args: Arc::new(args),
        reply: Arc::new(RecordDecl::of(&[&ADDRESS])),
        transmits: None,
        stopped_by: None,
    }
}

/// The address of a radio of the kind `key` with the address parts `parts`: the key, `:`, then every part that is
/// written, in order. `args` holds the arguments of the `address` command ([`command`]) after its check.
///
/// # Errors
///
/// [`CommandError::MissingArgument`] for a required part that is empty or not given. [`CommandError::BadArgument`] with
/// [`SettingError::Malformed`] for a text that breaks the [`Syntax`] of its part.
pub fn compose(key: &str, parts: &[AddressPart], args: &Values) -> Result<String, CommandError> {
    let mut lead = "";
    let mut body = String::new();
    for (i, part) in parts.iter().enumerate() {
        let arg = SettingId(u16::try_from(i).unwrap_or(u16::MAX));
        match (part.value, args.value(arg)) {
            (PartValue::Text { syntax, required }, Some(Value::Text(text))) => {
                let text = syntax.clean(text).map_err(|at| CommandError::BadArgument {
                    arg,
                    error: SettingError::Malformed { id: arg, at },
                })?;
                if text.is_empty() {
                    if required {
                        return Err(CommandError::MissingArgument { arg });
                    }
                    continue;
                }
                body.push_str(part.before);
                body.push_str(&text);
            }
            (
                PartValue::Integer {
                    default,
                    omit_default,
                    ..
                },
                Some(Value::I64(n)),
            ) => {
                if !(omit_default && default == Some(*n)) {
                    body.push_str(part.before);
                    body.push_str(&n.to_string());
                }
            }
            (PartValue::Switch, Some(Value::Bool(on))) => {
                if *on {
                    lead = part.before;
                }
            }
            _ => return Err(CommandError::MissingArgument { arg }),
        }
    }
    Ok(format!("{key}:{lead}{body}"))
}

impl Syntax {
    /// The text as it is written in an address. An empty result is a part that is not set.
    ///
    /// # Errors
    ///
    /// The byte offset of the first character that breaks the rule, or `None` if the fault has no single place.
    fn clean(self, text: &str) -> Result<Cow<'_, str>, Option<usize>> {
        // The offset in `text` of the first character of `t`, a tail of `text` trimmed at its end, that `bad` names.
        let first = |t: &str, bad: fn(char) -> bool| match t.find(bad) {
            Some(i) => Err(Some(text.trim_end().len() - t.len() + i)),
            None => Ok(()),
        };
        match self {
            Syntax::Host { v6 } => {
                let t = text.trim();
                let inner = t
                    .strip_prefix('[')
                    .and_then(|r| r.strip_suffix(']'))
                    .unwrap_or(t);
                if v6 && is_v6(inner) {
                    return Ok(Cow::Owned(format!("[{inner}]")));
                }
                first(t, |c| {
                    !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
                })?;
                Ok(Cow::Borrowed(t))
            }
            Syntax::Name => {
                let t = text.trim();
                first(t, |c| c.is_whitespace() || c == '/')?;
                Ok(Cow::Borrowed(t))
            }
            Syntax::StorePath => match StorePath::new(text) {
                Ok(_) | Err(PathError::Empty) => Ok(Cow::Borrowed(text)),
                Err(PathError::Root) => Err(Some(0)),
                Err(PathError::Char { at }) => Err(Some(at)),
                Err(PathError::EmptySegment | PathError::Dot) => Err(None),
            },
            Syntax::UrlPath => {
                let t = text.trim().trim_start_matches('/');
                first(t, |c| c.is_whitespace() || matches!(c, '?' | '#'))?;
                Ok(Cow::Borrowed(t))
            }
        }
    }
}

/// True if `text` is an IPv6 address, with or without a zone (`fe80::1%en0`).
fn is_v6(text: &str) -> bool {
    let (address, zone) = match text.split_once('%') {
        Some((a, z)) => (a, Some(z)),
        None => (text, None),
    };
    address.parse::<Ipv6Addr>().is_ok()
        && zone.is_none_or(|z| {
            !z.is_empty()
                && z.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::Commands;

    const HOST_PORT: &[AddressPart] = &[AddressPart::host(true), AddressPart::port(Some(1234))];
    const RECEIVERS: &[AddressPart] = &[
        AddressPart::host(false),
        AddressPart::port(None),
        AddressPart::receiver(4),
    ];
    const LINK: &[AddressPart] = &[
        AddressPart::host(true),
        AddressPart::port(Some(8073)),
        AddressPart::text("path", "Path", "/", Syntax::UrlPath),
        AddressPart::switch("tls", "TLS", "https://"),
    ];
    const NAMES: &[AddressPart] = &[
        AddressPart::text("stack", "Stack", "", Syntax::Name)
            .required()
            .rows("engine", "stacks"),
        AddressPart::text("submode", "Submode", "/", Syntax::Name),
    ];
    const FILE: &[AddressPart] =
        &[AddressPart::text("path", "File", "", Syntax::StorePath).required()];

    /// Runs the `address` command of `parts` with the arguments `set`, through the check of the command.
    fn address(parts: &[AddressPart], set: &[(&str, Value)]) -> Result<String, CommandError> {
        let commands = Commands::new(vec![command(parts)]).unwrap();
        let id = commands.id(ADDRESS_COMMAND).unwrap();
        let mut given = commands.def(id).unwrap().new_args();
        for (key, value) in set {
            let field = given.decl().id(key).unwrap();
            given.set_value(field, value).unwrap();
        }
        compose("k", parts, &commands.check(id, &given)?)
    }

    fn text(t: &str) -> Value {
        Value::Text(t.into())
    }

    #[test]
    fn a_port_is_written_with_its_default_and_is_required_without_one() {
        let host = ("host", text("192.0.2.1"));
        assert_eq!(
            address(HOST_PORT, std::slice::from_ref(&host)).unwrap(),
            "k:192.0.2.1:1234"
        );
        assert_eq!(
            address(HOST_PORT, &[host.clone(), ("port", Value::I64(5555))]).unwrap(),
            "k:192.0.2.1:5555"
        );
        assert_eq!(
            address(RECEIVERS, std::slice::from_ref(&host)),
            Err(CommandError::MissingArgument { arg: SettingId(1) })
        );
        assert!(matches!(
            address(HOST_PORT, &[host, ("port", Value::I64(0))]),
            Err(CommandError::BadArgument {
                arg: SettingId(1),
                error: SettingError::OutOfRange { .. }
            })
        ));
    }

    #[test]
    fn a_receiver_is_written_only_when_it_is_not_the_first() {
        let set = [("host", text("radio.local")), ("port", Value::I64(1024))];
        assert_eq!(address(RECEIVERS, &set).unwrap(), "k:radio.local:1024");
        let mut with = set.to_vec();
        with.push(("receiver", Value::I64(3)));
        assert_eq!(address(RECEIVERS, &with).unwrap(), "k:radio.local:1024/rx3");
        with[2].1 = Value::I64(5);
        assert!(matches!(
            address(RECEIVERS, &with),
            Err(CommandError::BadArgument {
                arg: SettingId(2),
                ..
            })
        ));
    }

    #[test]
    fn a_path_and_a_switch_are_written_only_when_set() {
        let host = ("host", text("sdr.example"));
        assert_eq!(
            address(LINK, std::slice::from_ref(&host)).unwrap(),
            "k:sdr.example:8073"
        );
        assert_eq!(
            address(LINK, &[host.clone(), ("path", text("/rx/one"))]).unwrap(),
            "k:sdr.example:8073/rx/one"
        );
        assert_eq!(
            address(
                LINK,
                &[
                    host.clone(),
                    ("port", Value::I64(443)),
                    ("tls", Value::Bool(true))
                ]
            )
            .unwrap(),
            "k:https://sdr.example:443"
        );
        assert_eq!(
            address(LINK, &[host, ("path", text("a b"))]),
            Err(CommandError::BadArgument {
                arg: SettingId(2),
                error: SettingError::Malformed {
                    id: SettingId(2),
                    at: Some(1)
                }
            })
        );
    }

    #[test]
    fn a_submode_is_written_only_when_set() {
        let stack = ("stack", text("fm"));
        assert_eq!(
            address(NAMES, std::slice::from_ref(&stack)).unwrap(),
            "k:fm"
        );
        assert_eq!(
            address(NAMES, &[stack, ("submode", text("wide"))]).unwrap(),
            "k:fm/wide"
        );
        assert_eq!(
            address(NAMES, &[]),
            Err(CommandError::MissingArgument { arg: SettingId(0) })
        );
        let decl = NAMES[0].decl();
        let rows = decl.field.refers_to.unwrap();
        assert_eq!(rows.owner.unwrap().as_str(), "engine");
        assert_eq!(rows.table, "stacks");
    }

    #[test]
    fn a_host_follows_its_rule() {
        let host = |h: &str| address(HOST_PORT, &[("host", text(h))]);
        assert_eq!(host(" radio-1.local ").unwrap(), "k:radio-1.local:1234");
        assert_eq!(host("2001:db8::1").unwrap(), "k:[2001:db8::1]:1234");
        assert_eq!(host("[2001:db8::1]").unwrap(), "k:[2001:db8::1]:1234");
        assert_eq!(host("fe80::1%en0").unwrap(), "k:[fe80::1%en0]:1234");
        let bad = |at| {
            Err(CommandError::BadArgument {
                arg: SettingId(0),
                error: SettingError::Malformed {
                    id: SettingId(0),
                    at: Some(at),
                },
            })
        };
        assert_eq!(
            host(""),
            Err(CommandError::MissingArgument { arg: SettingId(0) })
        );
        assert_eq!(
            host("  "),
            Err(CommandError::MissingArgument { arg: SettingId(0) })
        );
        assert_eq!(host("my radio"), bad(2));
        assert_eq!(host("radio:1234"), bad(5));
        assert_eq!(host("http://radio"), bad(4));
        let no_v6 = address(
            RECEIVERS,
            &[("host", text("2001:db8::1")), ("port", Value::I64(1024))],
        );
        assert_eq!(no_v6, bad(4));
    }

    #[test]
    fn a_file_path_follows_the_storage_rules() {
        let path = |p: &str| address(FILE, &[("path", text(p))]);
        assert_eq!(path("iq/x.wav").unwrap(), "k:iq/x.wav");
        assert_eq!(
            path(""),
            Err(CommandError::MissingArgument { arg: SettingId(0) })
        );
        for refused in ["/x.wav", "a/../x.wav", "c:x.wav"] {
            assert!(
                matches!(
                    path(refused),
                    Err(CommandError::BadArgument {
                        error: SettingError::Malformed { .. },
                        ..
                    })
                ),
                "{refused}"
            );
        }
    }
}
