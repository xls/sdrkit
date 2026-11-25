<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Pagers

POCSAG and FLEX pagers on one channel: addresses, numeric and text messages, with repeated copies combined.

Stack id: `pager`. Group: Paging.

Signal types: POCSAG.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 466.075 MHz |
| Tuning range | 25 MHz to 1 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 25 kHz |
| Offset from the signal | 50 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:pager` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `pocsag` | POCSAG | yes or no | true |  | Look for POCSAG syncs. |
| `flex` | FLEX | yes or no | true |  | Look for FLEX syncs. |
| `pocsag_512` | POCSAG 512 | yes or no | true |  |  |
| `pocsag_1200` | POCSAG 1200 | yes or no | true |  |  |
| `pocsag_2400` | POCSAG 2400 | yes or no | true |  | Turn off 2400 when only a slower speed is on air, to avoid false decodes. |
| `selcall` | Sequential calls | yes or no | false |  | Also decode the sequential tone calls sent on the same channel. |
| `channel` | Channel | choice | `25k` | `25k` (25 kHz), `12.5k` (12.5 kHz) | 12.5 kHz cuts a neighbour better but needs the signal on the channel centre. |
| `narrow_decoder` | Narrow second decoder | yes or no | true |  | On a 25 kHz channel, a second POCSAG and FLEX decoder behind a 7 kHz filter reads weak pages the first misses. |
| `combine` | Combine copies | yes or no | true |  | Paging networks repeat messages: vote the copies word by word. |
| `read` | Read POCSAG pages | choice | `auto` | `auto` (Auto), `function` (By function code), `alpha` (Force alphanumeric), `numeric` (Force numeric) |  |
| `read_by_address` | Read by address | text | empty |  | Rules for single addresses: "address:alpha" or "address:numeric", separated by commas or spaces. |
| `charset` | Character set | choice | `ascii` | `ascii` (ASCII), `german` (German (ä ö ü ß)) |  |
| `wide_input` | One carrier | yes or no | false |  | The input is one carrier at 192 ksps (a channel of a bank span). |
| `protocol` | Protocol | choice | `pocsag` | `pocsag` (POCSAG), `flex` (FLEX) | The protocol that send uses. Only POCSAG is sent: FLEX is a licensed paging service, and send refuses it. |
| `pocsag_rate` | Speed | choice | `1200` | `512` (512 bit/s), `1200` (1200 bit/s), `2400` (2400 bit/s) | The POCSAG bit rate sent: 512, 1200 or 2400 bit/s. |
| `deviation` | Deviation | frequency | 4.5 kHz | 2 kHz to 6 kHz | The frequency of a steady bit from the carrier: 4500 Hz in ITU-R M.584-2. |
| `polarity` | Polarity | choice | `normal` | `normal` (Normal (1 is the lower frequency)), `inverted` (Inverted (1 is the upper frequency)) | Normal sends a 1 as the lower frequency; some transmitters invert it. |
| `tx_level` | Level | number (of full scale) | 0.5 | 0.05 to 0.9 | The magnitude of the signal sent, of full scale. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `receiving` | Receiving | yes or no |
| `carrier` | Carrier | yes or no |
| `locked` | Locked | yes or no |
| `last_speed` | Last speed | text |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `pages` | Pages | whole number |
| `codewords_good` | Code words good | whole number |
| `codewords_repaired` | Code words repaired | whole number |
| `codewords_lost` | Code words lost | whole number |
| `pages_refused` | Pages refused | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_sent` | Pages sent | whole number |
| `tx_waiting` | Pages waiting | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `pages` | Pages | `time_s` (number (s)), `protocol` (text), `address` (whole number), `function` (whole number), `type` (choice), `message` (text), `damaged` (yes or no), `bits_repaired` (whole number), `copies` (whole number), `freq_hz` (frequency) |
| `speeds` | Speeds | `speed` (text), `good` (whole number), `repaired` (whole number), `lost` (whole number), `pages` (whole number), `transmissions` (whole number), `refused` (whole number) |
| `heard` | Addresses heard | `address` (whole number), `protocol` (choice), `pages` (whole number), `speed` (text), `last_text` (text), `last_heard` (time) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `pocsag.page` | POCSAG page | `source` (text), `text` (text), `function` (whole number), `baud` (whole number (bit/s)), `type` (choice), `damaged` (yes or no), `family` (choice) |
| `flex.page` | FLEX page | `source` (text), `text` (text), `speed` (text), `type` (choice), `damaged` (yes or no), `family` (choice) |
| `tones.selcall` | Selcall call | `source` (text), `text` (text), `digits` (text), `sent` (text), `set` (text), `also_fits` (text), `length_s` (duration (s)), `tone_s` (duration (s)) |
| `pager.tx` | POCSAG transmission | `state` (text), `reason` (text), `target` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `send` | Send a page | `address` (whole number, required), `function` (whole number, default 3), `page_type` (choice, default `alpha`), `text` (text, default empty) |  | Checks the page and queues it. A transmission starts when none is on air; pages sent during a transmission go in the next one. POCSAG only: with the setting protocol on FLEX it is refused. Transmit must be on. |
| `halt` | Halt |  |  | Drops the pages waiting and ends the transmission after the current bit with a 1 ms fall. |

