/* The structs that begin with `size`. A program built with an older header passes a shorter struct, and a program
   built with a newer header passes a longer struct whose new members are 0. Both get correct results, and a `size`
   below 4 is refused. tests/c_api.rs builds and runs it. */
#include "c_check.h"
#include <stddef.h>

/* sdrkit_stack_options of an older header: it ends before `centre_hz`. */
typedef struct {
    uint32_t size;
    const char *submode;
    const sdrkit_arg *settings;
    size_t count;
} older_stack_options;

/* sdrkit_stack_options of a newer header: one more member at the end. */
typedef struct {
    sdrkit_stack_options known;
    double added[4];
} newer_stack_options;

/* sdrkit_audio_info of a newer header. */
typedef struct {
    sdrkit_audio_info known;
    uint64_t added[2];
} newer_audio_info;

/* sdrkit_decl of a newer header. */
typedef struct {
    sdrkit_decl known;
    uint64_t added[2];
} newer_decl;

/* Reads audio until `info` holds frames, for about 20 s at most. `info` keeps the size that the caller set. */
static int32_t audio(SDRKIT_HANDLE session, sdrkit_audio_info *info, uint32_t size) {
    static float buffer[4800];
    for (int i = 0; i < 2000; i++) {
        info->size = size;
        info->frames = 0;
        int32_t code = sdrkit_audio_read(session, buffer, 4800, info);
        if (code != SDRKIT_OK || info->frames > 0) return code;
        pause_a_moment();
    }
    return SDRKIT_OK;
}

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0, radio = 0, session = 0, stack = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_radio_open(engine, "synth:fm", NULL, &radio));
    OK(sdrkit_object_find(engine, "stack.fm", &stack));

    /* An input and options that the caller passes. Shorter: the members after `size` bytes take their defaults. */
    sdrkit_input input = sdrkit_input_radio(radio);
    CHECK(input.size == sizeof(sdrkit_input));
    older_stack_options older = {sizeof(older_stack_options), NULL, NULL, 0};
    OK(sdrkit_stack_start(engine, "fm", (const sdrkit_stack_options *)&older, &input, &session));
    double tuned = 0, centre = 0;
    OK(sdrkit_session_get_tuned_hz(session, &tuned));
    CHECK(tuned > 0);
    OK(sdrkit_session_stop(session));
    OK(sdrkit_session_close(session));

    /* Longer: the library reads the members that it knows. */
    OK(sdrkit_radio_get_centre_hz(radio, &centre));
    newer_stack_options newer = {SDRKIT_STACK_OPTIONS_INIT, {0}};
    newer.known.size = sizeof(newer);
    newer.known.centre_hz = centre + 100e3;
    OK(sdrkit_stack_start(engine, "fm", &newer.known, &input, &session));
    OK(sdrkit_session_get_tuned_hz(session, &tuned));
    CHECK(tuned == centre + 100e3);

    /* A struct that the library fills. Shorter: the library writes `size` bytes and no more. */
    union {
        sdrkit_audio_info info;
        unsigned char bytes[sizeof(sdrkit_audio_info)];
    } part;
    memset(part.bytes, 0xEE, sizeof(part));
    uint32_t until_rate = (uint32_t)offsetof(sdrkit_audio_info, rate_hz);
    OK(audio(session, &part.info, until_rate));
    CHECK(part.info.size == until_rate && part.info.frames > 0);
    for (size_t i = until_rate; i < sizeof(part); i++) CHECK(part.bytes[i] == 0xEE);

    /* Longer: the library writes the members that it knows, says so in `size`, and leaves the rest. */
    newer_audio_info wide = {SDRKIT_AUDIO_INFO_INIT, {7, 7}};
    OK(audio(session, &wide.known, sizeof(wide)));
    CHECK(wide.known.size == sizeof(sdrkit_audio_info));
    CHECK(wide.known.frames > 0 && wide.known.rate_hz > 0 && wide.known.channels > 0);
    CHECK(wide.added[0] == 7 && wide.added[1] == 7);

    /* The declaration of a setting, shorter and longer. */
    sdrkit_decl whole = SDRKIT_DECL_INIT, cut = SDRKIT_DECL_INIT;
    OK(sdrkit_setting_decl(stack, 0, &whole));
    CHECK(whole.size == sizeof(sdrkit_decl) && whole.key.len > 0 && whole.label.len > 0);
    cut.size = (uint32_t)offsetof(sdrkit_decl, label);
    OK(sdrkit_setting_decl(stack, 0, &cut));
    CHECK(cut.size == offsetof(sdrkit_decl, label));
    CHECK(cut.key.len == whole.key.len && cut.label.len == 0);
    newer_decl long_decl = {SDRKIT_DECL_INIT, {7, 7}};
    long_decl.known.size = sizeof(long_decl);
    OK(sdrkit_setting_decl(stack, 0, &long_decl.known));
    CHECK(long_decl.known.size == sizeof(sdrkit_decl) && long_decl.known.type == whole.type);
    CHECK(long_decl.added[0] == 7 && long_decl.added[1] == 7);

    /* The detail of an error, shorter. */
    sdrkit_error_info detail = SDRKIT_ERROR_INFO_INIT;
    detail.size = (uint32_t)offsetof(sdrkit_error_info, setting);
    CHECK(sdrkit_setting_decl(stack, 100000, &whole) == SDRKIT_E_NO_SUCH_ID);
    OK(sdrkit_error_detail(stack, &detail));
    CHECK(detail.code == SDRKIT_E_NO_SUCH_ID && detail.size == offsetof(sdrkit_error_info, setting));

    /* A size that does not hold the `size` member is refused, and the call does nothing. */
    sdrkit_audio_info bad = SDRKIT_AUDIO_INFO_INIT;
    float one[2];
    bad.size = 0;
    CHECK(sdrkit_audio_read(session, one, 1, &bad) == SDRKIT_E_BAD_ARGUMENT);
    bad.size = 3;
    CHECK(sdrkit_audio_read(session, one, 1, &bad) == SDRKIT_E_BAD_ARGUMENT);
    cut.size = 0;
    CHECK(sdrkit_setting_decl(stack, 0, &cut) == SDRKIT_E_BAD_ARGUMENT);
    sdrkit_stack_options none = SDRKIT_STACK_OPTIONS_INIT;
    SDRKIT_HANDLE refused = 0;
    none.size = 3;
    CHECK(sdrkit_stack_start(engine, "fm", &none, &input, &refused) == SDRKIT_E_BAD_ARGUMENT);
    input.size = 0;
    CHECK(sdrkit_stack_start(engine, "fm", NULL, &input, &refused) == SDRKIT_E_BAD_ARGUMENT);
    CHECK(refused == 0);

    OK(sdrkit_session_stop(session));
    OK(sdrkit_free(engine));
    printf("c_sized ok\n");
    return 0;
}
