import XCTest

import SDRKit

/// Tests of the package against the test signals of the library. No test opens a real radio.
final class SDRKitTests: XCTestCase {
    /// Calls `body` every 20 ms until it returns a value, for at most `seconds`.
    private func poll<T>(seconds: Double, _ body: () throws -> T?) rethrows -> T? {
        let end = Date().addingTimeInterval(seconds)
        repeat {
            if let value = try body() { return value }
            Thread.sleep(forTimeInterval: 0.02)
        } while Date() < end
        return nil
    }

    /// Returns the error that `body` throws.
    private func error(of body: () throws -> Void) -> SDRKitError? {
        do {
            try body()
            return nil
        } catch {
            return error as? SDRKitError
        }
    }

    func testEngineStartsAndListsStacks() throws {
        let engine = try Engine()
        XCTAssertFalse(Engine.version.isEmpty)
        XCTAssertEqual(try engine.object.kind, .engine)
        XCTAssertEqual(try engine.object.key, "engine")
        let stacks = try XCTUnwrap(try engine.object.snapshot().table("stacks"))
        XCTAssertGreaterThan(stacks.rows.count, 100)
        XCTAssertFalse(stacks.declaration.columns.isEmpty)
        XCTAssertEqual(stacks.rows.first?.cells.count, stacks.declaration.columns.count)
    }

    func testEngineInFolder() throws {
        let folder = NSTemporaryDirectory() + "sdrkit-tests-\(UUID().uuidString)"
        defer { try? FileManager.default.removeItem(atPath: folder) }
        let engine = try Engine(folder: folder)
        XCTAssertTrue(FileManager.default.fileExists(atPath: folder))
        XCTAssertTrue(try engine.objects().contains { $0.key == "engine" && $0.kind == .engine })
    }

    func testCallsignSetAndReadBack() throws {
        let engine = try Engine()
        let callsign = try engine.object.setting("callsign")
        XCTAssertEqual(callsign.type, .text)
        XCTAssertFalse(callsign.label.isEmpty)

        // By id, with a typed value.
        try engine.object.set(id: callsign.id, to: .text("VK2ABC"))
        XCTAssertEqual(try engine.object.value(of: "callsign"), .text("VK2ABC"))

        // By key, with the value as text.
        try engine.object.set("callsign", text: "DL1ABC")
        XCTAssertEqual(try engine.object.value(id: callsign.id), .text("DL1ABC"))
    }

    func testFMLocksAndGivesSpectrumAndAudio() throws {
        let engine = try Engine()
        let radio = try engine.openRadio("synth:fm")
        let session = try engine.start(stack: "fm", on: radio)
        XCTAssertEqual(try session.kind, .session)
        try session.set("spectrum", to: .bool(true))

        // Audio that is not read in time is lost, so the test reads it from the start.
        var audio = [Float](repeating: 0, count: 9600)
        var frames = 0
        var rate = 0.0
        var channels = 0
        var layout = AudioInfo.Layout.unknown
        func drain() throws {
            let got = try session.readAudio(into: &audio)
            if got.frames > 0 {
                frames += got.frames
                rate = got.rateHz
                channels = got.channels
                layout = got.layout
                XCTAssertLessThanOrEqual(got.frames * got.channels, audio.count)
            }
        }

        let locked = try poll(seconds: 10) { () -> Status? in
            try drain()
            let status = try session.snapshot().status
            return status.state == .locked ? status : nil
        }
        XCTAssertEqual(locked?.state, .locked)
        XCTAssertNotNil(locked?.snrDb)
        XCTAssertFalse(locked?.summary.isEmpty ?? true)

        var bins: [Float] = []
        let spectrum = try poll(seconds: 5) { () -> SpectrumInfo? in
            try drain()
            return try session.readSpectrum(into: &bins)
        }
        XCTAssertNotNil(spectrum)
        XCTAssertGreaterThan(bins.count, 100)
        XCTAssertGreaterThan(spectrum?.spanHz ?? 0, 0)

        _ = try poll(seconds: 5) { () -> Bool? in
            try drain()
            return frames > 0 ? true : nil
        }
        XCTAssertGreaterThan(frames, 0)
        XCTAssertEqual(rate, 48000)
        XCTAssertGreaterThan(channels, 0)
        XCTAssertEqual(layout, channels == 1 ? .mono : .stereo)

        // The stack of the session has the same results under its own handle.
        XCTAssertEqual(try session.stack().snapshot().status.state, .locked)
        XCTAssertEqual(try session.problem, "")

        // A second handle of the session, as another thread opens it.
        let again = try XCTUnwrap(try engine.find(try session.key) as? Session)
        XCTAssertEqual(try again.snapshot().status.state, .locked)

        XCTAssertEqual(error(of: { try radio.close() })?.name, "SDRKIT_E_RADIO_IN_USE")
        XCTAssertEqual(error(of: { try session.close() })?.name, "SDRKIT_E_STILL_RUNNING")
        try session.stop()
        try session.close()
        try radio.close()
    }

