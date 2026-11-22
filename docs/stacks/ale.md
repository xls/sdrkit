<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->
# HF ALE

Automatic link establishment (2G, 8-FSK): soundings, calls, nets and AMD messages, with the stations heard.

Stack id: `ale`. Group: HF utility.

HF ALE, second generation. ```text iq -> [hf_front 24 kHz] -> [sideband audio 8 kHz] --+--> [word sync] -> ale.word --+--> [call assembler] -> ale.call --+ |                              +-----------------------------------+--> [tables] +--> [listen] -> audio iq, ale.call -> [transmitter: call, sound, amd, stop; answers] -> [tx sink] ```

Standard: MIL-STD-188-141 Appendix A, FED-STD-1045.

Signal types: ALE (2G).

## Tuning

| Property | Value |
|---|---|
| Default frequency | 14.109 MHz |
| Tuning range | Anywhere the radio tunes |
| Sample rate | 2 MHz |
| Smallest sample rate | 250 kHz |
| Signal bandwidth | 3 kHz |
| Offset from the signal | 20 kHz |
| Radio filter | 1.75 MHz |
| Automatic gain | Radio AGC |

## Settings

Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:ale` prints this table with every option.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `nets` | Net addresses | text | empty |  | Net addresses you know, separated by commas. Calls to them are net calls at once. Others are found when a station other than the one called answers. |
| `listen` | Listen | yes or no | false |  | Play the sideband (dial + 200 Hz to 3.8 kHz). Decoding is the same either way. |
| `demod` | Demodulator | choice | `auto` | `auto` (Auto), `usb` (USB), `lsb` (LSB), `nfm` (NFM), `am` (AM) | USB below 30 MHz, NFM above (Auto). The decoders see the same audio whether or not you listen. |
| `self_address` | Own address | text | empty |  | 1 to 15 characters of A-Z and 0-9, sent in the TIS or TWS words. Empty refuses call and sound. |
| `call_repeats` | Calling cycle repeats | whole number | 6 | 1 to 100 | How many times a call sends the called address before the own address, and a sounding the own address. 6 is the figure of the specification for one channel. |
| `sounding` | Sounding word | choice | `tws` | `tws` (TWS, no answer), `tis` (TIS, answer welcome) |  |
| `scan_channels` | Scanned channels | whole number | 0 | 0 to 100 | The channels in the scan list of the called station. A call first sends the called address for the scan time (channels x dwell), so that the station finds the call when it next listens on this channel. 0 sends no scanning part. |
| `scan_dwell_s` | Dwell on a channel | number (s) | 0.5 | 0.1 to 1 | How long the called station listens on each channel of its scan: 0.5 s at 2 channels a second, 0.2 s at 5. |
| `auto_answer` | Answer calls | yes or no | false |  | Answers a call to the own address that ends in TIS with the response, and so completes the three-way handshake, with no key press. Off at the start, so that nothing is sent unattended. The engine's transmit switch must be on. |
| `lqa` | Exchange link quality | yes or no | false |  | A call carries the LQA command, so that the response carries the figures of the call as heard; the acknowledgement then carries the figures of the response. Answers carry them also when the other station asks. |
| `answer_wait_s` | Wait for an answer | number (s) | 2 | 0.5 to 30 | The longest time from the end of a call or a response to the start of its answer. The other station hears the end of a transmission two words (0.784 s) after its last word. |

Settings of the test signal generator. They change the signal of the `synth:` source of the stack.

| Key | Label | Kind | Default | Range or options | Help |
|---|---|---|---|---|---|
| `gen_calls` | Calls | choice | `test_calls` | `test_calls` (The test calls), `sounding` (The sounding only) |  |
| `gen_ber` | Channel bit errors | number | 0 | 0 to 0.2 | The share of channel bits turned over before they become tones. |

## Result fields

| Key | Label | Kind |
|---|---|---|
| `in_sync` | Word sync | yes or no |
| `words_recent` | Words | yes or no |
| `words_ok` | Words decoded | whole number |
| `words_missed` | Words missed | whole number |
| `calls` | Calls | whole number |
| `stations` | Stations | whole number |
| `offset_hz` | Offset | frequency |
| `snr_db` | SNR | number (dB) |
| `ber` | Bit error rate | number |
| `level_dbfs` | Level | number (dBFS) |
| `clock` | Clock | choice |
| `tx_sending` | Sending | yes or no |
| `tx_kind` | Transmission | choice |
| `tx_to` | Called address | text |
| `tx_word` | Word sent | whole number |
| `tx_words` | Words to send | whole number |
| `tx_note` | Transmit note | text |
| `tx_link` | Link | choice |
| `tx_link_station` | Link station | text |
| `tx_link_reason` | Link reason | text |

## Result tables

| Key | Label | Columns |
|---|---|---|
| `events` | Events | `start` (duration (s)), `time` (time), `dial` (frequency), `from` (text), `to` (text), `type` (choice), `tis` (yes or no), `amd` (text), `commands` (text), `ber` (number), `snr_db` (number (dB)), `offset_hz` (frequency), `words` (whole number) |
| `stations` | Stations | `address` (text), `dial` (frequency), `snr_db` (number (dB)), `ber` (number), `last_type` (choice), `heard` (whole number), `last_heard` (time) |
| `words` | Words | `start` (duration (s)), `time` (time), `type` (choice), `text` (text), `votes` (whole number), `golay` (whole number) |
| `learned_nets` | Learned nets | `address` (text) |
| `lqa` | Link quality | `address` (text), `dial` (frequency), `lqa_ber` (whole number), `sinad_db` (whole number (dB)), `multipath` (whole number), `reported_ber` (whole number), `reported_sinad_db` (whole number (dB)), `reported_multipath` (whole number), `heard` (whole number), `last_heard` (time) |
| `messages` | Messages | `start` (duration (s)), `time` (time), `dial` (frequency), `from` (text), `to` (text), `text` (text) |

## Events

| Kind | Label | Fields |
|---|---|---|
| `ale.call` | ALE call | `type` (choice), `source` (text), `target` (text), `text` (text), `tis` (yes or no), `amd` (text), `commands` (text), `ber` (number), `snr_db` (number (dB)), `offset_hz` (frequency), `words` (whole number), `family` (choice) |
| `ale.tx` | ALE transmission | `state` (text), `reason` (text), `kind` (choice), `to` (text), `family` (choice) |
| `ale.link` | ALE link | `state` (choice), `source` (text), `reason` (text), `family` (choice) |

## Commands

| Key | Label | Arguments | Reply | Help |
|---|---|---|---|---|
| `clear` | Clear |  |  | Forgets everything heard so far. Favourites stay. |
| `remove` | Remove | `row` (text, required) |  | Forgets one object, a favourite too. |
| `call` | Call | `to` (text, required) |  | Sends one individual call on this channel: the called address in TO words the calling cycle repeats times, then the own address in TIS words. Transmit must be on and the own address set. |
| `sound` | Sound |  |  | Sends one sounding on this channel: the own address in TWS or TIS words, the calling cycle repeats times. Transmit must be on and the own address set. |
| `stop` | Stop sending |  |  | Ends the call or the sounding at once. |
| `amd` | Send AMD text | `to` (text, required), `text` (text, required) |  | Sends an AMD text. To the linked station: the called address twice, the text, the own address in TWS words. To another station: a call with the text before the own address, which starts the three-way handshake. Transmit must be on and the own address set. |

## Typed interface

The stack is in the family `hfdata`, version 1.6. The C header `sdrkit/hfdata.h`, the registry record `registry/hfdata.reg` and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.

Content kind: Utility decoders (`utility`). Media: sound (monitor).

### Result fields

Struct `AleFields`. The result fields of ALE.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `in_sync` | yes or no | always | 1.0 | In word sync (the lock state "Sync"). |
| `words_recent` | yes or no | always | 1.0 | A word was decoded in the last 60 s. |
| `words_ok` | whole number | always | 1.0 | Words decoded. |
| `words_missed` | whole number | always | 1.0 | Words missed. |
| `calls` | whole number | always | 1.0 | Calls in the event log. |
| `stations` | whole number | always | 1.0 | Stations heard. |
| `offset_hz` | frequency | always | 1.0 | The frequency correction. |
| `snr_db` | number (dB) | optional | 1.0 | The signal-to-noise ratio of the last call. Absent before a call. |
| `ber` | number | optional | 1.0 | The bit error rate of the last call. Absent before a call. |
| `level_dbfs` | number (dBFS) | always | 1.0 | The level of the audio, dBFS. |
| `clock` | choice | always | 1.0 | Where the times come from. |

### Result fields

Struct `AleTxFields`. The result fields of the ALE transmitter. The values of a transmission have no value while nothing is sent.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `tx_sending` | yes or no | always | 1.2 | Whether a call or a sounding is being sent. |
| `tx_kind` | choice | optional | 1.2 | What is being sent. |
| `tx_to` | text | optional | 1.2 | The called address of a call. |
| `tx_word` | whole number | optional | 1.2 | The word being sent, from 1. |
| `tx_words` | whole number | optional | 1.2 | The words of the transmission. |
| `tx_note` | text | always | 1.2 | What the transmitter does not do. |
| `tx_link` | choice | always | 1.3 | The state of the three-way handshake. |
| `tx_link_station` | text | optional | 1.3 | The other station of the handshake. Absent while idle. |
| `tx_link_reason` | text | always | 1.3 | Why the link state is what it is; empty when it needs no reason. |

### Table `events`

Struct `AleEventRow`. A call in the ALE event log. The row key is the sequence number of the call.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start` | duration (s) | always | 1.0 | The signal time of the first word. |
| `time` | time | optional | 1.0 | The UTC of the first word, when the clock is known. |
| `dial` | frequency | optional | 1.0 | The dial it was heard on, when known. |
| `from` | text | optional | 1.0 | The caller, when known. |
| `to` | text | always | 1.0 | The TO addresses then the THRU members. |
| `type` | choice | always | 1.0 | The type of the call. |
| `tis` | yes or no | always | 1.0 | TIS (else TWS). |
| `amd` | text | always | 1.0 | The AMD text. |
| `commands` | text | always | 1.0 | The commands. |
| `ber` | number | always | 1.0 | The bit error rate of the call. |
| `snr_db` | number (dB) | always | 1.0 | The mean word SNR in 3 kHz. |
| `offset_hz` | frequency | always | 1.0 | The frequency correction at the end of the call. |
| `words` | whole number | always | 1.0 | Words in the transmission. |

