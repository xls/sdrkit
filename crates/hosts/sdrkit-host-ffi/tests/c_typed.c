/* The stack "adsb" through its family header: the aircraft table, the tracker fields and an event read as structs, a
   setting read and written with its type, and a command. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"
#include "sdrkit/adsb.h"
#include "sdrkit/dvoice.h"

int main(int argc, char **argv) {
    CHECK(argc == 2);
    uint32_t major = 0, minor = 0, families = 0;
    OK(sdrkit_interface_version(&major, &minor));
    CHECK(major == SDRKIT_INTERFACE_VERSION_MAJOR && minor >= SDRKIT_INTERFACE_VERSION_MINOR);
    OK(sdrkit_family_version("adsb", &major, &minor));
    CHECK(major == SDRKIT_ADSB_VERSION_MAJOR && minor >= SDRKIT_ADSB_VERSION_MINOR);
    CHECK(sdrkit_family_version("none", &major, &minor) == SDRKIT_E_NO_SUCH_KEY);
    OK(sdrkit_families_count(&families));
    int listed = 0;
    for (uint32_t n = 0; n < families; n++) {
        sdrkit_string key;
        OK(sdrkit_family(n, &key, &major, &minor));
        listed += key.len == 4 && memcmp(key.ptr, "adsb", 4) == 0;
    }
    CHECK(listed == 1);

    SDRKIT_HANDLE engine = 0, radio = 0, session = 0, type = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_object_find(engine, "stack." SDRKIT_STACK_ADSB, &type));
    OK(sdrkit_radio_open(engine, "synth:" SDRKIT_STACK_ADSB, NULL, &radio));
    sdrkit_input input = sdrkit_input_radio(radio);
    OK(sdrkit_stack_start(engine, SDRKIT_STACK_ADSB, NULL, &input, &session));

    static sdrkit_adsb_aircraft_row rows[160];
    sdrkit_rows_info info = SDRKIT_ROWS_INFO_INIT;
    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    int with_position = 0;
    for (int i = 0; i < 3000 && with_position < 3; i++) {
        pause_a_moment();
        OK(sdrkit_snapshot_read(session, 0, &snapshot));
        OK(sdrkit_adsb_aircraft_rows(session, rows, 160, &info));
        CHECK(info.rows == info.total && info.total <= 160);
        with_position = 0;
        for (uint32_t n = 0; n < info.rows; n++) with_position += rows[n].has_position;
    }
    CHECK(with_position >= 3);
    for (uint32_t n = 0; n < info.rows; n++) {
        const sdrkit_adsb_aircraft_row *a = &rows[n];
        /* The library and this program have the same struct. */
        CHECK(a->size == sizeof *a);
        CHECK(a->icao.len == 6 && a->key.len == 6 && memcmp(a->icao.ptr, a->key.ptr, 6) == 0);
        CHECK(a->source.len == 1 && a->source.ptr[0] == '0');
        if (!a->has_position) continue;
        CHECK(a->position.lat >= -90.0 && a->position.lat <= 90.0 && !a->position.has_alt);
        CHECK(a->has_fix && a->fix >= SDRKIT_ADSB_AIRCRAFT_ROW_FIX_GLOBAL &&
              a->fix <= SDRKIT_ADSB_AIRCRAFT_ROW_FIX_LOCAL_FROM_RECEIVER);
        printf("%.*s %.*s %.4f %.4f %lld ft\n", (int)a->icao.len, a->icao.ptr,
               a->has_callsign ? (int)a->callsign.len : 0, a->callsign.ptr, a->position.lat,
               a->position.lon, a->has_alt_ft ? (long long)a->alt_ft : 0LL);
    }

    sdrkit_adsb_tracker_fields fields;
    OK(sdrkit_adsb_tracker_fields_read(session, &fields));
    CHECK(fields.size == sizeof fields);
    CHECK(fields.aircraft_count == (int64_t)info.total && fields.with_position >= 3);
    CHECK(fields.frames_ok > 0 && fields.message_rate > 0.0);

    /* A program compiled with an older, shorter struct gets the members that it knows. */
    struct { uint32_t size; sdrkit_string key; sdrkit_string icao; } old[2];
    OK(sdrkit_typed_rows(session, "adsb.AircraftRow", "aircraft", old, sizeof old[0], 2, &info));
    CHECK(old[0].size == sizeof old[0] && old[1].icao.len == 6);
    CHECK(memcmp(old[0].icao.ptr, rows[0].icao.ptr, 6) == 0);

    /* The radio does not publish the table, and the session has no table of another stack. */
    OK(sdrkit_snapshot_read(radio, 0, &snapshot));
    CHECK(sdrkit_adsb_aircraft_rows(radio, rows, 160, &info) == SDRKIT_E_WRONG_HANDLE);
    static sdrkit_uplink1030_addressed_row addressed[4];
    CHECK(sdrkit_uplink1030_addressed_rows(session, addressed, 4, &info) == SDRKIT_E_WRONG_HANDLE);

    /* An event as a struct. An event of another type is refused. */
    uint32_t taken = 0, seen = 0;
    OK(sdrkit_events_take(session, &taken));
    for (uint32_t n = 0; n < taken; n++) {
        sdrkit_adsb_aircraft_event ev;
        int32_t code = sdrkit_adsb_aircraft_event_read(session, n, &ev);
        if (code == SDRKIT_E_WRONG_TYPE) continue;
        OK(code);
        CHECK(ev.size == sizeof ev);
        CHECK(ev.source.len >= 6);
        CHECK(ev.time >= 0.0);
        CHECK(ev.event >= SDRKIT_ADSB_AIRCRAFT_EVENT_NEW && ev.event <= SDRKIT_ADSB_AIRCRAFT_EVENT_UPDATE);
        sdrkit_modeac_reply_event other;
        CHECK(sdrkit_modeac_reply_event_read(session, n, &other) == SDRKIT_E_WRONG_TYPE);
        seen++;
    }
    CHECK(seen > 0);

    /* A setting with its type: a number and an enumeration. One call sets one setting. */
    double expiry = 0.0;
    OK(sdrkit_adsb_get_expiry_s(session, &expiry));
    OK(sdrkit_adsb_set_expiry_s(session, expiry + 5.0));
    double now = 0.0;
    OK(sdrkit_adsb_get_expiry_s(session, &now));
    CHECK(now == expiry + 5.0);
    CHECK(sdrkit_adsb_set_expiry_s(session, -1.0) != SDRKIT_OK);
    sdrkit_adsb_correction correction;
    OK(sdrkit_adsb_set_correction(session, SDRKIT_ADSB_CORRECTION_1BIT));
    OK(sdrkit_adsb_get_correction(session, &correction));
    CHECK(correction == SDRKIT_ADSB_CORRECTION_1BIT);
    /* The stack type has the defaults, and a session of this stack has no setting of another stack. */
    OK(sdrkit_adsb_get_expiry_s(type, &now));
    CHECK(now == expiry);
    int64_t whole = 0;
    CHECK(sdrkit_typed_setting_get_i64(session, "expiry_s", &whole) == SDRKIT_E_WRONG_TYPE);
    CHECK(sdrkit_typed_setting_get_f64(session, "no_such_setting", &now) == SDRKIT_E_WRONG_HANDLE);

    /* A command: the table holds fewer aircraft at the next report. */
    int64_t before = fields.aircraft_count;
    OK(sdrkit_adsb_clear(session));
    for (int i = 0; i < 300 && fields.aircraft_count >= before; i++) {
        pause_a_moment();
        OK(sdrkit_snapshot_read(session, 0, &snapshot));
        OK(sdrkit_adsb_tracker_fields_read(session, &fields));
    }
    CHECK(fields.aircraft_count < before);
    CHECK(sdrkit_adsb_clear(radio) == SDRKIT_E_WRONG_HANDLE);

    OK(sdrkit_session_stop(session));
    OK(sdrkit_session_close(session));
    OK(sdrkit_radio_close(radio));
    OK(sdrkit_free(engine));
    printf("c_typed ok\n");
    return 0;
}
