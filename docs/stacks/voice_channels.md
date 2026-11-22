<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Voice channels

Every voice carrier inside the capture at once: CB, FRS / GMRS, PMR446, amateur FM repeaters and HF voice, with tones, a call log and a scan. A plan file that the host loads names the channels.

Stack id: `voice_channels`. Group: Listening.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 2.8 MHz to 480 MHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 48 kHz |
| Signal bandwidth | 1 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `cb` | CB radio | 0 Hz |
| `frs` | FRS / GMRS | 0 Hz |
| `pmr446` | PMR446 analog | 0 Hz |
| `ham_repeaters` | Ham FM repeaters | 0 Hz |
| `hfvoice` | HFGCS / HF voice | 0 Hz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:voice_channels` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `plan` | Channel plan | text | empty |  | The path of a plan file in the host's storage, in a format that plans.import takes (docs/schema/plans.json). Empty for no plan: every carrier is then labelled by its frequency. A change forgets the carriers and the channel statistics. |
| `raster_step_hz` | Raster step | frequency | 0 Hz | 0 Hz to 200 kHz | The spacing of the frequencies a carrier is searched on. 0 takes the narrowest channel of the plan, and without a plan the step that the stack or its submode fixes. |
| `raster_origin_hz` | Raster origin | frequency | 0 Hz | 0 Hz to 300 GHz | A frequency the raster passes through. 0 takes the first channel of the plan, and without a plan the multiples of the step from 0 Hz. |
| `channel_width_hz` | Channel width | frequency | 0 Hz | 0 Hz to 50 kHz | The width of a channel that no plan describes. 0 takes the width that the stack or its submode fixes. |
| `channel` | Channel | text | `auto` |  | A channel label of the plan, or auto for the strongest open channel. A label that the plan does not have is as auto. |
| `demod` | Demodulator | choice | `auto` | `auto` (Automatic), `fm` (FM), `am` (AM), `usb` (USB), `lsb` (LSB) | Automatic uses the demodulator of the plan channel, else the one of the submode: FM, upper sideband, or for CB the one the signal shows. |
| `squelch` | Squelch | number (%) | 50 | 0 to 100 | 0 opens at the weakest signal. |
| `deemphasis` | De-emphasis | yes or no | true |  |  |
| `scan` | Scan | yes or no | false |  |  |
| `scan_channels` | Channels | text | empty |  | Channel labels of the plan. Empty scans every channel of the plan. Without a plan the scan has no channels. |
| `scan_dwell_s` | Dwell | duration (s) | 0.4 s | 0.1 to 5 |  |
| `scan_hang_s` | Hang | duration (s) | 2 s | 0 to 6 |  |
| `carriers` | Carriers at once | whole number | 10 | 1 to 24 |  |
| `show_heard` | Only heard | yes or no | false |  | Lists only the channels that are active now or had a call. |
| `time_base` | Time of the first sample | time | 0 s | 0 to 100000000000 | UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_cnr_db` | Test signal C/N | number (dB) | 30 | 0 to 80 | The carrier-to-noise ratio of the test signal in 10 kHz. |
| `gen_scene` | Test signal scene | choice | `auto` | `auto` (Automatic), `mixed` (Mixed), `am_fm_ssb` (AM, FM and SSB), `fm_tones` (FM with tones), `sideband` (Upper sideband) | The transmissions of the test signal, at fixed offsets from the centre of the capture. Automatic takes the scene of the submode, and the mixed scene when the generator is given no submode. |

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

## Result tables

