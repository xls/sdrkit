//! Transport packet sources for generators and tests: numbered packets that a receiver test can check one by one, a test
//! multiplex with real PSI and SI (or an ATSC virtual channel table), a sound and a picture track, a transport stream file,
//! and null packets.

use crate::error::{Error, MuxFault};
use crate::input::TsAligner;
use crate::packet::{packetize_section, write_header, Header, NULL_PID, TS_SIZE};
use crate::pes::build_pes;
use crate::tables::{build_long, build_pat, build_pmt, build_sdt, descriptor, loop_bytes};
use std::collections::VecDeque;

/// Something that returns transport packets, one at a time.
pub trait PacketSource: Send {
    /// Writes the next packet into `out`.
    fn next_packet(&mut self, out: &mut [u8; TS_SIZE]);
}

/// A null packet (PID 0x1FFF, payload all 0xFF).
pub fn null_packet() -> [u8; TS_SIZE] {
    let mut p = [0xFFu8; TS_SIZE];
    write_header(
        &mut p,
        &Header {
            pid: NULL_PID,
            afc: 1,
            ..Default::default()
        },
    );
    p
}

/// Null packets only.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullSource;

impl PacketSource for NullSource {
    fn next_packet(&mut self, out: &mut [u8; TS_SIZE]) {
        *out = null_packet();
    }
}

/// The fill of bytes 8 to 187 of a numbered packet. Bytes 4 to 7 hold the number `n` (big-endian). The header is a
/// payload-only packet on one PID with the continuity counter `n mod 16`. The DVB-S, DVB-S2, DVB-C, ISDB-T, ATSC and DTMB
/// test streams use [`Fill::N7I13`], the DVB-T ones [`Fill::I31N7`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fill {
    /// Byte `i` is `(7 n + 13 i) mod 256`.
    #[default]
    N7I13,
    /// Byte `i` is `(31 i + 7 n) mod 256`.
    I31N7,
}

impl Fill {
    fn byte(self, n: u32, i: usize) -> u8 {
        let i = i as u32;
        match self {
            Fill::N7I13 => 7u32.wrapping_mul(n).wrapping_add(13 * i) as u8,
            Fill::I31N7 => i.wrapping_mul(31).wrapping_add(n.wrapping_mul(7)) as u8,
        }
    }
}

/// Numbered packets on one PID with one fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Numbered {
    /// The PID (13 bits).
    pub pid: u16,
    /// The fill.
    pub fill: Fill,
}

impl Numbered {
    /// The numbered packets of the DVB-S, DVB-S2 and DVB-C recordings: PID 0x0020, [`Fill::N7I13`].
    pub const SATELLITE_CABLE: Numbered = Numbered::new(0x0020, Fill::N7I13);
    /// The numbered packets of the ISDB-T, ATSC and DTMB vectors: PID 0x0100, [`Fill::N7I13`].
    pub const TERRESTRIAL: Numbered = Numbered::new(0x0100, Fill::N7I13);

    /// Numbered packets on `pid` (only the low 13 bits are used) with `fill`.
    pub const fn new(pid: u16, fill: Fill) -> Numbered {
        Numbered {
            pid: pid & 0x1FFF,
            fill,
        }
    }

    /// Packet number `n`.
    pub fn packet(&self, n: u32) -> [u8; TS_SIZE] {
        let mut p = [0u8; TS_SIZE];
        p[0] = 0x47;
        p[1] = (self.pid >> 8) as u8;
        p[2] = self.pid as u8;
        p[3] = 0x10 | (n & 15) as u8;
        p[4..8].copy_from_slice(&n.to_be_bytes());
        for (i, b) in p.iter_mut().enumerate().skip(8) {
            *b = self.fill.byte(n, i);
        }
        p
    }

    /// Returns the number of a packet when every byte matches this pattern. Returns `None` for a packet of another PID, with the
    /// transport_error_indicator, or with any wrong byte.
    pub fn number_of(&self, p: &[u8]) -> Option<u32> {
        if p.len() < TS_SIZE {
            return None;
        }
        let n = u32::from_be_bytes([p[4], p[5], p[6], p[7]]);
        (self.packet(n)[..] == p[..TS_SIZE]).then_some(n)
    }
}

/// Numbered packets from 0, one after the other.
#[derive(Clone, Copy, Debug)]
pub struct NumberedSource {
    /// The pattern.
    pub pattern: Numbered,
    /// The number of the next packet.
    pub next: u32,
}

impl NumberedSource {
    /// Numbered packets of `pattern` from number 0.
    pub fn new(pattern: Numbered) -> Self {
        NumberedSource { pattern, next: 0 }
    }
}

impl PacketSource for NumberedSource {
    fn next_packet(&mut self, out: &mut [u8; TS_SIZE]) {
        *out = self.pattern.packet(self.next);
        self.next = self.next.wrapping_add(1);
    }
}

