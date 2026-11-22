<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# ARDOP

Lists the ARDOP frames and stations heard: frame types, IDs, connect requests, and 4FSK, PSK and QAM data. Sends FEC text, the ID frame and Ping, and makes ARQ links.

Stack id: `ardop`. Group: HF and utility.

Signal types: ARDOP.

## Tuning

| Property | Value |
|---|---|
| Default frequency | 0 Hz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 12 kHz |
| Signal bandwidth | 4 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ardop` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `demod` | Demodulator | choice | `usb` | `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) | How the sideband audio is made. USB is usual on HF. NFM is for SSTV on 2 m. |
| `listen` | Listen | yes or no | false |  | Plays the sideband audio. Decoding is the same either way. |
| `leader_ms` | Leader | whole number (ms) | 240 | 100 to 1000 | The two-tone leader before each frame, 100 to 1000 ms in steps of 20. A receiver needs about 100 ms to find it. |
| `fec_type` | FEC frame type | choice | `4fsk_500_100` | `4fsk_200_50s` (4FSK.200.50S), `4fsk_500_100s` (4FSK.500.100S), `4fsk_500_100` (4FSK.500.100), `4psk_200_100` (4PSK.200.100), `4psk_200_100s` (4PSK.200.100S), `8psk_200_100` (8PSK.200.100), `16qam_200_100` (16QAM.200.100), `4psk_500_100` (4PSK.500.100), `8psk_500_100` (8PSK.500.100), `16qam_500_100` (16QAM.500.100), `4psk_1000_100` (4PSK.1000.100), `8psk_1000_100` (8PSK.1000.100), `16qam_1000_100` (16QAM.1000.100), `4psk_2000_100` (4PSK.2000.100), `8psk_2000_100` (8PSK.2000.100), `16qam_2000_100` (16QAM.2000.100) | The data frames of a text sent with no link: 4FSK.200.50S (16 bytes a frame, 200 Hz), 4FSK.500.100S (32 bytes), 4FSK.500.100 (64 bytes, 500 Hz), or 4PSK, 8PSK or 16QAM at 100 baud on 1, 2, 4 or 8 carriers (200, 500, 1000 or 2000 Hz, 16 to 1024 bytes a frame). |
| `auto_answer` | Answer calls | yes or no | false |  | Answers a connect request to the station callsign with ConAck500 and runs the link as the called station, with no key press. Off at the start, so that nothing is sent unattended: a request is then shown and not answered. The engine's transmit switch must be on. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_data` | Last frame | choice | `header` | `header` (8PSK header only), `4psk_200_100` (4PSK.200.100), `4psk_200_100s` (4PSK.200.100S), `8psk_200_100` (8PSK.200.100), `16qam_200_100` (16QAM.200.100), `4psk_500_100` (4PSK.500.100), `8psk_500_100` (8PSK.500.100), `16qam_500_100` (16QAM.500.100), `4psk_1000_100` (4PSK.1000.100), `8psk_1000_100` (8PSK.1000.100), `16qam_1000_100` (16QAM.1000.100), `4psk_2000_100` (4PSK.2000.100), `8psk_2000_100` (8PSK.2000.100), `16qam_2000_100` (16QAM.2000.100) | The last frame of the test script: the 8PSK.200.100 frame type with no body, or a whole PSK or QAM data frame of the type chosen. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `level_dbfs` | Level | number (dBFS) |
| `audio_dbfs` | Audio level | number (dBFS) |
| `leaders` | Leaders | whole number |
| `good_headers` | Headers | whole number |
| `unreadable_leaders` | Unreadable | whole number |
| `data_ok` | Data good | whole number |
| `data_bad` | Data bad | whole number |
| `rs_corrections` | RS corrections | whole number |
| `control` | Control frames | whole number |
| `other` | Other frames | whole number |
| `stations_heard` | Stations | whole number |
| `frames_ok` | Frames good | whole number |
| `text` | Text | text |
| `dropped` | Dropped | whole number |
| `tx_sending` | Sending | yes or no |
| `tx_frame` | Frame sent | text |
| `tx_waiting` | Bursts waiting | whole number |
| `link_state` | Link | choice |
| `link_peer` | Link station | text |
| `link_session` | Session | whole number |
| `link_sent` | Bytes sent | whole number |
| `link_received` | Bytes received | whole number |
| `link_waiting` | Bytes waiting | whole number |
| `link_retries` | Repeats | whole number |
| `link_reason` | Link reason | text |
| `link_text` | Link text | text |
| `link_request` | Request from | text |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `frames` | Frames | `time_s` (number (s)), `type` (whole number), `name` (text), `kind` (text), `modulation` (text), `baud` (whole number (Bd)), `session` (whole number), `from` (text), `to` (text), `locator` (text), `text` (text), `quality` (whole number), `snr_db` (number (dB)), `leader_ms` (whole number (ms)), `header_ok` (yes or no), `body_read` (yes or no), `body_ok` (yes or no), `rs_corrections` (whole number), `tone_offset_hz` (frequency), `tone_snr_db` (number (dB)) |
| `stations` | Stations | `callsign` (text), `locator` (text), `heard` (whole number), `last_heard` (time), `last_type` (text), `called` (text) |

