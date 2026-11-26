/*
 * sdrkit: the presets part of the C API. MIT licence.
 *
 * Written by hand; src/presets.rs defines what this file declares. The library holds these calls only if it was built
 * with the cargo feature `presets`. A library without the feature reads no data folder and holds no lookup by country.
 *
 * The engine and the stacks hold no regional data. A host that wants it opens a data folder (the folder `data/` of
 * the repository, or its own) and asks these calls: the plans that a country uses, the channels of a scan, the
 * frequency entries, a search, the labels of a span, the name of a frequency and the next channel of a plan. The host
 * then sets plain settings on the engine and the stacks. The host names the plans of each call by id.
 *
 *     sdrkit_presets_open("data", &presets)                      opens the folder and reads its plan files
 *     sdrkit_presets_plan_ids(presets, "AU", &n)                 lists the plans that a country uses
 *     sdrkit_presets_plan_id(presets, i, &id)                    reads one id of the list
 *     sdrkit_presets_scan_channels_set(presets, engine, ids, n, NULL)   gives the engine the channels of a scan
 *     sdrkit_presets_frequency_entries_set(presets, engine, NULL)       gives the engine the frequency entries
 *     sdrkit_presets_name_of(presets, 226.5e6, ids, n, &place)   names a frequency
 *     sdrkit_presets_dial(presets, &query, &dial)                where a stack starts in a country
 *     sdrkit_presets_settings(presets, engine, &query, &n)       the settings of a stack in a country, ready to set
 *     sdrkit_presets_service_end(presets, &query, &end)          why a stack has no channels in a country
 *     sdrkit_presets_close(presets)
 *
 * Handles. sdrkit_presets_open() returns a presets handle. It is not a handle of an engine: only the calls of this
 * file take it, and sdrkit_presets_close() frees it. A call on a closed handle returns SDRKIT_E_NO_SUCH_OBJECT.
 *
 * Errors. Every call returns SDRKIT_OK or an SDRKIT_E_* code of sdrkit.h. After an error,
 * sdrkit_presets_error_message() returns one sentence. A call writes its out parameters only on success.
 *
 * Views. A string that a call returns is a view into a buffer of the presets handle. The comment of the call states
 * how long it is valid. Copy what you keep. One presets handle is for one thread at a time.
 *
 * Structs. Every struct begins with `uint32_t size`. Declare each with its initialiser (SDRKIT_PRESETS_HIT_INIT, ...).
 * No JSON crosses these calls. A plan file that a call reads or writes is a file of the computer.
 */

#ifndef SDRKIT_PRESETS_H
#define SDRKIT_PRESETS_H

#include "sdrkit.h"

// What a search result is: a plan.
#define SDRKIT_PRESETS_HIT_PLAN 0
// A channel of a plan.
#define SDRKIT_PRESETS_HIT_CHANNEL 1
// A segment of a plan.
#define SDRKIT_PRESETS_HIT_SEGMENT 2
// An entry of the frequency database.
#define SDRKIT_PRESETS_HIT_ENTRY 3

// The format of a plan file is taken from its text; an export writes CSV.
#define SDRKIT_PRESETS_FORMAT_AUTO 0
// CSV, one channel or segment in a row.
#define SDRKIT_PRESETS_FORMAT_CSV 1
// JSON.
#define SDRKIT_PRESETS_FORMAT_JSON 2

// One search result.
typedef struct sdrkit_presets_hit {
    // The number of bytes of the struct.
    uint32_t size;
    // An `SDRKIT_PRESETS_HIT_*` constant.
    int32_t kind;
    // The plan id, `<plan>/<channel>` for a channel or a segment, or the id of the entry.
    struct sdrkit_string id;
    // The name to show.
    struct sdrkit_string name;
    // The name of the plan of a channel, the kind of a plan, the stack of an entry.
    struct sdrkit_string detail;
    // The plan id of a plan, a channel or a segment. Empty for an entry.
    struct sdrkit_string plan;
    // `frequency_hz` holds a value.
    bool has_frequency_hz;
    // The frequency, Hz.
    double frequency_hz;
    // `width_hz` holds a value.
    bool has_width_hz;
    // The width, Hz.
    double width_hz;
    // How well the result matches. The results are in the order of the score, best first.
    double score;
} sdrkit_presets_hit;

