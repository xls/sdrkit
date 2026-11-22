// sdrkit: the typed interface of the stack family "analogtv", version 1.2. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum AnalogtvInterface {
    public static let major = 1
    public static let minor = 2
}

/// The result fields of the `atv` receiver, which its status component writes.
public struct AtvFields {
    public enum State: Int32 {
        case searching = 0
        case sync = 1
        case locked = 2
    }
    public enum ColourSystem: Int32 {
        case pal = 0
        case palM = 1
        case palN = 2
        case ntsc = 3
        case ntscN = 4
        case ntsc443 = 5
        case secam = 6
        case mono = 7
    }
    public enum Detector: Int32 {
        case synchronous = 0
        case envelope = 1
        case fm = 2
    }
    public enum Modulation: Int32 {
        case am = 0
        case fm = 1
        case nbtvAm = 2
        case nbtvFm = 3
    }
    public enum SoundSystem: Int32 {
        case fm = 0
        case am = 1
        case a2Mono = 2
        case a2Stereo = 3
        case a2DualA = 4
        case a2DualB = 5
        case nicamStereo = 6
        case nicamDual1 = 7
        case nicamDual2 = 8
        case nicamMono = 9
        case noneFound = 10
        case notDecoded = 11
        case picture = 12
    }
    public enum FmPolarity: Int32 {
        case trying = 0
        case tipLow = 1
        case tipHigh = 2
    }
    public enum NicamMode: Int32 {
        case stereo = 0
        case dualMono = 1
        case mono = 2
        case data = 3
    }
    public enum A2Mode: Int32 {
        case mono = 0
        case stereo = 1
        case dual = 2
    }
    /// The state of the receiver.
    public var state: State?
    /// The system and its raster, "PAL B/G 625/50". Absent while the receiver knows none.
    public var system: String?
    /// The colour system in use.
    public var colourSystem: ColourSystem?
    /// The picture has colour now.
    public var colour: Bool
    /// The colour killer is on (the burst is too weak).
    public var colourKiller: Bool
    /// Lines a frame.
    public var lines: Int64
    /// Fields a second.
    public var fieldRate: Double
    /// The line rate.
    public var lineRate: Double
    /// The line rate against its nominal value.
    public var lineErrorPpm: Double
    /// The detector in use.
    public var detector: Detector?
    /// The modulation the receiver runs.
    public var modulation: Modulation?
    /// The vision carrier, from the centre of the input.
    public var visionCarrier: Double
    /// The sound carrier, from the centre of the input. Absent while none is found.
    public var soundCarrier: Double?
    /// The sound carrier above the vision carrier (negative: below it).
    public var soundSpacing: Double
    /// The offset of the vision carrier.
    public var offsetHz: Double
    /// The level of the vision carrier.
    public var levelDbfs: Double
    /// The peak vision carrier against the noise in 5 MHz.
    public var carrierToNoiseDb: Double
    /// The SNR of the picture.
    public var snrDb: Double
    /// A sound carrier is there.
    public var soundPresent: Bool
    /// The peak deviation of the FM sound carrier.
    public var soundDeviation: Double
    /// The audio level.
    public var audioLevelDbfs: Double
    /// The sound that plays.
    public var soundSystem: SoundSystem?
    /// The share of lines whose sync was found.
    public var syncFoundPct: Double
    /// Lines since the line loop started.
    public var lineCount: Int64
    /// Field syncs found.
    public var fieldCount: Int64
    /// Pictures sent.
    public var pictures: Int64
    /// Snow pictures sent.
    public var snowPictures: Int64
    /// The field completed last (1 or 2).
    public var lastField: Int64
    /// Fields decoded completely.
    public var fieldsOk: Int64
    /// Fields damaged.
    public var fieldsBad: Int64
    /// The sync depth, a share of the peak carrier.
    public var syncDepthPct: Double
    /// The sync compression measured.
    public var syncCompressionPct: Double
    /// The white peak (1 is the nominal white).
    public var whitePeak: Double
    /// The colour burst, a share of its nominal level.
    public var burstPct: Double
    /// The phase error of the burst.
    public var burstPhaseErrorDeg: Double
    /// The SECAM colour subcarrier level, a share of its nominal level.
    public var chromaLevelPct: Double
    /// How SECAM was found.
    public var secamIdentification: String?
    /// The polarity of FM video.
    public var fmPolarity: FmPolarity?
    /// The NBTV line loop is locked.
    public var nbtvLineLocked: Bool
    /// The NBTV picture is locked.
    public var nbtvPictureLocked: Bool
    /// The NICAM frame alignment holds. Absent without a NICAM carrier.
    public var nicamLocked: Bool?
    /// The NICAM sound is valid and can play.
    public var nicamValid: Bool?
    /// The NICAM sound mode.
    public var nicamMode: NicamMode?
    /// NICAM sound words whose parity failed, since the lock.
    public var nicamParityErrors: Int64?
    /// The share of NICAM sound words whose parity failed.
    public var nicamErrorPct: Double?
    /// The A2 identification found.
    public var a2Mode: A2Mode?

