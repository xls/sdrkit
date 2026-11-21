//! The EDACS message layer: the Standard / Networked layout and the Extended Addressing layout, the ESK byte, the texts, and
//! the builders of the test signal.
//!
//! Standard: TIA TSB-69.3, US patent 7,546,135.

/// The ESK byte, XORed into bits 27 to 20 of both messages.
pub const ESK: u32 = 0xA00_0000;

/// What a frame's messages mean.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Command {
    /// Not interpreted.
    #[default]
    Unknown,
    /// Group voice assignment (or update).
    Group,
    /// Individual call assignment (or update).
    Individual,
    /// Data assignment.
    Data,
    /// Interconnect assignment.
    Interconnect,
    /// Login acknowledge.
    Login,
    /// Status.
    Status,
    /// Site ID.
    SiteId,
    /// Adjacent site.
    Adjacent,
    /// Call end (console drop).
    CallEnd,
    /// System all-call.
    AllCall,
    /// System assigned ID.
    AssignedId,
    /// A command that is named, not acted on.
    Named,
}

/// One message read through the layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdacsMessage {
    /// Signal time, s.
    pub time: f64,
    /// Message A and B (after the ESK byte is removed).
    pub a: u32,
    /// B (0 when it failed its check).
    pub b: u32,
    /// What it is.
    pub command: Command,
    /// LCN (-1 when none).
    pub lcn: i32,
    /// Group (-1 when none).
    pub group: i64,
    /// Unit or caller (-1 when unknown).
    pub unit: i64,
    /// Called unit (individual calls, else -1).
    pub called: i64,
    /// Emergency.
    pub emergency: bool,
    /// Digital (ProVoice: not decoded).
    pub digital: bool,
    /// Transmission trunked.
    pub trunked: bool,
    /// An update of a call in progress.
    pub update: bool,
    /// Site (-1).
    pub site: i32,
    /// Area (EA, -1).
    pub area: i32,
    /// Priority.
    pub priority: i32,
    /// Index (adjacent site).
    pub index: i32,
    /// Port (data).
    pub port: i32,
    /// Status.
    pub status: i32,
    /// Failsoft, SCAT, auxiliary.
    pub failsoft: bool,
    /// SCAT.
    pub scat: bool,
    /// Auxiliary.
    pub aux: bool,
    /// The name of a named command.
    pub name: String,
    /// "Read as".
    pub text: String,
}

impl sdrkit_graph::Item for EdacsMessage {
    const KIND: &'static str = "edacs.message";
}

fn bits(x: u32, hi: u32, lo: u32) -> u32 {
    (x >> lo) & ((1u32 << (hi - lo + 1)) - 1)
}

fn bit(x: u32, b: u32) -> bool {
    (x >> b) & 1 == 1
}

/// Reads messages A and B (B = 0 when it failed). `extended`: the Extended Addressing layout. `esk`: remove the ESK byte.
pub fn read(a: u32, b: u32, extended: bool, esk: bool, time: f64) -> EdacsMessage {
    let (a, b) = if esk {
        (a ^ ESK, if b != 0 { b ^ ESK } else { 0 })
    } else {
        (a, b)
    };
    let mut m = EdacsMessage {
        time,
        a,
        b,
        lcn: -1,
        group: -1,
        unit: -1,
        called: -1,
        site: -1,
        area: -1,
        priority: -1,
        index: -1,
        port: -1,
        status: -1,
        ..Default::default()
    };
    if extended {
        read_ea(&mut m, a, b);
    } else {
        read_std(&mut m, a, b);
    }
    // There is no LCN 0, and LCNs from 26 up are status codes.
    if matches!(
        m.command,
        Command::Group | Command::Individual | Command::CallEnd
    ) && !(1..=25).contains(&m.lcn)
    {
        m.command = Command::Unknown;
    }
    m.text = text(&m, extended);
    m
}

fn named(m: &mut EdacsMessage, name: &str) {
    m.command = Command::Named;
    m.name = name.into();
}

