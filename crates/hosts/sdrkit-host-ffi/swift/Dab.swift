// sdrkit: the typed interface of the stack family "dab", version 2.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum DabInterface {
    public static let major = 2
    public static let minor = 0
}

/// The measurements of the signal. No SNR before the lock, and no offset and clock error while the receiver searches.
public struct DabSignalFields {
    /// The SNR measure.
    public var snrDb: Double?
    /// The carrier offset, without the centring shift.
    public var offsetHz: Double?
    /// The sample clock error.
    public var clockPpm: Double?
    /// The input level.
    public var levelDbfs: Double

    init(_ c: sdrkit_dab_signal_fields) {
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabSignalFields, SDRKitCode> {
        var c = sdrkit_dab_signal_fields()
        let code = sdrkit_dab_signal_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabSignalFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The ensemble. No value for an item that the ensemble has not signalled yet.
public struct DabEnsembleFields {
    /// The ensemble label.
    public var ensembleLabel: String?
    /// The ensemble short label.
    public var ensembleShortLabel: String?
    /// The ensemble identifier (EId).
    public var eid: Int64?
    /// The extended country code (FIG 0/9).
    public var ecc: Int64?
    /// The country id (top 4 bits of the EId).
    public var country: Int64?
    /// The ensemble time.
    public var utc: Double?
    /// The local time offset (FIG 0/9).
    public var localTimeOffset: Double?
    /// Every label and the service organisation are received.
    public var ensembleValid: Bool
    /// Services in the ensemble.
    public var serviceCount: Int64

    init(_ c: sdrkit_dab_ensemble_fields) {
        ensembleLabel = c.has_ensemble_label ? sdrkitText(c.ensemble_label) : nil
        ensembleShortLabel = c.has_ensemble_short_label ? sdrkitText(c.ensemble_short_label) : nil
        eid = c.has_eid ? c.eid : nil
        ecc = c.has_ecc ? c.ecc : nil
        country = c.has_country ? c.country : nil
        utc = c.has_utc ? c.utc : nil
        localTimeOffset = c.has_local_time_offset ? c.local_time_offset : nil
        ensembleValid = c.ensemble_valid
        serviceCount = c.service_count
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabEnsembleFields, SDRKitCode> {
        var c = sdrkit_dab_ensemble_fields()
        let code = sdrkit_dab_ensemble_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabEnsembleFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The synchronisation, the FIC and the front end.
public struct DabSyncFields {
    public enum Detection: Int32 {
        case coherent = 0
        case differential = 1
    }
    /// The last synchronisation symbol (PRS) was found.
    public var sync: Bool
    /// The PRS correlation peak over its median. No value while the receiver searches.
    public var prsPeak: Double?
    /// Frames demodulated.
    public var frames: Int64
    /// FIBs with a good CRC.
    public var fibOk: Int64
    /// FIBs with a bad CRC.
    public var fibBad: Int64
    /// Good FIBs in the last frame.
    public var fibOkLastFrame: Int64
    /// The share of the FIC that carries FIGs.
    public var ficFillPct: Double
    /// The quiet time of the null symbol.
    public var quiet: Double
    /// Sync losses.
    public var syncLosses: Int64
    /// Relocks.
    public var relocks: Int64
    /// CIFs erased after lost samples.
    public var erasedCifs: Int64
    /// Impulses blanked.
    public var impulsesBlanked: Int64
    /// The spectrum was mirrored.
    public var mirrored: Bool
    /// Signal time.
    public var signalTime: Double
    /// The detection in use. No value while the receiver searches.
    public var detection: Detection?

    init(_ c: sdrkit_dab_sync_fields) {
        sync = c.sync
        prsPeak = c.has_prs_peak ? c.prs_peak : nil
        frames = c.frames
        fibOk = c.fib_ok
        fibBad = c.fib_bad
        fibOkLastFrame = c.fib_ok_last_frame
        ficFillPct = c.fic_fill_pct
        quiet = c.quiet
        syncLosses = c.sync_losses
        relocks = c.relocks
        erasedCifs = c.erased_cifs
        impulsesBlanked = c.impulses_blanked
        mirrored = c.mirrored
        signalTime = c.signal_time
        detection = c.has_detection ? Detection(rawValue: c.detection) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabSyncFields, SDRKitCode> {
        var c = sdrkit_dab_sync_fields()
        let code = sdrkit_dab_sync_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabSyncFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The service playing. No value while no service plays.
public struct DabPlayingFields {
    public enum SelectedKind: Int32 {
        case dabPlus = 0
        case dab = 1
        case dmb = 2
        case data = 3
    }
    public enum Playing: Int32 {
        case nothing = 0
        case scrambled = 1
        case waiting = 2
        case noAudio = 3
        case decoding = 4
    }
    /// The row key of the service playing in [`SERVICES`].
    public var selectedService: String?
    /// The sub-channel playing.
    public var selectedSubchannel: Int64?
    /// The label of the service playing.
    public var selectedLabel: String?
    /// The kind of the service playing.
    public var selectedKind: SelectedKind?
    /// The bit rate playing.
    public var selectedBitrateKbps: Int64?
    /// The protection playing ("EEP 3-A", "UEP 12").
    public var selectedProtection: String?
    /// What the audio of the service playing does.
    public var playing: Playing?

    init(_ c: sdrkit_dab_playing_fields) {
        selectedService = c.has_selected_service ? sdrkitText(c.selected_service) : nil
        selectedSubchannel = c.has_selected_subchannel ? c.selected_subchannel : nil
        selectedLabel = c.has_selected_label ? sdrkitText(c.selected_label) : nil
        selectedKind = c.has_selected_kind ? SelectedKind(rawValue: c.selected_kind) : nil
        selectedBitrateKbps = c.has_selected_bitrate_kbps ? c.selected_bitrate_kbps : nil
        selectedProtection = c.has_selected_protection ? sdrkitText(c.selected_protection) : nil
        playing = Playing(rawValue: c.playing)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabPlayingFields, SDRKitCode> {
        var c = sdrkit_dab_playing_fields()
        let code = sdrkit_dab_playing_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabPlayingFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The audio of the service playing. No codec, rate, channels, SBR or PS before the audio configuration is known, and no concealed share before the first access unit.
public struct DabAudioFields {
    public enum AudioCodec: Int32 {
        case aac = 0
        case heAac = 1
        case heAacV2 = 2
        case mp2 = 3
    }
    /// The audio codec.
    public var audioCodec: AudioCodec?
    /// The audio sample rate.
    public var audioRate: Double?
    /// Audio channels.
    public var audioChannels: Int64?
    /// SBR in use.
    public var audioSbr: Bool?
    /// Parametric stereo in use.
    public var audioPs: Bool?
    /// Access units in a DAB+ superframe.
    public var audioUnitsPerSuperframe: Int64?
    /// Logical frames of the sub-channel playing.
    public var audioLogicalFrames: Int64
    /// Good access units.
    public var audioUnitsOk: Int64
    /// Bad access units.
    public var audioUnitsBad: Int64
    /// Good superframes.
    public var audioSuperframesOk: Int64
    /// Bad superframes.
    public var audioSuperframesBad: Int64
    /// Reed-Solomon bytes corrected.
    public var audioRsCorrected: Int64
    /// Reed-Solomon words beyond repair.
    public var audioRsUncorrectable: Int64
    /// Layer II frames.
    public var audioMp2Frames: Int64
    /// Bad Layer II frames.
    public var audioMp2Bad: Int64
    /// The share of access units concealed.
    public var audioConcealedPct: Double?

    init(_ c: sdrkit_dab_audio_fields) {
        audioCodec = c.has_audio_codec ? AudioCodec(rawValue: c.audio_codec) : nil
        audioRate = c.has_audio_rate ? c.audio_rate : nil
        audioChannels = c.has_audio_channels ? c.audio_channels : nil
        audioSbr = c.has_audio_sbr ? c.audio_sbr : nil
        audioPs = c.has_audio_ps ? c.audio_ps : nil
        audioUnitsPerSuperframe = c.has_audio_units_per_superframe ? c.audio_units_per_superframe : nil
        audioLogicalFrames = c.audio_logical_frames
        audioUnitsOk = c.audio_units_ok
        audioUnitsBad = c.audio_units_bad
        audioSuperframesOk = c.audio_superframes_ok
        audioSuperframesBad = c.audio_superframes_bad
        audioRsCorrected = c.audio_rs_corrected
        audioRsUncorrectable = c.audio_rs_uncorrectable
        audioMp2Frames = c.audio_mp2_frames
        audioMp2Bad = c.audio_mp2_bad
        audioConcealedPct = c.has_audio_concealed_pct ? c.audio_concealed_pct : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabAudioFields, SDRKitCode> {
        var c = sdrkit_dab_audio_fields()
        let code = sdrkit_dab_audio_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabAudioFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The dynamic label and the slide of the service playing. No value before the first label or slide.
public struct DabPadFields {
    /// The character set code of the dynamic label (TS 101 756 table 1).
    public var labelCharset: Int64?
    /// The slide number: a new number is a new picture.
    public var slideSequence: Int64?
    /// The slide width.
    public var slideWidth: Int64?
    /// The slide height.
    public var slideHeight: Int64?
    /// The slide size.
    public var slideBytes: Int64?

    init(_ c: sdrkit_dab_pad_fields) {
        labelCharset = c.has_label_charset ? c.label_charset : nil
        slideSequence = c.has_slide_sequence ? c.slide_sequence : nil
        slideWidth = c.has_slide_width ? c.slide_width : nil
        slideHeight = c.has_slide_height ? c.slide_height : nil
        slideBytes = c.has_slide_bytes ? c.slide_bytes : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabPadFields, SDRKitCode> {
        var c = sdrkit_dab_pad_fields()
        let code = sdrkit_dab_pad_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabPadFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// When things were first reported, after the start. No value before it happened.
public struct DabTimingFields {
    /// Lock.
    public var timingLock: Double?
    /// The first good FIB.
    public var timingFirstFib: Double?
    /// The ensemble complete.
    public var timingEnsembleValid: Double?
    /// Every label known.
    public var timingAllLabels: Double?
    /// The first audio of the service playing.
    public var timingFirstAccessUnit: Double?
    /// The first label.
    public var timingLabel: Double?
    /// The first slide.
    public var timingSlide: Double?

    init(_ c: sdrkit_dab_timing_fields) {
        timingLock = c.has_timing_lock ? c.timing_lock : nil
        timingFirstFib = c.has_timing_first_fib ? c.timing_first_fib : nil
        timingEnsembleValid = c.has_timing_ensemble_valid ? c.timing_ensemble_valid : nil
        timingAllLabels = c.has_timing_all_labels ? c.timing_all_labels : nil
        timingFirstAccessUnit = c.has_timing_first_access_unit ? c.timing_first_access_unit : nil
        timingLabel = c.has_timing_label ? c.timing_label : nil
        timingSlide = c.has_timing_slide ? c.timing_slide : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabTimingFields, SDRKitCode> {
        var c = sdrkit_dab_timing_fields()
        let code = sdrkit_dab_timing_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabTimingFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The announcement that applies to the service playing. No value while none applies.
public struct DabAnnouncementFields {
    /// The cluster of the announcement.
    public var announcementCluster: Int64?
    /// The type of the announcement (TS 101 756 table 14: 0 alarm, 1 road traffic ...).
    public var announcementType: Int64?
    /// The name of that type.
    public var announcementName: String?
    /// The sub-channel of the announcement.
    public var announcementSubchannel: Int64?

    init(_ c: sdrkit_dab_announcement_fields) {
        announcementCluster = c.has_announcement_cluster ? c.announcement_cluster : nil
        announcementType = c.has_announcement_type ? c.announcement_type : nil
        announcementName = c.has_announcement_name ? sdrkitText(c.announcement_name) : nil
        announcementSubchannel = c.has_announcement_subchannel ? c.announcement_subchannel : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabAnnouncementFields, SDRKitCode> {
        var c = sdrkit_dab_announcement_fields()
        let code = sdrkit_dab_announcement_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabAnnouncementFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The DMB video sub-channel. No value before the DMB decoder reports, and no size before a sequence parameter set.
public struct DabDmbFields {
    /// DMB transport stream packets.
    public var dmbTsPackets: Int64?
    /// DMB Reed-Solomon words corrected.
    public var dmbRsCorrected: Int64?
    /// DMB Reed-Solomon words beyond repair.
    public var dmbRsFailed: Int64?
    /// DMB video access units.
    public var dmbVideoUnits: Int64?
    /// DMB audio access units.
    public var dmbAudioUnits: Int64?
    /// The width of the DMB video, from its sequence parameter set.
    public var dmbVideoWidth: Int64?
    /// The height of the DMB video.
    public var dmbVideoHeight: Int64?

    init(_ c: sdrkit_dab_dmb_fields) {
        dmbTsPackets = c.has_dmb_ts_packets ? c.dmb_ts_packets : nil
        dmbRsCorrected = c.has_dmb_rs_corrected ? c.dmb_rs_corrected : nil
        dmbRsFailed = c.has_dmb_rs_failed ? c.dmb_rs_failed : nil
        dmbVideoUnits = c.has_dmb_video_units ? c.dmb_video_units : nil
        dmbAudioUnits = c.has_dmb_audio_units ? c.dmb_audio_units : nil
        dmbVideoWidth = c.has_dmb_video_width ? c.dmb_video_width : nil
        dmbVideoHeight = c.has_dmb_video_height ? c.dmb_video_height : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabDmbFields, SDRKitCode> {
        var c = sdrkit_dab_dmb_fields()
        let code = sdrkit_dab_dmb_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabDmbFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The counts of the data decoders. No value before a decoder reports.
public struct DabDataFields {
    /// FIG records kept for the analyser.
    public var ficRecords: Int64
    /// Guide objects decoded.
    public var spiObjects: Int64?
    /// Journaline objects received.
    public var journalineReceived: Int64?
    /// Null symbols analysed for transmitter identification.
    public var tiiAnalysed: Int64?

    init(_ c: sdrkit_dab_data_fields) {
        ficRecords = c.fic_records
        spiObjects = c.has_spi_objects ? c.spi_objects : nil
        journalineReceived = c.has_journaline_received ? c.journaline_received : nil
        tiiAnalysed = c.has_tii_analysed ? c.tii_analysed : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabDataFields, SDRKitCode> {
        var c = sdrkit_dab_data_fields()
        let code = sdrkit_dab_data_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabDataFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The health indicators and the signal quality.
public struct DabHealthFields {
    public enum HealthSync: Int32 {
        case off = 0
        case amber = 1
        case green = 2
    }
    public enum HealthPrs: Int32 {
        case off = 0
        case amber = 1
        case green = 2
    }
    public enum HealthFic: Int32 {
        case off = 0
        case amber = 1
        case green = 2
    }
    public enum HealthEnsemble: Int32 {
        case off = 0
        case amber = 1
        case green = 2
    }
    public enum HealthMsc: Int32 {
        case off = 0
        case amber = 1
        case green = 2
    }
    public enum HealthAudio: Int32 {
        case off = 0
        case amber = 1
        case green = 2
    }
    public enum HealthConcealed: Int32 {
        case off = 0
        case amber = 1
        case green = 2
    }
    public enum QualityText: Int32 {
        case poor = 0
        case fair = 1
        case good = 2
        case excellent = 3
    }
    /// The receiver is locked.
    public var healthSync: HealthSync?
    /// The synchronisation symbol peak.
    public var healthPrs: HealthPrs?
    /// The FIBs of the last frame.
    public var healthFic: HealthFic?
    /// The ensemble.
    public var healthEnsemble: HealthEnsemble?
    /// The sub-channel playing.
    public var healthMsc: HealthMsc?
    /// The audio.
    public var healthAudio: HealthAudio?
    /// The concealment.
    public var healthConcealed: HealthConcealed?
    /// The signal quality, 0 to 1: (SNR − 4 dB) / 18 dB. No value before the lock.
    public var quality: Double?
    /// The signal quality in words. No value before the lock.
    public var qualityText: QualityText?

    init(_ c: sdrkit_dab_health_fields) {
        healthSync = HealthSync(rawValue: c.health_sync)
        healthPrs = HealthPrs(rawValue: c.health_prs)
        healthFic = HealthFic(rawValue: c.health_fic)
        healthEnsemble = HealthEnsemble(rawValue: c.health_ensemble)
        healthMsc = HealthMsc(rawValue: c.health_msc)
        healthAudio = HealthAudio(rawValue: c.health_audio)
        healthConcealed = HealthConcealed(rawValue: c.health_concealed)
        quality = c.has_quality ? c.quality : nil
        qualityText = c.has_quality_text ? QualityText(rawValue: c.quality_text) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DabHealthFields, SDRKitCode> {
        var c = sdrkit_dab_health_fields()
        let code = sdrkit_dab_health_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DabHealthFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A service of the ensemble. The row key is the SId in hexadecimal: 4 digits, 8 for a data service.
public struct DabServiceRow {
    public enum Kind: Int32 {
        case dabPlus = 0
        case dab = 1
        case dmb = 2
        case data = 3
    }
    /// The row key.
    public var key: String
    /// The service identifier (SId).
    public var sid: Int64
    /// The label.
    public var label: String?
    /// The short label.
    public var shortLabel: String?
    /// What the service carries.
    public var kind: Kind?
    /// The sub-channel of the primary component.
    public var subchannel: Int64?
    /// The bit rate of that sub-channel.
    public var bitrateKbps: Int64?
    /// The protection of that sub-channel ("EEP 3-A", "UEP 12").
    public var protection: String?
    /// The ASCTy of the audio component, or the DSCTy of a data component.
    public var componentType: Int64?
    /// The programme type code.
    public var pty: Int64?
    /// The name of the programme type.
    public var ptyName: String?
    /// The programme type is static.
    public var ptyStatic: Bool
    /// The language code (TS 101 756 tables 9 and 10).
    public var language: Int64?
    /// The name of the language.
    public var languageName: String?
    /// Scrambled (conditional access).
    public var scrambled: Bool
    /// The announcement support flags (ASu).
    public var announcementSupport: Int64
    /// The dynamic label of the service, while it plays.
    public var nowPlaying: String?
    /// The service plays.
    public var playing: Bool

    init(_ c: sdrkit_dab_service_row) {
        key = sdrkitText(c.key)
        sid = c.sid
        label = c.has_label ? sdrkitText(c.label) : nil
        shortLabel = c.has_short_label ? sdrkitText(c.short_label) : nil
        kind = Kind(rawValue: c.kind)
        subchannel = c.has_subchannel ? c.subchannel : nil
        bitrateKbps = c.has_bitrate_kbps ? c.bitrate_kbps : nil
        protection = c.has_protection ? sdrkitText(c.protection) : nil
        componentType = c.has_component_type ? c.component_type : nil
        pty = c.has_pty ? c.pty : nil
        ptyName = c.has_pty_name ? sdrkitText(c.pty_name) : nil
        ptyStatic = c.pty_static
        language = c.has_language ? c.language : nil
        languageName = c.has_language_name ? sdrkitText(c.language_name) : nil
        scrambled = c.scrambled
        announcementSupport = c.announcement_support
        nowPlaying = c.has_now_playing ? sdrkitText(c.now_playing) : nil
        playing = c.playing
    }
}

/// A sub-channel. The row key is its id.
public struct DabSubchannelRow {
    /// The row key.
    public var key: String
    /// The sub-channel id.
    public var id: Int64
    /// The first capacity unit.
    public var startCu: Int64
    /// The size in capacity units.
    public var sizeCu: Int64
    /// The protection ("EEP 3-A", "UEP 12").
    public var protection: String?
    /// The bit rate.
    public var bitrateKbps: Int64

    init(_ c: sdrkit_dab_subchannel_row) {
        key = sdrkitText(c.key)
        id = c.id
        startCu = c.start_cu
        sizeCu = c.size_cu
        protection = c.has_protection ? sdrkitText(c.protection) : nil
        bitrateKbps = c.bitrate_kbps
    }
}

/// A service component. The row key is `<service>/<n>`; n counts from 0 in the order of FIG 0/2 and FIG 0/8.
public struct DabServiceComponentRow {
    /// The row key.
    public var key: String
    /// The row key of the service.
    public var service: String
    /// The transport mechanism (TMId: 0 audio stream, 1 data stream, 3 packet data).
    public var tmid: Int64
    /// The ASCTy of an audio component, or the DSCTy of a data component.
    public var componentType: Int64
    /// The sub-channel.
    public var subchannel: Int64?
    /// The packet address.
    public var packetAddress: Int64?
    /// The service component identifier within the service (SCIdS).
    public var scids: Int64?
    /// The primary component.
    public var primary: Bool
    /// Scrambled (conditional access).
    public var scrambled: Bool
    /// The component label.
    public var label: String?

    init(_ c: sdrkit_dab_service_component_row) {
        key = sdrkitText(c.key)
        service = sdrkitText(c.service)
        tmid = c.tmid
        componentType = c.component_type
        subchannel = c.has_subchannel ? c.subchannel : nil
        packetAddress = c.has_packet_address ? c.packet_address : nil
        scids = c.has_scids ? c.scids : nil
        primary = c.primary
        scrambled = c.scrambled
        label = c.has_label ? sdrkitText(c.label) : nil
    }
}

/// A user application of a service (FIG 0/13). The row key is `<service>/<SCIdS>/<type>`.
public struct DabUserApplicationRow {
    /// The row key.
    public var key: String
    /// The row key of the service.
    public var service: String
    /// The service component identifier within the service (SCIdS).
    public var scids: Int64
    /// The user application type (TS 101 756 table 16).
    public var appType: Int64
    /// The name of the type.
    public var name: String

    init(_ c: sdrkit_dab_user_application_row) {
        key = sdrkitText(c.key)
        service = sdrkitText(c.service)
        scids = c.scids
        appType = c.app_type
        name = sdrkitText(c.name)
    }
}

/// An announcement cluster of a service (FIG 0/18). The row key is `<service>/<cluster>`.
public struct DabServiceClusterRow {
    /// The row key.
    public var key: String
    /// The row key of the service.
    public var service: String
    /// The announcement cluster.
    public var cluster: Int64

    init(_ c: sdrkit_dab_service_cluster_row) {
        key = sdrkitText(c.key)
        service = sdrkitText(c.service)
        cluster = c.cluster
    }
}

/// A transmitter of the database of FIG 0/22. The row key is `<main>.<sub>`.
public struct DabTiiEntryRow {
    /// The row key.
    public var key: String
    /// The transmitter main identifier.
    public var main: Int64
    /// The transmitter sub-identifier.
    public var sub: Int64
    /// The position.
    public var position: SDRKitPosition
    /// The delay.
    public var delayUs: Int64

    init(_ c: sdrkit_dab_tii_entry_row) {
        key = sdrkitText(c.key)
        main = c.main
        sub = c.sub
        position = SDRKitPosition(c.position)
        delayUs = c.delay_us
    }
}

/// A frequency where the service playing can also be received. The row key is the frequency in Hz.
public struct DabFollowingRow {
    /// The row key.
    public var key: String
    /// The frequency.
    public var freq: Double
    /// The channel name.
    public var channel: String
    /// The ensemble identifier.
    public var eid: Int64

    init(_ c: sdrkit_dab_following_row) {
        key = sdrkitText(c.key)
        freq = c.freq
        channel = sdrkitText(c.channel)
        eid = c.eid
    }
}

/// An announcement running. The row key is its cluster.
public struct DabAnnouncementRow {
    /// The row key.
    public var key: String
    /// The announcement cluster.
    public var cluster: Int64
    /// The announcement switching flags (ASw).
    public var asw: Int64
    /// The sub-channel of the announcement.
    public var subchannel: Int64

    init(_ c: sdrkit_dab_announcement_row) {
        key = sdrkitText(c.key)
        cluster = c.cluster
        asw = c.asw
        subchannel = c.subchannel
    }
}

/// A transmitter identified. The row key is `<main>.<sub>`.
public struct DabTransmitterRow {
    /// The row key.
    public var key: String
    /// The transmitter main identifier.
    public var main: Int64
    /// The transmitter sub-identifier.
    public var sub: Int64
    /// The level relative to the strongest transmitter.
    public var levelDb: Double
    /// The position, from the transmitter database.
    public var position: SDRKitPosition?
    /// The delay, from the transmitter database.
    public var delayUs: Int64?

    init(_ c: sdrkit_dab_transmitter_row) {
        key = sdrkitText(c.key)
        main = c.main
        sub = c.sub
        levelDb = c.level_db
        position = c.has_position ? SDRKitPosition(c.position) : nil
        delayUs = c.has_delay_us ? c.delay_us : nil
    }
}

/// A Journaline object. The row key is its id in hexadecimal.
public struct DabJournalineRow {
    public enum Kind: Int32 {
        case menu = 0
        case text = 1
        case title = 2
        case list = 3
    }
    /// The row key.
    public var key: String
    /// The object id.
    public var id: Int64
    /// The kind of the object.
    public var kind: Kind?
    /// The title.
    public var title: String
    /// The body text.
    public var body: String?

    init(_ c: sdrkit_dab_journaline_row) {
        key = sdrkitText(c.key)
        id = c.id
        kind = Kind(rawValue: c.kind)
        title = sdrkitText(c.title)
        body = c.has_body ? sdrkitText(c.body) : nil
    }
}

/// A menu entry of a Journaline object. The row key is `<object>/<n>`.
public struct DabJournalineLinkRow {
    /// The row key.
    public var key: String
    /// The row key of the object.
    public var object: String
    /// The object the entry leads to.
    public var target: Int64
    /// The text of the entry.
    public var text: String

    init(_ c: sdrkit_dab_journaline_link_row) {
        key = sdrkitText(c.key)
        object = sdrkitText(c.object)
        target = c.target
        text = sdrkitText(c.text)
    }
}

/// A list item of a Journaline object. The row key is `<object>/<n>`. Columns are separated by a tab.
public struct DabJournalineItemRow {
    /// The row key.
    public var key: String
    /// The row key of the object.
    public var object: String
    /// The text of the item.
    public var text: String

    init(_ c: sdrkit_dab_journaline_item_row) {
        key = sdrkitText(c.key)
        object = sdrkitText(c.object)
        text = sdrkitText(c.text)
    }
}

/// The statistics of a FIG type. The row key is the FIG type ("0/2").
public struct DabFigTypeRow {
    /// The row key.
    public var key: String
    /// The FIG type.
    public var fig: String
    /// FIGs received.
    public var count: Int64
    /// Bytes received.
    public var bytes: Int64

    init(_ c: sdrkit_dab_fig_type_row) {
        key = sdrkitText(c.key)
        fig = sdrkitText(c.fig)
        count = c.count
        bytes = c.bytes
    }
}

/// The fields of the event of a complete ensemble.
public struct DabEnsembleEvent {
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
    /// The EId in hexadecimal.
    public var source: String?
    /// The ensemble label.
    public var text: String
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_dab_ensemble_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DabEnsembleEvent, SDRKitCode> {
        var c = sdrkit_dab_ensemble_event()
        let code = sdrkit_dab_ensemble_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DabEnsembleEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a changed service list.
public struct DabServicesEvent {
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
    /// The EId in hexadecimal.
    public var source: String?
    /// The number of services.
    public var count: Int64
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_dab_services_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        count = c.count
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DabServicesEvent, SDRKitCode> {
        var c = sdrkit_dab_services_event()
        let code = sdrkit_dab_services_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DabServicesEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a started announcement.
public struct DabAnnouncementStartedEvent {
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
    }
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
    /// The EId in hexadecimal.
    public var source: String?
    /// The announcement cluster.
    public var cluster: Int64
    /// The announcement switching flags (ASw).
    public var asw: Int64
    /// The sub-channel of the announcement.
    public var subchannel: Int64
    /// The name of the first type of the flags.
    public var text: String
    /// Urgent for an alarm announcement (type 0), else a notice.
    public var urgency: Urgency?
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_dab_announcement_started_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        cluster = c.cluster
        asw = c.asw
        subchannel = c.subchannel
        text = sdrkitText(c.text)
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DabAnnouncementStartedEvent, SDRKitCode> {
        var c = sdrkit_dab_announcement_started_event()
        let code = sdrkit_dab_announcement_started_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DabAnnouncementStartedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of an ended announcement.
public struct DabAnnouncementEndedEvent {
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
    /// The EId in hexadecimal.
    public var source: String?
    /// The announcement cluster.
    public var cluster: Int64
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_dab_announcement_ended_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        cluster = c.cluster
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DabAnnouncementEndedEvent, SDRKitCode> {
        var c = sdrkit_dab_announcement_ended_event()
        let code = sdrkit_dab_announcement_ended_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DabAnnouncementEndedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a lost lock.
public struct DabSyncLostEvent {
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
    /// The family of the sender: the receiver.
    public var family: Family?

    init(_ c: sdrkit_dab_sync_lost_event) {
        time = c.time
        freqHz = c.freq_hz
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DabSyncLostEvent, SDRKitCode> {
        var c = sdrkit_dab_sync_lost_event()
        let code = sdrkit_dab_sync_lost_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DabSyncLostEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a new dynamic label.
public struct DabLabelEvent {
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
    /// The SId of the service playing, in hexadecimal.
    public var source: String?
    /// The label.
    public var text: String
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_dab_label_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DabLabelEvent, SDRKitCode> {
        var c = sdrkit_dab_label_event()
        let code = sdrkit_dab_label_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DabLabelEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event of a new slide.
public struct DabSlideEvent {
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
    /// The SId of the service playing, in hexadecimal.
    public var source: String?
    /// The slide number.
    public var slideSequence: Int64
    /// The name of the slide.
    public var text: String?
    /// The family of the sender: broadcast.
    public var family: Family?

    init(_ c: sdrkit_dab_slide_event) {
        time = c.time
        freqHz = c.freq_hz
        source = c.has_source ? sdrkitText(c.source) : nil
        slideSequence = c.slide_sequence
        text = c.has_text ? sdrkitText(c.text) : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DabSlideEvent, SDRKitCode> {
        var c = sdrkit_dab_slide_event()
        let code = sdrkit_dab_slide_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DabSlideEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum DabComponents {
    /// The result fields of the component "playout" of the stack "dab" in the snapshot of `handle`.
    public static func dabPlayoutFields(_ handle: SDRKIT_HANDLE) -> Result<BcdataPlayoutFields, SDRKitCode> {
        var c = sdrkit_bcdata_playout_fields()
        let code = sdrkit_dab_playout_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BcdataPlayoutFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum DabTables {
    /// The changed rows of the table "services" of the stack "dab", newest first, at most `capacity` rows.
    /// The column `label` names a row.
    public static func dabServices(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[DabServiceRow], SDRKitCode> {
        var rows = [sdrkit_dab_service_row](repeating: sdrkit_dab_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabServiceRow.init))
    }
    /// The changed rows of the table "subchannels" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabSubchannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[DabSubchannelRow], SDRKitCode> {
        var rows = [sdrkit_dab_subchannel_row](repeating: sdrkit_dab_subchannel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_subchannels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabSubchannelRow.init))
    }
    /// The changed rows of the table "service_components" of the stack "dab", newest first, at most `capacity` rows.
    /// The column `label` names a row.
    public static func dabServiceComponents(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DabServiceComponentRow], SDRKitCode> {
        var rows = [sdrkit_dab_service_component_row](repeating: sdrkit_dab_service_component_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_service_components_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabServiceComponentRow.init))
    }
    /// The changed rows of the table "user_applications" of the stack "dab", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func dabUserApplications(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DabUserApplicationRow], SDRKitCode> {
        var rows = [sdrkit_dab_user_application_row](repeating: sdrkit_dab_user_application_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_user_applications_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabUserApplicationRow.init))
    }
    /// The changed rows of the table "service_clusters" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabServiceClusters(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DabServiceClusterRow], SDRKitCode> {
        var rows = [sdrkit_dab_service_cluster_row](repeating: sdrkit_dab_service_cluster_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_service_clusters_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabServiceClusterRow.init))
    }
    /// The changed rows of the table "tii_database" of the stack "dab", newest first, at most `capacity` rows.
    /// Place of a row: the column `position`.
    public static func dabTiiDatabase(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[DabTiiEntryRow], SDRKitCode> {
        var rows = [sdrkit_dab_tii_entry_row](repeating: sdrkit_dab_tii_entry_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_tii_database_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabTiiEntryRow.init))
    }
    /// The changed rows of the table "following" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabFollowing(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[DabFollowingRow], SDRKitCode> {
        var rows = [sdrkit_dab_following_row](repeating: sdrkit_dab_following_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_following_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabFollowingRow.init))
    }
    /// The changed rows of the table "announcements" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabAnnouncements(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[DabAnnouncementRow], SDRKitCode> {
        var rows = [sdrkit_dab_announcement_row](repeating: sdrkit_dab_announcement_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_announcements_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabAnnouncementRow.init))
    }
    /// The changed rows of the table "transmitters" of the stack "dab", newest first, at most `capacity` rows.
    /// Place of a row: the column `position`.
    public static func dabTransmitters(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[DabTransmitterRow], SDRKitCode> {
        var rows = [sdrkit_dab_transmitter_row](repeating: sdrkit_dab_transmitter_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_transmitters_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabTransmitterRow.init))
    }
    /// The changed rows of the table "journaline" of the stack "dab", newest first, at most `capacity` rows.
    /// The column `title` names a row.
    public static func dabJournaline(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DabJournalineRow], SDRKitCode> {
        var rows = [sdrkit_dab_journaline_row](repeating: sdrkit_dab_journaline_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_journaline_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabJournalineRow.init))
    }
    /// The changed rows of the table "journaline_links" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabJournalineLinks(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[DabJournalineLinkRow], SDRKitCode> {
        var rows = [sdrkit_dab_journaline_link_row](repeating: sdrkit_dab_journaline_link_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_journaline_links_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabJournalineLinkRow.init))
    }
    /// The changed rows of the table "journaline_items" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabJournalineItems(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[DabJournalineItemRow], SDRKitCode> {
        var rows = [sdrkit_dab_journaline_item_row](repeating: sdrkit_dab_journaline_item_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_journaline_items_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabJournalineItemRow.init))
    }
    /// The changed rows of the table "fig_types" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabFigTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[DabFigTypeRow], SDRKitCode> {
        var rows = [sdrkit_dab_fig_type_row](repeating: sdrkit_dab_fig_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_fig_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DabFigTypeRow.init))
    }
    /// The changed rows of the table "label_tags" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabLabelTags(_ handle: SDRKIT_HANDLE, capacity: Int = 128) -> Result<[BcdataLabelTagRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_label_tag_row](repeating: sdrkit_bcdata_label_tag_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_label_tags_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataLabelTagRow.init))
    }
    /// The changed rows of the table "guide_services" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabGuideServices(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[BcdataGuideServiceRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_guide_service_row](repeating: sdrkit_bcdata_guide_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_guide_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataGuideServiceRow.init))
    }
    /// The changed rows of the table "guide_programmes" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabGuideProgrammes(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[BcdataGuideProgrammeRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_guide_programme_row](repeating: sdrkit_bcdata_guide_programme_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_guide_programmes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataGuideProgrammeRow.init))
    }
    /// The changed rows of the table "guide_groups" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabGuideGroups(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[BcdataGuideGroupRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_guide_group_row](repeating: sdrkit_bcdata_guide_group_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_guide_groups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataGuideGroupRow.init))
    }
    /// The changed rows of the table "slide_categories" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabSlideCategories(_ handle: SDRKIT_HANDLE, capacity: Int = 4096) -> Result<[BcdataSlideCategoryRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_slide_category_row](repeating: sdrkit_bcdata_slide_category_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_slide_categories_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataSlideCategoryRow.init))
    }
    /// The changed rows of the table "logos" of the stack "dab", newest first, at most `capacity` rows.
    public static func dabLogos(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[BcdataLogoRow], SDRKitCode> {
        var rows = [sdrkit_bcdata_logo_row](repeating: sdrkit_bcdata_logo_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dab_logos_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BcdataLogoRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum DabCommands {
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func dabServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dab_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dab_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dab_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum DabSettings {
    /// The options of the setting "detection" of the stack "dab".
    public enum DabDetection: Int32 {
        case auto = 0
        case differential = 1
    }
    /// Detection.
    public static func dabDetection(_ handle: SDRKIT_HANDLE) -> Result<DabDetection, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "detection", &v)
        return code == SDRKIT_OK ? .success(DabDetection(rawValue: v) ?? DabDetection(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDabDetection(_ handle: SDRKIT_HANDLE, _ value: DabDetection) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "detection", value.rawValue))
    }
    /// Centre the ensemble.
    public static func dabCentring(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "centring", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDabCentring(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "centring", value))
    }
    /// Impulse blanker.
    public static func dabImpulseBlanker(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "impulse_blanker", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDabImpulseBlanker(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "impulse_blanker", value))
    }
    /// Early start.
    public static func dabEarlyStart(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "early_start", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDabEarlyStart(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "early_start", value))
    }
    /// The options of the setting "aac_packaging" of the stack "dab".
    public enum DabAacPackaging: Int32 {
        case raw = 0
        case latm = 1
    }
    /// AAC packaging.
    public static func dabAacPackaging(_ handle: SDRKIT_HANDLE) -> Result<DabAacPackaging, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "aac_packaging", &v)
        return code == SDRKIT_OK ? .success(DabAacPackaging(rawValue: v) ?? DabAacPackaging(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDabAacPackaging(_ handle: SDRKIT_HANDLE, _ value: DabAacPackaging) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "aac_packaging", value.rawValue))
    }
    /// Channels.
    public static func dabChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDabChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Data sub-channel.
    public static func dabDataSubchannel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "data_subchannel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDabDataSubchannel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "data_subchannel", value))
    }
    /// Transmitter identification.
    public static func dabTii(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tii", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDabTii(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tii", value))
    }
    /// Announcement types.
    public static func dabAnnouncementTypes(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "announcement_types", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDabAnnouncementTypes(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "announcement_types", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum DabGeneratorSettings {
    /// Transmitter identification.
    public static func dabTii(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_tii", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDabTii(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_tii", value))
    }
    /// Extended labels.
    public static func dabFig2(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_fig2", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDabFig2(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_fig2", value))
    }
    /// Programme type.
    public static func dabPty(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_pty", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDabPty(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_pty", value))
    }
}
