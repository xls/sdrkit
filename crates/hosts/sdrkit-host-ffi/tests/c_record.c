/* The record service through its family header: an audio recording of an FM session started and stopped with the
   typed commands, the running row whose size grows while the file is open, and the row of the finished file.
   tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0, radio = 0, session = 0, record = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_radio_open(engine, "synth:fm", NULL, &radio));
    sdrkit_input input = sdrkit_input_radio(radio);
    OK(sdrkit_stack_start(engine, "fm", NULL, &input, &session));
    OK(sdrkit_object_find(engine, "service.record", &record));
    OK(sdrkit_record_set_reserve_bytes(record, 0));

    sdrkit_string key;
    OK(sdrkit_object_key(session, &key));
    char session_key[64];
    CHECK(key.len < sizeof session_key);
    memcpy(session_key, key.ptr, key.len);
    session_key[key.len] = 0;
    sdrkit_record_audio_start_args start = SDRKIT_RECORD_AUDIO_START_ARGS_INIT;
    start.session = (sdrkit_string){session_key, key.len};
    sdrkit_record_audio_start_reply reply = {0};
    reply.size = sizeof reply;
    OK(sdrkit_record_audio_start(record, &start, &reply));
    CHECK(reply.recording.len > 0 && reply.recording.len < 64);
    char row[64];
    memcpy(row, reply.recording.ptr, reply.recording.len);
    size_t row_len = reply.recording.len;

    /* The size of the open file grows in the running row. */
    static sdrkit_record_running_row running[8];
    sdrkit_rows_info info = SDRKIT_ROWS_INFO_INIT;
    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    int64_t bytes = 0;
    for (int i = 0; i < 1000 && bytes == 0; i++) {
        pause_a_moment();
        OK(sdrkit_snapshot_read(record, 0, &snapshot));
        OK(sdrkit_record_running_rows(record, running, 8, &info));
        if (info.rows == 1) {
            CHECK(running[0].size == sizeof running[0]);
            CHECK(string_is(running[0].session, session_key));
            CHECK(running[0].kind == SDRKIT_RECORD_RUNNING_ROW_KIND_AUDIO);
            bytes = running[0].bytes;
        }
    }
    CHECK(bytes > 0);
    CHECK(running[0].has_file);

    sdrkit_record_audio_stop_args stop = SDRKIT_RECORD_AUDIO_STOP_ARGS_INIT;
    stop.row = (sdrkit_string){row, row_len};
    OK(sdrkit_record_audio_stop(record, &stop));
    static sdrkit_record_recording_row done[8];
    uint32_t files = 0;
    for (int i = 0; i < 1000 && files == 0; i++) {
        pause_a_moment();
        OK(sdrkit_snapshot_read(record, 0, &snapshot));
        OK(sdrkit_record_running_rows(record, running, 8, &info));
        if (info.total != 0) continue;
        OK(sdrkit_record_recordings_rows(record, done, 8, &info));
        files = info.total;
    }
    CHECK(files == 1);
    CHECK(done[0].kind == SDRKIT_RECORD_RECORDING_ROW_KIND_AUDIO && done[0].bytes >= bytes);
    printf("%.*s %lld bytes\n", (int)done[0].key.len, done[0].key.ptr, (long long)done[0].bytes);

    OK(sdrkit_session_stop(session));
    OK(sdrkit_session_close(session));
    OK(sdrkit_radio_close(radio));
    sdrkit_free(engine);
    printf("c_record ok\n");
    return 0;
}
