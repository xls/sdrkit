<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# UVB-76 buzzer monitor

Times the buzz train of the UVB-76 AM station: buzz rate, length, period and spread; train breaks and long transmissions. A monitor: no voice is decoded.

Stack id: `uvb76`. Group: HF utility.

The UVB-76 buzzer monitor: it times the buzz train of the AM station (about 25 buzzes a minute, 1.2 s each), finds a break of the train and a long transmission (a voice message is possible). It detects and measures only: no voice is decoded and no message is interpreted. pause and the rate of 20 to 34 buzzes a minute are those of the Wikipedia article "UVB-76" and the Buzzer Primer of priyom.org; the stack measures what it hears.

Standard: no issuing body publishes the signal. The frequency, the buzz length (1.2 s, 0.8 s before November 2010), the

## Tuning

| Property | Value |
|---|---|
| Default frequency | 6.5 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:uvb76` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `min_buzz_s` | Shortest buzz | duration (s) | 0.15 s | 0.05 to 1 | A shorter rise of the modulation is a click and is ignored. |
| `long_tx_s` | Longest buzz | duration (s) | 4 s | 1 to 20 | Longer modulation is a long transmission (a voice message is possible). |
| `carrier_share` | Carrier line share | number | 0.012 | 0.002 to 0.1 | The share of the channel power in the strongest five bins that shows a carrier; without a carrier nothing is timed. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_period_s` | Buzz period | duration (s) | 2.4 s | 0.5 to 15 |  |
| `gen_buzz_s` | Buzz length | duration (s) | 1.2 s | 0.1 to 10 |  |
| `gen_depth` | Modulation depth | number | 0.6 | 0 to 1 |  |
| `gen_break_at_s` | Pause at | duration (s) | 0 s | 0 to 3600 | 0: no pause. |
| `gen_break_s` | Pause | duration (s) | 0 s | 0 to 600 |  |
| `gen_voice_at_s` | Voice at | duration (s) | 0 s | 0 to 3600 | 0: no long transmission. |
| `gen_voice_s` | Voice length | duration (s) | 0 s | 0 to 600 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `note` | Note | text |
| `state` | State | whole number |
| `carrier` | Carrier | yes or no |
| `buzzes` | Buzzes | whole number |
| `rate_per_min` | Buzz rate | number (/min) |
| `buzz_s` | Buzz length | duration (s) |
| `period_s` | Period | duration (s) |
| `spread_ms` | Period spread | number (ms) |
| `breaks` | Breaks | whole number |
| `long_transmissions` | Long transmissions | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `rows` | Rows | `time` (time), `kind` (text), `id` (text), `text` (text), `snr` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `uvb76.event` | Event | `text` (text), `kind` (text), `value` (text) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Searches the signal again; keeps the text and the tables. |

## Typed interface

The stack is in the family `hfdata`, version 1.2. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `Uvb76Fields`. The result fields of the UVB-76 buzzer monitor. The measures of the train are absent while no train is locked.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `note` | text | always | 1.0 | What the stack does not do and what no public document confirms. |
| `state` | whole number | always | 1.0 | The state: 0 no carrier, 1 carrier, 2 buzz train locked. |
| `carrier` | yes or no | always | 1.0 | A carrier is in the channel. |
| `buzzes` | whole number | always | 1.0 | Buzzes heard. |
| `rate_per_min` | number (/min) | optional | 1.0 | The buzz rate, 60 over the median period. |
| `buzz_s` | duration (s) | optional | 1.0 | The median buzz length. |
| `period_s` | duration (s) | optional | 1.0 | The median period. |
| `spread_ms` | number (ms) | optional | 1.0 | The median distance of the periods from their median. |
| `breaks` | whole number | always | 1.0 | Train breaks logged. |
| `long_transmissions` | whole number | always | 1.0 | Long transmissions logged. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the channel, dBFS. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `rows`

Struct `SenderRow`. A row of a number-sender stack: one thing read. The row key is the count of rows before it, as six digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | time | always | 1.0 | The time it was read. |
| `kind` | text | always | 1.0 | What it is: start, message start, groups, block sync, break, end, sync, text. |
| `id` | text | always | 1.0 | The signal and speed it was read as. |
| `text` | text | always | 1.0 | What was read. |
| `snr` | number (dB) | optional | 1.0 | The SNR in 3 kHz of the last fit of the tones. Absent without a fit. |

### Event `uvb76.event`

Struct `Uvb76Event`. A buzz train, a break or a long transmission.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The text of the row. |
| `kind` | text | always | 1.0 | Buzz train, Train break or Long transmission. |
| `value` | text | always | 1.0 | The value: the rate, the time since the last buzz or the length. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_uvb76_clear` | none | none |
| `reset` | `sdrkit_uvb76_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `min_buzz_s` | `sdrkit_uvb76_get_min_buzz_s` | `sdrkit_uvb76_set_min_buzz_s` |
| `long_tx_s` | `sdrkit_uvb76_get_long_tx_s` | `sdrkit_uvb76_set_long_tx_s` |
| `carrier_share` | `sdrkit_uvb76_get_carrier_share` | `sdrkit_uvb76_set_carrier_share` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_period_s` | `sdrkit_uvb76_gen_get_period_s` | `sdrkit_uvb76_gen_set_period_s` |
| `gen_buzz_s` | `sdrkit_uvb76_gen_get_buzz_s` | `sdrkit_uvb76_gen_set_buzz_s` |
| `gen_depth` | `sdrkit_uvb76_gen_get_depth` | `sdrkit_uvb76_gen_set_depth` |
| `gen_break_at_s` | `sdrkit_uvb76_gen_get_break_at_s` | `sdrkit_uvb76_gen_set_break_at_s` |
| `gen_break_s` | `sdrkit_uvb76_gen_get_break_s` | `sdrkit_uvb76_gen_set_break_s` |
| `gen_voice_at_s` | `sdrkit_uvb76_gen_get_voice_at_s` | `sdrkit_uvb76_gen_set_voice_at_s` |
| `gen_voice_s` | `sdrkit_uvb76_gen_get_voice_s` | `sdrkit_uvb76_gen_set_voice_s` |

## Try it

```sh
sdrkit run uvb76 --source synth:uvb76
sdrkit run uvb76 --source <driver>:<address> --freq 6.5
```
<!-- generated: end -->
