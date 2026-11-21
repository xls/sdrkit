//! Joining the words of a transmission into a call, and the call assembler component.

use super::sync::{AleSync, AleWord};
use super::word::{self, Lqa, CMD, DATA, FROM, REP, THRU, TIS, TO, TWS};
use sdrkit_core::ConfigError;
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, Timed};
use serde::{Deserialize, Serialize};

/// The kind of a call, before the relations give it its type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallKind {
    Sounding,
    Individual,
    Group,
    AllCall,
    AnyCall,
}

impl CallKind {
    /// The type of the call before any relation applies.
    pub fn call_type(self) -> CallType {
        match self {
            CallKind::Sounding => CallType::Sounding,
            CallKind::Individual => CallType::Individual,
            CallKind::Group => CallType::Group,
            CallKind::AllCall => CallType::AllCall,
            CallKind::AnyCall => CallType::AnyCall,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The type of a call, after the relations to the calls before it.
    #[derive(Serialize, Deserialize)]
    pub enum CallType {
        /// A sounding.
        Sounding = ("sounding", "Sounding"),
        /// A call to one station.
        Individual = ("individual", "Individual"),
        /// A call to a net address.
        Net = ("net", "Net"),
        /// A group call.
        Group = ("group", "Group"),
        /// A call to all stations.
        AllCall = ("all_call", "AllCall"),
        /// A call that any station can answer.
        AnyCall = ("any_call", "AnyCall"),
        /// The answer of the called station.
        Response = ("response", "Response"),
        /// The acknowledgement of a response.
        Ack = ("ack", "Ack"),
        /// The answer of a net member to a net call.
        NetResponse = ("net_response", "Net response"),
    }
}

impl std::fmt::Display for CallType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use sdrkit_core::Choice;
        f.write_str(Self::CHOICES[usize::from(self.index())].label.as_ref())
    }
}

/// A call (`ale.call`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AleCall {
    /// Signal time of the first word, seconds.
    pub time: f64,
    pub kind: CallKind,
    /// The caller ("" when unknown).
    pub from: String,
    /// The called addresses.
    pub to: Vec<String>,
    /// The members of a group call (THRU).
    pub members: Vec<String>,
    /// The conclusion was TIS (an answer requested).
    pub tis: bool,
    /// AMD text ("" for none).
    pub amd: String,
    /// Commands ("LQA BER 2 SINAD 24 MP 1", "CMD x 0x0123").
    pub commands: Vec<String>,
    /// The first LQA report of the call: the caller's figures of the station it calls.
    pub lqa: Option<Lqa>,
    pub words: u32,
    pub votes: u32,
    /// Vote errors / (147 x words).
    pub ber: f64,
    /// Mean word SNR in 3 kHz, dB.
    pub snr_db: f64,
    /// AFC at the end, Hz.
    pub offset_hz: f64,
}

impl Item for AleCall {
    const KIND: &'static str = "ale.call";
}

/// The LQA report of a command field's second and third characters.
pub fn lqa_text(c2: u8, c3: u8) -> String {
    Lqa::of_chars(c2, c3).text()
}

impl AleCall {
    /// The link quality of this call as received: [`Lqa::measure`] of its words.
    pub fn measured(&self) -> Lqa {
        Lqa::measure(self.words, self.votes, self.snr_db)
    }
}

/// Assembles the words of one transmission (type and characters, in order) into a call; `None` for an unknown call.
pub fn assemble(words: &[(u8, [u8; 3])]) -> Option<AssembledFields> {
    // Fields: a non-DATA/REP word opens one; DATA and REP append.
    let mut fields: Vec<(u8, Vec<u8>)> = Vec::new();
    for &(t, c) in words {
        if t == DATA || t == REP {
            if let Some(f) = fields.last_mut() {
                f.1.extend_from_slice(&c);
            }
        } else {
            fields.push((t, c.to_vec()));
        }
    }
    let mut a = AssembledFields::default();
    let mut from_from: Option<String> = None;
    let mut last_amd: Option<Vec<u8>> = None;
    let mut amd_parts: Vec<String> = Vec::new();
    for (t, text) in &fields {
        let s: String = text.iter().map(|&c| c as char).collect();
        match *t {
            TO => {
                let addr = word::trim_address(&s);
                if !a.to.contains(&addr) {
                    a.to.push(addr);
                }
            }
            THRU => {
                let addr = word::trim_address(&s);
                if !a.members.contains(&addr) {
                    a.members.push(addr);
                }
            }
            TIS | TWS => {
                a.from = Some(word::trim_address(&s));
                a.tis = Some(*t == TIS);
            }
            FROM => {
                from_from = Some(word::trim_address(&s));
            }
            CMD => {
                if text[0] >= 0x60 {
                    let cmd = if text[0] == b'a' && text.len() >= 3 {
                        a.lqa.get_or_insert(Lqa::of_chars(text[1], text[2]));
                        lqa_text(text[1], text[2])
                    } else {
                        let mut v = 0u32;
                        for &c in text.iter().skip(1).take(2) {
                            v = (v << 7) | (c as u32 & 0x7F);
                        }
                        format!("CMD {} 0x{:04X}", word::printable(text[0]), v)
                    };
                    if !a.commands.contains(&cmd) {
                        a.commands.push(cmd);
                    }
                } else {
                    if last_amd.as_deref() == Some(text.as_slice()) {
                        continue;
                    }
                    last_amd = Some(text.clone());
                    let t: String = text.iter().map(|&c| word::printable(c)).collect();
                    let t = t.trim_end().to_string();
                    if !t.is_empty() {
                        amd_parts.push(t);
                    }
                }
            }
            _ => {}
        }
    }
    if a.from.is_none() {
        a.from = from_from;
    }
    a.amd = amd_parts.join(" ");
    a.kind = if a.to.iter().any(|t| t == "@?@") {
        Some(CallKind::AllCall)
    } else if a.to.iter().any(|t| t == "@@?") {
        Some(CallKind::AnyCall)
    } else if !a.members.is_empty() {
        Some(CallKind::Group)
    } else if !a.to.is_empty() {
        Some(CallKind::Individual)
    } else if a.from.is_some() {
        Some(CallKind::Sounding)
    } else {
        None
    };
    a.kind.map(|_| a)
}

