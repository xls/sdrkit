<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# RC links

2.4 GHz radio-control links from bursts anywhere in the capture: FrSky, FlySky, Futaba S-FHSS and Spektrum DSM transmitter ids and channel values. Hops are not followed.

Stack id: `rc_links`. Group: Aircraft and drones.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 2.44 GHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 8 MHz |
| Smallest sample rate | 1 MHz |
| Signal bandwidth | 8 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:rc_links` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `submode` | Link | choice | `all` | `all` (All), `frsky` (FrSky), `flysky` (FlySky), `sfhss` (Futaba S-FHSS), `dsm` (Spektrum DSM), `ppm72` (RC model PPM (26 to 75 MHz)) | The links searched. All runs each link whose least sample rate the input reaches. |
| `threshold_db` | Burst threshold | number (dB) | 9 | 6 to 20 | How far above the noise floor of a frequency bin a burst must rise. |
| `flysky_bit_order` | FlySky bit order | choice | `both` | `both` (Both), `msb` (Most significant first), `lsb` (Least significant first) | The bit order of FlySky bytes on the air. No public document gives it; both are tried by default. |
| `dsm_chip_polarity` | DSM chip polarity | choice | `high` | `high` (1 chip on the higher frequency), `low` (1 chip on the lower frequency) | Which frequency a 1 chip of DSM is sent on. No public document gives it. |
| `sfhss_crc` | S-FHSS CRC | choice | `either` | `either` (Either), `ccitt` (CRC-16/IBM-3740), `cc2500` (CRC-16/CMS) | The CRC of S-FHSS packets. No public document gives it; both are tried by default and the one that matched is shown. |
| `ppm_sync_ms` | Shortest sync gap (PPM) | number (ms) | 3.5 | 2 to 10 |  |
| `ppm_channel_min_ms` | Shortest channel (PPM) | number (ms) | 0.55 | 0.3 to 5 |  |
| `ppm_channel_max_ms` | Longest channel (PPM) | number (ms) | 2.7 | 0.3 to 5 |  |
| `ppm_centre_ms` | Centre of travel (PPM) | number (ms) | 1.5 | 1 to 2 | A channel this long is 0 %; 0.5 ms less is -100 % and 0.5 ms more +100 %. |
| `ppm_row_s` | Row interval (PPM) | number (s) | 0.5 | 0.1 to 10 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_channels_ms` | PPM channels | text (ms) | `1.5, 1.5, 1.5, 1.5` |  | The channel times of the PPM test signal, separated by commas; empty for a steady carrier. |
| `gen_frame_ms` | PPM frame | number (ms) | 20 | 5 to 50 |  |
| `gen_pulse_us` | PPM pulse | number (us) | 350 | 50 to 1000 |  |
| `gen_jitter_us` | PPM timing jitter | number (us) | 0 | 0 to 100 |  |
| `gen_ppm_clock_ppm` | PPM transmitter clock error | number (ppm) | 0 | -200000 to 200000 |  |
| `gen_ppm_depth` | PPM keying depth | number | 1 | 0.1 to 1 |  |
| `gen_ppm_invert` | PPM carrier only in the pulse | yes or no | false |  | Off: the pulse is a notch in a steady carrier. On: the carrier is on only during the pulse. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `frames_ok` | Frames good | whole number |
| `bursts` | Bursts | whole number |
| `bursts_without_frame` | Bursts without a frame | whole number |
| `state` | State | whole number |
| `links_heard` | Links | whole number |
| `covered` | Searched | text |
| `snr_db` | SNR | number (dB) |
| `note` | Note | text |
| `dropped` | Dropped | whole number |
| `channels` | PPM channels | whole number |
| `frame_ms` | PPM frame | number (ms) |
| `frame_rate_hz` | PPM frame rate | number (Hz) |
| `pulse_state` | PPM pulse state | choice |
| `channel_ms` | PPM channel times | text (ms) |
| `channel_pct` | PPM channels in travel | text (%) |
| `ppm_frames_ok` | PPM frames good | whole number |
| `ppm_frames_bad` | PPM frames bad | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `links` | Links | `link` (text), `id` (text), `frames` (whole number), `first_heard` (time), `last_heard` (time), `last_type` (text), `last_detail` (text), `position` (position), `alt_m` (number (m)) |
| `frames` | Frames | `utc` (time), `time_s` (number (s)), `link` (text), `id` (text), `type` (text), `detail` (text), `hex` (text), `freq_hz` (number (Hz)), `snr_db` (number (dB)), `encrypted` (yes or no) |
| `ppm_frames` | PPM frames | `time_s` (duration (s)), `channels` (whole number), `values_ms` (text (ms)), `frame_ms` (number (ms)), `level_db` (number (dB)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `rc_links.link` | New radio-control link | `source` (text), `link` (text), `family` (choice) |
| `rc.ppm_frame` | PPM frame | `time_s` (duration (s)), `channels` (whole number), `values_ms` (text (ms)), `frame_ms` (number (ms)), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |

## Typed interface

The stack is in the family `drones`, version 1.3. The C header `sdrkit/drones.h`, the registry record `registry/drones.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Networks and cells (`networks`). Media: none.

### Result fields

Struct `LinkFields`. The result fields of the link stacks.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `frames_ok` | whole number | always | 1.0 | Frames (packets, blocks, bursts) that passed their check. |
| `bursts` | whole number | always | 1.0 | Bursts analysed. |
| `bursts_without_frame` | whole number | always | 1.0 | Bursts without a frame. |
| `state` | whole number | always | 1.0 | 0 nothing, 1 bursts seen, 2 frames decoded lately. |
| `links_heard` | whole number | always | 1.0 | Links in the table. |
| `covered` | text | always | 1.0 | The span of the capture that is searched. |
| `snr_db` | number (dB) | optional | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Result fields

Struct `RcLinksPpmFields`. The PPM fields of `rc_links` (the submode `ppm72`). The measurements have no value in the other submodes and before a good frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `channels` | whole number | optional | 1.0 | The channels of the last good frame. |
| `frame_ms` | number (ms) | optional | 1.0 | The last good frame: the sync and the channels. |
| `frame_rate_hz` | number (Hz) | optional | 1.0 | The frames per second. |
| `pulse_state` | choice | optional | 1.0 | The carrier state of the pulses. |
| `channel_ms` | text (ms) | optional | 1.0 | The channel times of the last good frame in ms, separated by commas. |
| `channel_pct` | text (%) | optional | 1.0 | The channels in percent of travel, separated by commas: (ms - centre) x 200. |
| `ppm_frames_ok` | whole number | always | 1.0 | The good frames: as many channels as the frame before. |
| `ppm_frames_bad` | whole number | always | 1.0 | The bad frames: a channel out of its window, or 1 or more than 12 channels. |

### Table `links`

Struct `HeardLinkRow`. A link heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `link` | text | always | 1.0 | The link name. |
| `id` | text | always | 1.0 | The transmitter id. |
| `frames` | whole number | always | 1.0 | Frames of a link. |
| `first_heard` | time | optional | 1.0 | Wall-clock time when first heard. |
| `last_heard` | time | optional | 1.0 | Wall-clock time when last heard, in whole seconds. A host shows the age from it. |
| `last_type` | text | always | 1.0 | The type of the last frame of a link. |
| `last_detail` | text | always | 1.0 | The fields of the last frame of a link. |
| `position` | position | optional | 1.0 | The position of the object that the event is about. |
| `alt_m` | number (m) | optional | 1.0 | The altitude of the last position. |

The column `id` names a row. Place of a row: the column `position`. Height: `alt_m`.

### Table `frames`

Struct `LinkFrameRow`. A frame in the frame log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc` | time | optional | 1.0 | The wall time of a frame. |
| `time_s` | number (s) | always | 1.0 | The signal time of a frame. |
| `link` | text | always | 1.0 | The link name. |
| `id` | text | always | 1.0 | The transmitter id. |
| `type` | text | always | 1.0 | What a frame is. |
| `detail` | text | always | 1.0 | The fields of a frame. |
| `hex` | text | always | 1.0 | The first bytes of a frame as hex. |
| `freq_hz` | number (Hz) | always | 1.0 | The frequency of a frame. |
| `snr_db` | number (dB) | always | 1.0 | Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). |
| `encrypted` | yes or no | always | 1.0 | The payload is encrypted. |