## Result series

| Key | Label | X unit | Y unit |
|---|---|---|---|
| `levels` | Level | s | dB |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ardop.frame` | Frame heard | `source` (text), `target` (text), `family` (choice), `name` (text), `type` (whole number) |
| `hfkbd.tx` | Keyboard transmission | `state` (text), `reason` (text), `text` (text), `family` (choice) |
| `ardop.link` | ARDOP link | `state` (choice), `source` (text), `reason` (text), `text` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `clear` | Clear |  |  | Forgets everything decoded so far. |
| `send` | Send a text | `text` (text, required) |  | While a link is up, the text joins the data of the link. Otherwise it is sent once as FEC data frames of the FEC frame type, to no station. Transmit must be on and the station callsign set. |
| `send_id` | Send the ID |  |  | Sends one ID frame with the station callsign and locator. Transmit must be on and the station callsign set. |
| `ping` | Ping | `target` (text, required) |  | Sends one Ping frame from the station callsign to the called station. Transmit must be on and the station callsign set. |
| `connect` | Connect | `target` (text, required) |  | Calls the station with ConReq500M, every 9 s until it answers, at most 11 times. On the answer the link is up: the text of `send` goes as 4FSK.500.100 data frames, each sent again until the other station acknowledges it. Transmit must be on and the station callsign set. |
| `disconnect` | Disconnect |  |  | Ends the link once the data waiting is sent: DISC, answered by END. |
| `stop` | Stop sending |  |  | Ends the burst on air at once, drops the bursts waiting and closes the link without DISC. |

## Typed interface

The stack is in the family `hfkbd`, version 2.7. The C header `sdrkit/hfkbd.h`, the registry record `registry/hfkbd.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

### Result fields

Struct `FrontFields`. The levels that the front end measures. A level has no value before the first input.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `level_dbfs` | number (dBFS) | optional | 1.0 | The power of the input. |
| `audio_dbfs` | number (dBFS) | optional | 1.0 | The power of the 200 to 3800 Hz sideband before the AGC. |

### Result fields

Struct `ArdopFields`. The result fields of ARDOP.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `leaders` | whole number | always | 1.0 | Leaders heard. |
| `good_headers` | whole number | always | 1.0 | Headers read. |
| `unreadable_leaders` | whole number | always | 1.0 | Leaders whose header did not read. |
| `data_ok` | whole number | always | 1.0 | Data frames good. |
| `data_bad` | whole number | always | 1.0 | Data frames bad. |
| `rs_corrections` | whole number | always | 1.0 | Reed-Solomon corrections. |
| `control` | whole number | always | 1.0 | Control frames. |
| `other` | whole number | always | 1.0 | Other frames. |
| `stations_heard` | whole number | always | 1.0 | Stations heard. |
| `frames_ok` | whole number | always | 1.0 | Frames heard. |
| `text` | text | optional | 1.0 | The newest text, if a frame held one. |

### Result fields

Struct `DecoderFields`. The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `dropped` | whole number | optional | 1.0 | Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread. |

### Result fields

Struct `ArdopTxFields`. The result fields of the ARDOP sender and of its link.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 2.6 | Whether a burst is on air. |
| `tx_frame` | text | optional | 2.6 | The type of the frame on air. No value while nothing is sent. |
| `tx_waiting` | whole number | always | 2.6 | The bursts waiting to be sent. |
| `link_state` | choice | always | 2.6 | The state of the link. |
| `link_peer` | text | optional | 2.6 | The other station of the link. No value while no link was made. |
| `link_session` | whole number | optional | 2.6 | The session id of the link. No value while no link was made. |
| `link_sent` | whole number | always | 2.6 | The data bytes of the stack that the other station acknowledged. |
| `link_received` | whole number | always | 2.6 | The data bytes that the other station sent and the stack received. |
| `link_waiting` | whole number | always | 2.6 | The data bytes waiting to be sent on the link. |
| `link_retries` | whole number | always | 2.6 | The repeats in a row of the frame that waits for its answer. |
| `link_reason` | text | always | 2.6 | Why the link state is what it is; empty when it needs no reason. |
| `link_text` | text | optional | 2.6 | The text received on the link, the newest 4000 bytes. No value before the first byte. |
| `link_request` | text | optional | 2.6 | The station of the newest connect request to the station callsign that the stack did not answer. |