/// What a check of delivered numbered packets found. It counts packets of the pattern's PID only (other packets are counted in
/// `other`), so a test multiplex with PSI and null packets can be checked as it is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NumberedCheck {
    /// Packets that match the pattern.
    pub good: u64,
    /// Packets of the PID without the error flag that do not match.
    pub wrong: u64,
    /// Packets of the PID with the transport_error_indicator.
    pub flagged: u64,
    /// Packets of other PIDs.
    pub other: u64,
    /// Breaks in the numbering between good packets that flagged or wrong packets do not explain.
    pub gaps: u64,
    /// The first good number.
    pub first: Option<u32>,
    /// The last good number.
    pub last: Option<u32>,
    /// The signal time of the first good packet.
    pub first_time: Option<f64>,
    bad_since: bool,
}

impl NumberedCheck {
    /// Counts one delivered packet of `pattern` at signal time `time`.
    pub fn push(&mut self, pattern: &Numbered, p: &[u8], time: f64) {
        if p.len() < TS_SIZE {
            self.wrong += 1;
            self.bad_since = true;
            return;
        }
        let pid = ((p[1] as u16 & 0x1F) << 8) | p[2] as u16;
        if pid != pattern.pid {
            self.other += 1;
            return;
        }
        if p[1] & 0x80 != 0 {
            self.flagged += 1;
            self.bad_since = true;
            return;
        }
        match pattern.number_of(p) {
            Some(n) => {
                self.good += 1;
                self.first_time.get_or_insert(time);
                self.first.get_or_insert(n);
                if let Some(l) = self.last {
                    if n != l.wrapping_add(1) && !self.bad_since {
                        self.gaps += 1;
                    }
                }
                self.last = Some(n);
                self.bad_since = false;
            }
            None => {
                self.wrong += 1;
                self.bad_since = true;
            }
        }
    }
}

/// Packets from a transport stream file read into memory, from its first packet.
#[derive(Clone, Debug)]
pub struct FileSource {
    packets: Vec<[u8; TS_SIZE]>,
    at: usize,
    /// Start again at the end. If false, null packets follow the end.
    pub looping: bool,
}

impl FileSource {
    /// From the bytes of a `.ts` file (188-, 192- or 204-byte packets, aligned on their sync bytes).
    pub fn new(ts: &[u8]) -> Self {
        let mut packets = Vec::with_capacity(ts.len() / TS_SIZE);
        let mut a = TsAligner::new();
        let mut take = |p: crate::input::Aligned| {
            let mut b = [0u8; TS_SIZE];
            b.copy_from_slice(p.bytes);
            packets.push(b);
        };
        a.push(ts, &mut take);
        a.finish(&mut take);
        FileSource {
            packets,
            at: 0,
            looping: true,
        }
    }

    /// The packets.
    pub fn packets(&self) -> &[[u8; TS_SIZE]] {
        &self.packets
    }
}

impl PacketSource for FileSource {
    fn next_packet(&mut self, out: &mut [u8; TS_SIZE]) {
        if self.at >= self.packets.len() {
            if self.looping && !self.packets.is_empty() {
                self.at = 0;
            } else {
                *out = null_packet();
                return;
            }
        }
        *out = self.packets[self.at];
        self.at += 1;
    }
}

/// The network of a test multiplex: it gets a NIT with the network name, the service list and the logical channel numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestNetwork {
    /// network_id (also the original_network_id).
    pub network_id: u16,
    /// The network name.
    pub name: String,
}

/// One service of a test multiplex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestService {
    /// service_id (also the program_number).
    pub service_id: u16,
    /// The name in the SDT.
    pub name: String,
    /// service_type (EN 300 468 Table 87: 0x01 TV, 0x02 radio ...).
    pub service_type: u8,
    /// The logical channel number in the NIT (0: none).
    pub lcn: u16,
}

impl TestService {
    /// A TV service.
    pub fn tv(service_id: u16, name: &str, lcn: u16) -> Self {
        TestService {
            service_id,
            name: name.into(),
            service_type: 0x01,
            lcn,
        }
    }

    /// A radio service.
    pub fn radio(service_id: u16, name: &str, lcn: u16) -> Self {
        TestService {
            service_type: 0x02,
            ..Self::tv(service_id, name, lcn)
        }
    }
}

/// What a [`TestMux`] sends in the packets that are not tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Data {
    /// Numbered packets with this fill, on the data PID.
    Numbered(Fill),
    /// Only null packets.
    None,
}

/// The sound track of the first service of a [`TestMux`]: a tone of [`TestSound::TONE_HZ`] in MPEG-1 Layer II frames
/// (ISO/IEC 11172-3) of 48 kHz stereo at 128 kbit/s, one frame in a PES packet (stream type 0x03, stream_id 0xC0) with
/// its PTS, and a PCR in the first transport packet of each PES packet. The PID is the PCR PID of the service. The
/// frames go out at the time that the bit rate of the multiplex gives each packet, in the slots of null packets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestSound {
    /// The PID of the sound.
    pub pid: u16,
    /// The bit rate of the multiplex, bit/s.
    pub mux_bit_rate: f64,
}

