<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# HF beacons

Propagation monitor: the NCDXF/IARU beacons, the Long Dashes and the single-letter beacons heard in a wide HF capture.

Stack id: `hfbeacons`. Group: HF utility.

HF beacons: the propagation monitor. No decoder: it watches the frequencies of an editable beacon list in a wide HF capture, distinguishes each beacon from a random carrier by its keying and schedule, measures its SNR and keeps its history. ```text iq (wide) -> [stage 1 filter bank] -> [stage 2, channel meter] -> hfb.excess --+--> [runs] -> hfb.run --+--> [keying judges] -> hfb.detection -> [stats] -> results +-------------------------+ iq (wide) -> [HF channel front end at the beacon - 700 Hz] -> [sideband audio] -> [listen] -> audio iq -> [own beacon transmitter: start, send, stop] -> [tx sink] ```

Signal types: CW (Morse).

## Tuning

| Property | Value |
|---|---|
| Default frequency | 14.5 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 10 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 8 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:hfbeacons` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `ppm` | Frequency error | number (ppm) | 0 | -100 to 100 | The frequency error of the radio's oscillator: 1 ppm is 14 Hz at 14 MHz. Signals are looked for at f x (1 + ppm 10^-6). |
| `hop` | Step through the bands | yes or no | true |  | Step through the bands when the capture is narrower than the list. |
| `dwell_s` | Dwell | duration (s) | 60 s | 10 to 600 | Time per stop. 180 s per stop hears all 18 NCDXF beacons on every band in the capture. The Long Dashes cycle is 27 s. Letters repeat within seconds. |
| `watch_ncdxf` | NCDXF/IARU | yes or no | true |  |  |
| `watch_dashes` | Long Dashes | yes or no | true |  |  |
| `watch_letters` | Single letters | yes or no | true |  |  |
| `watch_carriers` | Your carriers | yes or no | true |  |  |
| `list_json` | Beacon list | text | empty |  | A beacon list in the JSON format of the list file. Empty: the built-in list. |
| `clock_offset_s` | Clock offset | number (s) | 0 | -30 to 30 | How far the computer clock runs ahead of true UTC. The NCDXF slots need it right to about a second. |
| `start_utc_s` | Recording start (UTC seconds; 0 unknown) | number (s) | 0 | 0 to 100000000000 |  |
| `listen` | Listen | yes or no | false |  | Play the sideband (dial + 200 Hz to 3.8 kHz). Decoding is the same either way. |
| `listen_khz` | Listen to the beacon on | number (kHz) | 0 | 0 to 1000000 |  |
| `call` | Call sign | text | empty |  | 1 to 12 characters of A-Z, 0-9 and /. Empty refuses start. |
| `message` | Message | text | `VVV DE {call}` |  | Up to 60 characters of A-Z, 0-9, space and / ? . , =. {call} stands for the call sign; a message without it is followed by DE and the call sign; empty sends the call sign alone. |
| `wpm` | Speed | number (wpm) | 22 | 5 to 40 | Words a minute of the word PARIS: a dot is 1.2 / wpm s. |
| `pitch_hz` | Tone | frequency | 800 Hz | 300 Hz to 2.5 kHz | The tone above the dial: the beacon is on the dial plus this frequency. |
| `long_dash_s` | Long dash | number (s) | 3 | 0 to 10 | A steady carrier after the message, a word gap after it, for level checks. 0 sends none. |
| `interval_min` | Interval | whole number (min) | 10 | 1 to 60 | From the start of one sending to the start of the next. A sending longer than the interval is followed at once by the next. |
| `power_steps` | Power steps | yes or no | false |  | Four dashes of 1 s at 0, -10, -20 and -30 dB after the long dash, with no gap, as in the slot of the international beacon project. |
| `offset_s` | Start in the interval | number (s) | 0 | 0 to 3599 | The sendings of start begin this many seconds after each interval boundary of UTC (an interval of 10 min: on the hour, 10 past, ...). Taken modulo the interval. |
| `count` | Transmissions | whole number | 0 | 0 to 1000 | The beacon ends after this many sendings. 0 sends until stop. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_signal` | Signal | choice | `beacons` | `beacons` (Beacons), `nothing` (Nothing) |  |
| `gen_centre_hz` | Centre (0 = 3.65 MHz) | frequency | 0 Hz | 0 Hz to 1 GHz |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `span_low` | Span from | frequency |
| `span_high` | Span to | frequency |
| `in_span` | Beacons in the span | whole number |
| `heard_now` | Heard now | whole number |
| `channels` | Channels watched | whole number |
| `frames` | Frames processed | whole number |
| `runs_started` | Runs started | whole number |
| `clock` | Clock | choice |
| `clock_valid` | Clock valid | yes or no |
| `list_count` | Beacons in the list | whole number |
| `station_known` | Station position known | yes or no |
| `best_band` | Band to try now | text |
| `best_band_stations` | Stations on it | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `tx_beacon_on` | Beacon on | yes or no |
| `tx_sending` | Sending | yes or no |
| `tx_progress_pct` | Sending progress | number (%) |
| `tx_next_s` | Next sending in | duration (s) |
| `tx_sendings` | Sendings | whole number |
| `tx_sending_s` | Sending time on air | duration (s) |
| `tx_duty_pct` | Duty of the schedule | number (%) |
| `tx_note` | Transmit note | text |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `beacons` | Beacons | `name` (text), `frequency` (frequency), `kind` (choice), `uncertain` (yes or no), `in_capture` (yes or no), `heard` (yes or no), `confidence` (whole number), `last` (duration (s)), `last_heard` (time), `snr_db` (number (dB)), `best_1h_db` (number (dB)), `best_24h_db` (number (dB)), `steps` (whole number), `step_snrs` (text), `distance_km` (number (km)), `bearing_deg` (number (deg)), `trend` (text), `listens` (whole number), `heard_count` (whole number), `note` (text), `band` (text), `position` (position) |
| `bands` | Bands open | `band` (text), `in_list` (whole number), `in_capture` (whole number), `heard_now` (whole number), `farthest_km` (number (km)) |
| `propagation` | Propagation | `band` (text), `state` (choice), `stations` (whole number), `best_snr_db` (number (dB)), `farthest_km` (number (km)), `score` (number) |

