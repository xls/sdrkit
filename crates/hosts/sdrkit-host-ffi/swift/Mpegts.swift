// sdrkit: the typed interface of the shared interface "mpegts", whose items the stacks of several families show, version 1.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum MpegtsInterface {
    public static let major = 1
    public static let minor = 1
}

/// The result fields of `mpegts.ts_monitor`: the counts of the transport stream, which a stack shows under its own keys.
public struct MpegtsTsMonitorFields {
    /// Packets seen.
    public var packets: Int64
    /// Packets with the transport_error_indicator.
    public var flagged: Int64
    /// Scrambled packets.
    public var scrambled: Int64
    /// Numbered packets that match.
    public var numberedGood: Int64
    /// Numbered packets that do not match.
    public var numberedWrong: Int64
    /// Breaks in the numbering.
    public var numberedGaps: Int64
    /// Bit rate over the last second.
    public var bitrateBps: Double
    /// A PAT was seen.
    public var patSeen: Bool
    /// The service names of the SDT, one per line.
    public var services: String

    init(_ c: sdrkit_mpegts_ts_monitor_fields) {
        packets = c.packets
        flagged = c.flagged
        scrambled = c.scrambled
        numberedGood = c.numbered_good
        numberedWrong = c.numbered_wrong
        numberedGaps = c.numbered_gaps
        bitrateBps = c.bitrate_bps
        patSeen = c.pat_seen
        services = sdrkitText(c.services)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsMonitorFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_monitor_fields()
        let code = sdrkit_mpegts_ts_monitor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsMonitorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `mpegts.ts_demux`: the counts of the packet layer and the selection.
public struct MpegtsTsDemuxFields {
    public enum PlayerStatus: Int32 {
        case starting = 0
        case playing = 1
        case scrambled = 2
        case noService = 3
    }
    /// Packets seen.
    public var packets: Int64
    /// Null packets seen.
    public var nullPackets: Int64
    /// Packets with the transport_error_indicator.
    public var teiPackets: Int64
    /// Continuity errors.
    public var ccErrors: Int64
    /// The bit rate of the multiplex.
    public var muxKbps: Double
    /// The bit rate of the null packets.
    public var nullKbps: Double
    /// The service that plays. No value while none plays.
    public var selectedService: Int64?
    /// The audio PID that plays. No value while none plays.
    public var selectedAudio: Int64?
    /// The subtitle PID shown. No value while none is shown.
    public var selectedSubtitle: Int64?
    /// What the player does.
    public var playerStatus: PlayerStatus?
    /// Whether the decoders of the library decode the audio that plays: absent while no audio plays, and when the host gave no decoder of MPEG audio or AAC (it decodes the coded audio itself).
    public var audioDecoded: Bool?

    init(_ c: sdrkit_mpegts_ts_demux_fields) {
        packets = c.packets
        nullPackets = c.null_packets
        teiPackets = c.tei_packets
        ccErrors = c.cc_errors
        muxKbps = c.mux_kbps
        nullKbps = c.null_kbps
        selectedService = c.has_selected_service ? c.selected_service : nil
        selectedAudio = c.has_selected_audio ? c.selected_audio : nil
        selectedSubtitle = c.has_selected_subtitle ? c.selected_subtitle : nil
        playerStatus = PlayerStatus(rawValue: c.player_status)
        audioDecoded = c.has_audio_decoded ? c.audio_decoded : nil
    }
}

/// The result fields of `mpegts.psi_si`: the network and the time of the stream.
public struct MpegtsPsiSiFields {
    /// The network name.
    public var networkName: String
    /// network_id.
    public var networkId: Int64
    /// original_network_id.
    public var onid: Int64
    /// transport_stream_id.
    public var tsid: Int64
    /// The number of services.
    public var serviceCount: Int64
    /// The UTC of the stream. No value before the stream gave the time.
    public var utc: Double?
    /// The local time offset in force. No value before the stream gave one.
    public var localOffsetMin: Int64?

    init(_ c: sdrkit_mpegts_psi_si_fields) {
        networkName = sdrkitText(c.network_name)
        networkId = c.network_id
        onid = c.onid
        tsid = c.tsid
        serviceCount = c.service_count
        utc = c.has_utc ? c.utc : nil
        localOffsetMin = c.has_local_offset_min ? c.local_offset_min : nil
    }
}

/// The result fields of `mpegts.dvb_subtitle`.
public struct MpegtsDvbSubtitleFields {
    /// Display sets decoded.
    public var displaySets: Int64

    init(_ c: sdrkit_mpegts_dvb_subtitle_fields) {
        displaySets = c.display_sets
    }
}

/// The result fields of `mpegts.teletext`: the counts of the stream and the page shown.
public struct MpegtsTeletextStreamFields {
    /// Teletext data units decoded.
    public var dataUnits: Int64
    /// Teletext pages held.
    public var pages: Int64
    /// The page shown.
    public var page: Int64
    /// The text of the page shown, its rows joined with line feeds. No value while the page is not held.
    public var pageText: String?

    init(_ c: sdrkit_mpegts_teletext_stream_fields) {
        dataUnits = c.data_units
        pages = c.pages
        page = c.page
        pageText = c.has_page_text ? sdrkitText(c.page_text) : nil
    }
}

/// The result fields of `mpegts.pes_video`: the stream and its picture.
public struct MpegtsPesVideoFields {
    public enum Codec: Int32 {
        case mpeg1video = 0
        case mpeg2video = 1
        case h264 = 2
        case hevc = 3
        case avs = 4
        case avs2 = 5
        case avs3 = 6
        case mp2 = 7
        case aac = 8
        case ac3 = 9
        case eac3 = 10
        case ac4 = 11
        case dts = 12
        case dra = 13
        case lpcm = 14
        case opus = 15
        case teletext = 16
        case dvbsub = 17
        case data = 18
    }
    /// The codec of the stream.
    public var codec: Codec?
    /// The PID.
    public var pid: Int64
    /// The picture width.
    public var width: Int64
    /// The picture height.
    public var height: Int64
    /// Frames per second. No value before the stream gave the rate.
    public var frameRate: Double?

    init(_ c: sdrkit_mpegts_pes_video_fields) {
        codec = Codec(rawValue: c.codec)
        pid = c.pid
        width = c.width
        height = c.height
        frameRate = c.has_frame_rate ? c.frame_rate : nil
    }
}

/// The result fields of `mpegts.pes_audio`: the stream and its sound.
public struct MpegtsPesAudioFields {
    public enum Codec: Int32 {
        case mpeg1video = 0
        case mpeg2video = 1
        case h264 = 2
        case hevc = 3
        case avs = 4
        case avs2 = 5
        case avs3 = 6
        case mp2 = 7
        case aac = 8
        case ac3 = 9
        case eac3 = 10
        case ac4 = 11
        case dts = 12
        case dra = 13
        case lpcm = 14
        case opus = 15
        case teletext = 16
        case dvbsub = 17
        case data = 18
    }
    /// The codec of the stream.
    public var codec: Codec?
    /// The PID.
    public var pid: Int64
    /// The audio sample rate.
    public var sampleRate: Double
    /// The audio channels.
    public var channels: Int64

    init(_ c: sdrkit_mpegts_pes_audio_fields) {
        codec = Codec(rawValue: c.codec)
        pid = c.pid
        sampleRate = c.sample_rate
        channels = c.channels
    }
}

/// A teletext page held. The row key is the page number.
public struct MpegtsTeletextPageRow {
    /// The row key.
    public var key: String
    /// The page number.
    public var page: Int64
    /// The number of subpages held.
    public var subpages: Int64

    init(_ c: sdrkit_mpegts_teletext_page_row) {
        key = sdrkitText(c.key)
        page = c.page
        subpages = c.subpages
    }
}

/// A subpage of a teletext page. The row key is `<page>/<subcode>`, the subcode as four hex digits.
public struct MpegtsTeletextSubpageRow {
    /// The row key.
    public var key: String
    /// The row key of the page in `teletext_pages`.
    public var pageRow: String
    /// The subcode, in nibble form.
    public var subcode: Int64
    /// The version, raised on each change.
    public var version: Int64

    init(_ c: sdrkit_mpegts_teletext_subpage_row) {
        key = sdrkitText(c.key)
        pageRow = sdrkitText(c.page_row)
        subcode = c.subcode
        version = c.version
    }
}

/// A service of the multiplex. The row key is the service_id.
public struct MpegtsTsServiceRow {
    public enum Kind: Int32 {
        case tv = 0
        case radio = 1
        case data = 2
        case teletext = 3
        case other = 4
        case unknown = 5
    }
    public enum CaState: Int32 {
        case unknown = 0
        case clear = 1
        case flaggedClear = 2
        case scrambled = 3
    }
    /// The row key.
    public var key: String
    /// service_id.
    public var serviceId: Int64
    /// The name.
    public var name: String
    /// The provider.
    public var provider: String
    /// service_type.
    public var serviceType: Int64
    /// What the service is.
    public var kind: Kind?
    /// The logical channel number (0 if none).
    public var lcn: Int64
    /// Shown in a channel list.
    public var visible: Bool
    /// Scrambled.
    public var scrambled: Bool
    /// Whether it is scrambled.
    public var caState: CaState?
    /// The PMT PID.
    public var pmtPid: Int64
    /// The PCR PID.
    public var pcrPid: Int64
    /// running_status.
    public var runningStatus: Int64
    /// Listed in the PAT.
    public var inPat: Bool
    /// EIT_schedule_flag.
    public var eitSchedule: Bool
    /// EIT_present_following_flag.
    public var eitPresentFollowing: Bool
    /// original_network_id.
    public var onid: Int64
    /// transport_stream_id.
    public var tsid: Int64
    /// The title of the present event.
    public var nowTitle: String?
    /// The text of the present event.
    public var nowText: String?
    /// The start of the present event.
    public var nowStart: Double?
    /// The length of the present event.
    public var nowDuration: Double?
    /// The title of the following event.
    public var nextTitle: String?
    /// The text of the following event.
    public var nextText: String?
    /// The start of the following event.
    public var nextStart: Double?
    /// The length of the following event.
    public var nextDuration: Double?

    init(_ c: sdrkit_mpegts_ts_service_row) {
        key = sdrkitText(c.key)
        serviceId = c.service_id
        name = sdrkitText(c.name)
        provider = sdrkitText(c.provider)
        serviceType = c.service_type
        kind = Kind(rawValue: c.kind)
        lcn = c.lcn
        visible = c.visible
        scrambled = c.scrambled
        caState = CaState(rawValue: c.ca_state)
        pmtPid = c.pmt_pid
        pcrPid = c.pcr_pid
        runningStatus = c.running_status
        inPat = c.in_pat
        eitSchedule = c.eit_schedule
        eitPresentFollowing = c.eit_present_following
        onid = c.onid
        tsid = c.tsid
        nowTitle = c.has_now_title ? sdrkitText(c.now_title) : nil
        nowText = c.has_now_text ? sdrkitText(c.now_text) : nil
        nowStart = c.has_now_start ? c.now_start : nil
        nowDuration = c.has_now_duration ? c.now_duration : nil
        nextTitle = c.has_next_title ? sdrkitText(c.next_title) : nil
        nextText = c.has_next_text ? sdrkitText(c.next_text) : nil
        nextStart = c.has_next_start ? c.next_start : nil
        nextDuration = c.has_next_duration ? c.next_duration : nil
    }
}

/// A track of a service: an elementary stream, or one entry of a subtitle or teletext stream. The row key is `<service>/<pid>` or `<service>/<pid>/<n>`.
public struct MpegtsTsTrackRow {
    public enum Kind: Int32 {
        case video = 0
        case audio = 1
        case subtitles = 2
        case teletext = 3
        case data = 4
    }
    public enum Codec: Int32 {
        case mpeg1video = 0
        case mpeg2video = 1
        case h264 = 2
        case hevc = 3
        case avs = 4
        case avs2 = 5
        case avs3 = 6
        case mp2 = 7
        case aac = 8
        case ac3 = 9
        case eac3 = 10
        case ac4 = 11
        case dts = 12
        case dra = 13
        case lpcm = 14
        case opus = 15
        case teletext = 16
        case dvbsub = 17
        case data = 18
    }
    public enum Packaging: Int32 {
        case annexb = 0
        case elementary = 1
        case adts = 2
        case latm = 3
        case frames = 4
        case smpte302m = 5
        case pes = 6
        case none = 7
    }
    /// The row key.
    public var key: String
    /// The row key of the service in `services`.
    public var service: String
    /// The PID.
    public var pid: Int64
    /// stream_type.
    public var streamType: Int64
    /// What the track is.
    public var kind: Kind?
    /// The codec.
    public var codec: Codec?
    /// The packaging.
    public var packaging: Packaging?
    /// The four-character code.
    public var fourcc: String?
    /// The ISO 639-2 language code.
    public var lang: String
    /// audio_type of the language entry.
    public var audioType: Int64
    /// An audio description for people who cannot see.
    public var audioDescription: Bool
    /// Subtitles for the hard of hearing.
    public var hardOfHearing: Bool
    /// component_tag.
    public var componentTag: Int64?
    /// The bit rate of the PID.
    public var kbps: Double
    /// Scrambled.
    public var scrambled: Bool
    /// The profile.
    public var profile: String?
    /// The level.
    public var level: String?
    /// The picture width. No value for a track that is not video.
    public var width: Int64?
    /// The picture height. No value for a track that is not video.
    public var height: Int64?
    /// Frames per second.
    public var frameRate: Double?
    /// The display aspect.
    public var aspect: String?
    /// Interlaced.
    public var interlaced: Bool?
    /// The audio sample rate. No value for a track that is not audio.
    public var sampleRate: Double?
    /// The audio channels. No value for a track that is not audio.
    public var channels: Int64?
    /// The channel layout.
    public var layout: String?
    /// The bit rate that the frame header provides.
    public var bitrateKbps: Int64?
    /// The AAC object type or the MPEG audio layer.
    public var objectType: String?
    /// subtitling_type, or teletext_type.
    public var entryType: Int64?
    /// composition_page_id.
    public var compositionPage: Int64?
    /// ancillary_page_id.
    public var ancillaryPage: Int64?
    /// teletext_magazine_number.
    public var magazine: Int64?
    /// teletext_page_number.
    public var teletextPage: Int64?

    init(_ c: sdrkit_mpegts_ts_track_row) {
        key = sdrkitText(c.key)
        service = sdrkitText(c.service)
        pid = c.pid
        streamType = c.stream_type
        kind = c.has_kind ? Kind(rawValue: c.kind) : nil
        codec = c.has_codec ? Codec(rawValue: c.codec) : nil
        packaging = c.has_packaging ? Packaging(rawValue: c.packaging) : nil
        fourcc = c.has_fourcc ? sdrkitText(c.fourcc) : nil
        lang = sdrkitText(c.lang)
        audioType = c.audio_type
        audioDescription = c.audio_description
        hardOfHearing = c.hard_of_hearing
        componentTag = c.has_component_tag ? c.component_tag : nil
        kbps = c.kbps
        scrambled = c.scrambled
        profile = c.has_profile ? sdrkitText(c.profile) : nil
        level = c.has_level ? sdrkitText(c.level) : nil
        width = c.has_width ? c.width : nil
        height = c.has_height ? c.height : nil
        frameRate = c.has_frame_rate ? c.frame_rate : nil
        aspect = c.has_aspect ? sdrkitText(c.aspect) : nil
        interlaced = c.has_interlaced ? c.interlaced : nil
        sampleRate = c.has_sample_rate ? c.sample_rate : nil
        channels = c.has_channels ? c.channels : nil
        layout = c.has_layout ? sdrkitText(c.layout) : nil
        bitrateKbps = c.has_bitrate_kbps ? c.bitrate_kbps : nil
        objectType = c.has_object_type ? sdrkitText(c.object_type) : nil
        entryType = c.has_entry_type ? c.entry_type : nil
        compositionPage = c.has_composition_page ? c.composition_page : nil
        ancillaryPage = c.has_ancillary_page ? c.ancillary_page : nil
        magazine = c.has_magazine ? c.magazine : nil
        teletextPage = c.has_teletext_page ? c.teletext_page : nil
    }
}

/// An event of the programme guide. The row key is `<onid>/<tsid>/<service_id>/<event_id>`.
public struct MpegtsTsGuideRow {
    /// The row key.
    public var key: String
    /// The row key of the service in `services`.
    public var service: String
    /// original_network_id.
    public var onid: Int64
    /// transport_stream_id.
    public var tsid: Int64
    /// service_id.
    public var serviceId: Int64
    /// event_id.
    public var eventId: Int64
    /// The start.
    public var start: Double
    /// The length.
    public var duration: Double
    /// running_status.
    public var runningStatus: Int64
    /// The title.
    public var title: String
    /// The short text.
    public var text: String
    /// The extended text.
    public var extended: String
    /// The ISO 639-2 language code.
    public var lang: String
    /// The first level-1 content nibble. No value for an event without one.
    public var genre: Int64?
    /// The content bytes (up to 8).
    public var content: [UInt8]
    /// The highest minimum age over all countries (0 if none).
    public var minAge: Int64
    /// The programme CRID.
    public var cridProgramme: String
    /// The series CRID.
    public var cridSeries: String
    /// free_CA_mode.
    public var freeCa: Bool
    /// From an EIT of another transport stream.
    public var otherTs: Bool
    /// From a schedule table.
    public var schedule: Bool

    init(_ c: sdrkit_mpegts_ts_guide_row) {
        key = sdrkitText(c.key)
        service = sdrkitText(c.service)
        onid = c.onid
        tsid = c.tsid
        serviceId = c.service_id
        eventId = c.event_id
        start = c.start
        duration = c.duration
        runningStatus = c.running_status
        title = sdrkitText(c.title)
        text = sdrkitText(c.text)
        extended = sdrkitText(c.extended)
        lang = sdrkitText(c.lang)
        genre = c.has_genre ? c.genre : nil
        content = [UInt8](UnsafeBufferPointer(start: c.content.ptr, count: c.content.len))
        minAge = c.min_age
        cridProgramme = sdrkitText(c.crid_programme)
        cridSeries = sdrkitText(c.crid_series)
        freeCa = c.free_ca
        otherTs = c.other_ts
        schedule = c.schedule
    }
}

/// The parental rating of a guide event in one country. The row key is `<guide row>/<country>`.
public struct MpegtsTsGuideRatingRow {
    /// The row key.
    public var key: String
    /// The row key of the guide event in `guide`.
    public var event: String
    /// The ISO 3166 country code.
    public var country: String
    /// The rating (minimum age minus 3).
    public var rating: Int64

    init(_ c: sdrkit_mpegts_ts_guide_rating_row) {
        key = sdrkitText(c.key)
        event = sdrkitText(c.event)
        country = sdrkitText(c.country)
        rating = c.rating
    }
}

/// A PID of the multiplex. The row key is the PID as four hex digits.
public struct MpegtsTsPidRow {
    /// The row key.
    public var key: String
    /// The PID.
    public var pid: Int64
    /// What the PID carries.
    public var label: String
    /// Packets of the PID.
    public var packets: Int64
    /// Continuity errors.
    public var ccErrors: Int64
    /// Packets of the PID with the transport_error_indicator.
    public var tei: Int64
    /// The bit rate of the PID.
    public var kbps: Double
    /// Scrambled.
    public var scrambled: Bool

    init(_ c: sdrkit_mpegts_ts_pid_row) {
        key = sdrkitText(c.key)
        pid = c.pid
        label = sdrkitText(c.label)
        packets = c.packets
        ccErrors = c.cc_errors
        tei = c.tei
        kbps = c.kbps
        scrambled = c.scrambled
    }
}

/// The section counts of one table. The row key is the table name (PAT, PMT, SDT, ...).
public struct MpegtsTsSectionRow {
    /// The row key.
    public var key: String
    /// Sections accepted.
    public var accepted: Int64
    /// Sections with a bad CRC.
    public var crc: Int64
    /// Sections with a bad length.
    public var length: Int64
    /// Sections refused for another reason.
    public var other: Int64

    init(_ c: sdrkit_mpegts_ts_section_row) {
        key = sdrkitText(c.key)
        accepted = c.accepted
        crc = c.crc
        length = c.length
        other = c.other
    }
}

/// A teletext page that arrived.
public struct MpegtsTeletextPageEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The page number.
    public var page: Int64
    /// The subcode, in nibble form.
    public var subcode: Int64
    /// The version, raised on each change.
    public var version: Int64
    /// The text of the page, its 25 rows joined with line feeds.
    public var text: String

    init(_ c: sdrkit_mpegts_teletext_page_event) {
        time = c.time
        freqHz = c.freq_hz
        page = c.page
        subcode = c.subcode
        version = c.version
        text = sdrkitText(c.text)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MpegtsTeletextPageEvent, SDRKitCode> {
        var c = sdrkit_mpegts_teletext_page_event()
        let code = sdrkit_mpegts_teletext_page_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MpegtsTeletextPageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new service list.
public struct MpegtsServiceListEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// transport_stream_id.
    public var tsid: Int64
    /// original_network_id.
    public var onid: Int64
    /// network_id.
    public var networkId: Int64
    /// The network name.
    public var networkName: String
    /// The number of services.
    public var serviceCount: Int64

    init(_ c: sdrkit_mpegts_service_list_event) {
        time = c.time
        freqHz = c.freq_hz
        tsid = c.tsid
        onid = c.onid
        networkId = c.network_id
        networkName = sdrkitText(c.network_name)
        serviceCount = c.service_count
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MpegtsServiceListEvent, SDRKitCode> {
        var c = sdrkit_mpegts_service_list_event()
        let code = sdrkit_mpegts_service_list_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MpegtsServiceListEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new or changed guide event.
public struct MpegtsEpgEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// original_network_id.
    public var onid: Int64
    /// transport_stream_id.
    public var tsid: Int64
    /// service_id.
    public var serviceId: Int64
    /// event_id.
    public var eventId: Int64
    /// The start.
    public var start: Double
    /// The length.
    public var duration: Double
    /// The title.
    public var title: String

    init(_ c: sdrkit_mpegts_epg_event) {
        time = c.time
        freqHz = c.freq_hz
        onid = c.onid
        tsid = c.tsid
        serviceId = c.service_id
        eventId = c.event_id
        start = c.start
        duration = c.duration
        title = sdrkitText(c.title)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MpegtsEpgEvent, SDRKitCode> {
        var c = sdrkit_mpegts_epg_event()
        let code = sdrkit_mpegts_epg_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MpegtsEpgEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new stream time.
public struct MpegtsTsTimeEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The UTC of the stream.
    public var utc: Double
    /// The local time offset in force. No value before the stream gave one.
    public var localOffsetMin: Int64?

    init(_ c: sdrkit_mpegts_ts_time_event) {
        time = c.time
        freqHz = c.freq_hz
        utc = c.utc
        localOffsetMin = c.has_local_offset_min ? c.local_offset_min : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MpegtsTsTimeEvent, SDRKitCode> {
        var c = sdrkit_mpegts_ts_time_event()
        let code = sdrkit_mpegts_ts_time_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsTimeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new network.
public struct MpegtsTsNetworkEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// network_id.
    public var networkId: Int64
    /// The network name.
    public var networkName: String
    /// original_network_id.
    public var onid: Int64
    /// transport_stream_id.
    public var tsid: Int64

    init(_ c: sdrkit_mpegts_ts_network_event) {
        time = c.time
        freqHz = c.freq_hz
        networkId = c.network_id
        networkName = sdrkitText(c.network_name)
        onid = c.onid
        tsid = c.tsid
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MpegtsTsNetworkEvent, SDRKitCode> {
        var c = sdrkit_mpegts_ts_network_event()
        let code = sdrkit_mpegts_ts_network_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsNetworkEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a subtitle display.
public struct MpegtsDvbSubtitleEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The presentation time stamp, 90 kHz. No value for a display without one.
    public var pts: Int64?
    /// How long the display shows at most.
    public var timeoutS: Double
    /// The width of the display.
    public var width: Int64
    /// The height of the display.
    public var height: Int64
    /// The display sequence number.
    public var seq: Int64
    /// The number of regions (0 clears the screen).
    public var regionCount: Int64

    init(_ c: sdrkit_mpegts_dvb_subtitle_event) {
        time = c.time
        freqHz = c.freq_hz
        pts = c.has_pts ? c.pts : nil
        timeoutS = c.timeout_s
        width = c.width
        height = c.height
        seq = c.seq
        regionCount = c.region_count
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MpegtsDvbSubtitleEvent, SDRKitCode> {
        var c = sdrkit_mpegts_dvb_subtitle_event()
        let code = sdrkit_mpegts_dvb_subtitle_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MpegtsDvbSubtitleEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum MpegtsTables {
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum MpegtsCommands {
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum MpegtsSettings {
}
