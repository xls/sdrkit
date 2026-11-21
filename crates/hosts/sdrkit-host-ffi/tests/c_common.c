/* The values of a setting that people normally use: the filter widths of the general stack follow its demodulator. The
   content of a command that sends a carrier. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"

/* The option index of the option `key` of setting `id` of `h`, or -1. */
static int option_of(SDRKIT_HANDLE h, sdrkit_id id, const char *key) {
    sdrkit_decl d = SDRKIT_DECL_INIT;
    if (sdrkit_setting_decl(h, id, &d) != SDRKIT_OK) return -1;
    for (uint32_t n = 0; n < d.options; n++) {
        sdrkit_option o = SDRKIT_OPTION_INIT;
        if (sdrkit_setting_option(h, id, n, &o) != SDRKIT_OK) return -1;
        if (string_is(o.key, key)) return (int)n;
    }
    return -1;
}

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0, kind = 0, radio = 0, session = 0;
    OK(sdrkit_engine_new(argv[1], &engine));

    /* The type of the stack has no values: the list is the one of the default demodulator, NFM. */
    OK(sdrkit_object_find(engine, "stack.general", &kind));
    sdrkit_id width;
    OK(sdrkit_setting_find(kind, "width", &width));
    sdrkit_decl d = SDRKIT_DECL_INIT;
    OK(sdrkit_setting_decl(kind, width, &d));
    CHECK(d.common == 3);
    double values[8] = {0};
    uint32_t count = 0;
    OK(sdrkit_setting_common(kind, width, values, 8, &count));
    CHECK(count == 3 && values[0] == 8000.0 && values[1] == 12500.0 && values[2] == 25000.0);
    /* A short buffer gets the first values and the whole count; no buffer gets the count. */
    values[1] = 0.0;
    OK(sdrkit_setting_common(kind, width, values, 1, &count));
    CHECK(count == 3 && values[0] == 8000.0 && values[1] == 0.0);
    OK(sdrkit_setting_common(kind, width, NULL, 0, &count));
    CHECK(count == 3);
    CHECK(sdrkit_setting_common(kind, width, NULL, 1, &count) == SDRKIT_E_NULL_POINTER);
    CHECK(sdrkit_setting_common(kind, 9999, values, 8, &count) == SDRKIT_E_NO_SUCH_ID);
    /* The tune command of the stack sends an unmodulated carrier. */
    sdrkit_id tune;
    OK(sdrkit_command_find(kind, "tune", &tune));
    sdrkit_command_info info = SDRKIT_COMMAND_INFO_INIT;
    OK(sdrkit_command_decl(kind, tune, &info));
    CHECK(info.transmits && info.content == SDRKIT_TX_CONTENT_CARRIER);
    /* A setting without common values has none. */
    sdrkit_id gain;
    OK(sdrkit_setting_find(kind, "gain", &gain));
    OK(sdrkit_setting_decl(kind, gain, &d));
    CHECK(d.common == 0);

    /* A session: the list follows the demodulator that is set. */
    OK(sdrkit_radio_open(engine, "synth:general", NULL, &radio));
    sdrkit_input input = sdrkit_input_radio(radio);
    OK(sdrkit_stack_start(engine, "general", NULL, &input, &session));
    sdrkit_id demod;
    OK(sdrkit_setting_find(session, "width", &width));
    OK(sdrkit_setting_find(session, "demod", &demod));
    int cw = option_of(session, demod, "cw");
    CHECK(cw >= 0);
    sdrkit_variant v = {0};
    v.type = SDRKIT_TYPE_ENUM;
    v.enum_index = (uint32_t)cw;
    OK(sdrkit_setting_set(session, demod, &v));
    OK(sdrkit_setting_decl(session, width, &d));
    CHECK(d.common == 4);
    OK(sdrkit_setting_common(session, width, values, 8, &count));
    CHECK(count == 4 && values[0] == 50.0 && values[3] == 500.0);

    OK(sdrkit_session_stop(session));
    OK(sdrkit_free(engine));
    printf("c_common ok\n");
    return 0;
}
