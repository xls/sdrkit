/*
 * sdrkit: the typed interface of the stack family "cell", version 1.2. MIT licence.
 *
 * Written from the interface description of the family. Do not edit.
 *
 * Structs. Each struct starts with `size`. A call that fills a struct writes at most the size of the struct that
 * the caller was compiled with, and sets `size` to the number of bytes that it wrote. A struct only grows at its
 * end: a library that is newer than the caller fills the members that the caller knows, and a library that is
 * older leaves the members that it does not know at 0, with `size` below the size of the struct. The caller
 * sets `size` of an arguments struct to its sizeof before the call: use the *_INIT initialiser.
 *
 * Values. A member with a `has_` member before it holds a value only if the `has_` member is true. A string or
 * bytes member that the library writes is a view into a buffer of the handle, valid until the next call on the
 * same handle. A member of an enumeration holds a value of the enum that its comment names.
 *
 * Versions. The minor version rises when the family gets a new stack, table, member, setting, command or event
 * type. The major version rises when one is removed or renamed, or its type or unit changes.
 * sdrkit_family_version() returns the version of the family in the library, or SDRKIT_E_NO_SUCH_KEY if the
 * library was built without the family. sdrkit_object_find() with "stack.<id>" finds out if an engine has a
 * stack.
 *
 * Results. Take a snapshot with sdrkit_snapshot_read() on the handle of a session, then call the *_rows() and
 * *_read() functions on the same handle. Events. Take the events with sdrkit_events_take(), then call the
 * *_event_read() function of the type of each event; it returns SDRKIT_E_WRONG_TYPE for an event of another type.
 * Settings and commands. Call the *_get_*(), *_set_*() and command functions on the handle of a session of the
 * stack; the getters also take the handle of the stack type ("stack.<id>") and return the defaults. One call
 * sets one setting. A function returns SDRKIT_E_WRONG_HANDLE if the object of the handle does not have the
 * table, the fields, the setting or the command with the types of this header.
 */

#ifndef SDRKIT_CELL_H
#define SDRKIT_CELL_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_CELL_VERSION_MAJOR 1
#define SDRKIT_CELL_VERSION_MINOR 2

// The stack "railway_cir": its id, and the keys of its event types.
#define SDRKIT_STACK_RAILWAY_CIR "railway_cir"
#define SDRKIT_EVENT_RAILWAY_CIR_KLW "railway_cir.klw"
#define SDRKIT_EVENT_RAILWAY_CIR_TDCS "railway_cir.tdcs"

// The stack "cell": its id, and the keys of its event types.
#define SDRKIT_STACK_CELL "cell"
#define SDRKIT_EVENT_CELL_LTE_CELL "cell.lte_cell"

// The stack "nr": its id, and the keys of its event types.
#define SDRKIT_STACK_NR "nr"
#define SDRKIT_EVENT_CELL_NR_CELL "cell.nr_cell"

// The stack "nbiot": its id, and the keys of its event types.
#define SDRKIT_STACK_NBIOT "nbiot"
#define SDRKIT_EVENT_CELL_NBIOT_CELL "cell.nbiot_cell"

// The stack "gsmr": its id, and the keys of its event types.
#define SDRKIT_STACK_GSMR "gsmr"
#define SDRKIT_EVENT_CELL_GSM_CELL "cell.gsm_cell"

// The stack "lbj": its id, and the keys of its event types.
#define SDRKIT_STACK_LBJ "lbj"
#define SDRKIT_EVENT_LBJ_MESSAGE "lbj.message"

// The stack "ebicab": its id, and the keys of its event types.
#define SDRKIT_STACK_EBICAB "ebicab"
#define SDRKIT_EVENT_BALISE_TELEGRAM "balise.telegram"

// The stack "umts": its id, and the keys of its event types.
#define SDRKIT_STACK_UMTS "umts"
#define SDRKIT_EVENT_CELL_UMTS_CELL "cell.umts_cell"

// The stack "cdma2000": its id, and the keys of its event types.
#define SDRKIT_STACK_CDMA2000 "cdma2000"
#define SDRKIT_EVENT_CELL_CDMA2000_CELL "cell.cdma2000_cell"

// The stack "tdscdma": its id, and the keys of its event types.
#define SDRKIT_STACK_TDSCDMA "tdscdma"
#define SDRKIT_EVENT_CELL_TDSCDMA_CELL "cell.tdscdma_cell"

// The stack "eot": its id, and the keys of its event types.
#define SDRKIT_STACK_EOT "eot"
#define SDRKIT_EVENT_EOT_FRAME "eot.frame"

// The stack "cv2x": its id, and the keys of its event types.
#define SDRKIT_STACK_CV2X "cv2x"

// The options of the setting "gen_scene" of the stack "cell".
typedef enum sdrkit_cell_gen_scene {
  // 10 MHz, 2 ports
  SDRKIT_CELL_GEN_SCENE_10_MHZ_2_PORTS = 0,
  // 5 MHz, 4 ports
  SDRKIT_CELL_GEN_SCENE_5_MHZ_4_PORTS = 1,
  // 1.4 MHz
  SDRKIT_CELL_GEN_SCENE_1_4_MHZ = 2,
  // 20 MHz
  SDRKIT_CELL_GEN_SCENE_20_MHZ = 3,
} sdrkit_cell_gen_scene;

// The options of the setting "gen_scene" of the stack "nr".
typedef enum sdrkit_nr_gen_scene {
  // Two n78 cells
  SDRKIT_NR_GEN_SCENE_TWO_N78_CELLS = 0,
  // n28 cell
  SDRKIT_NR_GEN_SCENE_N28_CELL = 1,
  // Barred n78 cell
  SDRKIT_NR_GEN_SCENE_BARRED_N78_CELL = 2,
} sdrkit_nr_gen_scene;

// The options of the setting "gen_scene" of the stack "nbiot".
typedef enum sdrkit_nbiot_gen_scene {
  // Standalone
  SDRKIT_NBIOT_GEN_SCENE_STANDALONE = 0,
  // Guard band
  SDRKIT_NBIOT_GEN_SCENE_GUARD_BAND = 1,
  // In-band, same PCI
  SDRKIT_NBIOT_GEN_SCENE_IN_BAND_SAME_PCI = 2,
  // In-band, other PCI
  SDRKIT_NBIOT_GEN_SCENE_IN_BAND_OTHER_PCI = 3,
} sdrkit_nbiot_gen_scene;

// The options of the setting "gsm_band" of the stack "gsmr".
typedef enum sdrkit_gsmr_gsm_band {
  // 900 MHz
  SDRKIT_GSMR_GSM_BAND_900_MHZ = 0,
  // 900 + 1800 + 850
  SDRKIT_GSMR_GSM_BAND_ALL = 1,
} sdrkit_gsmr_gsm_band;

// The options of the setting "gen_scene" of the stack "gsmr".
typedef enum sdrkit_gsmr_gen_scene {
  // Two cells
  SDRKIT_GSMR_GEN_SCENE_TWO_CELLS = 0,
  // Band edge
  SDRKIT_GSMR_GEN_SCENE_BAND_EDGE = 1,
  // Three cells
  SDRKIT_GSMR_GEN_SCENE_THREE_CELLS = 2,
} sdrkit_gsmr_gen_scene;

// The options of the setting "gen_scene" of the stack "ebicab".
typedef enum sdrkit_ebicab_gen_scene {
  // Long telegrams
  SDRKIT_EBICAB_GEN_SCENE_LONG_TELEGRAMS = 0,
  // Short telegram
  SDRKIT_EBICAB_GEN_SCENE_SHORT_TELEGRAM = 1,
  // All three in turn
  SDRKIT_EBICAB_GEN_SCENE_ALL_THREE_IN_TURN = 2,
  // Random bits
  SDRKIT_EBICAB_GEN_SCENE_RANDOM_BITS = 3,
} sdrkit_ebicab_gen_scene;

// The options of the setting "band_class" of the stack "cdma2000".
typedef enum sdrkit_cdma2000_band_class {
  // Not known
  SDRKIT_CDMA2000_BAND_CLASS_NONE = 0,
  // 0 (800 MHz)
  SDRKIT_CDMA2000_BAND_CLASS_0 = 1,
  // 1 (1900 MHz)
  SDRKIT_CDMA2000_BAND_CLASS_1 = 2,
  // 4 (1800 MHz)
  SDRKIT_CDMA2000_BAND_CLASS_4 = 3,
} sdrkit_cdma2000_band_class;

// The options of the setting "direction" of the stack "eot".
typedef enum sdrkit_eot_direction {
  // End of train to head of train
  SDRKIT_EOT_DIRECTION_EOT = 0,
  // Head of train to end of train
  SDRKIT_EOT_DIRECTION_HOT = 1,
} sdrkit_eot_direction;

// The options of the setting "channel_mhz" of the stack "cv2x".
typedef enum sdrkit_cv2x_channel_mhz {
  // 10 MHz
  SDRKIT_CV2X_CHANNEL_MHZ_10_MHZ = 0,
  // 20 MHz
  SDRKIT_CV2X_CHANNEL_MHZ_20_MHZ = 1,
} sdrkit_cv2x_channel_mhz;

// The options of `state` of `sdrkit_railway_cir_fields`.
typedef enum sdrkit_railway_cir_fields_state {
  // Waiting for samples
  SDRKIT_RAILWAY_CIR_FIELDS_STATE_WAITING = 0,
  // Listening
  SDRKIT_RAILWAY_CIR_FIELDS_STATE_LISTENING = 1,
  // Decoding
  SDRKIT_RAILWAY_CIR_FIELDS_STATE_DECODING = 2,
} sdrkit_railway_cir_fields_state;

// The result fields of `railway_cir`.
typedef struct sdrkit_railway_cir_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The summary of the last message.
  sdrkit_string text;
  // Messages decoded.
  int64_t messages;
  // Sync words found.
  int64_t syncs;
  // Readings that failed: CRC failures, rejections and ambiguous frames.
  int64_t failed_checks;
  // True if `carrier_offset_hz` holds a value.
  bool has_carrier_offset_hz;
  // The carrier offset of the channel. Unit: Hz.
  double carrier_offset_hz;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The level of the channel. Unit: dB.
  double level_db;
  // What the receiver is doing. A value of `sdrkit_railway_cir_fields_state`.
  int32_t state;
} sdrkit_railway_cir_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_cell_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_cell_decoder_fields;

// A row of the message list of `railway_cir`, newest first. The row key is the sequence number of the message.
typedef struct sdrkit_railway_cir_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The wall time of the reception. Seconds since 1970 (UTC).
  double received;
  // The protocol: `KLW` or `TDCS`.
  sdrkit_string protocol;
  // True if `train` holds a value.
  bool has_train;
  // The train (TDCS).
  sdrkit_string train;
  // The locomotive.
  sdrkit_string locomotive;
  // True if `action` holds a value.
  bool has_action;
  // The action (KLW).
  sdrkit_string action;
  // True if `tail_id` holds a value.
  bool has_tail_id;
  // The tail device id (KLW).
  sdrkit_string tail_id;
  // True if `pressure_kpa` holds a value.
  bool has_pressure_kpa;
  // The brake pipe pressure (KLW). Unit: kPa.
  double pressure_kpa;
  // True if `speed_kmh` holds a value.
  bool has_speed_kmh;
  // The speed (TDCS). Unit: km/h.
  double speed_kmh;
  // True if `km_post_km` holds a value.
  bool has_km_post_km;
  // The kilometre post (TDCS). Unit: km.
  double km_post_km;
  // The frame marks itself as a test frame (TDCS).
  bool test_frame;
  // True if `cars` holds a value.
  bool has_cars;
  // The car count (TDCS).
  int64_t cars;
  // The bits corrected.
  int64_t corrections;
  // The raw frame as hex.
  sdrkit_string raw;
} sdrkit_railway_cir_message_row;