### Table `frames`

Struct `ArdopFrameRow`. A row of the ARDOP frames. The row key is the number of the frame, counted from the start.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `time_s` | number (s) | always | 1.0 | The signal time. |
| `type` | whole number | always | 1.0 | The frame type. |
| `name` | text | always | 1.0 | The name of the type. |
| `kind` | text | always | 1.0 | What the frame is. |
| `modulation` | text | always | 1.0 | Its modulation. |
| `baud` | whole number (Bd) | always | 1.0 | Its speed. |
| `session` | whole number | always | 1.0 | The session id. |
| `from` | text | optional | 1.0 | The sender, if the frame names one. |
| `to` | text | optional | 1.0 | The addressee, if the frame names one. |
| `locator` | text | optional | 1.0 | The locator, if the frame holds one. |
| `text` | text | optional | 1.0 | The text, if the frame holds one. |
| `quality` | whole number | optional | 1.0 | The quality that the frame reports. |
| `snr_db` | number (dB) | optional | 1.0 | The SNR, if it was measured. |
| `leader_ms` | whole number (ms) | optional | 1.0 | The length of the leader, if it was measured. |
| `header_ok` | yes or no | always | 1.0 | The header read. |
| `body_read` | yes or no | always | 1.0 | The body was read. |
| `body_ok` | yes or no | always | 1.0 | The body is good. |
| `rs_corrections` | whole number | always | 1.0 | Reed-Solomon corrections. |
| `tone_offset_hz` | frequency | always | 1.0 | The offset of the tones. |
| `tone_snr_db` | number (dB) | always | 1.0 | The SNR of the tones. |

### Table `stations`

Struct `ArdopStationRow`. A row of the ARDOP stations. The row key is the callsign.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `callsign` | text | always | 1.0 | The callsign. |
| `locator` | text | optional | 1.0 | The locator, if one was heard. |
| `heard` | whole number | optional | 1.0 | The times heard. |
| `last_heard` | time | optional | 1.0 | The UTC of the last time heard. |
| `last_type` | text | always | 1.0 | The type of its last frame. |
| `called` | text | optional | 1.0 | The station it called, if it called one. |

### Event `ardop.frame`

Struct `ArdopFrameEvent`. The fields of an ARDOP frame heard.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `source` | text | optional | 1.0 | The sender, if the frame names one. |
| `target` | text | optional | 1.0 | The addressee, if the frame names one. |
| `family` | choice | always | 1.0 | The family of the source: a message. |
| `name` | text | always | 1.0 | The name of the type. |
| `type` | whole number | always | 1.0 | The frame type. |

### Event `hfkbd.tx`

Struct `KeyboardTxEvent`. The fields of a change of a keyboard transmission.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 2.4 | The state: started, queued, finished or refused. |
| `reason` | text | always | 2.4 | Why it changed: empty, stopped, the transmitter is off, the format changed, tune carrier, or why a text was refused. |
| `text` | text | always | 2.4 | The text: as given to `send` for started, queued and refused; as coded for finished. |
| `family` | choice | always | 2.4 | The family of the source: a message. |

### Event `ardop.link`

