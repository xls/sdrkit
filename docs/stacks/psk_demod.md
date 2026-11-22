<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# PSK demodulator

A blind BPSK / QPSK / OQPSK / 8PSK analyser and demodulator for any narrow signal, with bit output. Receive only.

Stack id: `psk_demod`. Group: Satellite.

Signal types: Narrow digital.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 435 MHz |
| Tuning range | 100 kHz to 6 GHz |
| Sample rate | 2 MHz |
| Smallest sample rate | 8 kHz |
| Signal bandwidth | 200 kHz |
| Offset from the signal | 250 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:psk_demod` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `order` | Order | choice | `auto` | `auto` (Found by the analyser), `bpsk` (BPSK), `qpsk` (QPSK), `oqpsk` (OQPSK), `8psk` (8PSK) |  |
| `baud` | Symbol rate | frequency | 0 Hz | 0 Hz to 1 MHz | 0: found by the analyser. |
| `shape` | Pulse shape | choice | `auto` | `auto` (Found by the analyser), `rect` (Rectangular), `0.2` (RRC 0.2), `0.25` (RRC 0.25), `0.35` (RRC 0.35), `0.5` (RRC 0.5), `0.8` (RRC 0.8), `1.0` (RRC 1.0) |  |
| `span` | Search span | text | `auto` |  | auto (200 kHz or what the radio gives), a number of Hz (2500 to 200000), 500k, 2M or whole. |
| `snap_standard` | Snap to standard rates | yes or no | true |  |  |
| `follow_doppler` | Follow Doppler | yes or no | true |  |  |
| `click_hz` | Clicked frequency | text (Hz) | empty |  | Hz from the dial; empty: the strongest signal. |
| `differential` | Differential | choice | `none` | `none` (None), `dpsk` (Differential PSK), `nrzm` (NRZ-M), `nrzs` (NRZ-S) |  |
| `descrambler` | Descrambler | choice | `none` | `none` (None), `g3ruh` (G3RUH), `ccsds` (CCSDS) |  |
| `sync` | Sync word | text | empty |  | Bits or hex ("1ACFFC1D"); every rotation and both spectral senses are tried. |
| `invert` | Invert the bits | yes or no | false |  |  |
| `save_bits` | Save bits | yes or no | false |  | Hands the bits, packed into bytes, to the host (event psk_demod.bytes). |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_order` | Order | choice | `qpsk` | `bpsk` (BPSK), `qpsk` (QPSK), `oqpsk` (OQPSK), `8psk` (8PSK) |  |
| `gen_baud` | Symbol rate | frequency | 4.8 kHz | 75 Hz to 100 kHz |  |
| `gen_rolloff` | Roll-off (0: rectangular) | number | 0.35 | 0 to 1 |  |
| `gen_coding` | Coding | choice | `none` | `none` (None), `dpsk` (Differential PSK), `nrzm` (NRZ-M), `nrzs` (NRZ-S), `g3ruh` (G3RUH), `ccsds` (CCSDS) |  |
| `gen_sync` | Sync word | text | `1ACFFC1D` |  |  |
| `gen_conjugate` | Conjugated symbols | yes or no | false |  | The transmitter swaps I and Q (the decoder must find the spectral inversion). |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `symbol_rate_hz` | Symbol rate | frequency |
| `order` | Order | choice |
| `lock` | Lock | number |
| `carrier_hz` | Carrier | frequency |
| `drift_hz_s` | Drift | number (Hz/s) |
| `evm_pct` | EVM | number (%) |
| `esn0_db` | Es/N0 | number (dB) |
| `ebn0_db` | Eb/N0 | number (dB) |
| `symbols` | Symbols | whole number |
| `bits` | Bits | whole number |
| `frames_ok` | Frames good | whole number |
| `bit_stream` | Bits | text |
| `bits_first` | First bit | whole number |
| `rotation_deg` | Rotation | number (deg) |
| `conjugated` | Spectrum inverted | yes or no |
| `rail_shift` | Rail shift | yes or no |
| `resolved` | Resolved | yes or no |
| `signal_hz` | Signal | frequency |
| `width_3db_hz` | Width at 3 dB | frequency |
| `width_10db_hz` | Width at 10 dB | frequency |
| `occupied_hz` | Occupied bandwidth | frequency |
| `signal_snr_db` | In-band SNR | number (dB) |
| `baud_raw_hz` | Measured rate | frequency |
| `snapped` | Snapped | yes or no |
| `rate_confidence` | Rate confidence | number |
| `order_confidence` | Order confidence | number |
| `shape` | Shape | choice |
| `roll_off` | Roll-off | number |
| `envelope_db` | Envelope line | number (dB) |
| `line_x2_db` | x^2 line | number (dB) |
| `line_x4_db` | x^4 line | number (dB) |
| `line_x8_db` | x^8 line | number (dB) |
| `note` | Note | choice |
| `dropped` | Dropped | whole number |
| `receiver.dropped` | Dropped | whole number |
| `bits.dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `rates` | Rate candidates | `rate_hz` (frequency), `strength_db` (number (dB)) |
| `trials` | Order trials | `order` (choice), `symbol_rate_hz` (frequency), `evm` (number) |
| `syncs` | Sync words at | `bit` (whole number) |
| `log` | Log | `at` (duration (s)), `text` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `constellation` | Constellation |  |  |
| `eye` | Eye of the I rail | samples |  |
| `envelope_spectrum` | Envelope spectrum | bins | dB |
| `power_spectrum` | M-th power spectrum | bins | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `psk_demod.lock` | Signal locked | `text` (text), `snr_db` (number (dB)), `family` (choice), `symbol_rate_hz` (frequency), `order` (choice) |
| `psk_demod.bytes` | Bits as bytes | `text` (text), `family` (choice), `bytes` (bytes) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |

## Typed interface

The stack is in the family `satcom`, version 1.3. The C header `sdrkit/satcom.h`, the registry record `registry/satcom.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite links (`satellite_links`). Media: none.