fn read_std(m: &mut EdacsMessage, a: u32, b: u32) {
    let mta = bits(a, 27, 25);
    let mtb = bits(a, 24, 22);
    let mtd = bits(a, 21, 17);
    match mta {
        0..=3 => {
            m.command = Command::Group;
            m.digital = bit(a, 26);
            m.emergency = bit(a, 25);
            m.unit = ((bits(a, 24, 18) << 7) | bits(b, 23, 17)) as i64;
            m.lcn = bits(a, 16, 12) as i32;
            m.trunked = bit(a, 11);
            m.group = bits(a, 10, 0) as i64;
        }
        5 => {
            m.command = Command::Data;
            m.port = (bits(a, 22, 20) * 8 + bits(b, 22, 20)) as i32;
            m.lcn = bits(a, 19, 15) as i32;
            if bit(a, 14) {
                m.unit = bits(a, 13, 0) as i64;
            } else {
                m.group = bits(a, 10, 0) as i64;
            }
        }
        6 => {
            m.command = Command::Login;
            m.group = bits(a, 24, 14) as i64;
            m.unit = bits(a, 13, 0) as i64;
        }
        7 => match mtb {
            0 => {
                m.command = Command::Status;
                m.status = bits(a, 21, 14) as i32;
                m.unit = bits(a, 13, 0) as i64;
            }
            1 => {
                m.command = Command::Interconnect;
                m.lcn = bits(a, 19, 15) as i32;
                m.digital = bit(a, 20);
                m.trunked = bit(a, 21);
                if bit(a, 14) {
                    m.unit = bits(a, 13, 0) as i64;
                } else {
                    m.group = bits(a, 10, 0) as i64;
                }
            }
            3 => {
                m.update = true;
                m.lcn = bits(a, 19, 15) as i32;
                m.digital = bit(a, 20);
                m.trunked = bit(a, 21);
                if bit(a, 14) {
                    m.command = Command::Individual;
                    m.called = bits(a, 13, 0) as i64;
                    m.unit = bits(b, 13, 0) as i64;
                } else {
                    m.command = Command::Group;
                    m.group = bits(a, 10, 0) as i64;
                    m.emergency = bit(a, 13);
                }
            }
            4 => {
                m.command = Command::AssignedId;
                m.group = bits(a, 10, 0) as i64;
                m.status = bits(a, 21, 11) as i32;
            }
            5 => {
                m.command = Command::Individual;
                m.trunked = bit(a, 21);
                m.lcn = bits(a, 19, 15) as i32;
                m.digital = bit(a, 14);
                m.called = bits(a, 13, 0) as i64;
                m.unit = bits(b, 13, 0) as i64;
            }
            6 => {
                m.lcn = bits(a, 18, 14) as i32;
                m.unit = bits(a, 13, 0) as i64;
                if bit(a, 19) {
                    m.command = Command::CallEnd;
                } else {
                    named(m, "console unkey");
                }
            }
            7 => match mtd {
                0x01 => {
                    m.command = Command::Adjacent;
                    m.lcn = bits(a, 16, 12) as i32;
                    m.index = bits(a, 11, 9) as i32;
                    m.site = bits(a, 8, 4) as i32;
                }
                0x08..=0x0B => {
                    m.command = Command::SiteId;
                    m.lcn = bits(a, 16, 12) as i32;
                    m.priority = bits(a, 11, 9) as i32;
                    m.scat = bit(a, 7);
                    m.failsoft = bit(a, 6);
                    m.aux = bit(a, 5);
                    m.site = bits(a, 4, 0) as i32;
                }
                0x0F => {
                    m.command = Command::AllCall;
                    m.lcn = bits(a, 16, 12) as i32;
                    m.digital = bit(a, 11);
                    m.update = bit(a, 10);
                    m.trunked = bit(a, 9);
                    m.unit = (bits(a, 6, 0) | (bits(b, 7, 1) << 6)) as i64;
                }
                0x00 => named(m, "cancel dynamic regroup"),
                0x02 => named(m, "extended site options"),
                0x04 => named(m, "regroup plan bitmap"),
                0x05 => named(m, "auxiliary control channel"),
                0x06 => named(m, "initiate test call"),
                0x07 => named(m, "unit enable / disable"),
                0x10 => named(m, "dynamic regrouping"),
                _ => {}
            },
            _ => {}
        },
        _ => {}
    }
}

