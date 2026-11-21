import Foundation
import SDRKitC

/// Owns the handle of an engine. Every object of the engine holds its `EngineHandle`, so the engine is freed after
/// the last of its objects.
final class EngineHandle {
    let handle: UInt64

    init(handle: UInt64) {
        self.handle = handle
    }

    deinit {
        // Freeing the engine stops every session, closes every radio and frees every handle of the engine.
        sdrkit_free(handle)
    }
}

/// A handle of one object of an engine: the engine itself, a radio, a session, the stack of a session, a component,
/// a service, or a stack, driver or component type of the catalog.
///
/// An object has settings, commands, results and events. `deinit` frees the handle with `sdrkit_free`. That frees
/// the handle only: a session runs until `Session.stop()`, and a radio stays open until `Radio.close()`.
///
/// Thread safety: the library allows one thread at a time on a handle, and the class adds no lock. It is not
/// `Sendable`. Use an object from one thread or one serial queue. A second thread opens its own handle of the same
/// object with `Engine.find(_:)`.
public class Object {
    let handle: UInt64
    let owner: EngineHandle

    /// The handle of the object for a call of the C header that the Swift types do not cover. It is valid while
    /// the object lives.
    public var cHandle: UInt64 { handle }
    private var results = Snapshot()
    private var eventTypes: [UInt32: EventType] = [:]

    init(handle: UInt64, owner: EngineHandle) {
        self.handle = handle
        self.owner = owner
    }

    deinit {
        sdrkit_free(handle)
    }

    /// Throws the error of `code` unless `code` is `SDRKIT_OK`.
    func ok(_ code: Int32) throws {
        try check(code, handle: handle)
    }

    /// Runs `call`, a typed call of the generated family files such as `FmFields.read` or
    /// `SessionSettings.setSessionTunedHz`, on the handle of the object, and returns its value. Throws `SDRKitError`
    /// with the detail of the library if the call fails.
    public func typed<T>(_ call: (SDRKIT_HANDLE) -> Result<T, SDRKitCode>) throws -> T {
        switch call(handle) {
        case .success(let value): return value
        case .failure(let error): throw SDRKitError(code: error.code, handle: handle)
        }
    }

    /// The key of the object, as `Engine.find(_:)` reads it. Empty for the stack or a component of a session.
    public var key: String {
        get throws {
            var s = sdrkit_string()
            try ok(sdrkit_object_key(handle, &s))
            return copy(s)
        }
    }

    /// The type of the object.
    public var kind: ObjectKind {
        get throws {
            var t: Int32 = 0
            try ok(sdrkit_object_type(handle, &t))
            guard let kind = ObjectKind(rawValue: t) else {
                throw SDRKitError(code: SDRKIT_E_WRONG_HANDLE, message: "the handle is not an object", detail: nil)
            }
            return kind
        }
    }

    // MARK: Settings

    /// Reads one declaration with `decl` and its options with `option`.
    func declaration(
        id: UInt32,
        decl: (UnsafeMutablePointer<sdrkit_decl>) -> Int32,
        option: (UInt32, UnsafeMutablePointer<sdrkit_option>) -> Int32
    ) throws -> Declaration {
        var raw = sdrkit_decl.sized
        try ok(decl(&raw))
        var d = Declaration(id: id, raw)
        for n in 0..<raw.options {
            var o = sdrkit_option.sized
            try ok(option(n, &o))
            d.options.append(Option(key: copy(o.key), label: copy(o.label)))
        }
        return d
    }

    /// Reads the declaration of setting `id` with its options and its common values.
    func settingDeclaration(id: UInt32) throws -> Declaration {
        let h = handle
        var d = try declaration(
            id: id,
            decl: { sdrkit_setting_decl(h, id, $0) },
            option: { sdrkit_setting_option(h, id, $0, $1) })
        d.commonValues = try commonValues(id: id)
        return d
    }

    /// The values of setting `id` that people normally use, for the present option of the setting that they depend on.
    public func commonValues(id: UInt32) throws -> [Double] {
        var values = [Double](repeating: 0, count: 16)
        var count: UInt32 = 0
        while true {
            let capacity = UInt32(values.count)
            try values.withUnsafeMutableBufferPointer { b in
                try ok(sdrkit_setting_common(handle, id, b.baseAddress, capacity, &count))
            }
            if count <= capacity { return Array(values.prefix(Int(count))) }
            values = [Double](repeating: 0, count: Int(count))
        }
    }

    /// The declarations of the settings, by id.
    public var settings: [Declaration] {
        get throws {
            let h = handle
            var count: UInt32 = 0
            try ok(sdrkit_settings_count(h, &count))
            return try (0..<count).map { id in try settingDeclaration(id: id) }
        }
    }

