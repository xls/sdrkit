<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# GSC paging

Golay sequential code pagers: tone, numeric, alphanumeric and tone-and-voice pages, single calls and batches, with the pager code and its address function.

Stack id: `gsc`. Group: Paging.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 160 MHz |
| Tuning range | 26 MHz to 1 GHz |
| Sample rate | 250 kHz |
| Smallest sample rate | 96 kHz |
| Signal bandwidth | 12.5 kHz |
| Offset from the signal | 50 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:gsc` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_pages` | Test signal pages | choice | `all` | `all` (Tone, numeric, alphanumeric and voice pages), `tone` (A tone page), `numeric` (A numeric page), `alpha` (An alphanumeric page), `voice` (A voice page) |  |
| `gen_batch` | Test signal batch | yes or no | false |  |  |
| `gen_no_preamble` | Test signal without preamble | yes or no | false |  |  |
| `gen_deviation_hz` | Test signal deviation | number (Hz) | 4500 | 500 to 8000 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `pages` | Pages | whole number |
| `tone_pages` | Tone pages | whole number |
| `numeric_pages` | Numeric pages | whole number |
| `alpha_pages` | Alpha pages | whole number |
| `voice_pages` | Voice pages | whole number |
| `preambles` | Preambles | whole number |
| `locked` | Locked | yes or no |
| `preamble` | Preamble | whole number |
| `call` | Call | text |
| `inverted` | Inverted | yes or no |
| `deviation_hz` | Deviation | frequency |
| `offset_hz` | Offset | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `quality_db` | Block quality | number (dB) |
| `carrier` | Carrier | yes or no |
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |
| `modem.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `pages` | Pages | `time_s` (number (s)), `kind` (text), `code` (text), `function` (whole number), `words` (text), `preamble` (whole number), `call` (text), `text` (text), `blocks` (whole number), `note` (text), `bits_corrected` (whole number), `word` (whole number), `voice_s` (number (s)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `gsc.page` | GSC page | `target` (text), `kind` (text), `function` (whole number), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `trunk`, version 1.6. The C header `sdrkit/trunk.h`, the registry record `registry/trunk.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Paging (`paging`). Media: text.

### Result fields

Struct `GscFields`. The counts and the measurements of the GSC receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `pages` | whole number | always | 1.1 | The pages of every kind. |
| `tone_pages` | whole number | always | 1.1 | The tone-only pages. |
| `numeric_pages` | whole number | always | 1.1 | The numeric pages. |
| `alpha_pages` | whole number | always | 1.1 | The alphanumeric pages. |
| `voice_pages` | whole number | always | 1.1 | The tone-and-voice pages. |
| `preambles` | whole number | always | 1.1 | The preamble words seen (at most one a second). |
| `locked` | yes or no | always | 1.1 | The receiver holds block lock. |
| `preamble` | whole number | optional | 1.1 | The preamble index remembered (0 to 9). |
| `call` | text | optional | 1.1 | How the call is sent: "single call" or "batch" (known after the start code). |
| `inverted` | yes or no | always | 1.1 | The spectrum is inverted (from the last start code). |
| `deviation_hz` | frequency | optional | 1.1 | The deviation measured from the levels. |
| `offset_hz` | frequency | optional | 1.1 | The offset of the carrier from the tuned frequency. |
| `clock_ppm` | number (ppm) | optional | 1.1 | The error of the sender's bit clock. |
| `quality_db` | number (dB) | optional | 1.1 | The quality of the last block read while locked: the mean of the soft bits squared over their variance. |
| `carrier` | yes or no | optional | 1.1 | A carrier is present. |
| `snr_db` | number (dB) | optional | 1.1 | The signal to noise ratio of the carrier. |
| `level_dbfs` | number (dBFS) | optional | 1.1 | The level of the input. |
| `note` | text | always | 1.1 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `pages`

Struct `GscPageRow`. A row of the GSC table: a page or the end of a voice page. The row key is the number of the row.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.1 | The signal time of the row. |
| `kind` | text | always | 1.1 | What the row is: "Tone page", "Numeric page", "Alpha page", "Voice page" or "Voice end". |
| `code` | text | always | 1.1 | The pager code: six digits, or N and five digits when no preamble was seen. |
| `function` | whole number | always | 1.1 | The address function, 1 to 4. |
| `words` | text | always | 1.1 | Which words of the address are inverted. |
| `preamble` | whole number | optional | 1.1 | The preamble index (0 to 9). |
| `call` | text | always | 1.1 | How the call was sent: "single call", "batch" or "no preamble seen". |
| `text` | text | always | 1.1 | The text of a data page, or "tone and voice". |
| `blocks` | whole number | optional | 1.1 | The data blocks of a data page. |
| `note` | text | optional | 1.1 | A note: more blocks were announced, the voice is not decoded, how a voice page ended. |
| `bits_corrected` | whole number | always | 1.1 | The bits corrected in the two address words. |
| `word` | whole number | optional | 1.1 | The activation word of a voice page, or the word that ended it. |
| `voice_s` | number (s) | optional | 1.1 | The length of the voice. |

