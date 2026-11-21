<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# e-Czas (225 kHz, Poland)

The official Polish time from the phase keying of the 225 kHz long-wave carrier.

Stack id: `eczas`. Group: Science, time and utility.

Signal types: Time signal.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 225 kHz |
| Tuning range | 200 kHz to 250 kHz |
| Sample rate | 256 kHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 10 kHz |
| Offset from the signal | 12 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:eczas` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `audio_input` | Audio recording | yes or no | false |  |  |
| `audio_tone_hz` | Carrier tone in the audio | frequency | 1 kHz | 100 Hz to 20 kHz |  |
| `listen` | Listen | yes or no | false |  |  |
| `clock_mode` | Clock to compare | choice | `live` | `none` (None (a file)), `live` (Host clock (a radio)), `fixed` (Fixed origin (tests)) |  |
| `clock_origin` | Sample 0 at (UTC, Unix s) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_start_utc` | Test signal: start (UTC, 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |
| `gen_carrier_only` | Test signal: carrier only | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `utc` | Received time | time |
| `tz_hours` | Local offset | whole number (h) |
| `leap_announce` | Leap second announced | yes or no |
| `leap_subtract` | Leap second taken away | yes or no |
| `dst_change_soon` | Summer time change soon | yes or no |
| `tx_state` | Transmitter | choice |
| `last_packet` | Last packet | time |
| `confirmed` | Confirmed | yes or no |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `sync_quality` | Sync quality | number |
| `syncs` | Syncs | whole number |
| `frames` | Frames | whole number |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `other_slots` | Other slots | whole number |
| `false_syncs` | False syncs | whole number |
| `distance_m` | Distance | number (m) |
| `delay_ms` | Path delay | number (ms) |
| `clock_offset_s` | Clock offset | number (s) |
| `clock_offset_avg_s` | Clock offset (10 packets) | number (s) |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `signal_time_s` (number (s)), `host_time` (time), `kind` (choice), `ok` (yes or no), `text` (text), `bytes` (bytes) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `level` | Carrier level | s | dBFS |
| `snr` | C/N in 100 Hz | s | dB |
| `clock_offsets` | Clock offset | s | s |
| `phase` | Phase |  |  |
| `trace` | Slicer | s |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `eczas.time` | e-Czas time | `text` (text), `family` (choice), `utc` (time), `tz_hours` (whole number (h)), `tx_state` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `science`, version 1.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `EczasFields`. The result fields of `eczas`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The received UTC now. |
| `tz_hours` | whole number (h) | optional | 1.0 | The hours that the packet adds to UTC for the local time in Poland. |
| `leap_announce` | yes or no | optional | 1.0 | A leap second is announced. |
| `leap_subtract` | yes or no | optional | 1.0 | The announced leap second is taken away. |
| `dst_change_soon` | yes or no | optional | 1.0 | A change of summer time is near. |
| `tx_state` | choice | optional | 1.0 | The transmitter state. |
| `last_packet` | time | optional | 1.0 | The UTC of the last time packet. |
| `confirmed` | yes or no | always | 1.0 | Two packets in a row agree. |
| `offset_hz` | frequency | always | 1.0 | Carrier offset from where it was expected, Hz. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `level_dbfs` | number (dBFS) | always | 1.0 | Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). |
| `sync_quality` | number | always | 1.0 | The quality of the last sync. |
| `syncs` | whole number | always | 1.0 | Syncs found. |
| `frames` | whole number | always | 1.0 | Frames cut. |
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `other_slots` | whole number | always | 1.0 | Packets of the other slots. |
| `false_syncs` | whole number | always | 1.0 | Syncs not followed by the sync word. |
| `distance_m` | number (m) | optional | 1.0 | The distance from the transmitter. |
| `delay_ms` | number (ms) | optional | 1.0 | The path delay. |
| `clock_offset_s` | number (s) | optional | 1.0 | The computer clock minus the received time, now. |
| `clock_offset_avg_s` | number (s) | optional | 1.0 | The same, averaged over the last 10 packets. |

### Table `frames`

Struct `EczasFrameRow`. A frame of `eczas`. The row key is the sequence number of the frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal_time_s` | number (s) | always | 1.0 | The signal time of a frame. |
| `host_time` | time | optional | 1.0 | The host time of a frame. |
| `kind` | choice | always | 1.0 | What a frame is. |
| `ok` | yes or no | always | 1.0 | The frame was accepted. |
| `text` | text | always | 1.0 | The text of a frame. |
| `bytes` | bytes | always | 1.0 | The bytes of a frame. |

### Event `eczas.time`

Struct `EczasTimeEvent`. A time packet received: `text` holds the time for people, `utc` the time.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of a frame. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |
| `utc` | time | always | 1.0 | The received UTC now. |
| `tz_hours` | whole number (h) | always | 1.0 | The hours that the packet adds to UTC for the local time in Poland. |
| `tx_state` | choice | always | 1.0 | The transmitter state. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_eczas_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `audio_input` | `sdrkit_eczas_get_audio_input` | `sdrkit_eczas_set_audio_input` |
| `audio_tone_hz` | `sdrkit_eczas_get_audio_tone_hz` | `sdrkit_eczas_set_audio_tone_hz` |
| `listen` | `sdrkit_eczas_get_listen` | `sdrkit_eczas_set_listen` |
| `clock_mode` | `sdrkit_eczas_get_clock_mode` | `sdrkit_eczas_set_clock_mode` |
| `clock_origin` | `sdrkit_eczas_get_clock_origin` | `sdrkit_eczas_set_clock_origin` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_start_utc` | `sdrkit_eczas_gen_get_start_utc` | `sdrkit_eczas_gen_set_start_utc` |
| `gen_carrier_only` | `sdrkit_eczas_gen_get_carrier_only` | `sdrkit_eczas_gen_set_carrier_only` |

## Try it

```sh
sdrkit run eczas --source synth:eczas
sdrkit run eczas --source <driver>:<address> --freq 0.225
```

## Test recordings

- `tests/vectors/science/eczas_225k_48k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