    func testFMLocksAfterATypedRetune() throws {
        let engine = try Engine()
        let radio = try engine.openRadio("synth:fm")
        let session = try engine.start(stack: "fm", on: radio)
        _ = try session.snapshot()
        let before = try session.typed(SessionFields.read).retunes

        try session.typed { SessionSettings.setSessionTunedHz($0, 104.9e6) }
        XCTAssertEqual(try session.typed(SessionSettings.sessionTunedHz), 104.9e6)

        // Two seconds of signal at the new frequency, with the audio read as it comes.
        var audio = [Float](repeating: 0, count: 9600)
        var frames = 0
        var retunes = before
        let end = Date().addingTimeInterval(2)
        while Date() < end {
            frames += try session.readAudio(into: &audio).frames
            _ = try session.snapshot()
            retunes = try session.typed(SessionFields.read).retunes
            Thread.sleep(forTimeInterval: 0.02)
        }
        let status = try session.snapshot().status
        let fm = try session.typed(FmFields.read)

        XCTAssertGreaterThan(retunes, before)
        XCTAssertEqual(status.state, .locked)
        XCTAssertTrue(fm.carrier)
        XCTAssertGreaterThan(frames, 0)

        try session.stop()
        try session.close()
        try radio.close()
    }

    func testSettingsExportAndImport() throws {
        let engine = try Engine()
        try engine.object.set("callsign", text: "VK2ABC")
        let saved = try engine.exportSettings()
        XCTAssertFalse(saved.isEmpty)

        try engine.object.set("callsign", text: "DL1ABC")
        try engine.importSettings(saved)
        XCTAssertEqual(try engine.object.value(of: "callsign"), .text("VK2ABC"))

        // A second engine takes the bytes too.
        let other = try Engine()
        try other.importSettings(saved)
        XCTAssertEqual(try other.object.value(of: "callsign"), .text("VK2ABC"))

        let damaged = try XCTUnwrap(error(of: { try engine.importSettings(Data([1, 2, 3])) }))
        XCTAssertEqual(damaged.name, "SDRKIT_E_IMPORT")
    }

    func testDVBTMovesTheRadioToTheFrequencyOfTheHost() throws {
        let engine = try Engine()
        let radio = try engine.openRadio("synth:dvbt")
        let session = try engine.start(stack: "dvbt", on: radio, frequency: 177.5e6)
        let centre = try poll(seconds: 10) { () -> Value? in
            let value = try radio.snapshot().value(of: "centre_hz")
            return value == .hertz(177.5e6) ? value : nil
        }
        XCTAssertEqual(centre, .hertz(177.5e6))
        try session.stop()
        try session.close()
        try radio.close()
    }

    func testHostAudioIsTakenAndNotWantedWithoutATransmitter() throws {
        let engine = try Engine()
        let radio = try engine.openRadio("synth:general")
        let session = try engine.start(stack: "general", on: radio)
        let tone = (0..<480).map { Float(0.25 * sin(Double($0) * 0.13)) }
        try session.writeAudio(tone, channels: 1, rateHz: 48000)
        try tone.withUnsafeBufferPointer { try session.writeAudio($0, channels: 1, rateHz: 44100) }
        try session.writeAudio(tone, channels: 2, rateHz: 48000)
        XCTAssertEqual(error(of: { try session.writeAudio(tone, channels: 1, rateHz: 0) })?.name, "SDRKIT_E_CONFIG")
        XCTAssertEqual(
            error(of: { try session.writeAudio(tone, channels: 6, rateHz: 48000) })?.name, "SDRKIT_E_UNSUPPORTED")
        XCTAssertEqual(
            error(of: { try session.writeAudio(tone, channels: 0, rateHz: 48000) })?.name, "SDRKIT_E_UNSUPPORTED")
        // The test signal cannot transmit: the session does not ask for host audio.
        let fields = try poll(seconds: 10) { () -> SessionFields? in
            _ = try session.snapshot()
            let f = try session.typed(SessionFields.read)
            return f.samples > 0 ? f : nil
        }
        XCTAssertNil(fields?.audioInRateHz)
        XCTAssertEqual(fields?.audioInWanted, false)
        try session.stop()
        try session.close()
        try radio.close()
    }