    init(_ c: sdrkit_atv_fields) {
        state = State(rawValue: c.state)
        system = c.has_system ? sdrkitText(c.system) : nil
        colourSystem = c.has_colour_system ? ColourSystem(rawValue: c.colour_system) : nil
        colour = c.colour
        colourKiller = c.colour_killer
        lines = c.lines
        fieldRate = c.field_rate
        lineRate = c.line_rate
        lineErrorPpm = c.line_error_ppm
        detector = Detector(rawValue: c.detector)
        modulation = Modulation(rawValue: c.modulation)
        visionCarrier = c.vision_carrier
        soundCarrier = c.has_sound_carrier ? c.sound_carrier : nil
        soundSpacing = c.sound_spacing
        offsetHz = c.offset_hz
        levelDbfs = c.level_dbfs
        carrierToNoiseDb = c.carrier_to_noise_db
        snrDb = c.snr_db
        soundPresent = c.sound_present
        soundDeviation = c.sound_deviation
        audioLevelDbfs = c.audio_level_dbfs
        soundSystem = SoundSystem(rawValue: c.sound_system)
        syncFoundPct = c.sync_found_pct
        lineCount = c.line_count
        fieldCount = c.field_count
        pictures = c.pictures
        snowPictures = c.snow_pictures
        lastField = c.last_field
        fieldsOk = c.fields_ok
        fieldsBad = c.fields_bad
        syncDepthPct = c.sync_depth_pct
        syncCompressionPct = c.sync_compression_pct
        whitePeak = c.white_peak
        burstPct = c.burst_pct
        burstPhaseErrorDeg = c.burst_phase_error_deg
        chromaLevelPct = c.chroma_level_pct
        secamIdentification = c.has_secam_identification ? sdrkitText(c.secam_identification) : nil
        fmPolarity = c.has_fm_polarity ? FmPolarity(rawValue: c.fm_polarity) : nil
        nbtvLineLocked = c.nbtv_line_locked
        nbtvPictureLocked = c.nbtv_picture_locked
        nicamLocked = c.has_nicam_locked ? c.nicam_locked : nil
        nicamValid = c.has_nicam_valid ? c.nicam_valid : nil
        nicamMode = c.has_nicam_mode ? NicamMode(rawValue: c.nicam_mode) : nil
        nicamParityErrors = c.has_nicam_parity_errors ? c.nicam_parity_errors : nil
        nicamErrorPct = c.has_nicam_error_pct ? c.nicam_error_pct : nil
        a2Mode = c.has_a2_mode ? A2Mode(rawValue: c.a2_mode) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AtvFields, SDRKitCode> {
        var c = sdrkit_atv_fields()
        let code = sdrkit_atv_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AtvFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the teletext decoder of the `atv` and `fpv` stacks. The counts have no value while no teletext line arrives.
public struct AnalogtvTeletextFields {
    /// The header of the last teletext page.
    public var teletextHeader: String?
    /// Teletext pages held.
    public var teletextPages: Int64?
    /// Teletext packets received.
    public var teletextPackets: Int64?
    /// Teletext lines whose framing code or address failed.
    public var teletextErrors: Int64?
    /// Teletext characters whose parity failed.
    public var teletextParityErrors: Int64?
    /// The teletext page shown.
    public var teletextPage: Int64
    /// The text of the teletext page shown, its rows joined with line feeds.
    public var pageText: String?

    init(_ c: sdrkit_analogtv_teletext_fields) {
        teletextHeader = c.has_teletext_header ? sdrkitText(c.teletext_header) : nil
        teletextPages = c.has_teletext_pages ? c.teletext_pages : nil
        teletextPackets = c.has_teletext_packets ? c.teletext_packets : nil
        teletextErrors = c.has_teletext_errors ? c.teletext_errors : nil
        teletextParityErrors = c.has_teletext_parity_errors ? c.teletext_parity_errors : nil
        teletextPage = c.teletext_page
        pageText = c.has_page_text ? sdrkitText(c.page_text) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AnalogtvTeletextFields, SDRKitCode> {
        var c = sdrkit_analogtv_teletext_fields()
        let code = sdrkit_analogtv_teletext_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AnalogtvTeletextFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the senders of `atv` and `fpv`.
public struct AnalogtvTvTxFields {
    /// Whether the sender sends.
    public var txSending: Bool
    /// The system sent, "PAL B/G 625/50". No value while nothing is sent.
    public var txSystem: String?
    /// The pictures of the host put on the air in the last transmission: one for each field that started with a new one.
    public var txFrames: Int64
    /// The fields sent in the last transmission.
    public var txFields: Int64

    init(_ c: sdrkit_analogtv_tv_tx_fields) {
        txSending = c.tx_sending
        txSystem = c.has_tx_system ? sdrkitText(c.tx_system) : nil
        txFrames = c.tx_frames
        txFields = c.tx_fields
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AnalogtvTvTxFields, SDRKitCode> {
        var c = sdrkit_analogtv_tv_tx_fields()
        let code = sdrkit_analogtv_tv_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AnalogtvTvTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A channel of the setting `channels`. The row key is the label.
public struct AtvChannelRow {
    /// The row key.
    public var key: String
    /// The channel label.
    public var label: String
    /// The vision carrier of the channel.
    public var vision: Double

    init(_ c: sdrkit_atv_channel_row) {
        key = sdrkitText(c.key)
        label = sdrkitText(c.label)
        vision = c.vision
    }
}

/// The result fields of the MAC receiver.
public struct MacFields {
    public enum State: Int32 {
        case searching = 0
        case sync = 1
        case locked = 2
    }
    public enum System: Int32 {
        case d2 = 0
        case d = 1
    }
    public enum Aspect: Int32 {
        case normal = 0
        case wide = 1
    }
    public enum CaSystem: Int32 {
        case eurocrypt = 0
        case other = 1
    }
    public enum SoundMode: Int32 {
        case companded = 0
        case scrambled = 1
    }
    /// The state of the receiver.
    public var state: State?
    /// The MAC variant. Absent while the receiver searches.
    public var system: System?
    /// The signal-to-noise ratio of the data burst.
    public var snrDb: Double
    /// The SNR of the picture.
    public var videoSnrDb: Double
    /// The carrier offset.
    public var offsetHz: Double
    /// The level of the signal, dBFS.
    public var levelDbfs: Double
    /// The FM deviation, Hz a volt of the baseband.
    public var deviationHzPerV: Double
    /// The spectrum is inverted (and corrected).
    public var spectrumInverted: Bool
    /// The line rate.
    public var lineRate: Double
    /// The line rate against its nominal value.
    public var lineErrorPpm: Double
    /// The share of lines whose sync word was found.
    public var lineSyncPct: Double
    /// Frame syncs found.
    public var frameSyncs: Int64
    /// Frames decoded.
    public var frames: Int64
    /// The bit error rate of the data burst.
    public var ber: Double
    /// The channel name. Absent until the service identification names it.
    public var channel: String?
    /// The network name.
    public var network: String?
    /// The country.
    public var country: String?
    /// The channel identifier.
    public var channelId: Int64?
    /// The picture's aspect ratio.
    public var aspect: Aspect?
    /// The picture or sound is scrambled.
    public var scrambled: Bool
    /// The access conditions, in words.
    public var access: String?
    /// The conditional access system, when the channel names one.
    public var caSystem: CaSystem?
    /// Sound words whose protection failed.
    public var soundErrors: Int64
    /// ECM packets received.
    public var ecmPackets: Int64
    /// Packet headers decoded.
    public var packetHeaders: Int64
    /// Packet headers corrected.
    public var packetsCorrected: Int64
    /// Dummy packets.
    public var packetsDummy: Int64
    /// Service identification groups whose CRC passed.
    public var siGroupsOk: Int64
    /// Service identification groups whose CRC failed.
    public var siGroupsBad: Int64
    /// The packet address of the sound.
    public var soundAddress: Int64?
    /// The sound is there and plays.
    public var soundPresent: Bool
    /// Sound packets received.
    public var soundPackets: Int64
    /// How the sound is coded.
    public var soundMode: SoundMode?
    /// The audio level.
    public var audioLevelDbfs: Double
    /// Sound blocks whose words failed their parity.
    public var blocksBad: Int64
    /// Pictures sent.
    public var pictures: Int64

    init(_ c: sdrkit_mac_fields) {
        state = State(rawValue: c.state)
        system = c.has_system ? System(rawValue: c.system) : nil
        snrDb = c.snr_db
        videoSnrDb = c.video_snr_db
        offsetHz = c.offset_hz
        levelDbfs = c.level_dbfs
        deviationHzPerV = c.deviation_hz_per_v
        spectrumInverted = c.spectrum_inverted
        lineRate = c.line_rate
        lineErrorPpm = c.line_error_ppm
        lineSyncPct = c.line_sync_pct
        frameSyncs = c.frame_syncs
        frames = c.frames
        ber = c.ber
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        network = c.has_network ? sdrkitText(c.network) : nil
        country = c.has_country ? sdrkitText(c.country) : nil
        channelId = c.has_channel_id ? c.channel_id : nil
        aspect = c.has_aspect ? Aspect(rawValue: c.aspect) : nil
        scrambled = c.scrambled
        access = c.has_access ? sdrkitText(c.access) : nil
        caSystem = c.has_ca_system ? CaSystem(rawValue: c.ca_system) : nil
        soundErrors = c.sound_errors
        ecmPackets = c.ecm_packets
        packetHeaders = c.packet_headers
        packetsCorrected = c.packets_corrected
        packetsDummy = c.packets_dummy
        siGroupsOk = c.si_groups_ok
        siGroupsBad = c.si_groups_bad
        soundAddress = c.has_sound_address ? c.sound_address : nil
        soundPresent = c.sound_present
        soundPackets = c.sound_packets
        soundMode = SoundMode(rawValue: c.sound_mode)
        audioLevelDbfs = c.audio_level_dbfs
        blocksBad = c.blocks_bad
        pictures = c.pictures
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MacFields, SDRKitCode> {
        var c = sdrkit_mac_fields()
        let code = sdrkit_mac_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MacFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the `fpv` receiver, which its display writes.
public struct FpvFields {
    public enum State: Int32 {
        case searching = 0
        case sync = 1
        case locked = 2
    }
    public enum ColourSystem: Int32 {
        case pal = 0
        case palM = 1
        case palN = 2
        case ntsc = 3
        case ntscN = 4
        case ntsc443 = 5
        case secam = 6
        case mono = 7
    }
    public enum Polarity: Int32 {
        case trying = 0
        case tipLow = 1
        case tipHigh = 2
    }
    /// The state of the receiver.
    public var state: State?
    /// The names of the channels at the tuned frequency. Absent off every channel.
    public var channel: String?
    /// The level of the channel.
    public var levelDbfs: Double
    /// The level of the channel above the noise floor.
    public var overNoiseDb: Double
    /// The noise floor.
    public var noiseDbfs: Double
    /// The offset of the transmitter from the channel.
    public var offsetHz: Double
    /// The system and its raster. Absent while the receiver knows none.
    public var system: String?
    /// The colour system in use.
    public var colourSystem: ColourSystem?
    /// Lines a frame.
    public var lines: Int64
    /// Fields a second.
    public var fieldRate: Double
    /// The share of lines whose sync was found.
    public var syncFoundPct: Double
    /// Pictures sent.
    public var pictures: Int64
    /// Fields decoded completely.
    public var fieldsOk: Int64
    /// Fields damaged.
    public var fieldsBad: Int64
    /// The polarity of the FM video.
    public var polarity: Polarity?
    /// The SNR of the picture.
    public var snrDb: Double
    /// A sound subcarrier is there.
    public var soundPresent: Bool
    /// The sound subcarrier, from the vision carrier. Absent while none is found.
    public var soundSubcarrier: Double?
    /// The 6.0 MHz subcarrier against the noise beside it.
    public var subcarrier60Db: Double
    /// The 6.5 MHz subcarrier against the noise beside it.
    public var subcarrier65Db: Double

    init(_ c: sdrkit_fpv_fields) {
        state = State(rawValue: c.state)
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        levelDbfs = c.level_dbfs
        overNoiseDb = c.over_noise_db
        noiseDbfs = c.noise_dbfs
        offsetHz = c.offset_hz
        system = c.has_system ? sdrkitText(c.system) : nil
        colourSystem = c.has_colour_system ? ColourSystem(rawValue: c.colour_system) : nil
        lines = c.lines
        fieldRate = c.field_rate
        syncFoundPct = c.sync_found_pct
        pictures = c.pictures
        fieldsOk = c.fields_ok
        fieldsBad = c.fields_bad
        polarity = Polarity(rawValue: c.polarity)
        snrDb = c.snr_db
        soundPresent = c.sound_present
        soundSubcarrier = c.has_sound_subcarrier ? c.sound_subcarrier : nil
        subcarrier60Db = c.subcarrier_60_db
        subcarrier65Db = c.subcarrier_65_db
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FpvFields, SDRKitCode> {
        var c = sdrkit_fpv_fields()
        let code = sdrkit_fpv_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FpvFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A channel of the FPV meter. The row key is the channel name ("R5").
public struct FpvChannelRow {
    /// The row key.
    public var key: String
    /// The name of the channel ("R5").
    public var name: String
    /// The band letter.
    public var band: String
    /// The number of the channel in its band.
    public var number: Int64
    /// The frequency of the channel.
    public var freq: Double
    /// The level of the channel.
    public var levelDbfs: Double
    /// The level of the channel above the noise floor.
    public var overNoiseDb: Double
    /// A transmitter is on the channel.
    public var present: Bool
    /// The channel is inside the capture.
    public var inside: Bool
    /// The stream time at which a transmitter was last heard on the channel.
    public var heard: Double?

    init(_ c: sdrkit_fpv_channel_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        band = sdrkitText(c.band)
        number = c.number
        freq = c.freq
        levelDbfs = c.level_dbfs
        overNoiseDb = c.over_noise_db
        present = c.present
        inside = c.inside
        heard = c.has_heard ? c.heard : nil
    }
}

/// A change of the state of the `atv` receiver.
public struct AtvStateEvent {
    public enum State: Int32 {
        case searching = 0
        case sync = 1
        case locked = 2
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
    /// The new state.
    public var state: State?
    /// The system of the receiver.
    public var text: String
    /// What the event is about: the receiver.
    public var family: Family?

    init(_ c: sdrkit_atv_state_event) {
        time = c.time
        freqHz = c.freq_hz
        state = State(rawValue: c.state)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AtvStateEvent, SDRKitCode> {
        var c = sdrkit_atv_state_event()
        let code = sdrkit_atv_state_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AtvStateEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A change of the state of the MAC receiver.
public struct MacStateEvent {
    public enum State: Int32 {
        case searching = 0
        case sync = 1
        case locked = 2
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
    /// The new state.
    public var state: State?
    /// The summary of the receiver.
    public var text: String
    /// What the event is about: the receiver.
    public var family: Family?

    init(_ c: sdrkit_mac_state_event) {
        time = c.time
        freqHz = c.freq_hz
        state = State(rawValue: c.state)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MacStateEvent, SDRKitCode> {
        var c = sdrkit_mac_state_event()
        let code = sdrkit_mac_state_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MacStateEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A change of the state of the `fpv` receiver.
public struct FpvStateEvent {
    public enum State: Int32 {
        case searching = 0
        case sync = 1
        case locked = 2
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
    /// The new state.
    public var state: State?
    /// The names of the channels at the tuned frequency.
    public var text: String
    /// What the event is about: the receiver.
    public var family: Family?

    init(_ c: sdrkit_fpv_state_event) {
        time = c.time
        freqHz = c.freq_hz
        state = State(rawValue: c.state)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FpvStateEvent, SDRKitCode> {
        var c = sdrkit_fpv_state_event()
        let code = sdrkit_fpv_state_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FpvStateEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum AnalogtvTables {
    /// The changed rows of the table "channels" of the stack "atv", newest first, at most `capacity` rows.
    /// The column `label` names a row.
    public static func atvChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[AtvChannelRow], SDRKitCode> {
        var rows = [sdrkit_atv_channel_row](repeating: sdrkit_atv_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atv_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AtvChannelRow.init))
    }
    /// The changed rows of the table "teletext_pages" of the stack "atv", newest first, at most `capacity` rows.
    public static func atvTeletextPages(_ handle: SDRKIT_HANDLE, capacity: Int = 800) -> Result<[MpegtsTeletextPageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_page_row](repeating: sdrkit_mpegts_teletext_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atv_teletext_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextPageRow.init))
    }
    /// The changed rows of the table "teletext_subpages" of the stack "atv", newest first, at most `capacity` rows.
    public static func atvTeletextSubpages(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTeletextSubpageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_subpage_row](repeating: sdrkit_mpegts_teletext_subpage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atv_teletext_subpages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextSubpageRow.init))
    }
    /// The changed rows of the table "channels" of the stack "fpv", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func fpvChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[FpvChannelRow], SDRKitCode> {
        var rows = [sdrkit_fpv_channel_row](repeating: sdrkit_fpv_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fpv_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FpvChannelRow.init))
    }
    /// The changed rows of the table "teletext_pages" of the stack "fpv", newest first, at most `capacity` rows.
    public static func fpvTeletextPages(_ handle: SDRKIT_HANDLE, capacity: Int = 800) -> Result<[MpegtsTeletextPageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_page_row](repeating: sdrkit_mpegts_teletext_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fpv_teletext_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextPageRow.init))
    }
    /// The changed rows of the table "teletext_subpages" of the stack "fpv", newest first, at most `capacity` rows.
    public static func fpvTeletextSubpages(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTeletextSubpageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_subpage_row](repeating: sdrkit_mpegts_teletext_subpage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fpv_teletext_subpages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextSubpageRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum AnalogtvCommands {
    /// Tune to a channel. Asks the host to tune to a channel of the setting channels.
    public static func atvChannelTune(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_atv_channel_args()
        args.size = UInt32(MemoryLayout<sdrkit_atv_channel_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_atv_channel_tune(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Start sending. Sends the television signal continuously from the start of a field: the picture of the command frame, or the test picture, and the host's audio. The transmit settings apply at each start. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func atvStart(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_atv_start(handle)
        return sdrkitResult(code)
    }
    /// Stop sending. Ends the transmission at the end of a field, with a fade of 5 ms.
    public static func atvStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_atv_stop(handle)
        return sdrkitResult(code)
    }
    /// Set the picture. Sends this picture from the start of the next field until the next one: a still picture, or video when the host gives a picture each field. It is scaled to the picture of the system with its shape kept, black bars above and below or at the sides.
    public static func atvFrame(_ handle: SDRKIT_HANDLE, pixels: [UInt8], width: Int64, height: Int64, pixelFormat: Int32) -> Result<Void, SDRKitCode> {
        var args = sdrkit_analogtv_tv_frame_args()
        args.size = UInt32(MemoryLayout<sdrkit_analogtv_tv_frame_args>.size)
        let code: Int32 = pixels.withUnsafeBufferPointer { pixels_ in { () -> Int32 in
        args.pixels = sdrkit_bytes(ptr: pixels_.baseAddress, len: pixels_.count)
        args.width = width
        args.height = height
        args.pixel_format = pixelFormat
        return sdrkit_atv_frame(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a teletext page. Shows a teletext page.
    public static func atvTeletextSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_analogtv_teletext_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_analogtv_teletext_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_atv_teletext_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear the channel levels. Forgets the channel levels, the noise floor and the level history.
    public static func fpvClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fpv_clear(handle)
        return sdrkitResult(code)
    }
    /// Tune to a channel. Asks the host to tune to an FPV channel.
    public static func fpvChannelTune(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_fpv_channel_args()
        args.size = UInt32(MemoryLayout<sdrkit_fpv_channel_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_fpv_channel_tune(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Scan. Asks the host to visit every channel of a plan for 0.7 s each.
    public static func fpvScan(_ handle: SDRKIT_HANDLE, plan: Int32) -> Result<Void, SDRKitCode> {
        var args = sdrkit_fpv_scan_args()
        args.size = UInt32(MemoryLayout<sdrkit_fpv_scan_args>.size)
        let code: Int32 = { () -> Int32 in
        args.plan = plan
        return sdrkit_fpv_scan(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Select a teletext page. Shows a teletext page.
    public static func fpvTeletextSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_analogtv_teletext_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_analogtv_teletext_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_fpv_teletext_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Start sending. Sends the television signal continuously from the start of a field: the picture of the command frame, or the test picture, and the host's audio. The transmit settings apply at each start. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func fpvStart(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fpv_start(handle)
        return sdrkitResult(code)
    }
    /// Stop sending. Ends the transmission at the end of a field, with a fade of 5 ms.
    public static func fpvStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fpv_stop(handle)
        return sdrkitResult(code)
    }
    /// Set the picture. Sends this picture from the start of the next field until the next one: a still picture, or video when the host gives a picture each field. It is scaled to the picture of the system with its shape kept, black bars above and below or at the sides.
    public static func fpvFrame(_ handle: SDRKIT_HANDLE, pixels: [UInt8], width: Int64, height: Int64, pixelFormat: Int32) -> Result<Void, SDRKitCode> {
        var args = sdrkit_analogtv_tv_frame_args()
        args.size = UInt32(MemoryLayout<sdrkit_analogtv_tv_frame_args>.size)
        let code: Int32 = pixels.withUnsafeBufferPointer { pixels_ in { () -> Int32 in
        args.pixels = sdrkit_bytes(ptr: pixels_.baseAddress, len: pixels_.count)
        args.width = width
        args.height = height
        args.pixel_format = pixelFormat
        return sdrkit_fpv_frame(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum AnalogtvSettings {
    /// The options of the setting "modulation" of the stack "atv".
    public enum AtvModulation: Int32 {
        case am = 0
        case fm = 1
        case nbtvAm = 2
        case nbtvFm = 3
    }
    /// Modulation.
    public static func atvModulation(_ handle: SDRKIT_HANDLE) -> Result<AtvModulation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "modulation", &v)
        return code == SDRKIT_OK ? .success(AtvModulation(rawValue: v) ?? AtvModulation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvModulation(_ handle: SDRKIT_HANDLE, _ value: AtvModulation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "modulation", value.rawValue))
    }
    /// Channels.
    public static func atvChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// The options of the setting "system" of the stack "atv".
    public enum AtvSystem: Int32 {
        case auto = 0
        case b = 1
        case g = 2
        case i = 3
        case dk = 4
        case m = 5
        case n = 6
        case a = 7
        case c = 8
        case e = 9
        case h = 10
        case kp = 11
        case l = 12
        case lp = 13
    }
    /// System.
    public static func atvSystem(_ handle: SDRKIT_HANDLE) -> Result<AtvSystem, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "system", &v)
        return code == SDRKIT_OK ? .success(AtvSystem(rawValue: v) ?? AtvSystem(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvSystem(_ handle: SDRKIT_HANDLE, _ value: AtvSystem) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "system", value.rawValue))
    }
    /// The options of the setting "colour" of the stack "atv".
    public enum AtvColour: Int32 {
        case auto = 0
        case mono = 1
        case pal = 2
        case ntsc = 3
        case secam = 4
    }
    /// Colour system.
    public static func atvColour(_ handle: SDRKIT_HANDLE) -> Result<AtvColour, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "colour", &v)
        return code == SDRKIT_OK ? .success(AtvColour(rawValue: v) ?? AtvColour(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvColour(_ handle: SDRKIT_HANDLE, _ value: AtvColour) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "colour", value.rawValue))
    }
    /// Colour.
    public static func atvColourOn(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "colour_on", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvColourOn(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "colour_on", value))
    }
    /// Saturation.
    public static func atvSaturation(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "saturation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvSaturation(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "saturation", value))
    }
    /// Tint.
    public static func atvTint(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tint", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTint(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tint", value))
    }
    /// The options of the setting "detector" of the stack "atv".
    public enum AtvDetector: Int32 {
        case auto = 0
        case envelope = 1
    }
    /// Envelope detector.
    public static func atvDetector(_ handle: SDRKIT_HANDLE) -> Result<AtvDetector, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "detector", &v)
        return code == SDRKIT_OK ? .success(AtvDetector(rawValue: v) ?? AtvDetector(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvDetector(_ handle: SDRKIT_HANDLE, _ value: AtvDetector) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "detector", value.rawValue))
    }
    /// The options of the setting "deinterlace" of the stack "atv".
    public enum AtvDeinterlace: Int32 {
        case weave = 0
        case bob = 1
    }
    /// Fields.
    public static func atvDeinterlace(_ handle: SDRKIT_HANDLE) -> Result<AtvDeinterlace, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "deinterlace", &v)
        return code == SDRKIT_OK ? .success(AtvDeinterlace(rawValue: v) ?? AtvDeinterlace(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvDeinterlace(_ handle: SDRKIT_HANDLE, _ value: AtvDeinterlace) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "deinterlace", value.rawValue))
    }
    /// The options of the setting "noise_reduction" of the stack "atv".
    public enum AtvNoiseReduction: Int32 {
        case auto = 0
        case off = 1
        case light = 2
        case medium = 3
        case strong = 4
    }
    /// Noise reduction.
    public static func atvNoiseReduction(_ handle: SDRKIT_HANDLE) -> Result<AtvNoiseReduction, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "noise_reduction", &v)
        return code == SDRKIT_OK ? .success(AtvNoiseReduction(rawValue: v) ?? AtvNoiseReduction(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvNoiseReduction(_ handle: SDRKIT_HANDLE, _ value: AtvNoiseReduction) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "noise_reduction", value.rawValue))
    }
    /// The options of the setting "black_setup" of the stack "atv".
    public enum AtvBlackSetup: Int32 {
        case system = 0
        case none = 1
        case n75Ire = 2
    }
    /// Black setup.
    public static func atvBlackSetup(_ handle: SDRKIT_HANDLE) -> Result<AtvBlackSetup, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "black_setup", &v)
        return code == SDRKIT_OK ? .success(AtvBlackSetup(rawValue: v) ?? AtvBlackSetup(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvBlackSetup(_ handle: SDRKIT_HANDLE, _ value: AtvBlackSetup) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "black_setup", value.rawValue))
    }
    /// Chroma delay.
    public static func atvChromaDelay(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "chroma_delay", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvChromaDelay(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "chroma_delay", value))
    }
    /// The options of the setting "channel_width" of the stack "atv".
    public enum AtvChannelWidth: Int32 {
        case n6Mhz = 0
        case n7Mhz = 1
        case n8Mhz = 2
    }
    /// Channel width.
    public static func atvChannelWidth(_ handle: SDRKIT_HANDLE) -> Result<AtvChannelWidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channel_width", &v)
        return code == SDRKIT_OK ? .success(AtvChannelWidth(rawValue: v) ?? AtvChannelWidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvChannelWidth(_ handle: SDRKIT_HANDLE, _ value: AtvChannelWidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channel_width", value.rawValue))
    }
    /// Show snow when there is no picture.
    public static func atvSnow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "snow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvSnow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "snow", value))
    }
    /// The options of the setting "sound_system" of the stack "atv".
    public enum AtvSoundSystem: Int32 {
        case auto = 0
        case carrier = 1
    }
    /// Sound.
    public static func atvSoundSystem(_ handle: SDRKIT_HANDLE) -> Result<AtvSoundSystem, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sound_system", &v)
        return code == SDRKIT_OK ? .success(AtvSoundSystem(rawValue: v) ?? AtvSoundSystem(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvSoundSystem(_ handle: SDRKIT_HANDLE, _ value: AtvSoundSystem) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sound_system", value.rawValue))
    }
    /// The options of the setting "dual_sound" of the stack "atv".
    public enum AtvDualSound: Int32 {
        case first = 0
        case second = 1
    }
    /// Two languages.
    public static func atvDualSound(_ handle: SDRKIT_HANDLE) -> Result<AtvDualSound, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "dual_sound", &v)
        return code == SDRKIT_OK ? .success(AtvDualSound(rawValue: v) ?? AtvDualSound(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvDualSound(_ handle: SDRKIT_HANDLE, _ value: AtvDualSound) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "dual_sound", value.rawValue))
    }
    /// Lines.
    public static func atvNbtvLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "nbtv_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvNbtvLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "nbtv_lines", value))
    }
    /// Pictures/s.
    public static func atvNbtvRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "nbtv_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvNbtvRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "nbtv_rate", value))
    }
    /// FM deviation.
    public static func atvNbtvDeviation(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "nbtv_deviation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvNbtvDeviation(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "nbtv_deviation", value))
    }
    /// Vertical scan.
    public static func atvNbtvVertical(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "nbtv_vertical", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvNbtvVertical(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "nbtv_vertical", value))
    }
    /// The options of the setting "tx_system" of the stack "atv".
    public enum AtvTxSystem: Int32 {
        case g = 0
        case b = 1
        case i = 2
        case dk = 3
        case m = 4
        case n = 5
        case a = 6
        case c = 7
        case e = 8
        case h = 9
        case kp = 10
        case l = 11
        case lp = 12
    }
    /// System.
    public static func atvTxSystem(_ handle: SDRKIT_HANDLE) -> Result<AtvTxSystem, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_system", &v)
        return code == SDRKIT_OK ? .success(AtvTxSystem(rawValue: v) ?? AtvTxSystem(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTxSystem(_ handle: SDRKIT_HANDLE, _ value: AtvTxSystem) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_system", value.rawValue))
    }
    /// The options of the setting "tx_colour" of the stack "atv".
    public enum AtvTxColour: Int32 {
        case system = 0
        case pal = 1
        case ntsc = 2
        case secam = 3
        case none = 4
    }
    /// Colour.
    public static func atvTxColour(_ handle: SDRKIT_HANDLE) -> Result<AtvTxColour, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_colour", &v)
        return code == SDRKIT_OK ? .success(AtvTxColour(rawValue: v) ?? AtvTxColour(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTxColour(_ handle: SDRKIT_HANDLE, _ value: AtvTxColour) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_colour", value.rawValue))
    }
    /// The options of the setting "tx_picture" of the stack "atv".
    public enum AtvTxPicture: Int32 {
        case card = 0
        case bars = 1
        case ramp = 2
    }
    /// Test picture.
    public static func atvTxPicture(_ handle: SDRKIT_HANDLE) -> Result<AtvTxPicture, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_picture", &v)
        return code == SDRKIT_OK ? .success(AtvTxPicture(rawValue: v) ?? AtvTxPicture(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTxPicture(_ handle: SDRKIT_HANDLE, _ value: AtvTxPicture) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_picture", value.rawValue))
    }
    /// Sound.
    public static func atvTxSound(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_sound", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTxSound(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_sound", value))
    }
    /// Sync compression.
    public static func atvTxCompression(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_compression", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTxCompression(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_compression", value))
    }
    /// Group-delay pre-correction.
    public static func atvTxGroupDelay(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_group_delay", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTxGroupDelay(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_group_delay", value))
    }
    /// Nyquist slope.
    public static func atvTxNyquist(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_nyquist", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTxNyquist(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_nyquist", value))
    }
    /// The options of the setting "tx_setup" of the stack "atv".
    public enum AtvTxSetup: Int32 {
        case n75Ire = 0
        case none = 1
    }
    /// Black setup in 525 lines.
    public static func atvTxSetup(_ handle: SDRKIT_HANDLE) -> Result<AtvTxSetup, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_setup", &v)
        return code == SDRKIT_OK ? .success(AtvTxSetup(rawValue: v) ?? AtvTxSetup(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTxSetup(_ handle: SDRKIT_HANDLE, _ value: AtvTxSetup) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_setup", value.rawValue))
    }
    /// The options of the setting "tx_secam_id" of the stack "atv".
    public enum AtvTxSecamId: Int32 {
        case both = 0
        case porch = 1
        case lines = 2
    }
    /// SECAM identification.
    public static func atvTxSecamId(_ handle: SDRKIT_HANDLE) -> Result<AtvTxSecamId, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_secam_id", &v)
        return code == SDRKIT_OK ? .success(AtvTxSecamId(rawValue: v) ?? AtvTxSecamId(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTxSecamId(_ handle: SDRKIT_HANDLE, _ value: AtvTxSecamId) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_secam_id", value.rawValue))
    }
    /// The options of the setting "system" of the stack "fpv".
    public enum FpvSystem: Int32 {
        case auto = 0
        case pal = 1
        case ntsc = 2
    }
    /// System.
    public static func fpvSystem(_ handle: SDRKIT_HANDLE) -> Result<FpvSystem, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "system", &v)
        return code == SDRKIT_OK ? .success(FpvSystem(rawValue: v) ?? FpvSystem(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvSystem(_ handle: SDRKIT_HANDLE, _ value: FpvSystem) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "system", value.rawValue))
    }
    /// Channels.
    public static func fpvChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// The options of the setting "sound_subcarrier" of the stack "fpv".
    public enum FpvSoundSubcarrier: Int32 {
        case auto = 0
        case n60 = 1
        case n65 = 2
    }
    /// Sound subcarrier.
    public static func fpvSoundSubcarrier(_ handle: SDRKIT_HANDLE) -> Result<FpvSoundSubcarrier, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sound_subcarrier", &v)
        return code == SDRKIT_OK ? .success(FpvSoundSubcarrier(rawValue: v) ?? FpvSoundSubcarrier(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvSoundSubcarrier(_ handle: SDRKIT_HANDLE, _ value: FpvSoundSubcarrier) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sound_subcarrier", value.rawValue))
    }
    /// Colour.
    public static func fpvColourOn(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "colour_on", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvColourOn(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "colour_on", value))
    }
    /// Saturation.
    public static func fpvSaturation(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "saturation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvSaturation(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "saturation", value))
    }
    /// The options of the setting "deinterlace" of the stack "fpv".
    public enum FpvDeinterlace: Int32 {
        case weave = 0
        case bob = 1
    }
    /// Fields.
    public static func fpvDeinterlace(_ handle: SDRKIT_HANDLE) -> Result<FpvDeinterlace, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "deinterlace", &v)
        return code == SDRKIT_OK ? .success(FpvDeinterlace(rawValue: v) ?? FpvDeinterlace(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvDeinterlace(_ handle: SDRKIT_HANDLE, _ value: FpvDeinterlace) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "deinterlace", value.rawValue))
    }
    /// Audio.
    public static func fpvAudio(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "audio", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvAudio(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "audio", value))
    }
    /// The options of the setting "tx_system" of the stack "fpv".
    public enum FpvTxSystem: Int32 {
        case pal = 0
        case ntsc = 1
    }
    /// System.
    public static func fpvTxSystem(_ handle: SDRKIT_HANDLE) -> Result<FpvTxSystem, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_system", &v)
        return code == SDRKIT_OK ? .success(FpvTxSystem(rawValue: v) ?? FpvTxSystem(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvTxSystem(_ handle: SDRKIT_HANDLE, _ value: FpvTxSystem) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_system", value.rawValue))
    }
    /// The options of the setting "tx_picture" of the stack "fpv".
    public enum FpvTxPicture: Int32 {
        case card = 0
        case bars = 1
    }
    /// Test picture.
    public static func fpvTxPicture(_ handle: SDRKIT_HANDLE) -> Result<FpvTxPicture, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_picture", &v)
        return code == SDRKIT_OK ? .success(FpvTxPicture(rawValue: v) ?? FpvTxPicture(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvTxPicture(_ handle: SDRKIT_HANDLE, _ value: FpvTxPicture) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_picture", value.rawValue))
    }
    /// Sound.
    public static func fpvTxSound(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_sound", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvTxSound(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_sound", value))
    }
    /// The options of the setting "tx_subcarrier" of the stack "fpv".
    public enum FpvTxSubcarrier: Int32 {
        case n65 = 0
        case n60 = 1
    }
    /// Sound subcarrier.
    public static func fpvTxSubcarrier(_ handle: SDRKIT_HANDLE) -> Result<FpvTxSubcarrier, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_subcarrier", &v)
        return code == SDRKIT_OK ? .success(FpvTxSubcarrier(rawValue: v) ?? FpvTxSubcarrier(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvTxSubcarrier(_ handle: SDRKIT_HANDLE, _ value: FpvTxSubcarrier) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_subcarrier", value.rawValue))
    }
    /// Deviation.
    public static func fpvTxDeviationMhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_deviation_mhz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvTxDeviationMhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_deviation_mhz", value))
    }
    /// Sync tip at the highest frequency.
    public static func fpvTxSyncHigh(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_sync_high", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvTxSyncHigh(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_sync_high", value))
    }
    /// AC coupling time constant.
    public static func fpvTxAcMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_ac_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvTxAcMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_ac_ms", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum AnalogtvGeneratorSettings {
    /// The options of the setting "gen_system" of the stack "atv".
    public enum AtvSystem: Int32 {
        case g = 0
        case b = 1
        case i = 2
        case dk = 3
        case m = 4
        case n = 5
        case a = 6
        case c = 7
        case e = 8
        case h = 9
        case kp = 10
        case l = 11
        case lp = 12
        case nbtvAm = 13
        case nbtvFm = 14
    }
    /// System.
    public static func atvSystem(_ handle: SDRKIT_HANDLE) -> Result<AtvSystem, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_system", &v)
        return code == SDRKIT_OK ? .success(AtvSystem(rawValue: v) ?? AtvSystem(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvSystem(_ handle: SDRKIT_HANDLE, _ value: AtvSystem) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_system", value.rawValue))
    }
    /// The options of the setting "gen_colour" of the stack "atv".
    public enum AtvColour: Int32 {
        case system = 0
        case pal = 1
        case ntsc = 2
        case secam = 3
        case none = 4
    }
    /// Colour.
    public static func atvColour(_ handle: SDRKIT_HANDLE) -> Result<AtvColour, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_colour", &v)
        return code == SDRKIT_OK ? .success(AtvColour(rawValue: v) ?? AtvColour(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvColour(_ handle: SDRKIT_HANDLE, _ value: AtvColour) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_colour", value.rawValue))
    }
    /// The options of the setting "gen_picture" of the stack "atv".
    public enum AtvPicture: Int32 {
        case card = 0
        case bars = 1
        case ramp = 2
    }
    /// Picture.
    public static func atvPicture(_ handle: SDRKIT_HANDLE) -> Result<AtvPicture, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_picture", &v)
        return code == SDRKIT_OK ? .success(AtvPicture(rawValue: v) ?? AtvPicture(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvPicture(_ handle: SDRKIT_HANDLE, _ value: AtvPicture) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_picture", value.rawValue))
    }
    /// The options of the setting "gen_sound" of the stack "atv".
    public enum AtvSound: Int32 {
        case gaps = 0
        case melody = 1
        case carrier = 2
        case none = 3
        case tone = 4
    }
    /// Sound.
    public static func atvSound(_ handle: SDRKIT_HANDLE) -> Result<AtvSound, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_sound", &v)
        return code == SDRKIT_OK ? .success(AtvSound(rawValue: v) ?? AtvSound(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvSound(_ handle: SDRKIT_HANDLE, _ value: AtvSound) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_sound", value.rawValue))
    }
    /// The options of the setting "gen_second" of the stack "atv".
    public enum AtvSecond: Int32 {
        case none = 0
        case a2Stereo = 1
        case a2Dual = 2
        case a2Mono = 3
        case nicamStereo = 4
        case nicamDual = 5
    }
    /// Second sound.
    public static func atvSecond(_ handle: SDRKIT_HANDLE) -> Result<AtvSecond, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_second", &v)
        return code == SDRKIT_OK ? .success(AtvSecond(rawValue: v) ?? AtvSecond(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvSecond(_ handle: SDRKIT_HANDLE, _ value: AtvSecond) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_second", value.rawValue))
    }
    /// Sync compression.
    public static func atvCompression(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_compression", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvCompression(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_compression", value))
    }
    /// Group-delay pre-correction.
    public static func atvGroupDelay(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_group_delay", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvGroupDelay(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_group_delay", value))
    }
    /// Nyquist-slope transmitter.
    public static func atvNyquist(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_nyquist", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvNyquist(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_nyquist", value))
    }
    /// The options of the setting "gen_setup" of the stack "atv".
    public enum AtvSetup: Int32 {
        case n75Ire = 0
        case none = 1
    }
    /// Black setup in 525 lines.
    public static func atvSetup(_ handle: SDRKIT_HANDLE) -> Result<AtvSetup, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_setup", &v)
        return code == SDRKIT_OK ? .success(AtvSetup(rawValue: v) ?? AtvSetup(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvSetup(_ handle: SDRKIT_HANDLE, _ value: AtvSetup) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_setup", value.rawValue))
    }
    /// The options of the setting "gen_secam_id" of the stack "atv".
    public enum AtvSecamId: Int32 {
        case both = 0
        case porch = 1
        case lines = 2
    }
    /// SECAM identification.
    public static func atvSecamId(_ handle: SDRKIT_HANDLE) -> Result<AtvSecamId, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_secam_id", &v)
        return code == SDRKIT_OK ? .success(AtvSecamId(rawValue: v) ?? AtvSecamId(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvSecamId(_ handle: SDRKIT_HANDLE, _ value: AtvSecamId) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_secam_id", value.rawValue))
    }
    /// Ident text.
    public static func atvIdent(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_ident", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvIdent(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_ident", $0) })
    }
    /// Teletext.
    public static func atvTeletext(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_teletext", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvTeletext(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_teletext", value))
    }
    /// Clock start time.
    public static func atvStart(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_start", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvStart(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_start", value))
    }
    /// NBTV lines.
    public static func atvNbtvLines(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_nbtv_lines", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvNbtvLines(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_nbtv_lines", value))
    }
    /// NBTV pictures/s.
    public static func atvNbtvRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_nbtv_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvNbtvRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_nbtv_rate", value))
    }
    /// NBTV picture.
    public static func atvNbtvPattern(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_nbtv_pattern", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtvNbtvPattern(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_nbtv_pattern", value))
    }
    /// The options of the setting "gen_variant" of the stack "mac".
    public enum MacVariant: Int32 {
        case d2 = 0
        case d = 1
    }
    /// Variant.
    public static func macVariant(_ handle: SDRKIT_HANDLE) -> Result<MacVariant, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_variant", &v)
        return code == SDRKIT_OK ? .success(MacVariant(rawValue: v) ?? MacVariant(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMacVariant(_ handle: SDRKIT_HANDLE, _ value: MacVariant) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_variant", value.rawValue))
    }
    /// Scrambled (EuroCrypt).
    public static func macScrambled(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_scrambled", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMacScrambled(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_scrambled", value))
    }
    /// 16:9.
    public static func macWide(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_wide", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMacWide(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_wide", value))
    }
    /// The options of the setting "gen_picture" of the stack "mac".
    public enum MacPicture: Int32 {
        case card = 0
        case bars = 1
        case ramp = 2
    }
    /// Picture.
    public static func macPicture(_ handle: SDRKIT_HANDLE) -> Result<MacPicture, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_picture", &v)
        return code == SDRKIT_OK ? .success(MacPicture(rawValue: v) ?? MacPicture(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMacPicture(_ handle: SDRKIT_HANDLE, _ value: MacPicture) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_picture", value.rawValue))
    }
    /// Tone.
    public static func macTone(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tone", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMacTone(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tone", value))
    }
    /// Channel name.
    public static func macChannel(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_channel", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMacChannel(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_channel", $0) })
    }
    /// Network name.
    public static func macNetwork(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_network", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMacNetwork(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_network", $0) })
    }
    /// Country.
    public static func macCountry(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_country", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMacCountry(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_country", $0) })
    }
    /// The options of the setting "gen_system" of the stack "fpv".
    public enum FpvSystem: Int32 {
        case pal = 0
        case ntsc = 1
    }
    /// System.
    public static func fpvSystem(_ handle: SDRKIT_HANDLE) -> Result<FpvSystem, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_system", &v)
        return code == SDRKIT_OK ? .success(FpvSystem(rawValue: v) ?? FpvSystem(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvSystem(_ handle: SDRKIT_HANDLE, _ value: FpvSystem) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_system", value.rawValue))
    }
    /// The options of the setting "gen_picture" of the stack "fpv".
    public enum FpvPicture: Int32 {
        case card = 0
        case bars = 1
    }
    /// Picture.
    public static func fpvPicture(_ handle: SDRKIT_HANDLE) -> Result<FpvPicture, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_picture", &v)
        return code == SDRKIT_OK ? .success(FpvPicture(rawValue: v) ?? FpvPicture(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvPicture(_ handle: SDRKIT_HANDLE, _ value: FpvPicture) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_picture", value.rawValue))
    }
    /// The options of the setting "gen_sound" of the stack "fpv".
    public enum FpvSound: Int32 {
        case n65 = 0
        case n60 = 1
        case none = 2
    }
    /// Sound.
    public static func fpvSound(_ handle: SDRKIT_HANDLE) -> Result<FpvSound, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_sound", &v)
        return code == SDRKIT_OK ? .success(FpvSound(rawValue: v) ?? FpvSound(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvSound(_ handle: SDRKIT_HANDLE, _ value: FpvSound) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_sound", value.rawValue))
    }
    /// No transmitter.
    public static func fpvNone(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_none", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvNone(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_none", value))
    }
    /// Capture centre from the transmitter.
    public static func fpvCentreMhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_mhz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvCentreMhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_mhz", value))
    }
    /// Deviation.
    public static func fpvDeviationMhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation_mhz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvDeviationMhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation_mhz", value))
    }
    /// Sync tip at the highest frequency.
    public static func fpvSyncHigh(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_sync_high", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvSyncHigh(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_sync_high", value))
    }
    /// AC coupling time constant.
    public static func fpvAcMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_ac_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFpvAcMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_ac_ms", value))
    }
}
