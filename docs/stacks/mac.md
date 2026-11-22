<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# MAC

D2-MAC and D-MAC satellite television: the picture, the digital sound and the service names.

Stack id: `mac`. Group: Video.

Signal types: MAC.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 50 MHz to 6 GHz |
| Sample rate | 20 MHz |
| Smallest sample rate | 16 MHz |
| Signal bandwidth | 27 MHz |
| Radio filter | 20 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:mac` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_variant` | Variant | choice | `d2` | `d2` (D2-MAC/packet), `d` (D-MAC/packet) |  |
| `gen_scrambled` | Scrambled (EuroCrypt) | yes or no | false |  |  |
| `gen_wide` | 16:9 | yes or no | false |  |  |
| `gen_picture` | Picture | choice | `card` | `card` (Test card), `bars` (Colour bars), `ramp` (Grey ramp) |  |
| `gen_tone` | Tone | frequency | 0 Hz | 0 Hz to 15 kHz |  |
| `gen_channel` | Channel name | text | `TEST TV` |  |  |
| `gen_network` | Network name | text | `TEST SATELLITE` |  |  |
| `gen_country` | Country | text | `TEST COUNTRY` |  |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `state` | State | choice |
| `system` | System | choice |
| `snr_db` | SNR | number (dB) |
| `video_snr_db` | Video SNR | number (dB) |
| `offset_hz` | Offset | frequency |
| `level_dbfs` | Level | number (dBFS) |
| `deviation_hz_per_v` | Deviation | number (Hz/V) |
| `spectrum_inverted` | Spectrum inverted | yes or no |
| `line_rate` | Line rate | frequency |
| `line_error_ppm` | Line rate error | number (ppm) |
| `line_sync_pct` | Line sync | number (%) |
| `frame_syncs` | Frame syncs | whole number |
| `frames` | Frames | whole number |
| `ber` | Bit error rate | number |
| `channel` | Channel | text |
| `network` | Network | text |
| `country` | Country | text |
| `channel_id` | Channel id | whole number |
| `aspect` | Aspect | choice |
| `scrambled` | Scrambled | yes or no |
| `access` | Access | text |
| `ca_system` | Access system | choice |
| `sound_errors` | Sound errors | whole number |
| `ecm_packets` | ECM packets | whole number |
| `packet_headers` | Packets | whole number |
| `packets_corrected` | Packets corrected | whole number |
| `packets_dummy` | Dummy packets | whole number |
| `si_groups_ok` | SI groups good | whole number |
| `si_groups_bad` | SI groups bad | whole number |
| `sound_address` | Sound address | whole number |
| `sound_present` | Sound carrier | yes or no |
| `sound_packets` | Sound packets | whole number |
| `sound_mode` | Sound coding | choice |
| `audio_level_dbfs` | Audio level | number (dBFS) |
| `blocks_bad` | Sound blocks bad | whole number |
| `pictures` | Pictures | whole number |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `eye` | Eye |  |  |

## Events

| Kind | Label | Fields |
|---|---|---|
| `mac.state` | State | `state` (choice), `text` (text), `family` (choice) |

## Typed interface

The stack is in the family `analogtv`, version 1.2. The C header `sdrkit/analogtv.h`, the registry record `registry/analogtv.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Television and video (`television`). Media: sound (programme), video.

### Result fields

Struct `MacFields`. The result fields of the MAC receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | The state of the receiver. |
| `system` | choice | optional | 1.0 | The MAC variant. Absent while the receiver searches. |
| `snr_db` | number (dB) | always | 1.0 | The signal-to-noise ratio of the data burst. |
| `video_snr_db` | number (dB) | always | 1.0 | The SNR of the picture. |
| `offset_hz` | frequency | always | 1.0 | The carrier offset. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the signal, dBFS. |
| `deviation_hz_per_v` | number (Hz/V) | always | 1.0 | The FM deviation, Hz a volt of the baseband. |
| `spectrum_inverted` | yes or no | always | 1.0 | The spectrum is inverted (and corrected). |
| `line_rate` | frequency | always | 1.0 | The line rate. |
| `line_error_ppm` | number (ppm) | always | 1.0 | The line rate against its nominal value. |
| `line_sync_pct` | number (%) | always | 1.0 | The share of lines whose sync word was found. |
| `frame_syncs` | whole number | always | 1.0 | Frame syncs found. |
| `frames` | whole number | always | 1.0 | Frames decoded. |
| `ber` | number | always | 1.0 | The bit error rate of the data burst. |
| `channel` | text | optional | 1.0 | The channel name. Absent until the service identification names it. |
| `network` | text | optional | 1.0 | The network name. |
| `country` | text | optional | 1.0 | The country. |
| `channel_id` | whole number | optional | 1.0 | The channel identifier. |
| `aspect` | choice | optional | 1.0 | The picture's aspect ratio. |
| `scrambled` | yes or no | always | 1.0 | The picture or sound is scrambled. |
| `access` | text | optional | 1.0 | The access conditions, in words. |
| `ca_system` | choice | optional | 1.0 | The conditional access system, when the channel names one. |
| `sound_errors` | whole number | always | 1.0 | Sound words whose protection failed. |
| `ecm_packets` | whole number | always | 1.0 | ECM packets received. |
| `packet_headers` | whole number | always | 1.0 | Packet headers decoded. |
| `packets_corrected` | whole number | always | 1.0 | Packet headers corrected. |
| `packets_dummy` | whole number | always | 1.0 | Dummy packets. |
| `si_groups_ok` | whole number | always | 1.0 | Service identification groups whose CRC passed. |
| `si_groups_bad` | whole number | always | 1.0 | Service identification groups whose CRC failed. |
| `sound_address` | whole number | optional | 1.0 | The packet address of the sound. |
| `sound_present` | yes or no | always | 1.0 | The sound is there and plays. |
| `sound_packets` | whole number | always | 1.0 | Sound packets received. |
| `sound_mode` | choice | always | 1.0 | How the sound is coded. |
| `audio_level_dbfs` | number (dBFS) | always | 1.0 | The audio level. |
| `blocks_bad` | whole number | always | 1.0 | Sound blocks whose words failed their parity. |
| `pictures` | whole number | always | 1.0 | Pictures sent. |

### Event `mac.state`

Struct `MacStateEvent`. A change of the state of the MAC receiver.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.0 | The new state. |
| `text` | text | always | 1.0 | The summary of the receiver. |
| `family` | choice | always | 1.0 | What the event is about: the receiver. |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_variant` | `sdrkit_mac_gen_get_variant` | `sdrkit_mac_gen_set_variant` |
| `gen_scrambled` | `sdrkit_mac_gen_get_scrambled` | `sdrkit_mac_gen_set_scrambled` |
| `gen_wide` | `sdrkit_mac_gen_get_wide` | `sdrkit_mac_gen_set_wide` |
| `gen_picture` | `sdrkit_mac_gen_get_picture` | `sdrkit_mac_gen_set_picture` |
| `gen_tone` | `sdrkit_mac_gen_get_tone` | `sdrkit_mac_gen_set_tone` |
| `gen_channel` | `sdrkit_mac_gen_get_channel` | `sdrkit_mac_gen_set_channel` |
| `gen_network` | `sdrkit_mac_gen_get_network` | `sdrkit_mac_gen_set_network` |
| `gen_country` | `sdrkit_mac_gen_get_country` | `sdrkit_mac_gen_set_country` |

## Try it

```sh
sdrkit run mac --source synth:mac
sdrkit run mac --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
