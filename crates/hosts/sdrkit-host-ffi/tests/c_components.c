/* The results of a stack with a decoder component that is not its status component: the counts of dropped items as
   one struct, a table with the key "rows", and what a change of frequency by the host does to a stack that has no
   handling of its own. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"
#include "sdrkit/trunk.h"

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0, radio = 0, session = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_radio_open(engine, "synth:" SDRKIT_STACK_ERMES, NULL, &radio));
    sdrkit_input input = sdrkit_input_radio(radio);
    OK(sdrkit_stack_start(engine, SDRKIT_STACK_ERMES, NULL, &input, &session));

    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    sdrkit_rows_info listed = SDRKIT_ROWS_INFO_INIT;
    sdrkit_ermes_fields fields;
    sdrkit_ermes_dropped dropped;
    sdrkit_session_fields counts;
    static sdrkit_ermes_entry_row rows[64];
    memset(&fields, 0, sizeof fields);
    for (int i = 0; i < 6000 && fields.batches < 3; i++) {
        pause_a_moment();
        OK(sdrkit_snapshot_read(session, 0, &snapshot));
        OK(sdrkit_ermes_fields_read(session, &fields));
    }
    CHECK(fields.batches >= 3);
    /* The counts of the decoders by name. A count holds a value only while its decoder runs on a helper thread. */
    OK(sdrkit_ermes_dropped_read(session, &dropped));
    CHECK(dropped.size == sizeof dropped);
    CHECK(!dropped.has_modem || dropped.modem >= 0);
    OK(sdrkit_ermes_rows(session, rows, 64, &listed));
    CHECK(listed.rows >= 3 && rows[0].size == sizeof rows[0] && rows[0].kind.len > 0);
    /* A radio has no decoder of this stack. */
    OK(sdrkit_snapshot_read(radio, 0, &snapshot));
    CHECK(sdrkit_ermes_dropped_read(radio, &dropped) == SDRKIT_E_WRONG_HANDLE);

    /* A change of frequency by the host. The stack has no handling of its own: the receiver forgets, and the count
       of batches starts again. */
    double tuned = 0;
    OK(sdrkit_snapshot_read(session, 0, &snapshot));
    OK(sdrkit_session_fields_read(session, &counts));
    int64_t retunes = counts.retunes;
    OK(sdrkit_session_get_tuned_hz(session, &tuned));
    OK(sdrkit_session_set_tuned_hz(session, tuned + 25e3));
    int taken = 0;
    for (int i = 0; i < 2000 && !taken; i++) {
        pause_a_moment();
        OK(sdrkit_snapshot_read(session, 0, &snapshot));
        OK(sdrkit_session_fields_read(session, &counts));
        OK(sdrkit_ermes_fields_read(session, &fields));
        OK(sdrkit_ermes_rows(session, rows, 64, &listed));
        taken = counts.retunes == retunes + 1;
    }
    CHECK(taken);
    CHECK(fields.batches <= 1 && listed.total <= 2);

    OK(sdrkit_session_stop(session));
    OK(sdrkit_free(engine));
    printf("c_components ok\n");
    return 0;
}
