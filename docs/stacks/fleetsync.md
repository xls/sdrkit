<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# FleetSync

The signalling bursts of FleetSync radios on analogue FM voice channels: every FM carrier inside the capture is followed, and the fleet and unit of each burst are listed with the calls. An identity reader: the other bits are shown raw.

Stack id: `fleetsync`. Group: Trunking.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 30 MHz to 960 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 1 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:fleetsync` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `raster_step_hz` | Raster step | frequency | 0 Hz | 0 Hz to 200 kHz | The spacing of the frequencies a carrier is searched on. 0 takes the narrowest channel of the plan, and without a plan the step that the stack or its submode fixes. |
| `raster_origin_hz` | Raster origin | frequency | 0 Hz | 0 Hz to 300 GHz | A frequency the raster passes through. 0 takes the first channel of the plan, and without a plan the multiples of the step from 0 Hz. |
| `channel_width_hz` | Channel width | frequency | 0 Hz | 0 Hz to 50 kHz | The width of a channel that no plan describes. 0 takes the width that the stack or its submode fixes. |
| `squelch` | Squelch | number (%) | 50 | 0 to 100 | 0 opens at the weakest signal. |
| `deemphasis` | De-emphasis | yes or no | true |  |  |
| `carriers` | Carriers at once | whole number | 10 | 1 to 24 |  |
| `bit_rate` | Signalling rate | choice | `auto` | `auto` (Automatic), `1200` (1200 bit/s), `2400` (2400 bit/s) | Automatic reads 1200 and 2400 bit/s at once. The tones at 2400 bit/s (1200 and 2400 Hz) are not confirmed by a public document. |
| `time_base` | Time of the first sample | time | 0 s | 0 to 100000000000 | UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_cnr_db` | Test signal C/N | number (dB) | 30 | 0 to 80 | The carrier-to-noise ratio of the test signal in 10 kHz. |
| `gen_fleet` | Test signal fleet | whole number | 107 | 99 to 354 | The maker's manuals allow 100 to 349; the frame carries 99 to 354. |
| `gen_unit` | Test signal unit | whole number | 1772 | 999 to 5094 | The unit of the first carrier; the second carrier sends the next unit. The maker's manuals allow 1000 to 4999; the frame carries 999 to 5094. |
| `gen_bit_rate` | Test signal rate | choice | `1200` | `1200` (1200 bit/s), `2400` (2400 bit/s) |  |
| `gen_frame` | Test signal frames | choice | `both` | `both` (Both), `plain` (FleetSync), `fs2` (FleetSync II) |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `submode` | Service | choice |
| `plan` | Channel plan | text |
| `plan_channels` | Plan channels | whole number |
| `raster_step_hz` | Raster step | frequency |
| `carriers` | Carriers | whole number |
| `active_channels` | Active channels | whole number |
| `open_channels` | Open channels | whole number |
| `playing` | Playing | text |
| `playing_hz` | Playing frequency | frequency |
| `scan` | Scan | yes or no |
| `scan_state` | Scan state | choice |
| `single_tuner` | Scan steps the radio | yes or no |
| `tune_hz` | Radio on | frequency |
| `multi_channel` | Whole span | yes or no |
| `calls` | Calls | whole number |
| `input_rate_hz` | Input rate | frequency |
| `centre_hz` | Centre | frequency |
| `signal_time_s` | Signal time | duration (s) |
| `reports` | Reports | whole number |
| `bursts` | Bursts | whole number |
| `check_failures` | Check failures | whole number |
| `units` | Units | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `calls` | Calls | `serial` (whole number), `time_utc` (time), `key` (whole number), `number` (whole number), `label` (text), `hz` (frequency), `start_s` (duration (s)), `duration_s` (duration (s)), `peak_dbfs` (number (dBFS)), `mean_dbfs` (number (dBFS)), `demod` (text), `tone` (text), `open` (yes or no) |
| `channels` | Channels | `key` (whole number), `number` (whole number), `label` (text), `name` (text), `hz` (frequency), `input_hz` (frequency), `active` (yes or no), `open` (yes or no), `playing` (yes or no), `in_scan` (yes or no), `on_air` (yes or no), `level_dbfs` (number (dBFS)), `demod` (choice), `tone` (text), `tone_alias` (text), `last_tone` (text), `calls` (whole number), `total_s` (duration (s)), `last_heard_s` (duration (s)), `last_heard_utc` (time), `peak_dbfs` (number (dBFS)) |
| `bursts` | Bursts | `serial` (whole number), `time_utc` (time), `hz` (frequency), `label` (text), `fleet` (whole number), `unit` (whole number), `in_range` (yes or no), `kind` (text), `bit_rate` (whole number (bit/s)), `word1` (text), `word2` (text), `raw` (text), `check` (text), `text` (text) |
| `units` | Units | `fleet` (whole number), `unit` (whole number), `in_range` (yes or no), `heard` (whole number), `last_heard` (time), `hz` (frequency), `kind` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `fleetsync.call` | Call | `serial` (whole number), `key` (whole number), `number` (whole number), `label` (text), `start_s` (duration (s)), `duration_s` (duration (s)), `peak_dbfs` (number (dBFS)), `mean_dbfs` (number (dBFS)), `demod` (text), `tone` (text) |
| `fleetsync.burst` | Burst | `serial` (whole number), `time_utc` (time), `hz` (frequency), `label` (text), `fleet` (whole number), `unit` (whole number), `in_range` (yes or no), `kind` (text), `bit_rate` (whole number (bit/s)), `word1` (text), `word2` (text), `raw` (text), `check` (text), `text` (text) |
| `tones.subaudio` | CTCSS or DCS | `family` (choice), `text` (text), `kind` (choice), `event` (choice), `tone_hz` (frequency), `eia` (yes or no), `motorola` (text), `measured_hz` (frequency), `code` (text), `standard` (yes or no), `alias` (text), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `voice`, version 1.1. The C header `sdrkit/voice.h`, the registry record `registry/voice.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `VoiceChannelsFields`. The result fields of the multi-carrier receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `submode` | choice | optional | 1.0 | The service. |
| `plan` | text | optional | 1.0 | The name of the plan in use. Absent without a plan. |
| `plan_channels` | whole number | always | 1.0 | The channels of the plan in use. |
| `raster_step_hz` | frequency | always | 1.0 | The raster step in use. |
| `carriers` | whole number | always | 1.0 | Carriers followed now. |
| `active_channels` | whole number | always | 1.0 | Channels with a carrier now. |
| `open_channels` | whole number | always | 1.0 | Channels with an open squelch. |
| `playing` | text | optional | 1.0 | The channel that plays. |
| `playing_hz` | frequency | optional | 1.0 | The frequency that plays. |
| `scan` | yes or no | always | 1.0 | The scan is on. |
| `scan_state` | choice | always | 1.0 | What the player does. |
| `single_tuner` | yes or no | always | 1.0 | The scan steps the radio. |
| `tune_hz` | frequency | optional | 1.0 | Where the stepping scan requires the radio. |
| `multi_channel` | yes or no | always | 1.0 | The capture holds more than one channel. |
| `calls` | whole number | always | 1.0 | Calls since the start. |
| `input_rate_hz` | frequency | always | 1.0 | The input rate. |
| `centre_hz` | frequency | always | 1.0 | The centre of the capture. |
| `signal_time_s` | duration (s) | always | 1.0 | The signal time. |
| `reports` | whole number | always | 1.0 | Reports made. |

### Result fields

Struct `FleetsyncFields`. The result fields of the FleetSync burst decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bursts` | whole number | always | 1.1 | Bursts so far. |
| `check_failures` | whole number | always | 1.1 | Frames found by their sync word whose check failed. |
| `units` | whole number | always | 1.1 | Fleet and unit pairs heard. |