// The counts and the level of the capture search of every cell stack.
typedef struct sdrkit_cell_search_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Captures searched.
  int64_t captures;
  // Retunes seen (scan steps).
  int64_t retunes;
  // Messages decoded.
  int64_t decoded;
  // Messages that failed their check.
  int64_t failed;
  // True if `capture_centre` holds a value.
  bool has_capture_centre;
  // The centre frequency of the newest capture. A frequency in Hz.
  double capture_centre;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The input level. Unit: dBFS.
  double level_dbfs;
} sdrkit_cell_search_fields;

// The result fields of `cell`.
typedef struct sdrkit_cell_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Cells in the table.
  int64_t cell_count;
  // Cells with SIB1.
  int64_t identified;
  // Operators of the cells.
  int64_t operators;
  // The notice.
  sdrkit_string banner;
} sdrkit_cell_fields;

// The options of `duplex` of `sdrkit_cell_row`.
typedef enum sdrkit_cell_row_duplex {
  // FDD
  SDRKIT_CELL_ROW_DUPLEX_F_D_D = 0,
  // TDD
  SDRKIT_CELL_ROW_DUPLEX_T_D_D = 1,
} sdrkit_cell_row_duplex;

// A row of the cells of `cell`. The row key is the cell key.
typedef struct sdrkit_cell_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `plmn` holds a value.
  bool has_plmn;
  // The primary network.
  sdrkit_string plmn;
  // True if `operator_` holds a value.
  bool has_operator_;
  // The operator of the primary network.
  sdrkit_string operator_;
  // True if `country` holds a value.
  bool has_country;
  // The country of the primary network.
  sdrkit_string country;
  // True if `plmn_count` holds a value.
  bool has_plmn_count;
  // The networks of the cell.
  int64_t plmn_count;
  // True if `band` holds a value.
  bool has_band;
  // The band.
  int64_t band;
  // Whether SIB1 confirmed the band.
  bool band_confirmed;
  // True if `earfcn` holds a value.
  bool has_earfcn;
  // The EARFCN.
  int64_t earfcn;
  // The nominal frequency. A frequency in Hz.
  double frequency;
  // The physical cell identity.
  int64_t pci;
  // The duplex. A value of `sdrkit_cell_row_duplex`.
  int32_t duplex;
  // True if `enb` holds a value.
  bool has_enb;
  // The eNB identity.
  int64_t enb;
  // True if `sector` holds a value.
  bool has_sector;
  // The sector.
  int64_t sector;
  // True if `cell_id` holds a value.
  bool has_cell_id;
  // The cell identity.
  int64_t cell_id;
  // True if `tac` holds a value.
  bool has_tac;
  // The tracking area code.
  int64_t tac;
  // True if `width` holds a value.
  bool has_width;
  // The bandwidth. A frequency in Hz.
  double width;
  // True if `ports` holds a value.
  bool has_ports;
  // The CRS ports.
  int64_t ports;
  // The RSRP. Unit: dBFS.
  double rsrp_dbfs;
  // The RSRQ. Unit: dB.
  double rsrq_db;
  // The SINR. Unit: dB.
  double sinr_db;
  // The carrier offset. A frequency in Hz.
  double carrier_offset;
  // True if `barred` holds a value.
  bool has_barred;
  // Whether the cell is barred.
  bool barred;
  // Whether SIB1 schedules ETWS.
  bool etws;
  // Whether SIB1 schedules CMAS.
  bool cmas;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the cell was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_cell_row;

// The options of `cyclic_prefix` of `sdrkit_cell_detail_row`.
typedef enum sdrkit_cell_detail_row_cyclic_prefix {
  // Normal
  SDRKIT_CELL_DETAIL_ROW_CYCLIC_PREFIX_NORMAL = 0,
  // Extended
  SDRKIT_CELL_DETAIL_ROW_CYCLIC_PREFIX_EXTENDED = 1,
} sdrkit_cell_detail_row_cyclic_prefix;

// The options of `phich_duration` of `sdrkit_cell_detail_row`.
typedef enum sdrkit_cell_detail_row_phich_duration {
  // Normal
  SDRKIT_CELL_DETAIL_ROW_PHICH_DURATION_NORMAL = 0,
  // Extended
  SDRKIT_CELL_DETAIL_ROW_PHICH_DURATION_EXTENDED = 1,
} sdrkit_cell_detail_row_phich_duration;

// A row of the details of `cell`. The row key is the cell key.
typedef struct sdrkit_cell_detail_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the cell.
  sdrkit_string cell;
  // The cyclic prefix. A value of `sdrkit_cell_detail_row_cyclic_prefix`.
  int32_t cyclic_prefix;
  // True if `phich_duration` holds a value.
  bool has_phich_duration;
  // The PHICH duration. A value of `sdrkit_cell_detail_row_phich_duration`.
  int32_t phich_duration;
  // True if `phich_ng` holds a value.
  bool has_phich_ng;
  // The PHICH Ng index.
  int64_t phich_ng;
  // True if `sfn` holds a value.
  bool has_sfn;
  // The SFN of the first whole frame.
  int64_t sfn;
  // The clock error. Unit: ppm.
  double clock_ppm;
  // SI messages decoded.
  int64_t si_messages;
  // SI messages that failed their CRC.
  int64_t si_failed;
  // True if `q_rx_lev_min_dbm` holds a value.
  bool has_q_rx_lev_min_dbm;
  // q-RxLevMin of SIB1. Unit: dBm.
  double q_rx_lev_min_dbm;
  // True if `p_max_dbm` holds a value.
  bool has_p_max_dbm;
  // p-Max of SIB1. Unit: dBm.
  double p_max_dbm;
  // True if `si_window` holds a value.
  bool has_si_window;
  // The SI window. A length of time in seconds.
  double si_window;
  // True if `value_tag` holds a value.
  bool has_value_tag;
  // The value tag of SIB1.
  int64_t value_tag;
  // True if `ref_signal_power_dbm` holds a value.
  bool has_ref_signal_power_dbm;
  // The CRS power of SIB2. Unit: dBm.
  double ref_signal_power_dbm;
  // True if `p_b` holds a value.
  bool has_p_b;
  // p-b of SIB2.
  int64_t p_b;
  // True if `ul_earfcn` holds a value.
  bool has_ul_earfcn;
  // The uplink EARFCN.
  int64_t ul_earfcn;
  // True if `ul_rbs` holds a value.
  bool has_ul_rbs;
  // The uplink RBs.
  int64_t ul_rbs;
  // True if `ra_preambles` holds a value.
  bool has_ra_preambles;
  // The random access preambles.
  int64_t ra_preambles;
  // True if `mod_period_coeff` holds a value.
  bool has_mod_period_coeff;
  // The modification period coefficient.
  int64_t mod_period_coeff;
  // True if `paging_cycle` holds a value.
  bool has_paging_cycle;
  // The paging cycle. Unit: frames.
  int64_t paging_cycle;
  // True if `t300` holds a value.
  bool has_t300;
  // t300. A length of time in seconds.
  double t300;
  // True if `t310` holds a value.
  bool has_t310;
  // t310. A length of time in seconds.
  double t310;
  // True if `n310` holds a value.
  bool has_n310;
  // n310.
  int64_t n310;
  // True if `time_alignment` holds a value.
  bool has_time_alignment;
  // The time alignment timer. A length of time in seconds.
  double time_alignment;
  // True if `ac_barring` holds a value.
  bool has_ac_barring;
  // Whether access class barring is on.
  bool ac_barring;
  // True if `q_hyst_db` holds a value.
  bool has_q_hyst_db;
  // q-Hyst of SIB3. Unit: dB.
  double q_hyst_db;
  // True if `priority` holds a value.
  bool has_priority;
  // The reselection priority of SIB3.
  int64_t priority;
  // True if `s_intra_search_db` holds a value.
  bool has_s_intra_search_db;
  // s-IntraSearch of SIB3. Unit: dB.
  double s_intra_search_db;
  // True if `s_non_intra_search_db` holds a value.
  bool has_s_non_intra_search_db;
  // s-NonIntraSearch of SIB3. Unit: dB.
  double s_non_intra_search_db;
  // True if `thresh_serving_low_db` holds a value.
  bool has_thresh_serving_low_db;
  // threshServingLow of SIB3. Unit: dB.
  double thresh_serving_low_db;
  // True if `t_reselection` holds a value.
  bool has_t_reselection;
  // t-Reselection of SIB3. A length of time in seconds.
  double t_reselection;
} sdrkit_cell_detail_row;

// A network of a cell of `cell`. The row key is the cell key and the place of the network.
typedef struct sdrkit_cell_plmn_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the cell.
  sdrkit_string cell;
  // The network.
  sdrkit_string plmn;
  // True if `operator_` holds a value.
  bool has_operator_;
  // The operator of the network.
  sdrkit_string operator_;
} sdrkit_cell_plmn_row;

// Another band of a cell of `cell`. The row key is the cell key and the band.
typedef struct sdrkit_cell_band_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the cell.
  sdrkit_string cell;
  // The band.
  int64_t band;
  // Whether the band is of the multi-band list.
  bool mfbi;
} sdrkit_cell_band_row;

// A system information block of a cell of `cell`. The row key is the cell key and the SIB number.
typedef struct sdrkit_cell_sib_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the cell.
  sdrkit_string cell;
  // The SIB number.
  int64_t sib;
  // True if `period` holds a value.
  bool has_period;
  // The period of the SIB. Unit: frames.
  int64_t period;
  // Whether the SIB was read.
  bool seen;
} sdrkit_cell_sib_row;

// The options of `list` of `sdrkit_cell_neighbour_row`.
typedef enum sdrkit_cell_neighbour_row_list {
  // Same frequency (SIB4)
  SDRKIT_CELL_NEIGHBOUR_ROW_LIST_INTRA = 0,
  // Other carrier (SIB5)
  SDRKIT_CELL_NEIGHBOUR_ROW_LIST_INTER = 1,
} sdrkit_cell_neighbour_row_list;

// A neighbour of a cell of `cell`.
typedef struct sdrkit_cell_neighbour_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the cell.
  sdrkit_string cell;
  // The list that names the neighbour. A value of `sdrkit_cell_neighbour_row_list`.
  int32_t list;
  // True if `earfcn` holds a value.
  bool has_earfcn;
  // The EARFCN of the neighbour.
  int64_t earfcn;
  // The physical cell identity of the neighbour.
  int64_t pci;
  // True if `q_offset_db` holds a value.
  bool has_q_offset_db;
  // q-OffsetCell. Unit: dB.
  double q_offset_db;
} sdrkit_cell_neighbour_row;

// Another carrier that a cell of `cell` names in SIB5. The row key is the cell key and the EARFCN.
typedef struct sdrkit_cell_inter_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The row key of the cell.
  sdrkit_string cell;
  // The EARFCN.
  int64_t earfcn;
  // The RBs.
  int64_t rbs;
  // True if `priority` holds a value.
  bool has_priority;
  // The reselection priority.
  int64_t priority;
  // threshX-High. Unit: dB.
  double thresh_high_db;
  // threshX-Low. Unit: dB.
  double thresh_low_db;
  // q-RxLevMin. Unit: dBm.
  double q_rx_lev_min_dbm;
} sdrkit_cell_inter_carrier_row;