Struct `ArdopLinkEvent`. The fields of a change of the ARDOP link.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 2.6 | The link state after the change. |
| `source` | text | always | 2.6 | The other station. |
| `reason` | text | always | 2.6 | What happened: connected, data received, the other station disconnected, a request not answered, or why the link failed. |
| `text` | text | always | 2.6 | The data received, as text; empty when none came. |
| `family` | choice | always | 2.6 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `remove` | `sdrkit_ardop_remove` | `sdrkit_hfkbd_station_args` | none |
| `clear` | `sdrkit_ardop_clear` | none | none |
| `send` | `sdrkit_ardop_send` | `sdrkit_hfkbd_keyboard_send_args` | none |
| `send_id` | `sdrkit_ardop_send_id` | none | none |
| `ping` | `sdrkit_ardop_ping` | `sdrkit_ardop_call_args` | none |
| `connect` | `sdrkit_ardop_connect` | `sdrkit_ardop_call_args` | none |
| `disconnect` | `sdrkit_ardop_disconnect` | none | none |
| `stop` | `sdrkit_ardop_stop` | none | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `demod` | `sdrkit_ardop_get_demod` | `sdrkit_ardop_set_demod` |
| `listen` | `sdrkit_ardop_get_listen` | `sdrkit_ardop_set_listen` |
| `leader_ms` | `sdrkit_ardop_get_leader_ms` | `sdrkit_ardop_set_leader_ms` |
| `fec_type` | `sdrkit_ardop_get_fec_type` | `sdrkit_ardop_set_fec_type` |
| `auto_answer` | `sdrkit_ardop_get_auto_answer` | `sdrkit_ardop_set_auto_answer` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_data` | `sdrkit_ardop_gen_get_data` | `sdrkit_ardop_gen_set_data` |

## Try it

```sh
sdrkit run ardop --source synth:ardop
sdrkit run ardop --source <driver>:<address> --freq 0
```

## Test recordings

- `tests/vectors/hfkbd/hfkbd_ardop_12k.sigmf-meta`
<!-- generated: end -->

## Transmit

The stack sends FEC text, the ID frame and Ping, and makes ARQ links as the calling station. The engine setting
`transmit_allowed` is the only switch: with it off every request is refused, a burst on air ends at once and a link closes.
Every request that sends needs the station callsign: 1 to 8 letters, digits, `-` or `/`.

### Commands and settings

| Command | Effect |
|---|---|
| `send` (`text`) | While a link is up, the text joins the data of the link. Otherwise it is sent once as FEC data frames, to no station (session id 0xFF), at most 1024 bytes. |
| `send_id` | One ID frame: the station callsign and its locator. |
| `ping` (`target`) | One Ping frame from the station callsign to `target`. |
| `connect` (`target`) | Calls `target` with ConReq500M and runs the link. |
| `disconnect` | Ends the link once the data waiting is sent. |
| `stop` | Ends the burst on air at once, drops the bursts waiting and closes the link without DISC. |

| Key | Meaning | Default |
|---|---|---|
| `leader_ms` | The two-tone leader of each frame, 100 to 1000 ms in steps of 20 | 240 |
| `fec_type` | The data frames of FEC text: `4fsk_200_50s` (16 bytes a frame), `4fsk_500_100s` (32), `4fsk_500_100` (64) | `4fsk_500_100` |
| `auto_answer` | Answers a connect request to the station callsign with no key press | off |

`auto_answer` is off at the start, so that nothing is sent unattended. Off, a connect request to the station callsign is
shown in `link_request` and in an `ardop.link` event, and nothing is sent. `demod` is shared with the receiver: on LSB the
signal is the mirror image; NFM and AM refuse every request.

### The signal

The tones are written at the stream rate as the upper sideband of the dial, at a peak of 0.5 of full scale; each leader tone
is at 0.25. A symbol is a sine of phase zero at its start, as its analytic signal, times a sign of +1 or -1. Every tone has a
whole number and a half of cycles in its symbol, so a sign that alternates gives a continuous phase.

- Leader: `leader_ms` / 20 symbols of 20 ms, each the sum of 1475 and 1525 Hz. Symbol i before the last has the sign
  (-1)^(N + i); the last, the sync symbol, has +1, the sign of the symbol before it: that repeat is the phase reversal the
  receiver finds.
- Frame type: ten symbols at 50 baud on 1425 + 50 s Hz: the type byte, its check symbol, the type XOR the session id, the check
  symbol again. Signs +1, -1, ...
- Body: each byte as four symbols, the most significant 2-bit group first. 4FSK.500.100 and 4FSK.500.100S at 100 baud on
  1350, 1450, 1550, 1650 Hz; the other bodies at 50 baud on 1425 to 1575 Hz.
- Each frame rises and falls by a 1 ms raised cosine inside its first and last symbol.
- A burst is 0.1 s of silence and its frames, 0.15 s apart. It ends at the last sample of its last frame, with the end mark.
  On a live radio whose host gives a transmit lead, the first sample states its time on air.

Times on air with a 240 ms leader: ID, ConReq and Ping 1.72 s; ConAck 0.68 s; ACK, NAK, IDLE, BREAK, DISC and END 0.44 s; a
4FSK.500.100 data frame 3.76 s.

The FEC frames alternate the even and the odd type, and the toggle runs on from one text to the next, so a receiver can tell
a new frame from a repeat. Only the
4FSK frame types that the receiver reads are sent; the PSK, QAM and 600 baud types are not built.

### The link

The link uses ConReq500M (0x32) and ConAck500 (0x3A, with the leader of `leader_ms`), 4FSK.500.100 data frames of up to 64
bytes (0x4A even, 0x4B odd), DataACK 0xF0 and DataNAK 0x10 (quality 70), IDLE, BREAK, DISC and END. The session id is the low
byte of the frame check over the caller's callsign followed by the called station's (upper case), 0xFF made 0xFE: N0CALL to
N1CALL is 0x7C. There is no change of speed and no choice of bandwidth. The link works between two stacks of the library; it
is not checked against other ARDOP stations.

- A frame waits 9 s for its answer and is sent again; after 10 repeats in a row the link fails with "no answer from the
  other station" (or closes with "no answer to DISC").
- The sending side sends data, else IDLE; with nothing to send it sends IDLE again 4 s after the last ACK.
- The receiving side answers a good data frame with ACK and gives its data once (a frame with the toggle of the last one is a
  repeat); a bad one with NAK. When it has data and hears IDLE, it sends BREAK and takes the turn on the ACK.
- DISC is answered by END. When that END is lost, a DISC repeated within 99 s is answered again; nothing else is sent after
  a link closed. A receiving side that hears nothing for 99 s fails.
- A frame heard within 0.05 s of a burst of the stack itself is its own and is ignored: a radio that receives while it sends
  does not turn the link. A frame of the other station starts at least 0.2 s after an own burst ends.
- `auto_answer` answers only when the stack could send: transmit on, USB or LSB, the rate, a callsign a frame can carry.

Result fields: `tx_sending`, `tx_frame`, `tx_waiting` (bursts), `link_state` (`idle`, `connecting`, `sending`, `receiving`,
`disconnecting`, `closed`, `failed`), `link_peer`, `link_session`, `link_sent` and `link_received` (data bytes), `link_waiting`,
`link_retries`, `link_reason`, `link_text` (the newest 4000 bytes received) and `link_request`. Events: `hfkbd.tx` for each
burst (started, queued, finished, refused) and `ardop.link` for each change of the link and each piece of data received.

### Checks

- The worked examples: the 240 ms leader signs, the symbols of ACK 0xF0 with session 0x7C, the times on air 1.72 s, 0.44 s and
  3.76 s, the session ids 0x7C and 0xA3. Each sample equals the symbol formula to 1e-3, on USB and LSB.
- The stack's own receiver reads the FEC text whole in each frame type, the ID frame (callsign and locator) and the Ping
  without noise. At 12 dB in 2.5 kHz over 10 seeds: 4FSK.200.50S 9 of 10, 4FSK.500.100S 10, 4FSK.500.100 10, the ID frame 10.
- 99 % power bandwidth of an FEC burst: 4FSK.200.50S 215 Hz (1390 to 1606 Hz above the dial), 4FSK.500.100S 416 Hz and
  4FSK.500.100 418 Hz (1288 to 1705 Hz). The 200 Hz type is 8 % wider than its name: its tones span 150 Hz at 50 baud.
- The link table with whole frames lost at 20 %: 2000 bytes one way and 500 back, over 5 seeds, identical bytes, DISC and END,
  in 340 to 460 s. With every frame lost after the connection the caller fails with "no answer from the other station".
- Two stacks on one channel at 20 dB, each hearing its own frames, leaders of 100 ms, passes of 20 ms, 20 % of the bursts
  lost: connect, 2000 bytes, 500 back, disconnect, identical bytes, both closed (451 s). With answering off the called stack
  shows the request and sends nothing. After a link closed, only a repeated DISC within 99 s of the stack's END is answered.
- Refused with transmit off, without a callsign, with a callsign a frame cannot carry, with a bad called station, on NFM.

### Settled and open points

- Session id: made from the two callsigns as above; the public specification's rule is not confirmed.
- Not built: answering a Ping with PingAck, the frame types of 1000 and 2000 Hz, PSK, QAM and 600 baud data, the change of
  speed by the quality reports, ConRejBusy and ConRejBW, the leader length measured on a ConReq, repeats of FEC frames, and the
  ID frame at the start and end of a link.
