/* list_stacks: lists every stack of the library with its settings, tables and commands. It reads them through the
   generic interface: the declarations that a program finds at run time, without a header of the stack.

   Usage: list_stacks [stack]
     stack  only this stack, such as "fm". The default is every stack. */
#include <sdrkit.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static SDRKIT_HANDLE engine;

/* Ends the program with the message of the engine if `code` is an error. */
static void must(int32_t code, const char *step) {
    sdrkit_string why = {"", 0};
    if (code == SDRKIT_OK) return;
    if (sdrkit_error_message(engine, &why) != SDRKIT_OK) why.len = 0;
    fprintf(stderr, "%s: %s. %.*s\n", step, sdrkit_code_name(code), (int)why.len, why.ptr);
    exit(1);
}

/* The name of an SDRKIT_TYPE_* constant. */
static const char *type_name(int32_t type) {
    static const char *names[] = {"none", "bool", "i64", "f64", "string", "Hz", "time", "duration", "position",
                                  "enum", "bytes", "any"};
    return type >= 0 && type < (int32_t)(sizeof names / sizeof names[0]) ? names[type] : "?";
}

/* Prints the settings, the tables with their columns, and the commands of the object of `object`. */
static void print_object(SDRKIT_HANDLE object) {
    uint32_t count = 0;
    sdrkit_decl decl = SDRKIT_DECL_INIT;
    sdrkit_results_info results = SDRKIT_RESULTS_INFO_INIT;
    sdrkit_table_info table = SDRKIT_TABLE_INFO_INIT;
    sdrkit_command_info command = SDRKIT_COMMAND_INFO_INIT;

    /* Settings: ids go from 0 to count - 1. Each declaration gives the key, the label, the type and the unit. */
    must(sdrkit_settings_count(object, &count), "settings");
    printf("  %u settings\n", count);
    for (uint32_t id = 0; id < count; id++) {
        must(sdrkit_setting_decl(object, id, &decl), "setting");
        printf("    %-24.*s %-8s %-6.*s %.*s\n", (int)decl.key.len, decl.key.ptr, type_name(decl.type),
               (int)decl.unit.len, decl.unit.ptr, (int)decl.label.len, decl.label.ptr);
    }

    /* Tables: the declaration of the results gives their number, each table its columns. */
    must(sdrkit_results_decl(object, &results), "results");
    printf("  %u result fields, %u tables\n", results.fields, results.tables);
    for (uint32_t t = 0; t < results.tables; t++) {
        must(sdrkit_table_decl(object, t, &table), "table");
        printf("    %.*s (%.*s):", (int)table.key.len, table.key.ptr, (int)table.label.len, table.label.ptr);
        uint32_t columns = table.columns;
        for (uint32_t c = 0; c < columns; c++) {
            must(sdrkit_column_decl(object, t, c, &decl), "column");
            printf(" %.*s", (int)decl.key.len, decl.key.ptr);
        }
        printf("\n");
    }

    /* Commands, with the number of their arguments. */
    must(sdrkit_commands_count(object, &count), "commands");
    printf("  %u commands\n", count);
    for (uint32_t id = 0; id < count; id++) {
        must(sdrkit_command_decl(object, id, &command), "command");
        printf("    %-24.*s %u arguments  %.*s\n", (int)command.key.len, command.key.ptr, command.arguments,
               (int)command.label.len, command.label.ptr);
    }
}

int main(int argc, char **argv) {
    const char *only = argc > 1 ? argv[1] : NULL;
    must(sdrkit_engine_new(NULL, &engine), "engine");

    /* The object list of the engine: the engine, its radios, sessions and services, then the types of the catalog.
       A stack type has the key "stack.<id>". The keys are views, so they are copied before the next call. */
    uint32_t objects = 0, stacks = 0;
    must(sdrkit_objects(engine, &objects), "objects");
    char **keys = calloc(objects, sizeof *keys);
    for (uint32_t n = 0; n < objects && keys; n++) {
        sdrkit_string key;
        int32_t type = 0;
        must(sdrkit_object(engine, n, &key, &type), "object");
        if (type != SDRKIT_OBJECT_STACK_TYPE) continue;
        keys[stacks] = malloc(key.len + 1);
        memcpy(keys[stacks], key.ptr, key.len);
        keys[stacks][key.len] = '\0';
        stacks++;
    }

    uint32_t shown = 0;
    for (uint32_t s = 0; s < stacks; s++) {
        const char *id = keys[s] + strlen("stack.");
        if (only == NULL || strcmp(id, only) == 0) {
            /* A handle of the stack type. Its settings hold the defaults of a new session. */
            SDRKIT_HANDLE stack;
            must(sdrkit_object_find(engine, keys[s], &stack), "find");
            printf("%s\n", id);
            print_object(stack);
            must(sdrkit_free(stack), "free the handle");
            shown++;
        }
        free(keys[s]);
    }
    free(keys);
    must(sdrkit_free(engine), "free");
    printf("%u of %u stacks\n", shown, stacks);
    return shown > 0 ? 0 : 1;
}
