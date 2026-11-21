<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# Lightning position (network)

Each station times sferics to the microsecond on a disciplined sample clock; one station, the server, collects the detections of all and locates each strike by time difference of arrival, with a 95 % error ellipse.

Stack id: `lightning_tdoa`. Group: Science, time and utility.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 100 kHz |
| Tuning range | 3 kHz to 30 MHz |
| Sample rate | 1 MHz |
| Smallest sample rate | 100 kHz |
| Signal bandwidth | 500 kHz |
| Radio filter | 500 kHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:lightning_tdoa` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `role` | Role | choice | `local` | `local` (Local), `client` (Client), `server` (Server) | Local times sferics and sends nothing. A client sends to the server. The server collects and locates. |
| `server` | Server address (client) | text | empty |  | host or host:port. |
| `port` | UDP port | whole number | 47390 | 1 to 65535 |  |
| `network_key` | Network key (secret) | text | empty |  | 12 characters or more; shorter keeps the network off. The host keeps it; it is never sent. |
| `station_name` | Station name | text | empty |  | 1 to 24 printable characters. Its hash is the station id. |
| `server_is_station` | The server's radio counts as a station | yes or no | true |  |  |
| `system_clock` | Computer clock | choice | `ntp` | `system` (Not synchronised), `ntp` (NTP), `ptp` (PTP) |  |
| `system_sigma_us` | Computer clock error | number (µs) | 0 | 1 to 1000000 |  |
| `use_radio_time` | Use the radio's GPS time | yes or no | true |  | GPS marks that the host gives (time_mark) are used. |
| `timing_id` | Timing transmitter id | whole number | 1 | 1 to 255 | The same at every station of a network. |
| `timing_label` | Timing transmitter | text | empty |  |  |
| `timing_hz` | Timing transmitter frequency | frequency | 0 Hz | 3 kHz to 30 MHz | Used when it lies within 0.45 times the rate of the tuned centre. |
| `timing_baud` | Timing transmitter symbol rate | number (Bd) | 200 | 10 to 2000 | No public document gives the rate of each transmitter; 200 Bd is assumed. |
| `timing_lat` | Timing transmitter latitude | number (°) | 0 | -90 to 90 |  |
| `timing_lon` | Timing transmitter longitude | number (°) | 0 | -180 to 180 |  |
| `threshold_db` | Sferic threshold | number (dB) | 12 | 6 to 30 | A sferic starts when the 10 µs power average rises this far over the floor. |
| `velocity_factor` | Ground-wave velocity / c | number | 0.9993 | 0.99 to 1 | An assumption: 0.1 % error moves fixes by about 0.3 km at 400 km. |
| `hops` | Ionosphere hops for long paths | yes or no | false |  | Paths of 1500 km or more travel by hops between the ground and the reflecting layer. |
| `hop_height_km` | Reflection height | number (km) | 80 | 60 to 100 |  |
| `min_stations` | Fine stations for a fix | whole number | 4 | 3 to 64 | Three-station fixes, which nothing checks, only at 3. |
| `start_utc` | UTC of sample 0 (files; 0 = host clock) | number (s) | 0 | 0 to 100000000000 |  |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_strike_rate` | Test signal: strikes a second | number (/s) | 2 | 0 to 50 | Strikes come at random gaps, at least 2 ms apart, within 3° of latitude and 4° of longitude of 52.5 N 13.4 E. |
| `gen_timing_tx` | Test signal: timing transmitter | yes or no | false |  | A 200 Bd MSK transmitter at 52.0 N 10.0 E, amplitude 0.05. |
| `gen_timing_offset_hz` | Test signal: timing transmitter offset | frequency | 50 kHz | -1 MHz to 1 MHz | From the tuned centre; at most the rate / 2.5. |
| `gen_station_lat` | Test signal: station latitude | number (°) | 52.5 | -90 to 90 | Where the test signal is heard, when the host gives no station position. |
| `gen_station_lon` | Test signal: station longitude | number (°) | 13.4 | -180 to 180 |  |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `role` | Role | text |
| `pulses` | Sferics timed | whole number |
| `last_peak_db` | Last peak | number (dB) |
| `floor_db` | Floor | number (dBFS) |
| `time_source` | Time source | text |
| `clock_sigma_us` | Clock error | number (µs) |
| `clock_ppm` | Sample clock drift | number (ppm) |
| `clock_ppm_sigma` | Drift error | number (ppm) |
| `fine` | Fine timing | yes or no |
| `sigma_gps_us` | GPS clock | number (µs) |
| `sigma_gnss_us` | GNSS clock | number (µs) |
| `sigma_cv_us` | Common-view clock | number (µs) |
| `sigma_ptp_us` | PTP clock | number (µs) |
| `sigma_ntp_us` | NTP clock | number (µs) |
| `sigma_system_us` | Computer clock | number (µs) |
| `common_view` | Common view in use | yes or no |
| `timing_locked` | Timing transmitter locked | yes or no |
| `timing_snr_db` | Timing SNR | number (dB) |
| `timing_sigma_us` | Timing boundary error | number (µs) |
| `timing_carrier_hz` | Timing carrier error | number (Hz) |
| `network` | Network | text |
| `sent` | Packets sent | whole number |
| `send_errors` | Send errors | whole number |
| `packets_ok` | Packets accepted | whole number |
| `refused_mac` | Refused (MAC) | whole number |
| `replays` | Replays | whole number |
| `stale` | Stale | whole number |
| `malformed` | Malformed | whole number |
| `rate_limited` | Rate limited | whole number |
| `groups` | Groups | whole number |
| `fixes` | Fixes | whole number |
| `rejected` | Rejected | whole number |
| `unmatched` | Unmatched | whole number |
| `coarse_only` | Coarse only | whole number |
| `too_long` | Too long | whole number |
| `note` | Note | text |
| `dropped` | Dropped | whole number |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `strikes` | Strikes | `utc_ns` (whole number (ns)), `lat` (number (°)), `lon` (number (°)), `major_km` (number (km)), `minor_km` (number (km)), `az_deg` (number (°)), `stations` (whole number), `also_heard` (whole number), `rms_us` (number (µs)), `chi2` (number), `dof` (whole number), `unchecked` (yes or no), `ambiguous` (yes or no), `utc` (yes or no) |
| `stations` | Stations | `id` (text), `name` (text), `lat` (number (°)), `lon` (number (°)), `time_source` (text), `sigma_us` (number (µs)), `age_s` (number (s)), `detections` (whole number), `used` (whole number), `fine` (yes or no), `tied_to_utc` (yes or no), `cv_correction_us` (number (µs)) |

