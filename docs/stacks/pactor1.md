<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# PACTOR-I (experimental)

PACTOR-I monitor: both stations of an ARQ link and unproto broadcasts, with memory ARQ, text and callsigns (experimental).

Stack id: `pactor1`. Group: HF utility.

PACTOR-I, experimental, receive only. ```text iq -> [hf_front 24 kHz] -> [sideband audio 8 kHz] --+--> [tone pair finder] -> pactor.pair --+ +--> [burst detector (audio history)] <--+-> pactor.burst -> [burst reader] -> pactor.frame -> [link] -> results +--> [listen] -> audio ```

Signal types: PACTOR-I.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 14.08 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:pactor1` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `listen` | Listen | yes or no | false |  | Play the sideband (dial + 200 Hz to 3.8 kHz). Decoding is the same either way. |
| `demod` | Demodulator | choice | `auto` | `auto` (Auto), `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) | USB below 30 MHz, NFM above (Auto). The decoders see the same audio whether or not you listen. |
| `huffman_table` | Huffman table | text | empty |  | The full table of the public ARRL PACTOR page (Annex, section 9): one entry a line, the character or its decimal code, a tab, the code bits. Empty: the built-in partial table. Unknown codes show as ~. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_script` | Script | choice | `arq` | `arq` (ARQ), `fec` (Unproto (FEC)) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `signal` | Signal | yes or no |
| `packets_recent` | Packets | yes or no |
| `packets_ok` | Packets good | whole number |
| `packets_fast` | At 200 baud | whole number |
| `packets_slow` | At 100 baud | whole number |
| `packets_bad` | Packets failed | whole number |
| `control_signals` | Control signals | whole number |
| `repeats` | Repeats | whole number |
| `memory_arq_saves` | Memory ARQ saves | whole number |
| `changeovers` | Changeovers | whole number |
| `connects` | Connect packets | whole number |
| `fec_mode` | Unproto / FEC | yes or no |
| `huffman_complete` | Huffman table complete | yes or no |
| `huffman_entries` | Huffman entries | whole number |
| `pair_low` | Lower tone | frequency |
| `pair_high` | Upper tone | frequency |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `text` | Text | text |
| `clock` | Clock | choice |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `start` (duration (s)), `time` (time), `kind` (choice), `symbol_rate_hz` (frequency), `header` (whole number), `number` (whole number), `huffman` (yes or no), `break_in` (yes or no), `qrt` (yes or no), `crc_ok` (yes or no), `memory_arq` (yes or no), `repeat` (yes or no), `fec_repeat` (yes or no), `inverted` (yes or no), `side` (whole number), `text` (text), `data_hex` (text), `call` (text), `snr_db` (number (dB)), `pair_low` (frequency) |
| `stations` | Stations | `call` (text), `seen_as` (choice), `heard` (whole number), `last_heard` (time) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `level` | Level | s | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `pactor.frame` | PACTOR-I frame | `kind` (choice), `symbol_rate_hz` (frequency), `header` (whole number), `number` (whole number), `source` (text), `target` (text), `text` (text), `side` (whole number), `inverted` (yes or no), `repeat` (yes or no), `snr_db` (number (dB)), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `hfdata`, version 1.6. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: sound (monitor).

### Result fields

Struct `Pactor1Fields`. The result fields of PACTOR-I.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `signal` | yes or no | always | 1.0 | A tone pair is locked or a frame was read in the last 30 s (the lock state "Signal"). |
| `packets_recent` | yes or no | always | 1.0 | A frame was read in the last 30 s (the lock state "Packets"). |
| `packets_ok` | whole number | always | 1.0 | Good packets. |
| `packets_fast` | whole number | always | 1.0 | Good packets at 200 baud. |
| `packets_slow` | whole number | always | 1.0 | Good packets at 100 baud. |
| `packets_bad` | whole number | always | 1.0 | Packets whose CRC failed. |
| `control_signals` | whole number | always | 1.0 | Control signals. |
| `repeats` | whole number | always | 1.0 | Repeated packets. |
| `memory_arq_saves` | whole number | always | 1.0 | Packets read by memory ARQ. |
| `changeovers` | whole number | always | 1.0 | Changeovers (CS3). |
| `connects` | whole number | always | 1.0 | Connect packets. |
| `fec_mode` | yes or no | always | 1.0 | The link is in unproto (FEC) mode, else ARQ. |
| `huffman_complete` | yes or no | always | 1.0 | The Huffman table holds all 95 printable characters. |
| `huffman_entries` | whole number | always | 1.0 | Entries in the Huffman table. |
| `pair_low` | frequency | optional | 1.0 | The locked lower tone. Absent without a lock. |
| `pair_high` | frequency | optional | 1.0 | The locked upper tone. Absent without a lock. |
| `offset_hz` | frequency | optional | 1.0 | The centre of the pair against 1500 Hz. Absent without a lock. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the tone pair. Absent without a lock. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The audio level of the last second. |
| `text` | text | always | 1.0 | The text of both sides. |
| `clock` | choice | always | 1.0 | Where the times come from. |

### Table `frames`

Struct `Pactor1FrameRow`. A frame of PACTOR-I. The row key is the frame's number in the log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start` | duration (s) | always | 1.0 | The signal time of the frame's first bit. |
| `time` | time | optional | 1.0 | The UTC of the frame, when the clock is known. |
| `kind` | choice | always | 1.0 | The kind of frame. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate. |
| `header` | whole number | optional | 1.0 | The header byte of a packet, 0x55 or 0xAA. |
| `number` | whole number | optional | 1.0 | The packet number 0 to 3. |
| `huffman` | yes or no | always | 1.0 | Huffman text. |
| `break_in` | yes or no | always | 1.0 | The break-in flag. |
| `qrt` | yes or no | always | 1.0 | The QRT flag. |
| `crc_ok` | yes or no | always | 1.0 | The CRC passed. |
| `memory_arq` | yes or no | always | 1.0 | Read by memory ARQ. |
| `repeat` | yes or no | always | 1.0 | A repeat of the packet before. |
| `fec_repeat` | yes or no | always | 1.0 | A repeat with no control signal since the packet before. |
| `inverted` | yes or no | always | 1.0 | The tones were swapped. |
| `side` | whole number | always | 1.0 | The sending side, 1 or 2. |
| `text` | text | always | 1.0 | The text of the frame. |
| `data_hex` | text | always | 1.0 | The data bytes in hexadecimal. |
| `call` | text | optional | 1.0 | The called station of a connect packet: the row key of its row of the stations table. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the frame. |
| `pair_low` | frequency | always | 1.0 | The lower tone of the frame. |

