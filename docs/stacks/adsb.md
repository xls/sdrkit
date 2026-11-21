<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ADS-B / Mode S

Aircraft on 1090 MHz: positions, callsigns, altitudes and speeds from ADS-B and Mode S replies.

Stack id: `adsb`. Group: Aircraft.

Signal types: ADS-B / Mode S.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 1.09 GHz |
| Tuning range | 1.085 GHz to 1.095 GHz |
| Sample rate | 4 MHz |
| Smallest sample rate | 2 MHz |
| Signal bandwidth | 2 MHz |
| Radio filter | 3.5 MHz |
| Automatic gain | High fixed gain |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:adsb` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `correction` | Error correction | choice | `3bits` | `off` (Off), `1bit` (1 bit), `3bits` (Up to 3 bits) | Repairs extended squitters and all-call replies. Up to 3 bits finds more messages but lets more false ones through. |
| `repair_unknown` | Repair unknown addresses | yes or no | false |  | Also repair frames from aircraft not heard yet. Raises the chance of a false aircraft. |
| `expiry_s` | Expiry time | number (s) | 60 | 5 to 3600 | How long an aircraft's address stays known after its last trusted message. |
| `effort` | Effort | choice | `auto` | `auto` (Automatic), `2` (All passes), `1` (Strong candidates only), `0` (Plain detector) | Extra decoding passes for weak signals. Automatic steps down when the computer is busy. |
| `modeac` | Mode A/C replies | yes or no | false |  | Also decode Mode A/C replies (squawk and Mode C altitude) on 1090 MHz. |
| `feeds_in` | Feeds from other receivers | text | empty |  | host:port of Beast, SBS or ASTERIX sources, separated by commas. |
| `allow_remote` | Allow hosts outside the local network | yes or no | false |  |  |
| `beast_server` | Beast server | yes or no | false |  | Beast binary on TCP port 30005. |
| `beast_port` | Beast port | whole number | 30005 | 0 to 65535 | 0 asks the system for a free port. |
| `sbs_server` | SBS server | yes or no | false |  | BaseStation text lines on TCP port 30003. |
| `sbs_port` | SBS port | whole number | 30003 | 0 to 65535 | 0 asks the system for a free port. |
| `asterix_server` | ASTERIX server | yes or no | false |  | EUROCONTROL ASTERIX categories 021, 048 and 062 on TCP. |
| `asterix_port` | ASTERIX port | whole number | 30010 | 0 to 65535 |  |
| `asterix_cat` | ASTERIX categories | choice | `all` | `all` (021, 048 and 062), `21` (021 only), `48` (048 only), `62` (062 only) |  |
| `sac` | ASTERIX SAC | whole number | 0 | 0 to 255 |  |
| `sic` | ASTERIX SIC | whole number | 1 | 0 to 255 |  |
| `allow_lan` | Servers accept the local network | yes or no | false |  | Otherwise only programs on this computer can connect. |
| `targets` | Outbound targets | text | empty |  | format@host:port, separated by ';': beast, sbs, asterix21, asterix48 or asterix62. Add /udp for UDP and #text for a first line. |
| `drop_s` | Drop time | number (s) | 60 | 0 to 3600 | Aircraft leave the table after this silence. |
| `gps_layer` | GPS interference layer | yes or no | false |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_aircraft` | Aircraft | whole number | 12 | 0 to 300 |  |
| `gen_replies` | Radar replies | yes or no | true |  |  |
| `gen_emergency` | Emergency demonstration | yes or no | false |  |  |
| `gen_rate` | Message rate multiplier | number | 1 | 0.1 to 20 |  |
| `gen_filter_mhz` | Pulse filter width | number (MHz) | 0 | 0 to 5 | The filter that shapes the pulse edges. 0 for automatic (about seven eighths of the sample rate). |
| `gen_centre_lat` | Airspace centre latitude | number (°) | 25.25 | -90 to 90 |  |
| `gen_centre_lon` | Airspace centre longitude | number (°) | 55.36 | -180 to 180 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `aircraft_count` | Aircraft | whole number |
| `with_position` | With position | whole number |
| `message_rate` | Messages per second | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `noise_dbfs` | Noise floor | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `max_range_m` | Max range | number (m) |
| `report` | Report | whole number |
| `time_s` | Signal time | number (s) |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `repaired` | Repaired messages | whole number |
| `loud` | Loud messages | whole number |
| `candidates` | Candidates | whole number |
| `effort` | Demodulator effort | whole number |
| `position_hint` | Position hint | position (°) |
| `coverage_positions` | Positions counted | whole number |
| `coverage_max_alt_ft` | Highest altitude | whole number (ft) |
| `feed_messages` | Messages fed | whole number |
| `modeac_replies` | Mode A/C replies | whole number |
| `modeac_codes` | Mode A/C codes | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `aircraft` | Aircraft | `icao` (text), `callsign` (text), `registration` (text), `country` (text), `category` (text), `squawk` (text), `emergency` (whole number), `emergency_name` (text), `on_ground` (yes or no), `alert` (yes or no), `spi` (yes or no), `position` (position (°)), `fix` (choice), `position_time` (time), `alt_ft` (whole number (ft)), `gnss_alt_ft` (whole number (ft)), `speed_kt` (number (kt)), `speed_kind` (choice), `heading_deg` (number (°)), `heading_is_track` (yes or no), `vertical_rate_fpm` (whole number (ft/min)), `gnss_minus_baro_ft` (whole number (ft)), `selected_alt_ft` (whole number (ft)), `selected_heading_deg` (number (°)), `baro_setting_hpa` (number (hPa)), `adsb_version` (whole number), `nacp` (whole number), `sil` (whole number), `nic` (whole number), `ias_kt` (number (kt)), `mach` (number), `roll_deg` (number (°)), `wind_derived_from_deg` (number (°)), `wind_derived_kt` (number (kt)), `wind_reported_from_deg` (number (°)), `wind_reported_kt` (number (kt)), `sat_derived_c` (number (°C)), `sat_reported_c` (number (°C)), `pressure_hpa` (number (hPa)), `humidity_pct` (number (%)), `hazards` (text), `advisory` (text), `advisory_threat` (text), `advisory_time` (time), `subnetwork_version` (whole number), `acas_ra_capable` (yes or no), `gicb` (text), `datalink` (yes or no), `tisb` (yes or no), `level_dbfs` (number (dBFS)), `distance_m` (number (m)), `bearing_deg` (number (°)), `source` (text), `trail` (bytes), `heard` (whole number), `last_heard` (time) |
| `messages` | Messages | `time_s` (number (s)), `hex` (text), `icao` (text), `df` (whole number), `fixed_bits` (whole number), `level_dbfs` (number (dBFS)), `message` (text) |
| `sources` | Sources | `name` (text), `format` (choice), `connected` (yes or no), `messages` (whole number), `aircraft` (whole number), `error` (text) |
| `by_df` | Messages by format | `format_number` (whole number), `count` (whole number) |
| `gps_cells` | GPS interference | `corner` (position (°)), `aircraft` (whole number), `level` (whole number) |
| `feeds` | Feeds | `name` (text), `port` (whole number), `clients` (whole number), `dropped` (whole number), `connected` (yes or no), `error` (text) |
| `modeac` | Mode A/C | `code` (text), `squawk` (text), `alt_ft` (whole number (ft)), `replies` (whole number), `spi` (yes or no), `level_dbfs` (number (dBFS)), `last_heard` (time) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `coverage_range` | Farthest range by bearing | ° | m |
| `coverage_histogram` | Positions by range | m |  |
| `coverage_rate` | Messages per second | s | Hz |