## Map points

The stack puts points on a map: Lightning strikes.

## Events

| Kind | Label | Fields |
|---|---|---|
| `lightning.strike` | Lightning strike | `utc_ns` (whole number (ns)), `lat` (number (°)), `lon` (number (°)), `major_km` (number (km)), `minor_km` (number (km)), `az_deg` (number (°)), `stations` (whole number), `also_heard` (whole number), `rms_us` (number (µs)), `chi2` (number), `dof` (whole number), `unchecked` (yes or no), `ambiguous` (yes or no), `utc` (yes or no) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `reset` | Reset |  |  | Empties the tables and counters and learns the floor of the sferic timer again; the clock discipline is kept. |
| `datagram` | Feed a datagram | `bytes` (bytes, required) |  | The server takes one datagram as if it came from the network. |
| `time_mark` | Time mark | `source` (text, required), `sample` (whole number, required), `utc_ns` (whole number (ns), required), `sigma_ns` (number (ns), default 0), `bias_ns` (number (ns), default 0) |  | UTC at an input sample, from the radio's GPS time or a GNSS receiver, as the host reads it. |

## Typed interface

The stack is in the family `science`, version 1.0. The C header `sdrkit/science.h`, the registry record `registry/science.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `LightningTdoaFields`. The result fields of `lightning_tdoa`.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `role` | text | always | 1.0 | The role in use. |
| `pulses` | whole number | always | 1.0 | Sferics timed. |
| `last_peak_db` | number (dB) | optional | 1.0 | The peak of the newest sferic. |
| `floor_db` | number (dBFS) | optional | 1.0 | The floor of the sferic timer. |
| `time_source` | text | always | 1.0 | The best usable time source. |
| `clock_sigma_us` | number (µs) | optional | 1.0 | Its 1-sigma now. |
| `clock_ppm` | number (ppm) | optional | 1.0 | The rate error of its sample clock. |
| `clock_ppm_sigma` | number (ppm) | optional | 1.0 | The 1-sigma of the drift. |
| `fine` | yes or no | always | 1.0 | A detection now would be fine. |
| `sigma_gps_us` | number (µs) | optional | 1.0 | The 1-sigma of the GPS clock. |
| `sigma_gnss_us` | number (µs) | optional | 1.0 | The 1-sigma of the GNSS clock. |
| `sigma_cv_us` | number (µs) | optional | 1.0 | The 1-sigma of the common-view clock. |
| `sigma_ptp_us` | number (µs) | optional | 1.0 | The 1-sigma of the PTP clock. |
| `sigma_ntp_us` | number (µs) | optional | 1.0 | The 1-sigma of the NTP clock. |
| `sigma_system_us` | number (µs) | optional | 1.0 | The 1-sigma of the unsynchronised computer clock. |
| `common_view` | yes or no | always | 1.0 | The common-view clock stamps the detections. |
| `timing_locked` | yes or no | optional | 1.0 | The timing transmitter is locked. |
| `timing_snr_db` | number (dB) | optional | 1.0 | Its SNR. |
| `timing_sigma_us` | number (µs) | optional | 1.0 | The 1-sigma of its boundary. |
| `timing_carrier_hz` | number (Hz) | optional | 1.0 | Its carrier error. |
| `network` | text | always | 1.0 | The state of the network. |
| `sent` | whole number | always | 1.0 | Datagrams sent. |
| `send_errors` | whole number | always | 1.0 | Datagrams that could not be sent. |
| `packets_ok` | whole number | optional | 1.0 | Datagrams accepted by the server. |
| `refused_mac` | whole number | optional | 1.0 | Refused by the MAC. |
| `replays` | whole number | optional | 1.0 | Refused as replays. |
| `stale` | whole number | optional | 1.0 | Refused as stale. |
| `malformed` | whole number | optional | 1.0 | Refused as malformed. |
| `rate_limited` | whole number | optional | 1.0 | Refused by the rate limit. |
| `groups` | whole number | optional | 1.0 | Groups of detections tried. |
| `fixes` | whole number | optional | 1.0 | Fixes made. |
| `rejected` | whole number | optional | 1.0 | Fixes refused by the chi-square test. |
| `unmatched` | whole number | optional | 1.0 | Groups without a fix. |
| `coarse_only` | whole number | optional | 1.0 | Groups with too few fine stations. |
| `too_long` | whole number | always | 1.0 | Impulses too long for lightning. |
| `note` | text | always | 1.0 | What no public document confirms. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Table `strikes`

Struct `LightningTdoaStrikeRow`. A located strike (server). The row key is the sequence number of the strike, eight digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc_ns` | whole number (ns) | always | 1.0 | The time of a strike, UTC ns. |
| `lat` | number (°) | always | 1.0 | Latitude. |
| `lon` | number (°) | always | 1.0 | Longitude. |
| `major_km` | number (km) | always | 1.0 | The semi-major axis of the 95 % ellipse. |
| `minor_km` | number (km) | always | 1.0 | The semi-minor axis. |
| `az_deg` | number (°) | always | 1.0 | The azimuth of the major axis. |
| `stations` | whole number | always | 1.0 | Stations used. |
| `also_heard` | whole number | always | 1.0 | Coarse stations that heard it. |
| `rms_us` | number (µs) | always | 1.0 | The rms of the residuals. |
| `chi2` | number | always | 1.0 | Chi-square. |
| `dof` | whole number | always | 1.0 | Degrees of freedom. |
| `unchecked` | yes or no | always | 1.0 | No degree of freedom checks it. |
| `ambiguous` | yes or no | always | 1.0 | A second solution fits as well. |
| `utc` | yes or no | always | 1.0 | The time is UTC. |