// The span that labels are for.
typedef struct sdrkit_presets_view {
    // The number of bytes of the struct.
    uint32_t size;
    // The left edge, Hz.
    double lo_hz;
    // The right edge, Hz.
    double hi_hz;
    // The width, pixels.
    double width_px;
    // The width of one character, pixels. 0 for 7.
    double char_px;
    // The rows of labels. 0 for 3.
    uint32_t rows;
} sdrkit_presets_view;

// One placed label.
typedef struct sdrkit_presets_label {
    // The number of bytes of the struct.
    uint32_t size;
    // The label is a span (a band or a range). Otherwise it is one frequency.
    bool span;
    // The frequency, Hz: the lower edge of a span.
    double hz;
    // `end_hz` holds a value.
    bool has_end_hz;
    // The upper edge of a span, Hz.
    double end_hz;
    // The text.
    struct sdrkit_string text;
    // The id of the entry of the label. Empty for a name of a plan.
    struct sdrkit_string id;
    // The position of the frequency from the left edge, pixels.
    double x_px;
    // The width of a span, pixels.
    double span_px;
    // The row, 0 nearest the spectrum.
    uint32_t row;
} sdrkit_presets_label;

// A place in a plan: a channel, a raster point of a segment, or a frequency inside a segment.
typedef struct sdrkit_presets_place {
    // The number of bytes of the struct.
    uint32_t size;
    // The channel is reallocated.
    bool reallocated;
    // The plan id.
    struct sdrkit_string plan;
    // The name to show.
    struct sdrkit_string name;
    // The centre of the channel or the raster point, Hz.
    double centre_hz;
    // `width_hz` holds a value.
    bool has_width_hz;
    // The width of the channel, Hz.
    double width_hz;
    // The offset of the asked frequency from `centre_hz`, Hz.
    double offset_hz;
} sdrkit_presets_place;

// What a preset is asked for: a stack in a country, with the choices of the user.
typedef struct sdrkit_presets_query {
    // The number of bytes of the struct.
    uint32_t size;
    // The stack id.
    const char *stack;
    // The country, ISO 3166 alpha-2.
    const char *country;
    // The city of the country, for the known stations. NULL for none.
    const char *city;
    // The submode of the stack. NULL for its default.
    const char *submode;
    // `frequency_hz` holds a value.
    bool has_frequency_hz;
    // The frequency to start on, Hz, in place of that of the data.
    double frequency_hz;
} sdrkit_presets_query;

// One channel that the data has for a stack in a country.
typedef struct sdrkit_presets_channel {
    // The number of bytes of the struct.
    uint32_t size;
    // The number or the name of the channel.
    struct sdrkit_string label;
    // The centre frequency, Hz.
    double frequency_hz;
    // `width_hz` holds a value.
    bool has_width_hz;
    // The width, Hz.
    double width_hz;
} sdrkit_presets_channel;

// One supplied entry of the frequency database for a stack in a country.
typedef struct sdrkit_presets_frequency {
    // The number of bytes of the struct.
    uint32_t size;
    // The stable id of the entry.
    struct sdrkit_string id;
    // The name.
    struct sdrkit_string name;
    // The frequency to tune, Hz.
    double frequency_hz;
    // `bandwidth_hz` holds a value.
    bool has_bandwidth_hz;
    // The bandwidth, Hz.
    double bandwidth_hz;
    // The notes of the entry. Empty if it has none.
    struct sdrkit_string note;
} sdrkit_presets_frequency;

