/* The settings of a stack type, listed with labels, options and help, and the saved settings of the engine. tests/c_api.rs
 * builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"
#include <stdlib.h>

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0, fm = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_object_find(engine, "stack.fm", &fm));

    uint32_t count = 0;
    OK(sdrkit_settings_count(fm, &count));
    CHECK(count > 0);
    int enums = 0;
    for (sdrkit_id id = 0; id < count; id++) {
        sdrkit_decl d = SDRKIT_DECL_INIT;
        OK(sdrkit_setting_decl(fm, id, &d));
        printf("%.*s (%.*s): %.*s\n", (int)d.key.len, d.key.ptr, (int)d.label.len, d.label.ptr, (int)d.help.len,
               d.help.ptr);
        for (uint32_t n = 0; n < d.options; n++) {
            sdrkit_option option = SDRKIT_OPTION_INIT;
            OK(sdrkit_setting_option(fm, id, n, &option));
            printf("  - %.*s\n", (int)option.label.len, option.label.ptr);
        }
        enums += d.type == SDRKIT_TYPE_ENUM;
    }
    CHECK(enums > 0);

    /* Set a setting of the engine and read it back. */
    sdrkit_id callsign;
    OK(sdrkit_setting_find(engine, "callsign", &callsign));
    sdrkit_variant v = {0};
    v.type = SDRKIT_TYPE_STRING;
    v.string.ptr = "N0CALL";
    v.string.len = 6;
    OK(sdrkit_setting_set(engine, callsign, &v));
    sdrkit_variant got;
    OK(sdrkit_setting_get(engine, callsign, &got));
    CHECK(string_is(got.string, "N0CALL"));
    /* Save the settings, change one, and load the saved bytes back. The view is valid until the next call: copy it. */
    sdrkit_bytes view;
    OK(sdrkit_settings_export(engine, &view));
    CHECK(view.len > 6);
    size_t size = view.len;
    uint8_t *saved = malloc(size);
    CHECK(saved != NULL);
    memcpy(saved, view.ptr, size);
    v.string.ptr = "VK2XYZ";
    OK(sdrkit_setting_set(engine, callsign, &v));
    OK(sdrkit_settings_import(engine, saved, size));
    OK(sdrkit_setting_get(engine, callsign, &got));
    CHECK(string_is(got.string, "N0CALL"));

    /* Bytes of a later version, cut short or damaged are refused, and nothing changes. */
    saved[4] = 2;
    CHECK(sdrkit_settings_import(engine, saved, size) == SDRKIT_E_VERSION);
    saved[4] = 1;
    CHECK(sdrkit_settings_import(engine, saved, size - 1) == SDRKIT_E_IMPORT);
    saved[size / 2] ^= 0x5a;
    CHECK(sdrkit_settings_import(engine, saved, size) == SDRKIT_E_IMPORT);
    CHECK(sdrkit_settings_import(engine, (const uint8_t *)"{}", 2) == SDRKIT_E_IMPORT);
    OK(sdrkit_setting_get(engine, callsign, &got));
    CHECK(string_is(got.string, "N0CALL"));
    free(saved);

    OK(sdrkit_free(engine));
    printf("c_settings ok\n");
    return 0;
}