## Map points

The stack puts points on a map: Beacons heard.

## Events

| Kind | Label | Fields |
|---|---|---|
| `hfb.detection` | Beacon heard | `source` (text), `snr_db` (number (dB)), `confidence` (whole number), `text` (text), `position` (position), `family` (choice) |
| `hfb.tx` | Own beacon | `state` (text), `reason` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `list.restore` | Restore the built-in list |  |  | Goes back to the built-in beacon list. |
| `start` | Start the beacon |  |  | Sends the message in Morse at offset_s after the next interval boundary of UTC (at once when that is now), then again each interval, until stop or count sendings. Each sending is one transmission; between sendings the transmitter is free. Transmit must be on and the call sign set. |
| `stop` | Stop the beacon |  |  | Ends the sending on air within 50 ms, with its fall, and ends the schedule. |
| `send` | Send the beacon once |  |  | Sends the message in Morse once, at once, outside the schedule. Transmit must be on and the call sign set. |

## Typed interface

The stack is in the family `hfdata`, version 1.6. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: sound (monitor).

### Result fields

Struct `HfbeaconsFields`. The result fields of the HF beacon monitor.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `span_low` | frequency | always | 1.0 | The low edge of the span shown (centre - 0.4 x rate). |
| `span_high` | frequency | always | 1.0 | The high edge of the span shown (centre + 0.4 x rate). |
| `in_span` | whole number | always | 1.0 | Beacons of the list in the span. |
| `heard_now` | whole number | always | 1.0 | Beacons heard now. |
| `channels` | whole number | always | 1.0 | Channels watched. |
| `frames` | whole number | always | 1.0 | Frames processed. |
| `runs_started` | whole number | always | 1.0 | Keying runs started. |
| `clock` | choice | always | 1.0 | Where the times come from. |
| `clock_valid` | yes or no | always | 1.0 | The clock is good enough for the time slots of the beacons. |
| `list_count` | whole number | always | 1.0 | Beacons in the list. |
| `station_known` | yes or no | always | 1.0 | The station position is known. |
| `best_band` | text | optional | 1.0 | The band to try now, when the model names one. |
| `best_band_stations` | whole number | optional | 1.0 | The stations heard on that band. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the input, dBFS. |

