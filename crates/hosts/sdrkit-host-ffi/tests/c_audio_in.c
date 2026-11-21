/* Host audio for a session: sdrkit_session_write_audio, its errors, and the result fields that ask for host audio. A
   session of the general stack on the test signal cannot transmit; a graph with the transmitter of the general stack on
   a host feed transmits to the host, and asks for the audio only while push-to-talk is on. tests/c_api.rs builds and
   runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"
#include "sdrkit/engine.h"
#include "sdrkit/general.h"
#include "sdrkit/session.h"
#include <math.h>

/* Takes a snapshot of `session` and reads its result fields into `f`. */
static int fields_of(SDRKIT_HANDLE session, sdrkit_session_fields *f) {
    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    OK(sdrkit_snapshot_read(session, 0, &snapshot));
    f->size = sizeof *f;
    OK(sdrkit_session_fields_read(session, f));
    return 0;
}

/* Waits until the field `audio_in_wanted` of `session` is `want`. Returns 0 when it is. */
static int wait_wanted(SDRKIT_HANDLE session, SDRKIT_HANDLE feed, bool want) {
    static float zeros[2 * 480];
    sdrkit_session_fields f;
    for (int i = 0; i < 2000; i++) {
        OK(sdrkit_samples_push(feed, SDRKIT_SAMPLE_CF32, zeros, sizeof zeros));
        CHECK(fields_of(session, &f) == 0);
        if (f.audio_in_wanted == want) return 0;
        pause_a_moment();
    }
    return 1;
}

