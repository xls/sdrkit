/* The stacks "ais" and "fm" through their family headers: a command with an arguments struct, a table read as
   structs, result fields, and settings of the text and enumeration types. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"
#include "sdrkit/fm.h"
#include "sdrkit/marine.h"

int main(int argc, char **argv) {
    CHECK(argc == 2);
    uint32_t major = 0, minor = 0;
    OK(sdrkit_family_version("marine", &major, &minor));
    CHECK(major == SDRKIT_MARINE_VERSION_MAJOR && minor >= SDRKIT_MARINE_VERSION_MINOR);

    SDRKIT_HANDLE engine = 0, radio = 0, session = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_radio_open(engine, "synth:" SDRKIT_STACK_AIS, NULL, &radio));
    sdrkit_input input = sdrkit_input_radio(radio);
    OK(sdrkit_stack_start(engine, SDRKIT_STACK_AIS, NULL, &input, &session));

    static sdrkit_ais_station_row stations[64];
    sdrkit_rows_info info = SDRKIT_ROWS_INFO_INIT;
    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    for (int i = 0; i < 3000 && info.total < 3; i++) {
        pause_a_moment();
        OK(sdrkit_snapshot_read(session, 0, &snapshot));
        OK(sdrkit_ais_stations_rows(session, stations, 64, &info));
    }
    CHECK(info.total >= 3 && info.rows >= 1);
    const sdrkit_ais_station_row *first = &stations[0];
    CHECK(first->size == sizeof *first && first->mmsi.len == 9 && first->key.len == 9);

    /* A command with arguments: select the first station, and read it from the table of the selected station. */
    char mmsi[10] = {0};
    memcpy(mmsi, first->mmsi.ptr, 9);
    sdrkit_ais_station_args args = SDRKIT_AIS_STATION_ARGS_INIT;
    args.row.ptr = mmsi;
    args.row.len = 9;
    OK(sdrkit_ais_select(session, &args));
    sdrkit_ais_selected_row selected[1];
    info.total = 0;
    for (int i = 0; i < 500 && info.total == 0; i++) {
        pause_a_moment();
        OK(sdrkit_snapshot_read(session, 0, &snapshot));
        OK(sdrkit_ais_selected_rows(session, selected, 1, &info));
    }
    CHECK(info.total == 1 && selected[0].mmsi.len == 9 && memcmp(selected[0].mmsi.ptr, mmsi, 9) == 0);

    /* A row that does not exist is refused, and so is an arguments struct without its size. */
    args.row.ptr = "000000000";
    CHECK(sdrkit_ais_select(session, &args) == SDRKIT_E_NO_SUCH_ROW);
    args.size = 0;
    CHECK(sdrkit_ais_select(session, &args) == SDRKIT_E_BAD_ARGUMENT);
    args.size = sizeof args;
    args.row.ptr = mmsi;
    OK(sdrkit_ais_remove(session, &args));

    sdrkit_ais_fields fields;
    OK(sdrkit_snapshot_read(session, 0, &snapshot));
    OK(sdrkit_ais_fields_read(session, &fields));
    CHECK(fields.size == sizeof fields && fields.frames_ok > 0 && fields.stations >= 1);
    OK(sdrkit_session_stop(session));
    OK(sdrkit_session_close(session));
    OK(sdrkit_radio_close(radio));

    /* FM: the result fields, and settings of the enumeration, switch and text types. */
    OK(sdrkit_radio_open(engine, "synth:" SDRKIT_STACK_FM, NULL, &radio));
    input = sdrkit_input_radio(radio);
    OK(sdrkit_stack_start(engine, SDRKIT_STACK_FM, NULL, &input, &session));
    sdrkit_fm_fields fm;
    memset(&fm, 0, sizeof fm);
    for (int i = 0; i < 3000 && fm.station.len == 0; i++) {
        pause_a_moment();
        OK(sdrkit_snapshot_read(session, 0, &snapshot));
        OK(sdrkit_fm_fields_read(session, &fm));
    }
    CHECK(fm.size == sizeof fm && fm.station.len == 8 && fm.has_pi && fm.pi.len == 4);
    CHECK(fm.band_start_hz > 60e6 && fm.band_end_hz > fm.band_start_hz);

    sdrkit_fm_stereo stereo;
    OK(sdrkit_fm_set_stereo(session, SDRKIT_FM_STEREO_MONO));
    OK(sdrkit_fm_get_stereo(session, &stereo));
    CHECK(stereo == SDRKIT_FM_STEREO_MONO);
    bool mono = false;
    OK(sdrkit_fm_set_force_mono(session, true));
    OK(sdrkit_fm_get_force_mono(session, &mono));
    CHECK(mono);
    sdrkit_string text;
    sdrkit_string other = {"hello", 5};
    OK(sdrkit_fm_gen_set_rt(session, other));
    OK(sdrkit_fm_gen_get_rt(session, &text));
    CHECK(string_is(text, "hello"));
    OK(sdrkit_fm_clear(session));

    OK(sdrkit_session_stop(session));
    OK(sdrkit_session_close(session));
    OK(sdrkit_radio_close(radio));
    OK(sdrkit_free(engine));
    printf("c_typed_marine ok\n");
    return 0;
}