### Result fields

Struct `HfbTxFields`. The result fields of the own beacon. The values of a sending have no value while nothing is sent.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_beacon_on` | yes or no | always | 1.2 | Whether the beacon runs: it sends now or waits for its next sending. |
| `tx_sending` | yes or no | always | 1.2 | Whether a sending is on air. |
| `tx_progress_pct` | number (%) | optional | 1.2 | The share of the sending on air that is sent. |
| `tx_next_s` | duration (s) | optional | 1.2 | The time to the next sending, while the beacon waits. |
| `tx_sendings` | whole number | always | 1.2 | The sendings since `start`. |
| `tx_sending_s` | duration (s) | always | 1.2 | The time on air of one sending with the present settings. |
| `tx_duty_pct` | number (%) | always | 1.3 | The share of the interval that one sending is on air. |
| `tx_note` | text | always | 1.2 | What the beacon is: a proposal of the specification, not taken from a standard. |

### Table `beacons`

Struct `HfbeaconsBeaconRow`. A beacon of the list. The row key is the place in the list, "0000" first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `name` | text | always | 1.0 | The beacon's name. |
| `frequency` | frequency | always | 1.0 | The frequency of the beacon. |
| `kind` | choice | always | 1.0 | The keying pattern. |
| `uncertain` | yes or no | always | 1.0 | The entry of the list is uncertain. |
| `in_capture` | yes or no | always | 1.0 | The beacon is inside the capture. |
| `heard` | yes or no | always | 1.0 | The beacon is heard now. |
| `confidence` | whole number | always | 1.0 | The confidence of the identity. |
| `last` | duration (s) | optional | 1.0 | The signal time when last heard. |
| `last_heard` | time | optional | 1.0 | The UTC when last heard, in whole seconds. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR when last heard. |
| `best_1h_db` | number (dB) | optional | 1.0 | The best SNR of the last hour. |
| `best_24h_db` | number (dB) | optional | 1.0 | The best SNR of the last 24 hours. |
| `steps` | whole number | always | 1.0 | Power steps heard. |
| `step_snrs` | text | always | 1.0 | The SNR of each power step. |
| `distance_km` | number (km) | optional | 1.0 | The distance to the station. |
| `bearing_deg` | number (deg) | optional | 1.0 | The bearing from the station. |
| `trend` | text | always | 1.0 | The newest 48 history points in dB, "-" for listened and not heard. |
| `listens` | whole number | always | 1.0 | History points in 24 hours. |
| `heard_count` | whole number | always | 1.0 | History points heard. |
| `note` | text | always | 1.0 | A note on the beacon. |
| `band` | text | always | 1.0 | The band of the beacon. |
| `position` | position | optional | 1.0 | The position of the beacon. |

The column `name` names a row. Place of a row: the column `position`.

### Table `bands`

Struct `HfbeaconsBandRow`. A band of the beacon list. The row key is the place, "00" first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `band` | text | always | 1.0 | The band. |
| `in_list` | whole number | always | 1.0 | Beacons of the band in the list. |
| `in_capture` | whole number | always | 1.0 | Beacons of the band in the capture. |
| `heard_now` | whole number | always | 1.0 | Beacons of the band heard now. |
| `farthest_km` | number (km) | optional | 1.0 | The farthest beacon heard. |

The column `band` names a row.

### Table `propagation`

Struct `HfbeaconsPropagationRow`. The propagation of a band. The row key is the place, "00" first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `band` | text | always | 1.0 | The band. |
| `state` | choice | always | 1.0 | The state of the band. |
| `stations` | whole number | always | 1.0 | Stations heard. |
| `best_snr_db` | number (dB) | optional | 1.0 | The best SNR. |
| `farthest_km` | number (km) | optional | 1.0 | The farthest station heard. |
| `score` | number | always | 1.0 | The score of the band. |

The column `band` names a row.

### Event `hfb.detection`

Struct `HfbDetectionEvent`. A beacon heard with its identity confirmed: `source` is the beacon's name, `text` the note.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The beacon's name. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `confidence` | whole number | always | 1.0 | The confidence of the identity. |
| `text` | text | always | 1.0 | The note. |
| `position` | position | optional | 1.0 | The position of the beacon, when known. |
| `family` | choice | always | 1.0 | What the event is about. |

### Event `hfb.tx`

Struct `HfbTxEvent`. The fields of a change of the own beacon.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.2 | The state: started (the first sending), sending (a later one), sent (a sending ended), finished (the beacon ended) or refused. |
| `reason` | text | always | 1.2 | Why it changed: empty, stopped, the transmitter is off, the format changed, or why it was refused. |
| `text` | text | always | 1.2 | The text keyed. |
| `family` | choice | always | 1.2 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_hfbeacons_clear` | none | none |
| `list.restore` | `sdrkit_hfbeacons_list_restore` | none | none |
| `start` | `sdrkit_hfbeacons_start` | none | none |
| `stop` | `sdrkit_hfbeacons_stop` | none | none |
| `send` | `sdrkit_hfbeacons_send` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `ppm` | `sdrkit_hfbeacons_get_ppm` | `sdrkit_hfbeacons_set_ppm` |
| `hop` | `sdrkit_hfbeacons_get_hop` | `sdrkit_hfbeacons_set_hop` |
| `dwell_s` | `sdrkit_hfbeacons_get_dwell_s` | `sdrkit_hfbeacons_set_dwell_s` |
| `watch_ncdxf` | `sdrkit_hfbeacons_get_watch_ncdxf` | `sdrkit_hfbeacons_set_watch_ncdxf` |
| `watch_dashes` | `sdrkit_hfbeacons_get_watch_dashes` | `sdrkit_hfbeacons_set_watch_dashes` |
| `watch_letters` | `sdrkit_hfbeacons_get_watch_letters` | `sdrkit_hfbeacons_set_watch_letters` |
| `watch_carriers` | `sdrkit_hfbeacons_get_watch_carriers` | `sdrkit_hfbeacons_set_watch_carriers` |
| `list_json` | `sdrkit_hfbeacons_get_list_json` | `sdrkit_hfbeacons_set_list_json` |
| `clock_offset_s` | `sdrkit_hfbeacons_get_clock_offset_s` | `sdrkit_hfbeacons_set_clock_offset_s` |
| `start_utc_s` | `sdrkit_hfbeacons_get_start_utc_s` | `sdrkit_hfbeacons_set_start_utc_s` |
| `listen` | `sdrkit_hfbeacons_get_listen` | `sdrkit_hfbeacons_set_listen` |
| `listen_khz` | `sdrkit_hfbeacons_get_listen_khz` | `sdrkit_hfbeacons_set_listen_khz` |
| `call` | `sdrkit_hfbeacons_get_call` | `sdrkit_hfbeacons_set_call` |
| `message` | `sdrkit_hfbeacons_get_message` | `sdrkit_hfbeacons_set_message` |
| `wpm` | `sdrkit_hfbeacons_get_wpm` | `sdrkit_hfbeacons_set_wpm` |
| `pitch_hz` | `sdrkit_hfbeacons_get_pitch_hz` | `sdrkit_hfbeacons_set_pitch_hz` |
| `long_dash_s` | `sdrkit_hfbeacons_get_long_dash_s` | `sdrkit_hfbeacons_set_long_dash_s` |
| `interval_min` | `sdrkit_hfbeacons_get_interval_min` | `sdrkit_hfbeacons_set_interval_min` |
| `power_steps` | `sdrkit_hfbeacons_get_power_steps` | `sdrkit_hfbeacons_set_power_steps` |
| `offset_s` | `sdrkit_hfbeacons_get_offset_s` | `sdrkit_hfbeacons_set_offset_s` |
| `count` | `sdrkit_hfbeacons_get_count` | `sdrkit_hfbeacons_set_count` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_signal` | `sdrkit_hfbeacons_gen_get_signal` | `sdrkit_hfbeacons_gen_set_signal` |
| `gen_centre_hz` | `sdrkit_hfbeacons_gen_get_centre_hz` | `sdrkit_hfbeacons_gen_set_centre_hz` |

## Try it

```sh
sdrkit run hfbeacons --source synth:hfbeacons
sdrkit run hfbeacons --source <driver>:<address> --freq 14.5
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit: the own beacon

