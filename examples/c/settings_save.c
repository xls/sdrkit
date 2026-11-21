/* settings_save: sets settings of the engine and of an FM session, exports every setting of the engine as bytes,
   writes them to a file, and imports the file into a second engine. Only the library reads the bytes.

   Usage: settings_save [source] [file]
     source  "synth:fm" (the default) is a test signal. A radio is an address such as "rtl_tcp:192.168.1.20:1234".
     file    the file for the bytes, "sdrkit_settings.bin" by default. */
#include <sdrkit.h>
#include <sdrkit/fm.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static SDRKIT_HANDLE engine;

/* Ends the program with the message of the engine if `code` is an error. */
static void must(int32_t code, const char *step) {
    sdrkit_string why = {"", 0};
    if (code == SDRKIT_OK) return;
    if (sdrkit_error_message(engine, &why) != SDRKIT_OK) why.len = 0;
    fprintf(stderr, "%s: %s. %.*s\n", step, sdrkit_code_name(code), (int)why.len, why.ptr);
    exit(1);
}

/* A view of the C string `s`. */
static sdrkit_string text(const char *s) {
    sdrkit_string v;
    v.ptr = s;
    v.len = strlen(s);
    return v;
}

/* Makes an engine, opens `source` and starts the stack "fm" on it. */
static SDRKIT_HANDLE start_fm(const char *source, SDRKIT_HANDLE *radio) {
    SDRKIT_HANDLE session;
    must(sdrkit_engine_new(NULL, &engine), "engine");
    must(sdrkit_radio_open(engine, source, NULL, radio), "radio");
    sdrkit_input input = sdrkit_input_radio(*radio);
    must(sdrkit_stack_start(engine, "fm", NULL, &input, &session), "start");
    return session;
}

/* Stops the session and frees the engine with every handle of it. */
static void finish(SDRKIT_HANDLE session) {
    must(sdrkit_session_stop(session), "stop");
    must(sdrkit_free(engine), "free");
}

int main(int argc, char **argv) {
    const char *source = argc > 1 ? argv[1] : "synth:fm";
    const char *path = argc > 2 ? argv[2] : "sdrkit_settings.bin";
    SDRKIT_HANDLE radio, session;

    /* The first engine: a setting of the engine and two of the stack, with their typed calls. */
    session = start_fm(source, &radio);
    must(sdrkit_engine_set_callsign(engine, text("N0CALL")), "callsign");
    must(sdrkit_fm_set_force_mono(session, true), "mono");
    must(sdrkit_fm_set_deemphasis(session, SDRKIT_FM_DEEMPHASIS_75_US), "deemphasis");

    /* The bytes are a view until the next call on the engine: write them out at once. */
    sdrkit_bytes blob;
    must(sdrkit_settings_export(engine, &blob), "export");
    FILE *f = fopen(path, "wb");
    if (!f || fwrite(blob.ptr, 1, blob.len, f) != blob.len || fclose(f) != 0) {
        fprintf(stderr, "cannot write %s\n", path);
        return 1;
    }
    printf("wrote %u bytes of settings to %s\n", (unsigned)blob.len, path);
    finish(session);

    /* The second engine starts with the defaults, then reads the file. */
    session = start_fm(source, &radio);
    f = fopen(path, "rb");
    if (!f) {
        fprintf(stderr, "cannot read %s\n", path);
        return 1;
    }
    static uint8_t data[1 << 20];
    size_t size = fread(data, 1, sizeof data, f);
    fclose(f);
    must(sdrkit_settings_import(engine, data, size), "import");

    sdrkit_string callsign;
    bool mono = false;
    sdrkit_fm_deemphasis deemphasis = SDRKIT_FM_DEEMPHASIS_50_US;
    must(sdrkit_engine_get_callsign(engine, &callsign), "callsign");
    printf("callsign %.*s\n", (int)callsign.len, callsign.ptr);
    must(sdrkit_fm_get_force_mono(session, &mono), "mono");
    must(sdrkit_fm_get_deemphasis(session, &deemphasis), "deemphasis");
    printf("force mono %s, deemphasis %s\n", mono ? "yes" : "no",
           deemphasis == SDRKIT_FM_DEEMPHASIS_75_US ? "75 us" : "not 75 us");
    int same = mono && deemphasis == SDRKIT_FM_DEEMPHASIS_75_US;
    finish(session);
    printf("%s\n", same ? "imported the saved settings" : "the saved settings did not come back");
    return same ? 0 : 1;
}