// A site of `cell`: the cells of one eNB. The row key is the network and the eNB.
typedef struct sdrkit_cell_site_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `plmn` holds a value.
  bool has_plmn;
  // The network.
  sdrkit_string plmn;
  // True if `operator_` holds a value.
  bool has_operator_;
  // The operator of the network.
  sdrkit_string operator_;
  // The eNB identity.
  int64_t enb;
  // The tracking area code.
  int64_t tac;
  // The cells of the site.
  int64_t cell_count;
} sdrkit_cell_site_row;

// An operator of `cell`. The row key is the network.
typedef struct sdrkit_cell_operator_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The network.
  sdrkit_string plmn;
  // True if `operator_` holds a value.
  bool has_operator_;
  // The operator of the network.
  sdrkit_string operator_;
  // True if `country` holds a value.
  bool has_country;
  // The country of the network.
  sdrkit_string country;
  // The cells of the operator.
  int64_t cell_count;
} sdrkit_cell_operator_row;

// A carrier of the newest capture of `cell`. The row key is its place, strongest first.
typedef struct sdrkit_cell_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frequency. A frequency in Hz.
  double frequency;
  // The width. A frequency in Hz.
  double width;
  // The level. Unit: dB.
  double level_db;
} sdrkit_cell_carrier_row;

// The result fields of `nr`.
typedef struct sdrkit_nr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Cells in the table.
  int64_t cell_count;
  // Cells with a MIB.
  int64_t with_mib;
  // True if `gscn_lo` holds a value.
  bool has_gscn_lo;
  // The lowest GSCN the newest capture searched.
  int64_t gscn_lo;
  // True if `gscn_hi` holds a value.
  bool has_gscn_hi;
  // The highest GSCN the newest capture searched.
  int64_t gscn_hi;
  // True if `candidates` holds a value.
  bool has_candidates;
  // The candidates the newest capture searched.
  int64_t candidates;
  // The notice.
  sdrkit_string banner;
} sdrkit_nr_fields;

// A row of the cells of `nr`.
typedef struct sdrkit_nr_cell_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `band` holds a value.
  bool has_band;
  // The band.
  int64_t band;
  // True if `gscn` holds a value.
  bool has_gscn;
  // The GSCN.
  int64_t gscn;
  // The frequency of the SSB. A frequency in Hz.
  double frequency;
  // The physical cell identity.
  int64_t pci;
  // The subcarrier spacing of the SSB. A frequency in Hz.
  double ssb_scs;
  // The beams seen.
  int64_t beams;
  // True if `sfn` holds a value.
  bool has_sfn;
  // The SFN of the MIB.
  int64_t sfn;
  // Whether the MIB schedules SIB1.
  bool sib1;
  // The RSRP. Unit: dBFS.
  double rsrp_dbfs;
  // The RSRQ. Unit: dB.
  double rsrq_db;
  // The SINR. Unit: dB.
  double sinr_db;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the cell was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_nr_cell_row;

// The result fields of `nbiot`.
typedef struct sdrkit_nbiot_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Cells in the table.
  int64_t cell_count;
  // Cells with SIB1-NB.
  int64_t identified;
} sdrkit_nbiot_fields;

// The options of `operation_mode` of `sdrkit_nbiot_cell_row`.
typedef enum sdrkit_nbiot_cell_row_operation_mode {
  // In-band (same PCI)
  SDRKIT_NBIOT_CELL_ROW_OPERATION_MODE_INBAND_SAME_PCI = 0,
  // In-band (other PCI)
  SDRKIT_NBIOT_CELL_ROW_OPERATION_MODE_INBAND_DIFFERENT_PCI = 1,
  // Guard band
  SDRKIT_NBIOT_CELL_ROW_OPERATION_MODE_GUARDBAND = 2,
  // Standalone
  SDRKIT_NBIOT_CELL_ROW_OPERATION_MODE_STANDALONE = 3,
} sdrkit_nbiot_cell_row_operation_mode;

// A row of the cells of `nbiot`.
typedef struct sdrkit_nbiot_cell_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `plmn` holds a value.
  bool has_plmn;
  // The primary network.
  sdrkit_string plmn;
  // True if `operator_` holds a value.
  bool has_operator_;
  // The operator of the primary network.
  sdrkit_string operator_;
  // True if `band` holds a value.
  bool has_band;
  // The band.
  int64_t band;
  // True if `earfcn` holds a value.
  bool has_earfcn;
  // The EARFCN.
  int64_t earfcn;
  // The nominal frequency (the offset from 0 Hz when the radio centre is not known). A frequency in Hz.
  double frequency;
  // The physical cell identity.
  int64_t pci;
  // True if `operation_mode` holds a value.
  bool has_operation_mode;
  // The operation mode. A value of `sdrkit_nbiot_cell_row_operation_mode`.
  int32_t operation_mode;
  // True if `enb` holds a value.
  bool has_enb;
  // The eNB identity.
  int64_t enb;
  // True if `sector` holds a value.
  bool has_sector;
  // The sector.
  int64_t sector;
  // True if `tac` holds a value.
  bool has_tac;
  // The tracking area code.
  int64_t tac;
  // True if `barred` holds a value.
  bool has_barred;
  // Whether the cell is barred.
  bool barred;
  // The level. Unit: dBFS.
  double level_dbfs;
  // The SNR. Unit: dB.
  double snr_db;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the cell was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_nbiot_cell_row;

// The result fields of `gsmr`.
typedef struct sdrkit_gsmr_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Cells in the table.
  int64_t cell_count;
  // Cells with a location area.
  int64_t identified;
  // Cells in the railway band.
  int64_t railway_cells;
} sdrkit_gsmr_fields;

// The options of `band` of `sdrkit_gsmr_cell_row`.
typedef enum sdrkit_gsmr_cell_row_band {
  // GSM-R
  SDRKIT_GSMR_CELL_ROW_BAND_GSM_R = 0,
  // GSM 900
  SDRKIT_GSMR_CELL_ROW_BAND_GSM_900 = 1,
  // GSM 850
  SDRKIT_GSMR_CELL_ROW_BAND_GSM_850 = 2,
  // DCS 1800
  SDRKIT_GSMR_CELL_ROW_BAND_DCS_1800 = 3,
} sdrkit_gsmr_cell_row_band;

// A row of the cells of `gsmr`.
typedef struct sdrkit_gsmr_cell_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `plmn` holds a value.
  bool has_plmn;
  // The network.
  sdrkit_string plmn;
  // True if `operator_` holds a value.
  bool has_operator_;
  // The operator of the network.
  sdrkit_string operator_;
  // True if `arfcn` holds a value.
  bool has_arfcn;
  // The ARFCN.
  int64_t arfcn;
  // The frequency. A frequency in Hz.
  double frequency;
  // The base station identity code.
  int64_t bsic;
  // The network colour code.
  int64_t ncc;
  // The base station colour code.
  int64_t bcc;
  // True if `lac` holds a value.
  bool has_lac;
  // The location area code.
  int64_t lac;
  // True if `cell_id` holds a value.
  bool has_cell_id;
  // The cell identity.
  int64_t cell_id;
  // True if `band` holds a value.
  bool has_band;
  // The band of the ARFCN. A value of `sdrkit_gsmr_cell_row_band`.
  int32_t band;
  // The level. Unit: dBFS.
  double level_dbfs;
  // The SNR. Unit: dB.
  double snr_db;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the cell was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_gsmr_cell_row;

// A carrier of the newest capture of `gsmr`. The row key is its place, strongest first.
typedef struct sdrkit_gsmr_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frequency. A frequency in Hz.
  double frequency;
  // The level over the floor of the spectrum. Unit: dB.
  double level_db;
  // Whether an SCH decoded on the carrier.
  bool synchronised;
} sdrkit_gsmr_carrier_row;

// The result fields of `lbj`.
typedef struct sdrkit_lbj_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Bursts with a sync word.
  int64_t bursts;
  // Messages shown.
  int64_t messages;
  // Messages refused by the checks.
  int64_t failed_checks;
  // Empty frames.
  int64_t blank_frames;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the channel. Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the channel. Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the newest message. A frequency in Hz.
  double offset_hz;
} sdrkit_lbj_fields;

// The options of `direction` of `sdrkit_lbj_message_row`.
typedef enum sdrkit_lbj_message_row_direction {
  // Up
  SDRKIT_LBJ_MESSAGE_ROW_DIRECTION_UP = 0,
  // Down
  SDRKIT_LBJ_MESSAGE_ROW_DIRECTION_DOWN = 1,
} sdrkit_lbj_message_row_direction;

// A message of `lbj`. The row key is the place of the message in the order received.
typedef struct sdrkit_lbj_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `received` holds a value.
  bool has_received;
  // When the message came (wall clock), when known. Seconds since 1970 (UTC).
  double received;
  // True if `train` holds a value.
  bool has_train;
  // The train (prefix and digits), or the network time of a time message.
  sdrkit_string train;
  // True if `speed_kmh` holds a value.
  bool has_speed_kmh;
  // The speed. Unit: km/h.
  int64_t speed_kmh;
  // True if `km_post_km` holds a value.
  bool has_km_post_km;
  // The kilometre post. Unit: km.
  double km_post_km;
  // True if `direction` holds a value.
  bool has_direction;
  // Up or down. A value of `sdrkit_lbj_message_row_direction`.
  int32_t direction;
  // True if `locomotive` holds a value.
  bool has_locomotive;
  // The locomotive class and number.
  sdrkit_string locomotive;
  // True if `route` holds a value.
  bool has_route;
  // The route name.
  sdrkit_string route;
  // True if `position` holds a value.
  bool has_position;
  // The position.
  sdrkit_position position;
  // The level of the burst. Unit: dBFS.
  double level_dbfs;
} sdrkit_lbj_message_row;

// The result fields of `ebicab`.
typedef struct sdrkit_ebicab_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Bursts.
  int64_t bursts;
  // Telegrams decoded.
  int64_t telegrams;
  // Long telegrams.
  int64_t long_;
  // Short telegrams.
  int64_t short_;
  // Telegrams that came inverted.
  int64_t inverted;
  // Bursts without a telegram.
  int64_t bursts_without_telegram;
  // Parity hits.
  int64_t parity_hits;
  // True if `bit_rate_hz` holds a value.
  bool has_bit_rate_hz;
  // The bit rate of the newest burst. A frequency in Hz.
  double bit_rate_hz;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the newest burst. A frequency in Hz.
  double offset_hz;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The SNR of the newest burst. Unit: dB.
  double snr_db;
} sdrkit_ebicab_fields;

// A telegram of `ebicab`. The row key is the place of the telegram in the order received.
typedef struct sdrkit_ebicab_telegram_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `received` holds a value.
  bool has_received;
  // When the telegram came (wall clock), when known. Seconds since 1970 (UTC).
  double received;
  // The length. Unit: bits.
  int64_t length_bits;
  // NID_C.
  int64_t nid_c;
  // NID_BG.
  int64_t nid_bg;
  // N_PIG.
  int64_t n_pig;
  // N_TOTAL.
  int64_t n_total;
  // M_VERSION.
  int64_t m_version;
  // The copies received.
  int64_t copies;
  // The user bits.
  sdrkit_bytes user_bits;
} sdrkit_ebicab_telegram_row;

// A balise of `ebicab` heard.
typedef struct sdrkit_ebicab_balise_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // NID_C.
  int64_t nid_c;
  // NID_BG.
  int64_t nid_bg;
  // The place of the balise in its group.
  int64_t balise_in_group;
  // The balises of the group.
  int64_t group_size;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the balise was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
} sdrkit_ebicab_balise_row;

