// The presets part: the regional data of a data folder for a stack in a country. The module SDRKitC has it only if
// the framework was built with the presets part (the default of build_xcframework.sh).
#if canImport(SDRKitC.Presets)
import Foundation
import SDRKitC

/// One channel that the data has for a stack in a country.
public struct PresetChannel: Equatable, Sendable {
    /// The number or the name of the channel.
    public var label: String
    /// The centre frequency, Hz.
    public var frequencyHz: Double
    /// The width, Hz, if the plan has one.
    public var widthHz: Double?
}

/// One supplied entry of the frequency database for a stack in a country.
public struct PresetFrequency: Equatable, Sendable {
    /// The stable id of the entry.
    public var id: String
    /// The name.
    public var name: String
    /// The frequency to tune, Hz.
    public var frequencyHz: Double
    /// The bandwidth, Hz, if the entry states one.
    public var bandwidthHz: Double?
    /// The notes of the entry. Empty if it has none.
    public var note: String
}

/// Where a stack starts in a country.
public struct PresetDial: Equatable, Sendable {
    /// The frequency to start on, Hz.
    public var frequencyHz: Double
    /// The width of the channel at that frequency, Hz.
    public var widthHz: Double?
    /// The id of the plan or the band group of the channels. Empty if there is none.
    public var plan: String
    /// The submode to start in. Nil for the default of the stack.
    public var submode: String?
    /// The network of the known station at the frequency. Empty if the frequency is not of a known station.
    public var network: String
}

/// One Meshtastic frequency slot, as a row of the table `slots` of the stack gives it.
public struct PresetSlot: Equatable, Sendable {
    /// The slot number, from 1.
    public var slot: Int
    /// The centre of the slot, Hz.
    public var frequencyHz: Double
    /// The stack listens on the slot with the channel settings asked for.
    public var current: Bool
    /// The slot of the default channel name of the preset.
    public var presetDefault: Bool
}

/// A data folder: plans, known stations, cities and frequency entries. The engine holds no regional data; a host asks
/// this object for the plain values of a country and sets them on the engine and the stacks.
///
/// Thread safety: as for `Object`. Use it from one thread or one serial queue.
public final class Presets {
    let handle: UInt64

    /// Opens the data folder at `folder` and reads its plan files. Throws `SDRKIT_E_NOT_FOUND` if the folder or its
    /// folder `plans` does not exist, `SDRKIT_E_IO` or `SDRKIT_E_MALFORMED` for a file that cannot be used.
    public init(folder: String) throws {
        var h: UInt64 = 0
        let code = sdrkit_presets_open(try checked(folder), &h)
        if code != SDRKIT_OK {
            throw Presets.error(code, handle: 0)
        }
        handle = h
    }

    deinit {
        sdrkit_presets_close(handle)
    }

    /// The error of `code` with the sentence of the presets handle `handle`.
    static func error(_ code: Int32, handle: UInt64) -> SDRKitError {
        var s = sdrkit_string()
        let message = sdrkit_presets_error_message(handle, &s) == SDRKIT_OK ? copy(s) : ""
        return SDRKitError(code: code, message: message, detail: nil)
    }

    private func ok(_ code: Int32) throws {
        if code != SDRKIT_OK {
            throw Presets.error(code, handle: handle)
        }
    }

    /// Runs `body` with the query of `stack` in `country`, valid during the call.
    private func query<R>(
        _ stack: String, _ country: String, _ city: String?, _ submode: String?, _ frequency: Double?,
        _ body: (UnsafePointer<sdrkit_presets_query>) throws -> R
    ) throws -> R {
        let (stack, country) = (try checked(stack), try checked(country))
        let city = try city.map(checked)
        let submode = try submode.map(checked)
        return try stack.withCString { s in
            try country.withCString { c in
                try withOptionalCString(city) { ci in
                    try withOptionalCString(submode) { m in
                        var q = sdrkit_presets_query.sized
                        q.stack = s
                        q.country = c
                        q.city = ci
                        q.submode = m
                        q.has_frequency_hz = frequency != nil
                        q.frequency_hz = frequency ?? 0
                        return try body(&q)
                    }
                }
            }
        }
    }

    /// Returns the countries that have at least one plan, by code.
    public func countries() throws -> [(code: String, name: String)] {
        var n: UInt32 = 0
        try ok(sdrkit_presets_countries(handle, &n))
        return try (0..<n).map { i in
            var c = sdrkit_presets_country.sized
            try ok(sdrkit_presets_country_read(handle, i, &c))
            return (code: copy(c.code), name: copy(c.name))
        }
    }

    /// Returns the ids of the plans that `country` (ISO 3166 alpha-2) uses, the plan to start with first in each kind.
    /// Empty for a country that the data does not name.
    public func planIDs(country: String) throws -> [String] {
        var n: UInt32 = 0
        try ok(sdrkit_presets_plan_ids(handle, try checked(country), &n))
        return try (0..<n).map { i in
            var s = sdrkit_string()
            try ok(sdrkit_presets_plan_id(handle, i, &s))
            return copy(s)
        }
    }

