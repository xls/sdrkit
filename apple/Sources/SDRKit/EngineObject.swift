import Foundation
import SDRKitC

/// An engine: the radios, the sessions and the catalog of stacks, drivers and components.
///
/// `deinit` releases the engine. The library frees it when the last `Object` of the engine is released too, and
/// then stops every session and closes every radio.
///
/// Thread safety: as for `Object`. Use the engine from one thread or one serial queue. The calls of the engine use
/// the handle of the engine; `object` and every object that the engine returns have their own handles, so each of
/// them can belong to another thread.
public final class Engine {
    let core: EngineHandle

    /// The engine as an object: its settings such as `callsign`, its commands, and its tables such as `stacks`,
    /// `radios` and `sessions`.
    public let object: Object

    /// The version of the library, `major.minor.patch`.
    public static var version: String {
        var s = sdrkit_string()
        return sdrkit_version(&s) == SDRKIT_OK ? copy(s) : ""
    }

    /// Makes an engine that keeps its files in the folder at `folder`, and makes the folder if it does not exist.
    /// With `folder` nil, the engine keeps its files in memory, and they are lost with the engine. Throws
    /// `SDRKIT_E_NOT_FOUND`, `SDRKIT_E_DENIED` or another storage code if the folder cannot be made.
    public init(folder: String? = nil) throws {
        var h: UInt64 = 0
        if let folder {
            try check(sdrkit_engine_new(try checked(folder), &h), handle: 0)
        } else {
            try check(sdrkit_engine_new(nil, &h), handle: 0)
        }
        core = EngineHandle(handle: h)
        var o: UInt64 = 0
        try check(sdrkit_object_find(h, "engine", &o), handle: h)
        object = Object(handle: o, owner: core)
    }

    /// Runs `call`, a typed call of the generated family files such as `EngineSettings.engineCallsign`, on the handle of
    /// the engine, and returns its value. The same as `object.typed(_:)`.
    public func typed<T>(_ call: (SDRKIT_HANDLE) -> Result<T, SDRKitCode>) throws -> T {
        try object.typed(call)
    }

    /// Returns a new handle of the object with the key `key`: `engine`, `radio.r<n>`, `session.s<n>`, `share.h<n>`,
    /// `stack.<id>`, `service.<key>`, `driver.<key>` or `component.<type>`. The object is a `Radio` for a radio, a
    /// `Session` for a session and a `Share` for a share. Throws `SDRKIT_E_BAD_KEY` if `key` is not an object key, `SDRKIT_E_NO_SUCH_KEY` if no object
    /// has the key.
    public func find(_ key: String) throws -> Object {
        var o: UInt64 = 0
        try check(sdrkit_object_find(core.handle, try checked(key), &o), handle: core.handle)
        var type: Int32 = -1
        _ = sdrkit_object_type(o, &type)
        switch type {
        case SDRKIT_OBJECT_RADIO: return Radio(handle: o, owner: core)
        case SDRKIT_OBJECT_SESSION: return Session(handle: o, owner: core)
        case SDRKIT_OBJECT_SHARE: return Share(handle: o, owner: core)
        default: return Object(handle: o, owner: core)
        }
    }

    /// Returns the keys and the types of the live objects: the engine, the radios, the sessions, the shares, the
    /// services, then the stack, driver and component types.
    public func objects() throws -> [(key: String, kind: ObjectKind)] {
        let h = core.handle
        var count: UInt32 = 0
        try check(sdrkit_objects(h, &count), handle: h)
        var out: [(key: String, kind: ObjectKind)] = []
        for n in 0..<count {
            var s = sdrkit_string()
            var type: Int32 = 0
            try check(sdrkit_object(h, n, &s, &type), handle: h)
            if let kind = ObjectKind(rawValue: type) {
                out.append((copy(s), kind))
            }
        }
        return out
    }

    /// Returns the ids and the C values of `settings`, which are keys of settings of the object with the key
    /// `object`.
    private func resolve(_ settings: [String: Value], of object: String) throws -> [(UInt32, Value)] {
        if settings.isEmpty { return [] }
        let type = try find(object)
        return try settings.map { (try type.settingID($0.key), $0.value) }
    }

    /// Returns the settings of every object of the engine as saved settings: bytes that the host stores as they are
    /// and gives to `importSettings(_:)` in a later run. Only the library reads them. `transmit_allowed` is not saved.
    public func exportSettings() throws -> Data {
        var blob = sdrkit_bytes()
        try check(sdrkit_settings_export(core.handle, &blob), handle: core.handle)
        return copy(blob)
    }