// The options of `hint` of `sdrkit_ebicab_other_burst_row`.
typedef enum sdrkit_ebicab_other_burst_row_hint {
  // About the length of an EBICab 700 telegram (32 bits)
  SDRKIT_EBICAB_OTHER_BURST_ROW_HINT_EBICAB_700 = 0,
  // About the length of an EBICab 900 telegram (255 bits)
  SDRKIT_EBICAB_OTHER_BURST_ROW_HINT_EBICAB_900 = 1,
  // Short Eurobalise telegram length, but no valid telegram
  SDRKIT_EBICAB_OTHER_BURST_ROW_HINT_SHORT_EUROBALISE = 2,
  // Long Eurobalise telegram length, but no valid telegram
  SDRKIT_EBICAB_OTHER_BURST_ROW_HINT_LONG_EUROBALISE = 3,
} sdrkit_ebicab_other_burst_row_hint;

// A burst of `ebicab` without a telegram. The row key is the place of the burst in the order received.
typedef struct sdrkit_ebicab_other_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `received` holds a value.
  bool has_received;
  // When the burst came (wall clock), when known. Seconds since 1970 (UTC).
  double received;
  // The bits of the burst.
  int64_t bits;
  // The length of the burst. A length of time in seconds.
  double duration;
  // The SNR. Unit: dB.
  double snr_db;
  // The first bits.
  sdrkit_bytes first_bits;
  // True if `hint` holds a value.
  bool has_hint;
  // What the length of the burst suggests. A value of `sdrkit_ebicab_other_burst_row_hint`.
  int32_t hint;
} sdrkit_ebicab_other_burst_row;

// The result fields of `umts`.
typedef struct sdrkit_umts_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Cells in the table.
  int64_t cell_count;
} sdrkit_umts_fields;

// A row of the cells of `umts`. The row key is the primary scrambling code and the radio centre.
typedef struct sdrkit_umts_cell_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The primary scrambling code.
  int64_t code;
  // The code group.
  int64_t group;
  // Ec/Io of the CPICH. Unit: dB.
  double ec_io_db;
  // The CPICH level (RSCP). Unit: dBFS.
  double level_dbfs;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // The frame offset. Unit: chips.
  int64_t frame_offset;
  // True if `sttd` holds a value.
  bool has_sttd;
  // Whether the cell sends with STTD.
  bool sttd;
  // Captures in which the cell was seen.
  int64_t captures_seen;
  // BCH blocks decoded.
  int64_t bch_blocks;
  // True if `plmn` holds a value.
  bool has_plmn;
  // The network.
  sdrkit_string plmn;
  // True if `operator_` holds a value.
  bool has_operator_;
  // The operator of the network.
  sdrkit_string operator_;
  // True if `cell_id` holds a value.
  bool has_cell_id;
  // The cell identity.
  int64_t cell_id;
  // True if `cell_id_parts` holds a value.
  bool has_cell_id_parts;
  // The RNC and the cell of the identity.
  sdrkit_string cell_id_parts;
  // True if `uarfcn` holds a value.
  bool has_uarfcn;
  // The UARFCN.
  int64_t uarfcn;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the cell was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_umts_cell_row;

// The result fields of `cdma2000`.
typedef struct sdrkit_cdma2000_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Base stations in the table.
  int64_t station_count;
  // Base stations with a Sync Channel Message.
  int64_t identified;
} sdrkit_cdma2000_fields;

// A row of the base stations of `cdma2000`.
typedef struct sdrkit_cdma2000_cell_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The PN offset.
  sdrkit_string pn;
  // True if `sid` holds a value.
  bool has_sid;
  // The system identity.
  int64_t sid;
  // True if `nid` holds a value.
  bool has_nid;
  // The network identity.
  int64_t nid;
  // Ec/Io of the pilot. Unit: dB.
  double ec_io_db;
  // The pilot level. Unit: dBFS.
  double level_dbfs;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // The clock error. Unit: ppm.
  double clock_ppm;
  // True if `paging` holds a value.
  bool has_paging;
  // The paging channel.
  sdrkit_string paging;
  // True if `protocol` holds a value.
  bool has_protocol;
  // The protocol revisions.
  sdrkit_string protocol;
  // True if `system_time` holds a value.
  bool has_system_time;
  // The system time.
  sdrkit_string system_time;
  // True if `local_time` holds a value.
  bool has_local_time;
  // The local time.
  sdrkit_string local_time;
  // True if `lc_state` holds a value.
  bool has_lc_state;
  // The long code state.
  sdrkit_string lc_state;
  // Messages good of all.
  sdrkit_string messages;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the base station was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_cdma2000_cell_row;

// The result fields of `tdscdma`.
typedef struct sdrkit_tdscdma_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Cells in the table.
  int64_t cell_count;
} sdrkit_tdscdma_fields;

// A row of the cells of `tdscdma`. The row key is the code and the radio centre.
typedef struct sdrkit_tdscdma_cell_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The SYNC-DL code.
  int64_t sync_dl;
  // True if `parameter` holds a value.
  bool has_parameter;
  // The cell parameter, "p / p+1" when only the pair is known.
  sdrkit_string parameter;
  // The DwPTS level. Unit: dBFS.
  double level_dbfs;
  // The DwPTS SNR. Unit: dB.
  double snr_db;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // The quadruples counted.
  sdrkit_string quadruples;
  // True if `frame_parity` holds a value.
  bool has_frame_parity;
  // The SFN parity of the first whole frame of the newest capture.
  sdrkit_string frame_parity;
  // The midamble clarity.
  double clarity;
  // Captures in which the cell was seen.
  int64_t captures_seen;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When the cell was last heard. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_tdscdma_cell_row;

// The result fields of `eot`.
typedef struct sdrkit_eot_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames shown.
  int64_t frames_ok;
  // Frames the code refused.
  int64_t frames_bad;
  // Frames with bits repaired.
  int64_t frames_repaired;
  // Frame syncs.
  int64_t syncs;
  // True if `last_unit` holds a value.
  bool has_last_unit;
  // The unit of the newest frame.
  sdrkit_string last_unit;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The input level. Unit: dBFS.
  double level_dbfs;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_eot_fields;

// A frame of `eot`, newest first.
typedef struct sdrkit_eot_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // The link: end of train or head of train.
  sdrkit_string link;
  // The unit.
  sdrkit_string unit;
  // The frame for people.
  sdrkit_string text;
} sdrkit_eot_frame_row;

// The result fields of `cv2x`.
typedef struct sdrkit_cv2x_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Whether the sidelink structure is there.
  bool detected;
  // The SC-FDMA symbol length of the channel width. Unit: us.
  double symbol_us;
  // The cyclic prefix length of the channel width. Unit: us.
  double prefix_us;
  // True if `slot_timing` holds a value.
  bool has_slot_timing;
  // The slot timing of the detection. Unit: samples.
  int64_t slot_timing;
  // True if `correlation` holds a value.
  bool has_correlation;
  // The prefix correlation of the detection.
  double correlation;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset of the detection. A frequency in Hz.
  double offset_hz;
  // True if `busy_per_s` holds a value.
  bool has_busy_per_s;
  // Busy subframes a second. Unit: 1/s.
  double busy_per_s;
  // True if `busy_share` holds a value.
  bool has_busy_share;
  // The share of busy subframes. Unit: %.
  double busy_share;
  // True if `guard_seen` holds a value.
  bool has_guard_seen;
  // Whether the last symbol of the subframe is a guard.
  bool guard_seen;
  // True if `frame` holds a value.
  bool has_frame;
  // The frame shape over the 1 ms subframe.
  sdrkit_string frame;
  // What the stack shows of the content.
  sdrkit_string content;
  // The summary.
  sdrkit_string text;
  // What no public document read for the stack confirms.
  sdrkit_string note;
} sdrkit_cv2x_fields;

// A detection start of `cv2x`, newest first.
typedef struct sdrkit_cv2x_detection_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time. Unit: s.
  double time_s;
  // The carrier offset. A frequency in Hz.
  double offset_hz;
  // The prefix correlation.
  double correlation;
  // The slot timing. Unit: samples.
  int64_t slot_timing;
} sdrkit_cv2x_detection_row;

// The items that each decoder component of the stack "lbj" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_lbj_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `modem` holds a value.
  bool has_modem;
  // The items that the component "modem" dropped.
  int64_t modem;
} sdrkit_lbj_dropped;

// The items that each decoder component of the stack "ebicab" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_ebicab_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `uplink` holds a value.
  bool has_uplink;
  // The items that the component "uplink" dropped.
  int64_t uplink;
} sdrkit_ebicab_dropped;

// The items that each decoder component of the stack "eot" dropped because it was slow. A member holds a
// value while its decoder runs on a helper thread.
typedef struct sdrkit_eot_dropped {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `modem` holds a value.
  bool has_modem;
  // The items that the component "modem" dropped.
  int64_t modem;
} sdrkit_eot_dropped;

// A message of the end-of-train device.
typedef struct sdrkit_railway_cir_klw_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The summary: `KLW <action> <locomotive>`.
  sdrkit_string text;
  // The locomotive.
  sdrkit_string locomotive;
  // The reserved command byte.
  int64_t reserved;
  // The action code.
  int64_t action_code;
  // The action.
  sdrkit_string action;
  // True if `parameter` holds a value.
  bool has_parameter;
  // The parameter as 4 characters.
  sdrkit_string parameter;
  // True if `pressure_kpa` holds a value.
  bool has_pressure_kpa;
  // The brake pipe pressure of a pressure action. Unit: kPa.
  double pressure_kpa;
  // True if `tail_id` holds a value.
  bool has_tail_id;
  // The tail device id.
  sdrkit_string tail_id;
  // True if `extra` holds a value.
  bool has_extra;
  // Extra bytes as hex.
  sdrkit_string extra;
  // True if `clock` holds a value.
  bool has_clock;
  // The calibration clock, `YYYY-MM-DDThh:mm:ss`.
  sdrkit_string clock;
  // The bits corrected.
  int64_t corrections;
  // The raw frame as hex.
  sdrkit_string raw;
} sdrkit_railway_cir_klw_event;

// A dispatch message.
typedef struct sdrkit_railway_cir_tdcs_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The summary: `TDCS <train> <locomotive>`.
  sdrkit_string text;
  // The train.
  sdrkit_string train;
  // The locomotive.
  sdrkit_string locomotive;
  // The locomotive type code.
  int64_t loco_code;
  // The locomotive serial.
  int64_t loco_serial;
  // True if `km_post_km` holds a value.
  bool has_km_post_km;
  // The kilometre post. Unit: km.
  double km_post_km;
  // The mileage increases.
  bool increasing;
  // The frame marks itself as a test frame.
  bool test_frame;
  // The speed. Unit: km/h.
  double speed_kmh;
  // The weight. Unit: t.
  double weight_t;
  // The length field divided by 10 (its unit is not published).
  double length;
  // The car count.
  int64_t cars;
  // The station.
  int64_t station;
  // The track.
  int64_t track;
  // The actual track.
  int64_t actual_track;
  // The driver id.
  int64_t driver;
  // The signal type.
  int64_t signal_type;
  // The semaphore type.
  int64_t semaphore_type;
  // The locomotive role.
  int64_t role;
  // The bits corrected.
  int64_t corrections;
  // The raw frame as hex.
  sdrkit_string raw;
} sdrkit_railway_cir_tdcs_event;

// The options of `family` of `sdrkit_cell_lte_cell_event`.
typedef enum sdrkit_cell_lte_cell_event_family {
  // Aircraft
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CELL_LTE_CELL_EVENT_FAMILY_OTHER = 12,
} sdrkit_cell_lte_cell_event_family;

// A cell found or identified by a search stack.
typedef struct sdrkit_cell_lte_cell_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The row key of the cell in the `cells` table.
  sdrkit_string source;
  // What was found.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_cell_lte_cell_event_family`.
  int32_t family;
} sdrkit_cell_lte_cell_event;