### Table `calls`

Struct `CallRow`. A call in the call log.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `serial` | whole number | always | 1.0 | The number of a call. |
| `time_utc` | time | optional | 1.0 | When a call started. |
| `key` | whole number | always | 1.0 | The channel key. |
| `number` | whole number | always | 1.0 | The number of the plan. |
| `label` | text | always | 1.0 | The short label. |
| `hz` | frequency | always | 1.0 | Where to listen. |
| `start_s` | duration (s) | always | 1.0 | The signal time a call started. |
| `duration_s` | duration (s) | always | 1.0 | How long a call lasted. |
| `peak_dbfs` | number (dBFS) | always | 1.0 | The highest level. |
| `mean_dbfs` | number (dBFS) | always | 1.0 | The mean level of a call. |
| `demod` | text | always | 1.0 | The demodulator of a call. |
| `tone` | text | always | 1.0 | The tone or code heard. |
| `open` | yes or no | always | 1.0 | The squelch is open. |

### Table `channels`

Struct `ChannelRow`. A channel of the plan or a carrier heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `key` | whole number | always | 1.0 | The channel key. |
| `number` | whole number | always | 1.0 | The number of the plan. |
| `label` | text | always | 1.0 | The short label. |
| `name` | text | always | 1.0 | The use of the channel. |
| `hz` | frequency | always | 1.0 | Where to listen. |
| `input_hz` | frequency | optional | 1.0 | The repeater input. |
| `active` | yes or no | always | 1.0 | A carrier is there. |
| `open` | yes or no | always | 1.0 | The squelch is open. |
| `playing` | yes or no | always | 1.0 | The channel plays. |
| `in_scan` | yes or no | always | 1.0 | The channel is in the scan list. |
| `on_air` | yes or no | always | 1.0 | The schedule says the channel broadcasts now. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The channel level. |
| `demod` | choice | always | 1.0 | The demodulator in use. |
| `tone` | text | always | 1.0 | The tone or code heard. |
| `tone_alias` | text | always | 1.0 | The other reading of a DCS word. |
| `last_tone` | text | always | 1.0 | The last tone or code heard on the channel. |
| `calls` | whole number | always | 1.0 | Calls on the channel. |
| `total_s` | duration (s) | always | 1.0 | The time the channel was open in all. |
| `last_heard_s` | duration (s) | optional | 1.0 | The time since the channel was last open. |
| `last_heard_utc` | time | optional | 1.0 | When the channel was last open. |
| `peak_dbfs` | number (dBFS) | optional | 1.0 | The highest level. |

