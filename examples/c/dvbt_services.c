/* dvbt_services: receives a DVB-T multiplex, waits for lock, lists its services, selects one and pulls its audio.
   The channel and the settings of the stack for it (its width) come from the presets part for a country. Video is not shown: sdrkit_media_read gives
   the video packets to a host that has a decoder for them.

   Usage: dvbt_services [source] [seconds] [country channel] [service]
     source   "synth:dvbt" (the default) is a test signal. A radio is an address such as "rtl_tcp:192.168.1.20:1234".
     seconds  how long to wait for lock and services, and then how long to pull audio; 30 by default.
     country  an ISO 3166 code such as "AU", with the channel name of its television plan, such as "28".
              The test signal needs neither.
     service  the name of the service to select. The default is the first one that can play.

   The data folder is the environment variable SDRKIT_DATA, else SDRKIT_DATA_DIR given at build time. */
#ifndef _WIN32
#define _POSIX_C_SOURCE 200112L
#endif
#include <sdrkit.h>
#include <sdrkit/dvbt.h>
#include <sdrkit_presets.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef SDRKIT_DATA_DIR
#define SDRKIT_DATA_DIR "data"
#endif

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
static void pause_ms(unsigned ms) { Sleep(ms); }
#else
#include <time.h>
static void pause_ms(unsigned ms) {
    struct timespec t;
    t.tv_sec = 0;
    t.tv_nsec = (long)ms * 1000000L;
    nanosleep(&t, NULL);
}
#endif

static SDRKIT_HANDLE engine, presets;

/* Ends the program with the message of the engine or of the presets if `code` is an error. */
static void must(int32_t code, const char *step) {
    sdrkit_string why = {"", 0};
    if (code == SDRKIT_OK) return;
    if (sdrkit_error_message(engine, &why) != SDRKIT_OK) why.len = 0;
    if (why.len == 0 && sdrkit_presets_error_message(presets, &why) != SDRKIT_OK) why.len = 0;
    fprintf(stderr, "%s: %s. %.*s\n", step, sdrkit_code_name(code), (int)why.len, why.ptr);
    exit(1);
}

/* Writes the centre of the channel `name` that the data has for DVB-T in `country` to `centre_hz`, and the settings
   of the stack for that channel (the width, the channel list) to `args`. Their views are valid until the next
   presets call. Returns the number of settings. */
static uint32_t channel_of(const char *country, const char *name, double *centre_hz, sdrkit_arg *args, uint32_t cap) {
    const char *data = getenv("SDRKIT_DATA") ? getenv("SDRKIT_DATA") : SDRKIT_DATA_DIR;
    sdrkit_presets_query query = SDRKIT_PRESETS_QUERY_INIT;
    sdrkit_presets_channel channel = SDRKIT_PRESETS_CHANNEL_INIT;
    sdrkit_presets_setting setting = SDRKIT_PRESETS_SETTING_INIT;
    uint32_t count = 0, i;
    query.stack = "dvbt";
    query.country = country;
    must(sdrkit_presets_open(data, &presets), "presets");
    must(sdrkit_presets_channels(presets, &query, &count), "channels");
    for (i = 0; i < count; i++) {
        must(sdrkit_presets_channel_read(presets, i, &channel), "channel");
        if (channel.label.len == strlen(name) && memcmp(channel.label.ptr, name, channel.label.len) == 0) break;
    }
    if (i == count) {
        fprintf(stderr, "%s has no DVB-T channel %s\n", country, name);
        exit(1);
    }
    *centre_hz = channel.frequency_hz;
    query.has_frequency_hz = true;
    query.frequency_hz = channel.frequency_hz;
    must(sdrkit_presets_settings(presets, engine, &query, &count), "settings");
    if (count > cap) count = cap;
    for (i = 0; i < count; i++) {
        must(sdrkit_presets_setting_read(presets, i, &setting), "setting");
        args[i] = setting.arg;
    }
    return count;
}

int main(int argc, char **argv) {
    const char *source = argc > 1 ? argv[1] : "synth:dvbt";
    double seconds = argc > 2 ? atof(argv[2]) : 30.0;
    const char *country = argc > 4 ? argv[3] : NULL;
    const char *channel = argc > 4 ? argv[4] : NULL;
    const char *wanted = argc > 5 ? argv[5] : NULL;
    SDRKIT_HANDLE radio, session;

    sdrkit_stack_options options = SDRKIT_STACK_OPTIONS_INIT;
    sdrkit_arg settings[16];
    must(sdrkit_engine_new(NULL, &engine), "engine");
    if (country != NULL) {
        options.count = channel_of(country, channel, &options.centre_hz, settings, 16);
        options.settings = settings;
        printf("channel %s of %s: %.3f MHz, %u settings of the data\n", channel, country, options.centre_hz / 1e6,
               (unsigned)options.count);
    }

    must(sdrkit_radio_open(engine, source, NULL, &radio), "radio");
    sdrkit_input input = sdrkit_input_radio(radio);
    must(sdrkit_stack_start(engine, "dvbt", &options, &input, &session), "start");

    /* Waits for lock and for the list of services. The snapshot state is the same for every stack. */
    static sdrkit_mpegts_ts_service_row services[64];
    sdrkit_rows_info listed = SDRKIT_ROWS_INFO_INIT;
    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    int waited = 0;
    for (; waited < (int)(seconds * 10.0); waited++) {
        must(sdrkit_snapshot_read(session, 0, &snapshot), "snapshot");
        must(sdrkit_dvbt_services_rows(session, services, 64, &listed), "services");
        if (snapshot.state == SDRKIT_STATE_LOCKED && listed.rows > 0) break;
        pause_ms(100);
    }
    if (listed.rows == 0) {
        printf("no services after %.0f s: %.*s\n", seconds, (int)snapshot.summary.len, snapshot.summary.ptr);
        return 1;
    }
    printf("locked after %.1f s, %u services\n", waited / 10.0, listed.rows);

    /* The rows are views until the next call on the session; the selection below is that call. */
    int chosen = -1;
    for (uint32_t n = 0; n < listed.rows && n < 64; n++) {
        const sdrkit_mpegts_ts_service_row *s = &services[n];
        printf("  %3lld  %-24.*s  service %lld%s\n", (long long)s->lcn, (int)s->name.len, s->name.ptr,
               (long long)s->service_id, s->scrambled ? "  scrambled" : "");
        if (chosen < 0 && wanted != NULL && s->name.len == strlen(wanted) && memcmp(s->name.ptr, wanted, s->name.len) == 0)
            chosen = (int)n;
    }
    sdrkit_dvbt_service_select_args select = SDRKIT_DVBT_SERVICE_SELECT_ARGS_INIT;
    if (chosen >= 0) select.row = services[chosen].key; /* else empty: the first service that can play */
    must(sdrkit_dvbt_service_select(session, &select), "select");

    /* The decoded audio of the selected service, as interleaved float frames. */
    static float audio[16384];
    sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;
    double frames = 0, rate = 0;
    for (int pass = 0; pass < (int)(seconds * 50.0); pass++) {
        must(sdrkit_audio_read(session, audio, sizeof audio / sizeof audio[0], &got), "audio");
        if (got.frames > 0) rate = got.rate_hz;
        frames += (double)got.frames;
        pause_ms(20);
    }

    must(sdrkit_session_stop(session), "stop");
    must(sdrkit_free(engine), "free");
    if (presets) sdrkit_presets_close(presets);
    printf("%.1f s of audio at %.0f Hz\n", rate > 0 ? frames / rate : 0.0, rate);
    return 0;
}