    func testEngineTablesFollowRadiosAndSessions() throws {
        let engine = try Engine()
        let before = try engine.object.snapshot()
        XCTAssertEqual(before.table("radios")?.rows.count, 0)
        XCTAssertEqual(before.table("sessions")?.rows.count, 0)

        let radio = try engine.openRadio("synth:fm")
        let session = try engine.start(stack: "fm", on: radio)
        let open = try engine.object.snapshot()
        XCTAssertNotNil(open.table("radios")?.row(try radio.key))
        XCTAssertNotNil(open.table("sessions")?.row(try session.key))
        XCTAssertGreaterThan(open.table("stacks")?.rows.count ?? 0, 100)

        try session.stop()
        try session.close()
        try radio.close()
        let after = try engine.object.snapshot()
        XCTAssertEqual(after.table("radios")?.rows.count, 0)
        XCTAssertEqual(after.table("sessions")?.rows.count, 0)
        XCTAssertEqual(after.table("stacks")?.rows.count, open.table("stacks")?.rows.count)
    }

    func testAShareServesARadioAndClosesWithIt() throws {
        let engine = try Engine()
        // Listing the USB radios opens no device; the kinds of USB radios are in the table of drivers.
        _ = try engine.devices()
        let drivers = try engine.object.allRows { EngineTables.engineDrivers($0) }
        XCTAssertNotNil(drivers.first { $0.key == "rtlusb" })
        XCTAssertEqual(drivers.first { $0.key == "rtl_tcp" }?.available, true)

        let radio = try engine.openRadio("synth:fm")
        let share = try engine.openShare(radio: radio)
        XCTAssertTrue(try share.key.hasPrefix("share.h"))
        XCTAssertEqual(error(of: { _ = try engine.openShare(radio: radio) })?.name, "SDRKIT_E_SHARE_OPEN")
        XCTAssertEqual(try share.typed(ShareSettings.shareControl), .follow)
        try share.typed { ShareSettings.setShareControl($0, .slice) }
        // An rtl_tcp listener on this machine only, at a port that the system chooses.
        let rtlTcp = ShareListenerRow.ProtocolOption.rtlTcp.rawValue
        let reply = try share.typed {
            ShareCommands.shareListen($0, protocol: rtlTcp, port: 0, host: "127.0.0.1")
        }
        XCTAssertGreaterThan(reply.port, 0)
        let listeners = try share.allRows { ShareTables.shareListeners($0) }
        XCTAssertEqual(listeners.map(\.port), [reply.port])
        XCTAssertEqual(try share.typed(ShareFields.read).listeners, 1)
        XCTAssertEqual(try engine.object.allRows { EngineTables.engineShares($0) }.count, 1)
        // The radio closes with its share on it.
        try radio.close()
        XCTAssertEqual(try engine.object.allRows { EngineTables.engineShares($0) }.count, 0)
        XCTAssertThrowsError(try share.typed(ShareFields.read))
    }

    func testATypedTableReadOfEveryRow() throws {
        let engine = try Engine()
        let all = try engine.object.snapshot().table("stacks")?.rows.count ?? 0
        XCTAssertGreaterThan(all, 100)
        // After a snapshot without changes, the typed read has no changed row; the read of every row has them all.
        _ = try engine.object.snapshot()
        XCTAssertEqual(try engine.object.typed { EngineTables.engineStacks($0) }.count, 0)
        let rows = try engine.object.allRows { EngineTables.engineStacks($0) }
        XCTAssertEqual(rows.count, all)
        // The FM sender takes stereo host audio; DVB-T takes none.
        XCTAssertEqual(rows.first { $0.key == "fm" }?.hostAudioChannels, 2)
        XCTAssertEqual(rows.first { $0.key == "dvbt" }?.hostAudioChannels, 0)
        XCTAssertEqual(try engine.object.snapshot().table("stacks")?.rows.count, all)
    }

