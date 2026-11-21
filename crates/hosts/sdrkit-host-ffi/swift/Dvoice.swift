// sdrkit: the typed interface of the stack family "dvoice", version 1.4. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum DvoiceInterface {
    public static let major = 1
    public static let minor = 4
}

/// The measurements that every digital voice stack publishes.
public struct DvoiceSharedFields {
    public enum Verdict: Int32 {
        case noInput = 0
        case noSync = 1
        case identity = 2
        case fec = 3
        case marginal = 4
        case decoding = 5
    }
    /// The channel level.
    public var levelDbfs: Double
    /// The noise floor.
    public var noiseDbfs: Double
    /// The carrier to noise ratio of the channel.
    public var cnrDb: Double?
    /// The signal to noise ratio on the symbols.
    public var snrDb: Double?
    /// The carrier offset.
    public var offsetHz: Double
    /// The measured deviation.
    public var deviationHz: Double
    /// The symbol clock error.
    public var clockPpm: Double
    /// The bit error rate.
    public var ber: Double
    /// The signal is inverted.
    public var inverted: Bool
    /// Blocks that passed their error check.
    public var blocksOk: Int64
    /// Blocks that failed their error check.
    public var blocksBad: Int64
    /// Frames whose identity field was read.
    public var frames: Int64
    /// Frame syncs found.
    public var syncs: Int64
    /// Reports written.
    public var reports: Int64
    /// Why the stack decodes or does not.
    public var verdict: Verdict?