    /// Applies saved settings that `exportSettings()` returned, in this run or an earlier one. An object or a setting
    /// that the engine does not have now is skipped. Throws `SDRKIT_E_VERSION` if a later version of the library wrote
    /// the bytes, and `SDRKIT_E_IMPORT` if they are not saved settings, are damaged, or hold a value that a setting
    /// refuses; the call then changes nothing.
    public func importSettings(_ data: Data) throws {
        let h = core.handle
        try data.withUnsafeBytes { (b: UnsafeRawBufferPointer) in
            try check(
                sdrkit_settings_import(h, b.bindMemory(to: UInt8.self).baseAddress, b.count), handle: h)
        }
    }

    /// Returns the declarations of the settings that `openRadio(_:settings:)` takes for `address`, by id: those of
    /// the object `driver.<scheme>`, then those that the radio at the address adds and that are known before it opens,
    /// such as the settings of the test signal of the stack of `synth:<stack>`. `commonValues` is empty: the open
    /// radio gives them. Throws `SDRKIT_E_NO_SUCH_KEY` for an unknown driver or stack.
    public func radioSettings(_ address: String) throws -> [Declaration] {
        let h = core.handle
        let at = try checked(address)
        var count: UInt32 = 0
        try check(sdrkit_radio_open_settings_count(h, at, &count), handle: h)
        return try (0..<count).map { id in
            var raw = sdrkit_decl.sized
            try check(sdrkit_radio_open_setting_decl(h, at, id, &raw), handle: h)
            var d = Declaration(id: id, raw)
            for n in 0..<raw.options {
                var o = sdrkit_option.sized
                try check(sdrkit_radio_open_setting_option(h, at, id, n, &o), handle: h)
                d.options.append(Option(key: copy(o.key), label: copy(o.label), help: copy(o.help)))
            }
            return d
        }
    }

    /// Opens the radio at `address` and returns it. The scheme of the address selects the driver: `iiod:<host>`,
    /// `rtl_tcp:<host>:<port>`, `synth:<stack>` for a test signal, `file:<path>` for a recording in the folder of
    /// the engine. `settings` holds values of the settings that `radioSettings(_:)` declares for the address, by key:
    /// those of the driver and, for a test signal, those of the signal, such as `gen_ps` for `synth:fm`. Throws
    /// `SDRKIT_E_NO_SUCH_KEY` for an unknown driver or setting key, a setting code for a refused setting,
    /// `SDRKIT_E_RADIO_OPEN` if a radio of this address is open and the device has one receiver, or `SDRKIT_E_DRIVER`.
    public func openRadio(_ address: String, settings: [String: Value] = [:]) throws -> Radio {
        let h = core.handle
        let at = try checked(address)
        let ids = try settings.map { (key, value) -> (UInt32, Value) in
            var id: UInt32 = 0
            try check(sdrkit_radio_open_setting_find(h, at, try checked(key), &id), handle: h)
            return (id, value)
        }
        let storage = Variants()
        let args = storage.arguments(ids)
        var radio: UInt64 = 0
        try withExtendedLifetime(storage) {
            try args.withUnsafeBufferPointer { a in
                var options = sdrkit_radio_options.sized
                options.settings = a.baseAddress
                options.count = a.count
                try check(sdrkit_radio_open(h, at, &options, &radio), handle: h)
            }
        }
        return Radio(handle: radio, owner: core)
    }

    /// Returns the USB radios that are attached, from the engine table `devices`: for each the address that
    /// `openRadio(_:settings:)` takes (`usb:rtl:<serial>`), its driver, name, serial number and USB port, its state
    /// (free, open, no permission, needs a driver, busy) and the key of the radio that it is open as. Listing opens no
    /// device. The events `device.arrived` and `device.left` of `object` say when to read the list again. The list is
    /// empty on a platform without USB access, such as iOS; the table `drivers` then states `available` = false for
    /// the USB kinds.
    public func devices() throws -> [EngineDeviceRow] {
        try object.allRows { EngineTables.engineDevices($0) }
    }

    /// Opens the share of `radio` and returns it: an object that serves the samples of the radio to network clients.
    /// Start a listener with its command `listen` (rtl_tcp or SoapyRemote); its settings say who steers the radio and
    /// which addresses may connect. The share never sets the rate or the frequency of the radio by itself and never
    /// stops a session from doing so. No client transmits until the command `grant` lets it and the engine setting
    /// `transmit_allowed` is on. `Radio.close()` closes the share of the radio first. Throws `SDRKIT_E_SHARE_OPEN` if
    /// the radio has a share.
    public func openShare(radio: Radio) throws -> Share {
        var share: UInt64 = 0
        try check(sdrkit_share_open(radio.handle, &share), handle: radio.handle)
        return Share(handle: share, owner: core)
    }

