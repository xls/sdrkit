/* stack_count: opens an engine, prints the version of the library and the number of its stacks, and exits. The three
   CMake projects of examples/cmake build it.

   Usage: stack_count */
#include <sdrkit.h>
#include <stdio.h>

int main(void) {
    SDRKIT_HANDLE engine;
    sdrkit_string version = {"", 0};
    uint32_t objects = 0, stacks = 0;

    if (sdrkit_engine_new(NULL, &engine) != SDRKIT_OK) {
        fprintf(stderr, "sdrkit_engine_new failed\n");
        return 1;
    }
    sdrkit_version(&version);
    /* The object list of the engine holds one object of the type SDRKIT_OBJECT_STACK_TYPE for each stack. */
    if (sdrkit_objects(engine, &objects) != SDRKIT_OK) {
        fprintf(stderr, "sdrkit_objects failed\n");
        sdrkit_free(engine);
        return 1;
    }
    for (uint32_t n = 0; n < objects; n++) {
        sdrkit_string key;
        int32_t type = 0;
        if (sdrkit_object(engine, n, &key, &type) == SDRKIT_OK && type == SDRKIT_OBJECT_STACK_TYPE) stacks++;
    }
    sdrkit_free(engine);
    printf("sdrkit %.*s: %u stacks\n", (int)version.len, version.ptr, stacks);
    return stacks > 0 ? 0 : 1;
}
