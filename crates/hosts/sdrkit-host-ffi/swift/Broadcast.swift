// sdrkit: the typed interface of the stack family "broadcast", version 2.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum BroadcastInterface {
    public static let major = 2
    public static let minor = 1
}

/// The results of the DRM stack. No FAC values before the first FAC, no protection before the multiplex description, no time before SDC type 8, no measurement before the first frame and no first audio before it came.
public struct DrmFields {
    public enum RobustnessMode: Int32 {
        case a = 0
        case b = 1
        case c = 2
        case d = 3
        case e = 4
    }
    public enum AudioState: Int32 {
        case noAudioStream = 0
        case noDecoder = 1
        case decoding = 2
    }
    /// The robustness mode, A to E.
    public var robustnessMode: RobustnessMode?
    /// The spectrum occupancy code.
    public var occupancy: Int64?
    /// The channel bandwidth.
    public var bandwidth: Double?
    /// The interleaver as text.
    public var interleaver: String?
    /// The MSC mapping, with the hierarchical flag.
    public var msc: String?
    /// The SDC mapping.
    public var sdc: String?
    /// The protection levels and their code rates.
    public var protection: String?
    /// The protection level of part B.
    public var protectionB: Int64?
    /// The time of SDC type 8 as text.
    public var time: String?
    /// The input level.
    public var levelDbfs: Double?
    /// The SNR.
    public var snrDb: Double?
    /// The carrier offset.
    public var offsetHz: Double?
    /// The sample clock error.
    public var clockPpm: Double?
    /// The delay of the strongest path from the symbol start.
    public var delayMs: Double?
    /// The guard correlation at the symbol timing.
    public var guardCorrelation: Double?
    /// The quality, 0 to 1.
    public var quality: Double?
    /// FAC blocks good.
    public var facOk: Int64
    /// FAC blocks bad.
    public var facBad: Int64
    /// SDC blocks good.
    public var sdcOk: Int64
    /// SDC blocks bad.
    public var sdcBad: Int64
    /// Multiplex frames good.
    public var muxOk: Int64
    /// Multiplex frames bad.
    public var muxBad: Int64
    /// Audio frames good.
    public var audioOk: Int64
    /// Audio frames bad.
    public var audioBad: Int64
    /// Fades held.
    public var fades: Int64
    /// Re-synchronisations.
    public var resyncs: Int64
    /// The audio state.
    public var audioState: AudioState?
    /// What plays, or why nothing can.
    public var audioInfo: String
    /// Text segments good.
    public var textOk: Int64
    /// Text segments bad.
    public var textBad: Int64
    /// Seconds of signal.
    public var signalTime: Double
    /// When the first audio frame came.
    public var firstAudio: Double?
    /// Holding through a fade.
    public var holding: Bool
    /// The input is mirrored (I and Q swapped).
    public var mirrored: Bool
    /// The row key of the service playing in the services table. No value before its first audio frame.
    public var playingService: String?

