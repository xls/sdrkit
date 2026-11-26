<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SDRplay radio (SDRconnect)

SDRconnect, the receiver program of SDRplay, through its WebSocket interface.

Driver key: `sdrconnect`.

## Address

- `sdrconnect:<host>`: port 5454.
- `sdrconnect:<host>:<port>`

The device has one receiver: one radio of an address is open at a time, and the sessions share it.

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
| `lna_state` | LNA state | whole number | -1 | 0 to 255 | The API of SDRconnect has one gain control, the LNA state. The IF gain and the AGC stay as SDRconnect has them. A state outside the range SDRconnect reports for the radio and its band is refused. -1 leaves the state as it is. |
| `rate_source` | Rate set by | choice | `stack` | `stack` (As the stack asks), `server` (Keep the rate of SDRconnect) | As the stack asks: sdrkit sets the sample rate of SDRconnect (SDRconnect 1.0.7 and later). Keep: sdrkit uses the rate SDRconnect runs, and resamples. |
| `tuner` | Tuner | choice | `primary` | `primary` (Primary (tuner 1)), `secondary` (Secondary (tuner 2)) | For an RSPduo in dual-tuner mode in SDRconnect: the tuner whose IQ to take. Other radios have the primary tuner only. The secondary tuner needs SDRconnect 1.0.8 or later. |
| `wss` | Secure WebSocket (wss) | yes or no | false |  | A WebSocket over TLS. SDRconnect Headless 1.0.10 and later offers it. SDRconnect with its window serves plain WebSockets only. |
| `tls_pin` | Server certificate (SHA-256) | text | empty |  | For wss with a self-signed certificate: the SHA-256 digest of the certificate of the server, 64 hexadecimal digits. Empty uses the trust of the system. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cs16 | 62.5 kHz to 10 MHz | 1 |

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
| `signal_power_db` | Signal power | number (dB) |
| `signal_snr_db` | Signal SNR | number (dB) |
| `overload` | Overload | yes or no |
| `radio_started` | Radio streaming | yes or no |
| `can_control` | Control | yes or no |
| `lna_state` | LNA state | whole number |
| `lna_min` | Lowest LNA state | whole number |
| `lna_max` | Highest LNA state | whole number |
| `antenna` | Antenna | text |
| `device` | Radio | text |
| `api_version` | API version | text |
| `server_centre_hz` | Centre in SDRconnect | frequency |
| `server_rate_hz` | Rate in SDRconnect | frequency |
| `mbit_s` | Link speed | number (Mbit/s) |
| `ignored` | Messages ignored | whole number |
| `malformed` | Messages malformed | whole number |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:sdrconnect <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `address` | Build an address | `host` (text, required), `port` (whole number, default 5454) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Try it

```sh
sdrkit run fm --source sdrconnect:192.168.1.30
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
| `signal_power_db` | Signal power | number (dB) |
| `signal_snr_db` | Signal SNR | number (dB) |
| `overload` | Overload | yes or no |
| `radio_started` | Radio streaming | yes or no |
| `can_control` | Control | yes or no |
| `lna_state` | LNA state | whole number |
| `lna_min` | Lowest LNA state | whole number |
| `lna_max` | Highest LNA state | whole number |
| `antenna` | Antenna | text |
| `device` | Radio | text |
| `api_version` | API version | text |
| `server_centre_hz` | Centre in SDRconnect | frequency |
| `server_rate_hz` | Rate in SDRconnect | frequency |
| `mbit_s` | Link speed | number (Mbit/s) |
| `ignored` | Messages ignored | whole number |
| `malformed` | Messages malformed | whole number |
