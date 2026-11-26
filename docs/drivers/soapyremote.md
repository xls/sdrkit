<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# SoapyRemote server

A SoapyRemote server, which shares the SoapySDR radios of one computer.

Driver key: `soapyremote`.

## Address

- `soapyremote:<host>`: port 55132.
- `soapyremote:<host>:<port>`

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
| `ppm` | Frequency correction | number (ppm) | 0 | -200 to 200 | The clock error in ppm, positive if the clock runs fast (signals show up a little low). |
| `antenna` | Antenna | text | empty |  | The antenna input of the radio. Empty keeps the radio's default. A name the radio does not list is refused. |
| `clocksrc` | Clock source | text | empty |  | Where the radio takes its clock from (internal, external, gpsdo). Empty keeps it as it is. A source the radio does not list is refused. |
| `timesrc` | Time source (PPS) | text | empty |  | Where the radio takes its one pulse per second from. Empty keeps it as it is. A source the radio does not list is refused. |
| `format` | Sample format | choice | `auto` | `auto` (Auto (the radio's own)), `cs16` (CS16 (16-bit)), `cs8` (CS8 (8-bit, half the data)), `cf32` (CF32 (float, twice the data)) | What the server sends. Auto is the radio's own format (no conversion on the server). CS8 halves the data rate, with less dynamic range. CF32 doubles it and gains nothing for most radios. 2.4 Msps of CS16 is about 79 Mbit/s with headers, CS8 about 40 Mbit/s. |
| `transport` | Stream transport | choice | `tcp` | `udp` (UDP), `tcp` (TCP) | TCP loses nothing: a packet that the network lost is sent again, which delays the stream for a moment. UDP, the protocol's own default, loses each packet that the network loses, and each loss is a hole in the signal: on Wi-Fi, crackle in the sound. Received TCP messages are at least 4096 bytes. |
| `mtu` | Datagram size | choice | `1500` | `1500` (Standard (1500)), `9000` (Jumbo frames (9000)) | 1500 works on every network. Use jumbo frames only when every switch and both computers are set for them. If not, no samples arrive. |
| `window` | Receive buffer | number (MiB) | 4 | 0.25 to 32 | How much the network can have in flight. More helps on Wi-Fi at high rates. |
| `remote` | Remote driver | text | empty |  | Which radio of the server to use: a driver name (rtlsdr) or a list (driver=sdrplay,serial=1). Empty: the first radio of the server. The server must run SoapySDRServer --bind. The protocol has no password: anyone on the network can use the radio. |
| `serverlog` | Show the server's log | yes or no | true |  | The messages of the server appear in the log, marked server:. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cs16 | 0 Hz to 179769313486231564793826746329356487460061771816610493194377291867566634083253736182911671780280864445928163680987122391750825462330354250895282439122322875506826024599142533926918074193061745122574500020189880363468340637347674643851875759782894318316386198487970256787451014597457079993094755057664 GHz | 1 |
| Iq | Cs8 | 0 Hz to 179769313486231564793826746329356487460061771816610493194377291867566634083253736182911671780280864445928163680987122391750825462330354250895282439122322875506826024599142533926918074193061745122574500020189880363468340637347674643851875759782894318316386198487970256787451014597457079993094755057664 GHz | 1 |
| Iq | Cu8 | 0 Hz to 179769313486231564793826746329356487460061771816610493194377291867566634083253736182911671780280864445928163680987122391750825462330354250895282439122322875506826024599142533926918074193061745122574500020189880363468340637347674643851875759782894318316386198487970256787451014597457079993094755057664 GHz | 1 |
| Iq | Cf32 | 0 Hz to 179769313486231564793826746329356487460061771816610493194377291867566634083253736182911671780280864445928163680987122391750825462330354250895282439122322875506826024599142533926918074193061745122574500020189880363468340637347674643851875759782894318316386198487970256787451014597457079993094755057664 GHz | 1 |

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
| `server_id` | Server id | text |
| `rpc_version` | RPC version | text |
| `driver` | Driver | text |
| `hardware` | Hardware | text |
| `serial` | Serial number | text |
| `rx_channels` | Receive channels | whole number |
| `tx_channels` | Transmit channels | whole number |
| `wire_format` | Format on the network | text |
| `native_format` | Native format | text |
| `native_scale` | Native full scale | number |
| `mtu_bytes` | Datagram size | whole number (B) |
| `window_bytes` | Receive buffer | whole number (B) |
| `window_packets` | Window | whole number (packets) |
| `per_datagram` | Samples per datagram | whole number |
| `mbit_s` | Link speed | number (Mbit/s) |
| `arriving_hz` | Arriving rate | frequency |
| `overflows` | Overflows at the server | whole number |
| `lost_datagrams` | Datagrams lost | whole number |
| `late_datagrams` | Late datagrams | whole number |
| `malformed_datagrams` | Malformed datagrams | whole number |
| `stream_errors` | Stream errors | whole number |
| `reconnects` | Reconnects | whole number |
| `log_dropped` | Server log lines dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `sensors` | Sensors | `name` (text), `value` (text) |
| `hardware_info` | Hardware information | `name` (text), `value` (text) |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:soapyremote <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `address` | Build an address | `host` (text, required), `port` (whole number, default 55132) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Transmit

The radio can transmit. The range of the transmit power and the power that the transmitter starts with come from the radio when it opens: the bottom of the range for a radio that can drive an amplifier, 10 dB below full power for a low-power board.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tx_frequency` | Transmit frequency | frequency | 100 MHz | 0 Hz to 1000 GHz |  |
| `tx_rate` | Transmit rate | frequency | 0 Hz | 0 Hz to 1 GHz |  |
| `tx_bandwidth` | Transmit bandwidth | frequency | 0 Hz | 0 Hz to 1 GHz |  |
| `tx_power` | Transmit power | number (dB) | -10 | -200 to 0 | The output relative to the full output of the radio: 0 dB is full power, -10 dB is a tenth of it. |
| `txoutput` | Output | whole number | 1 | 1 to 8 | The transmit output of the radio, from 1. An output the radio does not have is refused. |
| `tx_ppm` | Transmit frequency correction | number (ppm) | 0 | -200 to 200 | The clock error in ppm, positive when the clock runs fast. |

Streams it accepts:

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cf32 | 0 Hz to 179769313486231564793826746329356487460061771816610493194377291867566634083253736182911671780280864445928163680987122391750825462330354250895282439122322875506826024599142533926918074193061745122574500020189880363468340637347674643851875759782894318316386198487970256787451014597457079993094755057664 GHz | 1 |

## Try it

```sh
sdrkit run fm --source soapyremote:192.168.1.30
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
| `server_id` | Server id | text |
| `rpc_version` | RPC version | text |
| `driver` | Driver | text |
| `hardware` | Hardware | text |
| `serial` | Serial number | text |
| `rx_channels` | Receive channels | whole number |
| `tx_channels` | Transmit channels | whole number |
| `wire_format` | Format on the network | text |
| `native_format` | Native format | text |
| `native_scale` | Native full scale | number |
| `mtu_bytes` | Datagram size | whole number (B) |
| `window_bytes` | Receive buffer | whole number (B) |
| `window_packets` | Window | whole number (packets) |
| `per_datagram` | Samples per datagram | whole number |
| `mbit_s` | Link speed | number (Mbit/s) |
| `arriving_hz` | Arriving rate | frequency |
| `overflows` | Overflows at the server | whole number |
| `lost_datagrams` | Datagrams lost | whole number |
| `late_datagrams` | Late datagrams | whole number |
| `malformed_datagrams` | Malformed datagrams | whole number |
| `stream_errors` | Stream errors | whole number |
| `reconnects` | Reconnects | whole number |
| `log_dropped` | Server log lines dropped | whole number |