### Table `stations`

Struct `LightningTdoaStationRow`. A station of the network (server). The row key is the station id, eight hex digits.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `id` | text | always | 1.0 | The station id. |
| `name` | text | always | 1.0 | The station name. |
| `lat` | number (°) | always | 1.0 | Latitude. |
| `lon` | number (°) | always | 1.0 | Longitude. |
| `time_source` | text | always | 1.0 | The best usable time source. |
| `sigma_us` | number (µs) | always | 1.0 | The 1-sigma of the station's clock. |
| `age_s` | number (s) | always | 1.0 | The time since the station's last datagram. |
| `detections` | whole number | always | 1.0 | Detections received. |
| `used` | whole number | always | 1.0 | Detections used in fixes. |
| `fine` | yes or no | always | 1.0 | A detection now would be fine. |
| `tied_to_utc` | yes or no | always | 1.0 | The station's clock is UTC. |
| `cv_correction_us` | number (µs) | optional | 1.0 | The common-view correction. |

### Event `lightning.strike`

Struct `LightningStrikeEvent`. A strike located (server).

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `utc_ns` | whole number (ns) | always | 1.0 | The time of a strike, UTC ns. |
| `lat` | number (°) | always | 1.0 | Latitude. |
| `lon` | number (°) | always | 1.0 | Longitude. |
| `major_km` | number (km) | always | 1.0 | The semi-major axis of the 95 % ellipse. |
| `minor_km` | number (km) | always | 1.0 | The semi-minor axis. |
| `az_deg` | number (°) | always | 1.0 | The azimuth of the major axis. |
| `stations` | whole number | always | 1.0 | Stations used. |
| `also_heard` | whole number | always | 1.0 | Coarse stations that heard it. |
| `rms_us` | number (µs) | always | 1.0 | The rms of the residuals. |
| `chi2` | number | always | 1.0 | Chi-square. |
| `dof` | whole number | always | 1.0 | Degrees of freedom. |
| `unchecked` | yes or no | always | 1.0 | No degree of freedom checks it. |
| `ambiguous` | yes or no | always | 1.0 | A second solution fits as well. |
| `utc` | yes or no | always | 1.0 | The time is UTC. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_lightning_tdoa_clear` | none | none |
| `reset` | `sdrkit_lightning_tdoa_reset` | none | none |
| `datagram` | `sdrkit_lightning_tdoa_datagram` | `sdrkit_lightning_tdoa_datagram_args` | none |
| `time_mark` | `sdrkit_lightning_tdoa_time_mark` | `sdrkit_lightning_tdoa_time_mark_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `role` | `sdrkit_lightning_tdoa_get_role` | `sdrkit_lightning_tdoa_set_role` |
| `server` | `sdrkit_lightning_tdoa_get_server` | `sdrkit_lightning_tdoa_set_server` |
| `port` | `sdrkit_lightning_tdoa_get_port` | `sdrkit_lightning_tdoa_set_port` |
| `network_key` | `sdrkit_lightning_tdoa_get_network_key` | `sdrkit_lightning_tdoa_set_network_key` |
| `station_name` | `sdrkit_lightning_tdoa_get_station_name` | `sdrkit_lightning_tdoa_set_station_name` |
| `server_is_station` | `sdrkit_lightning_tdoa_get_server_is_station` | `sdrkit_lightning_tdoa_set_server_is_station` |
| `system_clock` | `sdrkit_lightning_tdoa_get_system_clock` | `sdrkit_lightning_tdoa_set_system_clock` |
| `system_sigma_us` | `sdrkit_lightning_tdoa_get_system_sigma_us` | `sdrkit_lightning_tdoa_set_system_sigma_us` |
| `use_radio_time` | `sdrkit_lightning_tdoa_get_use_radio_time` | `sdrkit_lightning_tdoa_set_use_radio_time` |
| `timing_id` | `sdrkit_lightning_tdoa_get_timing_id` | `sdrkit_lightning_tdoa_set_timing_id` |
| `timing_label` | `sdrkit_lightning_tdoa_get_timing_label` | `sdrkit_lightning_tdoa_set_timing_label` |
| `timing_hz` | `sdrkit_lightning_tdoa_get_timing_hz` | `sdrkit_lightning_tdoa_set_timing_hz` |
| `timing_baud` | `sdrkit_lightning_tdoa_get_timing_baud` | `sdrkit_lightning_tdoa_set_timing_baud` |
| `timing_lat` | `sdrkit_lightning_tdoa_get_timing_lat` | `sdrkit_lightning_tdoa_set_timing_lat` |
| `timing_lon` | `sdrkit_lightning_tdoa_get_timing_lon` | `sdrkit_lightning_tdoa_set_timing_lon` |
| `threshold_db` | `sdrkit_lightning_tdoa_get_threshold_db` | `sdrkit_lightning_tdoa_set_threshold_db` |
| `velocity_factor` | `sdrkit_lightning_tdoa_get_velocity_factor` | `sdrkit_lightning_tdoa_set_velocity_factor` |
| `hops` | `sdrkit_lightning_tdoa_get_hops` | `sdrkit_lightning_tdoa_set_hops` |
| `hop_height_km` | `sdrkit_lightning_tdoa_get_hop_height_km` | `sdrkit_lightning_tdoa_set_hop_height_km` |
| `min_stations` | `sdrkit_lightning_tdoa_get_min_stations` | `sdrkit_lightning_tdoa_set_min_stations` |
| `start_utc` | `sdrkit_lightning_tdoa_get_start_utc` | `sdrkit_lightning_tdoa_set_start_utc` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_strike_rate` | `sdrkit_lightning_tdoa_gen_get_strike_rate` | `sdrkit_lightning_tdoa_gen_set_strike_rate` |
| `gen_timing_tx` | `sdrkit_lightning_tdoa_gen_get_timing_tx` | `sdrkit_lightning_tdoa_gen_set_timing_tx` |
| `gen_timing_offset_hz` | `sdrkit_lightning_tdoa_gen_get_timing_offset_hz` | `sdrkit_lightning_tdoa_gen_set_timing_offset_hz` |
| `gen_station_lat` | `sdrkit_lightning_tdoa_gen_get_station_lat` | `sdrkit_lightning_tdoa_gen_set_station_lat` |
| `gen_station_lon` | `sdrkit_lightning_tdoa_gen_get_station_lon` | `sdrkit_lightning_tdoa_gen_set_station_lon` |

## Try it

```sh
sdrkit run lightning_tdoa --source synth:lightning_tdoa
sdrkit run lightning_tdoa --source <driver>:<address> --freq 0.1
```
<!-- generated: end -->

## Description

A lightning discharge radiates a radio impulse, a sferic, that every station within a few hundred kilometres hears. Each station of this stack times the sferics it hears to a fraction of a microsecond: the receiver averages the power over 10 µs, finds impulses over its own noise floor, and takes the half-peak point of the leading edge, interpolated between samples. A disciplined sample clock turns the sample index into UTC with a stated error. Its marks come from the host's computer clock (PTP, NTP or not synchronised), from GPS or GNSS time that the host gives (`time_mark`), and from the symbol clock of a VLF MSK timing transmitter that all stations receive (common view). Only detections with a GPS, GNSS or common-view stamp of 20 µs or better take part in a fix; the others are only "also heard". One station, the server, receives the detections of all stations as signed UDP datagrams, aligns the common-view clocks on the boundary readings of the timing transmitter, matches the detections of one strike, and locates it by time difference of arrival with a 95 % error ellipse. 1 µs of timing error is about 300 m. Nothing goes on air; the datagrams carry sdrkit's own format and travel only between the user's own stations. Not confirmed by a public document: the figures and rules of the timing sources (their 1-sigmas and biases, the fit windows, the rate prior of 50 ppm, the refusal of outlying marks), the sferic timer (the 10 µs power average, the floor of 1 ms means, the end 3 dB under the threshold after 100 µs, 5 ms as too long, the 1 ms merge), the MSK timer, the 200 Bd symbol rate of the timing transmitters, the ground-wave velocity 0.9993 c and the hop model, the common-view alignment, the matching bounds, the solver's start grid and limits, and the packet format, which is sdrkit's own. The framework gives no hardware timestamp of a sample, so GPS and GNSS time comes only from the host through the `time_mark` command.

## Tuning: notes

The stack holds no dial and no transmitter list: the usual dial is in `data/vlf/vlf_dials.csv` and the timing transmitters are in `data/vlf/timing_transmitters.csv`, for the host to offer. The timing transmitter is used only when its carrier lies within 0.45 times the rate of the tuned centre; a notch on its carrier then runs in front of the sferic timer. The front end's blind IQ correction is off for this stack, because it would take impulses for an imbalance.

## Settings: notes

Advanced settings. The owner works without any of them set.

The station position comes from the host's station; without it nothing is sent. The network is on only for a client or a server with a key of 12 characters or more (and, for a client, a station name and a server address) and a host that gives a network; the `network` field says why it is off. A recording takes the computer clock as `start_utc + sample / rate`; live samples take the host's clock when each group of samples arrives.

## Result fields: notes

`time_source` is the best clock usable now (GPS, GNSS, common view, PTP, NTP or system clock); `clock_sigma_us` and `clock_ppm` are its 1-sigma now and the measured rate error of the radio's sample clock (positive: fast). The per-source fields are empty for a clock without marks. The server fields (`packets_ok` to `coarse_only`) are empty unless the role is server. The lamp searches until the floor is valid, tracks, and locks while a detection now would be fine.

## Result tables: notes

Both tables are the server's: the 500 newest strikes and up to 64 stations. Each strike is also a map point (label: stations and ellipse). `utc` is false when no GPS or GNSS station ties the common-view clocks to UTC; the time then carries an unknown epoch. `unchecked` marks a three-station fix and `ambiguous` one with a second solution that fits as well.

## Commands: notes

`clear` and `reset` keep the clocks, the timing-transmitter timer and the replay guard of the server. `datagram` is refused when the role is not server, the network key is shorter than 12 characters, or the datagram fails a check. `time_mark` takes `gps` (the 1-sigma 20 ns, a bias of 100 ns and two samples; refused when `use_radio_time` is off) or `gnss` (`sigma_ns` 0 takes 1.5 × 5 m / c; `bias_ns` is the error of moving the fix onto this radio's samples).

## Try it: notes

```sh
sdrkit run lightning_tdoa --source synth:lightning_tdoa --seconds 8
sdrkit run lightning_tdoa --source synth:lightning_tdoa gen_timing_tx=true timing_hz=150000 timing_lat=52 timing_lon=10 --seconds 12
sdrkit run lightning_tdoa --source iiod:192.168.2.1 --freq 0.1 role=client server=10.0.0.5 network_key=a-long-shared-key station_name=north
```
