/* adsb_table: receives ADS-B and prints each aircraft row of the stack when it changes, as a typed struct.

   Usage: adsb_table [source] [seconds]
     source   "synth:adsb" (the default) is a test signal. A radio is an address such as "rtl_tcp:192.168.1.20:1234".
     seconds  how long to run, 10 by default. */
#ifndef _WIN32
#define _POSIX_C_SOURCE 200112L
#endif
#include <sdrkit.h>
#include <sdrkit/adsb.h>
#include <stdio.h>
#include <stdlib.h>

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
    const char *source = argc > 1 ? argv[1] : "synth:adsb";
    double seconds = argc > 2 ? atof(argv[2]) : 10.0;
    SDRKIT_HANDLE radio, session;

    must(sdrkit_engine_new(NULL, &engine), "engine");
    must(sdrkit_radio_open(engine, source, NULL, &radio), "radio");
    sdrkit_input input = sdrkit_input_radio(radio);
    /* NULL options: the defaults of the stack, on its standard frequency of 1090 MHz. */
    must(sdrkit_stack_start(engine, "adsb", NULL, &input, &session), "start");

    static sdrkit_adsb_aircraft_row rows[64];
    uint64_t since = 0;
    uint32_t aircraft = 0;
    int passes = (int)(seconds * 5.0);
    for (int pass = 0; pass < passes; pass++) {
        /* A snapshot with the change counter of the last one holds only the rows that changed since then. */
        sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
        sdrkit_rows_info listed = SDRKIT_ROWS_INFO_INIT;
        must(sdrkit_snapshot_read(session, since, &snapshot), "snapshot");
        since = snapshot.counter;
        must(sdrkit_adsb_aircraft_rows(session, rows, 64, &listed), "aircraft");
        aircraft = listed.total;

        /* A member with a `has_` member before it can be absent. Strings are views until the next call. */
        for (uint32_t n = 0; n < listed.rows && n < 64; n++) {
            const sdrkit_adsb_aircraft_row *a = &rows[n];
            printf("%.*s", (int)a->icao.len, a->icao.ptr);
            if (a->has_callsign) printf("  %.*s", (int)a->callsign.len, a->callsign.ptr);
            if (a->has_alt_ft) printf("  %lld ft", (long long)a->alt_ft);
            if (a->has_speed_kt) printf("  %.0f kt", a->speed_kt);
            if (a->has_position) printf("  %.4f %.4f", a->position.lat, a->position.lon);
            printf("\n");
        }
        fflush(stdout);
        pause_ms(200);
    }

    must(sdrkit_session_stop(session), "stop");
    must(sdrkit_session_close(session), "close the session");
    must(sdrkit_radio_close(radio), "close the radio");
    must(sdrkit_free(engine), "free");
    printf("%u aircraft in the table\n", aircraft);
    return 0;
}