impl TestSound {
    /// The frequency of the tone, Hz.
    pub const TONE_HZ: f32 = 1000.0;
    /// The amplitude of the tone, of full scale.
    pub const AMPLITUDE: f32 = 0.25;
    /// The sample rate, Hz.
    pub const SAMPLE_RATE: u32 = 48_000;
    /// The bit rate of the sound, kbit/s.
    pub const BIT_RATE_KBPS: u16 = 128;
    /// The time from the PCR of a packet to the PTS of the frame that it starts, s.
    pub const DELAY_S: f64 = 0.25;
}

/// The picture track of the first service of a [`TestMux`]: eight vertical colour bars of 75 % (ITU-R BT.801) in a picture
/// of [`TestVideo::WIDTH`] by [`TestVideo::HEIGHT`] at 25 frames per second, MPEG-2 video main profile at main level
/// (ISO/IEC 13818-2). Every picture is the same intra picture with its sequence header and a closed group of pictures, one
/// picture in a PES packet (stream type 0x02, stream_id 0xE0) with its PTS. Without a [`TestSound`] the video carries the
/// PCR. The pictures go out at the time that the bit rate of the multiplex gives each packet, in the slots of null
/// packets that the sound leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestVideo {
    /// The PID of the video.
    pub pid: u16,
    /// The bit rate of the multiplex, bit/s.
    pub mux_bit_rate: f64,
}

impl TestVideo {
    /// The width of the picture, pixels.
    pub const WIDTH: u32 = 256;
    /// The height of the picture, pixels.
    pub const HEIGHT: u32 = 144;
    /// The frame rate, Hz.
    pub const FRAME_RATE: f64 = 25.0;
    /// The colours of the bars from left to right, Y, Cb and Cr: white, yellow, cyan, green, magenta, red, blue, black.
    pub const BARS: [[u8; 3]; 8] = [
        [180, 128, 128],
        [162, 44, 142],
        [131, 156, 44],
        [112, 72, 58],
        [84, 184, 198],
        [65, 100, 212],
        [35, 212, 114],
        [16, 128, 128],
    ];
    /// The time from the PCR of a packet to the PTS of the picture that it starts, s.
    pub const DELAY_S: f64 = 0.25;
    /// The bit rate that the sequence header states, bit/s.
    pub const BIT_RATE: u32 = 400_000;

    /// The access unit of one picture: the sequence header and its extension, the group of pictures header, the picture
    /// header and its coding extension, and one slice per macroblock row. Each macroblock is intra coded with only the DC
    /// coefficient of each block (ISO/IEC 13818-2 6.2 and tables B-1, B-2, B-12, B-13 and B-14).
    pub fn picture() -> Vec<u8> {
        use sdrkit_codec_mpeg2video::tables::{
            ADDRESS_INCREMENT, COEFF_END_ZERO, DC_SIZE_CHROMA, DC_SIZE_LUMA, MB_INTRA, TYPE_I,
        };
        let code = |table: &[(&'static str, u8)], v: u8| {
            table.iter().find(|e| e.1 == v).map_or("", |e| e.0)
        };
        let mut b = Bits::default();
        b.start(0xB3);
        b.put(Self::WIDTH, 12);
        b.put(Self::HEIGHT, 12);
        b.put(3, 4); // aspect ratio 16:9
        b.put(3, 4); // 25 Hz
        b.put(Self::BIT_RATE / 400, 18);
        b.put(1, 1); // marker
        b.put(20, 10); // vbv_buffer_size, units of 16 kbit
        b.put(0, 3); // not constrained, the default quantiser matrices
        b.start(0xB5);
        b.put(1, 4); // sequence extension
        b.put(0x48, 8); // main profile at main level
        b.put(1, 1); // progressive sequence
        b.put(1, 2); // 4:2:0
        b.put(0, 4); // size extensions
        b.put(0, 12); // bit rate extension
        b.put(1, 1); // marker
        b.put(0, 8); // vbv_buffer_size extension
        b.put(0, 1); // low_delay
        b.put(0, 7); // frame rate extension
        b.start(0xB8);
        b.put(1 << 12, 25); // time code 0 with its marker bit
        b.put(1, 1); // closed_gop
        b.put(0, 1); // broken_link
        b.start(0x00);
        b.put(0, 10); // temporal_reference
        b.put(1, 3); // intra picture
        b.put(0xFFFF, 16); // vbv_delay
        b.put(0, 1); // extra_bit_picture
        b.start(0xB5);
        b.put(8, 4); // picture coding extension
        b.put(0xFFFF, 16); // f_codes, not used
        b.put(0, 2); // intra_dc_precision 8 bits
        b.put(3, 2); // frame picture
        b.put(0, 1); // top_field_first
        b.put(1, 1); // frame_pred_frame_dct
        b.put(0, 5); // concealment vectors, q_scale_type, intra_vlc_format, alternate_scan, repeat_first_field
        b.put(1, 1); // chroma_420_type
        b.put(1, 1); // progressive_frame
        b.put(0, 1); // composite_display_flag
        let columns = Self::WIDTH / 16;
        for row in 0..Self::HEIGHT / 16 {
            b.start(row as u8 + 1);
            b.put(8, 5); // quantiser_scale_code
            b.put(0, 1); // extra_bit_slice
            let mut dc = [128i32; 3];
            for mx in 0..columns {
                let bar = Self::BARS[(mx * 8 / columns) as usize];
                b.code(code(ADDRESS_INCREMENT, 1));
                b.code(code(TYPE_I, MB_INTRA));
                for c in [0, 0, 0, 0, 1, 2] {
                    let diff = i32::from(bar[c]) - dc[c];
                    dc[c] = i32::from(bar[c]);
                    let size = 32 - diff.unsigned_abs().leading_zeros();
                    let table = if c == 0 { DC_SIZE_LUMA } else { DC_SIZE_CHROMA };
                    b.code(code(table, size as u8));
                    if size > 0 {
                        let v = if diff < 0 { diff + (1 << size) - 1 } else { diff };
                        b.put(v as u32, size);
                    }
                    b.code(COEFF_END_ZERO);
                }
            }
        }
        b.out
    }
}

/// A bit writer for [`TestVideo::picture`]: bits from the most significant, start codes byte aligned.
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    fill: u32,
}