The column `from` names a row.

### Table `stations`

Struct `AleStationRow`. A caller of ALE. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The address. |
| `dial` | frequency | optional | 1.0 | The dial of the last call, when known. |
| `snr_db` | number (dB) | always | 1.0 | The SNR of the last call. |
| `ber` | number | always | 1.0 | The bit error rate of the last call. |
| `last_type` | choice | always | 1.0 | The type of the last call. |
| `heard` | whole number | optional | 1.0 | Times heard. |
| `last_heard` | time | optional | 1.0 | When last heard. |

The column `address` names a row.

### Table `words`

Struct `AleWordRow`. A decoded ALE word. The row key is the sequence number of the word.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start` | duration (s) | always | 1.0 | The signal time of its first symbol. |
| `time` | time | optional | 1.0 | The UTC, when the clock is known. |
| `type` | choice | always | 1.0 | The word type. |
| `text` | text | always | 1.0 | The three characters of the word (non-printing ones as '.'). |
| `votes` | whole number | always | 1.0 | Vote errors. |
| `golay` | whole number | always | 1.0 | Golay corrections. |

### Table `learned_nets`

Struct `AleNetRow`. A net address learned from the answers. The row key is the address.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.0 | The address. |

The column `address` names a row.

### Table `lqa`

Struct `AleLqaRow`. The link quality of a station on a channel. The row key is the address and the dial.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `address` | text | always | 1.3 | The address. |
| `dial` | frequency | optional | 1.3 | The dial, when known. |
| `lqa_ber` | whole number | always | 1.3 | The mean vote errors a word of its last call, 0 to 30. |
| `sinad_db` | whole number (dB) | always | 1.3 | The SINAD of its last call, 0 to 30 dB. |
| `multipath` | whole number | always | 1.3 | The multipath figure of its last call (not measured: 0). |
| `reported_ber` | whole number | optional | 1.3 | The BER that the station reported of this station's signal, in its last call to the own address with an LQA report. |
| `reported_sinad_db` | whole number (dB) | optional | 1.3 | The SINAD that the station reported of this station's signal, dB. |
| `reported_multipath` | whole number | optional | 1.3 | The multipath that the station reported. |
| `heard` | whole number | optional | 1.3 | Calls heard from it on this channel. |
| `last_heard` | time | optional | 1.3 | When last heard on this channel. |

The column `address` names a row.

### Table `messages`

Struct `AleMessageRow`. An AMD text heard. The row key is the sequence number of the text.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `start` | duration (s) | always | 1.3 | The signal time of the call's first word. |
| `time` | time | optional | 1.3 | The UTC of the call's first word, when the clock is known. |
| `dial` | frequency | optional | 1.3 | The dial it was heard on, when known. |
| `from` | text | optional | 1.3 | The caller, when known. |
| `to` | text | always | 1.3 | The called addresses. |
| `text` | text | always | 1.3 | The text. |

The column `from` names a row.

### Event `ale.call`

Struct `AleCallEvent`. An ALE call: `source` is the caller, `target` the called addresses and `text` the call for people.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `type` | choice | always | 1.0 | The type of the call. |
| `source` | text | always | 1.0 | The caller. |
| `target` | text | always | 1.0 | The called addresses. |
| `text` | text | always | 1.0 | The call for people. |
| `tis` | yes or no | always | 1.0 | TIS (else TWS). |
| `amd` | text | always | 1.0 | The AMD text. |
| `commands` | text | always | 1.0 | The commands. |
| `ber` | number | always | 1.0 | The bit error rate of the call. |
| `snr_db` | number (dB) | always | 1.0 | The mean word SNR in 3 kHz. |
| `offset_hz` | frequency | always | 1.0 | The frequency correction at the end of the call. |
| `words` | whole number | always | 1.0 | Words in the transmission. |
| `family` | choice | always | 1.0 | What the event is about: a message. |

### Event `ale.tx`

Struct `AleTxEvent`. The fields of a change of the ALE transmission.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | text | always | 1.2 | The state: started, finished or refused. |
| `reason` | text | always | 1.2 | Why it changed: empty, stopped, the transmitter is off, the format changed, or why it was refused. |
| `kind` | choice | always | 1.2 | What is sent. |
| `to` | text | always | 1.2 | The called address of a call; empty for a sounding. |
| `family` | choice | always | 1.2 | The family of the source: a message. |

### Event `ale.link`

Struct `AleLinkEvent`. The fields of a step of the three-way handshake.

| Key | Kind | Value | Since | Description |
|---|---|---|---|---|
| `state` | choice | always | 1.3 | The link state after the step. |
| `source` | text | always | 1.3 | The other station. |
| `reason` | text | always | 1.3 | The step: the call sent, the call heard, the response heard, the acknowledgement sent or heard, or why there was no answer. |
| `family` | choice | always | 1.3 | The family of the source: a message. |

### Commands

| Key | C function | Arguments struct | Reply struct |
|---|---|---|---|
| `clear` | `sdrkit_ale_clear` | none | none |
| `remove` | `sdrkit_ale_remove` | `sdrkit_ale_station_args` | none |
| `call` | `sdrkit_ale_call` | `sdrkit_ale_call_args` | none |
| `sound` | `sdrkit_ale_sound` | none | none |
| `stop` | `sdrkit_ale_stop` | none | none |
| `amd` | `sdrkit_ale_amd` | `sdrkit_ale_amd_args` | none |

### Settings

| Key | C getter | C setter |
|---|---|---|
| `nets` | `sdrkit_ale_get_nets` | `sdrkit_ale_set_nets` |
| `listen` | `sdrkit_ale_get_listen` | `sdrkit_ale_set_listen` |
| `demod` | `sdrkit_ale_get_demod` | `sdrkit_ale_set_demod` |
| `self_address` | `sdrkit_ale_get_self_address` | `sdrkit_ale_set_self_address` |
| `call_repeats` | `sdrkit_ale_get_call_repeats` | `sdrkit_ale_set_call_repeats` |
| `sounding` | `sdrkit_ale_get_sounding` | `sdrkit_ale_set_sounding` |
| `scan_channels` | `sdrkit_ale_get_scan_channels` | `sdrkit_ale_set_scan_channels` |
| `scan_dwell_s` | `sdrkit_ale_get_scan_dwell_s` | `sdrkit_ale_set_scan_dwell_s` |
| `auto_answer` | `sdrkit_ale_get_auto_answer` | `sdrkit_ale_set_auto_answer` |
| `lqa` | `sdrkit_ale_get_lqa` | `sdrkit_ale_set_lqa` |
| `answer_wait_s` | `sdrkit_ale_get_answer_wait_s` | `sdrkit_ale_set_answer_wait_s` |

### Settings of the test signal generator

| Key | C getter | C setter |
|---|---|---|
| `gen_calls` | `sdrkit_ale_gen_get_calls` | `sdrkit_ale_gen_set_calls` |
| `gen_ber` | `sdrkit_ale_gen_get_ber` | `sdrkit_ale_gen_set_ber` |

## Try it

```sh
sdrkit run ale --source synth:ale
sdrkit run ale --source <driver>:<address> --freq 14.109
```

## Test recordings

- `tests/vectors/hfdata/ale_24k.sigmf-meta`
<!-- generated: end -->

## Settings: notes

Advanced settings. The owner works without any of them set.

## Transmit

The stack sends calls, soundings and AMD texts on the channel it is tuned to, and answers calls in the three-way handshake (call, response, acknowledgement). The engine's transmit switch must be on and the own address (`self_address`, 1 to 15 characters of A-Z and 0-9) set; an empty own address refuses `call`, `sound` and `amd` with an error that names `self_address`.

- `call` with the argument `to`: the called address in TO words, sent `call_repeats` times (default 6), then the own address with TIS first. An empty `to` calls the allcall `@?@`, and the own address then follows with TWS.
- `sound`: the own address in TWS words (or TIS words, with `sounding=tis`), sent `call_repeats` times.
- `amd` with the arguments `to` and `text`: an AMD text of 1 to 90 characters of ASCII-64 (space to underscore; lower case is sent as upper case). To the linked station (an empty `to` names it): the called address twice, the text (CMD, then DATA and REP words, padded with spaces), the own address in TWS words. To another station: a call with the text before the own address, which starts the handshake.
- `stop`: ends the transmission with a 2 ms ramp and ends a handshake in progress.

Scanning call: when the called station scans a list of channels, set `scan_channels` (the channels of its list, 0 to 100, default 0: no scanning part) and `scan_dwell_s` (its dwell on each channel, 0.1 to 1 s, default 0.5 s, that is 2 channels a second; 0.2 s at 5). A call then first sends the called address in TO words for at least the scan time (channels x dwell), whole addresses only, and then the call itself. The station finds the call when it next listens on this channel and reads it to its end. Example: 12 channels at 0.5 s and the address ABCDEF (two words): 8 times the address (6.272 s) before the 6 of the call.

The handshake. A call to the own address that ends in TIS is answered when `auto_answer` is on (it is off at the start): the response is the caller's address twice in TO words, then the own address in TIS words. A response to the own call is answered by the acknowledgement: the called address twice in TO words, then the own address in TWS words. The stack answers as soon as its receiver closes the call it heard (two words, 0.784 s, after its last word). It waits for an answer `answer_wait_s` (default 2 s) from the end of its call or response to the start of the answer, plus the length of the answer and the two words that close it. The result field `tx_link` gives the state: idle, calling, responding (answered, waiting for the acknowledgement), linked, timed out, or not answered; `tx_link_station` the other station and `tx_link_reason` why. Each step is an `ale.link` event. With the engine's transmit switch off, nothing is sent: a call to the own address is heard and logged, and the state is not answered with the reason "the transmitter is off". The specification marks the timing of the standard for the handshake as not confirmed; the waits are settings.

LQA. The stack measures the link quality of each call it hears from its words: BER is the mean vote errors a word (0 to 30), SINAD is 10 log10(1 + SNR) dB from the mean word SNR in 3 kHz (0 to 30). Multipath is not measured and is sent as 0. These figures are not taken from the standard's text; the specification defines the fields of the LQA command only. The table `lqa` keeps them per station and dial, with the figures the station reported of this station's signal in the LQA command of its last call to the own address. `stop` and a change of format end a link. With `lqa` on, a call carries the LQA command with no figures (BER and SINAD 31) to ask for them; the response then carries the figures of the call as heard, and the acknowledgement the figures of the response. An answer also carries them when the call or the response it answers had an LQA command.

AMD texts heard go to the table `messages` (signal time, UTC, dial, from, to, text), and stay in the event log.

The signal is that of the receiver and of MIL-STD-188-141 Appendix A: each 24-bit word (3-bit type, three 7-bit characters) is coded as two extended Golay (24,12) halves, the second inverted, interleaved with a stuff bit to 49 bits, and sent three times, three bits a tone. The eight tones are 750 to 2500 Hz in 250 Hz steps above the dial (upper sideband), 8 ms each, with continuous phase over the whole transmission and raised-cosine ramps of 2 ms at its start and end. A word lasts 392 ms; the call from XYZ to ABC is 7 words, 2.744 s. The level is 0.5 of full scale (-6 dBFS), constant envelope, at any session rate from 12 ksps. Each transmission ends with an end mark; on a live radio whose host gives a transmit lead, its first sample states its time on air.

The result fields `tx_sending`, `tx_kind` (call, sounding, response, acknowledgement, AMD message), `tx_to`, `tx_word` and `tx_words` show the transmission and its progress; the event `ale.tx` reports started, finished and refused.

Not built: scanning a list of channels, soundings on a schedule, the multipath figure of LQA, and listening before transmitting; `tx_note` says so.

Example: `sdrkit run ale --source <driver>:<address> --freq 14.109 self_address=XYZ`, then the command `call` with `to=ABC`.
