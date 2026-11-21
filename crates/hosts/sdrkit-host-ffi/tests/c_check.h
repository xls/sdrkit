/* Shared by the C test programs: a check that prints the line that failed, and small helpers. */
#ifndef SDRKIT_C_CHECK_H
#define SDRKIT_C_CHECK_H
#include "sdrkit.h"
#include <stdio.h>
#include <string.h>
#include <time.h>

#define CHECK(cond)                                                               \
    do {                                                                          \
        if (!(cond)) {                                                            \
            fprintf(stderr, "%s:%d: failed: %s\n", __FILE__, __LINE__, #cond);    \
            return 1;                                                             \
        }                                                                         \
    } while (0)

/* A call that must succeed. */
#define OK(call) CHECK((call) == SDRKIT_OK)

/* Returns true if `s` holds the C string `c`. */
static inline int string_is(sdrkit_string s, const char *c) {
    return s.len == strlen(c) && memcmp(s.ptr, c, s.len) == 0;
}

/* Waits about 10 ms. */
static inline void pause_a_moment(void) {
    struct timespec t = {0, 10 * 1000 * 1000};
    nanosleep(&t, NULL);
}

#endif