    /// Returns the id of the setting with the key `key`. Throws `SDRKIT_E_NO_SUCH_KEY` if no setting has the key.
    public func settingID(_ key: String) throws -> UInt32 {
        var id: UInt32 = 0
        try ok(sdrkit_setting_find(handle, try checked(key), &id))
        return id
    }

    /// Returns the declaration of the setting with the key `key`.
    public func setting(_ key: String) throws -> Declaration {
        try settingDeclaration(id: try settingID(key))
    }

    /// Returns the value of the setting `id`. For a type of the catalog, the value is the default.
    public func value(id: UInt32) throws -> Value {
        var v = sdrkit_variant()
        try ok(sdrkit_setting_get(handle, id, &v))
        return Value(v)
    }

    /// Returns the value of the setting with the key `key`.
    public func value(of key: String) throws -> Value {
        try value(id: try settingID(key))
    }

    /// Sets the setting `id` to `value`. The value must match the declaration: its type, range and options. A
    /// refused value changes nothing. Throws a setting code such as `SDRKIT_E_WRONG_TYPE` or `SDRKIT_E_OUT_OF_RANGE`.
    public func set(id: UInt32, to value: Value) throws {
        let storage = Variants()
        var v = storage.variant(value)
        try withExtendedLifetime(storage) {
            try ok(sdrkit_setting_set(handle, id, &v))
        }
    }

    /// Sets the setting with the key `key` to `value`.
    public func set(_ key: String, to value: Value) throws {
        try set(id: try settingID(key), to: value)
    }

    /// Sets the setting with the key `key` to the value that `text` states: `yes` or `no` for a switch, a number, the
    /// key or the label of an option, or the text itself. Throws `SDRKIT_E_MALFORMED` if the text is not a value of
    /// the setting.
    public func set(_ key: String, text: String) throws {
        try ok(sdrkit_set(handle, try checked(key), try checked(text)))
    }

    // MARK: Commands

    /// The declarations of the commands, by id.
    public var commands: [Command] {
        get throws {
            let h = handle
            var count: UInt32 = 0
            try ok(sdrkit_commands_count(h, &count))
            return try (0..<count).map { id in
                var info = sdrkit_command_info.sized
                try ok(sdrkit_command_decl(h, id, &info))
                var command = Command(
                    id: id, key: copy(info.key), label: copy(info.label), help: copy(info.help),
                    arguments: [], replyFields: [], transmits: info.transmits ? info.content : nil)
                let stop = copy(info.stopped_by)
                command.stoppedBy = stop.isEmpty ? nil : stop
                command.arguments = try (0..<info.arguments).map { a in
                    try declaration(
                        id: a,
                        decl: { sdrkit_argument_decl(h, id, a, $0) },
                        option: { sdrkit_argument_option(h, id, a, $0, $1) })
                }
                command.replyFields = try (0..<info.reply_fields).map { f in
                    try declaration(
                        id: f,
                        decl: { sdrkit_reply_field_decl(h, id, f, $0) },
                        option: { sdrkit_reply_field_option(h, id, f, $0, $1) })
                }
                return command
            }
        }
    }

    /// Returns the id of the command with the key `key`. Throws `SDRKIT_E_NO_SUCH_KEY` if no command has the key.
    public func commandID(_ key: String) throws -> UInt32 {
        var id: UInt32 = 0
        try ok(sdrkit_command_find(handle, try checked(key), &id))
        return id
    }

    /// Runs the command `id` with `arguments`, each the id of an argument and its value, and returns the reply
    /// fields by id. An argument that is not given has its default value. Throws `SDRKIT_E_MISSING_ARGUMENT`, a
    /// setting code for a refused argument, `SDRKIT_E_NOT_NOW`, `SDRKIT_E_NO_SUCH_ROW`, `SDRKIT_E_TRANSMIT_OFF` or
    /// `SDRKIT_E_COMMAND_FAILED`.
    public func call(id: UInt32, arguments: [(UInt32, Value)] = []) throws -> [Value] {
        let storage = Variants()
        let args = storage.arguments(arguments)
        var fields: UInt32 = 0
        try withExtendedLifetime(storage) {
            try ok(sdrkit_command_invoke(handle, id, args, args.count, &fields))
        }
        return try (0..<fields).map { f in
            var v = sdrkit_variant()
            try ok(sdrkit_reply(handle, f, &v))
            return Value(v)
        }
    }