    init(_ c: sdrkit_dvoice_shared_fields) {
        levelDbfs = c.level_dbfs
        noiseDbfs = c.noise_dbfs
        cnrDb = c.has_cnr_db ? c.cnr_db : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.offset_hz
        deviationHz = c.deviation_hz
        clockPpm = c.clock_ppm
        ber = c.ber
        inverted = c.inverted
        blocksOk = c.blocks_ok
        blocksBad = c.blocks_bad
        frames = c.frames
        syncs = c.syncs
        reports = c.reports
        verdict = Verdict(rawValue: c.verdict)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvoiceSharedFields, SDRKitCode> {
        var c = sdrkit_dvoice_shared_fields()
        let code = sdrkit_dvoice_shared_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvoiceSharedFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of DMR.
public struct DmrFields {
    public enum Link: Int32 {
        case baseStation = 0
        case mobile = 1
        case directMode = 2
    }
    public enum CachActivity1: Int32 {
        case none = 0
        case groupCsbk = 1
        case privateCsbk = 2
        case groupVoice = 3
        case privateVoice = 4
        case privateData = 5
        case groupData = 6
        case emergencyGroupVoice = 7
        case emergencyPrivateVoice = 8
        case other = 9
    }
    public enum CachActivity2: Int32 {
        case none = 0
        case groupCsbk = 1
        case privateCsbk = 2
        case groupVoice = 3
        case privateVoice = 4
        case privateData = 5
        case groupData = 6
        case emergencyGroupVoice = 7
        case emergencyPrivateVoice = 8
        case other = 9
    }
    public enum CachActivityLast1: Int32 {
        case none = 0
        case groupCsbk = 1
        case privateCsbk = 2
        case groupVoice = 3
        case privateVoice = 4
        case privateData = 5
        case groupData = 6
        case emergencyGroupVoice = 7
        case emergencyPrivateVoice = 8
        case other = 9
    }
    public enum CachActivityLast2: Int32 {
        case none = 0
        case groupCsbk = 1
        case privateCsbk = 2
        case groupVoice = 3
        case privateVoice = 4
        case privateData = 5
        case groupData = 6
        case emergencyGroupVoice = 7
        case emergencyPrivateVoice = 8
        case other = 9
    }
    /// Colour code.
    public var colourCode: Int64?
    /// Link.
    public var link: Link?
    /// Slots locked.
    public var slotsLocked: Int64
    /// Activity, slot 1.
    public var cachActivity1: CachActivity1?
    /// Activity, slot 2.
    public var cachActivity2: CachActivity2?
    /// Last activity, slot 1.
    public var cachActivityLast1: CachActivityLast1?
    /// Last activity, slot 2.
    public var cachActivityLast2: CachActivityLast2?
    /// Talker alias, slot 1.
    public var talkerAlias1: String
    /// Talker alias, slot 2.
    public var talkerAlias2: String

    init(_ c: sdrkit_dmr_fields) {
        colourCode = c.has_colour_code ? c.colour_code : nil
        link = c.has_link ? Link(rawValue: c.link) : nil
        slotsLocked = c.slots_locked
        cachActivity1 = c.has_cach_activity_1 ? CachActivity1(rawValue: c.cach_activity_1) : nil
        cachActivity2 = c.has_cach_activity_2 ? CachActivity2(rawValue: c.cach_activity_2) : nil
        cachActivityLast1 = c.has_cach_activity_last_1 ? CachActivityLast1(rawValue: c.cach_activity_last_1) : nil
        cachActivityLast2 = c.has_cach_activity_last_2 ? CachActivityLast2(rawValue: c.cach_activity_last_2) : nil
        talkerAlias1 = sdrkitText(c.talker_alias_1)
        talkerAlias2 = sdrkitText(c.talker_alias_2)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DmrFields, SDRKitCode> {
        var c = sdrkit_dmr_fields()
        let code = sdrkit_dmr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DmrFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the voice monitor.
public struct DvoiceMonitorFields {
    /// The call that plays now: a row key of the `calls` table.
    public var playing: String?
    /// The target held.
    public var held: String?
    /// What the codec plug-ins say: a codec that is not installed or was turned off.
    public var codec: String?

    init(_ c: sdrkit_dvoice_monitor_fields) {
        playing = c.has_playing ? sdrkitText(c.playing) : nil
        held = c.has_held ? sdrkitText(c.held) : nil
        codec = c.has_codec ? sdrkitText(c.codec) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvoiceMonitorFields, SDRKitCode> {
        var c = sdrkit_dvoice_monitor_fields()
        let code = sdrkit_dvoice_monitor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvoiceMonitorFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct DvoiceDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_dvoice_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvoiceDecoderFields, SDRKitCode> {
        var c = sdrkit_dvoice_decoder_fields()
        let code = sdrkit_dvoice_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvoiceDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of a sender that sends text only, for want of a speech encoder: DMR.
public struct DmrDataTxFields {
    public enum TxKind: Int32 {
        case text = 0
    }
    /// Whether a transmission is under way.
    public var transmitting: Bool
    /// The transmit frequency: the listening frequency plus the transmit offset. No value while the stack does not transmit.
    public var txFreqHz: Double?
    /// What the transmission sends, or the last one sent.
    public var txKind: TxKind?
    /// The bursts or frames of the transmission, or of the last one, that are written.
    public var txFrames: Int64
    /// What the sender sends, and why it sends no voice.
    public var txNote: String

    init(_ c: sdrkit_dmr_data_tx_fields) {
        transmitting = c.transmitting
        txFreqHz = c.has_tx_freq_hz ? c.tx_freq_hz : nil
        txKind = c.has_tx_kind ? TxKind(rawValue: c.tx_kind) : nil
        txFrames = c.tx_frames
        txNote = sdrkitText(c.tx_note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DmrDataTxFields, SDRKitCode> {
        var c = sdrkit_dmr_data_tx_fields()
        let code = sdrkit_dmr_data_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DmrDataTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the call log. The row key is the call id.
public struct DvoiceCallRow {
    public enum Kind: Int32 {
        case voice = 0
        case groupVoice = 1
        case privateVoice = 2
        case allCall = 3
        case data = 4
        case packet = 5
        case status = 6
        case shortMessage = 7
        case shortData = 8
        case control = 9
        case grant = 10
    }
    public enum AccessKind: Int32 {
        case colourCode = 0
        case nac = 1
        case ran = 2
        case can = 3
        case squelchCode = 4
    }
    public enum Encryption: Int32 {
        case unknown = 0
        case clear = 1
        case encrypted = 2
        case decrypted = 3
    }
    public enum End: Int32 {
        case active = 0
        case terminator = 1
        case hangTime = 2
        case replaced = 3
        case closed = 4
        case single = 5
    }
    /// The row key.
    public var key: String
    /// The id of a call.
    public var id: Int64
    /// Signal time of the start of a call, seconds.
    public var startS: Double
    /// How long a call lasted.
    public var duration: Double
    /// The time slot, 0 when the stack has none.
    public var slot: Int64
    /// What a transmission carries.
    public var kind: Kind?
    /// Who sends.
    public var src: String
    /// Who is called.
    public var dst: String
    /// The target is a group.
    public var group: Bool
    /// An emergency call.
    public var emergency: Bool
    /// Which access code the call carried.
    public var accessKind: AccessKind?
    /// The value of the access code.
    public var accessCode: Int64?
    /// Encrypted or not.
    public var encryption: Encryption?
    /// The name of the algorithm.
    public var algorithm: String
    /// The algorithm id as sent.
    public var algId: Int64?
    /// The key id.
    public var encKeyId: Int64?
    /// The talker alias.
    public var alias: String
    /// Text carried by a transmission.
    public var text: String
    /// Voice frames received.
    public var voiceFrames: Int64
    /// Voice frames passed to the media output.
    public var mediaFrames: Int64
    /// The bit error rate of a call.
    public var ber: Double
    /// The call is on air.
    public var active: Bool
    /// The call started from an identity inside it.
    public var lateEntry: Bool
    /// The call ended by its terminator.
    public var endSeen: Bool
    /// The call ended after the hang time.
    public var signalLost: Bool
    /// The carrier frequency.
    public var carrierHz: Double
    /// Where a call is in its life.
    public var end: End?

    init(_ c: sdrkit_dvoice_call_row) {
        key = sdrkitText(c.key)
        id = c.id
        startS = c.start_s
        duration = c.duration
        slot = c.slot
        kind = Kind(rawValue: c.kind)
        src = sdrkitText(c.src)
        dst = sdrkitText(c.dst)
        group = c.group
        emergency = c.emergency
        accessKind = c.has_access_kind ? AccessKind(rawValue: c.access_kind) : nil
        accessCode = c.has_access_code ? c.access_code : nil
        encryption = Encryption(rawValue: c.encryption)
        algorithm = sdrkitText(c.algorithm)
        algId = c.has_alg_id ? c.alg_id : nil
        encKeyId = c.has_enc_key_id ? c.enc_key_id : nil
        alias = sdrkitText(c.alias)
        text = sdrkitText(c.text)
        voiceFrames = c.voice_frames
        mediaFrames = c.media_frames
        ber = c.ber
        active = c.active
        lateEntry = c.late_entry
        endSeen = c.end_seen
        signalLost = c.signal_lost
        carrierHz = c.carrier_hz
        end = End(rawValue: c.end)
    }
}

/// A fact of a call that only some stacks send. The row key is the call id, a slash and the detail key.
public struct DvoiceCallDetailRow {
    public enum Detail: Int32 {
        case baud = 0
        case callType = 1
        case commMode = 2
        case status = 3
        case channel = 4
        case freqHz = 5
        case sap = 6
        case blocks = 7
        case my = 8
        case suffix = 9
        case ur = 10
        case rpt1 = 11
        case rpt2 = 12
        case flag1 = 13
        case flags = 14
        case headerOk = 15
        case headerFromSlowData = 16
        case squelch = 17
        case position = 18
        case streamType = 19
        case encryptionType = 20
        case lsfFromLich = 21
        case signed = 22
        case data = 23
        case frames = 24
        case `protocol` = 25
        case fid = 26
        case serviceOptions = 27
        case note = 28
        case opcode = 29
        case format = 30
        case crcOk = 31
        case dpf = 32
        case downlink = 33
        case uplink = 34
        case dataType = 35
        case callId = 36
        case address = 37
        case mcc = 38
        case mnc = 39
    }
    /// The row key.
    public var key: String
    /// The call a row belongs to: a row key of the `calls` table.
    public var call: String
    /// Which fact a detail row holds.
    public var detail: Detail?
    /// The value of a detail.
    public var value: SDRKitValue

    init(_ c: sdrkit_dvoice_call_detail_row) {
        key = sdrkitText(c.key)
        call = sdrkitText(c.call)
        detail = Detail(rawValue: c.detail)
        value = SDRKitValue(c.value)
    }
}

/// A text message. The row key counts the messages.
public struct DvoiceMessageRow {
    public enum Format: Int32 {
        case shortData = 0
        case packetData = 1
        case slowData = 2
        case sms = 3
        case metaText = 4
        case freedvText = 5
        case ysfText = 6
        case shortMessage = 7
        case packet = 8
    }
    /// The row key.
    public var key: String
    /// Signal time, seconds.
    public var timeS: Double
    /// Who sends.
    public var src: String
    /// Who is called.
    public var dst: String
    /// The target is a group.
    public var group: Bool
    /// How a message was sent.
    public var format: Format?
    /// The DMR short data format code.
    public var dataFormat: Int64?
    /// Text carried by a transmission.
    public var text: String
    /// The error check was good.
    public var crcOk: Bool
    /// The time slot, 0 when the stack has none.
    public var slot: Int64
    /// An SMS.
    public var sms: Bool

    init(_ c: sdrkit_dvoice_message_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        src = sdrkitText(c.src)
        dst = sdrkitText(c.dst)
        group = c.group
        format = Format(rawValue: c.format)
        dataFormat = c.has_data_format ? c.data_format : nil
        text = sdrkitText(c.text)
        crcOk = c.crc_ok
        slot = c.slot
        sms = c.sms
    }
}

/// A position. The row key counts the positions.
public struct DvoicePositionRow {
    public enum Format: Int32 {
        case lrrp = 0
        case nmea = 1
        case dprs = 2
        case m17Gnss = 3
        case ysfGps = 4
        case dmrGps = 5
    }
    public enum StationType: Int32 {
        case fixed = 0
        case mobile = 1
        case handheld = 2
        case other = 3
    }
    /// The row key.
    public var key: String
    /// Signal time, seconds.
    public var timeS: Double
    /// Who sends.
    public var src: String
    /// The position, latitude first.
    public var position: SDRKitPosition
    /// The height.
    public var heightM: Double?
    /// The speed.
    public var speedKt: Double?
    /// The course.
    public var courseDeg: Double?
    /// The accuracy of a position.
    public var accuracyM: Double?
    /// How a position was sent.
    public var format: Format?
    /// The comment sent with a position.
    public var comment: String
    /// The kind of station that sent a position.
    public var stationType: StationType?
    /// The data source code of a position.
    public var dataSource: Int64?

    init(_ c: sdrkit_dvoice_position_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        src = sdrkitText(c.src)
        position = SDRKitPosition(c.position)
        heightM = c.has_height_m ? c.height_m : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        accuracyM = c.has_accuracy_m ? c.accuracy_m : nil
        format = Format(rawValue: c.format)
        comment = sdrkitText(c.comment)
        stationType = c.has_station_type ? StationType(rawValue: c.station_type) : nil
        dataSource = c.has_data_source ? c.data_source : nil
    }
}

/// The site and system identity of the stack. The row key is the stack id.
public struct DvoiceSiteRow {
    /// The row key.
    public var key: String
    /// Signal time, seconds.
    public var timeS: Double
    /// The identity is complete.
    public var valid: Bool
    /// The system id.
    public var system: Int64?
    /// The wide area communications network.
    public var wacn: Int64?
    /// The RF subsystem.
    public var rfss: Int64?
    /// The site.
    public var site: Int64?
    /// The network access code.
    public var nac: Int64?
    /// The control channel.
    public var controlChannel: String?
    /// The location id.
    public var location: Int64?
    /// The radio access number.
    public var ran: Int64?
    /// The country code.
    public var country: Int64?
    /// The network code.
    public var network: Int64?
    /// The base station.
    public var baseStation: Int64?
    /// The channel of a site.
    public var channel: Int64?
    /// The colour code of a cell.
    public var colourCode: Int64?
    /// The location area of a cell.
    public var locationArea: Int64?
    /// The main carrier number of a cell.
    public var mainCarrier: Int64?
    /// The frequency band of a cell.
    public var band: Int64?
    /// The main carrier frequency of a cell.
    public var mainCarrierHz: Double?

    init(_ c: sdrkit_dvoice_site_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        valid = c.valid
        system = c.has_system ? c.system : nil
        wacn = c.has_wacn ? c.wacn : nil
        rfss = c.has_rfss ? c.rfss : nil
        site = c.has_site ? c.site : nil
        nac = c.has_nac ? c.nac : nil
        controlChannel = c.has_control_channel ? sdrkitText(c.control_channel) : nil
        location = c.has_location ? c.location : nil
        ran = c.has_ran ? c.ran : nil
        country = c.has_country ? c.country : nil
        network = c.has_network ? c.network : nil
        baseStation = c.has_base_station ? c.base_station : nil
        channel = c.has_channel ? c.channel : nil
        colourCode = c.has_colour_code ? c.colour_code : nil
        locationArea = c.has_location_area ? c.location_area : nil
        mainCarrier = c.has_main_carrier ? c.main_carrier : nil
        band = c.has_band ? c.band : nil
        mainCarrierHz = c.has_main_carrier_hz ? c.main_carrier_hz : nil
    }
}

/// A channel grant. The row key counts the grants.
public struct DvoiceGrantRow {
    /// The row key.
    public var key: String
    /// Signal time, seconds.
    public var timeS: Double
    /// The talkgroup or radio granted.
    public var target: Int64
    /// The target is a group.
    public var group: Bool
    /// The source of a grant request.
    public var sourceId: Int64
    /// The channel number of a grant as sent.
    public var channelNumber: Int64
    /// The channel of a grant for people.
    public var channel: String
    /// The frequency of a grant.
    public var freqHz: Double?
    /// The time slot, 0 when the stack has none.
    public var slot: Int64?
    /// An emergency call.
    public var emergency: Bool
    /// The call is encrypted.
    public var encrypted: Bool
    /// The priority of a grant.
    public var priority: Int64
    /// An update of a running call.
    public var update: Bool

    init(_ c: sdrkit_dvoice_grant_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        target = c.target
        group = c.group
        sourceId = c.source_id
        channelNumber = c.channel_number
        channel = sdrkitText(c.channel)
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        slot = c.has_slot ? c.slot : nil
        emergency = c.emergency
        encrypted = c.encrypted
        priority = c.priority
        update = c.update
    }
}

/// A talkgroup heard, with its counters. The row key is the talkgroup.
public struct DvoiceTalkgroupRow {
    /// The row key.
    public var key: String
    /// A talkgroup.
    public var talkgroup: Int64
    /// Grants to a talkgroup.
    public var grants: Int64
    /// Voice calls to a talkgroup.
    public var calls: Int64
    /// The call is encrypted.
    public var encrypted: Bool
    /// When it was last heard.
    public var lastHeard: Double?

    init(_ c: sdrkit_dvoice_talkgroup_row) {
        key = sdrkitText(c.key)
        talkgroup = c.talkgroup
        grants = c.grants
        calls = c.calls
        encrypted = c.encrypted
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A call the monitor heard. The row key is the call id.
public struct DvoiceHistoryRow {
    public enum Verdict: Int32 {
        case play = 0
        case busy = 1
        case lowerPriority = 2
        case muted = 3
        case filtered = 4
        case notHeld = 5
        case squelched = 6
        case encrypted = 7
        case noCodec = 8
    }
    /// The row key.
    public var key: String
    /// The call a row belongs to: a row key of the `calls` table.
    public var call: String
    /// Signal time of the start of a call, seconds.
    public var startS: Double
    /// Who sends.
    public var src: String
    /// Who is called.
    public var dst: String
    /// Who is called and its name.
    public var label: String
    /// Why a call is or is not heard.
    public var verdict: Verdict?
    /// Frames played.
    public var playedFrames: Int64
    /// The call is encrypted.
    public var encrypted: Bool
    /// The call is on air.
    public var active: Bool

    init(_ c: sdrkit_dvoice_history_row) {
        key = sdrkitText(c.key)
        call = sdrkitText(c.call)
        startS = c.start_s
        src = sdrkitText(c.src)
        dst = sdrkitText(c.dst)
        label = sdrkitText(c.label)
        verdict = Verdict(rawValue: c.verdict)
        playedFrames = c.played_frames
        encrypted = c.encrypted
        active = c.active
    }
}

/// A source radio or callsign. The row key is the source.
public struct DvoiceHeardRow {
    /// The row key.
    public var key: String
    /// Who sends.
    public var src: String
    /// Calls heard.
    public var calls: Int64
    /// Time on air.
    public var airtime: Double
    /// Signal time of the end of the last call, seconds.
    public var lastS: Double
    /// The mean bit error rate.
    public var meanBer: Double
    /// An emergency call.
    public var emergency: Bool

    init(_ c: sdrkit_dvoice_heard_row) {
        key = sdrkitText(c.key)
        src = sdrkitText(c.src)
        calls = c.calls
        airtime = c.airtime
        lastS = c.last_s
        meanBer = c.mean_ber
        emergency = c.emergency
    }
}

/// The result fields of P25 phase 1.
public struct P25Fields {
    /// The network access code heard.
    public var nac: Int64?
    /// Frames.
    public var p25Frames: Int64

    init(_ c: sdrkit_p25_fields) {
        nac = c.has_nac ? c.nac : nil
        p25Frames = c.p25_frames
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<P25Fields, SDRKitCode> {
        var c = sdrkit_p25_fields()
        let code = sdrkit_p25_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(P25Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A channel identifier. The row key is the identifier.
public struct DvoiceIdentifierRow {
    /// The row key.
    public var key: String
    /// Identifier.
    public var id: Int64
    /// VHF or UHF.
    public var vhfUhf: Bool
    /// Base.
    public var baseHz: Double
    /// Spacing.
    public var spacingHz: Double
    /// Transmit offset.
    public var txOffsetHz: Double
    /// Bandwidth.
    public var bandwidthHz: Double

    init(_ c: sdrkit_dvoice_identifier_row) {
        key = sdrkitText(c.key)
        id = c.id
        vhfUhf = c.vhf_uhf
        baseHz = c.base_hz
        spacingHz = c.spacing_hz
        txOffsetHz = c.tx_offset_hz
        bandwidthHz = c.bandwidth_hz
    }
}

/// An adjacent site. The row key is the system, the RFSS and the site.
public struct DvoiceAdjacentRow {
    /// The row key.
    public var key: String
    /// The system id.
    public var system: Int64
    /// The RF subsystem.
    public var rfss: Int64
    /// The site.
    public var site: Int64
    /// The channel of a grant for people.
    public var channel: String
    /// The frequency of a grant.
    public var freqHz: Double?

    init(_ c: sdrkit_dvoice_adjacent_row) {
        key = sdrkitText(c.key)
        system = c.system
        rfss = c.rfss
        site = c.site
        channel = sdrkitText(c.channel)
        freqHz = c.has_freq_hz ? c.freq_hz : nil
    }
}

/// The result fields of M17.
public struct M17Fields {
    /// Link setup frames.
    public var lsfOk: Int64
    /// Stream frames.
    public var streamFrames: Int64
    /// Packets.
    public var packetsOk: Int64

    init(_ c: sdrkit_m17_fields) {
        lsfOk = c.lsf_ok
        streamFrames = c.stream_frames
        packetsOk = c.packets_ok
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<M17Fields, SDRKitCode> {
        var c = sdrkit_m17_fields()
        let code = sdrkit_m17_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(M17Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the M17 sender.
public struct M17TxFields {
    public enum TxKind: Int32 {
        case voice = 0
        case packet = 1
    }
    /// Whether a transmission is under way.
    public var transmitting: Bool
    /// The transmit frequency: the listening frequency plus the transmit offset. No value while the stack does not transmit.
    public var txFreqHz: Double?
    /// What the transmission sends, or the last one sent.
    public var txKind: TxKind?
    /// The stream or packet frames of the transmission, or of the last one, that are written.
    public var txFrames: Int64

    init(_ c: sdrkit_m17_tx_fields) {
        transmitting = c.transmitting
        txFreqHz = c.has_tx_freq_hz ? c.tx_freq_hz : nil
        txKind = c.has_tx_kind ? TxKind(rawValue: c.tx_kind) : nil
        txFrames = c.tx_frames
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<M17TxFields, SDRKitCode> {
        var c = sdrkit_m17_tx_fields()
        let code = sdrkit_m17_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(M17TxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of D-STAR.
public struct DstarFields {
    /// Headers.
    public var headersOk: Int64
    /// Voice frames.
    public var dstarVoiceFrames: Int64
    /// Inverted sync.
    public var syncInverted: Bool

    init(_ c: sdrkit_dstar_fields) {
        headersOk = c.headers_ok
        dstarVoiceFrames = c.dstar_voice_frames
        syncInverted = c.sync_inverted
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DstarFields, SDRKitCode> {
        var c = sdrkit_dstar_fields()
        let code = sdrkit_dstar_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DstarFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the D-STAR sender.
public struct DstarTxFields {
    public enum TxKind: Int32 {
        case text = 0
    }
    /// Whether a transmission is under way.
    public var transmitting: Bool
    /// The transmit frequency: the listening frequency plus the transmit offset. No value while the stack does not transmit.
    public var txFreqHz: Double?
    /// What the transmission sends, or the last one sent.
    public var txKind: TxKind?
    /// The voice frames of the transmission, or of the last one, that are written.
    public var txFrames: Int64
    /// What the sender puts in the voice frames.
    public var txNote: String

    init(_ c: sdrkit_dstar_tx_fields) {
        transmitting = c.transmitting
        txFreqHz = c.has_tx_freq_hz ? c.tx_freq_hz : nil
        txKind = c.has_tx_kind ? TxKind(rawValue: c.tx_kind) : nil
        txFrames = c.tx_frames
        txNote = sdrkitText(c.tx_note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DstarTxFields, SDRKitCode> {
        var c = sdrkit_dstar_tx_fields()
        let code = sdrkit_dstar_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DstarTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of NXDN.
public struct NxdnFields {
    /// RAN.
    public var ran: Int64?
    /// Frames.
    public var nxdnFrames: Int64
    /// CRC good.
    public var crcOk: Int64
    /// CRC bad.
    public var crcBad: Int64

    init(_ c: sdrkit_nxdn_fields) {
        ran = c.has_ran ? c.ran : nil
        nxdnFrames = c.nxdn_frames
        crcOk = c.crc_ok
        crcBad = c.crc_bad
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NxdnFields, SDRKitCode> {
        var c = sdrkit_nxdn_fields()
        let code = sdrkit_nxdn_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NxdnFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of dPMR.
public struct DpmrFields {
    /// Colour code.
    public var colourCode: Int64?

    init(_ c: sdrkit_dpmr_fields) {
        colourCode = c.has_colour_code ? c.colour_code : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DpmrFields, SDRKitCode> {
        var c = sdrkit_dpmr_fields()
        let code = sdrkit_dpmr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DpmrFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of System Fusion.
public struct YsfFields {
    /// Data type.
    public var dataType: String

    init(_ c: sdrkit_ysf_fields) {
        dataType = sdrkitText(c.data_type)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<YsfFields, SDRKitCode> {
        var c = sdrkit_ysf_fields()
        let code = sdrkit_ysf_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(YsfFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of P25 phase 2.
public struct P25p2Fields {
    /// Superframe location.
    public var location: Int64

    init(_ c: sdrkit_p25p2_fields) {
        location = c.location
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<P25p2Fields, SDRKitCode> {
        var c = sdrkit_p25p2_fields()
        let code = sdrkit_p25p2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(P25p2Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of TETRAPOL.
public struct TetrapolFields {
    /// Voice frames.
    public var tetrapolVoiceFrames: Int64
    /// Signalling frames whose CRC checks.
    public var tetrapolSignallingFrames: Int64

    init(_ c: sdrkit_tetrapol_fields) {
        tetrapolVoiceFrames = c.tetrapol_voice_frames
        tetrapolSignallingFrames = c.tetrapol_signalling_frames
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TetrapolFields, SDRKitCode> {
        var c = sdrkit_tetrapol_fields()
        let code = sdrkit_tetrapol_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TetrapolFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of TETRA.
public struct TetraFields {
    /// Bursts.
    public var bursts: Int64

    init(_ c: sdrkit_tetra_fields) {
        bursts = c.bursts
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TetraFields, SDRKitCode> {
        var c = sdrkit_tetra_fields()
        let code = sdrkit_tetra_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TetraFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of TETRA DMO.
public struct TetraDmoFields {
    /// Synchronisation bursts.
    public var dsb: Int64
    /// Normal bursts.
    public var dnb: Int64
    /// SCH/F and STCH good.
    public var dataBlocks: Int64
    /// SCH/S and SCH/H bad.
    public var syncBlocksBad: Int64
    /// Traffic bursts.
    public var trafficBursts: Int64
    /// Traffic refused.
    public var trafficRefused: Int64
    /// Communication type.
    public var commType: Int64?
    /// Frame.
    public var frame: Int64?
    /// Slot.
    public var slot: Int64?
    /// Encryption state.
    public var aieState: Int64?
    /// Last source.
    public var lastSource: String?
    /// Last destination.
    public var lastDestination: String?
    /// MNI.
    public var mni: String?
    /// Note.
    public var note: String

    init(_ c: sdrkit_tetra_dmo_fields) {
        dsb = c.dsb
        dnb = c.dnb
        dataBlocks = c.data_blocks
        syncBlocksBad = c.sync_blocks_bad
        trafficBursts = c.traffic_bursts
        trafficRefused = c.traffic_refused
        commType = c.has_comm_type ? c.comm_type : nil
        frame = c.has_frame ? c.frame : nil
        slot = c.has_slot ? c.slot : nil
        aieState = c.has_aie_state ? c.aie_state : nil
        lastSource = c.has_last_source ? sdrkitText(c.last_source) : nil
        lastDestination = c.has_last_destination ? sdrkitText(c.last_destination) : nil
        mni = c.has_mni ? sdrkitText(c.mni) : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TetraDmoFields, SDRKitCode> {
        var c = sdrkit_tetra_dmo_fields()
        let code = sdrkit_tetra_dmo_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TetraDmoFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of FreeDV.
public struct FreedvFields {
    /// The active waveform: "700D", "700E", "1600", or empty when none is.
    public var submode: String
    /// The sync state of the 700D waveform: "search", "trial" or "synced"; empty when it does not run.
    public var sync700d: String
    /// The sync state of the 700E waveform: "search", "trial" or "synced"; empty when it does not run.
    public var sync700e: String
    /// The sync state of the 1600 waveform: "search" or "synced"; empty when it does not run.
    public var sync1600: String
    /// The active waveform held sync at 3 dB or more for 1.5 s.
    public var confirmed: Bool
    /// The frequency estimate of the active OFDM waveform, relative to the 1500 Hz centre.
    public var freqOffsetHz: Double
    /// The sample clock offset of the active OFDM waveform.
    public var clockOffsetPpm: Double
    /// The last timing metric of the active OFDM waveform (0 to about 1).
    public var timingMetric: Double
    /// Unique word bit errors of the bits of the unique word, since the call started.
    public var uwBerPct: Double
    /// Frames whose LDPC decoder did not satisfy every check, since the call started.
    public var ldpcFailedFrames: Int64
    /// Modem frames decoded with sync.
    public var freedvFrames: Int64
    /// Codec frames sent to the speech output.
    public var speechFrames: Int64
    /// The text line in progress, or the last finished line.
    public var callsign: String

    init(_ c: sdrkit_freedv_fields) {
        submode = sdrkitText(c.submode)
        sync700d = sdrkitText(c.sync_700d)
        sync700e = sdrkitText(c.sync_700e)
        sync1600 = sdrkitText(c.sync_1600)
        confirmed = c.confirmed
        freqOffsetHz = c.freq_offset_hz
        clockOffsetPpm = c.clock_offset_ppm
        timingMetric = c.timing_metric
        uwBerPct = c.uw_ber_pct
        ldpcFailedFrames = c.ldpc_failed_frames
        freedvFrames = c.freedv_frames
        speechFrames = c.speech_frames
        callsign = sdrkitText(c.callsign)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FreedvFields, SDRKitCode> {
        var c = sdrkit_freedv_fields()
        let code = sdrkit_freedv_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FreedvFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the FreeDV sender.
public struct FreedvTxFields {
    public enum TxKind: Int32 {
        case n700D = 0
        case n700E = 1
        case voice1600 = 2
    }
    /// Whether a transmission is under way.
    public var transmitting: Bool
    /// The transmit frequency (the suppressed carrier): the listening frequency plus the transmit offset. No value while the stack does not transmit.
    public var txFreqHz: Double?
    /// What the transmission sends, or the last one sent.
    public var txKind: TxKind?
    /// The modem frames of the transmission, or of the last one, that are written.
    public var txFrames: Int64

    init(_ c: sdrkit_freedv_tx_fields) {
        transmitting = c.transmitting
        txFreqHz = c.has_tx_freq_hz ? c.tx_freq_hz : nil
        txKind = c.has_tx_kind ? TxKind(rawValue: c.tx_kind) : nil
        txFrames = c.tx_frames
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FreedvTxFields, SDRKitCode> {
        var c = sdrkit_freedv_tx_fields()
        let code = sdrkit_freedv_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FreedvTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the automatic detector.
public struct DvoiceAutoFields {
    public enum Detected: Int32 {
        case dmr = 0
        case nxdn = 1
        case dpmr = 2
        case p25 = 3
        case ysf = 4
        case m17 = 5
    }
    /// The stack decoded last.
    public var detected: Detected?

    init(_ c: sdrkit_dvoice_auto_fields) {
        detected = c.has_detected ? Detected(rawValue: c.detected) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvoiceAutoFields, SDRKitCode> {
        var c = sdrkit_dvoice_auto_fields()
        let code = sdrkit_dvoice_auto_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvoiceAutoFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A stack that the detector heard. The row key is the stack id.
public struct DvoiceAutoSeenRow {
    public enum Stack: Int32 {
        case dmr = 0
        case nxdn = 1
        case dpmr = 2
        case p25 = 3
        case ysf = 4
        case m17 = 5
    }
    /// The row key.
    public var key: String
    /// A stack the detector decoded.
    public var stack: Stack?
    /// Blocks decoded.
    public var blocks: Int64
    /// Signal time of the last decoded block, seconds.
    public var lastS: Double

    init(_ c: sdrkit_dvoice_auto_seen_row) {
        key = sdrkitText(c.key)
        stack = Stack(rawValue: c.stack)
        blocks = c.blocks
        lastS = c.last_s
    }
}

/// The result fields of DCR.
public struct DcrFields {
    /// Frames read after a sync.
    public var dcrFrames: Int64
    /// SACCH units that passed their CRC.
    public var sacchOk: Int64
    /// SACCH units that failed their CRC.
    public var sacchBad: Int64
    /// Frames whose RICH parity failed.
    public var richBad: Int64
    /// Call signs read from a PICH.
    public var pichOk: Int64
    /// Data blocks that passed their CRC.
    public var dataOk: Int64
    /// Data blocks that failed their CRC.
    public var dataBad: Int64
    /// Voice frames of 20 ms in frames whose SACCH decoded.
    public var voiceFrames: Int64
    /// Calls opened.
    public var calls: Int64
    /// The bits that the Viterbi decoder repaired, of the coded bits of the blocks that passed their CRC.
    public var berPct: Double
    /// The user code of the last message that counted, 0 to 511.
    public var lastUserCode: Int64?
    /// The last call sign read, 9 digits.
    public var lastCallSign: String?
    /// The plan channel of the tuned frequency, or "not on the plan". No value without a plan.
    public var channel: String?
    /// The plan channels in the span of a scan.
    public var scanChannels: Int64
    /// The decoders of scanned channels that run.
    public var scanDecoders: Int64
    /// The decoders of scanned channels that decode.
    public var scanDecoding: Int64
    /// What the stack decodes of the voice.
    public var note: String

    init(_ c: sdrkit_dcr_fields) {
        dcrFrames = c.dcr_frames
        sacchOk = c.sacch_ok
        sacchBad = c.sacch_bad
        richBad = c.rich_bad
        pichOk = c.pich_ok
        dataOk = c.data_ok
        dataBad = c.data_bad
        voiceFrames = c.voice_frames
        calls = c.calls
        berPct = c.ber_pct
        lastUserCode = c.has_last_user_code ? c.last_user_code : nil
        lastCallSign = c.has_last_call_sign ? sdrkitText(c.last_call_sign) : nil
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        scanChannels = c.scan_channels
        scanDecoders = c.scan_decoders
        scanDecoding = c.scan_decoding
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DcrFields, SDRKitCode> {
        var c = sdrkit_dcr_fields()
        let code = sdrkit_dcr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DcrFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the DCR list: a call, a call end, a call sign, a data block or a message. The row key counts the rows.
public struct DcrEventRow {
    public enum Kind: Int32 {
        case call = 0
        case callEnd = 1
        case callSign = 2
        case data = 3
        case manufacturerMessage = 4
        case idle = 5
        case unknownMessage = 6
    }
    /// The row key.
    public var key: String
    /// Signal time, seconds.
    public var timeS: Double
    /// What the row reports.
    public var kind: Kind?
    /// Who: "UC n", a call sign, "TCH1" or "TCH2", after the plan label of a scanned channel.
    public var identity: String
    /// The text.
    public var text: String
    /// The plan label of a scanned channel.
    public var channel: String?

    init(_ c: sdrkit_dcr_event_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        identity = sdrkitText(c.identity)
        text = sdrkitText(c.text)
        channel = c.has_channel ? sdrkitText(c.channel) : nil
    }
}

/// A call started, ended, or was a single transmission. The field `end` is `active` when it started.
public struct DvoiceCallEvent {
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
    public enum Kind: Int32 {
        case voice = 0
        case groupVoice = 1
        case privateVoice = 2
        case allCall = 3
        case data = 4
        case packet = 5
        case status = 6
        case shortMessage = 7
        case shortData = 8
        case control = 9
        case grant = 10
    }
    public enum End: Int32 {
        case active = 0
        case terminator = 1
        case hangTime = 2
        case replaced = 3
        case closed = 4
        case single = 5
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// Who sends.
    public var source: String
    /// Who is called.
    public var target: String
    /// The talkgroup of a group call, else the time slot.
    public var channel: String?
    /// The text.
    public var text: String?
    /// How urgent it is.
    public var urgency: Urgency?
    /// The family of the event.
    public var family: Family?
    /// The content is encrypted.
    public var `private`: Bool
    /// The id of a call.
    public var id: Int64
    /// What a transmission carries.
    public var kind: Kind?
    /// The target is a group.
    public var group: Bool
    /// Where a call is in its life.
    public var end: End?
    /// How long a call lasted.
    public var duration: Double?

    init(_ c: sdrkit_dvoice_call_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
        `private` = c.private_
        id = c.id
        kind = Kind(rawValue: c.kind)
        group = c.group
        end = End(rawValue: c.end)
        duration = c.has_duration ? c.duration : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DvoiceCallEvent, SDRKitCode> {
        var c = sdrkit_dvoice_call_event()
        let code = sdrkit_dvoice_call_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DvoiceCallEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A text message.
public struct DvoiceMessageEvent {
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
    public enum Format: Int32 {
        case shortData = 0
        case packetData = 1
        case slowData = 2
        case sms = 3
        case metaText = 4
        case freedvText = 5
        case ysfText = 6
        case shortMessage = 7
        case packet = 8
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// Who sends.
    public var source: String
    /// Who is called.
    public var target: String
    /// The text.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// How a message was sent.
    public var format: Format?
    /// The target is a group.
    public var group: Bool

    init(_ c: sdrkit_dvoice_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        format = Format(rawValue: c.format)
        group = c.group
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DvoiceMessageEvent, SDRKitCode> {
        var c = sdrkit_dvoice_message_event()
        let code = sdrkit_dvoice_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DvoiceMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A position.
public struct DvoicePositionEvent {
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
    public enum Format: Int32 {
        case lrrp = 0
        case nmea = 1
        case dprs = 2
        case m17Gnss = 3
        case ysfGps = 4
        case dmrGps = 5
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// Who sends.
    public var source: String
    /// The position, latitude first.
    public var position: SDRKitPosition
    /// The height.
    public var heightM: Double?
    /// The speed.
    public var speedKt: Double?
    /// The course.
    public var courseDeg: Double?
    /// The text.
    public var text: String?
    /// The family of the event.
    public var family: Family?
    /// How a position was sent.
    public var format: Format?

    init(_ c: sdrkit_dvoice_position_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        position = SDRKitPosition(c.position)
        heightM = c.has_height_m ? c.height_m : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        family = Family(rawValue: c.family)
        format = Format(rawValue: c.format)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DvoicePositionEvent, SDRKitCode> {
        var c = sdrkit_dvoice_position_event()
        let code = sdrkit_dvoice_position_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DvoicePositionEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum DvoiceTables {
    /// The changed rows of the table "calls" of the stack "dmr", newest first, at most `capacity` rows.
    public static func dmrCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dmr_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "dmr", newest first, at most `capacity` rows.
    public static func dmrCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dmr_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "dmr", newest first, at most `capacity` rows.
    public static func dmrMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dmr_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "dmr", newest first, at most `capacity` rows.
    public static func dmrPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dmr_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "dmr", newest first, at most `capacity` rows.
    public static func dmrSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dmr_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "dmr", newest first, at most `capacity` rows.
    public static func dmrGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dmr_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "dmr", newest first, at most `capacity` rows.
    public static func dmrTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dmr_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "dmr", newest first, at most `capacity` rows.
    public static func dmrHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dmr_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "dmr", newest first, at most `capacity` rows.
    public static func dmrHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dmr_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25Calls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25CallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25Positions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25Sites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25Grants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25Talkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "identifiers" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25Identifiers(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[DvoiceIdentifierRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_identifier_row](repeating: sdrkit_dvoice_identifier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_identifiers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceIdentifierRow.init))
    }
    /// The changed rows of the table "adjacent_sites" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25AdjacentSites(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[DvoiceAdjacentRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_adjacent_row](repeating: sdrkit_dvoice_adjacent_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_adjacent_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceAdjacentRow.init))
    }
    /// The changed rows of the table "history" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25History(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "p25", newest first, at most `capacity` rows.
    public static func p25Heard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "m17", newest first, at most `capacity` rows.
    public static func m17Calls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_m17_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "m17", newest first, at most `capacity` rows.
    public static func m17CallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_m17_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "m17", newest first, at most `capacity` rows.
    public static func m17Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_m17_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "m17", newest first, at most `capacity` rows.
    public static func m17Positions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_m17_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "m17", newest first, at most `capacity` rows.
    public static func m17Sites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_m17_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "m17", newest first, at most `capacity` rows.
    public static func m17Grants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_m17_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "m17", newest first, at most `capacity` rows.
    public static func m17Talkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_m17_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "m17", newest first, at most `capacity` rows.
    public static func m17History(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_m17_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "m17", newest first, at most `capacity` rows.
    public static func m17Heard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_m17_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "dstar", newest first, at most `capacity` rows.
    public static func dstarCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dstar_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "dstar", newest first, at most `capacity` rows.
    public static func dstarCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dstar_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "dstar", newest first, at most `capacity` rows.
    public static func dstarMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dstar_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "dstar", newest first, at most `capacity` rows.
    public static func dstarPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dstar_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "dstar", newest first, at most `capacity` rows.
    public static func dstarSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dstar_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "dstar", newest first, at most `capacity` rows.
    public static func dstarGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dstar_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "dstar", newest first, at most `capacity` rows.
    public static func dstarTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dstar_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "dstar", newest first, at most `capacity` rows.
    public static func dstarHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dstar_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "dstar", newest first, at most `capacity` rows.
    public static func dstarHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dstar_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "nxdn", newest first, at most `capacity` rows.
    public static func nxdnCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nxdn_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "nxdn", newest first, at most `capacity` rows.
    public static func nxdnCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nxdn_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "nxdn", newest first, at most `capacity` rows.
    public static func nxdnMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nxdn_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "nxdn", newest first, at most `capacity` rows.
    public static func nxdnPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nxdn_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "nxdn", newest first, at most `capacity` rows.
    public static func nxdnSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nxdn_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "nxdn", newest first, at most `capacity` rows.
    public static func nxdnGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nxdn_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "nxdn", newest first, at most `capacity` rows.
    public static func nxdnTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nxdn_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "nxdn", newest first, at most `capacity` rows.
    public static func nxdnHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nxdn_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "nxdn", newest first, at most `capacity` rows.
    public static func nxdnHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nxdn_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "dpmr", newest first, at most `capacity` rows.
    public static func dpmrCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dpmr_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "dpmr", newest first, at most `capacity` rows.
    public static func dpmrCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dpmr_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "dpmr", newest first, at most `capacity` rows.
    public static func dpmrMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dpmr_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "dpmr", newest first, at most `capacity` rows.
    public static func dpmrPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dpmr_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "dpmr", newest first, at most `capacity` rows.
    public static func dpmrSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dpmr_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "dpmr", newest first, at most `capacity` rows.
    public static func dpmrGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dpmr_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "dpmr", newest first, at most `capacity` rows.
    public static func dpmrTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dpmr_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "dpmr", newest first, at most `capacity` rows.
    public static func dpmrHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dpmr_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "dpmr", newest first, at most `capacity` rows.
    public static func dpmrHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dpmr_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "ysf", newest first, at most `capacity` rows.
    public static func ysfCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ysf_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "ysf", newest first, at most `capacity` rows.
    public static func ysfCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ysf_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "ysf", newest first, at most `capacity` rows.
    public static func ysfMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ysf_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "ysf", newest first, at most `capacity` rows.
    public static func ysfPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ysf_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "ysf", newest first, at most `capacity` rows.
    public static func ysfSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ysf_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "ysf", newest first, at most `capacity` rows.
    public static func ysfGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ysf_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "ysf", newest first, at most `capacity` rows.
    public static func ysfTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ysf_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "ysf", newest first, at most `capacity` rows.
    public static func ysfHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ysf_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "ysf", newest first, at most `capacity` rows.
    public static func ysfHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ysf_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "p25p2", newest first, at most `capacity` rows.
    public static func p25p2Calls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25p2_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "p25p2", newest first, at most `capacity` rows.
    public static func p25p2CallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25p2_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "p25p2", newest first, at most `capacity` rows.
    public static func p25p2Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25p2_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "p25p2", newest first, at most `capacity` rows.
    public static func p25p2Positions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25p2_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "p25p2", newest first, at most `capacity` rows.
    public static func p25p2Sites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25p2_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "p25p2", newest first, at most `capacity` rows.
    public static func p25p2Grants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25p2_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "p25p2", newest first, at most `capacity` rows.
    public static func p25p2Talkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25p2_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "p25p2", newest first, at most `capacity` rows.
    public static func p25p2History(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25p2_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "p25p2", newest first, at most `capacity` rows.
    public static func p25p2Heard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_p25p2_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "tetrapol", newest first, at most `capacity` rows.
    public static func tetrapolCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetrapol_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "tetrapol", newest first, at most `capacity` rows.
    public static func tetrapolCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetrapol_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "tetrapol", newest first, at most `capacity` rows.
    public static func tetrapolMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetrapol_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "tetrapol", newest first, at most `capacity` rows.
    public static func tetrapolPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetrapol_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "tetrapol", newest first, at most `capacity` rows.
    public static func tetrapolSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetrapol_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "tetrapol", newest first, at most `capacity` rows.
    public static func tetrapolGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetrapol_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "tetrapol", newest first, at most `capacity` rows.
    public static func tetrapolTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetrapol_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "tetrapol", newest first, at most `capacity` rows.
    public static func tetrapolHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetrapol_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "tetrapol", newest first, at most `capacity` rows.
    public static func tetrapolHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetrapol_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "tetra", newest first, at most `capacity` rows.
    public static func tetraCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "tetra", newest first, at most `capacity` rows.
    public static func tetraCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "tetra", newest first, at most `capacity` rows.
    public static func tetraMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "tetra", newest first, at most `capacity` rows.
    public static func tetraPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "tetra", newest first, at most `capacity` rows.
    public static func tetraSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "tetra", newest first, at most `capacity` rows.
    public static func tetraGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "tetra", newest first, at most `capacity` rows.
    public static func tetraTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "tetra", newest first, at most `capacity` rows.
    public static func tetraHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "tetra", newest first, at most `capacity` rows.
    public static func tetraHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "tetra_dmo", newest first, at most `capacity` rows.
    public static func tetraDmoCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_dmo_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "tetra_dmo", newest first, at most `capacity` rows.
    public static func tetraDmoCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_dmo_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "tetra_dmo", newest first, at most `capacity` rows.
    public static func tetraDmoMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_dmo_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "tetra_dmo", newest first, at most `capacity` rows.
    public static func tetraDmoPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_dmo_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "tetra_dmo", newest first, at most `capacity` rows.
    public static func tetraDmoSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_dmo_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "tetra_dmo", newest first, at most `capacity` rows.
    public static func tetraDmoGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_dmo_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "tetra_dmo", newest first, at most `capacity` rows.
    public static func tetraDmoTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_dmo_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "tetra_dmo", newest first, at most `capacity` rows.
    public static func tetraDmoHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_dmo_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "tetra_dmo", newest first, at most `capacity` rows.
    public static func tetraDmoHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tetra_dmo_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "freedv", newest first, at most `capacity` rows.
    public static func freedvCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freedv_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "freedv", newest first, at most `capacity` rows.
    public static func freedvCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freedv_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "freedv", newest first, at most `capacity` rows.
    public static func freedvMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freedv_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "freedv", newest first, at most `capacity` rows.
    public static func freedvPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freedv_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "freedv", newest first, at most `capacity` rows.
    public static func freedvSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freedv_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "freedv", newest first, at most `capacity` rows.
    public static func freedvGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freedv_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "freedv", newest first, at most `capacity` rows.
    public static func freedvTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freedv_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "history" of the stack "freedv", newest first, at most `capacity` rows.
    public static func freedvHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freedv_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "freedv", newest first, at most `capacity` rows.
    public static func freedvHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_freedv_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "modes_seen" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoModesSeen(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[DvoiceAutoSeenRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_auto_seen_row](repeating: sdrkit_dvoice_auto_seen_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_modes_seen_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceAutoSeenRow.init))
    }
    /// The changed rows of the table "identifiers" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoIdentifiers(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[DvoiceIdentifierRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_identifier_row](repeating: sdrkit_dvoice_identifier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_identifiers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceIdentifierRow.init))
    }
    /// The changed rows of the table "adjacent_sites" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoAdjacentSites(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[DvoiceAdjacentRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_adjacent_row](repeating: sdrkit_dvoice_adjacent_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_adjacent_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceAdjacentRow.init))
    }
    /// The changed rows of the table "history" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "dvoice-auto", newest first, at most `capacity` rows.
    public static func dvoiceAutoHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvoice_auto_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
    /// The changed rows of the table "calls" of the stack "dcr", newest first, at most `capacity` rows.
    public static func dcrCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceCallRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_row](repeating: sdrkit_dvoice_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dcr_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallRow.init))
    }
    /// The changed rows of the table "call_details" of the stack "dcr", newest first, at most `capacity` rows.
    public static func dcrCallDetails(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[DvoiceCallDetailRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_call_detail_row](repeating: sdrkit_dvoice_call_detail_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dcr_call_details_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceCallDetailRow.init))
    }
    /// The changed rows of the table "messages" of the stack "dcr", newest first, at most `capacity` rows.
    public static func dcrMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[DvoiceMessageRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_message_row](repeating: sdrkit_dvoice_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dcr_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceMessageRow.init))
    }
    /// The changed rows of the table "positions" of the stack "dcr", newest first, at most `capacity` rows.
    public static func dcrPositions(_ handle: SDRKIT_HANDLE, capacity: Int = 512) -> Result<[DvoicePositionRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_position_row](repeating: sdrkit_dvoice_position_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dcr_positions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoicePositionRow.init))
    }
    /// The changed rows of the table "sites" of the stack "dcr", newest first, at most `capacity` rows.
    public static func dcrSites(_ handle: SDRKIT_HANDLE, capacity: Int = 1) -> Result<[DvoiceSiteRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_site_row](repeating: sdrkit_dvoice_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dcr_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceSiteRow.init))
    }
    /// The changed rows of the table "grants" of the stack "dcr", newest first, at most `capacity` rows.
    public static func dcrGrants(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DvoiceGrantRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_grant_row](repeating: sdrkit_dvoice_grant_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dcr_grants_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceGrantRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "dcr", newest first, at most `capacity` rows.
    public static func dcrTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_talkgroup_row](repeating: sdrkit_dvoice_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dcr_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceTalkgroupRow.init))
    }
    /// The changed rows of the table "events" of the stack "dcr", newest first, at most `capacity` rows.
    public static func dcrEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DcrEventRow], SDRKitCode> {
        var rows = [sdrkit_dcr_event_row](repeating: sdrkit_dcr_event_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dcr_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DcrEventRow.init))
    }
    /// The changed rows of the table "history" of the stack "dcr", newest first, at most `capacity` rows.
    public static func dcrHistory(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[DvoiceHistoryRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_history_row](repeating: sdrkit_dvoice_history_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dcr_history_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHistoryRow.init))
    }
    /// The changed rows of the table "heard" of the stack "dcr", newest first, at most `capacity` rows.
    public static func dcrHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DvoiceHeardRow], SDRKitCode> {
        var rows = [sdrkit_dvoice_heard_row](repeating: sdrkit_dvoice_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dcr_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DvoiceHeardRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum DvoiceCommands {
    /// Clear. Forgets everything decoded so far.
    public static func dmrClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dmr_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func dmrHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dmr_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func dmrRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dmr_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func dmrRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dmr_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a text. Sends the text as defined short data in direct mode, on the setting slot with the setting colour_code, from the setting radio_id to the setting called_id, or to the setting talkgroup when called_id is 0: up to 752 ISO 8859-1 characters, or 376 other characters as UTF-16LE. The library has no AMBE+2 encoder, so the stack sends no voice. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func dmrSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dmr_data_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_dmr_data_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_dmr_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Halt. Ends the transmission after the frame or the burst on air, with the end sequence of the stack.
    public static func dmrHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dmr_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func p25Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_p25_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func p25HoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_p25_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func p25Release(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_p25_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func p25Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_p25_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func m17Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_m17_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func m17HoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_m17_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func m17Release(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_m17_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func m17Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_m17_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Push to talk. With on true, sends a voice stream from the microphone input (Codec 2, 3200 bit/s) from the setting callsign to the setting destination, until ptt with on false, halt or the time-out. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
    public static func m17Ptt(_ handle: SDRKIT_HANDLE, on: Bool) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_ptt_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_ptt_args>.size)
        let code: Int32 = { () -> Int32 in
        args.on = on
        return sdrkit_m17_ptt(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Send a text. Sends the text as an SMS packet (protocol 5) from the setting callsign to the setting destination. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func m17Send(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_m17_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_m17_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_m17_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send data. Sends the bytes as a packet with the protocol byte, from the setting callsign to the setting destination. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func m17SendData(_ handle: SDRKIT_HANDLE, `protocol`: Int64, data: [UInt8]) -> Result<Void, SDRKitCode> {
        var args = sdrkit_m17_send_data_args()
        args.size = UInt32(MemoryLayout<sdrkit_m17_send_data_args>.size)
        let code: Int32 = data.withUnsafeBufferPointer { data_ in { () -> Int32 in
        args.protocol = `protocol`
        args.data = sdrkit_bytes(ptr: data_.baseAddress, len: data_.count)
        return sdrkit_m17_send_data(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Halt. Ends the transmission after the frame or the burst on air, with the end sequence of the stack.
    public static func m17Halt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_m17_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func dstarClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dstar_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func dstarHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dstar_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func dstarRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dstar_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func dstarRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dstar_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a text. Sends the radio header from the setting callsign to the setting your_call, and the text in the slow data of voice frames that carry AMBE silence: the library has no AMBE encoder. The transmission ends after every slow-data block went out once, or at halt. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func dstarSend(_ handle: SDRKIT_HANDLE, text: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dstar_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_dstar_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in { () -> Int32 in
        args.text = text_
        return sdrkit_dstar_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Halt. Ends the transmission after the frame or the burst on air, with the end sequence of the stack.
    public static func dstarHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dstar_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func nxdnClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_nxdn_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func nxdnHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_nxdn_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func nxdnRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_nxdn_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func nxdnRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_nxdn_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func dpmrClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dpmr_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func dpmrHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dpmr_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func dpmrRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dpmr_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func dpmrRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dpmr_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func ysfClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ysf_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func ysfHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ysf_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func ysfRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ysf_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func ysfRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ysf_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func p25p2Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_p25p2_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func p25p2HoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_p25p2_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func p25p2Release(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_p25p2_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func p25p2Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_p25p2_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func tetrapolClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tetrapol_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func tetrapolHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tetrapol_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func tetrapolRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tetrapol_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func tetrapolRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_tetrapol_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func tetraClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tetra_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func tetraHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tetra_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func tetraRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tetra_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func tetraRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_tetra_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func tetraDmoClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tetra_dmo_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func tetraDmoHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tetra_dmo_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func tetraDmoRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tetra_dmo_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func tetraDmoRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_tetra_dmo_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func freedvClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_freedv_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func freedvHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_freedv_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func freedvRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_freedv_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func freedvRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_freedv_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Push to talk. With on true, sends the microphone input on the waveform of the submode: Codec 2 1300 on 1600 in the submode 1600, else Codec 2 700C on 700D (700E in the submode 700E); with the setting callsign in the text channel, until ptt with on false, halt or the time-out. The submode RADE does not send. It transmits (voice). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "ptt" stops it.
    public static func freedvPtt(_ handle: SDRKIT_HANDLE, on: Bool) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_ptt_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_ptt_args>.size)
        let code: Int32 = { () -> Int32 in
        args.on = on
        return sdrkit_freedv_ptt(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Halt. Ends the transmission after the frame or the burst on air, with the end sequence of the stack.
    public static func freedvHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_freedv_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func dvoiceAutoClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvoice_auto_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func dvoiceAutoHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvoice_auto_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func dvoiceAutoRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvoice_auto_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func dvoiceAutoRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvoice_auto_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func dcrClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dcr_clear(handle)
        return sdrkitResult(code)
    }
    /// Hold current. Plays only the target of the call that plays now, or of the newest call.
    public static func dcrHoldCurrent(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dcr_hold_current(handle)
        return sdrkitResult(code)
    }
    /// Release. Lets the held target go.
    public static func dcrRelease(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dcr_release(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func dcrRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvoice_talkgroup_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvoice_talkgroup_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dcr_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum DvoiceSettings {
    /// Error squelch.
    public static func dmrSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func dmrAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func dmrAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func dmrMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func dmrPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func dmrHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func dmrLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func dmrHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func dmrHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func dmrListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Radio ID.
    public static func dmrRadioId(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "radio_id", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrRadioId(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "radio_id", value))
    }
    /// Radio ID called.
    public static func dmrCalledId(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "called_id", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrCalledId(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "called_id", value))
    }
    /// Talkgroup.
    public static func dmrTalkgroup(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "talkgroup", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrTalkgroup(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "talkgroup", value))
    }
    /// Colour code.
    public static func dmrColourCode(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "colour_code", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrColourCode(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "colour_code", value))
    }
    /// The options of the setting "slot" of the stack "dmr".
    public enum DmrSlot: Int32 {
        case slot1 = 0
        case slot2 = 1
    }
    /// Time slot.
    public static func dmrSlot(_ handle: SDRKIT_HANDLE) -> Result<DmrSlot, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "slot", &v)
        return code == SDRKIT_OK ? .success(DmrSlot(rawValue: v) ?? DmrSlot(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrSlot(_ handle: SDRKIT_HANDLE, _ value: DmrSlot) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "slot", value.rawValue))
    }
    /// Transmit offset.
    public static func dmrTxSplit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_split", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrTxSplit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_split", value))
    }
    /// Error squelch.
    public static func p25SquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25SquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func p25AllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25AllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func p25Allow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setP25Allow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func p25MuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setP25MuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func p25Priority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setP25Priority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func p25Hold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setP25Hold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func p25Labels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setP25Labels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func p25HoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25HoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func p25HangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25HangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func p25Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Error squelch.
    public static func m17SquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setM17SquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func m17AllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setM17AllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func m17Allow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setM17Allow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func m17MuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setM17MuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func m17Priority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setM17Priority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func m17Hold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setM17Hold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func m17Labels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setM17Labels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func m17HoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setM17HoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func m17HangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setM17HangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func m17Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setM17Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Own call sign.
    public static func m17Callsign(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "callsign", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setM17Callsign(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "callsign", $0) })
    }
    /// Destination.
    public static func m17Destination(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "destination", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setM17Destination(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "destination", $0) })
    }
    /// Channel access number.
    public static func m17Can(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "can", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setM17Can(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "can", value))
    }
    /// The options of the setting "voice_mode" of the stack "m17".
    public enum M17VoiceMode: Int32 {
        case voice3200BitS = 0
    }
    /// Voice.
    public static func m17VoiceMode(_ handle: SDRKIT_HANDLE) -> Result<M17VoiceMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "voice_mode", &v)
        return code == SDRKIT_OK ? .success(M17VoiceMode(rawValue: v) ?? M17VoiceMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setM17VoiceMode(_ handle: SDRKIT_HANDLE, _ value: M17VoiceMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "voice_mode", value.rawValue))
    }
    /// Text.
    public static func m17MetaText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "meta_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setM17MetaText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "meta_text", $0) })
    }
    /// Transmit offset.
    public static func m17TxSplit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_split", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setM17TxSplit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_split", value))
    }
    /// Time-out.
    public static func m17TxTimeout(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_timeout", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setM17TxTimeout(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_timeout", value))
    }
    /// Error squelch.
    public static func dstarSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func dstarAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func dstarAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func dstarMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func dstarPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func dstarHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func dstarLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func dstarHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func dstarHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func dstarListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Own call sign.
    public static func dstarCallsign(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "callsign", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarCallsign(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "callsign", $0) })
    }
    /// Suffix.
    public static func dstarSuffix(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "suffix", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarSuffix(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "suffix", $0) })
    }
    /// Your call sign.
    public static func dstarYourCall(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "your_call", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarYourCall(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "your_call", $0) })
    }
    /// Repeater 1.
    public static func dstarRpt1(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "rpt1", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarRpt1(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "rpt1", $0) })
    }
    /// Repeater 2.
    public static func dstarRpt2(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "rpt2", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarRpt2(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "rpt2", $0) })
    }
    /// Via repeater.
    public static func dstarViaRepeater(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "via_repeater", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarViaRepeater(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "via_repeater", value))
    }
    /// Transmit offset.
    public static func dstarTxSplit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_split", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDstarTxSplit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_split", value))
    }
    /// Error squelch.
    public static func nxdnSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNxdnSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func nxdnAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNxdnAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func nxdnAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNxdnAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func nxdnMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNxdnMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func nxdnPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNxdnPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func nxdnHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNxdnHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func nxdnLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNxdnLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func nxdnHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNxdnHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func nxdnHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNxdnHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func nxdnListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNxdnListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Error squelch.
    public static func dpmrSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDpmrSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func dpmrAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDpmrAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func dpmrAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDpmrAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func dpmrMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDpmrMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func dpmrPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDpmrPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func dpmrHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDpmrHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func dpmrLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDpmrLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func dpmrHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDpmrHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func dpmrHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDpmrHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func dpmrListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDpmrListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Error squelch.
    public static func ysfSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setYsfSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func ysfAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setYsfAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func ysfAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setYsfAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func ysfMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setYsfMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func ysfPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setYsfPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func ysfHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setYsfHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func ysfLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setYsfLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func ysfHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setYsfHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func ysfHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setYsfHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func ysfListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setYsfListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Error squelch.
    public static func p25p2SquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25p2SquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func p25p2AllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25p2AllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func p25p2Allow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setP25p2Allow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func p25p2MuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setP25p2MuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func p25p2Priority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setP25p2Priority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func p25p2Hold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setP25p2Hold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func p25p2Labels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setP25p2Labels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func p25p2HoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25p2HoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func p25p2HangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25p2HangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func p25p2Listen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25p2Listen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Error squelch.
    public static func tetrapolSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetrapolSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func tetrapolAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetrapolAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func tetrapolAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetrapolAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func tetrapolMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetrapolMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func tetrapolPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetrapolPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func tetrapolHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetrapolHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func tetrapolLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetrapolLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func tetrapolHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetrapolHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func tetrapolHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetrapolHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func tetrapolListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetrapolListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Error squelch.
    public static func tetraSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func tetraAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func tetraAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func tetraMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func tetraPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func tetraHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func tetraLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func tetraHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func tetraHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func tetraListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Error squelch.
    public static func tetraDmoSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func tetraDmoAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func tetraDmoAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func tetraDmoMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func tetraDmoPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func tetraDmoHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func tetraDmoLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func tetraDmoHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func tetraDmoHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func tetraDmoListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Error squelch.
    public static func freedvSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func freedvAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func freedvAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func freedvMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func freedvPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func freedvHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func freedvLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func freedvHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func freedvHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func freedvListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Callsign text.
    public static func freedvCallsign(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "callsign", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvCallsign(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "callsign", $0) })
    }
    /// Clipper.
    public static func freedvTxClip(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_clip", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvTxClip(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_clip", value))
    }
    /// Transmit filter.
    public static func freedvTxFilter(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_filter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvTxFilter(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_filter", value))
    }
    /// Transmit offset.
    public static func freedvTxSplit(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_split", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvTxSplit(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_split", value))
    }
    /// Time-out.
    public static func freedvTxTimeout(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_timeout", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvTxTimeout(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_timeout", value))
    }
    /// Error squelch.
    public static func dvoiceAutoSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvoiceAutoSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func dvoiceAutoAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvoiceAutoAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func dvoiceAutoAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvoiceAutoAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func dvoiceAutoMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvoiceAutoMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func dvoiceAutoPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvoiceAutoPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func dvoiceAutoHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvoiceAutoHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func dvoiceAutoLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvoiceAutoLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func dvoiceAutoHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvoiceAutoHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func dvoiceAutoHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvoiceAutoHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func dvoiceAutoListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvoiceAutoListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Error squelch.
    public static func dcrSquelchPct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "squelch_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrSquelchPct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "squelch_pct", value))
    }
    /// Allowed only.
    public static func dcrAllowOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "allow_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrAllowOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "allow_only", value))
    }
    /// Allow list.
    public static func dcrAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Mute list.
    public static func dcrMuteList(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mute_list", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrMuteList(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mute_list", $0) })
    }
    /// Priority list.
    public static func dcrPriority(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "priority", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrPriority(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "priority", $0) })
    }
    /// Hold.
    public static func dcrHold(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "hold", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrHold(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "hold", $0) })
    }
    /// Labels.
    public static func dcrLabels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "labels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrLabels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "labels", $0) })
    }
    /// Hold timeout.
    public static func dcrHoldTimeoutS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hold_timeout_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrHoldTimeoutS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hold_timeout_s", value))
    }
    /// Hang time.
    public static func dcrHangS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hang_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrHangS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hang_s", value))
    }
    /// Listen.
    public static func dcrListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// Channel plan.
    public static func dcrPlan(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "plan", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrPlan(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "plan", $0) })
    }
    /// Watch every plan channel in the span.
    public static func dcrScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan", value))
    }
    /// Channels decoded at once.
    public static func dcrMaxChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrMaxChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_channels", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum DvoiceGeneratorSettings {
    /// Colour code.
    public static func dmrCc(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_cc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDmrCc(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_cc", value))
    }
    /// NAC.
    public static func p25Nac(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_nac", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setP25Nac(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_nac", value))
    }
    /// Test signal communication type.
    public static func tetraDmoCommType(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_comm_type", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoCommType(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_comm_type", value))
    }
    /// Test signal group call.
    public static func tetraDmoGroup(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_group", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoGroup(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_group", value))
    }
    /// Test signal encryption state.
    public static func tetraDmoAiEnc(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_ai_enc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoAiEnc(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_ai_enc", value))
    }
    /// Test signal end-to-end encryption.
    public static func tetraDmoE2e(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_e2e", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoE2e(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_e2e", value))
    }
    /// Test signal traffic bursts.
    public static func tetraDmoTrafficFrames(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_traffic_frames", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoTrafficFrames(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_traffic_frames", value))
    }
    /// Test signal set-up bursts.
    public static func tetraDmoSetupRepeats(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_setup_repeats", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTetraDmoSetupRepeats(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_setup_repeats", value))
    }
    /// Clipper.
    public static func freedvClip(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_clip", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFreedvClip(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_clip", value))
    }
    /// The options of the setting "gen_content" of the stack "dcr".
    public enum DcrContent: Int32 {
        case userCode1 = 0
        case userCode2 = 1
        case userCode3 = 2
        case privacy = 3
        case sweep = 4
        case messages = 5
    }
    /// Content.
    public static func dcrContent(_ handle: SDRKIT_HANDLE) -> Result<DcrContent, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_content", &v)
        return code == SDRKIT_OK ? .success(DcrContent(rawValue: v) ?? DcrContent(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrContent(_ handle: SDRKIT_HANDLE, _ value: DcrContent) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_content", value.rawValue))
    }
    /// Deviation scale.
    public static func dcrDeviationScale(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation_scale", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDcrDeviationScale(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation_scale", value))
    }
}