    init(_ c: sdrkit_drm_fields) {
        robustnessMode = c.has_robustness_mode ? RobustnessMode(rawValue: c.robustness_mode) : nil
        occupancy = c.has_occupancy ? c.occupancy : nil
        bandwidth = c.has_bandwidth ? c.bandwidth : nil
        interleaver = c.has_interleaver ? sdrkitText(c.interleaver) : nil
        msc = c.has_msc ? sdrkitText(c.msc) : nil
        sdc = c.has_sdc ? sdrkitText(c.sdc) : nil
        protection = c.has_protection ? sdrkitText(c.protection) : nil
        protectionB = c.has_protection_b ? c.protection_b : nil
        time = c.has_time ? sdrkitText(c.time) : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        delayMs = c.has_delay_ms ? c.delay_ms : nil
        guardCorrelation = c.has_guard_correlation ? c.guard_correlation : nil
        quality = c.has_quality ? c.quality : nil
        facOk = c.fac_ok
        facBad = c.fac_bad
        sdcOk = c.sdc_ok
        sdcBad = c.sdc_bad
        muxOk = c.mux_ok
        muxBad = c.mux_bad
        audioOk = c.audio_ok
        audioBad = c.audio_bad
        fades = c.fades
        resyncs = c.resyncs
        audioState = AudioState(rawValue: c.audio_state)
        audioInfo = sdrkitText(c.audio_info)
        textOk = c.text_ok
        textBad = c.text_bad
        signalTime = c.signal_time
        firstAudio = c.has_first_audio ? c.first_audio : nil
        holding = c.holding
        mirrored = c.mirrored
        playingService = c.has_playing_service ? sdrkitText(c.playing_service) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DrmFields, SDRKitCode> {
        var c = sdrkit_drm_fields()
        let code = sdrkit_drm_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DrmFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A service. The row key is its short id.
public struct DrmServiceRow {
    /// The row key.
    public var key: String
    /// The short id.
    public var shortId: Int64
    /// The 24-bit service id, six hex digits.
    public var serviceId: String
    /// "audio" or "data".
    public var kind: String
    /// The label, once the SDC gave it.
    public var label: String?
    /// The language.
    public var language: String
    /// The country, once the SDC gave it.
    public var country: String?
    /// The programme type or application id.
    public var programme: String
    /// Conditional access is flagged.
    public var ca: Bool
    /// The audio coding as text, once the SDC gave it.
    public var codec: String?
    /// The core sampling rate, once the SDC gave it.
    public var coreRate: Double?
    /// The audio mode code.
    public var audioMode: Int64
    /// SBR.
    public var sbr: Bool
    /// Text messages carried.
    public var text: Bool
    /// The stream id.
    public var stream: Int64
    /// The net bit rate of the stream.
    public var bitRateBps: Double
    /// The application name of a data service.
    public var application: String?
    /// The mode text of a data service.
    public var modeText: String?
    /// Whether the service is decoded.
    public var decoded: Bool

    init(_ c: sdrkit_drm_service_row) {
        key = sdrkitText(c.key)
        shortId = c.short_id
        serviceId = sdrkitText(c.service_id)
        kind = sdrkitText(c.kind)
        label = c.has_label ? sdrkitText(c.label) : nil
        language = sdrkitText(c.language)
        country = c.has_country ? sdrkitText(c.country) : nil
        programme = sdrkitText(c.programme)
        ca = c.ca
        codec = c.has_codec ? sdrkitText(c.codec) : nil
        coreRate = c.has_core_rate ? c.core_rate : nil
        audioMode = c.audio_mode
        sbr = c.sbr
        text = c.text
        stream = c.stream
        bitRateBps = c.bit_rate_bps
        application = c.has_application ? sdrkitText(c.application) : nil
        modeText = c.has_mode_text ? sdrkitText(c.mode_text) : nil
        decoded = c.decoded
    }
}

/// The bytes per logical frame of a stream. The row key is the stream number.
public struct DrmStreamRow {
    /// The row key.
    public var key: String
    /// The bytes of part A.
    public var partA: Int64
    /// The bytes of part B.
    public var partB: Int64

    init(_ c: sdrkit_drm_stream_row) {
        key = sdrkitText(c.key)
        partA = c.part_a
        partB = c.part_b
    }
}

/// A MOT object of a data service. The row key is `<short id>:<transport id>`.
public struct DrmObjectRow {
    /// The row key.
    public var key: String
    /// The short id of the service.
    public var shortId: Int64
    /// The MOT transport id.
    public var transportId: Int64
    /// The name, or the transport id for an object without one.
    public var name: String
    /// The content type.
    public var contentType: String
    /// The size in bytes.
    public var sizeField: Int64
    /// Times received.
    public var received: Int64
    /// The order of arrival.
    public var order: Int64
    /// What the object is, where the stack can say.
    public var note: String?

    init(_ c: sdrkit_drm_object_row) {
        key = sdrkitText(c.key)
        shortId = c.short_id
        transportId = c.transport_id
        name = sdrkitText(c.name)
        contentType = sdrkitText(c.content_type)
        sizeField = c.size_
        received = c.received
        order = c.order
        note = c.has_note ? sdrkitText(c.note) : nil
    }
}

/// The latest slide of a service. The row key is the short id.
public struct DrmSlideRow {
    /// The row key.
    public var key: String
    /// The short id of the service.
    public var shortId: Int64
    /// The order number of the slide.
    public var order: Int64
    /// The name.
    public var name: String
    /// The category title.
    public var category: String?
    /// The category id.
    public var categoryId: Int64?
    /// The slide id.
    public var slideId: Int64?
    /// The click-through URL.
    public var clickUrl: String?
    /// The alternative location URL.
    public var altUrl: String?
    /// The trigger time as text.
    public var trigger: String?
    /// The content type.
    public var contentType: String
    /// The size in bytes.
    public var sizeField: Int64
    /// The width of the picture.
    public var width: Int64
    /// The height of the picture.
    public var height: Int64

    init(_ c: sdrkit_drm_slide_row) {
        key = sdrkitText(c.key)
        shortId = c.short_id
        order = c.order
        name = sdrkitText(c.name)
        category = c.has_category ? sdrkitText(c.category) : nil
        categoryId = c.has_category_id ? c.category_id : nil
        slideId = c.has_slide_id ? c.slide_id : nil
        clickUrl = c.has_click_url ? sdrkitText(c.click_url) : nil
        altUrl = c.has_alt_url ? sdrkitText(c.alt_url) : nil
        trigger = c.has_trigger ? sdrkitText(c.trigger) : nil
        contentType = sdrkitText(c.content_type)
        sizeField = c.size_
        width = c.width
        height = c.height
    }
}

/// A Journaline object. The row key is its id.
public struct DrmPageRow {
    /// The row key.
    public var key: String
    /// The object id.
    public var id: Int64
    /// The object type.
    public var kind: String
    /// The title.
    public var title: String
    /// The content: links (id and text), body or list items, one per line.
    public var content: String

    init(_ c: sdrkit_drm_page_row) {
        key = sdrkitText(c.key)
        id = c.id
        kind = sdrkitText(c.kind)
        title = sdrkitText(c.title)
        content = sdrkitText(c.content)
    }
}

/// The packet counters of a packet-mode service. The row key is its short id.
public struct DrmPacketRow {
    /// The row key.
    public var key: String
    /// The short id of the service.
    public var shortId: Int64
    /// Packets.
    public var packets: Int64
    /// Packets with a bad CRC.
    public var badCrc: Int64
    /// Packets lost.
    public var lost: Int64
    /// Continuation packets without their first.
    public var orphans: Int64
    /// Padding packets.
    public var empty: Int64
    /// Packets of another packet id.
    public var otherId: Int64
    /// Data units complete.
    public var unitsOk: Int64
    /// Data units dropped.
    public var unitsDropped: Int64
    /// Bytes of the data units.
    public var unitBytes: Int64
    /// TPEG frames counted.
    public var tpegFrames: Int64

    init(_ c: sdrkit_drm_packet_row) {
        key = sdrkitText(c.key)
        shortId = c.short_id
        packets = c.packets
        badCrc = c.bad_crc
        lost = c.lost
        orphans = c.orphans
        empty = c.empty
        otherId = c.other_id
        unitsOk = c.units_ok
        unitsDropped = c.units_dropped
        unitBytes = c.unit_bytes
        tpegFrames = c.tpeg_frames
    }
}

/// A note on what the receiver cannot decode. The row key is its order.
public struct DrmNoteRow {
    /// The row key.
    public var key: String
    /// The note.
    public var note: String

    init(_ c: sdrkit_drm_note_row) {
        key = sdrkitText(c.key)
        note = sdrkitText(c.note)
    }
}

/// The results of the HamDRM stack. No mode, occupancy, constellation, protection, interleaver, label, SNR or offset before the first frame, and no file before its first segment.
public struct HamdrmFields {
    public enum Mode: Int32 {
        case a = 0
        case b = 1
        case e = 2
    }
    public enum Qam: Int32 {
        case n4Qam = 0
        case n16Qam = 1
        case n64Qam = 2
    }
    public enum Interleave: Int32 {
        case short = 0
        case long = 1
    }
    /// Whether the format is loaded, and from what (or why not).
    public var format: String
    /// The robustness mode, A, B or E.
    public var mode: Mode?
    /// The occupancy.
    public var occupancy: Double?
    /// The constellation.
    public var qam: Qam?
    /// The protection level, 0 or 1.
    public var protection: Int64?
    /// The interleaver.
    public var interleave: Interleave?
    /// The label of the FAC (the station's call sign).
    public var label: String?
    /// The SNR of the last frame.
    public var snrDb: Double?
    /// The carrier offset of the last frame.
    public var offsetHz: Double?
    /// Frames good.
    public var framesOk: Int64
    /// Frames bad.
    public var framesBad: Int64
    /// Files completed.
    public var filesCompleted: Int64
    /// The transport id of the file being received.
    public var fileId: Int64?
    /// Its name.
    public var fileName: String?
    /// Its call sign.
    public var fileCall: String?
    /// Its body size.
    public var fileSize: Int64?
    /// Its segments in total, once the last segment number is known.
    public var segmentsTotal: Int64?
    /// Its segments received.
    public var segmentsReceived: Int64?
    /// The transmissions heard.
    public var transmissions: Int64?
    /// Whether it is complete.
    public var done: Bool?

    init(_ c: sdrkit_hamdrm_fields) {
        format = sdrkitText(c.format)
        mode = c.has_mode ? Mode(rawValue: c.mode) : nil
        occupancy = c.has_occupancy ? c.occupancy : nil
        qam = c.has_qam ? Qam(rawValue: c.qam) : nil
        protection = c.has_protection ? c.protection : nil
        interleave = c.has_interleave ? Interleave(rawValue: c.interleave) : nil
        label = c.has_label ? sdrkitText(c.label) : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        filesCompleted = c.files_completed
        fileId = c.has_file_id ? c.file_id : nil
        fileName = c.has_file_name ? sdrkitText(c.file_name) : nil
        fileCall = c.has_file_call ? sdrkitText(c.file_call) : nil
        fileSize = c.has_file_size ? c.file_size : nil
        segmentsTotal = c.has_segments_total ? c.segments_total : nil
        segmentsReceived = c.has_segments_received ? c.segments_received : nil
        transmissions = c.has_transmissions ? c.transmissions : nil
        done = c.has_done ? c.done : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HamdrmFields, SDRKitCode> {
        var c = sdrkit_hamdrm_fields()
        let code = sdrkit_hamdrm_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HamdrmFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A finished file. The row key is its transport id.
public struct HamdrmFileRow {
    /// The row key.
    public var key: String
    /// The transport id.
    public var id: Int64
    /// The name.
    public var name: String
    /// The call sign of the sender.
    public var call: String
    /// The content type.
    public var mime: String
    /// The body size.
    public var sizeField: Int64
    /// The transmissions heard.
    public var transmissions: Int64
    /// When it was complete, in signal time.
    public var completed: Double
    /// The CRC-32 of the body.
    public var crc32: Int64

    init(_ c: sdrkit_hamdrm_file_row) {
        key = sdrkitText(c.key)
        id = c.id
        name = sdrkitText(c.name)
        call = sdrkitText(c.call)
        mime = sdrkitText(c.mime)
        sizeField = c.size_
        transmissions = c.transmissions
        completed = c.completed
        crc32 = c.crc32
    }
}

/// A missing body segment of the file being received. The row key is the segment number.
public struct HamdrmMissingRow {
    /// The row key.
    public var key: String
    /// The number of the segment.
    public var segment: Int64

    init(_ c: sdrkit_hamdrm_missing_row) {
        key = sdrkitText(c.key)
        segment = c.segment
    }
}

/// The results of the HD Radio stack. No band or mode before the first frame, no measurement while the receiver searches and no station data before the PIDS carry it.
public struct HdradioFields {
    public enum Band: Int32 {
        case fmHybrid = 0
        case amHybrid = 1
        case amAllDigital = 2
    }
    public enum ServiceMode: Int32 {
        case mp1 = 0
        case mp3 = 1
        case ma1 = 2
        case ma3 = 3
    }
    /// The kind of signal.
    public var band: Band?
    /// The service mode.
    public var serviceMode: ServiceMode?
    /// The carrier offset.
    public var offsetHz: Double?
    /// The MER of the lower sideband (AM: the primary).
    public var merLowerDb: Double?
    /// The MER of the upper sideband (AM: the secondary).
    public var merUpperDb: Double?
    /// The bit error rate of the P1 channel.
    public var ber: Double?
    /// Blocks.
    public var blocks: Int64
    /// Block syncs.
    public var blockSyncs: Int64
    /// P1 frames good.
    public var p1Ok: Int64
    /// P1 frames bad.
    public var p1Bad: Int64
    /// P3 frames good.
    public var p3Ok: Int64
    /// P3 frames bad.
    public var p3Bad: Int64
    /// PIDS frames good.
    public var pidsOk: Int64
    /// PIDS frames bad.
    public var pidsBad: Int64
    /// Audio PDUs.
    public var pdus: Int64
    /// AAS packets good.
    public var aasOk: Int64
    /// AAS packets bad.
    public var aasBad: Int64
    /// The input level.
    public var levelDbfs: Double?
    /// Signal time.
    public var signalTime: Double
    /// Why the digital sound (HDC) does not play.
    public var codecNote: String?
    /// P1 frames arrive.
    public var p1On: Bool
    /// PIDS frames arrive.
    public var pidsOn: Bool
    /// Song data seen.
    public var psdOn: Bool
    /// Data seen.
    public var dataOn: Bool
    /// The row key of the program to hear in [`PROGRAMS`].
    public var program: String
    /// The call sign.
    public var callSign: String?
    /// The universal name.
    public var universalName: String?
    /// The long name.
    public var longName: String?
    /// The slogan.
    public var slogan: String?
    /// The station message.
    public var message: String?
    /// The country.
    public var country: String?
    /// The facility id.
    public var facilityId: Int64?
    /// The location of the station.
    public var location: SDRKitPosition?
    /// The altitude of the station.
    public var altitudeM: Double?

    init(_ c: sdrkit_hdradio_fields) {
        band = c.has_band ? Band(rawValue: c.band) : nil
        serviceMode = c.has_service_mode ? ServiceMode(rawValue: c.service_mode) : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        merLowerDb = c.has_mer_lower_db ? c.mer_lower_db : nil
        merUpperDb = c.has_mer_upper_db ? c.mer_upper_db : nil
        ber = c.has_ber ? c.ber : nil
        blocks = c.blocks
        blockSyncs = c.block_syncs
        p1Ok = c.p1_ok
        p1Bad = c.p1_bad
        p3Ok = c.p3_ok
        p3Bad = c.p3_bad
        pidsOk = c.pids_ok
        pidsBad = c.pids_bad
        pdus = c.pdus
        aasOk = c.aas_ok
        aasBad = c.aas_bad
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        signalTime = c.signal_time
        codecNote = c.has_codec_note ? sdrkitText(c.codec_note) : nil
        p1On = c.p1_on
        pidsOn = c.pids_on
        psdOn = c.psd_on
        dataOn = c.data_on
        program = sdrkitText(c.program)
        callSign = c.has_call_sign ? sdrkitText(c.call_sign) : nil
        universalName = c.has_universal_name ? sdrkitText(c.universal_name) : nil
        longName = c.has_long_name ? sdrkitText(c.long_name) : nil
        slogan = c.has_slogan ? sdrkitText(c.slogan) : nil
        message = c.has_message ? sdrkitText(c.message) : nil
        country = c.has_country ? sdrkitText(c.country) : nil
        facilityId = c.has_facility_id ? c.facility_id : nil
        location = c.has_location ? SDRKitPosition(c.location) : nil
        altitudeM = c.has_altitude_m ? c.altitude_m : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HdradioFields, SDRKitCode> {
        var c = sdrkit_hdradio_fields()
        let code = sdrkit_hdradio_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HdradioFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// What the HD Radio stack plays, which its sound stage writes.
public struct HdradioHdSoundFields {
    public enum Sound: Int32 {
        case none = 0
        case analogueFm = 1
    }
    /// The sound that plays.
    public var sound: Sound?

    init(_ c: sdrkit_hdradio_hd_sound_fields) {
        sound = Sound(rawValue: c.sound)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HdradioHdSoundFields, SDRKitCode> {
        var c = sdrkit_hdradio_hd_sound_fields()
        let code = sdrkit_hdradio_hd_sound_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HdradioHdSoundFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An audio program. The row key is "HD1" to "HD8".
public struct HdradioProgramRow {
    /// The row key.
    public var key: String
    /// The program number, 0 for HD1.
    public var number: Int64
    /// "HD1" to "HD8".
    public var label: String
    /// The name from the SIG.
    public var name: String?
    /// The program type code.
    public var programType: Int64
    /// The name of the program type.
    public var typeName: String?
    /// Restricted access.
    public var restricted: Bool
    /// The codec mode.
    public var codecMode: Int64?
    /// The blend control.
    public var blend: Int64?
    /// The sound experience code.
    public var soundExperience: Int64
    /// The bit rate.
    public var bitRateBps: Double
    /// Packets good.
    public var packetsOk: Int64
    /// Packets bad.
    public var packetsBad: Int64
    /// Listed in the SIS.
    public var inSis: Bool
    /// Listed in the SIG.
    public var inSig: Bool
    /// On air.
    public var broadcasting: Bool
    /// Song data messages.
    public var psdCount: Int64
    /// The song title.
    public var title: String?
    /// The artist.
    public var artist: String?
    /// The album.
    public var album: String?
    /// The genre.
    public var genre: String?
    /// The comment.
    public var comment: String?
    /// The unique file identifier.
    public var ufid: String?
    /// The commercial frame, its parts separated by "; ".
    public var commercial: String?
    /// The LOT number of the album art.
    public var artLot: Int64?
    /// The type of the album art.
    public var artMime: String?
    /// The port of the station logo.
    public var logoPort: Int64?
    /// The port of the album art.
    public var artPort: Int64?

    init(_ c: sdrkit_hdradio_program_row) {
        key = sdrkitText(c.key)
        number = c.number
        label = sdrkitText(c.label)
        name = c.has_name ? sdrkitText(c.name) : nil
        programType = c.program_type
        typeName = c.has_type_name ? sdrkitText(c.type_name) : nil
        restricted = c.restricted
        codecMode = c.has_codec_mode ? c.codec_mode : nil
        blend = c.has_blend ? c.blend : nil
        soundExperience = c.sound_experience
        bitRateBps = c.bit_rate_bps
        packetsOk = c.packets_ok
        packetsBad = c.packets_bad
        inSis = c.in_sis
        inSig = c.in_sig
        broadcasting = c.broadcasting
        psdCount = c.psd_count
        title = c.has_title ? sdrkitText(c.title) : nil
        artist = c.has_artist ? sdrkitText(c.artist) : nil
        album = c.has_album ? sdrkitText(c.album) : nil
        genre = c.has_genre ? sdrkitText(c.genre) : nil
        comment = c.has_comment ? sdrkitText(c.comment) : nil
        ufid = c.has_ufid ? sdrkitText(c.ufid) : nil
        commercial = c.has_commercial ? sdrkitText(c.commercial) : nil
        artLot = c.has_art_lot ? c.art_lot : nil
        artMime = c.has_art_mime ? sdrkitText(c.art_mime) : nil
        logoPort = c.has_logo_port ? c.logo_port : nil
        artPort = c.has_art_port ? c.art_port : nil
    }
}

/// A data service component. The row key is its port.
public struct HdradioDataServiceRow {
    /// The row key.
    public var key: String
    /// The name of the service.
    public var name: String
    /// The service number.
    public var service: Int64
    /// The port.
    public var port: Int64
    /// The component type.
    public var kind: Int64
    /// The content type.
    public var mime: String
    /// Packets.
    public var packets: Int64
    /// Bytes.
    public var bytes: Int64

    init(_ c: sdrkit_hdradio_data_service_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        service = c.service
        port = c.port
        kind = c.kind
        mime = sdrkitText(c.mime)
        packets = c.packets
        bytes = c.bytes
    }
}

/// A file of the LOT transport. The row key is `<port>:<lot>`.
public struct HdradioFileRow {
    /// The row key.
    public var key: String
    /// The port.
    public var port: Int64
    /// The LOT number.
    public var lot: Int64
    /// The file name.
    public var name: String
    /// The content type.
    public var mime: String
    /// The size in bytes.
    public var sizeField: Int64
    /// Bytes received.
    public var received: Int64
    /// The file is complete.
    public var complete: Bool
    /// The version.
    public var version: Int64
    /// The expiry date.
    public var expiryMjd: Int64
    /// When it last changed, in signal time.
    public var updated: Double

    init(_ c: sdrkit_hdradio_file_row) {
        key = sdrkitText(c.key)
        port = c.port
        lot = c.lot
        name = sdrkitText(c.name)
        mime = sdrkitText(c.mime)
        sizeField = c.size_
        received = c.received
        complete = c.complete
        version = c.version
        expiryMjd = c.expiry_mjd
        updated = c.updated
    }
}

/// The results of the CDR stack. No transmission mode, spectrum or LDPC rate before the system information, no offset or SNR before the first sub-frame, no level before the first report of the decoder, no iterations before the first code word and no network before the network information table.
public struct CdrFields {
    /// Whether the GY/T 268.1 tables are loaded ("loaded"), or what is missing.
    public var tables: String
    /// The transmission mode (1 to 3).
    public var txMode: Int64?
    /// The spectrum mode.
    public var spectrumMode: Int64?
    /// The spectrum mode in words.
    public var spectrum: String?
    /// The LDPC rate of the service data ("3/4").
    public var ldpcRate: String?
    /// The carrier offset.
    public var offsetHz: Double?
    /// The SNR (Es/N0).
    public var snrDb: Double?
    /// The input level.
    public var levelDbfs: Double?
    /// Sub-frames received.
    public var subframes: Int64
    /// System information good.
    public var siOk: Int64
    /// System information bad.
    public var siBad: Int64
    /// LDPC code words good.
    public var cwOk: Int64
    /// LDPC code words bad.
    public var cwBad: Int64
    /// Mean LDPC iterations.
    public var iterations: Double?
    /// Control multiplex frames good.
    public var controlOk: Int64
    /// Control multiplex frames bad.
    public var controlBad: Int64
    /// Service multiplex frames good.
    public var serviceOk: Int64
    /// Service multiplex frames bad.
    public var serviceBad: Int64
    /// The network name.
    public var network: String?
    /// The country of the network.
    public var country: String?
    /// The network id.
    public var networkId: Int64?
    /// The row key of the service playing in [`SERVICES`].
    public var selectedService: String?
    /// DRA+ units of the service listened to.
    public var unitsIn: Int64

    init(_ c: sdrkit_cdr_fields) {
        tables = sdrkitText(c.tables)
        txMode = c.has_tx_mode ? c.tx_mode : nil
        spectrumMode = c.has_spectrum_mode ? c.spectrum_mode : nil
        spectrum = c.has_spectrum ? sdrkitText(c.spectrum) : nil
        ldpcRate = c.has_ldpc_rate ? sdrkitText(c.ldpc_rate) : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        subframes = c.subframes
        siOk = c.si_ok
        siBad = c.si_bad
        cwOk = c.cw_ok
        cwBad = c.cw_bad
        iterations = c.has_iterations ? c.iterations : nil
        controlOk = c.control_ok
        controlBad = c.control_bad
        serviceOk = c.service_ok
        serviceBad = c.service_bad
        network = c.has_network ? sdrkitText(c.network) : nil
        country = c.has_country ? sdrkitText(c.country) : nil
        networkId = c.has_network_id ? c.network_id : nil
        selectedService = c.has_selected_service ? sdrkitText(c.selected_service) : nil
        unitsIn = c.units_in
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CdrFields, SDRKitCode> {
        var c = sdrkit_cdr_fields()
        let code = sdrkit_cdr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CdrFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A service of the multiplex. The row key is the service id in hexadecimal.
public struct CdrServiceRow {
    /// The row key.
    public var key: String
    /// The service id.
    public var sid: Int64
    /// Audio units seen.
    public var audio: Bool
    /// Data units seen.
    public var data: Bool
    /// The sample rate of the first audio stream.
    public var sampleRate: Int64?
    /// Channels of the first audio stream.
    public var channels: Int64?
    /// The language of the first audio stream.
    public var language: String?
    /// The audio algorithm code of the first stream.
    public var algorithm: Int64?
    /// The share of the multiplex.
    public var kbps: Double
    /// Audio units in the last sub-frame.
    public var units: Int64
    /// The last data unit, when it is printable text.
    public var text: String?
    /// Sub-frame headers good.
    public var good: Int64
    /// Sub-frame headers or units bad.
    public var bad: Int64

    init(_ c: sdrkit_cdr_service_row) {
        key = sdrkitText(c.key)
        sid = c.sid
        audio = c.audio
        data = c.data
        sampleRate = c.has_sample_rate ? c.sample_rate : nil
        channels = c.has_channels ? c.channels : nil
        language = c.has_language ? sdrkitText(c.language) : nil
        algorithm = c.has_algorithm ? c.algorithm : nil
        kbps = c.kbps
        units = c.units
        text = c.has_text ? sdrkitText(c.text) : nil
        good = c.good
        bad = c.bad
    }
}

/// A data unit type of a service. The row key is `<service>/<type>`.
public struct CdrDataTypeRow {
    /// The row key.
    public var key: String
    /// The row key of the service.
    public var service: String
    /// The data unit type.
    public var dataType: Int64

    init(_ c: sdrkit_cdr_data_type_row) {
        key = sdrkitText(c.key)
        service = sdrkitText(c.service)
        dataType = c.data_type
    }
}

/// A frequency of the network information table. The row key is its place in the table.
public struct CdrFrequencyRow {
    /// The row key.
    public var key: String
    /// The frequency.
    public var freqHz: Double

    init(_ c: sdrkit_cdr_frequency_row) {
        key = sdrkitText(c.key)
        freqHz = c.freq_hz
    }
}

/// The results of the C-QUAM stack that the stereo matrix writes. No offset, carrier level, C/N or lock quality without a carrier, no pilot level before the first pilot check, no levels before the first audio, no width in mono and no noise floor before the first noise measure.
public struct CquamFields {
    public enum StereoMode: Int32 {
        case auto = 0
        case mono = 1
        case stereo = 2
    }
    /// The carrier offset.
    public var offsetHz: Double?
    /// Carrier to noise in the channel.
    public var cnDb: Double?
    /// The carrier level.
    public var carrierDbfs: Double?
    /// Stereo now.
    public var stereo: Bool
    /// The 25 Hz pilot is seen (also with mono forced).
    public var pilotSeen: Bool
    /// The pilot over its neighbours.
    public var pilotDb: Double?
    /// The pilot as a percentage of the carrier.
    public var pilotPct: Double?
    /// The mean cos φ of the stereo phase (1 for a mono station).
    public var lockQuality: Double?
    /// The left level, rms over 100 ms.
    public var leftDbfs: Double?
    /// The right level, rms over 100 ms.
    public var rightDbfs: Double?
    /// L - R over L + R.
    public var widthDb: Double?
    /// The peak modulation of L + R in the last second.
    public var modPeakPct: Double
    /// The stereo mode setting.
    public var stereoMode: StereoMode?
    /// The audio filter width either side of the carrier.
    public var filterWidth: Double
    /// The noise floor in the audio width.
    public var noiseDbfs: Double?
    /// Pilot checks that found the pilot.
    public var pilotChecksOk: Int64
    /// Pilot checks without it.
    public var pilotChecksBad: Int64
    /// Signal time.
    public var signalTime: Double

    init(_ c: sdrkit_cquam_fields) {
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        cnDb = c.has_cn_db ? c.cn_db : nil
        carrierDbfs = c.has_carrier_dbfs ? c.carrier_dbfs : nil
        stereo = c.stereo
        pilotSeen = c.pilot_seen
        pilotDb = c.has_pilot_db ? c.pilot_db : nil
        pilotPct = c.has_pilot_pct ? c.pilot_pct : nil
        lockQuality = c.has_lock_quality ? c.lock_quality : nil
        leftDbfs = c.has_left_dbfs ? c.left_dbfs : nil
        rightDbfs = c.has_right_dbfs ? c.right_dbfs : nil
        widthDb = c.has_width_db ? c.width_db : nil
        modPeakPct = c.mod_peak_pct
        stereoMode = StereoMode(rawValue: c.stereo_mode)
        filterWidth = c.filter_width
        noiseDbfs = c.has_noise_dbfs ? c.noise_dbfs : nil
        pilotChecksOk = c.pilot_checks_ok
        pilotChecksBad = c.pilot_checks_bad
        signalTime = c.signal_time
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CquamFields, SDRKitCode> {
        var c = sdrkit_cquam_fields()
        let code = sdrkit_cquam_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CquamFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The input rate, which the front end of the C-QUAM stack writes.
public struct CquamFrontFields {
    /// The input rate.
    public var inputRate: Double

    init(_ c: sdrkit_cquam_front_fields) {
        inputRate = c.input_rate
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<CquamFrontFields, SDRKitCode> {
        var c = sdrkit_cquam_front_fields()
        let code = sdrkit_cquam_front_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(CquamFrontFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a new text message.
public struct DrmTextEvent {
    public enum Family: Int32 {
        case aircraft = 0
        case vessel = 1
        case aprs = 2
        case pager = 3
        case sonde = 4
        case broadcast = 5
        case voice = 6
        case message = 7
        case sensor = 8
        case satellite = 9
        case receiver = 10
        case alert = 11
        case other = 12
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text message.
    public var text: String
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_drm_text_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DrmTextEvent, SDRKitCode> {
        var c = sdrkit_drm_text_event()
        let code = sdrkit_drm_text_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DrmTextEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a received MOT object.
public struct DrmObjectEvent {
    public enum Family: Int32 {
        case aircraft = 0
        case vessel = 1
        case aprs = 2
        case pager = 3
        case sonde = 4
        case broadcast = 5
        case voice = 6
        case message = 7
        case sensor = 8
        case satellite = 9
        case receiver = 10
        case alert = 11
        case other = 12
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The MOT transport id.
    public var transportId: Int64
    /// The name of the object.
    public var text: String
    /// The content type.
    public var contentType: String
    /// The size in bytes.
    public var sizeField: Int64
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_drm_object_event) {
        time = c.time
        freqHz = c.freq_hz
        transportId = c.transport_id
        text = sdrkitText(c.text)
        contentType = sdrkitText(c.content_type)
        sizeField = c.size_
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DrmObjectEvent, SDRKitCode> {
        var c = sdrkit_drm_object_event()
        let code = sdrkit_drm_object_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DrmObjectEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the song event.
public struct HdradioHdrPsdEvent {
    public enum Family: Int32 {
        case aircraft = 0
        case vessel = 1
        case aprs = 2
        case pager = 3
        case sonde = 4
        case broadcast = 5
        case voice = 6
        case message = 7
        case sensor = 8
        case satellite = 9
        case receiver = 10
        case alert = 11
        case other = 12
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The program ("HD1").
    public var channel: String
    /// The song title.
    public var title: String
    /// The artist.
    public var artist: String
    /// "artist - title", or the one of them that is there.
    public var text: String
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_hdradio_hdr_psd_event) {
        time = c.time
        freqHz = c.freq_hz
        channel = sdrkitText(c.channel)
        title = sdrkitText(c.title)
        artist = sdrkitText(c.artist)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<HdradioHdrPsdEvent, SDRKitCode> {
        var c = sdrkit_hdradio_hdr_psd_event()
        let code = sdrkit_hdradio_hdr_psd_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(HdradioHdrPsdEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum BroadcastTables {
    /// The changed rows of the table "services" of the stack "drm", newest first, at most `capacity` rows.
    /// The column `label` names a row.
    public static func drmServices(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[DrmServiceRow], SDRKitCode> {
        var rows = [sdrkit_drm_service_row](repeating: sdrkit_drm_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DrmServiceRow.init))
    }
    /// The changed rows of the table "streams" of the stack "drm", newest first, at most `capacity` rows.
    public static func drmStreams(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[DrmStreamRow], SDRKitCode> {
        var rows = [sdrkit_drm_stream_row](repeating: sdrkit_drm_stream_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_streams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DrmStreamRow.init))
    }
    /// The changed rows of the table "objects" of the stack "drm", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func drmObjects(_ handle: SDRKIT_HANDLE, capacity: Int = 96) -> Result<[DrmObjectRow], SDRKitCode> {
        var rows = [sdrkit_drm_object_row](repeating: sdrkit_drm_object_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_objects_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DrmObjectRow.init))
    }
    /// The changed rows of the table "slides" of the stack "drm", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func drmSlides(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[DrmSlideRow], SDRKitCode> {
        var rows = [sdrkit_drm_slide_row](repeating: sdrkit_drm_slide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_slides_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DrmSlideRow.init))
    }
    /// The changed rows of the table "journaline" of the stack "drm", newest first, at most `capacity` rows.
    /// The column `title` names a row.
    public static func drmJournaline(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DrmPageRow], SDRKitCode> {
        var rows = [sdrkit_drm_page_row](repeating: sdrkit_drm_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_journaline_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DrmPageRow.init))
    }
    /// The changed rows of the table "packets" of the stack "drm", newest first, at most `capacity` rows.
    public static func drmPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 4) -> Result<[DrmPacketRow], SDRKitCode> {
        var rows = [sdrkit_drm_packet_row](repeating: sdrkit_drm_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DrmPacketRow.init))
    }
    /// The changed rows of the table "notes" of the stack "drm", newest first, at most `capacity` rows.
    public static func drmNotes(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[DrmNoteRow], SDRKitCode> {
        var rows = [sdrkit_drm_note_row](repeating: sdrkit_drm_note_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_notes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DrmNoteRow.init))
    }
    /// The changed rows of the table "label_tags" of the stack "drm", newest first, at most `capacity` rows.
    public static func drmLabelTags(_ handle: SDRKIT_HANDLE, capacity: Int = 128) -> Result<[BcdataLabelTagRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_label_tag_row](repeating: sdrkit_bcdata_label_tag_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_label_tags_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataLabelTagRow.init))
    }
    /// The changed rows of the table "guide_services" of the stack "drm", newest first, at most `capacity` rows.
    public static func drmGuideServices(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[BcdataGuideServiceRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_guide_service_row](repeating: sdrkit_bcdata_guide_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_guide_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataGuideServiceRow.init))
    }
    /// The changed rows of the table "guide_programmes" of the stack "drm", newest first, at most `capacity` rows.
    public static func drmGuideProgrammes(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[BcdataGuideProgrammeRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_guide_programme_row](repeating: sdrkit_bcdata_guide_programme_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_guide_programmes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataGuideProgrammeRow.init))
    }
    /// The changed rows of the table "guide_groups" of the stack "drm", newest first, at most `capacity` rows.
    public static func drmGuideGroups(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[BcdataGuideGroupRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_guide_group_row](repeating: sdrkit_bcdata_guide_group_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_guide_groups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataGuideGroupRow.init))
    }
    /// The changed rows of the table "slide_categories" of the stack "drm", newest first, at most `capacity` rows.
    public static func drmSlideCategories(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[BcdataSlideCategoryRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_slide_category_row](repeating: sdrkit_bcdata_slide_category_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_slide_categories_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataSlideCategoryRow.init))
    }
    /// The changed rows of the table "logos" of the stack "drm", newest first, at most `capacity` rows.
    public static func drmLogos(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[BcdataLogoRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_logo_row](repeating: sdrkit_bcdata_logo_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_drm_logos_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataLogoRow.init))
    }
    /// The changed rows of the table "history" of the stack "hamdrm", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func hamdrmHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[HamdrmFileRow], SDRKitCode> {
        var rows = [sdrkit_hamdrm_file_row](repeating: sdrkit_hamdrm_file_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hamdrm_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HamdrmFileRow.init))
    }
    /// The changed rows of the table "missing" of the stack "hamdrm", newest first, at most `capacity` rows.
    public static func hamdrmMissing(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[HamdrmMissingRow], SDRKitCode> {
        var rows = [sdrkit_hamdrm_missing_row](repeating: sdrkit_hamdrm_missing_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hamdrm_missing_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HamdrmMissingRow.init))
    }
    /// The changed rows of the table "programs" of the stack "hdradio", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func hdradioPrograms(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[HdradioProgramRow], SDRKitCode> {
        var rows = [sdrkit_hdradio_program_row](repeating: sdrkit_hdradio_program_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hdradio_programs_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HdradioProgramRow.init))
    }
    /// The changed rows of the table "data_services" of the stack "hdradio", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func hdradioDataServices(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[HdradioDataServiceRow], SDRKitCode> {
        var rows = [sdrkit_hdradio_data_service_row](repeating: sdrkit_hdradio_data_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hdradio_data_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HdradioDataServiceRow.init))
    }
    /// The changed rows of the table "files" of the stack "hdradio", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func hdradioFiles(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[HdradioFileRow], SDRKitCode> {
        var rows = [sdrkit_hdradio_file_row](repeating: sdrkit_hdradio_file_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hdradio_files_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HdradioFileRow.init))
    }
    /// The changed rows of the table "label_tags" of the stack "hdradio", newest first, at most `capacity` rows.
    public static func hdradioLabelTags(_ handle: SDRKIT_HANDLE, capacity: Int = 128) -> Result<[BcdataLabelTagRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_label_tag_row](repeating: sdrkit_bcdata_label_tag_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hdradio_label_tags_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataLabelTagRow.init))
    }
    /// The changed rows of the table "guide_services" of the stack "hdradio", newest first, at most `capacity` rows.
    public static func hdradioGuideServices(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[BcdataGuideServiceRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_guide_service_row](repeating: sdrkit_bcdata_guide_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hdradio_guide_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataGuideServiceRow.init))
    }
    /// The changed rows of the table "guide_programmes" of the stack "hdradio", newest first, at most `capacity` rows.
    public static func hdradioGuideProgrammes(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[BcdataGuideProgrammeRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_guide_programme_row](repeating: sdrkit_bcdata_guide_programme_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hdradio_guide_programmes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataGuideProgrammeRow.init))
    }
    /// The changed rows of the table "guide_groups" of the stack "hdradio", newest first, at most `capacity` rows.
    public static func hdradioGuideGroups(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[BcdataGuideGroupRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_guide_group_row](repeating: sdrkit_bcdata_guide_group_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hdradio_guide_groups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataGuideGroupRow.init))
    }
    /// The changed rows of the table "slide_categories" of the stack "hdradio", newest first, at most `capacity` rows.
    public static func hdradioSlideCategories(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[BcdataSlideCategoryRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_slide_category_row](repeating: sdrkit_bcdata_slide_category_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hdradio_slide_categories_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataSlideCategoryRow.init))
    }
    /// The changed rows of the table "logos" of the stack "hdradio", newest first, at most `capacity` rows.
    public static func hdradioLogos(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[BcdataLogoRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_logo_row](repeating: sdrkit_bcdata_logo_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_hdradio_logos_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataLogoRow.init))
    }
    /// The changed rows of the table "services" of the stack "cdr", newest first, at most `capacity` rows.
    public static func cdrServices(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[CdrServiceRow], SDRKitCode> {
        var rows = [sdrkit_cdr_service_row](repeating: sdrkit_cdr_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cdr_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CdrServiceRow.init))
    }
    /// The changed rows of the table "service_data_types" of the stack "cdr", newest first, at most `capacity` rows.
    public static func cdrServiceDataTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[CdrDataTypeRow], SDRKitCode> {
        var rows = [sdrkit_cdr_data_type_row](repeating: sdrkit_cdr_data_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cdr_service_data_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CdrDataTypeRow.init))
    }
    /// The changed rows of the table "network_frequencies" of the stack "cdr", newest first, at most `capacity` rows.
    public static func cdrNetworkFrequencies(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[CdrFrequencyRow], SDRKitCode> {
        var rows = [sdrkit_cdr_frequency_row](repeating: sdrkit_cdr_frequency_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_cdr_network_frequencies_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(CdrFrequencyRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum BroadcastCommands {
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func drmServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_broadcast_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_broadcast_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_drm_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets the signal and starts again.
    public static func drmClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_drm_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets the signal and starts again.
    public static func hamdrmClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hamdrm_clear(handle)
        return sdrkitResult(code)
    }
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func hdradioServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_broadcast_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_broadcast_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_hdradio_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets the signal and starts again.
    public static func hdradioClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_hdradio_clear(handle)
        return sdrkitResult(code)
    }
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func cdrServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_broadcast_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_broadcast_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_cdr_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets the signal and starts again.
    public static func cdrClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cdr_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets the signal and starts again.
    public static func cdrResync(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cdr_resync(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets the signal and starts again.
    public static func cquamClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_cquam_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum BroadcastSettings {
    /// HamDRM format.
    public static func hamdrmFormat(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hamdrm_format", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHamdrmFormat(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hamdrm_format", $0) })
    }
    /// GY/T 268.1 tables.
    public static func cdrTables(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "cdr_tables", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setCdrTables(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "cdr_tables", $0) })
    }
    /// The options of the setting "stereo" of the stack "cquam".
    public enum CquamStereo: Int32 {
        case auto = 0
        case mono = 1
        case stereo = 2
    }
    /// Stereo.
    public static func cquamStereo(_ handle: SDRKIT_HANDLE) -> Result<CquamStereo, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "stereo", &v)
        return code == SDRKIT_OK ? .success(CquamStereo(rawValue: v) ?? CquamStereo(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCquamStereo(_ handle: SDRKIT_HANDLE, _ value: CquamStereo) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "stereo", value.rawValue))
    }
    /// Width.
    public static func cquamWidth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCquamWidth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "width", value))
    }
    /// Squelch (4 dB).
    public static func cquamSquelch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "squelch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCquamSquelch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "squelch", value))
    }
    /// Channel offset.
    public static func cquamChannelOffset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "channel_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCquamChannelOffset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "channel_offset", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum BroadcastGeneratorSettings {
    /// The options of the setting "gen_robustness" of the stack "drm".
    public enum DrmRobustness: Int32 {
        case a = 0
        case b = 1
        case c = 2
        case d = 3
        case e = 4
    }
    /// Robustness mode.
    public static func drmRobustness(_ handle: SDRKIT_HANDLE) -> Result<DrmRobustness, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_robustness", &v)
        return code == SDRKIT_OK ? .success(DrmRobustness(rawValue: v) ?? DrmRobustness(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDrmRobustness(_ handle: SDRKIT_HANDLE, _ value: DrmRobustness) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_robustness", value.rawValue))
    }
    /// The options of the setting "gen_occupancy" of the stack "drm".
    public enum DrmOccupancy: Int32 {
        case usual = 0
        case n45Khz = 1
        case n5Khz = 2
        case n9Khz = 3
        case n10Khz = 4
        case n18Khz = 5
        case n20Khz = 6
    }
    /// Occupancy.
    public static func drmOccupancy(_ handle: SDRKIT_HANDLE) -> Result<DrmOccupancy, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_occupancy", &v)
        return code == SDRKIT_OK ? .success(DrmOccupancy(rawValue: v) ?? DrmOccupancy(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDrmOccupancy(_ handle: SDRKIT_HANDLE, _ value: DrmOccupancy) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_occupancy", value.rawValue))
    }
    /// The options of the setting "gen_msc_qam" of the stack "drm".
    public enum DrmMscQam: Int32 {
        case high = 0
        case low = 1
    }
    /// MSC.
    public static func drmMscQam(_ handle: SDRKIT_HANDLE) -> Result<DrmMscQam, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_msc_qam", &v)
        return code == SDRKIT_OK ? .success(DrmMscQam(rawValue: v) ?? DrmMscQam(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDrmMscQam(_ handle: SDRKIT_HANDLE, _ value: DrmMscQam) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_msc_qam", value.rawValue))
    }
    /// The options of the setting "gen_protection_b" of the stack "drm".
    public enum DrmProtectionB: Int32 {
        case usual = 0
        case n0 = 1
        case n1 = 2
        case n2 = 3
        case n3 = 4
    }
    /// Protection.
    public static func drmProtectionB(_ handle: SDRKIT_HANDLE) -> Result<DrmProtectionB, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_protection_b", &v)
        return code == SDRKIT_OK ? .success(DrmProtectionB(rawValue: v) ?? DrmProtectionB(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDrmProtectionB(_ handle: SDRKIT_HANDLE, _ value: DrmProtectionB) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_protection_b", value.rawValue))
    }
    /// The options of the setting "gen_interleaver" of the stack "drm".
    public enum DrmInterleaver: Int32 {
        case long = 0
        case short = 1
    }
    /// Interleaver.
    public static func drmInterleaver(_ handle: SDRKIT_HANDLE) -> Result<DrmInterleaver, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_interleaver", &v)
        return code == SDRKIT_OK ? .success(DrmInterleaver(rawValue: v) ?? DrmInterleaver(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDrmInterleaver(_ handle: SDRKIT_HANDLE, _ value: DrmInterleaver) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_interleaver", value.rawValue))
    }
    /// The options of the setting "gen_audio" of the stack "drm".
    public enum DrmAudio: Int32 {
        case melody = 0
        case tone = 1
        case silence = 2
        case noise = 3
        case notAac = 4
    }
    /// Audio.
    public static func drmAudio(_ handle: SDRKIT_HANDLE) -> Result<DrmAudio, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_audio", &v)
        return code == SDRKIT_OK ? .success(DrmAudio(rawValue: v) ?? DrmAudio(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDrmAudio(_ handle: SDRKIT_HANDLE, _ value: DrmAudio) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_audio", value.rawValue))
    }
    /// Text message.
    public static func drmTextMessage(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_text_message", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDrmTextMessage(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_text_message", value))
    }
    /// The options of the setting "gen_data" of the stack "drm".
    public enum DrmData: Int32 {
        case none = 0
        case motSlideShow = 1
        case motWebsite = 2
        case tpeg = 3
        case epg = 4
        case journaline = 5
    }
    /// Data service.
    public static func drmData(_ handle: SDRKIT_HANDLE) -> Result<DrmData, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_data", &v)
        return code == SDRKIT_OK ? .success(DrmData(rawValue: v) ?? DrmData(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDrmData(_ handle: SDRKIT_HANDLE, _ value: DrmData) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_data", value.rawValue))
    }
    /// The options of the setting "gen_mode" of the stack "hamdrm".
    public enum HamdrmMode: Int32 {
        case a = 0
        case b = 1
        case e = 2
    }
    /// Mode.
    public static func hamdrmMode(_ handle: SDRKIT_HANDLE) -> Result<HamdrmMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &v)
        return code == SDRKIT_OK ? .success(HamdrmMode(rawValue: v) ?? HamdrmMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHamdrmMode(_ handle: SDRKIT_HANDLE, _ value: HamdrmMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_mode", value.rawValue))
    }
    /// The options of the setting "gen_occupancy" of the stack "hamdrm".
    public enum HamdrmOccupancy: Int32 {
        case n23 = 0
        case n25 = 1
    }
    /// Occupancy.
    public static func hamdrmOccupancy(_ handle: SDRKIT_HANDLE) -> Result<HamdrmOccupancy, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_occupancy", &v)
        return code == SDRKIT_OK ? .success(HamdrmOccupancy(rawValue: v) ?? HamdrmOccupancy(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHamdrmOccupancy(_ handle: SDRKIT_HANDLE, _ value: HamdrmOccupancy) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_occupancy", value.rawValue))
    }
    /// The options of the setting "gen_qam" of the stack "hamdrm".
    public enum HamdrmQam: Int32 {
        case n4Qam = 0
        case n16Qam = 1
        case n64Qam = 2
    }
    /// QAM.
    public static func hamdrmQam(_ handle: SDRKIT_HANDLE) -> Result<HamdrmQam, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_qam", &v)
        return code == SDRKIT_OK ? .success(HamdrmQam(rawValue: v) ?? HamdrmQam(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHamdrmQam(_ handle: SDRKIT_HANDLE, _ value: HamdrmQam) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_qam", value.rawValue))
    }
    /// The options of the setting "gen_protection" of the stack "hamdrm".
    public enum HamdrmProtection: Int32 {
        case n0 = 0
        case n1 = 1
    }
    /// Protection.
    public static func hamdrmProtection(_ handle: SDRKIT_HANDLE) -> Result<HamdrmProtection, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_protection", &v)
        return code == SDRKIT_OK ? .success(HamdrmProtection(rawValue: v) ?? HamdrmProtection(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHamdrmProtection(_ handle: SDRKIT_HANDLE, _ value: HamdrmProtection) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_protection", value.rawValue))
    }
    /// The options of the setting "gen_interleave" of the stack "hamdrm".
    public enum HamdrmInterleave: Int32 {
        case short = 0
        case long = 1
    }
    /// Interleaver.
    public static func hamdrmInterleave(_ handle: SDRKIT_HANDLE) -> Result<HamdrmInterleave, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_interleave", &v)
        return code == SDRKIT_OK ? .success(HamdrmInterleave(rawValue: v) ?? HamdrmInterleave(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHamdrmInterleave(_ handle: SDRKIT_HANDLE, _ value: HamdrmInterleave) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_interleave", value.rawValue))
    }
    /// Leave out every n-th body segment.
    public static func hamdrmSkipEvery(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_skip_every", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHamdrmSkipEvery(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_skip_every", value))
    }
    /// Transmissions per round.
    public static func hamdrmTransmissions(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_transmissions", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHamdrmTransmissions(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_transmissions", value))
    }
    /// The options of the setting "gen_waveform" of the stack "hdradio".
    public enum HdradioWaveform: Int32 {
        case mp1 = 0
        case ma1 = 1
        case mp3 = 2
        case ma3 = 3
    }
    /// Waveform.
    public static func hdradioWaveform(_ handle: SDRKIT_HANDLE) -> Result<HdradioWaveform, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_waveform", &v)
        return code == SDRKIT_OK ? .success(HdradioWaveform(rawValue: v) ?? HdradioWaveform(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHdradioWaveform(_ handle: SDRKIT_HANDLE, _ value: HdradioWaveform) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_waveform", value.rawValue))
    }
    /// Analogue level.
    public static func hdradioAnalogDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_analog_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHdradioAnalogDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_analog_db", value))
    }
    /// The options of the setting "gen_tx_mode" of the stack "cdr".
    public enum CdrTxMode: Int32 {
        case n1 = 0
        case n2 = 1
        case n3 = 2
    }
    /// Transmission mode.
    public static func cdrTxMode(_ handle: SDRKIT_HANDLE) -> Result<CdrTxMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_tx_mode", &v)
        return code == SDRKIT_OK ? .success(CdrTxMode(rawValue: v) ?? CdrTxMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCdrTxMode(_ handle: SDRKIT_HANDLE, _ value: CdrTxMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_tx_mode", value.rawValue))
    }
    /// The options of the setting "gen_spectrum_mode" of the stack "cdr".
    public enum CdrSpectrumMode: Int32 {
        case n1 = 0
        case n2 = 1
        case n9 = 2
        case n10 = 3
        case n22 = 4
        case n23 = 5
    }
    /// Spectrum mode.
    public static func cdrSpectrumMode(_ handle: SDRKIT_HANDLE) -> Result<CdrSpectrumMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_spectrum_mode", &v)
        return code == SDRKIT_OK ? .success(CdrSpectrumMode(rawValue: v) ?? CdrSpectrumMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCdrSpectrumMode(_ handle: SDRKIT_HANDLE, _ value: CdrSpectrumMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_spectrum_mode", value.rawValue))
    }
    /// The options of the setting "gen_data_qam" of the stack "cdr".
    public enum CdrDataQam: Int32 {
        case qpsk = 0
        case n16Qam = 1
        case n64Qam = 2
    }
    /// Data.
    public static func cdrDataQam(_ handle: SDRKIT_HANDLE) -> Result<CdrDataQam, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_data_qam", &v)
        return code == SDRKIT_OK ? .success(CdrDataQam(rawValue: v) ?? CdrDataQam(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCdrDataQam(_ handle: SDRKIT_HANDLE, _ value: CdrDataQam) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_data_qam", value.rawValue))
    }
    /// The options of the setting "gen_ldpc_rate" of the stack "cdr".
    public enum CdrLdpcRate: Int32 {
        case n14 = 0
        case n13 = 1
        case n12 = 2
        case n34 = 3
    }
    /// LDPC rate.
    public static func cdrLdpcRate(_ handle: SDRKIT_HANDLE) -> Result<CdrLdpcRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_ldpc_rate", &v)
        return code == SDRKIT_OK ? .success(CdrLdpcRate(rawValue: v) ?? CdrLdpcRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCdrLdpcRate(_ handle: SDRKIT_HANDLE, _ value: CdrLdpcRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_ldpc_rate", value.rawValue))
    }
    /// The options of the setting "gen_sdi_qam" of the stack "cdr".
    public enum CdrSdiQam: Int32 {
        case qpsk = 0
        case n16Qam = 1
        case n64Qam = 2
    }
    /// SDI.
    public static func cdrSdiQam(_ handle: SDRKIT_HANDLE) -> Result<CdrSdiQam, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_sdi_qam", &v)
        return code == SDRKIT_OK ? .success(CdrSdiQam(rawValue: v) ?? CdrSdiQam(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCdrSdiQam(_ handle: SDRKIT_HANDLE, _ value: CdrSdiQam) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_sdi_qam", value.rawValue))
    }
    /// The options of the setting "gen_allocation" of the stack "cdr".
    public enum CdrAllocation: Int32 {
        case n1 = 0
        case n2 = 1
        case n3 = 2
    }
    /// Allocation.
    public static func cdrAllocation(_ handle: SDRKIT_HANDLE) -> Result<CdrAllocation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_allocation", &v)
        return code == SDRKIT_OK ? .success(CdrAllocation(rawValue: v) ?? CdrAllocation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCdrAllocation(_ handle: SDRKIT_HANDLE, _ value: CdrAllocation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_allocation", value.rawValue))
    }
    /// FM programme.
    public static func cdrFmProgramme(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_fm_programme", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCdrFmProgramme(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_fm_programme", value))
    }
    /// Text service.
    public static func cdrTextService(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_text_service", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCdrTextService(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_text_service", value))
    }
    /// The options of the setting "gen_kind" of the stack "cquam".
    public enum CquamKind: Int32 {
        case stereo = 0
        case am = 1
        case noise = 2
    }
    /// Test signal.
    public static func cquamKind(_ handle: SDRKIT_HANDLE) -> Result<CquamKind, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_kind", &v)
        return code == SDRKIT_OK ? .success(CquamKind(rawValue: v) ?? CquamKind(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setCquamKind(_ handle: SDRKIT_HANDLE, _ value: CquamKind) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_kind", value.rawValue))
    }
    /// Left silent.
    public static func cquamLeftSilent(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_left_silent", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCquamLeftSilent(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_left_silent", value))
    }
    /// Left tone.
    public static func cquamLeftHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_left_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setCquamLeftHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_left_hz", value))
    }
}