| Key | Label | Columns |
|---|---|---|
| `calls` | Calls | `serial` (whole number), `time_utc` (time), `key` (whole number), `number` (whole number), `label` (text), `hz` (frequency), `start_s` (duration (s)), `duration_s` (duration (s)), `peak_dbfs` (number (dBFS)), `mean_dbfs` (number (dBFS)), `demod` (text), `tone` (text), `open` (yes or no) |
| `channels` | Channels | `key` (whole number), `number` (whole number), `label` (text), `name` (text), `hz` (frequency), `input_hz` (frequency), `active` (yes or no), `open` (yes or no), `playing` (yes or no), `in_scan` (yes or no), `on_air` (yes or no), `level_dbfs` (number (dBFS)), `demod` (choice), `tone` (text), `tone_alias` (text), `last_tone` (text), `calls` (whole number), `total_s` (duration (s)), `last_heard_s` (duration (s)), `last_heard_utc` (time), `peak_dbfs` (number (dBFS)) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `voice_channels.call` | Call | `serial` (whole number), `key` (whole number), `number` (whole number), `label` (text), `start_s` (duration (s)), `duration_s` (duration (s)), `peak_dbfs` (number (dBFS)), `mean_dbfs` (number (dBFS)), `demod` (text), `tone` (text) |
| `tones.subaudio` | CTCSS or DCS | `family` (choice), `text` (text), `kind` (choice), `event` (choice), `tone_hz` (frequency), `eia` (yes or no), `motorola` (text), `measured_hz` (frequency), `code` (text), `standard` (yes or no), `alias` (text), `level_db` (number (dB)) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |

## Typed interface

The stack is in the family `voice`, version 1.4. The C header `sdrkit/voice.h`, the registry record `registry/voice.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Voice calls (`voice`). Media: sound (voice).

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

The rows are the calls of the stack. The column `label` names a row. Time: `time_utc`.

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

The rows are the groups of the stack. The column `name` names a row. Time: `last_heard_utc`.

### Event `voice_channels.call`

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
| `clear` | `sdrkit_voice_channels_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `plan` | `sdrkit_voice_channels_get_plan` | `sdrkit_voice_channels_set_plan` |
| `raster_step_hz` | `sdrkit_voice_channels_get_raster_step_hz` | `sdrkit_voice_channels_set_raster_step_hz` |
| `raster_origin_hz` | `sdrkit_voice_channels_get_raster_origin_hz` | `sdrkit_voice_channels_set_raster_origin_hz` |
| `channel_width_hz` | `sdrkit_voice_channels_get_channel_width_hz` | `sdrkit_voice_channels_set_channel_width_hz` |
| `channel` | `sdrkit_voice_channels_get_channel` | `sdrkit_voice_channels_set_channel` |
| `demod` | `sdrkit_voice_channels_get_demod` | `sdrkit_voice_channels_set_demod` |
| `squelch` | `sdrkit_voice_channels_get_squelch` | `sdrkit_voice_channels_set_squelch` |
| `deemphasis` | `sdrkit_voice_channels_get_deemphasis` | `sdrkit_voice_channels_set_deemphasis` |
| `scan` | `sdrkit_voice_channels_get_scan` | `sdrkit_voice_channels_set_scan` |
| `scan_channels` | `sdrkit_voice_channels_get_scan_channels` | `sdrkit_voice_channels_set_scan_channels` |
| `scan_dwell_s` | `sdrkit_voice_channels_get_scan_dwell_s` | `sdrkit_voice_channels_set_scan_dwell_s` |
| `scan_hang_s` | `sdrkit_voice_channels_get_scan_hang_s` | `sdrkit_voice_channels_set_scan_hang_s` |
| `carriers` | `sdrkit_voice_channels_get_carriers` | `sdrkit_voice_channels_set_carriers` |
| `show_heard` | `sdrkit_voice_channels_get_show_heard` | `sdrkit_voice_channels_set_show_heard` |
| `time_base` | `sdrkit_voice_channels_get_time_base` | `sdrkit_voice_channels_set_time_base` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_cnr_db` | `sdrkit_voice_channels_gen_get_cnr_db` | `sdrkit_voice_channels_gen_set_cnr_db` |
| `gen_scene` | `sdrkit_voice_channels_gen_get_scene` | `sdrkit_voice_channels_gen_set_scene` |

## Try it

```sh
sdrkit run voice_channels --source synth:voice_channels
sdrkit run voice_channels --source synth:voice_channels --submode cb
sdrkit run voice_channels --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Channel plans

The stack holds no channel plan and no rule for a country. The host tunes the radio and names a plan file in the `plan` setting. The file is in the host's storage and has the format of `docs/schema/plans.json` (the format that `plans.import` takes). Without a plan the stack finds every carrier on the raster of the submode, labels it by its frequency, and its status says "no channel plan loaded".

