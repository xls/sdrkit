/* The presets calls over the data folder of the repository: the plans of a country, the channels of a scan and the
   frequency entries for an engine, a search, labels, names, steps and the user's plan file. tests/c_api.rs builds and
   runs it when the library has the feature `presets`. */
#include "c_check.h"
#include "sdrkit_presets.h"

/* Returns true if the list of `presets` holds the plan id `id`. */
static int lists(SDRKIT_HANDLE presets, uint32_t n, const char *id) {
    for (uint32_t i = 0; i < n; i++) {
        sdrkit_string s;
        if (sdrkit_presets_plan_id(presets, i, &s) == SDRKIT_OK && string_is(s, id)) return 1;
    }
    return 0;
}

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE presets = 0, engine = 0;
    sdrkit_string m;
    CHECK(sdrkit_presets_open("/no/such/folder", &presets) == SDRKIT_E_NOT_FOUND);
    OK(sdrkit_presets_error_message(0, &m));
    CHECK(m.len > 0);
    OK(sdrkit_presets_open(SDRKIT_DATA_FOLDER, &presets));
    CHECK(sdrkit_presets_plan_ids(engine, "AU", &(uint32_t){0}) == SDRKIT_E_NO_SUCH_OBJECT);

    /* The plans of two countries differ. */
    uint32_t n = 0;
    OK(sdrkit_presets_plan_ids(presets, "AU", &n));
    CHECK(n > 3 && lists(presets, n, "tv_au") && !lists(presets, n, "tv_us"));
    CHECK(sdrkit_presets_plan_id(presets, n, &m) == SDRKIT_E_NO_SUCH_ID);
    OK(sdrkit_presets_plan_ids(presets, "US", &n));
    CHECK(lists(presets, n, "tv_us") && !lists(presets, n, "tv_au"));
    OK(sdrkit_presets_plan_ids(presets, "XX", &n));
    CHECK(n == 0);

    /* The engine takes the channels of a scan and the frequency entries from the host. */
    const char *au[] = {"tv_au", "dab_band3"};
    uint32_t stacks = 0, entries = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_presets_scan_channels_set(presets, engine, au, 2, &stacks));
    CHECK(stacks >= 2);
    OK(sdrkit_presets_frequency_entries_set(presets, engine, &entries));
    CHECK(entries > 100);
    CHECK(sdrkit_presets_scan_channels_set(presets, presets, au, 2, NULL) == SDRKIT_E_NO_SUCH_OBJECT);

    /* A name, a typed channel and a step. */
    sdrkit_presets_place place = SDRKIT_PRESETS_PLACE_INIT;
    OK(sdrkit_presets_name_of(presets, 226.5e6, au, 1, &place));
    CHECK(string_is(place.name, "12") && string_is(place.plan, "tv_au") && place.has_width_hz && place.width_hz == 7e6);
    CHECK(sdrkit_presets_name_of(presets, 226.5e6, NULL, 0, &place) == SDRKIT_E_NOT_FOUND);
    OK(sdrkit_presets_channel_named(presets, "ch 9A", au, 2, &place));
    CHECK(place.centre_hz == 205.5e6);
    OK(sdrkit_presets_step(presets, "tv_au", 226.5e6, false, false, &place));
    CHECK(place.centre_hz < 226.5e6 && place.name.len > 0);
    CHECK(sdrkit_presets_step(presets, "no_such_plan", 1e6, true, false, &place) == SDRKIT_E_NOT_FOUND);

    /* The channels, the dial and the settings of a stack in a country. */
    sdrkit_presets_query query = SDRKIT_PRESETS_QUERY_INIT;
    sdrkit_presets_channel channel = SDRKIT_PRESETS_CHANNEL_INIT;
    sdrkit_presets_dial_info dial = SDRKIT_PRESETS_DIAL_INFO_INIT;
    sdrkit_presets_setting setting = SDRKIT_PRESETS_SETTING_INIT;
    query.stack = "dvbt";
    query.country = "AU";
    OK(sdrkit_presets_channels(presets, &query, &n));
    CHECK(n > 30);
    OK(sdrkit_presets_channel_read(presets, 0, &channel));
    CHECK(channel.frequency_hz == 177.5e6 && channel.has_width_hz && channel.width_hz == 7e6 && channel.label.len > 0);
    CHECK(sdrkit_presets_channel_read(presets, n, &channel) == SDRKIT_E_NO_SUCH_ID);
    OK(sdrkit_presets_dial(presets, &query, &dial));
    CHECK(dial.frequency_hz == 177.5e6 && dial.plan.len > 0 && dial.has_width_hz);
    OK(sdrkit_presets_settings(presets, engine, &query, &n));
    CHECK(n >= 2);
    int width = 0;
    for (uint32_t i = 0; i < n; i++) {
        OK(sdrkit_presets_setting_read(presets, i, &setting));
        if (string_is(setting.key, "bandwidth")) width = setting.arg.value.type == SDRKIT_TYPE_ENUM;
    }
    CHECK(width);
    CHECK(sdrkit_presets_setting_read(presets, n, &setting) == SDRKIT_E_NO_SUCH_ID);
    /* The dials of a stack, as rows that the channel read reads. */
    query.stack = "ft8";
    OK(sdrkit_presets_dials(presets, &query, &n));
    CHECK(n > 5);
    int twenty = 0;
    for (uint32_t i = 0; i < n; i++) {
        OK(sdrkit_presets_channel_read(presets, i, &channel));
        if (string_is(channel.label, "20m")) twenty = channel.frequency_hz == 14.074e6 && !channel.has_width_hz;
    }
    CHECK(twenty);
    query.stack = "wsjt";
    query.submode = "ft4";
    uint32_t in_ft4 = 0;
    OK(sdrkit_presets_dials(presets, &query, &in_ft4));
    CHECK(in_ft4 > 0 && in_ft4 < n);
    query.submode = NULL;
    query.stack = "dvbt";
    OK(sdrkit_presets_dials(presets, &query, &n));
    CHECK(n == 0);
    /* The supplied frequency entries of a stack in a country. */
    sdrkit_presets_frequency frequency = SDRKIT_PRESETS_FREQUENCY_INIT;
    query.stack = "ft8";
    OK(sdrkit_presets_frequencies(presets, &query, &n));
    CHECK(n > 5);
    int ft8_20m = 0;
    for (uint32_t i = 0; i < n; i++) {
        OK(sdrkit_presets_frequency_read(presets, i, &frequency));
        if (frequency.frequency_hz == 14.074e6)
            ft8_20m = frequency.id.len > 0 && frequency.name.len > 0 && frequency.has_bandwidth_hz;
    }
    CHECK(ft8_20m);
    CHECK(sdrkit_presets_frequency_read(presets, n, &frequency) == SDRKIT_E_NO_SUCH_ID);
    query.country = "X";
    CHECK(sdrkit_presets_frequencies(presets, &query, &n) == SDRKIT_E_BAD_ARGUMENT);
    query.country = "AU";
    /* Why a stack has no channels in a country: 3G ended in Australia in 2024, GSM in 2018. */
    sdrkit_presets_service_info end = SDRKIT_PRESETS_SERVICE_INFO_INIT;
    query.stack = "umts";
    OK(sdrkit_presets_service_end(presets, &query, &end));
    CHECK(end.out_of_service && !end.partial && end.has_year && end.year == 2024 && string_is(end.ended, "2024"));
    CHECK(string_is(end.sentence, "3G (UMTS) was switched off in Australia in 2024"));
    query.stack = "gsmr";
    OK(sdrkit_presets_service_end(presets, &query, &end));
    CHECK(end.out_of_service && !end.partial && end.year == 2018);
    CHECK(string_is(end.sentence, "2G (GSM) was switched off in Australia in 2018"));
    query.stack = "dvbt";
    OK(sdrkit_presets_service_end(presets, &query, &end));
    CHECK(!end.out_of_service && !end.has_year && end.sentence.len == 0);

    /* The Meshtastic slots of a region with no session: ANZ LongFast has 52, slot 20 at 919.875 MHz is current. */
    sdrkit_presets_slot_query slots = SDRKIT_PRESETS_SLOT_QUERY_INIT;
    sdrkit_presets_slot slot = SDRKIT_PRESETS_SLOT_INIT;
    slots.region = "ANZ";
    OK(sdrkit_presets_meshtastic_slots(presets, &slots, &n));
    CHECK(n == 52);
    OK(sdrkit_presets_meshtastic_slot_read(presets, 19, &slot));
    CHECK(slot.slot == 20 && slot.frequency_hz == 919.875e6 && slot.current && slot.preset_default);
    CHECK(sdrkit_presets_meshtastic_slot_read(presets, n, &slot) == SDRKIT_E_NO_SUCH_ID);
    slots.channel_num = 53;
    CHECK(sdrkit_presets_meshtastic_slots(presets, &slots, &n) == SDRKIT_E_OUT_OF_RANGE);
    slots.channel_num = 0;
    slots.region = "Atlantis";
    CHECK(sdrkit_presets_meshtastic_slots(presets, &slots, &n) == SDRKIT_E_NOT_FOUND);

    /* The countries that have plans. */
    sdrkit_presets_country country = SDRKIT_PRESETS_COUNTRY_INIT;
    OK(sdrkit_presets_countries(presets, &n));
    CHECK(n > 20);
    int australia = 0;
    for (uint32_t i = 0; i < n; i++) {
        OK(sdrkit_presets_country_read(presets, i, &country));
        if (string_is(country.code, "AU")) australia = string_is(country.name, "Australia");
    }
    CHECK(australia);
    CHECK(sdrkit_presets_country_read(presets, n, &country) == SDRKIT_E_NO_SUCH_ID);

    query.country = "XX";
    CHECK(sdrkit_presets_dial(presets, &query, &dial) == SDRKIT_E_NOT_FOUND);
    query.country = "Australia";
    CHECK(sdrkit_presets_channels(presets, &query, &n) == SDRKIT_E_BAD_ARGUMENT);
    query.stack = "no_such_stack";
    query.country = "AU";
    CHECK(sdrkit_presets_settings(presets, engine, &query, &n) != SDRKIT_OK);

    /* A search by frequency finds the channel first. */
    uint32_t found = 0;
    sdrkit_presets_hit hit = SDRKIT_PRESETS_HIT_INIT;
    OK(sdrkit_presets_search(presets, "226.5", au, 2, &found));
    CHECK(found > 0);
    OK(sdrkit_presets_hit_read(presets, 0, &hit));
    CHECK(hit.kind == SDRKIT_PRESETS_HIT_CHANNEL && hit.has_frequency_hz && hit.frequency_hz == 226.5e6);
    CHECK(sdrkit_presets_hit_read(presets, found, &hit) == SDRKIT_E_NO_SUCH_ID);
    OK(sdrkit_presets_search(presets, "aprs", NULL, 0, &found));
    CHECK(found > 0);

    /* Labels of a span. */
    sdrkit_presets_view view = SDRKIT_PRESETS_VIEW_INIT;
    sdrkit_presets_label label = SDRKIT_PRESETS_LABEL_INIT;
    CHECK(sdrkit_presets_labels(presets, au, 1, NULL, &view, &found) == SDRKIT_E_BAD_ARGUMENT);
    view.lo_hz = 174e6;
    view.hi_hz = 230e6;
    view.width_px = 1200;
    OK(sdrkit_presets_labels(presets, au, 1, "AU", &view, &found));
    CHECK(found > 0);
    OK(sdrkit_presets_label_read(presets, 0, &label));
    CHECK(label.text.len > 0 && label.hz >= 174e6 && label.hz <= 230e6);

    /* The user's plans: an import writes the file, a removal takes the plan away. */
    char file[1024], csv[1024], out[1024];
    snprintf(file, sizeof file, "%s/mine.json", argv[1]);
    snprintf(csv, sizeof csv, "%s/list.csv", argv[1]);
    snprintf(out, sizeof out, "%s/out.csv", argv[1]);
    FILE *f = fopen(csv, "w");
    CHECK(f != NULL);
    fputs("# modes: nfm\nname,centre_hz,width_hz\nA,100000000,200000\nB,100200000,200000\n", f);
    fclose(f);
    OK(sdrkit_presets_personal_plans(presets, file, &n));
    CHECK(n == 0);
    OK(sdrkit_presets_plan_import(presets, csv, SDRKIT_PRESETS_FORMAT_CSV, "my_list", "My list", &n));
    CHECK(n == 1);
    const char *mine[] = {"my_list"};
    OK(sdrkit_presets_name_of(presets, 100.2e6, mine, 1, &place));
    CHECK(string_is(place.name, "B"));
    /* The plan lists the stack and no country: its channels are channels of the stack in every country. */
    query.stack = "nfm";
    query.country = "DE";
    OK(sdrkit_presets_channels(presets, &query, &n));
    int own = 0;
    for (uint32_t i = 0; i < n; i++) {
        OK(sdrkit_presets_channel_read(presets, i, &channel));
        own += channel.frequency_hz == 100.2e6 && string_is(channel.label, "B");
    }
    CHECK(own == 1);
    CHECK(sdrkit_presets_plan_import(presets, file, 9, "x", "X", NULL) == SDRKIT_E_BAD_ARGUMENT);
    CHECK(sdrkit_presets_plan_import(presets, out, SDRKIT_PRESETS_FORMAT_CSV, "x", "X", NULL) == SDRKIT_E_NOT_FOUND);
    OK(sdrkit_presets_plan_export(presets, "my_list", SDRKIT_PRESETS_FORMAT_CSV, out));
    CHECK(sdrkit_presets_plan_export(presets, "no_such_plan", SDRKIT_PRESETS_FORMAT_CSV, out) == SDRKIT_E_NOT_FOUND);
    OK(sdrkit_presets_plan_remove(presets, "my_list"));
    CHECK(sdrkit_presets_plan_remove(presets, "my_list") == SDRKIT_E_NOT_FOUND);
    OK(sdrkit_presets_error_message(presets, &m));
    CHECK(m.len > 0);

    /* A closed handle stays closed when the next open takes its place. */
    SDRKIT_HANDLE next = 0;
    OK(sdrkit_presets_close(presets));
    CHECK(sdrkit_presets_close(presets) == SDRKIT_E_NO_SUCH_OBJECT);
    OK(sdrkit_presets_open(SDRKIT_DATA_FOLDER, &next));
    CHECK(next != presets && sdrkit_presets_plan_ids(presets, "AU", &n) == SDRKIT_E_NO_SUCH_OBJECT);
    OK(sdrkit_presets_close(next));
    OK(sdrkit_free(engine));
    printf("c_presets ok\n");
    return 0;
}
