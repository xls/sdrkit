// sdrkit: the typed interface of the settings that every share has, version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum ShareInterface {
    public static let major = 1
    public static let minor = 0
}

/// The state and the counts of a share.
public struct ShareFields {
    /// The radio that the share serves.
    public var radio: String
    /// The session of the share on the radio.
    public var session: String
    /// The sample format that the share carries: `cu8`, `cs8`, `cs16` or `cf32`.
    public var format: String
    /// The sample rate of the capture.
    public var rateHz: Double
    /// The centre frequency of the capture.
    public var centreHz: Double
    /// The listeners that run.
    public var listeners: Int64
    /// The clients that are connected.
    public var clients: Int64
    /// The blocks of samples that the share queued for its clients.
    public var blocks: Int64
    /// The samples that the share queued for its clients.
    public var samples: Int64
    /// The blocks that found no free buffer and reached no client.
    public var poolMisses: Int64
    /// A session of the host reads the radio: with the control Remote, no client steers.
    public var hostReads: Bool
    /// The requests of clients that changed the radio or a slice.
    public var requestsDone: Int64
    /// The requests of clients that the share refused.
    public var requestsRefused: Int64
    /// A client transmits through the share now.
    public var transmitting: Bool
    /// The requests to transmit that the share took.
    public var txTaken: Int64
    /// The requests to transmit that the share refused.
    public var txRefused: Int64
    /// The samples of clients that went to the transmit side of the session.
    public var txSamples: Int64
    /// Why the share does not serve: the hub or a listener could not be made.
    public var problem: String?

