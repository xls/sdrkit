<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SSTV

Slow-scan television: colour pictures sent line by line as tones (Robot, Martin, Scottie, PD, Wraase).

Stack id: `sstv`. Group: HF and utility.

Signal types: SSTV.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 3 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:sstv` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `picture_filter` | Picture filter | yes or no | true |  | Smooths the noise of a weak picture along each scan line and over 3 x 3 pixels, with the noise measured on the syncs and in the picture. A clean picture is left as it is. |
| `demod` | Demodulator | choice | `usb` | `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) | How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m. |
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |
| `tx_mode` | Transmit mode | choice | `Robot 36` | `Robot 36`, `Robot 72`, `Martin M1`, `Martin M2`, `Scottie S1`, `Scottie S2`, `Scottie DX`, `PD50`, `PD90`, `PD120`, `PD180`, `Wraase SC2-180` | The SSTV mode of the pictures sent. |
| `fit` | Fit | choice | `bars` | `bars` (Keep the shape, black bars), `crop` (Keep the shape, crop), `stretch` (Stretch) | How a picture of another shape than the mode is fitted: whole with black bars, cut to the shape of the mode, or stretched. |
| `repeat` | Repeat | yes or no | false |  | Sends the picture again after each pause until stop. Off at the start: a picture is sent once. |
| `pause_s` | Pause between pictures | duration (s) | 1.9 s | 0 to 600 | The time between the end of a picture and the start of the next one when repeat is on. The transmitter is off in the pause. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_mode` | Picture mode | choice | `Robot 36` | `Robot 36`, `Robot 72`, `Martin M1`, `Martin M2`, `Scottie S1`, `Scottie S2`, `Scottie DX`, `PD50`, `PD90`, `PD120`, `PD180`, `Wraase SC2-180` | The SSTV mode of the test picture. |
| `gen_vis` | Send the header | yes or no | true |  | Off: the picture starts without its VIS header, so the stack finds it by the line period. |
| `gen_picture` | Test picture | choice | `card` | `card` (Test card), `texture` (Texture) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `mode` | Mode | text |
| `vis` | VIS code | whole number |
| `slant_ppm` | Slant | number (ppm) |
| `offset_hz` | Offset | frequency |
| `picture_id` | Picture | whole number |
| `picture_width` | Width | whole number (px) |
| `picture_height` | Height | whole number (px) |
| `picture_rows` | Rows | whole number |
| `picture_progress_pct` | Progress | number (%) |
| `picture_complete` | Complete | yes or no |
| `picture_colour` | Colour | yes or no |
| `picture_mode` | Picture mode | text |
| `pictures` | Pictures | whole number |
| `dropped` | Dropped | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_mode` | Transmit mode | text |
| `tx_line` | Line sent | whole number |
| `tx_lines` | Lines to send | whole number |
| `tx_picture_s` | Picture time on air | duration (s) |
| `tx_next_s` | Next picture in | duration (s) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `sstv.picture` | Picture received | `picture_id` (whole number), `picture_width` (whole number (px)), `picture_height` (whole number (px)), `picture_rows` (whole number), `picture_complete` (yes or no), `picture_mode` (text) |
| `sstv.tx` | SSTV transmission | `state` (text), `reason` (text), `mode` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send a picture | `pixels` (bytes, required), `width` (whole number (px), required), `height` (whole number (px), required), `pixel_format` (choice, required) |  | Scales the picture to the transmit mode by the fit setting and sends it: the VIS header, then the lines; again after each pause while repeat is on. Transmit must be on. |
| `stop` | Stop sending |  |  | Ends the picture at once. |

## Typed interface

The stack is in the family `hfkbd`, version 2.10. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Pictures and fax (`pictures`). Media: sound (monitor), pictures.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `SstvFields`. The result fields of SSTV.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `mode` | text | optional | 1.0 | The mode of the picture being received or the newest one. It has no value before the first picture. |
| `vis` | whole number | optional | 1.0 | Its VIS code, -1 when it was found from the line period. |
| `slant_ppm` | number (ppm) | optional | 1.0 | Its slant: the line period over the nominal one, minus 1. |
| `offset_hz` | frequency | always | 1.0 | The tuning error. |