// Where a stack starts in a country.
typedef struct sdrkit_presets_dial_info {
    // The number of bytes of the struct.
    uint32_t size;
    // The frequency to start on, Hz.
    double frequency_hz;
    // `width_hz` holds a value.
    bool has_width_hz;
    // The width of the channel at that frequency, Hz.
    double width_hz;
    // The id of the plan or the band group of the channels. Empty if there is none.
    struct sdrkit_string plan;
    // The submode to start in. Empty for the default of the stack.
    struct sdrkit_string submode;
    // The network of the known station at the frequency. Empty if the frequency is not of a known station.
    struct sdrkit_string network;
} sdrkit_presets_dial_info;

// One country that the data has plans for.
typedef struct sdrkit_presets_country {
    // The number of bytes of the struct.
    uint32_t size;
    // The ISO 3166 alpha-2 code, upper case.
    struct sdrkit_string code;
    // The name of the country in the plan files. Empty if they give none.
    struct sdrkit_string name;
} sdrkit_presets_country;

// One setting of a stack for a country, ready to set: its key, and its id and value as an argument of a start.
typedef struct sdrkit_presets_setting {
    // The number of bytes of the struct.
    uint32_t size;
    // The key of the setting.
    struct sdrkit_string key;
    // The id of the setting of the stack type and its value, of the type of the setting.
    struct sdrkit_arg arg;
} sdrkit_presets_setting;

// What the Meshtastic slots are asked for: a region, a preset and the channel settings.
typedef struct sdrkit_presets_slot_query {
    // The number of bytes of the struct.
    uint32_t size;
    // A Meshtastic region ("ANZ"), a band plan of the mesh stacks ("EU 868") or a country, ISO 3166 alpha-2.
    const char *region;
    // The Meshtastic preset. NULL for LongFast.
    const char *preset;
    // The name of the primary channel. NULL for the name of the preset.
    const char *channel_name;
    // The channel number, from 1. 0 takes the slot from the channel name.
    uint32_t channel_num;
} sdrkit_presets_slot_query;

// One Meshtastic frequency slot, as a row of the table `slots` of the stack gives it.
typedef struct sdrkit_presets_slot {
    // The number of bytes of the struct.
    uint32_t size;
    // The slot number, from 1.
    uint32_t slot;
    // The centre of the slot, Hz.
    double frequency_hz;
    // The stack listens on the slot with the channel settings of the query.
    bool current;
    // The slot of the default channel name of the preset.
    bool preset_default;
} sdrkit_presets_slot;

// Why a stack offers no channels, or fewer, in a country: the mode, or a part of it, is out of service there.
typedef struct sdrkit_presets_service_info {
    // The number of bytes of the struct.
    uint32_t size;
    // The mode, or a part of it, is out of service in the country. False: the rest of the struct is empty.
    bool out_of_service;
    // Only a part of the mode ended; the channels of the part that remains stay in the lists.
    bool partial;
    // `year` holds a value.
    bool has_year;
    // The year the service ended.
    int32_t year;
    // The date the service ended as the data gives it ("2024", "2024-10" or "2024-10-28"). Empty if not known.
    struct sdrkit_string ended;
    // One sentence for a person: "3G (UMTS) was switched off in Australia in 2024".
    struct sdrkit_string sentence;
} sdrkit_presets_service_info;

