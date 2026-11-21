/* fm_audio: receives FM, writes the audio to a WAV file, and prints the station name from RDS, stereo and the signal
   quality from the typed result set of the stack.

   Usage: fm_audio [source] [seconds] [MHz] [file]
     source   "synth:fm" (the default) is a test signal. A radio is an address such as "rtl_tcp:192.168.1.20:1234".
     seconds  how much audio to write, 5 by default.
     MHz      the frequency to tune to. The test signal needs none.
     file     the WAV file, "fm_audio.wav" by default. */
#ifndef _WIN32
#define _POSIX_C_SOURCE 200112L
#endif
#include <sdrkit.h>
#include <sdrkit/fm.h>
#include <stdio.h>
#include <stdlib.h>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
static void pause_ms(unsigned ms) { Sleep(ms); }
#else
#include <time.h>
static void pause_ms(unsigned ms) {
    struct timespec t;
    t.tv_sec = 0;
    t.tv_nsec = (long)ms * 1000000L;
    nanosleep(&t, NULL);
}
#endif

static SDRKIT_HANDLE engine;

/* Ends the program with the message of the engine if `code` is an error. */
static void must(int32_t code, const char *step) {
    sdrkit_string why = {"", 0};
    if (code == SDRKIT_OK) return;
    if (sdrkit_error_message(engine, &why) != SDRKIT_OK) why.len = 0;
    fprintf(stderr, "%s: %s. %.*s\n", step, sdrkit_code_name(code), (int)why.len, why.ptr);
    exit(1);
}

/* Writes `v` as `bytes` little-endian bytes. */
static void put_le(FILE *f, uint32_t v, int bytes) {
    for (int i = 0; i < bytes; i++) fputc((int)((v >> (8 * i)) & 0xff), f);
}

/* Writes the 44-byte header of a 16-bit PCM WAV file with `data_bytes` bytes of samples. */
static void wav_header(FILE *f, uint32_t rate, uint32_t channels, uint32_t data_bytes) {
    fwrite("RIFF", 1, 4, f);
    put_le(f, 36 + data_bytes, 4);
    fwrite("WAVEfmt ", 1, 8, f);
    put_le(f, 16, 4);                 /* size of the format chunk */
    put_le(f, 1, 2);                  /* PCM */
    put_le(f, channels, 2);
    put_le(f, rate, 4);
    put_le(f, rate * channels * 2, 4); /* bytes per second */
    put_le(f, channels * 2, 2);       /* bytes per frame */
    put_le(f, 16, 2);                 /* bits per sample */
    fwrite("data", 1, 4, f);
    put_le(f, data_bytes, 4);
}

int main(int argc, char **argv) {
    const char *source = argc > 1 ? argv[1] : "synth:fm";
    double seconds = argc > 2 ? atof(argv[2]) : 5.0;
    double mhz = argc > 3 ? atof(argv[3]) : 0.0;
    const char *path = argc > 4 ? argv[4] : "fm_audio.wav";
    SDRKIT_HANDLE radio, session;

    /* An engine with its files in memory, the radio, and a session of the stack "fm" that reads the radio. */
    must(sdrkit_engine_new(NULL, &engine), "engine");
    must(sdrkit_radio_open(engine, source, NULL, &radio), "radio");
    sdrkit_input input = sdrkit_input_radio(radio);
    must(sdrkit_stack_start(engine, "fm", NULL, &input, &session), "start");
    /* The typed call of the setting `tuned_hz` of every session (sdrkit/session.h). */
    if (mhz > 0) must(sdrkit_session_set_tuned_hz(session, mhz * 1e6), "frequency");

    FILE *wav = fopen(path, "wb");
    if (!wav) {
        fprintf(stderr, "cannot write %s\n", path);
        return 1;
    }
    wav_header(wav, 0, 0, 0); /* written again with the sizes at the end */

    static float audio[16384];
    sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;
    uint32_t rate = 0, channels = 0, data_bytes = 0;
    double frames = 0;
    /* One pass each 20 ms. The loop ends after `seconds` of audio, or after three times that if audio is late. */
    int passes = (int)(seconds * 150.0) + 50;
    for (int pass = 0; pass < passes && (rate == 0 || frames < seconds * rate); pass++) {
        /* Copies the audio that waits: whole interleaved frames, with their rate and channel count in `got`. */
        must(sdrkit_audio_read(session, audio, sizeof audio / sizeof audio[0], &got), "audio");
        if (got.frames > 0 && rate == 0) {
            rate = (uint32_t)got.rate_hz;
            channels = got.channels;
        }
        for (size_t i = 0; i < got.frames * got.channels; i++) {
            float s = audio[i] > 1.0f ? 1.0f : audio[i] < -1.0f ? -1.0f : audio[i];
            put_le(wav, (uint32_t)(uint16_t)(int16_t)(s * 32767.0f), 2);
        }
        data_bytes += (uint32_t)(got.frames * got.channels * 2);
        frames += (double)got.frames;

        if (pass % 50 == 49) {
            /* One copy of the results, then the result set of the stack as a struct. Its strings are views that
               stay valid until the next call on the session. */
            sdrkit_snapshot_info snapshot = SDRKIT_SNAPSHOT_INFO_INIT;
            sdrkit_fm_fields fm;
            double tuned = 0;
            must(sdrkit_snapshot_read(session, 0, &snapshot), "snapshot");
            must(sdrkit_fm_fields_read(session, &fm), "fields");
            must(sdrkit_session_get_tuned_hz(session, &tuned), "frequency");
            printf("%.3f MHz  station \"%.*s\"  %s  SNR %.1f dB  RDS %.0f %%\n", tuned / 1e6, (int)fm.station.len,
                   fm.station.ptr, fm.stereo ? "stereo" : "mono", fm.snr_db, fm.rds_ok_pct);
            fflush(stdout);
        }
        pause_ms(20);
    }

    fseek(wav, 0, SEEK_SET);
    wav_header(wav, rate, channels, data_bytes);
    fclose(wav);

    must(sdrkit_session_stop(session), "stop");
    must(sdrkit_session_close(session), "close the session");
    must(sdrkit_radio_close(radio), "close the radio");
    must(sdrkit_free(engine), "free");
    printf("wrote %.1f s of audio to %s\n", rate ? frames / rate : 0.0, path);
    return frames > 0 ? 0 : 1;
}