### Result fields

Struct `PictureFields`. The picture being received, or else the newest one. Each field of a picture has no value before the first.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `picture_id` | whole number | optional | 1.0 | The number of the picture. |
| `picture_width` | whole number (px) | optional | 1.0 | Its width. |
| `picture_height` | whole number (px) | optional | 1.0 | Its height. |
| `picture_rows` | whole number | optional | 1.0 | The rows received. |
| `picture_progress_pct` | number (%) | optional | 1.0 | How much of it is in. |
| `picture_complete` | yes or no | optional | 1.0 | It is complete. |
| `picture_colour` | yes or no | optional | 1.0 | It is in colour. |
| `picture_mode` | text | optional | 1.0 | Its mode. |
| `pictures` | whole number | always | 1.0 | The pictures finished. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Result fields

Struct `SstvTxFields`. The result fields of the SSTV transmitter.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 2.3 | Whether a picture is being sent. |
| `tx_mode` | text | optional | 2.3 | The mode of the picture being sent. No value while nothing is sent. |
| `tx_line` | whole number | optional | 2.3 | The line being sent, from 1; 0 during the VIS header. No value while nothing is sent. |
| `tx_lines` | whole number | optional | 2.3 | The lines of the picture being sent. No value while nothing is sent. |
| `tx_picture_s` | duration (s) | always | 2.3 | The time on air of one picture in the transmit mode: the VIS header and the lines. |
| `tx_next_s` | duration (s) | optional | 2.6 | The time until the picture is sent again, while repeat waits in the pause. No value otherwise. |

### Event `sstv.picture`

Struct `PictureEvent`. The fields of a picture received or cut off.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `picture_id` | whole number | always | 1.0 | The number of the picture. |
| `picture_width` | whole number (px) | always | 1.0 | Its width. |
| `picture_height` | whole number (px) | always | 1.0 | Its height. |
| `picture_rows` | whole number | always | 1.0 | The rows received. |
| `picture_complete` | yes or no | always | 1.0 | It is complete. |
| `picture_mode` | text | always | 1.0 | Its mode. |

### Event `sstv.tx`

Struct `SstvTxEvent`. The fields of a change of the SSTV transmission.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 2.3 | The state: started, finished or refused. |
| `reason` | text | always | 2.3 | Why it changed: empty, stopped, the transmitter is off, or why a picture was refused. |
| `mode` | text | always | 2.3 | The mode of the picture. |
| `family` | choice | always | 2.3 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_sstv_clear` | none | none |
| `send` | `sdrkit_sstv_send` | `sdrkit_sstv_send_args` | none |
| `stop` | `sdrkit_sstv_stop` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `picture_filter` | `sdrkit_sstv_get_picture_filter` | `sdrkit_sstv_set_picture_filter` |
| `demod` | `sdrkit_sstv_get_demod` | `sdrkit_sstv_set_demod` |
| `listen` | `sdrkit_sstv_get_listen` | `sdrkit_sstv_set_listen` |
| `tx_mode` | `sdrkit_sstv_get_tx_mode` | `sdrkit_sstv_set_tx_mode` |
| `fit` | `sdrkit_sstv_get_fit` | `sdrkit_sstv_set_fit` |
| `repeat` | `sdrkit_sstv_get_repeat` | `sdrkit_sstv_set_repeat` |
| `pause_s` | `sdrkit_sstv_get_pause_s` | `sdrkit_sstv_set_pause_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_mode` | `sdrkit_sstv_gen_get_mode` | `sdrkit_sstv_gen_set_mode` |
| `gen_vis` | `sdrkit_sstv_gen_get_vis` | `sdrkit_sstv_gen_set_vis` |
| `gen_picture` | `sdrkit_sstv_gen_get_picture` | `sdrkit_sstv_gen_set_picture` |

## Try it

```sh
sdrkit run sstv --source synth:sstv
sdrkit run sstv --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/hfkbd/hfkbd_sstv_robot36_12k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends a picture that the host gives. The engine setting `transmit_allowed` is the only switch: with it off, `send`
is refused.

