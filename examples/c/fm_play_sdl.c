/* fm_play_sdl: receives FM and plays the audio through SDL 3. It prints the station name from RDS, stereo and the
   signal quality once a second. SDL is a dependency of this program only, not of the library.

   Usage: fm_play_sdl [source] [seconds] [MHz]
     source   "synth:fm" (the default) is a test signal. A radio is an address such as "rtl_tcp:192.168.1.20:1234".
     seconds  how long to play, 30 by default.
     MHz      the frequency to tune to. The test signal needs none. */
#include <sdrkit.h>
#include <sdrkit/fm.h>
#include <SDL3/SDL.h>
#include <stdio.h>
#include <stdlib.h>

static SDRKIT_HANDLE engine;

/* Ends the program with the message of the engine if `code` is an error. */
static void must(int32_t code, const char *step) {
    sdrkit_string why = {"", 0};
    if (code == SDRKIT_OK) return;
    if (sdrkit_error_message(engine, &why) != SDRKIT_OK) why.len = 0;
    fprintf(stderr, "%s: %s. %.*s\n", step, sdrkit_code_name(code), (int)why.len, why.ptr);
    exit(1);
}

int main(int argc, char **argv) {
    const char *source = argc > 1 ? argv[1] : "synth:fm";
    double seconds = argc > 2 ? atof(argv[2]) : 30.0;
    double mhz = argc > 3 ? atof(argv[3]) : 0.0;
    SDRKIT_HANDLE radio, session;

    if (!SDL_Init(SDL_INIT_AUDIO)) {
        fprintf(stderr, "SDL: %s\n", SDL_GetError());
        return 1;
    }
    must(sdrkit_engine_new(NULL, &engine), "engine");
    must(sdrkit_radio_open(engine, source, NULL, &radio), "radio");
    sdrkit_input input = sdrkit_input_radio(radio);
    must(sdrkit_stack_start(engine, "fm", NULL, &input, &session), "start");
    if (mhz > 0) must(sdrkit_session_set_tuned_hz(session, mhz * 1e6), "frequency");

    static float audio[16384];
    sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;
    SDL_AudioStream *stream = NULL;
    for (int pass = 0; pass < (int)(seconds * 50.0); pass++) {
        must(sdrkit_audio_read(session, audio, sizeof audio / sizeof audio[0], &got), "audio");
        /* The stream opens with the format of the first audio: float samples, interleaved frames. */
        if (got.frames > 0 && stream == NULL) {
            SDL_AudioSpec spec;
            spec.format = SDL_AUDIO_F32;
            spec.channels = got.channels;
            spec.freq = (int)got.rate_hz;
            stream = SDL_OpenAudioDeviceStream(SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK, &spec, NULL, NULL);
            if (stream == NULL) {
                fprintf(stderr, "SDL: %s\n", SDL_GetError());
                return 1;
            }
            SDL_ResumeAudioStreamDevice(stream);
        }
        if (got.frames > 0) SDL_PutAudioStreamData(stream, audio, (int)(got.frames * got.channels * sizeof(float)));

        if (pass % 50 == 49) {
            sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
            sdrkit_fm_fields fm;
            must(sdrkit_snapshot_read(session, 0, &snapshot), "snapshot");
            must(sdrkit_fm_fields_read(session, &fm), "fields");
            printf("station \"%.*s\"  %s  SNR %.1f dB\n", (int)fm.station.len, fm.station.ptr,
                   fm.stereo ? "stereo" : "mono", fm.snr_db);
            fflush(stdout);
        }
        SDL_Delay(20);
    }

    if (stream) SDL_DestroyAudioStream(stream);
    SDL_Quit();
    must(sdrkit_session_stop(session), "stop");
    must(sdrkit_free(engine), "free");
    return 0;
}