## Typed interface

The stack is in the family `trunk`, version 1.6. The C header `sdrkit/trunk.h`, the registry record `registry/trunk.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Paging (`paging`). Media: text.

### Result fields

Struct `PagerFields`. The result fields of the pager stack.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `receiving` | yes or no | always | 1.0 | A decoder receives a transmission. |
| `carrier` | yes or no | always | 1.0 | A carrier is present. |
| `locked` | yes or no | always | 1.0 | A decoder holds sync. |
| `last_speed` | text | always | 1.0 | The protocol and speed of the last sync ("POCSAG 1200"). |
| `offset_hz` | frequency | optional | 1.0 | The offset of the carrier from the tuned frequency at the last sync. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio of the carrier, while a carrier is present. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the input. |
| `pages` | whole number | always | 1.0 | The pages read. |
| `codewords_good` | whole number | always | 1.0 | The code words valid as received or repaired. |
| `codewords_repaired` | whole number | always | 1.0 | The code words repaired. |
| `codewords_lost` | whole number | always | 1.0 | The code words lost. |
| `pages_refused` | whole number | always | 1.0 | The pages that the acceptance rules refused (address ranges, vector values, message checks). |

### Result fields

Struct `PagerTxFields`. The result fields of the POCSAG sender.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.2 | Whether a transmission is on air. |
| `tx_sent` | whole number | always | 1.2 | The pages of the transmission on air. |
| `tx_waiting` | whole number | always | 1.2 | The pages waiting for the next transmission. |

### Table `pages`

Struct `PagerPageRow`. A row of the page list. The row key is the sequence number of the page.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time of the page. |
| `protocol` | text | always | 1.0 | The protocol and speed ("POCSAG 1200", "FLEX 3200/4", "Selcall ZVEI1"). |
| `address` | whole number | always | 1.0 | The RIC or capcode (0 for a sequential call). |
| `function` | whole number | optional | 1.0 | The POCSAG function code. |
| `type` | choice | always | 1.0 | What the page carries. |
| `message` | text | always | 1.0 | The message. |
| `damaged` | yes or no | always | 1.0 | A word was lost or the end is not sure. |
| `bits_repaired` | whole number | always | 1.0 | The bits that the repairs changed. |
| `copies` | whole number | always | 1.0 | The copies voted into the page. |
| `freq_hz` | frequency | optional | 1.0 | The frequency of the carrier, when it is known. |

The rows are the messages of the stack. Addressee: `address`. Text: `message`. Time: `time_s`.

### Table `speeds`

Struct `PagerSpeedRow`. A row of the statistics of each speed. The row key is the speed.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `speed` | text | always | 1.0 | The protocol and speed. |
| `good` | whole number | always | 1.0 | The code words valid as received. |
| `repaired` | whole number | always | 1.0 | The code words repaired. |
| `lost` | whole number | always | 1.0 | The code words lost. |
| `pages` | whole number | always | 1.0 | The pages read. |
| `transmissions` | whole number | always | 1.0 | The batches (POCSAG) or frames (FLEX). |
| `refused` | whole number | always | 1.0 | The pages that the acceptance rules refused. |

### Table `heard`

Struct `PagerHeardRow`. A row of the addresses heard. The row key is the protocol and the address ("pocsag/1234567").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | whole number | always | 1.0 | The RIC or capcode. |
| `protocol` | choice | always | 1.0 | The paging protocol. |
| `pages` | whole number | always | 1.0 | The pages of the address. |
| `speed` | text | always | 1.0 | The protocol and speed of the last page. |
| `last_text` | text | optional | 1.0 | The last text (a tone-only page does not erase it). |
| `last_heard` | time | optional | 1.0 | When it was last heard. |

The rows are the stations or nodes of the stack. Time: `last_heard`.

### Event `pocsag.page`

Struct `PocsagPageEvent`. A POCSAG page: `source` is the RIC, `text` the message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The RIC. |
| `text` | text | always | 1.0 | The message. |
| `function` | whole number | optional | 1.0 | The function code. |
| `baud` | whole number (bit/s) | optional | 1.0 | The bit rate. |
| `type` | choice | always | 1.0 | What the page carries. |
| `damaged` | yes or no | always | 1.0 | A word was lost or the end is not sure. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |

### Event `flex.page`

Struct `FlexPageEvent`. A FLEX page: `source` is the capcode, `text` the message.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The capcode. |
| `text` | text | always | 1.0 | The message. |
| `speed` | text | always | 1.0 | The FLEX speed ("FLEX 3200/4"). |
| `type` | choice | always | 1.0 | What the page carries. |
| `damaged` | yes or no | always | 1.0 | A word was lost or the end is not sure. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |

### Event `tones.selcall`

Struct `SelcallFields`. The fields of a `tones.selcall` event: a selcall call.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The digits of the call. |
| `text` | text | always | 1.0 | One line for people: the digits. |
| `digits` | text | always | 1.0 | The digits of a call as heard. |
| `sent` | text | always | 1.0 | The digits as sent, with the repeat tones. |
| `set` | text | always | 1.0 | The tone set the call was read as. |
| `also_fits` | text | optional | 1.0 | The other tone sets the call fits, separated by commas. |
| `length_s` | duration (s) | always | 1.0 | The length of the call. |
| `tone_s` | duration (s) | always | 1.0 | The mean length of a tone. |

### Event `pager.tx`

Struct `PagerTxEvent`. The fields of a change of a POCSAG transmission.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.2 | The state: started, queued, finished or refused. |
| `reason` | text | always | 1.2 | Why it changed: empty, halted, the transmitter is off, the format changed, or why a page was refused. |
| `target` | text | always | 1.2 | The addresses of the pages, separated by commas. |
| `text` | text | always | 1.2 | The text of the page for started, queued and refused; empty for finished. |
| `family` | choice | always | 1.2 | The family of the source: a pager. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_pager_clear` | none | none |
| `send` | `sdrkit_pager_send` | `sdrkit_pager_send_args` | none |
| `halt` | `sdrkit_pager_halt` | none | none |