impl Bits {
    fn put(&mut self, v: u32, n: u32) {
        for i in (0..n).rev() {
            if self.fill == 0 {
                self.out.push(0);
            }
            if let Some(last) = self.out.last_mut() {
                *last |= (((v >> i) & 1) as u8) << (7 - self.fill);
            }
            self.fill = (self.fill + 1) % 8;
        }
    }

    /// A variable-length code written as text, such as "1111 0".
    fn code(&mut self, code: &str) {
        for c in code.bytes().filter(|c| *c != b' ') {
            self.put(u32::from(c == b'1'), 1);
        }
    }

    fn start(&mut self, code: u8) {
        self.fill = 0;
        self.out.extend([0, 0, 1, code]);
    }
}

/// How to make a [`TestMux`]. Start with [`TestMux::builder`].
#[derive(Clone, Debug)]
pub struct TestMuxBuilder {
    tsid: u16,
    services: Vec<TestService>,
    network: Option<TestNetwork>,
    provider: String,
    data_pid: u16,
    data: Data,
    period: usize,
    data_every: u64,
    sound: Option<TestSound>,
    video: Option<TestVideo>,
    vct_major: Option<u16>,
}

impl TestMuxBuilder {
    /// Adds a service. Its PMT goes on PID `0x0100 + 0x10 k` for the `k`-th service.
    pub fn service(mut self, s: TestService) -> Self {
        self.services.push(s);
        self
    }

    /// Adds a network and its NIT (PID 0x0010).
    pub fn network(mut self, n: TestNetwork) -> Self {
        self.network = Some(n);
        self
    }

    /// The provider name in the SDT (default "sdrkit").
    pub fn provider(mut self, name: &str) -> Self {
        self.provider = name.into();
        self
    }

    /// The data stream: its PID (also every service's PCR PID, stream type 0x06) and what it carries. Default: PID 0x0101,
    /// numbered packets with [`Fill::N7I13`].
    pub fn data(mut self, pid: u16, data: Data) -> Self {
        self.data_pid = pid;
        self.data = data;
        self
    }

    /// The tables repeat every `packets` packets (default 60).
    pub fn period(mut self, packets: usize) -> Self {
        self.period = packets;
        self
    }

    /// One packet in `n` that is not a table is a data packet, the rest are null packets (default 1: no null packets).
    pub fn data_every(mut self, n: u64) -> Self {
        self.data_every = n;
        self
    }

    /// Gives the first service the sound track `sound`. The sound needs null packets: a `data_every` of 1 leaves it
    /// no slot.
    pub fn sound(mut self, sound: TestSound) -> Self {
        self.sound = Some(sound);
        self
    }

    /// Gives the first service the picture track `video`. As the sound, it needs null packets.
    pub fn video(mut self, video: TestVideo) -> Self {
        self.video = Some(video);
        self
    }

    /// Puts the services into a terrestrial virtual channel table (ATSC A/65 6.3.1, PID 0x1FFB) in place of the SDT:
    /// service `k` is channel `major`.`k + 1`, with the first seven characters of its name. A TV service is a digital
    /// television channel, a radio service an audio channel, and any other a data channel.
    pub fn virtual_channels(mut self, major: u16) -> Self {
        self.vct_major = Some(major);
        self
    }