    func testStacksStateTheirKindAndTheirMedia() throws {
        let engine = try Engine()
        let rows = try engine.object.allRows { EngineTables.engineStacks($0) }
        // Every stack states its kind of content.
        XCTAssertTrue(rows.allSatisfy { $0.kind != nil })
        let fm = try XCTUnwrap(rows.first { $0.key == "fm" })
        XCTAssertEqual(fm.kind, .broadcast)
        XCTAssertEqual(fm.audio, .programme)
        let adsb = try XCTUnwrap(rows.first { $0.key == "adsb" })
        XCTAssertEqual(adsb.kind, .tracking)
        XCTAssertNil(adsb.audio)
        XCTAssertTrue(rows.first { $0.key == "dvbt" }?.video ?? false)
        // The table of the stacks names its rows by the column `name`, and states no order and no place.
        let stacks = try XCTUnwrap(try engine.object.resultsDeclaration().tables.first { $0.key == "stacks" })
        XCTAssertEqual(stacks.labelColumn.map { stacks.columns[Int($0)].key }, "name")
        XCTAssertNil(stacks.sort)
        XCTAssertFalse(stacks.place.onMap)
        // The stacks are no messages, calls, nodes or groups, and no column is a part of a message.
        XCTAssertNil(stacks.role)
        XCTAssertEqual(stacks.message, TableMessage())
        // The call sign of the station chooses no picture.
        XCTAssertNil(try engine.object.setting("callsign").role)
        // It takes no value from a table, and it is no key reading.
        XCTAssertEqual(try engine.object.setting("callsign").refersColumn, "")
        XCTAssertNil(try engine.object.setting("callsign").reading)
    }

    func testSettingErrors() throws {
        let engine = try Engine()

        let wrongType = try XCTUnwrap(error(of: { try engine.object.set("callsign", to: .bool(true)) }))
        XCTAssertEqual(wrongType.name, "SDRKIT_E_WRONG_TYPE")
        XCTAssertFalse(wrongType.message.isEmpty)
        XCTAssertEqual(wrongType.detail?.expected, .text)
        XCTAssertEqual(wrongType.detail?.got, .bool)
        XCTAssertEqual(wrongType.detail?.setting, try engine.object.settingID("callsign"))

        let unknown = try XCTUnwrap(error(of: { try engine.object.set("no_such_setting", to: .integer(1)) }))
        XCTAssertEqual(unknown.name, "SDRKIT_E_NO_SUCH_KEY")

        let unknownText = try XCTUnwrap(error(of: { try engine.object.set("no_such_setting", text: "1") }))
        XCTAssertEqual(unknownText.name, "SDRKIT_E_NO_SUCH_KEY")

        let badKey = try XCTUnwrap(error(of: { _ = try engine.find("no such object") }))
        XCTAssertEqual(badKey.name, "SDRKIT_E_BAD_KEY")
    }

    func testCommandWithMissingArgument() throws {
        let engine = try Engine()
        let restore = try XCTUnwrap(try engine.object.commands.first { $0.key == "state.restore" })
        let state = try XCTUnwrap(restore.arguments.first { $0.key == "state" })
        XCTAssertTrue(state.flags.contains(.required))

        let missing = try XCTUnwrap(error(of: { try engine.object.call("state.restore") }))
        XCTAssertEqual(missing.name, "SDRKIT_E_MISSING_ARGUMENT")
        XCTAssertEqual(missing.detail?.setting, state.id)

        // A command with a reply: the saved state comes back as bytes, and the engine takes it again.
        let reply = try engine.object.call("state.save")
        let saved = try XCTUnwrap(reply.values.first)
        XCTAssertEqual(saved.type, .bytes)
        try engine.object.call("state.restore", arguments: ["state": saved])

        let unknown = try XCTUnwrap(error(of: { try engine.object.call("no.such.command") }))
        XCTAssertEqual(unknown.name, "SDRKIT_E_NO_SUCH_KEY")
    }

