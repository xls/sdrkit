/* A table of the engine, read from a snapshot, and read again by its change counter. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0;
    OK(sdrkit_engine_new(argv[1], &engine));

    sdrkit_id stacks, name;
    OK(sdrkit_table_find(engine, "stacks", &stacks));
    sdrkit_table_info table = SDRKIT_TABLE_INFO_INIT;
    OK(sdrkit_table_decl(engine, stacks, &table));
    OK(sdrkit_column_find(engine, stacks, "name", &name));

    sdrkit_snapshot_info info = SDRKIT_SNAPSHOT_INFO_INIT;
    OK(sdrkit_snapshot_read(engine, 0, &info));
    sdrkit_rows_info rows = SDRKIT_ROWS_INFO_INIT;
    OK(sdrkit_snapshot_table(engine, stacks, &rows));
    CHECK(rows.rows == rows.total && rows.total > 0);
    for (uint32_t n = 0; n < rows.rows && n < 5; n++) {
        sdrkit_string key;
        sdrkit_variant cell;
        OK(sdrkit_snapshot_row(engine, stacks, n, &key));
        OK(sdrkit_snapshot_cell(engine, stacks, n, name, &cell));
        CHECK(cell.type == SDRKIT_TYPE_STRING);
        printf("%.*s: %.*s\n", (int)key.len, key.ptr, (int)cell.string.len, cell.string.ptr);
    }

    /* Nothing changed: the next snapshot after the counter holds no row. */
    OK(sdrkit_snapshot_read(engine, info.counter, &info));
    OK(sdrkit_snapshot_table(engine, stacks, &rows));
    CHECK(rows.rows == 0);
    OK(sdrkit_free(engine));
    printf("c_results ok\n");
    return 0;
}
