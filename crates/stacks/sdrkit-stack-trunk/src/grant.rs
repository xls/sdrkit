//! `trunk.grant`: a voice grant, what the trunking stacks hand the follower.

/// A voice grant.
#[derive(Clone, Debug, PartialEq)]
pub struct Grant {
    /// The protocol for people: "MPT1327", "LTR", "EDACS".
    pub protocol: &'static str,
    /// The talkgroup or called unit, as text.
    pub talkgroup: String,
    /// The talkgroup or called unit as a number where the protocol has one (-1 otherwise).
    pub talkgroup_id: i64,
    /// The calling unit, as text ("" when unknown).
    pub caller: String,
    /// The calling unit as a number (-1 when unknown).
    pub caller_id: i64,
    /// The voice channel's frequency, Hz (0 when the channel plan has none).
    pub freq_hz: f64,
    /// The channel number (logical channel, repeater, MPT 1327 channel).
    pub channel: i32,
    /// The slot (-1: not a TDMA system).
    pub slot: i32,
    /// A group call (false: individual).
    pub group: bool,
    /// An emergency call.
    pub emergency: bool,
    /// Encrypted (never followed).
    pub encrypted: bool,
    /// 1 (high) to 7 (low).
    pub priority: u8,
    /// A digital (P25 Phase 1) voice channel; false for analogue FM.
    pub digital: bool,
    /// The width of the voice channel, Hz (0: the follower's default).
    pub bandwidth_hz: f64,
    /// Signal time, s.
    pub time: f64,
}

impl Default for Grant {
    fn default() -> Self {
        Grant {
            protocol: "",
            talkgroup: String::new(),
            talkgroup_id: -1,
            caller: String::new(),
            caller_id: -1,
            freq_hz: 0.0,
            channel: -1,
            slot: -1,
            group: true,
            emergency: false,
            encrypted: false,
            priority: 4,
            digital: false,
            bandwidth_hz: 0.0,
            time: 0.0,
        }
    }
}

impl sdrkit_graph::Item for Grant {
    const KIND: &'static str = "trunk.grant";
}