## Map points

The stack puts points on a map: Aircraft.

## Events

| Kind | Label | Fields |
|---|---|---|
| `adsb.aircraft` | Aircraft seen or updated | `event` (choice), `source` (text), `callsign` (text), `registration` (text), `category` (text), `squawk` (text), `emergency` (whole number), `on_ground` (yes or no), `altitude_ft` (whole number (ft)), `speed_kt` (number (kt)), `course_deg` (number (°)), `vertical_rate_fpm` (whole number (ft/min)), `position` (position), `distance_km` (number (km)), `messages` (whole number), `urgency` (choice), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `reset_stats` | Reset statistics |  |  | Starts the range statistics again. |

## Typed interface

The stack is in the family `adsb`, version 1.1. The C header `sdrkit/adsb.h`, the registry record `registry/adsb.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `TrackerFields`. The result fields of the tracker.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `aircraft_count` | whole number | always | 1.0 | Aircraft in the table. |
| `with_position` | whole number | always | 1.0 | Aircraft with a position. |
| `message_rate` | frequency | always | 1.0 | Good messages per second over the last second. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The mean level of the messages. |
| `noise_dbfs` | number (dBFS) | always | 1.0 | The noise floor. |
| `snr_db` | number (dB) | optional | 1.0 | The mean signal to noise ratio of the messages. |
| `max_range_m` | number (m) | optional | 1.0 | The farthest position counted. |
| `report` | whole number | always | 1.0 | Reports made. |
| `time_s` | number (s) | always | 1.0 | The signal time of the report, seconds from the start of the input. |
| `frames_ok` | whole number | always | 1.0 | Messages that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Messages that failed their check. |
| `repaired` | whole number | always | 1.0 | Messages repaired. |
| `loud` | whole number | always | 1.0 | Messages at least 3 dB below full scale. |
| `candidates` | whole number | always | 1.0 | Preamble candidates. |
| `effort` | whole number | always | 1.0 | The demodulator effort in use. |
| `position_hint` | position (°) | optional | 1.0 | A station position that fits the aircraft heard, when the one given is far from all of them. |
| `coverage_positions` | whole number | always | 1.0 | Positions counted in the range statistics. |
| `coverage_max_alt_ft` | whole number (ft) | optional | 1.0 | The highest altitude counted. |

### Result fields

Struct `FeedFields`. The result fields of the feeds out.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `feed_messages` | whole number | always | 1.0 | Messages sent to the feeds. |

### Result fields

Struct `ModeAcFields`. The result fields of the Mode A/C decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `modeac_replies` | whole number | always | 1.0 | Replies decoded. |
| `modeac_codes` | whole number | always | 1.0 | Codes shown. |

### Table `aircraft`

Struct `AircraftRow`. A row of the ADS-B aircraft table: the tracker's state for one aircraft. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `icao` | text | always | 1.0 | The 24-bit address, six hex digits (`~` in front for a non-ICAO address). |
| `callsign` | text | optional | 1.0 | The flight identification. |
| `registration` | text | optional | 1.0 | The registration derived from the address. |
| `country` | text | optional | 1.0 | The State the address is allocated to. |
| `category` | text | optional | 1.0 | The emitter category, set letter and code ("A3"). |
| `squawk` | text | optional | 1.0 | The identity code, four octal digits. |
| `emergency` | whole number | always | 1.0 | The emergency state (0 none). |
| `emergency_name` | text | optional | 1.0 | The emergency state in words. |
| `on_ground` | yes or no | optional | 1.0 | On the ground. |
| `alert` | yes or no | always | 1.0 | The alert bit of the flight status. |
| `spi` | yes or no | always | 1.0 | The special position identification (ident) bit. |
| `position` | position (°) | optional | 1.0 | The last position. |
| `fix` | choice | optional | 1.0 | How the position was found, or which CPR frames wait for a partner. |
| `position_time` | time | optional | 1.0 | When the position was heard, wall time. |
| `alt_ft` | whole number (ft) | optional | 1.0 | The barometric altitude. |
| `gnss_alt_ft` | whole number (ft) | optional | 1.0 | The GNSS height. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed. |
| `speed_kind` | choice | optional | 1.0 | What the speed is. |
| `heading_deg` | number (°) | optional | 1.0 | The track or heading. |
| `heading_is_track` | yes or no | optional | 1.0 | Whether the heading is the track over the ground. |
| `vertical_rate_fpm` | whole number (ft/min) | optional | 1.0 | The vertical rate. |
| `gnss_minus_baro_ft` | whole number (ft) | optional | 1.0 | GNSS height minus barometric altitude. |
| `selected_alt_ft` | whole number (ft) | optional | 1.0 | The selected altitude. |
| `selected_heading_deg` | number (°) | optional | 1.0 | The selected heading. |
| `baro_setting_hpa` | number (hPa) | optional | 1.0 | The barometric pressure setting. |
| `adsb_version` | whole number | optional | 1.0 | The ADS-B version number. |
| `nacp` | whole number | optional | 1.0 | The navigation accuracy category for position. |
| `sil` | whole number | optional | 1.0 | The source integrity level. |
| `nic` | whole number | optional | 1.0 | The navigation integrity category. |
| `ias_kt` | number (kt) | optional | 1.0 | The indicated airspeed. |
| `mach` | number | optional | 1.0 | The Mach number. |
| `roll_deg` | number (°) | optional | 1.0 | The roll angle. |
| `wind_derived_from_deg` | number (°) | optional | 1.0 | Where the derived wind blows from. |
| `wind_derived_kt` | number (kt) | optional | 1.0 | The derived wind speed. |
| `wind_reported_from_deg` | number (°) | optional | 1.0 | Where the reported wind blows from. |
| `wind_reported_kt` | number (kt) | optional | 1.0 | The reported wind speed. |
| `sat_derived_c` | number (°C) | optional | 1.0 | The static air temperature from TAS and Mach. |
| `sat_reported_c` | number (°C) | optional | 1.0 | The static air temperature reported. |
| `pressure_hpa` | number (hPa) | optional | 1.0 | The static pressure reported. |
| `humidity_pct` | number (%) | optional | 1.0 | The humidity reported. |
| `hazards` | text | optional | 1.0 | The hazards reported, with their levels. |
| `advisory` | text | optional | 1.0 | The ACAS resolution advisory in words. |
| `advisory_threat` | text | optional | 1.0 | The address of the threat of the advisory. |
| `advisory_time` | time | optional | 1.0 | When the advisory was heard, wall time. |
| `subnetwork_version` | whole number | optional | 1.0 | The Mode S subnetwork version. |
| `acas_ra_capable` | yes or no | optional | 1.0 | ACAS generates resolution advisories. |
| `gicb` | text | optional | 1.0 | The GICB registers the transponder holds. |
| `datalink` | yes or no | always | 1.0 | Comm-B messages were heard. |
| `tisb` | yes or no | always | 1.0 | Heard through TIS-B. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The mean pulse level. |
| `distance_m` | number (m) | optional | 1.0 | The distance from the station. |
| `bearing_deg` | number (°) | optional | 1.0 | The bearing from the station. |
| `source` | text | always | 1.0 | Where the last update came from: the row key of the sources table ("0" this receiver, else the number of the feed). |
| `trail` | bytes | always | 1.0 | The trail, packed. |
| `heard` | whole number | optional | 1.0 | Times heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. |

### Table `messages`

Struct `MessageRow`. A row of the message monitor: one good message of this receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | Signal time of the preamble. |
| `hex` | text | always | 1.0 | The frame in hex. |
| `icao` | text | always | 1.0 | The address. |
| `df` | whole number | always | 1.0 | The downlink format. |
| `fixed_bits` | whole number | always | 1.0 | Bits the repair flipped. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level. |
| `message` | text | always | 1.0 | The description in words. |

### Table `sources`

Struct `SourceRow`. A receiver that the table takes messages from. The row key is the number of the source: "0" for this receiver, then the number of each feed in.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The name. |
| `format` | choice | optional | 1.0 | The format, known after the first bytes. |
| `connected` | yes or no | always | 1.0 | Connected now. |
| `messages` | whole number | always | 1.0 | Messages taken. |
| `aircraft` | whole number | always | 1.0 | Aircraft whose last update came from it. |
| `error` | text | optional | 1.0 | The last error. |

### Table `by_df`

Struct `CountRow`. A count of messages of one format. The row key is the format number in two digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `format_number` | whole number | always | 1.0 | A downlink or uplink format number. |
| `count` | whole number | always | 1.0 | How many. |

### Table `gps_cells`

Struct `GpsCellRow`. A GPS interference cell of half a degree with at least three aircraft.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `corner` | position (°) | always | 1.0 | The south-west corner of the cell. |
| `aircraft` | whole number | always | 1.0 | Aircraft in the cell. |
| `level` | whole number | always | 1.0 | The level, 0 to 3. |

### Table `feeds`

Struct `FeedRow`. A feed out: a server or an outbound target. The row key is the name.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The server ("beast", "sbs", "asterix") or the outbound target ("format@host:port"). |
| `port` | whole number | optional | 1.0 | The TCP port of a server or a target. |
| `clients` | whole number | optional | 1.0 | Clients connected to a server. |
| `dropped` | whole number | optional | 1.0 | Messages dropped because a client or target was slow. |
| `connected` | yes or no | optional | 1.0 | A target is connected. |
| `error` | text | optional | 1.0 | Why a server did not start. |

### Table `modeac`

Struct `ModeAcRow`. A Mode A/C code heard at least the set number of times. The row key is the code.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `code` | text | always | 1.0 | The 13 code bits in hex. |
| `squawk` | text | always | 1.0 | The code read as a squawk. |
| `alt_ft` | whole number (ft) | optional | 1.0 | The code read as a Mode C altitude. |
| `replies` | whole number | optional | 1.0 | Replies heard. |
| `spi` | yes or no | always | 1.0 | An SPI pulse was heard. |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the last reply. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. |

### Event `adsb.aircraft`

Struct `AircraftSeen`. The fields of the event of an aircraft seen or updated. The address is the source. The urgency is an emergency for squawk 7500, 7600 or 7700 and for an emergency state.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `event` | choice | always | 1.0 | The first event of an aircraft, or a later one. |
| `source` | text | always | 1.0 | The address. |
| `callsign` | text | optional | 1.0 | The flight identification. |
| `registration` | text | optional | 1.0 | The registration derived from the address. |
| `category` | text | optional | 1.0 | The emitter category, set letter and code ("A3"). |
| `squawk` | text | optional | 1.0 | The identity code, four octal digits. |
| `emergency` | whole number | always | 1.0 | The emergency state (0 none). |
| `on_ground` | yes or no | optional | 1.0 | On the ground. |
| `altitude_ft` | whole number (ft) | optional | 1.0 | The barometric altitude. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground. |
| `vertical_rate_fpm` | whole number (ft/min) | optional | 1.0 | The vertical rate. |
| `position` | position | optional | 1.0 | The position. |
| `distance_km` | number (km) | optional | 1.0 | The distance from the station. |
| `messages` | whole number | always | 1.0 | Messages heard. |
| `urgency` | choice | always | 1.0 | How urgent the event is. |
| `text` | text | optional | 1.0 | Why the event is urgent. |
| `family` | choice | always | 1.0 | What the event is about. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_adsb_clear` | none | none |
| `reset_stats` | `sdrkit_adsb_reset_stats` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `correction` | `sdrkit_adsb_get_correction` | `sdrkit_adsb_set_correction` |
| `repair_unknown` | `sdrkit_adsb_get_repair_unknown` | `sdrkit_adsb_set_repair_unknown` |
| `expiry_s` | `sdrkit_adsb_get_expiry_s` | `sdrkit_adsb_set_expiry_s` |
| `effort` | `sdrkit_adsb_get_effort` | `sdrkit_adsb_set_effort` |
| `modeac` | `sdrkit_adsb_get_modeac` | `sdrkit_adsb_set_modeac` |
| `feeds_in` | `sdrkit_adsb_get_feeds_in` | `sdrkit_adsb_set_feeds_in` |
| `allow_remote` | `sdrkit_adsb_get_allow_remote` | `sdrkit_adsb_set_allow_remote` |
| `beast_server` | `sdrkit_adsb_get_beast_server` | `sdrkit_adsb_set_beast_server` |
| `beast_port` | `sdrkit_adsb_get_beast_port` | `sdrkit_adsb_set_beast_port` |
| `sbs_server` | `sdrkit_adsb_get_sbs_server` | `sdrkit_adsb_set_sbs_server` |
| `sbs_port` | `sdrkit_adsb_get_sbs_port` | `sdrkit_adsb_set_sbs_port` |
| `asterix_server` | `sdrkit_adsb_get_asterix_server` | `sdrkit_adsb_set_asterix_server` |
| `asterix_port` | `sdrkit_adsb_get_asterix_port` | `sdrkit_adsb_set_asterix_port` |
| `asterix_cat` | `sdrkit_adsb_get_asterix_cat` | `sdrkit_adsb_set_asterix_cat` |
| `sac` | `sdrkit_adsb_get_sac` | `sdrkit_adsb_set_sac` |
| `sic` | `sdrkit_adsb_get_sic` | `sdrkit_adsb_set_sic` |
| `allow_lan` | `sdrkit_adsb_get_allow_lan` | `sdrkit_adsb_set_allow_lan` |
| `targets` | `sdrkit_adsb_get_targets` | `sdrkit_adsb_set_targets` |
| `drop_s` | `sdrkit_adsb_get_drop_s` | `sdrkit_adsb_set_drop_s` |
| `gps_layer` | `sdrkit_adsb_get_gps_layer` | `sdrkit_adsb_set_gps_layer` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_aircraft` | `sdrkit_adsb_gen_get_aircraft` | `sdrkit_adsb_gen_set_aircraft` |
| `gen_replies` | `sdrkit_adsb_gen_get_replies` | `sdrkit_adsb_gen_set_replies` |
| `gen_emergency` | `sdrkit_adsb_gen_get_emergency` | `sdrkit_adsb_gen_set_emergency` |
| `gen_rate` | `sdrkit_adsb_gen_get_rate` | `sdrkit_adsb_gen_set_rate` |
| `gen_filter_mhz` | `sdrkit_adsb_gen_get_filter_mhz` | `sdrkit_adsb_gen_set_filter_mhz` |
| `gen_centre_lat` | `sdrkit_adsb_gen_get_centre_lat` | `sdrkit_adsb_gen_set_centre_lat` |
| `gen_centre_lon` | `sdrkit_adsb_gen_get_centre_lon` | `sdrkit_adsb_gen_set_centre_lon` |

## Try it

```sh
sdrkit run adsb --source synth:adsb
sdrkit run adsb --source iiod:192.168.2.1 --freq 1090
```

## Test recordings

- `tests/vectors/adsb/adsb_airspace_2m.sigmf-meta`
- `tests/vectors/real/adsb/adsb_1090_sydney.sigmf-meta`
- `tests/vectors/real/adsb/adsb_1090_sydney_auto_gain.sigmf-meta`
<!-- generated: end -->
