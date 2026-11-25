/*
 * sdrkit: the initialisers of the structs of sdrkit.h that begin with `size`. MIT licence.
 *
 * Written from sdrkit.h. Do not edit. sdrkit.h includes this file.
 *
 * Each initialiser sets `size` to the sizeof of the struct and every other member to 0. Declare every such
 * struct with it, a struct that a call fills too:
 *
 *     sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;
 *
 * A call returns SDRKIT_E_BAD_ARGUMENT for a struct whose `size` is below 4.
 */

#ifndef SDRKIT_INIT_H
#define SDRKIT_INIT_H

#define SDRKIT_COMMAND_INFO_INIT {sizeof(sdrkit_command_info), {0}, {0}, {0}, 0, 0, 0, 0, {0}}
#define SDRKIT_DECL_INIT {sizeof(sdrkit_decl), {0}, {0}, {0}, {0}, {0}, 0, 0, 0, 0, 0, {0}, {0}, 0, {0}, {0}, 0, {0}, 0, 0, {0}, 0}
#define SDRKIT_OPTION_INIT {sizeof(sdrkit_option), {0}, {0}, {0}}
#define SDRKIT_INPUT_INIT {sizeof(sdrkit_input), 0, 0, 0, 0}
#define SDRKIT_RADIO_OPTIONS_INIT {sizeof(sdrkit_radio_options), 0, 0}
#define SDRKIT_STACK_OPTIONS_INIT {sizeof(sdrkit_stack_options), 0, 0, 0, 0}
#define SDRKIT_STACK_NEED_INFO_INIT {sizeof(sdrkit_stack_need_info), 0, 0, 0, 0, 0, 0, 0, 0}
#define SDRKIT_ERROR_INFO_INIT {sizeof(sdrkit_error_info), 0, 0, 0, 0, 0, 0, {0}, {0}, {0}, 0, 0, 0, 0, 0, 0, {0}}
#define SDRKIT_EVENT_TYPE_INFO_INIT {sizeof(sdrkit_event_type_info), {0}, {0}, 0}
#define SDRKIT_EVENT_INFO_INIT {sizeof(sdrkit_event_info), 0, 0, 0, 0}
#define SDRKIT_PORT_INFO_INIT {sizeof(sdrkit_port_info), {0}, 0, 0, 0}
#define SDRKIT_REPORT_INFO_INIT {sizeof(sdrkit_report_info), 0, 0}
#define SDRKIT_REPORT_PORT_INIT {sizeof(sdrkit_report_port), {0}, 0, 0, 0, 0}
#define SDRKIT_REPORT_ADAPTER_INIT {sizeof(sdrkit_report_adapter), {0}, {0}, {0}, {0}, 0}
#define SDRKIT_RESULTS_INFO_INIT {sizeof(sdrkit_results_info), 0, 0, 0, 0, {0}}
#define SDRKIT_TABLE_INFO_INIT {sizeof(sdrkit_table_info), {0}, {0}, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
#define SDRKIT_PLOT_INFO_INIT {sizeof(sdrkit_plot_info), {0}, {0}, {0}, {0}, {0}}
#define SDRKIT_SNAPSHOT_INFO_INIT {sizeof(sdrkit_snapshot_info), 0, 0, 0, 0, 0, 0, 0, {0}, 0, 0}
#define SDRKIT_ROWS_INFO_INIT {sizeof(sdrkit_rows_info), 0, 0, 0}
#define SDRKIT_POINT_INIT {sizeof(sdrkit_point), {0}, 0, 0, {0}, 0, 0, {0}}
#define SDRKIT_SERIES_INFO_INIT {sizeof(sdrkit_series_info), 0, 0, 0, 0, 0}
#define SDRKIT_RASTER_INFO_INIT {sizeof(sdrkit_raster_info), 0, 0, 0, 0, 0, 0, 0, 0, 0}
#define SDRKIT_STORE_ENTRY_INIT {sizeof(sdrkit_store_entry), 0, 0, 0, 0}
#define SDRKIT_STORAGE_INIT {sizeof(sdrkit_storage), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
#define SDRKIT_AUDIO_INFO_INIT {sizeof(sdrkit_audio_info), 0, 0, 0, 0}
#define SDRKIT_IQ_INFO_INIT {sizeof(sdrkit_iq_info), 0, 0, 0}
#define SDRKIT_MEDIA_INFO_INIT {sizeof(sdrkit_media_info), 0, 0, 0, 0, {0}, 0, 0, {0}, 0, 0, 0, 0, 0, 0, 0, 0, {0}, 0, 0, {0}, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, {0}}
#define SDRKIT_SPECTRUM_FRAME_INIT {sizeof(sdrkit_spectrum_frame), 0, 0, 0, 0, 0, 0, 0}
#define SDRKIT_TAP_INFO_INIT {sizeof(sdrkit_tap_info), 0, 0, 0, 0, 0}

#endif