    /// Makes the multiplex.
    pub fn build(self) -> Result<TestMux, Error> {
        if self.services.is_empty() {
            return Err(MuxFault::NoServices.into());
        }
        for (i, s) in self.services.iter().enumerate() {
            if self.services[..i]
                .iter()
                .any(|o| o.service_id == s.service_id)
            {
                return Err(MuxFault::DuplicateService(s.service_id).into());
            }
        }
        if self.data_every == 0 {
            return Err(MuxFault::BadDataEvery.into());
        }
        let pmt_pid = |i: usize| 0x0100 + 0x10 * i as u16;
        let dp = self.data_pid;
        let table_pids = (0..self.services.len()).map(pmt_pid);
        if !(0x0010..NULL_PID).contains(&dp) || dp == 0x0011 || table_pids.clone().any(|p| p == dp)
        {
            return Err(MuxFault::BadDataPid(dp).into());
        }
        let sound = match self.sound {
            Some(t) => {
                let sp = t.pid;
                if !(0x0010..NULL_PID).contains(&sp)
                    || sp == 0x0011
                    || sp == dp
                    || table_pids.clone().any(|p| p == sp)
                {
                    return Err(MuxFault::BadSoundPid(sp).into());
                }
                // The sound alone needs 3 packets of 188 bytes for each frame of 24 ms.
                if !t.mux_bit_rate.is_finite() || t.mux_bit_rate <= 3.0 * 188.0 * 8.0 / FRAME_S {
                    return Err(MuxFault::BadSoundRate.into());
                }
                Some(Sound::new(t)?)
            }
            None => None,
        };
        let video = match self.video {
            Some(v) => {
                let vp = v.pid;
                if !(0x0010..NULL_PID).contains(&vp)
                    || vp == 0x0011
                    || vp == dp
                    || self.sound.is_some_and(|t| t.pid == vp)
                    || table_pids.clone().any(|p| p == vp)
                {
                    return Err(MuxFault::BadVideoPid(vp).into());
                }
                let video = Video::new(v, sound.is_none());
                // The sound and the video together, at most 3 packets for each sound frame.
                let sound_rate = if sound.is_some() { 3.0 / FRAME_S } else { 0.0 };
                let need = (video.packets as f64 * TestVideo::FRAME_RATE + sound_rate) * 188.0 * 8.0;
                if !v.mux_bit_rate.is_finite() || v.mux_bit_rate <= need {
                    return Err(MuxFault::BadVideoRate.into());
                }
                Some(video)
            }
            None => None,
        };
        let text_ok = |s: &str| s.len() < 255;
        if !text_ok(&self.provider)
            || self.services.iter().any(|s| !text_ok(&s.name))
            || self.network.as_ref().is_some_and(|n| !text_ok(&n.name))
        {
            return Err(MuxFault::TextTooLong.into());
        }
        let onid = self.network.as_ref().map_or(1, |n| n.network_id);
        let mut sections: Vec<(Vec<u8>, u16)> = Vec::new();
        let pat: Vec<(u16, u16)> = self
            .services
            .iter()
            .enumerate()
            .map(|(i, s)| (s.service_id, pmt_pid(i)))
            .collect();
        sections.push((build_pat(self.tsid, 0, &pat), 0x0000));
        if let Some(n) = &self.network {
            sections.push((nit(n, self.tsid, &self.services), 0x0010));
        }
        for (i, s) in self.services.iter().enumerate() {
            let mut streams = Vec::new();
            let mut pcr_pid = dp;
            if i == 0 {
                if let Some(v) = &video {
                    streams.push((0x02, v.pid, vec![]));
                    pcr_pid = v.pid;
                }
                if let Some(t) = &sound {
                    streams.push((0x03, t.pid, vec![]));
                    pcr_pid = t.pid;
                }
            }
            streams.push((0x06, dp, vec![]));
            sections.push((build_pmt(s.service_id, 0, pcr_pid, &[], &streams), pmt_pid(i)));
        }
        if let Some(major) = self.vct_major {
            sections.push((tvct(self.tsid, major, &self.services), 0x1FFB));
        } else {
            let provider = dvb_name(&self.provider);
            let names: Vec<String> = self.services.iter().map(|s| dvb_name(&s.name)).collect();
            let sdt: Vec<(u16, u8, &str, &str)> = self
                .services
                .iter()
                .zip(&names)
                .map(|(s, n)| (s.service_id, s.service_type, provider.as_str(), n.as_str()))
                .collect();
            sections.push((build_sdt(self.tsid, onid, 0, 0, 0, &sdt), 0x0011));
        }
        let mut tables = Vec::new();
        for (sec, pid) in sections {
            let mut cc = 0;
            tables.extend(packetize_section(&sec, pid, &mut cc));
        }
        if tables.len() > self.period {
            return Err(MuxFault::PeriodTooShort { need: tables.len() }.into());
        }
        Ok(TestMux {
            tables,
            period: self.period as u64,
            data_every: self.data_every,
            data: self.data,
            data_pid: dp,
            i: 0,
            gap: 0,
            n: 0,
            cc: Vec::new(),
            sound,
            video,
        }
        .with_counters())
    }
}