### Result fields

Struct `PskDemodFields`. The result fields of the PSK demodulator.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `symbol_rate_hz` | frequency | optional | 1.0 | The symbol rate. |
| `order` | choice | optional | 1.0 | The modulation order the demodulator runs. |
| `lock` | number | optional | 1.0 | The carrier loop's lock value (1 on the points). |
| `carrier_hz` | frequency | optional | 1.0 | The carrier, from the dial. |
| `drift_hz_s` | number (Hz/s) | optional | 1.0 | The drift. |
| `evm_pct` | number (%) | optional | 1.0 | The EVM. |
| `esn0_db` | number (dB) | optional | 1.0 | Es/N0 from the error vector. |
| `ebn0_db` | number (dB) | optional | 1.0 | Eb/N0 from the error vector. |
| `symbols` | whole number | always | 1.0 | Symbols demodulated since the lock. |
| `bits` | whole number | always | 1.0 | Bits since the start. |
| `frames_ok` | whole number | always | 1.0 | Frames that passed their check. |
| `bit_stream` | text | always | 1.0 | The newest bits, oldest first, as the characters 0 and 1. |
| `bits_first` | whole number | always | 1.0 | The index of the first bit of `bit_stream`. |
| `rotation_deg` | number (deg) | always | 1.0 | The phase rotation the sync words resolved. |
| `conjugated` | yes or no | always | 1.0 | Whether the sync words resolved a spectral inversion. |
| `rail_shift` | yes or no | always | 1.0 | Whether the sync words resolved an OQPSK rail shift. |
| `resolved` | yes or no | always | 1.0 | Whether the phase ambiguity is resolved. |
| `signal_hz` | frequency | optional | 1.0 | The centre of the signal found, from the dial. |
| `width_3db_hz` | frequency | optional | 1.0 | Its width at 3 dB. |
| `width_10db_hz` | frequency | optional | 1.0 | Its width at 10 dB. |
| `occupied_hz` | frequency | optional | 1.0 | Its 99 % occupied bandwidth. |
| `signal_snr_db` | number (dB) | optional | 1.0 | Its in-band SNR. |
| `baud_raw_hz` | frequency | optional | 1.0 | The symbol rate the analyser measured, before snapping. |
| `snapped` | yes or no | optional | 1.0 | Whether the rate was snapped to a standard one. |
| `rate_confidence` | number | optional | 1.0 | The confidence of the rate, 0 to 1. |
| `order_confidence` | number | optional | 1.0 | The confidence of the order, 0 to 1. |
| `shape` | choice | optional | 1.0 | The pulse shape. |
| `roll_off` | number | optional | 1.0 | The roll-off of root raised cosine pulses. |
| `envelope_db` | number (dB) | optional | 1.0 | The envelope line over the median. |
| `line_x2_db` | number (dB) | optional | 1.0 | The squared signal's line over the median. |
| `line_x4_db` | number (dB) | optional | 1.0 | The fourth power's line over the median. |
| `line_x8_db` | number (dB) | optional | 1.0 | The eighth power's line over the median. |
| `note` | choice | optional | 1.0 | What the analyser said when it found no PSK signal. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `rates`

