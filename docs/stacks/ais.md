<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# AIS

Ships, base stations and aids to navigation on AIS 1 and 2 and the long-range channels, on a map and in a table.

Stack id: `ais`. Group: Marine.

Signal types: AIS.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 162 MHz |
| Tuning range | 156 MHz to 163 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 100 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ais` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channels` | Channels | choice | `all` | `all` (All four), `ais12` (AIS 1 + 2 only), `longrange` (75 + 76 only) | Which AIS channels are decoded. |
| `collision_cancel` | Collision handling | yes or no | true |  | Tries again on other frequencies and after cancelling a stronger burst. |
| `satellite` | Satellite mode | yes or no | false |  | Wider filters, Doppler candidates and successive cancellation for reception from orbit. |
| `hop` | Hopping | yes or no | false |  | Alternates between AIS 1 + 2 and 75 + 76 when the radio cannot hold all four. |
| `time_base` | Time base | time | 0 s | 0 to 100000000000 | UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock. |
| `satellite_tle` | Satellite elements | text | empty |  | Two-line elements of the satellite (with checksums), for predicted Doppler and passes. |
| `min_elevation` | Minimum elevation | number (°) | 0 | 0 to 40 |  |
| `arm_passes` | Arm the passes | yes or no | false |  | Predicts the next pass over the station. The station position is needed. |
| `cpa` | CPA alerts | yes or no | false |  | Alerts for vessels that will pass close to the station. The station position is needed. |
| `cpa_nm` | CPA limit | number (nm) | 0.5 | 0.05 to 5 |  |
| `tcpa_min` | TCPA limit | number (min) | 15 | 1 to 60 |  |
| `cpa_cooldown_min` | CPA cool-down | number (min) | 10 | 1 to 120 |  |
| `own_moving` | Own ship moving | yes or no | false |  |  |
| `own_sog_kn` | Own speed | number (kn) | 0 | 0 to 40 |  |
| `own_cog_deg` | Own course | number (°) | 0 | 0 to 359 |  |
| `geofences` | Geofences | text | empty |  | One per line: name;lat,lon,radius_nm or name;poly;lat,lon lat,lon ... |
| `filter` | Filter | text | empty |  | classes=63;groups=1023;moving=0;sog=0.5 |
| `nmea_out` | Send AIS as NMEA | yes or no | true |  |  |
| `names` | Name table | text | empty |  | Ship and station names by MMSI to import, one record per line. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_vessels` | Vessels | whole number | 15 | 1 to 40 | Vessels in the port demo, beside the base station, two aids and a SAR aircraft. |
| `gen_speed` | Speed of time | number | 1 | 0.1 to 100 | Vessels move this many times faster than real time. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `bursts` | Bursts | whole number |
| `recovered` | Recovered | whole number |
| `cancelled` | Cancelled | whole number |
| `collision_recoveries` | Collision recoveries | whole number |
| `stations` | Stations | whole number |
| `with_position` | With a position | whole number |
| `long_range_stations` | Long-range stations | whole number |
| `snr_db` | SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `reports` | Reports | whole number |
| `signal_time` | Signal time | duration (s) |
| `filtered` | Some stations hidden | yes or no |
| `names` | Names | whole number |
| `next_pass` | Next pass | time |
| `next_pass_elevation_deg` | Next pass elevation | number (°) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `stations` | Stations | `name` (text), `mmsi` (text), `callsign` (text), `type` (text), `speed_kt` (number (kt)), `course_deg` (number (°)), `position` (position), `long_range_position` (yes or no), `channel` (text), `status` (text), `destination` (text), `last_heard` (time), `heard` (whole number) |
| `track_points` | Tracks | `station` (text), `position` (position), `time` (time) |
| `counters` | Messages by type | `type` (text), `count_a` (whole number), `count_b` (whole number), `count_c` (whole number), `count_d` (whole number), `total` (whole number) |
| `channels` | Channels | `name` (text), `freq_hz` (frequency), `frames_ok` (whole number), `frames_bad` (whole number), `messages_per_min` (number (/min)), `level_dbfs` (number (dBFS)), `noise_dbfs` (number (dBFS)), `doppler_hz` (frequency), `doppler_rate_hz_s` (number (Hz/s)) |
| `sentences` | Sentences | `sentence` (text) |
| `receptions` | Receptions | `time` (time), `channel` (text), `msg_type` (whole number), `mmsi` (text), `snr_db` (number (dB)), `offset_hz` (frequency), `layer` (choice) |
| `alerts` | Alerts | `time` (time), `kind` (choice), `fence` (text), `mmsi` (text), `name` (text), `position` (position), `distance_m` (number (m)), `cpa_m` (number (m)), `tcpa` (duration (s)) |
| `hop_plan` | Hopping plan | `centre_hz` (frequency), `dwell` (duration (s)) |
| `selected` | Selected station | `name` (text), `mmsi` (text), `class` (text), `callsign` (text), `imo` (whole number), `type` (text), `destination` (text), `eta` (text), `draught_m` (number (m)), `length_m` (number (m)), `beam_m` (number (m)), `position` (position), `speed_kt` (number (kt)), `course_deg` (number (°)), `heading_deg` (number (°)), `rate_of_turn_deg_min` (number (°/min)), `altitude_m` (number (m)), `status` (text), `heard` (whole number), `last_type` (whole number), `channel` (text), `last_heard` (time), `long_range` (yes or no), `latency` (yes or no), `sentences` (text) |

## Map points

The stack puts points on a map: Stations.

## Events

| Kind | Label | Fields |
|---|---|---|
| `ais.vessel` | Vessel | `source` (text), `family` (choice), `urgency` (choice), `text` (text), `name` (text), `callsign` (text), `ship_type` (whole number), `position` (position), `speed_kt` (number (kt)), `course_deg` (number (°)), `nav_status` (whole number), `class` (text), `destination` (text) |
| `ais.nmea` | AIS sentence | `source` (text), `family` (choice), `sentence` (text) |
| `ais.cpa` | CPA alert | `source` (text), `family` (choice), `urgency` (choice), `name` (text), `position` (position), `distance_m` (number (m)), `cpa_m` (number (m)), `tcpa` (duration (s)) |
| `ais.geofence` | Geofence alert | `source` (text), `family` (choice), `urgency` (choice), `name` (text), `position` (position), `distance_m` (number (m)), `fence` (text), `transition` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `select` | Select | `row` (text, required) |  | Shows one station in full. |
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `marine`, version 1.3. The C header `sdrkit/marine.h`, the registry record `registry/marine.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Position tracking (`tracking`). Media: none.