    /// Runs the command with the key `key` with `arguments` by argument key, and returns the reply by field key.
    /// Throws `SDRKIT_E_NO_SUCH_KEY` for an unknown command or argument, or the codes of `call(id:arguments:)`.
    @discardableResult
    public func call(_ key: String, arguments: [String: Value] = [:]) throws -> [String: Value] {
        let id = try commandID(key)
        let args = try arguments.map { name, value -> (UInt32, Value) in
            var a: UInt32 = 0
            try ok(sdrkit_argument_find(handle, id, try checked(name), &a))
            return (a, value)
        }
        let values = try call(id: id, arguments: args)
        var reply: [String: Value] = [:]
        for (f, value) in values.enumerated() {
            var raw = sdrkit_decl.sized
            try ok(sdrkit_reply_field_decl(handle, id, UInt32(f), &raw))
            reply[copy(raw.key)] = value
        }
        return reply
    }

    // MARK: Results

    /// Returns the declaration of the results: the fields, tables, series and rasters.
    public func resultsDeclaration() throws -> ResultsDeclaration {
        let h = handle
        var info = sdrkit_results_info.sized
        try ok(sdrkit_results_decl(h, &info))
        var out = ResultsDeclaration()
        out.pointsLabel = copy(info.points_label)
        out.fields = try (0..<info.fields).map { id in
            try declaration(
                id: id,
                decl: { sdrkit_field_decl(h, id, $0) },
                option: { sdrkit_field_option(h, id, $0, $1) })
        }
        out.tables = try (0..<info.tables).map { t in
            var raw = sdrkit_table_info.sized
            try ok(sdrkit_table_decl(h, t, &raw))
            var table = TableDeclaration(id: t, key: copy(raw.key), label: copy(raw.label), columns: [], cap: raw.cap)
            table.columns = try (0..<raw.columns).map { c in
                try declaration(
                    id: c,
                    decl: { sdrkit_column_decl(h, t, c, $0) },
                    option: { sdrkit_column_option(h, t, c, $0, $1) })
            }
            return table
        }
        func plot(_ id: UInt32, _ read: (UnsafeMutablePointer<sdrkit_plot_info>) -> Int32) throws -> PlotDeclaration {
            var raw = sdrkit_plot_info.sized
            try ok(read(&raw))
            return PlotDeclaration(
                id: id, key: copy(raw.key), label: copy(raw.label), xUnit: copy(raw.x_unit), yUnit: copy(raw.y_unit),
                unit: copy(raw.unit))
        }
        out.series = try (0..<info.series).map { id in try plot(id) { sdrkit_series_decl(h, id, $0) } }
        out.rasters = try (0..<info.rasters).map { id in try plot(id) { sdrkit_raster_decl(h, id, $0) } }
        return out
    }

    /// Reads the results of the object and returns them: the status, every result field, every table with all its
    /// rows, the log and the map points.
    ///
    /// The object keeps the results of the previous call and reads from the library only what changed after it, so
    /// a call at a regular interval costs little.
    public func snapshot() throws -> Snapshot {
        if try readSnapshot() {
            // A table lost rows: read everything again.
            results = Snapshot()
            _ = try readSnapshot()
        }
        return results
    }

    /// Takes a snapshot that holds every row of every table, runs `read` on it, a typed table read of the generated
    /// family files such as `{ DvbtTables.dvbtServices($0) }`, and returns its rows: every current row of the table,
    /// newest first, at most the capacity of `read`. Run a typed table read with `typed(_:)` for the changed rows only,
    /// after `snapshot()`. Throws `SDRKitError` with the detail of the library if a call fails.
    public func allRows<Row>(_ read: (SDRKIT_HANDLE) -> Result<[Row], SDRKitCode>) throws -> [Row] {
        var info = sdrkit_snapshot_info.sized
        try ok(sdrkit_snapshot_read(handle, 0, &info))
        if info.declaration_changed {
            // The handle reports a new declaration once: the next `snapshot()` reads everything again.
            results = Snapshot()
        }
        return try typed(read)
    }

