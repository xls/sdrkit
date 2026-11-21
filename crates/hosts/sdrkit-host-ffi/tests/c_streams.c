/* From a new engine to audio of a stack on a radio, then the events of the radio. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0, radio = 0, session = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_radio_open(engine, "synth:fm", NULL, &radio));
    sdrkit_input input = sdrkit_input_radio(radio);
    OK(sdrkit_stack_start(engine, "fm", NULL, &input, &session));
    static float audio[4800];
    sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;
    for (int i = 0; i < 2000 && got.frames == 0; i++) {
        OK(sdrkit_audio_read(session, audio, 4800, &got));
        if (got.frames == 0) pause_a_moment();
    }
    CHECK(got.frames > 0 && got.channels >= 1);
    printf("%zu frames at %.0f Hz\n", got.frames, got.rate_hz);

    /* Each event has the id of its event type. */
    sdrkit_id connected;
    OK(sdrkit_event_type_find(engine, "radio.connected", &connected));
    int seen = 0;
    for (int i = 0; i < 500 && !seen; i++) {
        uint32_t count = 0;
        OK(sdrkit_events_take(radio, &count));
        for (uint32_t n = 0; n < count; n++) {
            sdrkit_event_info ev = SDRKIT_EVENT_INFO_INIT;
            OK(sdrkit_event(radio, n, &ev));
            seen |= ev.type == connected;
        }
        if (!seen) pause_a_moment();
    }
    CHECK(seen);

    OK(sdrkit_session_stop(session));
    OK(sdrkit_session_close(session));
    OK(sdrkit_radio_close(radio));
    OK(sdrkit_free(engine));
    printf("c_streams ok\n");
    return 0;
}