The argument `address` of `send` takes the column `address` of a row of the table `heard`.

### Settings

| Key | C getter | C setter |
|---|---|---|
| `pocsag` | `sdrkit_pager_get_pocsag` | `sdrkit_pager_set_pocsag` |
| `flex` | `sdrkit_pager_get_flex` | `sdrkit_pager_set_flex` |
| `pocsag_512` | `sdrkit_pager_get_pocsag_512` | `sdrkit_pager_set_pocsag_512` |
| `pocsag_1200` | `sdrkit_pager_get_pocsag_1200` | `sdrkit_pager_set_pocsag_1200` |
| `pocsag_2400` | `sdrkit_pager_get_pocsag_2400` | `sdrkit_pager_set_pocsag_2400` |
| `selcall` | `sdrkit_pager_get_selcall` | `sdrkit_pager_set_selcall` |
| `channel` | `sdrkit_pager_get_channel` | `sdrkit_pager_set_channel` |
| `narrow_decoder` | `sdrkit_pager_get_narrow_decoder` | `sdrkit_pager_set_narrow_decoder` |
| `combine` | `sdrkit_pager_get_combine` | `sdrkit_pager_set_combine` |
| `read` | `sdrkit_pager_get_read` | `sdrkit_pager_set_read` |
| `read_by_address` | `sdrkit_pager_get_read_by_address` | `sdrkit_pager_set_read_by_address` |
| `charset` | `sdrkit_pager_get_charset` | `sdrkit_pager_set_charset` |
| `wide_input` | `sdrkit_pager_get_wide_input` | `sdrkit_pager_set_wide_input` |
| `protocol` | `sdrkit_pager_get_protocol` | `sdrkit_pager_set_protocol` |
| `pocsag_rate` | `sdrkit_pager_get_pocsag_rate` | `sdrkit_pager_set_pocsag_rate` |
| `deviation` | `sdrkit_pager_get_deviation` | `sdrkit_pager_set_deviation` |
| `polarity` | `sdrkit_pager_get_polarity` | `sdrkit_pager_set_polarity` |
| `tx_level` | `sdrkit_pager_get_tx_level` | `sdrkit_pager_set_tx_level` |

## Try it

```sh
sdrkit run pager --source synth:pager
sdrkit run pager --source iiod:192.168.2.1 --freq 466.075
```
<!-- generated: end -->

## Transmit

The stack sends POCSAG pages (ITU-R M.584-2) for amateur paging. It does not send FLEX: FLEX is a licensed paging service, and
`send` is refused with that reason while the setting `protocol` is FLEX. The engine setting `transmit_allowed` is the only
switch: with it off, `send` is refused and a transmission on air ends at once.

