<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ERMES paging

ERMES pagers: 4-level FSK at 3125 symbols/s, the network identity, the initial addresses and the numeric and text messages of each batch.

Stack id: `ermes`. Group: Paging.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 170 MHz |
| Tuning range | 25 MHz to 1 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 96 kHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 50 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ermes` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_last_batch` | Test signal last batch | yes or no | false |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `batches` | Batches | whole number |
| `bad_batches` | Bad batches | whole number |
| `addresses` | Addresses | whole number |
| `messages` | Messages | whole number |
| `words_repaired` | Code words repaired | whole number |
| `syncs` | Syncs | whole number |
| `network` | Network | text |
| `carrier` | Carrier | yes or no |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `offset_hz` | Offset | frequency |
| `note` | Note | text |
| `dropped` | Dropped | whole number |
| `modem.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `rows` | Batches and messages | `time_s` (number (s)), `kind` (text), `batch` (text), `address` (whole number), `category` (text), `text` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ermes.batch` | ERMES batch | `batch` (text), `network` (text), `text` (text), `family` (choice) |
| `ermes.message` | ERMES message | `target` (text), `category` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `trunk`, version 1.6. The C header `sdrkit/trunk.h`, the registry record `registry/trunk.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Paging (`paging`). Media: text.

### Result fields

Struct `ErmesFields`. The counts and the carrier of the ERMES receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `batches` | whole number | always | 1.0 | The good batches. |
| `bad_batches` | whole number | always | 1.0 | The batches dropped. |
| `addresses` | whole number | always | 1.0 | The initial addresses. |
| `messages` | whole number | always | 1.0 | The messages. |
| `words_repaired` | whole number | always | 1.0 | The code words repaired. |
| `syncs` | whole number | always | 1.0 | The times that the preamble and the sync word were found. |
| `network` | text | optional | 1.0 | The network of the last good batch. |
| `carrier` | yes or no | optional | 1.0 | A carrier is present. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the carrier. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input. |
| `offset_hz` | frequency | optional | 1.0 | The offset of the carrier from the tuned frequency. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `rows`

Struct `ErmesEntryRow`. A row of the ERMES table: the initial addresses of a batch, or a message. The row key is the number of the row.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the row. |
| `kind` | text | always | 1.0 | What the row is: the addresses of a batch, or a message. |
| `batch` | text | always | 1.0 | The batch letter. |
| `address` | whole number | optional | 1.0 | The local address of a message. |
| `category` | text | optional | 1.0 | The category of a message. |
| `text` | text | always | 1.0 | The addresses, or the text of the message. |

The rows are the messages of the stack. Addressee: `address`. Text: `text`. Time: `time_s`.

### Event `ermes.batch`

Struct `ErmesBatchEvent`. A good ERMES batch.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `batch` | text | always | 1.0 | The batch letter. |
| `network` | text | always | 1.0 | The network. |
| `text` | text | always | 1.0 | The initial addresses, decimal. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |

### Event `ermes.message`

Struct `ErmesMessageEvent`. An ERMES message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `target` | text | optional | 1.0 | The local address. |
| `category` | text | optional | 1.0 | The category. |
| `text` | text | always | 1.0 | The text of the message. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ermes_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_last_batch` | `sdrkit_ermes_gen_get_last_batch` | `sdrkit_ermes_gen_set_last_batch` |

## Try it

```sh
sdrkit run ermes --source synth:ermes
sdrkit run ermes --source iiod:192.168.2.1 --freq 170
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
