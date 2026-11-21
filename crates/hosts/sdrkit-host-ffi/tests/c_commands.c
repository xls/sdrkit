/* A command with a reply, and a refused command with its error detail. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0;
    OK(sdrkit_engine_new(argv[1], &engine));

    uint32_t count = 0;
    OK(sdrkit_commands_count(engine, &count));
    for (sdrkit_id id = 0; id < count; id++) {
        sdrkit_command_info c = SDRKIT_COMMAND_INFO_INIT;
        OK(sdrkit_command_decl(engine, id, &c));
        printf("%.*s: %.*s\n", (int)c.key.len, c.key.ptr, (int)c.label.len, c.label.ptr);
    }

    sdrkit_id save, restore, state;
    OK(sdrkit_command_find(engine, "state.save", &save));
    uint32_t fields = 0;
    OK(sdrkit_command_invoke(engine, save, NULL, 0, &fields));
    CHECK(fields == 1);
    sdrkit_variant saved;
    OK(sdrkit_reply(engine, 0, &saved));
    CHECK(saved.type == SDRKIT_TYPE_BYTES && saved.bytes.len > 0);

    /* Without its required argument the command is refused before it runs. */
    OK(sdrkit_command_find(engine, "state.restore", &restore));
    OK(sdrkit_argument_find(engine, restore, "state", &state));
    sdrkit_decl arg = SDRKIT_DECL_INIT;
    OK(sdrkit_argument_decl(engine, restore, state, &arg));
    CHECK(arg.flags & SDRKIT_DECL_REQUIRED);
    CHECK(sdrkit_command_invoke(engine, restore, NULL, 0, &fields) == SDRKIT_E_MISSING_ARGUMENT);
    sdrkit_error_info detail = SDRKIT_ERROR_INFO_INIT;
    OK(sdrkit_error_detail(engine, &detail));
    CHECK(detail.setting == state);

    /* An argument of the wrong type is refused, and the error detail holds both types. */
    sdrkit_arg a = {0};
    a.id = state;
    a.value.type = SDRKIT_TYPE_F64;
    CHECK(sdrkit_command_invoke(engine, restore, &a, 1, &fields) == SDRKIT_E_WRONG_TYPE);
    OK(sdrkit_error_detail(engine, &detail));
    CHECK(detail.expected == SDRKIT_TYPE_BYTES && detail.got == SDRKIT_TYPE_F64);

    /* The saved state is restored. */
    a.value = saved;
    OK(sdrkit_command_invoke(engine, restore, &a, 1, &fields));
    OK(sdrkit_free(engine));
    printf("c_commands ok\n");
    return 0;
}