#define SDRKIT_PRESETS_HIT_INIT {sizeof(sdrkit_presets_hit), 0, {0}, {0}, {0}, {0}, 0, 0, 0, 0, 0}
#define SDRKIT_PRESETS_VIEW_INIT {sizeof(sdrkit_presets_view), 0, 0, 0, 0, 0}
#define SDRKIT_PRESETS_LABEL_INIT {sizeof(sdrkit_presets_label), 0, 0, 0, 0, {0}, {0}, 0, 0, 0}
#define SDRKIT_PRESETS_PLACE_INIT {sizeof(sdrkit_presets_place), 0, {0}, {0}, 0, 0, 0, 0}
#define SDRKIT_PRESETS_QUERY_INIT {sizeof(sdrkit_presets_query), 0, 0, 0, 0, 0, 0}
#define SDRKIT_PRESETS_CHANNEL_INIT {sizeof(sdrkit_presets_channel), {0}, 0, 0, 0}
#define SDRKIT_PRESETS_FREQUENCY_INIT {sizeof(sdrkit_presets_frequency), {0}, {0}, 0, 0, 0, {0}}
#define SDRKIT_PRESETS_DIAL_INFO_INIT {sizeof(sdrkit_presets_dial_info), 0, 0, 0, {0}, {0}, {0}}
#define SDRKIT_PRESETS_COUNTRY_INIT {sizeof(sdrkit_presets_country), {0}, {0}}
#define SDRKIT_PRESETS_SETTING_INIT {sizeof(sdrkit_presets_setting), {0}, {0}}
#define SDRKIT_PRESETS_SLOT_QUERY_INIT {sizeof(sdrkit_presets_slot_query), 0, 0, 0, 0}
#define SDRKIT_PRESETS_SLOT_INIT {sizeof(sdrkit_presets_slot), 0, 0, 0, 0}
#define SDRKIT_PRESETS_SERVICE_INFO_INIT {sizeof(sdrkit_presets_service_info), 0, 0, 0, 0, {0}, {0}}

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

// Opens the data folder `folder` of the computer, reads its plan files, and writes a handle to `presets`. Free it
// with `sdrkit_presets_close`. Returns `SDRKIT_E_NOT_FOUND` if the folder or its folder `plans` does not exist,
// `SDRKIT_E_IO` if a file cannot be read, and `SDRKIT_E_MALFORMED` for a plan file that does not have its format;
// `sdrkit_presets_error_message` with handle 0 then returns the sentence.
int32_t sdrkit_presets_open(const char *folder,
                            SDRKIT_HANDLE *presets);

// Frees the presets handle `presets`. Every view of its calls ends.
int32_t sdrkit_presets_close(SDRKIT_HANDLE presets);

// Writes the sentence of the last error of `presets` to `message`; with handle 0, that of the last
// `sdrkit_presets_open` that failed. The string is a view, valid until the next call on `presets`.
int32_t sdrkit_presets_error_message(SDRKIT_HANDLE presets,
                                     struct sdrkit_string *message);

// Sets the file `file` of the computer as the file of the user's plans of `presets`, adds its plans if it exists,
// and writes their number to `count`. `sdrkit_presets_plan_import` and `sdrkit_presets_plan_remove` then write the
// file. Returns `SDRKIT_E_MALFORMED` for a file that is not a plan file.
int32_t sdrkit_presets_personal_plans(SDRKIT_HANDLE presets,
                                      const char *file,
                                      uint32_t *count);

// Lists the countries that have at least one plan, by code, and writes their number to `count`.
// `sdrkit_presets_country_read` reads one.
int32_t sdrkit_presets_countries(SDRKIT_HANDLE presets,
                                 uint32_t *count);

// Writes country `index` of the last `sdrkit_presets_countries` to `country`. Its strings are views, valid until the
// next `sdrkit_presets_countries` on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
int32_t sdrkit_presets_country_read(SDRKIT_HANDLE presets,
                                    uint32_t index,
                                    struct sdrkit_presets_country *country);

// Lists the plans that the country `country` (ISO 3166 alpha-2) uses, the plan to start with first in each kind, and
// writes their number to `count`. `sdrkit_presets_plan_id` reads one. A country that the data does not name has none.
int32_t sdrkit_presets_plan_ids(SDRKIT_HANDLE presets,
                                const char *country,
                                uint32_t *count);

// Writes plan id `index` of the last `sdrkit_presets_plan_ids` to `id`. The string is a view, valid until the next
// `sdrkit_presets_plan_ids` on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
int32_t sdrkit_presets_plan_id(SDRKIT_HANDLE presets,
                               uint32_t index,
                               struct sdrkit_string *id);

