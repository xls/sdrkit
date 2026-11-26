//! The transport stream components. [`register`] adds them to a registry under the names `mpegts.<name>`, with the
//! conversions that the graph inserts: a transport stream to PES, to media packets, to coded media of one codec, and to MPEG-4
//! access units.

use crate::codec::{CodecInfo, EsCodec, EsKind};
use crate::demux::sl::{AccessUnit, SlDemux};
use crate::demux::{
    Demultiplexer, DemuxOutput, PidReport, PlayerStatus, Route, RouteSet, Section, TsFilter,
};
use crate::epg::EpgEvent;
use crate::es::{EsPacket, EsState};
use crate::input::TsAligner;
use crate::packet::TS_SIZE;
use crate::pes::PesPacket;
use crate::publish::{
    ServiceList, SiTables, TeletextSelectArgs, TeletextTables, TsNetwork, TsTime, CHANNELS, CODEC,
    FRAME_RATE, GUIDE, GUIDE_RATINGS, HEIGHT, LOCAL_OFFSET_MIN, NETWORK_ID, NETWORK_NAME, ONID,
    PID, SAMPLE_RATE, SCRAMBLED, SERVICES, SERVICE_COUNT, SERVICE_ROW, SERVICE_SELECT_KIND,
    TELETEXT_PAGE_TABLE, TELETEXT_SELECT, TELETEXT_SUBPAGE_TABLE, TRACKS, TRACK_ROW,
    TRACK_SELECT_KIND, TSID, UTC, WIDTH,
};
use crate::si::SiDb;
use crate::stream::{self, TsOut, TsPacket};
use crate::subtitle::{SubtitleDecoder, SubtitleDisplay};
use crate::teletext::{TeletextDecoder, TeletextPage};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Table, Time};
use sdrkit_core::media::{Codec, MediaFormat, MediaSpec, Packaging};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::{
    Choice, ConfigError, Record, Results, Setting, SettingError, SettingId, SettingSet, TableId,
    Value, Values,
};
use sdrkit_graph::adapt::{Adapter, AdapterClass};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, FormatConstraint, Io, Kept,
    Kind, MediaPacket, Port, RateFrom, Registry, TagKind, Timed,
};
use sdrkit_util_text::dvb::DefaultTable;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// A typed output format that keeps the frequencies of the input, so events and results name their frequency.
fn typed_from(kind: Kind, inputs: &[Format]) -> Format {
    let mut f = Format::new(kind, 0.0);
    if let Some(i) = inputs.first() {
        f.centre_hz = i.centre_hz;
        f.tuned_hz = i.tuned_hz;
    }
    f
}

/// A typed output port with no rate.
fn typed<T: sdrkit_graph::Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

/// The item indices of input `i` where data was lost, from its tags.
fn losses(io: &Io, i: usize, lost: &mut Vec<u64>) {
    lost.clear();
    lost.extend(
        io.input(i)
            .tags()
            .iter()
            .filter(|t| {
                matches!(
                    t.kind,
                    TagKind::Discontinuity { .. } | TagKind::Retuned { .. }
                )
            })
            .map(|t| t.at),
    );
}

