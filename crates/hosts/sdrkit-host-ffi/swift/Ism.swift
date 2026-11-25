// sdrkit: the typed interface of the stack family "ism", version 4.2. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum IsmInterface {
    public static let major = 4
    public static let minor = 2
}

/// The result fields of the burst finder of the ISM sensor stack.
public struct IsmDetectFields {
    /// Bursts found in the band.
    public var burstsDetected: Int64
    /// The mean noise floor of a frequency bin, once the finder has frames.
    public var noiseFloorDbfs: Double?
    /// The mean input level since the last report, if samples came.
    public var inputLevelDbfs: Double?

    init(_ c: sdrkit_ism_detect_fields) {
        burstsDetected = c.bursts_detected
        noiseFloorDbfs = c.has_noise_floor_dbfs ? c.noise_floor_dbfs : nil
        inputLevelDbfs = c.has_input_level_dbfs ? c.input_level_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IsmDetectFields, SDRKitCode> {
        var c = sdrkit_ism_detect_fields()
        let code = sdrkit_ism_detect_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IsmDetectFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The count of the Sigfox carrier finder.
public struct SigfoxFinderFields {
    /// Narrow carriers passed to the demodulator.
    public var carriers: Int64

    init(_ c: sdrkit_sigfox_finder_fields) {
        carriers = c.carriers
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SigfoxFinderFields, SDRKitCode> {
        var c = sdrkit_sigfox_finder_fields()
        let code = sdrkit_sigfox_finder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SigfoxFinderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.
public struct IsmGuardFields {
    /// Input samples replaced because they were not finite or far beyond full scale.
    public var badSamples: Int64

    init(_ c: sdrkit_ism_guard_fields) {
        badSamples = c.bad_samples
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IsmGuardFields, SDRKitCode> {
        var c = sdrkit_ism_guard_fields()
        let code = sdrkit_ism_guard_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IsmGuardFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the device table of the ISM sensor stack.
public struct IsmFields {
    /// Devices in the table.
    public var devices: Int64
    /// Distinct transmissions decoded.
    public var packetsDecoded: Int64
    /// Trains no protocol decoded.
    public var unknownBursts: Int64
    /// Sigfox messages in the list.
    public var sigfoxMessages: Int64

    init(_ c: sdrkit_ism_fields) {
        devices = c.devices
        packetsDecoded = c.packets_decoded
        unknownBursts = c.unknown_bursts
        sigfoxMessages = c.sigfox_messages
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IsmFields, SDRKitCode> {
        var c = sdrkit_ism_fields()
        let code = sdrkit_ism_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IsmFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the pulse analyser (submode `pulse_analyser`): the facts of the last frame with a line code, and the counts. A fact has no value before the first such frame and after a change of frequency, and a timing has a value only for the line codes that have it.
public struct IsmPulseFields {
    public enum LineCode: Int32 {
        case pwm = 0
        case ppm = 1
        case manchester = 2
        case pcm = 3
    }
    /// The line code guessed from the pulse timings.
    public var lineCode: LineCode?
    /// Bits read.
    public var bits: Int64?
    /// The first 96 bits as 0 and 1, with "..." when there are more.
    public var bitsBin: String?
    /// The first 96 bits as hex; the last digit is padded with zero bits.
    public var bitsHex: String?
    /// PWM: the short width. PPM: the short gap.
    public var shortUs: Double?
    /// PWM: the long width. PPM: the long gap.
    public var longUs: Double?
    /// PPM: the pulse width.
    public var pulseUs: Double?
    /// PWM: the bit period.
    public var periodUs: Double?
    /// Manchester: the half-bit.
    public var halfBitUs: Double?
    /// PCM: the unit.
    public var unitUs: Double?
    /// Manchester and PCM: the bit rate.
    public var bitRate: Double?
    /// The pulses of the frame.
    public var pulses: Int64?
    /// From the start of the first pulse to the end of the last.
    public var frameMs: Double?
    /// Frames in a row with the same bits, each within 0.5 s of the one before.
    public var repeats: Int64?
    /// Frames analysed, with a line code or not.
    public var framesSeen: Int64
    /// Frames with a line code.
    public var framesGood: Int64

    init(_ c: sdrkit_ism_pulse_fields) {
        lineCode = c.has_line_code ? LineCode(rawValue: c.line_code) : nil
        bits = c.has_bits ? c.bits : nil
        bitsBin = c.has_bits_bin ? sdrkitText(c.bits_bin) : nil
        bitsHex = c.has_bits_hex ? sdrkitText(c.bits_hex) : nil
        shortUs = c.has_short_us ? c.short_us : nil
        longUs = c.has_long_us ? c.long_us : nil
        pulseUs = c.has_pulse_us ? c.pulse_us : nil
        periodUs = c.has_period_us ? c.period_us : nil
        halfBitUs = c.has_half_bit_us ? c.half_bit_us : nil
        unitUs = c.has_unit_us ? c.unit_us : nil
        bitRate = c.has_bit_rate ? c.bit_rate : nil
        pulses = c.has_pulses ? c.pulses : nil
        frameMs = c.has_frame_ms ? c.frame_ms : nil
        repeats = c.has_repeats ? c.repeats : nil
        framesSeen = c.frames_seen
        framesGood = c.frames_good
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IsmPulseFields, SDRKitCode> {
        var c = sdrkit_ism_pulse_fields()
        let code = sdrkit_ism_pulse_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IsmPulseFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the sender of a frame stack.
public struct IsmTxFields {
    /// Whether a frame is on air.
    public var txSending: Bool
    /// The frames sent since the stack started.
    public var txSent: Int64
    /// The frames waiting for the transmitter.
    public var txWaiting: Int64

    init(_ c: sdrkit_ism_tx_fields) {
        txSending = c.tx_sending
        txSent = c.tx_sent
        txWaiting = c.tx_waiting
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IsmTxFields, SDRKitCode> {
        var c = sdrkit_ism_tx_fields()
        let code = sdrkit_ism_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IsmTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the devices. The row key is the model, the id and the channel.
public struct IsmDeviceRow {
    public enum Check: Int32 {
        case crc = 0
        case checksum = 1
        case parity = 2
        case `repeat` = 3
        case manchester = 4
    }
    /// The row key.
    public var key: String
    /// The device model.
    public var model: String
    /// The device kind.
    public var kind: String
    /// The device id.
    public var id: String
    /// The channel, if the device sends one.
    public var channel: String?
    /// The battery state: yes for good, no for low.
    public var batteryOk: Bool?
    /// The fields of the last reading as one line.
    public var readings: String
    /// The history of the first number with a unit, once the device sent one.
    public var history: String?
    /// The level of the last transmission.
    public var levelDbfs: Double
    /// The signal to noise ratio of the last transmission.
    public var snrDb: Double
    /// The UTC of the last transmission.
    public var lastHeard: Double?
    /// Distinct transmissions.
    public var count: Int64
    /// The frequency of the last transmission.
    public var frequency: Double
    /// The frame test of the last reading.
    public var check: Check?
    /// The name of the device for people: the model and the id ("Acurite-Tower 1234").
    public var name: String

    init(_ c: sdrkit_ism_device_row) {
        key = sdrkitText(c.key)
        model = sdrkitText(c.model)
        kind = sdrkitText(c.kind)
        id = sdrkitText(c.id)
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        batteryOk = c.has_battery_ok ? c.battery_ok : nil
        readings = sdrkitText(c.readings)
        history = c.has_history ? sdrkitText(c.history) : nil
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        lastHeard = c.has_last_heard ? c.last_heard : nil
        count = c.count
        frequency = c.frequency
        check = Check(rawValue: c.check)
        name = sdrkitText(c.name)
    }
}

/// A row of the Z-Wave view of the devices. The row key is the key of the device.
public struct IsmZwaveRow {
    /// The row key.
    public var key: String
    /// The home id, if the frame gave one.
    public var homeId: String?
    /// The node, if the frame gave one.
    public var node: Int64?
    /// The rate.
    public var rate: String
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The level of the last frame.
    public var levelDbfs: Double
    /// Distinct frames.
    public var count: Int64
    /// The command, if the frame gave one.
    public var command: String?
    /// The value of the command, if the frame gave one.
    public var value: String?

    init(_ c: sdrkit_ism_zwave_row) {
        key = sdrkitText(c.key)
        homeId = c.has_home_id ? sdrkitText(c.home_id) : nil
        node = c.has_node ? c.node : nil
        rate = sdrkitText(c.rate)
        lastHeard = c.has_last_heard ? c.last_heard : nil
        levelDbfs = c.level_dbfs
        count = c.count
        command = c.has_command ? sdrkitText(c.command) : nil
        value = c.has_value ? sdrkitText(c.value) : nil
    }
}

/// A row of the reading log.
public struct IsmLogRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The device model.
    public var model: String
    /// The device id.
    public var id: String
    /// The frequency.
    public var frequency: Double
    /// The level.
    public var levelDbfs: Double
    /// The fields of the reading as one line.
    public var text: String

    init(_ c: sdrkit_ism_log_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        model = sdrkitText(c.model)
        id = sdrkitText(c.id)
        frequency = c.frequency
        levelDbfs = c.level_dbfs
        text = sdrkitText(c.text)
    }
}

/// A row of the unknown signals: a train no protocol decoded, with its line-code guess.
public struct IsmUnknownRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The frequency.
    public var frequency: Double
    /// OOK or FSK.
    public var modulation: String
    /// The guessed line code.
    public var lineCode: String
    /// Pulses of the train.
    public var pulses: Int64
    /// The short width of the guess.
    public var shortUs: Double
    /// The long width of the guess.
    public var longUs: Double
    /// The level.
    public var levelDbfs: Double
    /// The signal to noise ratio.
    public var snrDb: Double
    /// The pulses as text to copy ("OOK PWM 464 us/1404 us; 464/1404 ...").
    public var copy: String

    init(_ c: sdrkit_ism_unknown_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        frequency = c.frequency
        modulation = sdrkitText(c.modulation)
        lineCode = sdrkitText(c.line_code)
        pulses = c.pulses
        shortUs = c.short_us
        longUs = c.long_us
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        copy = sdrkitText(c.copy)
    }
}

/// A row of the Sigfox messages heard inside the ISM sensor stack. The row key is the key of the message.
public struct IsmSigfoxRow {
    /// The row key.
    public var key: String
    /// The signal time of the first frame.
    public var timeS: Double
    /// The device id.
    public var deviceId: String
    /// The message counter.
    public var seq: Int64
    /// The payload as hex, or the one-bit value.
    public var payload: String
    /// The frequency of the first repetition.
    public var frequency: Double
    /// The level of the strongest repetition.
    public var levelDbfs: Double
    /// Repetitions heard.
    public var reps: Int64
    /// The authentication code, never checked, and the CRC.
    public var macCrc: String

    init(_ c: sdrkit_ism_sigfox_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        deviceId = sdrkitText(c.device_id)
        seq = c.seq
        payload = sdrkitText(c.payload)
        frequency = c.frequency
        levelDbfs = c.level_dbfs
        reps = c.reps
        macCrc = sdrkitText(c.mac_crc)
    }
}

/// A row of the analysed frames with a line code. The row key is the number of the frame in the log.
public struct IsmPulseFrameRow {
    public enum LineCode: Int32 {
        case pwm = 0
        case ppm = 1
        case manchester = 2
        case pcm = 3
    }
    /// The row key.
    public var key: String
    /// The signal time of the first pulse.
    public var timeS: Double
    /// The frequency.
    public var frequency: Double
    /// The line code guessed from the pulse timings.
    public var lineCode: LineCode?
    /// Bits read.
    public var bits: Int64
    /// The first 96 bits as 0 and 1, with "..." when there are more.
    public var bitsBin: String
    /// The first 96 bits as hex.
    public var bitsHex: String
    /// PWM: the short width. PPM: the short gap.
    public var shortUs: Double?
    /// PWM: the long width. PPM: the long gap.
    public var longUs: Double?
    /// PPM: the pulse width.
    public var pulseUs: Double?
    /// PWM: the bit period.
    public var periodUs: Double?
    /// Manchester: the half-bit.
    public var halfBitUs: Double?
    /// PCM: the unit.
    public var unitUs: Double?
    /// Manchester and PCM: the bit rate.
    public var bitRate: Double?
    /// The pulses of the frame.
    public var pulses: Int64
    /// From the start of the first pulse to the end of the last.
    public var frameMs: Double
    /// Frames in a row with the same bits, this one included.
    public var repeats: Int64
    /// The level of the marks.
    public var levelDbfs: Double

    init(_ c: sdrkit_ism_pulse_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        frequency = c.frequency
        lineCode = LineCode(rawValue: c.line_code)
        bits = c.bits
        bitsBin = sdrkitText(c.bits_bin)
        bitsHex = sdrkitText(c.bits_hex)
        shortUs = c.has_short_us ? c.short_us : nil
        longUs = c.has_long_us ? c.long_us : nil
        pulseUs = c.has_pulse_us ? c.pulse_us : nil
        periodUs = c.has_period_us ? c.period_us : nil
        halfBitUs = c.has_half_bit_us ? c.half_bit_us : nil
        unitUs = c.has_unit_us ? c.unit_us : nil
        bitRate = c.has_bit_rate ? c.bit_rate : nil
        pulses = c.pulses
        frameMs = c.frame_ms
        repeats = c.repeats
        levelDbfs = c.level_dbfs
    }
}

/// The result fields of the Wireless M-Bus stack.
public struct WmbusFields {
    /// Frames whose CRCs passed.
    public var framesOk: Int64
    /// Frames with a failed CRC.
    public var framesBad: Int64
    /// The meters in the table.
    public var meters: Int64
    /// Syncs found.
    public var syncs: Int64
    /// Syncs that did not give a frame.
    public var falseSyncs: Int64
    /// Frames with an encrypted payload.
    public var encryptedFrames: Int64

    init(_ c: sdrkit_wmbus_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        meters = c.meters
        syncs = c.syncs
        falseSyncs = c.false_syncs
        encryptedFrames = c.encrypted_frames
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WmbusFields, SDRKitCode> {
        var c = sdrkit_wmbus_fields()
        let code = sdrkit_wmbus_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WmbusFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the meters. The row key is the maker, the id and the type.
public struct WmbusMeterRow {
    /// The row key.
    public var key: String
    /// The manufacturer letters.
    public var maker: String
    /// The meter id (8 digits).
    public var meterId: String
    /// The device type name.
    public var deviceType: String
    /// The physical mode of the last frame ("S1", "T1", "C1 (A)" ...).
    public var phy: String
    /// The channel of the last frame.
    public var frequency: Double
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The payload is encrypted.
    public var encrypted: Bool
    /// The summary of the last frame.
    public var lastReading: String
    /// The application status.
    public var status: String

    init(_ c: sdrkit_wmbus_meter_row) {
        key = sdrkitText(c.key)
        maker = sdrkitText(c.maker)
        meterId = sdrkitText(c.meter_id)
        deviceType = sdrkitText(c.device_type)
        phy = sdrkitText(c.phy)
        frequency = c.frequency
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        levelDbfs = c.level_dbfs
        encrypted = c.encrypted
        lastReading = sdrkitText(c.last_reading)
        status = sdrkitText(c.status)
    }
}

/// A row of the data records of the last frame of each meter. The row key is the row key of the meter, "/" and the index.
public struct WmbusRecordRow {
    /// The row key.
    public var key: String
    /// The row key of the meter.
    public var meter: String
    /// The place of the record in its frame, from 0.
    public var index: Int64
    /// The quantity ("Volume").
    public var quantity: String
    /// The value with its unit ("123.456 m3").
    public var value: String
    /// The notes ("storage 1"), if any.
    public var notes: String?

    init(_ c: sdrkit_wmbus_record_row) {
        key = sdrkitText(c.key)
        meter = sdrkitText(c.meter)
        index = c.index
        quantity = sdrkitText(c.quantity)
        value = sdrkitText(c.value)
        notes = c.has_notes ? sdrkitText(c.notes) : nil
    }
}

/// A row of the frame log. The row key is the number of the frame in the log.
public struct WmbusFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// The UTC of the frame, when the samples are live.
    public var time: Double?
    /// The physical mode.
    public var phy: String
    /// The channel.
    public var frequency: Double
    /// The meter ("KAM 12345678"), if the frame names one.
    public var meter: String?
    /// The level of the frame.
    public var levelDbfs: Double
    /// Every CRC of the frame passed.
    public var crcOk: Bool
    /// The payload is encrypted.
    public var encrypted: Bool
    /// The summary of the frame.
    public var result: String
    /// The frame bytes in hex.
    public var frame: String

    init(_ c: sdrkit_wmbus_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        time = c.has_time ? c.time : nil
        phy = sdrkitText(c.phy)
        frequency = c.frequency
        meter = c.has_meter ? sdrkitText(c.meter) : nil
        levelDbfs = c.level_dbfs
        crcOk = c.crc_ok
        encrypted = c.encrypted
        result = sdrkitText(c.result)
        frame = sdrkitText(c.frame)
    }
}

/// A row of the channels. The row key is the frequency in MHz and the modes.
public struct WmbusChannelRow {
    /// The row key.
    public var key: String
    /// The channel frequency.
    public var frequency: Double
    /// The physical modes on the channel.
    public var phy: String
    /// The slow power of the channel.
    public var levelDbfs: Double
    /// The noise floor of the channel.
    public var noiseDbfs: Double
    /// Good frames on the channel.
    public var frames: Int64
    /// Frames with a failed CRC on the channel.
    public var crcFailures: Int64

    init(_ c: sdrkit_wmbus_channel_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        phy = sdrkitText(c.phy)
        levelDbfs = c.level_dbfs
        noiseDbfs = c.noise_dbfs
        frames = c.frames
        crcFailures = c.crc_failures
    }
}

/// The result fields of the Z-Wave stack.
public struct ZwaveFields {
    /// Frames decoded at every rate.
    public var framesOk: Int64
    /// Bursts whose check failed.
    public var framesBad: Int64
    /// The networks in the table.
    public var networks: Int64
    /// The nodes in the table.
    public var nodes: Int64
    /// Frames at R1.
    public var framesR1: Int64
    /// Frames at R2.
    public var framesR2: Int64
    /// Frames at R3.
    public var framesR3: Int64
    /// Security S0 frames.
    public var s0Frames: Int64
    /// Security S2 frames.
    public var s2Frames: Int64
    /// Acknowledgements.
    public var acks: Int64

    init(_ c: sdrkit_zwave_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        networks = c.networks
        nodes = c.nodes
        framesR1 = c.frames_r1
        framesR2 = c.frames_r2
        framesR3 = c.frames_r3
        s0Frames = c.s0_frames
        s2Frames = c.s2_frames
        acks = c.acks
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ZwaveFields, SDRKitCode> {
        var c = sdrkit_zwave_fields()
        let code = sdrkit_zwave_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ZwaveFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the Z-Wave frame log. The row key is the number of the frame in the log.
public struct ZwaveFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// The UTC of the frame, when the samples are live.
    public var time: Double?
    /// The channel frequency.
    public var frequency: Double
    /// The rate ("R1", "R2", "R3").
    public var rate: String
    /// The home id (8 hex digits).
    public var homeId: String
    /// Source and destination ("2 > 1").
    public var route: String
    /// The header type.
    public var frameType: String
    /// The sequence number.
    public var seq: Int64
    /// An acknowledgement is requested.
    public var ackRequested: Bool
    /// The encryption ("none", "S0", "S2").
    public var encryption: String
    /// The level of the frame.
    public var levelDbfs: Double
    /// The SNR of the frame.
    public var snrDb: Double
    /// The one-line description.
    public var command: String
    /// The MPDU in hex.
    public var mpdu: String

    init(_ c: sdrkit_zwave_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        time = c.has_time ? c.time : nil
        frequency = c.frequency
        rate = sdrkitText(c.rate)
        homeId = sdrkitText(c.home_id)
        route = sdrkitText(c.route)
        frameType = sdrkitText(c.frame_type)
        seq = c.seq
        ackRequested = c.ack_requested
        encryption = sdrkitText(c.encryption)
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        command = sdrkitText(c.command)
        mpdu = sdrkitText(c.mpdu)
    }
}

/// A row of the Z-Wave nodes. The row key is the home id and the node id.
public struct ZwaveNodeRow {
    /// The row key.
    public var key: String
    /// The row key of the network of the node.
    public var homeId: String
    /// The node id.
    public var node: Int64
    /// "controller" or "node".
    public var role: String
    /// Frames the node sent.
    public var sent: Int64
    /// Frames addressed to the node.
    public var addressed: Int64
    /// Encrypted frames the node sent.
    public var encrypted: Int64
    /// The rate of the last frame.
    public var rate: String
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The last command class the node sent.
    public var lastClass: String
    /// The node uses beaming.
    public var beaming: Bool
    /// The node sent at low power.
    public var lowPower: Bool
    /// The name of the node for people: the home id and the node id ("C0FFEE01 node 2").
    public var name: String

    init(_ c: sdrkit_zwave_node_row) {
        key = sdrkitText(c.key)
        homeId = sdrkitText(c.home_id)
        node = c.node
        role = sdrkitText(c.role)
        sent = c.sent
        addressed = c.addressed
        encrypted = c.encrypted
        rate = sdrkitText(c.rate)
        levelDbfs = c.level_dbfs
        lastHeard = c.has_last_heard ? c.last_heard : nil
        lastClass = sdrkitText(c.last_class)
        beaming = c.beaming
        lowPower = c.low_power
        name = sdrkitText(c.name)
    }
}

/// A row of the Z-Wave networks. The row key is the home id.
public struct ZwaveNetworkRow {
    /// The row key.
    public var key: String
    /// The home id.
    public var homeId: String
    /// The nodes heard.
    public var nodes: Int64
    /// Node 1 of the network has sent.
    public var controller: Bool
    /// The times heard.
    public var heard: Int64?
    /// Encrypted frames.
    public var encrypted: Int64
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The UTC of the first frame.
    public var firstHeard: Double?
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_zwave_network_row) {
        key = sdrkitText(c.key)
        homeId = sdrkitText(c.home_id)
        nodes = c.nodes
        controller = c.controller
        heard = c.has_heard ? c.heard : nil
        encrypted = c.encrypted
        levelDbfs = c.level_dbfs
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the Z-Wave channels in use. The row key is the frequency and the rate.
public struct ZwaveChannelRow {
    /// The row key.
    public var key: String
    /// The channel frequency.
    public var frequency: Double
    /// The rate.
    public var rate: String
    /// The channel is in the band taken in.
    public var covered: Bool
    /// Frames decoded on the channel.
    public var frames: Int64
    /// Bursts whose check failed on the channel.
    public var failedChecks: Int64

    init(_ c: sdrkit_zwave_channel_row) {
        key = sdrkitText(c.key)
        frequency = c.frequency
        rate = sdrkitText(c.rate)
        covered = c.covered
        frames = c.frames
        failedChecks = c.failed_checks
    }
}

/// A row of the command class counts. The row key is the name of the class.
public struct ZwaveClassRow {
    /// The row key.
    public var key: String
    /// The command class.
    public var `class`: String
    /// How often it was sent.
    public var count: Int64

    init(_ c: sdrkit_zwave_class_row) {
        key = sdrkitText(c.key)
        `class` = sdrkitText(c.class_)
        count = c.count
    }
}

/// The result fields of the Zigbee stack.
public struct ZigbeeFields {
    /// Good frames.
    public var framesOk: Int64
    /// Frames whose SFD was found and whose FCS failed.
    public var framesBad: Int64
    /// Preamble candidates.
    public var candidates: Int64
    /// Frames with an encrypted payload.
    public var encryptedFrames: Int64
    /// The networks in the table.
    public var networks: Int64
    /// The devices in the table.
    public var devices: Int64
    /// The channel listened to (0 when not known).
    public var channel: Int64
    /// Good frames in the last second.
    public var framesPerS: Int64
    /// The SNR of the last frame since the last change of frequency.
    public var snrDb: Double?
    /// The carrier offset of the last frame since the last change of frequency.
    public var offsetHz: Double?

    init(_ c: sdrkit_zigbee_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        candidates = c.candidates
        encryptedFrames = c.encrypted_frames
        networks = c.networks
        devices = c.devices
        channel = c.channel
        framesPerS = c.frames_per_s
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ZigbeeFields, SDRKitCode> {
        var c = sdrkit_zigbee_fields()
        let code = sdrkit_zigbee_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ZigbeeFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the Zigbee networks. The row key is the PAN and the channel as one number.
public struct ZigbeeNetworkRow {
    /// The row key.
    public var key: String
    /// The PAN id.
    public var pan: String
    /// The extended PAN id, if a beacon gave it.
    public var extPan: String?
    /// The channel.
    public var ch: Int64
    /// The coordinator, if known.
    public var coordinator: String?
    /// The devices in the PAN.
    public var devices: Int64
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The name of the network for people: the extended PAN id when known, else the PAN id.
    public var name: String

    init(_ c: sdrkit_zigbee_network_row) {
        key = sdrkitText(c.key)
        pan = sdrkitText(c.pan)
        extPan = c.has_ext_pan ? sdrkitText(c.ext_pan) : nil
        ch = c.ch
        coordinator = c.has_coordinator ? sdrkitText(c.coordinator) : nil
        devices = c.devices
        levelDbfs = c.level_dbfs
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        name = sdrkitText(c.name)
    }
}

/// A row of the devices of an IEEE 802.15.4 stack. The row key is the number of the device.
public struct IsmPanDeviceRow {
    /// The row key.
    public var key: String
    /// The short address, if heard.
    public var short: String?
    /// The extended address, if heard.
    public var extended: String?
    /// The vendor of the extended address.
    public var vendor: String?
    /// The PAN id, if known.
    public var pan: String?
    /// The channel.
    public var ch: Int64
    /// "coordinator", "router" or "device".
    public var role: String
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The description of the last frame.
    public var lastFrame: String
    /// The name of the device for people: the vendor and the short address, else the extended address.
    public var name: String

    init(_ c: sdrkit_ism_pan_device_row) {
        key = sdrkitText(c.key)
        short = c.has_short_ ? sdrkitText(c.short_) : nil
        extended = c.has_extended ? sdrkitText(c.extended) : nil
        vendor = c.has_vendor ? sdrkitText(c.vendor) : nil
        pan = c.has_pan ? sdrkitText(c.pan) : nil
        ch = c.ch
        role = sdrkitText(c.role)
        levelDbfs = c.level_dbfs
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        lastFrame = sdrkitText(c.last_frame)
        name = sdrkitText(c.name)
    }
}

/// A row of the Zigbee frame log. The row key is the number of the frame in the log.
public struct ZigbeeFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// The channel.
    public var ch: Int64
    /// The MAC frame type.
    public var type: String
    /// "source > destination".
    public var route: String
    /// The PAN id, if known.
    public var pan: String?
    /// The PSDU length with the FCS.
    public var len: Int64
    /// Some payload is encrypted.
    public var encrypted: Bool
    /// The level of the frame.
    public var levelDbfs: Double
    /// The description.
    public var details: String

    init(_ c: sdrkit_zigbee_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        ch = c.ch
        type = sdrkitText(c.type)
        route = sdrkitText(c.route)
        pan = c.has_pan ? sdrkitText(c.pan) : nil
        len = c.len
        encrypted = c.encrypted
        levelDbfs = c.level_dbfs
        details = sdrkitText(c.details)
    }
}

/// The result fields of the message list of the Sigfox stack.
public struct SigfoxFields {
    /// Frames decoded, every repetition.
    public var framesOk: Int64
    /// The messages in the list.
    public var messages: Int64
    /// The zone of the dial, if the dial is in one.
    public var zone: String?

    init(_ c: sdrkit_sigfox_fields) {
        framesOk = c.frames_ok
        messages = c.messages
        zone = c.has_zone ? sdrkitText(c.zone) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SigfoxFields, SDRKitCode> {
        var c = sdrkit_sigfox_fields()
        let code = sdrkit_sigfox_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SigfoxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The count of the Sigfox demodulator.
public struct SigfoxDemodFields {
    /// Carriers that did not decode.
    public var carriersBad: Int64

    init(_ c: sdrkit_sigfox_demod_fields) {
        carriersBad = c.carriers_bad
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SigfoxDemodFields, SDRKitCode> {
        var c = sdrkit_sigfox_demod_fields()
        let code = sdrkit_sigfox_demod_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SigfoxDemodFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the messages. The row key is the device id, the counter and the signal time of the first frame.
public struct SigfoxMessageRow {
    /// The row key.
    public var key: String
    /// The signal time of the first frame.
    public var timeS: Double
    /// The device id.
    public var deviceId: String
    /// The message counter.
    public var seq: Int64
    /// The payload as hex, or the one-bit value.
    public var payload: String
    /// The payload length.
    public var payloadBytes: Int64
    /// The frequency of the first repetition.
    public var frequency: Double
    /// The level of the strongest repetition.
    public var levelDbfs: Double
    /// Repetitions heard.
    public var reps: Int64
    /// The ranks heard as a mask (bit 0 is rank 1).
    public var ranks: Int64
    /// The bit rate.
    public var bitRate: Double
    /// The authentication code, as received and never checked.
    public var auth: String
    /// The CRC, checked.
    public var crc: String
    /// The frame type of the first repetition.
    public var frameType: String
    /// The length indicator.
    public var li: Int64
    /// The container size.
    public var containerBytes: Int64
    /// The device requests a downlink.
    public var downlink: Bool
    /// A control message.
    public var control: Bool

    init(_ c: sdrkit_sigfox_message_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        deviceId = sdrkitText(c.device_id)
        seq = c.seq
        payload = sdrkitText(c.payload)
        payloadBytes = c.payload_bytes
        frequency = c.frequency
        levelDbfs = c.level_dbfs
        reps = c.reps
        ranks = c.ranks
        bitRate = c.bit_rate
        auth = sdrkitText(c.auth)
        crc = sdrkitText(c.crc)
        frameType = sdrkitText(c.frame_type)
        li = c.li
        containerBytes = c.container_bytes
        downlink = c.downlink
        control = c.control
    }
}

/// The result fields of the sensor table of the ANT+ stack.
public struct AntFields {
    /// Frames with a good CRC.
    public var framesOk: Int64
    /// The sensors in the table.
    public var sensors: Int64
    /// The newest heart rate of any strap, if a strap sent one.
    public var heartRateBpm: Double?
    /// The deviation of the last frame. It has no value before the first frame.
    public var deviation: Double?
    /// The channel the receiver is tuned to.
    public var channel: Double

    init(_ c: sdrkit_ant_fields) {
        framesOk = c.frames_ok
        sensors = c.sensors
        heartRateBpm = c.has_heart_rate_bpm ? c.heart_rate_bpm : nil
        deviation = c.has_deviation ? c.deviation : nil
        channel = c.channel
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AntFields, SDRKitCode> {
        var c = sdrkit_ant_fields()
        let code = sdrkit_ant_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AntFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The counts of the ANT+ demodulator.
public struct AntDemodFields {
    /// Frames whose CRC failed.
    public var framesBad: Int64
    /// Sync words found.
    public var syncs: Int64

    init(_ c: sdrkit_ant_demod_fields) {
        framesBad = c.frames_bad
        syncs = c.syncs
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AntDemodFields, SDRKitCode> {
        var c = sdrkit_ant_demod_fields()
        let code = sdrkit_ant_demod_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AntDemodFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the sensors. The row key is the device number, type and transmission type as one number.
public struct AntSensorRow {
    /// The row key.
    public var key: String
    /// The device number.
    public var device: Int64
    /// The device type name.
    public var type: String
    /// The network marker as hex.
    public var marker: String
    /// The manufacturer, once a page names it.
    public var manufacturer: String?
    /// The serial number, once a page gives it.
    public var serial: Int64?
    /// The main value of the sensor in words.
    public var value: String
    /// The battery status, once a page gives it.
    public var battery: String?
    /// The measured message period.
    public var period: Double?
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The carrier frequency.
    public var frequency: Double
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The name of the sensor for people: its type and device number.
    public var name: String

    init(_ c: sdrkit_ant_sensor_row) {
        key = sdrkitText(c.key)
        device = c.device
        type = sdrkitText(c.type)
        marker = sdrkitText(c.marker)
        manufacturer = c.has_manufacturer ? sdrkitText(c.manufacturer) : nil
        serial = c.has_serial ? c.serial : nil
        value = sdrkitText(c.value)
        battery = c.has_battery ? sdrkitText(c.battery) : nil
        period = c.has_period ? c.period : nil
        levelDbfs = c.level_dbfs
        frequency = c.frequency
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        name = sdrkitText(c.name)
    }
}

/// A row of the frame log. The row key is the number of the frame in the log.
public struct AntFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// The device number.
    public var device: Int64
    /// The device type name.
    public var type: String
    /// The transmission type.
    public var txType: Int64
    /// The network marker as hex.
    public var marker: String
    /// The level of the frame.
    public var levelDbfs: Double
    /// The payload as hex.
    public var payload: String
    /// The page and its decoded text.
    public var text: String

    init(_ c: sdrkit_ant_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        device = c.device
        type = sdrkitText(c.type)
        txType = c.tx_type
        marker = sdrkitText(c.marker)
        levelDbfs = c.level_dbfs
        payload = sdrkitText(c.payload)
        text = sdrkitText(c.text)
    }
}

/// The result fields of the tables of the nRF24L01 stack.
public struct Nrf24NrfFields {
    /// Frames decoded.
    public var frames: Int64
    /// Bursts without a frame.
    public var badBursts: Int64
    /// Bursts.
    public var bursts: Int64
    /// Frames with a CRC-16.
    public var crc16Frames: Int64
    /// Frames with a CRC-8.
    public var crc8Frames: Int64
    /// The addresses in the table.
    public var addresses: Int64
    /// The channel number N (2400 + N MHz).
    public var channel: Int64

    init(_ c: sdrkit_nrf24_nrf_fields) {
        frames = c.frames
        badBursts = c.bad_bursts
        bursts = c.bursts
        crc16Frames = c.crc16_frames
        crc8Frames = c.crc8_frames
        addresses = c.addresses
        channel = c.channel
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Nrf24NrfFields, SDRKitCode> {
        var c = sdrkit_nrf24_nrf_fields()
        let code = sdrkit_nrf24_nrf_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Nrf24NrfFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the frame log. The row key is the number of the frame in the log.
public struct Nrf24NrfFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// The bit rate.
    public var bitRate: Double
    /// The address as hex.
    public var address: String
    /// The address length in bytes.
    public var addressLen: Int64
    /// The payload length in bytes.
    public var length: Int64
    /// The PID.
    public var pid: Int64
    /// The NO_ACK bit.
    public var noAck: Bool
    /// The CRC bits.
    public var crcBits: Int64
    /// The payload as hex.
    public var payload: String
    /// The printable text of the payload.
    public var text: String
    /// The SNR.
    public var snrDb: Double
    /// The same frame again in a row within 2 s.
    public var repeats: Int64

    init(_ c: sdrkit_nrf24_nrf_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        bitRate = c.bit_rate
        address = sdrkitText(c.address)
        addressLen = c.address_len
        length = c.length
        pid = c.pid
        noAck = c.no_ack
        crcBits = c.crc_bits
        payload = sdrkitText(c.payload)
        text = sdrkitText(c.text)
        snrDb = c.snr_db
        repeats = c.repeats
    }
}

/// A row of the addresses. The row key is the address as hex.
public struct Nrf24NrfAddressRow {
    /// The row key.
    public var key: String
    /// The address as hex.
    public var address: String
    /// The address length in bytes.
    public var addressLen: Int64
    /// The bit rate.
    public var bitRate: Double
    /// The CRC bits.
    public var crcBits: Int64
    /// The last payload length in bytes.
    public var length: Int64
    /// The carrier frequency.
    public var frequency: Double
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_nrf24_nrf_address_row) {
        key = sdrkitText(c.key)
        address = sdrkitText(c.address)
        addressLen = c.address_len
        bitRate = c.bit_rate
        crcBits = c.crc_bits
        length = c.length
        frequency = c.frequency
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the bursts without a frame. The row key is the number of the burst in the log.
public struct Nrf24NrfBadBurstRow {
    /// The row key.
    public var key: String
    /// The signal time of the burst.
    public var timeS: Double
    /// The duration of the burst.
    public var durationUs: Double
    /// The SNR.
    public var snrDb: Double

    init(_ c: sdrkit_nrf24_nrf_bad_burst_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        durationUs = c.duration_us
        snrDb = c.snr_db
    }
}

/// The result fields of the Sub-GHz chat stack.
public struct SubghzchatFields {
    /// Lines heard.
    public var messages: Int64
    /// Packets with a good CRC.
    public var framesOk: Int64
    /// Packets whose CRC failed.
    public var framesBad: Int64
    /// Bursts seen.
    public var bursts: Int64
    /// Lines decrypted.
    public var decrypted: Int64
    /// Encrypted lines that the key does not open.
    public var unreadable: Int64
    /// Encrypted repeats dropped.
    public var repeats: Int64
    /// Lines the user sent.
    public var sent: Int64
    /// A key is set.
    public var keySet: Bool
    /// The SNR of the packets. It has no value before the first good packet.
    public var snrDb: Double?
    /// The level of the last good packet. It has no value before the first good packet.
    public var levelDbfs: Double?

    init(_ c: sdrkit_subghzchat_fields) {
        messages = c.messages
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        bursts = c.bursts
        decrypted = c.decrypted
        unreadable = c.unreadable
        repeats = c.repeats
        sent = c.sent
        keySet = c.key_set
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SubghzchatFields, SDRKitCode> {
        var c = sdrkit_subghzchat_fields()
        let code = sdrkit_subghzchat_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SubghzchatFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the chat. The row key is the number of the line in the log.
public struct SubghzchatLineRow {
    public enum Kind: Int32 {
        case plain = 0
        case decrypted = 1
        case encrypted = 2
        case `repeat` = 3
        case octets = 4
    }
    /// The row key.
    public var key: String
    /// The signal time of the line.
    public var timeS: Double
    /// True for a line that this station sent.
    public var own: Bool
    /// How the line was read.
    public var kind: Kind?
    /// The text.
    public var text: String
    /// The packets joined into the line.
    public var packets: Int64
    /// The level of the first packet of a line heard.
    public var levelDbfs: Double?
    /// The SNR of the first packet of a line heard.
    public var snrDb: Double?
    /// The carrier offset of the first packet of a line heard.
    public var offsetHz: Double?
    /// The run id and counter of an encrypted line.
    public var run: String?

    init(_ c: sdrkit_subghzchat_line_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        own = c.own
        kind = Kind(rawValue: c.kind)
        text = sdrkitText(c.text)
        packets = c.packets
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        run = c.has_run ? sdrkitText(c.run) : nil
    }
}

/// A row of the packet log. The row key is the number of the packet in the log.
public struct SubghzchatPacketRow {
    /// The row key.
    public var key: String
    /// The signal time of the packet.
    public var timeS: Double
    /// The payload length.
    public var length: Int64
    /// The CRC is right.
    public var crcOk: Bool
    /// The packet was found inverted.
    public var inverted: Bool
    /// The level of the packet.
    public var levelDbfs: Double
    /// The SNR of the packet.
    public var snrDb: Double
    /// The carrier offset of the packet.
    public var offsetHz: Double
    /// The payload in hex.
    public var hex: String

    init(_ c: sdrkit_subghzchat_packet_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        length = c.length
        crcOk = c.crc_ok
        inverted = c.inverted
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        offsetHz = c.offset_hz
        hex = sdrkitText(c.hex)
    }
}

/// The result fields of the base station table of the DECT stack.
public struct DectFields {
    /// The base stations in the table.
    public var bases: Int64
    /// Bursts from base stations.
    public var baseBursts: Int64
    /// Bursts from handsets.
    public var handsetBursts: Int64
    /// The carrier the receiver is tuned to.
    public var carrier: Double
    /// The band of the carrier, if the carrier is one of a band.
    public var band: String?
    /// The number of the carrier in its band, if the carrier is one of a band.
    public var carrierNumber: Int64?

    init(_ c: sdrkit_dect_fields) {
        bases = c.bases
        baseBursts = c.base_bursts
        handsetBursts = c.handset_bursts
        carrier = c.carrier
        band = c.has_band ? sdrkitText(c.band) : nil
        carrierNumber = c.has_carrier_number ? c.carrier_number : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DectFields, SDRKitCode> {
        var c = sdrkit_dect_fields()
        let code = sdrkit_dect_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DectFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The counts of the DECT demodulator.
public struct DectDemodFields {
    /// A-fields with a good R-CRC.
    public var framesOk: Int64
    /// Bursts whose S-field was found and whose A-field failed.
    public var framesBad: Int64

    init(_ c: sdrkit_dect_demod_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DectDemodFields, SDRKitCode> {
        var c = sdrkit_dect_demod_fields()
        let code = sdrkit_dect_demod_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DectDemodFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the base stations. The row key is the RFPI.
public struct DectBaseRow {
    /// The row key.
    public var key: String
    /// The RFPI as 10 hex digits.
    public var rfpi: String
    /// The access rights class.
    public var arc: String
    /// The equipment manufacturer code (class A).
    public var emc: Int64?
    /// The fixed part number (class A).
    public var fpn: Int64?
    /// The radio fixed part number (class A).
    public var rpn: Int64?
    /// The slots in use, as 24 digits.
    public var slots: String
    /// The level of the last beacon.
    public var levelDbfs: Double
    /// The carrier frequency.
    public var frequency: Double
    /// The last Qt tail as hex.
    public var lastQt: String?
    /// The decoded system information of the last Qt tail.
    public var systemInfo: String?
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last beacon.
    public var lastHeard: Double?

    init(_ c: sdrkit_dect_base_row) {
        key = sdrkitText(c.key)
        rfpi = sdrkitText(c.rfpi)
        arc = sdrkitText(c.arc)
        emc = c.has_emc ? c.emc : nil
        fpn = c.has_fpn ? c.fpn : nil
        rpn = c.has_rpn ? c.rpn : nil
        slots = sdrkitText(c.slots)
        levelDbfs = c.level_dbfs
        frequency = c.frequency
        lastQt = c.has_last_qt ? sdrkitText(c.last_qt) : nil
        systemInfo = c.has_system_info ? sdrkitText(c.system_info) : nil
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the burst log. The row key is the number of the burst in the log.
public struct DectBurstRow {
    /// The row key.
    public var key: String
    /// The signal time of the burst.
    public var timeS: Double
    /// "base" or "handset".
    public var from: String
    /// The name of the tail type.
    public var tailType: String
    /// The tail as hex.
    public var tail: String
    /// The level of the burst.
    public var levelDbfs: Double

    init(_ c: sdrkit_dect_burst_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        from = sdrkitText(c.from)
        tailType = sdrkitText(c.tail_type)
        tail = sdrkitText(c.tail)
        levelDbfs = c.level_dbfs
    }
}

/// The result fields of the tag table of the NFC stack.
public struct NfcFields {
    /// Frames heard.
    public var frames: Int64
    /// Tags identified.
    public var tags: Int64
    /// CRC or parity failures outside encrypted sessions.
    public var failures: Int64
    /// Frames from tags.
    public var tagFrames: Int64

    init(_ c: sdrkit_nfc_fields) {
        frames = c.frames
        tags = c.tags
        failures = c.failures
        tagFrames = c.tag_frames
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NfcFields, SDRKitCode> {
        var c = sdrkit_nfc_fields()
        let code = sdrkit_nfc_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NfcFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The levels of the NFC detector. They have no value before the first input.
public struct NfcDetectFields {
    /// The field level of the input.
    public var fieldDbfs: Double?
    /// The level of the stronger subcarrier detector.
    public var subcarrierDbfs: Double?

    init(_ c: sdrkit_nfc_detect_fields) {
        fieldDbfs = c.has_field_dbfs ? c.field_dbfs : nil
        subcarrierDbfs = c.has_subcarrier_dbfs ? c.subcarrier_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<NfcDetectFields, SDRKitCode> {
        var c = sdrkit_nfc_detect_fields()
        let code = sdrkit_nfc_detect_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NfcDetectFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the tags. The row key is the UID as hex.
public struct NfcTagRow {
    /// The row key.
    public var key: String
    /// The UID as hex, most significant first.
    public var uid: String
    /// The type of the tag.
    public var type: String
    /// The card-type hint.
    public var hint: String?
    /// The ATQA.
    public var atqa: String?
    /// The SAK.
    public var sak: String?
    /// The ATS historical bytes.
    public var historical: String?
    /// The PUPI application data.
    public var appData: String?
    /// The DSFID, AFI, block count and size and IC version byte of an ISO 15693 tag.
    public var system: String?
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_nfc_tag_row) {
        key = sdrkitText(c.key)
        uid = sdrkitText(c.uid)
        type = sdrkitText(c.type)
        hint = c.has_hint ? sdrkitText(c.hint) : nil
        atqa = c.has_atqa ? sdrkitText(c.atqa) : nil
        sak = c.has_sak ? sdrkitText(c.sak) : nil
        historical = c.has_historical ? sdrkitText(c.historical) : nil
        appData = c.has_app_data ? sdrkitText(c.app_data) : nil
        system = c.has_system ? sdrkitText(c.system) : nil
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the frame log. The row key is the number of the frame in the log.
public struct NfcFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// "reader" or "tag".
    public var direction: String
    /// The link.
    public var link: String
    /// The bytes in hex, or nothing for content not shown.
    public var hex: String
    /// The name of the frame.
    public var name: String
    /// The CRC result.
    public var crc: String
    /// The parity result.
    public var parityOk: Bool
    /// The level of the frame.
    public var levelDbfs: Double

    init(_ c: sdrkit_nfc_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        direction = sdrkitText(c.direction)
        link = sdrkitText(c.link)
        hex = sdrkitText(c.hex)
        name = sdrkitText(c.name)
        crc = sdrkitText(c.crc)
        parityOk = c.parity_ok
        levelDbfs = c.level_dbfs
    }
}

/// The result fields of the Wi-SUN FAN stack.
public struct WisunFields {
    /// Good frames.
    public var framesOk: Int64
    /// Bad frames.
    public var framesBad: Int64
    /// Frames corrected by the least-reliable-bit search.
    public var correctedFrames: Int64
    /// Frames with an FEC SFD, counted and not decoded.
    public var fecFrames: Int64
    /// Mode switches.
    public var modeSwitches: Int64
    /// Secured frames.
    public var encryptedFrames: Int64
    /// The networks in the table.
    public var networks: Int64
    /// The nodes in the table.
    public var nodes: Int64
    /// The links in the table.
    public var links: Int64

    init(_ c: sdrkit_wisun_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        correctedFrames = c.corrected_frames
        fecFrames = c.fec_frames
        modeSwitches = c.mode_switches
        encryptedFrames = c.encrypted_frames
        networks = c.networks
        nodes = c.nodes
        links = c.links
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WisunFields, SDRKitCode> {
        var c = sdrkit_wisun_fields()
        let code = sdrkit_wisun_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WisunFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the Wi-SUN networks. The row key is the PAN id.
public struct WisunNetworkRow {
    /// The row key.
    public var key: String
    /// The network name, once heard.
    public var name: String?
    /// The border router EUI-64, once known.
    public var borderRouter: String?
    /// The PAN size, once heard.
    public var sizeField: Int64?
    /// The nodes in the PAN.
    public var nodes: Int64
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The unicast schedule, once heard.
    public var schedule: String?
    /// The PAN id, as `send` takes it.
    public var panId: String

    init(_ c: sdrkit_wisun_network_row) {
        key = sdrkitText(c.key)
        name = c.has_name ? sdrkitText(c.name) : nil
        borderRouter = c.has_border_router ? sdrkitText(c.border_router) : nil
        sizeField = c.has_size_ ? c.size_ : nil
        nodes = c.nodes
        levelDbfs = c.level_dbfs
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        schedule = c.has_schedule ? sdrkitText(c.schedule) : nil
        panId = sdrkitText(c.pan_id)
    }
}

/// A row of the Wi-SUN nodes. The row key is the EUI-64.
public struct WisunNodeRow {
    /// The row key.
    public var key: String
    /// The vendor of the EUI-64.
    public var vendor: String?
    /// The role: node, LFN, joining, router or border router.
    public var role: String
    /// The row key of the network of the node, once known.
    public var pan: String?
    /// The routing cost, once heard.
    public var cost: Int64?
    /// The level of the last frame.
    public var levelDbfs: Double
    /// Frames sent.
    public var frames: Int64
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The last channel centre.
    public var freq: Double
    /// The authenticator EUI-64, once heard.
    public var authenticator: String?
    /// The EAP state, once heard.
    public var eap: String?
    /// The summary of the last frame.
    public var last: String
    /// The EUI-64, as `send` takes it.
    public var address: String

    init(_ c: sdrkit_wisun_node_row) {
        key = sdrkitText(c.key)
        vendor = c.has_vendor ? sdrkitText(c.vendor) : nil
        role = sdrkitText(c.role)
        pan = c.has_pan ? sdrkitText(c.pan) : nil
        cost = c.has_cost ? c.cost : nil
        levelDbfs = c.level_dbfs
        frames = c.frames
        lastHeard = c.has_last_heard ? c.last_heard : nil
        freq = c.freq
        authenticator = c.has_authenticator ? sdrkitText(c.authenticator) : nil
        eap = c.has_eap ? sdrkitText(c.eap) : nil
        last = sdrkitText(c.last)
        address = sdrkitText(c.address)
    }
}

/// A row of the Wi-SUN links. The row key is the two ends.
public struct WisunLinkRow {
    /// The row key.
    public var key: String
    /// The row key of the sender.
    public var from: String
    /// The row key of the receiver.
    public var to: String
    /// Frames from the sender.
    public var frames: Int64
    /// Acks from the receiver.
    public var acks: Int64
    /// The level the receiver reported for the frames of the sender, once reported.
    public var rslDbm: Double?
    /// The level of the sender here.
    public var levelDbfs: Double
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_wisun_link_row) {
        key = sdrkitText(c.key)
        from = sdrkitText(c.from)
        to = sdrkitText(c.to)
        frames = c.frames
        acks = c.acks
        rslDbm = c.has_rsl_dbm ? c.rsl_dbm : nil
        levelDbfs = c.level_dbfs
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the Wi-SUN frame log. The row key is the number of the frame in the log.
public struct WisunFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// The channel centre.
    public var freq: Double
    /// The operating class and channel number in the plan of the bit rate, if the channel is in the plan.
    public var ch: String?
    /// The bit rate.
    public var kbps: Int64
    /// The UTT or 802.15.4 frame type name.
    public var type: String
    /// "source > destination".
    public var route: String
    /// The level of the frame.
    public var levelDbfs: Double
    /// The summary of the frame.
    public var details: String
    /// The MPDU without the FCS in hex.
    public var mpdu: String

    init(_ c: sdrkit_wisun_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        freq = c.freq
        ch = c.has_ch ? sdrkitText(c.ch) : nil
        kbps = c.kbps
        type = sdrkitText(c.type)
        route = sdrkitText(c.route)
        levelDbfs = c.level_dbfs
        details = sdrkitText(c.details)
        mpdu = sdrkitText(c.mpdu)
    }
}

/// The result fields of the Thread stack.
public struct ThreadFields {
    /// Good frames.
    public var framesOk: Int64
    /// Candidates whose check failed.
    public var framesBad: Int64
    /// Frames that the link layer encrypts.
    public var encryptedFrames: Int64
    /// Beacon frames.
    public var beacons: Int64
    /// Data frames.
    public var dataFrames: Int64
    /// Acknowledgements.
    public var acks: Int64
    /// Command frames.
    public var commands: Int64
    /// Good frames on channel 11.
    public var ch11: Int64
    /// Good frames on channel 12.
    public var ch12: Int64
    /// Good frames on channel 13.
    public var ch13: Int64
    /// Good frames on channel 14.
    public var ch14: Int64
    /// Good frames on channel 15.
    public var ch15: Int64
    /// Good frames on channel 16.
    public var ch16: Int64
    /// Good frames on channel 17.
    public var ch17: Int64
    /// Good frames on channel 18.
    public var ch18: Int64
    /// Good frames on channel 19.
    public var ch19: Int64
    /// Good frames on channel 20.
    public var ch20: Int64
    /// Good frames on channel 21.
    public var ch21: Int64
    /// Good frames on channel 22.
    public var ch22: Int64
    /// Good frames on channel 23.
    public var ch23: Int64
    /// Good frames on channel 24.
    public var ch24: Int64
    /// Good frames on channel 25.
    public var ch25: Int64
    /// Good frames on channel 26.
    public var ch26: Int64
    /// Good frames in the last second.
    public var framesPerS: Int64
    /// The channel listened to (0 when not known).
    public var channel: Int64
    /// The stack steps through the channels.
    public var scanning: Bool
    /// The networks in the table.
    public var networks: Int64
    /// The devices in the table.
    public var devices: Int64
    /// The level of the last frame since the last change of frequency.
    public var levelDbfs: Double?
    /// The SNR of the last frame since the last change of frequency.
    public var snrDb: Double?
    /// The carrier offset of the last frame since the last change of frequency.
    public var offsetHz: Double?

    init(_ c: sdrkit_thread_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        encryptedFrames = c.encrypted_frames
        beacons = c.beacons
        dataFrames = c.data_frames
        acks = c.acks
        commands = c.commands
        ch11 = c.ch11
        ch12 = c.ch12
        ch13 = c.ch13
        ch14 = c.ch14
        ch15 = c.ch15
        ch16 = c.ch16
        ch17 = c.ch17
        ch18 = c.ch18
        ch19 = c.ch19
        ch20 = c.ch20
        ch21 = c.ch21
        ch22 = c.ch22
        ch23 = c.ch23
        ch24 = c.ch24
        ch25 = c.ch25
        ch26 = c.ch26
        framesPerS = c.frames_per_s
        channel = c.channel
        scanning = c.scanning
        networks = c.networks
        devices = c.devices
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ThreadFields, SDRKitCode> {
        var c = sdrkit_thread_fields()
        let code = sdrkit_thread_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ThreadFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the Thread networks. The row key is the PAN and the channel as one number.
public struct ThreadNetworkRow {
    /// The row key.
    public var key: String
    /// The PAN id.
    public var pan: String
    /// The channel where it was heard (0 when not known).
    public var ch: Int64
    /// The network name of a Thread beacon.
    public var networkName: String?
    /// The extended PAN id.
    public var extPan: String?
    /// The Thread version of the beacon.
    public var version: Int64?
    /// The last beacon permitted joining.
    public var joiningOpen: Bool
    /// A Thread beacon or a 6LoWPAN header was seen in the PAN.
    public var threadSeen: Bool
    /// The sender of the beacon.
    public var coordinator: String?
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The times heard.
    public var heard: Int64?
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_thread_network_row) {
        key = sdrkitText(c.key)
        pan = sdrkitText(c.pan)
        ch = c.ch
        networkName = c.has_network_name ? sdrkitText(c.network_name) : nil
        extPan = c.has_ext_pan ? sdrkitText(c.ext_pan) : nil
        version = c.has_version ? c.version : nil
        joiningOpen = c.joining_open
        threadSeen = c.thread_seen
        coordinator = c.has_coordinator ? sdrkitText(c.coordinator) : nil
        levelDbfs = c.level_dbfs
        heard = c.has_heard ? c.heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the Thread frame log. The row key is the number of the frame in the log.
public struct ThreadFrameRow {
    /// The row key.
    public var key: String
    /// The UTC of the frame, when the samples are live.
    public var utc: Double?
    /// The signal time of the frame.
    public var timeS: Double
    /// The channel.
    public var ch: Int64
    /// The MAC frame type.
    public var type: String
    /// The source address.
    public var source: String
    /// The destination address.
    public var target: String
    /// The PAN id, if known.
    public var pan: String?
    /// The PSDU length with the FCS.
    public var len: Int64
    /// The link layer encrypts the payload.
    public var encrypted: Bool
    /// The level of the frame.
    public var levelDbfs: Double
    /// The sequence number, if the frame has one.
    public var seq: Int64?
    /// The frame requests an acknowledgement.
    public var ackRequest: Bool
    /// The description.
    public var details: String

    init(_ c: sdrkit_thread_frame_row) {
        key = sdrkitText(c.key)
        utc = c.has_utc ? c.utc : nil
        timeS = c.time_s
        ch = c.ch
        type = sdrkitText(c.type)
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        pan = c.has_pan ? sdrkitText(c.pan) : nil
        len = c.len
        encrypted = c.encrypted
        levelDbfs = c.level_dbfs
        seq = c.has_seq ? c.seq : nil
        ackRequest = c.ack_request
        details = sdrkitText(c.details)
    }
}

/// The result fields of the mioty stack.
public struct MiotyFields {
    /// Frames read.
    public var framesOk: Int64
    /// Bursts detected.
    public var burstsDetected: Int64
    /// Patterns that did not decode.
    public var failedHypotheses: Int64
    /// The devices in the table.
    public var devices: Int64
    /// The SNR of the last frame. It has no value before the first frame.
    public var snrDb: Double?
    /// The power of the input. It has no value before the first input.
    public var levelDbfs: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_mioty_fields) {
        framesOk = c.frames_ok
        burstsDetected = c.bursts_detected
        failedHypotheses = c.failed_hypotheses
        devices = c.devices
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MiotyFields, SDRKitCode> {
        var c = sdrkit_mioty_fields()
        let code = sdrkit_mioty_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MiotyFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct IsmDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_ism_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IsmDecoderFields, SDRKitCode> {
        var c = sdrkit_ism_decoder_fields()
        let code = sdrkit_ism_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IsmDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the frame log. The row key is the number of the frame in the log.
public struct MiotyFrameRow {
    /// The row key.
    public var key: String
    /// The UTC of the frame, when the samples are live.
    public var utc: Double?
    /// The signal time of the frame.
    public var timeS: Double
    /// The device.
    public var id: String
    /// The pattern of the bursts.
    public var pattern: String
    /// The bytes of the MPDU.
    public var psi: Int64
    /// The MAC mode.
    public var mmode: Int64
    /// The MAC.
    public var mac: String
    /// The first bytes of the MPDU as hex.
    public var mpdu: String
    /// The frequency.
    public var freqHz: Double
    /// The SNR.
    public var snrDb: Double
    /// The bursts of the pattern that matched.
    public var burstsMatched: Int64

    init(_ c: sdrkit_mioty_frame_row) {
        key = sdrkitText(c.key)
        utc = c.has_utc ? c.utc : nil
        timeS = c.time_s
        id = sdrkitText(c.id)
        pattern = sdrkitText(c.pattern)
        psi = c.psi
        mmode = c.mmode
        mac = sdrkitText(c.mac)
        mpdu = sdrkitText(c.mpdu)
        freqHz = c.freq_hz
        snrDb = c.snr_db
        burstsMatched = c.bursts_matched
    }
}

/// A row of the devices. The row key is the device.
public struct MiotyDeviceRow {
    /// The row key.
    public var key: String
    /// The device.
    public var id: String
    /// The frames heard.
    public var frames: Int64
    /// The last packet counter.
    public var counter: Int64
    /// The SNR of the last frame.
    public var snrDb: Double
    /// The frequency of the last frame.
    public var freqHz: Double
    /// The UTC of the first frame.
    public var firstHeard: Double?
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_mioty_device_row) {
        key = sdrkitText(c.key)
        id = sdrkitText(c.id)
        frames = c.frames
        counter = c.counter
        snrDb = c.snr_db
        freqHz = c.freq_hz
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of the DECT NR+ stack.
public struct DectNrplusNrPlusFields {
    /// Packets read with a good check.
    public var framesOk: Int64
    /// Short training field detections.
    public var stfDetections: Int64
    /// The devices in the table.
    public var devices: Int64
    /// The SNR of the last packet. It has no value before the first packet.
    public var snrDb: Double?
    /// The carrier offset of the last packet. It has no value before the first packet.
    public var cfoHz: Double?
    /// The power of the input. It has no value before the first input.
    public var levelDbfs: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_dect_nrplus_nr_plus_fields) {
        framesOk = c.frames_ok
        stfDetections = c.stf_detections
        devices = c.devices
        snrDb = c.has_snr_db ? c.snr_db : nil
        cfoHz = c.has_cfo_hz ? c.cfo_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DectNrplusNrPlusFields, SDRKitCode> {
        var c = sdrkit_dect_nrplus_nr_plus_fields()
        let code = sdrkit_dect_nrplus_nr_plus_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DectNrplusNrPlusFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the packet log. The row key is the number of the packet in the log.
public struct DectNrplusNrPlusPacketRow {
    /// The row key.
    public var key: String
    /// The UTC of the packet, when the samples are live.
    public var utc: Double?
    /// The signal time of the packet.
    public var timeS: Double
    /// The device.
    public var id: String
    /// The type of the packet.
    public var type: String
    /// The fields of the packet.
    public var text: String
    /// The carrier offset.
    public var cfoHz: Double
    /// The SNR.
    public var snrDb: Double

    init(_ c: sdrkit_dect_nrplus_nr_plus_packet_row) {
        key = sdrkitText(c.key)
        utc = c.has_utc ? c.utc : nil
        timeS = c.time_s
        id = sdrkitText(c.id)
        type = sdrkitText(c.type)
        text = sdrkitText(c.text)
        cfoHz = c.cfo_hz
        snrDb = c.snr_db
    }
}

/// A row of the devices. The row key is the device number.
public struct DectNrplusNrPlusDeviceRow {
    /// The row key.
    public var key: String
    /// The transmitter.
    public var transmitter: String
    /// The device.
    public var id: String
    /// The packets heard.
    public var packets: Int64
    /// The fields of the last packet.
    public var text: String
    /// The SNR of the last packet.
    public var snrDb: Double
    /// The UTC of the first packet.
    public var firstHeard: Double?
    /// The UTC of the last packet.
    public var lastHeard: Double?

    init(_ c: sdrkit_dect_nrplus_nr_plus_device_row) {
        key = sdrkitText(c.key)
        transmitter = sdrkitText(c.transmitter)
        id = sdrkitText(c.id)
        packets = c.packets
        text = sdrkitText(c.text)
        snrDb = c.snr_db
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of the New Packet Radio stack.
public struct Npr70NprFields {
    /// Sync words found.
    public var syncWords: Int64
    /// The symbol rates searched, in ksym/s.
    public var ratesSearched: String
    /// The power of the input. It has no value before the first input.
    public var levelDbfs: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_npr70_npr_fields) {
        syncWords = c.sync_words
        ratesSearched = sdrkitText(c.rates_searched)
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Npr70NprFields, SDRKitCode> {
        var c = sdrkit_npr70_npr_fields()
        let code = sdrkit_npr70_npr_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Npr70NprFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the frame log. The row key is the number of the frame in the log.
public struct Npr70NprFrameRow {
    /// The row key.
    public var key: String
    /// The UTC of the frame, when the samples are live.
    public var utc: Double?
    /// The signal time of the frame.
    public var timeS: Double
    /// The network.
    public var network: String
    /// The symbol rate.
    public var symbolRateHz: Double
    /// The sync word.
    public var sync: String
    /// The tuning offset of the frame.
    public var tuningHz: Double
    /// What the payload is.
    public var payload: String

    init(_ c: sdrkit_npr70_npr_frame_row) {
        key = sdrkitText(c.key)
        utc = c.has_utc ? c.utc : nil
        timeS = c.time_s
        network = sdrkitText(c.network)
        symbolRateHz = c.symbol_rate_hz
        sync = sdrkitText(c.sync)
        tuningHz = c.tuning_hz
        payload = sdrkitText(c.payload)
    }
}

/// A row of the networks heard. The row key is the network number.
public struct Npr70NprNetworkRow {
    /// The row key.
    public var key: String
    /// The network.
    public var network: String
    /// The frames heard.
    public var frames: Int64
    /// The symbol rate.
    public var symbolRateHz: Double
    /// The UTC of the first frame.
    public var firstHeard: Double?
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_npr70_npr_network_row) {
        key = sdrkitText(c.key)
        network = sdrkitText(c.network)
        frames = c.frames
        symbolRateHz = c.symbol_rate_hz
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of the DSRC road toll stack.
public struct DsrcTollDsrcFields {
    /// Frames with a good check.
    public var framesOk: Int64
    /// Broadcast frames.
    public var broadcastFrames: Int64
    /// Private link frames.
    public var privateFrames: Int64
    /// Frames that failed their check.
    public var framesBad: Int64
    /// The power of the input. It has no value before the first input.
    public var levelDbfs: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_dsrc_toll_dsrc_fields) {
        framesOk = c.frames_ok
        broadcastFrames = c.broadcast_frames
        privateFrames = c.private_frames
        framesBad = c.frames_bad
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DsrcTollDsrcFields, SDRKitCode> {
        var c = sdrkit_dsrc_toll_dsrc_fields()
        let code = sdrkit_dsrc_toll_dsrc_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DsrcTollDsrcFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the frame log. The row key is the number of the frame in the log.
public struct DsrcTollDsrcFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// The class of the frame.
    public var kind: String
    /// The bytes of the frame, without the check.
    public var bytes: Int64
    /// The control byte as hex, if the frame has one.
    public var control: String?
    /// What is not shown.
    public var identifier: String

    init(_ c: sdrkit_dsrc_toll_dsrc_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = sdrkitText(c.kind)
        bytes = c.bytes
        control = c.has_control ? sdrkitText(c.control) : nil
        identifier = sdrkitText(c.identifier)
    }
}

/// The result fields of the EnOcean stack.
public struct EnoceanFields {
    /// Telegrams; the subtelegrams of one telegram count once.
    public var telegrams: Int64
    /// Subtelegrams with a good check.
    public var subtelegrams: Int64
    /// Frames that the line coding accepted and the check refused.
    public var framesBad: Int64
    /// Senders heard.
    public var devices: Int64
    /// Bursts over the noise floor.
    public var bursts: Int64
    /// The power of the input. It has no value before the first input.
    public var levelDbfs: Double?
    /// What no real signal has confirmed and what is not read.
    public var note: String

    init(_ c: sdrkit_enocean_fields) {
        telegrams = c.telegrams
        subtelegrams = c.subtelegrams
        framesBad = c.frames_bad
        devices = c.devices
        bursts = c.bursts
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EnoceanFields, SDRKitCode> {
        var c = sdrkit_enocean_fields()
        let code = sdrkit_enocean_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(EnoceanFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the devices. The row key is the sender id, hex.
public struct EnoceanDeviceRow {
    /// The row key.
    public var key: String
    /// The name of the device for people: "EnOcean", the profile when known, and the id.
    public var name: String
    /// The radio protocol last heard.
    public var radio: String
    /// The last telegram type.
    public var telegramType: String
    /// The profile (EEP), from a teach-in or certain from the telegram type.
    public var profile: String?
    /// The last reading.
    public var reading: String
    /// Telegrams heard.
    public var telegrams: Int64
    /// Secure telegrams heard.
    public var secure: Int64
    /// The level of the last telegram.
    public var levelDbfs: Double
    /// The UTC of the first telegram.
    public var firstHeard: Double?
    /// The UTC of the last telegram.
    public var lastHeard: Double?

    init(_ c: sdrkit_enocean_device_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        radio = sdrkitText(c.radio)
        telegramType = sdrkitText(c.telegram_type)
        profile = c.has_profile ? sdrkitText(c.profile) : nil
        reading = sdrkitText(c.reading)
        telegrams = c.telegrams
        secure = c.secure
        levelDbfs = c.level_dbfs
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the telegram log. The row key is the number of the telegram in the log.
public struct EnoceanTelegramRow {
    /// The row key.
    public var key: String
    /// The UTC of the first subtelegram, when the samples are live.
    public var utc: Double?
    /// The signal time of the first subtelegram.
    public var timeS: Double
    /// The sender id, hex.
    public var sender: String
    /// The radio protocol: ERP1 or ERP2.
    public var radio: String
    /// The telegram type (RORG).
    public var telegramType: String
    /// What the telegram says.
    public var reading: String
    /// The payload as hex; empty for a secure telegram.
    public var data: String
    /// The destination id of an addressed telegram, hex.
    public var destination: String?
    /// The repeater level of the first subtelegram.
    public var repeater: String
    /// Subtelegrams heard of this telegram.
    public var subtelegrams: Int64
    /// The check that passed.
    public var check: String
    /// The level of the first subtelegram.
    public var levelDbfs: Double

    init(_ c: sdrkit_enocean_telegram_row) {
        key = sdrkitText(c.key)
        utc = c.has_utc ? c.utc : nil
        timeS = c.time_s
        sender = sdrkitText(c.sender)
        radio = sdrkitText(c.radio)
        telegramType = sdrkitText(c.telegram_type)
        reading = sdrkitText(c.reading)
        data = sdrkitText(c.data)
        destination = c.has_destination ? sdrkitText(c.destination) : nil
        repeater = sdrkitText(c.repeater)
        subtelegrams = c.subtelegrams
        check = sdrkitText(c.check)
        levelDbfs = c.level_dbfs
    }
}

/// The fields of a device heard for the first time. `source` is the model and the id.
public struct IsmDeviceEvent {
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
    /// The model and the id.
    public var source: String
    /// The line of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The device model.
    public var model: String
    /// The device id.
    public var id: String

    init(_ c: sdrkit_ism_device_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        model = sdrkitText(c.model)
        id = sdrkitText(c.id)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<IsmDeviceEvent, SDRKitCode> {
        var c = sdrkit_ism_device_event()
        let code = sdrkit_ism_device_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(IsmDeviceEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new message event.
public struct SigfoxMessageEvent {
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
    /// The device id.
    public var source: String
    /// The log line of the message.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The device id.
    public var deviceId: String
    /// The message counter.
    public var seq: Int64
    /// The payload as hex.
    public var payload: String

    init(_ c: sdrkit_sigfox_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        deviceId = sdrkitText(c.device_id)
        seq = c.seq
        payload = sdrkitText(c.payload)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SigfoxMessageEvent, SDRKitCode> {
        var c = sdrkit_sigfox_message_event()
        let code = sdrkit_sigfox_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SigfoxMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an analysed frame with a line code.
public struct IsmPulseFrameEvent {
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
    public enum LineCode: Int32 {
        case pwm = 0
        case ppm = 1
        case manchester = 2
        case pcm = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The line of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The line code guessed from the pulse timings.
    public var lineCode: LineCode?
    /// Bits read.
    public var bits: Int64
    /// The first 96 bits as hex.
    public var bitsHex: String
    /// Frames in a row with the same bits, this one included.
    public var repeats: Int64
    /// The level of the marks.
    public var levelDbfs: Double

    init(_ c: sdrkit_ism_pulse_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        lineCode = LineCode(rawValue: c.line_code)
        bits = c.bits
        bitsHex = sdrkitText(c.bits_hex)
        repeats = c.repeats
        levelDbfs = c.level_dbfs
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<IsmPulseFrameEvent, SDRKitCode> {
        var c = sdrkit_ism_pulse_frame_event()
        let code = sdrkit_ism_pulse_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(IsmPulseFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of a transmission of a frame stack.
public struct IsmTxEvent {
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
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
    public var reason: String
    /// The addressee or the identity of the frame.
    public var target: String
    /// What the frame carries, for started and queued; empty otherwise.
    public var text: String
    /// The family of the source: a sensor.
    public var family: Family?

    init(_ c: sdrkit_ism_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<IsmTxEvent, SDRKitCode> {
        var c = sdrkit_ism_tx_event()
        let code = sdrkit_ism_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(IsmTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a frame event.
public struct WmbusFrameEvent {
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
    /// The meter: maker and id ("KAM 12345678").
    public var source: String
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The manufacturer letters.
    public var maker: String
    /// The meter id.
    public var meterId: String
    /// The device type name.
    public var deviceType: String
    /// The physical mode.
    public var phy: String
    /// The payload is encrypted.
    public var encrypted: Bool
    /// The level of the frame.
    public var levelDbfs: Double

    init(_ c: sdrkit_wmbus_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        maker = sdrkitText(c.maker)
        meterId = sdrkitText(c.meter_id)
        deviceType = sdrkitText(c.device_type)
        phy = sdrkitText(c.phy)
        encrypted = c.encrypted
        levelDbfs = c.level_dbfs
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<WmbusFrameEvent, SDRKitCode> {
        var c = sdrkit_wmbus_frame_event()
        let code = sdrkit_wmbus_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(WmbusFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of a transmission of a frame stack.
public struct WmbusTxEvent {
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
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
    public var reason: String
    /// The addressee or the identity of the frame.
    public var target: String
    /// What the frame carries, for started and queued; empty otherwise.
    public var text: String
    /// The family of the source: a sensor.
    public var family: Family?

    init(_ c: sdrkit_wmbus_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<WmbusTxEvent, SDRKitCode> {
        var c = sdrkit_wmbus_tx_event()
        let code = sdrkit_wmbus_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(WmbusTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a Z-Wave frame event.
public struct ZwaveFrameEvent {
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
    /// The sender: home id and node ("C0FFEE01:2").
    public var source: String
    /// The addressee, for a frame to one node.
    public var target: String?
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The home id.
    public var homeId: String
    /// Source and destination.
    public var route: String
    /// The rate.
    public var rate: String
    /// The header type.
    public var frameType: String
    /// The encryption.
    public var encryption: String
    /// The one-line description.
    public var command: String
    /// The level of the frame.
    public var levelDbfs: Double
    /// The SNR of the frame.
    public var snrDb: Double

    init(_ c: sdrkit_zwave_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = c.has_target ? sdrkitText(c.target) : nil
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        homeId = sdrkitText(c.home_id)
        route = sdrkitText(c.route)
        rate = sdrkitText(c.rate)
        frameType = sdrkitText(c.frame_type)
        encryption = sdrkitText(c.encryption)
        command = sdrkitText(c.command)
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ZwaveFrameEvent, SDRKitCode> {
        var c = sdrkit_zwave_frame_event()
        let code = sdrkit_zwave_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ZwaveFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event that Z-Wave frames start or stop coming in.
public struct ZwaveStateEvent {
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
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_zwave_state_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ZwaveStateEvent, SDRKitCode> {
        var c = sdrkit_zwave_state_event()
        let code = sdrkit_zwave_state_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ZwaveStateEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of a transmission of a frame stack.
public struct ZwaveTxEvent {
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
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
    public var reason: String
    /// The addressee or the identity of the frame.
    public var target: String
    /// What the frame carries, for started and queued; empty otherwise.
    public var text: String
    /// The family of the source: a sensor.
    public var family: Family?

    init(_ c: sdrkit_zwave_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ZwaveTxEvent, SDRKitCode> {
        var c = sdrkit_zwave_tx_event()
        let code = sdrkit_zwave_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ZwaveTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new PAN event.
public struct ZigbeePanEvent {
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
    /// The PAN id.
    public var source: String
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The PAN id.
    public var pan: String
    /// The channel.
    public var ch: Int64

    init(_ c: sdrkit_zigbee_pan_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        pan = sdrkitText(c.pan)
        ch = c.ch
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ZigbeePanEvent, SDRKitCode> {
        var c = sdrkit_zigbee_pan_event()
        let code = sdrkit_zigbee_pan_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ZigbeePanEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of a transmission of a frame stack.
public struct ZigbeeTxEvent {
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
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
    public var reason: String
    /// The addressee or the identity of the frame.
    public var target: String
    /// What the frame carries, for started and queued; empty otherwise.
    public var text: String
    /// The family of the source: a sensor.
    public var family: Family?

    init(_ c: sdrkit_zigbee_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ZigbeeTxEvent, SDRKitCode> {
        var c = sdrkit_zigbee_tx_event()
        let code = sdrkit_zigbee_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ZigbeeTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new sensor event.
public struct AntSensorEvent {
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
    /// The device number.
    public var source: String
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The device number.
    public var device: Int64
    /// The device type name.
    public var type: String
    /// The network marker as hex.
    public var marker: String

    init(_ c: sdrkit_ant_sensor_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        device = c.device
        type = sdrkitText(c.type)
        marker = sdrkitText(c.marker)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AntSensorEvent, SDRKitCode> {
        var c = sdrkit_ant_sensor_event()
        let code = sdrkit_ant_sensor_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AntSensorEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of a transmission of a frame stack.
public struct AntTxEvent {
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
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
    public var reason: String
    /// The addressee or the identity of the frame.
    public var target: String
    /// What the frame carries, for started and queued; empty otherwise.
    public var text: String
    /// The family of the source: a sensor.
    public var family: Family?

    init(_ c: sdrkit_ant_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AntTxEvent, SDRKitCode> {
        var c = sdrkit_ant_tx_event()
        let code = sdrkit_ant_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AntTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new address event.
public struct Nrf24AddressEvent {
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
    /// The address as hex.
    public var source: String
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The bit rate.
    public var bitRate: Double
    /// The CRC bits.
    public var crcBits: Int64

    init(_ c: sdrkit_nrf24_address_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        bitRate = c.bit_rate
        crcBits = c.crc_bits
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Nrf24AddressEvent, SDRKitCode> {
        var c = sdrkit_nrf24_address_event()
        let code = sdrkit_nrf24_address_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Nrf24AddressEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a chat line event.
public struct SubghzchatLineEvent {
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
        case plain = 0
        case decrypted = 1
        case encrypted = 2
        case `repeat` = 3
        case octets = 4
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// How the line was read.
    public var kind: Kind?
    /// The SNR of the first packet.
    public var snrDb: Double

    init(_ c: sdrkit_subghzchat_line_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        kind = Kind(rawValue: c.kind)
        snrDb = c.snr_db
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SubghzchatLineEvent, SDRKitCode> {
        var c = sdrkit_subghzchat_line_event()
        let code = sdrkit_subghzchat_line_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SubghzchatLineEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event that packets come in after a quiet time.
public struct SubghzchatStateEvent {
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
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_subghzchat_state_event) {
        time = c.time
        freqHz = c.freq_hz
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SubghzchatStateEvent, SDRKitCode> {
        var c = sdrkit_subghzchat_state_event()
        let code = sdrkit_subghzchat_state_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SubghzchatStateEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new base station event.
public struct DectBaseEvent {
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
    /// The RFPI as 10 hex digits.
    public var source: String
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The RFPI as 10 hex digits.
    public var rfpi: String
    /// The equipment manufacturer code (class A).
    public var emc: Int64?

    init(_ c: sdrkit_dect_base_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        rfpi = sdrkitText(c.rfpi)
        emc = c.has_emc ? c.emc : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DectBaseEvent, SDRKitCode> {
        var c = sdrkit_dect_base_event()
        let code = sdrkit_dect_base_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DectBaseEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a tag event.
public struct NfcTagEvent {
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
    /// The UID as hex.
    public var source: String
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The UID as hex.
    public var uid: String

    init(_ c: sdrkit_nfc_tag_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        uid = sdrkitText(c.uid)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<NfcTagEvent, SDRKitCode> {
        var c = sdrkit_nfc_tag_event()
        let code = sdrkit_nfc_tag_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(NfcTagEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new Wi-SUN PAN event.
public struct WisunPanEvent {
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
    /// The PAN id.
    public var source: String
    /// The text of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?
    /// The PAN id.
    public var pan: String

    init(_ c: sdrkit_wisun_pan_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
        pan = sdrkitText(c.pan)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<WisunPanEvent, SDRKitCode> {
        var c = sdrkit_wisun_pan_event()
        let code = sdrkit_wisun_pan_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(WisunPanEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a change of a transmission of a frame stack.
public struct WisunTxEvent {
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
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
    public var reason: String
    /// The addressee or the identity of the frame.
    public var target: String
    /// What the frame carries, for started and queued; empty otherwise.
    public var text: String
    /// The family of the source: a sensor.
    public var family: Family?

    init(_ c: sdrkit_wisun_tx_event) {
        time = c.time
        freqHz = c.freq_hz
        state = sdrkitText(c.state)
        reason = sdrkitText(c.reason)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<WisunTxEvent, SDRKitCode> {
        var c = sdrkit_wisun_tx_event()
        let code = sdrkit_wisun_tx_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(WisunTxEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new Thread network event.
public struct ThreadNetworkEvent {
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
    /// The PAN id.
    public var source: String
    /// The text of the event.
    public var text: String
    /// The network name.
    public var networkName: String
    /// The extended PAN id.
    public var extPan: String
    /// The channel.
    public var ch: Int64
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_thread_network_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        networkName = sdrkitText(c.network_name)
        extPan = sdrkitText(c.ext_pan)
        ch = c.ch
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ThreadNetworkEvent, SDRKitCode> {
        var c = sdrkit_thread_network_event()
        let code = sdrkit_thread_network_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ThreadNetworkEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a new Thread device event.
public struct ThreadDeviceEvent {
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
    /// The address of the device.
    public var source: String
    /// The PAN id, if known.
    public var pan: String?
    /// The channel.
    public var ch: Int64
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_thread_device_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        pan = c.has_pan ? sdrkitText(c.pan) : nil
        ch = c.ch
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ThreadDeviceEvent, SDRKitCode> {
        var c = sdrkit_thread_device_event()
        let code = sdrkit_thread_device_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ThreadDeviceEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a frame event.
public struct MiotyFrameEvent {
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
    /// The device.
    public var id: String
    /// The pattern of the bursts.
    public var pattern: String
    /// The bytes of the MPDU.
    public var psi: Int64
    /// The MAC.
    public var mac: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_mioty_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        id = sdrkitText(c.id)
        pattern = sdrkitText(c.pattern)
        psi = c.psi
        mac = sdrkitText(c.mac)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MiotyFrameEvent, SDRKitCode> {
        var c = sdrkit_mioty_frame_event()
        let code = sdrkit_mioty_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MiotyFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a packet event.
public struct DectNrplusPacketEvent {
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
    /// The device.
    public var id: String
    /// The type of the packet.
    public var type: String
    /// The fields of the packet.
    public var text: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_dect_nrplus_packet_event) {
        time = c.time
        freqHz = c.freq_hz
        id = sdrkitText(c.id)
        type = sdrkitText(c.type)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DectNrplusPacketEvent, SDRKitCode> {
        var c = sdrkit_dect_nrplus_packet_event()
        let code = sdrkit_dect_nrplus_packet_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DectNrplusPacketEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a frame event.
public struct Npr70FrameEvent {
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
    /// The network.
    public var network: String
    /// The symbol rate.
    public var symbolRateHz: Double
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_npr70_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        network = sdrkitText(c.network)
        symbolRateHz = c.symbol_rate_hz
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<Npr70FrameEvent, SDRKitCode> {
        var c = sdrkit_npr70_frame_event()
        let code = sdrkit_npr70_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(Npr70FrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a frame event.
public struct DsrcTollFrameEvent {
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
    /// The class of the frame.
    public var kind: String
    /// The bytes of the frame, without the check.
    public var bytes: Int64
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_dsrc_toll_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        kind = sdrkitText(c.kind)
        bytes = c.bytes
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<DsrcTollFrameEvent, SDRKitCode> {
        var c = sdrkit_dsrc_toll_frame_event()
        let code = sdrkit_dsrc_toll_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(DsrcTollFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a telegram event.
public struct EnoceanTelegramEvent {
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
    /// The sender id, hex.
    public var sender: String
    /// The telegram type (RORG).
    public var telegramType: String
    /// What the telegram says.
    public var reading: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_enocean_telegram_event) {
        time = c.time
        freqHz = c.freq_hz
        sender = sdrkitText(c.sender)
        telegramType = sdrkitText(c.telegram_type)
        reading = sdrkitText(c.reading)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<EnoceanTelegramEvent, SDRKitCode> {
        var c = sdrkit_enocean_telegram_event()
        let code = sdrkit_enocean_telegram_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(EnoceanTelegramEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "ant" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct AntDropped {
    /// The items that the component "demod" dropped.
    public var demod: Int64?

    init(_ c: sdrkit_ant_dropped) {
        demod = c.has_demod ? c.demod : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AntDropped, SDRKitCode> {
        var c = sdrkit_ant_dropped()
        let code = sdrkit_ant_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(AntDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "nrf24" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct Nrf24Dropped {
    /// The items that the component "demod" dropped.
    public var demod: Int64?

    init(_ c: sdrkit_nrf24_dropped) {
        demod = c.has_demod ? c.demod : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Nrf24Dropped, SDRKitCode> {
        var c = sdrkit_nrf24_dropped()
        let code = sdrkit_nrf24_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(Nrf24Dropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "dect" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct DectDropped {
    /// The items that the component "demod" dropped.
    public var demod: Int64?

    init(_ c: sdrkit_dect_dropped) {
        demod = c.has_demod ? c.demod : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DectDropped, SDRKitCode> {
        var c = sdrkit_dect_dropped()
        let code = sdrkit_dect_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(DectDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum IsmComponents {
    /// The result fields of the component "burstdetect" of the stack "ism" in the snapshot of `handle`.
    public static func ismBurstdetectIsmDetectFields(_ handle: SDRKIT_HANDLE) -> Result<IsmDetectFields, SDRKitCode> {
        var c = sdrkit_ism_detect_fields()
        let code = sdrkit_ism_burstdetect_ism_detect_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IsmDetectFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "decode" of the stack "ism" in the snapshot of `handle`.
    public static func ismDecodeFields(_ handle: SDRKIT_HANDLE) -> Result<OokdevDecodeFields, SDRKitCode> {
        var c = sdrkit_ookdev_decode_fields()
        let code = sdrkit_ism_decode_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(OokdevDecodeFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "sfx_finder" of the stack "ism" in the snapshot of `handle`.
    public static func ismSfxFinderSigfoxFinderFields(_ handle: SDRKIT_HANDLE) -> Result<SigfoxFinderFields, SDRKitCode> {
        var c = sdrkit_sigfox_finder_fields()
        let code = sdrkit_ism_sfx_finder_sigfox_finder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SigfoxFinderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "finder" of the stack "sigfox" in the snapshot of `handle`.
    public static func sigfoxFinderSigfoxFinderFields(_ handle: SDRKIT_HANDLE) -> Result<SigfoxFinderFields, SDRKitCode> {
        var c = sdrkit_sigfox_finder_fields()
        let code = sdrkit_sigfox_finder_sigfox_finder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SigfoxFinderFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "demod" of the stack "sigfox" in the snapshot of `handle`.
    public static func sigfoxDemodSigfoxDemodFields(_ handle: SDRKIT_HANDLE) -> Result<SigfoxDemodFields, SDRKitCode> {
        var c = sdrkit_sigfox_demod_fields()
        let code = sdrkit_sigfox_demod_sigfox_demod_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SigfoxDemodFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "demod" of the stack "ant" in the snapshot of `handle`.
    public static func antDemodAntDemodFields(_ handle: SDRKIT_HANDLE) -> Result<AntDemodFields, SDRKitCode> {
        var c = sdrkit_ant_demod_fields()
        let code = sdrkit_ant_demod_ant_demod_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AntDemodFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "demod" of the stack "dect" in the snapshot of `handle`.
    public static func dectDemodDectDemodFields(_ handle: SDRKIT_HANDLE) -> Result<DectDemodFields, SDRKitCode> {
        var c = sdrkit_dect_demod_fields()
        let code = sdrkit_dect_demod_dect_demod_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DectDemodFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "detect" of the stack "nfc" in the snapshot of `handle`.
    public static func nfcDetectNfcDetectFields(_ handle: SDRKIT_HANDLE) -> Result<NfcDetectFields, SDRKitCode> {
        var c = sdrkit_nfc_detect_fields()
        let code = sdrkit_nfc_detect_nfc_detect_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(NfcDetectFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum IsmTables {
    /// The changed rows of the table "devices" of the stack "ism", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `last_heard`.
    public static func ismDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[IsmDeviceRow], SDRKitCode> {
        var rows = [sdrkit_ism_device_row](repeating: sdrkit_ism_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ism_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsmDeviceRow.init))
    }
    /// The changed rows of the table "zwave" of the stack "ism", newest first, at most `capacity` rows.
    public static func ismZwave(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[IsmZwaveRow], SDRKitCode> {
        var rows = [sdrkit_ism_zwave_row](repeating: sdrkit_ism_zwave_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ism_zwave_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsmZwaveRow.init))
    }
    /// The changed rows of the table "log" of the stack "ism", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func ismLog(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[IsmLogRow], SDRKitCode> {
        var rows = [sdrkit_ism_log_row](repeating: sdrkit_ism_log_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ism_log_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsmLogRow.init))
    }
    /// The changed rows of the table "unknown" of the stack "ism", newest first, at most `capacity` rows.
    public static func ismUnknown(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[IsmUnknownRow], SDRKitCode> {
        var rows = [sdrkit_ism_unknown_row](repeating: sdrkit_ism_unknown_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ism_unknown_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsmUnknownRow.init))
    }
    /// The changed rows of the table "sigfox" of the stack "ism", newest first, at most `capacity` rows.
    /// The column `device_id` names a row.
    public static func ismSigfox(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[IsmSigfoxRow], SDRKitCode> {
        var rows = [sdrkit_ism_sigfox_row](repeating: sdrkit_ism_sigfox_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ism_sigfox_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsmSigfoxRow.init))
    }
    /// The changed rows of the table "pulse_frames" of the stack "ism", newest first, at most `capacity` rows.
    public static func ismPulseFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[IsmPulseFrameRow], SDRKitCode> {
        var rows = [sdrkit_ism_pulse_frame_row](repeating: sdrkit_ism_pulse_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ism_pulse_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsmPulseFrameRow.init))
    }
    /// The changed rows of the table "meters" of the stack "wmbus", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `meter_id` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `last_heard`.
    public static func wmbusMeters(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[WmbusMeterRow], SDRKitCode> {
        var rows = [sdrkit_wmbus_meter_row](repeating: sdrkit_wmbus_meter_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wmbus_meters_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WmbusMeterRow.init))
    }
    /// The changed rows of the table "meter_records" of the stack "wmbus", newest first, at most `capacity` rows.
    public static func wmbusMeterRecords(_ handle: SDRKIT_HANDLE, capacity: Int = 3000) -> Result<[WmbusRecordRow], SDRKitCode> {
        var rows = [sdrkit_wmbus_record_row](repeating: sdrkit_wmbus_record_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wmbus_meter_records_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WmbusRecordRow.init))
    }
    /// The changed rows of the table "frames" of the stack "wmbus", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time`.
    public static func wmbusFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[WmbusFrameRow], SDRKitCode> {
        var rows = [sdrkit_wmbus_frame_row](repeating: sdrkit_wmbus_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wmbus_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WmbusFrameRow.init))
    }
    /// The changed rows of the table "channels" of the stack "wmbus", newest first, at most `capacity` rows.
    public static func wmbusChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[WmbusChannelRow], SDRKitCode> {
        var rows = [sdrkit_wmbus_channel_row](repeating: sdrkit_wmbus_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wmbus_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WmbusChannelRow.init))
    }
    /// The changed rows of the table "frames" of the stack "zwave", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time`.
    public static func zwaveFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[ZwaveFrameRow], SDRKitCode> {
        var rows = [sdrkit_zwave_frame_row](repeating: sdrkit_zwave_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_zwave_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ZwaveFrameRow.init))
    }
    /// The changed rows of the table "nodes" of the stack "zwave", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `last_heard`.
    public static func zwaveNodes(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[ZwaveNodeRow], SDRKitCode> {
        var rows = [sdrkit_zwave_node_row](repeating: sdrkit_zwave_node_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_zwave_nodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ZwaveNodeRow.init))
    }
    /// The changed rows of the table "networks" of the stack "zwave", newest first, at most `capacity` rows.
    /// The column `home_id` names a row.
    /// Default order: the column `level_dbfs`, highest first.
    public static func zwaveNetworks(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[ZwaveNetworkRow], SDRKitCode> {
        var rows = [sdrkit_zwave_network_row](repeating: sdrkit_zwave_network_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_zwave_networks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ZwaveNetworkRow.init))
    }
    /// The changed rows of the table "channels" of the stack "zwave", newest first, at most `capacity` rows.
    public static func zwaveChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[ZwaveChannelRow], SDRKitCode> {
        var rows = [sdrkit_zwave_channel_row](repeating: sdrkit_zwave_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_zwave_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ZwaveChannelRow.init))
    }
    /// The changed rows of the table "classes" of the stack "zwave", newest first, at most `capacity` rows.
    public static func zwaveClasses(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[ZwaveClassRow], SDRKitCode> {
        var rows = [sdrkit_zwave_class_row](repeating: sdrkit_zwave_class_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_zwave_classes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ZwaveClassRow.init))
    }
    /// The changed rows of the table "networks" of the stack "zigbee", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Default order: the column `level_dbfs`, highest first.
    public static func zigbeeNetworks(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[ZigbeeNetworkRow], SDRKitCode> {
        var rows = [sdrkit_zigbee_network_row](repeating: sdrkit_zigbee_network_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_zigbee_networks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ZigbeeNetworkRow.init))
    }
    /// The changed rows of the table "devices" of the stack "zigbee", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `last_heard`.
    public static func zigbeeDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[IsmPanDeviceRow], SDRKitCode> {
        var rows = [sdrkit_ism_pan_device_row](repeating: sdrkit_ism_pan_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_zigbee_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsmPanDeviceRow.init))
    }
    /// The changed rows of the table "frames" of the stack "zigbee", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time_s`.
    public static func zigbeeFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[ZigbeeFrameRow], SDRKitCode> {
        var rows = [sdrkit_zigbee_frame_row](repeating: sdrkit_zigbee_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_zigbee_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ZigbeeFrameRow.init))
    }
    /// The changed rows of the table "sigfox" of the stack "sigfox", newest first, at most `capacity` rows.
    /// The column `device_id` names a row.
    public static func sigfox(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[SigfoxMessageRow], SDRKitCode> {
        var rows = [sdrkit_sigfox_message_row](repeating: sdrkit_sigfox_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_sigfox_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SigfoxMessageRow.init))
    }
    /// The changed rows of the table "sensors" of the stack "ant", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `last_heard`.
    public static func antSensors(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[AntSensorRow], SDRKitCode> {
        var rows = [sdrkit_ant_sensor_row](repeating: sdrkit_ant_sensor_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ant_sensors_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AntSensorRow.init))
    }
    /// The changed rows of the table "frames" of the stack "ant", newest first, at most `capacity` rows.
    public static func antFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[AntFrameRow], SDRKitCode> {
        var rows = [sdrkit_ant_frame_row](repeating: sdrkit_ant_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ant_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AntFrameRow.init))
    }
    /// The changed rows of the table "frames" of the stack "nrf24", newest first, at most `capacity` rows.
    /// The column `address` names a row.
    public static func nrf24Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[Nrf24NrfFrameRow], SDRKitCode> {
        var rows = [sdrkit_nrf24_nrf_frame_row](repeating: sdrkit_nrf24_nrf_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nrf24_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Nrf24NrfFrameRow.init))
    }
    /// The changed rows of the table "addresses" of the stack "nrf24", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `address` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `last_heard`.
    public static func nrf24Addresses(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[Nrf24NrfAddressRow], SDRKitCode> {
        var rows = [sdrkit_nrf24_nrf_address_row](repeating: sdrkit_nrf24_nrf_address_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nrf24_addresses_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Nrf24NrfAddressRow.init))
    }
    /// The changed rows of the table "bad_bursts" of the stack "nrf24", newest first, at most `capacity` rows.
    public static func nrf24BadBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[Nrf24NrfBadBurstRow], SDRKitCode> {
        var rows = [sdrkit_nrf24_nrf_bad_burst_row](repeating: sdrkit_nrf24_nrf_bad_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nrf24_bad_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Nrf24NrfBadBurstRow.init))
    }
    /// The changed rows of the table "chat" of the stack "subghzchat", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// Text: `text`. Time: `time_s`.
    /// Own message: `own`.
    public static func subghzchatChat(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[SubghzchatLineRow], SDRKitCode> {
        var rows = [sdrkit_subghzchat_line_row](repeating: sdrkit_subghzchat_line_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_subghzchat_chat_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SubghzchatLineRow.init))
    }
    /// The changed rows of the table "packets" of the stack "subghzchat", newest first, at most `capacity` rows.
    /// The column `hex` names a row.
    public static func subghzchatPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[SubghzchatPacketRow], SDRKitCode> {
        var rows = [sdrkit_subghzchat_packet_row](repeating: sdrkit_subghzchat_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_subghzchat_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SubghzchatPacketRow.init))
    }
    /// The changed rows of the table "bases" of the stack "dect", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `rfpi` names a row.
    /// Default order: the column `level_dbfs`, highest first.
    /// Time: `last_heard`.
    public static func dectBases(_ handle: SDRKIT_HANDLE, capacity: Int = 60) -> Result<[DectBaseRow], SDRKitCode> {
        var rows = [sdrkit_dect_base_row](repeating: sdrkit_dect_base_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dect_bases_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DectBaseRow.init))
    }
    /// The changed rows of the table "bursts" of the stack "dect", newest first, at most `capacity` rows.
    /// The column `from` names a row.
    public static func dectBursts(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DectBurstRow], SDRKitCode> {
        var rows = [sdrkit_dect_burst_row](repeating: sdrkit_dect_burst_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dect_bursts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DectBurstRow.init))
    }
    /// The changed rows of the table "tags" of the stack "nfc", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `uid` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `last_heard`.
    public static func nfcTags(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[NfcTagRow], SDRKitCode> {
        var rows = [sdrkit_nfc_tag_row](repeating: sdrkit_nfc_tag_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nfc_tags_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NfcTagRow.init))
    }
    /// The changed rows of the table "frames" of the stack "nfc", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `name` names a row.
    /// Time: `time_s`.
    public static func nfcFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[NfcFrameRow], SDRKitCode> {
        var rows = [sdrkit_nfc_frame_row](repeating: sdrkit_nfc_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_nfc_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(NfcFrameRow.init))
    }
    /// The changed rows of the table "networks" of the stack "wisun", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Default order: the column `level_dbfs`, highest first.
    public static func wisunNetworks(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[WisunNetworkRow], SDRKitCode> {
        var rows = [sdrkit_wisun_network_row](repeating: sdrkit_wisun_network_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wisun_networks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WisunNetworkRow.init))
    }
    /// The changed rows of the table "nodes" of the stack "wisun", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `address` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `last_heard`.
    public static func wisunNodes(_ handle: SDRKIT_HANDLE, capacity: Int = 2000) -> Result<[WisunNodeRow], SDRKitCode> {
        var rows = [sdrkit_wisun_node_row](repeating: sdrkit_wisun_node_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wisun_nodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WisunNodeRow.init))
    }
    /// The changed rows of the table "links" of the stack "wisun", newest first, at most `capacity` rows.
    /// The column `from` names a row.
    public static func wisunLinks(_ handle: SDRKIT_HANDLE, capacity: Int = 4000) -> Result<[WisunLinkRow], SDRKitCode> {
        var rows = [sdrkit_wisun_link_row](repeating: sdrkit_wisun_link_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wisun_links_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WisunLinkRow.init))
    }
    /// The changed rows of the table "frames" of the stack "wisun", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time_s`.
    public static func wisunFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 800) -> Result<[WisunFrameRow], SDRKitCode> {
        var rows = [sdrkit_wisun_frame_row](repeating: sdrkit_wisun_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wisun_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WisunFrameRow.init))
    }
    /// The changed rows of the table "networks" of the stack "thread", newest first, at most `capacity` rows.
    /// The column `network_name` names a row.
    /// Default order: the column `level_dbfs`, highest first.
    public static func threadNetworks(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[ThreadNetworkRow], SDRKitCode> {
        var rows = [sdrkit_thread_network_row](repeating: sdrkit_thread_network_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_thread_networks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ThreadNetworkRow.init))
    }
    /// The changed rows of the table "devices" of the stack "thread", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `last_heard`.
    public static func threadDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[IsmPanDeviceRow], SDRKitCode> {
        var rows = [sdrkit_ism_pan_device_row](repeating: sdrkit_ism_pan_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_thread_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsmPanDeviceRow.init))
    }
    /// The changed rows of the table "frames" of the stack "thread", newest first, at most `capacity` rows.
    /// The column `source` names a row.
    public static func threadFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[ThreadFrameRow], SDRKitCode> {
        var rows = [sdrkit_thread_frame_row](repeating: sdrkit_thread_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_thread_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ThreadFrameRow.init))
    }
    /// The changed rows of the table "frames" of the stack "mioty", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func miotyFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MiotyFrameRow], SDRKitCode> {
        var rows = [sdrkit_mioty_frame_row](repeating: sdrkit_mioty_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mioty_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MiotyFrameRow.init))
    }
    /// The changed rows of the table "devices" of the stack "mioty", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `id` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `first_heard`.
    public static func miotyDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MiotyDeviceRow], SDRKitCode> {
        var rows = [sdrkit_mioty_device_row](repeating: sdrkit_mioty_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mioty_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MiotyDeviceRow.init))
    }
    /// The changed rows of the table "packets" of the stack "dect_nrplus", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func dectNrplusPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DectNrplusNrPlusPacketRow], SDRKitCode> {
        var rows = [sdrkit_dect_nrplus_nr_plus_packet_row](repeating: sdrkit_dect_nrplus_nr_plus_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dect_nrplus_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DectNrplusNrPlusPacketRow.init))
    }
    /// The changed rows of the table "devices" of the stack "dect_nrplus", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `id` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `first_heard`.
    public static func dectNrplusDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[DectNrplusNrPlusDeviceRow], SDRKitCode> {
        var rows = [sdrkit_dect_nrplus_nr_plus_device_row](repeating: sdrkit_dect_nrplus_nr_plus_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dect_nrplus_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DectNrplusNrPlusDeviceRow.init))
    }
    /// The changed rows of the table "frames" of the stack "npr70", newest first, at most `capacity` rows.
    public static func npr70Frames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[Npr70NprFrameRow], SDRKitCode> {
        var rows = [sdrkit_npr70_npr_frame_row](repeating: sdrkit_npr70_npr_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_npr70_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Npr70NprFrameRow.init))
    }
    /// The changed rows of the table "networks" of the stack "npr70", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `network` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `first_heard`.
    public static func npr70Networks(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[Npr70NprNetworkRow], SDRKitCode> {
        var rows = [sdrkit_npr70_npr_network_row](repeating: sdrkit_npr70_npr_network_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_npr70_networks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Npr70NprNetworkRow.init))
    }
    /// The changed rows of the table "frames" of the stack "dsrc_toll", newest first, at most `capacity` rows.
    /// The column `identifier` names a row.
    public static func dsrcTollFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[DsrcTollDsrcFrameRow], SDRKitCode> {
        var rows = [sdrkit_dsrc_toll_dsrc_frame_row](repeating: sdrkit_dsrc_toll_dsrc_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dsrc_toll_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(DsrcTollDsrcFrameRow.init))
    }
    /// The changed rows of the table "devices" of the stack "enocean", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Default order: the column `last_heard`, highest first.
    /// Time: `first_heard`.
    public static func enoceanDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[EnoceanDeviceRow], SDRKitCode> {
        var rows = [sdrkit_enocean_device_row](repeating: sdrkit_enocean_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_enocean_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EnoceanDeviceRow.init))
    }
    /// The changed rows of the table "telegrams" of the stack "enocean", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `sender` names a row.
    /// Time: `utc`.
    public static func enoceanTelegrams(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[EnoceanTelegramRow], SDRKitCode> {
        var rows = [sdrkit_enocean_telegram_row](repeating: sdrkit_enocean_telegram_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_enocean_telegrams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EnoceanTelegramRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum IsmCommands {
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func ismClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ism_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func ismRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ism_pan_device_args()
        args.size = UInt32(MemoryLayout<sdrkit_ism_pan_device_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ism_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a frame. Sends one transmission of the device family (its copies of the frame, as the device sends them) on the tuned frequency, as OOK or FSK by the family. Transmissions sent during one wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func ismSend(_ handle: SDRKIT_HANDLE, family: Int32? = nil, id: Int64, command: Int64? = nil, channel: Int64? = nil, batteryOk: Bool? = nil, temperature: Double? = nil, humidity: Int64? = nil, windSpeed: Double? = nil, windGust: Double? = nil, windDir: Double? = nil, rain: Double? = nil, uv: Double? = nil, light: Double? = nil, pressure: Double? = nil, flags: Int64? = nil, current: Double? = nil, interval: Int64? = nil, power: Int64? = nil, payload: String? = nil, temperature2: Double? = nil, meterType: Int64? = nil, consumption: Int64? = nil, moisture: Int64? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ism_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_ism_send_args>.size)
        let code: Int32 = sdrkitWithString(payload ?? "") { payload_ in { () -> Int32 in
        args.has_family = family != nil
        if let given = family { args.family = given }
        args.id = id
        args.has_command = command != nil
        if let given = command { args.command = given }
        args.has_channel = channel != nil
        if let given = channel { args.channel = given }
        args.has_battery_ok = batteryOk != nil
        if let given = batteryOk { args.battery_ok = given }
        args.has_temperature = temperature != nil
        if let given = temperature { args.temperature = given }
        args.has_humidity = humidity != nil
        if let given = humidity { args.humidity = given }
        args.has_wind_speed = windSpeed != nil
        if let given = windSpeed { args.wind_speed = given }
        args.has_wind_gust = windGust != nil
        if let given = windGust { args.wind_gust = given }
        args.has_wind_dir = windDir != nil
        if let given = windDir { args.wind_dir = given }
        args.has_rain = rain != nil
        if let given = rain { args.rain = given }
        args.has_uv = uv != nil
        if let given = uv { args.uv = given }
        args.has_light = light != nil
        if let given = light { args.light = given }
        args.has_pressure = pressure != nil
        if let given = pressure { args.pressure = given }
        args.has_flags = flags != nil
        if let given = flags { args.flags = given }
        args.has_current = current != nil
        if let given = current { args.current = given }
        args.has_interval = interval != nil
        if let given = interval { args.interval = given }
        args.has_power = power != nil
        if let given = power { args.power = given }
        args.has_payload = payload != nil
        args.payload = payload_
        args.has_temperature_2 = temperature2 != nil
        if let given = temperature2 { args.temperature_2 = given }
        args.has_meter_type = meterType != nil
        if let given = meterType { args.meter_type = given }
        args.has_consumption = consumption != nil
        if let given = consumption { args.consumption = given }
        args.has_moisture = moisture != nil
        if let given = moisture { args.moisture = given }
        return sdrkit_ism_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Halt. Drops the frames waiting and ends the frame on air with the fall of the carrier.
    public static func ismHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ism_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func wmbusClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wmbus_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func wmbusRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wmbus_meter_args()
        args.size = UInt32(MemoryLayout<sdrkit_wmbus_meter_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_wmbus_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a frame. Sends a link-layer frame with the C field 0x44 (SND-NR) on the channel of the mode nearest the tuned frequency, with the CRCs of its format. Frames sent during a frame wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func wmbusSend(_ handle: SDRKIT_HANDLE, mode: Int32? = nil, manufacturer: String, address: String, version: Int64, deviceType: Int64, payload: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wmbus_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_wmbus_send_args>.size)
        let code: Int32 = sdrkitWithString(manufacturer) { manufacturer_ in sdrkitWithString(address) { address_ in sdrkitWithString(payload) { payload_ in { () -> Int32 in
        args.has_mode = mode != nil
        if let given = mode { args.mode = given }
        args.manufacturer = manufacturer_
        args.address = address_
        args.version = version
        args.device_type = deviceType
        args.payload = payload_
        return sdrkit_wmbus_send(handle, &args)
        }() } } }
        return sdrkitResult(code)
    }
    /// Halt. Drops the frames waiting and ends the frame on air with the fall of the carrier.
    public static func wmbusHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wmbus_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func zwaveClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_zwave_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func zwaveRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_zwave_node_args()
        args.size = UInt32(MemoryLayout<sdrkit_zwave_node_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_zwave_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a frame. Sends a singlecast MPDU on the first channel of the rate in the setting channels (the tuned frequency when the setting is empty). Frames sent during a frame wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func zwaveSend(_ handle: SDRKIT_HANDLE, homeId: String, source: Int64, destination: Int64, payload: String, rate: Int32? = nil, ack: Bool? = nil, sequence: Int64? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_zwave_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_zwave_send_args>.size)
        let code: Int32 = sdrkitWithString(homeId) { home_id_ in sdrkitWithString(payload) { payload_ in { () -> Int32 in
        args.home_id = home_id_
        args.source = source
        args.destination = destination
        args.payload = payload_
        args.has_rate = rate != nil
        if let given = rate { args.rate = given }
        args.has_ack = ack != nil
        if let given = ack { args.ack = given }
        args.has_sequence = sequence != nil
        if let given = sequence { args.sequence = given }
        return sdrkit_zwave_send(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Halt. Drops the frames waiting and ends the frame on air with the fall of the carrier.
    public static func zwaveHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_zwave_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func zigbeeClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_zigbee_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func zigbeeRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ism_pan_device_args()
        args.size = UInt32(MemoryLayout<sdrkit_ism_pan_device_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_zigbee_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a frame. Sends an IEEE 802.15.4 MAC frame on the tuned frequency: frame version 2003 without security, frame version 2006 with the auxiliary security header and CCM* at the security level. With both addresses the source PAN id is left out. Frames sent during a frame wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func zigbeeSend(_ handle: SDRKIT_HANDLE, panId: String, destination: String, source: String, payload: String, frameType: Int32? = nil, sequence: Int64? = nil, ack: Bool? = nil, securityLevel: Int64? = nil, key: String? = nil, frameCounter: Int64? = nil, keyIdMode: Int64? = nil, keyIndex: Int64? = nil, keySource: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_zigbee_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_zigbee_send_args>.size)
        let code: Int32 = sdrkitWithString(panId) { pan_id_ in sdrkitWithString(destination) { destination_ in sdrkitWithString(source) { source_ in sdrkitWithString(payload) { payload_ in sdrkitWithString(key ?? "") { key_ in sdrkitWithString(keySource ?? "") { key_source_ in { () -> Int32 in
        args.pan_id = pan_id_
        args.destination = destination_
        args.source = source_
        args.payload = payload_
        args.has_frame_type = frameType != nil
        if let given = frameType { args.frame_type = given }
        args.has_sequence = sequence != nil
        if let given = sequence { args.sequence = given }
        args.has_ack = ack != nil
        if let given = ack { args.ack = given }
        args.has_security_level = securityLevel != nil
        if let given = securityLevel { args.security_level = given }
        args.has_key = key != nil
        args.key = key_
        args.has_frame_counter = frameCounter != nil
        if let given = frameCounter { args.frame_counter = given }
        args.has_key_id_mode = keyIdMode != nil
        if let given = keyIdMode { args.key_id_mode = given }
        args.has_key_index = keyIndex != nil
        if let given = keyIndex { args.key_index = given }
        args.has_key_source = keySource != nil
        args.key_source = key_source_
        return sdrkit_zigbee_send(handle, &args)
        }() } } } } } }
        return sdrkitResult(code)
    }
    /// Halt. Drops the frames waiting and ends the frame on air with the fall of the carrier.
    public static func zigbeeHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_zigbee_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func sigfoxClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_sigfox_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func antClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ant_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func antRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ant_sensor_args()
        args.size = UInt32(MemoryLayout<sdrkit_ant_sensor_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ant_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a frame. Sends one ANT+ broadcast frame (the ANT+ network marker, the channel id, the data page and the CRC) on the tuned frequency. Frames sent during a frame wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func antSend(_ handle: SDRKIT_HANDLE, device: Int64, deviceType: Int64, txType: Int64, payload: String, pairing: Bool? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ant_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_ant_send_args>.size)
        let code: Int32 = sdrkitWithString(payload) { payload_ in { () -> Int32 in
        args.device = device
        args.device_type = deviceType
        args.tx_type = txType
        args.payload = payload_
        args.has_pairing = pairing != nil
        if let given = pairing { args.pairing = given }
        return sdrkit_ant_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Halt. Drops the frames waiting and ends the frame on air with the fall of the carrier.
    public static func antHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ant_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func nrf24Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_nrf24_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func nrf24Remove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_nrf24_nrf_address_args()
        args.size = UInt32(MemoryLayout<sdrkit_nrf24_nrf_address_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_nrf24_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func subghzchatClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_subghzchat_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func dectClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dect_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func dectRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dect_base_args()
        args.size = UInt32(MemoryLayout<sdrkit_dect_base_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dect_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func nfcClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_nfc_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func nfcRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_nfc_tag_args()
        args.size = UInt32(MemoryLayout<sdrkit_nfc_tag_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_nfc_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func wisunClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wisun_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func wisunRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wisun_node_args()
        args.size = UInt32(MemoryLayout<sdrkit_wisun_node_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_wisun_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a frame. Sends an IEEE 802.15.4 frame of version 2015 with the UTT-IE of the frame type, the header IEs, a header termination IE and the payload IEs, whitened with a 4-octet FCS, on the channel of the plan nearest the tuned frequency. Frames sent during a frame wait for it. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "halt" stops it.
    public static func wisunSend(_ handle: SDRKIT_HANDLE, frameType: Int32? = nil, panId: String, source: String, destination: String, payload: String, headerIes: String? = nil, sequence: Int64? = nil, kbps: Int64? = nil, ufsi: Int64? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wisun_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_wisun_send_args>.size)
        let code: Int32 = sdrkitWithString(panId) { pan_id_ in sdrkitWithString(source) { source_ in sdrkitWithString(destination) { destination_ in sdrkitWithString(payload) { payload_ in sdrkitWithString(headerIes ?? "") { header_ies_ in { () -> Int32 in
        args.has_frame_type = frameType != nil
        if let given = frameType { args.frame_type = given }
        args.pan_id = pan_id_
        args.source = source_
        args.destination = destination_
        args.payload = payload_
        args.has_header_ies = headerIes != nil
        args.header_ies = header_ies_
        args.has_sequence = sequence != nil
        if let given = sequence { args.sequence = given }
        args.has_kbps = kbps != nil
        if let given = kbps { args.kbps = given }
        args.has_ufsi = ufsi != nil
        if let given = ufsi { args.ufsi = given }
        return sdrkit_wisun_send(handle, &args)
        }() } } } } }
        return sdrkitResult(code)
    }
    /// Halt. Drops the frames waiting and ends the frame on air with the fall of the carrier.
    public static func wisunHalt(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wisun_halt(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func threadClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_thread_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func threadRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ism_pan_device_args()
        args.size = UInt32(MemoryLayout<sdrkit_ism_pan_device_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_thread_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func miotyClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mioty_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func dectNrplusClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dect_nrplus_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func npr70Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_npr70_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func dsrcTollClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dsrc_toll_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func enoceanClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_enocean_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum IsmSettings {
    /// Detection threshold.
    public static func ismThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold", value))
    }
    /// The options of the setting "submode" of the stack "ism".
    public enum IsmSubmode: Int32 {
        case devices = 0
        case pulseAnalyser = 1
    }
    /// Submode.
    public static func ismSubmode(_ handle: SDRKIT_HANDLE) -> Result<IsmSubmode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "submode", &v)
        return code == SDRKIT_OK ? .success(IsmSubmode(rawValue: v) ?? IsmSubmode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmSubmode(_ handle: SDRKIT_HANDLE, _ value: IsmSubmode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "submode", value.rawValue))
    }
    /// Longest gap inside a frame.
    public static func ismFrameGapMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "frame_gap_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmFrameGapMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "frame_gap_ms", value))
    }
    /// Fewest pulses in a frame.
    public static func ismMinPulses(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "min_pulses", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmMinPulses(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "min_pulses", value))
    }
    /// Cluster split ratio.
    public static func ismClusterRatio(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "cluster_ratio", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmClusterRatio(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "cluster_ratio", value))
    }
    /// Level.
    public static func ismTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Level.
    public static func wmbusTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWmbusTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Channels.
    public static func zwaveChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setZwaveChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Accept 0xF0 as the R3 start of frame.
    public static func zwaveSofF0R3(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "sof_f0_r3", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setZwaveSofF0R3(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "sof_f0_r3", value))
    }
    /// Level.
    public static func zwaveTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setZwaveTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Channel.
    public static func zigbeeChannel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setZigbeeChannel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "channel", value))
    }
    /// Scan channels 11 to 26.
    public static func zigbeeScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setZigbeeScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan", value))
    }
    /// Scan dwell.
    public static func zigbeeDwell(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dwell", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setZigbeeDwell(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dwell", value))
    }
    /// Keys (kept in memory only).
    public static func zigbeeKeys(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "keys", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setZigbeeKeys(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "keys", $0) })
    }
    /// Level.
    public static func zigbeeTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setZigbeeTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Other ANT networks.
    public static func antOtherNetworks(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "other_networks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAntOtherNetworks(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "other_networks", value))
    }
    /// Wheel circumference.
    public static func antWheelMm(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "wheel_mm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAntWheelMm(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "wheel_mm", value))
    }
    /// Level.
    public static func antTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAntTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Channel.
    public static func nrf24Channel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNrf24Channel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "channel", value))
    }
    /// Your chat name.
    public static func subghzchatName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSubghzchatName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "name", $0) })
    }
    /// The options of the setting "encryption" of the stack "subghzchat".
    public enum SubghzchatEncryption: Int32 {
        case off = 0
        case password = 1
        case hex = 2
    }
    /// Encryption.
    public static func subghzchatEncryption(_ handle: SDRKIT_HANDLE) -> Result<SubghzchatEncryption, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "encryption", &v)
        return code == SDRKIT_OK ? .success(SubghzchatEncryption(rawValue: v) ?? SubghzchatEncryption(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSubghzchatEncryption(_ handle: SDRKIT_HANDLE, _ value: SubghzchatEncryption) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "encryption", value.rawValue))
    }
    /// Password (kept in memory only).
    public static func subghzchatPassword(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "password", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSubghzchatPassword(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "password", $0) })
    }
    /// Hex key (kept in memory only).
    public static func subghzchatKey(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "key", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setSubghzchatKey(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "key", $0) })
    }
    /// The options of the setting "frequency" of the stack "subghzchat".
    public enum SubghzchatFrequency: Int32 {
        case n43392 = 0
        case n31500 = 1
        case n39000 = 2
        case n86835 = 3
        case n91500 = 4
    }
    /// Chat frequency.
    public static func subghzchatFrequency(_ handle: SDRKIT_HANDLE) -> Result<SubghzchatFrequency, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frequency", &v)
        return code == SDRKIT_OK ? .success(SubghzchatFrequency(rawValue: v) ?? SubghzchatFrequency(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSubghzchatFrequency(_ handle: SDRKIT_HANDLE, _ value: SubghzchatFrequency) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frequency", value.rawValue))
    }
    /// The options of the setting "band" of the stack "dect".
    public enum DectBand: Int32 {
        case eu = 0
        case us = 1
        case br = 2
        case la = 3
        case ism900 = 4
    }
    /// Band.
    public static func dectBand(_ handle: SDRKIT_HANDLE) -> Result<DectBand, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "band", &v)
        return code == SDRKIT_OK ? .success(DectBand(rawValue: v) ?? DectBand(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDectBand(_ handle: SDRKIT_HANDLE, _ value: DectBand) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "band", value.rawValue))
    }
    /// Scan the band's carriers.
    public static func dectScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDectScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan", value))
    }
    /// Scan dwell.
    public static func dectDwell(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dwell", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDectDwell(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dwell", value))
    }
    /// Usable ranges.
    public static func wisunRanges(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "ranges", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWisunRanges(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "ranges", $0) })
    }
    /// Operating classes.
    public static func wisunClasses(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "classes", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWisunClasses(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "classes", $0) })
    }
    /// Level.
    public static func wisunTxLevel(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_level", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWisunTxLevel(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_level", value))
    }
    /// Channel.
    public static func threadChannel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThreadChannel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "channel", value))
    }
    /// Scan channels 11 to 26.
    public static func threadScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThreadScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan", value))
    }
    /// Scan dwell.
    public static func threadDwell(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dwell", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThreadDwell(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dwell", value))
    }
    /// Bursts needed to try a pattern.
    public static func miotyMinBursts(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "min_bursts", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMiotyMinBursts(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "min_bursts", value))
    }
    /// Symbol rates searched.
    public static func npr70Rates(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "rates", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setNpr70Rates(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "rates", $0) })
    }
    /// Bit rate.
    public static func dsrcTollBitRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "bit_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDsrcTollBitRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "bit_rate", value))
    }
    /// Detection threshold.
    public static func enoceanThreshold(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEnoceanThreshold(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum IsmGeneratorSettings {
    /// The options of the setting "gen_scene" of the stack "ism".
    public enum IsmScene: Int32 {
        case n43392MhzSensors = 0
        case n8683MhzSensors = 1
        case zWave = 2
    }
    /// Test signal scene.
    public static func ismScene(_ handle: SDRKIT_HANDLE) -> Result<IsmScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(IsmScene(rawValue: v) ?? IsmScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmScene(_ handle: SDRKIT_HANDLE, _ value: IsmScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Strong continuous carrier.
    public static func ismCarrier(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_carrier", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmCarrier(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_carrier", value))
    }
    /// The options of the setting "gen_pattern" of the stack "ism".
    public enum IsmPattern: Int32 {
        case pwm = 0
        case ppm = 1
        case manchester = 2
    }
    /// Pulse analyser test pattern.
    public static func ismPattern(_ handle: SDRKIT_HANDLE) -> Result<IsmPattern, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_pattern", &v)
        return code == SDRKIT_OK ? .success(IsmPattern(rawValue: v) ?? IsmPattern(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmPattern(_ handle: SDRKIT_HANDLE, _ value: IsmPattern) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_pattern", value.rawValue))
    }
    /// Pulse analyser test bits.
    public static func ismBits(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_bits", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmBits(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_bits", $0) })
    }
    /// Pulse analyser test unit.
    public static func ismTUs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_t_us", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmTUs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_t_us", value))
    }
    /// Pulse analyser test PPM pulse.
    public static func ismPulseUs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_pulse_us", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmPulseUs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_pulse_us", value))
    }
    /// Pulse analyser test PPM gap of a 0.
    public static func ismGap0Us(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_gap0_us", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmGap0Us(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_gap0_us", value))
    }
    /// Pulse analyser test PPM gap of a 1.
    public static func ismGap1Us(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_gap1_us", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmGap1Us(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_gap1_us", value))
    }
    /// Pulse analyser test jitter.
    public static func ismJitter(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_jitter", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmJitter(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_jitter", value))
    }
    /// Pulse analyser test frames.
    public static func ismFrames(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_frames", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmFrames(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_frames", value))
    }
    /// Pulse analyser test gap after a frame.
    public static func ismFrameGapS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_frame_gap_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsmFrameGapS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_frame_gap_s", value))
    }
    /// The options of the setting "gen_plan" of the stack "wmbus".
    public enum WmbusPlan: Int32 {
        case n868Mhz = 0
        case n169Mhz = 1
    }
    /// Test signal plan.
    public static func wmbusPlan(_ handle: SDRKIT_HANDLE) -> Result<WmbusPlan, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_plan", &v)
        return code == SDRKIT_OK ? .success(WmbusPlan(rawValue: v) ?? WmbusPlan(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWmbusPlan(_ handle: SDRKIT_HANDLE, _ value: WmbusPlan) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_plan", value.rawValue))
    }
    /// The options of the setting "gen_phy" of the stack "wmbus".
    public enum WmbusPhy: Int32 {
        case all = 0
        case s1 = 1
        case t1 = 2
        case c1A = 3
        case c1B = 4
        case r2 = 5
        case n1 = 6
    }
    /// Test signal mode.
    public static func wmbusPhy(_ handle: SDRKIT_HANDLE) -> Result<WmbusPhy, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_phy", &v)
        return code == SDRKIT_OK ? .success(WmbusPhy(rawValue: v) ?? WmbusPhy(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWmbusPhy(_ handle: SDRKIT_HANDLE, _ value: WmbusPhy) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_phy", value.rawValue))
    }
    /// Encrypted meter.
    public static func wmbusEncrypted(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_encrypted", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWmbusEncrypted(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_encrypted", value))
    }
    /// No meters (noise only).
    public static func wmbusSilence(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_silence", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWmbusSilence(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_silence", value))
    }
    /// Test signal radio centre.
    public static func zwaveCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setZwaveCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Every frame with a bad FCS.
    public static func zigbeeBadFcs(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_bad_fcs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setZigbeeBadFcs(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_bad_fcs", value))
    }
    /// Adjacent transmitter level (0 off).
    public static func zigbeeInterfererDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_interferer_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setZigbeeInterfererDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_interferer_db", value))
    }
    /// Adjacent transmitter offset.
    public static func zigbeeInterfererHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_interferer_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setZigbeeInterfererHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_interferer_hz", value))
    }
    /// The options of the setting "gen_bit_rate" of the stack "sigfox".
    public enum SigfoxBitRate: Int32 {
        case n100BitS = 0
        case n600BitS = 1
    }
    /// Test signal bit rate.
    public static func sigfoxBitRate(_ handle: SDRKIT_HANDLE) -> Result<SigfoxBitRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_bit_rate", &v)
        return code == SDRKIT_OK ? .success(SigfoxBitRate(rawValue: v) ?? SigfoxBitRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSigfoxBitRate(_ handle: SDRKIT_HANDLE, _ value: SigfoxBitRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_bit_rate", value.rawValue))
    }
    /// The options of the setting "gen_reps" of the stack "sigfox".
    public enum SigfoxReps: Int32 {
        case n3Frames = 0
        case n1Frame = 1
    }
    /// Repetitions.
    public static func sigfoxReps(_ handle: SDRKIT_HANDLE) -> Result<SigfoxReps, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_reps", &v)
        return code == SDRKIT_OK ? .success(SigfoxReps(rawValue: v) ?? SigfoxReps(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setSigfoxReps(_ handle: SDRKIT_HANDLE, _ value: SigfoxReps) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_reps", value.rawValue))
    }
    /// Gap between messages.
    public static func sigfoxGap(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_gap", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSigfoxGap(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_gap", value))
    }
    /// The options of the setting "gen_sensors" of the stack "ant".
    public enum AntSensors: Int32 {
        case all = 0
        case strap = 1
        case none = 2
    }
    /// Test signal sensors.
    public static func antSensors(_ handle: SDRKIT_HANDLE) -> Result<AntSensors, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_sensors", &v)
        return code == SDRKIT_OK ? .success(AntSensors(rawValue: v) ?? AntSensors(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAntSensors(_ handle: SDRKIT_HANDLE, _ value: AntSensors) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_sensors", value.rawValue))
    }
    /// The options of the setting "gen_rate" of the stack "nrf24".
    public enum Nrf24Rate: Int32 {
        case n1MbitS = 0
        case n2MbitS = 1
        case n250KbitS = 2
        case all = 3
    }
    /// Test signal rate.
    public static func nrf24Rate(_ handle: SDRKIT_HANDLE) -> Result<Nrf24Rate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &v)
        return code == SDRKIT_OK ? .success(Nrf24Rate(rawValue: v) ?? Nrf24Rate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNrf24Rate(_ handle: SDRKIT_HANDLE, _ value: Nrf24Rate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate", value.rawValue))
    }
    /// The options of the setting "gen_crc" of the stack "nrf24".
    public enum Nrf24Crc: Int32 {
        case crc16 = 0
        case crc8 = 1
        case both = 2
    }
    /// Test frames.
    public static func nrf24Crc(_ handle: SDRKIT_HANDLE) -> Result<Nrf24Crc, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_crc", &v)
        return code == SDRKIT_OK ? .success(Nrf24Crc(rawValue: v) ?? Nrf24Crc(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setNrf24Crc(_ handle: SDRKIT_HANDLE, _ value: Nrf24Crc) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_crc", value.rawValue))
    }
    /// Random-bit bursts (no frame).
    public static func nrf24Random(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_random", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setNrf24Random(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_random", value))
    }
    /// The options of the setting "gen_option" of the stack "dect".
    public enum DectOption: Int32 {
        case twoBasesWithHandsets = 0
        case oneBase = 1
        case noBase = 2
    }
    /// Test signal bases.
    public static func dectOption(_ handle: SDRKIT_HANDLE) -> Result<DectOption, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_option", &v)
        return code == SDRKIT_OK ? .success(DectOption(rawValue: v) ?? DectOption(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDectOption(_ handle: SDRKIT_HANDLE, _ value: DectOption) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_option", value.rawValue))
    }
    /// Span of channels used.
    public static func wisunSpan(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_span", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWisunSpan(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_span", value))
    }
    /// Test signal radio centre.
    public static func wisunCentreHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWisunCentreHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value))
    }
    /// Test signal network name.
    public static func threadNetworkName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_network_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setThreadNetworkName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_network_name", $0) })
    }
    /// Every frame with a bad FCS.
    public static func threadBadFcs(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_bad_fcs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThreadBadFcs(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_bad_fcs", value))
    }
    /// Adjacent transmitter level (0 off).
    public static func threadInterfererDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_interferer_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThreadInterfererDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_interferer_db", value))
    }
    /// Adjacent transmitter offset.
    public static func threadInterfererHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_interferer_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setThreadInterfererHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_interferer_hz", value))
    }
    /// Test signal keying depth.
    public static func dsrcTollDepth(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_depth", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDsrcTollDepth(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_depth", value))
    }
    /// The options of the setting "gen_radio" of the stack "enocean".
    public enum EnoceanRadio: Int32 {
        case erp1 = 0
        case erp2 = 1
    }
    /// Test signal protocol.
    public static func enoceanRadio(_ handle: SDRKIT_HANDLE) -> Result<EnoceanRadio, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_radio", &v)
        return code == SDRKIT_OK ? .success(EnoceanRadio(rawValue: v) ?? EnoceanRadio(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEnoceanRadio(_ handle: SDRKIT_HANDLE, _ value: EnoceanRadio) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_radio", value.rawValue))
    }
}
