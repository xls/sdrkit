<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# X-band direct broadcast

Direct-broadcast and dump downlinks of polar weather and earth-observation satellites (JPSS, EOS, MetOp, Landsat, Proba, GCOM, Oceansat, SARAL, Kanopus-V, DMSP): frames, instrument pictures and colour products. Most links need a tracking dish and tens of Msps; a recording or a frame file is the usual input.

Stack id: `eodb`. Group: Weather satellites.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | 950 MHz to 9 GHz |
| Sample rate | 25 MHz |
| Smallest sample rate | 600 kHz |
| Signal bandwidth | 21 MHz |
| Automatic gain | Radio AGC |

## Submodes

Choose one with `--submode <id>`.

| Id | Name | Default frequency |
|---|---|---|
| `jpss_hrd` | JPSS HRD | 0 Hz |
| `jpss_hrd_n21` | JPSS HRD (NOAA-21) | 0 Hz |
| `eos_aqua_aura` | EOS Aqua and Aura | 0 Hz |
| `eos_terra` | EOS Terra | 0 Hz |
| `metop_dump` | MetOp X-band dump | 0 Hz |
| `metopsg_ddb` | MetOp-SG DDB | 0 Hz |
| `landsat_x` | Landsat X band | 0 Hz |
| `landsat_s` | Landsat S band | 0 Hz |
| `proba_s` | Proba S band | 0 Hz |
| `proba_v_x` | Proba-V X band | 0 Hz |
| `gcom_w1` | GCOM-W1 | 0 Hz |
| `gcom_c1` | GCOM-C1 | 0 Hz |
| `stereo` | STEREO beacon | 0 Hz |
| `oceansat2` | Oceansat-2 | 0 Hz |
| `oceansat3_l` | Oceansat-3 L band | 0 Hz |
| `saral_l` | SARAL L band | 0 Hz |
| `oceansat3_x` | Oceansat-3 X band | 0 Hz |
| `saral_x` | SARAL X band | 0 Hz |
| `kanopus_v` | Kanopus-V | 0 Hz |
| `dmsp_rtd` | DMSP RTD | 0 Hz |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:eodb` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `symbol_rate` | Symbol rate | frequency | 0 Hz | 0 Hz to 400 MHz | 0 takes the symbol rate of the profile, which no public document confirms. |
| `roll_off` | Matched filter roll-off | number | 0 | 0 to 1 | 0 takes the roll-off of the profile, which no public document confirms. |
| `satellites` | Satellite list | text | `links/xband_downlinks.csv` |  | A file in the host's storage with the satellites of each link and their spacecraft ids (data/links/xband_downlinks.csv). Empty: no satellite names. |
| `avhrr_calibration` | AVHRR calibration | text | `satimg/avhrr_calibration.csv` |  | A file in the host's storage with the AVHRR calibration coefficients of each satellite (data/satimg/avhrr_calibration.csv). Empty: no temperature products. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_ambiguity` | Test signal ambiguity | whole number | 0 | 0 to 7 | Turns the test signal by 90 degrees this many times (modulo 4), mirrored from 4 on. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `snr_db` | SNR | number (dB) |
| `level_dbfs` | Level | number (dBFS) |
| `offset_hz` | Offset | frequency |
| `clock_ppm` | Clock error | number (ppm) |
| `symbol_rate_hz` | Symbol rate | frequency |
| `frames_ok` | Frames good | whole number |
| `frames_bad` | Frames bad | whole number |
| `signal_time` | Signal time | duration (s) |
| `input_rate_hz` | Input rate | frequency |
| `lock_metric` | Carrier lock | number |
| `frame_lock` | Frame lock | yes or no |
| `sync_hits` | Sync words | whole number |
| `bytes_corrected` | Bytes corrected | whole number |
| `spacecraft` | Spacecraft | whole number |
| `vcid` | Virtual channel | whole number |
| `stream` | Bit stream | whole number |
| `inverted` | Inverted | yes or no |
| `note` | Note | text |
| `carrier_locked` | Carrier locked | yes or no |
| `packets` | Packets | whole number |
| `packets_lost` | Packets lost | whole number |
| `instrument_packets` | Packets or frames in pictures | whole number |
| `lines` | Lines | whole number |
| `satellite` | Satellite | text |
| `hypothesis` | Locked hypothesis | text |
| `rate_too_low` | Rate too low | yes or no |
| `imported` | CADUs from files | whole number |
| `gate_skipped` | Samples let go by | whole number |
| `gate_skipped_pct` | Input let go by | number (%) |
| `gate_line` | Gate line over mean | number |
| `hint` | Hint | text |
| `rows` | Rows | whole number |
| `strips` | Strips | whole number |
| `bad_strips` | Bad strips | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time` (duration (s)), `spacecraft` (whole number), `vcid` (whole number), `bytes` (whole number), `check` (text), `hex` (text) |
| `virtual_channels` | Virtual channels | `vcid` (whole number), `frames` (whole number) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `esn0` | Es/N0 | s | dB |
| `byte_errors` | Byte errors | s | % |
| `frames_per_s` | Frames per second | s |  |
| `constellation` | Constellation |  |  |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Empties the frame table and the counters; the synchronisation stays. |
| `open_frames` | Open frame file | `path` (text, required) |  | Decodes a file of CADUs of the link's length, each starting with the marker 1ACFFC1D. |
| `product` | Make product | `product` (text, required), `width` (whole number (px), default 0) |  | Makes a colour product of the channel pictures. |

## Typed interface

The stack is in the family `satimg`, version 1.2. The C header `sdrkit/satimg.h`, the registry record `registry/satimg.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Satellite imagery (`satellite_imagery`). Media: pictures.

