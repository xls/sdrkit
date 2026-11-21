// fm_audio: the FM program of examples/c/fm_audio.c on a small C++17 wrapper. The wrapper owns each handle (RAII) and
// turns an SDRKIT_E_* code into an exception.
//
// Usage: fm_audio_cpp [source] [seconds] [MHz] [file]
//   source   "synth:fm" (the default) is a test signal. A radio is an address such as "rtl_tcp:192.168.1.20:1234".
//   seconds  how much audio to write, 5 by default.
//   MHz      the frequency to tune to. The test signal needs none.
//   file     the WAV file, "fm_audio_cpp.wav" by default.
#include <sdrkit.h>
#include <sdrkit/fm.h>

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <stdexcept>
#include <string>
#include <thread>
#include <utility>
#include <vector>

namespace sdrkit {

// An error of a call: its SDRKIT_E_* code and the sentence of the engine.
class error : public std::runtime_error {
  public:
    error(int32_t code, const std::string &what) : std::runtime_error(what), code_(code) {}
    int32_t code() const { return code_; }

  private:
    int32_t code_;
};

// Throws an error if `code` is not SDRKIT_OK. `any` is a handle of the engine, for its message.
inline void check(int32_t code, SDRKIT_HANDLE any, const char *step) {
    if (code == SDRKIT_OK) return;
    sdrkit_string why = {"", 0};
    if (sdrkit_error_message(any, &why) != SDRKIT_OK) why.len = 0;
    throw error(code, std::string(step) + ": " + sdrkit_code_name(code) + ". " + std::string(why.ptr, why.len));
}

inline std::string to_string(sdrkit_string s) { return std::string(s.ptr, s.len); }

// The engine. Freeing it stops every session and frees every handle of it.
class engine {
  public:
    explicit engine(const char *folder = nullptr) { check(sdrkit_engine_new(folder, &h_), 0, "engine"); }
    ~engine() {
        if (h_) sdrkit_free(h_);
    }
    engine(const engine &) = delete;
    engine &operator=(const engine &) = delete;
    SDRKIT_HANDLE get() const { return h_; }

  private:
    SDRKIT_HANDLE h_ = 0;
};

// An open radio, closed at the end of its scope.
class radio {
  public:
    radio(const engine &e, const char *address) { check(sdrkit_radio_open(e.get(), address, nullptr, &h_), e.get(), "radio"); }
    ~radio() {
        sdrkit_radio_close(h_);
        sdrkit_free(h_);
    }
    radio(const radio &) = delete;
    radio &operator=(const radio &) = delete;
    SDRKIT_HANDLE get() const { return h_; }

  private:
    SDRKIT_HANDLE h_ = 0;
};

// A session of a stack on a radio, stopped and closed at the end of its scope.
class session {
  public:
    session(const engine &e, const radio &r, const char *stack) {
        sdrkit_input input = sdrkit_input_radio(r.get());
        check(sdrkit_stack_start(e.get(), stack, nullptr, &input, &h_), e.get(), "start");
    }
    ~session() {
        sdrkit_session_stop(h_);
        sdrkit_session_close(h_);
        sdrkit_free(h_);
    }
    session(const session &) = delete;
    session &operator=(const session &) = delete;
    SDRKIT_HANDLE get() const { return h_; }

    // Copies the waiting audio into `out` and returns its format.
    sdrkit_audio_info audio(std::vector<float> &out) const {
        sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;
        check(sdrkit_audio_read(h_, out.data(), out.size(), &got), h_, "audio");
        return got;
    }

    // Takes one copy of the results; the typed reads below read that copy.
    void snapshot() const {
        sdrkit_snapshot_info info = SDRKIT_SNAPSHOT_INFO_INIT;
        check(sdrkit_snapshot_read(h_, 0, &info), h_, "snapshot");
    }

  private:
    SDRKIT_HANDLE h_ = 0;
};

} // namespace sdrkit

// Writes the 44-byte header of a 16-bit PCM WAV file.
static void wav_header(std::ofstream &f, uint32_t rate, uint32_t channels, uint32_t data_bytes) {
    auto put = [&f](uint32_t v, int bytes) {
        for (int i = 0; i < bytes; i++) f.put(static_cast<char>((v >> (8 * i)) & 0xff));
    };
    f.write("RIFF", 4);
    put(36 + data_bytes, 4);
    f.write("WAVEfmt ", 8);
    put(16, 4);
    put(1, 2);
    put(channels, 2);
    put(rate, 4);
    put(rate * channels * 2, 4);
    put(channels * 2, 2);
    put(16, 2);
    f.write("data", 4);
    put(data_bytes, 4);
}

int main(int argc, char **argv) {
    const char *source = argc > 1 ? argv[1] : "synth:fm";
    const double seconds = argc > 2 ? std::atof(argv[2]) : 5.0;
    const double mhz = argc > 3 ? std::atof(argv[3]) : 0.0;
    const std::string path = argc > 4 ? argv[4] : "fm_audio_cpp.wav";
    try {
        sdrkit::engine engine;
        sdrkit::radio radio(engine, source);
        sdrkit::session fm(engine, radio, "fm");
        if (mhz > 0) sdrkit::check(sdrkit_session_set_tuned_hz(fm.get(), mhz * 1e6), fm.get(), "frequency");

        std::ofstream wav(path, std::ios::binary);
        if (!wav) throw std::runtime_error("cannot write " + path);
        wav_header(wav, 0, 0, 0);

        std::vector<float> audio(16384);
        uint32_t rate = 0, channels = 0, data_bytes = 0;
        double frames = 0;
        const int passes = static_cast<int>(seconds * 150.0) + 50;
        for (int pass = 0; pass < passes && (rate == 0 || frames < seconds * rate); pass++) {
            const sdrkit_audio_info got = fm.audio(audio);
            if (got.frames > 0 && rate == 0) {
                rate = static_cast<uint32_t>(got.rate_hz);
                channels = got.channels;
            }
            for (size_t i = 0; i < got.frames * got.channels; i++) {
                const float s = audio[i] > 1.0f ? 1.0f : audio[i] < -1.0f ? -1.0f : audio[i];
                const auto v = static_cast<uint16_t>(static_cast<int16_t>(s * 32767.0f));
                wav.put(static_cast<char>(v & 0xff));
                wav.put(static_cast<char>(v >> 8));
            }
            data_bytes += static_cast<uint32_t>(got.frames * got.channels * 2);
            frames += static_cast<double>(got.frames);

            if (pass % 50 == 49) {
                fm.snapshot();
                sdrkit_fm_fields fields;
                sdrkit::check(sdrkit_fm_fields_read(fm.get(), &fields), fm.get(), "fields");
                std::printf("station \"%s\"  %s  SNR %.1f dB\n", sdrkit::to_string(fields.station).c_str(),
                            fields.stereo ? "stereo" : "mono", fields.snr_db);
                std::fflush(stdout);
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(20));
        }
        wav.seekp(0);
        wav_header(wav, rate, channels, data_bytes);
        std::printf("wrote %.1f s of audio to %s\n", rate ? frames / rate : 0.0, path.c_str());
        return frames > 0 ? 0 : 1;
    } catch (const sdrkit::error &e) {
        std::fprintf(stderr, "%s\n", e.what());
        return 1;
    } catch (const std::exception &e) {
        std::fprintf(stderr, "%s\n", e.what());
        return 1;
    }
}