### Sending a picture

`send` takes the picture as pixels, not as a file:

| Argument | Kind | Meaning |
|---|---|---|
| `pixels` | bytes | The pixel bytes, rows top first, no padding between rows |
| `width` | integer | 1 to 8192 pixels |
| `height` | integer | 1 to 8192 pixels |
| `pixel_format` | choice | `rgb8` (3 bytes a pixel), `bgra` (4 bytes, alpha not used) or `grey8` (1 byte) |

The byte count must be width x height x the bytes of a pixel. The pictures that the receive side hands out are BGRA rasters,
so a received picture can be sent again as it is. `stop` ends the picture, and a repeat, at once. `send` while a picture is on
air is refused.

The settings of the transmitter:

| Key | Meaning | Default |
|---|---|---|
| `tx_mode` | The mode of the pictures sent: one of the twelve modes of the receiver | Robot 36 |
| `fit` | How a picture of another shape is fitted: `bars`, `crop` or `stretch` | `bars` |
| `repeat` | Sends the picture again after each pause until `stop` | off |
| `pause_s` | The pause between two pictures of a repeat, 0 to 600 s | 1.9 |
| `demod` | Shared with the receiver: `usb` the upper sideband, `lsb` the lower, `nfm` FM; `am` refuses `send` | `usb` |

`repeat` is off at the start: a picture goes once. On, the same picture goes again as a new burst `pause_s` after the end of
the last one, with the transmitter off in the pause; `tx_next_s` shows the time to the next picture.

- `bars`: the whole picture, scaled by the smaller of the two size ratios, centred, with black bars at the sides or at the top
  and bottom.
- `crop`: the middle of the picture with the shape of the mode, scaled by the larger ratio.
- `stretch`: the whole picture to the whole mode; the shape is not kept.

A part that is larger than its place is reduced by the area mean of the pixels it covers. A part that is smaller is enlarged by
the nearest pixel: place pixel (x, y) takes pixel (floor(x W / w), floor(y H / h)). The stack draws nothing over the picture: a
call sign or a caption is the host's work.

### The signal

One tone at a time, at a constant level of 0.5 of full scale, with a continuous phase. The tone changes at a sample boundary;
tone k starts at the sample nearest to the sum of the lengths of the tones before it, so the total length is exact to a sample.
On `usb` the tones are the upper sideband of the dial: an audio tone of f Hz is sent at dial + f Hz. On `lsb` they are the
mirror image, at dial - f Hz. On `nfm` (2 m) the carrier is at the dial and the real audio of the tones moves it with a peak
deviation of 2500 Hz (a 12.5 kHz channel), with no pre-emphasis, at the same constant level.

1. The VIS header, 910 ms: 1900 Hz for 300 ms, 1200 Hz for 10 ms, 1900 Hz for 300 ms, then ten bits of 30 ms: the start bit
   (1200 Hz), seven data bits least significant first (1 at 1100 Hz, 0 at 1300 Hz), the parity bit (even parity over the
   eight) and the stop bit (1200 Hz).
2. In the Scottie modes, the start pulse: 9 ms of 1200 Hz.
3. The lines, as the receiver reads them. A level v from 0 to 255 is sent at 1500 + 800 v / 255 Hz; the sync is 1200 Hz. A
   scan of L ms with N pixels sends pixel i from i L / N to (i + 1) L / N ms after its start.

