<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Test signal

The test signal of a stack. No radio is involved.

Driver key: `synth`.

## Address

- `synth:<stack>`: the test signal of the stack.
- `synth:<stack>/<submode>`
- `synth`: silence.

Each open radio of an address is a receiver of its own.

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
| `pace` | Speed | number (x real time) | 1 | 0.05 to 1 | 1 plays in real time. Lower values play in slow motion, for slow test machines. |
| `benchmark` | As fast as possible | yes or no | false |  | Plays as fast as the receiver takes the samples, to measure its speed. |
| `quantize` | 8-bit quantisation | yes or no | true |  | Rounds I and Q to 8 bits like the converter of a low-cost radio, with noise of one step and its clipping 12 dB above the nominal level. |
| `gainmodel` | Gain model | yes or no | false |  | Scales the level by the gains below (62 dB in all is the nominal level), to test gain control. |
| `lna` | LNA gain | number (dB) | 24 | 0 to 40 |  |
| `vga` | VGA gain | number (dB) | 24 | 0 to 62 |  |
| `amp` | Amplifier | yes or no | true |  | Adds 14 dB. |
| `gen_seed` | Seed | whole number | 1 | 0 to 9007199254740991 | The same seed gives the same signal. |
| `gen_level_dbfs` | Level | number (dBFS) | 0 | -150 to 20 | The signal's mean power; 0 dBFS is a mean power of 1. |
| `gen_noise` | Noise | choice | `off` | `off` (Off), `snr` (SNR in the signal's bandwidth), `cn0` (C/N0), `esn0` (Es/N0), `ebn0` (Eb/N0), `floor` (Fixed floor) | How the noise level is given. |
| `gen_snr_db` | SNR | number (dB) | 20 | -50 to 100 | Signal to noise in the signal's bandwidth. |
| `gen_cn0_dbhz` | C/N0 | number (dB-Hz) | 60 | -20 to 160 | Carrier to noise density. |
| `gen_esn0_db` | Es/N0 | number (dB) | 10 | -30 to 100 | Energy per symbol to noise density. |
| `gen_ebn0_db` | Eb/N0 | number (dB) | 10 | -30 to 100 | Energy per information bit to noise density. |
| `gen_noise_floor_dbfs` | Noise floor | number (dBFS) | -60 | -200 to 20 | Noise power over the whole sample rate. |
| `gen_offset_hz` | Offset | number (Hz) | 0 | -1000000000 to 1000000000 | Carrier frequency offset. |
| `gen_drift_hz_s` | Drift | number (Hz/s) | 0 | -10000000 to 10000000 | Change of the offset per second. |
| `gen_clock_ppm` | Clock error | number (ppm) | 0 | -10000 to 10000 | The receiver's sample clock runs this fast. |
| `gen_bit_jitter` | Bit-edge jitter | number (bit) | 0 | 0 to 0.25 | Each bit edge moves by a random amount up to this share of a bit. |
| `gen_fading` | Channel | choice | `none` | `none` (None), `rayleigh` (Rayleigh (flat)), `rician` (Rician (flat)), `echo` (Steady echo), `echoes` (Steady echoes (a list)), `drm1` (DRM channel 1 (AWGN)), `drm2` (DRM channel 2 (Rice with delay)), `drm3` (DRM channel 3 (US Consortium)), `drm4` (DRM channel 4 (CCIR poor)), `drm5` (DRM channel 5), `drm6` (DRM channel 6), `drm7` (DRM channel 7 (VHF AWGN)), `drm8` (DRM channel 8 (VHF urban)), `drm9` (DRM channel 9 (VHF rural)), `drm10` (DRM channel 10 (VHF terrain obstructed)), `drm11` (DRM channel 11 (VHF hilly terrain)), `drm12` (DRM channel 12 (VHF SFN)), `hf_good` (HF good (F.520)), `hf_moderate` (HF moderate (F.520)), `hf_poor` (HF poor (F.520)), `hf_flutter` (HF flutter (F.520)), `f1487_low_quiet` (HF low latitude, quiet (F.1487)), `f1487_low_moderate` (HF low latitude, moderate (F.1487)), `f1487_low_disturbed` (HF low latitude, disturbed (F.1487)), and others | A fading or multipath channel from the standards. |
| `gen_doppler_hz` | Doppler | number (Hz) | 10 | 0.001 to 100000 | Maximum Doppler of moving and flat fading channels. |
| `gen_rician_k_db` | Rice factor | number (dB) | 6 | -30 to 40 | Direct path over scattered power. |
| `gen_echo_delay_s` | Echo delay | number (s) | 0.001 | 0 to 1 | Delay of the steady echo. |
| `gen_echo_db` | Echo level | number (dB) | -6 | -80 to 20 | Power of the echo relative to the direct path. |
| `gen_echo_phase_deg` | Echo phase | number (°) | 0 | -180 to 180 | Carrier phase of the echo. |
| `gen_echoes` | Echoes | text | empty |  | The steady echoes of the channel "echoes": entries separated by ";", each the delay in seconds (negative for a pre-echo), the level in dB and the phase in degrees, for example "2e-6 -8 0; -1e-6 -10 90". |
| `gen_phase_noise` | Phase noise | yes or no | false |  |  |
| `gen_phase_noise_dbc_hz` | Phase noise level | number (dBc/Hz) | -80 | -200 to 0 | On the 1/f^2 slope. |
| `gen_phase_noise_offset_hz` | Phase noise offset | number (Hz) | 10000 | 1 to 100000000 | Where the level is measured. |
| `gen_inverted` | Inverted spectrum | yes or no | false |  |  |
| `gen_hum_hz` | Hum frequency | number (Hz) | 50 | 1 to 1000 | Mains ripple frequency. |
| `gen_hum_depth` | Hum depth | number | 0 | 0 to 1 | Amplitude ripple; 0 for none. |
| `gen_impulse_rate_hz` | Impulses | number (Hz) | 0 | 0 to 1000000 | Mean impulses per second; 0 for none. |
| `gen_impulse_width_s` | Impulse length | number (s) | 0.0001 | 0 to 1 | Length of each impulse. |
| `gen_impulse_dbfs` | Impulse level | number (dBFS) | 0 | -150 to 40 | Power of an impulse. |
| `gen_iq_gain_db` | IQ gain error | number (dB) | 0 | -6 to 6 | Gain of Q relative to I. |
| `gen_iq_phase_deg` | IQ phase error | number (°) | 0 | -45 to 45 | Phase of Q relative to I. |
| `gen_dc_i` | DC offset I | number | 0 | -1 to 1 | Added to I, full-scale units. |
| `gen_dc_q` | DC offset Q | number | 0 | -1 to 1 | Added to Q, full-scale units. |
| `gen_adc_bits` | ADC bits | whole number | 0 | 0 to 24 | Quantise and clip; 0 for an ideal receiver. |

The radio can add settings when it opens, such as the gain of each stage. `show radio` lists them on the console of `run`.

## Settings of the test signal

When the radio opens, it adds the settings of the stack whose test signal it plays and this setting. Give them as `--set key=value`.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_at_hz` | Signal frequency | frequency | 0 Hz | 0 Hz to 1000 GHz | The frequency of the station of the signal: the radio has the signal only while its samples hold this frequency. 0 puts the station where the session listens, at every centre. |

## Streams it produces

| Kind | Sample | Rates | Channels |
|---|---|---|---|
| Iq | Cf32 | 1 Hz to 1 GHz | 1 |

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

They work before any radio is open. Run one with `sdrkit call kind:driver:synth <command>`.

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `address` | Build an address | `stack` (text, required), `submode` (text, default empty) | `address` (text) | Returns the address of a radio of this kind, made from its parts. It connects to nothing. |

## Try it

```sh
sdrkit run fm --source synth:fm
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
