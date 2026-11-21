//! Why a payload is not a frame of a protocol, and why a key that the user supplies is refused.

/// A part of a frame that is too short or missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The Meshtastic packet header.
    MeshtasticHeader,
    /// A MeshCore packet.
    MeshCorePacket,
    /// The MeshCore transport codes.
    TransportCodes,
    /// The MeshCore payload.
    MeshCorePayload,
    /// A MeshCore group text.
    GroupText,
    /// A MeshCom packet.
    MeshComPacket,
    /// The MeshCom trailer.
    MeshComTrailer,
    /// The end of a MeshCom payload.
    MeshComPayloadEnd,
    /// A LoRaWAN rejoin request.
    RejoinRequest,
}

impl Part {
    fn words(self) -> &'static str {
        match self {
            Part::MeshtasticHeader => "a Meshtastic header",
            Part::MeshCorePacket => "a MeshCore packet",
            Part::TransportCodes => "the transport codes",
            Part::MeshCorePayload => "the payload",
            Part::GroupText => "a group text",
            Part::MeshComPacket => "a MeshCom packet",
            Part::MeshComTrailer => "the trailer",
            Part::MeshComPayloadEnd => "the end of the payload",
            Part::RejoinRequest => "a rejoin request",
        }
    }
}

/// A LoRaWAN frame type whose length is fixed or has a least value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameType {
    /// A join request: 23 bytes.
    JoinRequest,
    /// A join accept: 17 or 33 bytes.
    JoinAccept,
    /// A rejoin request: 19 or 24 bytes.
    RejoinRequest,
    /// A data frame: 12 bytes at least.
    DataFrame,
}

impl FrameType {
    fn name(self) -> &'static str {
        match self {
            FrameType::JoinRequest => "join request",
            FrameType::JoinAccept => "join accept",
            FrameType::RejoinRequest => "rejoin request",
            FrameType::DataFrame => "data frame",
        }
    }
    fn expected(self) -> &'static str {
        match self {
            FrameType::JoinRequest => " (23 expected)",
            FrameType::JoinAccept => " (17 or 33 expected)",
            FrameType::RejoinRequest => "",
            FrameType::DataFrame => " (12 at least)",
        }
    }
}

/// Why a payload is not a frame of the protocol that was tried. The text is the note that the packet list shows.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum ParseError {
    /// The protocol does not apply, with no more to say.
    #[error("not a frame of this protocol")]
    NotThis,
    /// The frame is shorter than a part of it.
    #[error("too short for {}", .0.words())]
    Short(Part),
    /// The MeshCore path length byte does not fit the frame.
    #[error("bad path length byte")]
    PathLength,
    /// The MeshCore payload is longer than the protocol allows.
    #[error("payload longer than {max} bytes")]
    PayloadTooLong {
        /// The most the protocol allows.
        max: usize,
    },
    /// A MeshCore group text of a type that is not plain text.
    #[error("unsupported text type {0}")]
    TextType(u8),
    /// The frame does not start with the LoRa APRS prefix.
    #[error("no LoRa APRS prefix")]
    NoPrefix,
    /// The text is not `SRC>DEST,PATH:INFO`.
    #[error("not a TNC2 packet")]
    NotTnc2,
    /// A callsign of the TNC2 header is not valid.
    #[error("bad call in the TNC2 header")]
    BadCall,
    /// The path of the TNC2 header is not valid.
    #[error("bad path in the TNC2 header")]
    BadPath,
    /// The information field is not APRS.
    #[error("not APRS: {0}")]
    Aprs(sdrkit_proto_link::Error),
    /// The first byte is not a MeshCom packet type.
    #[error("MeshCom: not a MeshCom type")]
    MeshComType,
    /// A MeshCom acknowledgement of the wrong form.
    #[error("MeshCom: not a MeshCom ack")]
    MeshComAck,
    /// No `>` follows the MeshCom source.
    #[error("MeshCom: no '>' after the source")]
    MeshComSource,
    /// No type character follows the MeshCom destination.
    #[error("MeshCom: no type character after the destination")]
    MeshComDestination,
    /// The MeshCom checksum is wrong.
    #[error("MeshCom: checksum wrong")]
    MeshComChecksum,
    /// The MeshCom source callsign is not valid.
    #[error("MeshCom: bad source call")]
    MeshComCall,
    /// The payload is empty.
    #[error("empty")]
    Empty,
    /// The LoRaWAN major version or the reserved bits are not zero.
    #[error("not LoRaWAN R1 (RFU or major bits set)")]
    NotLorawanR1,
    /// A LoRaWAN frame of a length that its type does not have.
    #[error("{} of {got} bytes{}", .kind.name(), .kind.expected())]
    Length {
        /// The frame type.
        kind: FrameType,
        /// Its length.
        got: usize,
    },
    /// The LoRaWAN FOptsLen does not fit the frame.
    #[error("FOptsLen {fopts} does not fit {got} bytes")]
    FOptsLen {
        /// The FOptsLen.
        fopts: usize,
        /// The frame length.
        got: usize,
    },
}

/// Why a key or a secret that the user supplies is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    /// The text is not base64, hex or a key word.
    #[error("the key is not base64, hex or a key word")]
    NotKeyText,
    /// The key is longer than 32 bytes.
    #[error("the key is too long")]
    TooLong,
    /// A MeshCore channel secret is not 16 or 32 bytes of hex or base64.
    #[error("the secret is not 16 or 32 bytes of hex or base64")]
    Secret,
    /// A public key is not 32 bytes of hex or base64.
    #[error("a public key is 32 bytes of hex or base64")]
    PublicKey,
    /// The table has no room for another contact.
    #[error("the contact table is full")]
    Full,
}