The column `hex` names a row.

### Table `ppm_frames`

Struct `RcLinksPpmFrameRow`. A row of the PPM frame table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the sync that ended the frame. |
| `channels` | whole number | always | 1.0 | The channels. |
| `values_ms` | text (ms) | always | 1.0 | The channel times in ms, separated by commas. |
| `frame_ms` | number (ms) | always | 1.0 | The frame: the sync and the channels. |
| `level_db` | number (dB) | always | 1.0 | The peak over the floor of the envelope, 20 log10. |

### Event `rc_links.link`

Struct `RcLinksLinkEvent`. A new radio-control link.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | always | 1.0 | The sender of a decoded message: a registration, an address, a call sign or a code. |
| `link` | text | always | 1.0 | The link name. |
| `family` | choice | always | 1.0 | What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. |

### Event `rc.ppm_frame`

Struct `RcPpmFrameEvent`. A good PPM frame.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | duration (s) | always | 1.0 | The signal time of the sync that ended the frame. |
| `channels` | whole number | always | 1.0 | The channels. |
| `values_ms` | text (ms) | always | 1.0 | The channel times in ms, separated by commas. |
| `frame_ms` | number (ms) | always | 1.0 | The frame: the sync and the channels. |
| `level_db` | number (dB) | always | 1.0 | The peak over the floor of the envelope, 20 log10. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_rc_links_clear` | none | none |
| `remove` | `sdrkit_rc_links_remove` | `sdrkit_drones_link_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `submode` | `sdrkit_rc_links_get_submode` | `sdrkit_rc_links_set_submode` |
| `threshold_db` | `sdrkit_rc_links_get_threshold_db` | `sdrkit_rc_links_set_threshold_db` |
| `flysky_bit_order` | `sdrkit_rc_links_get_flysky_bit_order` | `sdrkit_rc_links_set_flysky_bit_order` |
| `dsm_chip_polarity` | `sdrkit_rc_links_get_dsm_chip_polarity` | `sdrkit_rc_links_set_dsm_chip_polarity` |
| `sfhss_crc` | `sdrkit_rc_links_get_sfhss_crc` | `sdrkit_rc_links_set_sfhss_crc` |
| `ppm_sync_ms` | `sdrkit_rc_links_get_ppm_sync_ms` | `sdrkit_rc_links_set_ppm_sync_ms` |
| `ppm_channel_min_ms` | `sdrkit_rc_links_get_ppm_channel_min_ms` | `sdrkit_rc_links_set_ppm_channel_min_ms` |
| `ppm_channel_max_ms` | `sdrkit_rc_links_get_ppm_channel_max_ms` | `sdrkit_rc_links_set_ppm_channel_max_ms` |
| `ppm_centre_ms` | `sdrkit_rc_links_get_ppm_centre_ms` | `sdrkit_rc_links_set_ppm_centre_ms` |
| `ppm_row_s` | `sdrkit_rc_links_get_ppm_row_s` | `sdrkit_rc_links_set_ppm_row_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_channels_ms` | `sdrkit_rc_links_gen_get_channels_ms` | `sdrkit_rc_links_gen_set_channels_ms` |
| `gen_frame_ms` | `sdrkit_rc_links_gen_get_frame_ms` | `sdrkit_rc_links_gen_set_frame_ms` |
| `gen_pulse_us` | `sdrkit_rc_links_gen_get_pulse_us` | `sdrkit_rc_links_gen_set_pulse_us` |
| `gen_jitter_us` | `sdrkit_rc_links_gen_get_jitter_us` | `sdrkit_rc_links_gen_set_jitter_us` |
| `gen_ppm_clock_ppm` | `sdrkit_rc_links_gen_get_ppm_clock_ppm` | `sdrkit_rc_links_gen_set_ppm_clock_ppm` |
| `gen_ppm_depth` | `sdrkit_rc_links_gen_get_ppm_depth` | `sdrkit_rc_links_gen_set_ppm_depth` |
| `gen_ppm_invert` | `sdrkit_rc_links_gen_get_ppm_invert` | `sdrkit_rc_links_gen_set_ppm_invert` |