    /// Opens a host feed of complex samples at `rateHz` around `centreHz` and returns it. Sessions read the feed,
    /// and the caller writes it with `Feed.push`.
    public func openFeed(rateHz: Double, centreHz: Double) throws -> Feed {
        var feed: UInt64 = 0
        try check(sdrkit_feed_open(core.handle, rateHz, centreHz, &feed), handle: core.handle)
        return Feed(handle: feed, owner: core)
    }

    /// What the stack `stack` in `submode` (nil for the default) with its default settings needs on this device, by
    /// its recorded cost, before a session starts it. Nil while the calibration of the device has not finished: the
    /// first tens of milliseconds after the engine started. The call never waits. Throws `SDRKIT_E_NO_SUCH_KEY` for an
    /// unknown stack, or `SDRKIT_E_CONFIG` if the stack cannot be made in `submode`.
    public func need(stack: String, submode: String? = nil) throws -> StackNeed? {
        let h = core.handle
        let name = try checked(stack)
        var raw = sdrkit_stack_need_info.sized
        let code: Int32
        if let submode {
            code = try checked(submode).withCString { sdrkit_stack_need(h, name, $0, &raw) }
        } else {
            code = sdrkit_stack_need(h, name, nil, &raw)
        }
        if code == SDRKIT_E_NOT_NOW {
            return nil
        }
        try check(code, handle: h)
        return StackNeed(
            cores: raw.cores, coresFree: raw.cores_free, threads: Int(raw.threads), realTime: raw.real_time,
            units: raw.units, coreUnits: raw.core_units, coreUnitsSlow: raw.core_units_slow, stages: Int(raw.stages))
    }

    /// Starts a session of the stack `stack` on `radio` and returns it. The session is tuned to `frequency` in Hz,
    /// or to the default frequency of the stack if `frequency` is nil. If no other
    /// session reads the radio, the radio is first set to the sample rate of the stack and to that frequency.
    /// `submode` selects a submode of the stack, and `settings` holds values of the settings of the object
    /// `stack.<stack>`, by key. Throws `SDRKIT_E_NO_SUCH_KEY` for an unknown stack, a setting code for a refused
    /// setting or frequency, `SDRKIT_E_CONFIG`, or `SDRKIT_E_NO_SUCH_OBJECT` for a closed radio.
    public func start(
        stack: String, on radio: Radio, frequency: Double? = nil, submode: String? = nil,
        settings: [String: Value] = [:]
    ) throws -> Session {
        try start(
            stack: stack, input: sdrkit_input_radio(radio.handle), frequency: frequency, submode: submode,
            settings: settings)
    }

    /// Starts a session of the stack `stack` on the host feed `feed` and returns it. Any number of sessions can
    /// read one feed. The other parameters are those of `start(stack:on:frequency:submode:settings:)`.
    public func start(
        stack: String, on feed: Feed, submode: String? = nil, settings: [String: Value] = [:]
    ) throws -> Session {
        try start(
            stack: stack, input: sdrkit_input_host(feed.handle), frequency: nil, submode: submode, settings: settings)
    }

    private func start(
        stack: String, input: sdrkit_input, frequency: Double?, submode: String?, settings: [String: Value]
    ) throws -> Session {
        let h = core.handle
        let storage = Variants()
        let args = storage.arguments(try resolve(settings, of: "stack.\(stack)"))
        let name = try checked(stack)
        var input = input
        var session: UInt64 = 0
        func run(_ submode: UnsafePointer<CChar>?) throws {
            try args.withUnsafeBufferPointer { a in
                var options = sdrkit_stack_options.sized
                options.submode = submode
                options.settings = a.baseAddress
                options.count = a.count
                options.centre_hz = frequency ?? 0
                try check(sdrkit_stack_start(h, name, &options, &input, &session), handle: h)
            }
        }
        try withExtendedLifetime(storage) {
            if let submode {
                try checked(submode).withCString { try run($0) }
            } else {
                try run(nil)
            }
        }
        return Session(handle: session, owner: core)
    }
}

/// What a stack needs on this device by its recorded cost (`Engine.need(stack:submode:)`).
public struct StackNeed: Equatable, Sendable {
    /// The fast cores of this device that the stack needs. 0 for a stack with no recorded cost.
    public let cores: Double
    /// The cores that the running sessions leave of the setting `cores_max`.
    public let coresFree: Double
    /// The threads that the engine would give the stack now.
    public let threads: Int
    /// The stack keeps up with its signal here: its largest stage fits one core, and the stack fits the free cores.
    public let realTime: Bool
    /// The recorded cost in calibration units per second of signal.
    public let units: Double
    /// The calibration units per second of one fast core of this device, and of one slow core.
    public let coreUnits: Double
    public let coreUnitsSlow: Double
    /// The stages of the stack.
    public let stages: Int
}