### Table `bursts`

Struct `FleetsyncBurstRow`. A FleetSync burst: a row of the burst log and the fields of its event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `serial` | whole number | always | 1.1 | The number of the burst. |
| `time_utc` | time | optional | 1.1 | When it was decoded. |
| `hz` | frequency | always | 1.1 | The frequency of the carrier. |
| `label` | text | always | 1.1 | The frequency in MHz, 4 decimals. |
| `fleet` | whole number | always | 1.1 | The fleet number. |
| `unit` | whole number | always | 1.1 | The unit number. |
| `in_range` | yes or no | always | 1.1 | The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals. |
| `kind` | text | always | 1.1 | "FleetSync" or "FleetSync II". |
| `bit_rate` | whole number (bit/s) | always | 1.1 | The signalling rate. |
| `word1` | text | always | 1.1 | Word 1, 8 hex digits. |
| `word2` | text | always | 1.1 | Word 2, 8 hex digits. |
| `raw` | text | always | 1.1 | Word 1 bits 31 to 16 and word 2 bits 27 to 16, not interpreted: 4 and 3 hex digits. |
| `check` | text | always | 1.1 | "good", or for FleetSync II the bits that the code repaired. |
| `text` | text | always | 1.1 | The burst in words. |

### Table `units`

Struct `FleetsyncUnitRow`. A fleet and unit heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `fleet` | whole number | always | 1.1 | The fleet number. |
| `unit` | whole number | always | 1.1 | The unit number. |
| `in_range` | yes or no | always | 1.1 | The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals. |
| `heard` | whole number | optional | 1.1 | Bursts heard. |
| `last_heard` | time | optional | 1.1 | When it was last heard. |
| `hz` | frequency | always | 1.1 | The carrier of its last burst. |
| `kind` | text | always | 1.1 | The type of its last burst. |

### Event `fleetsync.call`

Struct `VoiceChannelsCallEvent`. A call that ended.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `serial` | whole number | always | 1.0 | The number of a call. |
| `key` | whole number | always | 1.0 | The channel key. |
| `number` | whole number | always | 1.0 | The number of the plan. |
| `label` | text | always | 1.0 | The short label. |
| `start_s` | duration (s) | always | 1.0 | The signal time a call started. |
| `duration_s` | duration (s) | always | 1.0 | How long a call lasted. |
| `peak_dbfs` | number (dBFS) | always | 1.0 | The highest level. |
| `mean_dbfs` | number (dBFS) | always | 1.0 | The mean level of a call. |
| `demod` | text | always | 1.0 | The demodulator of a call. |
| `tone` | text | always | 1.0 | The tone or code heard. |

