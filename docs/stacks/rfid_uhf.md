<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# UHF RFID reader commands (EPC Gen2)

The commands an EPC Gen2 reader sends to its tags, with their fields, Tari, RTcal, TRcal and the tag rate they ask. Tag replies are not decoded.

Stack id: `rfid_uhf`. Group: Tags and charging.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 400 kHz |
| Signal bandwidth | 600 kHz |
| Automatic gain | Found once and held |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:rfid_uhf` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `list_unchecked` | List commands that have no checksum | yes or no | true |  | QueryRep, QueryAdjust, ACK and NAK are counted either way; off, they make no row and no event. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_tari_us` | Tari | number (us) | 12.5 | 6.25 to 25 |  |
| `gen_data1_tari` | Data-1 | number (Tari) | 1.75 | 1.5 to 2 |  |
| `gen_pw_tari` | Low pulse (PW) | number (Tari) | 0.45 | 0.265 to 0.525 |  |
| `gen_depth` | Modulation depth | number | 0.9 | 0.8 to 1 |  |
| `gen_trcal_rtcal` | TRcal | number (RTcal) | 2 | 1.1 to 3 |  |
| `gen_reader_clock_ppm` | Reader clock error | number (ppm) | 0 | -50000 to 50000 | Every time of the reader runs this much fast (positive) or slow. |
| `gen_gap_s` | Gap between commands | number (s) | 0.002 | 0.0005 to 1 |  |
| `gen_lead_s` | Lead | number (s) | 0.002 | 0 to 10 |  |
| `gen_commands` | Commands | text | `query dr=0 m=2 trext=1 sel=3 session=2 target=1 q=4; ack rn=BEEF; queryrep session=2; queryadjust session=1 updn=110; nak; select target=4 action=0 bank=1 pointer=32 mask=1010011100110101 truncate=0; reqrn rn=1234; read bank=3 pointer=5 count=4 rn=A5A5` |  | Commands separated by ';': a name (query, queryrep, queryadjust, ack, nak, select, reqrn, read, write, kill, lock, access) and key=value fields; rn, data, half and payload are hex, updn and mask are bits, flip=N sends bit N inverted. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `text` | Text | text |
| `state` | State | choice |
| `good` | Good | whole number |
| `bad` | Refused | whole number |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `tari_us` | Tari (data-0) | number (us) |
| `rtcal_us` | RTcal | number (us) |
| `rtcal_tari` | RTcal in Tari | number |
| `trcal_us` | TRcal | number (us) |
| `blf_dr8_khz` | Tag link frequency, DR 8 | number (kHz) |
| `blf_dr643_khz` | Tag link frequency, DR 64/3 | number (kHz) |
| `commands_ok` | Commands decoded | whole number |
| `commands_failed` | Frames failed | whole number |
| `count_query` | Query | whole number |
| `count_query_rep` | QueryRep | whole number |
| `count_query_adjust` | QueryAdjust | whole number |
| `count_ack` | ACK | whole number |
| `count_nak` | NAK | whole number |
| `count_select` | Select | whole number |
| `count_req_rn` | Req_RN | whole number |
| `count_read` | Read | whole number |
| `count_write` | Write | whole number |
| `count_kill` | Kill | whole number |
| `count_lock` | Lock | whole number |
| `count_access` | Access | whole number |
| `reader_carrier` | Reader carrier seen | yes or no |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `commands` | Commands | `time_s` (duration (s)), `command` (choice), `session` (whole number), `q` (whole number), `slots` (whole number), `target` (text), `sel` (text), `coding` (text), `pilot` (yes or no), `dr` (text), `updn` (text), `rn16` (text), `bank` (text), `pointer` (whole number), `word_count` (whole number), `mask` (text), `truncate` (yes or no), `action` (whole number), `lock_payload` (text), `level_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `rfid.command` | Reader command | `time_s` (duration (s)), `command` (choice), `session` (whole number), `q` (whole number), `slots` (whole number), `target` (text), `sel` (text), `coding` (text), `pilot` (yes or no), `dr` (text), `updn` (text), `rn16` (text), `bank` (text), `pointer` (whole number), `word_count` (whole number), `mask` (text), `truncate` (yes or no), `action` (whole number), `lock_payload` (text), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Drops the trackers and the frame in progress; the table stays. |

## Typed interface

The stack is in the family `tags`, version 1.1. The C header `sdrkit/tags.h`, the registry record `registry/tags.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Sensors and devices (`devices`). Media: none.

