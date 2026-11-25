<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# EBICab / Eurobalise

Eurobalise up-link telegrams between the rails (SUBSET-036), and the bursts of other balise systems.

Stack id: `ebicab`. Group: Cellular and railway.

Signal types: Balise.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 4.234 MHz |
| Tuning range | 1 MHz to 30 MHz |
| Sample rate | 10 MHz |
| Smallest sample rate | 4 MHz |
| Signal bandwidth | 1.8 MHz |
| Offset from the signal | 1 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ebicab` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_scene` | Test signal scene | choice | `2` | `0` (Long telegrams), `1` (Short telegram), `2` (All three in turn), `3` (Random bits) |  |
| `gen_burst_ms` | Test signal burst | number (ms) | 6 | 0.5 to 100 |  |
| `gen_gap_ms` | Test signal gap | number (ms) | 20 | 1 to 1000 |  |
| `gen_random_bits` | Test signal random bits per burst | whole number | 400 | 24 to 4000 |  |
| `gen_bit_rate_pct` | Test signal bit rate error | number (%) | 0 | -2.5 to 2.5 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `bursts` | Bursts | whole number |
| `telegrams` | Telegrams | whole number |
| `long` | Long telegrams | whole number |
| `short` | Short telegrams | whole number |
| `inverted` | Inverted telegrams | whole number |
| `bursts_without_telegram` | Bursts without a telegram | whole number |
| `parity_hits` | Parity hits | whole number |
| `bit_rate_hz` | Bit rate | frequency |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `dropped` | Dropped | whole number |
| `uplink.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `telegrams` | Telegrams | `received` (time), `length_bits` (whole number (bits)), `nid_c` (whole number), `nid_bg` (whole number), `n_pig` (whole number), `n_total` (whole number), `m_version` (whole number), `copies` (whole number), `user_bits` (bytes) |
| `balises` | Balises | `nid_c` (whole number), `nid_bg` (whole number), `balise_in_group` (whole number), `group_size` (whole number), `last_heard` (time), `heard` (whole number) |
| `other_bursts` | Other bursts | `received` (time), `bits` (whole number), `duration` (duration (s)), `snr_db` (number (dB)), `first_bits` (bytes), `hint` (choice) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `balise.telegram` | Balise telegram | `source` (text), `text` (text), `family` (choice), `length_bits` (whole number (bits)), `nid_c` (whole number), `nid_bg` (whole number), `n_pig` (whole number) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `cell`, version 1.4. The C header `sdrkit/cell.h`, the registry record `registry/cell.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: none.

### Result fields

Struct `EbicabFields`. The result fields of `ebicab`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bursts` | whole number | always | 1.0 | Bursts. |
| `telegrams` | whole number | always | 1.0 | Telegrams decoded. |
| `long` | whole number | always | 1.0 | Long telegrams. |
| `short` | whole number | always | 1.0 | Short telegrams. |
| `inverted` | whole number | always | 1.0 | Telegrams that came inverted. |
| `bursts_without_telegram` | whole number | always | 1.0 | Bursts without a telegram. |
| `parity_hits` | whole number | always | 1.0 | Parity hits. |
| `bit_rate_hz` | frequency | optional | 1.0 | The bit rate of the newest burst. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset of the newest burst. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR of the newest burst. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `telegrams`

Struct `EbicabTelegramRow`. A telegram of `ebicab`. The row key is the place of the telegram in the order received.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `received` | time | optional | 1.0 | When the telegram came (wall clock), when known. |
| `length_bits` | whole number (bits) | always | 1.0 | The length. |
| `nid_c` | whole number | always | 1.0 | NID_C. |
| `nid_bg` | whole number | always | 1.0 | NID_BG. |
| `n_pig` | whole number | always | 1.0 | N_PIG. |
| `n_total` | whole number | always | 1.0 | N_TOTAL. |
| `m_version` | whole number | always | 1.0 | M_VERSION. |
| `copies` | whole number | always | 1.0 | The copies received. |
| `user_bits` | bytes | always | 1.0 | The user bits. |

The rows are a log: they arrive over time. Time: `received`.

### Table `balises`

Struct `EbicabBaliseRow`. A balise of `ebicab` heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `nid_c` | whole number | always | 1.0 | NID_C. |
| `nid_bg` | whole number | always | 1.0 | NID_BG. |
| `balise_in_group` | whole number | always | 1.0 | The place of the balise in its group. |
| `group_size` | whole number | always | 1.0 | The balises of the group. |
| `last_heard` | time | optional | 1.0 | When the balise was last heard. |
| `heard` | whole number | optional | 1.0 | Times heard. |

### Table `other_bursts`

Struct `EbicabOtherBurstRow`. A burst of `ebicab` without a telegram. The row key is the place of the burst in the order received.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `received` | time | optional | 1.0 | When the burst came (wall clock), when known. |
| `bits` | whole number | always | 1.0 | The bits of the burst. |
| `duration` | duration (s) | always | 1.0 | The length of the burst. |
| `snr_db` | number (dB) | always | 1.0 | The SNR. |
| `first_bits` | bytes | always | 1.0 | The first bits. |
| `hint` | choice | optional | 1.0 | What the length of the burst suggests. |

The rows are a log: they arrive over time. Time: `received`.

### Event `balise.telegram`

Struct `EbicabTelegramEvent`. A new telegram of `ebicab`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The balise. |
| `text` | text | always | 1.0 | The telegram for people. |
| `family` | choice | always | 1.0 | The family of the source. |
| `length_bits` | whole number (bits) | always | 1.0 | The length. |
| `nid_c` | whole number | always | 1.0 | NID_C. |
| `nid_bg` | whole number | always | 1.0 | NID_BG. |
| `n_pig` | whole number | always | 1.0 | N_PIG. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ebicab_clear` | none | none |
| `remove` | `sdrkit_ebicab_remove` | `sdrkit_ebicab_remove_args` | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_scene` | `sdrkit_ebicab_gen_get_scene` | `sdrkit_ebicab_gen_set_scene` |
| `gen_burst_ms` | `sdrkit_ebicab_gen_get_burst_ms` | `sdrkit_ebicab_gen_set_burst_ms` |
| `gen_gap_ms` | `sdrkit_ebicab_gen_get_gap_ms` | `sdrkit_ebicab_gen_set_gap_ms` |
| `gen_random_bits` | `sdrkit_ebicab_gen_get_random_bits` | `sdrkit_ebicab_gen_set_random_bits` |
| `gen_bit_rate_pct` | `sdrkit_ebicab_gen_get_bit_rate_pct` | `sdrkit_ebicab_gen_set_bit_rate_pct` |

## Try it

```sh
sdrkit run ebicab --source synth:ebicab
sdrkit run ebicab --source <driver>:<address> --freq 4.234
```

## Test recordings

- `tests/vectors/cell/ebicab_4000k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