    /// Gives the scan service of `engine` the channels of the plans `plans`: for each stack that a plan names, the
    /// name and the frequency of each channel. The list replaces the one before; an id that no plan has is skipped.
    /// Returns the number of stacks that have channels.
    @discardableResult
    public func setScanChannels(on engine: Engine, plans: [String]) throws -> Int {
        var stacks: UInt32 = 0
        try withCStrings(try plans.map(checked)) { ids in
            try ok(sdrkit_presets_scan_channels_set(handle, engine.core.handle, ids, ids.count, &stacks))
        }
        return Int(stacks)
    }

    /// Gives the frequency database of `engine` the entries of the file `freqdb/builtin.csv` of the folder, and
    /// returns their number. The list replaces the one before. Throws `SDRKIT_E_NOT_FOUND` if the folder has no such
    /// file.
    @discardableResult
    public func setFrequencyEntries(on engine: Engine) throws -> Int {
        var n: UInt32 = 0
        try ok(sdrkit_presets_frequency_entries_set(handle, engine.core.handle, &n))
        return Int(n)
    }

    /// Sets `file` as the file of the user's plans, adds its plans if it exists, and returns their number. A plan of
    /// the file that lists stacks gives its channels to `channels(stack:country:city:)` of those stacks. Throws
    /// `SDRKIT_E_MALFORMED` for a file that is not a plan file.
    @discardableResult
    public func personalPlans(file: String) throws -> Int {
        var n: UInt32 = 0
        try ok(sdrkit_presets_personal_plans(handle, try checked(file), &n))
        return Int(n)
    }

    /// Reads the `n` rows of the last `sdrkit_presets_channels` or `sdrkit_presets_dials`.
    private func channelRows(_ n: UInt32) throws -> [PresetChannel] {
        try (0..<n).map { i in
            var c = sdrkit_presets_channel.sized
            try ok(sdrkit_presets_channel_read(handle, i, &c))
            return PresetChannel(
                label: copy(c.label), frequencyHz: c.frequency_hz, widthHz: c.has_width_hz ? c.width_hz : nil)
        }
    }

    /// Returns the channels that the data has for `stack` in `country` (ISO 3166 alpha-2), lowest first: those of the
    /// plan of the stack, else the scan channels of the plans of the country, else the dials of every band plan that a
    /// mesh stack offers in the country, else the downlinks of the stack, else its starting dials. The channels of the user's plans (`personalPlans(file:)`) that list the stack, and the
    /// country or no country, come in addition. Throws `SDRKIT_E_BAD_ARGUMENT` for a country that is not a code of two
    /// letters.
    public func channels(stack: String, country: String, city: String? = nil) throws -> [PresetChannel] {
        var n: UInt32 = 0
        try query(stack, country, city, nil, nil) { try ok(sdrkit_presets_channels(handle, $0, &n)) }
        return try channelRows(n)
    }

    /// Returns the dials that the data has for `stack`, lowest first: the dials of `submode` or of `stack`, each
    /// labelled by its band ("20m"); if neither has one and the data gives the stack the setting `dials`, every dial,
    /// labelled by its stack and band ("FT8 20m"). Empty if the data has none. Throws `SDRKIT_E_BAD_ARGUMENT` for a
    /// country that is not a code of two letters.
    public func dials(stack: String, country: String, submode: String? = nil) throws -> [PresetChannel] {
        var n: UInt32 = 0
        try query(stack, country, nil, submode, nil) { try ok(sdrkit_presets_dials(handle, $0, &n)) }
        return try channelRows(n)
    }

    /// Returns the supplied entries of the frequency database (`freqdb/builtin.csv`) for `stack` in `country`, lowest
    /// first: the entries whose stack is `stack`, and the entries of the general receiver or of no stack that have
    /// `stack` as a tag; each applies everywhere, in the country or in its ITU region. Throws `SDRKIT_E_BAD_ARGUMENT`
    /// for a country that is not a code of two letters.
    public func frequencies(stack: String, country: String) throws -> [PresetFrequency] {
        var n: UInt32 = 0
        try query(stack, country, nil, nil, nil) { try ok(sdrkit_presets_frequencies(handle, $0, &n)) }
        return try (0..<n).map { i in
            var f = sdrkit_presets_frequency.sized
            try ok(sdrkit_presets_frequency_read(handle, i, &f))
            return PresetFrequency(
                id: copy(f.id), name: copy(f.name), frequencyHz: f.frequency_hz,
                bandwidthHz: f.has_bandwidth_hz ? f.bandwidth_hz : nil, note: copy(f.note))
        }
    }