### Result fields

Struct `FrameTableFields`. The fields of every stack that lists frames: the link, the frame lock and the note.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `snr_db` | number (dB) | optional | 1.0 | The Es/N0 of the demodulator, dB. |
| `level_dbfs` | number (dBFS) | optional | 1.0 | The level of the input, dBFS. |
| `offset_hz` | frequency | optional | 1.0 | The carrier offset: the energy location, the search and the loop. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The error of the symbol clock, ppm. |
| `symbol_rate_hz` | frequency | optional | 1.0 | The nominal symbol rate. |
| `frames_ok` | whole number | always | 1.0 | Frames decoded. |
| `frames_bad` | whole number | always | 1.0 | Frames that failed their check. |
| `signal_time` | duration (s) | always | 1.0 | The seconds of signal decoded. |
| `input_rate_hz` | frequency | optional | 1.0 | The input rate. |
| `lock_metric` | number | optional | 1.0 | The carrier lock metric, 0 to 1. |
| `frame_lock` | yes or no | always | 1.0 | A confirmed frame lock is held. |
| `sync_hits` | whole number | always | 1.0 | Sync words found. |
| `bytes_corrected` | whole number | always | 1.0 | Reed-Solomon symbols corrected. |
| `spacecraft` | whole number | optional | 1.0 | The spacecraft of a frame header. |
| `vcid` | whole number | optional | 1.0 | The virtual channel of a frame header. |
| `stream` | whole number | optional | 1.0 | The QPSK stream in use, 0 to 3. |
| `inverted` | yes or no | always | 1.0 | The data arrive inverted. |
| `note` | text | always | 1.0 | A note ("picture not decoded: ..."). |

### Result fields

Struct `EodbFields`. The fields of EODB beside those of the frame table.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `carrier_locked` | yes or no | always | 1.0 | The carrier is locked (also while frames decode). |
| `packets` | whole number | always | 1.0 | Packets. |
| `packets_lost` | whole number | always | 1.0 | Space packets lost. |
| `instrument_packets` | whole number | always | 1.0 | Space packets that went into a picture. |
| `lines` | whole number | always | 1.0 | Picture lines. |
| `satellite` | text | always | 1.0 | The satellite of the last good frame. |
| `hypothesis` | text | always | 1.0 | The code hypothesis that holds the lock. |
| `rate_too_low` | yes or no | always | 1.0 | The input rate is below the least rate of the link. |
| `imported` | whole number | always | 1.0 | CADUs of frame files taken. |
| `gate_skipped` | whole number | optional | 1.0 | Input samples let go by the gate. |
| `gate_skipped_pct` | number (%) | optional | 1.0 | The share of the input let go by the gate, percent. |
| `gate_line` | number | optional | 1.0 | The height of the last gate line over the mean. |
| `hint` | text | always | 1.0 | What the user can do (empty when all is well). |

### Result fields

Struct `PictureFields`. The result set of the picture component.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `rows` | whole number | always | 1.0 | The rows of the picture shown. |
| `strips` | whole number | always | 1.0 | The strips kept, over all instruments. |
| `bad_strips` | whole number | always | 1.0 | The strips refused because their samples did not match their size. |

### Table `frames`

