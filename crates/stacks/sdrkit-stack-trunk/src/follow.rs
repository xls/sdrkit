//! `trunk.follow`: which voice grant to follow. It takes the grants of a control channel and requests the host to follow one
//! voice channel at a time (`HostRequest::Follow`), and to come back when the call ends or a better call comes. The host tunes
//! the radios, runs the second receiver and records the audio.

use crate::grant::Grant;
use crate::typed::{FollowEvent, TRUNK_FOLLOW};
use crate::{typed_format, typed_port};
use sdrkit_core::publish::{field, keys};
use sdrkit_core::{
    ConfigError, Event, HostRequest, Setting, SettingError, SettingId, SettingSet, SignalType,
    Value, Values,
};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, Timed};
use sdrkit_util_ids::lists::IdList;
use sdrkit_util_ids::names::IdKind;

/// The stack that decodes an analog voice channel.
/// What a followed voice channel carries.
pub const VOICE_SIGNAL: SignalType = SignalType::NarrowFmVoice;
/// What a followed digital voice channel carries.
pub const DIGITAL_VOICE_SIGNAL: SignalType = SignalType::P25Phase1;
/// The bandwidth of a followed voice channel, Hz, unless the grant gives one.
pub const VOICE_BANDWIDTH_HZ: f64 = 12.5e3;
/// A late grant for a call that ended less than this long ago does not open it again, s.
const REOPEN_S: f64 = 4.0;

/// Follow calls at all.
pub const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow calls", "", false)
    .help("Ask the radio to listen to the voice channel of each call.");
/// The talkgroups followed, as text.
pub const FOLLOW_GROUPS: Setting<field::Text> = Setting::new(
    "follow_groups",
    "Follow groups",
    "",
    "",
)
.help(
    "Talkgroups to follow (\"1201 2000-2099\"). Empty follows every call. A bad item is refused.",
);
/// The longest follow.
pub const FOLLOW_MAX: Setting<field::Duration> =
    Setting::new("follow_max_s", "Longest follow", "s", 120.0)
        .range(5.0, 1800.0, 1.0)
        .help("A call followed longer than this ends (a stuck carrier).");

sdrkit_core::settings! {
    /// The settings of `trunk.follow`.
    pub enum FollowSetting {
        /// Follow calls.
        Follow = FOLLOW,
        /// The talkgroups followed.
        Groups = FOLLOW_GROUPS,
        /// The longest follow.
        MaxS = FOLLOW_MAX,
    }
}

sdrkit_core::choice_enum! {
    /// What a follow decision did.
    pub enum FollowAction {
        /// The host was requested to follow the call.
        Follow = ("follow", "Follow"),
        /// The call was not followed.
        Skip = ("skipped", "Skipped"),
        /// The host was requested to come back.
        Release = ("release", "Release"),
    }
}

sdrkit_core::choice_enum! {
    /// Why a call was not followed, or why a follow ended.
    pub enum FollowReason {
        /// The call is encrypted.
        Encrypted = ("encrypted", "Encrypted"),
        /// The channel plan has no frequency for the channel.
        NoFrequency = ("no_frequency", "No frequency for the channel"),
        /// The talkgroup is not in the list.
        NotListed = ("not_listed", "Not in the followed groups"),
        /// Another call of the same or a higher priority is followed.
        Busy = ("busy", "Busy with another call"),
        /// The call ended.
        Ended = ("ended", "The call ended"),
        /// A better call holds the radio.
        Replaced = ("replaced", "A better call came"),
        /// Following was turned off.
        Off = ("off", "Following turned off"),
    }
}

/// A call being followed.
#[derive(Clone, Debug, PartialEq)]
struct Followed {
    freq_hz: f64,
    talkgroup: String,
    caller: String,
    channel: i32,
    priority: u8,
    emergency: bool,
    until: f64,
}

/// The follow decision (module documentation).
#[derive(Clone, Debug)]
pub struct FollowPolicy {
    /// Follow at all.
    pub on: bool,
    /// The talkgroups followed (empty: all).
    pub groups: IdList,
    /// The talkgroups followed, as text from the setting.
    pub groups_text: String,
    /// The longest follow, s.
    pub max_s: f64,
    now: Option<Followed>,
    ended: Option<(f64, f64)>,
}

impl Default for FollowPolicy {
    fn default() -> Self {
        FollowPolicy {
            on: false,
            groups: IdList::new(IdKind::Decimal),
            groups_text: String::new(),
            max_s: 120.0,
            now: None,
            ended: None,
        }
    }
}

/// The event of a decision on grant `g`.
fn grant_event(t: f64, g: &Grant, action: FollowAction, reason: Option<FollowReason>) -> Event {
    TRUNK_FOLLOW.event(
        t,
        g.freq_hz,
        &FollowEvent {
            action,
            reason,
            source: (!g.caller.is_empty()).then_some(g.caller.as_str()),
            target: Some(&g.talkgroup),
            channel: (g.channel >= 0).then(|| g.channel.to_string()).as_deref(),
            urgency: urgency(g.emergency),
            family: keys::Family::Voice,
        },
    )
}