### Result fields

Struct `AisFields`. The result fields of the AIS stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | The reception state of a display. |
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `bursts` | whole number | always | 1.0 | Bursts detected. |
| `recovered` | whole number | always | 1.0 | Messages found on another frequency candidate or part of a burst. |
| `cancelled` | whole number | always | 1.0 | Messages found after cancelling a stronger burst. |
| `collision_recoveries` | whole number | always | 1.0 | Recovered and cancelled messages together. |
| `stations` | whole number | always | 1.0 | Stations in the table. |
| `with_position` | whole number | always | 1.0 | Stations with a position. |
| `long_range_stations` | whole number | always | 1.0 | Stations that sent message 27. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `offset_hz` | frequency | optional | 1.0 | Carrier offset from where it was expected, Hz. |
| `reports` | whole number | always | 1.0 | Reports made. |
| `signal_time` | duration (s) | always | 1.0 | Signal time since the start. |
| `filtered` | yes or no | always | 1.0 | The view filter hides some stations. |
| `names` | whole number | always | 1.0 | Names in the name table. |
| `next_pass` | time | optional | 1.0 | The start of the next pass of the satellite over the station. |
| `next_pass_elevation_deg` | number (°) | optional | 1.0 | The highest elevation of the next pass. |

### Table `stations`

Struct `AisStationRow`. A row of the AIS station table. The row key is the MMSI.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The station name, else its MMSI. |
| `mmsi` | text | always | 1.0 | The MMSI, nine digits. |
| `callsign` | text | optional | 1.0 | The call sign of a station heard. |
| `type` | text | always | 1.0 | The ship type in words, else the class. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `long_range_position` | yes or no | always | 1.0 | The position came from message 27. |
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `status` | text | optional | 1.0 | The navigational status in words. |
| `destination` | text | optional | 1.0 | The destination. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |
| `heard` | whole number | optional | 1.0 | Times heard. |

The column `name` names a row. Place of a row: the column `position`. Course: `course_deg`. Speed: `speed_kt`.

### Table `track_points`

Struct `AisTrackPointRow`. A row of the track points of the stations whose track is shown. The row key is the MMSI, `/` and the time of the point.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `station` | text | always | 1.0 | The station a track point belongs to. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `time` | time | optional | 1.0 | When (wall-clock time, when known). |

The column `station` names a row. Place of a row: the column `position`.

### Table `counters`

Struct `AisCounterRow`. A row of the count of messages by type. The row key is the message type in two digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `type` | text | always | 1.0 | The message type name. |
| `count_a` | whole number | always | 1.0 | Messages on AIS 1. |
| `count_b` | whole number | always | 1.0 | Messages on AIS 2. |
| `count_c` | whole number | always | 1.0 | Messages on channel 75. |
| `count_d` | whole number | always | 1.0 | Messages on channel 76. |
| `total` | whole number | always | 1.0 | Messages on every channel. |

