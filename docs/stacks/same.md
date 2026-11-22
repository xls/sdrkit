<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# NOAA Weather Radio SAME

Emergency and weather alerts (SAME headers) on the US and Canadian weather radio channels.

Stack id: `same`. Group: Science, time and utility.

Signal types: SAME.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 162.4 MHz to 162.55 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 64 kHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:same` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `channel` | Weather channel | choice | `wx7` | `wx1` (WX1), `wx2` (WX2), `wx3` (WX3), `wx4` (WX4), `wx5` (WX5), `wx6` (WX6), `wx7` (WX7) |  |
| `country` | Location names | choice | `us` | `us` (United States), `canada` (Canada) |  |
| `county_text` | County table | text | empty |  | The text of a county table: lines SSCCC,Name or SS,CCC,Name or the Census form. The host reads the file. |
| `year` | Year for issue times | whole number | 0 | 0 to 3000 |  |
| `listen` | Listen | yes or no | false |  |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Test signal: scene | choice | `two` | `two` (Two alerts), `tornado` (Tornado warning), `noise` (No message) |  |
| `gen_loop` | Test signal: loop | yes or no | true |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `channel` | Channel | choice |
| `frequency` | Frequency | frequency |
| `messages_ok` | Messages | whole number |
| `messages_bad` | Damaged messages | whole number |
| `tone_now` | Alert tone now | yes or no |
| `country` | Location names | choice |
| `counties` | Counties loaded | whole number |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `carrier` | Carrier error | frequency |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `alerts` | Messages | `signal_time_s` (number (s)), `host_time` (time), `kind` (choice), `event` (text), `event_name` (text), `event_kind` (choice), `originator` (text), `originator_name` (text), `locations` (whole number), `valid_for` (duration (s)), `issued` (time), `callsign` (text), `repeats` (whole number), `agreeing` (whole number), `bits_changed` (whole number), `snr_db` (number (dB)), `carrier` (frequency), `tone` (duration (s)), `raw` (text), `error` (choice) |
| `alert_locations` | Locations | `alert` (text), `code` (text), `place` (text), `position` (position) |
| `tones` | Alert tones | `signal_time_s` (number (s)), `duration` (duration (s)), `level_db` (number (dB)) |

## Map points

The stack puts points on a map: Alert areas.

## Events

| Kind | Label | Fields |
|---|---|---|
| `same.alert` | SAME alert | `source` (text), `text` (text), `family` (choice), `urgency` (choice), `position` (position), `event` (text), `event_name` (text), `event_kind` (choice), `originator` (text), `valid_for` (duration (s)), `callsign` (text), `locations` (whole number), `raw` (text) |
| `same.end` | SAME end of message | `text` (text), `family` (choice), `raw` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `clear_counties` | Clear the county table |  |  | Forgets the loaded county table. |

## Typed interface

The stack is in the family `science`, version 1.2. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Datalink messages (`datalink`). Media: sound (programme), text.

### Result fields

Struct `SameFields`. The result fields of `same`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channel` | choice | always | 1.0 | The channel. |
| `frequency` | frequency | always | 1.0 | The channel frequency. |
| `messages_ok` | whole number | always | 1.0 | Messages decoded. |
| `messages_bad` | whole number | always | 1.0 | Messages damaged. |
| `tone_now` | yes or no | always | 1.0 | An attention tone is on the air now. |
| `country` | choice | always | 1.0 | The country of the location names. |
| `counties` | whole number | always | 1.0 | Counties in the loaded table. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `carrier` | frequency | always | 1.0 | The carrier error. |

### Table `alerts`

Struct `SameAlertRow`. A SAME message. The row key is the count of rows added before it, as six digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_time_s` | number (s) | always | 1.0 | The signal time of a row. |
| `host_time` | time | optional | 1.0 | The host time of a row. |
| `kind` | choice | always | 1.0 | What the row is. |
| `event` | text | optional | 1.0 | The event code. |
| `event_name` | text | optional | 1.0 | The event name. |
| `event_kind` | choice | optional | 1.0 | The kind of event (warning, watch ...). |
| `originator` | text | optional | 1.0 | The originator code. |
| `originator_name` | text | optional | 1.0 | The originator's name. |
| `locations` | whole number | always | 1.0 | The number of locations. |
| `valid_for` | duration (s) | optional | 1.0 | How long the message is valid. |
| `issued` | time | optional | 1.0 | When the message was issued. |
| `callsign` | text | optional | 1.0 | The sender's call sign. |
| `repeats` | whole number | always | 1.0 | The repeats heard. |
| `agreeing` | whole number | always | 1.0 | The repeats identical to the result. |
| `bits_changed` | whole number | always | 1.0 | Bits the vote changed. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `carrier` | frequency | always | 1.0 | The carrier error. |
| `tone` | duration (s) | optional | 1.0 | The attention tone that followed. |
| `raw` | text | always | 1.0 | The raw text. |
| `error` | choice | optional | 1.0 | Why the message is damaged. |

The column `callsign` names a row.

### Table `alert_locations`

Struct `SameAlertLocationRow`. A location of a SAME message. The row key is `<alert>/<n>`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `alert` | text | always | 1.0 | The alert a location belongs to. |
| `code` | text | always | 1.0 | A location code (PSSCCC). |
| `place` | text | always | 1.0 | The location as people read it. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |

The column `place` names a row. Place of a row: the column `position`.

### Table `tones`

Struct `SameToneRow`. An attention tone. The row key is the sequence number of the tone.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_time_s` | number (s) | always | 1.0 | The signal time of a row. |
| `duration` | duration (s) | always | 1.0 | The length of a tone. |
| `level_db` | number (dB) | always | 1.0 | The level of a tone. |

### Event `same.alert`

Struct `SameAlertEvent`. A SAME alert. `source` is the sender's call sign, `text` the event and the places, `position` the first place on the map.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `text` | text | always | 1.0 | The text of a decoded message. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `urgency` | choice | always | 1.0 | How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `event` | text | always | 1.0 | The event code. |
| `event_name` | text | optional | 1.0 | The event name. |
| `event_kind` | choice | optional | 1.0 | The kind of event (warning, watch ...). |
| `originator` | text | always | 1.0 | The originator code. |
| `valid_for` | duration (s) | always | 1.0 | How long the message is valid. |
| `callsign` | text | always | 1.0 | The sender's call sign. |
| `locations` | whole number | always | 1.0 | The number of locations. |
| `raw` | text | always | 1.0 | The raw text. |

### Event `same.end`

Struct `SameEndEvent`. A SAME end of message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `raw` | text | always | 1.0 | The raw text. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_same_clear` | none | none |
| `clear_counties` | `sdrkit_same_clear_counties` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `channel` | `sdrkit_same_get_channel` | `sdrkit_same_set_channel` |
| `country` | `sdrkit_same_get_country` | `sdrkit_same_set_country` |
| `county_text` | `sdrkit_same_get_county_text` | `sdrkit_same_set_county_text` |
| `year` | `sdrkit_same_get_year` | `sdrkit_same_set_year` |
| `listen` | `sdrkit_same_get_listen` | `sdrkit_same_set_listen` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_same_gen_get_scene` | `sdrkit_same_gen_set_scene` |
| `gen_loop` | `sdrkit_same_gen_get_loop` | `sdrkit_same_gen_set_loop` |

## Try it

```sh
sdrkit run same --source synth:same
sdrkit run same --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/science/same_two_alerts_64k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