The stack can send an own beacon: a message in Morse code, a long dash and the power steps, sent at a set second of each interval. The beacon is a proposal of the specification, not taken from a standard or from the beacon project. The engine's transmit switch must be on and the call sign (`call`, 1 to 12 characters of A-Z, 0-9 and /) set; an empty call sign refuses `start` and `send` with an error that names `call`.

- `start`: sends at `offset_s` seconds (0 to 3599, default 0, taken modulo the interval) after the next interval boundary of UTC, at once when the start falls on that second, then again `interval_min` minutes (1 to 60, default 10) after the start of each sending, until `stop` or `count` sendings (0 to 1000; default 0, no limit).
- `send`: sends once, at once, outside the schedule.
- `stop`: ends the sending on air within 50 ms, with its fall, and ends the schedule.

Settings: `message` (default `VVV DE {call}`; `{call}` stands for the call sign, a message without it is followed by DE and the call sign, an empty message sends the call sign alone; A-Z, 0-9, space and / ? . , =), `wpm` (5 to 40, default 22), `pitch_hz` (300 to 2500 Hz, default 800), `long_dash_s` (0 to 10 s, default 3), `power_steps` (default off: four dashes of 1 s at 0, -10, -20 and -30 dB after the long dash, with no gap, as in the slot of the international beacon project). A change applies to the next sending.

