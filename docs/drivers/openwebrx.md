<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# OpenWebRX receiver

An OpenWebRX server. The server sends demodulated audio and never IQ.

Driver key: `openwebrx`.

## Address

- `openwebrx:<host>`: port 8073.
- `openwebrx:<host>:<port>`
- `openwebrx:https://<host>/<path>`

Each open radio of an address is a receiver of its own.

## Radio settings

Every radio has these. Give one as `--set key=value`. If the stack has a setting with the same key, write `radio.key=value`.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `centre_hz` | Centre frequency | frequency | 100 MHz | 0 Hz to 1000 GHz |  |
| `gain` | Gain | number (dB) | 30 | -100 to 200 |  |
| `agc` | Automatic gain | yes or no | true |  |  |
| `rate` | Sample rate | frequency | 0 Hz | 0 Hz to 1 GHz | The rate of the samples from the radio. On Automatic the radio takes the least rate at or above what the stack needs; a rate that is set stays, and a stack that needs more is refused. Write 0 to return to Automatic. |
| `bandwidth` | Bandwidth | frequency | 0 Hz | 0 Hz to 1 GHz | The width of the analogue filter of the radio. Without a width, the driver chooses one that fits the sample rate. |
| `rate_exact` | Exact rate | yes or no | false |  | Sends the rate as it is, even outside the rates the driver picks. |
| `bias_tee` | Bias tee | yes or no | false |  |  |

## Driver settings

