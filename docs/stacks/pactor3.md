<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# PACTOR-III (physical layer)

PACTOR-III physical layer: the carriers and speed level, the symbol clock and the frame structure (no text).

Stack id: `pactor3`. Group: HF utility.

Signal types: PACTOR-III.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 14.072 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:pactor3` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `analysis_interval_s` | Analysis interval | duration (s) | 4 s | 0.5 to 60 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_level` | Speed level | whole number | 3 | 1 to 6 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `signal` | Signal | yes or no |
| `variant` | Variant | whole number |
| `other_variant` | Another variant | yes or no |
| `symbol_rate_hz` | Symbol rate | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `centre` | Centre | frequency |
| `offset_hz` | Offset | frequency |
| `spacing` | Spacing | frequency |
| `bandwidth` | Occupies about | frequency |
| `clock_line_db` | Clock line | number (dB) |
| `carrier_count` | Carriers | whole number |
| `speed_level` | Speed level | whole number |
| `constellation` | Constellation | whole number |
| `symbol_count` | Symbols | whole number |
| `stretch_start` | Stretch start | duration (s) |
| `stretch_length` | Stretch length | duration (s) |
| `frame_found` | Frame found | yes or no |
| `frame_symbols` | Frame repeats (symbols) | whole number |
| `frame_period` | Frame repeats | duration (s) |
| `frame_copies` | Copies | whole number |
| `frame_agreement_pct` | Agreement of copies | number (%) |
| `constant_start` | Constant part from symbol | whole number |
| `constant_symbols` | Constant part (symbols) | whole number |
| `constant_hex` | Constant part | text |
| `note` | Note | text |
| `symbols_1` | Symbols, carrier 1 | text |
| `symbols_2` | Symbols, carrier 2 | text |
| `symbols_3` | Symbols, carrier 3 | text |
| `analyses` | Analyses | whole number |
| `analyses_with_signal` | Analyses with a signal | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `clock` | Clock | choice |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `carriers` | Carriers | `frequency` (frequency), `line_db` (number (dB)), `steps` (whole number), `modulation` (choice), `concentration_pct` (number (%)) |
| `events` | Events | `start` (duration (s)), `time` (time), `text` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `level` | Level | s | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `pactorhf.analysis` | PACTOR signal | `variant` (whole number), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `hfdata`, version 1.4. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `PactorhfFields`. The result fields of the PACTOR-II, -III and -IV stacks: the measures of the last analysis.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal` | yes or no | always | 1.0 | A signal was found by the last analysis. |
| `variant` | whole number | always | 1.0 | The variant found: 2, 3 or 4 (0 for nothing). |
| `other_variant` | yes or no | always | 1.0 | The signal on the channel is not the variant that the stack receives. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate. Absent without a signal. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The clock error. Absent without a signal. |
| `centre` | frequency | optional | 1.0 | The middle of the outer carriers. |
| `offset_hz` | frequency | optional | 1.0 | The centre against its nominal place. |
| `spacing` | frequency | optional | 1.0 | The carrier spacing (0 for one carrier). |
| `bandwidth` | frequency | optional | 1.0 | The occupied bandwidth. |
| `clock_line_db` | number (dB) | optional | 1.0 | The clock line of the symbol clock. |
| `carrier_count` | whole number | always | 1.0 | The number of carriers. |
| `speed_level` | whole number | always | 1.0 | The PACTOR-III speed level (0 when unknown or another variant). |
| `constellation` | whole number | always | 1.0 | The PACTOR-IV constellation estimate: 2, 4, 8, 16 (QAM), 0 unknown. |
| `symbol_count` | whole number | always | 1.0 | The number of symbols demodulated on each carrier. |
| `stretch_start` | duration (s) | optional | 1.0 | The start of the demodulated stretch, into the analysed audio. |
| `stretch_length` | duration (s) | optional | 1.0 | The length of the demodulated stretch. |
| `frame_found` | yes or no | always | 1.0 | A repeating frame structure was found. |
| `frame_symbols` | whole number | optional | 1.0 | The frame period in symbols. |
| `frame_period` | duration (s) | optional | 1.0 | The frame period. |
| `frame_copies` | whole number | optional | 1.0 | The number of frame copies in the buffer. |
| `frame_agreement_pct` | number (%) | optional | 1.0 | The mean agreement of neighbouring copies. |
| `constant_start` | whole number | optional | 1.0 | The first symbol of the constant part. |
| `constant_symbols` | whole number | optional | 1.0 | The length of the constant part, in symbols. |
| `constant_hex` | text | always | 1.0 | The constant part as hexadecimal, 2 bits per symbol. |
| `note` | text | always | 1.0 | The analysis note (PACTOR-IV). |
| `symbols_1` | text | always | 1.0 | The newest phase steps of the first carrier, one digit per symbol. |
| `symbols_2` | text | always | 1.0 | The newest phase steps of the second carrier. |
| `symbols_3` | text | always | 1.0 | The newest phase steps of the third carrier. |
| `analyses` | whole number | always | 1.0 | The analyses run. |
| `analyses_with_signal` | whole number | always | 1.0 | The analyses that found a signal. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The audio level of the last second. |
| `clock` | choice | always | 1.0 | Where the times come from. |

### Table `carriers`

Struct `PactorhfCarrierRow`. A carrier of the last analysis. The row key is the carrier's number, "00" first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frequency` | frequency | always | 1.0 | The carrier frequency. |
| `line_db` | number (dB) | always | 1.0 | The clock line of the carrier. |
| `steps` | whole number | always | 1.0 | Phase steps per symbol. |
| `modulation` | choice | optional | 1.0 | The modulation, when the phase steps are 2, 4, 8 or 16. |
| `concentration_pct` | number (%) | always | 1.0 | The concentration of the phase steps, 0 to 100 %. |

### Table `events`

Struct `PactorhfEventRow`. An event of the PACTOR-II, -III and -IV stacks. The row key is the sequence number.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start` | duration (s) | always | 1.0 | The signal time. |
| `time` | time | optional | 1.0 | The UTC, when the clock is known. |
| `text` | text | always | 1.0 | What happened. |

### Event `pactorhf.analysis`

Struct `PactorhfAnalysisEvent`. A new variant or carrier count: `text` describes the signal.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `variant` | whole number | always | 1.0 | The variant found. |
| `text` | text | always | 1.0 | The signal in words. |
| `family` | choice | always | 1.0 | What the event is about. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_pactor3_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `analysis_interval_s` | `sdrkit_pactor3_get_analysis_interval_s` | `sdrkit_pactor3_set_analysis_interval_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_level` | `sdrkit_pactor3_gen_get_level` | `sdrkit_pactor3_gen_set_level` |

## Try it

```sh
sdrkit run pactor3 --source synth:pactor3
sdrkit run pactor3 --source <driver>:<address> --freq 14.072
```

## Test recordings

- `tests/vectors/hfdata/pactor3_24k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