The column `type` names a row.

### Table `channels`

Struct `AisChannelRow`. A row of the channel table of the AIS stack. The row key is the letter of the channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The channel's name. |
| `freq_hz` | frequency | always | 1.0 | The frequency a station was heard on. |
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `messages_per_min` | number (/min) | always | 1.0 | Good messages in the last minute. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `noise_dbfs` | number (dBFS) | optional | 1.0 | Noise floor, dBFS. |
| `doppler_hz` | frequency | optional | 1.0 | The Doppler of the channel's bursts now (satellite mode). |
| `doppler_rate_hz_s` | number (Hz/s) | optional | 1.0 | The rate of the Doppler (satellite mode). |

The column `name` names a row. Default order: the column `freq_hz`, lowest first.

### Table `sentences`

Struct `AisSentenceRow`. A row of the NMEA sentence table. The row key is the place of the sentence in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `sentence` | text | always | 1.0 | An `!AIVDM` sentence. |

### Table `receptions`

Struct `AisReceptionRow`. A row of the reception table. The row key is the place of the reception in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | When (wall-clock time, when known). |
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `msg_type` | whole number | always | 1.0 | The message type. |
| `mmsi` | text | always | 1.0 | The MMSI, nine digits. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `offset_hz` | frequency | always | 1.0 | Carrier offset from where it was expected, Hz. |
| `layer` | choice | always | 1.0 | How a message was found. |

The column `mmsi` names a row.

### Table `alerts`

Struct `AisAlertRow`. A row of the alert table. The row key is the place of the alert in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | optional | 1.0 | When (wall-clock time, when known). |
| `kind` | choice | always | 1.0 | What an alert is about. |
| `fence` | text | optional | 1.0 | The fence of an enter or exit. |
| `mmsi` | text | always | 1.0 | The MMSI, nine digits. |
| `name` | text | always | 1.0 | The station name, else its MMSI. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `distance_m` | number (m) | always | 1.0 | The distance from the station (CPA) or the fence. |
| `cpa_m` | number (m) | optional | 1.0 | The distance at the closest point of approach. |
| `tcpa` | duration (s) | optional | 1.0 | The time to the closest point of approach. |

The column `name` names a row. Place of a row: the column `position`.

### Table `hop_plan`

Struct `AisHopRow`. A row of the hop plan of a receiver that is narrower than the four channels. The row key is the number of the step.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `centre_hz` | frequency | always | 1.0 | The centre of a hopping step. |
| `dwell` | duration (s) | optional | 1.0 | The dwell of a hopping step (none for one step that covers all). |

Default order: the column `centre_hz`, lowest first.

### Table `selected`

Struct `AisSelectedRow`. The row of the selected station, with every value that the station sent. The row key is the MMSI.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The station name, else its MMSI. |
| `mmsi` | text | always | 1.0 | The MMSI, nine digits. |
| `class` | text | always | 1.0 | The station class. |
| `callsign` | text | optional | 1.0 | The call sign of a station heard. |
| `imo` | whole number | optional | 1.0 | The IMO number. |
| `type` | text | optional | 1.0 | The ship type in words, else the class. |
| `destination` | text | optional | 1.0 | The destination. |
| `eta` | text | optional | 1.0 | The estimated arrival, "MM-DD HH:MM UTC". |
| `draught_m` | number (m) | optional | 1.0 | The draught. |
| `length_m` | number (m) | optional | 1.0 | The length. |
| `beam_m` | number (m) | optional | 1.0 | The beam. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `heading_deg` | number (°) | optional | 1.0 | The true heading. |
| `rate_of_turn_deg_min` | number (°/min) | optional | 1.0 | The rate of turn. |
| `altitude_m` | number (m) | optional | 1.0 | The altitude of a SAR aircraft. |
| `status` | text | optional | 1.0 | The navigational status in words. |
| `heard` | whole number | optional | 1.0 | Times heard. |
| `last_type` | whole number | always | 1.0 | The type of the last message. |
| `channel` | text | always | 1.0 | The channel or talk group of a call. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |
| `long_range` | yes or no | always | 1.0 | It sent message 27. |
| `latency` | yes or no | optional | 1.0 | The position latency flag of message 27. |
| `sentences` | text | always | 1.0 | The last sentences, one per line. |

The column `name` names a row. Place of a row: the column `position`. Height: `altitude_m`. Course: `course_deg`. Speed: `speed_kt`.

### Event `ais.vessel`

