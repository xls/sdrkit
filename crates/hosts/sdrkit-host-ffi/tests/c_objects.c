/* The typed results, tables, events and commands of the session, the radio and the engine, the spectrum frames of a
   session, and what a change of frequency does to the results of a session. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"
#include "sdrkit/fm.h"
#include <math.h>

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0, radio = 0, session = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_radio_open(engine, "synth:fm", NULL, &radio));
    sdrkit_input input = sdrkit_input_radio(radio);
    OK(sdrkit_stack_start(engine, "fm", NULL, &input, &session));

    /* The session: its state and counts beside the fields of its stack, in one snapshot. */
    static float audio[9600];
    sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;
    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    sdrkit_session_fields counts;
    sdrkit_fm_fields fm;
    memset(&fm, 0, sizeof fm);
    for (int i = 0; i < 6000 && fm.station.len == 0; i++) {
        pause_a_moment();
        OK(sdrkit_audio_read(session, audio, 9600, &got));
        OK(sdrkit_snapshot_read(session, 0, &snapshot));
        OK(sdrkit_fm_fields_read(session, &fm));
    }
    CHECK(fm.station.len > 0);
    OK(sdrkit_session_fields_read(session, &counts));
    CHECK(counts.size == sizeof counts);
    CHECK(counts.state == SDRKIT_SESSION_FIELDS_STATE_RUNNING && counts.samples > 0);
    CHECK(!counts.has_problem && !counts.has_tx_sent);
    /* The measurements of the receiver: the members of the session hold what the snapshot holds. */
    CHECK(counts.has_snr_db == !isnan(snapshot.snr_db));
    CHECK(counts.has_level_dbfs == !isnan(snapshot.level_dbfs));
    CHECK(counts.has_offset_hz == !isnan(snapshot.offset_hz));
    CHECK(!counts.has_level_dbfs || fabs(counts.level_dbfs - snapshot.level_dbfs) < 1e-3);
    CHECK(!counts.has_offset_hz || counts.offset_hz == snapshot.offset_hz);

    /* The spectrum: a frame into the buffer of the caller. A buffer that is too small holds the first bins, and the
       frame says how many bins it has. */
    static float bins[4096];
    sdrkit_spectrum_frame frame = SDRKIT_SPECTRUM_FRAME_INIT;
    OK(sdrkit_spectrum_read(session, bins, 4096, &frame));
    CHECK(!frame.found && frame.bins == 0);
    OK(sdrkit_session_set_spectrum_size(session, SDRKIT_SESSION_SPECTRUM_SIZE_1024));
    OK(sdrkit_session_set_spectrum(session, true));
    for (int i = 0; i < 2000 && !frame.found; i++) {
        pause_a_moment();
        OK(sdrkit_spectrum_read(session, bins, 4096, &frame));
    }
    CHECK(frame.found && frame.size == sizeof frame);
    CHECK(frame.bins == 1024 && frame.total == 1024);
    CHECK(frame.span_hz > 0 && frame.centre_hz > 0 && frame.frame_rate > 0 && frame.time >= 0);
    frame.found = false;
    for (int i = 0; i < 2000 && !frame.found; i++) {
        pause_a_moment();
        OK(sdrkit_spectrum_read(session, bins, 16, &frame));
    }
    CHECK(frame.found && frame.bins == 16 && frame.total == 1024);
    CHECK(sdrkit_spectrum_read(session, NULL, 16, &frame) == SDRKIT_E_NULL_POINTER);
    OK(sdrkit_session_set_spectrum(session, false));
    /* A radio has no fields of a session. */
    OK(sdrkit_snapshot_read(radio, 0, &snapshot));
    CHECK(sdrkit_session_fields_read(radio, &counts) == SDRKIT_E_WRONG_HANDLE);

    /* The radio: its link and counts, and its events. */
    sdrkit_radio_fields link;
    OK(sdrkit_radio_fields_read(radio, &link));
    CHECK(link.size == sizeof link && link.state == SDRKIT_RADIO_FIELDS_STATE_UP);
    CHECK(link.samples > 0 && link.has_rate_hz && link.rate_hz > 0);
    uint32_t taken = 0, connected = 0;
    OK(sdrkit_events_take(radio, &taken));
    for (uint32_t n = 0; n < taken; n++) {
        sdrkit_radio_connected_event up;
        sdrkit_radio_lost_event down;
        int32_t code = sdrkit_radio_connected_event_read(radio, n, &up);
        if (code == SDRKIT_E_WRONG_TYPE) continue;
        OK(code);
        CHECK(up.size == sizeof up && up.time >= 0.0);
        CHECK(sdrkit_radio_lost_event_read(radio, n, &down) == SDRKIT_E_WRONG_TYPE);
        connected++;
    }
    CHECK(connected == 1);
    sdrkit_radio_connected_event none;
    CHECK(sdrkit_radio_connected_event_read(radio, taken, &none) == SDRKIT_E_NO_SUCH_ID);
    /* A session has no command of a radio. */
    CHECK(sdrkit_radio_reconnect(session) == SDRKIT_E_WRONG_HANDLE);

    /* A change of frequency. When the call returns, the session shows no result of its stack from the frequency
       before, and it keeps its own fields. The results stay away until the receiver has taken the frequency. */
    double tuned = 0;
    int64_t retunes = counts.retunes;
    OK(sdrkit_session_get_tuned_hz(session, &tuned));
    OK(sdrkit_snapshot_read(session, 0, &snapshot));
    OK(sdrkit_session_fields_read(session, &counts));
    retunes = counts.retunes;
    OK(sdrkit_session_set_tuned_hz(session, tuned + 8e6));
    int taken_by_receiver = 0;
    for (int i = 0; i < 2000 && !taken_by_receiver; i++) {
        OK(sdrkit_snapshot_read(session, 0, &snapshot));
        OK(sdrkit_fm_fields_read(session, &fm));
        OK(sdrkit_session_fields_read(session, &counts));
        CHECK(fm.station.len == 0 && !fm.has_pi && !fm.rds_sync);
        CHECK(counts.samples > 0 && counts.state == SDRKIT_SESSION_FIELDS_STATE_RUNNING);
        taken_by_receiver = counts.retunes == retunes + 1;
        pause_a_moment();
    }
    CHECK(taken_by_receiver);
    /* The stack clears what belongs to one frequency: the name of the station before does not come back at once. */
    for (int i = 0; i < 10; i++) {
        OK(sdrkit_snapshot_read(session, 0, &snapshot));
        OK(sdrkit_fm_fields_read(session, &fm));
        CHECK(fm.station.len == 0);
        pause_a_moment();
    }

    /* The engine: a command with a reply, and a command with arguments. */
    sdrkit_engine_state_save_reply saved;
    OK(sdrkit_engine_state_save(engine, &saved));
    CHECK(saved.size == sizeof saved && saved.state.len > 0);
    sdrkit_engine_state_restore_args restore = SDRKIT_ENGINE_STATE_RESTORE_ARGS_INIT;
    restore.state.len = 3;
    restore.state.ptr = (const uint8_t *)"abc";
    CHECK(sdrkit_engine_state_restore(engine, &restore) != SDRKIT_OK);
    CHECK(sdrkit_engine_state_save(session, &saved) == SDRKIT_E_WRONG_HANDLE);
    /* The tables of the engine as structs: the stacks, and the requests. */
    static sdrkit_engine_stack_row hits[64];
    static sdrkit_engine_request_row requests[8];
    sdrkit_rows_info listed = SDRKIT_ROWS_INFO_INIT;
    OK(sdrkit_snapshot_read(engine, 0, &snapshot));
    OK(sdrkit_engine_stacks_rows(engine, hits, 64, &listed));
    CHECK(listed.total >= 100 && listed.rows == listed.total);
    CHECK(hits[0].size == sizeof hits[0] && hits[0].key.len > 0 && hits[0].name.len > 0);
    OK(sdrkit_engine_requests_rows(engine, requests, 8, &listed));
    CHECK(listed.rows <= 8);
    CHECK(sdrkit_engine_stacks_rows(session, hits, 64, &listed) == SDRKIT_E_WRONG_HANDLE);
    sdrkit_engine_request_event request;
    OK(sdrkit_events_take(engine, &taken));
    CHECK(sdrkit_engine_request_event_read(engine, taken, &request) == SDRKIT_E_NO_SUCH_ID);

    OK(sdrkit_session_stop(session));
    OK(sdrkit_free(engine));
    printf("c_objects ok\n");
    return 0;
}