### Event `fleetsync.burst`

Struct `FleetsyncBurstRow`. A FleetSync burst: a row of the burst log and the fields of its event.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `serial` | whole number | always | 1.1 | The number of the burst. |
| `time_utc` | time | optional | 1.1 | When it was decoded. |
| `hz` | frequency | always | 1.1 | The frequency of the carrier. |
| `label` | text | always | 1.1 | The frequency in MHz, 4 decimals. |
| `fleet` | whole number | always | 1.1 | The fleet number. |
| `unit` | whole number | always | 1.1 | The unit number. |
| `in_range` | yes or no | always | 1.1 | The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals. |
| `kind` | text | always | 1.1 | "FleetSync" or "FleetSync II". |
| `bit_rate` | whole number (bit/s) | always | 1.1 | The signalling rate. |
| `word1` | text | always | 1.1 | Word 1, 8 hex digits. |
| `word2` | text | always | 1.1 | Word 2, 8 hex digits. |
| `raw` | text | always | 1.1 | Word 1 bits 31 to 16 and word 2 bits 27 to 16, not interpreted: 4 and 3 hex digits. |
| `check` | text | always | 1.1 | "good", or for FleetSync II the bits that the code repaired. |
| `text` | text | always | 1.1 | The burst in words. |

### Event `tones.subaudio`

Struct `SubAudioFields`. The fields of a `tones.subaudio` event: a change of a CTCSS tone or of a DCS code.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `text` | text | always | 1.0 | One line for people. |
| `kind` | choice | always | 1.0 | The signalling. |
| `event` | choice | always | 1.0 | The change. |
| `tone_hz` | frequency | optional | 1.0 | The CTCSS tone. |
| `eia` | yes or no | optional | 1.0 | The CTCSS tone is an EIA tone. |
| `motorola` | text | optional | 1.0 | The Motorola code of the CTCSS tone. |
| `measured_hz` | frequency | optional | 1.0 | The measured CTCSS tone. |
| `code` | text | optional | 1.0 | The DCS code with its polarity ("D023N"). |
| `standard` | yes or no | optional | 1.0 | The DCS code is a standard code. |
| `alias` | text | optional | 1.0 | A standard code that the same word also reads as. |
| `level_db` | number (dB) | always | 1.0 | The level of the tone, dB relative to a full-scale sine. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_fleetsync_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `raster_step_hz` | `sdrkit_fleetsync_get_raster_step_hz` | `sdrkit_fleetsync_set_raster_step_hz` |
| `raster_origin_hz` | `sdrkit_fleetsync_get_raster_origin_hz` | `sdrkit_fleetsync_set_raster_origin_hz` |
| `channel_width_hz` | `sdrkit_fleetsync_get_channel_width_hz` | `sdrkit_fleetsync_set_channel_width_hz` |
| `squelch` | `sdrkit_fleetsync_get_squelch` | `sdrkit_fleetsync_set_squelch` |
| `deemphasis` | `sdrkit_fleetsync_get_deemphasis` | `sdrkit_fleetsync_set_deemphasis` |
| `carriers` | `sdrkit_fleetsync_get_carriers` | `sdrkit_fleetsync_set_carriers` |
| `bit_rate` | `sdrkit_fleetsync_get_bit_rate` | `sdrkit_fleetsync_set_bit_rate` |
| `time_base` | `sdrkit_fleetsync_get_time_base` | `sdrkit_fleetsync_set_time_base` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_cnr_db` | `sdrkit_fleetsync_gen_get_cnr_db` | `sdrkit_fleetsync_gen_set_cnr_db` |
| `gen_fleet` | `sdrkit_fleetsync_gen_get_fleet` | `sdrkit_fleetsync_gen_set_fleet` |
| `gen_unit` | `sdrkit_fleetsync_gen_get_unit` | `sdrkit_fleetsync_gen_set_unit` |
| `gen_bit_rate` | `sdrkit_fleetsync_gen_get_bit_rate` | `sdrkit_fleetsync_gen_set_bit_rate` |
| `gen_frame` | `sdrkit_fleetsync_gen_get_frame` | `sdrkit_fleetsync_gen_set_frame` |

## Try it

```sh
sdrkit run fleetsync --source synth:fleetsync
sdrkit run fleetsync --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## How it works