// Gives the scan service of the engine `engine` the channels of the `count` plans `plans`: for each stack that a
// plan names, the name and the frequency of each channel. Writes the number of stacks that have channels to
// `stacks`, which can be NULL. The list replaces the one before. An id that no plan has is skipped.
// `engine` is the handle of the engine or of an object of it.
int32_t sdrkit_presets_scan_channels_set(SDRKIT_HANDLE presets,
                                         SDRKIT_HANDLE engine,
                                         const char *const *plans,
                                         size_t count,
                                         uint32_t *stacks);

// Gives the frequency database of the engine `engine` the entries of the file `freqdb/builtin.csv` of the folder,
// and writes their number to `count`, which can be NULL. The list replaces the one before. Returns
// `SDRKIT_E_NOT_FOUND` if the folder has no such file.
int32_t sdrkit_presets_frequency_entries_set(SDRKIT_HANDLE presets,
                                             SDRKIT_HANDLE engine,
                                             uint32_t *count);

// Searches every plan, the channels and segments of the `count` plans `plans`, and the frequency entries of the
// folder for `text`: a name, a frequency ("474", "14074 kHz") or a channel name ("21", "ch 16"). Writes the number
// of results to `found`, best first. `sdrkit_presets_hit_read` reads one.
int32_t sdrkit_presets_search(SDRKIT_HANDLE presets,
                              const char *text,
                              const char *const *plans,
                              size_t count,
                              uint32_t *found);

// Writes result `index` of the last `sdrkit_presets_search` to `hit`. Its strings are views, valid until the next
// `sdrkit_presets_search` on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
int32_t sdrkit_presets_hit_read(SDRKIT_HANDLE presets,
                           uint32_t index,
                           struct sdrkit_presets_hit *hit);

// Places the labels of the span `view`: the names of the frequency entries of the folder that apply in the country
// `country` (NULL for every entry) and the channels and segments of the `count` plans `plans`. Writes the number of
// labels to `found`. `sdrkit_presets_label_read` reads one. Returns `SDRKIT_E_BAD_ARGUMENT` for a view without a width.
int32_t sdrkit_presets_labels(SDRKIT_HANDLE presets,
                              const char *const *plans,
                              size_t count,
                              const char *country,
                              const struct sdrkit_presets_view *view,
                              uint32_t *found);

// Writes label `index` of the last `sdrkit_presets_labels` to `label`: the labels of one frequency first, then the
// spans. Its strings are views, valid until the next `sdrkit_presets_labels` on `presets`. Returns
// `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
int32_t sdrkit_presets_label_read(SDRKIT_HANDLE presets,
                             uint32_t index,
                             struct sdrkit_presets_label *label);

// Writes the name of the channel or the segment at `hz` in the `count` plans `plans` to `place`, the most specific
// one. Its strings are views, valid until the next call on `presets` that writes a place. Returns
// `SDRKIT_E_NOT_FOUND` if no plan of the list holds the frequency.
int32_t sdrkit_presets_name_of(SDRKIT_HANDLE presets,
                               double hz,
                               const char *const *plans,
                               size_t count,
                               struct sdrkit_presets_place *place);

// Writes the channel that the typed name `name` ("21", "9A", "ch 16") has in the first of the `count` plans `plans`
// that has it to `place`. The views are as those of `sdrkit_presets_name_of`. Returns `SDRKIT_E_NOT_FOUND` if no plan
// of the list has the name.
int32_t sdrkit_presets_channel_named(SDRKIT_HANDLE presets,
                                     const char *name,
                                     const char *const *plans,
                                     size_t count,
                                     struct sdrkit_presets_place *place);

// Writes the next channel of the plan `plan` above `hz` (`up`) or below it to `place`. A reallocated channel counts
// with `include_reallocated`. The views are as those of `sdrkit_presets_name_of`. Returns `SDRKIT_E_NOT_FOUND` if no
// plan has the id or the plan has no channel there.
int32_t sdrkit_presets_step(SDRKIT_HANDLE presets,
                            const char *plan,
                            double hz,
                            bool up,
                            bool include_reallocated,
                            struct sdrkit_presets_place *place);