Struct `PskDemodRateRow`. A rate candidate of the analyser. The row key is its place ("0").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rate_hz` | frequency | always | 1.0 | A rate candidate. |
| `strength_db` | number (dB) | always | 1.0 | A line's strength over the median. |

### Table `trials`

Struct `PskDemodTrialRow`. A trial demodulation of the analyser. The row key is its place ("00").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `order` | choice | always | 1.0 | The modulation order the demodulator runs. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate. |
| `evm` | number | always | 1.0 | The error vector of a trial, unit power. |

### Table `syncs`

Struct `PskDemodSyncRow`. The bit index after a recent sync word, oldest first. The row key is its place ("00").

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `bit` | whole number | always | 1.0 | The bit index after a sync word. |

### Table `log`

Struct `PskDemodLogRow`. A line of the log of analyses. The row key is the sequence number of the line.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `at` | duration (s) | always | 1.0 | The signal time of a line of the log. |
| `text` | text | always | 1.0 | The line. |

### Event `psk_demod.lock`

Struct `PskDemodLockEvent`. A signal was locked.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The order and the symbol rate. |
| `snr_db` | number (dB) | optional | 1.0 | The signal to noise ratio. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `symbol_rate_hz` | frequency | always | 1.0 | The symbol rate. |
| `order` | choice | optional | 1.0 | The modulation order the demodulator runs. |

### Event `psk_demod.bytes`

Struct `PskDemodBytesEvent`. Bits packed into bytes for the host. The text is the bytes in hexadecimal.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `text` | text | always | 1.0 | The bytes in hexadecimal. |
| `family` | choice | always | 1.0 | The kind of thing that the event is about. |
| `bytes` | bytes | always | 1.0 | The bytes of a bytes event. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_psk_demod_clear` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `order` | `sdrkit_psk_demod_get_order` | `sdrkit_psk_demod_set_order` |
| `baud` | `sdrkit_psk_demod_get_baud` | `sdrkit_psk_demod_set_baud` |
| `shape` | `sdrkit_psk_demod_get_shape` | `sdrkit_psk_demod_set_shape` |
| `span` | `sdrkit_psk_demod_get_span` | `sdrkit_psk_demod_set_span` |
| `snap_standard` | `sdrkit_psk_demod_get_snap_standard` | `sdrkit_psk_demod_set_snap_standard` |
| `follow_doppler` | `sdrkit_psk_demod_get_follow_doppler` | `sdrkit_psk_demod_set_follow_doppler` |
| `click_hz` | `sdrkit_psk_demod_get_click_hz` | `sdrkit_psk_demod_set_click_hz` |
| `differential` | `sdrkit_psk_demod_get_differential` | `sdrkit_psk_demod_set_differential` |
| `descrambler` | `sdrkit_psk_demod_get_descrambler` | `sdrkit_psk_demod_set_descrambler` |
| `sync` | `sdrkit_psk_demod_get_sync` | `sdrkit_psk_demod_set_sync` |
| `invert` | `sdrkit_psk_demod_get_invert` | `sdrkit_psk_demod_set_invert` |
| `save_bits` | `sdrkit_psk_demod_get_save_bits` | `sdrkit_psk_demod_set_save_bits` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_order` | `sdrkit_psk_demod_gen_get_order` | `sdrkit_psk_demod_gen_set_order` |
| `gen_baud` | `sdrkit_psk_demod_gen_get_baud` | `sdrkit_psk_demod_gen_set_baud` |
| `gen_rolloff` | `sdrkit_psk_demod_gen_get_rolloff` | `sdrkit_psk_demod_gen_set_rolloff` |
| `gen_coding` | `sdrkit_psk_demod_gen_get_coding` | `sdrkit_psk_demod_gen_set_coding` |
| `gen_sync` | `sdrkit_psk_demod_gen_get_sync` | `sdrkit_psk_demod_gen_set_sync` |
| `gen_conjugate` | `sdrkit_psk_demod_gen_get_conjugate` | `sdrkit_psk_demod_gen_set_conjugate` |

## Try it

```sh
sdrkit run psk_demod --source synth:psk_demod
sdrkit run psk_demod --source iiod:192.168.2.1 --freq 435
```
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.
