<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Time signals

The time and date from DCF77, MSF, WWVB, JJY, BPC, WWV/WWVH, BPM, HLA and RWM, compared with your clock.

Stack id: `time`. Group: Science, time and utility.

Signal types: Time signal.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 77.5 kHz |
| Tuning range | 30 kHz to 30 MHz |
| Sample rate | 256 kHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 10 kHz |
| Offset from the signal | 12 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:time` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `station` | Station | choice | `auto` | `auto` (Automatic (from the dial)), `dcf77` (DCF77), `msf` (MSF), `wwvb` (WWVB), `jjy40` (JJY 40 kHz), `jjy60` (JJY 60 kHz), `bpc` (BPC), `wwv` (WWV / WWVH), `chu` (CHU), `bpm` (BPM), `bpmc` (BPM coded), `hla` (HLA), `rwm` (RWM) |  |
| `sixty_khz` | 60 kHz is | choice | `msf` | `msf` (MSF), `wwvb` (WWVB), `jjy60` (JJY 60 kHz) |  |
| `audio_input` | Demodulated audio recording | yes or no | false |  |  |
| `audio_tone_hz` | Carrier tone of an LF recording | frequency | 1 kHz | 100 Hz to 20 kHz |  |
| `clock_slow_pct` | Recording clock slow by | number (%) | 0 | 0 to 10 |  |
| `listen` | Listen | yes or no | false |  |  |
| `clock_mode` | Clock to compare | choice | `live` | `none` (None (a file)), `live` (Host clock (a radio)), `fixed` (Fixed origin (tests)) |  |
| `clock_origin` | Sample 0 at (UTC, Unix s) | number (s) | 0 | 0 to 100000000000 |  |
| `year_hint` | Year hint | whole number | 0 | 0 to 3000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_start_utc` | Test signal: start (UTC, 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `station` | Station | choice |
| `wwvh` | WWVH | yes or no |
| `carrier_dbfs` | Carrier | number (dBFS) |
| `snr_db` | SNR | number (dB) |
| `depth_pct` | Keying depth | number (%) |
| `tick_ms` | Second edge | number (ms) |
| `jitter_ms` | Edge jitter | number (ms) |
| `minutes_ok` | Minutes decoded | whole number |
| `minutes_bad` | Minutes refused | whole number |
| `confirmed` | Confirmed | yes or no |
| `distance_m` | Distance | number (m) |
| `delay_ms` | Path delay | number (ms) |
| `path` | Path | choice |
| `symbols` | Symbols | text |
| `utc` | Received time | time |
| `received` | Received | text |
| `dut1_s` | DUT1 | number (s) |
| `summer` | Summer time | yes or no |
| `weekday` | Weekday | choice |
| `clock_offset_s` | Clock offset | number (s) |
| `clock_offset_avg_s` | Clock offset (1 min) | number (s) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `minutes` | Minutes | `time_s` (number (s)), `host_time` (time), `ok` (yes or no), `text` (text), `refusal` (choice), `second` (whole number), `symbols` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `level` | Carrier level | s | dBFS |
| `snr` | SNR | s | dB |
| `tick` | Second edge | s | ms |
| `offset` | Clock offset | s | s |

## Events

| Kind | Label | Fields |
|---|---|---|
| `time.minute` | Time signal minute | `text` (text), `family` (choice), `utc` (time), `station` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `science`, version 2.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Science and propagation (`science`). Media: sound (monitor).

Key readings: `utc`, `station`, `confirmed`, `clock_offset_s`, `snr_db`.

### Result fields

Struct `TimeFields`. The result fields of `time`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `station` | choice | always | 1.0 | The station decoded. |
| `wwvh` | yes or no | optional | 1.0 | WWVH (1200 Hz ticks) rather than WWV (1000 Hz ticks). |
| `carrier_dbfs` | number (dBFS) | always | 1.0 | The carrier level. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `depth_pct` | number (%) | always | 1.0 | The keying depth (LF) or the tick level over the background (HF). |
| `tick_ms` | number (ms) | always | 1.0 | Where the second's edge falls, after a whole second of signal time. |
| `jitter_ms` | number (ms) | always | 1.0 | The jitter of the edges. |
| `minutes_ok` | whole number | always | 1.0 | Minutes decoded. |
| `minutes_bad` | whole number | always | 1.0 | Minutes refused. |
| `confirmed` | yes or no | always | 1.0 | Two minutes in a row agree. |
| `distance_m` | number (m) | optional | 1.0 | The distance from the station. |
| `delay_ms` | number (ms) | optional | 1.0 | The path delay. |
| `path` | choice | optional | 1.0 | How the path delay was found. |
| `symbols` | text | always | 1.0 | The symbols of this minute so far. |
| `utc` | time | optional | 1.0 | The received UTC now. |
| `received` | text | optional | 1.0 | The last minute as the station sends it. |
| `dut1_s` | number (s) | optional | 1.0 | DUT1 (UT1 − UTC). |
| `summer` | yes or no | optional | 1.0 | Summer time is in force. |
| `weekday` | choice | optional | 1.0 | The day of the week sent, by name. |
| `clock_offset_s` | number (s) | optional | 1.0 | The computer clock minus the received time, now. |
| `clock_offset_avg_s` | number (s) | optional | 1.0 | The same, averaged over the last minute. |

### Table `minutes`

Struct `TimeMinuteRow`. A minute of `time`. The row key is the sequence number of the minute.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of a minute. |
| `host_time` | time | optional | 1.0 | The host time of a minute. |
| `ok` | yes or no | always | 1.0 | The decoded flag of a minute. |
| `text` | text | optional | 1.0 | The text of a minute. |
| `refusal` | choice | optional | 1.0 | Why a minute was refused. |
| `second` | whole number | optional | 1.0 | The second where the decoder stopped. |
| `symbols` | text | always | 1.0 | The symbols of this minute so far. |

The rows are a log: they arrive over time. Time: `host_time`.

### Event `time.minute`

Struct `TimeMinuteEvent`. A minute decoded: `text` holds the time as the station sends it, `utc` the start of the minute.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a decoded message. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `utc` | time | always | 1.0 | The received UTC now. |
| `station` | choice | always | 1.0 | The station decoded. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_time_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `station` | `sdrkit_time_get_station` | `sdrkit_time_set_station` |
| `sixty_khz` | `sdrkit_time_get_sixty_khz` | `sdrkit_time_set_sixty_khz` |
| `audio_input` | `sdrkit_time_get_audio_input` | `sdrkit_time_set_audio_input` |
| `audio_tone_hz` | `sdrkit_time_get_audio_tone_hz` | `sdrkit_time_set_audio_tone_hz` |
| `clock_slow_pct` | `sdrkit_time_get_clock_slow_pct` | `sdrkit_time_set_clock_slow_pct` |
| `listen` | `sdrkit_time_get_listen` | `sdrkit_time_set_listen` |
| `clock_mode` | `sdrkit_time_get_clock_mode` | `sdrkit_time_set_clock_mode` |
| `clock_origin` | `sdrkit_time_get_clock_origin` | `sdrkit_time_set_clock_origin` |
| `year_hint` | `sdrkit_time_get_year_hint` | `sdrkit_time_set_year_hint` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_start_utc` | `sdrkit_time_gen_get_start_utc` | `sdrkit_time_gen_set_start_utc` |

## Try it

```sh
sdrkit run time --source synth:time
sdrkit run time --source <driver>:<address> --freq 0.0775
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