// The options of `family` of `sdrkit_cell_nr_cell_event`.
typedef enum sdrkit_cell_nr_cell_event_family {
  // Aircraft
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CELL_NR_CELL_EVENT_FAMILY_OTHER = 12,
} sdrkit_cell_nr_cell_event_family;

// A cell found or identified by a search stack.
typedef struct sdrkit_cell_nr_cell_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The row key of the cell in the `cells` table.
  sdrkit_string source;
  // What was found.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_cell_nr_cell_event_family`.
  int32_t family;
} sdrkit_cell_nr_cell_event;

// The options of `family` of `sdrkit_cell_nbiot_cell_event`.
typedef enum sdrkit_cell_nbiot_cell_event_family {
  // Aircraft
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CELL_NBIOT_CELL_EVENT_FAMILY_OTHER = 12,
} sdrkit_cell_nbiot_cell_event_family;

// A cell found or identified by a search stack.
typedef struct sdrkit_cell_nbiot_cell_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The row key of the cell in the `cells` table.
  sdrkit_string source;
  // What was found.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_cell_nbiot_cell_event_family`.
  int32_t family;
} sdrkit_cell_nbiot_cell_event;

// The options of `family` of `sdrkit_cell_gsm_cell_event`.
typedef enum sdrkit_cell_gsm_cell_event_family {
  // Aircraft
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CELL_GSM_CELL_EVENT_FAMILY_OTHER = 12,
} sdrkit_cell_gsm_cell_event_family;

// A cell found or identified by a search stack.
typedef struct sdrkit_cell_gsm_cell_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The row key of the cell in the `cells` table.
  sdrkit_string source;
  // What was found.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_cell_gsm_cell_event_family`.
  int32_t family;
} sdrkit_cell_gsm_cell_event;

// The options of `family` of `sdrkit_lbj_message_event`.
typedef enum sdrkit_lbj_message_event_family {
  // Aircraft
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_LBJ_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_lbj_message_event_family;

// The options of `direction` of `sdrkit_lbj_message_event`.
typedef enum sdrkit_lbj_message_event_direction {
  // Up
  SDRKIT_LBJ_MESSAGE_EVENT_DIRECTION_UP = 0,
  // Down
  SDRKIT_LBJ_MESSAGE_EVENT_DIRECTION_DOWN = 1,
} sdrkit_lbj_message_event_direction;

// A message of `lbj` shown or completed by its pair.
typedef struct sdrkit_lbj_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // The train, or the network time of a time message.
  sdrkit_string source;
  // The message for people.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_lbj_message_event_family`.
  int32_t family;
  // True if `position` holds a value.
  bool has_position;
  // The position.
  sdrkit_position position;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed in knots. Unit: kt.
  double speed_kt;
  // True if `speed_kmh` holds a value.
  bool has_speed_kmh;
  // The speed. Unit: km/h.
  int64_t speed_kmh;
  // True if `km_post_km` holds a value.
  bool has_km_post_km;
  // The kilometre post. Unit: km.
  double km_post_km;
  // True if `direction` holds a value.
  bool has_direction;
  // Up or down. A value of `sdrkit_lbj_message_event_direction`.
  int32_t direction;
  // True if `locomotive` holds a value.
  bool has_locomotive;
  // The locomotive class and number.
  sdrkit_string locomotive;
  // True if `route` holds a value.
  bool has_route;
  // The route name.
  sdrkit_string route;
} sdrkit_lbj_message_event;

// The options of `family` of `sdrkit_ebicab_balise_telegram_event`.
typedef enum sdrkit_ebicab_balise_telegram_event_family {
  // Aircraft
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_EBICAB_BALISE_TELEGRAM_EVENT_FAMILY_OTHER = 12,
} sdrkit_ebicab_balise_telegram_event_family;

// A new telegram of `ebicab`.
typedef struct sdrkit_ebicab_balise_telegram_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The balise.
  sdrkit_string source;
  // The telegram for people.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_ebicab_balise_telegram_event_family`.
  int32_t family;
  // The length. Unit: bits.
  int64_t length_bits;
  // NID_C.
  int64_t nid_c;
  // NID_BG.
  int64_t nid_bg;
  // N_PIG.
  int64_t n_pig;
} sdrkit_ebicab_balise_telegram_event;

// The options of `family` of `sdrkit_cell_umts_cell_event`.
typedef enum sdrkit_cell_umts_cell_event_family {
  // Aircraft
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CELL_UMTS_CELL_EVENT_FAMILY_OTHER = 12,
} sdrkit_cell_umts_cell_event_family;

// A cell found or identified by a search stack.
typedef struct sdrkit_cell_umts_cell_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The row key of the cell in the `cells` table.
  sdrkit_string source;
  // What was found.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_cell_umts_cell_event_family`.
  int32_t family;
} sdrkit_cell_umts_cell_event;

// The options of `family` of `sdrkit_cell_cdma2000_cell_event`.
typedef enum sdrkit_cell_cdma2000_cell_event_family {
  // Aircraft
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CELL_CDMA2000_CELL_EVENT_FAMILY_OTHER = 12,
} sdrkit_cell_cdma2000_cell_event_family;

// A cell found or identified by a search stack.
typedef struct sdrkit_cell_cdma2000_cell_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The row key of the cell in the `cells` table.
  sdrkit_string source;
  // What was found.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_cell_cdma2000_cell_event_family`.
  int32_t family;
} sdrkit_cell_cdma2000_cell_event;

// The options of `family` of `sdrkit_cell_tdscdma_cell_event`.
typedef enum sdrkit_cell_tdscdma_cell_event_family {
  // Aircraft
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_CELL_TDSCDMA_CELL_EVENT_FAMILY_OTHER = 12,
} sdrkit_cell_tdscdma_cell_event_family;

// A cell found or identified by a search stack.
typedef struct sdrkit_cell_tdscdma_cell_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The row key of the cell in the `cells` table.
  sdrkit_string source;
  // What was found.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_cell_tdscdma_cell_event_family`.
  int32_t family;
} sdrkit_cell_tdscdma_cell_event;

// The options of `family` of `sdrkit_eot_frame_event`.
typedef enum sdrkit_eot_frame_event_family {
  // Aircraft
  SDRKIT_EOT_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_EOT_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_EOT_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_EOT_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_EOT_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_EOT_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_EOT_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_EOT_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_EOT_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_EOT_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_EOT_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_EOT_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_EOT_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_eot_frame_event_family;

// One frame with every field. The fields of the end-of-train message are absent in a head-of-train frame.
typedef struct sdrkit_eot_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The link.
  sdrkit_string link;
  // The unit.
  sdrkit_string source;
  // The message type.
  int64_t message_type;
  // True if `chaining` holds a value.
  bool has_chaining;
  // The chaining bits.
  int64_t chaining;
  // True if `battery` holds a value.
  bool has_battery;
  // The battery condition.
  int64_t battery;
  // True if `pressure_psi` holds a value.
  bool has_pressure_psi;
  // The brake pipe pressure. Unit: psi.
  int64_t pressure_psi;
  // True if `charge_pct` holds a value.
  bool has_charge_pct;
  // The battery charge. Unit: %.
  int64_t charge_pct;
  // True if `spare` holds a value.
  bool has_spare;
  // The spare bit.
  bool spare;
  // True if `valve_open` holds a value.
  bool has_valve_open;
  // The valve is open.
  bool valve_open;
  // True if `confirmation` holds a value.
  bool has_confirmation;
  // The confirmation bit.
  bool confirmation;
  // True if `turbine` holds a value.
  bool has_turbine;
  // The turbine runs.
  bool turbine;
  // True if `moving` holds a value.
  bool has_moving;
  // The train moves.
  bool moving;
  // True if `marker_battery` holds a value.
  bool has_marker_battery;
  // The marker battery is good.
  bool marker_battery;
  // True if `marker_light` holds a value.
  bool has_marker_light;
  // The marker light is on.
  bool marker_light;
  // The bits repaired.
  int64_t bits_corrected;
  // The frame for people.
  sdrkit_string text;
  // The family of the source. A value of `sdrkit_eot_frame_event_family`.
  int32_t family;
} sdrkit_eot_frame_event;

// The arguments of `remove` of a search stack.
typedef struct sdrkit_cell_search_remove_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the cell.
  sdrkit_string row;
} sdrkit_cell_search_remove_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_CELL_SEARCH_REMOVE_ARGS_INIT {sizeof(sdrkit_cell_search_remove_args), {0}}

// The arguments of `remove` of `ebicab`.
typedef struct sdrkit_ebicab_remove_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the balise.
  sdrkit_string row;
} sdrkit_ebicab_remove_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_EBICAB_REMOVE_ARGS_INIT {sizeof(sdrkit_ebicab_remove_args), {0}}

// Fills `rows` with the changed rows of the table "messages" of the stack "railway_cir" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_railway_cir_messages_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_railway_cir_message_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.RailwayCirMessageRow", "messages", rows, sizeof(sdrkit_railway_cir_message_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "railway_cir.klw"
// (KLW message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_railway_cir_klw_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_railway_cir_klw_event *event) {
  return sdrkit_typed_event(handle, n, "railway_cir.klw", "cell.RailwayCirKlwEvent", event, sizeof(sdrkit_railway_cir_klw_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "railway_cir.tdcs"
// (TDCS message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_railway_cir_tdcs_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_railway_cir_tdcs_event *event) {
  return sdrkit_typed_event(handle, n, "railway_cir.tdcs", "cell.RailwayCirTdcsEvent", event, sizeof(sdrkit_railway_cir_tdcs_event));
}

// Runs the command "clear" (Clear) of the stack "railway_cir" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_railway_cir_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "railway_cir" are the sdrkit_railway_cir_gen_*() functions
// after the settings of its receiver.

// The setting "history" (Messages kept) of the stack "railway_cir". The oldest message is dropped beyond this count.
static inline int32_t sdrkit_railway_cir_get_history(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "history", value);
}
static inline int32_t sdrkit_railway_cir_set_history(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "history", value);
}

// The setting "locomotives" (Locomotive classes) of the stack "railway_cir". The class names of the locomotive type codes, one code and its name a line (the data file data/cell/locomotives.txt). Without it a locomotive shows its type code.
static inline int32_t sdrkit_railway_cir_get_locomotives(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "locomotives", value);
}
static inline int32_t sdrkit_railway_cir_set_locomotives(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "locomotives", value);
}

// The setting "gen_invert_bits" (Invert the air bits) of the test signal generator of the stack "railway_cir".
static inline int32_t sdrkit_railway_cir_gen_get_invert_bits(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_invert_bits", value);
}
static inline int32_t sdrkit_railway_cir_gen_set_invert_bits(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_invert_bits", value);
}

// The setting "gen_deviation_hz" (Tone deviation) of the test signal generator of the stack "railway_cir". Unit: Hz.
static inline int32_t sdrkit_railway_cir_gen_get_deviation_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", value);
}
static inline int32_t sdrkit_railway_cir_gen_set_deviation_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value);
}