Struct `AisVesselEvent`. The fields of the event of a station heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `text` | text | optional | 1.0 | The text of a decoded message. |
| `name` | text | always | 1.0 | The station name, else its MMSI. |
| `callsign` | text | always | 1.0 | The call sign of a station heard. |
| `ship_type` | whole number | optional | 1.0 | The ship and cargo type code. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `speed_kt` | number (kt) | optional | 1.0 | The speed over the ground of the object that the event is about, knots. |
| `course_deg` | number (°) | optional | 1.0 | The course over the ground of the object that the event is about, degrees clockwise from true north. |
| `nav_status` | whole number | optional | 1.0 | The navigational status code. |
| `class` | text | always | 1.0 | The station class. |
| `destination` | text | always | 1.0 | The destination. |

### Event `ais.nmea`

Struct `AisNmeaEvent`. The fields of the event of an NMEA sentence.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `sentence` | text | always | 1.0 | An `!AIVDM` sentence. |

### Event `ais.cpa`

Struct `AisCpaEvent`. The fields of the event of a closest point of approach alert.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `name` | text | always | 1.0 | The station name, else its MMSI. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `distance_m` | number (m) | always | 1.0 | The distance from the station (CPA) or the fence. |
| `cpa_m` | number (m) | optional | 1.0 | The distance at the closest point of approach. |
| `tcpa` | duration (s) | optional | 1.0 | The time to the closest point of approach. |

### Event `ais.geofence`

Struct `AisGeofenceEvent`. The fields of the event of a geofence alert.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `name` | text | always | 1.0 | The station name, else its MMSI. |
| `position` | position | always | 1.0 | The position of the object that the event is about. |
| `distance_m` | number (m) | always | 1.0 | The distance from the station (CPA) or the fence. |
| `fence` | text | always | 1.0 | The fence of an enter or exit. |
| `transition` | choice | always | 1.0 | A fence crossed: enter or exit. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `select` | `sdrkit_ais_select` | `sdrkit_ais_station_args` | none |
| `clear` | `sdrkit_ais_clear` | none | none |
| `remove` | `sdrkit_ais_remove` | `sdrkit_ais_station_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channels` | `sdrkit_ais_get_channels` | `sdrkit_ais_set_channels` |
| `collision_cancel` | `sdrkit_ais_get_collision_cancel` | `sdrkit_ais_set_collision_cancel` |
| `satellite` | `sdrkit_ais_get_satellite` | `sdrkit_ais_set_satellite` |
| `hop` | `sdrkit_ais_get_hop` | `sdrkit_ais_set_hop` |
| `time_base` | `sdrkit_ais_get_time_base` | `sdrkit_ais_set_time_base` |
| `satellite_tle` | `sdrkit_ais_get_satellite_tle` | `sdrkit_ais_set_satellite_tle` |
| `min_elevation` | `sdrkit_ais_get_min_elevation` | `sdrkit_ais_set_min_elevation` |
| `arm_passes` | `sdrkit_ais_get_arm_passes` | `sdrkit_ais_set_arm_passes` |
| `cpa` | `sdrkit_ais_get_cpa` | `sdrkit_ais_set_cpa` |
| `cpa_nm` | `sdrkit_ais_get_cpa_nm` | `sdrkit_ais_set_cpa_nm` |
| `tcpa_min` | `sdrkit_ais_get_tcpa_min` | `sdrkit_ais_set_tcpa_min` |
| `cpa_cooldown_min` | `sdrkit_ais_get_cpa_cooldown_min` | `sdrkit_ais_set_cpa_cooldown_min` |
| `own_moving` | `sdrkit_ais_get_own_moving` | `sdrkit_ais_set_own_moving` |
| `own_sog_kn` | `sdrkit_ais_get_own_sog_kn` | `sdrkit_ais_set_own_sog_kn` |
| `own_cog_deg` | `sdrkit_ais_get_own_cog_deg` | `sdrkit_ais_set_own_cog_deg` |
| `geofences` | `sdrkit_ais_get_geofences` | `sdrkit_ais_set_geofences` |
| `filter` | `sdrkit_ais_get_filter` | `sdrkit_ais_set_filter` |
| `nmea_out` | `sdrkit_ais_get_nmea_out` | `sdrkit_ais_set_nmea_out` |
| `names` | `sdrkit_ais_get_names` | `sdrkit_ais_set_names` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_vessels` | `sdrkit_ais_gen_get_vessels` | `sdrkit_ais_gen_set_vessels` |
| `gen_speed` | `sdrkit_ais_gen_get_speed` | `sdrkit_ais_gen_set_speed` |

## Try it

```sh
sdrkit run ais --source synth:ais
sdrkit run ais --source iiod:192.168.2.1 --freq 162
```

## Test recordings

- `tests/vectors/marine/ais_gpsd_250k.sigmf-meta`
<!-- generated: end -->
