/* A running FM session changes its frequency with the typed setting of the session: once inside the samples of its
   radio, and once outside them, where the radio moves its centre. The handle of the session stays valid, the session
   keeps running, and it reports the new frequency. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"

/* Returns true if the session gives audio within about 20 s. */
static int audio_runs(SDRKIT_HANDLE session) {
    static float audio[4800];
    sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;
    for (int i = 0; i < 2000 && got.frames == 0; i++) {
        if (sdrkit_audio_read(session, audio, 4800, &got) != SDRKIT_OK) return 0;
        if (got.frames == 0) pause_a_moment();
    }
    return got.frames > 0;
}

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0, radio = 0, session = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_radio_open(engine, "synth:fm", NULL, &radio));
    sdrkit_input input = sdrkit_input_radio(radio);
    OK(sdrkit_stack_start(engine, "fm", NULL, &input, &session));
    CHECK(audio_runs(session));

    double centre = 0, rate = 0, tuned = -1, now = 0;
    OK(sdrkit_radio_get_centre_hz(radio, &centre));
    OK(sdrkit_radio_get_rate(radio, &rate));
    CHECK(centre > 0 && rate > 0);

    /* Inside the samples: the session moves, the radio keeps its centre. */
    double inside = centre + rate / 8;
    OK(sdrkit_session_set_tuned_hz(session, inside));
    OK(sdrkit_session_get_tuned_hz(session, &tuned));
    CHECK(tuned == inside);
    CHECK(audio_runs(session));
    OK(sdrkit_radio_get_centre_hz(radio, &now));
    CHECK(now == centre);

    /* Outside the samples: the radio moves its centre so that the samples hold the frequency. */
    double outside = centre + 4 * rate;
    OK(sdrkit_session_set_tuned_hz(session, outside));
    OK(sdrkit_session_get_tuned_hz(session, &tuned));
    CHECK(tuned == outside);
    for (int i = 0; i < 2000 && now == centre; i++) {
        pause_a_moment();
        OK(sdrkit_radio_get_centre_hz(radio, &now));
    }
    CHECK(now != centre && now - rate / 2 <= outside && outside <= now + rate / 2);
    CHECK(audio_runs(session));
    OK(sdrkit_session_get_tuned_hz(session, &tuned));
    CHECK(tuned == outside);

    OK(sdrkit_session_stop(session));
    OK(sdrkit_free(engine));
    printf("c_retune ok\n");
    return 0;
}