    /// Reads one snapshot after `results.counter` and merges it into `results`. Returns true if a table of
    /// `results` then holds more rows than the table of the object has.
    private func readSnapshot() throws -> Bool {
        let h = handle
        let since = results.counter
        var info = sdrkit_snapshot_info.sized
        try ok(sdrkit_snapshot_read(h, since, &info))
        results.status = Status(
            state: State(rawValue: info.state) ?? .off,
            snrDb: known(info.snr_db),
            levelDbfs: known(info.level_dbfs),
            offsetHz: known(info.offset_hz),
            summary: copy(info.summary))
        let all = since == 0 || info.declaration_changed
        if all {
            let decl = try resultsDeclaration()
            results.fields = decl.fields.map { Field(declaration: $0, value: .none) }
            results.tables = decl.tables.map { Table(declaration: $0) }
        }
        results.log = try (0..<info.log_lines).map { n in
            var s = sdrkit_string()
            try ok(sdrkit_snapshot_log(h, n, &s))
            return copy(s)
        }
        results.points = try (0..<info.points).map { n in
            var p = sdrkit_point.sized
            try ok(sdrkit_snapshot_point(h, n, &p))
            return MapPoint(
                id: copy(p.id), latitude: p.lat, longitude: p.lon, label: copy(p.label), heading: known(p.heading))
        }
        if all || info.fields_changed {
            for id in results.fields.indices {
                var v = sdrkit_variant()
                try ok(sdrkit_snapshot_field(h, UInt32(id), &v))
                results.fields[id].value = Value(v)
            }
        }
        var lost = false
        for t in results.tables.indices {
            let table = UInt32(t)
            let columns = results.tables[t].declaration.columns.count
            var rows = sdrkit_rows_info.sized
            try ok(sdrkit_snapshot_table(h, table, &rows))
            // The changed rows are newest first. A row that the copy holds gets its new cells; the new rows go to the
            // front in the same order.
            var index: [String: Int] = [:]
            for (i, row) in results.tables[t].rows.enumerated() where rows.rows > 0 {
                index[row.key] = i
            }
            var fresh: [Row] = []
            for n in 0..<rows.rows {
                var s = sdrkit_string()
                try ok(sdrkit_snapshot_row(h, table, n, &s))
                let key = copy(s)
                var cells: [Value] = []
                cells.reserveCapacity(columns)
                for c in 0..<columns {
                    var v = sdrkit_variant()
                    try ok(sdrkit_snapshot_cell(h, table, n, UInt32(c), &v))
                    cells.append(Value(v))
                }
                if let i = index[key] {
                    results.tables[t].rows[i].cells = cells
                } else {
                    fresh.append(Row(key: key, cells: cells))
                }
            }
            results.tables[t].rows.insert(contentsOf: fresh, at: 0)
            lost = lost || results.tables[t].rows.count > Int(rows.total)
        }
        results.counter = info.counter
        return lost
    }

    /// Returns the points of the series with the key `key`. Throws `SDRKIT_E_NO_SUCH_KEY` if no series has the key.
    public func series(_ key: String) throws -> Series {
        var id: UInt32 = 0
        try ok(sdrkit_series_find(handle, try checked(key), &id))
        var info = sdrkit_series_info.sized
        try ok(sdrkit_snapshot_series(handle, id, 0, &info))
        return Series(counter: info.counter, points: copy(info.points, count: info.count * 2), line: info.line)
    }

    /// Returns the values of the raster with the key `key`. Throws `SDRKIT_E_NO_SUCH_KEY` if no raster has the key.
    public func raster(_ key: String) throws -> Raster {
        var id: UInt32 = 0
        try ok(sdrkit_raster_find(handle, try checked(key), &id))
        var info = sdrkit_raster_info.sized
        try ok(sdrkit_snapshot_raster(handle, id, 0, &info))
        return Raster(
            counter: info.counter, values: copy(info.values, count: info.width * info.height), width: info.width,
            height: info.height, xStart: info.x_start, xStep: info.x_step, yStart: info.y_start, yStep: info.y_step)
    }

    // MARK: Events

    /// Returns the declaration of the event type `id` of the catalog of the engine.
    private func eventType(_ id: UInt32) throws -> EventType {
        if let known = eventTypes[id] { return known }
        let h = handle
        var info = sdrkit_event_type_info.sized
        try ok(sdrkit_event_type_decl(h, id, &info))
        var type = EventType(id: id, key: copy(info.key), label: copy(info.label), fields: [])
        type.fields = try (0..<info.fields).map { f in
            try declaration(
                id: f,
                decl: { sdrkit_event_field_decl(h, id, f, $0) },
                option: { sdrkit_event_field_option(h, id, f, $0, $1) })
        }
        eventTypes[id] = type
        return type
    }

    /// Takes the waiting events of the object and returns them. The engine, radios, sessions and services have
    /// events. An event is returned once.
    public func takeEvents() throws -> [Event] {
        var count: UInt32 = 0
        try ok(sdrkit_events_take(handle, &count))
        return try (0..<count).map { n in
            var info = sdrkit_event_info.sized
            try ok(sdrkit_event(handle, n, &info))
            let type = try eventType(info.type)
            let fields = try type.fields.map { decl -> Field in
                var v = sdrkit_variant()
                try ok(sdrkit_event_field(handle, n, decl.id, &v))
                return Field(declaration: decl, value: Value(v))
            }
            return Event(type: type.key, label: type.label, time: info.time, frequencyHz: info.freq_hz, fields: fields)
        }
    }
}