Martin, Scottie and Wraase send R, G and B as they are. Robot and PD send Y = 0.299 R + 0.587 G + 0.114 B, R-Y = 128 + (R - Y) /
1.4 and B-Y = 128 + (B - Y) / 1.78, the colour differences clamped to 0 to 255; the receiver uses the inverse. The chroma of
Robot 36 has 160 pixels, each the mean of a 2 x 2 group (R-Y on even lines, B-Y on odd ones); Robot 72 sends R-Y and B-Y of 160
pixels, each the mean of two columns of the line's row; PD averages each column over the two rows of the line.

There is no silence before or after the picture in the burst. Time on air of one picture, header included:

| Mode | Seconds |
|---|---|
| Robot 36 | 36.910 |
| Robot 72 | 72.910 |
| Martin M1 | 115.200 |
| Martin M2 | 58.970 |
| Scottie S1 | 110.543 |
| Scottie S2 | 72.008 |
| Scottie DX | 269.796 |
| PD50 | 50.594 |
| PD90 | 90.899 |
| PD120 | 127.013 |
| PD180 | 187.962 |
| Wraase SC2-180 | 182.932 |

### On air

The transmitter writes into the transmit sink as many samples in each pass as the pass brings in, at any session rate from 8
kHz up (the stack itself needs 12 kHz). On a live radio whose host gives a transmit lead, the first sample states its time on
air, as every sender of the library does: the time of the pass that writes it plus the lead and two passes. The last sample
carries the end mark. `stop`, transmit going off and a change of format end the picture at once: the next pass writes one silent
sample with the end mark.

Result fields: `tx_sending`, `tx_mode`, `tx_line` (the line on air, from 1; 0 during the header), `tx_lines`, `tx_picture_s`
(the time on air of a picture in the transmit mode, shown before `send`) and `tx_next_s` (the time to the next picture of a
repeat). The event `sstv.tx` says when a picture started (with the reason `repeat` for the pictures after the first),
finished (with the reason: stopped, the transmitter is off, the format changed) or was refused (with the reason).

### Checks

The tests send a 400 x 300 test picture through the stack with the `bars` fit and read it back with the stack's own receiver.
Mean absolute error against the picture as fitted, in levels of 0 to 255: Martin M2 3.4, Scottie S2 2.8 (2.6 without the start
pulse), Robot 36 6.1, PD50 2.8; the limit is 12. On the samples of a white picture in every mode at 48 kHz, each part of the first
two lines has its tone and each change of tone is at the time of the specification's tables to a sample; the VIS code and its
parity are read from the tones. Every time on air is the nominal one within 1 ms.

Robot 36 on LSB, read by the receiver on LSB: mean error 6.1, every phase step between -2300 and -1100 Hz. On NFM, read by the
receiver on NFM: mean error 6.2, a constant level, every phase step within 2500 Hz, the 99 % power bandwidth 8.9 kHz (Carson's
rule gives 9.6 kHz). AM refuses `send`. Repeat with a pause of 1 s: the second picture starts 1.0 s after the end of the first,
as its own burst with its end mark; `stop` in the pause ends the repeat.

### Settled and open points

- VIS code of PD180: 96. The public table of the mode description gives PD180 96 and PD240 97. The receiver already had 96;
  nothing was changed.
- Scottie start pulse: sent. The public mode description puts one 9 ms sync pulse of 1200 Hz before the first line of a Scottie
  picture; later lines have their sync between blue and red. The receiver reads a Scottie picture with or without the pulse.
- Not confirmed by a public document: the line timings of Wraase SC2-180, which are built as the specification states, and the
  divisors 1.4 and 1.78 of the colour differences, which are the specification's formula (the full-range form of ITU-R BT.601
  would give 1.402 and 1.772; the difference is under 0.5 of a level).
- Lower sideband, repeat and FM on 2 m: built. The sideband is the `demod` setting the receiver has, not a setting of its own.
  The FM deviation of 2500 Hz is a figure of the library, not of a public document.
- Not built: the VIS header off for sending, and a call sign or caption drawn on the picture (the host's work).