Transmitting on a paging or railway frequency without the operator's authority and a licence is forbidden in nearly every
country. False pages can call out emergency services or warn of trains that are not there. Use the sender into a dummy load,
on a cable, in a shielded enclosure, or on a frequency your licence covers (for example an amateur paging frequency).

### Commands and settings

| Command | Effect |
|---|---|
| `send` | Checks the page and queues it. A transmission starts when none is on air; pages sent during a transmission go together in the next one. At most 16 pages wait. |
| `halt` | Drops the pages waiting and ends the transmission after the bit on air with a 1 ms fall and the end mark. |

The arguments of `send`: `address` (the RIC, 0 to 2 097 151, required), `function` (0 to 3, default 3; by convention 0 is
numeric and 3 alphanumeric), `page_type` (tone only, numeric or alphanumeric, default alphanumeric) and `text` (at most 1000
characters; empty sends a tone-only page). Numeric text takes 0 to 9, `*`, `U`, space, `-`, `]` and `[` (`)` and `(` are
taken for the last two: the values 14 and 15); trailing spaces are not kept, since a numeric message is filled with spaces.
Alphanumeric text takes 7-bit ASCII with no control character other than CR and LF. `send` is refused for an address or a
function out of range, a character the page type cannot carry, a tone-only page with a text, and the addresses 2 007 664 to
2 007 671 with function 0, whose address word is the idle word.

The settings, in the section Transmit: `protocol` (POCSAG or FLEX, default POCSAG), `pocsag_rate` (512, 1200 or 2400 bit/s,
default 1200), `deviation` (2000 to 6000 Hz, default 4500), `polarity` (normal: a 1 is the lower frequency; or inverted) and
`tx_level` (the magnitude, 0.05 to 0.9 of full scale, default 0.5). A change applies to the next transmission.

### Results and events

`tx_sending` (a transmission is on air), `tx_sent` (the pages of the transmission on air) and `tx_waiting`. The event
`pager.tx` says when a page started a transmission, was queued, was refused (with the reason), and when a transmission
finished (with the reason: halted, the transmitter is off, the format changed); `target` holds the addresses.

### On air

The sender writes into the transmit sink as many samples in each pass as the pass brings in, at the session rate. On a live
radio whose host gives a transmit lead, the first sample states its time on air. The last sample carries the end mark. The
carrier is at the tuned frequency; it rises and falls with a 1 ms raised cosine, and between them the envelope is constant at
`tx_level`.

### The signal

- 2-FSK on an FM carrier, NRZ, at plus or minus the deviation. Each change of frequency is a raised sine over 30 % of a bit
  either side of the boundary.
- A preamble of 576 bits 1, 0, 1, 0 ..., then batches: the sync word `7CD215D8` and 16 code words in 8 frames of 2. A page
  starts in the frame of the 3 low bits of its address; the frames before it carry the idle word `7A89C197`. Its message words
  follow and continue over as many batches as they need; then an idle word, and idle words to the end of the last batch.
- A code word is 32 bits, first bit most significant: a flag (0 address, 1 message), 20 bits, the 10 check bits of BCH(31,21)
  (generator x^10 + x^9 + x^8 + x^6 + x^5 + x^3 + 1) and one even parity bit. An address word holds the address shifted right
  by 3 (18 bits) and the function (2 bits).
- Numeric: 4-bit characters, each sent least significant bit first, filled with spaces (12) to whole words. Alphanumeric:
  7-bit characters, each sent least significant bit first, then EOT (04) and zero bits to a whole word.

Example: address 1 234 567, function 3, "HELLO": frame 7, address word `4B5A1A25`, message words `89A2634D`, `CCF92332`,
`80000769`.

### Checks

- The stack's own receive side reads each page whole with no bit repaired: numeric and alphanumeric pages at 512, 1200 and
  2400 bit/s at 12 dB in 25 kHz, an address in each of the 8 frames, a message over several batches, both polarities, tone-only
  pages, and pages queued during a transmission.
- The code words: the sync and the idle words, the two worked examples of the specification, and the address word of
  1 234 567 checked by hand; the bits on air are the preamble and the words, a 1 the lower frequency with normal polarity.
- At 1200 bit/s and 4500 Hz: the measured deviation is 4500 Hz at the centres of the bits, and the 99 % power bandwidth is
  10.8 kHz (Carson: 11.4 kHz; the limit is 16 kHz).
- Also covered: the refusals (the switch off, FLEX, the arguments), `halt` and the end mark.