fn read_ea(m: &mut EdacsMessage, a: u32, b: u32) {
    let mt1 = bits(a, 27, 23);
    let mt2 = bits(a, 22, 19);
    match mt1 {
        0x1F => match mt2 {
            0x1 => {
                m.command = Command::Adjacent;
                m.lcn = bits(a, 16, 12) as i32;
                m.index = bits(a, 11, 8) as i32;
                m.site = bits(a, 7, 0) as i32;
            }
            0xA => {
                m.command = Command::SiteId;
                m.site = (bits(a, 14, 12) * 32 + bits(a, 4, 0)) as i32;
                m.area = bits(a, 11, 5) as i32;
            }
            0x0 => named(m, "initiate test call"),
            0xB => named(m, "dynamic regroup plan bitmap"),
            0xC => named(m, "supergroup patch"),
            0x4 | 0x7 | 0x8 | 0xD => named(m, "system command"),
            _ => {}
        },
        0x03 | 0x06 => {
            m.command = Command::Group;
            m.digital = mt1 == 0x03;
            m.lcn = bits(a, 21, 17) as i32;
            m.update = bit(a, 16);
            m.group = bits(a, 15, 0) as i64;
            m.trunked = bit(b, 21);
            m.emergency = bit(b, 20);
            m.unit = bits(b, 19, 0) as i64;
        }
        0x10 => {
            m.command = Command::Individual;
            m.digital = bit(a, 21);
            m.update = bit(a, 20);
            m.called = bits(a, 19, 0) as i64;
            m.lcn = bits(b, 24, 20) as i32;
            m.unit = bits(b, 19, 0) as i64;
        }
        0x02 => {
            m.command = Command::Data;
            m.group = bits(a, 15, 0) as i64;
            m.unit = bits(b, 19, 0) as i64;
        }
        0x16 => {
            m.command = Command::AllCall;
            m.digital = bit(a, 16);
            m.update = bit(a, 15);
            m.unit = bits(b, 19, 0) as i64;
        }
        0x01 => named(m, "TDMA group grant"),
        0x12 => named(m, "call end / unkey"),
        0x19 => named(m, "group message"),
        _ => {}
    }
}

fn text(m: &EdacsMessage, extended: bool) -> String {
    let lcn = m.lcn;
    match m.command {
        Command::Group => {
            let mut s = format!(
                "group {}, LCN {lcn}, group {}",
                if m.update { "update" } else { "call" },
                m.group
            );
            if m.unit >= 0 {
                s.push_str(&format!(", unit {}", m.unit));
            }
            if m.emergency {
                s.push_str(", emergency");
            }
            if m.digital {
                s.push_str(", digital");
            }
            s
        }
        Command::Individual => {
            let mut s = format!(
                "individual call, LCN {lcn}, unit {}, from {}",
                m.called, m.unit
            );
            if m.digital {
                s.push_str(", digital");
            }
            s
        }
        Command::Data => {
            if extended {
                format!("data call, group {}, unit {}", m.group, m.unit)
            } else if m.unit >= 0 {
                format!("data call, LCN {lcn}, unit {}, port {}", m.unit, m.port)
            } else {
                format!("data call, LCN {lcn}, group {}, port {}", m.group, m.port)
            }
        }
        Command::Interconnect => {
            if m.unit >= 0 {
                format!("interconnect, LCN {lcn}, unit {}", m.unit)
            } else {
                format!("interconnect, LCN {lcn}, group {}", m.group)
            }
        }
        Command::SiteId => {
            if extended {
                format!("site {}, area {}", m.site, m.area)
            } else {
                let mut s = format!(
                    "site {}, control channel LCN {lcn}, priority {}",
                    m.site, m.priority
                );
                if m.failsoft {
                    s.push_str(", failsoft");
                }
                if m.scat {
                    s.push_str(", SCAT");
                }
                if m.aux {
                    s.push_str(", auxiliary");
                }
                s
            }
        }
        Command::Adjacent => format!(
            "adjacent site {}, control channel LCN {lcn}, index {}",
            m.site, m.index
        ),
        Command::CallEnd => format!("call end (drop), LCN {lcn}"),
        Command::Login => format!("login, group {}, unit {}", m.group, m.unit),
        Command::Status => {
            if m.status == 248 {
                format!("status request, unit {}", m.unit)
            } else {
                format!("status {:03}, unit {}", m.status, m.unit)
            }
        }
        Command::AllCall => {
            if lcn >= 0 {
                format!("system all-call, LCN {lcn}")
            } else {
                "system all-call".into()
            }
        }
        Command::AssignedId => format!("system assigned ID {}, group {}", m.status, m.group),
        Command::Named => m.name.clone(),
        Command::Unknown => "not interpreted".into(),
    }
}

/// Builds the messages of a command (for the test signal and the tests). The fields used are those of the layout.
pub mod build {
    use super::ESK;

