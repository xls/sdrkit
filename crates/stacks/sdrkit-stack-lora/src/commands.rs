//! The optional arguments of the commands of the family and the commands without arguments, each once as a constant.
//! The commands with arguments are in [`crate::typed`]. The components declare them, and a stack's commands are aliases
//! to them.

use crate::mesh::mt_client::MtRequest;
use sdrkit_core::command::Command;
use sdrkit_core::publish::field;
use sdrkit_core::settings::Argument;
use sdrkit_core::Setting;

sdrkit_core::choice_enum! {
    /// Whether an own Meshtastic text asks for an acknowledgement.
    pub enum WantAck {
        /// As the stack decides: always for a direct text, and for a channel text as the setting `mt_ack_broadcast`
        /// says.
        Setting = ("setting", "As the settings say"),
        /// Ask for an acknowledgement.
        Yes = ("yes", "Yes"),
        /// Do not ask.
        No = ("no", "No"),
    }
}

impl WantAck {
    /// The choice as a switch: `None` for [`WantAck::Setting`].
    pub fn given(self) -> Option<bool> {
        match self {
            WantAck::Setting => None,
            WantAck::Yes => Some(true),
            WantAck::No => Some(false),
        }
    }
}

sdrkit_core::choice_enum! {
    /// Where a MeshCore advert goes.
    pub enum AdvertScope {
        /// To the whole mesh.
        Flood = ("flood", "To the mesh"),
        /// Only to the nodes in range.
        Local = ("local", "Local"),
    }
}

/// The key of a Meshtastic channel in base64 or hex, or a key word. Empty takes the default key.
pub const KEY: Setting<field::Text> = Setting::new("key", "Key", "", "")
    .help("Base64 or hex, or the word default or none. Leave it empty for the default key.");
/// The secret of a MeshCore channel in hex or base64. Empty takes the secret of the channel Public or of the hashtag
/// channel of the name.
pub const MC_KEY: Setting<field::Text> = Setting::new("key", "Key", "", "").help(
    "16 or 32 bytes in hex or base64. Leave it empty for the channel Public and for a hashtag channel: the name xyz is then the channel #xyz.",
);
/// The name of a contact.
pub const CONTACT_NAME: Setting<field::Text> = Setting::new("name", "Name", "", "");
/// The slot of an own Meshtastic channel: 0 is the primary.
pub const INDEX: Setting<field::I64> = Setting::new("index", "Slot", "", 0).range(0.0, 7.0, 1.0);
/// The position precision of an own Meshtastic channel.
pub const PRECISION: Setting<field::I64> =
    Setting::new("precision", "Position precision", "bits", 13).range(0.0, 32.0, 1.0);
/// The addressee of a Meshtastic text: "^all" or a node id "!xxxxxxxx".
pub const MT_TO: Setting<field::Text> =
    Setting::new("to", "To", "", "^all").help("\"^all\" or a node id \"!xxxxxxxx\".");
/// The channel of a text: a name of the `channels` table.
pub const CHANNEL: Setting<field::Text> = Setting::new("channel", "Channel", "", "")
    .help("A name of the Channels table. Leave it empty for the first channel.");
/// The packet id a text replies to (0 none).
pub const REPLY_ID: Setting<field::I64> =
    Setting::new("reply_id", "Reply to", "", 0).range(0.0, 4_294_967_295.0, 1.0);
/// A reaction: a Unicode code point (0 none).
pub const EMOJI: Setting<field::I64> =
    Setting::new("emoji", "Reaction", "", 0).range(0.0, 1_114_111.0, 1.0);
/// Whether an own text asks for an acknowledgement.
pub const WANT_ACK: Setting<field::Choice<WantAck>> =
    Setting::choice("want_ack", "Want-ack", WantAck::Setting)
        .help("Whether the addressee or the mesh is asked to acknowledge the text.");
/// The hop limit of an own text (0 takes the setting `mt_hop_limit`).
pub const HOP_LIMIT: Setting<field::I64> = Setting::new("hop_limit", "Hop limit", "", 0)
    .range(0.0, 7.0, 1.0)
    .zero_means("The setting mt_hop_limit")
    .help("How many times the mesh may relay the text, 1 to 7.");
/// What the request is for.
pub const WHAT: Setting<field::Choice<MtRequest>> =
    Setting::choice("what", "Request", MtRequest::NodeInfo);
/// The addressee of a MeshCore direct text: a contact's key in hex (a prefix will do) or its name. Empty sends to a channel.
pub const MC_TO: Setting<field::Text> = Setting::new("to", "To", "", "")
    .help("A contact's key in hex (a prefix will do) or its name. Empty sends on the channel.");
/// Where an advert goes.
pub const SCOPE: Setting<field::Choice<AdvertScope>> =
    Setting::choice("scope", "To", AdvertScope::Flood);

/// The row key of a row of the `channels` table, the argument of `channel.remove`.
pub const CHANNEL_ROW: Argument<field::Text> = sdrkit_util_objects::commands::row_of("channels");
/// Sets the counters to zero.
pub const RESET: Command = Command::new("reset", "Reset").help("Sets the counters to zero.");
/// Sends the own APRS beacon now: the position of the station, else the comment as a status.
pub const APRS_BEACON: Command = Command::new("aprs.beacon", "Send an APRS beacon")
    .help("The position of the station with the symbol and the comment of the settings. Without a position the comment goes as a status.")
    .transmits(sdrkit_core::TxContent::Beacon);
