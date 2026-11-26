// sdrkit: the typed interface of the stack family "lora", version 5.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum LoraInterface {
    public static let major = 5
    public static let minor = 1
}

/// The result fields of the mesh decoder.
public struct MeshFields {
    /// Frames with a good CRC since the last reset.
    public var framesOk: Int64
    /// Frames whose CRC failed since the last reset.
    public var framesBad: Int64
    /// Frame preambles seen.
    public var preambles: Int64
    /// Headers that failed their checksum.
    public var headerFailed: Int64
    /// Meshtastic nodes.
    public var mtNodes: Int64
    /// MeshCore nodes.
    public var mcNodes: Int64
    /// Messages in the table.
    public var messages: Int64
    /// APRS stations.
    public var stations: Int64
    /// LoRaWAN devices.
    public var devices: Int64
    /// The band plan.
    public var plan: String
    /// What is searched.
    public var searching: String
    /// The dial.
    public var dial: Double
    /// The protocols searched, in words.
    public var protocols: String
    /// The custom LoRa setting is searched.
    public var custom: Bool
    /// The signal to noise ratio of the last good frame, while frames come.
    public var snrDb: Double?
    /// The carrier offset of the last good frame, while frames come.
    public var offsetHz: Double?
    /// The level of the input, once the bank reports.
    public var levelDbfs: Double?

