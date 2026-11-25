// sdrkit: what the Swift files of the stack families share. MIT licence.
// Written by the interface generator. Do not edit.

import SDRKitC

/// The code of a call that failed: an `SDRKIT_E_*` constant.
public struct SDRKitCode: Error {
    public let code: Int32
}

/// A place on the Earth.
public struct SDRKitPosition {
    /// Degrees north.
    public var lat: Double
    /// Degrees east.
    public var lon: Double
    /// Metres above the WGS 84 ellipsoid.
    public var altM: Double?

    init(_ c: sdrkit_position) {
        lat = c.lat
        lon = c.lon
        altM = c.has_alt ? c.alt_m : nil
    }

    var c: sdrkit_position {
        sdrkit_position(lat: lat, lon: lon, alt_m: altM ?? 0, has_alt: altM != nil)
    }
}

/// A value that carries its own type: a cell of a column whose rows hold values of several types.
public enum SDRKitValue {
    case none
    case bool(Bool)
    case integer(Int64)
    case number(Double)
    case text(String)
    case hz(Double)
    case time(Double)
    case duration(Double)
    case position(lat: Double, lon: Double)
    /// The index of an option.
    case option(UInt32)
    /// The mask of a flag set: bit n is flag n of the declaration.
    case flags(UInt64)
    case bytes([UInt8])

    init(_ c: sdrkit_variant) {
        switch c.type {
        case SDRKIT_TYPE_BOOL: self = .bool(c.flag)
        case SDRKIT_TYPE_I64: self = .integer(c.integer)
        case SDRKIT_TYPE_F64: self = .number(c.number)
        case SDRKIT_TYPE_STRING: self = .text(sdrkitText(c.string))
        case SDRKIT_TYPE_HZ: self = .hz(c.number)
        case SDRKIT_TYPE_TIME: self = .time(c.number)
        case SDRKIT_TYPE_DURATION: self = .duration(c.number)
        case SDRKIT_TYPE_POSITION: self = .position(lat: c.lat, lon: c.lon)
        case SDRKIT_TYPE_ENUM: self = .option(c.enum_index)
        case SDRKIT_TYPE_FLAGS: self = .flags(c.flags)
        case SDRKIT_TYPE_BYTES: self = .bytes([UInt8](UnsafeBufferPointer(start: c.bytes.ptr, count: c.bytes.len)))
        default: self = .none
        }
    }
}

/// One flag of a flag set: its bit, the key that data files use, the name for people and one line of help.
public struct SDRKitFlag<Set> {
    public let flag: Set
    public let key: String
    public let label: String
    public let help: String
}

/// A set of the named flags of a field, held as a mask. `entries` lists every flag in bit order.
public protocol SDRKitFlagSet: OptionSet where RawValue == UInt64, Element == Self {
    static var entries: [SDRKitFlag<Self>] { get }
}

extension SDRKitFlagSet {
    /// The entries of the flags that are set, in bit order.
    public var active: [SDRKitFlag<Self>] {
        Self.entries.filter { contains($0.flag) }
    }
}

func sdrkitText(_ s: sdrkit_string) -> String {
    String(decoding: UnsafeRawBufferPointer(start: s.ptr, count: s.len), as: UTF8.self)
}

/// Calls `body` with `text` as a string of the C interface, valid during the call.
func sdrkitWithString<R>(_ text: String, _ body: (sdrkit_string) -> R) -> R {
    var copy = text
    return copy.withUTF8 { b in
        b.withMemoryRebound(to: CChar.self) { body(sdrkit_string(ptr: $0.baseAddress, len: $0.count)) }
    }
}

func sdrkitResult(_ code: Int32) -> Result<Void, SDRKitCode> {
    code == SDRKIT_OK ? .success(()) : .failure(SDRKitCode(code: code))
}

/// A struct of the C interface that begins with `size`.
public protocol SDRKitSized {
    init()
    var size: UInt32 { get set }
}

extension SDRKitSized {
    /// The struct with its size set and every other member 0. Pass it to every call that takes or fills the struct.
    public static var sized: Self {
        var value = Self()
        value.size = UInt32(MemoryLayout<Self>.size)
        return value
    }
}

extension sdrkit_command_info: SDRKitSized {}
extension sdrkit_decl: SDRKitSized {}
extension sdrkit_option: SDRKitSized {}
extension sdrkit_input: SDRKitSized {}
extension sdrkit_radio_options: SDRKitSized {}
extension sdrkit_stack_options: SDRKitSized {}
extension sdrkit_stack_need_info: SDRKitSized {}
extension sdrkit_error_info: SDRKitSized {}
extension sdrkit_event_type_info: SDRKitSized {}
extension sdrkit_event_info: SDRKitSized {}
extension sdrkit_port_info: SDRKitSized {}
extension sdrkit_report_info: SDRKitSized {}
extension sdrkit_report_port: SDRKitSized {}
extension sdrkit_report_adapter: SDRKitSized {}
extension sdrkit_results_info: SDRKitSized {}
extension sdrkit_table_info: SDRKitSized {}
extension sdrkit_plot_info: SDRKitSized {}
extension sdrkit_snapshot_info: SDRKitSized {}
extension sdrkit_rows_info: SDRKitSized {}
extension sdrkit_point: SDRKitSized {}
extension sdrkit_series_info: SDRKitSized {}
extension sdrkit_raster_info: SDRKitSized {}
extension sdrkit_store_entry: SDRKitSized {}
extension sdrkit_storage: SDRKitSized {}
extension sdrkit_audio_info: SDRKitSized {}
extension sdrkit_iq_info: SDRKitSized {}
extension sdrkit_media_info: SDRKitSized {}
extension sdrkit_spectrum_frame: SDRKitSized {}
extension sdrkit_tap_info: SDRKitSized {}