    /// Returns where `stack` starts in `country`: the strongest known station of `city`, else the first channel of its
    /// plan, else its first downlink, else its starting dial; `frequency` in Hz replaces it. Nil if the data has
    /// nothing for the stack in the country.
    public func dial(
        stack: String, country: String, city: String? = nil, submode: String? = nil, frequency: Double? = nil
    ) throws -> PresetDial? {
        var d = sdrkit_presets_dial_info.sized
        let code = try query(stack, country, city, submode, frequency) { sdrkit_presets_dial(handle, $0, &d) }
        if code == SDRKIT_E_NOT_FOUND {
            return nil
        }
        try ok(code)
        let mode = copy(d.submode)
        return PresetDial(
            frequencyHz: d.frequency_hz, widthHz: d.has_width_hz ? d.width_hz : nil, plan: copy(d.plan),
            submode: mode.isEmpty ? nil : mode, network: copy(d.network))
    }

    /// Returns the settings that the data has for `stack` in `country`, by key, as values of the settings of the stack
    /// in `engine`: a choice as its option index. They go as they are into `Engine.start(stack:on:...settings:)`.
    /// Empty if the data has nothing for the stack in the country. Throws `SDRKIT_E_NO_SUCH_KEY` if the engine has no
    /// such stack, `SDRKIT_E_MALFORMED` if a value of the data does not fit its setting.
    public func settings(
        stack: String, country: String, engine: Engine, city: String? = nil, submode: String? = nil,
        frequency: Double? = nil
    ) throws -> [String: Value] {
        var n: UInt32 = 0
        let code = try query(stack, country, city, submode, frequency) {
            sdrkit_presets_settings(handle, engine.core.handle, $0, &n)
        }
        if code == SDRKIT_E_NOT_FOUND {
            return [:]
        }
        try ok(code)
        var out: [String: Value] = [:]
        for i in 0..<n {
            var s = sdrkit_presets_setting.sized
            try ok(sdrkit_presets_setting_read(handle, i, &s))
            out[copy(s.key)] = Value(s.arg.value)
        }
        return out
    }

    /// Returns the Meshtastic frequency slots of `preset` (nil: LongFast) in `region`, lowest first, as the table
    /// `slots` of a running stack `meshtastic` lists them. `region` is a Meshtastic region ("ANZ"), a band plan of the
    /// mesh stacks ("EU 868") or a country code. The slot of `channelNumber` (from 1), else of `channelName` (nil: the
    /// name of the preset), is current; a preset with its own frequency in the region makes the slot of that frequency
    /// current, if it is one. Throws `SDRKIT_E_NOT_FOUND` for a region or a preset that the data does not have, and
    /// `SDRKIT_E_OUT_OF_RANGE` for a channel number above the slots of the band.
    public func meshtasticSlots(
        region: String, preset: String? = nil, channelName: String? = nil, channelNumber: Int = 0
    ) throws -> [PresetSlot] {
        guard let number = UInt32(exactly: channelNumber) else {
            throw SDRKitError(code: SDRKIT_E_OUT_OF_RANGE, message: "the channel number is out of range", detail: nil)
        }
        let region = try checked(region)
        let preset = try preset.map(checked)
        let name = try channelName.map(checked)
        var n: UInt32 = 0
        try region.withCString { r in
            try withOptionalCString(preset) { p in
                try withOptionalCString(name) { c in
                    var q = sdrkit_presets_slot_query.sized
                    q.region = r
                    q.preset = p
                    q.channel_name = c
                    q.channel_num = number
                    try ok(sdrkit_presets_meshtastic_slots(handle, &q, &n))
                }
            }
        }
        return try (0..<n).map { i in
            var s = sdrkit_presets_slot.sized
            try ok(sdrkit_presets_meshtastic_slot_read(handle, i, &s))
            return PresetSlot(
                slot: Int(s.slot), frequencyHz: s.frequency_hz, current: s.current, presetDefault: s.preset_default)
        }
    }
}

/// Calls `body` with `texts` as an array of NUL-terminated strings, valid during the call.
private func withCStrings<R>(_ texts: [String], _ body: ([UnsafePointer<CChar>?]) throws -> R) rethrows -> R {
    let copies = texts.map { strdup($0) }
    defer { copies.forEach { free($0) } }
    return try body(copies.map { UnsafePointer($0) })
}

/// Calls `body` with `text` as a NUL-terminated string, or with nil.
private func withOptionalCString<R>(_ text: String?, _ body: (UnsafePointer<CChar>?) throws -> R) rethrows -> R {
    guard let text else { return try body(nil) }
    return try text.withCString { try body($0) }
}

extension sdrkit_presets_query: SDRKitSized {}
extension sdrkit_presets_channel: SDRKitSized {}
extension sdrkit_presets_frequency: SDRKitSized {}
extension sdrkit_presets_dial_info: SDRKitSized {}
extension sdrkit_presets_country: SDRKitSized {}
extension sdrkit_presets_setting: SDRKitSized {}
extension sdrkit_presets_slot_query: SDRKitSized {}
extension sdrkit_presets_slot: SDRKitSized {}
#endif
