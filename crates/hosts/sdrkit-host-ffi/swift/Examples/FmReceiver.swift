// An FM receiver on the test signal `synth:fm`, with the typed Swift interface. It sets two settings, reads audio,
// prints the station, its alternative frequencies and each radio text, changes the frequency once, clears the
// results, and stops. Build it with the files of `swift/` and the module `SDRKitC` of the C header.

import Foundation
import SDRKitC

/// Throws the code of a call that failed.
func must(_ code: Int32) throws {
    if code != SDRKIT_OK { throw SDRKitCode(code: code) }
}

func fmReceiver() throws {
    var engine: SDRKIT_HANDLE = 0, radio: SDRKIT_HANDLE = 0, session: SDRKIT_HANDLE = 0
    try must(sdrkit_engine_new(nil, &engine))
    try must(sdrkit_radio_open(engine, "synth:fm", nil, &radio))
    var input = sdrkit_input_radio(radio)
    try must(sdrkit_stack_start(engine, "fm", nil, &input, &session))  // nil: the default frequency
    try FmSettings.setFmDeemphasis(session, .n50Us).get()
    try FmSettings.setFmForceMono(session, true).get()

    var audio = [Float](repeating: 0, count: 9600)
    // 0: wait for the station name, its alternative frequencies and its text. 1: wait for the carrier at the new
    // frequency.
    var frames = 0, phase = 0, texts = 0
    var retunes: Int64 = 0
    for pass in 0..<3000 where phase < 2 {
        var got = sdrkit_audio_info.sized
        try must(sdrkit_audio_read(session, &audio, audio.count, &got))  // play got.frames frames of got.channels
        frames += got.frames

        // One snapshot, then its parts as values.
        var snapshot = sdrkit_snapshot_info.sized
        try must(sdrkit_snapshot_read(session, 0, &snapshot))
        let counts = try SessionFields.read(session).get()
        let fm = try FmFields.read(session).get()
        let alternatives = try FmTables.fmAf(session).get()

        var events: UInt32 = 0
        try must(sdrkit_events_take(session, &events))
        for n in 0..<events {
            switch FmTextEvent.read(session, n) {
            case .success(let text):
                print("text at \(text.freqHz / 1e6) MHz: \(text.text)")
                texts += 1
            case .failure(let error) where error.code == SDRKIT_E_WRONG_TYPE: continue  // an event of another type
            case .failure(let error): throw error
            }
        }

        // The text needs every sample for some seconds: a build that drops samples goes on without it.
        if phase == 0, !fm.station.isEmpty, alternatives.count >= 2, texts > 0 || pass > 500 {
            let tuned = try SessionSettings.sessionTunedHz(session).get()
            let list = alternatives.map { String($0.freqHz / 1e6) }.joined(separator: " ")
            print("\(tuned / 1e6) MHz \(fm.station), \(fm.stereo ? "stereo" : "mono"), alternatives: \(list)")
            // A new frequency. The session keeps running, and the results of the stack start empty.
            retunes = counts.retunes
            try SessionSettings.setSessionTunedHz(session, tuned + 8e6).get()
            phase = 1
        } else if phase == 1, counts.retunes > retunes, fm.carrier {
            print("a carrier at the new frequency, station name \"\(fm.station)\"")
            try FmCommands.fmClear(session).get()
            phase = 2
        }
        Thread.sleep(forTimeInterval: 0.02)
    }
    print("\(frames) frames of audio")

    try must(sdrkit_session_stop(session))
    try must(sdrkit_session_close(session))
    try must(sdrkit_radio_close(radio))
    try must(sdrkit_free(engine))
}
