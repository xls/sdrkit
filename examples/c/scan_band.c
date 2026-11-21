/* scan_band: a channel scan of a stack over the channels that the presets part gives for a country. The scan service
   tunes the session from channel to channel, judges each by the lock profile of the stack, and lists what it found.

   Usage: scan_band [source] [seconds] [stack] [country]
     source   "synth:dab" (the default) is a test signal. A radio is an address such as "rtl_tcp:192.168.1.20:1234".
     seconds  the longest time to scan, 120 by default.
     stack    the stack to scan with, "dab" by default.
     country  an ISO 3166 code, "AU" by default. Its plans give the channels.

   The data folder is the environment variable SDRKIT_DATA, else SDRKIT_DATA_DIR given at build time. The scan service
   has no typed header: this program reaches it through the generic interface, by key. */
#ifndef _WIN32
#define _POSIX_C_SOURCE 200112L
#endif
#include <sdrkit.h>
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

/* Writes result field `key` of the last snapshot of `object` to `value`. */
static void field(SDRKIT_HANDLE object, const char *key, sdrkit_variant *value) {
    sdrkit_id id;
    must(sdrkit_field_find(object, key, &id), key);
    must(sdrkit_snapshot_field(object, id, value), key);
}

int main(int argc, char **argv) {
    const char *source = argc > 1 ? argv[1] : "synth:dab";
    double seconds = argc > 2 ? atof(argv[2]) : 120.0;
    const char *stack = argc > 3 ? argv[3] : "dab";
    const char *country = argc > 4 ? argv[4] : "AU";
    const char *data = getenv("SDRKIT_DATA") ? getenv("SDRKIT_DATA") : SDRKIT_DATA_DIR;
    SDRKIT_HANDLE radio, session, scan;

    must(sdrkit_engine_new(NULL, &engine), "engine");

    /* The plans of the country give the scan service the channels of each stack that they name. */
    const char *ids[64];
    char copies[64][64];
    uint32_t count = 0, stacks = 0;
    must(sdrkit_presets_open(data, &presets), "presets");
    must(sdrkit_presets_plan_ids(presets, country, &count), "plans");
    if (count > 64) count = 64;
    for (uint32_t i = 0; i < count; i++) {
        sdrkit_string id;
        must(sdrkit_presets_plan_id(presets, i, &id), "plan");
        size_t len = id.len < 63 ? id.len : 63;
        memcpy(copies[i], id.ptr, len);
        copies[i][len] = '\0';
        ids[i] = copies[i];
    }
    must(sdrkit_presets_scan_channels_set(presets, engine, ids, count, &stacks), "scan channels");
    printf("%u plans of %s give channels to %u stacks\n", count, country, stacks);

    must(sdrkit_radio_open(engine, source, NULL, &radio), "radio");
    sdrkit_input input = sdrkit_input_radio(radio);
    must(sdrkit_stack_start(engine, stack, NULL, &input, &session), "start");

    /* The command "start" of the service "service.scan" takes the key of the session and the stack id. The scanner
       knows the session a moment after its first samples, so the call is made again until it is taken. */
    sdrkit_string key;
    char session_key[64];
    must(sdrkit_object_key(session, &key), "session key");
    snprintf(session_key, sizeof session_key, "%.*s", (int)key.len, key.ptr);
    must(sdrkit_object_find(engine, "service.scan", &scan), "scan service");
    const char *args[] = {"session", session_key, "stack", stack};
    int32_t code = SDRKIT_E_NOT_NOW;
    for (int tries = 0; tries < 100 && code != SDRKIT_OK; tries++) {
        code = sdrkit_call(scan, "start", args, 2);
        if (code != SDRKIT_OK) pause_ms(100);
    }
    must(code, "scan");

    /* Progress from the result fields of the service, until its state is "done". */
    sdrkit_variant value;
    sdrkit_option option = SDRKIT_OPTION_INIT;
    sdrkit_id state_id;
    must(sdrkit_field_find(scan, "state", &state_id), "state");
    int done = 0;
    int64_t shown = -1;
    for (int pass = 0; pass < (int)(seconds * 5.0) && !done; pass++) {
        sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
        double hz;
        int64_t index, channels, found;
        must(sdrkit_snapshot_read(scan, 0, &snapshot), "snapshot");
        /* The option key is a view: it is compared before the next call. */
        field(scan, "state", &value);
        must(sdrkit_field_option(scan, state_id, value.enum_index, &option), "state");
        done = option.key.len == 4 && memcmp(option.key.ptr, "done", 4) == 0;
        field(scan, "freq_hz", &value);
        hz = value.number;
        field(scan, "index", &value);
        index = value.integer;
        field(scan, "count", &value);
        channels = value.integer;
        field(scan, "found", &value);
        found = value.integer;
        if (!done && channels > 0 && index != shown) {
            printf("channel %lld of %lld  %.3f MHz  found %lld\n", (long long)index + 1, (long long)channels,
                   hz / 1e6, (long long)found);
            fflush(stdout);
            shown = index;
        }
        pause_ms(200);
    }

    /* The hits: one row for each channel where the stack locked. Cells are read by column key. */
    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    sdrkit_rows_info rows = SDRKIT_ROWS_INFO_INIT;
    sdrkit_id hits, freq, name, locked;
    must(sdrkit_snapshot_read(scan, 0, &snapshot), "snapshot");
    must(sdrkit_table_find(scan, "hits", &hits), "hits");
    must(sdrkit_column_find(scan, hits, "freq_hz", &freq), "column");
    must(sdrkit_column_find(scan, hits, "name", &name), "column");
    must(sdrkit_column_find(scan, hits, "locked", &locked), "column");
    must(sdrkit_snapshot_table(scan, hits, &rows), "hits");
    for (uint32_t n = 0; n < rows.rows; n++) {
        double hz;
        must(sdrkit_snapshot_cell(scan, hits, n, freq, &value), "cell");
        hz = value.number;
        must(sdrkit_snapshot_cell(scan, hits, n, locked, &value), "cell");
        if (!value.flag) continue;
        must(sdrkit_snapshot_cell(scan, hits, n, name, &value), "cell");
        printf("locked at %.3f MHz %.*s\n", hz / 1e6, (int)value.string.len, value.string.ptr);
    }

    sdrkit_call(scan, "stop", NULL, 0);
    must(sdrkit_session_stop(session), "stop");
    must(sdrkit_free(engine), "free");
    sdrkit_presets_close(presets);
    printf("scan %s\n", done ? "done" : "stopped at the time limit");
    return 0;
}
