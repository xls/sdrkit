import Foundation
import SDRKitC

/// The error of a call of the library.
public struct SDRKitError: Error, CustomStringConvertible, LocalizedError, Sendable {
    /// The numbers and names that the library gives with an error. A member that does not apply is nil or empty.
    public struct Detail: Sendable {
        /// The id of the setting, or of the argument of a command.
        public var setting: UInt32?
        /// The id of the command.
        public var command: UInt32?
        /// The id of the result field.
        public var field: UInt32?
        /// The id of the table.
        public var table: UInt32?
        /// A count or an index: an option index, the number of sessions that read a radio, a byte count, the error
        /// code of a device.
        public var index: Int64?
        /// The name of the component at fault.
        public var component: String
        /// The output of a refused connection.
        public var from: String
        /// The input of a refused connection.
        public var to: String
        /// The stream type of the output of a refused connection.
        public var fromType: StreamType
        /// The type that the value must have.
        public var expected: ValueType
        /// The type of the value in the call.
        public var got: ValueType
        /// The lower limit.
        public var min: Double?
        /// The upper limit.
        public var max: Double?
        /// The number in the call.
        public var value: Double?
        /// For `SDRKIT_E_NO_SUCH_KEY` from the engine: the key, the stack id or the driver key that no object has.
        public var key: String = ""
    }

    /// The `SDRKIT_E_*` code.
    public let code: Int32
    /// One English sentence for a log. Empty if the library has none.
    public let message: String
    /// The detail of the error, if the library has it.
    public let detail: Detail?

    /// The name of the code, such as `SDRKIT_E_OUT_OF_RANGE`.
    public var name: String { SDRKitError.name(of: code) }

    /// True if the handle was freed or its object no longer exists.
    public var isDeadHandle: Bool { code == SDRKIT_E_NO_SUCH_OBJECT }

    /// True if a session of a feed has not used the samples of the previous push. Push the samples again later.
    public var isBehind: Bool { code == SDRKIT_E_BEHIND }

    public var description: String { message.isEmpty ? name : "\(name): \(message)" }

    public var errorDescription: String? { description }

    /// Returns the name of the error code `code`, or an empty string if `code` is not an error code.
    public static func name(of code: Int32) -> String {
        // The library returns a static NUL-terminated string and never NULL.
        String(cString: sdrkit_code_name(code))
    }

    init(code: Int32, message: String, detail: Detail?) {
        self.code = code
        self.message = message
        self.detail = detail
    }

    /// Makes the error of the code `code` that a call on `handle` returned, with the message and the detail of the
    /// last error of the engine of `handle`. `handle` is 0 for a call that takes no handle.
    ///
    /// The library keeps one last error for each engine. If another thread failed on the same engine after the call,
    /// the last error is that of the other thread: its code differs, and the error then has no message and no detail.
    init(code: Int32, handle: UInt64) {
        if code == SDRKIT_E_NO_SUCH_OBJECT && handle != 0 {
            self.init(code: code, message: "the handle is closed, or its object no longer exists", detail: nil)
            return
        }
        var raw = sdrkit_error_info.sized
        guard sdrkit_error_detail(handle, &raw) == SDRKIT_OK, raw.code == code else {
            self.init(code: code, message: "", detail: nil)
            return
        }
        // The strings of the detail are copied here, before the next call on the handle.
        let detail = Detail(
            setting: raw.setting == UInt32.max ? nil : raw.setting,
            command: raw.command == UInt32.max ? nil : raw.command,
            field: raw.field == UInt32.max ? nil : raw.field,
            table: raw.table == UInt32.max ? nil : raw.table,
            index: raw.index == -1 ? nil : raw.index,
            component: copy(raw.component),
            from: copy(raw.from),
            to: copy(raw.to),
            fromType: StreamType(rawValue: raw.from_type) ?? .none,
            expected: ValueType(raw: raw.expected),
            got: ValueType(raw: raw.got),
            min: known(raw.min),
            max: known(raw.max),
            value: known(raw.value),
            key: copy(raw.key)
        )
        var s = sdrkit_string()
        let message = sdrkit_error_message(handle, &s) == SDRKIT_OK ? copy(s) : ""
        self.init(code: code, message: message, detail: detail)
    }
}

/// Throws the error of `code` on `handle` unless `code` is `SDRKIT_OK`.
func check(_ code: Int32, handle: UInt64) throws {
    if code != SDRKIT_OK {
        throw SDRKitError(code: code, handle: handle)
    }
}