// Fills `rows` with the changed rows of the table "cells" of the stack "cell" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cell_cells_rows(SDRKIT_HANDLE handle,
                                            sdrkit_cell_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.CellRow", "cells", rows, sizeof(sdrkit_cell_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "details" of the stack "cell" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cell_details_rows(SDRKIT_HANDLE handle,
                                              sdrkit_cell_detail_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.CellDetailRow", "details", rows, sizeof(sdrkit_cell_detail_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "cell_plmns" of the stack "cell" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cell_plmns_rows(SDRKIT_HANDLE handle,
                                            sdrkit_cell_plmn_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.CellPlmnRow", "cell_plmns", rows, sizeof(sdrkit_cell_plmn_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "cell_bands" of the stack "cell" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cell_bands_rows(SDRKIT_HANDLE handle,
                                            sdrkit_cell_band_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.CellBandRow", "cell_bands", rows, sizeof(sdrkit_cell_band_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "cell_sibs" of the stack "cell" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cell_sibs_rows(SDRKIT_HANDLE handle,
                                           sdrkit_cell_sib_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.CellSibRow", "cell_sibs", rows, sizeof(sdrkit_cell_sib_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "neighbours" of the stack "cell" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cell_neighbours_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_cell_neighbour_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.CellNeighbourRow", "neighbours", rows, sizeof(sdrkit_cell_neighbour_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "inter_carriers" of the stack "cell" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cell_inter_carriers_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_cell_inter_carrier_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.CellInterCarrierRow", "inter_carriers", rows, sizeof(sdrkit_cell_inter_carrier_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sites" of the stack "cell" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cell_sites_rows(SDRKIT_HANDLE handle,
                                            sdrkit_cell_site_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.CellSiteRow", "sites", rows, sizeof(sdrkit_cell_site_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "operators" of the stack "cell" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cell_operators_rows(SDRKIT_HANDLE handle,
                                                sdrkit_cell_operator_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.CellOperatorRow", "operators", rows, sizeof(sdrkit_cell_operator_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "cell" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `level_db`, highest first.
static inline int32_t sdrkit_cell_carriers_rows(SDRKIT_HANDLE handle,
                                               sdrkit_cell_carrier_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.CellCarrierRow", "carriers", rows, sizeof(sdrkit_cell_carrier_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cell.lte_cell"
// (LTE cell). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cell_lte_cell_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cell_lte_cell_event *event) {
  return sdrkit_typed_event(handle, n, "cell.lte_cell", "cell.FoundCellEvent", event, sizeof(sdrkit_cell_lte_cell_event));
}

// Runs the command "clear" (Clear) of the stack "cell" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_cell_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "cell" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_cell_remove(SDRKIT_HANDLE handle, const sdrkit_cell_search_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "cell.SearchRemoveArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "cell" are the sdrkit_cell_gen_*() functions
// after the settings of its receiver.

// The setting "capture_s" (Capture length) of the stack "cell". Unit: s. Seconds of input searched at a time.
static inline int32_t sdrkit_cell_get_capture_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "capture_s", value);
}
static inline int32_t sdrkit_cell_set_capture_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "capture_s", value);
}

// The setting "system_information" (Read system information) of the stack "cell". Decode SIB1 to SIB5 (needs the whole carrier in the sample band).
static inline int32_t sdrkit_cell_get_system_information(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "system_information", value);
}
static inline int32_t sdrkit_cell_set_system_information(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "system_information", value);
}

// The setting "max_cells" (Cells per carrier) of the stack "cell".
static inline int32_t sdrkit_cell_get_max_cells(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_cells", value);
}
static inline int32_t sdrkit_cell_set_max_cells(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_cells", value);
}

// The setting "min_pss_db" (PSS threshold) of the stack "cell". Unit: dB.
static inline int32_t sdrkit_cell_get_min_pss_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_pss_db", value);
}
static inline int32_t sdrkit_cell_set_min_pss_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_pss_db", value);
}

// The setting "max_cfo_khz" (Carrier offset searched) of the stack "cell". Unit: kHz.
static inline int32_t sdrkit_cell_get_max_cfo_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "max_cfo_khz", value);
}
static inline int32_t sdrkit_cell_set_max_cfo_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "max_cfo_khz", value);
}

// The setting "hide_bands" (Hide bands) of the stack "cell". Band numbers to leave out ("14 28, 64").
static inline int32_t sdrkit_cell_get_hide_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hide_bands", value);
}
static inline int32_t sdrkit_cell_set_hide_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hide_bands", value);
}

// The setting "only_bands" (Only bands) of the stack "cell".
static inline int32_t sdrkit_cell_get_only_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "only_bands", value);
}
static inline int32_t sdrkit_cell_set_only_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "only_bands", value);
}

// The setting "search" (Search) of the stack "cell".
static inline int32_t sdrkit_cell_get_search(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "search", value);
}
static inline int32_t sdrkit_cell_set_search(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "search", value);
}

// The setting "min_rsrp" (Minimum RSRP) of the stack "cell". Unit: dBFS.
static inline int32_t sdrkit_cell_get_min_rsrp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_rsrp", value);
}
static inline int32_t sdrkit_cell_set_min_rsrp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_rsrp", value);
}

// The setting "band_known" (Band confirmed only) of the stack "cell".
static inline int32_t sdrkit_cell_get_band_known(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "band_known", value);
}
static inline int32_t sdrkit_cell_set_band_known(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "band_known", value);
}

// The setting "identified" (Identified only) of the stack "cell".
static inline int32_t sdrkit_cell_get_identified(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "identified", value);
}
static inline int32_t sdrkit_cell_set_identified(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "identified", value);
}

// The setting "not_barred" (Leave out barred cells) of the stack "cell".
static inline int32_t sdrkit_cell_get_not_barred(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "not_barred", value);
}
static inline int32_t sdrkit_cell_set_not_barred(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "not_barred", value);
}

// The setting "one_per_cell" (One row per cell) of the stack "cell".
static inline int32_t sdrkit_cell_get_one_per_cell(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "one_per_cell", value);
}
static inline int32_t sdrkit_cell_set_one_per_cell(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "one_per_cell", value);
}

// The setting "band_order" (Band order) of the stack "cell". Band numbers taken first where bands overlap ("1 3 7"); the other bands follow in band number order.
static inline int32_t sdrkit_cell_get_band_order(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_order", value);
}
static inline int32_t sdrkit_cell_set_band_order(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_order", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "cell". A value of `sdrkit_cell_gen_scene`.
static inline int32_t sdrkit_cell_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_cell_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cell_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_cell_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_cell_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_sfn0" (Test signal first SFN) of the test signal generator of the stack "cell".
static inline int32_t sdrkit_cell_gen_get_sfn0(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_sfn0", value);
}
static inline int32_t sdrkit_cell_gen_set_sfn0(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_sfn0", value);
}

// The setting "gen_load" (Test signal load) of the test signal generator of the stack "cell".
static inline int32_t sdrkit_cell_gen_get_load(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_load", value);
}
static inline int32_t sdrkit_cell_gen_set_load(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_load", value);
}

// Fills `rows` with the changed rows of the table "cells" of the stack "nr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_nr_cells_rows(SDRKIT_HANDLE handle,
                                          sdrkit_nr_cell_row *rows,
                                          uint32_t capacity,
                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.NrCellRow", "cells", rows, sizeof(sdrkit_nr_cell_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cell.nr_cell"
// (NR cell). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cell_nr_cell_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cell_nr_cell_event *event) {
  return sdrkit_typed_event(handle, n, "cell.nr_cell", "cell.FoundCellEvent", event, sizeof(sdrkit_cell_nr_cell_event));
}

// Runs the command "clear" (Clear) of the stack "nr" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_nr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "nr" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_nr_remove(SDRKIT_HANDLE handle, const sdrkit_cell_search_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "cell.SearchRemoveArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "nr" are the sdrkit_nr_gen_*() functions
// after the settings of its receiver.

// The setting "capture_s" (Capture length) of the stack "nr". Unit: s. Seconds of input searched at a time.
static inline int32_t sdrkit_nr_get_capture_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "capture_s", value);
}
static inline int32_t sdrkit_nr_set_capture_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "capture_s", value);
}

// The setting "max_cells" (Cells per candidate) of the stack "nr".
static inline int32_t sdrkit_nr_get_max_cells(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_cells", value);
}
static inline int32_t sdrkit_nr_set_max_cells(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_cells", value);
}

// The setting "min_pss_db" (PSS threshold) of the stack "nr". Unit: dB.
static inline int32_t sdrkit_nr_get_min_pss_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_pss_db", value);
}
static inline int32_t sdrkit_nr_set_min_pss_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_pss_db", value);
}

// The setting "hide_bands" (Hide bands) of the stack "nr".
static inline int32_t sdrkit_nr_get_hide_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "hide_bands", value);
}
static inline int32_t sdrkit_nr_set_hide_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "hide_bands", value);
}

// The setting "only_bands" (Only bands) of the stack "nr".
static inline int32_t sdrkit_nr_get_only_bands(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "only_bands", value);
}
static inline int32_t sdrkit_nr_set_only_bands(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "only_bands", value);
}

// The setting "min_rsrp" (Minimum RSRP) of the stack "nr". Unit: dBFS.
static inline int32_t sdrkit_nr_get_min_rsrp(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_rsrp", value);
}
static inline int32_t sdrkit_nr_set_min_rsrp(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_rsrp", value);
}

// The setting "mib_only" (Only cells with a MIB) of the stack "nr".
static inline int32_t sdrkit_nr_get_mib_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mib_only", value);
}
static inline int32_t sdrkit_nr_set_mib_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mib_only", value);
}

// The setting "not_barred" (Leave out barred cells) of the stack "nr".
static inline int32_t sdrkit_nr_get_not_barred(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "not_barred", value);
}
static inline int32_t sdrkit_nr_set_not_barred(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "not_barred", value);
}

// The setting "band_order" (Band order) of the stack "nr". Band numbers taken first where bands overlap ("1 3 7"); the other bands follow in band number order.
static inline int32_t sdrkit_nr_get_band_order(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_order", value);
}
static inline int32_t sdrkit_nr_set_band_order(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_order", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "nr". A value of `sdrkit_nr_gen_scene`.
static inline int32_t sdrkit_nr_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_nr_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_nr_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_nr_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_nr_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "cells" of the stack "nbiot" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_nbiot_cells_rows(SDRKIT_HANDLE handle,
                                             sdrkit_nbiot_cell_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.NbiotCellRow", "cells", rows, sizeof(sdrkit_nbiot_cell_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cell.nbiot_cell"
// (NB-IoT cell). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cell_nbiot_cell_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cell_nbiot_cell_event *event) {
  return sdrkit_typed_event(handle, n, "cell.nbiot_cell", "cell.FoundCellEvent", event, sizeof(sdrkit_cell_nbiot_cell_event));
}

// Runs the command "clear" (Clear) of the stack "nbiot" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_nbiot_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "nbiot" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_nbiot_remove(SDRKIT_HANDLE handle, const sdrkit_cell_search_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "cell.SearchRemoveArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "nbiot" are the sdrkit_nbiot_gen_*() functions
// after the settings of its receiver.

// The setting "capture_s" (Capture length) of the stack "nbiot". Unit: s. Seconds of input searched at a time.
static inline int32_t sdrkit_nbiot_get_capture_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "capture_s", value);
}
static inline int32_t sdrkit_nbiot_set_capture_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "capture_s", value);
}

// The setting "min_npss" (NPSS threshold) of the stack "nbiot".
static inline int32_t sdrkit_nbiot_get_min_npss(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_npss", value);
}
static inline int32_t sdrkit_nbiot_set_min_npss(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_npss", value);
}

// The setting "read_sib1" (Read SIB1-NB) of the stack "nbiot".
static inline int32_t sdrkit_nbiot_get_read_sib1(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "read_sib1", value);
}
static inline int32_t sdrkit_nbiot_set_read_sib1(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "read_sib1", value);
}

// The setting "freq_correction_ppm" (Radio frequency correction) of the stack "nbiot". Unit: ppm. The radio's known frequency error. It splits a carrier offset into raster offset and tuning error.
static inline int32_t sdrkit_nbiot_get_freq_correction_ppm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "freq_correction_ppm", value);
}
static inline int32_t sdrkit_nbiot_set_freq_correction_ppm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "freq_correction_ppm", value);
}