int main(int argc, char **argv) {
    CHECK(argc == 2);
    static float mic[480];
    for (int i = 0; i < 480; i++) mic[i] = 0.25f * sinf(0.13f * (float)i);

    SDRKIT_HANDLE engine = 0, radio = 0, session = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_radio_open(engine, "synth:general", NULL, &radio));
    sdrkit_input input = sdrkit_input_radio(radio);
    OK(sdrkit_stack_start(engine, "general", NULL, &input, &session));

    /* The call takes mono audio at any rate above 0, and NULL with no frames. */
    OK(sdrkit_session_write_audio(session, mic, 480, 1, 48000.0));
    OK(sdrkit_session_write_audio(session, mic, 480, 1, 44100.0));
    OK(sdrkit_session_write_audio(session, NULL, 0, 1, 48000.0));
    CHECK(sdrkit_session_write_audio(session, mic, 480, 1, 0.0) == SDRKIT_E_CONFIG);
    sdrkit_error_info detail = SDRKIT_ERROR_INFO_INIT;
    OK(sdrkit_error_detail(engine, &detail));
    CHECK(detail.code == SDRKIT_E_CONFIG && detail.value == 0.0);
    CHECK(sdrkit_session_write_audio(session, mic, 480, 1, -48000.0) == SDRKIT_E_CONFIG);
    CHECK(sdrkit_session_write_audio(session, mic, 480, 1, NAN) == SDRKIT_E_CONFIG);
    CHECK(sdrkit_session_write_audio(session, NULL, 480, 1, 48000.0) == SDRKIT_E_NULL_POINTER);
    /* Stereo is 240 frames of left then right. Another channel count is refused. */
    OK(sdrkit_session_write_audio(session, mic, 240, 2, 48000.0));
    CHECK(sdrkit_session_write_audio(session, mic, 80, 6, 48000.0) == SDRKIT_E_UNSUPPORTED);
    OK(sdrkit_error_detail(engine, &detail));
    CHECK(detail.code == SDRKIT_E_UNSUPPORTED && detail.value == 6.0);
    CHECK(sdrkit_session_write_audio(session, mic, 480, 0, 48000.0) == SDRKIT_E_UNSUPPORTED);
    CHECK(sdrkit_session_write_audio(radio, mic, 480, 1, 48000.0) == SDRKIT_E_WRONG_HANDLE);
    CHECK(sdrkit_session_write_audio(engine, mic, 480, 1, 48000.0) == SDRKIT_E_WRONG_HANDLE);

    /* The test signal cannot transmit: the session does not ask for host audio, and push-to-talk is refused. */
    sdrkit_session_fields f;
    int ran = 0;
    for (int i = 0; i < 2000 && !ran; i++) {
        CHECK(fields_of(session, &f) == 0);
        ran = f.samples > 0;
        if (!ran) pause_a_moment();
    }
    CHECK(ran);
    CHECK(!f.has_audio_in_rate_hz && !f.audio_in_wanted);
    sdrkit_general_ptt_args on = SDRKIT_GENERAL_PTT_ARGS_INIT;
    on.on = true;
    CHECK(sdrkit_general_ptt(session, &on) == SDRKIT_E_TRANSMIT_OFF);
    OK(sdrkit_session_stop(session));
    OK(sdrkit_session_close(session));
    OK(sdrkit_radio_close(radio));

    /* A graph that transmits to the host: the samples of a host feed give the pace, the audio of the host is the
       microphone of the transmitter of the general stack. */
    SDRKIT_HANDLE feed = 0, graph = 0, tx = 0;
    OK(sdrkit_feed_open(engine, 48000.0, 145e6, &feed));
    OK(sdrkit_graph_new(engine, &graph));
    OK(sdrkit_graph_add(graph, "mic", "host.in"));
    OK(sdrkit_graph_add(graph, "transmit", "general.transmit"));
    OK(sdrkit_graph_add(graph, "tx", "tx.sink"));
    OK(sdrkit_graph_connect(graph, "input", "mic.in"));
    OK(sdrkit_graph_connect(graph, "input", "transmit.in"));
    OK(sdrkit_graph_connect(graph, "mic.out", "transmit.mic"));
    OK(sdrkit_graph_connect(graph, "transmit.out", "tx.in"));
    sdrkit_input host = sdrkit_input_host(feed);
    OK(sdrkit_graph_start(graph, &host, &session));
    OK(sdrkit_engine_set_transmit_allowed(engine, true));
    OK(sdrkit_object_component(session, "transmit", &tx));

    /* The session can transmit: it takes host audio at 48 kHz, and wants none before push-to-talk. */
    int asks = 0;
    static float zeros[2 * 480];
    for (int i = 0; i < 2000 && !asks; i++) {
        OK(sdrkit_samples_push(feed, SDRKIT_SAMPLE_CF32, zeros, sizeof zeros));
        CHECK(fields_of(session, &f) == 0);
        asks = f.has_audio_in_rate_hz;
        if (!asks) pause_a_moment();
    }
    CHECK(asks && f.audio_in_rate_hz == 48000.0 && !f.audio_in_wanted);

    /* Push-to-talk on: the session wants audio, and the audio goes out as transmit samples. */
    OK(sdrkit_general_ptt(tx, &on));
    CHECK(wait_wanted(session, feed, true) == 0);
    static float iq[2 * 4096];
    size_t sent = 0;
    for (int i = 0; i < 200; i++) {
        OK(sdrkit_session_write_audio(session, mic, 480, 1, 48000.0));
        OK(sdrkit_samples_push(feed, SDRKIT_SAMPLE_CF32, zeros, sizeof zeros));
        sdrkit_iq_info got = SDRKIT_IQ_INFO_INIT;
        OK(sdrkit_transmit_read(session, iq, 4096, &got));
        sent += got.samples;
        pause_a_moment();
    }
    CHECK(sent > 0);

    /* Push-to-talk off: the session wants no audio. */
    sdrkit_general_ptt_args off = SDRKIT_GENERAL_PTT_ARGS_INIT;
    OK(sdrkit_general_ptt(tx, &off));
    CHECK(wait_wanted(session, feed, false) == 0);

    OK(sdrkit_free(tx));
    OK(sdrkit_session_stop(session));
    OK(sdrkit_free(engine));
    printf("c_audio_in ok\n");
    return 0;
}