/// The time of one Layer II frame at 48 kHz, s.
const FRAME_S: f64 = 1152.0 / 48_000.0;

/// The state of the sound track of a [`TestMux`].
#[derive(Clone, Debug)]
struct Sound {
    pid: u16,
    /// The time of one transport packet of the multiplex, s.
    packet_s: f64,
    /// The coded frame of the tone. A tone of whole periods in a frame gives the same frame every time.
    frame: Vec<u8>,
    /// The frames sent.
    frames: u64,
    /// The packets of the frame that is on its way.
    queue: VecDeque<[u8; TS_SIZE]>,
    cc: u8,
}

impl Sound {
    fn new(t: TestSound) -> Result<Sound, Error> {
        let mut enc = sdrkit_codec_mpa::Encoder::new(TestSound::BIT_RATE_KBPS, TestSound::SAMPLE_RATE, true)
            .map_err(|_| MuxFault::BadSoundRate)?;
        let step = std::f32::consts::TAU * TestSound::TONE_HZ / TestSound::SAMPLE_RATE as f32;
        let tone: Vec<f32> = (0..1152)
            .map(|i| (i as f32 * step).sin() * TestSound::AMPLITUDE)
            .collect();
        // The first frame fills the memory of the analysis filter; the frames after it are all the same.
        let mut frame = Vec::new();
        for _ in 0..3 {
            enc.encode(&tone, &tone, 0, &mut frame)
                .map_err(|_| MuxFault::BadSoundRate)?;
        }
        Ok(Sound {
            pid: t.pid,
            packet_s: TS_SIZE as f64 * 8.0 / t.mux_bit_rate,
            frame,
            frames: 0,
            queue: VecDeque::new(),
            cc: 0,
        })
    }

    /// The next packet of the sound for packet `i` of the multiplex, if a frame is on its way or due.
    fn packet(&mut self, i: u64) -> Option<[u8; TS_SIZE]> {
        let now = i as f64 * self.packet_s;
        if self.queue.is_empty() && self.frames as f64 * FRAME_S <= now {
            const WRAP: u64 = 1 << 33;
            let pts = ((self.frames as f64 * FRAME_S + TestSound::DELAY_S) * 90_000.0).round() as u64 % WRAP;
            let pcr = (now * 27e6).round() as u64 % (WRAP * 300);
            let pes = build_pes(0xC0, Some(pts as i64), None, &self.frame, true);
            self.queue.extend(pes_packets(&pes, self.pid, &mut self.cc, Some(pcr)));
            self.frames += 1;
        }
        self.queue.pop_front()
    }
}

/// The state of the picture track of a [`TestMux`].
#[derive(Clone, Debug)]
struct Video {
    pid: u16,
    /// The video carries the PCR.
    pcr: bool,
    /// The time of one transport packet of the multiplex, s.
    packet_s: f64,
    /// The access unit of the picture: every picture is the same.
    picture: Vec<u8>,
    /// The transport packets of one picture.
    packets: usize,
    /// The pictures sent.
    pictures: u64,
    /// The packets of the picture that is on its way.
    queue: VecDeque<[u8; TS_SIZE]>,
    cc: u8,
}

impl Video {
    fn new(v: TestVideo, pcr: bool) -> Video {
        let picture = TestVideo::picture();
        let pes = build_pes(0xE0, Some(0), None, &picture, true);
        let packets = pes_packets(&pes, v.pid, &mut 0, pcr.then_some(0)).len();
        Video {
            pid: v.pid,
            pcr,
            packet_s: TS_SIZE as f64 * 8.0 / v.mux_bit_rate,
            picture,
            packets,
            pictures: 0,
            queue: VecDeque::new(),
            cc: 0,
        }
    }

    /// The next packet of the video for packet `i` of the multiplex, if a picture is on its way or due.
    fn packet(&mut self, i: u64) -> Option<[u8; TS_SIZE]> {
        let now = i as f64 * self.packet_s;
        let at = self.pictures as f64 / TestVideo::FRAME_RATE;
        if self.queue.is_empty() && at <= now {
            const WRAP: u64 = 1 << 33;
            let pts = ((at + TestVideo::DELAY_S) * 90_000.0).round() as u64 % WRAP;
            let pcr = (now * 27e6).round() as u64 % (WRAP * 300);
            let pes = build_pes(0xE0, Some(pts as i64), None, &self.picture, true);
            let pcr = self.pcr.then_some(pcr);
            self.queue.extend(pes_packets(&pes, self.pid, &mut self.cc, pcr));
            self.pictures += 1;
        }
        self.queue.pop_front()
    }
}