    init(_ c: sdrkit_mesh_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        preambles = c.preambles
        headerFailed = c.header_failed
        mtNodes = c.mt_nodes
        mcNodes = c.mc_nodes
        messages = c.messages
        stations = c.stations
        devices = c.devices
        plan = sdrkitText(c.plan)
        searching = sdrkitText(c.searching)
        dial = c.dial
        protocols = sdrkitText(c.protocols)
        custom = c.custom
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MeshFields, SDRKitCode> {
        var c = sdrkit_mesh_fields()
        let code = sdrkit_mesh_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MeshFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The own Meshtastic node, once the Meshtastic client reports.
public struct MeshMtFields {
    /// The own node id.
    public var mtNodeId: String?
    /// The own public key, base64.
    public var mtPublicKey: String?
    /// The own long name.
    public var mtLongName: String?
    /// The own short name.
    public var mtShortName: String?
    /// Own packets waiting for an acknowledgement.
    public var mtInFlight: Int64?
    /// Packets of other nodes sent again.
    public var mtRebroadcasts: Int64?
    /// Warnings about the settings of the own node.
    public var mtWarnings: String?
    /// The channel frequency that the client sends on, while the session listens to a channel of the network.
    public var mtFreq: Double?

    init(_ c: sdrkit_mesh_mt_fields) {
        mtNodeId = c.has_mt_node_id ? sdrkitText(c.mt_node_id) : nil
        mtPublicKey = c.has_mt_public_key ? sdrkitText(c.mt_public_key) : nil
        mtLongName = c.has_mt_long_name ? sdrkitText(c.mt_long_name) : nil
        mtShortName = c.has_mt_short_name ? sdrkitText(c.mt_short_name) : nil
        mtInFlight = c.has_mt_in_flight ? c.mt_in_flight : nil
        mtRebroadcasts = c.has_mt_rebroadcasts ? c.mt_rebroadcasts : nil
        mtWarnings = c.has_mt_warnings ? sdrkitText(c.mt_warnings) : nil
        mtFreq = c.has_mt_freq ? c.mt_freq : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MeshMtFields, SDRKitCode> {
        var c = sdrkit_mesh_mt_fields()
        let code = sdrkit_mesh_mt_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MeshMtFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The own MeshCore node, once the MeshCore client reports.
public struct MeshMcFields {
    /// The own public key, hex.
    public var mcPublicKey: String?
    /// The own name.
    public var mcName: String?
    /// The contacts.
    public var mcContacts: Int64?
    /// The channel frequency that the client sends on, while the session listens to a channel of the network.
    public var mcFreq: Double?

    init(_ c: sdrkit_mesh_mc_fields) {
        mcPublicKey = c.has_mc_public_key ? sdrkitText(c.mc_public_key) : nil
        mcName = c.has_mc_name ? sdrkitText(c.mc_name) : nil
        mcContacts = c.has_mc_contacts ? c.mc_contacts : nil
        mcFreq = c.has_mc_freq ? c.mc_freq : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MeshMcFields, SDRKitCode> {
        var c = sdrkit_mesh_mc_fields()
        let code = sdrkit_mesh_mc_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MeshMcFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The own APRS station, once the amateur client reports.
public struct LoraHamFields {
    /// The source call of own APRS frames: the callsign of the station with the SSID. Empty without a callsign.
    public var aprsCall: String?
    /// The frequency that the client sends on, while the session listens to a channel of its link.
    public var aprsFreq: Double?

    init(_ c: sdrkit_lora_ham_fields) {
        aprsCall = c.has_aprs_call ? sdrkitText(c.aprs_call) : nil
        aprsFreq = c.has_aprs_freq ? c.aprs_freq : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LoraHamFields, SDRKitCode> {
        var c = sdrkit_lora_ham_fields()
        let code = sdrkit_lora_ham_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraHamFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The state of the transmit scheduler.
public struct MeshTxFields {
    /// Frames waiting to be sent.
    public var txPending: Int64
    /// Frames sent.
    public var txSent: Int64

    init(_ c: sdrkit_mesh_tx_fields) {
        txPending = c.tx_pending
        txSent = c.tx_sent
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<MeshTxFields, SDRKitCode> {
        var c = sdrkit_mesh_tx_fields()
        let code = sdrkit_mesh_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MeshTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.
public struct LoraGuardFields {
    /// Input samples replaced because they were not finite or far beyond full scale.
    public var badSamples: Int64

    init(_ c: sdrkit_lora_guard_fields) {
        badSamples = c.bad_samples
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LoraGuardFields, SDRKitCode> {
        var c = sdrkit_lora_guard_fields()
        let code = sdrkit_lora_guard_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraGuardFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The counts of the LoRa modulator.
public struct LoraModulateFields {
    /// Bursts written to the output.
    public var txBursts: Int64
    /// Samples written to the output.
    public var txSamples: Int64
    /// Frames that could not be modulated: a bad LoRa setting, a payload that does not fit, or a carrier outside the band.
    public var txDropped: Int64

    init(_ c: sdrkit_lora_modulate_fields) {
        txBursts = c.tx_bursts
        txSamples = c.tx_samples
        txDropped = c.tx_dropped
    }
}

/// A row of the packet list. The row key is the number of the packet.
public struct MeshPacketRow {
    public enum Net: Int32 {
        case meshtastic = 0
        case meshcore = 1
        case loraaprs = 2
        case meshcom = 3
        case lorawan = 4
        case lora = 5
        case aprs = 6
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The protocol.
    public var net: Net?
    /// The packet type, as the protocol names it.
    public var type: String?
    /// The sender.
    public var source: String?
    /// The addressee.
    public var target: String?
    /// The payload length.
    public var bytes: Int64
    /// The content, decoded or raw.
    public var content: String?
    /// The hops taken.
    public var hops: Int64?
    /// The signal to noise ratio of the frame.
    public var snrDb: Double
    /// The spreading factor.
    public var sf: Int64
    /// The bandwidth.
    public var bw: Double
    /// More about the packet.
    public var summary: String?

    init(_ c: sdrkit_mesh_packet_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        net = Net(rawValue: c.net)
        type = c.has_type ? sdrkitText(c.type) : nil
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        bytes = c.bytes
        content = c.has_content ? sdrkitText(c.content) : nil
        hops = c.has_hops ? c.hops : nil
        snrDb = c.snr_db
        sf = c.sf
        bw = c.bw
        summary = c.has_summary ? sdrkitText(c.summary) : nil
    }
}

/// A row of the node list. The row key is `<net key>/<id>`.
public struct MeshNodeRow {
    public enum Net: Int32 {
        case meshtastic = 0
        case meshcore = 1
        case loraaprs = 2
        case meshcom = 3
        case lorawan = 4
        case lora = 5
        case aprs = 6
    }
    /// The row key.
    public var key: String
    /// The protocol.
    public var net: Net?
    /// The node name.
    public var name: String
    /// The short name.
    public var short: String?
    /// The node id.
    public var id: String
    /// The hardware model.
    public var hw: String?
    /// The role.
    public var role: String?
    /// The UTC of the last packet.
    public var lastHeard: Double?
    /// The packets of the node.
    public var heard: Int64?
    /// The signal to noise ratio of the last packet.
    public var snrDb: Double?
    /// The hops of the last packet.
    public var hops: Int64?
    /// The battery charge; above 100 the node is powered from outside.
    public var batteryPct: Double?
    /// The battery voltage.
    public var voltageV: Double?
    /// The last position.
    public var position: SDRKitPosition?

    init(_ c: sdrkit_mesh_node_row) {
        key = sdrkitText(c.key)
        net = Net(rawValue: c.net)
        name = sdrkitText(c.name)
        short = c.has_short_ ? sdrkitText(c.short_) : nil
        id = sdrkitText(c.id)
        hw = c.has_hw ? sdrkitText(c.hw) : nil
        role = c.has_role ? sdrkitText(c.role) : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        hops = c.has_hops ? c.hops : nil
        batteryPct = c.has_battery_pct ? c.battery_pct : nil
        voltageV = c.has_voltage_v ? c.voltage_v : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
    }
}

/// A row of the messages. The row key is the place of the message in the log.
public struct MeshMessageRow {
    public enum Net: Int32 {
        case meshtastic = 0
        case meshcore = 1
        case loraaprs = 2
        case meshcom = 3
        case lorawan = 4
        case lora = 5
        case aprs = 6
    }
    public enum State: Int32 {
        case queued = 0
        case waiting = 1
        case sent = 2
        case ackedByMesh = 3
        case delivered = 4
        case relayed = 5
        case rejected = 6
        case failed = 7
    }
    public enum Failure: Int32 {
        case transmitOff = 0
        case notListening = 1
        case noAck = 2
        case noRoute = 3
        case gotNak = 4
        case timeout = 5
        case noInterface = 6
        case maxRetransmit = 7
        case noChannel = 8
        case tooLarge = 9
        case noResponse = 10
        case dutyCycleLimit = 11
        case badRequest = 12
        case notAuthorized = 13
        case pkiFailed = 14
        case pkiUnknownPubkey = 15
        case adminBadSessionKey = 16
        case adminPublicKeyUnauthorized = 17
        case rateLimitExceeded = 18
        case pkiSendFailPublicKey = 19
        case routingError = 20
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The protocol.
    public var net: Net?
    /// The channel.
    public var channel: String?
    /// The sender.
    public var source: String?
    /// The name of the sender.
    public var sender: String?
    /// The addressee.
    public var target: String?
    /// The text.
    public var text: String
    /// The hops taken.
    public var hops: Int64?
    /// The delivery state of an own message.
    public var state: State?
    /// Why the delivery failed.
    public var failure: Failure?
    /// The distinct relays heard for an own broadcast.
    public var relays: Int64?
    /// The packet id: the reply of `mt.send` for an own Meshtastic text, the message number of an APRS message (the reply of `aprs.send` for an own one). None for a protocol without packet ids.
    public var packetId: Int64?
    /// The UTC when the text was received or sent, if the clock of the session is known.
    public var utc: Double?
    /// The signal to noise ratio of the frame of a received text.
    public var snrDb: Double?
    /// The level of the frame of a received text, relative to the full scale of the input (not an absolute power).
    public var levelDbfs: Double?
    /// The text comes from a copy with a good CRC. False for a damaged copy that was shown after the wait without a good copy; a later good copy replaces the text and sets it. None for an own message.
    public var crcOk: Bool?
    /// The copies of the packet heard while the message is remembered (10 minutes), damaged ones included. None for an own message.
    public var copies: Int64?
    /// Why the delivery failed, as text for people: the label of `failure`.
    public var failureText: String?

    init(_ c: sdrkit_mesh_message_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        net = Net(rawValue: c.net)
        channel = c.has_channel ? sdrkitText(c.channel) : nil
        source = c.has_source ? sdrkitText(c.source) : nil
        sender = c.has_sender ? sdrkitText(c.sender) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        text = sdrkitText(c.text)
        hops = c.has_hops ? c.hops : nil
        state = c.has_state ? State(rawValue: c.state) : nil
        failure = c.has_failure ? Failure(rawValue: c.failure) : nil
        relays = c.has_relays ? c.relays : nil
        packetId = c.has_packet_id ? c.packet_id : nil
        utc = c.has_utc ? c.utc : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        crcOk = c.has_crc_ok ? c.crc_ok : nil
        copies = c.has_copies ? c.copies : nil
        failureText = c.has_failure_text ? sdrkitText(c.failure_text) : nil
    }
}

/// A row of the telemetry and position readings. The row key is the number of the reading.
public struct MeshReadingRow {
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The node of the reading.
    public var node: String
    /// The battery charge.
    public var batteryPct: Double?
    /// The battery voltage.
    public var voltageV: Double?
    /// The channel use.
    public var chUtilPct: Double?
    /// The own air time.
    public var airTxPct: Double?
    /// The temperature.
    public var tempC: Double?
    /// The relative humidity.
    public var humidityPct: Double?
    /// The barometric pressure.
    public var pressureHpa: Double?
    /// The position.
    public var position: SDRKitPosition?
    /// The altitude.
    public var altM: Double?

    init(_ c: sdrkit_mesh_reading_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        node = sdrkitText(c.node)
        batteryPct = c.has_battery_pct ? c.battery_pct : nil
        voltageV = c.has_voltage_v ? c.voltage_v : nil
        chUtilPct = c.has_ch_util_pct ? c.ch_util_pct : nil
        airTxPct = c.has_air_tx_pct ? c.air_tx_pct : nil
        tempC = c.has_temp_c ? c.temp_c : nil
        humidityPct = c.has_humidity_pct ? c.humidity_pct : nil
        pressureHpa = c.has_pressure_hpa ? c.pressure_hpa : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altM = c.has_alt_m ? c.alt_m : nil
    }
}

/// A row of the APRS stations of LoRa APRS and MeshCom. The row key is `<net key>/<call>`.
public struct MeshStationRow {
    public enum Net: Int32 {
        case meshtastic = 0
        case meshcore = 1
        case loraaprs = 2
        case meshcom = 3
        case lorawan = 4
        case lora = 5
        case aprs = 6
    }
    /// The row key.
    public var key: String
    /// The protocol.
    public var net: Net?
    /// The call or the object name.
    public var call: String
    /// The map symbol.
    public var symbol: String?
    /// The last position.
    public var position: SDRKitPosition?
    /// The type of the last packet.
    public var type: String?
    /// The UTC of the last packet.
    public var lastHeard: Double?
    /// The packets of the station.
    public var heard: Int64?
    /// The signal to noise ratio of the last packet.
    public var snrDb: Double
    /// The relay path.
    public var path: String?
    /// The comment.
    public var comment: String?
    /// The battery charge.
    public var batteryPct: Double?

    init(_ c: sdrkit_mesh_station_row) {
        key = sdrkitText(c.key)
        net = Net(rawValue: c.net)
        call = sdrkitText(c.call)
        symbol = c.has_symbol ? sdrkitText(c.symbol) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        type = c.has_type ? sdrkitText(c.type) : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
        snrDb = c.snr_db
        path = c.has_path ? sdrkitText(c.path) : nil
        comment = c.has_comment ? sdrkitText(c.comment) : nil
        batteryPct = c.has_battery_pct ? c.battery_pct : nil
    }
}

/// A row of the LoRaWAN devices. The row key is the device id.
public struct MeshDeviceRow {
    /// The row key.
    public var key: String
    /// The DevAddr or the DevEUI.
    public var id: String
    /// The network operator.
    public var network: String?
    /// The maker, or the JoinEUI.
    public var vendor: String?
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The frames of the device.
    public var heard: Int64?
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The signal to noise ratio of the last frame.
    public var snrDb: Double
    /// The last frame counter.
    public var fcnt: Int64?
    /// The frames that the counter missed.
    public var missed: Int64
    /// The spreading factor of the last frame, none for FSK.
    public var sf: Int64?
    /// The frequency of the last frame.
    public var freq: Double
    /// Uplinks.
    public var uplinks: Int64
    /// Downlinks.
    public var downlinks: Int64
    /// Join requests.
    public var joins: Int64
    /// The type of the last frame.
    public var type: String?
    /// The last FPort.
    public var fport: Int64?
    /// The last MAC commands or status.
    public var mac: String?

    init(_ c: sdrkit_mesh_device_row) {
        key = sdrkitText(c.key)
        id = sdrkitText(c.id)
        network = c.has_network ? sdrkitText(c.network) : nil
        vendor = c.has_vendor ? sdrkitText(c.vendor) : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
        levelDbfs = c.level_dbfs
        snrDb = c.snr_db
        fcnt = c.has_fcnt ? c.fcnt : nil
        missed = c.missed
        sf = c.has_sf ? c.sf : nil
        freq = c.freq
        uplinks = c.uplinks
        downlinks = c.downlinks
        joins = c.joins
        type = c.has_type ? sdrkitText(c.type) : nil
        fport = c.has_fport ? c.fport : nil
        mac = c.has_mac ? sdrkitText(c.mac) : nil
    }
}

/// A row of the packet counts by protocol and type. The row key is `<net key>/<type>`.
public struct MeshTypeRow {
    public enum Net: Int32 {
        case meshtastic = 0
        case meshcore = 1
        case loraaprs = 2
        case meshcom = 3
        case lorawan = 4
        case lora = 5
        case aprs = 6
    }
    /// The row key.
    public var key: String
    /// The protocol.
    public var net: Net?
    /// The packet type.
    public var type: String
    /// The packets of the type.
    public var count: Int64

    init(_ c: sdrkit_mesh_type_row) {
        key = sdrkitText(c.key)
        net = Net(rawValue: c.net)
        type = sdrkitText(c.type)
        count = c.count
    }
}

/// A row of the decoders: one searched LoRa setting. The row key is the entry number of the plan.
public struct MeshDecoderRow {
    public enum Net: Int32 {
        case meshtastic = 0
        case meshcore = 1
        case loraaprs = 2
        case meshcom = 3
        case lorawan = 4
        case lora = 5
        case aprs = 6
    }
    /// The row key.
    public var key: String
    /// The protocol.
    public var net: Net?
    /// The preset.
    public var preset: String
    /// The channel frequency.
    public var freq: Double
    /// The channel offset from the dial.
    public var offset: Double
    /// The spreading factor.
    public var sf: Int64
    /// The bandwidth.
    public var bw: Double
    /// The coding rate denominator: 5 is 4/5, 8 is 4/8.
    public var cr: Int64
    /// The sync word.
    public var sync: Int64
    /// The preamble length.
    public var preamble: Int64
    /// Low data rate optimisation.
    public var ldro: Bool
    /// The channel lies inside the captured band.
    public var inBand: Bool
    /// Frames decoded.
    public var frames: Int64
    /// Frames lost to errors.
    public var errors: Int64
    /// The signal to noise ratio of the last good frame.
    public var snrDb: Double?
    /// The carrier offset of the last good frame.
    public var cfo: Double?
    /// The signal time of the last frame.
    public var lastS: Double?

    init(_ c: sdrkit_mesh_decoder_row) {
        key = sdrkitText(c.key)
        net = Net(rawValue: c.net)
        preset = sdrkitText(c.preset)
        freq = c.freq
        offset = c.offset
        sf = c.sf
        bw = c.bw
        cr = c.cr
        sync = c.sync
        preamble = c.preamble
        ldro = c.ldro
        inBand = c.in_band
        frames = c.frames
        errors = c.errors
        snrDb = c.has_snr_db ? c.snr_db : nil
        cfo = c.has_cfo ? c.cfo : nil
        lastS = c.has_last_s ? c.last_s : nil
    }
}

/// A row of the MeshCore contacts. The row key is the hex of the public key.
public struct MeshContactRow {
    /// The row key.
    public var key: String
    /// The name.
    public var name: String
    /// The first 12 hex digits of the public key.
    public var keyField: String
    /// The UTC of the last packet, when the clock is known.
    public var lastHeard: Double?
    /// The signal to noise ratio of the last packet.
    public var snrDb: Double
    /// The learned path, or "flood".
    public var path: String

    init(_ c: sdrkit_mesh_contact_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        keyField = sdrkitText(c.key_)
        lastHeard = c.has_last_heard ? c.last_heard : nil
        snrDb = c.snr_db
        path = sdrkitText(c.path)
    }
}

/// A row of the channels with keys. The row key is `<net key>/<name>`. The key itself is never shown.
public struct MeshChannelRow {
    public enum Net: Int32 {
        case meshtastic = 0
        case meshcore = 1
        case loraaprs = 2
        case meshcom = 3
        case lorawan = 4
        case lora = 5
        case aprs = 6
    }
    /// The row key.
    public var key: String
    /// The protocol.
    public var net: Net?
    /// The name of the channel.
    public var channel: String
    /// The channel holds a key.
    public var keySet: Bool

    init(_ c: sdrkit_mesh_channel_row) {
        key = sdrkitText(c.key)
        net = Net(rawValue: c.net)
        channel = sdrkitText(c.channel)
        keySet = c.key_set
    }
}

/// A row of the Meshtastic frequency slots of the primary preset in the slot band. The row key is the slot number with four digits.
public struct MeshSlotRow {
    /// The row key.
    public var key: String
    /// The slot number, from 1, as the setting `mt_channel_num` counts.
    public var slot: Int64
    /// The centre of the slot plus the setting `mt_freq_offset`.
    public var freqHz: Double
    /// The slot that the stack listens on: that of `mt_channel_num`, else of the hash of `mt_channel_name`; with `mt_override_freq` the slot whose frequency it is, and no row for a frequency between slots.
    public var current: Bool
    /// The slot of the default channel name of the primary preset.
    public var presetDefault: Bool

    init(_ c: sdrkit_mesh_slot_row) {
        key = sdrkitText(c.key)
        slot = c.slot
        freqHz = c.freq_hz
        current = c.current
        presetDefault = c.preset_default
    }
}

/// The reply of `mt.send`.
public struct LoraMtSendReply {
    /// The packet id of the text: the column `packet_id` of its row in the messages and the field `packet_id` of its `mesh.text` events. The column `state` of the row says whether it was acknowledged.
    public var packetId: Int64

    init(_ c: sdrkit_lora_mt_send_reply) {
        packetId = c.packet_id
    }
}

/// The reply of `aprs.send`.
public struct AprsSendReply {
    /// The message number: the column `packet_id` of its row in the messages. The column `state` of the row says whether the addressee acknowledged it.
    public var packetId: Int64

    init(_ c: sdrkit_aprs_send_reply) {
        packetId = c.packet_id
    }
}

/// The result fields of the LoRaWAN stack of every channel.
public struct LorawanwideWideFields {
    /// The level of the input, once the bank reports.
    public var levelDbfs: Double?
    /// Frames with a good CRC.
    public var framesOk: Int64
    /// Frames that are not LoRaWAN or whose CRC failed.
    public var framesBad: Int64
    /// Frame preambles seen.
    public var preambles: Int64
    /// Headers that failed their checksum.
    public var headerFailed: Int64
    /// FSK sync words found.
    public var fskSyncs: Int64
    /// FSK frames.
    public var fskFrames: Int64
    /// FSK frames whose CRC failed.
    public var fskCrcErrors: Int64
    /// LoRa demodulators running.
    public var demodulators: Int64
    /// Channels in the plan.
    public var channelsListed: Int64
    /// Channels decoded.
    public var channelsDecoded: Int64
    /// What is decoded, in words.
    public var note: String
    /// The plan in words: its count of channels and their span.
    public var plan: String
    /// LoRaWAN devices.
    public var devices: Int64

    init(_ c: sdrkit_lorawanwide_wide_fields) {
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        preambles = c.preambles
        headerFailed = c.header_failed
        fskSyncs = c.fsk_syncs
        fskFrames = c.fsk_frames
        fskCrcErrors = c.fsk_crc_errors
        demodulators = c.demodulators
        channelsListed = c.channels_listed
        channelsDecoded = c.channels_decoded
        note = sdrkitText(c.note)
        plan = sdrkitText(c.plan)
        devices = c.devices
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LorawanwideWideFields, SDRKitCode> {
        var c = sdrkit_lorawanwide_wide_fields()
        let code = sdrkit_lorawanwide_wide_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LorawanwideWideFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the LoRaWAN frame log. The row key is the place of the frame in the log.
public struct LorawanwideWideFrameRow {
    public enum Modulation: Int32 {
        case lora = 0
        case fsk = 1
    }
    /// The row key.
    public var key: String
    /// The signal time.
    public var timeS: Double
    /// The frame type.
    public var type: String?
    /// The DevAddr or the DevEUI.
    public var device: String?
    /// The network operator.
    public var network: String?
    /// The frame counter.
    public var fcnt: Int64?
    /// The FPort.
    public var fport: Int64?
    /// The encrypted payload length of a LoRaWAN frame.
    public var payloadBytes: Int64?
    /// The FCtrl flags.
    public var fctrl: String?
    /// The MAC commands.
    public var macCommands: String?
    /// The message integrity code, hex.
    public var mic: String?
    /// How the frame was modulated.
    public var modulation: Modulation?
    /// The spreading factor, none for the FSK channel.
    public var sf: Int64?
    /// The signal to noise ratio of the frame.
    public var snrDb: Double
    /// The frequency of the frame.
    public var freq: Double
    /// More about the frame.
    public var summary: String

    init(_ c: sdrkit_lorawanwide_wide_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        type = c.has_type ? sdrkitText(c.type) : nil
        device = c.has_device ? sdrkitText(c.device) : nil
        network = c.has_network ? sdrkitText(c.network) : nil
        fcnt = c.has_fcnt ? c.fcnt : nil
        fport = c.has_fport ? c.fport : nil
        payloadBytes = c.has_payload_bytes ? c.payload_bytes : nil
        fctrl = c.has_fctrl ? sdrkitText(c.fctrl) : nil
        macCommands = c.has_mac_commands ? sdrkitText(c.mac_commands) : nil
        mic = c.has_mic ? sdrkitText(c.mic) : nil
        modulation = Modulation(rawValue: c.modulation)
        sf = c.has_sf ? c.sf : nil
        snrDb = c.snr_db
        freq = c.freq
        summary = sdrkitText(c.summary)
    }
}

/// A row of the LoRaWAN channels. The row key is the place of the channel in the plan.
public struct LorawanwideWideChannelRow {
    /// The row key.
    public var key: String
    /// The channel frequency.
    public var freq: Double
    /// The bandwidth.
    public var bw: Double
    /// The least spreading factor searched.
    public var sfMin: Int64
    /// The greatest spreading factor searched.
    public var sfMax: Int64
    /// The channel lies inside the captured band.
    public var inBand: Bool
    /// Preambles.
    public var preambles: Int64
    /// Frames decoded.
    public var frames: Int64
    /// CRC errors.
    public var crcErrors: Int64

    init(_ c: sdrkit_lorawanwide_wide_channel_row) {
        key = sdrkitText(c.key)
        freq = c.freq
        bw = c.bw
        sfMin = c.sf_min
        sfMax = c.sf_max
        inBand = c.in_band
        preambles = c.preambles
        frames = c.frames
        crcErrors = c.crc_errors
    }
}

/// The result fields of the raw LoRa stack.
public struct LorarawRawFields {
    /// The level of the input, once the bank reports.
    public var levelDbfs: Double?
    /// The carrier offset of the last frame, while frames come.
    public var offsetHz: Double?
    /// Frames with a good CRC.
    public var framesOk: Int64
    /// Frames whose CRC failed.
    public var framesBad: Int64
    /// Frame preambles seen.
    public var preambles: Int64
    /// Headers that failed their checksum.
    public var headerFailed: Int64
    /// What is searched.
    public var searching: String
    /// The dial.
    public var dial: Double
    /// Frames sent.
    public var txSent: Int64
    /// Frames at SF5.
    public var sf5: Int64
    /// Frames at SF6.
    public var sf6: Int64
    /// Frames at SF7.
    public var sf7: Int64
    /// Frames at SF8.
    public var sf8: Int64
    /// Frames at SF9.
    public var sf9: Int64
    /// Frames at SF10.
    public var sf10: Int64
    /// Frames at SF11.
    public var sf11: Int64
    /// Frames at SF12.
    public var sf12: Int64
    /// Frames at 125 kHz.
    public var bw125: Int64
    /// Frames at 250 kHz.
    public var bw250: Int64
    /// Frames at 500 kHz.
    public var bw500: Int64
    /// The signal to noise ratio of the last frame, while frames come.
    public var snrDb: Double?

    init(_ c: sdrkit_loraraw_raw_fields) {
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        preambles = c.preambles
        headerFailed = c.header_failed
        searching = sdrkitText(c.searching)
        dial = c.dial
        txSent = c.tx_sent
        sf5 = c.sf5
        sf6 = c.sf6
        sf7 = c.sf7
        sf8 = c.sf8
        sf9 = c.sf9
        sf10 = c.sf10
        sf11 = c.sf11
        sf12 = c.sf12
        bw125 = c.bw125
        bw250 = c.bw250
        bw500 = c.bw500
        snrDb = c.has_snr_db ? c.snr_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<LorarawRawFields, SDRKitCode> {
        var c = sdrkit_loraraw_raw_fields()
        let code = sdrkit_loraraw_raw_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LorarawRawFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the raw frame log. The row key is the place of the frame in the log.
public struct LorarawRawFrameRow {
    public enum Crc: Int32 {
        case ok = 0
        case bad = 1
        case none = 2
    }
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// The spreading factor.
    public var sf: Int64
    /// The bandwidth.
    public var bw: Double
    /// The coding rate denominator.
    public var cr: Int64
    /// The payload length.
    public var len: Int64
    /// What the CRC says.
    public var crc: Crc?
    /// The signal to noise ratio of the frame.
    public var snrDb: Double
    /// The level of the frame.
    public var levelDbfs: Double
    /// The carrier offset.
    public var cfo: Double
    /// The payload.
    public var payload: [UInt8]
    /// The payload as text for people.
    public var text: String

    init(_ c: sdrkit_loraraw_raw_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        sf = c.sf
        bw = c.bw
        cr = c.cr
        len = c.len
        crc = Crc(rawValue: c.crc)
        snrDb = c.snr_db
        levelDbfs = c.level_dbfs
        cfo = c.cfo
        payload = [UInt8](UnsafeBufferPointer(start: c.payload.ptr, count: c.payload.len))
        text = sdrkitText(c.text)
    }
}

/// A row of the raw decoders. The row key is the tag of the detector.
public struct LorarawRawDecoderRow {
    /// The row key.
    public var key: String
    /// The spreading factor.
    public var sf: Int64
    /// The bandwidth.
    public var bw: Double
    /// The sync words searched, in words.
    public var syncs: String
    /// The channel lies inside the captured band.
    public var inBand: Bool
    /// Preambles.
    public var preambles: Int64
    /// Frames decoded.
    public var frames: Int64
    /// CRC errors.
    public var crcErrors: Int64
    /// Headers failed.
    public var headerFailed: Int64
    /// Sync words failed.
    public var syncFailed: Int64

    init(_ c: sdrkit_loraraw_raw_decoder_row) {
        key = sdrkitText(c.key)
        sf = c.sf
        bw = c.bw
        syncs = sdrkitText(c.syncs)
        inBand = c.in_band
        preambles = c.preambles
        frames = c.frames
        crcErrors = c.crc_errors
        headerFailed = c.header_failed
        syncFailed = c.sync_failed
    }
}

/// The result fields of the APRS stack.
public struct AprsFields {
    /// Frames with a good CRC.
    public var framesOk: Int64
    /// Frames whose CRC failed.
    public var framesBad: Int64
    /// Frames at 1200 bit/s.
    public var frames1200: Int64
    /// Frames at 9600 bit/s.
    public var frames9600: Int64
    /// APRS frames, without the digipeated copies.
    public var aprsFrames: Int64
    /// Digipeated copies.
    public var copies: Int64
    /// Frames that are not AX.25.
    public var notAx25: Int64
    /// Stations, objects and items in the table.
    public var stations: Int64
    /// A carrier is present.
    public var carrier: Bool
    /// The signal to noise ratio, once the demodulator reports.
    public var snrDb: Double?
    /// The level of the input, once the demodulator reports.
    public var levelDbfs: Double?

    init(_ c: sdrkit_aprs_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        frames1200 = c.frames_1200
        frames9600 = c.frames_9600
        aprsFrames = c.aprs_frames
        copies = c.copies
        notAx25 = c.not_ax25
        stations = c.stations
        carrier = c.carrier
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AprsFields, SDRKitCode> {
        var c = sdrkit_aprs_fields()
        let code = sdrkit_aprs_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AprsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the APRS frame log. The row key is the place of the frame in the log.
public struct AprsFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// The sender.
    public var source: String
    /// The destination.
    public var target: String
    /// The digipeater path.
    public var path: String
    /// The APRS kind or the AX.25 frame type.
    public var type: String
    /// The frame in one line.
    public var text: String
    /// The bit rate.
    public var bitRate: Double

    init(_ c: sdrkit_aprs_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        path = sdrkitText(c.path)
        type = sdrkitText(c.type)
        text = sdrkitText(c.text)
        bitRate = c.bit_rate
    }
}

/// A row of the APRS stations. The row key is the callsign with its SSID, or the name of an object or item.
public struct AprsStationRow {
    /// The row key.
    public var key: String
    /// The callsign with its SSID, or the name of an object or item.
    public var call: String
    /// The map symbol.
    public var symbol: String
    /// The last position.
    public var position: SDRKitPosition?
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The frames of the station.
    public var heard: Int64?
    /// The course.
    public var courseDeg: Double?
    /// The speed.
    public var speedKt: Double?
    /// The altitude.
    public var altM: Double?
    /// The comment or status.
    public var comment: String
    /// The station that sent an object, an item or a third-party packet.
    public var via: String

    init(_ c: sdrkit_aprs_station_row) {
        key = sdrkitText(c.key)
        call = sdrkitText(c.call)
        symbol = sdrkitText(c.symbol)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        altM = c.has_alt_m ? c.alt_m : nil
        comment = sdrkitText(c.comment)
        via = sdrkitText(c.via)
    }
}

/// The result fields of the FANET stack.
public struct FanetFields {
    /// Frames with a good CRC.
    public var framesOk: Int64
    /// Frames whose CRC failed.
    public var framesBad: Int64
    /// The signal to noise ratio of the last frame, while frames come.
    public var snrDb: Double?
    /// The carrier offset of the last frame, while frames come.
    public var offsetHz: Double?
    /// The level of the input, once the bank reports.
    public var levelDbfs: Double?
    /// The channel of the plan.
    public var listening: String
    /// The stations in the table.
    public var stationsHeard: Int64

    init(_ c: sdrkit_fanet_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        listening = sdrkitText(c.listening)
        stationsHeard = c.stations_heard
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<FanetFields, SDRKitCode> {
        var c = sdrkit_fanet_fields()
        let code = sdrkit_fanet_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(FanetFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the FANET frame log. The row key is the place of the frame in the log.
public struct FanetFrameRow {
    public enum Kind: Int32 {
        case tracking = 0
        case ground = 1
        case name = 2
        case message = 3
        case service = 4
        case thermal = 5
        case other = 6
    }
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var timeS: Double
    /// What the frame is.
    public var kind: Kind?
    /// The sender.
    public var source: String
    /// The addressee of a unicast frame, else empty.
    public var target: String
    /// The frame in one line.
    public var decoded: String
    /// The position that the frame gives, if it gives one.
    public var position: SDRKitPosition?
    /// The level of the frame.
    public var levelDb: Double
    /// The signal to noise ratio of the frame.
    public var snrDb: Double

    init(_ c: sdrkit_fanet_frame_row) {
        key = sdrkitText(c.key)
        timeS = c.time_s
        kind = Kind(rawValue: c.kind)
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        decoded = sdrkitText(c.decoded)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        levelDb = c.level_db
        snrDb = c.snr_db
    }
}

/// A row of the FANET stations. The row key and `source` are the station id "MM:DDDD".
public struct FanetStationRow {
    public enum Kind: Int32 {
        case aircraft = 0
        case person = 1
        case weather = 2
        case other = 3
    }
    /// The row key.
    public var key: String
    /// The station id.
    public var source: String
    /// The name that the station sent.
    public var name: String
    /// What the station is.
    public var kind: Kind?
    /// The aircraft type, from a tracking frame.
    public var aircraft: String?
    /// The last position.
    public var position: SDRKitPosition?
    /// The altitude, from a tracking frame.
    public var altM: Double?
    /// The speed, from a tracking frame.
    public var speedKt: Double?
    /// The climb rate, from a tracking frame.
    public var climbMS: Double?
    /// The heading, from a tracking frame.
    public var courseDeg: Double?
    /// The last service frame in one line.
    public var weather: String
    /// The frames of the station.
    public var heard: Int64?
    /// The UTC of the first frame.
    public var firstHeard: Double?
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_fanet_station_row) {
        key = sdrkitText(c.key)
        source = sdrkitText(c.source)
        name = sdrkitText(c.name)
        kind = Kind(rawValue: c.kind)
        aircraft = c.has_aircraft ? sdrkitText(c.aircraft) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        altM = c.has_alt_m ? c.alt_m : nil
        speedKt = c.has_speed_kt ? c.speed_kt : nil
        climbMS = c.has_climb_m_s ? c.climb_m_s : nil
        courseDeg = c.has_course_deg ? c.course_deg : nil
        weather = sdrkitText(c.weather)
        heard = c.has_heard ? c.heard : nil
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A track point of a FANET station. The row key is the station id and the signal time of the point.
public struct FanetTrackRow {
    /// The row key.
    public var key: String
    /// The station that the point belongs to.
    public var station: String
    /// The position.
    public var position: SDRKitPosition
    /// The signal time of the point.
    public var timeS: Double

    init(_ c: sdrkit_fanet_track_row) {
        key = sdrkitText(c.key)
        station = sdrkitText(c.station)
        position = SDRKitPosition(c.position)
        timeS = c.time_s
    }
}

/// The result fields of the Reticulum stack.
public struct ReticulumRnsFields {
    /// Frames that gave a packet.
    public var framesOk: Int64
    /// Frames that gave no packet.
    public var framesBad: Int64
    /// Preambles whose sync word followed.
    public var preambles: Int64
    /// The signal to noise ratio of the last frame, while frames come.
    public var snrDb: Double?
    /// The level of the input, once the bank reports.
    public var levelDbfs: Double?
    /// The carrier offset of the last frame, while frames come.
    public var offsetHz: Double?
    /// The LoRa settings that the plan searches.
    public var searching: String
    /// The destinations in the table.
    public var destinationsHeard: Int64

    init(_ c: sdrkit_reticulum_rns_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        preambles = c.preambles
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        searching = sdrkitText(c.searching)
        destinationsHeard = c.destinations_heard
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ReticulumRnsFields, SDRKitCode> {
        var c = sdrkit_reticulum_rns_fields()
        let code = sdrkit_reticulum_rns_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ReticulumRnsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the Reticulum packet log. The row key is the place of the packet in the log.
public struct ReticulumRnsPacketRow {
    public enum TypeOption: Int32 {
        case data = 0
        case announce = 1
        case linkrequest = 2
        case proof = 3
    }
    /// The row key.
    public var key: String
    /// The UTC of the packet, when known.
    public var utc: Double?
    /// The signal time of the packet.
    public var timeS: Double
    /// The destination hash as hex.
    public var destination: String
    /// The type of the packet.
    public var type: TypeOption?
    /// The packet in one line.
    public var content: String
    /// The channel plus the carrier offset.
    public var freqHz: Double
    /// The signal to noise ratio of the frame.
    public var snrDb: Double
    /// The payload is encrypted.
    public var encrypted: Bool
    /// The first bytes of the packet.
    public var hex: [UInt8]
    /// The LoRa setting of the frame.
    public var setting: String

    init(_ c: sdrkit_reticulum_rns_packet_row) {
        key = sdrkitText(c.key)
        utc = c.has_utc ? c.utc : nil
        timeS = c.time_s
        destination = sdrkitText(c.destination)
        type = TypeOption(rawValue: c.type)
        content = sdrkitText(c.content)
        freqHz = c.freq_hz
        snrDb = c.snr_db
        encrypted = c.encrypted
        hex = [UInt8](UnsafeBufferPointer(start: c.hex.ptr, count: c.hex.len))
        setting = sdrkitText(c.setting)
    }
}

/// A row of the Reticulum destinations. The row key is the destination hash as hex.
public struct ReticulumRnsDestinationRow {
    public enum TypeOption: Int32 {
        case data = 0
        case announce = 1
        case linkrequest = 2
        case proof = 3
    }
    /// The row key.
    public var key: String
    /// The destination hash as hex.
    public var destination: String
    /// The type of the last packet.
    public var type: TypeOption?
    /// The last packet in one line.
    public var content: String
    /// The packets to the destination.
    public var packets: Int64?
    /// The identity that an announce gave, as hex.
    public var identity: String
    /// The text that an announce gave.
    public var appText: String
    /// The UTC of the first packet.
    public var firstHeard: Double?
    /// The UTC of the last packet.
    public var lastHeard: Double?

    init(_ c: sdrkit_reticulum_rns_destination_row) {
        key = sdrkitText(c.key)
        destination = sdrkitText(c.destination)
        type = TypeOption(rawValue: c.type)
        content = sdrkitText(c.content)
        packets = c.has_packets ? c.packets : nil
        identity = sdrkitText(c.identity)
        appText = sdrkitText(c.app_text)
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The result fields of the ExpressLRS stack.
public struct ElrsFields {
    /// Frames whose CRC fits a link.
    public var framesOk: Int64
    /// Frames whose CRC fits no link.
    public var framesBad: Int64
    /// Preambles whose sync word followed.
    public var preambles: Int64
    /// The binding phrase of the user's own link is set.
    public var phraseSet: Bool
    /// The hop channels inside the capture.
    public var hopChannelsInside: Int64
    /// The hop channels of the plan.
    public var hopChannels: Int64
    /// The signal to noise ratio of the last frame, while frames come.
    public var snrDb: Double?
    /// The level of the input, once the bank reports.
    public var levelDbfs: Double?
    /// The links in the table.
    public var linksHeard: Int64
    /// What the decoder cannot confirm, and what it needs.
    public var note: String

    init(_ c: sdrkit_elrs_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        preambles = c.preambles
        phraseSet = c.phrase_set
        hopChannelsInside = c.hop_channels_inside
        hopChannels = c.hop_channels
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        linksHeard = c.links_heard
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ElrsFields, SDRKitCode> {
        var c = sdrkit_elrs_fields()
        let code = sdrkit_elrs_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ElrsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the frame log. The row key is the place of the frame in the log.
public struct ElrsFrameRow {
    public enum Kind: Int32 {
        case sync = 0
        case data = 1
        case rc = 2
    }
    /// The row key.
    public var key: String
    /// The UTC of the frame, when known.
    public var utc: Double?
    /// The signal time of the frame.
    public var timeS: Double
    /// The link of the frame.
    public var link: String
    /// What the frame is.
    public var kind: Kind?
    /// The frame in one line.
    public var content: String
    /// The hop channel plus the carrier offset.
    public var freqHz: Double
    /// The index of the hop channel.
    public var hopChannel: Int64
    /// The spreading factor.
    public var sf: Int64
    /// The length of the frame.
    public var bytes: Int64
    /// The IQ was inverted.
    public var inverted: Bool
    /// The signal to noise ratio of the frame.
    public var snrDb: Double
    /// The payload is encrypted.
    public var encrypted: Bool
    /// The frame bytes as hex.
    public var hex: String

    init(_ c: sdrkit_elrs_frame_row) {
        key = sdrkitText(c.key)
        utc = c.has_utc ? c.utc : nil
        timeS = c.time_s
        link = sdrkitText(c.link)
        kind = Kind(rawValue: c.kind)
        content = sdrkitText(c.content)
        freqHz = c.freq_hz
        hopChannel = c.hop_channel
        sf = c.sf
        bytes = c.bytes
        inverted = c.inverted
        snrDb = c.snr_db
        encrypted = c.encrypted
        hex = sdrkitText(c.hex)
    }
}

/// A row of the links. The row key and `source` are the link, 4 hex digits.
public struct ElrsLinkRow {
    public enum Kind: Int32 {
        case sync = 0
        case data = 1
        case rc = 2
    }
    /// The row key.
    public var key: String
    /// The link.
    public var source: String
    /// What the last frame of the link was.
    public var kind: Kind?
    /// The last frame of the link in one line.
    public var content: String
    /// The frames of the link.
    public var frames: Int64
    /// The sync frames of the link.
    public var syncFrames: Int64
    /// The rate of the last frame.
    public var rate: String
    /// The hop index of the last sync frame, once one came.
    public var hopIndex: Int64?
    /// The hop channels on which the link was heard, in rising order.
    public var heardOn: String
    /// The UTC of the first frame.
    public var firstHeard: Double?
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_elrs_link_row) {
        key = sdrkitText(c.key)
        source = sdrkitText(c.source)
        kind = Kind(rawValue: c.kind)
        content = sdrkitText(c.content)
        frames = c.frames
        syncFrames = c.sync_frames
        rate = sdrkitText(c.rate)
        hopIndex = c.has_hop_index ? c.hop_index : nil
        heardOn = sdrkitText(c.heard_on)
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// The fields of one LoRa or FSK frame.
public struct LoraPacketEvent {
    public enum Net: Int32 {
        case meshtastic = 0
        case meshcore = 1
        case loraaprs = 2
        case meshcom = 3
        case lorawan = 4
        case lora = 5
        case aprs = 6
    }
    public enum Modulation: Int32 {
        case lora = 0
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
    /// The protocol of the frame.
    public var net: Net?
    /// How the frame was modulated.
    public var modulation: Modulation?
    /// The signal to noise ratio of the frame.
    public var snrDb: Double
    /// The spreading factor, none for FSK.
    public var sf: Int64?
    /// The payload.
    public var payload: [UInt8]
    /// The CRC is right, or the frame has none.
    public var crcOk: Bool
    /// The sender, if the protocol names one.
    public var source: String?
    /// The addressee, if the protocol names one.
    public var target: String?
    /// LoRaWAN: the device address, eight hex digits.
    public var devaddr: String?
    /// LoRaWAN: the IQ is inverted (a downlink).
    public var iqInverted: Bool?
    /// The frame has a CRC.
    public var crc: Bool?
    /// The bandwidth.
    public var bw: Double?
    /// The coding rate denominator.
    public var cr: Int64?
    /// The header is implicit.
    public var implicit: Bool?
    /// The sync word.
    public var sync: Int64?
    /// The level of the frame.
    public var levelDbfs: Double?
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_lora_packet_event) {
        time = c.time
        freqHz = c.freq_hz
        net = Net(rawValue: c.net)
        modulation = Modulation(rawValue: c.modulation)
        snrDb = c.snr_db
        sf = c.has_sf ? c.sf : nil
        payload = [UInt8](UnsafeBufferPointer(start: c.payload.ptr, count: c.payload.len))
        crcOk = c.crc_ok
        source = c.has_source ? sdrkitText(c.source) : nil
        target = c.has_target ? sdrkitText(c.target) : nil
        devaddr = c.has_devaddr ? sdrkitText(c.devaddr) : nil
        iqInverted = c.has_iq_inverted ? c.iq_inverted : nil
        crc = c.has_crc ? c.crc : nil
        bw = c.has_bw ? c.bw : nil
        cr = c.has_cr ? c.cr : nil
        implicit = c.has_implicit ? c.implicit : nil
        sync = c.has_sync ? c.sync : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<LoraPacketEvent, SDRKitCode> {
        var c = sdrkit_lora_packet_event()
        let code = sdrkit_lora_packet_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(LoraPacketEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a text message, sent or received.
public struct MeshTextEvent {
    public enum Net: Int32 {
        case meshtastic = 0
        case meshcore = 1
        case loraaprs = 2
        case meshcom = 3
        case lorawan = 4
        case lora = 5
        case aprs = 6
    }
    public enum State: Int32 {
        case queued = 0
        case waiting = 1
        case sent = 2
        case ackedByMesh = 3
        case delivered = 4
        case relayed = 5
        case rejected = 6
        case failed = 7
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
    /// The sender.
    public var source: String
    /// The addressee.
    public var target: String
    /// The channel.
    public var channel: String
    /// The text.
    public var text: String
    /// The protocol.
    public var net: Net?
    /// The name of the sender, if known.
    public var sender: String?
    /// The hops taken, if known.
    public var hops: Int64?
    /// The packet id.
    public var packetId: Int64
    /// The message that the text replies to.
    public var replyId: Int64
    /// A reaction, as a Unicode code point.
    public var emoji: Int64
    /// Sent by the own node.
    public var own: Bool
    /// The delivery state of an own message.
    public var state: State?
    /// The family of the event.
    public var family: Family?
    /// The signal to noise ratio of the frame of a received text.
    public var snrDb: Double?
    /// The level of the frame of a received text, relative to the full scale of the input (not an absolute power).
    public var levelDbfs: Double?
    /// The text comes from a copy with a good CRC. False for a damaged copy shown after the wait; a later good copy sends the text again with true. None for an own message.
    public var crcOk: Bool?

    init(_ c: sdrkit_mesh_text_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        channel = sdrkitText(c.channel)
        text = sdrkitText(c.text)
        net = Net(rawValue: c.net)
        sender = c.has_sender ? sdrkitText(c.sender) : nil
        hops = c.has_hops ? c.hops : nil
        packetId = c.packet_id
        replyId = c.reply_id
        emoji = c.emoji
        own = c.own
        state = c.has_state ? State(rawValue: c.state) : nil
        family = Family(rawValue: c.family)
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        crcOk = c.has_crc_ok ? c.crc_ok : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MeshTextEvent, SDRKitCode> {
        var c = sdrkit_mesh_text_event()
        let code = sdrkit_mesh_text_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MeshTextEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a node heard for the first time or with news.
public struct MeshNodeEvent {
    public enum Net: Int32 {
        case meshtastic = 0
        case meshcore = 1
        case loraaprs = 2
        case meshcom = 3
        case lorawan = 4
        case lora = 5
        case aprs = 6
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
    /// The node id.
    public var source: String
    /// The node name, if known.
    public var name: String?
    /// The position, if known.
    public var position: SDRKitPosition?
    /// The protocol.
    public var net: Net?
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_mesh_node_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        name = c.has_name ? sdrkitText(c.name) : nil
        position = c.has_position ? SDRKitPosition(c.position) : nil
        net = Net(rawValue: c.net)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<MeshNodeEvent, SDRKitCode> {
        var c = sdrkit_mesh_node_event()
        let code = sdrkit_mesh_node_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(MeshNodeEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a frame that the stack requests the host to send.
public struct LoraTxRequestEvent {
    public enum What: Int32 {
        case text = 0
        case retransmission = 1
        case ack = 2
        case nodeinfo = 3
        case position = 4
        case traceroute = 5
        case telemetry = 6
        case rebroadcast = 7
        case advert = 8
        case localAdvert = 9
        case beacon = 10
        case raw = 11
        case cancel = 12
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// A number for the reply.
    public var id: Int64
    /// What the frame is.
    public var what: What?
    /// The payload.
    public var payload: [UInt8]
    /// The frequency to send on.
    public var freqHzField: Double
    /// The earliest send time, signal seconds.
    public var notBeforeS: Double
    /// The time on air of a LoRa frame.
    public var airTime: Double?
    /// The own message that the frame carries.
    public var messageKey: Int64?
    /// The LoRa setting in words.
    public var setting: String?
    /// The packet speed of an AX.25 frame.
    public var afskBaud: Int64?

    init(_ c: sdrkit_lora_tx_request_event) {
        time = c.time
        freqHz = c.freq_hz
        id = c.id
        what = What(rawValue: c.what)
        payload = [UInt8](UnsafeBufferPointer(start: c.payload.ptr, count: c.payload.len))
        freqHzField = c.freq_hz_
        notBeforeS = c.not_before_s
        airTime = c.has_air_time ? c.air_time : nil
        messageKey = c.has_message_key ? c.message_key : nil
        setting = c.has_setting ? sdrkitText(c.setting) : nil
        afskBaud = c.has_afsk_baud ? c.afsk_baud : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<LoraTxRequestEvent, SDRKitCode> {
        var c = sdrkit_lora_tx_request_event()
        let code = sdrkit_lora_tx_request_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(LoraTxRequestEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of one AX.25 frame.
public struct AprsPacketEvent {
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
    /// The destination.
    public var target: String
    /// The digipeater path.
    public var path: String
    /// The information field as received.
    public var info: String
    /// The APRS kind or the AX.25 frame type.
    public var type: String
    /// The frame in one line.
    public var text: String
    /// The position that the frame gives, if it gives one.
    public var position: SDRKitPosition?
    /// The frame in TNC2 text.
    public var tnc2: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_aprs_packet_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        path = sdrkitText(c.path)
        info = sdrkitText(c.info)
        type = sdrkitText(c.type)
        text = sdrkitText(c.text)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        tnc2 = sdrkitText(c.tnc2)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<AprsPacketEvent, SDRKitCode> {
        var c = sdrkit_aprs_packet_event()
        let code = sdrkit_aprs_packet_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(AprsPacketEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a station heard for the first time.
public struct FanetStationEvent {
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
    /// The station id.
    public var source: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_fanet_station_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FanetStationEvent, SDRKitCode> {
        var c = sdrkit_fanet_station_event()
        let code = sdrkit_fanet_station_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FanetStationEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a message.
public struct FanetMessageEvent {
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
    /// The addressee of a unicast message, else empty.
    public var target: String
    /// The message.
    public var text: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_fanet_message_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        target = sdrkitText(c.target)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FanetMessageEvent, SDRKitCode> {
        var c = sdrkit_fanet_message_event()
        let code = sdrkit_fanet_message_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FanetMessageEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a ground tracker that requests help.
public struct FanetDistressEvent {
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
    /// The station id.
    public var source: String
    /// The state of the tracker in words.
    public var text: String
    /// The position of the tracker, if the frame gives one.
    public var position: SDRKitPosition?
    /// How urgent the request is.
    public var urgency: Urgency?
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_fanet_distress_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        position = c.has_position ? SDRKitPosition(c.position) : nil
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<FanetDistressEvent, SDRKitCode> {
        var c = sdrkit_fanet_distress_event()
        let code = sdrkit_fanet_distress_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(FanetDistressEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a destination heard for the first time.
public struct ReticulumDestinationEvent {
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
    /// The destination hash as hex.
    public var source: String
    /// The line of the event.
    public var text: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_reticulum_destination_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ReticulumDestinationEvent, SDRKitCode> {
        var c = sdrkit_reticulum_destination_event()
        let code = sdrkit_reticulum_destination_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ReticulumDestinationEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an announce.
public struct ReticulumAnnounceEvent {
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
    /// The destination hash as hex.
    public var source: String
    /// The identity as hex.
    public var identity: String
    /// The announced text.
    public var text: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_reticulum_announce_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        identity = sdrkitText(c.identity)
        text = sdrkitText(c.text)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ReticulumAnnounceEvent, SDRKitCode> {
        var c = sdrkit_reticulum_announce_event()
        let code = sdrkit_reticulum_announce_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ReticulumAnnounceEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a link heard for the first time.
public struct ElrsLinkEvent {
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
    /// The link, 4 hex digits.
    public var source: String
    /// The family of the event.
    public var family: Family?

    init(_ c: sdrkit_elrs_link_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ElrsLinkEvent, SDRKitCode> {
        var c = sdrkit_elrs_link_event()
        let code = sdrkit_elrs_link_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ElrsLinkEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum LoraComponents {
    /// The result fields of the component "modulate" of the stack "mesh" in the snapshot of `handle`.
    public static func meshModulateFields(_ handle: SDRKIT_HANDLE) -> Result<LoraModulateFields, SDRKitCode> {
        var c = sdrkit_lora_modulate_fields()
        let code = sdrkit_mesh_modulate_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraModulateFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "modulate" of the stack "meshtastic" in the snapshot of `handle`.
    public static func meshtasticModulateFields(_ handle: SDRKIT_HANDLE) -> Result<LoraModulateFields, SDRKitCode> {
        var c = sdrkit_lora_modulate_fields()
        let code = sdrkit_meshtastic_modulate_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraModulateFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "modulate" of the stack "meshcore" in the snapshot of `handle`.
    public static func meshcoreModulateFields(_ handle: SDRKIT_HANDLE) -> Result<LoraModulateFields, SDRKitCode> {
        var c = sdrkit_lora_modulate_fields()
        let code = sdrkit_meshcore_modulate_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraModulateFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "modulate" of the stack "lorawan" in the snapshot of `handle`.
    public static func lorawanModulateFields(_ handle: SDRKIT_HANDLE) -> Result<LoraModulateFields, SDRKitCode> {
        var c = sdrkit_lora_modulate_fields()
        let code = sdrkit_lorawan_modulate_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraModulateFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "modulate" of the stack "loraaprs" in the snapshot of `handle`.
    public static func loraaprsModulateFields(_ handle: SDRKIT_HANDLE) -> Result<LoraModulateFields, SDRKitCode> {
        var c = sdrkit_lora_modulate_fields()
        let code = sdrkit_loraaprs_modulate_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraModulateFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "modulate" of the stack "meshcom" in the snapshot of `handle`.
    public static func meshcomModulateFields(_ handle: SDRKIT_HANDLE) -> Result<LoraModulateFields, SDRKitCode> {
        var c = sdrkit_lora_modulate_fields()
        let code = sdrkit_meshcom_modulate_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraModulateFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "modulate" of the stack "loraraw" in the snapshot of `handle`.
    public static func lorarawModulateFields(_ handle: SDRKIT_HANDLE) -> Result<LoraModulateFields, SDRKitCode> {
        var c = sdrkit_lora_modulate_fields()
        let code = sdrkit_loraraw_modulate_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraModulateFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "modulate" of the stack "aprs" in the snapshot of `handle`.
    public static func aprsModulateFields(_ handle: SDRKIT_HANDLE) -> Result<LoraModulateFields, SDRKitCode> {
        var c = sdrkit_lora_modulate_fields()
        let code = sdrkit_aprs_modulate_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(LoraModulateFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum LoraTables {
    /// The changed rows of the table "packets" of the stack "mesh", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `source` names a row.
    /// Time: `time_s`.
    public static func meshPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshPacketRow], SDRKitCode> {
        var rows = [sdrkit_mesh_packet_row](repeating: sdrkit_mesh_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshPacketRow.init))
    }
    /// The changed rows of the table "nodes" of the stack "mesh", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    /// Time: `last_heard`.
    public static func meshNodes(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshNodeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_node_row](repeating: sdrkit_mesh_node_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_nodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshNodeRow.init))
    }
    /// The changed rows of the table "messages" of the stack "mesh", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `sender` names a row.
    /// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
    /// Delivery state: `state`. Failure reason: `failure_text`.
    public static func meshMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshMessageRow], SDRKitCode> {
        var rows = [sdrkit_mesh_message_row](repeating: sdrkit_mesh_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshMessageRow.init))
    }
    /// The changed rows of the table "readings" of the stack "mesh", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `node` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`.
    /// Time: `time_s`.
    public static func meshReadings(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshReadingRow], SDRKitCode> {
        var rows = [sdrkit_mesh_reading_row](repeating: sdrkit_mesh_reading_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_readings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshReadingRow.init))
    }
    /// The changed rows of the table "stations" of the stack "mesh", newest first, at most `capacity` rows.
    /// The column `call` names a row.
    /// Place of a row: the column `position`.
    public static func meshStations(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshStationRow], SDRKitCode> {
        var rows = [sdrkit_mesh_station_row](repeating: sdrkit_mesh_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshStationRow.init))
    }
    /// The changed rows of the table "devices" of the stack "mesh", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func meshDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshDeviceRow], SDRKitCode> {
        var rows = [sdrkit_mesh_device_row](repeating: sdrkit_mesh_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDeviceRow.init))
    }
    /// The changed rows of the table "types" of the stack "mesh", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func meshTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshTypeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_type_row](repeating: sdrkit_mesh_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshTypeRow.init))
    }
    /// The changed rows of the table "decoders" of the stack "mesh", newest first, at most `capacity` rows.
    /// The column `preset` names a row.
    /// Default order: the column `freq`, lowest first.
    public static func meshDecoders(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[MeshDecoderRow], SDRKitCode> {
        var rows = [sdrkit_mesh_decoder_row](repeating: sdrkit_mesh_decoder_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_decoders_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDecoderRow.init))
    }
    /// The changed rows of the table "contacts" of the stack "mesh", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func meshContacts(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshContactRow], SDRKitCode> {
        var rows = [sdrkit_mesh_contact_row](repeating: sdrkit_mesh_contact_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_contacts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshContactRow.init))
    }
    /// The changed rows of the table "channels" of the stack "mesh", newest first, at most `capacity` rows.
    /// The rows are the groups of the stack.
    /// The column `channel` names a row.
    public static func meshChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MeshChannelRow], SDRKitCode> {
        var rows = [sdrkit_mesh_channel_row](repeating: sdrkit_mesh_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshChannelRow.init))
    }
    /// The changed rows of the table "slots" of the stack "mesh", newest first, at most `capacity` rows.
    /// The column `slot` names a row.
    /// Default order: the column `slot`, lowest first.
    public static func meshSlots(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[MeshSlotRow], SDRKitCode> {
        var rows = [sdrkit_mesh_slot_row](repeating: sdrkit_mesh_slot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_mesh_slots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshSlotRow.init))
    }
    /// The changed rows of the table "packets" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `source` names a row.
    /// Time: `time_s`.
    public static func meshtasticPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshPacketRow], SDRKitCode> {
        var rows = [sdrkit_mesh_packet_row](repeating: sdrkit_mesh_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshPacketRow.init))
    }
    /// The changed rows of the table "nodes" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    /// Time: `last_heard`.
    public static func meshtasticNodes(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshNodeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_node_row](repeating: sdrkit_mesh_node_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_nodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshNodeRow.init))
    }
    /// The changed rows of the table "messages" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `sender` names a row.
    /// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
    /// Delivery state: `state`. Failure reason: `failure_text`.
    public static func meshtasticMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshMessageRow], SDRKitCode> {
        var rows = [sdrkit_mesh_message_row](repeating: sdrkit_mesh_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshMessageRow.init))
    }
    /// The changed rows of the table "readings" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `node` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`.
    /// Time: `time_s`.
    public static func meshtasticReadings(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshReadingRow], SDRKitCode> {
        var rows = [sdrkit_mesh_reading_row](repeating: sdrkit_mesh_reading_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_readings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshReadingRow.init))
    }
    /// The changed rows of the table "stations" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The column `call` names a row.
    /// Place of a row: the column `position`.
    public static func meshtasticStations(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshStationRow], SDRKitCode> {
        var rows = [sdrkit_mesh_station_row](repeating: sdrkit_mesh_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshStationRow.init))
    }
    /// The changed rows of the table "devices" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func meshtasticDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshDeviceRow], SDRKitCode> {
        var rows = [sdrkit_mesh_device_row](repeating: sdrkit_mesh_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDeviceRow.init))
    }
    /// The changed rows of the table "types" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func meshtasticTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshTypeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_type_row](repeating: sdrkit_mesh_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshTypeRow.init))
    }
    /// The changed rows of the table "decoders" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The column `preset` names a row.
    /// Default order: the column `freq`, lowest first.
    public static func meshtasticDecoders(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[MeshDecoderRow], SDRKitCode> {
        var rows = [sdrkit_mesh_decoder_row](repeating: sdrkit_mesh_decoder_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_decoders_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDecoderRow.init))
    }
    /// The changed rows of the table "contacts" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func meshtasticContacts(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshContactRow], SDRKitCode> {
        var rows = [sdrkit_mesh_contact_row](repeating: sdrkit_mesh_contact_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_contacts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshContactRow.init))
    }
    /// The changed rows of the table "channels" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The rows are the groups of the stack.
    /// The column `channel` names a row.
    public static func meshtasticChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MeshChannelRow], SDRKitCode> {
        var rows = [sdrkit_mesh_channel_row](repeating: sdrkit_mesh_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshChannelRow.init))
    }
    /// The changed rows of the table "slots" of the stack "meshtastic", newest first, at most `capacity` rows.
    /// The column `slot` names a row.
    /// Default order: the column `slot`, lowest first.
    public static func meshtasticSlots(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[MeshSlotRow], SDRKitCode> {
        var rows = [sdrkit_mesh_slot_row](repeating: sdrkit_mesh_slot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshtastic_slots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshSlotRow.init))
    }
    /// The changed rows of the table "packets" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `source` names a row.
    /// Time: `time_s`.
    public static func meshcorePackets(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshPacketRow], SDRKitCode> {
        var rows = [sdrkit_mesh_packet_row](repeating: sdrkit_mesh_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshPacketRow.init))
    }
    /// The changed rows of the table "nodes" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    /// Time: `last_heard`.
    public static func meshcoreNodes(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshNodeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_node_row](repeating: sdrkit_mesh_node_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_nodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshNodeRow.init))
    }
    /// The changed rows of the table "messages" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `sender` names a row.
    /// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
    /// Delivery state: `state`. Failure reason: `failure_text`.
    public static func meshcoreMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshMessageRow], SDRKitCode> {
        var rows = [sdrkit_mesh_message_row](repeating: sdrkit_mesh_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshMessageRow.init))
    }
    /// The changed rows of the table "readings" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `node` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`.
    /// Time: `time_s`.
    public static func meshcoreReadings(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshReadingRow], SDRKitCode> {
        var rows = [sdrkit_mesh_reading_row](repeating: sdrkit_mesh_reading_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_readings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshReadingRow.init))
    }
    /// The changed rows of the table "stations" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The column `call` names a row.
    /// Place of a row: the column `position`.
    public static func meshcoreStations(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshStationRow], SDRKitCode> {
        var rows = [sdrkit_mesh_station_row](repeating: sdrkit_mesh_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshStationRow.init))
    }
    /// The changed rows of the table "devices" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func meshcoreDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshDeviceRow], SDRKitCode> {
        var rows = [sdrkit_mesh_device_row](repeating: sdrkit_mesh_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDeviceRow.init))
    }
    /// The changed rows of the table "types" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func meshcoreTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshTypeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_type_row](repeating: sdrkit_mesh_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshTypeRow.init))
    }
    /// The changed rows of the table "decoders" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The column `preset` names a row.
    /// Default order: the column `freq`, lowest first.
    public static func meshcoreDecoders(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[MeshDecoderRow], SDRKitCode> {
        var rows = [sdrkit_mesh_decoder_row](repeating: sdrkit_mesh_decoder_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_decoders_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDecoderRow.init))
    }
    /// The changed rows of the table "contacts" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func meshcoreContacts(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshContactRow], SDRKitCode> {
        var rows = [sdrkit_mesh_contact_row](repeating: sdrkit_mesh_contact_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_contacts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshContactRow.init))
    }
    /// The changed rows of the table "channels" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The rows are the groups of the stack.
    /// The column `channel` names a row.
    public static func meshcoreChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MeshChannelRow], SDRKitCode> {
        var rows = [sdrkit_mesh_channel_row](repeating: sdrkit_mesh_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshChannelRow.init))
    }
    /// The changed rows of the table "slots" of the stack "meshcore", newest first, at most `capacity` rows.
    /// The column `slot` names a row.
    /// Default order: the column `slot`, lowest first.
    public static func meshcoreSlots(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[MeshSlotRow], SDRKitCode> {
        var rows = [sdrkit_mesh_slot_row](repeating: sdrkit_mesh_slot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcore_slots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshSlotRow.init))
    }
    /// The changed rows of the table "packets" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `source` names a row.
    /// Time: `time_s`.
    public static func lorawanPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshPacketRow], SDRKitCode> {
        var rows = [sdrkit_mesh_packet_row](repeating: sdrkit_mesh_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshPacketRow.init))
    }
    /// The changed rows of the table "nodes" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    /// Time: `last_heard`.
    public static func lorawanNodes(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshNodeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_node_row](repeating: sdrkit_mesh_node_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_nodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshNodeRow.init))
    }
    /// The changed rows of the table "messages" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `sender` names a row.
    /// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
    /// Delivery state: `state`. Failure reason: `failure_text`.
    public static func lorawanMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshMessageRow], SDRKitCode> {
        var rows = [sdrkit_mesh_message_row](repeating: sdrkit_mesh_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshMessageRow.init))
    }
    /// The changed rows of the table "readings" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `node` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`.
    /// Time: `time_s`.
    public static func lorawanReadings(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshReadingRow], SDRKitCode> {
        var rows = [sdrkit_mesh_reading_row](repeating: sdrkit_mesh_reading_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_readings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshReadingRow.init))
    }
    /// The changed rows of the table "stations" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The column `call` names a row.
    /// Place of a row: the column `position`.
    public static func lorawanStations(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshStationRow], SDRKitCode> {
        var rows = [sdrkit_mesh_station_row](repeating: sdrkit_mesh_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshStationRow.init))
    }
    /// The changed rows of the table "devices" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func lorawanDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshDeviceRow], SDRKitCode> {
        var rows = [sdrkit_mesh_device_row](repeating: sdrkit_mesh_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDeviceRow.init))
    }
    /// The changed rows of the table "types" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func lorawanTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshTypeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_type_row](repeating: sdrkit_mesh_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshTypeRow.init))
    }
    /// The changed rows of the table "decoders" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The column `preset` names a row.
    /// Default order: the column `freq`, lowest first.
    public static func lorawanDecoders(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[MeshDecoderRow], SDRKitCode> {
        var rows = [sdrkit_mesh_decoder_row](repeating: sdrkit_mesh_decoder_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_decoders_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDecoderRow.init))
    }
    /// The changed rows of the table "contacts" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func lorawanContacts(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshContactRow], SDRKitCode> {
        var rows = [sdrkit_mesh_contact_row](repeating: sdrkit_mesh_contact_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_contacts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshContactRow.init))
    }
    /// The changed rows of the table "channels" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The rows are the groups of the stack.
    /// The column `channel` names a row.
    public static func lorawanChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MeshChannelRow], SDRKitCode> {
        var rows = [sdrkit_mesh_channel_row](repeating: sdrkit_mesh_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshChannelRow.init))
    }
    /// The changed rows of the table "slots" of the stack "lorawan", newest first, at most `capacity` rows.
    /// The column `slot` names a row.
    /// Default order: the column `slot`, lowest first.
    public static func lorawanSlots(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[MeshSlotRow], SDRKitCode> {
        var rows = [sdrkit_mesh_slot_row](repeating: sdrkit_mesh_slot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawan_slots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshSlotRow.init))
    }
    /// The changed rows of the table "packets" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `source` names a row.
    /// Time: `time_s`.
    public static func loraaprsPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshPacketRow], SDRKitCode> {
        var rows = [sdrkit_mesh_packet_row](repeating: sdrkit_mesh_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshPacketRow.init))
    }
    /// The changed rows of the table "nodes" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    /// Time: `last_heard`.
    public static func loraaprsNodes(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshNodeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_node_row](repeating: sdrkit_mesh_node_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_nodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshNodeRow.init))
    }
    /// The changed rows of the table "messages" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `sender` names a row.
    /// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
    /// Delivery state: `state`. Failure reason: `failure_text`.
    public static func loraaprsMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshMessageRow], SDRKitCode> {
        var rows = [sdrkit_mesh_message_row](repeating: sdrkit_mesh_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshMessageRow.init))
    }
    /// The changed rows of the table "readings" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `node` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`.
    /// Time: `time_s`.
    public static func loraaprsReadings(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshReadingRow], SDRKitCode> {
        var rows = [sdrkit_mesh_reading_row](repeating: sdrkit_mesh_reading_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_readings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshReadingRow.init))
    }
    /// The changed rows of the table "stations" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The column `call` names a row.
    /// Place of a row: the column `position`.
    public static func loraaprsStations(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshStationRow], SDRKitCode> {
        var rows = [sdrkit_mesh_station_row](repeating: sdrkit_mesh_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshStationRow.init))
    }
    /// The changed rows of the table "devices" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func loraaprsDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshDeviceRow], SDRKitCode> {
        var rows = [sdrkit_mesh_device_row](repeating: sdrkit_mesh_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDeviceRow.init))
    }
    /// The changed rows of the table "types" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func loraaprsTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshTypeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_type_row](repeating: sdrkit_mesh_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshTypeRow.init))
    }
    /// The changed rows of the table "decoders" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The column `preset` names a row.
    /// Default order: the column `freq`, lowest first.
    public static func loraaprsDecoders(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[MeshDecoderRow], SDRKitCode> {
        var rows = [sdrkit_mesh_decoder_row](repeating: sdrkit_mesh_decoder_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_decoders_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDecoderRow.init))
    }
    /// The changed rows of the table "contacts" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func loraaprsContacts(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshContactRow], SDRKitCode> {
        var rows = [sdrkit_mesh_contact_row](repeating: sdrkit_mesh_contact_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_contacts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshContactRow.init))
    }
    /// The changed rows of the table "channels" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The rows are the groups of the stack.
    /// The column `channel` names a row.
    public static func loraaprsChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MeshChannelRow], SDRKitCode> {
        var rows = [sdrkit_mesh_channel_row](repeating: sdrkit_mesh_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshChannelRow.init))
    }
    /// The changed rows of the table "slots" of the stack "loraaprs", newest first, at most `capacity` rows.
    /// The column `slot` names a row.
    /// Default order: the column `slot`, lowest first.
    public static func loraaprsSlots(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[MeshSlotRow], SDRKitCode> {
        var rows = [sdrkit_mesh_slot_row](repeating: sdrkit_mesh_slot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraaprs_slots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshSlotRow.init))
    }
    /// The changed rows of the table "packets" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `source` names a row.
    /// Time: `time_s`.
    public static func meshcomPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshPacketRow], SDRKitCode> {
        var rows = [sdrkit_mesh_packet_row](repeating: sdrkit_mesh_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshPacketRow.init))
    }
    /// The changed rows of the table "nodes" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    /// Time: `last_heard`.
    public static func meshcomNodes(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshNodeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_node_row](repeating: sdrkit_mesh_node_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_nodes_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshNodeRow.init))
    }
    /// The changed rows of the table "messages" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `sender` names a row.
    /// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
    /// Delivery state: `state`. Failure reason: `failure_text`.
    public static func meshcomMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshMessageRow], SDRKitCode> {
        var rows = [sdrkit_mesh_message_row](repeating: sdrkit_mesh_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshMessageRow.init))
    }
    /// The changed rows of the table "readings" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `node` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`.
    /// Time: `time_s`.
    public static func meshcomReadings(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshReadingRow], SDRKitCode> {
        var rows = [sdrkit_mesh_reading_row](repeating: sdrkit_mesh_reading_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_readings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshReadingRow.init))
    }
    /// The changed rows of the table "stations" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The column `call` names a row.
    /// Place of a row: the column `position`.
    public static func meshcomStations(_ handle: SDRKIT_HANDLE, capacity: Int = 150) -> Result<[MeshStationRow], SDRKitCode> {
        var rows = [sdrkit_mesh_station_row](repeating: sdrkit_mesh_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshStationRow.init))
    }
    /// The changed rows of the table "devices" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func meshcomDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshDeviceRow], SDRKitCode> {
        var rows = [sdrkit_mesh_device_row](repeating: sdrkit_mesh_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDeviceRow.init))
    }
    /// The changed rows of the table "types" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func meshcomTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshTypeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_type_row](repeating: sdrkit_mesh_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshTypeRow.init))
    }
    /// The changed rows of the table "decoders" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The column `preset` names a row.
    /// Default order: the column `freq`, lowest first.
    public static func meshcomDecoders(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[MeshDecoderRow], SDRKitCode> {
        var rows = [sdrkit_mesh_decoder_row](repeating: sdrkit_mesh_decoder_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_decoders_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDecoderRow.init))
    }
    /// The changed rows of the table "contacts" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func meshcomContacts(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshContactRow], SDRKitCode> {
        var rows = [sdrkit_mesh_contact_row](repeating: sdrkit_mesh_contact_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_contacts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshContactRow.init))
    }
    /// The changed rows of the table "channels" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The rows are the groups of the stack.
    /// The column `channel` names a row.
    public static func meshcomChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MeshChannelRow], SDRKitCode> {
        var rows = [sdrkit_mesh_channel_row](repeating: sdrkit_mesh_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshChannelRow.init))
    }
    /// The changed rows of the table "slots" of the stack "meshcom", newest first, at most `capacity` rows.
    /// The column `slot` names a row.
    /// Default order: the column `slot`, lowest first.
    public static func meshcomSlots(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[MeshSlotRow], SDRKitCode> {
        var rows = [sdrkit_mesh_slot_row](repeating: sdrkit_mesh_slot_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_meshcom_slots_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshSlotRow.init))
    }
    /// The changed rows of the table "frames" of the stack "lorawanwide", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `device` names a row.
    /// Time: `time_s`.
    public static func lorawanwideFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[LorawanwideWideFrameRow], SDRKitCode> {
        var rows = [sdrkit_lorawanwide_wide_frame_row](repeating: sdrkit_lorawanwide_wide_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawanwide_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LorawanwideWideFrameRow.init))
    }
    /// The changed rows of the table "channels" of the stack "lorawanwide", newest first, at most `capacity` rows.
    /// Default order: the column `freq`, lowest first.
    public static func lorawanwideChannels(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[LorawanwideWideChannelRow], SDRKitCode> {
        var rows = [sdrkit_lorawanwide_wide_channel_row](repeating: sdrkit_lorawanwide_wide_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawanwide_channels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LorawanwideWideChannelRow.init))
    }
    /// The changed rows of the table "devices" of the stack "lorawanwide", newest first, at most `capacity` rows.
    /// The column `id` names a row.
    public static func lorawanwideDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshDeviceRow], SDRKitCode> {
        var rows = [sdrkit_mesh_device_row](repeating: sdrkit_mesh_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawanwide_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshDeviceRow.init))
    }
    /// The changed rows of the table "types" of the stack "lorawanwide", newest first, at most `capacity` rows.
    /// The column `type` names a row.
    public static func lorawanwideTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[MeshTypeRow], SDRKitCode> {
        var rows = [sdrkit_mesh_type_row](repeating: sdrkit_mesh_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_lorawanwide_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshTypeRow.init))
    }
    /// The changed rows of the table "frames" of the stack "loraraw", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time_s`.
    public static func lorarawFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[LorarawRawFrameRow], SDRKitCode> {
        var rows = [sdrkit_loraraw_raw_frame_row](repeating: sdrkit_loraraw_raw_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraraw_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LorarawRawFrameRow.init))
    }
    /// The changed rows of the table "decoders" of the stack "loraraw", newest first, at most `capacity` rows.
    public static func lorarawDecoders(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[LorarawRawDecoderRow], SDRKitCode> {
        var rows = [sdrkit_loraraw_raw_decoder_row](repeating: sdrkit_loraraw_raw_decoder_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_loraraw_decoders_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(LorarawRawDecoderRow.init))
    }
    /// The changed rows of the table "frames" of the stack "aprs", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `source` names a row.
    /// Sender: `source`. Addressee: `target`. Text: `text`. Time: `time_s`.
    public static func aprsFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[AprsFrameRow], SDRKitCode> {
        var rows = [sdrkit_aprs_frame_row](repeating: sdrkit_aprs_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aprs_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AprsFrameRow.init))
    }
    /// The changed rows of the table "stations" of the stack "aprs", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `call` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.
    /// Time: `last_heard`.
    public static func aprsStations(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[AprsStationRow], SDRKitCode> {
        var rows = [sdrkit_aprs_station_row](repeating: sdrkit_aprs_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aprs_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(AprsStationRow.init))
    }
    /// The changed rows of the table "messages" of the stack "aprs", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `sender` names a row.
    /// Sender: `sender`. Addressee: `target`. Text: `text`. Time: `utc`.
    /// Delivery state: `state`. Failure reason: `failure_text`.
    public static func aprsMessages(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[MeshMessageRow], SDRKitCode> {
        var rows = [sdrkit_mesh_message_row](repeating: sdrkit_mesh_message_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_aprs_messages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MeshMessageRow.init))
    }
    /// The changed rows of the table "frames" of the stack "fanet", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `source` names a row.
    /// Place of a row: the column `position`.
    /// Time: `time_s`.
    public static func fanetFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[FanetFrameRow], SDRKitCode> {
        var rows = [sdrkit_fanet_frame_row](repeating: sdrkit_fanet_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fanet_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FanetFrameRow.init))
    }
    /// The changed rows of the table "stations" of the stack "fanet", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    /// Place of a row: the column `position`.
    /// Height: `alt_m`. Course: `course_deg`. Speed: `speed_kt`.
    public static func fanetStations(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[FanetStationRow], SDRKitCode> {
        var rows = [sdrkit_fanet_station_row](repeating: sdrkit_fanet_station_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fanet_stations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FanetStationRow.init))
    }
    /// The changed rows of the table "station_tracks" of the stack "fanet", newest first, at most `capacity` rows.
    /// The column `station` names a row.
    /// Place of a row: the column `position`.
    public static func fanetStationTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 4000) -> Result<[FanetTrackRow], SDRKitCode> {
        var rows = [sdrkit_fanet_track_row](repeating: sdrkit_fanet_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_fanet_station_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(FanetTrackRow.init))
    }
    /// The changed rows of the table "packets" of the stack "reticulum", newest first, at most `capacity` rows.
    /// The rows are the messages of the stack.
    /// The column `destination` names a row.
    /// Addressee: `destination`. Text: `content`. Time: `utc`.
    public static func reticulumPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[ReticulumRnsPacketRow], SDRKitCode> {
        var rows = [sdrkit_reticulum_rns_packet_row](repeating: sdrkit_reticulum_rns_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_reticulum_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ReticulumRnsPacketRow.init))
    }
    /// The changed rows of the table "destinations" of the stack "reticulum", newest first, at most `capacity` rows.
    /// The rows are the stations or nodes of the stack.
    /// The column `destination` names a row.
    /// Time: `first_heard`.
    public static func reticulumDestinations(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[ReticulumRnsDestinationRow], SDRKitCode> {
        var rows = [sdrkit_reticulum_rns_destination_row](repeating: sdrkit_reticulum_rns_destination_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_reticulum_destinations_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ReticulumRnsDestinationRow.init))
    }
    /// The changed rows of the table "frames" of the stack "elrs", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// The column `link` names a row.
    /// Time: `utc`.
    public static func elrsFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[ElrsFrameRow], SDRKitCode> {
        var rows = [sdrkit_elrs_frame_row](repeating: sdrkit_elrs_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_elrs_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ElrsFrameRow.init))
    }
    /// The changed rows of the table "links" of the stack "elrs", newest first, at most `capacity` rows.
    /// The column `source` names a row.
    public static func elrsLinks(_ handle: SDRKIT_HANDLE, capacity: Int = 100) -> Result<[ElrsLinkRow], SDRKitCode> {
        var rows = [sdrkit_elrs_link_row](repeating: sdrkit_elrs_link_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_elrs_links_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ElrsLinkRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum LoraCommands {
    /// Send a Meshtastic text. To a node ("!xxxxxxxx") or to the channel ("^all"). The reply holds the packet id of the text; the column `state` of its row in the messages says whether it was delivered. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func meshMtSend(_ handle: SDRKIT_HANDLE, text: String, channel: String? = nil, to: String? = nil, replyId: Int64? = nil, emoji: Int64? = nil, wantAck: Int32? = nil, hopLimit: Int64? = nil) -> Result<LoraMtSendReply, SDRKitCode> {
        var reply = sdrkit_lora_mt_send_reply()
        var args = sdrkit_lora_mt_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mt_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in sdrkitWithString(channel ?? "") { channel_ in sdrkitWithString(to ?? "") { to_ in { () -> Int32 in
        args.text = text_
        args.has_channel = channel != nil
        args.channel = channel_
        args.has_to = to != nil
        args.to = to_
        args.has_reply_id = replyId != nil
        if let given = replyId { args.reply_id = given }
        args.has_emoji = emoji != nil
        if let given = emoji { args.emoji = given }
        args.has_want_ack = wantAck != nil
        if let given = wantAck { args.want_ack = given }
        args.has_hop_limit = hopLimit != nil
        if let given = hopLimit { args.hop_limit = given }
        return sdrkit_mesh_mt_send(handle, &args, &reply)
        }() } } }
        return code == SDRKIT_OK ? .success(LoraMtSendReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Send a MeshCore text.  It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func meshMcSend(_ handle: SDRKIT_HANDLE, text: String, channel: String? = nil, to: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mc_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mc_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in sdrkitWithString(channel ?? "") { channel_ in sdrkitWithString(to ?? "") { to_ in { () -> Int32 in
        args.text = text_
        args.has_channel = channel != nil
        args.channel = channel_
        args.has_to = to != nil
        args.to = to_
        return sdrkit_mesh_mc_send(handle, &args)
        }() } } }
        return sdrkitResult(code)
    }
    /// Send an APRS message. Sends one numbered message under the callsign of the station. It goes again after 30, 60, 120 and 240 s until the addressee answers, within the duty limit; after the fifth transmission without an answer it fails. The column `state` of its row turns to delivered when the addressee acknowledges it. A numbered message to the station is acknowledged once per number in 30 s. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func meshAprsSend(_ handle: SDRKIT_HANDLE, to: String, text: String) -> Result<AprsSendReply, SDRKitCode> {
        var reply = sdrkit_aprs_send_reply()
        var args = sdrkit_aprs_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_aprs_send_args>.size)
        let code: Int32 = sdrkitWithString(to) { to_ in sdrkitWithString(text) { text_ in { () -> Int32 in
        args.to = to_
        args.text = text_
        return sdrkit_mesh_aprs_send(handle, &args, &reply)
        }() } }
        return code == SDRKIT_OK ? .success(AprsSendReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Add a Meshtastic channel. The session reads the texts of the channel and can send on it. The key is kept and never shown.
    public static func meshMtChannelAdd(_ handle: SDRKIT_HANDLE, name: String, key: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mt_channel_add_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mt_channel_add_args>.size)
        let code: Int32 = sdrkitWithString(name) { name_ in sdrkitWithString(key ?? "") { key_ in { () -> Int32 in
        args.name = name_
        args.has_key = key != nil
        args.key = key_
        return sdrkit_mesh_mt_channel_add(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Add a MeshCore channel. The session reads the texts of the channel and can send on it. The key is kept and never shown.
    public static func meshMcChannelAdd(_ handle: SDRKIT_HANDLE, name: String, key: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mc_channel_add_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mc_channel_add_args>.size)
        let code: Int32 = sdrkitWithString(name) { name_ in sdrkitWithString(key ?? "") { key_ in { () -> Int32 in
        args.name = name_
        args.has_key = key != nil
        args.key = key_
        return sdrkit_mesh_mc_channel_add(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Remove a channel. 
    public static func meshChannelRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_channel_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_channel_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_mesh_channel_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a MeshCore advert.  It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func meshMcAdvert(_ handle: SDRKIT_HANDLE, scope: Int32? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mc_advert_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mc_advert_args>.size)
        let code: Int32 = { () -> Int32 in
        args.has_scope = scope != nil
        if let given = scope { args.scope = given }
        return sdrkit_mesh_mc_advert(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Add a MeshCore contact. 
    public static func meshMcContactAdd(_ handle: SDRKIT_HANDLE, key: String, name: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mc_contact_add_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mc_contact_add_args>.size)
        let code: Int32 = sdrkitWithString(key) { key_ in sdrkitWithString(name ?? "") { name_ in { () -> Int32 in
        args.key = key_
        args.has_name = name != nil
        args.name = name_
        return sdrkit_mesh_mc_contact_add(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Send an APRS beacon. The position of the station with the symbol and the comment of the settings. Without a position the comment goes as a status. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func meshAprsBeacon(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mesh_aprs_beacon(handle)
        return sdrkitResult(code)
    }
    /// Ask a Meshtastic node.  It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func meshMtRequest(_ handle: SDRKIT_HANDLE, node: String, what: Int32? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mt_request_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mt_request_args>.size)
        let code: Int32 = sdrkitWithString(node) { node_ in { () -> Int32 in
        args.node = node_
        args.has_what = what != nil
        if let given = what { args.what = given }
        return sdrkit_mesh_mt_request(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func meshClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mesh_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func meshRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_mesh_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_mesh_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_mesh_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Reset. Forgets the signal and makes the bank again.
    public static func meshReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_mesh_reset(handle)
        return sdrkitResult(code)
    }
    /// Set a Meshtastic channel slot. Sets the key and the position precision of one of the 8 channel slots. Slot 0 is the primary channel: its name comes from the setting `mt_channel_name`.
    public static func meshMtChannel(_ handle: SDRKIT_HANDLE, index: Int64? = nil, name: String, key: String? = nil, precision: Int64? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mt_channel_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mt_channel_args>.size)
        let code: Int32 = sdrkitWithString(name) { name_ in sdrkitWithString(key ?? "") { key_ in { () -> Int32 in
        args.has_index = index != nil
        if let given = index { args.index = given }
        args.name = name_
        args.has_key = key != nil
        args.key = key_
        args.has_precision = precision != nil
        if let given = precision { args.precision = given }
        return sdrkit_mesh_mt_channel(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Set the Meshtastic key. 
    public static func meshMtIdentity(_ handle: SDRKIT_HANDLE, key: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_identity_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_identity_args>.size)
        let code: Int32 = sdrkitWithString(key) { key_ in { () -> Int32 in
        args.key = key_
        return sdrkit_mesh_mt_identity(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Set the MeshCore identity. 
    public static func meshMcIdentity(_ handle: SDRKIT_HANDLE, key: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_identity_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_identity_args>.size)
        let code: Int32 = sdrkitWithString(key) { key_ in { () -> Int32 in
        args.key = key_
        return sdrkit_mesh_mc_identity(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a Meshtastic text. To a node ("!xxxxxxxx") or to the channel ("^all"). The reply holds the packet id of the text; the column `state` of its row in the messages says whether it was delivered. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func meshtasticMtSend(_ handle: SDRKIT_HANDLE, text: String, channel: String? = nil, to: String? = nil, replyId: Int64? = nil, emoji: Int64? = nil, wantAck: Int32? = nil, hopLimit: Int64? = nil) -> Result<LoraMtSendReply, SDRKitCode> {
        var reply = sdrkit_lora_mt_send_reply()
        var args = sdrkit_lora_mt_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mt_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in sdrkitWithString(channel ?? "") { channel_ in sdrkitWithString(to ?? "") { to_ in { () -> Int32 in
        args.text = text_
        args.has_channel = channel != nil
        args.channel = channel_
        args.has_to = to != nil
        args.to = to_
        args.has_reply_id = replyId != nil
        if let given = replyId { args.reply_id = given }
        args.has_emoji = emoji != nil
        if let given = emoji { args.emoji = given }
        args.has_want_ack = wantAck != nil
        if let given = wantAck { args.want_ack = given }
        args.has_hop_limit = hopLimit != nil
        if let given = hopLimit { args.hop_limit = given }
        return sdrkit_meshtastic_mt_send(handle, &args, &reply)
        }() } } }
        return code == SDRKIT_OK ? .success(LoraMtSendReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Add a Meshtastic channel. The session reads the texts of the channel and can send on it. The key is kept and never shown.
    public static func meshtasticMtChannelAdd(_ handle: SDRKIT_HANDLE, name: String, key: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mt_channel_add_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mt_channel_add_args>.size)
        let code: Int32 = sdrkitWithString(name) { name_ in sdrkitWithString(key ?? "") { key_ in { () -> Int32 in
        args.name = name_
        args.has_key = key != nil
        args.key = key_
        return sdrkit_meshtastic_mt_channel_add(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Remove a channel. 
    public static func meshtasticChannelRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_channel_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_channel_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_meshtastic_channel_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Ask a Meshtastic node.  It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func meshtasticMtRequest(_ handle: SDRKIT_HANDLE, node: String, what: Int32? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mt_request_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mt_request_args>.size)
        let code: Int32 = sdrkitWithString(node) { node_ in { () -> Int32 in
        args.node = node_
        args.has_what = what != nil
        if let given = what { args.what = given }
        return sdrkit_meshtastic_mt_request(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func meshtasticClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_meshtastic_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func meshtasticRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_mesh_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_mesh_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_meshtastic_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Reset. Forgets the signal and makes the bank again.
    public static func meshtasticReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_meshtastic_reset(handle)
        return sdrkitResult(code)
    }
    /// Set a Meshtastic channel slot. Sets the key and the position precision of one of the 8 channel slots. Slot 0 is the primary channel: its name comes from the setting `mt_channel_name`.
    public static func meshtasticMtChannel(_ handle: SDRKIT_HANDLE, index: Int64? = nil, name: String, key: String? = nil, precision: Int64? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mt_channel_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mt_channel_args>.size)
        let code: Int32 = sdrkitWithString(name) { name_ in sdrkitWithString(key ?? "") { key_ in { () -> Int32 in
        args.has_index = index != nil
        if let given = index { args.index = given }
        args.name = name_
        args.has_key = key != nil
        args.key = key_
        args.has_precision = precision != nil
        if let given = precision { args.precision = given }
        return sdrkit_meshtastic_mt_channel(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Set the Meshtastic key. 
    public static func meshtasticMtIdentity(_ handle: SDRKIT_HANDLE, key: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_identity_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_identity_args>.size)
        let code: Int32 = sdrkitWithString(key) { key_ in { () -> Int32 in
        args.key = key_
        return sdrkit_meshtastic_mt_identity(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a MeshCore text.  It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func meshcoreMcSend(_ handle: SDRKIT_HANDLE, text: String, channel: String? = nil, to: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mc_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mc_send_args>.size)
        let code: Int32 = sdrkitWithString(text) { text_ in sdrkitWithString(channel ?? "") { channel_ in sdrkitWithString(to ?? "") { to_ in { () -> Int32 in
        args.text = text_
        args.has_channel = channel != nil
        args.channel = channel_
        args.has_to = to != nil
        args.to = to_
        return sdrkit_meshcore_mc_send(handle, &args)
        }() } } }
        return sdrkitResult(code)
    }
    /// Add a MeshCore channel. The session reads the texts of the channel and can send on it. The key is kept and never shown.
    public static func meshcoreMcChannelAdd(_ handle: SDRKIT_HANDLE, name: String, key: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mc_channel_add_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mc_channel_add_args>.size)
        let code: Int32 = sdrkitWithString(name) { name_ in sdrkitWithString(key ?? "") { key_ in { () -> Int32 in
        args.name = name_
        args.has_key = key != nil
        args.key = key_
        return sdrkit_meshcore_mc_channel_add(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Remove a channel. 
    public static func meshcoreChannelRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_channel_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_channel_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_meshcore_channel_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send a MeshCore advert.  It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func meshcoreMcAdvert(_ handle: SDRKIT_HANDLE, scope: Int32? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mc_advert_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mc_advert_args>.size)
        let code: Int32 = { () -> Int32 in
        args.has_scope = scope != nil
        if let given = scope { args.scope = given }
        return sdrkit_meshcore_mc_advert(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Add a MeshCore contact. 
    public static func meshcoreMcContactAdd(_ handle: SDRKIT_HANDLE, key: String, name: String? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_mc_contact_add_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_mc_contact_add_args>.size)
        let code: Int32 = sdrkitWithString(key) { key_ in sdrkitWithString(name ?? "") { name_ in { () -> Int32 in
        args.key = key_
        args.has_name = name != nil
        args.name = name_
        return sdrkit_meshcore_mc_contact_add(handle, &args)
        }() } }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func meshcoreClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_meshcore_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func meshcoreRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_mesh_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_mesh_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_meshcore_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Reset. Forgets the signal and makes the bank again.
    public static func meshcoreReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_meshcore_reset(handle)
        return sdrkitResult(code)
    }
    /// Set the MeshCore identity. 
    public static func meshcoreMcIdentity(_ handle: SDRKIT_HANDLE, key: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_lora_identity_args()
        args.size = UInt32(MemoryLayout<sdrkit_lora_identity_args>.size)
        let code: Int32 = sdrkitWithString(key) { key_ in { () -> Int32 in
        args.key = key_
        return sdrkit_meshcore_mc_identity(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func lorawanClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_lorawan_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func lorawanRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_mesh_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_mesh_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_lorawan_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Reset. Forgets the signal and makes the bank again.
    public static func lorawanReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_lorawan_reset(handle)
        return sdrkitResult(code)
    }
    /// Send an APRS message. Sends one numbered message under the callsign of the station. It goes again after 30, 60, 120 and 240 s until the addressee answers, within the duty limit; after the fifth transmission without an answer it fails. The column `state` of its row turns to delivered when the addressee acknowledges it. A numbered message to the station is acknowledged once per number in 30 s. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func loraaprsAprsSend(_ handle: SDRKIT_HANDLE, to: String, text: String) -> Result<AprsSendReply, SDRKitCode> {
        var reply = sdrkit_aprs_send_reply()
        var args = sdrkit_aprs_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_aprs_send_args>.size)
        let code: Int32 = sdrkitWithString(to) { to_ in sdrkitWithString(text) { text_ in { () -> Int32 in
        args.to = to_
        args.text = text_
        return sdrkit_loraaprs_aprs_send(handle, &args, &reply)
        }() } }
        return code == SDRKIT_OK ? .success(AprsSendReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Send an APRS beacon. The position of the station with the symbol and the comment of the settings. Without a position the comment goes as a status. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func loraaprsAprsBeacon(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_loraaprs_aprs_beacon(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func loraaprsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_loraaprs_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func loraaprsRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_mesh_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_mesh_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_loraaprs_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Reset. Forgets the signal and makes the bank again.
    public static func loraaprsReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_loraaprs_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func meshcomClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_meshcom_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func meshcomRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_mesh_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_mesh_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_meshcom_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Reset. Forgets the signal and makes the bank again.
    public static func meshcomReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_meshcom_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func lorawanwideClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_lorawanwide_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Forgets the signal and makes the bank again.
    public static func lorawanwideReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_lorawanwide_reset(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func lorarawClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_loraraw_clear(handle)
        return sdrkitResult(code)
    }
    /// Reset. Forgets the signal and makes the bank again.
    public static func lorarawReset(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_loraraw_reset(handle)
        return sdrkitResult(code)
    }
    /// Send a frame. Sends the bytes with the LoRa setting of the stack. Transmit must be on. It transmits (frame). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func lorarawSend(_ handle: SDRKIT_HANDLE, payload: [UInt8]) -> Result<Void, SDRKitCode> {
        var args = sdrkit_loraraw_raw_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_loraraw_raw_send_args>.size)
        let code: Int32 = payload.withUnsafeBufferPointer { payload_ in { () -> Int32 in
        args.payload = sdrkit_bytes(ptr: payload_.baseAddress, len: payload_.count)
        return sdrkit_loraraw_send(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Send an APRS message. Sends one numbered message under the callsign of the station. It goes again after 30, 60, 120 and 240 s until the addressee answers, within the duty limit; after the fifth transmission without an answer it fails. The column `state` of its row turns to delivered when the addressee acknowledges it. A numbered message to the station is acknowledged once per number in 30 s. It transmits (text). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func aprsSend(_ handle: SDRKIT_HANDLE, to: String, text: String) -> Result<AprsSendReply, SDRKitCode> {
        var reply = sdrkit_aprs_send_reply()
        var args = sdrkit_aprs_send_args()
        args.size = UInt32(MemoryLayout<sdrkit_aprs_send_args>.size)
        let code: Int32 = sdrkitWithString(to) { to_ in sdrkitWithString(text) { text_ in { () -> Int32 in
        args.to = to_
        args.text = text_
        return sdrkit_aprs_send(handle, &args, &reply)
        }() } }
        return code == SDRKIT_OK ? .success(AprsSendReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Send an APRS beacon. The position of the station with the symbol and the comment of the settings. Without a position the comment goes as a status. It transmits (beacon). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.
    public static func aprsBeacon(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_aprs_beacon(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func aprsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_aprs_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func aprsRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_aprs_remove_args()
        args.size = UInt32(MemoryLayout<sdrkit_aprs_remove_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_aprs_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func fanetClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_fanet_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func reticulumClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_reticulum_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything heard so far. Favourites stay.
    public static func elrsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_elrs_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum LoraSettings {
    /// Dial.
    public static func meshDialFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dial_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshDialFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dial_freq", value))
    }
    /// Protocols.
    public static func meshProtocols(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "protocols", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshProtocols(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "protocols", value))
    }
    /// Every Meshtastic preset.
    public static func meshPresetSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "preset_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshPresetSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "preset_search", value))
    }
    /// Slot band start.
    public static func meshMtSlotStart(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_slot_start", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtSlotStart(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_slot_start", value))
    }
    /// Slot band end.
    public static func meshMtSlotEnd(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_slot_end", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtSlotEnd(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_slot_end", value))
    }
    /// Channels.
    public static func meshChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// LoRaWAN channels.
    public static func meshLwChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "lw_channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshLwChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "lw_channels", $0) })
    }
    /// The options of the setting "mt_preset" of the stack "mesh".
    public enum MeshMtPreset: Int32 {
        case longFast = 0
        case longSlow = 1
        case mediumSlow = 2
        case mediumFast = 3
        case shortSlow = 4
        case shortFast = 5
        case longModerate = 6
        case shortTurbo = 7
        case longTurbo = 8
        case edgeFastLow = 9
    }
    /// Meshtastic preset.
    public static func meshMtPreset(_ handle: SDRKIT_HANDLE) -> Result<MeshMtPreset, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "mt_preset", &v)
        return code == SDRKIT_OK ? .success(MeshMtPreset(rawValue: v) ?? MeshMtPreset(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtPreset(_ handle: SDRKIT_HANDLE, _ value: MeshMtPreset) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "mt_preset", value.rawValue))
    }
    /// Primary channel name.
    public static func meshMtChannelName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mt_channel_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtChannelName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mt_channel_name", $0) })
    }
    /// Channel number.
    public static func meshMtChannelNum(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "mt_channel_num", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtChannelNum(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "mt_channel_num", value))
    }
    /// Frequency override.
    public static func meshMtOverrideFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_override_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtOverrideFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_override_freq", value))
    }
    /// Frequency offset.
    public static func meshMtFreqOffset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_freq_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtFreqOffset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_freq_offset", value))
    }
    /// Custom LoRa setting.
    public static func meshCustom(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "custom", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshCustom(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "custom", value))
    }
    /// Frequency.
    public static func meshCustomFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "custom_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshCustomFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "custom_freq", value))
    }
    /// The options of the setting "custom_bw" of the stack "mesh".
    public enum MeshCustomBw: Int32 {
        case n78125 = 0
        case n104Khz = 1
        case n156Khz = 2
        case n208Khz = 3
        case n3125Khz = 4
        case n417Khz = 5
        case n625Khz = 6
        case n125Khz = 7
        case n250Khz = 8
        case n500Khz = 9
        case n8125Khz = 10
        case n1625Khz = 11
    }
    /// Bandwidth.
    public static func meshCustomBw(_ handle: SDRKIT_HANDLE) -> Result<MeshCustomBw, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &v)
        return code == SDRKIT_OK ? .success(MeshCustomBw(rawValue: v) ?? MeshCustomBw(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshCustomBw(_ handle: SDRKIT_HANDLE, _ value: MeshCustomBw) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_bw", value.rawValue))
    }
    /// Spreading factor.
    public static func meshCustomSf(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshCustomSf(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sf", value))
    }
    /// The options of the setting "custom_cr" of the stack "mesh".
    public enum MeshCustomCr: Int32 {
        case n45 = 0
        case n46 = 1
        case n47 = 2
        case n48 = 3
    }
    /// Coding rate.
    public static func meshCustomCr(_ handle: SDRKIT_HANDLE) -> Result<MeshCustomCr, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &v)
        return code == SDRKIT_OK ? .success(MeshCustomCr(rawValue: v) ?? MeshCustomCr(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshCustomCr(_ handle: SDRKIT_HANDLE, _ value: MeshCustomCr) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_cr", value.rawValue))
    }
    /// Sync word.
    public static func meshCustomSync(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sync", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshCustomSync(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sync", value))
    }
    /// Preamble.
    public static func meshCustomPreamble(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshCustomPreamble(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_preamble", value))
    }
    /// Lost-sample search.
    public static func meshJumpSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "jump_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshJumpSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "jump_search", value))
    }
    /// Expected jump.
    public static func meshJumpChips(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "jump_chips", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshJumpChips(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "jump_chips", value))
    }
    /// Raw payload.
    public static func meshRaw(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "raw", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshRaw(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "raw", value))
    }
    /// Computer clock at signal 0.
    public static func meshClock0(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshClock0(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock0", value))
    }
    /// Node ID.
    public static func meshMtNodeId(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mt_node_id", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtNodeId(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mt_node_id", $0) })
    }
    /// Long name.
    public static func meshMtLongName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mt_long_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtLongName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mt_long_name", $0) })
    }
    /// Short name.
    public static func meshMtShortName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mt_short_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtShortName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mt_short_name", $0) })
    }
    /// The options of the setting "mt_role" of the stack "mesh".
    public enum MeshMtRole: Int32 {
        case cLIENT = 0
        case cLIENTMUTE = 1
        case cLIENTHIDDEN = 2
    }
    /// Role.
    public static func meshMtRole(_ handle: SDRKIT_HANDLE) -> Result<MeshMtRole, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "mt_role", &v)
        return code == SDRKIT_OK ? .success(MeshMtRole(rawValue: v) ?? MeshMtRole(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtRole(_ handle: SDRKIT_HANDLE, _ value: MeshMtRole) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "mt_role", value.rawValue))
    }
    /// Licensed (ham) mode.
    public static func meshMtLicensed(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mt_licensed", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtLicensed(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mt_licensed", value))
    }
    /// Callsign prefix.
    public static func meshMtCallsignPrefix(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mt_callsign_prefix", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtCallsignPrefix(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mt_callsign_prefix", value))
    }
    /// Hop limit.
    public static func meshMtHopLimit(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "mt_hop_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtHopLimit(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "mt_hop_limit", value))
    }
    /// NodeInfo interval.
    public static func meshMtNodeinfoInterval(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_nodeinfo_interval", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtNodeinfoInterval(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_nodeinfo_interval", value))
    }
    /// The options of the setting "mt_position_mode" of the stack "mesh".
    public enum MeshMtPositionMode: Int32 {
        case off = 0
        case station = 1
    }
    /// Position.
    public static func meshMtPositionMode(_ handle: SDRKIT_HANDLE) -> Result<MeshMtPositionMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "mt_position_mode", &v)
        return code == SDRKIT_OK ? .success(MeshMtPositionMode(rawValue: v) ?? MeshMtPositionMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtPositionMode(_ handle: SDRKIT_HANDLE, _ value: MeshMtPositionMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "mt_position_mode", value.rawValue))
    }
    /// Position interval.
    public static func meshMtPositionInterval(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_position_interval", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtPositionInterval(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_position_interval", value))
    }
    /// Want-ack on channel texts.
    public static func meshMtAckBroadcast(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mt_ack_broadcast", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMtAckBroadcast(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mt_ack_broadcast", value))
    }
    /// Name.
    public static func meshMcName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mc_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMcName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mc_name", $0) })
    }
    /// Send the station's position.
    public static func meshMcShareLocation(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mc_share_location", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMcShareLocation(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mc_share_location", value))
    }
    /// Advert interval.
    public static func meshMcAdvertInterval(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mc_advert_interval", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshMcAdvertInterval(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mc_advert_interval", value))
    }
    /// SSID.
    public static func meshAprsSsid(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "aprs_ssid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshAprsSsid(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "aprs_ssid", value))
    }
    /// Destination.
    public static func meshAprsTocall(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_tocall", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshAprsTocall(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_tocall", $0) })
    }
    /// Path.
    public static func meshAprsPath(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_path", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshAprsPath(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_path", $0) })
    }
    /// Symbol.
    public static func meshAprsSymbol(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_symbol", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshAprsSymbol(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_symbol", $0) })
    }
    /// Comment.
    public static func meshAprsComment(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_comment", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshAprsComment(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_comment", $0) })
    }
    /// Duty-cycle limit.
    public static func meshDutyLimit(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "duty_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshDutyLimit(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "duty_limit", value))
    }
    /// Duty-cycle bands.
    public static func meshDutyBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "duty_bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshDutyBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "duty_bands", $0) })
    }
    /// Dial.
    public static func meshtasticDialFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dial_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticDialFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dial_freq", value))
    }
    /// Every Meshtastic preset.
    public static func meshtasticPresetSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "preset_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticPresetSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "preset_search", value))
    }
    /// Slot band start.
    public static func meshtasticMtSlotStart(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_slot_start", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtSlotStart(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_slot_start", value))
    }
    /// Slot band end.
    public static func meshtasticMtSlotEnd(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_slot_end", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtSlotEnd(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_slot_end", value))
    }
    /// The options of the setting "mt_preset" of the stack "meshtastic".
    public enum MeshtasticMtPreset: Int32 {
        case longFast = 0
        case longSlow = 1
        case mediumSlow = 2
        case mediumFast = 3
        case shortSlow = 4
        case shortFast = 5
        case longModerate = 6
        case shortTurbo = 7
        case longTurbo = 8
        case edgeFastLow = 9
    }
    /// Meshtastic preset.
    public static func meshtasticMtPreset(_ handle: SDRKIT_HANDLE) -> Result<MeshtasticMtPreset, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "mt_preset", &v)
        return code == SDRKIT_OK ? .success(MeshtasticMtPreset(rawValue: v) ?? MeshtasticMtPreset(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtPreset(_ handle: SDRKIT_HANDLE, _ value: MeshtasticMtPreset) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "mt_preset", value.rawValue))
    }
    /// Primary channel name.
    public static func meshtasticMtChannelName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mt_channel_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtChannelName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mt_channel_name", $0) })
    }
    /// Channel number.
    public static func meshtasticMtChannelNum(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "mt_channel_num", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtChannelNum(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "mt_channel_num", value))
    }
    /// Frequency override.
    public static func meshtasticMtOverrideFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_override_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtOverrideFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_override_freq", value))
    }
    /// Frequency offset.
    public static func meshtasticMtFreqOffset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_freq_offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtFreqOffset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_freq_offset", value))
    }
    /// Custom LoRa setting.
    public static func meshtasticCustom(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "custom", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticCustom(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "custom", value))
    }
    /// Frequency.
    public static func meshtasticCustomFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "custom_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticCustomFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "custom_freq", value))
    }
    /// The options of the setting "custom_bw" of the stack "meshtastic".
    public enum MeshtasticCustomBw: Int32 {
        case n78125 = 0
        case n104Khz = 1
        case n156Khz = 2
        case n208Khz = 3
        case n3125Khz = 4
        case n417Khz = 5
        case n625Khz = 6
        case n125Khz = 7
        case n250Khz = 8
        case n500Khz = 9
        case n8125Khz = 10
        case n1625Khz = 11
    }
    /// Bandwidth.
    public static func meshtasticCustomBw(_ handle: SDRKIT_HANDLE) -> Result<MeshtasticCustomBw, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &v)
        return code == SDRKIT_OK ? .success(MeshtasticCustomBw(rawValue: v) ?? MeshtasticCustomBw(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticCustomBw(_ handle: SDRKIT_HANDLE, _ value: MeshtasticCustomBw) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_bw", value.rawValue))
    }
    /// Spreading factor.
    public static func meshtasticCustomSf(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticCustomSf(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sf", value))
    }
    /// The options of the setting "custom_cr" of the stack "meshtastic".
    public enum MeshtasticCustomCr: Int32 {
        case n45 = 0
        case n46 = 1
        case n47 = 2
        case n48 = 3
    }
    /// Coding rate.
    public static func meshtasticCustomCr(_ handle: SDRKIT_HANDLE) -> Result<MeshtasticCustomCr, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &v)
        return code == SDRKIT_OK ? .success(MeshtasticCustomCr(rawValue: v) ?? MeshtasticCustomCr(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticCustomCr(_ handle: SDRKIT_HANDLE, _ value: MeshtasticCustomCr) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_cr", value.rawValue))
    }
    /// Sync word.
    public static func meshtasticCustomSync(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sync", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticCustomSync(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sync", value))
    }
    /// Preamble.
    public static func meshtasticCustomPreamble(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticCustomPreamble(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_preamble", value))
    }
    /// Lost-sample search.
    public static func meshtasticJumpSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "jump_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticJumpSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "jump_search", value))
    }
    /// Expected jump.
    public static func meshtasticJumpChips(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "jump_chips", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticJumpChips(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "jump_chips", value))
    }
    /// Raw payload.
    public static func meshtasticRaw(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "raw", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticRaw(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "raw", value))
    }
    /// Computer clock at signal 0.
    public static func meshtasticClock0(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticClock0(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock0", value))
    }
    /// Node ID.
    public static func meshtasticMtNodeId(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mt_node_id", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtNodeId(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mt_node_id", $0) })
    }
    /// Long name.
    public static func meshtasticMtLongName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mt_long_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtLongName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mt_long_name", $0) })
    }
    /// Short name.
    public static func meshtasticMtShortName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mt_short_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtShortName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mt_short_name", $0) })
    }
    /// The options of the setting "mt_role" of the stack "meshtastic".
    public enum MeshtasticMtRole: Int32 {
        case cLIENT = 0
        case cLIENTMUTE = 1
        case cLIENTHIDDEN = 2
    }
    /// Role.
    public static func meshtasticMtRole(_ handle: SDRKIT_HANDLE) -> Result<MeshtasticMtRole, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "mt_role", &v)
        return code == SDRKIT_OK ? .success(MeshtasticMtRole(rawValue: v) ?? MeshtasticMtRole(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtRole(_ handle: SDRKIT_HANDLE, _ value: MeshtasticMtRole) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "mt_role", value.rawValue))
    }
    /// Licensed (ham) mode.
    public static func meshtasticMtLicensed(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mt_licensed", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtLicensed(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mt_licensed", value))
    }
    /// Callsign prefix.
    public static func meshtasticMtCallsignPrefix(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mt_callsign_prefix", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtCallsignPrefix(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mt_callsign_prefix", value))
    }
    /// Hop limit.
    public static func meshtasticMtHopLimit(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "mt_hop_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtHopLimit(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "mt_hop_limit", value))
    }
    /// NodeInfo interval.
    public static func meshtasticMtNodeinfoInterval(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_nodeinfo_interval", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtNodeinfoInterval(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_nodeinfo_interval", value))
    }
    /// The options of the setting "mt_position_mode" of the stack "meshtastic".
    public enum MeshtasticMtPositionMode: Int32 {
        case off = 0
        case station = 1
    }
    /// Position.
    public static func meshtasticMtPositionMode(_ handle: SDRKIT_HANDLE) -> Result<MeshtasticMtPositionMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "mt_position_mode", &v)
        return code == SDRKIT_OK ? .success(MeshtasticMtPositionMode(rawValue: v) ?? MeshtasticMtPositionMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtPositionMode(_ handle: SDRKIT_HANDLE, _ value: MeshtasticMtPositionMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "mt_position_mode", value.rawValue))
    }
    /// Position interval.
    public static func meshtasticMtPositionInterval(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_position_interval", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtPositionInterval(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_position_interval", value))
    }
    /// Want-ack on channel texts.
    public static func meshtasticMtAckBroadcast(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mt_ack_broadcast", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticMtAckBroadcast(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mt_ack_broadcast", value))
    }
    /// Duty-cycle limit.
    public static func meshtasticDutyLimit(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "duty_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticDutyLimit(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "duty_limit", value))
    }
    /// Duty-cycle bands.
    public static func meshtasticDutyBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "duty_bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticDutyBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "duty_bands", $0) })
    }
    /// Dial.
    public static func meshcoreDialFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dial_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreDialFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dial_freq", value))
    }
    /// Slot band start.
    public static func meshcoreMtSlotStart(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_slot_start", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreMtSlotStart(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_slot_start", value))
    }
    /// Slot band end.
    public static func meshcoreMtSlotEnd(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mt_slot_end", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreMtSlotEnd(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mt_slot_end", value))
    }
    /// Channels.
    public static func meshcoreChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Custom LoRa setting.
    public static func meshcoreCustom(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "custom", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreCustom(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "custom", value))
    }
    /// Frequency.
    public static func meshcoreCustomFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "custom_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreCustomFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "custom_freq", value))
    }
    /// The options of the setting "custom_bw" of the stack "meshcore".
    public enum MeshcoreCustomBw: Int32 {
        case n78125 = 0
        case n104Khz = 1
        case n156Khz = 2
        case n208Khz = 3
        case n3125Khz = 4
        case n417Khz = 5
        case n625Khz = 6
        case n125Khz = 7
        case n250Khz = 8
        case n500Khz = 9
        case n8125Khz = 10
        case n1625Khz = 11
    }
    /// Bandwidth.
    public static func meshcoreCustomBw(_ handle: SDRKIT_HANDLE) -> Result<MeshcoreCustomBw, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &v)
        return code == SDRKIT_OK ? .success(MeshcoreCustomBw(rawValue: v) ?? MeshcoreCustomBw(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreCustomBw(_ handle: SDRKIT_HANDLE, _ value: MeshcoreCustomBw) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_bw", value.rawValue))
    }
    /// Spreading factor.
    public static func meshcoreCustomSf(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreCustomSf(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sf", value))
    }
    /// The options of the setting "custom_cr" of the stack "meshcore".
    public enum MeshcoreCustomCr: Int32 {
        case n45 = 0
        case n46 = 1
        case n47 = 2
        case n48 = 3
    }
    /// Coding rate.
    public static func meshcoreCustomCr(_ handle: SDRKIT_HANDLE) -> Result<MeshcoreCustomCr, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &v)
        return code == SDRKIT_OK ? .success(MeshcoreCustomCr(rawValue: v) ?? MeshcoreCustomCr(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreCustomCr(_ handle: SDRKIT_HANDLE, _ value: MeshcoreCustomCr) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_cr", value.rawValue))
    }
    /// Sync word.
    public static func meshcoreCustomSync(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sync", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreCustomSync(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sync", value))
    }
    /// Preamble.
    public static func meshcoreCustomPreamble(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreCustomPreamble(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_preamble", value))
    }
    /// Lost-sample search.
    public static func meshcoreJumpSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "jump_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreJumpSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "jump_search", value))
    }
    /// Expected jump.
    public static func meshcoreJumpChips(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "jump_chips", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreJumpChips(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "jump_chips", value))
    }
    /// Raw payload.
    public static func meshcoreRaw(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "raw", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreRaw(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "raw", value))
    }
    /// Computer clock at signal 0.
    public static func meshcoreClock0(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreClock0(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock0", value))
    }
    /// Name.
    public static func meshcoreMcName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "mc_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreMcName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "mc_name", $0) })
    }
    /// Send the station's position.
    public static func meshcoreMcShareLocation(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "mc_share_location", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreMcShareLocation(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "mc_share_location", value))
    }
    /// Advert interval.
    public static func meshcoreMcAdvertInterval(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "mc_advert_interval", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreMcAdvertInterval(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "mc_advert_interval", value))
    }
    /// Duty-cycle limit.
    public static func meshcoreDutyLimit(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "duty_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreDutyLimit(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "duty_limit", value))
    }
    /// Duty-cycle bands.
    public static func meshcoreDutyBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "duty_bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreDutyBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "duty_bands", $0) })
    }
    /// Dial.
    public static func lorawanDialFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dial_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanDialFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dial_freq", value))
    }
    /// LoRaWAN channels.
    public static func lorawanLwChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "lw_channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanLwChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "lw_channels", $0) })
    }
    /// Custom LoRa setting.
    public static func lorawanCustom(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "custom", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanCustom(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "custom", value))
    }
    /// Frequency.
    public static func lorawanCustomFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "custom_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanCustomFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "custom_freq", value))
    }
    /// The options of the setting "custom_bw" of the stack "lorawan".
    public enum LorawanCustomBw: Int32 {
        case n78125 = 0
        case n104Khz = 1
        case n156Khz = 2
        case n208Khz = 3
        case n3125Khz = 4
        case n417Khz = 5
        case n625Khz = 6
        case n125Khz = 7
        case n250Khz = 8
        case n500Khz = 9
        case n8125Khz = 10
        case n1625Khz = 11
    }
    /// Bandwidth.
    public static func lorawanCustomBw(_ handle: SDRKIT_HANDLE) -> Result<LorawanCustomBw, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &v)
        return code == SDRKIT_OK ? .success(LorawanCustomBw(rawValue: v) ?? LorawanCustomBw(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanCustomBw(_ handle: SDRKIT_HANDLE, _ value: LorawanCustomBw) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_bw", value.rawValue))
    }
    /// Spreading factor.
    public static func lorawanCustomSf(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanCustomSf(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sf", value))
    }
    /// The options of the setting "custom_cr" of the stack "lorawan".
    public enum LorawanCustomCr: Int32 {
        case n45 = 0
        case n46 = 1
        case n47 = 2
        case n48 = 3
    }
    /// Coding rate.
    public static func lorawanCustomCr(_ handle: SDRKIT_HANDLE) -> Result<LorawanCustomCr, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &v)
        return code == SDRKIT_OK ? .success(LorawanCustomCr(rawValue: v) ?? LorawanCustomCr(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanCustomCr(_ handle: SDRKIT_HANDLE, _ value: LorawanCustomCr) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_cr", value.rawValue))
    }
    /// Sync word.
    public static func lorawanCustomSync(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sync", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanCustomSync(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sync", value))
    }
    /// Preamble.
    public static func lorawanCustomPreamble(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanCustomPreamble(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_preamble", value))
    }
    /// Lost-sample search.
    public static func lorawanJumpSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "jump_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanJumpSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "jump_search", value))
    }
    /// Expected jump.
    public static func lorawanJumpChips(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "jump_chips", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanJumpChips(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "jump_chips", value))
    }
    /// Raw payload.
    public static func lorawanRaw(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "raw", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanRaw(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "raw", value))
    }
    /// Computer clock at signal 0.
    public static func lorawanClock0(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanClock0(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock0", value))
    }
    /// Dial.
    public static func loraaprsDialFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dial_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsDialFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dial_freq", value))
    }
    /// Channels.
    public static func loraaprsChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Custom LoRa setting.
    public static func loraaprsCustom(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "custom", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsCustom(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "custom", value))
    }
    /// Frequency.
    public static func loraaprsCustomFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "custom_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsCustomFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "custom_freq", value))
    }
    /// The options of the setting "custom_bw" of the stack "loraaprs".
    public enum LoraaprsCustomBw: Int32 {
        case n78125 = 0
        case n104Khz = 1
        case n156Khz = 2
        case n208Khz = 3
        case n3125Khz = 4
        case n417Khz = 5
        case n625Khz = 6
        case n125Khz = 7
        case n250Khz = 8
        case n500Khz = 9
        case n8125Khz = 10
        case n1625Khz = 11
    }
    /// Bandwidth.
    public static func loraaprsCustomBw(_ handle: SDRKIT_HANDLE) -> Result<LoraaprsCustomBw, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &v)
        return code == SDRKIT_OK ? .success(LoraaprsCustomBw(rawValue: v) ?? LoraaprsCustomBw(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsCustomBw(_ handle: SDRKIT_HANDLE, _ value: LoraaprsCustomBw) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_bw", value.rawValue))
    }
    /// Spreading factor.
    public static func loraaprsCustomSf(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsCustomSf(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sf", value))
    }
    /// The options of the setting "custom_cr" of the stack "loraaprs".
    public enum LoraaprsCustomCr: Int32 {
        case n45 = 0
        case n46 = 1
        case n47 = 2
        case n48 = 3
    }
    /// Coding rate.
    public static func loraaprsCustomCr(_ handle: SDRKIT_HANDLE) -> Result<LoraaprsCustomCr, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &v)
        return code == SDRKIT_OK ? .success(LoraaprsCustomCr(rawValue: v) ?? LoraaprsCustomCr(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsCustomCr(_ handle: SDRKIT_HANDLE, _ value: LoraaprsCustomCr) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_cr", value.rawValue))
    }
    /// Sync word.
    public static func loraaprsCustomSync(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sync", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsCustomSync(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sync", value))
    }
    /// Preamble.
    public static func loraaprsCustomPreamble(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsCustomPreamble(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_preamble", value))
    }
    /// Lost-sample search.
    public static func loraaprsJumpSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "jump_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsJumpSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "jump_search", value))
    }
    /// Expected jump.
    public static func loraaprsJumpChips(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "jump_chips", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsJumpChips(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "jump_chips", value))
    }
    /// Raw payload.
    public static func loraaprsRaw(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "raw", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsRaw(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "raw", value))
    }
    /// Computer clock at signal 0.
    public static func loraaprsClock0(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsClock0(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock0", value))
    }
    /// SSID.
    public static func loraaprsAprsSsid(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "aprs_ssid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsAprsSsid(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "aprs_ssid", value))
    }
    /// Destination.
    public static func loraaprsAprsTocall(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_tocall", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsAprsTocall(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_tocall", $0) })
    }
    /// Path.
    public static func loraaprsAprsPath(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_path", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsAprsPath(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_path", $0) })
    }
    /// Symbol.
    public static func loraaprsAprsSymbol(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_symbol", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsAprsSymbol(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_symbol", $0) })
    }
    /// Comment.
    public static func loraaprsAprsComment(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_comment", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsAprsComment(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_comment", $0) })
    }
    /// Duty-cycle limit.
    public static func loraaprsDutyLimit(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "duty_limit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsDutyLimit(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "duty_limit", value))
    }
    /// Duty-cycle bands.
    public static func loraaprsDutyBands(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "duty_bands", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsDutyBands(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "duty_bands", $0) })
    }
    /// Dial.
    public static func meshcomDialFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dial_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomDialFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dial_freq", value))
    }
    /// Channels.
    public static func meshcomChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Custom LoRa setting.
    public static func meshcomCustom(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "custom", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomCustom(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "custom", value))
    }
    /// Frequency.
    public static func meshcomCustomFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "custom_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomCustomFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "custom_freq", value))
    }
    /// The options of the setting "custom_bw" of the stack "meshcom".
    public enum MeshcomCustomBw: Int32 {
        case n78125 = 0
        case n104Khz = 1
        case n156Khz = 2
        case n208Khz = 3
        case n3125Khz = 4
        case n417Khz = 5
        case n625Khz = 6
        case n125Khz = 7
        case n250Khz = 8
        case n500Khz = 9
        case n8125Khz = 10
        case n1625Khz = 11
    }
    /// Bandwidth.
    public static func meshcomCustomBw(_ handle: SDRKIT_HANDLE) -> Result<MeshcomCustomBw, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_bw", &v)
        return code == SDRKIT_OK ? .success(MeshcomCustomBw(rawValue: v) ?? MeshcomCustomBw(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomCustomBw(_ handle: SDRKIT_HANDLE, _ value: MeshcomCustomBw) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_bw", value.rawValue))
    }
    /// Spreading factor.
    public static func meshcomCustomSf(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomCustomSf(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sf", value))
    }
    /// The options of the setting "custom_cr" of the stack "meshcom".
    public enum MeshcomCustomCr: Int32 {
        case n45 = 0
        case n46 = 1
        case n47 = 2
        case n48 = 3
    }
    /// Coding rate.
    public static func meshcomCustomCr(_ handle: SDRKIT_HANDLE) -> Result<MeshcomCustomCr, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "custom_cr", &v)
        return code == SDRKIT_OK ? .success(MeshcomCustomCr(rawValue: v) ?? MeshcomCustomCr(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomCustomCr(_ handle: SDRKIT_HANDLE, _ value: MeshcomCustomCr) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "custom_cr", value.rawValue))
    }
    /// Sync word.
    public static func meshcomCustomSync(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_sync", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomCustomSync(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_sync", value))
    }
    /// Preamble.
    public static func meshcomCustomPreamble(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "custom_preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomCustomPreamble(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "custom_preamble", value))
    }
    /// Lost-sample search.
    public static func meshcomJumpSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "jump_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomJumpSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "jump_search", value))
    }
    /// Expected jump.
    public static func meshcomJumpChips(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "jump_chips", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomJumpChips(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "jump_chips", value))
    }
    /// Raw payload.
    public static func meshcomRaw(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "raw", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomRaw(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "raw", value))
    }
    /// Computer clock at signal 0.
    public static func meshcomClock0(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomClock0(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock0", value))
    }
    /// Dial.
    public static func lorawanwideDialFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dial_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanwideDialFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dial_freq", value))
    }
    /// LoRaWAN channels.
    public static func lorawanwideLwChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "lw_channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanwideLwChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "lw_channels", $0) })
    }
    /// Downlinks.
    public static func lorawanwideDownlinks(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "downlinks", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanwideDownlinks(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "downlinks", value))
    }
    /// Default channels only.
    public static func lorawanwideDefaultsOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "defaults_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanwideDefaultsOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "defaults_only", value))
    }
    /// Private networks (0x12).
    public static func lorawanwidePrivateSync(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "private_sync", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanwidePrivateSync(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "private_sync", value))
    }
    /// Channels at most.
    public static func lorawanwideMaxChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanwideMaxChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_channels", value))
    }
    /// 250 and 500 kHz channels.
    public static func lorawanwideWideChannels(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wide_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanwideWideChannels(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wide_channels", value))
    }
    /// FSK channel.
    public static func lorawanwideFsk(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "fsk", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanwideFsk(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "fsk", value))
    }
    /// FSK frequency.
    public static func lorawanwideFskFreq(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "fsk_freq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanwideFskFreq(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "fsk_freq", value))
    }
    /// Computer clock at signal 0.
    public static func lorawanwideClock0(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanwideClock0(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock0", value))
    }
    /// Search SF7 to SF12.
    public static func lorarawAutoSf(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto_sf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawAutoSf(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto_sf", value))
    }
    /// Spreading factor.
    public static func lorarawSf(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawSf(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sf", value))
    }
    /// Search 125, 250 and 500 kHz.
    public static func lorarawAutoBw(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto_bw", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawAutoBw(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto_bw", value))
    }
    /// The options of the setting "bw" of the stack "loraraw".
    public enum LorarawBw: Int32 {
        case n78125 = 0
        case n104Khz = 1
        case n156Khz = 2
        case n208Khz = 3
        case n3125Khz = 4
        case n417Khz = 5
        case n625Khz = 6
        case n125Khz = 7
        case n250Khz = 8
        case n500Khz = 9
        case n8125Khz = 10
        case n1625Khz = 11
    }
    /// Bandwidth.
    public static func lorarawBw(_ handle: SDRKIT_HANDLE) -> Result<LorarawBw, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bw", &v)
        return code == SDRKIT_OK ? .success(LorarawBw(rawValue: v) ?? LorarawBw(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawBw(_ handle: SDRKIT_HANDLE, _ value: LorarawBw) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bw", value.rawValue))
    }
    /// Implicit header.
    public static func lorarawImplicit(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "implicit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawImplicit(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "implicit", value))
    }
    /// Coding rate.
    public static func lorarawCr(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "cr", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawCr(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "cr", value))
    }
    /// CRC.
    public static func lorarawCrc(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "crc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawCrc(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "crc", value))
    }
    /// Length.
    public static func lorarawLength(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "length", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawLength(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "length", value))
    }
    /// The options of the setting "sync" of the stack "loraraw".
    public enum LorarawSync: Int32 {
        case n0x12 = 0
        case n0x34 = 1
        case n0x2B = 2
        case any = 3
        case custom = 4
    }
    /// Sync word.
    public static func lorarawSync(_ handle: SDRKIT_HANDLE) -> Result<LorarawSync, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "sync", &v)
        return code == SDRKIT_OK ? .success(LorarawSync(rawValue: v) ?? LorarawSync(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawSync(_ handle: SDRKIT_HANDLE, _ value: LorarawSync) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "sync", value.rawValue))
    }
    /// Custom sync word.
    public static func lorarawSyncCustom(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sync_custom", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawSyncCustom(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sync_custom", value))
    }
    /// Preamble.
    public static func lorarawPreamble(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "preamble", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawPreamble(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "preamble", value))
    }
    /// The options of the setting "ldro" of the stack "loraraw".
    public enum LorarawLdro: Int32 {
        case auto = 0
        case off = 1
        case on = 2
    }
    /// LDRO.
    public static func lorarawLdro(_ handle: SDRKIT_HANDLE) -> Result<LorarawLdro, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "ldro", &v)
        return code == SDRKIT_OK ? .success(LorarawLdro(rawValue: v) ?? LorarawLdro(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawLdro(_ handle: SDRKIT_HANDLE, _ value: LorarawLdro) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "ldro", value.rawValue))
    }
    /// Offset from the dial.
    public static func lorarawOffset(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "offset", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawOffset(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "offset", value))
    }
    /// Lost-sample search.
    public static func lorarawJumpSearch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "jump_search", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawJumpSearch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "jump_search", value))
    }
    /// Repeat every.
    public static func lorarawTxRepeatS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_repeat_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawTxRepeatS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_repeat_s", value))
    }
    /// Computer clock at signal 0.
    public static func lorarawClock0(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "clock0", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawClock0(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "clock0", value))
    }
    /// 12.5 kHz channel.
    public static func aprsNarrow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "narrow", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsNarrow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "narrow", value))
    }
    /// The options of the setting "emphasis" of the stack "aprs".
    public enum AprsEmphasis: Int32 {
        case auto = 0
        case de = 1
        case pre = 2
    }
    /// Emphasis.
    public static func aprsEmphasis(_ handle: SDRKIT_HANDLE) -> Result<AprsEmphasis, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "emphasis", &v)
        return code == SDRKIT_OK ? .success(AprsEmphasis(rawValue: v) ?? AprsEmphasis(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsEmphasis(_ handle: SDRKIT_HANDLE, _ value: AprsEmphasis) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "emphasis", value.rawValue))
    }
    /// Audio input.
    public static func aprsAudioInput(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "audio_input", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsAudioInput(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "audio_input", value))
    }
    /// Listen.
    public static func aprsListen(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "listen", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsListen(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "listen", value))
    }
    /// SSID.
    public static func aprsSsid(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "aprs_ssid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsSsid(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "aprs_ssid", value))
    }
    /// Destination.
    public static func aprsTocall(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_tocall", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsTocall(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_tocall", $0) })
    }
    /// Path.
    public static func aprsPath(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_path", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsPath(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_path", $0) })
    }
    /// Symbol.
    public static func aprsSymbol(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_symbol", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsSymbol(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_symbol", $0) })
    }
    /// Comment.
    public static func aprsComment(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "aprs_comment", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsComment(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "aprs_comment", $0) })
    }
    /// The options of the setting "tx_baud" of the stack "aprs".
    public enum AprsTxBaud: Int32 {
        case n1200BitS = 0
        case n9600BitS = 1
    }
    /// Speed.
    public static func aprsTxBaud(_ handle: SDRKIT_HANDLE) -> Result<AprsTxBaud, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_baud", &v)
        return code == SDRKIT_OK ? .success(AprsTxBaud(rawValue: v) ?? AprsTxBaud(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsTxBaud(_ handle: SDRKIT_HANDLE, _ value: AprsTxBaud) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_baud", value.rawValue))
    }
    /// TX delay.
    public static func aprsTxDelay(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_delay", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsTxDelay(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_delay", value))
    }
    /// Pre-emphasis.
    public static func aprsTxPreEmphasis(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_pre_emphasis", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsTxPreEmphasis(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_pre_emphasis", value))
    }
    /// Beacon interval.
    public static func aprsBeaconInterval(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "beacon_interval", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsBeaconInterval(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "beacon_interval", value))
    }
    /// Match the SSID.
    public static func aprsMatchSsid(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "match_ssid", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsMatchSsid(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "match_ssid", value))
    }
    /// Wide input.
    public static func aprsWideInput(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "wide_input", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsWideInput(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "wide_input", value))
    }
    /// The options of the setting "bandwidth" of the stack "fanet".
    public enum FanetBandwidth: Int32 {
        case n125Khz = 0
        case n250Khz = 1
        case n500Khz = 2
    }
    /// Bandwidth.
    public static func fanetBandwidth(_ handle: SDRKIT_HANDLE) -> Result<FanetBandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &v)
        return code == SDRKIT_OK ? .success(FanetBandwidth(rawValue: v) ?? FanetBandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setFanetBandwidth(_ handle: SDRKIT_HANDLE, _ value: FanetBandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bandwidth", value.rawValue))
    }
    /// Forget stations after.
    public static func fanetExpiryS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "expiry_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFanetExpiryS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "expiry_s", value))
    }
    /// Search bandwidth and spreading factor.
    public static func reticulumAutoPlan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "auto_plan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setReticulumAutoPlan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "auto_plan", value))
    }
    /// The options of the setting "bw" of the stack "reticulum".
    public enum ReticulumBw: Int32 {
        case n625Khz = 0
        case n125Khz = 1
        case n250Khz = 2
        case n500Khz = 3
    }
    /// Bandwidth.
    public static func reticulumBw(_ handle: SDRKIT_HANDLE) -> Result<ReticulumBw, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bw", &v)
        return code == SDRKIT_OK ? .success(ReticulumBw(rawValue: v) ?? ReticulumBw(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setReticulumBw(_ handle: SDRKIT_HANDLE, _ value: ReticulumBw) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bw", value.rawValue))
    }
    /// Spreading factor.
    public static func reticulumSf(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sf", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setReticulumSf(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sf", value))
    }
    /// Hop channels.
    public static func elrsHopChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "hop_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElrsHopChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "hop_channels", value))
    }
    /// Hop channel spacing.
    public static func elrsHopStepHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hop_step_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElrsHopStepHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hop_step_hz", value))
    }
    /// The options of the setting "packet_rate" of the stack "elrs".
    public enum ElrsPacketRate: Int32 {
        case all = 0
        case n200Hz = 1
        case n100Hz = 2
        case n50Hz = 3
        case n25Hz = 4
        case n100full = 5
    }
    /// Packet rate.
    public static func elrsPacketRate(_ handle: SDRKIT_HANDLE) -> Result<ElrsPacketRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "packet_rate", &v)
        return code == SDRKIT_OK ? .success(ElrsPacketRate(rawValue: v) ?? ElrsPacketRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setElrsPacketRate(_ handle: SDRKIT_HANDLE, _ value: ElrsPacketRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "packet_rate", value.rawValue))
    }
    /// Binding phrase (your own link).
    public static func elrsBindingPhrase(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "binding_phrase", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setElrsBindingPhrase(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "binding_phrase", $0) })
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum LoraGeneratorSettings {
    /// The options of the setting "gen_scene" of the stack "mesh".
    public enum MeshScene: Int32 {
        case auto = 0
        case texting = 1
        case amateur = 2
        case lorawan = 3
    }
    /// Test signal scene.
    public static func meshScene(_ handle: SDRKIT_HANDLE) -> Result<MeshScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(MeshScene(rawValue: v) ?? MeshScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshScene(_ handle: SDRKIT_HANDLE, _ value: MeshScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Damaged copy first.
    public static func meshDamagedCopy(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_damaged_copy", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshDamagedCopy(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_damaged_copy", value))
    }
    /// The options of the setting "gen_scene" of the stack "meshtastic".
    public enum MeshtasticScene: Int32 {
        case auto = 0
        case texting = 1
        case amateur = 2
        case lorawan = 3
    }
    /// Test signal scene.
    public static func meshtasticScene(_ handle: SDRKIT_HANDLE) -> Result<MeshtasticScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(MeshtasticScene(rawValue: v) ?? MeshtasticScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticScene(_ handle: SDRKIT_HANDLE, _ value: MeshtasticScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Damaged copy first.
    public static func meshtasticDamagedCopy(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_damaged_copy", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshtasticDamagedCopy(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_damaged_copy", value))
    }
    /// The options of the setting "gen_scene" of the stack "meshcore".
    public enum MeshcoreScene: Int32 {
        case auto = 0
        case texting = 1
        case amateur = 2
        case lorawan = 3
    }
    /// Test signal scene.
    public static func meshcoreScene(_ handle: SDRKIT_HANDLE) -> Result<MeshcoreScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(MeshcoreScene(rawValue: v) ?? MeshcoreScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcoreScene(_ handle: SDRKIT_HANDLE, _ value: MeshcoreScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// The options of the setting "gen_scene" of the stack "lorawan".
    public enum LorawanScene: Int32 {
        case auto = 0
        case texting = 1
        case amateur = 2
        case lorawan = 3
    }
    /// Test signal scene.
    public static func lorawanScene(_ handle: SDRKIT_HANDLE) -> Result<LorawanScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(LorawanScene(rawValue: v) ?? LorawanScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLorawanScene(_ handle: SDRKIT_HANDLE, _ value: LorawanScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// The options of the setting "gen_scene" of the stack "loraaprs".
    public enum LoraaprsScene: Int32 {
        case auto = 0
        case texting = 1
        case amateur = 2
        case lorawan = 3
    }
    /// Test signal scene.
    public static func loraaprsScene(_ handle: SDRKIT_HANDLE) -> Result<LoraaprsScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(LoraaprsScene(rawValue: v) ?? LoraaprsScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setLoraaprsScene(_ handle: SDRKIT_HANDLE, _ value: LoraaprsScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// The options of the setting "gen_scene" of the stack "meshcom".
    public enum MeshcomScene: Int32 {
        case auto = 0
        case texting = 1
        case amateur = 2
        case lorawan = 3
    }
    /// Test signal scene.
    public static func meshcomScene(_ handle: SDRKIT_HANDLE) -> Result<MeshcomScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(MeshcomScene(rawValue: v) ?? MeshcomScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setMeshcomScene(_ handle: SDRKIT_HANDLE, _ value: MeshcomScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Implicit scene.
    public static func lorarawImplicit(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_implicit", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setLorarawImplicit(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_implicit", value))
    }
    /// The options of the setting "gen_option" of the stack "aprs".
    public enum AprsOption: Int32 {
        case both = 0
        case only1200BitS = 1
        case only9600BitS = 2
    }
    /// Test speeds.
    public static func aprsOption(_ handle: SDRKIT_HANDLE) -> Result<AprsOption, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_option", &v)
        return code == SDRKIT_OK ? .success(AprsOption(rawValue: v) ?? AprsOption(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setAprsOption(_ handle: SDRKIT_HANDLE, _ value: AprsOption) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_option", value.rawValue))
    }
    /// Test signal centre.
    public static func fanetCentre(_ handle: SDRKIT_HANDLE) -> Result<SDRKitPosition, SDRKitCode> {
        var v: sdrkit_position = sdrkit_position()
        let code = sdrkit_typed_setting_get_position(handle, "gen_centre", &v)
        return code == SDRKIT_OK ? .success(SDRKitPosition(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setFanetCentre(_ handle: SDRKIT_HANDLE, _ value: SDRKitPosition) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_position(handle, "gen_centre", value.c))
    }
    /// Test signal stations.
    public static func fanetStations(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_stations", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFanetStations(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_stations", value))
    }
    /// Wrong CRC in every n-th frame.
    public static func fanetBadCrc(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_bad_crc", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setFanetBadCrc(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_bad_crc", value))
    }
    /// Test signal: a split packet.
    public static func reticulumSplit(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_split", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setReticulumSplit(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_split", value))
    }
    /// Test signal: an announce with a wrong signature.
    public static func reticulumTampered(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_tampered", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setReticulumTampered(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_tampered", value))
    }
    /// Test signal binding phrase.
    public static func elrsPhrase(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_phrase", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setElrsPhrase(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_phrase", $0) })
    }
    /// Test signal rate row.
    public static func elrsRow(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_row", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElrsRow(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_row", value))
    }
    /// Test signal with inverted IQ.
    public static func elrsInvertedIq(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_inverted_iq", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElrsInvertedIq(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_inverted_iq", value))
    }
    /// Test signal with a second link without sync frames.
    public static func elrsSecondLink(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_second_link", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setElrsSecondLink(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_second_link", value))
    }
}