The folder `crates/stacks/sdrkit-stack-voice/data` has example plan files. The library does not contain them.

| File | Plan | Channels |
|---|---|---|
| `cb_cept.json` | CB Europe (CEPT) | 40 |
| `cb_cept80.json` | CB Europe with the national channels 41 to 80 | 80 |
| `cb_uk.json` | CB United Kingdom, CEPT and 27/81 | 80 |
| `cb_us.json` | CB USA and Canada | 40 |
| `cb_au_27mhz.json` | CB Australia 27 MHz | 40 |
| `cb_au_uhf.json` | UHF CB Australia | 80 |
| `cb_nz_26mhz.json` | CB New Zealand 26 MHz | 40 |
| `cb_nz_uhf.json` | UHF CB New Zealand | 80 |
| `frs_gmrs_us.json` | FRS and GMRS | 22 |
| `pmr446_cept.json` | PMR446 analogue | 16 |
| `ham_repeaters_r1.json` | Amateur FM repeaters, IARU Region 1 | 50 |
| `ham_repeaters_r2.json` | Amateur FM repeaters, IARU Region 2 | 174 |
| `ham_repeaters_r3.json` | Amateur FM repeaters, IARU Region 3 | 95 |
| `hfvoice_world.json` | HF voice: HFGCS, VOLMET and relay stations | 20 |
| `microphones_eu.json` | Wireless microphone bands, Europe | 3 bands, no channels |
| `microphones_us.json` | Wireless microphone bands, USA | 4 bands, no channels |

What the stack reads from a plan file:

| In the file | Meaning |
|---|---|
| Band `modulation` | `nfm` (or `fm`), `am`, `usb`, `lsb`: the demodulator of the channels of the band. Empty: the demodulator of the submode. Any other word (`data`): the channels carry no speech; they are shown, never played and not scanned. |
| Channel `pair_hz` | The repeater input. |
| Channel `in_use: false` | The channel is shown and the scan skips it. |
| Channel `note` | The use of the channel. A note that ends in `; minutes 20-24, 50-54` gives the minutes of each hour that the channel is on the air. The format has no field for a schedule. |
| Channel `name` | The label. A label that is a number is the channel number. |

The raster of the carrier finder comes from the settings `raster_step_hz` and `raster_origin_hz`. At 0 the step is the narrowest channel of the plan and the origin is its first channel. In `hfvoice` the step stays 1 kHz. Without a plan the frequency of an `hfvoice` transmission is an estimate: 1 kHz below the first raster cell that holds its speech. The 27 MHz CB channels of most countries end in 5 kHz and lie between the cells of the default 10 kHz raster of `cb`: load a plan, or set `raster_origin_hz` to a channel frequency (for example `raster_origin_hz=26965000`). A plan whose channels do not lie on one raster (IARU Region 2: 15 kHz and 25 kHz grids) needs a step that fits all of them, for example `raster_step_hz=5000`.

What a submode fixes:

| Submode | Demodulator | Raster step without a plan | Channel width without a plan | Other |
|---|---|---|---|---|
| `cb` | From the signal: AM, FM, USB or LSB | 10 kHz | 10 kHz | The raster cell names the channel |
| `frs`, `pmr446`, `ham_repeaters` | FM, with CTCSS and DCS | 6.25 kHz | 12.5 kHz | These three differ only in the default frequency |
| `hfvoice` | USB | 1 kHz | 2.7 kHz | The scan always steps the radio; the squelch follows the carrier finder |

## Try it: notes

```sh
sdrkit run voice_channels --source synth:voice_channels
sdrkit run voice_channels --source synth:voice_channels --submode cb
sdrkit run voice_channels --source synth:voice_channels/frs --submode frs
sdrkit run voice_channels plan=plans/cb_us.json --storage <folder with plans/cb_us.json> --submode cb --source <driver>:<address> --freq 27.21
sdrkit run voice_channels --source <driver>:<address> --freq 27.21
```