/// A terrestrial virtual channel table section (ATSC A/65 6.3.1, table 6.4): service `k` of `services` is channel
/// `major`.`k + 1` on the transport stream `tsid`, with no descriptors.
fn tvct(tsid: u16, major: u16, services: &[TestService]) -> Vec<u8> {
    let mut b = vec![0, services.len() as u8];
    for (k, s) in services.iter().enumerate() {
        let mut name = [0u16; 7];
        for (n, c) in name.iter_mut().zip(s.name.encode_utf16()) {
            *n = c;
        }
        for c in name {
            b.extend_from_slice(&c.to_be_bytes());
        }
        // 4 reserved bits, 10 bits major_channel_number, 10 bits minor_channel_number.
        let channel = (0xF << 20) | (u32::from(major & 0x3FF) << 10) | (k as u32 + 1) & 0x3FF;
        b.extend_from_slice(&channel.to_be_bytes()[1..]);
        b.push(0x04); // modulation_mode 8-VSB
        b.extend_from_slice(&[0; 4]); // carrier_frequency, not stated
        b.extend_from_slice(&tsid.to_be_bytes());
        b.extend_from_slice(&s.service_id.to_be_bytes());
        // ETM_location 0, not access controlled, not hidden, 2 reserved, hide_guide 0, 3 reserved, service_type.
        let service_type: u16 = match s.service_type {
            0x01 => 0x02,
            0x02 => 0x03,
            _ => 0x04,
        };
        b.extend_from_slice(&(0x0DC0 | service_type).to_be_bytes());
        b.extend_from_slice(&(k as u16 + 1).to_be_bytes()); // source_id
        b.extend_from_slice(&[0xFC, 0x00]); // descriptors_length 0
    }
    b.extend_from_slice(&[0xFC, 0x00]); // additional_descriptors_length 0
    build_long(0xC8, tsid, 0, 0, 0, &b)
}

/// Splits a PES packet into packets of `pid`, the first with PUSI and, when `pcr` is given, the PCR (27 MHz units) in its
/// adaptation field, the last filled up with adaptation field stuffing (ISO/IEC 13818-1 2.4.3.4 and 2.4.3.5). `cc` is the
/// counter of the first packet and is advanced.
fn pes_packets(pes: &[u8], pid: u16, cc: &mut u8, pcr: Option<u64>) -> Vec<[u8; TS_SIZE]> {
    // The adaptation field of the first packet: its length, the flags with PCR_flag, and the PCR.
    let field = pcr.map(|pcr| {
        let (base, ext) = (pcr / 300, pcr % 300);
        [
            7,
            0x10,
            (base >> 25) as u8,
            (base >> 17) as u8,
            (base >> 9) as u8,
            (base >> 1) as u8,
            ((base & 1) as u8) << 7 | 0x7E | ((ext >> 8) as u8 & 1),
            ext as u8,
        ]
    });
    let mut out = Vec::new();
    let mut rest = pes;
    let mut first = true;
    while first || !rest.is_empty() {
        let own: &[u8] = match (first, &field) {
            (true, Some(f)) => f,
            _ => &[],
        };
        let room = TS_SIZE - 4 - own.len();
        let n = rest.len().min(room);
        let stuff = room - n;
        let mut p = [0xFFu8; TS_SIZE];
        write_header(
            &mut p,
            &Header {
                pusi: first,
                pid,
                afc: if own.is_empty() && stuff == 0 { 1 } else { 3 },
                cc: *cc,
                ..Default::default()
            },
        );
        *cc = (*cc + 1) & 15;
        let mut at = 4;
        if !own.is_empty() {
            p[4..4 + own.len()].copy_from_slice(own);
            p[4] += stuff as u8;
            at += own.len() + stuff;
        } else if stuff > 0 {
            p[4] = (stuff - 1) as u8;
            if stuff > 1 {
                p[5] = 0;
            }
            at += stuff;
        }
        p[at..at + n].copy_from_slice(&rest[..n]);
        rest = &rest[n..];
        out.push(p);
        first = false;
    }
    out
}

/// A DVB text field for a name: plain ASCII as it is, other text as UTF-8 with selector 0x15 (EN 300 468 Annex A.2).
fn dvb_name(s: &str) -> String {
    if s.is_ascii() && !s.starts_with(|c: char| (c as u32) < 0x20) {
        s.to_string()
    } else {
        format!("\u{15}{s}")
    }
}

/// A NIT actual section: network name, one transport stream with its service list and logical channel numbers (EN 300 468
/// 5.2.1, descriptors 0x40 and 0x41, and the EACEM / NorDig logical_channel_descriptor 0x83).
fn nit(net: &TestNetwork, tsid: u16, services: &[TestService]) -> Vec<u8> {
    let mut b = loop_bytes(&descriptor(0x40, dvb_name(&net.name).as_bytes()));
    let mut sl = Vec::new();
    let mut lcn = Vec::new();
    for sv in services {
        sl.extend_from_slice(&sv.service_id.to_be_bytes());
        sl.push(sv.service_type);
        if sv.lcn != 0 {
            lcn.extend_from_slice(&sv.service_id.to_be_bytes());
            // visible_service_flag, 5 reserved bits, 10-bit LCN.
            lcn.extend_from_slice(&(0xFC00 | (sv.lcn & 0x03FF)).to_be_bytes());
        }
    }
    let mut ds = descriptor(0x41, &sl);
    if !lcn.is_empty() {
        ds.extend_from_slice(&descriptor(0x83, &lcn));
    }
    let mut ts = tsid.to_be_bytes().to_vec();
    ts.extend_from_slice(&net.network_id.to_be_bytes());
    ts.extend_from_slice(&loop_bytes(&ds));
    b.extend_from_slice(&loop_bytes(&ts));
    build_long(0x40, net.network_id, 0, 0, 0, &b)
}