// The setting "band_order" (Band order) of the stack "nbiot". Band numbers taken first where bands overlap ("1 3 7"); the other bands follow in band number order.
static inline int32_t sdrkit_nbiot_get_band_order(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "band_order", value);
}
static inline int32_t sdrkit_nbiot_set_band_order(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "band_order", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "nbiot". A value of `sdrkit_nbiot_gen_scene`.
static inline int32_t sdrkit_nbiot_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_nbiot_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_nbiot_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_nbiot_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_nbiot_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_load" (Test signal load) of the test signal generator of the stack "nbiot".
static inline int32_t sdrkit_nbiot_gen_get_load(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_load", value);
}
static inline int32_t sdrkit_nbiot_gen_set_load(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_load", value);
}

// The setting "gen_npbch_rotation" (Test signal NPBCH rotation) of the test signal generator of the stack "nbiot".
static inline int32_t sdrkit_nbiot_gen_get_npbch_rotation(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_npbch_rotation", value);
}
static inline int32_t sdrkit_nbiot_gen_set_npbch_rotation(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_npbch_rotation", value);
}

// Fills `rows` with the changed rows of the table "cells" of the stack "gsmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gsmr_cells_rows(SDRKIT_HANDLE handle,
                                            sdrkit_gsmr_cell_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.GsmrCellRow", "cells", rows, sizeof(sdrkit_gsmr_cell_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "gsmr" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `level_db`, highest first.
static inline int32_t sdrkit_gsmr_carriers_rows(SDRKIT_HANDLE handle,
                                               sdrkit_gsmr_carrier_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.GsmrCarrierRow", "carriers", rows, sizeof(sdrkit_gsmr_carrier_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cell.gsm_cell"
// (GSM cell). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cell_gsm_cell_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cell_gsm_cell_event *event) {
  return sdrkit_typed_event(handle, n, "cell.gsm_cell", "cell.FoundCellEvent", event, sizeof(sdrkit_cell_gsm_cell_event));
}

// Runs the command "clear" (Clear) of the stack "gsmr" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_gsmr_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "gsmr" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_gsmr_remove(SDRKIT_HANDLE handle, const sdrkit_cell_search_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "cell.SearchRemoveArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "gsmr" are the sdrkit_gsmr_gen_*() functions
// after the settings of its receiver.

// The setting "capture_s" (Capture length) of the stack "gsmr". Unit: s. Seconds of input searched at a time.
static inline int32_t sdrkit_gsmr_get_capture_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "capture_s", value);
}
static inline int32_t sdrkit_gsmr_set_capture_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "capture_s", value);
}

// The setting "max_carriers" (Carriers per capture) of the stack "gsmr". Humps searched, strongest first.
static inline int32_t sdrkit_gsmr_get_max_carriers(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_carriers", value);
}
static inline int32_t sdrkit_gsmr_set_max_carriers(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_carriers", value);
}

// The setting "gsm_band" (Band raster) of the stack "gsmr". A value of `sdrkit_gsmr_gsm_band`.
static inline int32_t sdrkit_gsmr_get_gsm_band(SDRKIT_HANDLE handle, sdrkit_gsmr_gsm_band *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gsm_band", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gsmr_gsm_band)index;
  return code;
}
static inline int32_t sdrkit_gsmr_set_gsm_band(SDRKIT_HANDLE handle, sdrkit_gsmr_gsm_band value) {
  return sdrkit_typed_setting_set_enum(handle, "gsm_band", (int32_t)value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "gsmr". A value of `sdrkit_gsmr_gen_scene`.
static inline int32_t sdrkit_gsmr_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_gsmr_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gsmr_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_gsmr_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_gsmr_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_centre_hz" (Test signal radio centre) of the test signal generator of the stack "gsmr". A frequency in Hz. The radio centre the test cells are placed against.
static inline int32_t sdrkit_gsmr_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_gsmr_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "lbj" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Place of a row: the column `position`.
// Speed: `speed_kmh`.
static inline int32_t sdrkit_lbj_messages_rows(SDRKIT_HANDLE handle,
                                              sdrkit_lbj_message_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.LbjMessageRow", "messages", rows, sizeof(sdrkit_lbj_message_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "lbj.message"
// (LBJ message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_lbj_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_lbj_message_event *event) {
  return sdrkit_typed_event(handle, n, "lbj.message", "cell.LbjMessageEvent", event, sizeof(sdrkit_lbj_message_event));
}

// Runs the command "clear" (Clear) of the stack "lbj" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_lbj_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "lbj" are the sdrkit_lbj_gen_*() functions
// after the settings of its receiver.

// The setting "locomotives" (Locomotive classes) of the stack "lbj". The class names of the locomotive type codes, one code and its name a line (the data file data/cell/locomotives.txt). Without it a locomotive shows its type code.
static inline int32_t sdrkit_lbj_get_locomotives(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "locomotives", value);
}
static inline int32_t sdrkit_lbj_set_locomotives(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "locomotives", value);
}

// The setting "gen_deviation_hz" (Test signal deviation) of the test signal generator of the stack "lbj". A frequency in Hz.
static inline int32_t sdrkit_lbj_gen_get_deviation_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", value);
}
static inline int32_t sdrkit_lbj_gen_set_deviation_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value);
}

// Fills `rows` with the changed rows of the table "telegrams" of the stack "ebicab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ebicab_telegrams_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_ebicab_telegram_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.EbicabTelegramRow", "telegrams", rows, sizeof(sdrkit_ebicab_telegram_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "balises" of the stack "ebicab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ebicab_balises_rows(SDRKIT_HANDLE handle,
                                                sdrkit_ebicab_balise_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.EbicabBaliseRow", "balises", rows, sizeof(sdrkit_ebicab_balise_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "other_bursts" of the stack "ebicab" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ebicab_other_bursts_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_ebicab_other_burst_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.EbicabOtherBurstRow", "other_bursts", rows, sizeof(sdrkit_ebicab_other_burst_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "balise.telegram"
// (Balise telegram). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ebicab_balise_telegram_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ebicab_balise_telegram_event *event) {
  return sdrkit_typed_event(handle, n, "balise.telegram", "cell.EbicabTelegramEvent", event, sizeof(sdrkit_ebicab_balise_telegram_event));
}

// Runs the command "clear" (Clear) of the stack "ebicab" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ebicab_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ebicab" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ebicab_remove(SDRKIT_HANDLE handle, const sdrkit_ebicab_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "cell.EbicabRemoveArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ebicab" are the sdrkit_ebicab_gen_*() functions
// after the settings of its receiver.

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "ebicab". A value of `sdrkit_ebicab_gen_scene`.
static inline int32_t sdrkit_ebicab_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_ebicab_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ebicab_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_ebicab_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_ebicab_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_burst_ms" (Test signal burst) of the test signal generator of the stack "ebicab". Unit: ms.
static inline int32_t sdrkit_ebicab_gen_get_burst_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_burst_ms", value);
}
static inline int32_t sdrkit_ebicab_gen_set_burst_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_burst_ms", value);
}

// The setting "gen_gap_ms" (Test signal gap) of the test signal generator of the stack "ebicab". Unit: ms.
static inline int32_t sdrkit_ebicab_gen_get_gap_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_gap_ms", value);
}
static inline int32_t sdrkit_ebicab_gen_set_gap_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_gap_ms", value);
}

// The setting "gen_random_bits" (Test signal random bits per burst) of the test signal generator of the stack "ebicab".
static inline int32_t sdrkit_ebicab_gen_get_random_bits(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_random_bits", value);
}
static inline int32_t sdrkit_ebicab_gen_set_random_bits(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_random_bits", value);
}

// The setting "gen_bit_rate_pct" (Test signal bit rate error) of the test signal generator of the stack "ebicab". Unit: %.
static inline int32_t sdrkit_ebicab_gen_get_bit_rate_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_bit_rate_pct", value);
}
static inline int32_t sdrkit_ebicab_gen_set_bit_rate_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_bit_rate_pct", value);
}

// Fills `rows` with the changed rows of the table "cells" of the stack "umts" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_umts_cells_rows(SDRKIT_HANDLE handle,
                                            sdrkit_umts_cell_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.UmtsCellRow", "cells", rows, sizeof(sdrkit_umts_cell_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cell.umts_cell"
// (UMTS cell). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cell_umts_cell_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cell_umts_cell_event *event) {
  return sdrkit_typed_event(handle, n, "cell.umts_cell", "cell.FoundCellEvent", event, sizeof(sdrkit_cell_umts_cell_event));
}

// Runs the command "clear" (Clear) of the stack "umts" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_umts_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "umts" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_umts_remove(SDRKIT_HANDLE handle, const sdrkit_cell_search_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "cell.SearchRemoveArgs", args, NULL, NULL, 0);
}

// The setting "capture_s" (Capture length) of the stack "umts". Unit: s. Seconds of input searched at a time.
static inline int32_t sdrkit_umts_get_capture_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "capture_s", value);
}
static inline int32_t sdrkit_umts_set_capture_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "capture_s", value);
}

// The setting "max_offset_hz" (Carrier search range) of the stack "umts". Unit: Hz. Cells are searched within this carrier offset either side, in steps of 2.5 kHz.
static inline int32_t sdrkit_umts_get_max_offset_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "max_offset_hz", value);
}
static inline int32_t sdrkit_umts_set_max_offset_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "max_offset_hz", value);
}

// The setting "max_cells" (Cells per capture) of the stack "umts". The most cells measured in one capture, strongest first.
static inline int32_t sdrkit_umts_get_max_cells(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_cells", value);
}
static inline int32_t sdrkit_umts_set_max_cells(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_cells", value);
}

// The setting "read_bch" (Read the broadcast channel) of the stack "umts". Decode the BCH of each cell for its PLMN and cell identity.
static inline int32_t sdrkit_umts_get_read_bch(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "read_bch", value);
}
static inline int32_t sdrkit_umts_set_read_bch(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "read_bch", value);
}

// The setting "roll_off" (Pulse roll-off) of the stack "umts". The roll-off of the root raised cosine matched filter (0.22 by TS 25.104; not confirmed against the text).
static inline int32_t sdrkit_umts_get_roll_off(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "roll_off", value);
}
static inline int32_t sdrkit_umts_set_roll_off(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "roll_off", value);
}

// Fills `rows` with the changed rows of the table "cells" of the stack "cdma2000" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cdma2000_cells_rows(SDRKIT_HANDLE handle,
                                                sdrkit_cdma2000_cell_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.Cdma2000CellRow", "cells", rows, sizeof(sdrkit_cdma2000_cell_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cell.cdma2000_cell"
// (CDMA2000 base station). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cell_cdma2000_cell_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cell_cdma2000_cell_event *event) {
  return sdrkit_typed_event(handle, n, "cell.cdma2000_cell", "cell.FoundCellEvent", event, sizeof(sdrkit_cell_cdma2000_cell_event));
}

// Runs the command "clear" (Clear) of the stack "cdma2000" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_cdma2000_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "cdma2000" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_cdma2000_remove(SDRKIT_HANDLE handle, const sdrkit_cell_search_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "cell.SearchRemoveArgs", args, NULL, NULL, 0);
}

// The setting "capture_s" (Capture length) of the stack "cdma2000". Unit: s. Seconds of input searched at a time.
static inline int32_t sdrkit_cdma2000_get_capture_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "capture_s", value);
}
static inline int32_t sdrkit_cdma2000_set_capture_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "capture_s", value);
}