The signal is a tone at the pitch above the dial (upper sideband), keyed on and off with raised-cosine edges of 5 ms, at 0.5 of full scale (-6 dBFS) key down; a power step moves the level over 5 ms. The timing is that of ITU-R M.1677-1: a dot is 1.2 / wpm s, a dash 3 dots, the gaps 1, 3 and 7 dots; the long dash and the steps follow the message after a word gap. Example: a 3 s long dash and the steps are 7 s of carrier. Each sending is one transmission with its end mark; on a live radio whose host gives a transmit lead, its first sample states its time on air. Between sendings the transmitter is free. The interval is counted in the session's samples from the first sending; the clock of the session places the first one. A change of format or frequency (a step of `hop` too) and the transmit switch going off end the beacon.

The result fields `tx_beacon_on`, `tx_sending`, `tx_progress_pct`, `tx_next_s`, `tx_sendings`, `tx_sending_s` and `tx_duty_pct` (the share of the interval on air) show the schedule and the sending; the event `hfb.tx` reports started, sending, sent, finished and refused.

The default of `count` is no limit, not one sending as the specification proposes, so that `start` keeps sending until `stop`. The receiver of the stack does not name an own Morse beacon by its keying. Unattended beacons need a licence in many countries; beacon project channels (14.100, 18.110, 21.150, 24.930 and 28.200 MHz) are not for own beacons.