    fn put(x: &mut u32, hi: u32, lo: u32, v: u32) {
        let mask = ((1u32 << (hi - lo + 1)) - 1) << lo;
        *x = (*x & !mask) | ((v << lo) & mask);
    }

    /// Applies the ESK byte.
    pub fn esk((a, b): (u32, u32)) -> (u32, u32) {
        (a ^ ESK, b ^ ESK)
    }

    /// Standard group voice assignment.
    #[allow(clippy::too_many_arguments)]
    pub fn group(
        lcn: u32,
        group: u32,
        unit: u32,
        emergency: bool,
        digital: bool,
        trunked: bool,
    ) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 27, 0);
        put(&mut a, 26, 26, digital as u32);
        put(&mut a, 25, 25, emergency as u32);
        put(&mut a, 24, 18, unit >> 7);
        put(&mut a, 16, 12, lcn);
        put(&mut a, 11, 11, trunked as u32);
        put(&mut a, 10, 0, group);
        let mut b = 0;
        put(&mut b, 23, 17, unit & 0x7F);
        (a, b)
    }
    /// Standard channel update of a group call.
    pub fn group_update(
        lcn: u32,
        group: u32,
        emergency: bool,
        digital: bool,
        trunked: bool,
    ) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 25, 7);
        put(&mut a, 24, 22, 3);
        put(&mut a, 21, 21, trunked as u32);
        put(&mut a, 20, 20, digital as u32);
        put(&mut a, 19, 15, lcn);
        put(&mut a, 13, 13, emergency as u32);
        put(&mut a, 10, 0, group);
        (a, 0)
    }
    /// Standard individual call assignment.
    pub fn individual(
        lcn: u32,
        called: u32,
        caller: u32,
        digital: bool,
        trunked: bool,
    ) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 25, 7);
        put(&mut a, 24, 22, 5);
        put(&mut a, 21, 21, trunked as u32);
        put(&mut a, 19, 15, lcn);
        put(&mut a, 14, 14, digital as u32);
        put(&mut a, 13, 0, called);
        let mut b = 0;
        put(&mut b, 13, 0, caller);
        (a, b)
    }
    /// Standard site ID.
    pub fn site_id(
        site: u32,
        lcn: u32,
        priority: u32,
        failsoft: bool,
        scat: bool,
        aux: bool,
    ) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 25, 7);
        put(&mut a, 24, 22, 7);
        put(&mut a, 21, 17, 0x08);
        put(&mut a, 16, 12, lcn);
        put(&mut a, 11, 9, priority);
        put(&mut a, 7, 7, scat as u32);
        put(&mut a, 6, 6, failsoft as u32);
        put(&mut a, 5, 5, aux as u32);
        put(&mut a, 4, 0, site);
        (a, 0)
    }
    /// Standard call end (console drop).
    pub fn call_end(lcn: u32, unit: u32) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 25, 7);
        put(&mut a, 24, 22, 6);
        put(&mut a, 19, 19, 1);
        put(&mut a, 18, 14, lcn);
        put(&mut a, 13, 0, unit);
        (a, 0)
    }
    /// Standard data assignment.
    pub fn data(lcn: u32, port: u32, unit: Option<u32>, group: u32) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 25, 5);
        put(&mut a, 24, 24, unit.is_some() as u32);
        put(&mut a, 22, 20, port >> 3);
        put(&mut a, 19, 15, lcn);
        match unit {
            Some(u) => {
                put(&mut a, 14, 14, 1);
                put(&mut a, 13, 0, u);
            }
            None => put(&mut a, 10, 0, group),
        }
        let mut b = 0;
        put(&mut b, 22, 20, port & 7);
        (a, b)
    }
    /// Standard login acknowledge.
    pub fn login(group: u32, unit: u32) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 25, 6);
        put(&mut a, 24, 14, group);
        put(&mut a, 13, 0, unit);
        (a, 0)
    }
    /// Standard status.
    pub fn status(status: u32, unit: u32) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 25, 7);
        put(&mut a, 24, 22, 0);
        put(&mut a, 21, 14, status);
        put(&mut a, 13, 0, unit);
        (a, 0)
    }
    /// Standard adjacent site.
    pub fn adjacent(lcn: u32, index: u32, site: u32) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 25, 7);
        put(&mut a, 24, 22, 7);
        put(&mut a, 21, 17, 0x01);
        put(&mut a, 16, 12, lcn);
        put(&mut a, 11, 9, index);
        put(&mut a, 8, 4, site);
        (a, 0)
    }
    /// Standard system all-call.
    pub fn all_call(lcn: u32, unit: u32) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 25, 7);
        put(&mut a, 24, 22, 7);
        put(&mut a, 21, 17, 0x0F);
        put(&mut a, 16, 12, lcn);
        put(&mut a, 6, 0, unit & 0x7F);
        let mut b = 0;
        put(&mut b, 7, 1, unit >> 6);
        (a, b)
    }
    /// Standard idle filler: a reserved command (MT-A 7, MT-B 7, MT-D 1F) with random content.
    pub fn filler_std(random: u32) -> (u32, u32) {
        let mut a = random & 0xFFF_FFFF;
        put(&mut a, 27, 25, 7);
        put(&mut a, 24, 22, 7);
        put(&mut a, 21, 17, 0x1F);
        (a, random.rotate_left(13) & 0xFFF_FFFF)
    }
    /// EA group voice assignment.
    pub fn ea_group(
        lcn: u32,
        group: u32,
        caller: u32,
        emergency: bool,
        digital: bool,
        trunked: bool,
        update: bool,
    ) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 23, if digital { 0x03 } else { 0x06 });
        put(&mut a, 21, 17, lcn);
        put(&mut a, 16, 16, update as u32);
        put(&mut a, 15, 0, group);
        let mut b = 0;
        put(&mut b, 21, 21, trunked as u32);
        put(&mut b, 20, 20, emergency as u32);
        put(&mut b, 19, 0, caller);
        (a, b)
    }
    /// EA individual call.
    pub fn ea_individual(lcn: u32, called: u32, caller: u32, digital: bool) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 23, 0x10);
        put(&mut a, 21, 21, digital as u32);
        put(&mut a, 19, 0, called);
        let mut b = 0;
        put(&mut b, 24, 20, lcn);
        put(&mut b, 19, 0, caller);
        (a, b)
    }
    /// EA site ID.
    pub fn ea_site(site: u32, area: u32) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 23, 0x1F);
        put(&mut a, 22, 19, 0xA);
        put(&mut a, 14, 12, site >> 5);
        put(&mut a, 11, 5, area);
        put(&mut a, 4, 0, site & 31);
        (a, 0)
    }
    /// EA adjacent site.
    pub fn ea_adjacent(lcn: u32, index: u32, site: u32) -> (u32, u32) {
        let mut a = 0;
        put(&mut a, 27, 23, 0x1F);
        put(&mut a, 22, 19, 0x1);
        put(&mut a, 16, 12, lcn);
        put(&mut a, 11, 8, index);
        put(&mut a, 7, 0, site);
        (a, 0)
    }
    /// EA idle filler (MT1 1F, MT2 E) with random content.
    pub fn filler_ea(random: u32) -> (u32, u32) {
        let mut a = random & 0xFFF_FFFF;
        put(&mut a, 27, 23, 0x1F);
        put(&mut a, 22, 19, 0xE);
        (a, random.rotate_left(13) & 0xFFF_FFFF)
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    #[test]
    fn d10_facts() {
        let m = read(0x4907AAB, 0x0680000, false, false, 0.0);
        assert_eq!(m.command, Command::Group);
        assert!(m.digital && !m.emergency && m.trunked);
        assert_eq!((m.unit, m.lcn, m.group), (0x1234, 7, 0x2AB));
        let m = read(0xF661ABC, 0x0000777, false, false, 0.0);
        assert_eq!(m.command, Command::Individual);
        assert!(m.trunked && !m.digital);
        assert_eq!((m.lcn, m.called, m.unit), (12, 0x1ABC, 0x777));
        let m = read(0xFD01651, 0, false, false, 0.0);
        assert_eq!(m.command, Command::SiteId);
        assert_eq!((m.lcn, m.priority, m.site), (1, 3, 17));
        assert!(!m.scat && m.failsoft && !m.aux);
        let m = read(0xF8D3001, 0, false, false, 0.0);
        assert_eq!((m.command, m.lcn, m.unit), (Command::CallEnd, 20, 0x3001));
        let m = read(0x192BEEF, 0x0354321, true, false, 0.0);
        assert_eq!(m.command, Command::Group);
        assert!(m.digital && !m.update && m.trunked && m.emergency);
        assert_eq!((m.lcn, m.group, m.unit), (9, 0xBEEF, 0x54321));
        let m = read(0xFD05AB7, 0, true, false, 0.0);
        assert_eq!((m.command, m.site, m.area), (Command::SiteId, 183, 85));
        let (a, b) = group(3, 77, 10, false, false, false);
        assert_eq!(
            read(a, b, false, false, 0.0).text,
            "group call, LCN 3, group 77, unit 10"
        );
        let (a, b) = group(3, 77, 10, false, false, false);
        let (ea, eb) = esk((a, b));
        assert_eq!((ea ^ a, eb ^ b), (0xA000000, 0xA000000));
        let r = read(ea, eb, false, true, 0.0);
        assert_eq!((r.group, r.lcn), (77, 3));
        let r = read(ea, eb, false, false, 0.0);
        assert!(r.group != 77 || r.lcn != 3 || r.command != Command::Group);
        assert_eq!(
            read(0xF88C000, 0, false, false, 0.0).text,
            "call end (drop), LCN 3"
        );
        assert_eq!(
            read(0x0843309, 0x0C20000, false, false, 0.0).text,
            "group call, LCN 3, group 777, unit 4321"
        );
        assert_eq!(
            read(0x2AC6378, 0x0660000, false, false, 0.0).text,
            "group call, LCN 6, group 888, unit 5555, emergency"
        );
        assert_eq!(
            read(0xFD01205, 0x08C7CDB, false, false, 0.0).text,
            "site 5, control channel LCN 1, priority 1"
        );
    }

    #[test]
    fn d10_round_trips() {
        let r = |(a, b): (u32, u32), ea: bool| read(a, b, ea, false, 0.0);
        let m = r(group(17, 2000, 16000, true, true, true), false);
        assert_eq!(
            (m.lcn, m.group, m.unit, m.emergency, m.digital, m.trunked),
            (17, 2000, 16000, true, true, true)
        );
        let m = r(group_update(17, 2000, true, false, true), false);
        assert!(m.update && m.emergency && m.group == 2000 && m.lcn == 17);
        let m = r(individual(25, 16383, 12345, true, false), false);
        assert_eq!(
            (m.lcn, m.called, m.unit, m.digital),
            (25, 16383, 12345, true)
        );
        let m = r(site_id(31, 4, 7, true, true, true), false);
        assert_eq!(
            (m.site, m.lcn, m.priority, m.failsoft, m.scat, m.aux),
            (31, 4, 7, true, true, true)
        );
        assert_eq!(r(call_end(9, 0), false).lcn, 9);
        let m = r(data(3, 42, Some(9000), 0), false);
        assert_eq!(
            (m.command, m.lcn, m.port, m.unit),
            (Command::Data, 3, 42, 9000)
        );
        let m = r(login(1500, 8000), false);
        assert_eq!((m.group, m.unit), (1500, 8000));
        let m = r(status(248, 5), false);
        assert_eq!(m.text, "status request, unit 5");
        let m = r(adjacent(6, 5, 20), false);
        assert_eq!((m.lcn, m.index, m.site), (6, 5, 20));
        let m = r(all_call(2, 5000), false);
        assert_eq!((m.command, m.lcn), (Command::AllCall, 2));
        let m = r(
            ea_group(18, 65535, 1048575, false, false, true, false),
            true,
        );
        assert_eq!((m.lcn, m.group, m.unit), (18, 65535, 1048575));
        let m = r(ea_individual(5, 1000000, 999999, false), true);
        assert_eq!((m.lcn, m.called, m.unit), (5, 1000000, 999999));
        let m = r(ea_site(255, 127), true);
        assert_eq!((m.site, m.area), (255, 127));
        let m = r(ea_adjacent(11, 9, 200), true);
        assert_eq!((m.lcn, m.index, m.site), (11, 9, 200));
        // D11: no LCN 0, LCN 28 is a status code.
        assert_eq!(
            r(group(0, 1, 1, false, false, false), false).command,
            Command::Unknown
        );
        assert_eq!(
            r(group(28, 1, 1, false, false, false), false).command,
            Command::Unknown
        );
    }

    #[test]
    fn d13_random_pairs() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(13);
        let mut known = 0;
        for _ in 0..20000 {
            let a = (rng.next_u64() & 0xFFF_FFFF) as u32;
            let b = (rng.next_u64() & 0xFFF_FFFF) as u32;
            if read(a, b, false, false, 0.0).command != Command::Unknown {
                known += 1;
            }
            // The extended layout reads any pair of words without a panic.
            read(a, b, true, true, 0.0);
        }
        assert!((3000..=18000).contains(&known), "{known}");
    }
}