The column `call` names a row.

### Table `stations`

Struct `Pactor1StationRow`. A station of PACTOR-I. The row key is the callsign.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `call` | text | always | 1.0 | The callsign. |
| `seen_as` | choice | always | 1.0 | How the station was seen. |
| `heard` | whole number | optional | 1.0 | Times heard. |
| `last_heard` | time | optional | 1.0 | When last heard. |

The column `call` names a row.

### Event `pactor.frame`

Struct `PactorFrameEvent`. A frame of PACTOR-I.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `kind` | choice | always | 1.0 | The kind of frame. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate. |
| `header` | whole number | optional | 1.0 | The header byte of a packet. |
| `number` | whole number | optional | 1.0 | The packet number 0 to 3. |
| `source` | text | optional | 1.0 | The sender; not read. |
| `target` | text | optional | 1.0 | The called station of a connect packet. |
| `text` | text | always | 1.0 | The text of the frame. |
| `side` | whole number | always | 1.0 | The sending side, 1 or 2. |
| `inverted` | yes or no | always | 1.0 | The tones were swapped. |
| `repeat` | yes or no | always | 1.0 | A repeat of the packet before. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the frame. |
| `family` | choice | always | 1.0 | What the event is about: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_pactor1_clear` | none | none |
| `remove` | `sdrkit_pactor1_remove` | `sdrkit_pactor1_station_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `listen` | `sdrkit_pactor1_get_listen` | `sdrkit_pactor1_set_listen` |
| `demod` | `sdrkit_pactor1_get_demod` | `sdrkit_pactor1_set_demod` |
| `huffman_table` | `sdrkit_pactor1_get_huffman_table` | `sdrkit_pactor1_set_huffman_table` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_script` | `sdrkit_pactor1_gen_get_script` | `sdrkit_pactor1_gen_set_script` |

## Try it

```sh
sdrkit run pactor1 --source synth:pactor1
sdrkit run pactor1 --source <driver>:<address> --freq 14.08
```

## Test recordings

- `tests/vectors/hfdata/pactor1_arq_24k.sigmf-meta`
- `tests/vectors/hfdata/pactor1_fec_24k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
