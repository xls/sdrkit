/* meshtastic: receives a Meshtastic mesh, prints each new text message, and lists the nodes it heard.

   Usage: meshtastic [source] [seconds] [MHz or country] [channel] [key]
     source   "synth:meshtastic" (the default) is a test signal. A radio is an address such as
              "rtl_tcp:192.168.1.20:1234".
     seconds  how long to listen, 20 by default.
     MHz      the frequency of the mesh. The test signal needs none.
     country  an ISO 3166 code such as "AU" in place of the frequency: the presets part gives the frequency and the
              settings of the stack in the country (its slots and channel lists).
     channel  the name of a private channel to add. Its key comes from the argument `key` or from the environment
              variable SDRKIT_MESHTASTIC_KEY, in base64 or hex. Without a channel the stack reads the primary channel
              with the published default key of the protocol.

   The data folder is the environment variable SDRKIT_DATA, else SDRKIT_DATA_DIR given at build time. */
#ifndef _WIN32
#define _POSIX_C_SOURCE 200112L
#endif
#include <sdrkit.h>
#include <sdrkit/lora.h>
#include <sdrkit_presets.h>
#include <ctype.h>
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

/* A view of the C string `s`. */
static sdrkit_string text(const char *s) {
    sdrkit_string v;
    v.ptr = s;
    v.len = strlen(s);
    return v;
}

/* Writes the frequency that the data has for the stack in `country` to `options` with the settings of the stack
   there, through `args`. Their views are valid until the next presets call. */
static void regional(const char *country, sdrkit_stack_options *options, sdrkit_arg *args, uint32_t cap) {
    const char *data = getenv("SDRKIT_DATA") ? getenv("SDRKIT_DATA") : SDRKIT_DATA_DIR;
    sdrkit_presets_query query = SDRKIT_PRESETS_QUERY_INIT;
    sdrkit_presets_dial_info dial = SDRKIT_PRESETS_DIAL_INFO_INIT;
    sdrkit_presets_setting setting = SDRKIT_PRESETS_SETTING_INIT;
    uint32_t count = 0;
    query.stack = "meshtastic";
    query.country = country;
    must(sdrkit_presets_open(data, &presets), "presets");
    must(sdrkit_presets_dial(presets, &query, &dial), "dial");
    must(sdrkit_presets_settings(presets, engine, &query, &count), "settings");
    if (count > cap) count = cap;
    for (uint32_t i = 0; i < count; i++) {
        must(sdrkit_presets_setting_read(presets, i, &setting), "setting");
        args[i] = setting.arg;
    }
    options->centre_hz = dial.frequency_hz;
    options->settings = args;
    options->count = count;
    printf("%s: %.3f MHz (%.*s), %u settings of the data\n", country, dial.frequency_hz / 1e6, (int)dial.plan.len,
           dial.plan.ptr, (unsigned)count);
}

int main(int argc, char **argv) {
    const char *source = argc > 1 ? argv[1] : "synth:meshtastic";
    double seconds = argc > 2 ? atof(argv[2]) : 20.0;
    const char *where = argc > 3 ? argv[3] : "0";
    const char *channel = argc > 4 ? argv[4] : NULL;
    const char *key = argc > 5 ? argv[5] : getenv("SDRKIT_MESHTASTIC_KEY");
    SDRKIT_HANDLE radio, mesh;

    sdrkit_arg settings[32];
    sdrkit_stack_options options = SDRKIT_STACK_OPTIONS_INIT;
    must(sdrkit_engine_new(NULL, &engine), "engine");
    if (isalpha((unsigned char)where[0]))
        regional(where, &options, settings, 32);
    else
        options.centre_hz = atof(where) * 1e6; /* 0: the frequency of the input */
    must(sdrkit_radio_open(engine, source, NULL, &radio), "radio");
    sdrkit_input input = sdrkit_input_radio(radio);
    must(sdrkit_stack_start(engine, "meshtastic", &options, &input, &mesh), "start");

    /* A private channel: the typed command "mt.channel.add" with its arguments as a struct. */
    if (channel != NULL) {
        if (key == NULL) {
            fprintf(stderr, "the channel %s needs a key: give it as an argument or in SDRKIT_MESHTASTIC_KEY\n", channel);
            return 1;
        }
        sdrkit_lora_mt_channel_add_args add = SDRKIT_LORA_MT_CHANNEL_ADD_ARGS_INIT;
        add.name = text(channel);
        add.has_key = true;
        add.key = text(key);
        must(sdrkit_meshtastic_mt_channel_add(mesh, &add), "channel");
    }

    static sdrkit_mesh_message_row messages[64];
    static sdrkit_mesh_node_row nodes[256];
    uint64_t since = 0;
    int passes = (int)(seconds * 5.0);
    for (int pass = 0; pass < passes; pass++) {
        /* Only the rows that changed after the counter of the last snapshot. */
        sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
        sdrkit_rows_info listed = SDRKIT_ROWS_INFO_INIT;
        must(sdrkit_snapshot_read(mesh, since, &snapshot), "snapshot");
        since = snapshot.counter;
        must(sdrkit_meshtastic_messages_rows(mesh, messages, 64, &listed), "messages");
        for (uint32_t n = 0; n < listed.rows && n < 64; n++) {
            const sdrkit_mesh_message_row *m = &messages[n];
            const sdrkit_string from = m->has_sender ? m->sender : m->has_source ? m->source : text("?");
            printf("%.*s: %.*s\n", (int)from.len, from.ptr, (int)m->text.len, m->text.ptr);
        }
        fflush(stdout);
        pause_ms(200);
    }

    /* Every node of the table: a snapshot with the counter 0 holds every row. */
    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    sdrkit_rows_info listed = SDRKIT_ROWS_INFO_INIT;
    must(sdrkit_snapshot_read(mesh, 0, &snapshot), "snapshot");
    must(sdrkit_meshtastic_nodes_rows(mesh, nodes, 256, &listed), "nodes");
    for (uint32_t n = 0; n < listed.rows && n < 256; n++) {
        const sdrkit_mesh_node_row *node = &nodes[n];
        printf("node %.*s %.*s", (int)node->id.len, node->id.ptr, (int)node->name.len, node->name.ptr);
        if (node->has_snr_db) printf("  SNR %.1f dB", node->snr_db);
        printf("\n");
    }

    must(sdrkit_session_stop(mesh), "stop");
    must(sdrkit_free(engine), "free");
    if (presets != 0) sdrkit_presets_close(presets);
    printf("%u nodes heard\n", listed.total);
    return 0;
}
