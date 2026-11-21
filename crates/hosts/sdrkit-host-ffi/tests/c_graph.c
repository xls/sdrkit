/* A graph built from component types and started (the test signal of a stack into the transmit stream), and a graph on
 * samples from the caller, validated. tests/c_api.rs builds and runs it. */
#define _POSIX_C_SOURCE 199309L
#include "c_check.h"

int main(int argc, char **argv) {
    CHECK(argc == 2);
    SDRKIT_HANDLE engine = 0, graph = 0, gen_type = 0, session = 0;
    OK(sdrkit_engine_new(argv[1], &engine));
    OK(sdrkit_graph_new(engine, &graph));
    OK(sdrkit_graph_add(graph, "gen", "gen.stack"));
    OK(sdrkit_graph_add(graph, "tx", "tx.sink"));

    /* The settings of a component are those of its type. */
    OK(sdrkit_object_find(engine, "component.gen.stack", &gen_type));
    sdrkit_id stack;
    OK(sdrkit_setting_find(gen_type, "stack", &stack));
    sdrkit_variant fm = {0};
    fm.type = SDRKIT_TYPE_STRING;
    fm.string.ptr = "fm";
    fm.string.len = 2;
    OK(sdrkit_graph_set(graph, "gen", stack, &fm));

    uint32_t ports = 0;
    OK(sdrkit_graph_ports(graph, "gen", &ports));
    for (uint32_t n = 0; n < ports; n++) {
        sdrkit_port_info p = SDRKIT_PORT_INFO_INIT;
        OK(sdrkit_graph_port(graph, "gen", n, &p));
        printf("gen %s %.*s\n", p.output ? "writes" : "reads", (int)p.name.len, p.name.ptr);
    }
    OK(sdrkit_graph_connect(graph, "gen", "tx"));
    CHECK(sdrkit_graph_connect(graph, "gen", "tx") == SDRKIT_E_CONNECT);
    sdrkit_error_info detail = SDRKIT_ERROR_INFO_INIT;
    OK(sdrkit_error_detail(engine, &detail));
    CHECK(string_is(detail.component, "tx") && string_is(detail.from, "gen.out") && string_is(detail.to, "tx.in"));

    sdrkit_input inside = sdrkit_input_inside();
    OK(sdrkit_graph_start(graph, &inside, &session));
    static float iq[2 * 4096];
    sdrkit_iq_info got = SDRKIT_IQ_INFO_INIT;
    for (int i = 0; i < 2000 && got.samples == 0; i++) {
        OK(sdrkit_transmit_read(session, iq, 4096, &got));
        if (got.samples == 0) pause_a_moment();
    }
    CHECK(got.samples > 0 && got.rate_hz > 0.0);

    /* A component of a running graph is an object, found by its name. */
    SDRKIT_HANDLE gen = 0;
    OK(sdrkit_object_component(session, "gen", &gen));
    sdrkit_variant running;
    OK(sdrkit_setting_get(gen, stack, &running));
    CHECK(string_is(running.string, "fm"));

    /* A graph on samples from the caller: validate resolves the format of every port. */
    SDRKIT_HANDLE plain = 0;
    OK(sdrkit_graph_new(engine, &plain));
    OK(sdrkit_graph_add(plain, "tx", "tx.sink"));
    OK(sdrkit_graph_connect(plain, "input", "tx.in"));
    sdrkit_report_info report = SDRKIT_REPORT_INFO_INIT;
    OK(sdrkit_graph_validate(plain, 48000.0, 1e6, &report));
    for (uint32_t n = 0; n < report.ports; n++) {
        sdrkit_report_port p = SDRKIT_REPORT_PORT_INIT;
        OK(sdrkit_graph_report_port(plain, n, &p));
        printf("%.*s at %.0f Hz\n", (int)p.name.len, p.name.ptr, p.rate_hz);
    }
    CHECK(report.ports >= 1);
    OK(sdrkit_free(engine));
    printf("c_graph ok\n");
    return 0;
}
