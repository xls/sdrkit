import Foundation
import SDRKitC

// Ownership rule of the C API. Every `sdrkit_string`, `sdrkit_bytes` and array pointer that the library writes is a
// view into a buffer of the handle of the call. The view is valid until the next call on the same handle. The caller
// frees no string and no buffer: `sdrkit_free` frees handles only. The name that `sdrkit_code_name` returns is static.
// A string that the caller passes in is a NUL-terminated UTF-8 string that must live for the call only.
//
// The functions below copy a view. Every caller copies all views of a call before it makes the next call on the
// handle.

/// Returns a copy of the string view `s`. Bytes that are not UTF-8 become U+FFFD.
func copy(_ s: sdrkit_string) -> String {
    guard let p = s.ptr, s.len > 0 else { return "" }
    return String(decoding: UnsafeRawBufferPointer(start: p, count: s.len), as: UTF8.self)
}

/// Returns a copy of the byte view `b`.
func copy(_ b: sdrkit_bytes) -> Data {
    guard let p = b.ptr, b.len > 0 else { return Data() }
    return Data(bytes: p, count: b.len)
}

/// Returns a copy of the `count` floats at `p`.
func copy(_ p: UnsafePointer<Float>?, count: Int) -> [Float] {
    guard let p, count > 0 else { return [] }
    return Array(UnsafeBufferPointer(start: p, count: count))
}

/// Returns the four-character code `c` as a string, or an empty string if the code is all zero.
func fourCCString(_ c: (UInt8, UInt8, UInt8, UInt8)) -> String {
    let bytes = [c.0, c.1, c.2, c.3]
    return bytes.allSatisfy { $0 == 0 } ? "" : String(decoding: bytes, as: UTF8.self)
}

/// Returns `s` if the library can read it as a NUL-terminated string. Throws `SDRKIT_E_BAD_STRING` if `s` holds a NUL.
func checked(_ s: String) throws -> String {
    if s.utf8.contains(0) {
        throw SDRKitError(code: SDRKIT_E_BAD_STRING, message: "the string holds a NUL character", detail: nil)
    }
    return s
}

/// Returns nil for NaN, else `x`.
func known(_ x: Double) -> Double? { x.isNaN ? nil : x }

/// Returns nil for NaN, else `x`.
func known(_ x: Float) -> Float? { x.isNaN ? nil : x }