Only this driver has these.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `passband` | Passband | frequency | 0 Hz | 0 Hz to 200 kHz | The passband width of the demodulator of the server. 0 takes the width of the mode. |
| `delivery` | Delivery | choice | `iq` | `iq` (IQ for sdrkit's decoders), `audio` (Server demodulates) | IQ for sdrkit's decoders receives 5.4 kHz of spectrum from the USB audio of the server, so the SSB, CW and data decoders of sdrkit run on it. The AGC of the server sets the level. Server demodulates gives the FM, AM, SSB or digital voice audio of the server. |
| `magic_key` | Magic key | text | empty |  | The key some servers ask for before a listener can move the centre frequency. |
| `user` | HTTP user (proxy login) | text | empty |  | Only for a receiver behind a password-protected web proxy. |
| `password` | HTTP password (proxy login) | text | empty |  | Only for a receiver behind a password-protected web proxy. Over ws:// or http:// the password goes unencrypted. |
| `squelch` | Server squelch | number (dB) | -150 | -150 to 0 | The server mutes its audio below this level. -150 is off. |
| `secondary_offset` | Decoder audio frequency | frequency | 0 Hz | 0 Hz to 6 kHz | The audio frequency of a single-signal decoder (BPSK31, RTTY). |
| `dmr_slot` | DMR time slot | choice | `3` | `1` (Slot 1), `2` (Slot 2), `3` (Both) | The DMR time slots that the server decodes. |
| `move_centre` | Move the server's centre frequency | yes or no | false |  | Retunes the shared radio for every listener, on servers that allow it. Some need the magic key the operator gave you. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Real | I16 | 12 kHz to 48 kHz | 1 |

## Readings

| Key | Label | Kind |
|---|---|---|
| `state` | Link | choice |
| `rate_hz` | Sample rate | frequency |
| `centre_hz` | Centre | frequency |
| `samples` | Samples | whole number |
| `dropped` | Dropped | whole number |
| `buffer_pct` | Buffer | number (%) |
| `radio_lost` | Lost by the radio | whole number |
| `link_down` | Lost while away | whole number |
| `delivered_pct` | Delivered | number (%) |
| `problem` | Problem | text |
| `server` | Server | text |
| `profile_id` | Profile id | text |
| `profile_name` | Profile | text |
| `band_low_hz` | Band from | frequency |
| `band_high_hz` | Band to | frequency |
| `listeners` | Listeners | whole number |
| `max_listeners` | Most listeners | whole number |
| `smeter_db` | S-meter | number (dB) |
| `cpu_pct` | Server CPU | number (%) |
| `temperature_c` | Temperature | number (°C) |
| `receiver` | Receiver | text |
| `location` | Location | text |
| `height_m` | Height | number (m) |
| `admin` | Admin | text |
| `decoder_line` | Server decoder | text |
| `server_error` | Server error | text |
| `kbit_s` | Link speed | number (kbit/s) |
| `audio_sps` | Audio | number (samples/s) |
| `lines_s` | Waterfall | number (lines/s) |
| `bytes_in` | Bytes received | whole number |
| `audio_adpcm` | Audio ADPCM | yes or no |
| `audio_sync` | Audio sync headers | yes or no |
| `waterfall_adpcm` | Waterfall ADPCM | yes or no |
| `fft_bins` | Waterfall bins | whole number |
| `waterfall_min_db` | Waterfall from | number (dB) |
| `waterfall_max_db` | Waterfall to | number (dB) |
| `resync_lost` | Lost to resyncs | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `bookmarks` | Bookmarks | `name` (text), `freq_hz` (frequency), `mode` (text), `tune_hz` (frequency) |
| `dial_frequencies` | Dial frequencies | `mode` (text), `freq_hz` (frequency) |
| `decodes` | Server decodes | `time` (time), `mode` (text), `freq` (number), `level_db` (number (dB)), `offset` (duration (s)), `message` (text), `rest` (text) |
| `profiles` | Profiles | `id` (text), `name` (text), `low_hz` (frequency), `high_hz` (frequency) |
| `modes` | Modes | `id` (text), `name` (text), `decoder` (yes or no), `underlying` (text), `available` (yes or no) |
| `metadata` | Digital voice | `name` (text), `value` (text) |

## Result pictures

| Key | Label | X unit | Y unit | Cell unit |
|---|---|---|---|---|
| `waterfall` | Waterfall | Hz | line | dB |
| `decoder_waterfall` | Decoder spectrum | Hz | line | dB |

## Map points

The stack puts points on a map: Receiver.

## Events

| Kind | Label | Fields |
|---|---|---|
| `radio.connected` | Radio connected | `source` (text), `text` (text), `family` (choice) |
| `radio.lost` | Radio lost | `source` (text), `text` (text), `family` (choice) |
| `radio.overflow` | Samples dropped | `source` (text), `text` (text), `family` (choice) |
| `radio.restart` | Restart needed | `source` (text), `text` (text), `family` (choice) |

## Commands of an open radio

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `reconnect` | Reconnect |  |  |  |

## Commands of the driver

They work before any radio is open. Run one with `sdrkit call kind:driver:openwebrx <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `address` | Build an address | `host` (text, required), `port` (whole number, default 8073), `path` (text, default empty), `tls` (yes or no, default false) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Try it

```sh
sdrkit run fm --source openwebrx:sdr.example.org:8073
```
<!-- generated: end -->

## Readings

| Key | Label | Kind |
|---|---|---|
| `state` | Link | choice |
| `rate_hz` | Sample rate | frequency |
| `centre_hz` | Centre | frequency |
| `samples` | Samples | whole number |
| `dropped` | Dropped | whole number |
| `buffer_pct` | Buffer | number (%) |
| `radio_lost` | Lost by the radio | whole number |
| `link_down` | Lost while away | whole number |
| `delivered_pct` | Delivered | number (%) |
| `problem` | Problem | text |
| `server` | Server | text |
| `profile_id` | Profile id | text |
| `profile_name` | Profile | text |
| `band_low_hz` | Band from | frequency |
| `band_high_hz` | Band to | frequency |
| `listeners` | Listeners | whole number |
| `max_listeners` | Most listeners | whole number |
| `smeter_db` | S-meter | number (dB) |
| `cpu_pct` | Server CPU | number (%) |
| `temperature_c` | Temperature | number (°C) |
| `receiver` | Receiver | text |
| `location` | Location | text |
| `height_m` | Height | number (m) |
| `admin` | Admin | text |
| `decoder_line` | Server decoder | text |
| `server_error` | Server error | text |
| `kbit_s` | Link speed | number (kbit/s) |
| `audio_sps` | Audio | number (samples/s) |
| `lines_s` | Waterfall | number (lines/s) |
| `bytes_in` | Bytes received | whole number |
| `audio_adpcm` | Audio ADPCM | yes or no |
| `audio_sync` | Audio sync headers | yes or no |
| `waterfall_adpcm` | Waterfall ADPCM | yes or no |
| `fft_bins` | Waterfall bins | whole number |
| `waterfall_min_db` | Waterfall from | number (dB) |
| `waterfall_max_db` | Waterfall to | number (dB) |
| `resync_lost` | Lost to resyncs | whole number |
