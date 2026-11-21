/* The calls by name: start a stack on a radio, set a setting and run a command by key with text values, read an event
   field as text. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"

/* Returns true if `s` contains the C string `c`. */
static int has(sdrkit_string s, const char *c) {
    size_t n = strlen(c);
    for (size_t i = 0; i + n <= s.len; i++)
        if (memcmp(s.ptr + i, c, n) == 0) return 1;
    return 0;
}

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0, radio = 0, session = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_radio_open(engine, "synth:fm", NULL, &radio));
    OK(sdrkit_start(radio, "fm", &session));
    CHECK(sdrkit_start(radio, "no such stack", &session) != SDRKIT_OK);

    /* A value is text. A refused value says which setting and what range. */
    OK(sdrkit_set(session, "bass", "3"));
    OK(sdrkit_set(session, "warm", "yes"));
    CHECK(sdrkit_set(session, "bass", "10") == SDRKIT_E_OUT_OF_RANGE);
    sdrkit_string m;
    OK(sdrkit_error_message(session, &m));
    CHECK(has(m, "\"Bass\" (bass)") && has(m, "-6 to 6") && !has(m, "Id("));
    CHECK(sdrkit_set(session, "bass", "loud") == SDRKIT_E_MALFORMED);
    CHECK(sdrkit_set(session, "no_such_setting", "1") == SDRKIT_E_NO_SUCH_KEY);

    /* The radio reports its connection as an event. A field that an event does not have reads as an empty text. */
    int connected = 0;
    for (int tries = 0; tries < 1000 && !connected; tries++) {
        uint32_t n = 0;
        OK(sdrkit_events_take(radio, &n));
        for (uint32_t i = 0; i < n; i++) {
            bool is = false;
            sdrkit_string t;
            OK(sdrkit_event_is(radio, i, "radio.connected", &is));
            OK(sdrkit_event_text(radio, i, "no_such_field", &t));
            CHECK(t.len == 0);
            connected |= is;
        }
        CHECK(sdrkit_event_is(radio, n, "radio.connected", &(bool){false}) == SDRKIT_E_NO_SUCH_ID);
        pause_a_moment();
    }
    CHECK(connected);

    /* A command by key. */
    OK(sdrkit_call(session, "clear", NULL, 0));
    CHECK(sdrkit_call(session, "no.such.command", NULL, 0) == SDRKIT_E_NO_SUCH_KEY);
    sdrkit_string p;
    OK(sdrkit_problem(session, &p));
    CHECK(p.len == 0);
    OK(sdrkit_free(engine));
    printf("c_byname ok\n");
    return 0;
}
