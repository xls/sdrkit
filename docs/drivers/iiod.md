<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# PlutoSDR / ANTSDR (network)

A PlutoSDR, an ANTSDR or another AD936x board, through the iiod network daemon.

Driver key: `iiod`.

## Address

- `iiod:<host>`: port 30431.
- `iiod:<host>:<port>`
- `iiod:ip:<host>`

The device has one receiver: one radio of an address is open at a time, and the sessions share it.

## Radio settings

Every radio has these. Give one as `--set key=value`. If the stack has a setting with the same key, write `radio.key=value`.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `centre_hz` | Centre frequency | frequency | 100 MHz | 0 Hz to 1000 GHz |  |
| `rate` | Sample rate | frequency | 0 Hz | 0 Hz to 1 GHz | The rate of the samples from the radio. On Automatic the radio takes the least rate at or above what the stack needs; a rate that is set stays, and a stack that needs more is refused. Write 0 to return to Automatic. |
| `bandwidth` | Bandwidth | frequency | 0 Hz | 0 Hz to 1 GHz | The width of the analogue filter of the radio. Without a width, the driver chooses one that fits the sample rate. |
| `gain` | Gain | number (dB) | 30 | -100 to 200 |  |
| `agc` | Automatic gain | yes or no | true |  |  |
| `rate_exact` | Exact rate | yes or no | false |  | Sends the rate as it is, even outside the rates the driver picks. |
| `bias_tee` | Bias tee | yes or no | false |  |  |

## Driver settings

Only this driver has these.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `ppm` | Frequency correction | number (ppm) | 0 | -200 to 200 | The clock error in ppm, positive if the clock runs fast. Corrects tuning and sample rate together. |
| `lowratefir` | Low-rate FIR (RX only) | yes or no | false |  | Receive below 2.1 Msps (down to 0.521 Msps, the 2.048 Msps of DAB among them) through the hardware FIR, RX only. Off: at least 2.1 Msps, resampled in software. The AD936x loads its receive and transmit filter taps from one file, so this also replaces the transmit filter taps until the radio restarts, and some boards switch both filters on together. Nothing is transmitted, but leave it off on a radio you also transmit with. |
| `gainmode` | Gain mode | choice | `manual` | `manual` (Manual), `slow_attack` (Radio AGC, slow attack), `fast_attack` (Radio AGC, fast attack), `hybrid` (Radio AGC, hybrid) | Manual: the gain setting sets the gain. With automatic gain on, the driver chooses the gain from the level of the samples (the peaks 10 dB below full scale), holds it, and moves it only when the level stays more than 6 dB off for half a second, after a retune, or when the samples clip. The AGC choices give the gain to the AD936x, and the gain setting does nothing: its AGC can step the gain while a signal is on, and each step turns the phase of the signal, which breaks the reception of OFDM signals such as DVB-T. Hybrid: the AGC holds the gain until told to change it. |
| `antenna` | Antenna | choice | `rx1` | `rx1` (RX1), `rx2` (RX2) | RX2 needs two receivers. |
| `receivers` | Receivers | choice | `one` | `one` (One (the antenna)), `both` (Both, as two streams), `interleaved` (Both, interleaved in one stream) | Both receivers at once, on one clock and one LO (AD9361 boards: ANTSDR E310, Pluto+). Two streams: RX1 in the first, RX2 in the second. Interleaved: one stream of RX1, RX2, RX1... at twice the rate. Twice the data on the link. |
| `gainmode2` | RX2 gain mode | choice | `same` | `same` (Same as RX1), `manual` (Manual), `slow_attack` (Radio AGC, slow attack), `fast_attack` (Radio AGC, fast attack), `hybrid` (Radio AGC, hybrid) | With both receivers: the gain mode of RX2, or the same as RX1. |
| `gain2` | RX2 gain | number (dB) | 30 | 0 to 77 | With both receivers and RX2 in manual gain: its gain. |
| `rxport` | RX port | choice | `auto` | `auto` (As the radio is), `A_BALANCED`, `B_BALANCED`, `C_BALANCED`, `A_N`, `A_P`, `B_N`, `B_P`, `C_N`, `C_P`, `TX_MONITOR1`, `TX_MONITOR2`, `TX_MONITOR1_2` | The AD936x input that the receivers use. Most boards wire only A_BALANCED. Some switch bands with B and C. |
| `quadtrack` | Quadrature tracking | yes or no | true |  | The AD936x keeps correcting the I/Q balance. Turn it off only for measurements that need it untouched. |
| `rfdctrack` | RF DC tracking | yes or no | true |  | The AD936x keeps removing the DC offset of the RF stage. |
| `bbdctrack` | Baseband DC tracking | yes or no | true |  | The AD936x keeps removing the DC offset of the baseband. Off keeps a carrier at the centre frequency. |
| `kbuffers` | Buffers | whole number | 8 | 2 to 64 | Sample buffers that the board keeps in flight. More ride out a slow link or a busy board, at the cost of latency. |
| `bufsize` | Buffer size | choice | `auto` | `auto` (Auto (25 ms)), `65536` (65 536), `131072` (131 072), `262144` (262 144), `524288` (524 288), `1048576` (1 048 576), `2097152` (2 097 152) | Samples per transfer. Auto holds at least 25 ms, which keeps high rates flowing over a network. |
| `protocol` | Protocol | choice | `auto` | `auto` (Auto), `text` (Text only) | Auto streams over the binary protocol if the server of the board offers it (libiio 1.x), else the text protocol. |
| `link` | Link | choice | `auto` | `auto` (Auto), `usb` (USB cable), `ethernet` (Ethernet) | Auto takes the default address as the network of the USB cable (about 3 Msps without loss). Ethernet means a Pluto+ or a USB Ethernet adapter, with no cap. USB holds the USB rate. |
| `iqformat` | IQ format | choice | `cs16` | `cs16` (CS16), `cs8` (CS8) | CS16 is 16-bit (the 12-bit converter). CS8 is half the data, so a higher rate over USB or a slow network, with less dynamic range. Tezuka firmware only. |
| `readings_s` | Readings every | number (s) | 5 | 0 to 60 | How often the temperature, the RSSI, the gain and the state of the board are read, on a connection of their own. 0 reads none. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cs16 | 520.833333 kHz to 61.44 MHz | 1 |
| Iq | Cs8 | 520.833333 kHz to 61.44 MHz | 1 |

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
| `temperature_c` | Temperature | number (°C) |
| `rssi1_db` | RX1 RSSI | number (dB) |
| `rssi2_db` | RX2 RSSI | number (dB) |
| `rx1_gain_db` | RX1 gain | number (dB) |
| `rx2_gain_db` | RX2 gain | number (dB) |
| `rx1_held_gain_db` | RX1 held gain | number (dB) |
| `rx1_gain_control` | RX1 gain control | choice |
| `rx1_gain_steps` | RX1 gain steps | whole number |
| `rx2_held_gain_db` | RX2 held gain | number (dB) |
| `rx2_gain_control` | RX2 gain control | choice |
| `rx2_gain_steps` | RX2 gain steps | whole number |
| `rx_overflows` | Capture overflows | whole number |
| `calibration` | Calibration | text |
| `ensm` | State | text |
| `tx_output_dbm` | Output (estimate, not measured) | number (dBm) |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:iiod <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `address` | Build an address | `host` (text, required), `port` (whole number, default 30431) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Transmit