/// The urgency of a call.
pub fn urgency(emergency: bool) -> keys::Urgency {
    if emergency {
        keys::Urgency::Emergency
    } else {
        keys::Urgency::None
    }
}

impl FollowPolicy {
    /// Why a grant cannot be followed: no known frequency, encrypted, or a talkgroup outside the list.
    fn refusal(&self, g: &Grant) -> Option<FollowReason> {
        if g.encrypted {
            return Some(FollowReason::Encrypted);
        }
        if g.freq_hz <= 0.0 || g.freq_hz.is_nan() {
            return Some(FollowReason::NoFrequency);
        }
        if !self.groups.is_empty()
            && !(g.talkgroup_id >= 0 && self.groups.contains_number(g.talkgroup_id as u64))
            && !self.groups.contains(&g.talkgroup)
        {
            return Some(FollowReason::NotListed);
        }
        None
    }
    /// Takes one grant at signal time `t`, and returns the requests it makes and the follow events.
    pub fn grant(
        &mut self,
        g: &Grant,
        t: f64,
        requests: &mut Vec<HostRequest>,
        events: &mut Vec<Event>,
    ) {
        self.expire(t, requests, events);
        if !self.on {
            return;
        }
        if let Some(why) = self.refusal(g) {
            events.push(grant_event(t, g, FollowAction::Skip, Some(why)));
            return;
        }
        if let Some(f) = &self.now {
            if (f.freq_hz - g.freq_hz).abs() < 1.0 {
                return;
            }
            // A call of higher priority (a lower number) or an emergency takes the radio.
            let better = (g.emergency && !f.emergency) || g.priority < f.priority;
            if !better {
                events.push(grant_event(
                    t,
                    g,
                    FollowAction::Skip,
                    Some(FollowReason::Busy),
                ));
                return;
            }
            let freq_hz = f.freq_hz;
            self.release(freq_hz, t, FollowReason::Replaced, requests, events);
        }
        if self
            .ended
            .is_some_and(|(f, at)| (f - g.freq_hz).abs() < 1.0 && t - at < REOPEN_S)
        {
            return;
        }
        let label = if g.group {
            format!("{} talkgroup {}", g.protocol, g.talkgroup)
        } else {
            format!("{} unit {}", g.protocol, g.talkgroup)
        };
        requests.push(HostRequest::Follow {
            freq_hz: g.freq_hz,
            signal: Some(if g.digital {
                DIGITAL_VOICE_SIGNAL
            } else {
                VOICE_SIGNAL
            }),
            talkgroup: if g.group {
                u32::try_from(g.talkgroup_id).ok()
            } else {
                None
            },
            bandwidth_hz: if g.bandwidth_hz > 0.0 {
                g.bandwidth_hz
            } else {
                VOICE_BANDWIDTH_HZ
            },
            label,
            max_s: self.max_s,
            settings: Vec::new(),
        });
        events.push(grant_event(t, g, FollowAction::Follow, None));
        self.now = Some(Followed {
            freq_hz: g.freq_hz,
            talkgroup: g.talkgroup.clone(),
            caller: g.caller.clone(),
            channel: g.channel,
            priority: g.priority,
            emergency: g.emergency,
            until: t + self.max_s,
        });
    }
    /// Ends the follow of `freq_hz` at `t` for `reason`: the host comes back.
    fn release(
        &mut self,
        freq_hz: f64,
        t: f64,
        reason: FollowReason,
        requests: &mut Vec<HostRequest>,
        events: &mut Vec<Event>,
    ) {
        let Some(f) = self.now.take_if(|f| (f.freq_hz - freq_hz).abs() < 1.0) else {
            return;
        };
        requests.push(HostRequest::Release { freq_hz });
        events.push(TRUNK_FOLLOW.event(
            t,
            freq_hz,
            &FollowEvent {
                action: FollowAction::Release,
                reason: Some(reason),
                source: (!f.caller.is_empty()).then_some(f.caller.as_str()),
                target: Some(&f.talkgroup),
                channel: (f.channel >= 0).then(|| f.channel.to_string()).as_deref(),
                urgency: urgency(f.emergency),
                family: keys::Family::Voice,
            },
        ));
        self.ended = Some((freq_hz, t));
    }
    /// The call on `freq_hz` ended at `t`: the host comes back.
    pub fn ended(
        &mut self,
        freq_hz: f64,
        t: f64,
        requests: &mut Vec<HostRequest>,
        events: &mut Vec<Event>,
    ) {
        self.release(freq_hz, t, FollowReason::Ended, requests, events);
    }
    /// Ends a follow that passed its longest time (the host ends it on its own).
    fn expire(&mut self, t: f64, requests: &mut Vec<HostRequest>, events: &mut Vec<Event>) {
        if let Some(f) = self.now.as_ref().filter(|f| t > f.until) {
            let freq = f.freq_hz;
            self.ended(freq, t, requests, events);
        }
    }
    /// Stops following (the setting went off).
    pub fn stop(&mut self, t: f64, requests: &mut Vec<HostRequest>, events: &mut Vec<Event>) {
        if let Some(freq) = self.following() {
            self.release(freq, t, FollowReason::Off, requests, events);
        }
    }
    /// The same settings with no call followed (a new stream).
    pub fn restarted(&self) -> FollowPolicy {
        FollowPolicy {
            on: self.on,
            groups: self.groups.clone(),
            groups_text: self.groups_text.clone(),
            max_s: self.max_s,
            ..FollowPolicy::default()
        }
    }
    /// The frequency followed now.
    pub fn following(&self) -> Option<f64> {
        self.now.as_ref().map(|f| f.freq_hz)
    }
    /// Takes one setting of [`FollowSetting`].
    ///
    /// # Errors
    ///
    /// [`SettingError::Malformed`] for a list item that is not a talkgroup or a range, with its byte position.
    pub fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match FollowSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FollowSetting::Follow => self.on = FOLLOW.read(id, v)?,
            FollowSetting::Groups => {
                let s = FOLLOW_GROUPS.read(id, v)?;
                self.groups = parse_groups(id, s)?;
                s.clone_into(&mut self.groups_text);
            }
            FollowSetting::MaxS => self.max_s = FOLLOW_MAX.read(id, v)?,
        }
        Ok(())
    }
    /// The present values of [`FollowSetting`].
    pub fn values(&self) -> Values {
        values_of([
            (FollowSetting::Follow, Value::Bool(self.on)),
            (FollowSetting::Groups, Value::Text(self.groups_text.clone())),
            (FollowSetting::MaxS, Value::Duration(self.max_s)),
        ])
    }
}