## Try it

```sh
sdrkit run rc_links --source synth:rc_links
sdrkit run rc_links --source iiod:192.168.2.1 --freq 2440
```
<!-- generated: end -->

## Description

The receiver finds bursts anywhere in the capture with short FFTs and a noise floor for each frequency bin; each run of lit bins
is a burst of its own, so transmitters that send at the same time on other frequencies are kept apart. For each link and rate
it mixes a burst to its centre, filters and decimates it to 8 samples a bit, and searches the discriminator for the air
pattern of the link at steps of 1/8 bit with at most one bit error. A burst decoded by one link is not offered to the others.

| Link | Rate | Air pattern | Check | Shown |
|---|---|---|---|---|
| FrSky D8 | 31 044 Bd | AA AA D3 91 | CRC-16/CMS | id, packet counter, 8 channel values (raw 12-bit), bind packets |
| FrSky D16 | 70 000 and 99 970 Bd | AA AA D3 91 | the D16 checksum | id, hop index, channel skip, bind packets; the channel values are not unpacked |
| FlySky AFHDS | 500 kbit/s | AA AA 54 75 C5 2A | none on the air: the ID code and the value ranges | id, 8 channel values (µs), bind packets |
| FlySky AFHDS 2A | 500 kbit/s | AA AA 54 75 C5 2A | none on the air: the ID code and the value ranges | transmitter and receiver id, 14 channel values, failsafe, bind, telemetry |
| Futaba S-FHSS | 128 143 bit/s | AA AA D3 91 | CRC-16/IBM-3740 or CRC-16/CMS (the one that matched is shown) | id, 4 channel values, hop channel and code, packet of the pair |
| Spektrum DSM2 / DSMX | 1 Mchip/s, 64-chip codes | preamble, SOP code and its complement | the format of the data words; the CRC is not checked | two id bytes, channel numbers and values, pn row and SOP column |

DSMX and DSM2 are told apart by the pn row of the rf channel, which needs the radio centre; without it the type is "DSM". Each
link runs only from its least sample rate: FrSky and S-FHSS 1 Msps, FlySky 2.5 Msps, DSM 4 Msps. The links that the rate drops
are named in the `covered` field.

Not confirmed by a public document: no public document gives any layout of these links. The FlySky bit order (both are tried
unless `flysky_bit_order` says one), which S-FHSS CRC is right (`sfhss_crc`), the DSM chip polarity (`dsm_chip_polarity`, by
default a 1 chip the higher frequency) and the DSM CRC, the frequency of an S-FHSS hop channel, and the FrSky D16 channel values. Not tried on a real
signal.

## Tuning: notes

The stack holds no band or channel list: the bands per region are in `data/plans/drone_links.json`, for the host to offer.

## Result fields: notes

`state` is 0 before any burst, 1 when bursts were seen and 2 when a frame was decoded in the last 5 s of signal.