    init(_ c: sdrkit_share_fields) {
        radio = sdrkitText(c.radio)
        session = sdrkitText(c.session)
        format = sdrkitText(c.format)
        rateHz = c.rate_hz
        centreHz = c.centre_hz
        listeners = c.listeners
        clients = c.clients
        blocks = c.blocks
        samples = c.samples
        poolMisses = c.pool_misses
        hostReads = c.host_reads
        requestsDone = c.requests_done
        requestsRefused = c.requests_refused
        transmitting = c.transmitting
        txTaken = c.tx_taken
        txRefused = c.tx_refused
        txSamples = c.tx_samples
        problem = c.has_problem ? sdrkitText(c.problem) : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ShareFields, SDRKitCode> {
        var c = sdrkit_share_fields()
        let code = sdrkit_share_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ShareFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the table `listeners`. The row key is the number of the listener, which the command `listen` returns.
public struct ShareListenerRow {
    public enum ProtocolOption: Int32 {
        case rtlTcp = 0
        case soapyremote = 1
    }
    /// The row key.
    public var key: String
    /// The protocol.
    public var `protocol`: ProtocolOption?
    /// The local address.
    public var address: String
    /// The port, as the system gave it for port 0.
    public var port: Int64
    /// The clients of the listener now.
    public var clients: Int64
    /// The connections that the listener took.
    public var accepted: Int64
    /// The connections refused because the address is outside the setting `allow`.
    public var refusedAllow: Int64
    /// The connections refused because `max_clients` clients were connected.
    public var refusedFull: Int64
    /// The bytes sent to the clients of the listener, past and present.
    public var bytes: Int64

    init(_ c: sdrkit_share_listener_row) {
        key = sdrkitText(c.key)
        `protocol` = ProtocolOption(rawValue: c.protocol)
        address = sdrkitText(c.address)
        port = c.port
        clients = c.clients
        accepted = c.accepted
        refusedAllow = c.refused_allow
        refusedFull = c.refused_full
        bytes = c.bytes
    }
}

/// A row of the table `clients`. The row key is the number of the client.
public struct ShareClientRow {
    public enum ProtocolOption: Int32 {
        case rtlTcp = 0
        case soapyremote = 1
    }
    /// The row key.
    public var key: String
    /// The listener that took the client.
    public var listener: String
    /// The protocol.
    public var `protocol`: ProtocolOption?
    /// The address of the client.
    public var address: String?
    /// The sample format of its stream. No value while it has no stream.
    public var format: String?
    /// When it connected.
    public var since: Double
    /// The bytes sent to it.
    public var bytes: Int64
    /// The samples sent to it.
    public var samples: Int64
    /// The samples dropped for this client because it was slow.
    public var droppedSamples: Int64
    /// The times that it began to lose samples.
    public var dropEvents: Int64
    /// The breaks in its stream: its own losses, the losses of the radio and retunes.
    public var gaps: Int64
    /// The blocks that wait for it.
    public var queueFill: Int64
    /// The blocks that its queue holds.
    public var queueCapacity: Int64
    /// The requests it made.
    public var requests: Int64
    /// The requests that the share refused outright. A request that was answered with the value in force is counted in the field `requests_refused` of the share.
    public var refusedRequests: Int64
    /// The centre of its slice, with the control Slice.
    public var sliceCentreHz: Double?
    /// The rate of its slice, with the control Slice.
    public var sliceRateHz: Double?
    /// With the control Remote, this client steers the radio while no session of the host reads it.
    public var steers: Bool
    /// The command `grant` let this client transmit.
    public var mayTransmit: Bool

    init(_ c: sdrkit_share_client_row) {
        key = sdrkitText(c.key)
        listener = sdrkitText(c.listener)
        `protocol` = ProtocolOption(rawValue: c.protocol)
        address = c.has_address ? sdrkitText(c.address) : nil
        format = c.has_format ? sdrkitText(c.format) : nil
        since = c.since
        bytes = c.bytes
        samples = c.samples
        droppedSamples = c.dropped_samples
        dropEvents = c.drop_events
        gaps = c.gaps
        queueFill = c.queue_fill
        queueCapacity = c.queue_capacity
        requests = c.requests
        refusedRequests = c.refused_requests
        sliceCentreHz = c.has_slice_centre_hz ? c.slice_centre_hz : nil
        sliceRateHz = c.has_slice_rate_hz ? c.slice_rate_hz : nil
        steers = c.steers
        mayTransmit = c.may_transmit
    }
}

/// The reply of `listen`.
public struct ShareListenReply {
    /// The number of the listener: its row key in the table `listeners`.
    public var listener: Int64
    /// The port that the listener has.
    public var port: Int64

    init(_ c: sdrkit_share_listen_reply) {
        listener = c.listener
        port = c.port
    }
}

/// The fields of the event `share.client_joined`.
public struct ShareClientJoinedEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of the client: its row key in the table `clients`.
    public var client: Int64
    /// The number of the listener that took it.
    public var listener: Int64
    /// The address of the client.
    public var address: String?

    init(_ c: sdrkit_share_client_joined_event) {
        time = c.time
        freqHz = c.freq_hz
        client = c.client
        listener = c.listener
        address = c.has_address ? sdrkitText(c.address) : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ShareClientJoinedEvent, SDRKitCode> {
        var c = sdrkit_share_client_joined_event()
        let code = sdrkit_share_client_joined_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ShareClientJoinedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event `share.client_left`.
public struct ShareClientLeftEvent {
    public enum Reason: Int32 {
        case closed = 0
        case tooSlow = 1
        case kicked = 2
        case shutdown = 3
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of the client.
    public var client: Int64
    /// The number of the listener that took it.
    public var listener: Int64
    /// The address of the client.
    public var address: String?
    /// Why it left.
    public var reason: Reason?

    init(_ c: sdrkit_share_client_left_event) {
        time = c.time
        freqHz = c.freq_hz
        client = c.client
        listener = c.listener
        address = c.has_address ? sdrkitText(c.address) : nil
        reason = Reason(rawValue: c.reason)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ShareClientLeftEvent, SDRKitCode> {
        var c = sdrkit_share_client_left_event()
        let code = sdrkit_share_client_left_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ShareClientLeftEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event `share.refused`.
public struct ShareRefusedEvent {
    public enum Reason: Int32 {
        case notAllowed = 0
        case full = 1
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The number of the listener.
    public var listener: Int64
    /// The address that connected.
    public var address: String?
    /// Why it was refused.
    public var reason: Reason?

    init(_ c: sdrkit_share_refused_event) {
        time = c.time
        freqHz = c.freq_hz
        listener = c.listener
        address = c.has_address ? sdrkitText(c.address) : nil
        reason = Reason(rawValue: c.reason)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ShareRefusedEvent, SDRKitCode> {
        var c = sdrkit_share_refused_event()
        let code = sdrkit_share_refused_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ShareRefusedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event `share.listen_failed`.
public struct ShareListenFailedEvent {
    public enum ProtocolOption: Int32 {
        case rtlTcp = 0
        case soapyremote = 1
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The protocol.
    public var `protocol`: ProtocolOption?
    /// The local address.
    public var address: String
    /// The port.
    public var port: Int64
    /// What the network said.
    public var reason: String

    init(_ c: sdrkit_share_listen_failed_event) {
        time = c.time
        freqHz = c.freq_hz
        `protocol` = ProtocolOption(rawValue: c.protocol)
        address = sdrkitText(c.address)
        port = c.port
        reason = sdrkitText(c.reason)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<ShareListenFailedEvent, SDRKitCode> {
        var c = sdrkit_share_listen_failed_event()
        let code = sdrkit_share_listen_failed_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(ShareListenFailedEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum ShareTables {
    /// The changed rows of the table "listeners" of the stack "share", newest first, at most `capacity` rows.
    public static func shareListeners(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[ShareListenerRow], SDRKitCode> {
        var rows = [sdrkit_share_listener_row](repeating: sdrkit_share_listener_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_share_listeners_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ShareListenerRow.init))
    }
    /// The changed rows of the table "clients" of the stack "share", newest first, at most `capacity` rows.
    public static func shareClients(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[ShareClientRow], SDRKitCode> {
        var rows = [sdrkit_share_client_row](repeating: sdrkit_share_client_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_share_clients_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(ShareClientRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum ShareCommands {
    /// Listen. Starts a listener of one protocol on the share. A share can have several listeners.
    public static func shareListen(_ handle: SDRKIT_HANDLE, `protocol`: Int32, port: Int64, host: String? = nil) -> Result<ShareListenReply, SDRKitCode> {
        var reply = sdrkit_share_listen_reply()
        var args = sdrkit_share_listen_args()
        args.size = UInt32(MemoryLayout<sdrkit_share_listen_args>.size)
        let code: Int32 = sdrkitWithString(host ?? "") { host_ in { () -> Int32 in
        args.protocol = `protocol`
        args.port = port
        args.has_host = host != nil
        args.host = host_
        return sdrkit_share_listen(handle, &args, &reply)
        }() }
        return code == SDRKIT_OK ? .success(ShareListenReply(reply)) : .failure(SDRKitCode(code: code))
    }
    /// Stop listening. Stops a listener. Its clients stay connected.
    public static func shareUnlisten(_ handle: SDRKIT_HANDLE, listener: Int64) -> Result<Void, SDRKitCode> {
        var args = sdrkit_share_unlisten_args()
        args.size = UInt32(MemoryLayout<sdrkit_share_unlisten_args>.size)
        let code: Int32 = { () -> Int32 in
        args.listener = listener
        return sdrkit_share_unlisten(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Disconnect. Disconnects one client.
    public static func shareKick(_ handle: SDRKIT_HANDLE, client: Int64) -> Result<Void, SDRKitCode> {
        var args = sdrkit_share_kick_args()
        args.size = UInt32(MemoryLayout<sdrkit_share_kick_args>.size)
        let code: Int32 = { () -> Int32 in
        args.client = client
        return sdrkit_share_kick(handle, &args)
        }()
        return sdrkitResult(code)
    }
    /// Grant. Sets what one client may do beyond listening: steer the radio, transmit. No client may transmit until this command says so.
    public static func shareGrant(_ handle: SDRKIT_HANDLE, client: Int64, steer: Bool? = nil, transmit: Bool? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_share_grant_args()
        args.size = UInt32(MemoryLayout<sdrkit_share_grant_args>.size)
        let code: Int32 = { () -> Int32 in
        args.client = client
        args.has_steer = steer != nil
        if let given = steer { args.steer = given }
        args.has_transmit = transmit != nil
        if let given = transmit { args.transmit = given }
        return sdrkit_share_grant(handle, &args)
        }()
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum ShareSettings {
    /// The options of the setting "control" of the stack "share".
    public enum ShareControl: Int32 {
        case follow = 0
        case remote = 1
        case slice = 2
    }
    /// Control.
    public static func shareControl(_ handle: SDRKIT_HANDLE) -> Result<ShareControl, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "control", &v)
        return code == SDRKIT_OK ? .success(ShareControl(rawValue: v) ?? ShareControl(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setShareControl(_ handle: SDRKIT_HANDLE, _ value: ShareControl) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "control", value.rawValue))
    }
    /// Clients at most.
    public static func shareMaxClients(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "max_clients", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setShareMaxClients(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "max_clients", value))
    }
    /// Client queue.
    public static func shareClientQueueMs(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "client_queue_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setShareClientQueueMs(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "client_queue_ms", value))
    }
    /// Drop a slow client after.
    public static func shareDropAfterS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "drop_after_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setShareDropAfterS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "drop_after_s", value))
    }
    /// Allowed addresses.
    public static func shareAllow(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "allow", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setShareAllow(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "allow", $0) })
    }
    /// Remote gain.
    public static func shareRemoteGain(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "remote_gain", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setShareRemoteGain(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "remote_gain", value))
    }
    /// The options of the setting "format" of the stack "share".
    public enum ShareFormat: Int32 {
        case native = 0
        case cu8 = 1
        case cs8 = 2
        case cs16 = 3
        case cf32 = 4
    }
    /// Sample format.
    public static func shareFormat(_ handle: SDRKIT_HANDLE) -> Result<ShareFormat, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "format", &v)
        return code == SDRKIT_OK ? .success(ShareFormat(rawValue: v) ?? ShareFormat(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setShareFormat(_ handle: SDRKIT_HANDLE, _ value: ShareFormat) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "format", value.rawValue))
    }
}