The rows are the messages of the stack. The column `call` names a row. Addressee: `code`. Text: `text`. Time: `time_s`.

### Event `gsc.page`

Struct `GscPageEvent`. A GSC page: `target` is the pager code.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `target` | text | always | 1.1 | The pager code. |
| `kind` | text | always | 1.1 | The kind of page. |
| `function` | whole number | always | 1.1 | The address function, 1 to 4. |
| `text` | text | always | 1.1 | The text of a data page. |
| `family` | choice | always | 1.1 | The kind of thing that the event is about. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_gsc_clear` | none | none |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_pages` | `sdrkit_gsc_gen_get_pages` | `sdrkit_gsc_gen_set_pages` |
| `gen_batch` | `sdrkit_gsc_gen_get_batch` | `sdrkit_gsc_gen_set_batch` |
| `gen_no_preamble` | `sdrkit_gsc_gen_get_no_preamble` | `sdrkit_gsc_gen_set_no_preamble` |
| `gen_deviation_hz` | `sdrkit_gsc_gen_get_deviation_hz` | `sdrkit_gsc_gen_set_deviation_hz` |

## Try it

```sh
sdrkit run gsc --source synth:gsc
sdrkit run gsc --source iiod:192.168.2.1 --freq 160
```
<!-- generated: end -->

## Description

The stack reads Golay sequential code pagers on one narrow FM paging channel: tone-only, numeric, alphanumeric and
tone-and-voice pages, single calls and batches, with and without the battery-saver preamble. The voice of a tone-and-voice page
is analogue audio on the channel: the stack marks it and its end, it does not decode it. Receive only.

## The signal

2-FSK on FM, mark (the upper frequency) is 1. Every comma bit and data bit runs at 600 bit/s; every bit of a Golay (23,12) word
is sent twice on that grid, so the words run at 300 bit/s. A word is sent `d0 .. d11`, then `p0 .. p10`.

| Part | Content |
|---|---|
| Preamble | 28 alternating comma bits, then one of ten preamble words 18 times (1.427 s); an inverted preamble word marks a batch |
| Start code | comma, Word 1 = the word of 713, a separator bit, Word 2 = its inverse (121 grid bits, 0.202 s) |
| Address | comma, Word 1 from a table of 50 values, separator, Word 2 = 100 x floor(A / 50) + (A mod 50) (+ 50 when G is 50 or more); the inversion of the two words gives the address function 1 to 4 |
| Data block | a comma bit, then 8 BCH (15,7) words sent by columns: 12 numeric or 8 alphanumeric characters, a continue bit and a sum check |
| Activation, deactivation | a two-word block whose Word 2 is the inverse of Word 1; the voice lies between them |

A pager code is six digits I G1 G0 A2 A1 A0; the preamble index is (I + G0) mod 10. Without a preamble the code shows as N and
G x 1000 + A.

## The receiver

The paging front end gives the 25 kHz channel at 48 000 samples/s; an FM discriminator and the mean of each pair of samples give
24 000 samples/s, and a moving mean of one 600 bit/s bit is read at the bit clock. An upper and a lower level mean set the centre
(the carrier offset) and the deviation. The clock moves on each bit edge; read between the bits of an alternating pattern, it
moves by half a bit. While unlocked the stack searches every bit for a preamble word (both senses), a start code (which locks
and sets the sense of the spectrum), an address without a preamble, and the end word of an open voice page; a candidate block is
taken when its ideal block correlates best of the five grid positions around it. While locked it reads one block every 121 bits:
a start code, a data block of the waiting page (soft decision over the 128 BCH words), an address, or an activation word.

A data page is read both ways and the reading with more plausible characters wins (0.2 more for a reading whose last block ends
with its own fill character); within 0.15 the other reading follows in brackets. A voice page ends when its activation word
comes again, at a deactivation word, or after 16 s.

## Not confirmed by a public document

The preamble values, the start code 713, the Word 1 values, the rule from a pager code to its words, the illegal addresses, the
order of the address functions 2 to 4, the character tables, the place of the continue bit and the BCH generator follow the
specification; the public text read confirms the format, the timing, the comma rules and the code sizes. The activation and
deactivation words are not published: the stack takes any word of their form, and the test signal uses 3061 and 2457. The
result field `note` says the same.

## Test signal

The default `synth:gsc` sends four single calls: a tone page to 954853, a numeric page "5551234" and an alphanumeric page
"CALL HQ" to 123456, and a voice page of 2 s to 954853. `gen_pages`, `gen_batch`, `gen_no_preamble` and `gen_deviation_hz`
change it.

## Measured

With a carrier offset of 800 Hz and a sender clock of +300 ppm, at 96 ksps: numeric and alphanumeric pages 12 of 12 down to
6 dB C/N in 12.5 kHz, 11 of 12 at 3 dB, 9 of 12 at 1 dB; tone pages 6 of 6 down to 1 dB.