// Adds the plans of the plan file `file` of the computer, in the format `format` (an `SDRKIT_PRESETS_FORMAT_*`
// constant), to the user's plans of `presets`, and writes their number to `added`, which can be NULL. A CSV file
// that gives no id or name takes `id` and `name`. A plan with the id of a plan of the folder replaces it. All or
// nothing. Returns `SDRKIT_E_NOT_FOUND` if the file does not exist and `SDRKIT_E_MALFORMED` if it is not a valid
// plan file.
int32_t sdrkit_presets_plan_import(SDRKIT_HANDLE presets,
                                   const char *file,
                                   int32_t format,
                                   const char *id,
                                   const char *name,
                                   uint32_t *added);

// Writes the plan `plan` to the file `file` of the computer in the format `format` (an `SDRKIT_PRESETS_FORMAT_*`
// constant). Returns `SDRKIT_E_NOT_FOUND` if no plan has the id and `SDRKIT_E_IO` if the write fails.
int32_t sdrkit_presets_plan_export(SDRKIT_HANDLE presets,
                                   const char *plan,
                                   int32_t format,
                                   const char *file);

// Removes the user's plan `plan`. A plan of the folder that it replaced is back. Returns `SDRKIT_E_NOT_FOUND` if the
// user has no plan of the id.
int32_t sdrkit_presets_plan_remove(SDRKIT_HANDLE presets,
                                   const char *plan);

// Lists the channels that the data has for the stack of `query` in its country, lowest first, and writes their number
// to `count`: those of the plan of the stack, else the scan channels of the plans of the country, else the dials of
// every band plan that a mesh stack offers in the country, else the downlinks of the stack, else its starting dials. The channels of the user's plans that list the stack, and the country or no
// country, come in addition. `sdrkit_presets_channel_read` reads one. Returns `SDRKIT_E_BAD_ARGUMENT` for a country
// that is not a code of two letters.
int32_t sdrkit_presets_channels(SDRKIT_HANDLE presets,
                                const struct sdrkit_presets_query *query,
                                uint32_t *count);

// Lists the dials that the data has for the stack of `query`, lowest first, and writes their number to `count`: the
// rows of the dial file of the submode of the query or of the stack, each labelled by its band; if neither has a row
// and the data gives the stack the setting `dials`, every row, labelled by its stack and band. The city and the
// frequency of the query are not used. `sdrkit_presets_channel_read` reads one. Returns `SDRKIT_E_BAD_ARGUMENT` for a
// country that is not a code of two letters.
int32_t sdrkit_presets_dials(SDRKIT_HANDLE presets,
                             const struct sdrkit_presets_query *query,
                             uint32_t *count);

// Writes channel `index` of the last `sdrkit_presets_channels` or `sdrkit_presets_dials` to `channel`. Its label is a
// view, valid until the next of these calls on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the
// list.
int32_t sdrkit_presets_channel_read(SDRKIT_HANDLE presets,
                                    uint32_t index,
                                    struct sdrkit_presets_channel *channel);

// Lists the supplied entries of the frequency database (`freqdb/builtin.csv`) for the stack of `query` in its country,
// lowest first, and writes their number to `count`: the entries whose stack is the stack, and the entries of the
// general receiver or of no stack that have the stack id as a tag; each applies everywhere, in the country or in its
// ITU region. The city, the submode and the frequency of the query are not used. `sdrkit_presets_frequency_read`
// reads one. Returns `SDRKIT_E_BAD_ARGUMENT` for a country that is not a code of two letters.
int32_t sdrkit_presets_frequencies(SDRKIT_HANDLE presets,
                                   const struct sdrkit_presets_query *query,
                                   uint32_t *count);

