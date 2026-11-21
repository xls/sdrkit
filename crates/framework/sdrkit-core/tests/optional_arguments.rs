//! A command with optional arguments: the derive declares what a list of constants declares, a caller that passes
//! `None` leaves the argument out, and the owner reads the default.

use sdrkit_core::interface::{Arguments, CommandKind};
use sdrkit_core::publish::{field, Field};
use sdrkit_core::settings::Argument;
use sdrkit_core::{Command, Commands, Setting};

const TEXT: Argument<field::Text> = Argument::new(Field::new("text", "Text", ""));
const CHANNEL: Setting<field::Text> = Setting::new("channel", "Channel", "", "")
    .help("A name of the Channels table. Leave it empty for the first channel.");
const TO: Setting<field::Text> = Setting::new("to", "To", "", "^all");
const REPLY_ID: Setting<field::I64> =
    Setting::new("reply_id", "Reply to", "", 0).range(0.0, 4_294_967_295.0, 1.0);
const BY_HAND: Command = Command::new("send", "Send").args(&[&TEXT, &CHANNEL, &TO, &REPLY_ID]);

/// The arguments of `send`.
#[derive(Arguments, Debug, PartialEq)]
struct SendArgs<'a> {
    /// The text.
    #[arg(label = "Text")]
    text: &'a str,
    /// The channel.
    #[arg(shared = CHANNEL)]
    channel: Option<&'a str>,
    /// The addressee.
    #[arg(label = "To", default = "^all")]
    to: Option<&'a str>,
    /// The packet that the text replies to.
    #[arg(
        label = "Reply to",
        default = 0,
        min = 0,
        max = 4_294_967_295.0,
        step = 1
    )]
    reply_id: Option<i64>,
}

const SEND: CommandKind<SendArgs<'static>> = CommandKind::new("send", "Send");

#[test]
fn the_derive_declares_what_the_constants_declare() {
    assert_eq!(SEND.command.def(), BY_HAND.def());
}

#[test]
fn an_argument_left_out_reads_as_its_default() {
    let commands = Commands::of(&[&SEND.command]);
    let id = commands.id("send").unwrap();
    let def = commands.def(id).unwrap();
    let mut rec = def.new_args();
    SendArgs {
        text: "hello",
        channel: None,
        to: Some("!0badcafe"),
        reply_id: None,
    }
    .write(&mut rec);
    let values = def.check(id, &rec).unwrap();
    let got: SendArgs = SEND.args(&values).unwrap();
    assert_eq!(
        got,
        SendArgs {
            text: "hello",
            channel: Some(""),
            to: Some("!0badcafe"),
            reply_id: Some(0),
        }
    );
    // A required argument that is left out is refused.
    assert!(def.check(id, &def.new_args()).is_err());
}
