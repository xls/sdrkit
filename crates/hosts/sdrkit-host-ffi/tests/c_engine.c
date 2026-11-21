/* The engine, its objects and its errors. tests/c_api.rs builds and runs it with a folder as its argument. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"
#include <time.h>

int main(int argc, char **argv) {
    CHECK(argc == 2);
    sdrkit_string version;
    OK(sdrkit_version(&version));
    CHECK(version.len > 0);

    /* A failed creator leaves its error on handle 0. */
    SDRKIT_HANDLE engine = 0;
    int32_t failed = sdrkit_engine_new("/dev/null/sdrkit", &engine);
    CHECK(failed != SDRKIT_OK);
    sdrkit_string message;
    OK(sdrkit_error_message(0, &message));
    CHECK(message.len > 0);
    printf("%s: %.*s\n", sdrkit_code_name(failed), (int)message.len, message.ptr);
    CHECK(strcmp(sdrkit_code_name(SDRKIT_E_NO_SUCH_OBJECT), "SDRKIT_E_NO_SUCH_OBJECT") == 0);

    /* An engine without a folder keeps its files in memory. */
    OK(sdrkit_engine_new(NULL, &engine));
    OK(sdrkit_free(engine));

    OK(sdrkit_engine_new(argv[1], &engine));

    /* List the live objects. */
    uint32_t count = 0;
    OK(sdrkit_objects(engine, &count));
    int services = 0;
    for (uint32_t n = 0; n < count; n++) {
        sdrkit_string key;
        int32_t type;
        OK(sdrkit_object(engine, n, &key, &type));
        services += type == SDRKIT_OBJECT_SERVICE;
    }
    CHECK(services > 0);

    /* Find an object by its key. */
    SDRKIT_HANDLE scan = 0;
    OK(sdrkit_object_find(engine, "service.scan", &scan));
    int32_t type;
    OK(sdrkit_object_type(scan, &type));
    CHECK(type == SDRKIT_OBJECT_SERVICE);
    sdrkit_string key;
    OK(sdrkit_object_key(scan, &key));
    CHECK(string_is(key, "service.scan"));
    CHECK(sdrkit_object_find(engine, "service.none", &scan) == SDRKIT_E_NO_SUCH_KEY);

    /* A refused value returns its code, the error detail and a sentence for a log. */
    sdrkit_id height;
    OK(sdrkit_setting_find(engine, "height_m", &height));
    sdrkit_decl decl = SDRKIT_DECL_INIT;
    OK(sdrkit_setting_decl(engine, height, &decl));
    CHECK(decl.flags & SDRKIT_DECL_RANGE);
    sdrkit_variant v = decl.default_value;
    v.number = decl.max + 1.0;
    CHECK(sdrkit_setting_set(engine, height, &v) == SDRKIT_E_OUT_OF_RANGE);
    sdrkit_error_info detail = SDRKIT_ERROR_INFO_INIT;
    OK(sdrkit_error_detail(engine, &detail));
    CHECK(detail.code == SDRKIT_E_OUT_OF_RANGE && detail.setting == height);
    sdrkit_string why;
    OK(sdrkit_error_message(engine, &why));
    printf("refused: %.*s\n", (int)why.len, why.ptr);

    /* What a stack needs on this device, before a session starts it: the answer comes once the calibration is done. */
    sdrkit_stack_need_info need = SDRKIT_STACK_NEED_INFO_INIT;
    int32_t got = SDRKIT_E_NOT_NOW;
    for (int tries = 0; tries < 400 && got == SDRKIT_E_NOT_NOW; tries++) {
        got = sdrkit_stack_need(engine, "dab", NULL, &need);
        if (got == SDRKIT_E_NOT_NOW) {
            nanosleep(&(struct timespec){0, 5000000}, NULL);
        }
    }
    OK(got);
    CHECK(need.core_units > 0 && need.core_units_slow > 0 && need.core_units_slow <= need.core_units);
    CHECK(need.cores > 0 && need.units > 0 && need.stages >= 2 && need.threads >= 1);
    printf("dab needs %.2f cores, %.1f free, %u threads\n", need.cores, need.cores_free, need.threads);
    CHECK(sdrkit_stack_need(engine, "no-such-stack", NULL, &need) == SDRKIT_E_NO_SUCH_KEY);

    /* The settings, the state, the tables and the log of an object, for a bug report, through the typed calls. */
    uint32_t settings = 0;
    OK(sdrkit_settings_count(scan, &settings));
    for (sdrkit_id id = 0; id < settings; id++) {
        sdrkit_variant value;
        OK(sdrkit_setting_get(scan, id, &value));
    }
    sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
    OK(sdrkit_snapshot_read(scan, 0, &snapshot));
    sdrkit_results_info results = SDRKIT_RESULTS_INFO_INIT;
    OK(sdrkit_results_decl(scan, &results));
    CHECK(results.tables > 0);
    for (sdrkit_id t = 0; t < results.tables; t++) {
        sdrkit_rows_info rows = SDRKIT_ROWS_INFO_INIT;
        OK(sdrkit_snapshot_table(scan, t, &rows));
        CHECK(rows.rows <= rows.total);
    }
    for (uint32_t line = 0; line < snapshot.log_lines; line++) {
        sdrkit_string text;
        OK(sdrkit_snapshot_log(scan, line, &text));
    }
    printf("service.scan: %u settings, %u tables, %u log lines\n", settings, results.tables, snapshot.log_lines);

    /* A freed handle is refused. */
    OK(sdrkit_free(scan));
    uint32_t n = 0;
    CHECK(sdrkit_commands_count(scan, &n) == SDRKIT_E_NO_SUCH_OBJECT);
    OK(sdrkit_free(engine));
    CHECK(sdrkit_free(engine) == SDRKIT_E_NO_SUCH_OBJECT);
    printf("c_engine ok\n");
    return 0;
}