/// A list of talkgroups and ranges ("1201 2000-2099") for setting `id`.
///
/// # Errors
///
/// [`SettingError::Malformed`] at the byte position of the first item that is not a talkgroup or a range.
pub fn parse_groups(id: SettingId, s: &str) -> Result<IdList, SettingError> {
    let (list, refused) = IdList::parse(IdKind::Decimal, s);
    match refused.first() {
        Some(bad) => Err(SettingError::Malformed {
            id,
            at: s.find(bad.as_str()),
        }),
        None => Ok(list),
    }
}

/// `trunk.follow`: the policy as a component. Grants come in. Host requests and follow events go out.
#[derive(Default)]
pub struct FollowComponent {
    policy: FollowPolicy,
    now: f64,
    requests: Vec<HostRequest>,
    events: Vec<Event>,
}

impl Component for FollowComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "trunk.follow",
            vec![Port::typed::<Grant>("in")],
            vec![
                typed_port::<HostRequest>("requests"),
                Port::new("events", Kind::Messages).rate_from(sdrkit_graph::RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FollowSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.policy = FollowPolicy {
            on: self.policy.on,
            groups: self.policy.groups.clone(),
            groups_text: std::mem::take(&mut self.policy.groups_text),
            max_s: self.policy.max_s,
            ..FollowPolicy::default()
        };
        Ok(vec![
            typed_format::<HostRequest>(&inputs[0]),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for g in io.items::<Grant>(0) {
            self.now = self.now.max(g.time);
            self.policy
                .grant(&g.value, g.time, &mut self.requests, &mut self.events);
        }
        let t = self.now;
        io.items_out::<HostRequest>(0)
            .extend(self.requests.drain(..).map(|r| Timed { time: t, value: r }));
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let was_on = self.policy.on;
        self.policy.apply(id, v)?;
        if was_on && !self.policy.on {
            self.policy
                .stop(self.now, &mut self.requests, &mut self.events);
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.policy.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn follow(g: &Grant) -> HostRequest {
        let mut p = FollowPolicy {
            on: true,
            ..FollowPolicy::default()
        };
        let (mut r, mut e) = (Vec::new(), Vec::new());
        p.grant(g, 1.0, &mut r, &mut e);
        r.pop().expect("a follow request")
    }

    #[test]
    fn a_digital_grant_asks_for_the_digital_signal_and_its_width() {
        let base = Grant {
            protocol: "test",
            talkgroup: "100".into(),
            talkgroup_id: 100,
            freq_hz: 851e6,
            ..Grant::default()
        };
        match follow(&base) {
            HostRequest::Follow {
                signal,
                bandwidth_hz,
                ..
            } => assert_eq!(
                (signal, bandwidth_hz),
                (Some(VOICE_SIGNAL), VOICE_BANDWIDTH_HZ)
            ),
            other => panic!("{other:?}"),
        }
        let digital = Grant {
            digital: true,
            bandwidth_hz: 25e3,
            ..base
        };
        match follow(&digital) {
            HostRequest::Follow {
                signal,
                bandwidth_hz,
                ..
            } => assert_eq!((signal, bandwidth_hz), (Some(DIGITAL_VOICE_SIGNAL), 25e3)),
            other => panic!("{other:?}"),
        }
        let p = FollowPolicy {
            on: true,
            max_s: 30.0,
            ..FollowPolicy::default()
        };
        let q = p.restarted();
        assert!(q.on && q.max_s == 30.0 && q.following().is_none());
    }
}
