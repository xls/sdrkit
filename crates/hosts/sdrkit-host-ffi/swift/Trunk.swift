// sdrkit: the typed interface of the stack family "trunk", version 1.6. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum TrunkInterface {
    public static let major = 1
    public static let minor = 6
}

/// The result fields of MPT 1327.
public struct Mpt1327Fields {
    /// The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
    public var locked: Bool
    /// Codewords good.
    public var codewordsGood: Int64
    /// Codewords repaired.
    public var codewordsRepaired: Int64
    /// Codewords bad.
    public var codewordsBad: Int64
    /// System codewords.
    public var ccscs: Int64
    /// The messages read.
    public var messages: Int64
    /// The system identity of the control channel.
    public var systemIdentity: Int64?
    /// Sync check off.
    public var syncCheckOff: Bool
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double

    init(_ c: sdrkit_mpt1327_fields) {
        locked = c.locked
        codewordsGood = c.codewords_good
        codewordsRepaired = c.codewords_repaired
        codewordsBad = c.codewords_bad
        ccscs = c.ccscs
        messages = c.messages
        systemIdentity = c.has_system_identity ? c.system_identity : nil
        syncCheckOff = c.sync_check_off
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Mpt1327Fields, SDRKitCode> {
        var c = sdrkit_mpt1327_fields()
        let code = sdrkit_mpt1327_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Mpt1327Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An MPT 1327 message. The row key is the sequence number of the message.
public struct Mpt1327MessageRow {
    public enum TypeOption: Int32 {
        case gTC = 0
        case aLH = 1
        case aLHS = 2
        case aLHD = 3
        case aLHE = 4
        case aLHR = 5
        case aLHX = 6
        case aLHF = 7
        case aCK = 8
        case aCKI = 9
        case aCKQ = 10
        case aCKX = 11
        case aCKV = 12
        case aCKE = 13
        case aCKT = 14
        case aCKB = 15
        case aHY = 16
        case aHYX = 17
        case aHYP = 18
        case aHYQ = 19
        case aHYC = 20
        case mARK = 21
        case mAINT = 22
        case cLEAR = 23
        case mOVE = 24
        case bCAST = 25
        case hEAD = 26
        case sAM = 27
        case sTDDATA = 28
        case sTDACK = 29
        case spare = 30
        case rSVD = 31
    }
    /// The row key.
    public var key: String
    /// The signal time of a message, s.
    public var timeS: Double
    /// The type of a message.
    public var type: TypeOption?
    /// The calling address.
    public var caller: String
    /// The called address.
    public var called: String
    /// A channel number.
    public var channel: Int64?
    /// What a message means, for people.
    public var meaning: String

    init(_ c: sdrkit_mpt1327_message_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        type = TypeOption(rawValue: c.type)
        caller = sdrkitText(c.caller)
        called = sdrkitText(c.called)
        channel = c.has_channel ? c.channel : nil
        meaning = sdrkitText(c.meaning)
    }
}

/// An MPT 1327 channel. The row key is the channel number, five digits.
public struct Mpt1327ChannelRow {
    public enum Use: Int32 {
        case control = 0
        case traffic = 1
    }
    /// The row key.
    public var key: String
    /// A channel number.
    public var channel: Int64
    /// The frequency of a channel.
    public var freqHz: Double?
    /// What a channel is used for.
    public var use: Use?
    /// The messages that named a channel.
    public var seen: Int64
    /// The last called address on a traffic channel.
    public var lastCalled: String?
    /// The last calling address on a traffic channel.
    public var lastCaller: String?

    init(_ c: sdrkit_mpt1327_channel_row) {
        key = sdrkitText(c.key)
        channel = c.channel
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        use = Use(rawValue: c.use)
        seen = c.seen
        lastCalled = c.has_last_called ? sdrkitText(c.last_called) : nil
        lastCaller = c.has_last_caller ? sdrkitText(c.last_caller) : nil
    }
}

/// The result fields of LTR.
public struct LtrFields {
    /// The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
    public var locked: Bool
    /// The good words.
    public var wordsGood: Int64
    /// Bad words.
    public var wordsBad: Int64
    /// Syncs.
    public var syncs: Int64
    /// The AREA bit.
    public var area: Int64?
    /// Repeaters seen.
    public var repeaters: Int64
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// Data swing.
    public var swingHz: Double
    /// Words heard.
    public var dataValid: Bool

    init(_ c: sdrkit_ltr_fields) {
        locked = c.locked
        wordsGood = c.words_good
        wordsBad = c.words_bad
        syncs = c.syncs
        area = c.has_area ? c.area : nil
        repeaters = c.repeaters
        levelDbfs = c.level_dbfs
        swingHz = c.swing_hz
        dataValid = c.data_valid
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LtrFields, SDRKitCode> {
        var c = sdrkit_ltr_fields()
        let code = sdrkit_ltr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LtrFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A good LTR word. The row key is the sequence number of the word.
public struct LtrWordRow {
    public enum Kind: Int32 {
        case call = 0
        case idle = 1
        case callsignId = 2
        case busy = 3
        case token = 4
    }
    /// The row key.
    public var key: String
    /// The talkgroup, "{AREA}-{HOME}-{ID}".
    public var talkgroup: String
    /// The AREA bit.
    public var area: Int64
    /// The GO-TO repeater.
    public var goto: Int64
    /// The HOME repeater.
    public var home: Int64
    /// The ID.
    public var id: Int64
    /// The FREE repeater.
    public var free: Int64
    /// The 7 check bits as received.
    public var check: Int64
    /// What the ID says.
    public var kind: Kind?
    /// The signal time of a word, s.
    public var timeS: Double

    init(_ c: sdrkit_ltr_word_row) {
        key = sdrkitText(c.key)
        talkgroup = sdrkitText(c.talkgroup)
        area = c.area
        goto = c.goto_
        home = c.home
        id = c.id
        free = c.free
        check = c.check
        kind = Kind(rawValue: c.kind)
        timeS = c.time_s
    }
}

/// How often a repeater was named in each field. The row key is the repeater.
public struct LtrRepeaterRow {
    /// The row key.
    public var key: String
    /// A repeater number.
    public var repeater: Int64
    /// Times a repeater was named as go-to.
    public var asGoto: Int64
    /// Times named as home.
    public var asHome: Int64
    /// Times named as free.
    public var asFree: Int64
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?

    init(_ c: sdrkit_ltr_repeater_row) {
        key = sdrkitText(c.key)
        repeater = c.repeater
        asGoto = c.as_goto
        asHome = c.as_home
        asFree = c.as_free
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of EDACS.
public struct EdacsFields {
    public enum Polarity: Int32 {
        case normal = 0
        case inverted = 1
    }
    public enum Following: Int32 {
        case idle = 0
        case listening = 1
        case waiting = 2
        case noTable = 3
        case noCentre = 4
        case outsideCapture = 5
    }
    /// The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
    public var locked: Bool
    /// Frames (packets, blocks, bursts) that passed their check.
    public var framesOk: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The data polarity.
    public var polarity: Polarity?
    /// The site.
    public var site: Int64?
    /// The area.
    public var area: Int64?
    /// The control channel's LCN.
    public var controlLcn: Int64?
    /// The highest LCN seen in a message.
    public var highestLcn: Int64?
    /// Words.
    public var words: Int64
    /// Words of a known type.
    public var wordsKnown: Int64
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double
    /// Data swing.
    public var swingHz: Double
    /// The LCN followed now.
    public var followingLcn: Int64?
    /// What the follower does now.
    public var following: Following?

    init(_ c: sdrkit_edacs_fields) {
        locked = c.locked
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        polarity = Polarity(rawValue: c.polarity)
        site = c.has_site ? c.site : nil
        area = c.has_area ? c.area : nil
        controlLcn = c.has_control_lcn ? c.control_lcn : nil
        highestLcn = c.has_highest_lcn ? c.highest_lcn : nil
        words = c.words
        wordsKnown = c.words_known
        levelDbfs = c.level_dbfs
        swingHz = c.swing_hz
        followingLcn = c.has_following_lcn ? c.following_lcn : nil
        following = Following(rawValue: c.following)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EdacsFields, SDRKitCode> {
        var c = sdrkit_edacs_fields()
        let code = sdrkit_edacs_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(EdacsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An EDACS call. `source` is the calling unit, `target` the group or the called unit, `channel` the LCN.
public struct EdacsCallRow {
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
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
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String?
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    public var target: String?
    /// The channel or talk group of a call.
    public var channel: String
    /// A frequency.
    public var freqHz: Double?
    /// A group call.
    public var group: Bool
    /// The start of a call, signal time.
    public var startS: Double
    /// The length of a call.
    public var duration: Double
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// A digital call (not decoded).
    public var digital: Bool
    /// A call that no message announced.
    public var carrierOnly: Bool
    /// Where a call is in its life.
    public var end: End?
    /// The call is followed now.
    public var followed: Bool

    init(_ c: sdrkit_edacs_call_row) {
        key = sdrkitText(c.key)
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        channel = sdrkitText(c.channel)
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        group = c.group
        startS = c.start_s
        duration = c.duration
        urgency = Urgency(rawValue: c.urgency)
        digital = c.digital
        carrierOnly = c.carrier_only
        end = End(rawValue: c.end)
        followed = c.followed
    }
}

/// An EDACS talkgroup. The row key is the group id.
public struct EdacsTalkgroupRow {
    /// The row key.
    public var key: String
    /// A group.
    public var gid: Int64
    /// Calls.
    public var calls: Int64
    /// Last unit.
    public var lastUnit: Int64?
    /// Last LCN.
    public var lastLcn: Int64
    /// A frequency.
    public var freqHz: Double?
    /// On air.
    public var active: Bool
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    public var lastHeard: Double?

    init(_ c: sdrkit_edacs_talkgroup_row) {
        key = sdrkitText(c.key)
        gid = c.gid
        calls = c.calls
        lastUnit = c.has_last_unit ? c.last_unit : nil
        lastLcn = c.last_lcn
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        active = c.active
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// An EDACS channel. The row key is the LCN.
public struct EdacsChannelRow {
    /// The row key.
    public var key: String
    /// A voice channel number.
    public var lcn: Int64
    /// A frequency.
    public var freqHz: Double?
    /// Level.
    public var levelDb: Double
    /// Floor.
    public var floorDb: Double
    /// In the capture.
    public var inCapture: Bool
    /// Carrier.
    public var carrier: Bool
    /// Control channel.
    public var control: Bool

    init(_ c: sdrkit_edacs_channel_row) {
        key = sdrkitText(c.key)
        lcn = c.lcn
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        levelDb = c.level_db
        floorDb = c.floor_db
        inCapture = c.in_capture
        carrier = c.carrier
        control = c.control
    }
}

/// An EDACS frame. The row key is the sequence number of the frame.
public struct EdacsFrameRow {
    /// The row key.
    public var key: String
    /// Message A.
    public var a: Int64
    /// Message B.
    public var b: Int64
    /// Copy errors of A.
    public var errA: Int64
    /// Copy errors of B.
    public var errB: Int64
    /// Time.
    public var timeS: Double
    /// Read as.
    public var text: String

    init(_ c: sdrkit_edacs_frame_row) {
        key = sdrkitText(c.key)
        a = c.a
        b = c.b
        errA = c.err_a
        errB = c.err_b
        timeS = c.time_s
        text = sdrkitText(c.text)
    }
}

/// A message type and how often it was heard. The row key is the type.
public struct EdacsWordRow {
    /// The row key.
    public var key: String
    /// Word.
    public var word: Int64
    /// Count.
    public var count: Int64

    init(_ c: sdrkit_edacs_word_row) {
        key = sdrkitText(c.key)
        word = c.word
        count = c.count
    }
}

/// The result fields of the tone decoders.
public struct TonesFields {
    public enum Demod: Int32 {
        case auto = 0
        case fm = 1
        case usb = 2
        case lsb = 3
        case am = 4
    }
    /// The audio level.
    public var audioDb: Double
    /// The signal to noise ratio of the carrier. No value for audio input.
    public var snrDb: Double?
    /// The offset of the carrier from the tuned frequency. No value for audio input.
    public var offsetHz: Double?
    /// A carrier was seen in the last 5 s. No value for audio input.
    public var carrierRecent: Bool?
    /// The CTCSS tone that is locked now.
    public var ctcssHz: Double?
    /// The DCS code that is locked now.
    public var dcs: String?
    /// The other reading of the waveform of the DCS code.
    public var dcsAlias: String?
    /// The strongest tone of the audio spectrum.
    public var strongestHz: Double?
    /// The DTMF keys of the string that goes on now.
    public var dtmf: String
    /// The listening gate is open.
    public var gateOpen: Bool
    /// The sample rate of the input.
    public var inputRateHz: Double
    /// The input is audio.
    public var audioInput: Bool
    /// The demodulation in use.
    public var demod: Demod?
    /// Every decision of the decoders.
    public var detections: Int64
    /// The CTCSS tones heard.
    public var ctcssCount: Int64
    /// The DCS codes heard.
    public var dcsCount: Int64
    /// The DTMF strings heard.
    public var dtmfCount: Int64
    /// The sequential calls heard.
    public var selcallCount: Int64
    /// The SELCAL calls heard.
    public var selcalCount: Int64
    /// The two-tone pages heard.
    public var twoToneCount: Int64

    init(_ c: sdrkit_tones_fields) {
        audioDb = c.audio_db
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        carrierRecent = c.has_carrier_recent ? c.carrier_recent : nil
        ctcssHz = c.has_ctcss_hz ? c.ctcss_hz : nil
        dcs = c.has_dcs ? sdrkitText(c.dcs) : nil
        dcsAlias = c.has_dcs_alias ? sdrkitText(c.dcs_alias) : nil
        strongestHz = c.has_strongest_hz ? c.strongest_hz : nil
        dtmf = sdrkitText(c.dtmf)
        gateOpen = c.gate_open
        inputRateHz = c.input_rate_hz
        audioInput = c.audio_input
        demod = Demod(rawValue: c.demod)
        detections = c.detections
        ctcssCount = c.ctcss_count
        dcsCount = c.dcs_count
        dtmfCount = c.dtmf_count
        selcallCount = c.selcall_count
        selcalCount = c.selcal_count
        twoToneCount = c.two_tone_count
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TonesFields, SDRKitCode> {
        var c = sdrkit_tones_fields()
        let code = sdrkit_tones_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TonesFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the list of tones heard. The row key is the sequence number of the row.
public struct TonesHeardRow {
    public enum Kind: Int32 {
        case ctcss = 0
        case dcs = 1
        case dtmf = 2
        case selcall = 3
        case selcal = 4
        case twoTone = 5
    }
    /// The row key.
    public var key: String
    /// The signal time of the start.
    public var startS: Double
    /// What the row holds.
    public var kind: Kind?
    /// What was heard ("123.0 Hz", "D023N", "159#", "12345", "AM-CP", "682.5 + 1000.0 Hz").
    public var text: String
    /// The length so far.
    public var length: Double
    /// The details.
    public var detail: String
    /// The level, relative to a full-scale sine.
    public var levelDb: Double
    /// It still goes on.
    public var active: Bool

    init(_ c: sdrkit_tones_heard_row) {
        key = sdrkitText(c.key)
        startS = c.start_s
        kind = Kind(rawValue: c.kind)
        text = sdrkitText(c.text)
        length = c.length
        detail = sdrkitText(c.detail)
        levelDb = c.level_db
        active = c.active
    }
}

/// The result fields of the pager stack.
public struct PagerFields {
    /// A decoder receives a transmission.
    public var receiving: Bool
    /// A carrier is present.
    public var carrier: Bool
    /// A decoder holds sync.
    public var locked: Bool
    /// The protocol and speed of the last sync ("POCSAG 1200").
    public var lastSpeed: String
    /// The offset of the carrier from the tuned frequency at the last sync.
    public var offsetHz: Double?
    /// The signal to noise ratio of the carrier, while a carrier is present.
    public var snrDb: Double?
    /// The level of the input.
    public var levelDbfs: Double
    /// The pages read.
    public var pages: Int64
    /// The code words valid as received or repaired.
    public var codewordsGood: Int64
    /// The code words repaired.
    public var codewordsRepaired: Int64
    /// The code words lost.
    public var codewordsLost: Int64
    /// The pages that the acceptance rules refused (address ranges, vector values, message checks).
    public var pagesRefused: Int64

    init(_ c: sdrkit_pager_fields) {
        receiving = c.receiving
        carrier = c.carrier
        locked = c.locked
        lastSpeed = sdrkitText(c.last_speed)
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.level_dbfs
        pages = c.pages
        codewordsGood = c.codewords_good
        codewordsRepaired = c.codewords_repaired
        codewordsLost = c.codewords_lost
        pagesRefused = c.pages_refused
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<PagerFields, SDRKitCode> {
        var c = sdrkit_pager_fields()
        let code = sdrkit_pager_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(PagerFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the POCSAG sender.
public struct PagerTxFields {
    /// Whether a transmission is on air.
    public var txSending: Bool
    /// The pages of the transmission on air.
    public var txSent: Int64
    /// The pages waiting for the next transmission.
    public var txWaiting: Int64

    init(_ c: sdrkit_pager_tx_fields) {
        txSending = c.tx_sending
        txSent = c.tx_sent
        txWaiting = c.tx_waiting
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<PagerTxFields, SDRKitCode> {
        var c = sdrkit_pager_tx_fields()
        let code = sdrkit_pager_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(PagerTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the page list. The row key is the sequence number of the page.
public struct PagerPageRow {
    public enum TypeOption: Int32 {
        case tone = 0
        case numeric = 1
        case alpha = 2
        case secure = 3
        case binary = 4
        case instruction = 5
        case selcall = 6
    }
    /// The row key.
    public var key: String
    /// The signal time of the page.
    public var timeS: Double
    /// The protocol and speed ("POCSAG 1200", "FLEX 3200/4", "Selcall ZVEI1").
    public var `protocol`: String
    /// The RIC or capcode (0 for a sequential call).
    public var address: Int64
    /// The POCSAG function code.
    public var function: Int64?
    /// What the page carries.
    public var type: TypeOption?
    /// The message.
    public var message: String
    /// A word was lost or the end is not sure.
    public var damaged: Bool
    /// The bits that the repairs changed.
    public var bitsRepaired: Int64
    /// The copies voted into the page.
    public var copies: Int64
    /// The frequency of the carrier, when it is known.
    public var freqHz: Double?

    init(_ c: sdrkit_pager_page_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        `protocol` = sdrkitText(c.protocol)
        address = c.address
        function = c.has_function ? c.function : nil
        type = TypeOption(rawValue: c.type)
        message = sdrkitText(c.message)
        damaged = c.damaged
        bitsRepaired = c.bits_repaired
        copies = c.copies
        freqHz = c.has_freq_hz ? c.freq_hz : nil
    }
}

/// A row of the statistics of each speed. The row key is the speed.
public struct PagerSpeedRow {
    /// The row key.
    public var key: String
    /// The protocol and speed.
    public var speed: String
    /// The code words valid as received.
    public var good: Int64
    /// The code words repaired.
    public var repaired: Int64
    /// The code words lost.
    public var lost: Int64
    /// The pages read.
    public var pages: Int64
    /// The batches (POCSAG) or frames (FLEX).
    public var transmissions: Int64
    /// The pages that the acceptance rules refused.
    public var refused: Int64

    init(_ c: sdrkit_pager_speed_row) {
        key = sdrkitText(c.key)
        speed = sdrkitText(c.speed)
        good = c.good
        repaired = c.repaired
        lost = c.lost
        pages = c.pages
        transmissions = c.transmissions
        refused = c.refused
    }
}

/// A row of the addresses heard. The row key is the protocol and the address ("pocsag/1234567").
public struct PagerHeardRow {
    public enum ProtocolOption: Int32 {
        case pocsag = 0
        case flex = 1
    }
    /// The row key.
    public var key: String
    /// The RIC or capcode.
    public var address: Int64
    /// The paging protocol.
    public var `protocol`: ProtocolOption?
    /// The pages of the address.
    public var pages: Int64
    /// The protocol and speed of the last page.
    public var speed: String
    /// The last text (a tone-only page does not erase it).
    public var lastText: String?
    /// When it was last heard.
    public var lastHeard: Double?

    init(_ c: sdrkit_pager_heard_row) {
        key = sdrkitText(c.key)
        address = c.address
        `protocol` = ProtocolOption(rawValue: c.protocol)
        pages = c.pages
        speed = sdrkitText(c.speed)
        lastText = c.has_last_text ? sdrkitText(c.last_text) : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The counts and the carrier of the ERMES receiver.
public struct ErmesFields {
    /// The good batches.
    public var batches: Int64
    /// The batches dropped.
    public var badBatches: Int64
    /// The initial addresses.
    public var addresses: Int64
    /// The messages.
    public var messages: Int64
    /// The code words repaired.
    public var wordsRepaired: Int64
    /// The times that the preamble and the sync word were found.
    public var syncs: Int64
    /// The network of the last good batch.
    public var network: String?
    /// A carrier is present.
    public var carrier: Bool?
    /// The signal to noise ratio of the carrier.
    public var snrDb: Double?
    /// The level of the input.
    public var levelDbfs: Double?
    /// The offset of the carrier from the tuned frequency.
    public var offsetHz: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_ermes_fields) {
        batches = c.batches
        badBatches = c.bad_batches
        addresses = c.addresses
        messages = c.messages
        wordsRepaired = c.words_repaired
        syncs = c.syncs
        network = c.has_network ? sdrkitText(c.network) : nil
        carrier = c.has_carrier ? c.carrier : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ErmesFields, SDRKitCode> {
        var c = sdrkit_ermes_fields()
        let code = sdrkit_ermes_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ErmesFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct TrunkDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_trunk_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TrunkDecoderFields, SDRKitCode> {
        var c = sdrkit_trunk_decoder_fields()
        let code = sdrkit_trunk_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TrunkDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the ERMES table: the initial addresses of a batch, or a message. The row key is the number of the row.
public struct ErmesEntryRow {
    /// The row key.
    public var key: String
    /// The signal time of the row.
    public var timeS: Double
    /// What the row is: the addresses of a batch, or a message.
    public var kind: String
    /// The batch letter.
    public var batch: String
    /// The local address of a message.
    public var address: Int64?
    /// The category of a message.
    public var category: String?
    /// The addresses, or the text of the message.
    public var text: String

    init(_ c: sdrkit_ermes_entry_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = sdrkitText(c.kind)
        batch = sdrkitText(c.batch)
        address = c.has_address ? c.address : nil
        category = c.has_category ? sdrkitText(c.category) : nil
        text = sdrkitText(c.text)
    }
}

/// The result fields of the Multitone detector.
public struct MultitoneFields {
    /// Carrier.
    public var carrier: Bool
    /// The transmissions measured.
    public var transmissions: Int64
    /// Bursts.
    public var bursts: Int64
    /// Valid BCH words.
    public var validWords: Int64
    /// Other words.
    public var otherWords: Int64
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double

    init(_ c: sdrkit_multitone_fields) {
        carrier = c.carrier
        transmissions = c.transmissions
        bursts = c.bursts
        validWords = c.valid_words
        otherWords = c.other_words
        levelDbfs = c.level_dbfs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MultitoneFields, SDRKitCode> {
        var c = sdrkit_multitone_fields()
        let code = sdrkit_multitone_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MultitoneFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A transmission. The row key is its number.
public struct MultitoneTransmissionRow {
    public enum Kind: Int32 {
        case ffskData = 0
        case fskData = 1
        case toneSequence = 2
        case repeating = 3
        case multiTone = 4
        case other = 5
    }
    /// The row key.
    public var key: String
    /// Time.
    public var timeS: Double
    /// Duration.
    public var duration: Double
    /// SNR.
    public var snrDb: Double
    /// Deviation.
    public var deviationHz: Double
    /// Kind.
    public var kind: Kind?
    /// Period.
    public var periodMs: Double?
    /// Burst.
    public var burst: Int64?

    init(_ c: sdrkit_multitone_transmission_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        duration = c.duration
        snrDb = c.snr_db
        deviationHz = c.deviation_hz
        kind = Kind(rawValue: c.kind)
        periodMs = c.has_period_ms ? c.period_ms : nil
        burst = c.has_burst ? c.burst : nil
    }
}

/// A tone of a transmission. The row key is the transmission, `/` and the place of the tone.
public struct MultitoneToneRow {
    /// The row key.
    public var key: String
    /// The transmission a row belongs to: a row key of [`TX_TABLE`].
    public var transmission: String
    /// A frequency.
    public var hz: Double
    /// Length.
    public var seconds: Double
    /// Start in the transmission.
    public var start: Double

    init(_ c: sdrkit_multitone_tone_row) {
        key = sdrkitText(c.key)
        transmission = sdrkitText(c.transmission)
        hz = c.hz
        seconds = c.seconds
        start = c.start
    }
}

/// A spectral line of a transmission. The row key is the transmission, `/` and the place of the line.
public struct MultitoneLineRow {
    /// The row key.
    public var key: String
    /// The transmission a row belongs to: a row key of [`TX_TABLE`].
    public var transmission: String
    /// A frequency.
    public var hz: Double
    /// Above the median.
    public var db: Double

    init(_ c: sdrkit_multitone_line_row) {
        key = sdrkitText(c.key)
        transmission = sdrkitText(c.transmission)
        hz = c.hz
        db = c.db
    }
}

/// A tone heard and how often. The row key is the tone.
public struct MultitoneHeardRow {
    /// The row key.
    public var key: String
    /// A frequency.
    public var hz: Double
    /// Count.
    public var count: Int64
    /// Length.
    public var seconds: Double
    /// Last.
    public var lastS: Double

    init(_ c: sdrkit_multitone_heard_row) {
        key = sdrkitText(c.key)
        hz = c.hz
        count = c.count
        seconds = c.seconds
        lastS = c.last_s
    }
}

/// An FSK burst. The row key is its number, eight digits.
public struct MultitoneBurstRow {
    public enum Kind: Int32 {
        case ffsk = 0
        case fsk = 1
    }
    /// The row key.
    public var key: String
    /// Burst.
    public var burst: Int64
    /// Kind.
    public var kind: Kind?
    /// Baud.
    public var baud: Double
    /// Bits.
    public var bits: Int64
    /// Valid BCH words.
    public var bchValid: Int64
    /// Words.
    public var bchTotal: Int64
    /// Verdict.
    public var verdict: String

    init(_ c: sdrkit_multitone_burst_row) {
        key = sdrkitText(c.key)
        burst = c.burst
        kind = Kind(rawValue: c.kind)
        baud = c.baud
        bits = c.bits
        bchValid = c.bch_valid
        bchTotal = c.bch_total
        verdict = sdrkitText(c.verdict)
    }
}

/// The result fields of Motorola Type II.
public struct MotorolaType2Fields {
    /// The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
    public var locked: Bool
    /// The state: 0 no input, 1 listening, 2 a good word in the last 5 s.
    public var rxState: Int64
    /// Good words.
    public var goodWords: Int64
    /// Bad words.
    public var badWords: Int64
    /// Information bits repaired.
    public var bitsRepaired: Int64
    /// Messages.
    public var messages: Int64
    /// Messages understood.
    public var messagesKnown: Int64
    /// A system ID.
    public var system: String?
    /// The site ID.
    public var siteId: Int64?
    /// Feature bits, as text.
    public var features: String?
    /// A control channel frequency.
    public var controlChannelHz: Double?
    /// Type II or Type I.
    public var systemType: String?
    /// SmartZone or SmartNet.
    public var network: String
    /// Inverted polarity.
    public var inverted: Bool
    /// The data swing.
    public var swingHz: Double
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    public var snrDb: Double?
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// The plan in use.
    public var plan: String?
    /// A note.
    public var note: String

    init(_ c: sdrkit_motorola_type2_fields) {
        locked = c.locked
        rxState = c.rx_state
        goodWords = c.good_words
        badWords = c.bad_words
        bitsRepaired = c.bits_repaired
        messages = c.messages
        messagesKnown = c.messages_known
        system = c.has_system ? sdrkitText(c.system) : nil
        siteId = c.has_site_id ? c.site_id : nil
        features = c.has_features ? sdrkitText(c.features) : nil
        controlChannelHz = c.has_control_channel_hz ? c.control_channel_hz : nil
        systemType = c.has_system_type ? sdrkitText(c.system_type) : nil
        network = sdrkitText(c.network)
        inverted = c.inverted
        swingHz = c.swing_hz
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        plan = c.has_plan ? sdrkitText(c.plan) : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MotorolaType2Fields, SDRKitCode> {
        var c = sdrkit_motorola_type2_fields()
        let code = sdrkit_motorola_type2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MotorolaType2Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A Type II message. The row key is the sequence number of the message.
public struct MotorolaType2MessageRow {
    /// The row key.
    public var key: String
    /// Signal time.
    public var timeS: Double
    /// An address.
    public var address: String
    /// A group word.
    public var group: Bool
    /// A command.
    public var command: String
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_motorola_type2_message_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        address = sdrkitText(c.address)
        group = c.group
        command = sdrkitText(c.command)
        text = sdrkitText(c.text)
    }
}

/// A Type II talkgroup. The row key is the talkgroup.
public struct MotorolaType2TalkgroupRow {
    /// The row key.
    public var key: String
    /// A talkgroup.
    public var talkgroup: String
    /// Grants counted.
    public var calls: Int64
    /// The last calling unit.
    public var lastSource: Int64?
    /// A frequency.
    public var freqHz: Double?
    /// The last time.
    public var lastS: Double
    /// Active.
    public var active: Bool
    /// Digital.
    public var digital: Bool
    /// Encrypted.
    public var encrypted: Bool
    /// Patched.
    public var patched: Bool

    init(_ c: sdrkit_motorola_type2_talkgroup_row) {
        key = sdrkitText(c.key)
        talkgroup = sdrkitText(c.talkgroup)
        calls = c.calls
        lastSource = c.has_last_source ? c.last_source : nil
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        lastS = c.last_s
        active = c.active
        digital = c.digital
        encrypted = c.encrypted
        patched = c.patched
    }
}

/// A Type II site. The row key is the system and the site.
public struct MotorolaType2SiteRow {
    /// The row key.
    public var key: String
    /// A system ID.
    public var system: String
    /// A site.
    public var site: Int64
    /// A band code.
    public var band: Int64
    /// Feature bits, as text.
    public var features: String
    /// A control channel frequency.
    public var controlChannelHz: Double?
    /// An adjacent site.
    public var adjacent: Bool

    init(_ c: sdrkit_motorola_type2_site_row) {
        key = sdrkitText(c.key)
        system = sdrkitText(c.system)
        site = c.site
        band = c.band
        features = sdrkitText(c.features)
        controlChannelHz = c.has_control_channel_hz ? c.control_channel_hz : nil
        adjacent = c.adjacent
    }
}

/// A member of a patch. The row key is the talkgroup and the member.
public struct MotorolaType2PatchRow {
    /// The row key.
    public var key: String
    /// A talkgroup.
    public var talkgroup: String
    /// A patch member.
    public var member: String
    /// The last time.
    public var lastS: Double

    init(_ c: sdrkit_motorola_type2_patch_row) {
        key = sdrkitText(c.key)
        talkgroup = sdrkitText(c.talkgroup)
        member = sdrkitText(c.member)
        lastS = c.last_s
    }
}

/// A Type II call. The row key is the talkgroup.
public struct MotorolaType2CallRow {
    /// The row key.
    public var key: String
    /// A talkgroup.
    public var talkgroup: String
    /// The source of a call.
    public var sourceUnit: Int64
    /// A frequency.
    public var freqHz: Double?
    /// Digital.
    public var digital: Bool
    /// Emergency.
    public var emergency: Bool
    /// Encrypted.
    public var encrypted: Bool
    /// The start of a call.
    public var startS: Double
    /// The length of a call.
    public var durationS: Double
    /// The state of a call.
    public var state: String
    /// A note.
    public var note: String?

    init(_ c: sdrkit_motorola_type2_call_row) {
        key = sdrkitText(c.key)
        talkgroup = sdrkitText(c.talkgroup)
        sourceUnit = c.source_unit
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        digital = c.digital
        emergency = c.emergency
        encrypted = c.encrypted
        startS = c.start_s
        durationS = c.duration_s
        state = sdrkitText(c.state)
        note = c.has_note ? sdrkitText(c.note) : nil
    }
}

/// The result fields of the OpenSky identifier.
public struct OpenskyFields {
    public enum Verdict: Int32 {
        case consistent = 0
        case other = 1
    }
    /// The state: 0 no input, 1 analysing, 2 a result.
    public var rxState: Int64
    /// Blocks analysed.
    public var blocks: Int64
    /// Symbol rate found, Hz.
    public var symbolRateHz: Double?
    /// The strength of the symbol rate line.
    public var lineDb: Double?
    /// The levels.
    public var levels: Int64?
    /// The deviation.
    public var deviationHz: Double?
    /// The burst period.
    public var burstMs: Double?
    /// The duty.
    public var duty: Double?
    /// The occupied width.
    public var widthHz: Double?
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    public var levelDbfs: Double?
    /// The verdict.
    public var verdict: Verdict?
    /// The text of a decoded message.
    public var text: String?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_opensky_fields) {
        rxState = c.rx_state
        blocks = c.blocks
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        lineDb = c.has_line_db ? c.line_db : nil
        levels = c.has_levels ? c.levels : nil
        deviationHz = c.has_deviation_hz ? c.deviation_hz : nil
        burstMs = c.has_burst_ms ? c.burst_ms : nil
        duty = c.has_duty ? c.duty : nil
        widthHz = c.has_width_hz ? c.width_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        verdict = c.has_verdict ? Verdict(rawValue: c.verdict) : nil
        text = c.has_text ? sdrkitText(c.text) : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<OpenskyFields, SDRKitCode> {
        var c = sdrkit_opensky_fields()
        let code = sdrkit_opensky_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(OpenskyFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The measures of one block. The row key is the sequence number of the block.
public struct OpenskyBlockRow {
    public enum Verdict: Int32 {
        case consistent = 0
        case other = 1
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// Symbol rate found, Hz.
    public var symbolRateHz: Double
    /// The levels.
    public var levels: Int64
    /// The occupied width.
    public var widthHz: Double
    /// The burst period.
    public var burstMs: Double
    /// The verdict.
    public var verdict: Verdict?
    /// The text of a decoded message.
    public var text: String

    init(_ c: sdrkit_opensky_block_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        symbolRateHz = c.symbol_rate_hz
        levels = c.levels
        widthHz = c.width_hz
        burstMs = c.burst_ms
        verdict = Verdict(rawValue: c.verdict)
        text = sdrkitText(c.text)
    }
}

/// The counts and the measurements of the GSC receiver.
public struct GscFields {
    /// The pages of every kind.
    public var pages: Int64
    /// The tone-only pages.
    public var tonePages: Int64
    /// The numeric pages.
    public var numericPages: Int64
    /// The alphanumeric pages.
    public var alphaPages: Int64
    /// The tone-and-voice pages.
    public var voicePages: Int64
    /// The preamble words seen (at most one a second).
    public var preambles: Int64
    /// The receiver holds block lock.
    public var locked: Bool
    /// The preamble index remembered (0 to 9).
    public var preamble: Int64?
    /// How the call is sent: "single call" or "batch" (known after the start code).
    public var call: String?
    /// The spectrum is inverted (from the last start code).
    public var inverted: Bool
    /// The deviation measured from the levels.
    public var deviationHz: Double?
    /// The offset of the carrier from the tuned frequency.
    public var offsetHz: Double?
    /// The error of the sender's bit clock.
    public var clockPpm: Double?
    /// The quality of the last block read while locked: the mean of the soft bits squared over their variance.
    public var qualityDb: Double?
    /// A carrier is present.
    public var carrier: Bool?
    /// The signal to noise ratio of the carrier.
    public var snrDb: Double?
    /// The level of the input.
    public var levelDbfs: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_gsc_fields) {
        pages = c.pages
        tonePages = c.tone_pages
        numericPages = c.numeric_pages
        alphaPages = c.alpha_pages
        voicePages = c.voice_pages
        preambles = c.preambles
        locked = c.locked
        preamble = c.has_preamble ? c.preamble : nil
        call = c.has_call ? sdrkitText(c.call) : nil
        inverted = c.inverted
        deviationHz = c.has_deviation_hz ? c.deviation_hz : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        qualityDb = c.has_quality_db ? c.quality_db : nil
        carrier = c.has_carrier ? c.carrier : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GscFields, SDRKitCode> {
        var c = sdrkit_gsc_fields()
        let code = sdrkit_gsc_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GscFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the GSC table: a page or the end of a voice page. The row key is the number of the row.
public struct GscPageRow {
    /// The row key.
    public var key: String
    /// The signal time of the row.
    public var timeS: Double
    /// What the row is: "Tone page", "Numeric page", "Alpha page", "Voice page" or "Voice end".
    public var kind: String
    /// The pager code: six digits, or N and five digits when no preamble was seen.
    public var code: String
    /// The address function, 1 to 4.
    public var function: Int64
    /// Which words of the address are inverted.
    public var words: String
    /// The preamble index (0 to 9).
    public var preamble: Int64?
    /// How the call was sent: "single call", "batch" or "no preamble seen".
    public var call: String
    /// The text of a data page, or "tone and voice".
    public var text: String
    /// The data blocks of a data page.
    public var blocks: Int64?
    /// A note: more blocks were announced, the voice is not decoded, how a voice page ended.
    public var note: String?
    /// The bits corrected in the two address words.
    public var bitsCorrected: Int64
    /// The activation word of a voice page, or the word that ended it.
    public var word: Int64?
    /// The length of the voice.
    public var voiceS: Double?

    init(_ c: sdrkit_gsc_page_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = sdrkitText(c.kind)
        code = sdrkitText(c.code)
        function = c.function
        words = sdrkitText(c.words)
        preamble = c.has_preamble ? c.preamble : nil
        call = sdrkitText(c.call)
        text = sdrkitText(c.text)
        blocks = c.has_blocks ? c.blocks : nil
        note = c.has_note ? sdrkitText(c.note) : nil
        bitsCorrected = c.bits_corrected
        word = c.has_word ? c.word : nil
        voiceS = c.has_voice_s ? c.voice_s : nil
    }
}

/// The counts, the link and the measurements of the SCADA receiver.
public struct ScadaFields {
    /// A chain of the modem holds lock.
    public var locked: Bool
    /// The link of the last good frame: the modem, the framing, the tones or deviation and the carrier offset.
    public var link: String?
    /// The messages kept.
    public var messages: Int64
    /// The Modbus messages (RTU and ASCII).
    public var modbus: Int64
    /// The DNP3 messages.
    public var dnp3: Int64
    /// The HDLC frames that hold neither protocol.
    public var unknown: Int64
    /// The frames whose CRC, LRC or block CRC failed.
    public var crcFailures: Int64
    /// The DNP3 Secure Authentication messages (shown raw).
    public var encrypted: Int64
    /// The locked bursts that gave no frame (vendor radio framings), kept as raw bits.
    public var notRecognised: Int64
    /// The devices heard.
    public var devices: Int64
    /// The carrier offset measured on the link of the last good frame.
    public var offsetHz: Double?
    /// The deviation measured on that link (the FM deviation on audio FSK, the outer one on 4-FSK).
    public var deviationHz: Double?
    /// The level of the channel.
    public var levelDbfs: Double?
    /// What no public document confirms, and what is shown raw.
    public var note: String

    init(_ c: sdrkit_scada_fields) {
        locked = c.locked
        link = c.has_link ? sdrkitText(c.link) : nil
        messages = c.messages
        modbus = c.modbus
        dnp3 = c.dnp3
        unknown = c.unknown
        crcFailures = c.crc_failures
        encrypted = c.encrypted
        notRecognised = c.not_recognised
        devices = c.devices
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        deviationHz = c.has_deviation_hz ? c.deviation_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ScadaFields, SDRKitCode> {
        var c = sdrkit_scada_fields()
        let code = sdrkit_scada_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ScadaFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the SCADA messages. The row key is the number of the row.
public struct ScadaMessageRow {
    /// The row key.
    public var key: String
    /// The message number.
    public var id: Int64
    /// The signal time.
    public var timeS: Double
    /// The wall-clock time, when the host gives a clock.
    public var wallClockUtc: Double?
    /// The link: the modem and the framing ("Bell 202 AFSK 1200 Bd, 8E1").
    public var link: String
    /// The protocol: "Modbus RTU", "Modbus ASCII", "DNP3" or "unknown".
    public var `protocol`: String
    /// The sender ("master", "slave 17", "outstation 1").
    public var from: String
    /// The receiver.
    public var to: String
    /// The function.
    public var function: String
    /// The summary: addresses, quantities, values, objects, indications.
    public var summary: String
    /// The check: "good", "failed", or empty when there is none.
    public var crc: String
    /// The direction: "request", "reply" or "unsolicited".
    public var direction: String?
    /// The id of the paired message.
    public var pairId: Int64?
    /// The time from the request to this reply.
    public var responseMs: Double?
    /// DNP3 Secure Authentication: shown raw.
    public var encrypted: Bool
    /// The bytes as hex, or the bits of a burst not recognised.
    public var rawHex: String
    /// -20 log10 of the eye error of the symbols.
    public var qualityDb: Double

    init(_ c: sdrkit_scada_message_row) {
        key = sdrkitText(c.key)
        id = c.id
        timeS = c.time_s
        wallClockUtc = c.has_wall_clock_utc ? c.wall_clock_utc : nil
        link = sdrkitText(c.link)
        `protocol` = sdrkitText(c.protocol)
        from = sdrkitText(c.from)
        to = sdrkitText(c.to)
        function = sdrkitText(c.function)
        summary = sdrkitText(c.summary)
        crc = sdrkitText(c.crc)
        direction = c.has_direction ? sdrkitText(c.direction) : nil
        pairId = c.has_pair_id ? c.pair_id : nil
        responseMs = c.has_response_ms ? c.response_ms : nil
        encrypted = c.encrypted
        rawHex = sdrkitText(c.raw_hex)
        qualityDb = c.quality_db
    }
}

/// A row of the SCADA devices. The row key is the protocol and the address ("modbus/17", "dnp3/1").
public struct ScadaDeviceRow {
    /// The row key.
    public var key: String
    /// "Modbus" or "DNP3".
    public var `protocol`: String
    /// The address.
    public var address: Int64
    /// The role: "slave", "master" or "outstation".
    public var role: String
    /// When it was first heard, wall clock.
    public var firstHeard: Double?
    /// When it was last heard, wall clock.
    public var lastHeard: Double?
    /// When it was first heard, signal time.
    public var firstS: Double
    /// When it was last heard, signal time.
    public var lastS: Double
    /// The last poll, signal time.
    public var lastPollS: Double?
    /// The polls sent or received.
    public var polls: Int64
    /// The replies sent or received.
    public var replies: Int64
    /// The polls that had no reply.
    public var unanswered: Int64
    /// The exception replies.
    public var exceptions: Int64
    /// The unsolicited responses.
    public var unsolicited: Int64
    /// The frames whose check failed.
    public var crcFailures: Int64
    /// The mean response time.
    public var meanResponseMs: Double?
    /// The last internal indications of a DNP3 outstation.
    public var lastIin: String?
    /// The addresses it talked with.
    public var peers: String
    /// The registers or points seen so far ("40108 = 555", "AI 0 = 1000").
    public var points: String

    init(_ c: sdrkit_scada_device_row) {
        key = sdrkitText(c.key)
        `protocol` = sdrkitText(c.protocol)
        address = c.address
        role = sdrkitText(c.role)
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        firstS = c.first_s
        lastS = c.last_s
        lastPollS = c.has_last_poll_s ? c.last_poll_s : nil
        polls = c.polls
        replies = c.replies
        unanswered = c.unanswered
        exceptions = c.exceptions
        unsolicited = c.unsolicited
        crcFailures = c.crc_failures
        meanResponseMs = c.has_mean_response_ms ? c.mean_response_ms : nil
        lastIin = c.has_last_iin ? sdrkitText(c.last_iin) : nil
        peers = sdrkitText(c.peers)
        points = sdrkitText(c.points)
    }
}

/// The reply of `export_csv`.
public struct ScadaCsvReply {
    /// The messages as CSV text: the header line, then one line a message.
    public var csv: String

    init(_ c: sdrkit_scada_csv_reply) {
        csv = sdrkitText(c.csv)
    }
}

/// An address codeword: `source` is the caller, `target` the called address, `channel` the channel named, `text` the meaning.
public struct Mpt1327MessageEvent {
    public enum TypeOption: Int32 {
        case gTC = 0
        case aLH = 1
        case aLHS = 2
        case aLHD = 3
        case aLHE = 4
        case aLHR = 5
        case aLHX = 6
        case aLHF = 7
        case aCK = 8
        case aCKI = 9
        case aCKQ = 10
        case aCKX = 11
        case aCKV = 12
        case aCKE = 13
        case aCKT = 14
        case aCKB = 15
        case aHY = 16
        case aHYX = 17
        case aHYP = 18
        case aHYQ = 19
        case aHYC = 20
        case mARK = 21
        case mAINT = 22
        case cLEAR = 23
        case mOVE = 24
        case bCAST = 25
        case hEAD = 26
        case sAM = 27
        case sTDDATA = 28
        case sTDACK = 29
        case spare = 30
        case rSVD = 31
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
    /// The type of a message.
    public var type: TypeOption?
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String?
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    public var target: String?
    /// The channel or talk group of a call.
    public var channel: String?
    /// The text of a decoded message.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_mpt1327_message_event) {
        time = c.time
        freqHz = c.freq_hz
        type = TypeOption(rawValue: c.type)
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Mpt1327MessageEvent, SDRKitCode> {
        var c = sdrkit_mpt1327_message_event()
        let code = sdrkit_mpt1327_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Mpt1327MessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A follow decision. `source` is the caller, `target` the talkgroup or unit, `channel` the channel number.
public struct TrunkFollowEvent {
    public enum Action: Int32 {
        case follow = 0
        case skipped = 1
        case release = 2
    }
    public enum Reason: Int32 {
        case encrypted = 0
        case noFrequency = 1
        case notListed = 2
        case busy = 3
        case ended = 4
        case replaced = 5
        case off = 6
    }
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
    /// The action of a follow decision.
    public var action: Action?
    /// The reason of a skip or a release.
    public var reason: Reason?
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String?
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    public var target: String?
    /// The channel or talk group of a call.
    public var channel: String?
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_trunk_follow_event) {
        time = c.time
        freqHz = c.freq_hz
        action = Action(rawValue: c.action)
        reason = c.has_reason ? Reason(rawValue: c.reason) : nil
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<TrunkFollowEvent, SDRKitCode> {
        var c = sdrkit_trunk_follow_event()
        let code = sdrkit_trunk_follow_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(TrunkFollowEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A good LTR word: `target` is the talkgroup.
public struct LtrWordEvent {
    public enum Kind: Int32 {
        case call = 0
        case idle = 1
        case callsignId = 2
        case busy = 3
        case token = 4
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
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    public var target: String
    /// The AREA bit.
    public var area: Int64
    /// The GO-TO repeater.
    public var goto: Int64
    /// The HOME repeater.
    public var home: Int64
    /// The ID.
    public var id: Int64
    /// The FREE repeater.
    public var free: Int64
    /// What the ID says.
    public var kind: Kind?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_ltr_word_event) {
        time = c.time
        freqHz = c.freq_hz
        target = sdrkitText(c.target)
        area = c.area
        goto = c.goto_
        home = c.home
        id = c.id
        free = c.free
        kind = Kind(rawValue: c.kind)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<LtrWordEvent, SDRKitCode> {
        var c = sdrkit_ltr_word_event()
        let code = sdrkit_ltr_word_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(LtrWordEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An EDACS call that started or ended.
public struct EdacsCallEvent {
    public enum End: Int32 {
        case active = 0
        case terminator = 1
        case hangTime = 2
        case replaced = 3
        case closed = 4
        case single = 5
    }
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
    /// Where a call is in its life.
    public var end: End?
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String?
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    public var target: String?
    /// The channel or talk group of a call.
    public var channel: String
    /// A group call.
    public var group: Bool
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// A call that no message announced.
    public var carrierOnly: Bool
    /// The length of a call.
    public var duration: Double
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_edacs_call_event) {
        time = c.time
        freqHz = c.freq_hz
        end = End(rawValue: c.end)
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        channel = sdrkitText(c.channel)
        group = c.group
        urgency = Urgency(rawValue: c.urgency)
        carrierOnly = c.carrier_only
        duration = c.duration
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<EdacsCallEvent, SDRKitCode> {
        var c = sdrkit_edacs_call_event()
        let code = sdrkit_edacs_call_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(EdacsCallEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A POCSAG page: `source` is the RIC, `text` the message.
public struct PagerPocsagPageEvent {
    public enum TypeOption: Int32 {
        case tone = 0
        case numeric = 1
        case alpha = 2
        case secure = 3
        case binary = 4
        case instruction = 5
        case selcall = 6
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
    /// The RIC.
    public var source: String
    /// The message.
    public var text: String
    /// The function code.
    public var function: Int64?
    /// The bit rate.
    public var baud: Int64?
    /// What the page carries.
    public var type: TypeOption?
    /// A word was lost or the end is not sure.
    public var damaged: Bool
    /// The kind of thing that the event is about.
    public var family: Family?

    init(_ c: sdrkit_pager_pocsag_page_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        function = c.has_function ? c.function : nil
        baud = c.has_baud ? c.baud : nil
        type = TypeOption(rawValue: c.type)
        damaged = c.damaged
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<PagerPocsagPageEvent, SDRKitCode> {
        var c = sdrkit_pager_pocsag_page_event()
        let code = sdrkit_pager_pocsag_page_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(PagerPocsagPageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A FLEX page: `source` is the capcode, `text` the message.
public struct PagerFlexPageEvent {
    public enum TypeOption: Int32 {
        case tone = 0
        case numeric = 1
        case alpha = 2
        case secure = 3
        case binary = 4
        case instruction = 5
        case selcall = 6
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
    /// The capcode.
    public var source: String
    /// The message.
    public var text: String
    /// The FLEX speed ("FLEX 3200/4").
    public var speed: String
    /// What the page carries.
    public var type: TypeOption?
    /// A word was lost or the end is not sure.
    public var damaged: Bool
    /// The kind of thing that the event is about.
    public var family: Family?

    init(_ c: sdrkit_pager_flex_page_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        speed = sdrkitText(c.speed)
        type = TypeOption(rawValue: c.type)
        damaged = c.damaged
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<PagerFlexPageEvent, SDRKitCode> {
        var c = sdrkit_pager_flex_page_event()
        let code = sdrkit_pager_flex_page_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(PagerFlexPageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of a POCSAG transmission.
public struct PagerTxEvent {
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
    /// The state: started, queued, finished or refused.
    public var state: String
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a page was refused.
    public var reason: String
    /// The addresses of the pages, separated by commas.
    public var target: String
    /// The text of the page for started, queued and refused; empty for finished.
    public var text: String
    /// The family of the source: a pager.
    public var family: Family?

    init(_ c: sdrkit_pager_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<PagerTxEvent, SDRKitCode> {
        var c = sdrkit_pager_tx_event()
        let code = sdrkit_pager_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(PagerTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A good ERMES batch.
public struct ErmesBatchEvent {
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
    /// The batch letter.
    public var batch: String
    /// The network.
    public var network: String
    /// The initial addresses, decimal.
    public var text: String
    /// The kind of thing that the event is about.
    public var family: Family?

    init(_ c: sdrkit_ermes_batch_event) {
        time = c.time
        freqHz = c.freq_hz
        batch = sdrkitText(c.batch)
        network = sdrkitText(c.network)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ErmesBatchEvent, SDRKitCode> {
        var c = sdrkit_ermes_batch_event()
        let code = sdrkit_ermes_batch_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ErmesBatchEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// An ERMES message.
public struct ErmesMessageEvent {
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
    /// The local address.
    public var target: String?
    /// The category.
    public var category: String?
    /// The text of the message.
    public var text: String
    /// The kind of thing that the event is about.
    public var family: Family?

    init(_ c: sdrkit_ermes_message_event) {
        time = c.time
        freqHz = c.freq_hz
        target = c.has_target ? sdrkitText(c.target) : nil
        category = c.has_category ? sdrkitText(c.category) : nil
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ErmesMessageEvent, SDRKitCode> {
        var c = sdrkit_ermes_message_event()
        let code = sdrkit_ermes_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ErmesMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A finished transmission.
public struct MultitoneTransmissionEvent {
    public enum Kind: Int32 {
        case ffskData = 0
        case fskData = 1
        case toneSequence = 2
        case repeating = 3
        case multiTone = 4
        case other = 5
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
    /// Kind.
    public var kind: Kind?
    /// Duration.
    public var duration: Double
    /// SNR.
    public var snrDb: Double
    /// Deviation.
    public var deviationHz: Double
    /// The text of a decoded message.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_multitone_transmission_event) {
        time = c.time
        freqHz = c.freq_hz
        kind = Kind(rawValue: c.kind)
        duration = c.duration
        snrDb = c.snr_db
        deviationHz = c.deviation_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MultitoneTransmissionEvent, SDRKitCode> {
        var c = sdrkit_multitone_transmission_event()
        let code = sdrkit_multitone_transmission_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MultitoneTransmissionEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A finished FSK burst.
public struct MultitoneBurstEvent {
    public enum Kind: Int32 {
        case ffsk = 0
        case fsk = 1
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
    /// Kind.
    public var kind: Kind?
    /// Baud.
    public var baud: Double
    /// Bits.
    public var bits: Int64
    /// Valid BCH words.
    public var bchValid: Int64
    /// The text of a decoded message.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_multitone_burst_event) {
        time = c.time
        freqHz = c.freq_hz
        kind = Kind(rawValue: c.kind)
        baud = c.baud
        bits = c.bits
        bchValid = c.bch_valid
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MultitoneBurstEvent, SDRKitCode> {
        var c = sdrkit_multitone_burst_event()
        let code = sdrkit_multitone_burst_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MultitoneBurstEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A Type II message.
public struct MotorolaType2MessageEvent {
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
    /// An address.
    public var address: String
    /// A group word.
    public var group: Bool
    /// A command.
    public var command: String
    /// The text of a decoded message.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_motorola_type2_message_event) {
        time = c.time
        freqHz = c.freq_hz
        address = sdrkitText(c.address)
        group = c.group
        command = sdrkitText(c.command)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MotorolaType2MessageEvent, SDRKitCode> {
        var c = sdrkit_motorola_type2_message_event()
        let code = sdrkit_motorola_type2_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MotorolaType2MessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A voice grant: `target` is the talkgroup, `source` the caller, `channel` the channel number.
public struct TrunkGrantEvent {
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
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    public var target: String
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    public var source: String?
    /// The channel or talk group of a call.
    public var channel: String
    /// A group word.
    public var group: Bool
    /// Emergency.
    public var emergency: Bool
    /// Encrypted.
    public var encrypted: Bool
    /// Digital.
    public var digital: Bool
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    public var urgency: Urgency?
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_trunk_grant_event) {
        time = c.time
        freqHz = c.freq_hz
        target = sdrkitText(c.target)
        source = c.has_source ? sdrkitText(c.source) : nil
        channel = sdrkitText(c.channel)
        group = c.group
        emergency = c.emergency
        encrypted = c.encrypted
        digital = c.digital
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<TrunkGrantEvent, SDRKitCode> {
        var c = sdrkit_trunk_grant_event()
        let code = sdrkit_trunk_grant_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(TrunkGrantEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The verdict of one block.
public struct OpenskyVerdictEvent {
    public enum Verdict: Int32 {
        case consistent = 0
        case other = 1
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
    /// The verdict.
    public var verdict: Verdict?
    /// Symbol rate found, Hz.
    public var symbolRateHz: Double
    /// The levels.
    public var levels: Int64
    /// The deviation.
    public var deviationHz: Double
    /// The burst period.
    public var burstMs: Double
    /// The occupied width.
    public var widthHz: Double
    /// The text of a decoded message.
    public var text: String
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    public var family: Family?

    init(_ c: sdrkit_opensky_verdict_event) {
        time = c.time
        freqHz = c.freq_hz
        verdict = Verdict(rawValue: c.verdict)
        symbolRateHz = c.symbol_rate_hz
        levels = c.levels
        deviationHz = c.deviation_hz
        burstMs = c.burst_ms
        widthHz = c.width_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<OpenskyVerdictEvent, SDRKitCode> {
        var c = sdrkit_opensky_verdict_event()
        let code = sdrkit_opensky_verdict_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(OpenskyVerdictEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A GSC page: `target` is the pager code.
public struct GscPageEvent {
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
    /// The pager code.
    public var target: String
    /// The kind of page.
    public var kind: String
    /// The address function, 1 to 4.
    public var function: Int64
    /// The text of a data page.
    public var text: String
    /// The kind of thing that the event is about.
    public var family: Family?

    init(_ c: sdrkit_gsc_page_event) {
        time = c.time
        freqHz = c.freq_hz
        target = sdrkitText(c.target)
        kind = sdrkitText(c.kind)
        function = c.function
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<GscPageEvent, SDRKitCode> {
        var c = sdrkit_gsc_page_event()
        let code = sdrkit_gsc_page_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(GscPageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A SCADA message: `source` is the sender, `target` the receiver, `text` the summary.
public struct ScadaMessageEvent {
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
    /// The sender.
    public var source: String
    /// The receiver.
    public var target: String
    /// The protocol.
    public var `protocol`: String
    /// The function.
    public var function: String
    /// The summary.
    public var text: String
    /// The check: "good", "failed", or empty.
    public var crc: String
    /// The kind of thing that the event is about.
    public var family: Family?

    init(_ c: sdrkit_scada_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        `protocol` = sdrkitText(c.protocol)
        function = sdrkitText(c.function)
        text = sdrkitText(c.text)
        crc = sdrkitText(c.crc)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ScadaMessageEvent, SDRKitCode> {
        var c = sdrkit_scada_message_event()
        let code = sdrkit_scada_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ScadaMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A new SCADA device.
public struct ScadaDeviceEvent {
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
    /// "Modbus" or "DNP3".
    public var `protocol`: String
    /// The address.
    public var address: Int64
    /// The role.
    public var role: String
    /// The kind of thing that the event is about.
    public var family: Family?

    init(_ c: sdrkit_scada_device_event) {
        time = c.time
        freqHz = c.freq_hz
        `protocol` = sdrkitText(c.protocol)
        address = c.address
        role = sdrkitText(c.role)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ScadaDeviceEvent, SDRKitCode> {
        var c = sdrkit_scada_device_event()
        let code = sdrkit_scada_device_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ScadaDeviceEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "ermes" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct ErmesDropped {
    /// The items that the component "modem" dropped.
    public var modem: Int64?

    init(_ c: sdrkit_ermes_dropped) {
        modem = c.has_modem ? c.modem : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ErmesDropped, SDRKitCode> {
        var c = sdrkit_ermes_dropped()
        let code = sdrkit_ermes_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(ErmesDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "gsc" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct GscDropped {
    /// The items that the component "modem" dropped.
    public var modem: Int64?

    init(_ c: sdrkit_gsc_dropped) {
        modem = c.has_modem ? c.modem : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<GscDropped, SDRKitCode> {
        var c = sdrkit_gsc_dropped()
        let code = sdrkit_gsc_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(GscDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "scada" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct ScadaDropped {
    /// The items that the component "modem" dropped.
    public var modem: Int64?

    init(_ c: sdrkit_scada_dropped) {
        modem = c.has_modem ? c.modem : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ScadaDropped, SDRKitCode> {
        var c = sdrkit_scada_dropped()
        let code = sdrkit_scada_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(ScadaDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum TrunkTables {
    /// The changed rows of the table "messages" of the stack "mpt1327", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time_s`.
    public static func mpt1327Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[Mpt1327MessageRow], SDRKitCode> {
        var rows = [sdrkit_mpt1327_message_row](repeating: sdrkit_mpt1327_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mpt1327_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Mpt1327MessageRow.init))
    }
    /// The changed rows of the table "channels" of the stack "mpt1327", newest first, at most `capacity` rows.
    public static func mpt1327Channels(_ handle: SDRKIT_HANDLE, capacity: Int = 2048) -> Result<[Mpt1327ChannelRow], SDRKitCode> {
        var rows = [sdrkit_mpt1327_channel_row](repeating: sdrkit_mpt1327_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mpt1327_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Mpt1327ChannelRow.init))
    }
    /// The changed rows of the table "words" of the stack "ltr", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `talkgroup` names a row.
    /// Time: `time_s`.
    public static func ltrWords(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[LtrWordRow], SDRKitCode> {
        var rows = [sdrkit_ltr_word_row](repeating: sdrkit_ltr_word_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ltr_words_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LtrWordRow.init))
    }
    /// The changed rows of the table "repeaters" of the stack "ltr", newest first, at most `capacity` rows.
    public static func ltrRepeaters(_ handle: SDRKIT_HANDLE, capacity: Int = 20) -> Result<[LtrRepeaterRow], SDRKitCode> {
        var rows = [sdrkit_ltr_repeater_row](repeating: sdrkit_ltr_repeater_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ltr_repeaters_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LtrRepeaterRow.init))
    }
    /// The changed rows of the table "calls" of the stack "edacs", newest first, at most `capacity` rows.
    /// The column `source` names a row.
    public static func edacsCalls(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[EdacsCallRow], SDRKitCode> {
        var rows = [sdrkit_edacs_call_row](repeating: sdrkit_edacs_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_edacs_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EdacsCallRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "edacs", newest first, at most `capacity` rows.
    public static func edacsTalkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[EdacsTalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_edacs_talkgroup_row](repeating: sdrkit_edacs_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_edacs_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EdacsTalkgroupRow.init))
    }
    /// The changed rows of the table "channels" of the stack "edacs", newest first, at most `capacity` rows.
    public static func edacsChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[EdacsChannelRow], SDRKitCode> {
        var rows = [sdrkit_edacs_channel_row](repeating: sdrkit_edacs_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_edacs_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EdacsChannelRow.init))
    }
    /// The changed rows of the table "frames" of the stack "edacs", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time_s`.
    public static func edacsFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[EdacsFrameRow], SDRKitCode> {
        var rows = [sdrkit_edacs_frame_row](repeating: sdrkit_edacs_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_edacs_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EdacsFrameRow.init))
    }
    /// The changed rows of the table "words" of the stack "edacs", newest first, at most `capacity` rows.
    public static func edacsWords(_ handle: SDRKIT_HANDLE, capacity: Int = 24) -> Result<[EdacsWordRow], SDRKitCode> {
        var rows = [sdrkit_edacs_word_row](repeating: sdrkit_edacs_word_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_edacs_words_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EdacsWordRow.init))
    }
    /// The changed rows of the table "events" of the stack "tones", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `start_s`.
    public static func tonesEvents(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[TonesHeardRow], SDRKitCode> {
        var rows = [sdrkit_tones_heard_row](repeating: sdrkit_tones_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_tones_events_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(TonesHeardRow.init))
    }
    /// The changed rows of the table "pages" of the stack "pager", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// Addressee: `address`. Text: `message`. Time: `time_s`.
    public static func pagerPages(_ handle: SDRKIT_HANDLE, capacity: Int = 2000) -> Result<[PagerPageRow], SDRKitCode> {
        var rows = [sdrkit_pager_page_row](repeating: sdrkit_pager_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pager_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PagerPageRow.init))
    }
    /// The changed rows of the table "speeds" of the stack "pager", newest first, at most `capacity` rows.
    public static func pagerSpeeds(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[PagerSpeedRow], SDRKitCode> {
        var rows = [sdrkit_pager_speed_row](repeating: sdrkit_pager_speed_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pager_speeds_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PagerSpeedRow.init))
    }
    /// The changed rows of the table "heard" of the stack "pager", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// Time: `last_heard`.
    public static func pagerHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[PagerHeardRow], SDRKitCode> {
        var rows = [sdrkit_pager_heard_row](repeating: sdrkit_pager_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_pager_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(PagerHeardRow.init))
    }
    /// The changed rows of the table "rows" of the stack "ermes", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// Addressee: `address`. Text: `text`. Time: `time_s`.
    public static func ermesRows(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[ErmesEntryRow], SDRKitCode> {
        var rows = [sdrkit_ermes_entry_row](repeating: sdrkit_ermes_entry_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ermes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ErmesEntryRow.init))
    }
    /// The changed rows of the table "transmissions" of the stack "multitone", newest first, at most `capacity` rows.
    /// The rows are the calls of the stack.
    /// Time: `time_s`.
    public static func multitoneTransmissions(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MultitoneTransmissionRow], SDRKitCode> {
        var rows = [sdrkit_multitone_transmission_row](repeating: sdrkit_multitone_transmission_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_multitone_transmissions_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MultitoneTransmissionRow.init))
    }
    /// The changed rows of the table "tx_tones" of the stack "multitone", newest first, at most `capacity` rows.
    public static func multitoneTxTones(_ handle: SDRKIT_HANDLE, capacity: Int = 2000) -> Result<[MultitoneToneRow], SDRKitCode> {
        var rows = [sdrkit_multitone_tone_row](repeating: sdrkit_multitone_tone_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_multitone_tx_tones_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MultitoneToneRow.init))
    }
    /// The changed rows of the table "tx_lines" of the stack "multitone", newest first, at most `capacity` rows.
    public static func multitoneTxLines(_ handle: SDRKIT_HANDLE, capacity: Int = 2000) -> Result<[MultitoneLineRow], SDRKitCode> {
        var rows = [sdrkit_multitone_line_row](repeating: sdrkit_multitone_line_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_multitone_tx_lines_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MultitoneLineRow.init))
    }
    /// The changed rows of the table "tones_heard" of the stack "multitone", newest first, at most `capacity` rows.
    public static func multitoneTonesHeard(_ handle: SDRKIT_HANDLE, capacity: Int = 24) -> Result<[MultitoneHeardRow], SDRKitCode> {
        var rows = [sdrkit_multitone_heard_row](repeating: sdrkit_multitone_heard_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_multitone_tones_heard_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MultitoneHeardRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "multitone", newest first, at most `capacity` rows.
    public static func multitoneBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MultitoneBurstRow], SDRKitCode> {
        var rows = [sdrkit_multitone_burst_row](repeating: sdrkit_multitone_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_multitone_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MultitoneBurstRow.init))
    }
    /// The changed rows of the table "messages" of the stack "motorola_type2", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `address` names a row.
    /// Time: `time_s`.
    public static func motorolaType2Messages(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[MotorolaType2MessageRow], SDRKitCode> {
        var rows = [sdrkit_motorola_type2_message_row](repeating: sdrkit_motorola_type2_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_motorola_type2_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MotorolaType2MessageRow.init))
    }
    /// The changed rows of the table "talkgroups" of the stack "motorola_type2", newest first, at most `capacity` rows.
    /// The column `talkgroup` names a row.
    public static func motorolaType2Talkgroups(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MotorolaType2TalkgroupRow], SDRKitCode> {
        var rows = [sdrkit_motorola_type2_talkgroup_row](repeating: sdrkit_motorola_type2_talkgroup_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_motorola_type2_talkgroups_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MotorolaType2TalkgroupRow.init))
    }
    /// The changed rows of the table "sites" of the stack "motorola_type2", newest first, at most `capacity` rows.
    public static func motorolaType2Sites(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MotorolaType2SiteRow], SDRKitCode> {
        var rows = [sdrkit_motorola_type2_site_row](repeating: sdrkit_motorola_type2_site_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_motorola_type2_sites_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MotorolaType2SiteRow.init))
    }
    /// The changed rows of the table "patches" of the stack "motorola_type2", newest first, at most `capacity` rows.
    /// The column `talkgroup` names a row.
    public static func motorolaType2Patches(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[MotorolaType2PatchRow], SDRKitCode> {
        var rows = [sdrkit_motorola_type2_patch_row](repeating: sdrkit_motorola_type2_patch_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_motorola_type2_patches_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MotorolaType2PatchRow.init))
    }
    /// The changed rows of the table "calls" of the stack "motorola_type2", newest first, at most `capacity` rows.
    /// The column `talkgroup` names a row.
    public static func motorolaType2Calls(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MotorolaType2CallRow], SDRKitCode> {
        var rows = [sdrkit_motorola_type2_call_row](repeating: sdrkit_motorola_type2_call_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_motorola_type2_calls_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MotorolaType2CallRow.init))
    }
    /// The changed rows of the table "blocks" of the stack "opensky", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time_s`.
    public static func openskyBlocks(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[OpenskyBlockRow], SDRKitCode> {
        var rows = [sdrkit_opensky_block_row](repeating: sdrkit_opensky_block_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_opensky_blocks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(OpenskyBlockRow.init))
    }
    /// The changed rows of the table "pages" of the stack "gsc", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `call` names a row.
    /// Addressee: `code`. Text: `text`. Time: `time_s`.
    public static func gscPages(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[GscPageRow], SDRKitCode> {
        var rows = [sdrkit_gsc_page_row](repeating: sdrkit_gsc_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_gsc_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(GscPageRow.init))
    }
    /// The changed rows of the table "messages" of the stack "scada", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `from` names a row.
    /// Time: `wall_clock_utc`.
    public static func scadaMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[ScadaMessageRow], SDRKitCode> {
        var rows = [sdrkit_scada_message_row](repeating: sdrkit_scada_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_scada_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScadaMessageRow.init))
    }
    /// The changed rows of the table "devices" of the stack "scada", newest first, at most `capacity` rows.
    public static func scadaDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[ScadaDeviceRow], SDRKitCode> {
        var rows = [sdrkit_scada_device_row](repeating: sdrkit_scada_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_scada_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ScadaDeviceRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum TrunkCommands {
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func ltrClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ltr_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func edacsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_edacs_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func tonesClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tones_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func pagerClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_pager_clear(handle)
        return sdrkitResult(code)
    }
    /// Send a page. Checks the page and queues it. A transmission starts when none is on air; pages sent during a transmission go in the next one. POCSAG only: with the setting protocol on FLEX it is refused. Transmit must be on. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func pagerSend(_ handle: SDRKIT_HANDLE, address: Int64, function: Int64? = nil, pageType: Int32? = nil, text: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_pager_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_pager_send_args>.size)
        let code: Int32 = sdrkitWithString(text ?? "") { text_ in { () -> Int32 in
        args.address = address
        args.has_function = function != nil
        if let given = function { args.function = given }
        args.has_page_type = pageType != nil
        if let given = pageType { args.page_type = given }
        args.has_text = text != nil
        args.text = text_
        return sdrkit_pager_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Halt. Drops the pages waiting and ends the transmission after the current bit with a 1 ms fall.
    public static func pagerHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_pager_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func ermesClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ermes_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func multitoneClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_multitone_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func motorolaType2Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_motorola_type2_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func openskyClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_opensky_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func gscClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_gsc_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func scadaClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_scada_clear(handle)
        return sdrkitResult(code)
    }
    /// Export the messages as CSV. Replies with the messages as CSV text: id, time, wall clock, link, protocol, from, to, function, summary, CRC, direction, pair, response time, encrypted, raw hex.
    public static func scadaExportCsv(_ handle: SDRKIT_HANDLE) -> Result<ScadaCsvReply, SDRKitCode> {
        var reply = sdrkit_scada_csv_reply()
        let code = sdrkit_scada_export_csv(handle, &reply)
        return code == SDRKIT_OK ? .success(ScadaCsvReply(reply)) : .failure(SDRKitCode(code: code))
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum TrunkSettings {
    /// The options of the setting "framing" of the stack "mpt1327".
    public enum Mpt1327Framing: Int32 {
        case standard = 0
        case tolerant = 1
        case auto = 2
    }
    /// Framing.
    public static func mpt1327Framing(_ handle: SDRKIT_HANDLE) -> Result<Mpt1327Framing, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "framing", &v)
        return code == SDRKIT_OK ? .success(Mpt1327Framing(rawValue: v) ?? Mpt1327Framing(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMpt1327Framing(_ handle: SDRKIT_HANDLE, _ value: Mpt1327Framing) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "framing", value.rawValue))
    }
    /// Control channel number.
    public static func mpt1327ControlChannel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "control_channel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMpt1327ControlChannel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "control_channel", value))
    }
    /// Channel spacing.
    public static func mpt1327SpacingKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "spacing_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMpt1327SpacingKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "spacing_khz", value))
    }
    /// Follow calls.
    public static func mpt1327Follow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMpt1327Follow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// Follow groups.
    public static func mpt1327FollowGroups(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "follow_groups", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMpt1327FollowGroups(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "follow_groups", $0) })
    }
    /// Longest follow.
    public static func mpt1327FollowMaxS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "follow_max_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMpt1327FollowMaxS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "follow_max_s", value))
    }
    /// Follow calls.
    public static func ltrFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLtrFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// Follow groups.
    public static func ltrFollowGroups(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "follow_groups", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLtrFollowGroups(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "follow_groups", $0) })
    }
    /// Longest follow.
    public static func ltrFollowMaxS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "follow_max_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLtrFollowMaxS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "follow_max_s", value))
    }
    /// Extended Addressing.
    public static func edacsExtended(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "extended", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEdacsExtended(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "extended", value))
    }
    /// ESK.
    public static func edacsEsk(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "esk", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEdacsEsk(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "esk", value))
    }
    /// Voice channel table.
    public static func edacsLcnTable(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "lcn_table", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEdacsLcnTable(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "lcn_table", $0) })
    }
    /// Follow calls.
    public static func edacsFollow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEdacsFollow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// Also follow carriers.
    public static func edacsFollowCarriers(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_carriers", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEdacsFollowCarriers(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_carriers", value))
    }
    /// Follow groups.
    public static func edacsFollowGroups(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "follow_groups", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEdacsFollowGroups(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "follow_groups", $0) })
    }
    /// The options of the setting "demod" of the stack "tones".
    public enum TonesDemod: Int32 {
        case auto = 0
        case fm = 1
        case usb = 2
        case lsb = 3
        case am = 4
    }
    /// Demodulation.
    public static func tonesDemod(_ handle: SDRKIT_HANDLE) -> Result<TonesDemod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "demod", &v)
        return code == SDRKIT_OK ? .success(TonesDemod(rawValue: v) ?? TonesDemod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setTonesDemod(_ handle: SDRKIT_HANDLE, _ value: TonesDemod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "demod", value.rawValue))
    }
    /// Audio input.
    public static func tonesAudioInput(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "audio_input", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTonesAudioInput(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "audio_input", value))
    }
    /// The options of the setting "gate" of the stack "tones".
    public enum TonesGate: Int32 {
        case always = 0
        case any = 1
        case chosen = 2
    }
    /// Gate.
    public static func tonesGate(_ handle: SDRKIT_HANDLE) -> Result<TonesGate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gate", &v)
        return code == SDRKIT_OK ? .success(TonesGate(rawValue: v) ?? TonesGate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setTonesGate(_ handle: SDRKIT_HANDLE, _ value: TonesGate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gate", value.rawValue))
    }
    /// The options of the setting "gate_kind" of the stack "tones".
    public enum TonesGateKind: Int32 {
        case ctcss = 0
        case dcs = 1
    }
    /// Gate on.
    public static func tonesGateKind(_ handle: SDRKIT_HANDLE) -> Result<TonesGateKind, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gate_kind", &v)
        return code == SDRKIT_OK ? .success(TonesGateKind(rawValue: v) ?? TonesGateKind(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setTonesGateKind(_ handle: SDRKIT_HANDLE, _ value: TonesGateKind) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gate_kind", value.rawValue))
    }
    /// The options of the setting "gate_tone" of the stack "tones".
    public enum TonesGateTone: Int32 {
        case n670 = 0
        case n693 = 1
        case n719 = 2
        case n744 = 3
        case n770 = 4
        case n797 = 5
        case n825 = 6
        case n854 = 7
        case n885 = 8
        case n915 = 9
        case n948 = 10
        case n974 = 11
        case n1000 = 12
        case n1035 = 13
        case n1072 = 14
        case n1109 = 15
        case n1148 = 16
        case n1188 = 17
        case n1230 = 18
        case n1273 = 19
        case n1318 = 20
        case n1365 = 21
        case n1413 = 22
        case n1462 = 23
        case n1514 = 24
        case n1567 = 25
        case n1598 = 26
        case n1622 = 27
        case n1655 = 28
        case n1679 = 29
        case n1713 = 30
        case n1738 = 31
        case n1773 = 32
        case n1799 = 33
        case n1835 = 34
        case n1862 = 35
        case n1899 = 36
        case n1928 = 37
        case n1966 = 38
        case n1995 = 39
        case n2035 = 40
        case n2065 = 41
        case n2107 = 42
        case n2181 = 43
        case n2257 = 44
        case n2291 = 45
        case n2336 = 46
        case n2418 = 47
        case n2503 = 48
        case n2541 = 49
    }
    /// Tone.
    public static func tonesGateTone(_ handle: SDRKIT_HANDLE) -> Result<TonesGateTone, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gate_tone", &v)
        return code == SDRKIT_OK ? .success(TonesGateTone(rawValue: v) ?? TonesGateTone(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setTonesGateTone(_ handle: SDRKIT_HANDLE, _ value: TonesGateTone) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gate_tone", value.rawValue))
    }
    /// The options of the setting "gate_code" of the stack "tones".
    public enum TonesGateCode: Int32 {
        case d023 = 0
        case d025 = 1
        case d026 = 2
        case d031 = 3
        case d032 = 4
        case d043 = 5
        case d047 = 6
        case d051 = 7
        case d054 = 8
        case d065 = 9
        case d071 = 10
        case d072 = 11
        case d073 = 12
        case d074 = 13
        case d114 = 14
        case d115 = 15
        case d116 = 16
        case d125 = 17
        case d131 = 18
        case d132 = 19
        case d134 = 20
        case d143 = 21
        case d152 = 22
        case d155 = 23
        case d156 = 24
        case d162 = 25
        case d165 = 26
        case d172 = 27
        case d174 = 28
        case d205 = 29
        case d223 = 30
        case d226 = 31
        case d243 = 32
        case d244 = 33
        case d245 = 34
        case d251 = 35
        case d261 = 36
        case d263 = 37
        case d265 = 38
        case d271 = 39
        case d306 = 40
        case d311 = 41
        case d315 = 42
        case d331 = 43
        case d343 = 44
        case d346 = 45
        case d351 = 46
        case d364 = 47
        case d365 = 48
        case d371 = 49
        case d411 = 50
        case d412 = 51
        case d413 = 52
        case d423 = 53
        case d431 = 54
        case d432 = 55
        case d445 = 56
        case d464 = 57
        case d465 = 58
        case d466 = 59
        case d503 = 60
        case d506 = 61
        case d516 = 62
        case d532 = 63
        case d546 = 64
        case d565 = 65
        case d606 = 66
        case d612 = 67
        case d624 = 68
        case d627 = 69
        case d631 = 70
        case d632 = 71
        case d654 = 72
        case d662 = 73
        case d664 = 74
        case d703 = 75
        case d712 = 76
        case d723 = 77
        case d731 = 78
        case d732 = 79
        case d734 = 80
        case d743 = 81
        case d754 = 82
    }
    /// Code.
    public static func tonesGateCode(_ handle: SDRKIT_HANDLE) -> Result<TonesGateCode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gate_code", &v)
        return code == SDRKIT_OK ? .success(TonesGateCode(rawValue: v) ?? TonesGateCode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setTonesGateCode(_ handle: SDRKIT_HANDLE, _ value: TonesGateCode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gate_code", value.rawValue))
    }
    /// Code inverted.
    public static func tonesGateInverted(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gate_inverted", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTonesGateInverted(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gate_inverted", value))
    }
    /// Listen.
    public static func tonesListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTonesListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// POCSAG.
    public static func pagerPocsag(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "pocsag", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerPocsag(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "pocsag", value))
    }
    /// FLEX.
    public static func pagerFlex(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "flex", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerFlex(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "flex", value))
    }
    /// POCSAG 512.
    public static func pagerPocsag512(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "pocsag_512", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerPocsag512(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "pocsag_512", value))
    }
    /// POCSAG 1200.
    public static func pagerPocsag1200(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "pocsag_1200", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerPocsag1200(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "pocsag_1200", value))
    }
    /// POCSAG 2400.
    public static func pagerPocsag2400(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "pocsag_2400", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerPocsag2400(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "pocsag_2400", value))
    }
    /// Sequential calls.
    public static func pagerSelcall(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "selcall", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerSelcall(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "selcall", value))
    }
    /// The options of the setting "channel" of the stack "pager".
    public enum PagerChannel: Int32 {
        case n25k = 0
        case n125k = 1
    }
    /// Channel.
    public static func pagerChannel(_ handle: SDRKIT_HANDLE) -> Result<PagerChannel, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(PagerChannel(rawValue: v) ?? PagerChannel(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerChannel(_ handle: SDRKIT_HANDLE, _ value: PagerChannel) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channel", value.rawValue))
    }
    /// Narrow second decoder.
    public static func pagerNarrowDecoder(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "narrow_decoder", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerNarrowDecoder(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "narrow_decoder", value))
    }
    /// Combine copies.
    public static func pagerCombine(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "combine", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerCombine(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "combine", value))
    }
    /// The options of the setting "read" of the stack "pager".
    public enum PagerRead: Int32 {
        case auto = 0
        case function = 1
        case alpha = 2
        case numeric = 3
    }
    /// Read POCSAG pages.
    public static func pagerRead(_ handle: SDRKIT_HANDLE) -> Result<PagerRead, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "read", &v)
        return code == SDRKIT_OK ? .success(PagerRead(rawValue: v) ?? PagerRead(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerRead(_ handle: SDRKIT_HANDLE, _ value: PagerRead) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "read", value.rawValue))
    }
    /// Read by address.
    public static func pagerReadByAddress(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "read_by_address", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerReadByAddress(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "read_by_address", $0) })
    }
    /// The options of the setting "charset" of the stack "pager".
    public enum PagerCharset: Int32 {
        case ascii = 0
        case german = 1
    }
    /// Character set.
    public static func pagerCharset(_ handle: SDRKIT_HANDLE) -> Result<PagerCharset, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "charset", &v)
        return code == SDRKIT_OK ? .success(PagerCharset(rawValue: v) ?? PagerCharset(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerCharset(_ handle: SDRKIT_HANDLE, _ value: PagerCharset) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "charset", value.rawValue))
    }
    /// One carrier.
    public static func pagerWideInput(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wide_input", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerWideInput(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wide_input", value))
    }
    /// The options of the setting "protocol" of the stack "pager".
    public enum PagerProtocol: Int32 {
        case pocsag = 0
        case flex = 1
    }
    /// Protocol.
    public static func pagerProtocol(_ handle: SDRKIT_HANDLE) -> Result<PagerProtocol, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "protocol", &v)
        return code == SDRKIT_OK ? .success(PagerProtocol(rawValue: v) ?? PagerProtocol(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerProtocol(_ handle: SDRKIT_HANDLE, _ value: PagerProtocol) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "protocol", value.rawValue))
    }
    /// The options of the setting "pocsag_rate" of the stack "pager".
    public enum PagerPocsagRate: Int32 {
        case n512BitS = 0
        case n1200BitS = 1
        case n2400BitS = 2
    }
    /// Speed.
    public static func pagerPocsagRate(_ handle: SDRKIT_HANDLE) -> Result<PagerPocsagRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "pocsag_rate", &v)
        return code == SDRKIT_OK ? .success(PagerPocsagRate(rawValue: v) ?? PagerPocsagRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerPocsagRate(_ handle: SDRKIT_HANDLE, _ value: PagerPocsagRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "pocsag_rate", value.rawValue))
    }
    /// Deviation.
    public static func pagerDeviation(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "deviation", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerDeviation(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "deviation", value))
    }
    /// The options of the setting "polarity" of the stack "pager".
    public enum PagerPolarity: Int32 {
        case normal = 0
        case inverted = 1
    }
    /// Polarity.
    public static func pagerPolarity(_ handle: SDRKIT_HANDLE) -> Result<PagerPolarity, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "polarity", &v)
        return code == SDRKIT_OK ? .success(PagerPolarity(rawValue: v) ?? PagerPolarity(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerPolarity(_ handle: SDRKIT_HANDLE, _ value: PagerPolarity) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "polarity", value.rawValue))
    }
    /// Level.
    public static func pagerTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setPagerTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Channel plan.
    public static func motorolaType2Plan(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "plan", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMotorolaType2Plan(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "plan", $0) })
    }
    /// Follow calls.
    public static func motorolaType2Follow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMotorolaType2Follow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow", value))
    }
    /// Also follow unit-to-unit calls.
    public static func motorolaType2FollowPrivate(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "follow_private", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMotorolaType2FollowPrivate(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "follow_private", value))
    }
    /// Follow groups.
    public static func motorolaType2FollowGroups(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "follow_groups", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMotorolaType2FollowGroups(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "follow_groups", $0) })
    }
    /// Longest follow.
    public static func motorolaType2FollowMaxS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "follow_max_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMotorolaType2FollowMaxS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "follow_max_s", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum TrunkGeneratorSettings {
    /// Test signal last batch.
    public static func ermesLastBatch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_last_batch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setErmesLastBatch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_last_batch", value))
    }
    /// Test signal deviation.
    public static func motorolaType2DeviationHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMotorolaType2DeviationHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value))
    }
    /// Test signal inverted.
    public static func motorolaType2Invert(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_invert", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMotorolaType2Invert(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_invert", value))
    }
    /// Test signal bad words.
    public static func motorolaType2WordErrors(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_word_errors", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMotorolaType2WordErrors(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_word_errors", value))
    }
    /// Test signal idle inside a message.
    public static func motorolaType2IdleInside(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_idle_inside", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMotorolaType2IdleInside(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_idle_inside", value))
    }
    /// The options of the setting "gen_shape" of the stack "opensky".
    public enum OpenskyShape: Int32 {
        case opensky = 0
        case p25 = 1
        case edacs = 2
        case n3600Baud2Levels1200HzContinuous = 3
    }
    /// Test signal shape.
    public static func openskyShape(_ handle: SDRKIT_HANDLE) -> Result<OpenskyShape, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_shape", &v)
        return code == SDRKIT_OK ? .success(OpenskyShape(rawValue: v) ?? OpenskyShape(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setOpenskyShape(_ handle: SDRKIT_HANDLE, _ value: OpenskyShape) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_shape", value.rawValue))
    }
    /// The options of the setting "gen_pages" of the stack "gsc".
    public enum GscPages: Int32 {
        case all = 0
        case tone = 1
        case numeric = 2
        case alpha = 3
        case voice = 4
    }
    /// Test signal pages.
    public static func gscPages(_ handle: SDRKIT_HANDLE) -> Result<GscPages, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_pages", &v)
        return code == SDRKIT_OK ? .success(GscPages(rawValue: v) ?? GscPages(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setGscPages(_ handle: SDRKIT_HANDLE, _ value: GscPages) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_pages", value.rawValue))
    }
    /// Test signal batch.
    public static func gscBatch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_batch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGscBatch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_batch", value))
    }
    /// Test signal without preamble.
    public static func gscNoPreamble(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_no_preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGscNoPreamble(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_no_preamble", value))
    }
    /// Test signal deviation.
    public static func gscDeviationHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setGscDeviationHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value))
    }
    /// The options of the setting "gen_link" of the stack "scada".
    public enum ScadaLink: Int32 {
        case bell202 = 0
        case v23 = 1
        case fsk24800 = 2
        case fsk29600 = 3
        case fsk219200 = 4
        case fsk44800 = 5
        case fsk49600 = 6
        case fsk419200 = 7
    }
    /// Test signal link.
    public static func scadaLink(_ handle: SDRKIT_HANDLE) -> Result<ScadaLink, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_link", &v)
        return code == SDRKIT_OK ? .success(ScadaLink(rawValue: v) ?? ScadaLink(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setScadaLink(_ handle: SDRKIT_HANDLE, _ value: ScadaLink) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_link", value.rawValue))
    }
    /// The options of the setting "gen_framing" of the stack "scada".
    public enum ScadaFraming: Int32 {
        case n8e1 = 0
        case n8o1 = 1
        case n8n1 = 2
        case n8n2 = 3
        case hdlcNrz = 4
        case hdlcNrzi = 5
    }
    /// Test signal framing.
    public static func scadaFraming(_ handle: SDRKIT_HANDLE) -> Result<ScadaFraming, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_framing", &v)
        return code == SDRKIT_OK ? .success(ScadaFraming(rawValue: v) ?? ScadaFraming(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setScadaFraming(_ handle: SDRKIT_HANDLE, _ value: ScadaFraming) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_framing", value.rawValue))
    }
    /// The options of the setting "gen_traffic" of the stack "scada".
    public enum ScadaTraffic: Int32 {
        case modbus = 0
        case modbusAscii = 1
        case dnp3 = 2
    }
    /// Test signal traffic.
    public static func scadaTraffic(_ handle: SDRKIT_HANDLE) -> Result<ScadaTraffic, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_traffic", &v)
        return code == SDRKIT_OK ? .success(ScadaTraffic(rawValue: v) ?? ScadaTraffic(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setScadaTraffic(_ handle: SDRKIT_HANDLE, _ value: ScadaTraffic) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_traffic", value.rawValue))
    }
}