// The setting "band_class" (Band class) of the stack "cdma2000". A value of `sdrkit_cdma2000_band_class`. The band class of the channel; the host sets it from its data. Only the Paging Channel frequency uses it.
static inline int32_t sdrkit_cdma2000_get_band_class(SDRKIT_HANDLE handle, sdrkit_cdma2000_band_class *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "band_class", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cdma2000_band_class)index;
  return code;
}
static inline int32_t sdrkit_cdma2000_set_band_class(SDRKIT_HANDLE handle, sdrkit_cdma2000_band_class value) {
  return sdrkit_typed_setting_set_enum(handle, "band_class", (int32_t)value);
}

// The setting "max_offset_hz" (Carrier search range) of the stack "cdma2000". Unit: Hz. Pilots are searched within this carrier offset either side.
static inline int32_t sdrkit_cdma2000_get_max_offset_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "max_offset_hz", value);
}
static inline int32_t sdrkit_cdma2000_set_max_offset_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "max_offset_hz", value);
}

// The setting "max_cells" (Pilots per capture) of the stack "cdma2000". The most pilots measured in one capture, strongest first.
static inline int32_t sdrkit_cdma2000_get_max_cells(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_cells", value);
}
static inline int32_t sdrkit_cdma2000_set_max_cells(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_cells", value);
}

// The setting "threshold" (Pilot threshold) of the stack "cdma2000". A code phase is a candidate when its power exceeds this many times the median.
static inline int32_t sdrkit_cdma2000_get_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold", value);
}
static inline int32_t sdrkit_cdma2000_set_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold", value);
}

// Fills `rows` with the changed rows of the table "cells" of the stack "tdscdma" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_tdscdma_cells_rows(SDRKIT_HANDLE handle,
                                               sdrkit_tdscdma_cell_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.TdscdmaCellRow", "cells", rows, sizeof(sdrkit_tdscdma_cell_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "cell.tdscdma_cell"
// (TD-SCDMA cell). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_cell_tdscdma_cell_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_cell_tdscdma_cell_event *event) {
  return sdrkit_typed_event(handle, n, "cell.tdscdma_cell", "cell.FoundCellEvent", event, sizeof(sdrkit_cell_tdscdma_cell_event));
}

// Runs the command "clear" (Clear) of the stack "tdscdma" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_tdscdma_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "tdscdma" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_tdscdma_remove(SDRKIT_HANDLE handle, const sdrkit_cell_search_remove_args *args) {
  return sdrkit_typed_command(handle, "remove", "cell.SearchRemoveArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "tdscdma" are the sdrkit_tdscdma_gen_*() functions
// after the settings of its receiver.

// The setting "capture_s" (Capture length) of the stack "tdscdma". Unit: s. Seconds of input searched at a time.
static inline int32_t sdrkit_tdscdma_get_capture_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "capture_s", value);
}
static inline int32_t sdrkit_tdscdma_set_capture_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "capture_s", value);
}

// The setting "threshold" (SYNC-DL threshold) of the stack "tdscdma". A SYNC-DL code is a candidate when its folded correlation exceeds this many times the median.
static inline int32_t sdrkit_tdscdma_get_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold", value);
}
static inline int32_t sdrkit_tdscdma_set_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold", value);
}

// The setting "max_cells" (Cells per capture) of the stack "tdscdma". The most cells measured in one capture, strongest first.
static inline int32_t sdrkit_tdscdma_get_max_cells(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_cells", value);
}
static inline int32_t sdrkit_tdscdma_set_max_cells(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_cells", value);
}

// The setting "roll_off" (Pulse roll-off) of the stack "tdscdma". The roll-off of the root raised cosine matched filter (0.22; not confirmed against the text of TS 25.105).
static inline int32_t sdrkit_tdscdma_get_roll_off(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "roll_off", value);
}
static inline int32_t sdrkit_tdscdma_set_roll_off(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "roll_off", value);
}

// The setting "gen_multipliers" (Test signal code multipliers) of the test signal generator of the stack "tdscdma". w(16, 1) = -1 and w(16, 2) = -j on the P-CCPCH; not confirmed against the text of TS 25.223.
static inline int32_t sdrkit_tdscdma_gen_get_multipliers(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_multipliers", value);
}
static inline int32_t sdrkit_tdscdma_gen_set_multipliers(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_multipliers", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "eot" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_eot_frames_rows(SDRKIT_HANDLE handle,
                                            sdrkit_eot_frame_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.EotFrameRow", "frames", rows, sizeof(sdrkit_eot_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "eot.frame"
// (End-of-train frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_eot_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_eot_frame_event *event) {
  return sdrkit_typed_event(handle, n, "eot.frame", "cell.EotFrameEvent", event, sizeof(sdrkit_eot_frame_event));
}

// Runs the command "clear" (Clear) of the stack "eot" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_eot_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "eot" are the sdrkit_eot_gen_*() functions
// after the settings of its receiver.

// The setting "direction" (Link) of the stack "eot". A value of `sdrkit_eot_direction`. End of train to head of train (telemetry) or head of train to end of train (commands).
static inline int32_t sdrkit_eot_get_direction(SDRKIT_HANDLE handle, sdrkit_eot_direction *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "direction", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_eot_direction)index;
  return code;
}
static inline int32_t sdrkit_eot_set_direction(SDRKIT_HANDLE handle, sdrkit_eot_direction value) {
  return sdrkit_typed_setting_set_enum(handle, "direction", (int32_t)value);
}

// The setting "gen_unit" (Test signal unit) of the test signal generator of the stack "eot".
static inline int32_t sdrkit_eot_gen_get_unit(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_unit", value);
}
static inline int32_t sdrkit_eot_gen_set_unit(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_unit", value);
}

// The setting "gen_psi" (Test signal brake pipe) of the test signal generator of the stack "eot". Unit: psi.
static inline int32_t sdrkit_eot_gen_get_psi(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_psi", value);
}
static inline int32_t sdrkit_eot_gen_set_psi(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_psi", value);
}

// The setting "gen_type" (Test signal message type) of the test signal generator of the stack "eot".
static inline int32_t sdrkit_eot_gen_get_type(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_type", value);
}
static inline int32_t sdrkit_eot_gen_set_type(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_type", value);
}

// The setting "gen_motion" (Test signal moving) of the test signal generator of the stack "eot".
static inline int32_t sdrkit_eot_gen_get_motion(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_motion", value);
}
static inline int32_t sdrkit_eot_gen_set_motion(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_motion", value);
}

// Fills `rows` with the changed rows of the table "detections" of the stack "cv2x" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_cv2x_detections_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_cv2x_detection_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "cell.Cv2xDetectionRow", "detections", rows, sizeof(sdrkit_cv2x_detection_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "cv2x" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_cv2x_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "cv2x" are the sdrkit_cv2x_gen_*() functions
// after the settings of its receiver.

// The setting "channel_mhz" (Channel width) of the stack "cv2x". A value of `sdrkit_cv2x_channel_mhz`. 10 MHz (15.36 Msps) or 20 MHz (30.72 Msps).
static inline int32_t sdrkit_cv2x_get_channel_mhz(SDRKIT_HANDLE handle, sdrkit_cv2x_channel_mhz *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channel_mhz", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_cv2x_channel_mhz)index;
  return code;
}
static inline int32_t sdrkit_cv2x_set_channel_mhz(SDRKIT_HANDLE handle, sdrkit_cv2x_channel_mhz value) {
  return sdrkit_typed_setting_set_enum(handle, "channel_mhz", (int32_t)value);
}

// The setting "gen_busy" (Test signal busy share) of the test signal generator of the stack "cv2x". Unit: %.
static inline int32_t sdrkit_cv2x_gen_get_busy(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_busy", value);
}
static inline int32_t sdrkit_cv2x_gen_set_busy(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_busy", value);
}

// The setting "gen_guard" (Test signal guard symbol) of the test signal generator of the stack "cv2x".
static inline int32_t sdrkit_cv2x_gen_get_guard(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_guard", value);
}
static inline int32_t sdrkit_cv2x_gen_set_guard(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_guard", value);
}

// The setting "gen_resource_blocks" (Test signal resource blocks) of the test signal generator of the stack "cv2x". 50 in a 10 MHz channel; not confirmed by a public document read for this stack.
static inline int32_t sdrkit_cv2x_gen_get_resource_blocks(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_resource_blocks", value);
}
static inline int32_t sdrkit_cv2x_gen_set_resource_blocks(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_resource_blocks", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_railway_cir_fields_read(SDRKIT_HANDLE handle, sdrkit_railway_cir_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.RailwayCirFields", fields, sizeof(sdrkit_railway_cir_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cell_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_cell_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.DecoderFields", fields, sizeof(sdrkit_cell_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cell_search_fields_read(SDRKIT_HANDLE handle, sdrkit_cell_search_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.SearchFields", fields, sizeof(sdrkit_cell_search_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cell_fields_read(SDRKIT_HANDLE handle, sdrkit_cell_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.CellFields", fields, sizeof(sdrkit_cell_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_nr_fields_read(SDRKIT_HANDLE handle, sdrkit_nr_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.NrFields", fields, sizeof(sdrkit_nr_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_nbiot_fields_read(SDRKIT_HANDLE handle, sdrkit_nbiot_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.NbiotFields", fields, sizeof(sdrkit_nbiot_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_gsmr_fields_read(SDRKIT_HANDLE handle, sdrkit_gsmr_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.GsmrFields", fields, sizeof(sdrkit_gsmr_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_lbj_fields_read(SDRKIT_HANDLE handle, sdrkit_lbj_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.LbjFields", fields, sizeof(sdrkit_lbj_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ebicab_fields_read(SDRKIT_HANDLE handle, sdrkit_ebicab_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.EbicabFields", fields, sizeof(sdrkit_ebicab_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_umts_fields_read(SDRKIT_HANDLE handle, sdrkit_umts_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.UmtsFields", fields, sizeof(sdrkit_umts_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cdma2000_fields_read(SDRKIT_HANDLE handle, sdrkit_cdma2000_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.Cdma2000Fields", fields, sizeof(sdrkit_cdma2000_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_tdscdma_fields_read(SDRKIT_HANDLE handle, sdrkit_tdscdma_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.TdscdmaFields", fields, sizeof(sdrkit_tdscdma_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_eot_fields_read(SDRKIT_HANDLE handle, sdrkit_eot_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.EotFields", fields, sizeof(sdrkit_eot_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_cv2x_fields_read(SDRKIT_HANDLE handle, sdrkit_cv2x_fields *fields) {
  return sdrkit_typed_fields(handle, "cell.Cv2xFields", fields, sizeof(sdrkit_cv2x_fields));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "lbj" in the
// snapshot of `handle`.
static inline int32_t sdrkit_lbj_dropped_read(SDRKIT_HANDLE handle, sdrkit_lbj_dropped *dropped) {
  return sdrkit_typed_fields(handle, "cell.lbj.dropped", dropped, sizeof(sdrkit_lbj_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "ebicab" in the
// snapshot of `handle`.
static inline int32_t sdrkit_ebicab_dropped_read(SDRKIT_HANDLE handle, sdrkit_ebicab_dropped *dropped) {
  return sdrkit_typed_fields(handle, "cell.ebicab.dropped", dropped, sizeof(sdrkit_ebicab_dropped));
}

// Fills `dropped` with the counts of dropped items of the decoder components of the stack "eot" in the
// snapshot of `handle`.
static inline int32_t sdrkit_eot_dropped_read(SDRKIT_HANDLE handle, sdrkit_eot_dropped *dropped) {
  return sdrkit_typed_fields(handle, "cell.eot.dropped", dropped, sizeof(sdrkit_eot_dropped));
}

#endif