### Result fields

Struct `TagsStatusFields`. The status of a stack of the family: the headline, the state, the counts of results and the input level.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The headline. |
| `state` | choice | always | 1.0 | The state. |
| `good` | whole number | always | 1.0 | The good results: what passed every check of its format. |
| `bad` | whole number | always | 1.0 | The refused results: what failed a check. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The input level of the last report, relative to full scale. |
| `note` | text | always | 1.0 | What the stack reads and what no public document confirms. |

### Result fields

Struct `RfidUhfFields`. The result fields of `rfid_uhf`. The timings are those of the last good command; they have no value without a reader carrier in the last 8 s.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tari_us` | number (us) | optional | 1.0 | Tari: the length of a data-0. |
| `rtcal_us` | number (us) | optional | 1.0 | RTcal: a data-0 and a data-1. |
| `rtcal_tari` | number | optional | 1.0 | RTcal in Tari. |
| `trcal_us` | number (us) | optional | 1.0 | TRcal of the last Query. |
| `blf_dr8_khz` | number (kHz) | optional | 1.0 | The backscatter link frequency that the last Query asks with the divide ratio 8: 8 / TRcal. |
| `blf_dr643_khz` | number (kHz) | optional | 1.0 | The backscatter link frequency with the divide ratio 64/3. |
| `commands_ok` | whole number | always | 1.0 | The commands that passed every check. |
| `commands_failed` | whole number | always | 1.0 | The frames that failed a timing rule, their length or their CRC. |
| `count_query` | whole number | always | 1.0 | The Query commands decoded. |
| `count_query_rep` | whole number | always | 1.0 | The QueryRep commands decoded. |
| `count_query_adjust` | whole number | always | 1.0 | The QueryAdjust commands decoded. |
| `count_ack` | whole number | always | 1.0 | The ACK commands decoded. |
| `count_nak` | whole number | always | 1.0 | The NAK commands decoded. |
| `count_select` | whole number | always | 1.0 | The Select commands decoded. |
| `count_req_rn` | whole number | always | 1.0 | The Req_RN commands decoded. |
| `count_read` | whole number | always | 1.0 | The Read commands decoded. |
| `count_write` | whole number | always | 1.0 | The Write commands decoded. |
| `count_kill` | whole number | always | 1.0 | The Kill commands decoded. |
| `count_lock` | whole number | always | 1.0 | The Lock commands decoded. |
| `count_access` | whole number | always | 1.0 | The Access commands decoded. |
| `reader_carrier` | yes or no | always | 1.0 | A modulated reader carrier was seen in the last 2 s. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `commands`

Struct `RfidUhfCommandRow`. A row of the command table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the end of the command. |
| `command` | choice | always | 1.0 | The command. |
| `session` | whole number | optional | 1.0 | The session, 0 to 3. |
| `q` | whole number | optional | 1.0 | Q of a Query. |
| `slots` | whole number | optional | 1.0 | The slots of a Query, 2 to the power Q. |
| `target` | text | optional | 1.0 | The target: A or B of a Query, S0 to S3, SL or RFU of a Select. |
| `sel` | text | optional | 1.0 | The tags a Query addresses: all, ~SL or SL. |
| `coding` | text | optional | 1.0 | The coding of the tag reply that a Query asks: FM0, Miller 2, 4 or 8. |
| `pilot` | yes or no | optional | 1.0 | A Query asks for a pilot tone (TRext). |
| `dr` | text | optional | 1.0 | The divide ratio of a Query: 8 or 64/3. |
| `updn` | text | optional | 1.0 | The change of Q of a QueryAdjust: +1, -1, unchanged or reserved. |
| `rn16` | text | optional | 1.0 | The RN16 of an ACK or a Req_RN, or the handle of an access command, 4 hex digits. |
| `bank` | text | optional | 1.0 | The memory bank of a Select (FileType, EPC, TID, File_0) or of a Read or Write (Reserved, EPC, TID, User). |
| `pointer` | whole number | optional | 1.0 | The bit pointer of a Select, or the word pointer of a Read or a Write. |
| `word_count` | whole number | optional | 1.0 | The words that a Read asks. |
| `mask` | text | optional | 1.0 | The mask of a Select, as bits (the first 96). |
| `truncate` | yes or no | optional | 1.0 | The Truncate bit of a Select. |
| `action` | whole number | optional | 1.0 | The action of a Select, 0 to 7. |
| `lock_payload` | text | optional | 1.0 | The mask and action bits of a Lock, 5 hex digits. |
| `level_db` | number (dB) | always | 1.0 | The level of the reader carrier, relative. |

### Event `rfid.command`

Struct `RfidCommandEvent`. A reader command passed every check.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the end of the command. |
| `command` | choice | always | 1.0 | The command. |
| `session` | whole number | optional | 1.0 | The session, 0 to 3. |
| `q` | whole number | optional | 1.0 | Q of a Query. |
| `slots` | whole number | optional | 1.0 | The slots of a Query, 2 to the power Q. |
| `target` | text | optional | 1.0 | The target: A or B of a Query, S0 to S3, SL or RFU of a Select. |
| `sel` | text | optional | 1.0 | The tags a Query addresses: all, ~SL or SL. |
| `coding` | text | optional | 1.0 | The coding of the tag reply that a Query asks: FM0, Miller 2, 4 or 8. |
| `pilot` | yes or no | optional | 1.0 | A Query asks for a pilot tone (TRext). |
| `dr` | text | optional | 1.0 | The divide ratio of a Query: 8 or 64/3. |
| `updn` | text | optional | 1.0 | The change of Q of a QueryAdjust: +1, -1, unchanged or reserved. |
| `rn16` | text | optional | 1.0 | The RN16 of an ACK or a Req_RN, or the handle of an access command, 4 hex digits. |
| `bank` | text | optional | 1.0 | The memory bank of a Select (FileType, EPC, TID, File_0) or of a Read or Write (Reserved, EPC, TID, User). |
| `pointer` | whole number | optional | 1.0 | The bit pointer of a Select, or the word pointer of a Read or a Write. |
| `word_count` | whole number | optional | 1.0 | The words that a Read asks. |
| `mask` | text | optional | 1.0 | The mask of a Select, as bits (the first 96). |
| `truncate` | yes or no | optional | 1.0 | The Truncate bit of a Select. |
| `action` | whole number | optional | 1.0 | The action of a Select, 0 to 7. |
| `lock_payload` | text | optional | 1.0 | The mask and action bits of a Lock, 5 hex digits. |
| `level_db` | number (dB) | always | 1.0 | The level of the reader carrier, relative. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_rfid_uhf_clear` | none | none |
| `reset` | `sdrkit_rfid_uhf_reset` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `list_unchecked` | `sdrkit_rfid_uhf_get_list_unchecked` | `sdrkit_rfid_uhf_set_list_unchecked` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_tari_us` | `sdrkit_rfid_uhf_gen_get_tari_us` | `sdrkit_rfid_uhf_gen_set_tari_us` |
| `gen_data1_tari` | `sdrkit_rfid_uhf_gen_get_data1_tari` | `sdrkit_rfid_uhf_gen_set_data1_tari` |
| `gen_pw_tari` | `sdrkit_rfid_uhf_gen_get_pw_tari` | `sdrkit_rfid_uhf_gen_set_pw_tari` |
| `gen_depth` | `sdrkit_rfid_uhf_gen_get_depth` | `sdrkit_rfid_uhf_gen_set_depth` |
| `gen_trcal_rtcal` | `sdrkit_rfid_uhf_gen_get_trcal_rtcal` | `sdrkit_rfid_uhf_gen_set_trcal_rtcal` |
| `gen_reader_clock_ppm` | `sdrkit_rfid_uhf_gen_get_reader_clock_ppm` | `sdrkit_rfid_uhf_gen_set_reader_clock_ppm` |
| `gen_gap_s` | `sdrkit_rfid_uhf_gen_get_gap_s` | `sdrkit_rfid_uhf_gen_set_gap_s` |
| `gen_lead_s` | `sdrkit_rfid_uhf_gen_get_lead_s` | `sdrkit_rfid_uhf_gen_set_lead_s` |
| `gen_commands` | `sdrkit_rfid_uhf_gen_get_commands` | `sdrkit_rfid_uhf_gen_set_commands` |

## Try it

```sh
sdrkit run rfid_uhf --source synth:rfid_uhf
sdrkit run rfid_uhf --source <driver>:<address> --freq 0
```
<!-- generated: end -->