    func testNeedOfAStackOnThisDevice() throws {
        let engine = try Engine()
        // Nil until the calibration of the device has finished.
        let need = try XCTUnwrap(try poll(seconds: 5) { try engine.need(stack: "dvbt") })
        XCTAssertGreaterThan(need.cores, 0)
        XCTAssertGreaterThan(need.coreUnits, 0)
        XCTAssertGreaterThan(need.stages, 1)
        XCTAssertEqual(error(of: { _ = try engine.need(stack: "no_such_stack") })?.name, "SDRKIT_E_NO_SUCH_KEY")
    }

    func testTypedCallOnTheEngineWithAPosition() throws {
        let engine = try Engine()
        let here = SDRKitPosition(lat: -33.87, lon: 151.21, altM: 50)
        try engine.typed { EngineSettings.setEnginePosition($0, here) }
        let read = try engine.typed { EngineSettings.enginePosition($0) }
        XCTAssertEqual(read.lat, here.lat, accuracy: 1e-9)
        XCTAssertEqual(read.lon, here.lon, accuracy: 1e-9)
        XCTAssertNil(SDRKitPosition(lat: 1, lon: 2).altM)
    }

    func testTypedScanStartHitsAndStop() throws {
        let engine = try Engine()
        let radio = try engine.openRadio("synth:fm")
        let session = try engine.start(stack: "fm", on: radio, frequency: 104.9e6)
        let key = try session.key
        let scan = try engine.find("service.scan")
        let reply = try scan.typed { ScanCommands.scanStart($0, session: key, stack: "fm") }
        XCTAssertGreaterThan(reply.channels, 0)
        let hits = try poll(seconds: 20) { () -> [ScanHitRow]? in
            let rows = try scan.allRows { ScanTables.scanHits($0) }
            return rows.isEmpty ? nil : rows
        }
        XCTAssertNotNil(hits)
        try scan.typed { ScanCommands.scanStop($0) }
        try session.stop()
        try session.close()
        try radio.close()
    }

    func testTypedRecordAudioStartRowsAndStop() throws {
        let engine = try Engine()
        let radio = try engine.openRadio("synth:fm")
        let session = try engine.start(stack: "fm", on: radio, frequency: 104.9e6)
        let record = try engine.find("service.record")
        try record.typed { RecordSettings.setRecordReserveBytes($0, 0) }
        let key = try session.key
        let reply = try record.typed { RecordCommands.recordAudioStart($0, session: key, seconds: 30) }
        // The size of the open file grows in the running row.
        let open = try poll(seconds: 10) { () -> RecordRunningRow? in
            let rows = try record.allRows { RecordTables.recordRunning($0) }
            return rows.first.flatMap { $0.bytes > 0 ? $0 : nil }
        }
        XCTAssertEqual(open?.key, reply.recording)
        try record.typed { RecordCommands.recordAudioStop($0, row: reply.recording) }
        let done = try poll(seconds: 10) { () -> [RecordRecordingRow]? in
            guard try record.allRows({ RecordTables.recordRunning($0) }).isEmpty else { return nil }
            let rows = try record.allRows { RecordTables.recordRecordings($0) }
            return rows.isEmpty ? nil : rows
        }
        XCTAssertEqual(done?.first?.kind, .audio)
        try session.stop()
        try session.close()
        try radio.close()
    }

    #if canImport(SDRKitC.Presets)
    /// The data folder of the repository.
    private var dataFolder: String {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("data").path
    }

    func testPresetsOfAStackInACountry() throws {
        let presets = try Presets(folder: dataFolder)
        let channels = try presets.channels(stack: "dvbt", country: "AU")
        XCTAssertGreaterThan(channels.count, 30)
        XCTAssertEqual(channels.first?.frequencyHz, 177.5e6)
        XCTAssertEqual(channels.first?.widthHz, 7e6)
        let dial = try XCTUnwrap(try presets.dial(stack: "dvbt", country: "AU"))
        XCTAssertEqual(dial.frequencyHz, 177.5e6)
        XCTAssertNil(try presets.dial(stack: "dvbt", country: "XX"))
        XCTAssertEqual(error(of: { _ = try presets.channels(stack: "dvbt", country: "Australia") })?.name,
                       "SDRKIT_E_BAD_ARGUMENT")

        // The settings go as they are into a start.
        let engine = try Engine()
        let settings = try presets.settings(stack: "dvbt", country: "AU", engine: engine)
        XCTAssertNotNil(settings["bandwidth"])
        let radio = try engine.openRadio("synth:dvbt")
        let session = try engine.start(stack: "dvbt", on: radio, frequency: dial.frequencyHz, settings: settings)
        XCTAssertEqual(try session.stack().value(of: "bandwidth"), settings["bandwidth"])
        try session.stop()
    }

