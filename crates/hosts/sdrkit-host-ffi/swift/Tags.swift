// sdrkit: the typed interface of the stack family "tags", version 1.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum TagsInterface {
    public static let major = 1
    public static let minor = 1
}

/// The status of a stack of the family: the headline, the state, the counts of results and the input level.
public struct TagsStatusFields {
    public enum State: Int32 {
        case noSignal = 0
        case energy = 1
        case decoding = 2
    }
    /// The headline.
    public var text: String
    /// The state.
    public var state: State?
    /// The good results: what passed every check of its format.
    public var good: Int64
    /// The refused results: what failed a check.
    public var bad: Int64
    /// The input level of the last report, relative to full scale.
    public var levelDbfs: Double?
    /// What the stack reads and what no public document confirms.
    public var note: String

    init(_ c: sdrkit_tags_status_fields) {
        text = sdrkitText(c.text)
        state = State(rawValue: c.state)
        good = c.good
        bad = c.bad
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TagsStatusFields, SDRKitCode> {
        var c = sdrkit_tags_status_fields()
        let code = sdrkit_tags_status_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TagsStatusFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `rfid_lf`.
public struct RfidLfFields {
    public enum BitRate: Int32 {
        case rf64 = 0
        case rf32 = 1
    }
    public enum Coding: Int32 {
        case manchester = 0
        case biphase = 1
    }
    /// The level of the reader carrier, relative; no value before the first input.
    public var carrierDb: Double?
    /// The modulation depth of the load (swing over carrier); no value without load modulation in the last 2 s.
    public var depthPct: Double?
    /// The different tags seen.
    public var tags: Int64
    /// The reads: frames that passed every check.
    public var reads: Int64
    /// Frames at the place of the frames of the last read (the same code, the same position in the stream) whose header and stop bit held and whose row or column parity failed.
    public var refused: Int64
    /// The bit rate of the last read.
    public var bitRate: BitRate?
    /// The code of the last read.
    public var coding: Coding?
    /// The last tag: version and number, 10 hex digits.
    public var lastTag: String?

    init(_ c: sdrkit_rfid_lf_fields) {
        carrierDb = c.has_carrier_db ? c.carrier_db : nil
        depthPct = c.has_depth_pct ? c.depth_pct : nil
        tags = c.tags
        reads = c.reads
        refused = c.refused
        bitRate = c.has_bit_rate ? BitRate(rawValue: c.bit_rate) : nil
        coding = c.has_coding ? Coding(rawValue: c.coding) : nil
        lastTag = c.has_last_tag ? sdrkitText(c.last_tag) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RfidLfFields, SDRKitCode> {
        var c = sdrkit_rfid_lf_fields()
        let code = sdrkit_rfid_lf_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RfidLfFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct TagsDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_tags_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TagsDecoderFields, SDRKitCode> {
        var c = sdrkit_tags_decoder_fields()
        let code = sdrkit_tags_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TagsDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the tag table: one tag, read until another tag came or it was not read for the hold time.
public struct RfidLfTagRow {
    public enum BitRate: Int32 {
        case rf64 = 0
        case rf32 = 1
    }
    public enum Coding: Int32 {
        case manchester = 0
        case biphase = 1
    }
    /// The row key.
    public var key: String
    /// The signal time of the last read.
    public var timeS: Double
    /// Version (2 hex digits) and number (8 hex digits).
    public var tag: String
    /// The version or customer code, 0 to 255.
    public var version: Int64
    /// The number, 0 to 4294967295.
    public var number: Int64
    /// The upper 16 bits of the number.
    public var numberHi: Int64
    /// The lower 16 bits of the number.
    public var numberLo: Int64
    /// The bit rate.
    public var bitRate: BitRate?
    /// The code.
    public var coding: Coding?
    /// The reads of the row.
    public var reads: Int64
    /// The carrier level, relative.
    public var levelDb: Double

    init(_ c: sdrkit_rfid_lf_tag_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        tag = sdrkitText(c.tag)
        version = c.version
        number = c.number
        numberHi = c.number_hi
        numberLo = c.number_lo
        bitRate = BitRate(rawValue: c.bit_rate)
        coding = Coding(rawValue: c.coding)
        reads = c.reads
        levelDb = c.level_db
    }
}

/// The result fields of `qi`.
public struct QiFields {
    public enum LastMessage: Int32 {
        case signalStrength = 0
        case endPowerTransfer = 1
        case controlError = 2
        case receivedPower = 3
        case chargeStatus = 4
        case holdOff = 5
        case generalRequest = 6
        case renegotiate = 7
        case specificRequest = 8
        case fodStatus = 9
        case receivedPower24 = 10
        case configuration = 11
        case wpid = 12
        case identification = 13
        case extendedIdentification = 14
        case proprietary = 15
        case other = 16
    }
    /// The level of the power signal, relative; no value before the first input.
    public var powerSignalDb: Double?
    /// The load modulation (swing over power signal); no value without load modulation in the last 2 s.
    public var loadModulationPct: Double?
    /// The packets that passed every check.
    public var packetsOk: Int64
    /// The packets that failed a parity, a start or stop bit, the checksum, or lost their bit clock after the header.
    public var packetsFailed: Int64
    /// The bit rate that the bit clock follows; no value without load modulation in the last 2 s.
    public var bitRate: Double?
    /// The type of the last good packet.
    public var lastMessage: LastMessage?

    init(_ c: sdrkit_qi_fields) {
        powerSignalDb = c.has_power_signal_db ? c.power_signal_db : nil
        loadModulationPct = c.has_load_modulation_pct ? c.load_modulation_pct : nil
        packetsOk = c.packets_ok
        packetsFailed = c.packets_failed
        bitRate = c.has_bit_rate ? c.bit_rate : nil
        lastMessage = c.has_last_message ? LastMessage(rawValue: c.last_message) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<QiFields, SDRKitCode> {
        var c = sdrkit_qi_fields()
        let code = sdrkit_qi_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(QiFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the packet table.
public struct QiPacketRow {
    public enum Message: Int32 {
        case signalStrength = 0
        case endPowerTransfer = 1
        case controlError = 2
        case receivedPower = 3
        case chargeStatus = 4
        case holdOff = 5
        case generalRequest = 6
        case renegotiate = 7
        case specificRequest = 8
        case fodStatus = 9
        case receivedPower24 = 10
        case configuration = 11
        case wpid = 12
        case identification = 13
        case extendedIdentification = 14
        case proprietary = 15
        case other = 16
    }
    /// The row key.
    public var key: String
    /// The signal time of the end of the packet.
    public var timeS: Double
    /// The header, 2 hex digits.
    public var header: String
    /// The type.
    public var message: Message?
    /// The message bytes in hex.
    public var bytes: String
    /// Signal strength, End Power Transfer code, control error (signed), received power, charge status or hold-off (ms).
    public var value: Int64?
    /// The reason of an End Power Transfer packet.
    public var endReason: String?
    /// The meaning of a Charge Status value.
    public var charge: String?
    /// The power class of a Configuration packet.
    public var powerClass: Int64?
    /// The maximum power value of a Configuration packet.
    public var maxPower: Int64?
    /// The version of an Identification packet, major.minor.
    public var version: String?
    /// The manufacturer code of an Identification packet, 4 hex digits.
    public var manufacturer: String?
    /// The basic device identifier of an Identification packet, 8 hex digits (31 bits).
    public var deviceId: String?
    /// An Extended Identification packet follows an Identification packet.
    public var extended: Bool?
    /// The power signal level, relative.
    public var levelDb: Double

    init(_ c: sdrkit_qi_packet_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        header = sdrkitText(c.header)
        message = Message(rawValue: c.message)
        bytes = sdrkitText(c.bytes)
        value = c.has_value ? c.value : nil
        endReason = c.has_end_reason ? sdrkitText(c.end_reason) : nil
        charge = c.has_charge ? sdrkitText(c.charge) : nil
        powerClass = c.has_power_class ? c.power_class : nil
        maxPower = c.has_max_power ? c.max_power : nil
        version = c.has_version ? sdrkitText(c.version) : nil
        manufacturer = c.has_manufacturer ? sdrkitText(c.manufacturer) : nil
        deviceId = c.has_device_id ? sdrkitText(c.device_id) : nil
        extended = c.has_extended ? c.extended : nil
        levelDb = c.level_db
    }
}

/// The result fields of `rfid_uhf`. The timings are those of the last good command; they have no value without a reader carrier in the last 8 s.
public struct RfidUhfFields {
    /// Tari: the length of a data-0.
    public var tariUs: Double?
    /// RTcal: a data-0 and a data-1.
    public var rtcalUs: Double?
    /// RTcal in Tari.
    public var rtcalTari: Double?
    /// TRcal of the last Query.
    public var trcalUs: Double?
    /// The backscatter link frequency that the last Query asks with the divide ratio 8: 8 / TRcal.
    public var blfDr8Khz: Double?
    /// The backscatter link frequency with the divide ratio 64/3.
    public var blfDr643Khz: Double?
    /// The commands that passed every check.
    public var commandsOk: Int64
    /// The frames that failed a timing rule, their length or their CRC.
    public var commandsFailed: Int64
    /// The Query commands decoded.
    public var countQuery: Int64
    /// The QueryRep commands decoded.
    public var countQueryRep: Int64
    /// The QueryAdjust commands decoded.
    public var countQueryAdjust: Int64
    /// The ACK commands decoded.
    public var countAck: Int64
    /// The NAK commands decoded.
    public var countNak: Int64
    /// The Select commands decoded.
    public var countSelect: Int64
    /// The Req_RN commands decoded.
    public var countReqRn: Int64
    /// The Read commands decoded.
    public var countRead: Int64
    /// The Write commands decoded.
    public var countWrite: Int64
    /// The Kill commands decoded.
    public var countKill: Int64
    /// The Lock commands decoded.
    public var countLock: Int64
    /// The Access commands decoded.
    public var countAccess: Int64
    /// A modulated reader carrier was seen in the last 2 s.
    public var readerCarrier: Bool

    init(_ c: sdrkit_rfid_uhf_fields) {
        tariUs = c.has_tari_us ? c.tari_us : nil
        rtcalUs = c.has_rtcal_us ? c.rtcal_us : nil
        rtcalTari = c.has_rtcal_tari ? c.rtcal_tari : nil
        trcalUs = c.has_trcal_us ? c.trcal_us : nil
        blfDr8Khz = c.has_blf_dr8_khz ? c.blf_dr8_khz : nil
        blfDr643Khz = c.has_blf_dr643_khz ? c.blf_dr643_khz : nil
        commandsOk = c.commands_ok
        commandsFailed = c.commands_failed
        countQuery = c.count_query
        countQueryRep = c.count_query_rep
        countQueryAdjust = c.count_query_adjust
        countAck = c.count_ack
        countNak = c.count_nak
        countSelect = c.count_select
        countReqRn = c.count_req_rn
        countRead = c.count_read
        countWrite = c.count_write
        countKill = c.count_kill
        countLock = c.count_lock
        countAccess = c.count_access
        readerCarrier = c.reader_carrier
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<RfidUhfFields, SDRKitCode> {
        var c = sdrkit_rfid_uhf_fields()
        let code = sdrkit_rfid_uhf_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(RfidUhfFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the command table.
public struct RfidUhfCommandRow {
    public enum Command: Int32 {
        case query = 0
        case queryRep = 1
        case queryAdjust = 2
        case ack = 3
        case nak = 4
        case select = 5
        case reqRn = 6
        case read = 7
        case write = 8
        case kill = 9
        case lock = 10
        case access = 11
    }
    /// The row key.
    public var key: String
    /// The signal time of the end of the command.
    public var timeS: Double
    /// The command.
    public var command: Command?
    /// The session, 0 to 3.
    public var session: Int64?
    /// Q of a Query.
    public var q: Int64?
    /// The slots of a Query, 2 to the power Q.
    public var slots: Int64?
    /// The target: A or B of a Query, S0 to S3, SL or RFU of a Select.
    public var target: String?
    /// The tags a Query addresses: all, ~SL or SL.
    public var sel: String?
    /// The coding of the tag reply that a Query asks: FM0, Miller 2, 4 or 8.
    public var coding: String?
    /// A Query asks for a pilot tone (TRext).
    public var pilot: Bool?
    /// The divide ratio of a Query: 8 or 64/3.
    public var dr: String?
    /// The change of Q of a QueryAdjust: +1, -1, unchanged or reserved.
    public var updn: String?
    /// The RN16 of an ACK or a Req_RN, or the handle of an access command, 4 hex digits.
    public var rn16: String?
    /// The memory bank of a Select (FileType, EPC, TID, File_0) or of a Read or Write (Reserved, EPC, TID, User).
    public var bank: String?
    /// The bit pointer of a Select, or the word pointer of a Read or a Write.
    public var pointer: Int64?
    /// The words that a Read asks.
    public var wordCount: Int64?
    /// The mask of a Select, as bits (the first 96).
    public var mask: String?
    /// The Truncate bit of a Select.
    public var truncate: Bool?
    /// The action of a Select, 0 to 7.
    public var action: Int64?
    /// The mask and action bits of a Lock, 5 hex digits.
    public var lockPayload: String?
    /// The level of the reader carrier, relative.
    public var levelDb: Double

    init(_ c: sdrkit_rfid_uhf_command_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        command = Command(rawValue: c.command)
        session = c.has_session ? c.session : nil
        q = c.has_q ? c.q : nil
        slots = c.has_slots ? c.slots : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        sel = c.has_sel ? sdrkitText(c.sel) : nil
        coding = c.has_coding ? sdrkitText(c.coding) : nil
        pilot = c.has_pilot ? c.pilot : nil
        dr = c.has_dr ? sdrkitText(c.dr) : nil
        updn = c.has_updn ? sdrkitText(c.updn) : nil
        rn16 = c.has_rn16 ? sdrkitText(c.rn16) : nil
        bank = c.has_bank ? sdrkitText(c.bank) : nil
        pointer = c.has_pointer ? c.pointer : nil
        wordCount = c.has_word_count ? c.word_count : nil
        mask = c.has_mask ? sdrkitText(c.mask) : nil
        truncate = c.has_truncate ? c.truncate : nil
        action = c.has_action ? c.action : nil
        lockPayload = c.has_lock_payload ? sdrkitText(c.lock_payload) : nil
        levelDb = c.level_db
    }
}

/// A tag was read for the first time, or again after another tag or after the hold time.
public struct RfidLfRfidTagEvent {
    public enum BitRate: Int32 {
        case rf64 = 0
        case rf32 = 1
    }
    public enum Coding: Int32 {
        case manchester = 0
        case biphase = 1
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time of the read.
    public var timeS: Double
    /// Version (2 hex digits) and number (8 hex digits).
    public var tag: String
    /// The version or customer code, 0 to 255.
    public var version: Int64
    /// The number, 0 to 4294967295.
    public var number: Int64
    /// The upper 16 bits of the number.
    public var numberHi: Int64
    /// The lower 16 bits of the number.
    public var numberLo: Int64
    /// The bit rate.
    public var bitRate: BitRate?
    /// The code.
    public var coding: Coding?
    /// The reads of the row.
    public var reads: Int64
    /// The carrier level, relative.
    public var levelDb: Double

    init(_ c: sdrkit_rfid_lf_rfid_tag_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        tag = sdrkitText(c.tag)
        version = c.version
        number = c.number
        numberHi = c.number_hi
        numberLo = c.number_lo
        bitRate = BitRate(rawValue: c.bit_rate)
        coding = Coding(rawValue: c.coding)
        reads = c.reads
        levelDb = c.level_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RfidLfRfidTagEvent, SDRKitCode> {
        var c = sdrkit_rfid_lf_rfid_tag_event()
        let code = sdrkit_rfid_lf_rfid_tag_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RfidLfRfidTagEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A packet passed every check.
public struct QiPacketEvent {
    public enum Message: Int32 {
        case signalStrength = 0
        case endPowerTransfer = 1
        case controlError = 2
        case receivedPower = 3
        case chargeStatus = 4
        case holdOff = 5
        case generalRequest = 6
        case renegotiate = 7
        case specificRequest = 8
        case fodStatus = 9
        case receivedPower24 = 10
        case configuration = 11
        case wpid = 12
        case identification = 13
        case extendedIdentification = 14
        case proprietary = 15
        case other = 16
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time of the end of the packet.
    public var timeS: Double
    /// The header, 2 hex digits.
    public var header: String
    /// The type.
    public var message: Message?
    /// The message bytes in hex.
    public var bytes: String
    /// Signal strength, End Power Transfer code, control error (signed), received power, charge status or hold-off (ms).
    public var value: Int64?
    /// The reason of an End Power Transfer packet.
    public var endReason: String?
    /// The meaning of a Charge Status value.
    public var charge: String?
    /// The power class of a Configuration packet.
    public var powerClass: Int64?
    /// The maximum power value of a Configuration packet.
    public var maxPower: Int64?
    /// The version of an Identification packet, major.minor.
    public var version: String?
    /// The manufacturer code of an Identification packet, 4 hex digits.
    public var manufacturer: String?
    /// The basic device identifier of an Identification packet, 8 hex digits (31 bits).
    public var deviceId: String?
    /// An Extended Identification packet follows an Identification packet.
    public var extended: Bool?
    /// The power signal level, relative.
    public var levelDb: Double

    init(_ c: sdrkit_qi_packet_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        header = sdrkitText(c.header)
        message = Message(rawValue: c.message)
        bytes = sdrkitText(c.bytes)
        value = c.has_value ? c.value : nil
        endReason = c.has_end_reason ? sdrkitText(c.end_reason) : nil
        charge = c.has_charge ? sdrkitText(c.charge) : nil
        powerClass = c.has_power_class ? c.power_class : nil
        maxPower = c.has_max_power ? c.max_power : nil
        version = c.has_version ? sdrkitText(c.version) : nil
        manufacturer = c.has_manufacturer ? sdrkitText(c.manufacturer) : nil
        deviceId = c.has_device_id ? sdrkitText(c.device_id) : nil
        extended = c.has_extended ? c.extended : nil
        levelDb = c.level_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<QiPacketEvent, SDRKitCode> {
        var c = sdrkit_qi_packet_event()
        let code = sdrkit_qi_packet_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(QiPacketEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A reader command passed every check.
public struct RfidUhfRfidCommandEvent {
    public enum Command: Int32 {
        case query = 0
        case queryRep = 1
        case queryAdjust = 2
        case ack = 3
        case nak = 4
        case select = 5
        case reqRn = 6
        case read = 7
        case write = 8
        case kill = 9
        case lock = 10
        case access = 11
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time of the end of the command.
    public var timeS: Double
    /// The command.
    public var command: Command?
    /// The session, 0 to 3.
    public var session: Int64?
    /// Q of a Query.
    public var q: Int64?
    /// The slots of a Query, 2 to the power Q.
    public var slots: Int64?
    /// The target: A or B of a Query, S0 to S3, SL or RFU of a Select.
    public var target: String?
    /// The tags a Query addresses: all, ~SL or SL.
    public var sel: String?
    /// The coding of the tag reply that a Query asks: FM0, Miller 2, 4 or 8.
    public var coding: String?
    /// A Query asks for a pilot tone (TRext).
    public var pilot: Bool?
    /// The divide ratio of a Query: 8 or 64/3.
    public var dr: String?
    /// The change of Q of a QueryAdjust: +1, -1, unchanged or reserved.
    public var updn: String?
    /// The RN16 of an ACK or a Req_RN, or the handle of an access command, 4 hex digits.
    public var rn16: String?
    /// The memory bank of a Select (FileType, EPC, TID, File_0) or of a Read or Write (Reserved, EPC, TID, User).
    public var bank: String?
    /// The bit pointer of a Select, or the word pointer of a Read or a Write.
    public var pointer: Int64?
    /// The words that a Read asks.
    public var wordCount: Int64?
    /// The mask of a Select, as bits (the first 96).
    public var mask: String?
    /// The Truncate bit of a Select.
    public var truncate: Bool?
    /// The action of a Select, 0 to 7.
    public var action: Int64?
    /// The mask and action bits of a Lock, 5 hex digits.
    public var lockPayload: String?
    /// The level of the reader carrier, relative.
    public var levelDb: Double

    init(_ c: sdrkit_rfid_uhf_rfid_command_event) {
        time = c.time
        freqHz = c.freq_hz
        timeS = c.time_s
        command = Command(rawValue: c.command)
        session = c.has_session ? c.session : nil
        q = c.has_q ? c.q : nil
        slots = c.has_slots ? c.slots : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        sel = c.has_sel ? sdrkitText(c.sel) : nil
        coding = c.has_coding ? sdrkitText(c.coding) : nil
        pilot = c.has_pilot ? c.pilot : nil
        dr = c.has_dr ? sdrkitText(c.dr) : nil
        updn = c.has_updn ? sdrkitText(c.updn) : nil
        rn16 = c.has_rn16 ? sdrkitText(c.rn16) : nil
        bank = c.has_bank ? sdrkitText(c.bank) : nil
        pointer = c.has_pointer ? c.pointer : nil
        wordCount = c.has_word_count ? c.word_count : nil
        mask = c.has_mask ? sdrkitText(c.mask) : nil
        truncate = c.has_truncate ? c.truncate : nil
        action = c.has_action ? c.action : nil
        lockPayload = c.has_lock_payload ? sdrkitText(c.lock_payload) : nil
        levelDb = c.level_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<RfidUhfRfidCommandEvent, SDRKitCode> {
        var c = sdrkit_rfid_uhf_rfid_command_event()
        let code = sdrkit_rfid_uhf_rfid_command_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(RfidUhfRfidCommandEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum TagsTables {
    /// The changed rows of the table "tags" of the stack "rfid_lf", newest first, at most `capacity` rows.
    public static func rfidLfTags(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[RfidLfTagRow], SDRKitCode> {
        var rows = [sdrkit_rfid_lf_tag_row](repeating: sdrkit_rfid_lf_tag_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rfid_lf_tags_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RfidLfTagRow.init))
    }
    /// The changed rows of the table "packets" of the stack "qi", newest first, at most `capacity` rows.
    /// The column `device_id` names a row.
    public static func qiPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[QiPacketRow], SDRKitCode> {
        var rows = [sdrkit_qi_packet_row](repeating: sdrkit_qi_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_qi_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(QiPacketRow.init))
    }
    /// The changed rows of the table "commands" of the stack "rfid_uhf", newest first, at most `capacity` rows.
    public static func rfidUhfCommands(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[RfidUhfCommandRow], SDRKitCode> {
        var rows = [sdrkit_rfid_uhf_command_row](repeating: sdrkit_rfid_uhf_command_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_rfid_uhf_commands_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(RfidUhfCommandRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum TagsCommands {
    /// Clear. Forgets everything decoded so far.
    public static func rfidLfClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rfid_lf_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, the bit clocks and the half-bits so far; the table stays.
    public static func rfidLfReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rfid_lf_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func qiClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_qi_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers, the bit clock and the packet in progress; the table stays.
    public static func qiReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_qi_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func rfidUhfClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rfid_uhf_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Drops the trackers and the frame in progress; the table stays.
    public static func rfidUhfReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_rfid_uhf_reset(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum TagsSettings {
    /// The options of the setting "bit_rates" of the stack "rfid_lf".
    public enum RfidLfBitRates: Int32 {
        case both = 0
        case rf64 = 1
        case rf32 = 2
    }
    /// Bit rates read.
    public static func rfidLfBitRates(_ handle: SDRKIT_HANDLE) -> Result<RfidLfBitRates, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bit_rates", &v)
        return code == SDRKIT_OK ? .success(RfidLfBitRates(rawValue: v) ?? RfidLfBitRates(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfBitRates(_ handle: SDRKIT_HANDLE, _ value: RfidLfBitRates) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bit_rates", value.rawValue))
    }
    /// The options of the setting "codes" of the stack "rfid_lf".
    public enum RfidLfCodes: Int32 {
        case both = 0
        case manchester = 1
        case biphase = 2
    }
    /// Codes read.
    public static func rfidLfCodes(_ handle: SDRKIT_HANDLE) -> Result<RfidLfCodes, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "codes", &v)
        return code == SDRKIT_OK ? .success(RfidLfCodes(rawValue: v) ?? RfidLfCodes(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfCodes(_ handle: SDRKIT_HANDLE, _ value: RfidLfCodes) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "codes", value.rawValue))
    }
    /// Same tag held for.
    public static func rfidLfSameTagS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "same_tag_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfSameTagS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "same_tag_s", value))
    }
    /// Swing gate.
    public static func rfidLfGateSigmas(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gate_sigmas", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfGateSigmas(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gate_sigmas", value))
    }
    /// Swing gate.
    public static func qiGateSigmas(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gate_sigmas", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQiGateSigmas(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gate_sigmas", value))
    }
    /// Bit clock followed within.
    public static func qiClockTolerancePct(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock_tolerance_pct", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQiClockTolerancePct(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock_tolerance_pct", value))
    }
    /// List commands that have no checksum.
    public static func rfidUhfListUnchecked(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "list_unchecked", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidUhfListUnchecked(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "list_unchecked", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum TagsGeneratorSettings {
    /// Tags.
    public static func rfidLfId(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_id", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfId(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_id", $0) })
    }
    /// Time of each tag.
    public static func rfidLfIdS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_id_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfIdS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_id_s", value))
    }
    /// Modulation depth.
    public static func rfidLfDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_depth", value))
    }
    /// The options of the setting "gen_rf_div" of the stack "rfid_lf".
    public enum RfidLfRfDiv: Int32 {
        case rf64 = 0
        case rf32 = 1
    }
    /// Bit rate.
    public static func rfidLfRfDiv(_ handle: SDRKIT_HANDLE) -> Result<RfidLfRfDiv, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rf_div", &v)
        return code == SDRKIT_OK ? .success(RfidLfRfDiv(rawValue: v) ?? RfidLfRfDiv(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfRfDiv(_ handle: SDRKIT_HANDLE, _ value: RfidLfRfDiv) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rf_div", value.rawValue))
    }
    /// The options of the setting "gen_coding" of the stack "rfid_lf".
    public enum RfidLfCoding: Int32 {
        case manchester = 0
        case biphase = 1
    }
    /// Code.
    public static func rfidLfCoding(_ handle: SDRKIT_HANDLE) -> Result<RfidLfCoding, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_coding", &v)
        return code == SDRKIT_OK ? .success(RfidLfCoding(rawValue: v) ?? RfidLfCoding(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfCoding(_ handle: SDRKIT_HANDLE, _ value: RfidLfCoding) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_coding", value.rawValue))
    }
    /// Tag clock error.
    public static func rfidLfBitClockPpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_bit_clock_ppm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfBitClockPpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_bit_clock_ppm", value))
    }
    /// Tag starts at.
    public static func rfidLfStartS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_start_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfStartS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_start_s", value))
    }
    /// Inverted load.
    public static func rfidLfInvert(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_invert", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidLfInvert(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_invert", value))
    }
    /// Load modulation depth.
    public static func qiDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQiDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_depth", value))
    }
    /// Bit clock error.
    public static func qiBitClockPpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_bit_clock_ppm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQiBitClockPpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_bit_clock_ppm", value))
    }
    /// Wander frequency.
    public static func qiWanderHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_wander_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQiWanderHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_wander_hz", value))
    }
    /// Wander depth.
    public static func qiWanderDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_wander_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQiWanderDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_wander_depth", value))
    }
    /// Time between packets.
    public static func qiGapS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_gap_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQiGapS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value))
    }
    /// Preamble.
    public static func qiPreamble(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQiPreamble(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_preamble", value))
    }
    /// Packets.
    public static func qiPackets(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_packets", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setQiPackets(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_packets", $0) })
    }
    /// Change after each packet.
    public static func qiEndEdgeBits(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_end_edge_bits", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setQiEndEdgeBits(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_end_edge_bits", value))
    }
    /// Tari.
    public static func rfidUhfTariUs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_tari_us", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidUhfTariUs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_tari_us", value))
    }
    /// Data-1.
    public static func rfidUhfData1Tari(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_data1_tari", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidUhfData1Tari(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_data1_tari", value))
    }
    /// Low pulse (PW).
    public static func rfidUhfPwTari(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pw_tari", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidUhfPwTari(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pw_tari", value))
    }
    /// Modulation depth.
    public static func rfidUhfDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidUhfDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_depth", value))
    }
    /// TRcal.
    public static func rfidUhfTrcalRtcal(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_trcal_rtcal", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidUhfTrcalRtcal(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_trcal_rtcal", value))
    }
    /// Reader clock error.
    public static func rfidUhfReaderClockPpm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_reader_clock_ppm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidUhfReaderClockPpm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_reader_clock_ppm", value))
    }
    /// Gap between commands.
    public static func rfidUhfGapS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_gap_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidUhfGapS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_gap_s", value))
    }
    /// Lead.
    public static func rfidUhfLeadS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_lead_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidUhfLeadS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_lead_s", value))
    }
    /// Commands.
    public static func rfidUhfCommands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_commands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setRfidUhfCommands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_commands", $0) })
    }
}
