import Foundation
import SDRKitC

/// The type of a value, as a declaration states it.
public enum ValueType: Int32, Sendable {
    case none = 0
    case bool = 1
    case integer = 2
    case real = 3
    case text = 4
    case hertz = 5
    case time = 6
    case duration = 7
    case position = 8
    case option = 9
    case bytes = 10
    /// Each value carries its own type. Only a column or an argument has this type.
    case any = 11
    /// A set of the flags of the declaration.
    case flags = 12

    init(raw: Int32) {
        self = ValueType(rawValue: raw) ?? .none
    }
}

/// One value of a setting, an argument, a reply, a result field, a table cell or an event field.
public enum Value: Equatable, Sendable {
    /// No value.
    case none
    /// Yes or no.
    case bool(Bool)
    /// A whole number.
    case integer(Int64)
    /// A number.
    case real(Double)
    /// A string.
    case text(String)
    /// A frequency in Hz.
    case hertz(Double)
    /// A time in seconds since 1970 (UTC).
    case time(Double)
    /// A length of time in seconds.
    case duration(Double)
    /// A position in degrees north and degrees east.
    case position(latitude: Double, longitude: Double)
    /// One of the options of the declaration, by its index.
    case option(UInt32)
    /// A set of the flags of the declaration, as a mask: bit n is flag n.
    case flags(UInt64)
    /// Raw bytes.
    case bytes(Data)

    /// The type of the value.
    public var type: ValueType {
        switch self {
        case .none: return .none
        case .bool: return .bool
        case .integer: return .integer
        case .real: return .real
        case .text: return .text
        case .hertz: return .hertz
        case .time: return .time
        case .duration: return .duration
        case .position: return .position
        case .option: return .option
        case .flags: return .flags
        case .bytes: return .bytes
        }
    }

    /// The number of a real, a frequency, a time, a duration or a whole number, else nil.
    public var number: Double? {
        switch self {
        case .real(let x), .hertz(let x), .time(let x), .duration(let x): return x
        case .integer(let n): return Double(n)
        default: return nil
        }
    }

    /// The string of a text value, else nil.
    public var string: String? {
        if case .text(let s) = self { return s }
        return nil
    }

    /// Copies the variant `v`. The views of `v` must still be valid.
    init(_ v: sdrkit_variant) {
        switch v.type {
        case SDRKIT_TYPE_BOOL: self = .bool(v.flag)
        case SDRKIT_TYPE_I64: self = .integer(v.integer)
        case SDRKIT_TYPE_F64: self = .real(v.number)
        case SDRKIT_TYPE_STRING: self = .text(copy(v.string))
        case SDRKIT_TYPE_HZ: self = .hertz(v.number)
        case SDRKIT_TYPE_TIME: self = .time(v.number)
        case SDRKIT_TYPE_DURATION: self = .duration(v.number)
        case SDRKIT_TYPE_POSITION: self = .position(latitude: v.lat, longitude: v.lon)
        case SDRKIT_TYPE_ENUM: self = .option(v.enum_index)
        case SDRKIT_TYPE_FLAGS: self = .flags(v.flags)
        case SDRKIT_TYPE_BYTES: self = .bytes(copy(v.bytes))
        default: self = .none
        }
    }
}

/// The C form of values that the caller passes to the library. The object owns the bytes that the variants point to,
/// so the variants are valid while the object lives.
final class Variants {
    private var buffers: [UnsafeMutableRawPointer] = []

    deinit {
        for b in buffers { b.deallocate() }
    }

    /// Returns a pointer to a copy of the `count` bytes at `source`. The pointer is never NULL.
    private func keep(_ source: UnsafeRawBufferPointer) -> UnsafeMutableRawPointer {
        let p = UnsafeMutableRawPointer.allocate(byteCount: max(source.count, 1), alignment: 1)
        if let base = source.baseAddress, source.count > 0 {
            p.copyMemory(from: base, byteCount: source.count)
        }
        buffers.append(p)
        return p
    }

    /// Returns the variant of `value`.
    func variant(_ value: Value) -> sdrkit_variant {
        var v = sdrkit_variant()
        v.type = value.type.rawValue
        switch value {
        case .none: break
        case .bool(let b): v.flag = b
        case .integer(let n): v.integer = n
        case .real(let x), .hertz(let x), .time(let x), .duration(let x): v.number = x
        case .position(let lat, let lon):
            v.lat = lat
            v.lon = lon
        case .option(let i): v.enum_index = i
        case .flags(let m): v.flags = m
        case .text(let s):
            let utf8 = Array(s.utf8)
            let p = utf8.withUnsafeBytes { keep($0) }
            v.string = sdrkit_string(ptr: UnsafePointer(p.assumingMemoryBound(to: CChar.self)), len: utf8.count)
        case .bytes(let d):
            let p = d.withUnsafeBytes { keep($0) }
            v.bytes = sdrkit_bytes(ptr: UnsafePointer(p.assumingMemoryBound(to: UInt8.self)), len: d.count)
        }
        return v
    }

    /// Returns the arguments of `values`: one `sdrkit_arg` for each id.
    func arguments(_ values: [(UInt32, Value)]) -> [sdrkit_arg] {
        values.map { sdrkit_arg(id: $0.0, value: variant($0.1)) }
    }
}

extension SDRKitPosition {
    /// A position at `lat` degrees north and `lon` degrees east, `altM` metres above the WGS 84 ellipsoid or nil.
    public init(lat: Double, lon: Double, altM: Double? = nil) {
        self.init(sdrkit_position(lat: lat, lon: lon, alt_m: altM ?? 0, has_alt: altM != nil))
    }
}