    func testPresetsListsOfTheFolder() throws {
        let presets = try Presets(folder: dataFolder)
        let countries = try presets.countries()
        XCTAssertTrue(countries.contains { $0.code == "AU" && $0.name == "Australia" })
        let plans = try presets.planIDs(country: "AU")
        XCTAssertTrue(plans.contains("tv_au"))
        XCTAssertEqual(try presets.planIDs(country: "XX"), [])

        let engine = try Engine()
        XCTAssertGreaterThan(try presets.setScanChannels(on: engine, plans: plans), 1)
        XCTAssertGreaterThan(try presets.setFrequencyEntries(on: engine), 100)

        // The dials of a stack as rows, labelled by band; the family stack without a submode has every dial.
        let ft8 = try presets.dials(stack: "ft8", country: "AU")
        XCTAssertEqual(ft8.first { $0.label == "20m" }?.frequencyHz, 14.074e6)
        XCTAssertEqual(try presets.dials(stack: "wsjt", country: "AU", submode: "ft8"), ft8)
        XCTAssertTrue(try presets.dials(stack: "wsjt", country: "AU").contains { $0.label == "FT8 20m" })
        XCTAssertEqual(try presets.dials(stack: "dvbt", country: "AU"), [])
    }

    func testPresetsSayWhyAModeIsOutOfService() throws {
        let presets = try Presets(folder: dataFolder)
        let umts = try XCTUnwrap(try presets.serviceEnd(stack: "umts", country: "AU"))
        XCTAssertEqual(umts.endedYear, 2024)
        XCTAssertFalse(umts.partial)
        XCTAssertEqual(umts.sentence, "3G (UMTS) was switched off in Australia in 2024")
        XCTAssertEqual(try presets.channels(stack: "umts", country: "AU"), [])
        let gsm = try XCTUnwrap(try presets.serviceEnd(stack: "gsmr", country: "AU"))
        XCTAssertEqual(gsm.sentence, "2G (GSM) was switched off in Australia in 2018")
        XCTAssertNil(try presets.serviceEnd(stack: "umts", country: "US"))
    }

    func testPersonalPlansAreChannelsOfTheirStack() throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("sdrkit-swift-\(getpid())")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        // A plan file of the user with one plan for the stack nfm.
        let file = folder.appendingPathComponent("plans.json")
        try personalPlan.write(to: file, atomically: true, encoding: .utf8)
        let presets = try Presets(folder: dataFolder)
        XCTAssertEqual(try presets.personalPlans(file: file.path), 1)
        for country in ["AU", "DE"] {
            let mine = try presets.channels(stack: "nfm", country: country).filter { $0.label == "Mine" }
            XCTAssertEqual(mine.map(\.frequencyHz), [151e6])
            XCTAssertEqual(mine.map(\.widthHz), [12500])
        }
        XCTAssertFalse(try presets.channels(stack: "dvbt", country: "AU").contains { $0.label == "Mine" })
    }

    /// A plan file with one plan of one channel for the stack nfm, as `sdrkit-presets/tests/lookup.rs` reads it too.
    private let personalPlan = """
        {"format": 1, "groups": [{"id": "mine", "name": "Mine", "kind": "other",
        "source": {"publisher": "The user", "title": "Mine", "date": "2026"}, "verified": false,
        "unverified_reason": "made by the user", "bands": [{"name": "Mine", "service": "land_mobile",
        "start_hz": 150000000, "end_hz": 152000000, "modes": ["nfm"],
        "channels": [{"name": "Mine", "centre_hz": 151000000, "width_hz": 12500}]}]}]}
        """
    #endif
}