Struct `FrameRow`. A frame of a stack that lists frames. The row key is the place of the frame in the stream.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time` | duration (s) | always | 1.0 | Signal time of a frame. |
| `spacecraft` | whole number | optional | 1.0 | The spacecraft of a frame header. |
| `vcid` | whole number | optional | 1.0 | The virtual channel of a frame header. |
| `bytes` | whole number | always | 1.0 | Bytes of a frame with its sync word. |
| `check` | text | always | 1.0 | The check of a frame: good, check failed, or none. |
| `hex` | text | always | 1.0 | The first 24 bytes of a frame after decoding. |

The column `hex` names a row.

### Table `virtual_channels`

Struct `VirtualChannelRow`. The frames of one virtual channel. The row key is the virtual channel.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `vcid` | whole number | always | 1.0 | The virtual channel of a frame header. |
| `frames` | whole number | always | 1.0 | The good frames of a virtual channel. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_eodb_clear` | none | none |
| `open_frames` | `sdrkit_eodb_open_frames` | `sdrkit_satimg_file_args` | none |
| `product` | `sdrkit_eodb_product` | `sdrkit_eodb_product_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `symbol_rate` | `sdrkit_eodb_get_symbol_rate` | `sdrkit_eodb_set_symbol_rate` |
| `roll_off` | `sdrkit_eodb_get_roll_off` | `sdrkit_eodb_set_roll_off` |
| `satellites` | `sdrkit_eodb_get_satellites` | `sdrkit_eodb_set_satellites` |
| `avhrr_calibration` | `sdrkit_eodb_get_avhrr_calibration` | `sdrkit_eodb_set_avhrr_calibration` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_ambiguity` | `sdrkit_eodb_gen_get_ambiguity` | `sdrkit_eodb_gen_set_ambiguity` |

## Try it

```sh
sdrkit run eodb --source synth:eodb
sdrkit run eodb --source synth:eodb --submode jpss_hrd
sdrkit run eodb --source <driver>:<address> --freq 0
```
<!-- generated: end -->

## Description

One CCSDS-style link receiver for 20 links. A submode is a profile: the fixed constants of one link (modulation, symbol
rate, roll-off, differential coding, K = 7 code, Reed-Solomon code with interleave and virtual fill, randomiser, frame
length, frame header, packet zone and virtual channel) and the instrument whose packets it decodes. The satellites, their
downlinks and spacecraft ids are a data file, not part of a profile.

The chain: a front end that decimates by floor(rate / (4 × symbol rate)) when that is 2 or more; a gate that, while no
carrier is found, looks at one piece of 131 072 samples through the matched filter (raised to the power 2 for BPSK, 4 for
QPSK; offset QPSK also tries the doublet of the squared signal at half the symbol rate) and lets the next 7 pieces go by
unread unless a line stands 16 times over the mean; the PSK demodulator (its matched filter reads the symbols from the samples as they are);
the code synchroniser and Viterbi decoder (10 wrong code bits of the coded marker accepted, every rotation, mirror and
pairing, NRZ-M from either start), or for uncoded links the four QPSK streams or the sign patterns of the rails with
NRZ-M; the frame synchroniser on 1ACFFC1D; the byte inversion, randomiser and Reed-Solomon code of the profile; the AOS or
TM header; the packets of the profile's virtual channel; and the instrument. Oceansat-2, Kanopus-V and DMSP have no CADUs:
their own frame stages take the symbols. After 8 s without a good frame the carrier is searched again.

All channel pictures are 16-bit, one picture per channel that grows with the pass; the strips show the top 8 bits. Colour
products are made on request (the command `product`). A frame file of CADUs of the profile's length (the command
`open_frames`) goes through the same frame stage; every good frame also leaves as a `satimg.frame` item so a host can
record frame files.

Standard: CCSDS 131.0-B-5 (marker, K = 7 code, randomiser, Reed-Solomon); CCSDS 732.0-B-4 and 132.0-B-3 (frame headers);
CCSDS 133.0-B-2 (space packets); CCSDS 121.0-B-3 (Rice, VIIRS).

## Tuning: notes

Each profile has its own asked and least rate (below). Below the least rate the stack decodes nothing and says "this link
needs X Msps or more (a recording works)".

## Submodes: notes

The stack holds no frequency (0 Hz): the host sets it from the downlinks of the data file
`data/links/xband_downlinks.csv`.

## Profiles