/// The fields of a call, as decoded from the words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssembledFields {
    pub kind: Option<CallKind>,
    pub from: Option<String>,
    pub to: Vec<String>,
    pub members: Vec<String>,
    pub tis: Option<bool>,
    pub amd: String,
    pub commands: Vec<String>,
    pub lqa: Option<Lqa>,
}

/// The call assembler as a component: `ale.word` in, `ale.call` out.
#[derive(Default)]
pub struct CallAssembler {
    words: Vec<AleWord>,
    calls: Vec<AleCall>,
}

impl CallAssembler {
    const MAX_WORDS: usize = 512;

    fn close(&mut self, afc: f64) {
        if self.words.is_empty() {
            return;
        }
        let list: Vec<(u8, [u8; 3])> = self.words.iter().map(|w| (w.wtype, w.chars)).collect();
        if let Some(f) = assemble(&list) {
            let n = self.words.len() as u32;
            let votes: u32 = self.words.iter().map(|w| w.votes).sum();
            let snr = self.words.iter().map(|w| w.snr_db).sum::<f64>() / n as f64;
            self.calls.push(AleCall {
                time: self.words[0].time,
                kind: f.kind.unwrap_or(CallKind::Sounding),
                from: f.from.unwrap_or_default(),
                to: f.to,
                members: f.members,
                tis: f.tis.unwrap_or(false),
                amd: f.amd,
                commands: f.commands,
                lqa: f.lqa,
                words: n,
                votes,
                ber: votes as f64 / (147.0 * n as f64),
                snr_db: snr,
                offset_hz: afc,
            });
        }
        self.words.clear();
    }
}

impl Component for CallAssembler {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.ale_call_assembler",
            vec![Port::typed::<AleSync>("words")],
            vec![Port::typed::<AleCall>("calls")],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.words.clear();
        Ok(vec![Format::new(Kind::typed::<AleCall>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for w in io.items::<AleSync>(0) {
            match &w.value {
                AleSync::Start(_) => self.words.clear(),
                AleSync::Word(x) => {
                    if self.words.len() < Self::MAX_WORDS {
                        self.words.push(x.clone());
                    }
                }
                AleSync::End { afc_hz, .. } => self.close(*afc_hz),
                AleSync::Reset(_) => self.words.clear(),
                AleSync::Status(_) => {}
            }
        }
        let o = io.items_out::<AleCall>(0);
        for c in self.calls.drain(..) {
            o.push(Timed {
                time: c.time,
                value: c,
            });
        }
    }
    fn values(&self) -> sdrkit_core::Values {
        no_values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ale::word::{address_words, amd_words, chars, lqa_word, make, wtype};

    fn tc(ws: &[u32]) -> Vec<(u8, [u8; 3])> {
        ws.iter().map(|&w| (wtype(w), chars(w))).collect()
    }

    #[test]
    fn u5_calls_assemble() {
        let mut ws = Vec::new();
        for _ in 0..3 {
            ws.extend(address_words(TO, "ABCDEFG"));
        }
        ws.push(lqa_word(3, 21, 2));
        ws.extend(amd_words("HELLO WORLD"));
        ws.extend(address_words(TIS, "XYZ1"));
        let a = assemble(&tc(&ws)).unwrap();
        assert_eq!(a.kind, Some(CallKind::Individual));
        assert_eq!(a.to, vec!["ABCDEFG".to_string()]);
        assert_eq!(a.from.as_deref(), Some("XYZ1"));
        assert_eq!(a.tis, Some(true));
        assert_eq!(a.amd, "HELLO WORLD");
        assert_eq!(a.commands, vec!["LQA BER 3 SINAD 21 MP 2".to_string()]);
        assert_eq!(
            a.lqa,
            Some(Lqa {
                ber: 3,
                sinad: 21,
                mp: 2
            })
        );

        let s: Vec<u32> = (0..3).map(|_| make(TWS, *b"ABC")).collect();
        let a = assemble(&tc(&s)).unwrap();
        assert_eq!((a.kind, a.tis), (Some(CallKind::Sounding), Some(false)));

        let g = vec![make(THRU, *b"M1@"), make(THRU, *b"M2@"), make(TIS, *b"CAL")];
        let a = assemble(&tc(&g)).unwrap();
        assert_eq!(a.kind, Some(CallKind::Group));
        assert_eq!(a.members.len(), 2);

        let ac = vec![make(TO, *b"@?@"), make(TWS, *b"CAL")];
        assert_eq!(assemble(&tc(&ac)).unwrap().kind, Some(CallKind::AllCall));
        assert!(assemble(&tc(&[make(DATA, *b"ABC")])).is_none());
    }
}