// Writes entry `index` of the last `sdrkit_presets_frequencies` to `frequency`. Its strings are views, valid until the
// next `sdrkit_presets_frequencies` on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
int32_t sdrkit_presets_frequency_read(SDRKIT_HANDLE presets,
                                      uint32_t index,
                                      struct sdrkit_presets_frequency *frequency);

// Lists the Meshtastic frequency slots of the preset of `query` in its region, lowest first, as the table `slots` of a
// running stack `meshtastic` lists them, and writes their number to `count`. The slot of the channel number of the
// query, else of its channel name, is current; a preset with its own frequency in the region makes the slot of that
// frequency current, if it is one. `sdrkit_presets_meshtastic_slot_read` reads one. Returns `SDRKIT_E_NOT_FOUND` for a
// region or a preset that the data does not have, and `SDRKIT_E_OUT_OF_RANGE` for a channel number above the slots of
// the band.
int32_t sdrkit_presets_meshtastic_slots(SDRKIT_HANDLE presets,
                                        const struct sdrkit_presets_slot_query *query,
                                        uint32_t *count);

// Writes slot `index` of the last `sdrkit_presets_meshtastic_slots` to `slot`. Returns `SDRKIT_E_NO_SUCH_ID` for an
// index outside the list.
int32_t sdrkit_presets_meshtastic_slot_read(SDRKIT_HANDLE presets,
                                            uint32_t index,
                                            struct sdrkit_presets_slot *slot);

// Writes where the stack of `query` starts in its country to `dial`: the strongest known station of the city, else the
// first channel of its plan, else its first downlink, else its starting dial; the frequency of the query replaces it.
// Its strings are views, valid until the next `sdrkit_presets_dial` on `presets`. Returns `SDRKIT_E_NOT_FOUND` if the
// data has nothing for the stack in the country, `SDRKIT_E_BAD_ARGUMENT` for a country that is not a code.
int32_t sdrkit_presets_dial(SDRKIT_HANDLE presets,
                            const struct sdrkit_presets_query *query,
                            struct sdrkit_presets_dial_info *dial);

// Lists the settings that the data has for the stack of `query` in its country, as settings of the stack type in the
// engine `engine` (the handle of the engine or of an object of it), and writes their number to `count`.
// `sdrkit_presets_setting_read` reads one; its `arg` goes as it is into the settings of `sdrkit_stack_start`. Returns
// `SDRKIT_E_NOT_FOUND` if the data has nothing for the stack in the country, `SDRKIT_E_NO_SUCH_KEY` if the engine has
// no such stack or the stack no setting of a key, and `SDRKIT_E_MALFORMED` if a value of the data does not fit its
// setting.
int32_t sdrkit_presets_settings(SDRKIT_HANDLE presets,
                                SDRKIT_HANDLE engine,
                                const struct sdrkit_presets_query *query,
                                uint32_t *count);

// Writes setting `index` of the last `sdrkit_presets_settings` to `setting`. Its strings are views, valid until the
// next `sdrkit_presets_settings` on `presets`. Returns `SDRKIT_E_NO_SUCH_ID` for an index outside the list.
int32_t sdrkit_presets_setting_read(SDRKIT_HANDLE presets,
                                    uint32_t index,
                                    struct sdrkit_presets_setting *setting);

// Writes to `end` whether the stack of `query`, or a part of it, is out of service in its country, the year it ended
// and one sentence for a person ("3G (UMTS) was switched off in Australia in 2024"): why `sdrkit_presets_channels`
// lists no channels, or only those of the part that remains. The sentence names the country as the data does and the
// service by the generation word of the stack's name. The city, the submode and the frequency of the query are not
// used. Its strings are views, valid until the next `sdrkit_presets_service_end` on `presets`. Returns
// `SDRKIT_E_BAD_ARGUMENT` for a country that is not a code of two letters.
int32_t sdrkit_presets_service_end(SDRKIT_HANDLE presets,
                                   const struct sdrkit_presets_query *query,
                                   struct sdrkit_presets_service_info *end);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* SDRKIT_PRESETS_H */