| Submode | Modulation | Symbol rate | Code | Frame | Instrument | Output | Rate asked / least (Msps) |
|---|---|---|---|---|---|---|---|
| `jpss_hrd` | QPSK | 15 M | K = 7, NRZ-M, RS 16/4 | 1024, AOS | VIIRS (VC 16) | pictures, products | 25 / 18 |
| `jpss_hrd_n21` | OQPSK | 25 M | K = 7, NRZ-M, RS 16/5 | 1279, AOS insert 9 | VIIRS (VC 16) | pictures, products | 40 / 32 |
| `eos_aqua_aura` | OQPSK | 7.5 M | NRZ-M a rail, RS 16/4 | 1024, AOS | MODIS (VC 30) | pictures, products | 15 / 10 |
| `eos_terra` | BPSK | 13.125 M | K = 7 swapped, NRZ-M, RS 16/4, randomiser inside | 1024, AOS | MODIS (VC 42) | pictures, products | 45 / 30 |
| `metop_dump` | QPSK | 35 M | RS 16/4, bytes inverted | 1024, AOS insert 2 | AVHRR (VC 9) | pictures, products | 45 / 40 |
| `metopsg_ddb` | QPSK | 93.3 M | K = 7 swapped, RS 16/4 | 1024, AOS | METimage (VC 16) | pictures | 140 / 105 |
| `landsat_x` | OQPSK | 220.4 M | LDPC, not decoded | 1034, AOS | TIRS (VC 5), frame files only | pictures | 300 / 240 |
| `landsat_s` | OQPSK | 1.067 M | K = 7, NRZ-M, RS 16/5 | 1279, AOS | none | frame log | 6 / 2 |
| `proba_s` | BPSK | 2 M | K = 7 swapped, RS 16/5, retried without randomiser | 1279, TM | VEGETATION (VC 4) | frame log, pictures | 4 / 3 |
| `proba_v_x` | OQPSK | 35 M | K = 7, RS 16/5 | 1279, TM | none | frame log | 45 / 38 |
| `gcom_w1` | OQPSK | 10 M | K = 7, NRZ-M, RS 8/5 fill 3 | 1264, AOS | AMSR2 (VC 17) | pictures | 20 / 13 |
| `gcom_c1` | OQPSK | 69.416 M | NRZ-M | 1264, AOS | none | counters | 90 / 75 |
| `stereo` | BPSK | 1.44 M | turbo, not decoded | 1119, AOS | none, frame files only | counters | 3 / 2 |
| `oceansat2` | QPSK | 21.226 M | quadrant difference | 92 220-byte frames | OCM | pictures | 35 / 28 |
| `oceansat3_l` | QPSK | 1 M | K = 7, NRZ-M, RS 16/4 | 1024, AOS | none | frame log | 3 / 1.5 |
| `saral_l` | QPSK | 400 k | K = 7, NRZ-M, RS 16/4 | 1024, AOS | none | frame log | 3 / 0.6 |
| `oceansat3_x` | QPSK | 80 M | RS 8/8 | 2044, AOS | none | counters | 120 / 90 |
| `saral_x` | QPSK | 16 M | K = 7, RS 16/5 | 1279, AOS | none | frame log | 25 / 20 |
| `kanopus_v` | QPSK | 61.44 M | four-phase difference | 3072-byte frames | PSS and MSS | pictures | 90 / 72 |
| `dmsp_rtd` | BPSK | 1.024 M | none | 150-bit frames | OLS | pictures | 2 / 1.5 |

Products: VIIRS natural colour (M10, M7, M5), true colour (M5, M4, M3), dust and fog; MODIS natural colour (7, 2, 1), true
colour (1, 4, 3), airmass, dust and fog; AVHRR natural colour (2, 1, 4), cloud-top temperature, sea surface temperature and
fire. The difference products are uncalibrated: relative contrast only. The AVHRR temperatures follow the method of the
NOAA KLM User's Guide section 7 (not checked against the text) with the coefficients of a data file.

## Settings: notes

Give a value as `key=value` after the stack name.

## Not confirmed by a public document

No public recording of any of these links was found, and no public document confirms the profile constants, the instrument
packet layouts (VIIRS, MODIS, AVHRR placement, METimage, TIRS, VEGETATION, AMSR2), or the frame formats of Oceansat-2,
Kanopus-V and DMSP. The `symbol_rate` and `roll_off` settings can replace the profile's values. The rule that an AMSR2
packet with flags 01 ends a line looks unusual and is not confirmed. The test signals are exact inverses of the receivers,
so the tests cannot catch a wrong rule. The stack was not run on a real carrier.

Not decoded: the LDPC code of Landsat X band and the turbo code of STEREO from samples (frame files only), ICER (STEREO) and
CCSDS 122 (Proba-V X band) pictures, the other instruments of these satellites, and calibrated VIIRS and MODIS products.

## Try it: notes

```sh
sdrkit run eodb --source synth:eodb/saral_l --rate 3M --seconds 8 --audio none
sdrkit run eodb --source synth:eodb/dmsp_rtd --rate 2M --seconds 8 --audio none
sdrkit run eodb --source <driver>:<address> --submode saral_l --freq 1698.4
```

The test signal of each profile is its test picture packed as the instrument sends it, in packets, frames and CADUs,
coded and modulated as the profile says, with the spacecraft id of a satellite of the list (NOAA-20 for `jpss_hrd`,
MetOp-B for `metop_dump`).
