/* dvbt_play_sdl: receives a DVB-T multiplex, waits for lock, selects the first service that can play, and plays its
   audio through SDL 3. Video is not shown: sdrkit_media_read gives the video packets to a host that has a decoder for
   them. SDL is a dependency of this program only, not of the library.

   Usage: dvbt_play_sdl [source] [seconds] [MHz]
     source   "synth:dvbt" (the default) is a test signal. A radio is an address such as "rtl_tcp:192.168.1.20:1234".
     seconds  how long to play, 60 by default.
     MHz      the centre of the multiplex. The test signal needs none. dvbt_services finds it from a channel name. */
#include <sdrkit.h>
#include <sdrkit/dvbt.h>
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
    const char *source = argc > 1 ? argv[1] : "synth:dvbt";
    double seconds = argc > 2 ? atof(argv[2]) : 60.0;
    double mhz = argc > 3 ? atof(argv[3]) : 0.0;
    SDRKIT_HANDLE radio, session;

    if (!SDL_Init(SDL_INIT_AUDIO)) {
        fprintf(stderr, "SDL: %s\n", SDL_GetError());
        return 1;
    }
    must(sdrkit_engine_new(NULL, &engine), "engine");
    must(sdrkit_radio_open(engine, source, NULL, &radio), "radio");
    sdrkit_input input = sdrkit_input_radio(radio);
    sdrkit_stack_options options = SDRKIT_STACK_OPTIONS_INIT;
    options.centre_hz = mhz * 1e6;
    must(sdrkit_stack_start(engine, "dvbt", &options, &input, &session), "start");

    /* Waits up to 30 s for lock and a service, then selects the first service that can play (an empty row key). */
    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    sdrkit_mpegts_ts_service_row services[8];
    sdrkit_rows_info listed = SDRKIT_ROWS_INFO_INIT;
    for (int wait = 0; wait < 300 && !(snapshot.state == SDRKIT_STATE_LOCKED && listed.total > 0); wait++) {
        must(sdrkit_snapshot_read(session, 0, &snapshot), "snapshot");
        must(sdrkit_dvbt_services_rows(session, services, 8, &listed), "services");
        SDL_Delay(100);
    }
    if (listed.total == 0) {
        printf("no services: %.*s\n", (int)snapshot.summary.len, snapshot.summary.ptr);
        return 1;
    }
    printf("playing %.*s\n", (int)services[0].name.len, services[0].name.ptr);
    sdrkit_dvbt_service_select_args select = SDRKIT_DVBT_SERVICE_SELECT_ARGS_INIT;
    must(sdrkit_dvbt_service_select(session, &select), "select");

    static float audio[16384];
    sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;
    SDL_AudioStream *stream = NULL;
    for (int pass = 0; pass < (int)(seconds * 50.0); pass++) {
        must(sdrkit_audio_read(session, audio, sizeof audio / sizeof audio[0], &got), "audio");
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
        SDL_Delay(20);
    }

    if (stream) SDL_DestroyAudioStream(stream);
    SDL_Quit();
    must(sdrkit_session_stop(session), "stop");
    must(sdrkit_free(engine), "free");
    return 0;
}