The stack is the multi-carrier receiver of [Voice channels](voice_channels.md) with no channel plan, as [MDC-1200](mdc1200.md). A carrier finder searches a raster of 12.5 kHz through the multiples of the step; `raster_step_hz` and `raster_origin_hz` change it. Every carrier is a narrow FM carrier, labelled by its frequency in MHz. The strongest open carrier plays. The table `channels` holds the carriers and the table `calls` the transmissions.

Each carrier has a burst decoder. It takes the voice band of the discriminator, before de-emphasis, of the blocks that pass the squelch gate.

The stack is an identity reader. It decodes the fleet and unit numbers, the frame type and the block check. It shows the other bits of the two data words raw, in hexadecimal. It does not decode status numbers, short text, selective calls or positions: no public document gives their layout.

## The burst

A burst is FFSK audio tones in plain NRZ: bit 1 is 1200 Hz, bit 0 is 1800 Hz at 1200 bit/s. A burst with the tones swapped (an inverted discriminator) is read as well. A burst is alternating bits, the sync word A23E, 4 bits that are not read, and then the frame.

| Frame | Content |
|---|---|
| FleetSync | word 1 and word 2, 32 bits each. Word 2 bits 15 to 0 are the block check of the 48 bits before it |
| FleetSync II | four blocks of 64 bits, each 16 data bits as four (15,7) code words. One wrong bit in each code word is repaired, and the block check must then hold |

| Field | Bits | Shown |
|---|---|---|
| fleet | word 1 bits 15 to 8, plus 99 | `fleet`, 99 to 354 |
| unit | word 1 bits 7 to 0 and word 2 bits 31 to 28, plus 999 | `unit`, 999 to 5094 |
| not interpreted | word 1 bits 31 to 16, word 2 bits 27 to 16 | `raw`, 4 and 3 hex digits |
| both words | | `word1`, `word2`, 8 hex digits each |

The maker's manuals let a radio have a fleet of 100 to 349 and a unit of 1000 to 4999. A burst outside these ranges is listed with `in_range` false.

A frame whose check fails is not listed. It is counted in `check_failures`, except while the decoder of the other rate reads a burst: with automatic rate, a burst of one rate can hold a sync word at the other rate. The table `bursts` holds the bursts, newest first. The table `units` holds each fleet and unit pair with the number of bursts, the time of the last one, its carrier and its type.

## Signalling rate

`bit_rate` sets the rate the decoder reads: 1200 bit/s, 2400 bit/s, or both at once (automatic, the default). The maker's manuals name 1200 and 2400 bit/s. At 2400 bit/s the stack takes the tones 1200 Hz and 2400 Hz, the MSK spacing of half the bit rate.

## What no public document confirms

No public document gives the FleetSync frame. The stack follows its specification, and the status line says so. These items are not confirmed:

- the sync word, the preamble and the 4 bits after the sync word;
- the block check and its polynomial;
- the FleetSync II code, its block order and its slot order;
- the offsets of 99 and 999 of the fleet and unit fields;
- the tone pair and the tone sense; at 2400 bit/s the tone pair is the stack's own choice;
- the meaning of the raw bits;
- the time from the key of a radio's carrier to its burst. The test signal leaves 0.3 s.

The receiver loses a burst at the very start of a transmission when the noise keeps the squelch closed for its first part. At the noise of `sdrkit verify` this happens to a burst without carrier before it.

## Test signal

The test signal has two narrow FM transmissions with speech, 37.5 kHz below and 62.5 kHz above the centre. Each starts with 0.3 s of unmodulated carrier, so that the squelch opens before the burst, and then sends a burst: a FleetSync frame of `gen_fleet` and `gen_unit`, and a FleetSync II frame of the same fleet and the next unit. `gen_frame` sends one type on both, and `gen_bit_rate` sends at 2400 bit/s. `gen_cnr_db` sets the noise.

The stack does not transmit.