/// The id of table `t`, which `p` declares.
fn table_id(p: &Published, t: &TableDef) -> TableId {
    p.results()
        .decl()
        .table_id(t.key)
        .expect("the component declares the table")
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// `mpegts.ts_input`: finds the packets of a byte stream (a file, a UDP payload, 188-, 192- or 204-byte packets) and returns
/// a transport stream with a loss tag.
#[derive(Default)]
pub struct TsInput {
    aligner: TsAligner,
    ts_out: TsOut,
    buf: Vec<[u8; TS_SIZE]>,
    times: Vec<f64>,
    losses: Vec<usize>,
    lost: Vec<u64>,
}

impl Component for TsInput {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpegts.ts_input",
            vec![Port::bytes("in")],
            vec![typed::<TsPacket>("out")],
        )
        .manual_tags()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.aligner.reset();
        Ok(vec![typed_from(stream::kind(), inputs)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut lost = std::mem::take(&mut self.lost);
        losses(io, 0, &mut lost);
        let first = io.input(0).first();
        self.buf.clear();
        self.times.clear();
        self.losses.clear();
        for (k, p) in io.bytes(0).iter().enumerate() {
            if lost.contains(&(first + k as u64)) {
                self.aligner.mark_loss();
            }
            let (buf, times, losses) = (&mut self.buf, &mut self.times, &mut self.losses);
            self.aligner.push(p.bytes(), &mut |a| {
                let mut b = [0u8; TS_SIZE];
                b.copy_from_slice(a.bytes);
                if a.loss {
                    losses.push(buf.len());
                }
                buf.push(b);
                times.push(p.time);
            });
        }
        self.lost = lost;
        let out = io.output(0);
        let first = out.first();
        for &i in &self.losses {
            out.push_tag(first + i as u64, TagKind::Discontinuity { lost: 0 });
        }
        // The aligner outputs packets that start with the sync byte, so none is refused.
        let _ = self
            .ts_out
            .extend(io.items_out::<TsPacket>(0), &self.buf, &self.times);
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

const AUDIO_LANG: Setting<field::Text> = Setting::new("audio_lang", "Audio language", "", "")
    .help("The audio language to play first (ISO 639-2, for example eng).");
const AUDIO_DESCRIPTION: Setting<field::Bool> =
    Setting::new("audio_description", "Audio description", "", false);
const SUBTITLE_LANG: Setting<field::Text> =
    Setting::new("subtitle_lang", "Subtitle language", "", "").help("Empty: subtitles off.");
const HARD_OF_HEARING: Setting<field::Bool> = Setting::new(
    "hard_of_hearing",
    "Subtitles for the hard of hearing",
    "",
    false,
);
const COUNTRY: Setting<field::Text> = Setting::new("country", "Country", "", "")
    .help("ISO 3166 alpha-3 code, for channel numbers and local time. Empty: no country.");
const GUIDE_LANG: Setting<field::Text> = Setting::new("guide_lang", "Guide language", "", "");
const DOWNMIX: Setting<field::Bool> = Setting::new("downmix", "Stereo downmix", "", true).help(
    "Sound of more than two channels (5.1) leaves the library as stereo, mixed with the gains of the codec's standard. Off: \
     all channels, in the order that the audio information states.",
);

const DECODE_VIDEO: Setting<field::Bool> = Setting::new("decode_video", "Decode video", "", false).help(
    "MPEG-2 video leaves the library as frames of raw pixels (I420) in display order, for a host that has no decoder \
     of MPEG-2 video. While the stream has lost pictures the last good frame leaves again. Off: the coded pictures. \
     H.264 and HEVC always leave as coded pictures.",
);

sdrkit_core::settings! {
    /// The settings of [`TsDemuxComponent`].
    pub enum TsDemuxSetting {
        /// The audio language to play first.
        AudioLang = AUDIO_LANG,
        /// Play audio description.
        AudioDescription = AUDIO_DESCRIPTION,
        /// The subtitle language.
        SubtitleLang = SUBTITLE_LANG,
        /// Subtitles for the hard of hearing first.
        HardOfHearing = HARD_OF_HEARING,
        /// The country.
        Country = COUNTRY,
        /// The guide language.
        GuideLang = GUIDE_LANG,
        /// Sound of more than two channels mixed down to stereo.
        Downmix = DOWNMIX,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
    }
}

sdrkit_core::commands! {
    /// The commands of [`TsDemuxComponent`].
    pub enum TsDemuxCommand {
        /// Plays a service.
        ServiceSelect = SERVICE_SELECT_KIND.command,
        /// Plays an audio track or shows a subtitle track.
        TrackSelect = TRACK_SELECT_KIND.command,
    }
}

/// The result fields of `mpegts.ts_demux`: the counts of the packet layer and the selection.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsDemuxFields {
    /// Packets seen.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Null packets seen.
    #[field(label = "Null packets")]
    pub null_packets: i64,
    /// Packets with the transport_error_indicator.
    #[field(label = "Packets with errors")]
    pub tei_packets: i64,
    /// Continuity errors.
    #[field(shared = CC_ERRORS)]
    pub cc_errors: i64,
    /// The bit rate of the multiplex.
    #[field(label = "Multiplex bit rate", unit = "kbit/s")]
    pub mux_kbps: f64,
    /// The bit rate of the null packets.
    #[field(label = "Null bit rate", unit = "kbit/s")]
    pub null_kbps: f64,
    /// The service that plays. No value while none plays.
    #[field(label = "Service playing")]
    pub selected_service: Option<i64>,
    /// The audio PID that plays. No value while none plays.
    #[field(label = "Audio PID")]
    pub selected_audio: Option<i64>,
    /// The subtitle PID shown. No value while none is shown.
    #[field(label = "Subtitle PID")]
    pub selected_subtitle: Option<i64>,
    /// What the player does.
    #[field(label = "Player")]
    pub player_status: PlayerStatus,
    /// Whether the decoders of the library decode the audio that plays: absent while no audio plays, and when the host
    /// gave no decoder of MPEG audio or AAC (it decodes the coded audio itself).
    #[field(label = "Sound decoded")]
    pub audio_decoded: Option<bool>,
}

/// Continuity errors.
const CC_ERRORS: Field<field::I64> = Field::new("cc_errors", "Continuity errors", "");

/// A PID of the multiplex. The row key is the PID as four hex digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsPidRow<'a> {
    /// The PID.
    #[field(shared = PID)]
    pub pid: i64,
    /// What the PID carries.
    #[field(label = "Label")]
    pub label: &'a str,
    /// Packets of the PID.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Continuity errors.
    #[field(shared = CC_ERRORS)]
    pub cc_errors: i64,
    /// Packets of the PID with the transport_error_indicator.
    #[field(label = "Packets with errors")]
    pub tei: i64,
    /// The bit rate of the PID.
    #[field(label = "Bit rate", unit = "kbit/s")]
    pub kbps: f64,
    /// Scrambled.
    #[field(shared = SCRAMBLED)]
    pub scrambled: bool,
}

/// The PIDs of the multiplex. Row key: the PID as four hex digits.
pub static PIDS: Table<TsPidRow<'static>> = Table::new("pids", "PIDs", 8192);

/// How often the PID reports and the counts go out, in seconds of signal.
const REPORT_EVERY_S: f64 = 0.5;

/// One row of the `pids` table as it was last written.
struct PidShown {
    pid: u16,
    packets: u64,
    cc_errors: u64,
    tei: u64,
    /// The bit rate in 0.1 kbit/s.
    kbps10: i64,
    scrambled: bool,
    label: String,
    /// The version of the service information that `label` was made from.
    label_version: u64,
}

/// `mpegts.ts_demux`: the packet layer, service and track selection, and the elementary streams of the service that plays.
///
/// Outputs: `sections` (every complete section), `video` and `audio` (media packets of the selected tracks), `teletext` and
/// `subtitle` (PES of the selected streams), `pids` (a report of each PID every 0.5 s), and `service_ts` (the service that
/// plays, as a transport stream with its own PAT). Results: the counts, the
/// selection, and the tables `services`, `tracks` and `pids`. Commands: `service.select`, `track.select`.
pub struct TsDemuxComponent {
    demux: Demultiplexer,
    out: DemuxOutput,
    published: Published,
    tables: SiTables,
    report_t: f64,
    lost: Vec<u64>,
    version: u64,
    key: String,
    /// The rows of the `pids` table as they were last written, in PID order.
    shown: Vec<PidShown>,
    country: String,
    service_ts: TsOut,
    /// The audio decoders of the host, from the run context.
    codecs: Option<Arc<sdrkit_core::codec::CodecSet>>,
    /// The audio format that the decoders were last searched for.
    audio_checked: Option<Arc<MediaFormat>>,
    /// Whether the decoders take that format; `None` when the host decodes the audio itself.
    audio_decoded: Option<bool>,
    /// Sound of more than two channels is mixed down to stereo.
    downmix: bool,
    /// The last audio format of more than two channels, and the same format that asks the decoder for stereo.
    stereo: Option<(Arc<MediaFormat>, Arc<MediaFormat>)>,
    /// MPEG-2 video leaves as frames of raw pixels.
    decode_video: bool,
    /// The decoder of the video stream that plays, while `decode_video` is on.
    video: Option<crate::video::VideoDecoder>,
}

/// Whether the decoders `codecs` of the host take the audio `format`. `None` if the host gave no decoder of MPEG audio or
/// AAC at all (it decodes the coded media itself).
pub fn decodes(codecs: &sdrkit_core::codec::CodecSet, format: &MediaFormat) -> Option<bool> {
    let decodes = codecs.has(Codec::Mp2.fourcc()) || codecs.has(Codec::Aac.fourcc());
    decodes.then(|| {
        codecs
            .find(format.fourcc(), "")
            .is_some_and(|e| (e.factory)(format).is_some())
    })
}

/// The line that the log gets if the decoders `codecs` of the host have none for the audio `format`. `None` if they
/// have one, or if the host gave no decoder of MPEG audio or AAC at all (it decodes the coded media itself).
pub fn no_decoder_note(
    codecs: &sdrkit_core::codec::CodecSet,
    format: &MediaFormat,
) -> Option<String> {
    (decodes(codecs, format) == Some(false)).then(|| {
        format!(
            "the audio of the service is {}: there is no decoder for it, the service has no sound",
            format.codec.name()
        )
    })
}

/// `format`, or for audio of more than two channels the same format that asks the decoder for stereo. `cache` keeps the
/// last such pair, so that the packets of one stream share one format.
fn stereo_of(
    cache: &mut Option<(Arc<MediaFormat>, Arc<MediaFormat>)>,
    format: &Arc<MediaFormat>,
) -> Arc<MediaFormat> {
    if format.audio.as_ref().is_none_or(|a| a.channels <= 2) {
        return Arc::clone(format);
    }
    match cache {
        Some((from, to)) if Arc::ptr_eq(from, format) || **from == **format => Arc::clone(to),
        _ => {
            let mut f = MediaFormat::clone(format);
            if let Some(a) = f.audio.as_mut() {
                a.channels = 2;
            }
            let to = Arc::new(f);
            *cache = Some((Arc::clone(format), Arc::clone(&to)));
            to
        }
    }
}

impl Default for TsDemuxComponent {
    fn default() -> Self {
        let mut demux = Demultiplexer::new();
        demux.forward_pes = RouteSet::NONE.with(Route::Teletext).with(Route::Subtitle);
        demux.forward_sections = true;
        demux.auto_select = true;
        demux.probe_all = true;
        TsDemuxComponent {
            demux,
            out: DemuxOutput::default(),
            published: Published::new(TsDemuxFields::SHAPE.fields).with_tables(&[
                SERVICES.def(),
                TRACKS.def(),
                PIDS.def(),
            ]),
            tables: SiTables::default(),
            report_t: f64::NEG_INFINITY,
            lost: Vec::new(),
            version: u64::MAX,
            key: String::new(),
            shown: Vec::new(),
            country: String::new(),
            service_ts: TsOut::default(),
            codecs: None,
            audio_checked: None,
            audio_decoded: None,
            downmix: true,
            stereo: None,
            decode_video: false,
            video: None,
        }
    }
}

impl TsDemuxComponent {
    /// The demultiplexer.
    pub fn demux(&self) -> &Demultiplexer {
        &self.demux
    }

    /// Plays service `row`, or with an empty row the first service that can play.
    fn select_service(&mut self, row: &str) -> Result<(), CommandError> {
        let sid = if row.is_empty() {
            None
        } else {
            let sid = row
                .parse::<u16>()
                .ok()
                .filter(|&s| self.demux.si.service(s).is_some())
                .ok_or_else(|| CommandError::NoSuchRow {
                    table: table_id(&self.published, &SERVICES),
                    key: row.to_owned(),
                })?;
            Some(sid)
        };
        self.demux.auto_select = sid.is_none();
        self.demux.select(sid);
        self.publish();
        Ok(())
    }

    fn select_track(&mut self, row: &str) -> Result<(), CommandError> {
        let table = table_id(&self.published, &TRACKS);
        let no_row = || CommandError::NoSuchRow {
            table,
            key: row.to_owned(),
        };
        let mut parts = row.split('/');
        let (Some(sid), Some(pid)) = (
            parts.next().and_then(|x| x.parse::<u16>().ok()),
            parts.next().and_then(|x| x.parse::<u16>().ok()),
        ) else {
            return Err(no_row());
        };
        let n = match parts.next() {
            Some(x) => Some(x.parse::<usize>().map_err(|_| no_row())?),
            None => None,
        };
        let e = self
            .demux
            .si
            .service(sid)
            .and_then(|s| s.streams.iter().find(|e| e.pid == pid))
            .ok_or_else(no_row)?;
        let subtitle = match (e.kind, n) {
            (Some(EsKind::Audio), None) => None,
            (Some(EsKind::Subtitles), Some(n)) => {
                let sub = e.subtitles.get(n).ok_or_else(no_row)?;
                Some((pid, sub.composition_page, sub.ancillary_page))
            }
            _ => return Err(no_row()),
        };
        if self.demux.selection.service != Some(sid) {
            self.demux.select(Some(sid));
        }
        match subtitle {
            None => self.demux.set_audio(Some(pid)),
            Some(choice) => self.demux.set_subtitle(Some(choice)),
        }
        self.publish();
        Ok(())
    }

    fn publish(&mut self) {
        let d = &self.demux;
        let st = &d.ts.stats;
        let p = &mut self.published;
        let sel = &d.selection;
        p.put(&TsDemuxFields {
            packets: st.packets as i64,
            null_packets: st.null_packets as i64,
            tei_packets: st.tei_packets as i64,
            cc_errors: st.cc_errors as i64,
            mux_kbps: st.mux_kbps.round(),
            null_kbps: st.null_kbps.round(),
            selected_service: sel.service.map(i64::from),
            selected_audio: sel.audio.map(i64::from),
            selected_subtitle: sel.subtitle.map(|s| i64::from(s.0)),
            player_status: sel.status,
            audio_decoded: self.audio_decoded.filter(|_| sel.audio.is_some()),
        });
        // Only a row that differs from what was last written is written; a label is made again when the service
        // information changed.
        let si = d.si.version;
        let mut at = 0;
        for rep in d.ts.reports() {
            while self.shown.get(at).is_some_and(|s| s.pid < rep.pid) {
                at += 1;
            }
            let new = self.shown.get(at).is_none_or(|s| s.pid != rep.pid);
            if new {
                let row = PidShown {
                    pid: rep.pid,
                    packets: 0,
                    cc_errors: 0,
                    tei: 0,
                    kbps10: 0,
                    scrambled: false,
                    label: String::new(),
                    label_version: u64::MAX,
                };
                self.shown.insert(at, row);
            }
            let s = &mut self.shown[at];
            let kbps10 = (rep.kbps * 10.0).round() as i64;
            let relabel = s.label_version != si;
            let same = !new
                && !relabel
                && (s.packets, s.cc_errors, s.tei) == (rep.packets, rep.cc_errors, rep.tei)
                && (s.kbps10, s.scrambled) == (kbps10, rep.scrambled);
            if same {
                continue;
            }
            // Writing to a String does not fail.
            if relabel {
                s.label.clear();
                let _ = write!(s.label, "{}", d.pid_label(rep.pid));
                s.label_version = si;
            }
            (s.packets, s.cc_errors, s.tei) = (rep.packets, rep.cc_errors, rep.tei);
            (s.kbps10, s.scrambled) = (kbps10, rep.scrambled);
            self.key.clear();
            let _ = write!(self.key, "{:04X}", rep.pid);
            let row = TsPidRow {
                pid: i64::from(rep.pid),
                label: &s.label,
                packets: rep.packets as i64,
                cc_errors: rep.cc_errors as i64,
                tei: rep.tei as i64,
                kbps: kbps10 as f64 / 10.0,
                scrambled: rep.scrambled,
            };
            PIDS.put(p, &self.key, &row);
        }
        self.version = d.si.version;
        self.tables.write_services(&d.si, p);
    }
}

impl Component for TsDemuxComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpegts.ts_demux",
            vec![stream::port("in")],
            vec![
                typed::<Section>("sections"),
                typed::<MediaPacket>("video"),
                typed::<MediaPacket>("audio"),
                typed::<PesPacket>("teletext"),
                typed::<PesPacket>("subtitle"),
                typed::<PidReport>("pids"),
                stream::port("service_ts").same_format_as(0),
            ],
        )
        .with_settings(TsDemuxSetting::schema())
        .manual_tags()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let typed = |k: Kind| typed_from(k, inputs);
        Ok(vec![
            typed(Kind::typed::<Section>()),
            typed(Kind::typed::<MediaPacket>()),
            typed(Kind::typed::<MediaPacket>()),
            typed(Kind::typed::<PesPacket>()),
            typed(Kind::typed::<PesPacket>()),
            typed(Kind::typed::<PidReport>()),
            inputs[0].clone(),
        ])
    }
    fn attach(&mut self, ctx: &sdrkit_graph::RunContext) {
        self.codecs = Some(Arc::clone(&ctx.codecs));
        self.audio_checked = None;
        self.audio_decoded = None;
    }
    fn process(&mut self, io: &mut Io) {
        let mut lost = std::mem::take(&mut self.lost);
        losses(io, 0, &mut lost);
        let first = io.input(0).first();
        for (k, p) in io.items::<TsPacket>(0).iter().enumerate() {
            let loss = lost.contains(&(first + k as u64));
            self.demux
                .push(p.value.bytes(), Some(p.time), loss, &mut self.out);
        }
        self.lost = lost;
        let out = &mut self.out;
        for s in out.sections.drain(..) {
            io.items_out::<Section>(0).push(Timed {
                time: s.time,
                value: s,
            });
        }
        let video = self.demux.selection.video;
        let mut checked = false;
        for m in out.media.drain(..).filter(EsPacket::decodable) {
            let o = if Some(m.pid) == video { 1 } else { 2 };
            // An audio type without a decoder is said once for each format.
            let new = |f: &Arc<MediaFormat>| !Arc::ptr_eq(f, &m.format) && **f != *m.format;
            if o == 2 && self.audio_checked.as_ref().is_none_or(new) {
                self.audio_checked = Some(Arc::clone(&m.format));
                self.audio_decoded = self.codecs.as_ref().and_then(|c| decodes(c, &m.format));
                let note = self
                    .codecs
                    .as_ref()
                    .and_then(|c| no_decoder_note(c, &m.format));
                if let Some(n) = note {
                    self.published.results_mut().log(n);
                }
                checked = true;
            }
            if o == 1 && self.decode_video && m.format.codec == Codec::Mpeg2Video {
                let _timer = io.section("mpeg2_video");
                // A decoder holds the pictures of one stream: another PID starts a new one.
                let decoder = match &mut self.video {
                    Some(v) if v.pid == m.pid => v,
                    v => v.insert(crate::video::VideoDecoder::new(m.pid)),
                };
                decoder.push(&m, io.items_out::<MediaPacket>(1));
                continue;
            }
            let format = if o == 2 && self.downmix {
                stereo_of(&mut self.stereo, &m.format)
            } else {
                Arc::clone(&m.format)
            };
            io.items_out::<MediaPacket>(o).push(Timed {
                time: m.packet.time,
                value: MediaPacket::new(m.packet, format),
            });
        }
        for (r, p) in out.pes.drain(..) {
            let o = match r {
                Route::Teletext => 3,
                Route::Subtitle => 4,
                Route::Video | Route::Audio => continue,
            };
            io.items_out::<PesPacket>(o).push(Timed {
                time: p.time,
                value: p,
            });
        }
        // The filter outputs packets that start with the sync byte, so none is refused.
        let _ = self.service_ts.extend(
            io.items_out::<TsPacket>(6),
            &out.filtered,
            &out.filtered_times,
        );
        out.clear();
        let now = self.demux.ts.now;
        if checked {
            self.publish();
        }
        if now - self.report_t >= REPORT_EVERY_S || now < self.report_t {
            self.report_t = now;
            for r in self.demux.reports() {
                io.items_out::<PidReport>(5).push(Timed {
                    time: now,
                    value: r,
                });
            }
            self.publish();
        } else if self.demux.si.version != self.version {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let d = &mut self.demux;
        match TsDemuxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TsDemuxSetting::AudioLang => {
                d.prefs.audio_lang = AUDIO_LANG.read(id, v)?.to_owned();
                d.set_audio(d.selection.audio_override);
            }
            TsDemuxSetting::AudioDescription => {
                d.prefs.audio_description = AUDIO_DESCRIPTION.read(id, v)?;
                d.set_audio(d.selection.audio_override);
            }
            TsDemuxSetting::SubtitleLang => {
                d.prefs.subtitle_lang = SUBTITLE_LANG.read(id, v)?.to_owned();
                d.selection.subtitle_override = None;
                d.set_audio(d.selection.audio_override);
            }
            TsDemuxSetting::HardOfHearing => {
                d.prefs.hard_of_hearing = HARD_OF_HEARING.read(id, v)?;
            }
            TsDemuxSetting::Country => {
                self.country = COUNTRY.read(id, v)?.to_ascii_uppercase();
                d.si.country = self.country.clone();
            }
            TsDemuxSetting::GuideLang => d.si.preferred_lang = GUIDE_LANG.read(id, v)?.to_owned(),
            TsDemuxSetting::Downmix => {
                self.downmix = DOWNMIX.read(id, v)?;
                self.stereo = None;
            }
            TsDemuxSetting::DecodeVideo => {
                self.decode_video = DECODE_VIDEO.read(id, v)?;
                self.video = None;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let d = &self.demux;
        values_of([
            (
                TsDemuxSetting::AudioLang,
                Value::Text(d.prefs.audio_lang.clone()),
            ),
            (
                TsDemuxSetting::AudioDescription,
                Value::Bool(d.prefs.audio_description),
            ),
            (
                TsDemuxSetting::SubtitleLang,
                Value::Text(d.prefs.subtitle_lang.clone()),
            ),
            (
                TsDemuxSetting::HardOfHearing,
                Value::Bool(d.prefs.hard_of_hearing),
            ),
            (TsDemuxSetting::Country, Value::Text(self.country.clone())),
            (
                TsDemuxSetting::GuideLang,
                Value::Text(d.si.preferred_lang.clone()),
            ),
            (TsDemuxSetting::Downmix, Value::Bool(self.downmix)),
            (
                TsDemuxSetting::DecodeVideo,
                Value::Bool(self.decode_video),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        TsDemuxCommand::commands()
    }
    /// The multiplex of the frequency before is gone: its counts, its PIDs, its services and its tracks. A service or a
    /// track table that the stack keeps holds its rows until the services of the new multiplex replace them.
    fn forget(&mut self, kept: &Kept) {
        self.demux.retuned(0.0, true);
        self.video = None;
        self.out.clear();
        self.report_t = f64::NEG_INFINITY;
        // The rows that stay are written again from the new multiplex.
        self.shown.clear();
        if !kept.table(PIDS.key) {
            self.published.clear_rows(PIDS.def());
        }
        if kept.table(SERVICES.key) || kept.table(TRACKS.key) {
            return;
        }
        self.publish();
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TsDemuxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TsDemuxCommand::ServiceSelect => self.select_service(args.arg(&SERVICE_ROW)?)?,
            TsDemuxCommand::TrackSelect => self.select_track(args.arg(&TRACK_ROW)?)?,
        }
        Ok(None)
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

sdrkit_core::choice_enum! {
    /// The character table of DVB text that has no selector byte.
    pub enum DefaultText {
        /// ISO/IEC 6937 (ETSI EN 300 468 Annex A).
        Iso6937 = ("iso6937", "ISO/IEC 6937"),
        /// GBK.
        Gbk = ("gbk", "GBK"),
        /// GBK where the bytes are valid GBK, else ISO/IEC 6937.
        GbkWhenValid = ("gbk_when_valid", "GBK where valid"),
    }
}

impl DefaultText {
    fn table(self) -> DefaultTable {
        match self {
            DefaultText::Iso6937 => DefaultTable::Iso6937,
            DefaultText::Gbk => DefaultTable::Gbk,
            DefaultText::GbkWhenValid => DefaultTable::GbkWhenValid,
        }
    }
}

const SI_COUNTRY: Setting<field::Text> = Setting::new("country", "Country", "", "")
    .help("ISO 3166 alpha-3 code, for channel numbers and local time. Empty: no country.");
const SI_GUIDE_LANG: Setting<field::Text> = Setting::new("guide_lang", "Guide language", "", "");
const DEFAULT_TEXT: Setting<field::Choice<DefaultText>> = Setting::choice(
    "default_text",
    "Text without a selector",
    DefaultText::GbkWhenValid,
);

sdrkit_core::settings! {
    /// The settings of [`PsiSi`].
    pub enum PsiSiSetting {
        /// The country.
        Country = SI_COUNTRY,
        /// The guide language.
        GuideLang = SI_GUIDE_LANG,
        /// The table of text without a selector.
        DefaultText = DEFAULT_TEXT,
    }
}

/// The result fields of `mpegts.psi_si`: the network and the time of the stream.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PsiSiFields<'a> {
    /// The network name.
    #[field(shared = NETWORK_NAME)]
    pub network_name: &'a str,
    /// network_id.
    #[field(shared = NETWORK_ID)]
    pub network_id: i64,
    /// original_network_id.
    #[field(shared = ONID)]
    pub onid: i64,
    /// transport_stream_id.
    #[field(shared = TSID)]
    pub tsid: i64,
    /// The number of services.
    #[field(shared = SERVICE_COUNT)]
    pub service_count: i64,
    /// The UTC of the stream. No value before the stream gave the time.
    #[field(shared = UTC)]
    pub utc: Option<Time>,
    /// The local time offset in force. No value before the stream gave one.
    #[field(shared = LOCAL_OFFSET_MIN)]
    pub local_offset_min: Option<i64>,
}

/// The section counts of one table. The row key is the table name (PAT, PMT, SDT, ...).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsSectionRow {
    /// Sections accepted.
    #[field(label = "Accepted")]
    pub accepted: i64,
    /// Sections with a bad CRC.
    #[field(label = "Bad CRC")]
    pub crc: i64,
    /// Sections with a bad length.
    #[field(label = "Bad length")]
    pub length: i64,
    /// Sections refused for another reason.
    #[field(label = "Other")]
    pub other: i64,
}

/// The section counts per table. Row key: the table name (PAT, PMT, SDT, ...).
pub static SECTIONS: Table<TsSectionRow> = Table::new("sections", "Sections", 64);

/// `mpegts.psi_si`: the service information. Sections and PID reports in. Out: the service list, guide events, the time and
/// the network. Results: the network and the time, and the tables `guide`, `guide_ratings` and `sections`.
pub struct PsiSi {
    db: SiDb,
    published: Published,
    tables: SiTables,
    last_version: u64,
    last_net: (u16, String),
    now: f64,
    events: Vec<EpgEvent>,
    default_text: DefaultText,
    country: String,
}

impl Default for PsiSi {
    fn default() -> Self {
        let [guide, ratings] = SiTables::GUIDE_TABLES;
        PsiSi {
            db: SiDb::new(),
            published: Published::new(PsiSiFields::SHAPE.fields).with_tables(&[
                guide,
                ratings,
                SECTIONS.def(),
            ]),
            tables: SiTables::default(),
            last_version: u64::MAX,
            last_net: (0, String::new()),
            now: 0.0,
            events: Vec::new(),
            default_text: DEFAULT_TEXT.default,
            country: String::new(),
        }
    }
}

impl PsiSi {
    /// The service information.
    pub fn db(&self) -> &SiDb {
        &self.db
    }

    /// The service list now.
    pub fn list(&self) -> ServiceList {
        ServiceList {
            tsid: self.db.tsid,
            onid: self.db.onid,
            network_id: self.db.network_id,
            network_name: self.db.network_name.clone(),
            services: self.db.services().into_iter().cloned().collect(),
        }
    }

    fn publish(&mut self) {
        let db = &self.db;
        let p = &mut self.published;
        p.put(&PsiSiFields {
            network_name: &db.network_name,
            network_id: i64::from(db.network_id),
            onid: i64::from(db.onid),
            tsid: i64::from(db.tsid),
            service_count: db.known().count() as i64,
            utc: db.utc_now(self.now).map(|u| Time(u as f64)),
            local_offset_min: db.local_offset_min(self.now).map(i64::from),
        });
        for (k, c) in &db.counts {
            let row = TsSectionRow {
                accepted: c.accepted as i64,
                crc: c.crc as i64,
                length: c.length as i64,
                other: c.other as i64,
            };
            SECTIONS.put(p, k, &row);
        }
        self.tables.write_guide(db, &self.events, p);
        self.events.clear();
    }
}

impl Component for PsiSi {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpegts.psi_si",
            vec![
                Port::typed::<Section>("sections"),
                Port::typed::<PidReport>("pids"),
            ],
            vec![
                typed::<ServiceList>("services"),
                typed::<EpgEvent>("events"),
                typed::<TsTime>("time"),
                typed::<TsNetwork>("network"),
            ],
        )
        .with_settings(PsiSiSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            typed_from(Kind::typed::<ServiceList>(), inputs),
            typed_from(Kind::typed::<EpgEvent>(), inputs),
            typed_from(Kind::typed::<TsTime>(), inputs),
            typed_from(Kind::typed::<TsNetwork>(), inputs),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut time_changed = false;
        for s in io.items::<Section>(0) {
            self.now = s.time;
            let ch = self.db.push_section(s.value.pid, s.value.bytes(), s.time);
            time_changed |= ch.time;
            for e in ch.events {
                io.items_out::<EpgEvent>(1).push(Timed {
                    time: s.time,
                    value: e.clone(),
                });
                self.events.push(e);
            }
        }
        let mut ticked = false;
        for r in io.items::<PidReport>(1) {
            let rep = &r.value;
            self.db.set_scrambling(rep.pid, rep.examined, rep.scrambled);
            self.db.set_stream(rep.pid, None, None, Some(rep.kbps));
            if let Some((codec, pk, props)) = &rep.stream {
                self.db
                    .set_stream(rep.pid, Some((*codec, *pk)), Some(props), None);
            }
            self.now = self.now.max(r.time);
            ticked = true;
        }
        if ticked {
            self.db.tick(self.now);
            self.db.refresh_now_next(self.now);
        }
        let mut changed = !self.events.is_empty();
        if self.db.version != self.last_version {
            self.last_version = self.db.version;
            io.items_out::<ServiceList>(0).push(Timed {
                time: self.now,
                value: self.list(),
            });
            changed = true;
        }
        if time_changed {
            if let Some(u) = self.db.utc_now(self.now) {
                io.items_out::<TsTime>(2).push(Timed {
                    time: self.now,
                    value: TsTime {
                        utc: u,
                        offset_min: self.db.local_offset_min(self.now),
                    },
                });
            }
            changed = true;
        }
        if changed {
            self.publish();
        }
        if self.db.network_id != self.last_net.0 || self.db.network_name != self.last_net.1 {
            self.last_net = (self.db.network_id, self.db.network_name.clone());
            io.items_out::<TsNetwork>(3).push(Timed {
                time: self.now,
                value: TsNetwork {
                    network_id: self.db.network_id,
                    name: self.db.network_name.clone(),
                    onid: self.db.onid,
                    tsid: self.db.tsid,
                },
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PsiSiSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PsiSiSetting::Country => {
                self.country = SI_COUNTRY.read(id, v)?.to_ascii_uppercase();
                self.db.country = self.country.clone();
            }
            PsiSiSetting::GuideLang => {
                self.db.preferred_lang = SI_GUIDE_LANG.read(id, v)?.to_owned();
            }
            PsiSiSetting::DefaultText => {
                self.default_text = DEFAULT_TEXT.read(id, v)?;
                self.db.text.default_table = self.default_text.table();
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (PsiSiSetting::Country, Value::Text(self.country.clone())),
            (
                PsiSiSetting::GuideLang,
                Value::Text(self.db.preferred_lang.clone()),
            ),
            (
                PsiSiSetting::DefaultText,
                Value::Choice(self.default_text.index()),
            ),
        ])
    }
    /// The multiplex of the frequency before is gone: its network, its time and its section counts. The guide is
    /// keyed by service and holds the events of every multiplex; it stays if the stack keeps the guide table.
    fn forget(&mut self, kept: &Kept) {
        self.db.clear_multiplex();
        if !(kept.table(GUIDE.key) || kept.table(GUIDE_RATINGS.key)) {
            self.db.epg = crate::epg::Epg::new();
        }
        self.db.counts.clear();
        self.db.first_utc = None;
        self.db.last_utc = None;
        self.db.offsets.clear();
        if !kept.table(SECTIONS.key) {
            self.published.clear_rows(SECTIONS.def());
        }
        self.events.clear();
        self.last_version = u64::MAX;
        self.last_net = (0, String::new());
        self.publish();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// The result fields of `mpegts.pes_video`: the stream and its picture.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PesVideoFields {
    /// The codec of the stream.
    #[field(shared = CODEC)]
    pub codec: EsCodec,
    /// The PID.
    #[field(shared = PID)]
    pub pid: i64,
    /// The picture width.
    #[field(shared = WIDTH)]
    pub width: i64,
    /// The picture height.
    #[field(shared = HEIGHT)]
    pub height: i64,
    /// Frames per second. No value before the stream gave the rate.
    #[field(shared = FRAME_RATE)]
    pub frame_rate: Option<f64>,
}

/// The result fields of `mpegts.pes_audio`: the stream and its sound.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PesAudioFields {
    /// The codec of the stream.
    #[field(shared = CODEC)]
    pub codec: EsCodec,
    /// The PID.
    #[field(shared = PID)]
    pub pid: i64,
    /// The audio sample rate.
    #[field(shared = SAMPLE_RATE)]
    pub sample_rate: Hertz,
    /// The audio channels.
    #[field(shared = CHANNELS)]
    pub channels: i64,
}

/// `mpegts.pes_video` and `mpegts.pes_audio`: the PES of one stream to media packets with their format. A new PID replaces
/// the stream. Results: `codec`, `pid`, and the picture or sound properties.
pub struct PesMedia {
    name: &'static str,
    streams: BTreeMap<u16, EsState>,
    published: Published,
    warned: Vec<u16>,
}

impl PesMedia {
    /// The video one.
    pub fn video() -> Self {
        PesMedia {
            name: "mpegts.pes_video",
            streams: BTreeMap::new(),
            published: Published::new(PesVideoFields::SHAPE.fields),
            warned: Vec::new(),
        }
    }
    /// The audio one.
    pub fn audio() -> Self {
        PesMedia {
            name: "mpegts.pes_audio",
            streams: BTreeMap::new(),
            published: Published::new(PesAudioFields::SHAPE.fields),
            warned: Vec::new(),
        }
    }
}

impl Component for PesMedia {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            self.name,
            vec![Port::typed::<PesPacket>("in")],
            vec![typed::<MediaPacket>("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.streams.clear();
        Ok(vec![typed_from(Kind::typed::<MediaPacket>(), inputs)])
    }
    fn process(&mut self, io: &mut Io) {
        for p in io.items::<PesPacket>(0) {
            let pe = &p.value;
            let info: CodecInfo = pe
                .info
                .unwrap_or_else(|| crate::codec::identify(0x06, &Default::default()));
            if !self.streams.contains_key(&pe.pid) {
                self.streams.clear();
            }
            let es = self.streams.entry(pe.pid).or_insert_with(|| {
                let mut e = EsState::new(pe.pid, info);
                e.aac_profile_level = pe.aac_profile_level;
                e
            });
            let out = io.items_out::<MediaPacket>(0);
            es.push(pe, pe.stc, &mut |m: EsPacket| {
                out.push(Timed {
                    time: p.time,
                    value: m.media(),
                })
            });
            if let Some((signalled, carried)) = es.warning.take() {
                if !self.warned.contains(&pe.pid) {
                    self.warned.push(pe.pid);
                    self.published.results_mut().log(format!(
                        "PID 0x{:04X} is signalled as {signalled:?} but carries {carried:?}",
                        pe.pid
                    ));
                }
            }
            let r = &mut self.published;
            let (codec, pid) = (es.info.codec, i64::from(pe.pid));
            if es.info.kind() == EsKind::Video {
                r.put(&PesVideoFields {
                    codec,
                    pid,
                    width: i64::from(es.props.width),
                    height: i64::from(es.props.height),
                    frame_rate: es
                        .props
                        .frame_rate
                        .map(|(n, d)| f64::from(n) / f64::from(d.max(1))),
                });
            } else {
                r.put(&PesAudioFields {
                    codec,
                    pid,
                    sample_rate: Hertz(f64::from(es.props.sample_rate)),
                    channels: i64::from(es.props.channels),
                });
            }
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// The result fields of `mpegts.teletext`: the counts of the stream and the page shown.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TeletextStreamFields<'a> {
    /// Teletext data units decoded.
    #[field(label = "Data units")]
    pub data_units: i64,
    /// Teletext pages held.
    #[field(label = "Pages")]
    pub pages: i64,
    /// The page shown.
    #[field(label = "Page")]
    pub page: i64,
    /// The text of the page shown, its rows joined with line feeds. No value while the page is not held.
    #[field(label = "Page text")]
    pub page_text: Option<&'a str>,
}

sdrkit_core::commands! {
    /// The commands of [`TeletextComponent`].
    pub enum TeletextCommand {
        /// Shows a page.
        TeletextSelect = TELETEXT_SELECT.command,
    }
}

/// The page shown first.
const FIRST_PAGE: u16 = 100;

/// `mpegts.teletext`: the pages of a teletext stream. PES of other streams are not read. Results: the page shown and its
/// text, and the tables `teletext_pages` and `teletext_subpages`. Command: `teletext.select`.
pub struct TeletextComponent {
    decoder: TeletextDecoder,
    tables: TeletextTables,
    pid: u16,
    page: u16,
    published: Published,
}

impl Default for TeletextComponent {
    fn default() -> Self {
        TeletextComponent {
            decoder: TeletextDecoder::new(),
            tables: TeletextTables::default(),
            pid: 0,
            page: FIRST_PAGE,
            published: Published::new(TeletextStreamFields::SHAPE.fields)
                .with_tables(&TeletextTables::TABLES),
        }
    }
}

impl TeletextComponent {
    /// The decoder and its pages.
    pub fn decoder(&self) -> &TeletextDecoder {
        &self.decoder
    }

    fn publish(&mut self) {
        let r = &mut self.published;
        let text = self
            .decoder
            .page(self.page, None)
            .map(|p| p.text().join("\n"));
        r.put(&TeletextStreamFields {
            data_units: self.decoder.stats.data_units as i64,
            pages: self.decoder.pages().len() as i64,
            page: i64::from(self.page),
            page_text: text.as_deref(),
        });
        self.tables.write(&self.decoder, r);
    }
}

impl Component for TeletextComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpegts.teletext",
            vec![Port::typed::<PesPacket>("in")],
            vec![typed::<TeletextPage>("pages")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.decoder.clear();
        Ok(vec![typed_from(Kind::typed::<TeletextPage>(), inputs)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut any = false;
        for p in io.items::<PesPacket>(0) {
            if p.value.info.is_some_and(|i| i.kind() != EsKind::Teletext) {
                continue;
            }
            if p.value.pid != self.pid {
                self.decoder.clear();
                self.pid = p.value.pid;
            }
            self.decoder.push_pes(p.value.payload.bytes());
            for (n, s) in self.decoder.take_completed() {
                if let Some(pg) = self.decoder.page(n, Some(s)) {
                    io.items_out::<TeletextPage>(0).push(Timed {
                        time: p.time,
                        value: pg,
                    });
                }
            }
            any = true;
        }
        if any {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    /// The pages of the frequency before are gone, unless the stack keeps the page tables.
    fn forget(&mut self, kept: &Kept) {
        if kept.table(TELETEXT_PAGE_TABLE.key) || kept.table(TELETEXT_SUBPAGE_TABLE.key) {
            return;
        }
        self.decoder.clear();
        self.page = FIRST_PAGE;
        self.publish();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        TeletextCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TeletextCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TeletextCommand::TeletextSelect => {
                let TeletextSelectArgs { row } = TELETEXT_SELECT.args(args)?;
                self.page = row
                    .parse::<u16>()
                    .ok()
                    .filter(|n| (100..=899).contains(n))
                    .ok_or_else(|| CommandError::NoSuchRow {
                        table: table_id(&self.published, &TELETEXT_PAGE_TABLE),
                        key: row.to_owned(),
                    })?;
                self.publish();
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// The result fields of `mpegts.dvb_subtitle`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DvbSubtitleFields {
    /// Display sets decoded.
    #[field(label = "Display sets")]
    pub display_sets: i64,
}

/// `mpegts.dvb_subtitle`: the display sets of a DVB subtitle stream. PES of other streams are not read. Result:
/// `display_sets`.
pub struct SubtitleComponent {
    decoder: Option<(u16, u16, u16, SubtitleDecoder)>,
    published: Published,
}

impl Default for SubtitleComponent {
    fn default() -> Self {
        SubtitleComponent {
            decoder: None,
            published: Published::new(DvbSubtitleFields::SHAPE.fields),
        }
    }
}

impl Component for SubtitleComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpegts.dvb_subtitle",
            vec![Port::typed::<PesPacket>("in")],
            vec![typed::<SubtitleDisplay>("displays")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.decoder = None;
        Ok(vec![typed_from(Kind::typed::<SubtitleDisplay>(), inputs)])
    }
    fn process(&mut self, io: &mut Io) {
        for p in io.items::<PesPacket>(0) {
            if p.value.info.is_some_and(|i| i.kind() != EsKind::Subtitles) {
                continue;
            }
            let (c, a) = p.value.pages.unwrap_or((1, 1));
            let reset = self
                .decoder
                .as_ref()
                .is_none_or(|d| (d.0, d.1, d.2) != (p.value.pid, c, a));
            if reset {
                self.decoder = Some((p.value.pid, c, a, SubtitleDecoder::new(c, a)));
            }
            let Some((_, _, _, d)) = self.decoder.as_mut() else {
                continue;
            };
            for s in d.push_pes(p.value.payload.bytes(), p.value.pts) {
                io.items_out::<SubtitleDisplay>(0).push(Timed {
                    time: p.time,
                    value: s,
                });
            }
            self.published.put(&DvbSubtitleFields {
                display_sets: d.display_sets as i64,
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn forget(&mut self, _: &Kept) {
        self.decoder = None;
        self.published.put(&DvbSubtitleFields { display_sets: 0 });
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

const WITH_SI: Setting<field::Bool> = Setting::new(
    "with_si",
    "Pass SDT, EIT and time (for a player)",
    "",
    false,
);

sdrkit_core::settings! {
    /// The settings of [`TsFilterComponent`].
    pub enum TsFilterSetting {
        /// Pass the SDT, EIT and time tables.
        WithSi = WITH_SI,
    }
}

sdrkit_core::commands! {
    /// The commands of [`TsFilterComponent`].
    pub enum TsFilterCommand {
        /// Passes another service.
        ServiceSelect = SERVICE_SELECT_KIND.command,
    }
}

/// `mpegts.ts_filter`: one service as a transport stream with its own PAT. The first playable service until
/// `service.select` names one.
pub struct TsFilterComponent {
    psi: Demultiplexer,
    filter: TsFilter,
    out_buf: Vec<[u8; TS_SIZE]>,
    times: Vec<f64>,
    ts_out: TsOut,
    scratch: DemuxOutput,
    /// The service to pass. `None`: the first playable service.
    service: Option<u16>,
    version: u64,
}

impl Default for TsFilterComponent {
    fn default() -> Self {
        let mut psi = Demultiplexer::new();
        psi.auto_select = true;
        psi.forward_pes = RouteSet::ALL;
        TsFilterComponent {
            psi,
            filter: TsFilter::new(WITH_SI.default),
            out_buf: Vec::new(),
            times: Vec::new(),
            ts_out: TsOut::default(),
            scratch: DemuxOutput::default(),
            service: None,
            version: u64::MAX,
        }
    }
}

impl Component for TsFilterComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpegts.ts_filter",
            vec![stream::port("in")],
            vec![stream::port("out").same_format_as(0)],
        )
        .with_settings(TsFilterSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        self.out_buf.clear();
        self.times.clear();
        for p in io.items::<TsPacket>(0) {
            let b = p.value.bytes();
            self.psi.push(b, Some(p.time), false, &mut self.scratch);
            self.scratch.clear();
            if self.psi.si.version != self.version {
                self.version = self.psi.si.version;
                let sid = self.service.or_else(|| self.psi.si.first_playable());
                if let Some(s) = sid.and_then(|s| self.psi.si.service(s)) {
                    let es: Vec<u16> = s.streams.iter().map(|e| e.pid).collect();
                    self.filter.set_service(
                        s.service_id,
                        s.pmt_pid,
                        s.pcr_pid,
                        &es,
                        self.psi.si.tsid,
                    );
                }
            }
            let n = self.out_buf.len();
            self.filter.push(b, &mut self.out_buf);
            for _ in n..self.out_buf.len() {
                self.times.push(p.time);
            }
        }
        // The filter outputs packets that start with the sync byte, so none is refused.
        let _ = self
            .ts_out
            .extend(io.items_out::<TsPacket>(0), &self.out_buf, &self.times);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TsFilterSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TsFilterSetting::WithSi => self.filter.with_si = WITH_SI.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(TsFilterSetting::WithSi, Value::Bool(self.filter.with_si))])
    }
    fn commands(&self) -> &Commands {
        TsFilterCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TsFilterCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TsFilterCommand::ServiceSelect => {
                let row = args.arg(&SERVICE_ROW)?;
                // This component has no services table: the row key is one of the services table of `mpegts.ts_demux`.
                self.service = if row.is_empty() {
                    None
                } else {
                    let arg = args.schema().id(SERVICE_ROW.field.key).map_err(|_| {
                        CommandError::NotAnArgument {
                            key: SERVICE_ROW.field.key,
                        }
                    })?;
                    let sid = row
                        .parse::<u16>()
                        .ok()
                        .filter(|&s| self.psi.si.service(s).is_some())
                        .ok_or(CommandError::NoSuchKey { arg })?;
                    Some(sid)
                };
                self.version = u64::MAX;
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// `mpegts.ts_media`: a transport stream as coded media of [`Codec::MpegTs`]. Each pass writes one packet that holds the
/// transport packets of the pass, 188 bytes each, at the time of the first one.
#[derive(Default)]
pub struct TsMedia {
    pool: sdrkit_graph::PacketPool,
}

impl Component for TsMedia {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpegts.ts_media",
            vec![stream::port("in")],
            vec![
                Port::media("out", &[MediaSpec::new(Codec::MpegTs, Packaging::Raw)])
                    .rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let mut f = typed_from(Kind::Bytes, inputs);
        f.media = Some(MediaFormat::new(Codec::MpegTs, Packaging::Raw));
        Ok(vec![f])
    }
    fn process(&mut self, io: &mut Io) {
        let packets = io.items::<TsPacket>(0);
        let Some(first) = packets.first() else {
            return;
        };
        let packet = self.pool.make(packets.len() * TS_SIZE, first.time, |b| {
            for (dst, p) in b.as_chunks_mut::<TS_SIZE>().0.iter_mut().zip(packets) {
                *dst = *p.value.bytes();
            }
        });
        io.bytes_out(0).push(packet);
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

sdrkit_core::choice_enum! {
    /// The coded media that `mpegts.media_demux` outputs.
    pub enum DemuxCodec {
        /// H.264.
        H264 = ("h264", "H.264"),
        /// HEVC.
        Hevc = ("hevc", "HEVC"),
        /// MPEG-2 video.
        Mpeg2Video = ("mpeg2video", "MPEG-2 video"),
        /// AAC-LC.
        Aac = ("aac", "AAC-LC"),
        /// HE-AAC.
        HeAac = ("he_aac", "HE-AAC"),
        /// HE-AAC v2.
        HeAacV2 = ("he_aac_v2", "HE-AAC v2"),
        /// MPEG-1 Layer I.
        Mp1 = ("mp1", "MPEG-1 Layer I"),
        /// MPEG-1 Layer II.
        Mp2 = ("mp2", "MPEG-1 Layer II"),
        /// MPEG-1 Layer III.
        Mp3 = ("mp3", "MPEG-1 Layer III"),
        /// AC-3.
        Ac3 = ("ac3", "AC-3"),
        /// E-AC-3.
        EAc3 = ("eac3", "E-AC-3"),
    }
}

impl DemuxCodec {
    /// Every entry, in choice order.
    const ALL: [DemuxCodec; 11] = [
        DemuxCodec::H264,
        DemuxCodec::Hevc,
        DemuxCodec::Mpeg2Video,
        DemuxCodec::Aac,
        DemuxCodec::HeAac,
        DemuxCodec::HeAacV2,
        DemuxCodec::Mp1,
        DemuxCodec::Mp2,
        DemuxCodec::Mp3,
        DemuxCodec::Ac3,
        DemuxCodec::EAc3,
    ];

    /// The codec.
    pub fn codec(self) -> Codec {
        match self {
            DemuxCodec::H264 => Codec::H264,
            DemuxCodec::Hevc => Codec::Hevc,
            DemuxCodec::Mpeg2Video => Codec::Mpeg2Video,
            DemuxCodec::Aac => Codec::Aac,
            DemuxCodec::HeAac => Codec::HeAac,
            DemuxCodec::HeAacV2 => Codec::HeAacV2,
            DemuxCodec::Mp1 => Codec::Mp1,
            DemuxCodec::Mp2 => Codec::Mp2,
            DemuxCodec::Mp3 => Codec::Mp3,
            DemuxCodec::Ac3 => Codec::Ac3,
            DemuxCodec::EAc3 => Codec::EAc3,
        }
    }

    /// The entry of `codec`. `None` for a codec that the demultiplexer does not give.
    pub fn of(codec: Codec) -> Option<DemuxCodec> {
        DemuxCodec::ALL.into_iter().find(|d| d.codec() == codec)
    }

    /// The packaging of its packets.
    pub fn packaging(self) -> Packaging {
        match self {
            DemuxCodec::H264 | DemuxCodec::Hevc => Packaging::AnnexB,
            DemuxCodec::Aac | DemuxCodec::HeAac | DemuxCodec::HeAacV2 => Packaging::Adts,
            DemuxCodec::Mpeg2Video
            | DemuxCodec::Mp1
            | DemuxCodec::Mp2
            | DemuxCodec::Mp3
            | DemuxCodec::Ac3
            | DemuxCodec::EAc3 => Packaging::Raw,
        }
    }
}

const DEMUX_CODEC: Setting<field::Choice<DemuxCodec>> =
    Setting::choice("codec", "Codec", DemuxCodec::Mp2);

sdrkit_core::settings! {
    /// The settings of `mpegts.media_demux`.
    pub enum MediaDemuxSetting {
        /// The codec given.
        Codec = DEMUX_CODEC,
    }
}

/// The output of a [`ServiceDemux`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DemuxOut {
    /// Every PES of the service that plays.
    Pes,
    /// The media packets of its selected video and audio.
    Media,
    /// The packets of one codec, as coded media.
    Coded(DemuxCodec),
}

/// The shared demultiplexer as a one-in, one-out component, for the conversions of a transport stream that the graph
/// inserts. It plays the first playable service.
///
/// - `mpegts.pes_demux` ([`ServiceDemux::pes`]): every PES of the service.
/// - `mpegts.es_demux` ([`ServiceDemux::media`]): the media packets of its video and audio, each with its format.
/// - `mpegts.media_demux` ([`ServiceDemux::coded`]): the packets of the codec that the setting `codec` names, as coded media
///   in that format. The packets of other codecs are not given.
pub struct ServiceDemux {
    demux: Demultiplexer,
    out: DemuxOutput,
    gives: DemuxOut,
    lost: Vec<u64>,
}

impl ServiceDemux {
    fn new(gives: DemuxOut) -> Self {
        let mut demux = Demultiplexer::new();
        demux.auto_select = true;
        if gives == DemuxOut::Pes {
            demux.forward_pes = RouteSet::ALL;
        }
        ServiceDemux {
            demux,
            out: DemuxOutput::default(),
            gives,
            lost: Vec::new(),
        }
    }

    /// Every PES of the service that plays.
    pub fn pes() -> Self {
        Self::new(DemuxOut::Pes)
    }

    /// The media packets of the selected video and audio.
    pub fn media() -> Self {
        Self::new(DemuxOut::Media)
    }

    /// The packets of `codec`, as coded media.
    pub fn coded(codec: DemuxCodec) -> Self {
        Self::new(DemuxOut::Coded(codec))
    }
}

impl Component for ServiceDemux {
    fn info(&self) -> ComponentInfo {
        let input = vec![stream::port("in")];
        match self.gives {
            DemuxOut::Pes => {
                ComponentInfo::processor("mpegts.pes_demux", input, vec![typed::<PesPacket>("out")])
            }
            DemuxOut::Media => ComponentInfo::processor(
                "mpegts.es_demux",
                input,
                vec![typed::<MediaPacket>("out")],
            ),
            DemuxOut::Coded(c) => ComponentInfo::processor(
                "mpegts.media_demux",
                input,
                vec![
                    Port::media("out", &[MediaSpec::new(c.codec(), c.packaging())])
                        .rate_from(RateFrom::Fixed(0.0)),
                ],
            )
            .with_settings(MediaDemuxSetting::schema()),
        }
        .manual_tags()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![match self.gives {
            DemuxOut::Pes => typed_from(Kind::typed::<PesPacket>(), inputs),
            DemuxOut::Media => typed_from(Kind::typed::<MediaPacket>(), inputs),
            DemuxOut::Coded(c) => {
                let mut f = typed_from(Kind::Bytes, inputs);
                f.media = Some(MediaFormat::new(c.codec(), c.packaging()));
                f
            }
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let mut lost = std::mem::take(&mut self.lost);
        losses(io, 0, &mut lost);
        let first = io.input(0).first();
        for (k, p) in io.items::<TsPacket>(0).iter().enumerate() {
            let loss = lost.contains(&(first + k as u64));
            self.demux
                .push(p.value.bytes(), Some(p.time), loss, &mut self.out);
        }
        self.lost = lost;
        match self.gives {
            DemuxOut::Pes => {
                io.items_out::<PesPacket>(0)
                    .extend(self.out.pes.drain(..).map(|(_, p)| Timed {
                        time: p.time,
                        value: p,
                    }));
            }
            DemuxOut::Media => {
                io.items_out::<MediaPacket>(0).extend(
                    self.out
                        .media
                        .drain(..)
                        .filter(EsPacket::decodable)
                        .map(|m| Timed {
                            time: m.packet.time,
                            value: m.media(),
                        }),
                );
            }
            DemuxOut::Coded(c) => {
                io.bytes_out(0).extend(
                    self.out
                        .media
                        .drain(..)
                        .filter(|m| m.format.codec == c.codec() && m.decodable())
                        .map(|m| m.packet),
                );
            }
        }
        self.out.clear();
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MediaDemuxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MediaDemuxSetting::Codec => match self.gives {
                DemuxOut::Coded(_) => self.gives = DemuxOut::Coded(DEMUX_CODEC.read(id, v)?),
                DemuxOut::Pes | DemuxOut::Media => return Err(SettingError::NoSuchId { id }),
            },
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        match self.gives {
            DemuxOut::Coded(c) => values_of([(MediaDemuxSetting::Codec, Value::Choice(c.index()))]),
            DemuxOut::Pes | DemuxOut::Media => no_values(),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// `mpegts.sl_demux`: the MPEG-4 elementary streams of the first programme of a transport stream (as DMB sends them), as
/// access units. Each unit keeps the time of the transport packet that completed it.
#[derive(Default)]
pub struct SlDemuxComponent {
    demux: SlDemux,
    units: Vec<AccessUnit>,
}

impl SlDemuxComponent {
    /// The demultiplexer.
    pub fn demux(&self) -> &SlDemux {
        &self.demux
    }
}

impl Component for SlDemuxComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mpegts.sl_demux",
            vec![stream::port("in")],
            vec![typed::<AccessUnit>("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_from(Kind::typed::<AccessUnit>(), inputs)])
    }
    fn process(&mut self, io: &mut Io) {
        for p in io.items::<TsPacket>(0) {
            self.demux.push(p.value.bytes(), &mut self.units);
            for u in self.units.drain(..) {
                io.items_out::<AccessUnit>(0).push(Timed {
                    time: p.time,
                    value: u,
                });
            }
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// Returns the settings of `mpegts.media_demux` that produce the coded media that `want` accepts, or `None` if `want` accepts
/// no codec that the demultiplexer outputs.
fn coded_for(_: &Format, want: &FormatConstraint) -> Option<Values> {
    let c = want.media.iter().find_map(|m| DemuxCodec::of(m.codec))?;
    Some(values_of([(
        MediaDemuxSetting::Codec,
        Value::Choice(c.index()),
    )]))
}

/// Adds every transport stream component to `r` under its `mpegts.` name, and adds the conversions of a transport stream
/// to the graph's catalog: to PES, to media packets, to coded media of one codec, and to MPEG-4 access units.
pub fn register(r: &mut Registry) {
    r.add("mpegts.ts_input", || Box::new(TsInput::default()));
    r.add("mpegts.ts_demux", || Box::new(TsDemuxComponent::default()));
    r.add("mpegts.psi_si", || Box::new(PsiSi::default()));
    r.add("mpegts.pes_video", || Box::new(PesMedia::video()));
    r.add("mpegts.pes_audio", || Box::new(PesMedia::audio()));
    r.add("mpegts.teletext", || Box::new(TeletextComponent::default()));
    r.add("mpegts.dvb_subtitle", || {
        Box::new(SubtitleComponent::default())
    });
    r.add(
        "mpegts.ts_filter",
        || Box::new(TsFilterComponent::default()),
    );
    r.add("mpegts.ts_media", || Box::new(TsMedia::default()));
    r.add("mpegts.ts_monitor", || {
        Box::new(crate::monitor::TsMonitorComponent::default())
    });
    r.add("mpegts.pes_demux", || Box::new(ServiceDemux::pes()));
    r.add("mpegts.es_demux", || Box::new(ServiceDemux::media()));
    r.add("mpegts.media_demux", || {
        Box::new(ServiceDemux::coded(DEMUX_CODEC.default))
    });
    r.add_adapter(Adapter {
        kind: "mpegts.pes_demux",
        from: stream::kind(),
        to: Kind::typed::<PesPacket>(),
        class: AdapterClass::Lossless,
        cost: 2,
        settings: |_, _| Some(no_values()),
        make: || Box::new(ServiceDemux::pes()),
    });
    r.add_adapter(Adapter {
        kind: "mpegts.es_demux",
        from: stream::kind(),
        to: Kind::typed::<MediaPacket>(),
        class: AdapterClass::Lossless,
        cost: 3,
        settings: |_, _| Some(no_values()),
        make: || Box::new(ServiceDemux::media()),
    });
    r.add_adapter(Adapter {
        kind: "mpegts.media_demux",
        from: stream::kind(),
        to: Kind::Bytes,
        class: AdapterClass::Lossless,
        cost: 3,
        settings: coded_for,
        make: || Box::new(ServiceDemux::coded(DEMUX_CODEC.default)),
    });
    r.add("mpegts.sl_demux", || Box::new(SlDemuxComponent::default()));
    r.add_adapter(Adapter {
        kind: "mpegts.sl_demux",
        from: stream::kind(),
        to: Kind::typed::<AccessUnit>(),
        class: AdapterClass::Lossless,
        cost: 3,
        settings: |_, _| Some(no_values()),
        make: || Box::new(SlDemuxComponent::default()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A demultiplexer that saw a multiplex of 20 services and 60 PIDs, and the packets of one more round of every PID.
    fn sixty_pids() -> (TsDemuxComponent, Vec<[u8; 188]>) {
        use crate::source::{PacketSource, TestMux, TestService};
        let mut b = TestMux::builder(1);
        for i in 0..20u16 {
            b = b.service(TestService::tv(100 + i, &format!("Service {i}"), i + 1));
        }
        let mut mux = b.build().unwrap();
        let mut c = TsDemuxComponent::default();
        let mut round = Vec::new();
        let mut p = [0u8; 188];
        for _ in 0..600 {
            mux.next_packet(&mut p);
            round.push(p);
        }
        let mut pid = 0x0200u16;
        loop {
            for q in &round {
                c.demux.push(q, None, false, &mut c.out);
            }
            c.out.clear();
            if c.demux.ts.reports().count() >= 60 {
                break;
            }
            let mut q = [0xFFu8; 188];
            let h = crate::packet::Header {
                pid,
                afc: 1,
                ..Default::default()
            };
            crate::packet::write_header(&mut q, &h);
            round.push(q);
            pid += 1;
        }
        assert_eq!(c.demux.ts.reports().count(), 60);
        (c, round)
    }

    /// The mean and the worst time of a report in µs: the whole report, and its services and tracks part. `busy`: every
    /// PID got packets since the report before.
    fn report_times(c: &mut TsDemuxComponent, round: &[[u8; 188]], busy: bool) -> (f64, f64, f64) {
        let (mut worst, mut sum, mut svc) = (0.0f64, 0.0f64, 0.0f64);
        let n = 200;
        for _ in 0..n {
            if busy {
                for q in round {
                    c.demux.push(q, None, false, &mut c.out);
                }
                c.out.clear();
            }
            let t = std::time::Instant::now();
            let reports = c.demux.reports().count();
            c.publish();
            let dt = t.elapsed().as_secs_f64() * 1e6;
            assert_eq!(reports, 60);
            worst = worst.max(dt);
            sum += dt;
            let t = std::time::Instant::now();
            c.tables.write_services(&c.demux.si, &mut c.published);
            svc += t.elapsed().as_secs_f64() * 1e6;
        }
        (sum / f64::from(n), worst, svc / f64::from(n))
    }

    /// cargo test --release -p sdrkit-proto-mpegts report_time -- --nocapture
    #[test]
    fn report_time_of_sixty_pids() {
        let (mut c, round) = sixty_pids();
        c.publish();
        for busy in [true, false] {
            let (mean, worst, services) = report_times(&mut c, &round, busy);
            eprintln!(
                "report of 60 PIDs, busy {busy}: mean {mean:.1} us, worst {worst:.1} us, services part {services:.1} us"
            );
        }
        let r = c.published.results();
        let t = r
            .decl()
            .table_id(PIDS.key)
            .and_then(|id| r.table(id))
            .unwrap();
        assert_eq!(t.len(), 60);
    }

    fn rows(c: &PsiSi, table: &str) -> usize {
        let r = c.published.results();
        r.decl()
            .table_id(table)
            .and_then(|id| r.table(id))
            .map_or(0, |t| t.len())
    }

    /// The service information with one guide event of another multiplex and counts of the present one.
    fn with_guide() -> PsiSi {
        let mut c = PsiSi::default();
        let ev = EpgEvent {
            service: (1, 2, 3),
            event_id: 7,
            start: 1_000,
            duration: 600,
            title: "News".to_owned(),
            ratings: vec![("AUS".to_owned(), 5)],
            ..EpgEvent::default()
        };
        assert!(c.db.epg.insert(ev.clone(), 0));
        c.events.push(ev);
        c.db.network_id = 12;
        c.db.counts.entry("PAT").or_default().accepted = 4;
        c.publish();
        assert_eq!(
            (
                rows(&c, "guide"),
                rows(&c, "guide_ratings"),
                rows(&c, "sections")
            ),
            (1, 1, 1)
        );
        c
    }

    #[test]
    fn a_change_of_frequency_keeps_a_kept_guide_and_forgets_the_multiplex() {
        let mut c = with_guide();
        c.forget(&Kept::of(&[], &["guide", "guide_ratings"]));
        assert_eq!(
            (
                rows(&c, "guide"),
                rows(&c, "guide_ratings"),
                rows(&c, "sections")
            ),
            (1, 1, 0)
        );
        let f = c.published.results().fields();
        assert_eq!(f.at(&PsiSiFields::NETWORK_ID), Some(0));
        assert_eq!(f.at(&PsiSiFields::UTC), None);
    }

    #[test]
    fn a_change_of_frequency_forgets_a_guide_that_is_not_kept() {
        let mut c = with_guide();
        c.forget(&Kept::of(&[], &[]));
        assert_eq!(
            (
                rows(&c, "guide"),
                rows(&c, "guide_ratings"),
                rows(&c, "sections")
            ),
            (0, 0, 0)
        );
    }
}
