// any_stack: runs any stack by its id and prints its result fields and tables through the generic interface. It knows
// no stack: names, types and options come from the declarations at run time.
//
// Usage: any_stack <stack> [source] [seconds] [rows]
//   stack    a stack id, such as "fm", "adsb" or "meshtastic". list_stacks prints them all.
//   source   "synth:<stack>" (the default) is the test signal of the stack. A radio is an address such as
//            "rtl_tcp:192.168.1.20:1234".
//   seconds  how long to run before the results are printed, 5 by default.
//   rows     the most rows to print of each table, 5 by default.
#include <sdrkit.h>

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <stdexcept>
#include <string>
#include <thread>

namespace {

// Throws if `code` is not SDRKIT_OK, with the sentence of the engine of `any`.
void check(int32_t code, SDRKIT_HANDLE any, const char *step) {
    if (code == SDRKIT_OK) return;
    sdrkit_string why = {"", 0};
    if (sdrkit_error_message(any, &why) != SDRKIT_OK) why.len = 0;
    throw std::runtime_error(std::string(step) + ": " + sdrkit_code_name(code) + ". " + std::string(why.ptr, why.len));
}

std::string text(sdrkit_string s) { return std::string(s.ptr, s.len); }

// A value as text, with its unit. `option` gives the label of option n of an enum value.
template <typename Option> std::string show(const sdrkit_variant &v, const std::string &unit, Option option) {
    char buf[64];
    switch (v.type) {
    case SDRKIT_TYPE_NONE: return "-";
    case SDRKIT_TYPE_BOOL: return v.flag ? "yes" : "no";
    case SDRKIT_TYPE_I64: return std::to_string(static_cast<long long>(v.integer));
    case SDRKIT_TYPE_STRING: return text(v.string);
    case SDRKIT_TYPE_ENUM: return option(v.enum_index);
    case SDRKIT_TYPE_BYTES: return std::to_string(v.bytes.len) + " bytes";
    case SDRKIT_TYPE_POSITION: std::snprintf(buf, sizeof buf, "%.5f %.5f", v.lat, v.lon); return buf;
    default:
        std::snprintf(buf, sizeof buf, "%g", v.number);
        return unit.empty() ? std::string(buf) : std::string(buf) + " " + unit;
    }
}

// Prints the result fields of the snapshot of `h`, then at most `most` rows of each table.
void print_results(SDRKIT_HANDLE h, uint32_t most) {
    sdrkit_results_info results = SDRKIT_RESULTS_INFO_INIT;
    check(sdrkit_results_decl(h, &results), h, "results");
    for (sdrkit_id f = 0; f < results.fields; f++) {
        sdrkit_decl decl = SDRKIT_DECL_INIT;
        sdrkit_variant v;
        check(sdrkit_field_decl(h, f, &decl), h, "field");
        const std::string key = text(decl.key), unit = text(decl.unit);
        check(sdrkit_snapshot_field(h, f, &v), h, "field value");
        if (v.type == SDRKIT_TYPE_NONE) continue; // absent now
        const std::string value = show(v, unit, [&](uint32_t n) {
            sdrkit_option o = SDRKIT_OPTION_INIT;
            return sdrkit_field_option(h, f, n, &o) == SDRKIT_OK ? text(o.label) : std::to_string(n);
        });
        std::printf("  %-28s %s\n", key.c_str(), value.c_str());
    }
    for (sdrkit_id t = 0; t < results.tables; t++) {
        sdrkit_table_info table = SDRKIT_TABLE_INFO_INIT;
        sdrkit_rows_info rows = SDRKIT_ROWS_INFO_INIT;
        check(sdrkit_table_decl(h, t, &table), h, "table");
        const std::string key = text(table.key);
        const uint32_t columns = table.columns;
        check(sdrkit_snapshot_table(h, t, &rows), h, "rows");
        std::printf("table %s: %u rows\n", key.c_str(), rows.total);
        for (uint32_t r = 0; r < rows.rows && r < most; r++) {
            std::string line;
            for (sdrkit_id c = 0; c < columns; c++) {
                sdrkit_decl decl = SDRKIT_DECL_INIT;
                sdrkit_variant v;
                check(sdrkit_column_decl(h, t, c, &decl), h, "column");
                const std::string column = text(decl.key), unit = text(decl.unit);
                check(sdrkit_snapshot_cell(h, t, r, c, &v), h, "cell");
                if (v.type == SDRKIT_TYPE_NONE) continue;
                line += "  " + column + "=" + show(v, unit, [&](uint32_t n) {
                    sdrkit_option o = SDRKIT_OPTION_INIT;
                    return sdrkit_column_option(h, t, c, n, &o) == SDRKIT_OK ? text(o.label) : std::to_string(n);
                });
            }
            std::printf("   %s\n", line.c_str());
        }
    }
}

} // namespace

int main(int argc, char **argv) {
    if (argc < 2) {
        std::fprintf(stderr, "usage: any_stack <stack> [source] [seconds] [rows]\n");
        return 2;
    }
    const std::string stack = argv[1];
    const std::string source = argc > 2 ? argv[2] : "synth:" + stack;
    const double seconds = argc > 3 ? std::atof(argv[3]) : 5.0;
    const uint32_t most = argc > 4 ? static_cast<uint32_t>(std::atoi(argv[4])) : 5;
    SDRKIT_HANDLE engine = 0, radio = 0, session = 0;
    try {
        check(sdrkit_engine_new(nullptr, &engine), 0, "engine");
        check(sdrkit_radio_open(engine, source.c_str(), nullptr, &radio), engine, "radio");
        sdrkit_input input = sdrkit_input_radio(radio);
        check(sdrkit_stack_start(engine, stack.c_str(), nullptr, &input, &session), engine, "start");

        // Audio that no one reads is dropped, so a stack with sound needs no reader here.
        std::this_thread::sleep_for(std::chrono::milliseconds(static_cast<int>(seconds * 1000.0)));

        sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
        check(sdrkit_snapshot_read(session, 0, &snapshot), session, "snapshot");
        const char *states[] = {"off", "searching", "tracking", "locked"};
        std::printf("%s on %s: %s, %s\n", stack.c_str(), source.c_str(),
                    snapshot.state >= 0 && snapshot.state < 4 ? states[snapshot.state] : "?",
                    text(snapshot.summary).c_str());
        print_results(session, most);
        sdrkit_session_stop(session);
        sdrkit_free(engine);
        return 0;
    } catch (const std::exception &e) {
        std::fprintf(stderr, "%s\n", e.what());
        if (engine) sdrkit_free(engine);
        return 1;
    }
}