/// The test multiplex: a PAT, an optional NIT, one PMT for each service and an SDT (or a virtual channel table), again and
/// again, with a data stream on one PID between them, an optional sound and picture track, and null packets in the gaps.
///
/// ```
/// use sdrkit_proto_mpegts::source::{PacketSource, TestMux, TestService};
///
/// let mut mux = TestMux::builder(1).service(TestService::tv(1, "sdrkit", 1)).build().unwrap();
/// let mut p = [0u8; 188];
/// mux.next_packet(&mut p);
/// assert_eq!((p[0], p[1] & 0x1F, p[2]), (0x47, 0, 0)); // the PAT comes first
/// ```
#[derive(Clone, Debug)]
pub struct TestMux {
    tables: Vec<[u8; TS_SIZE]>,
    period: u64,
    data_every: u64,
    data: Data,
    data_pid: u16,
    i: u64,
    gap: u64,
    n: u32,
    /// The continuity counter of each table PID.
    cc: Vec<(u16, u8)>,
    sound: Option<Sound>,
    video: Option<Video>,
}

fn pid_of(p: &[u8; TS_SIZE]) -> u16 {
    ((p[1] as u16 & 0x1F) << 8) | p[2] as u16
}

impl TestMux {
    fn with_counters(mut self) -> Self {
        for t in &self.tables {
            let pid = pid_of(t);
            if !self.cc.iter().any(|c| c.0 == pid) {
                self.cc.push((pid, 0));
            }
        }
        self
    }

    /// A builder for a multiplex with transport_stream_id `tsid`.
    pub fn builder(tsid: u16) -> TestMuxBuilder {
        TestMuxBuilder {
            tsid,
            services: Vec::new(),
            network: None,
            provider: "sdrkit".into(),
            data_pid: 0x0101,
            data: Data::Numbered(Fill::N7I13),
            period: 60,
            data_every: 1,
            sound: None,
            video: None,
            vct_major: None,
        }
    }

    /// The DVB-T generator's multiplex: one TV service, tables every 60 packets, one packet in 7 a numbered packet
    /// ([`Fill::I31N7`]) on PID 0x0101, null packets between.
    pub fn dvbt(service_id: u16, name: &str) -> Result<TestMux, Error> {
        TestMux::dvbt_builder(service_id, name).build()
    }

    /// The builder of [`TestMux::dvbt`], for a multiplex with more, such as a [`TestSound`].
    pub fn dvbt_builder(service_id: u16, name: &str) -> TestMuxBuilder {
        TestMux::builder(1)
            .service(TestService::tv(service_id, name, 0))
            .data(0x0101, Data::Numbered(Fill::I31N7))
            .data_every(7)
    }

    /// The satellite and cable test programme: service 1 "sdrkit test", tables every `period` packets, and every other packet a
    /// numbered packet of [`Numbered::SATELLITE_CABLE`].
    pub fn satellite_cable(period: usize) -> Result<TestMux, Error> {
        TestMux::builder(1)
            .service(TestService::tv(1, "sdrkit test", 0))
            .data(0x0020, Data::Numbered(Fill::N7I13))
            .period(period)
            .build()
    }

    /// The pattern of the data packets, when there are any.
    pub fn numbered(&self) -> Option<Numbered> {
        match self.data {
            Data::Numbered(f) => Some(Numbered::new(self.data_pid, f)),
            Data::None => None,
        }
    }
}

impl PacketSource for TestMux {
    fn next_packet(&mut self, out: &mut [u8; TS_SIZE]) {
        let k = (self.i % self.period) as usize;
        self.i += 1;
        if let Some(t) = self.tables.get(k) {
            *out = *t;
            let pid = pid_of(t);
            if let Some(c) = self.cc.iter_mut().find(|c| c.0 == pid) {
                out[3] = (out[3] & 0xF0) | c.1;
                c.1 = (c.1 + 1) & 15;
            }
            return;
        }
        self.gap += 1;
        match self.data {
            Data::Numbered(fill) if self.gap.is_multiple_of(self.data_every) => {
                *out = Numbered::new(self.data_pid, fill).packet(self.n);
                self.n = self.n.wrapping_add(1);
            }
            _ => {
                let i = self.i - 1;
                let sound = self.sound.as_mut().and_then(|s| s.packet(i));
                *out = sound
                    .or_else(|| self.video.as_mut().and_then(|v| v.packet(i)))
                    .unwrap_or_else(null_packet);
            }
        }
    }
}