The radio can transmit. The range of the transmit power and the power that the transmitter starts with come from the radio when it opens: the bottom of the range for a radio that can drive an amplifier, 10 dB below full power for a low-power board.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `tx_frequency` | Transmit frequency | frequency | 100 MHz | 0 Hz to 1000 GHz |  |
| `tx_rate` | Transmit rate | frequency | 0 Hz | 0 Hz to 1 GHz |  |
| `tx_bandwidth` | Transmit bandwidth | frequency | 0 Hz | 0 Hz to 1 GHz |  |
| `tx_power` | Transmit power | number (dB) | -10 | -200 to 0 | The output relative to the full output of the radio: 0 dB is full power, -10 dB is a tenth of it. |
| `transmitters` | Outputs | choice | `one` | `one` (One (the output)), `both` (Both, as two streams), `interleaved` (Both, interleaved in one stream) | Both outputs at once, on one clock and one TX LO (AD9361 boards: ANTSDR E310, Pluto+). Two streams: each output takes its own samples. Interleaved: one stream of TX1, TX2, TX1... At most 30.72 Msps with both. |
| `txport` | TX port | choice | `auto` | `auto` (As the radio is set), `A` (Port A), `B` (Port B) | The AD936x output port (A or B) that the transmitter uses. Both outputs share it. |

Advanced settings. The owner works without any of them set.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `txoutput` | Output | choice | `tx1` | `tx1` (TX1: first output, the only one of a Pluto), `tx2` (TX2: second output, on AD9361 boards) | The pair of converter channels that carries the samples if one output runs; the power setting goes to every output. Choose the other one if the antenna is on the other connector and nothing is heard (TX2 needs an AD9361 board: ANTSDR E310, Pluto+). |

Streams it accepts:

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cf32 | 2.1 MHz to 61.44 MHz | 1 |

## Try it

```sh
sdrkit run fm --source iiod:192.168.2.1
```
<!-- generated: end -->

## Estimated transmit output

The reading `tx_output_dbm` is an estimate, not a measurement. It is the transmit power setting (`tx_power`) added to
typical full-power outputs of the AD936x: 8 dBm at 800 MHz, 7.5 dBm at 2.4 GHz, 6.5 dBm at 3.5 GHz and 6 dBm at
5.5 GHz, with straight lines between them. These figures are typical values and are not verified against the data
sheet or by a measurement. The balun, the board and the cable of a radio take part of the output. Below 800 MHz and
above 5.5 GHz the reading has no value.

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
| `temperature_c` | Temperature | number (°C) |
| `rssi1_db` | RX1 RSSI | number (dB) |
| `rssi2_db` | RX2 RSSI | number (dB) |
| `rx1_gain_db` | RX1 gain | number (dB) |
| `rx2_gain_db` | RX2 gain | number (dB) |
| `calibration` | Calibration | text |
| `ensm` | State | text |
