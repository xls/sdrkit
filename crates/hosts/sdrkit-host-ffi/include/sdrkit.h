/*
 * sdrkit: a software-defined radio engine. MIT licence.
 *
 * Start here. A receiver needs these calls, in this order. The folder examples/ of the repository holds whole
 * programs; examples/c/fm_audio.c is the shortest receiver.
 *
 *     sdrkit_engine_new(folder, &engine)              makes the engine; NULL keeps its files in memory
 *     sdrkit_radio_open(engine, "synth:fm", NULL, &radio)   opens a radio; "synth:<stack>" is a test signal
 *     sdrkit_input_radio(radio)                       returns the input of a session that reads the radio
 *     sdrkit_stack_start(engine, "fm", &options, &input, &session)   starts the stack at options.centre_hz
 *     sdrkit_fm_set_force_mono(session, true)         sets one setting; the function is in sdrkit/fm.h
 *     sdrkit_audio_read(session, buffer, capacity, &got)    copies the audio that waits; call it again and again
 *     sdrkit_snapshot_read(session, 0, &snapshot)     takes one copy of the results into the handle
 *     sdrkit_fm_fields_read(session, &fields)         reads the result fields of the copy as a struct
 *     sdrkit_fm_af_rows(session, rows, capacity, &listed)   reads a table of the copy as structs
 *     sdrkit_events_take(session, &count)             takes the new events into the handle
 *     sdrkit_fm_text_event_read(session, n, &event)   reads event n as a struct; SDRKIT_E_WRONG_TYPE for another type
 *     sdrkit_session_set_tuned_hz(session, hz)        changes the frequency of the running session
 *     sdrkit_session_stop(session), sdrkit_session_close(session), sdrkit_radio_close(radio)
 *     sdrkit_free(engine)                             frees the engine and every handle of it
 *
 * Check the code of every call. After an error, sdrkit_error_message(handle, &text) returns one sentence.
 *
 * Typed headers. sdrkit/<family>.h (sdrkit/fm.h, sdrkit/adsb.h, ...) holds the structs and functions of the stacks of
 * one family: result fields, table rows, events, settings and commands. sdrkit/session.h, sdrkit/radio.h and
 * sdrkit/engine.h hold those of every session, every radio and the engine; this header includes them. A typed function
 * is the first choice: the compiler checks its name and its types. The calls of this header that take an id or a key
 * (sdrkit_setting_set(), sdrkit_snapshot_cell(), sdrkit_set(), ...) reach the same things from their declarations, for
 * a program that shows any stack.
 *
 * Handles. An SDRKIT_HANDLE is a number for an engine, an object of an engine, a graph or a host feed. 0 is never a
 * handle. sdrkit_engine_new() makes an engine; with a NULL folder, the engine keeps its files in memory.
 * sdrkit_object_find() and the other sdrkit_object_*() calls return object handles, sdrkit_radio_open() and
 * sdrkit_stack_start() return the handle of the new radio or session, sdrkit_graph_new() returns a graph and
 * sdrkit_feed_open() a host feed. sdrkit_free() frees any handle. Freeing the engine stops every session and frees
 * every handle of the engine. A call on a freed handle, or on a handle whose object no longer exists, returns
 * SDRKIT_E_NO_SUCH_OBJECT.
 *
 * Error codes. Every function returns an int32_t code, except the sdrkit_input_*() constructors and sdrkit_code_name().
 * SDRKIT_OK (0) is success; the SDRKIT_E_* constants are errors, and sdrkit_code_name() returns the name of a code. A
 * call writes its out parameters only on success. After an error, sdrkit_error_detail() fills a struct with the ids,
 * numbers and names of the error, and sdrkit_error_message() returns one sentence for a log. For a call that takes no
 * handle (sdrkit_engine_new()), read the error with handle 0.
 *
 * Pointers. An out parameter must not be NULL: the call returns SDRKIT_E_NULL_POINTER, unless the comment of the
 * function says that the parameter can be NULL. A buffer pointer must be valid for the capacity or count passed with
 * it. A const char* is a NUL-terminated UTF-8 string.
 *
 * Structs with a size. Every options and info struct begins with `uint32_t size`, the number of bytes of the struct.
 * Declare each with its initialiser of sdrkit/init.h, which sets `size` and makes every other member 0:
 *
 *     sdrkit_audio_info got = SDRKIT_AUDIO_INFO_INIT;
 *
 * A struct of a typed header that the library fills (sdrkit_fm_fields, a row, an event) needs no initialiser: its
 * function passes the size. A struct only grows at its end. For a struct that the caller passes (sdrkit_stack_options, sdrkit_input), the
 * library reads the bytes that both sides know and takes 0, the default, for the members that the caller does not
 * have. For a struct that the library fills, the library writes at most `size` bytes and sets `size` to the number
 * that it wrote, so a member that an older library does not know stays 0. A `size` below 4 is SDRKIT_E_BAD_ARGUMENT.
 * sdrkit_string, sdrkit_bytes, sdrkit_position, sdrkit_variant and sdrkit_arg are plain values without a size.
 *
 * Memory. The library owns everything that it returns. A call that returns a string, bytes or an array
 * (sdrkit_string, sdrkit_bytes and the pointers in an info struct) returns a view into a buffer of the handle. The
 * view is valid until the next call on the same handle. Copy what you keep, and free nothing but handles. A string is
 * UTF-8 and has no NUL at the end. The library copies what the caller passes before the call returns.
 *
 * Snapshots. Each handle holds one snapshot of the results, one list of taken events, one command reply and one object
 * list. sdrkit_snapshot_read(), sdrkit_events_take(), sdrkit_command_invoke() and sdrkit_objects() replace them, and the
 * calls that read them read the copy in the handle. Use a handle from one thread at a time. A second thread opens its
 * own handle of the same object with sdrkit_object_find().
 *
 * Object keys, for sdrkit_object_find(): "engine", "radio.r<n>", "session.s<n>", "stack.<id>", "service.<key>",
 * "driver.<key>", "component.<type>". sdrkit_objects() and sdrkit_object() list the live objects. The stack of a session
 * and a component of a session have no key: sdrkit_object_stack() and sdrkit_object_component() return them. The row
 * keys of the engine's tables "radios" and "sessions" are the object keys of the radios and sessions.
 *
 * Ids. Settings, arguments, fields, columns, tables, series, rasters, commands, event types and taps have an
 * sdrkit_id. The ids of a list go from 0 to its count - 1. The *_find() calls return the id of a key. The components
 * of a graph have names: sdrkit_graph_add() names a component, and the other graph calls take that name.
 */

#ifndef SDRKIT_H
#define SDRKIT_H

/* Written by cbindgen from crates/hosts/sdrkit-host-ffi/src. Do not edit. cbindgen.toml says how to write it again. */

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>

// A command sends a text message: a value of the member `content` of `sdrkit_command_info`.
#define SDRKIT_TX_CONTENT_TEXT 0

// A command sends speech.
#define SDRKIT_TX_CONTENT_VOICE 1

// A command sends a beacon or an announcement of the station.
#define SDRKIT_TX_CONTENT_BEACON 2

// A command sends a file, a picture or a stream of media.
#define SDRKIT_TX_CONTENT_FILE 3

// A command sends a frame or a request of its protocol.
#define SDRKIT_TX_CONTENT_FRAME 4

// A command sends an unmodulated carrier, to tune an antenna or an amplifier.
#define SDRKIT_TX_CONTENT_CARRIER 5

// `min`, `max` and `step` hold the range of the value.
#define SDRKIT_DECL_RANGE 1

// The argument has no default: the caller must pass it.
#define SDRKIT_DECL_REQUIRED 2

// A change takes effect after a restart of the object.
#define SDRKIT_DECL_RESTART 4

// The value cannot change.
#define SDRKIT_DECL_FIXED 8

// The setting is advanced: a program shows the other settings first, and these on request.
#define SDRKIT_DECL_ADVANCED 16

// The setting has no role.
#define SDRKIT_SETTING_ROLE_NONE 0

// The setting chooses whose picture the object shows: an instrument, a product.
#define SDRKIT_SETTING_ROLE_PICTURE_SOURCE 1

// The setting chooses the view of the picture shown: a channel, a composite.
#define SDRKIT_SETTING_ROLE_PICTURE_VIEW 2

// The bytes of the value are not declared, or the value is not bytes.
#define SDRKIT_CONTENT_NONE 0

// A picture or media of the codec in `fourcc`.
#define SDRKIT_CONTENT_MEDIA 1

// A plain UTF-8 string.
#define SDRKIT_CONTENT_PLAIN 2

// Comma-separated values.
#define SDRKIT_CONTENT_CSV 3

// JSON.
#define SDRKIT_CONTENT_JSON 4

// XML.
#define SDRKIT_CONTENT_XML 5

// Bytes of no declared format.
#define SDRKIT_CONTENT_BINARY 6

// The engine: its settings, the catalogs and the tables of radios and sessions.
#define SDRKIT_OBJECT_ENGINE 0

// An open radio.
#define SDRKIT_OBJECT_RADIO 1

// A session.
#define SDRKIT_OBJECT_SESSION 2

// The stack of a stack session.
#define SDRKIT_OBJECT_STACK 3

// A component of the graph of a session.
#define SDRKIT_OBJECT_COMPONENT 4

// A stack type of the catalog.
#define SDRKIT_OBJECT_STACK_TYPE 5

// A driver type of the catalog.
#define SDRKIT_OBJECT_DRIVER_TYPE 6

// A component type of the catalog.
#define SDRKIT_OBJECT_COMPONENT_TYPE 7

// A service.
#define SDRKIT_OBJECT_SERVICE 8

// A graph that the caller builds. It is not an object.
#define SDRKIT_OBJECT_GRAPH 9

// A host feed that `sdrkit_feed_open` opened. It is not an object.
#define SDRKIT_OBJECT_FEED 10

// The session reads the open radio `radio`.
#define SDRKIT_INPUT_RADIO 0

// The session reads the host feed `feed`: the samples that the caller pushes with `sdrkit_samples_push`.
#define SDRKIT_INPUT_HOST 1

// The session reads no samples: a component of the graph makes the signal.
#define SDRKIT_INPUT_INSIDE 2

// Success.
#define SDRKIT_OK 0

// A pointer that the call needs is NULL.
#define SDRKIT_E_NULL_POINTER 1

// A string is not UTF-8.
#define SDRKIT_E_BAD_STRING 2

// A constant argument or member is not one of its `SDRKIT_*` values.
#define SDRKIT_E_BAD_ARGUMENT 3

// The handle is of another type than the call takes, for example a graph where the call takes an object.
#define SDRKIT_E_WRONG_HANDLE 4

// The handle was freed, or its object has been removed.
#define SDRKIT_E_NO_SUCH_OBJECT 5

// No object, type, setting, command, table or port has the key.
#define SDRKIT_E_NO_SUCH_KEY 6

// The string is not an object key.
#define SDRKIT_E_BAD_KEY 7

// An id or an index is outside its list.
#define SDRKIT_E_NO_SUCH_ID 8

// The library failed inside. The engine stays usable.
#define SDRKIT_E_PANIC 9

// A value is of another type than its setting, field or argument.
#define SDRKIT_E_WRONG_TYPE 10

// A required value is missing.
#define SDRKIT_E_REQUIRED 11

// A number is outside the range of its setting.
#define SDRKIT_E_OUT_OF_RANGE 12

// A number is not finite.
#define SDRKIT_E_NOT_FINITE 13

// An option index is outside the options of its setting.
#define SDRKIT_E_NOT_AN_OPTION 14

// The object does not allow this value now.
#define SDRKIT_E_NOT_PERMITTED 15

// A string value does not have the form that its setting needs.
#define SDRKIT_E_MALFORMED 16

// The setting cannot change.
#define SDRKIT_E_FIXED 17

// The value conflicts with another setting.
#define SDRKIT_E_CONFLICT 18

// The object does not support the value.
#define SDRKIT_E_UNSUPPORTED 19

// The value needs a station setting of the engine first.
#define SDRKIT_E_NEEDS_STATION 20

// A required argument of a command is missing.
#define SDRKIT_E_MISSING_ARGUMENT 30

// The command has no such argument.
#define SDRKIT_E_NOT_AN_ARGUMENT 31

// The state of the object does not allow the command now.
#define SDRKIT_E_NOT_NOW 32

// The table has no row with the key.
#define SDRKIT_E_NO_SUCH_ROW 33

// The command failed.
#define SDRKIT_E_COMMAND_FAILED 34

// The command transmits, and the transmitter of the session is off: the engine setting `transmit_allowed` is off, or
// the session has no radio that can transmit. Nothing was sent.
#define SDRKIT_E_TRANSMIT_OFF 35

// The file or folder does not exist.
#define SDRKIT_E_NOT_FOUND 40

// The file exists already.
#define SDRKIT_E_ALREADY_EXISTS 41

// The storage is full.
#define SDRKIT_E_NO_SPACE 42

// The storage refused the operation.
#define SDRKIT_E_DENIED 43

// The path is not a valid storage path.
#define SDRKIT_E_INVALID_PATH 44

// Another input or output error.
#define SDRKIT_E_IO 45

// The graph does not build.
#define SDRKIT_E_GRAPH 50

// A connection is refused. The error detail holds the components, the ports and the stream types.
#define SDRKIT_E_CONNECT 51

// A stack or a component refused its configuration (a rate, a bandwidth, a codec).
#define SDRKIT_E_CONFIG 52

// A stage refused its parameters.
#define SDRKIT_E_STAGE 53

// The radio failed or refused.
#define SDRKIT_E_DRIVER 60

// Sessions read the radio.
#define SDRKIT_E_RADIO_IN_USE 61

// The session runs. Stop it first.
#define SDRKIT_E_STILL_RUNNING 62

// The session is stopped.
#define SDRKIT_E_STOPPED 63

// A radio of this address is open, and its driver opens an address once. The engine table `radios` lists the open
// radios with their addresses.
#define SDRKIT_E_RADIO_OPEN 64

// A session of the feed has not used the samples that it read before.
#define SDRKIT_E_BEHIND 65

// The bytes end in a part of a sample.
#define SDRKIT_E_PART_SAMPLE 66

// The session does not run a graph.
#define SDRKIT_E_NO_GRAPH 67

// The saved settings are refused: they are damaged, are not saved settings, or hold a refused value.
#define SDRKIT_E_IMPORT 68

// The saved state is refused.
#define SDRKIT_E_STATE 69

// A service failed.
#define SDRKIT_E_SERVICE 70

// The engine cannot serve a request of a service or a stack.
#define SDRKIT_E_REQUEST 71

// The system did not start a thread.
#define SDRKIT_E_THREAD 72

// A declaration of settings, results or events is not valid.
#define SDRKIT_E_DECLARATION 73

// The engine could not write its saved state.
#define SDRKIT_E_ENCODE 74

// A part of a saved state was not restored.
#define SDRKIT_E_RESTORE 76

// The engine sends the transmit samples of the session to its radio, so the caller cannot read them.
#define SDRKIT_E_TO_RADIO 77

// The session is a channel of the bank: its audio goes to the mix of the bank's session.
#define SDRKIT_E_IN_MIX 78

// A later version of the library wrote the saved settings or the saved state. This version does not read them.
#define SDRKIT_E_VERSION 79

// No stream type, or the type is not known yet.
#define SDRKIT_STREAM_NONE -1

// Complex samples.
#define SDRKIT_STREAM_IQ 0

// Real samples.
#define SDRKIT_STREAM_REAL 1

// Audio.
#define SDRKIT_STREAM_AUDIO 2

// Symbols.
#define SDRKIT_STREAM_SYMBOLS 3

// Hard bits.
#define SDRKIT_STREAM_HARD_BITS 4

// Soft bits.
#define SDRKIT_STREAM_LLR 5

// Bytes.
#define SDRKIT_STREAM_BYTES 6

// Messages.
#define SDRKIT_STREAM_MESSAGES 7

// Spectrum frames.
#define SDRKIT_STREAM_SPECTRUM 8

// Typed items.
#define SDRKIT_STREAM_ITEM 9

// The object does not run.
#define SDRKIT_STATE_OFF 0

// The object looks for a signal.
#define SDRKIT_STATE_SEARCHING 1

// The object has a signal and does not decode it yet.
#define SDRKIT_STATE_TRACKING 2

// The object decodes the signal.
#define SDRKIT_STATE_LOCKED 3

// The table states nothing about its rows.
#define SDRKIT_TABLE_ROLE_NONE 0

// Each row is a message: the main log of messages of the object.
#define SDRKIT_TABLE_ROLE_MESSAGES 1

// Each row is a call or a transmission: who, to whom, when.
#define SDRKIT_TABLE_ROLE_CALLS 2

// Each row is a station, a node or a unit that was heard.
#define SDRKIT_TABLE_ROLE_NODES 3

// Each row is a group that stations talk in: a talkgroup, a channel, a net.
#define SDRKIT_TABLE_ROLE_GROUPS 4

// Each row is a frame, a packet or a report that arrived: a running log. An object can have several.
#define SDRKIT_TABLE_ROLE_LOG 5

// Opens the file for reading. The callback returns `SDRKIT_E_NOT_FOUND` if the file does not exist.
#define SDRKIT_OPEN_READ 0

// Opens the file for writing. The callback makes the file, or empties it if it exists.
#define SDRKIT_OPEN_TRUNCATE 1

// Opens the file for writing at the end. The callback makes the file if it does not exist, and keeps its bytes.
#define SDRKIT_OPEN_APPEND 2

// Opens the file for writing at any offset. The callback makes the file if it does not exist, and keeps its bytes.
#define SDRKIT_OPEN_UPDATE 3

// Makes the file and opens it for writing. The callback returns `SDRKIT_E_ALREADY_EXISTS` if the file exists.
#define SDRKIT_OPEN_CREATE_NEW 4

// 32-bit floats, I then Q.
#define SDRKIT_SAMPLE_CF32 0

// Signed 16-bit integers, I then Q, little-endian.
#define SDRKIT_SAMPLE_CS16 1

// Signed 32-bit integers, I then Q, little-endian.
#define SDRKIT_SAMPLE_CS32 2

// Signed 8-bit integers, I then Q.
#define SDRKIT_SAMPLE_CS8 3

// Unsigned 8-bit integers with zero at 127.5, I then Q.
#define SDRKIT_SAMPLE_CU8 4

// H.264 video.
#define SDRKIT_MEDIA_H264 0

// HEVC video.
#define SDRKIT_MEDIA_HEVC 1

// MPEG-2 video.
#define SDRKIT_MEDIA_MPEG2_VIDEO 2

// AAC audio.
#define SDRKIT_MEDIA_AAC 3

// HE-AAC audio, v1 or v2.
#define SDRKIT_MEDIA_HE_AAC 4

// xHE-AAC audio.
#define SDRKIT_MEDIA_XHE_AAC 5

// AC-3 audio.
#define SDRKIT_MEDIA_AC3 6

// E-AC-3 audio.
#define SDRKIT_MEDIA_EAC3 7

// MPEG-1 Layer I audio.
#define SDRKIT_MEDIA_MP1 8

// MPEG-1 Layer II audio.
#define SDRKIT_MEDIA_MP2 9

// MPEG-1 Layer III audio.
#define SDRKIT_MEDIA_MP3 10

// A still picture, compressed or raw pixels. `fourcc` selects the format.
#define SDRKIT_MEDIA_PICTURE 11

// An MPEG transport stream: whole 188-byte transport packets.
#define SDRKIT_MEDIA_MPEG_TS 12

// Decoded video: one frame of raw pixels in display order. `fourcc` selects the pixel format.
#define SDRKIT_MEDIA_RAW_VIDEO 13

// Bare access units.
#define SDRKIT_PACKAGING_RAW 0

// H.264 or HEVC with start codes.
#define SDRKIT_PACKAGING_ANNEX_B 1

// H.264 or HEVC with a big-endian length of `length_size` bytes before each unit.
#define SDRKIT_PACKAGING_LENGTH_PREFIXED 2

// AAC with ADTS headers.
#define SDRKIT_PACKAGING_ADTS 3

// AAC in LATM / LOAS.
#define SDRKIT_PACKAGING_LATM 4

// Channels in an order that the library does not know.
#define SDRKIT_CHANNEL_LAYOUT_UNKNOWN 0

// One channel.
#define SDRKIT_CHANNEL_LAYOUT_MONO 1

// Left, right.
#define SDRKIT_CHANNEL_LAYOUT_STEREO 2

// 5.1: left, centre, right, left surround, right surround, low-frequency effects.
#define SDRKIT_CHANNEL_LAYOUT_SURROUND_5_1 3

// The packet is no picture.
#define SDRKIT_PICTURE_STATE_NONE 0

// The picture can change: more rows, more parts, or better data. A later packet with the same `picture_id` replaces
// this one.
#define SDRKIT_PICTURE_STATE_GROWING 1

// The picture is complete or its signal ended. A later packet with the same `picture_id` is the same picture drawn
// again, after a change of a setting of its view.
#define SDRKIT_PICTURE_STATE_FINISHED 2

// Pixels whose colour matrix the library does not know, or no pixels.
#define SDRKIT_COLOUR_MATRIX_UNKNOWN 0

// ITU-R BT.601 (standard definition).
#define SDRKIT_COLOUR_MATRIX_BT601 1

// ITU-R BT.709 (high definition).
#define SDRKIT_COLOUR_MATRIX_BT709 2

// ITU-R BT.2020.
#define SDRKIT_COLOUR_MATRIX_BT2020 3

// RGB or grey: no matrix.
#define SDRKIT_COLOUR_MATRIX_IDENTITY 4

// The major version of the C interface of this library: the base header and the rules of the family headers. A
// program built for another major version does not run with this library.
#define SDRKIT_INTERFACE_VERSION_MAJOR 1

// The minor version of the C interface of this library. It rises when the base header gets a function, a constant or
// a member at the end of a struct that starts with `size`.
#define SDRKIT_INTERFACE_VERSION_MINOR 3

// No value.
#define SDRKIT_TYPE_NONE 0

// Yes or no, in `flag`.
#define SDRKIT_TYPE_BOOL 1

// A whole number, in `integer`.
#define SDRKIT_TYPE_I64 2

// A number, in `number`.
#define SDRKIT_TYPE_F64 3

// A string, in `string`.
#define SDRKIT_TYPE_STRING 4

// A frequency in Hz, in `number`.
#define SDRKIT_TYPE_HZ 5

// A time in seconds since 1970 (UTC), in `number`.
#define SDRKIT_TYPE_TIME 6

// A length of time in seconds, in `number`.
#define SDRKIT_TYPE_DURATION 7

// A position in degrees, in `lat` and `lon`.
#define SDRKIT_TYPE_POSITION 8

// One of the options of the declaration, by its index in `enum_index`.
#define SDRKIT_TYPE_ENUM 9

// Raw bytes, in `bytes`.
#define SDRKIT_TYPE_BYTES 10

// Any type: each value carries its own. Only a column or an argument has this type.
#define SDRKIT_TYPE_ANY 11

// An engine, an object of an engine, or a graph. 0 is never a handle.
typedef uint64_t SDRKIT_HANDLE;

// A UTF-8 string of `len` bytes at `ptr`, with no NUL at the end. `ptr` is never NULL.
typedef struct sdrkit_string {
  // The first byte.
  const char *ptr;
  // The number of bytes.
  size_t len;
} sdrkit_string;

// The id of a setting, an argument, a field, a column, a table, a series, a raster, a command, an event type, a component
// or a tap.
typedef uint32_t sdrkit_id;

// The declaration of one command.
typedef struct sdrkit_command_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The key.
  struct sdrkit_string key;
  // The name for people.
  struct sdrkit_string label;
  // One or two sentences for people.
  struct sdrkit_string help;
  // The number of arguments. Argument ids go from 0 to `arguments` - 1.
  uint32_t arguments;
  // The number of reply fields. Most commands have none.
  uint32_t reply_fields;
  // The command transmits. The engine refuses it with `SDRKIT_E_TRANSMIT_OFF` while the transmitter of the session is
  // off.
  bool transmits;
  // What the command sends on the air, one of `SDRKIT_TX_CONTENT_*`. -1 if the command does not transmit.
  int32_t content;
  // The key of the command that stops the transmission that this command starts. Empty for a transmission that ends by
  // itself and for a command that does not transmit.
  struct sdrkit_string stopped_by;
} sdrkit_command_info;

// `len` bytes at `ptr`. `ptr` is never NULL in a value that the library writes.
typedef struct sdrkit_bytes {
  // The first byte.
  const uint8_t *ptr;
  // The number of bytes.
  size_t len;
} sdrkit_bytes;

// One value. `type` is an `SDRKIT_TYPE_*` constant and selects the member that holds the value. The other members are 0.
typedef struct sdrkit_variant {
  // An `SDRKIT_TYPE_*` constant.
  int32_t type;
  // For `SDRKIT_TYPE_BOOL`.
  bool flag;
  // For `SDRKIT_TYPE_ENUM`: the index of the option.
  uint32_t enum_index;
  // For `SDRKIT_TYPE_I64`.
  int64_t integer;
  // For `SDRKIT_TYPE_F64`, `SDRKIT_TYPE_HZ`, `SDRKIT_TYPE_TIME` and `SDRKIT_TYPE_DURATION`.
  double number;
  // For `SDRKIT_TYPE_POSITION`: degrees north.
  double lat;
  // For `SDRKIT_TYPE_POSITION`: degrees east.
  double lon;
  // For `SDRKIT_TYPE_STRING`.
  struct sdrkit_string string;
  // For `SDRKIT_TYPE_BYTES`.
  struct sdrkit_bytes bytes;
} sdrkit_variant;

// A value for the setting or argument `id`.
typedef struct sdrkit_arg {
  // The id of the setting or argument.
  sdrkit_id id;
  // The value.
  struct sdrkit_variant value;
} sdrkit_arg;

// The declaration of one value: a setting, an argument, a reply field, a result field, a table column or an event field.
// A member that does not apply to the value is 0 or empty.
typedef struct sdrkit_decl {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The key.
  struct sdrkit_string key;
  // The name for people.
  struct sdrkit_string label;
  // One or two sentences for people. Settings and arguments only.
  struct sdrkit_string help;
  // The section that the setting shows in, or empty.
  struct sdrkit_string section;
  // The unit, or empty.
  struct sdrkit_string unit;
  // An `SDRKIT_TYPE_*` constant.
  int32_t type;
  // `SDRKIT_DECL_*` bits.
  uint32_t flags;
  // The lowest number, if `flags` has `SDRKIT_DECL_RANGE`.
  double min;
  // The highest number, if `flags` has `SDRKIT_DECL_RANGE`.
  double max;
  // The step, or 0 for any number, if `flags` has `SDRKIT_DECL_RANGE`.
  double step;
  // The default value. Settings and arguments only.
  struct sdrkit_variant default_value;
  // What the number 0 means when it is not a quantity ("Automatic"), or empty. With a meaning, 0 is valid outside the
  // range, and a front end shows the meaning in place of the number. Settings and arguments only.
  struct sdrkit_string zero_means;
  // For `SDRKIT_TYPE_ENUM`: the number of options.
  uint32_t options;
  // For a column that holds row keys of another table: the key of the object of that table, or empty for the same
  // object.
  struct sdrkit_string refers_object;
  // The key of that table, or empty.
  struct sdrkit_string refers_table;
  // An `SDRKIT_CONTENT_*` constant: what the bytes of the value hold.
  int32_t content;
  // For `SDRKIT_CONTENT_MEDIA`: the four-character code of the codec.
  uint8_t fourcc[4];
  // The number of values of the setting that people normally use, for the present option of the setting that they
  // depend on. `sdrkit_setting_common()` reads them. Settings only.
  uint32_t common;
  // An `SDRKIT_SETTING_ROLE_*` constant: what the setting chooses. A front end finds the setting that chooses the
  // view of the picture by this member and not by the key. Settings only.
  int32_t role;
  // With `refers_table`: the key of the column of that table that holds the value to pass, or empty. Empty: the
  // value is a row key of the table. With a column, a front end offers the rows of the table and passes the cell
  // of this column of the chosen row; other values stay valid (the addressee of a send command). Arguments and
  // settings.
  struct sdrkit_string refers_column;
  // The place of the field among the key readings of its object, from 1: what a person looks at first. 0 for a
  // field that is no key reading. Result fields only.
  uint32_t reading;
} sdrkit_decl;

// One option of a value of type `SDRKIT_TYPE_ENUM`.
typedef struct sdrkit_option {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The key that data files use.
  struct sdrkit_string key;
  // The name for people.
  struct sdrkit_string label;
} sdrkit_option;

// The input of a new session. Make it with `sdrkit_input_radio`, `sdrkit_input_host` or `sdrkit_input_inside`.
typedef struct sdrkit_input {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct.
  uint32_t size;
  // An `SDRKIT_INPUT_*` constant.
  int32_t type;
  // For `SDRKIT_INPUT_RADIO`: the handle of the radio. For `SDRKIT_INPUT_HOST`: the handle of the feed.
  SDRKIT_HANDLE radio;
  // Not read. The rate of a host feed is set by `sdrkit_feed_open`.
  double rate_hz;
  // Not read. The centre of a host feed is set by `sdrkit_feed_open`.
  double centre_hz;
} sdrkit_input;

// The options of `sdrkit_radio_open`. All members 0 selects the defaults.
typedef struct sdrkit_radio_options {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct.
  uint32_t size;
  // `count` values of the settings that an open of the address takes, by the ids of `sdrkit_radio_open_setting_find()`:
  // those of the object `driver.<key>` with the same ids, then those that the radio at the address adds, such as the
  // settings of the test signal of a stack. The other settings keep their defaults.
  const struct sdrkit_arg *settings;
  // The number of entries in `settings`.
  size_t count;
} sdrkit_radio_options;

// The options of `sdrkit_stack_start`. All members 0 selects the defaults.
typedef struct sdrkit_stack_options {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct.
  uint32_t size;
  // The submode of the stack, a NUL-terminated string. NULL for the default submode.
  const char *submode;
  // `count` values of the settings of the object `stack.<id>`. The other settings keep their defaults.
  const struct sdrkit_arg *settings;
  // The number of entries in `settings`.
  size_t count;
  // On a radio: the frequency to tune the session to, Hz. 0 selects the default frequency of the stack or submode.
  double centre_hz;
} sdrkit_stack_options;

// What a stack needs on this device by its recorded cost, before a session starts it (`sdrkit_stack_need`).
typedef struct sdrkit_stack_need_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct.
  uint32_t size;
  // The fast cores of this device that the stack needs. 0 for a stack with no recorded cost.
  double cores;
  // The cores that the running sessions leave of the setting `cores_max`.
  double cores_free;
  // The threads that the engine would give the stack now.
  uint32_t threads;
  // The stack keeps up with its signal here: its largest stage fits one core, and the stack fits the free cores.
  bool real_time;
  // The recorded cost of the stack in calibration units per second of signal.
  double units;
  // The calibration units per second of one fast core of this device.
  double core_units;
  // The same of one slow core. Equal to `core_units` where the platform has one class of cores.
  double core_units_slow;
  // The stages of the stack.
  uint32_t stages;
} sdrkit_stack_need_info;

// The detail of the last error of an engine. A member that does not apply holds `SDRKIT_NO_ID` (ids), -1 (`index`), NaN
// (measures), `SDRKIT_STREAM_NONE` or `SDRKIT_TYPE_NONE`.
typedef struct sdrkit_error_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The error code.
  int32_t code;
  // The setting, or the argument of a command.
  sdrkit_id setting;
  // The command.
  sdrkit_id command;
  // The result field.
  sdrkit_id field;
  // The table.
  sdrkit_id table;
  // A count or an index: an option index, the number of sessions that read a radio, a byte count, the error code of a
  // device.
  int64_t index;
  // The name of the component at fault, as `sdrkit_graph_add` named it, or empty. For a refused connection, the
  // component of the input.
  struct sdrkit_string component;
  // The output of a refused connection, `"<component>.<port>"` or `"input"`, or empty.
  struct sdrkit_string from;
  // The input of a refused connection, `"<component>.<port>"`, or empty.
  struct sdrkit_string to;
  // The `SDRKIT_STREAM_*` type of the output.
  int32_t from_type;
  // The `SDRKIT_TYPE_*` type that the value must have.
  int32_t expected;
  // The `SDRKIT_TYPE_*` type of the value.
  int32_t got;
  // The lower limit.
  double min;
  // The upper limit.
  double max;
  // The number in the call.
  double value;
  // For `SDRKIT_E_NO_SUCH_KEY` from the engine: the key, the stack id or the driver key that no object has. Empty
  // otherwise.
  struct sdrkit_string key;
} sdrkit_error_info;

// The declaration of one event type of the catalog.
typedef struct sdrkit_event_type_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The key.
  struct sdrkit_string key;
  // The name for people.
  struct sdrkit_string label;
  // The number of fields. Field ids go from 0 to `fields` - 1.
  uint32_t fields;
} sdrkit_event_type_info;

// One event of the last `sdrkit_events_take`.
typedef struct sdrkit_event_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The id of its event type in the catalog.
  sdrkit_id type;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of fields.
  uint32_t fields;
} sdrkit_event_info;

// One port of a component.
typedef struct sdrkit_port_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The name of the port.
  struct sdrkit_string name;
  // True for an output, false for an input.
  bool output;
  // The `SDRKIT_STREAM_*` type that the port reads or writes, or `SDRKIT_STREAM_NONE` if it is not one fixed type.
  int32_t type;
  // True for an input that can stay unconnected.
  bool optional;
} sdrkit_port_info;

// The size of the report of `sdrkit_graph_validate`.
typedef struct sdrkit_report_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The number of ports that carry a stream, in stream order.
  uint32_t ports;
  // The number of adapters that the graph inserted.
  uint32_t adapters;
} sdrkit_report_info;

// One port of the report, with its resolved format.
typedef struct sdrkit_report_port {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // `<component>.<port>`, or `input`.
  struct sdrkit_string name;
  // The `SDRKIT_STREAM_*` type.
  int32_t type;
  // Items per second.
  double rate_hz;
  // Samples per audio frame.
  uint16_t channels;
  // The radio frequency at 0 Hz of the stream.
  double centre_hz;
} sdrkit_report_port;

// One adapter that the graph inserted.
typedef struct sdrkit_report_adapter {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The name of the adapter in the graph.
  struct sdrkit_string name;
  // Its component type.
  struct sdrkit_string type;
  // The port that it reads.
  struct sdrkit_string source;
  // The port that it writes.
  struct sdrkit_string output;
  // Its group delay in seconds.
  double group_delay_s;
} sdrkit_report_adapter;

// The size of the results of an object.
typedef struct sdrkit_results_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The number of result fields. Field ids go from 0 to `fields` - 1.
  uint32_t fields;
  // The number of tables.
  uint32_t tables;
  // The number of series.
  uint32_t series;
  // The number of rasters.
  uint32_t rasters;
  // The label of the map points, or empty if the object has none.
  struct sdrkit_string points_label;
} sdrkit_results_info;

// The declaration of one table.
typedef struct sdrkit_table_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The key.
  struct sdrkit_string key;
  // The name for people.
  struct sdrkit_string label;
  // The number of columns. Column ids go from 0 to `columns` - 1.
  uint32_t columns;
  // The most rows that the table keeps. When the table is full, a new row removes the oldest row.
  size_t cap;
  // The id of the column that names a row for people (a call sign, a station name), or `SDRKIT_NO_ID`.
  sdrkit_id label_column;
  // The id of the column that orders the rows by default, or `SDRKIT_NO_ID`: the newest row is first. It is a hint
  // for a front end. The rows of a snapshot are always newest first.
  sdrkit_id sort_column;
  // With `sort_column`: true for the highest value first, false for the lowest value first.
  bool sort_descending;
  // The id of the column of type `SDRKIT_TYPE_POSITION` that holds the place of a row on the map, or
  // `SDRKIT_NO_ID`.
  sdrkit_id position_column;
  // The id of the column of the latitude in degrees north, for a table without a position column, or
  // `SDRKIT_NO_ID`.
  sdrkit_id latitude_column;
  // The id of the column of the longitude in degrees east, for a table without a position column, or
  // `SDRKIT_NO_ID`.
  sdrkit_id longitude_column;
  // The id of the column of the height, or `SDRKIT_NO_ID`. The declaration of the column has the unit.
  sdrkit_id altitude_column;
  // The id of the column of the direction of travel in degrees from north, or `SDRKIT_NO_ID`.
  sdrkit_id course_column;
  // The id of the column of the speed, or `SDRKIT_NO_ID`. The declaration of the column has the unit.
  sdrkit_id speed_column;
  // An `SDRKIT_TABLE_ROLE_*` constant: what the rows are. A front end finds the message log, the call log, the
  // nodes and the groups of an object by this member and not by the key of the table.
  int32_t role;
  // The id of the column of the sender of a message or a call, or `SDRKIT_NO_ID`.
  sdrkit_id sender_column;
  // The id of the column of the addressee of a message or a call, or `SDRKIT_NO_ID`.
  sdrkit_id addressee_column;
  // The id of the column of the text of a message, or `SDRKIT_NO_ID`.
  sdrkit_id text_column;
  // The id of the column that holds the time of a row, or `SDRKIT_NO_ID`: a column of type `SDRKIT_TYPE_TIME`, or
  // a number of seconds of signal with the unit `s`. Only a table with a role, or with a column that states it,
  // has it.
  sdrkit_id time_column;
  // The id of the column of type `SDRKIT_TYPE_BOOL` that is true for a message that this station sent, or
  // `SDRKIT_NO_ID`. Without it, in a table with a `delivery_column`, a row with a delivery state is an own message.
  sdrkit_id own_column;
  // The id of the column of the delivery state of an own message, or `SDRKIT_NO_ID`: a column of type
  // `SDRKIT_TYPE_ENUM` that has no value in the row of a received message.
  sdrkit_id delivery_column;
  // The id of the column of the reason of a failed delivery, as text for people, or `SDRKIT_NO_ID`.
  sdrkit_id failure_column;
} sdrkit_table_info;

// The declaration of one series (a small plot) or raster (an image of values, such as a waterfall).
typedef struct sdrkit_plot_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The key.
  struct sdrkit_string key;
  // The name for people.
  struct sdrkit_string label;
  // The unit of the x axis.
  struct sdrkit_string x_unit;
  // The unit of the y axis.
  struct sdrkit_string y_unit;
  // The unit of the values of a raster, or empty for a series.
  struct sdrkit_string unit;
} sdrkit_plot_info;

// The snapshot state that `sdrkit_snapshot_read` writes.
typedef struct sdrkit_snapshot_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The change counter of the results. Pass it as `since` to the next `sdrkit_snapshot_read`.
  uint64_t counter;
  // The declaration of the results changed: read it again. The snapshot holds everything, as for `since` 0.
  bool declaration_changed;
  // A result field changed after `since`. The snapshot holds every field.
  bool fields_changed;
  // An `SDRKIT_STATE_*` constant.
  int32_t state;
  // The signal to noise ratio in dB, or NaN if not known.
  float snr_db;
  // The level in dB full scale, or NaN if not known.
  float level_dbfs;
  // The offset from the tuned frequency in Hz, or NaN if not known.
  double offset_hz;
  // One line for people.
  struct sdrkit_string summary;
  // The number of log lines in the snapshot.
  uint32_t log_lines;
  // The number of map points in the snapshot.
  uint32_t points;
} sdrkit_snapshot_info;

// The rows of one table in the snapshot.
typedef struct sdrkit_rows_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The number of rows that changed after `since`, newest first. A new row counts as changed.
  uint32_t rows;
  // The number of rows in the table.
  uint32_t total;
  // The change counter of the table.
  uint64_t counter;
} sdrkit_rows_info;

// A point on a map.
typedef struct sdrkit_point {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The id of the point.
  struct sdrkit_string id;
  // Degrees north.
  double lat;
  // Degrees east.
  double lon;
  // The name for people.
  struct sdrkit_string label;
  // Degrees from north for a moving target, or NaN.
  float heading;
  // The id of the table that holds the row of the point, or `SDRKIT_NO_ID` for a point without a row.
  sdrkit_id table;
  // The key of that row, or empty. The row can be absent from the table: a table drops its oldest rows.
  struct sdrkit_string row;
} sdrkit_point;

// The points of a series.
typedef struct sdrkit_series_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The series changed after `since`, and `points` holds it.
  bool changed;
  // The change counter of the series.
  uint64_t counter;
  // `count` points, each x then y.
  const float *points;
  // The number of points.
  size_t count;
  // True if the points make a line, false if they are separate points.
  bool line;
} sdrkit_series_info;

// The values of a raster.
typedef struct sdrkit_raster_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The raster changed after `since`, and `values` holds it.
  bool changed;
  // The change counter of the raster.
  uint64_t counter;
  // `width` times `height` values, row by row.
  const float *values;
  // The number of cells in a row.
  size_t width;
  // The number of rows.
  size_t height;
  // The x value at the centre of the first cell of a row, in the x unit of the raster.
  double x_start;
  // The change of x from one cell to the next. It can be negative.
  double x_step;
  // The y value at the centre of the first row, in the y unit of the raster.
  double y_start;
  // The change of y from one row to the next. It can be negative.
  double y_step;
} sdrkit_raster_info;

// Opens the file at `path` as `open_as` (an `SDRKIT_OPEN_*` constant) and writes a number for it to `file`. Opening for
// writing makes the missing folders of the path.
typedef int32_t (*sdrkit_open_fn)(void *context, const char *path, int32_t open_as, uint64_t *file);

// Reads at most `capacity` bytes at `offset` of `file` into `buffer`, and writes the number of bytes read to `got`: 0 at
// the end of the file.
typedef int32_t (*sdrkit_read_fn)(void *context,
                                  uint64_t file,
                                  uint64_t offset,
                                  uint8_t *buffer,
                                  size_t capacity,
                                  size_t *got);

// Writes all `size` bytes of `data` at `offset` of `file`.
typedef int32_t (*sdrkit_write_fn)(void *context,
                                   uint64_t file,
                                   uint64_t offset,
                                   const uint8_t *data,
                                   size_t size);

// Writes the length of `file` to `size`.
typedef int32_t (*sdrkit_len_fn)(void *context, uint64_t file, uint64_t *size);

// Cuts or extends `file` to `size` bytes.
typedef int32_t (*sdrkit_set_len_fn)(void *context, uint64_t file, uint64_t size);

// Makes every write to `file` durable.
typedef int32_t (*sdrkit_sync_fn)(void *context, uint64_t file);

// Closes `file`.
typedef void (*sdrkit_close_fn)(void *context, uint64_t file);

// Removes the file at `path`, or, if `tree` is true, the folder at `path` and everything in it.
typedef int32_t (*sdrkit_remove_fn)(void *context, const char *path, bool tree);

// Moves the file at `from` to `to`. If a file exists at `to`, the callback replaces it.
typedef int32_t (*sdrkit_rename_fn)(void *context, const char *from, const char *to);

// One entry of a folder, or the state of one path.
typedef struct sdrkit_store_entry {
  // The number of bytes of the struct. The side that makes the struct sets it to the sizeof of its struct: the
  // callback for an entry that it adds, the engine for the entry that `stat` fills.
  uint32_t size;
  // The last segment of the path, a NUL-terminated string. The engine copies it before the callback returns.
  const char *name;
  // The length in bytes. 0 for a folder.
  uint64_t len;
  // The time of the last change in seconds since 1970 (UTC), NaN if not known.
  double modified;
  // True for a folder.
  bool is_dir;
} sdrkit_store_entry;

// Adds one entry of a folder to the list of the engine. The entry has its `size` set: use `SDRKIT_STORE_ENTRY_INIT`.
// The engine does not add an entry with a `size` below 4.
typedef void (*sdrkit_add_entry_fn)(void *sink, const struct sdrkit_store_entry *entry);

// Calls `add` with `sink` once for each entry of the folder at `folder`.
typedef int32_t (*sdrkit_list_fn)(void *context,
                                  const char *folder,
                                  void *sink,
                                  sdrkit_add_entry_fn add);

// Fills `entry` with the state of the file or folder at `path`. The engine sets `size`, sets `modified` to
// NaN and the other members to 0 before the call. The callback writes the members that lie within `size`.
typedef int32_t (*sdrkit_stat_fn)(void *context, const char *path, struct sdrkit_store_entry *entry);

// Writes the number of free bytes to `bytes`, and writes false to `known` if the number is not known.
typedef int32_t (*sdrkit_free_space_fn)(void *context, uint64_t *bytes, bool *known);

// Called once when the engine stops using the storage.
typedef void (*sdrkit_release_fn)(void *context);

// The storage callbacks of the caller. Paths are relative, use `/` between segments, and never hold `..`. Each callback
// returns `SDRKIT_OK`, or `SDRKIT_E_NOT_FOUND`, `SDRKIT_E_ALREADY_EXISTS`, `SDRKIT_E_NO_SPACE`, `SDRKIT_E_DENIED` or
// `SDRKIT_E_IO`. The engine calls them from any of its threads. A callback must not call a function of this library.
typedef struct sdrkit_storage {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct.
  uint32_t size;
  // The first argument of every callback.
  void *context;
  // Opens a file.
  sdrkit_open_fn open;
  // Reads from a file.
  sdrkit_read_fn read;
  // Writes to a file.
  sdrkit_write_fn write;
  // The length of a file.
  sdrkit_len_fn len;
  // Cuts or extends a file.
  sdrkit_set_len_fn set_len;
  // Makes the writes to a file durable.
  sdrkit_sync_fn sync;
  // Closes a file.
  sdrkit_close_fn close;
  // Removes a file or a folder.
  sdrkit_remove_fn remove;
  // Moves a file.
  sdrkit_rename_fn rename;
  // Lists a folder.
  sdrkit_list_fn list;
  // The state of a path.
  sdrkit_stat_fn stat;
  // The free space.
  sdrkit_free_space_fn free_space;
  // Called once when the engine stops using the storage. Can be NULL.
  sdrkit_release_fn release;
} sdrkit_storage;

// The audio that `sdrkit_audio_read` copied.
typedef struct sdrkit_audio_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The number of frames copied, or 0 if no audio was ready.
  size_t frames;
  // Frames per second.
  double rate_hz;
  // Samples per frame.
  uint16_t channels;
  // The order of the channels: one of `SDRKIT_CHANNEL_LAYOUT_*`.
  int32_t layout;
} sdrkit_audio_info;

// The complex samples that `sdrkit_transmit_read` copied.
typedef struct sdrkit_iq_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The number of samples copied, or 0 if no samples were ready.
  size_t samples;
  // Samples per second.
  double rate_hz;
  // The frequency at 0 Hz of the samples.
  double centre_hz;
} sdrkit_iq_info;

// The media packet or picture that `sdrkit_media_read` or `sdrkit_picture_read` returned.
typedef struct sdrkit_media_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // True if a packet follows the counter `after`. The other members describe it.
  bool found;
  // The counter of the packet. Pass it as `after` to the next call.
  uint64_t counter;
  // The number of packets that the session dropped between `after` and this packet.
  uint64_t missed;
  // An `SDRKIT_MEDIA_*` constant: the decoder that the packet needs.
  int32_t type;
  // The four-character code of the codec or of the pixel format.
  uint8_t fourcc[4];
  // An `SDRKIT_PACKAGING_*` constant: how packets are framed.
  int32_t packaging;
  // For `SDRKIT_PACKAGING_LENGTH_PREFIXED`: the number of bytes of each length.
  uint8_t length_size;
  // The decoder configuration.
  struct sdrkit_bytes config;
  // The numerator of the unit of `pts` and `dts`, in seconds.
  uint32_t timebase_num;
  // The denominator of that unit.
  uint32_t timebase_den;
  // True if `pts` holds a presentation time.
  bool has_pts;
  // The presentation time.
  int64_t pts;
  // True if `dts` holds a decoding time.
  bool has_dts;
  // The decoding time.
  int64_t dts;
  // True if the packet starts a group that decodes alone.
  bool keyframe;
  // The time of the signal at which the session made the packet, in seconds.
  double time;
  // The bytes of the packet.
  struct sdrkit_bytes bytes;
  // For raw pixels: the width in pixels, else 0.
  uint32_t width;
  // For raw pixels: the height in pixels, else 0.
  uint32_t height;
  // For raw pixels: the bytes per row of each plane, 0 for a plane that does not exist.
  size_t strides[4];
  // For video: the aspect ratio of the displayed picture, width to height (16 and 9). 0 and 0 if it is not known.
  uint32_t aspect_num;
  // The height part of the aspect ratio.
  uint32_t aspect_den;
  // For video: the frames per second as a fraction. 0 and 0 if the rate is not known.
  uint32_t frame_rate_num;
  // The denominator of the frame rate.
  uint32_t frame_rate_den;
  // For decoded video: true if the two fields of the frame are from different times.
  bool interlaced;
  // For decoded video: true if the top field is the first in time.
  bool top_field_first;
  // For decoded video: true if the stream lost or damaged pictures and the frame is the last good one again.
  bool damaged;
  // True if the packet is a keyframe after which nothing refers to data before it: an H.264 or HEVC IDR picture, an
  // HEVC BLA picture, a decoded frame, an audio frame. A decoder that starts at a keyframe that is not a clean start
  // (a recovery point, a CRA picture, an MPEG-2 I picture) can show damage until the next clean start. Some streams
  // have no clean start at all (MPEG-2 video, H.264 with recovery points only): a host that waits for one in vain
  // starts at a keyframe and drops the pictures with an earlier `pts` until the next keyframe.
  bool clean_start;
  // For raw pixels: one of `SDRKIT_COLOUR_MATRIX_*`. `SDRKIT_COLOUR_MATRIX_UNKNOWN` for coded media.
  int32_t colour_matrix;
  // For raw pixels: true for full range (0 to 255), false for video range (16 to 235).
  bool full_range;
  // For a picture: the number of the picture in the session, from 1. Packets with the same number are states of
  // one picture, and each replaces the one before. A new picture has a new number, also when it has the size and
  // the name of the one before. 0 for media.
  uint64_t picture_id;
  // For a picture: one of `SDRKIT_PICTURE_STATE_*`. `SDRKIT_PICTURE_STATE_NONE` for media.
  int32_t picture_state;
  // For a picture: its name (a product, a channel, a picture mode, a file name). Empty if it has none.
  struct sdrkit_string picture_name;
} sdrkit_media_info;

// The spectrum frame that `sdrkit_spectrum_read` copied.
typedef struct sdrkit_spectrum_frame {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // True if a frame came after the last call. The other members describe it.
  bool found;
  // The number of bins copied to the buffer of the caller.
  uint32_t bins;
  // The number of bins of the frame. It is more than `bins` if the buffer was too small.
  uint32_t total;
  // The frequency at the middle bin, in Hz.
  double centre_hz;
  // The width of the frame in Hz.
  double span_hz;
  // Seconds of spectrum before this frame: the number of the frames that the session kept before it, divided by
  // `frame_rate`.
  double time;
  // Frames per second.
  double frame_rate;
} sdrkit_spectrum_frame;

// The part of a tapped stream that `sdrkit_tap_read` copied.
typedef struct sdrkit_tap_info {
  // The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
  // the number of bytes that it wrote.
  uint32_t size;
  // The number of values copied, or 0 if nothing was ready.
  size_t values;
  // The `SDRKIT_STREAM_*` type of the stream. For `SDRKIT_STREAM_IQ` the values are I then Q.
  int32_t type;
  // Items per second.
  double rate_hz;
  // The frequency at 0 Hz of the stream.
  double centre_hz;
  // The number of items that the tap dropped because the caller did not read in time.
  uint64_t dropped;
} sdrkit_tap_info;

// A place on the Earth.
typedef struct sdrkit_position {
  // Degrees north.
  double lat;
  // Degrees east.
  double lon;
  // Metres above the WGS 84 ellipsoid. It holds a value only if `has_alt` is true.
  double alt_m;
  // True if `alt_m` holds a value.
  bool has_alt;
} sdrkit_position;

// No id: the value of an id field of `sdrkit_error_info` that does not apply.
#define SDRKIT_NO_ID 4294967295

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

// Starts a session of the stack `stack` on the radio of `radio` with the defaults of the stack, on the default
// frequency of the stack, and writes its handle to `session`. A stack whose default frequency is 0 stays on the
// frequency of the radio until `sdrkit_session_set_tuned_hz` or the setting `tuned_hz` sets one.
// Returns `SDRKIT_E_NO_SUCH_KEY` for an unknown stack, `SDRKIT_E_WRONG_HANDLE` if `radio` is not a radio, or the codes
// of `sdrkit_stack_start`. `sdrkit_stack_start` is the first choice: it takes the frequency, the settings and any input.
int32_t sdrkit_start(SDRKIT_HANDLE radio,
                     const char *stack,
                     SDRKIT_HANDLE *session);

// Sets the setting with the key `key` of the object of `handle` to `value`, written as text: `yes` or `no` for a
// switch, a number, the key or the label of an option, or the text itself. Returns `SDRKIT_E_NO_SUCH_KEY` if no
// setting has the key, `SDRKIT_E_MALFORMED` if the text is not a value of the setting, or the codes of
// `sdrkit_setting_set`. `sdrkit_error_message` names the setting. The typed function of the setting in the header of
// its family (`sdrkit_fm_set_squelch`, `sdrkit_session_set_tuned_hz`) is the first choice: the compiler checks its name
// and the type of its value.
int32_t sdrkit_set(SDRKIT_HANDLE handle,
                   const char *key,
                   const char *value);

// Runs the command with the key `command` of the object of `handle`. `args` holds `count` pairs of strings: the key of
// an argument, then its value written as text (as `sdrkit_set` reads a value). `args` can be NULL if `count` is 0. An
// argument that is not given has its default. The reply is read with `sdrkit_reply`. Returns `SDRKIT_E_NO_SUCH_KEY` if there is no such command, or the
// codes of `sdrkit_command_invoke`. `sdrkit_error_message` names the command and the argument. The typed function of
// the command in the header of its family (`sdrkit_fm_clear`) is the first choice: its arguments are a struct.
int32_t sdrkit_call(SDRKIT_HANDLE handle,
                    const char *command,
                    const char *const *args,
                    uint32_t count);

// Writes to `is` whether event `n` of the last take is of the event type with the key `event_type` (`mesh.text`).
// Returns `SDRKIT_E_NO_SUCH_ID` if the event does not exist. The typed function of the event type in the header of its
// family (`sdrkit_fm_text_event_read`) is the first choice: it returns `SDRKIT_E_WRONG_TYPE` for another type and fills
// a struct.
int32_t sdrkit_event_is(SDRKIT_HANDLE handle,
                        uint32_t n,
                        const char *event_type,
                        bool *is);

// Writes the field with the key `key` of event `n` of the last take to `text`, as text for people. The text is empty if
// the event has no such field or the field has no value. The string is valid until the next call on `handle` that
// writes a string. Returns `SDRKIT_E_NO_SUCH_ID` if the event does not exist. The typed function of the event type
// (`sdrkit_fm_text_event_read`) is the first choice: it gives each field with its type.
int32_t sdrkit_event_text(SDRKIT_HANDLE handle,
                          uint32_t n,
                          const char *key,
                          struct sdrkit_string *text);

// Writes what is wrong with the session of `session` now to `text`, as sentences: the problem of its receiver, the last
// refused request, and what stops it from transmitting (the transmit switch is off, the transmitter is muted). The text
// is empty if nothing is wrong. Returns `SDRKIT_E_WRONG_HANDLE` if `session` is not a session. `sdrkit_session_fields_read`
// gives the same problems as separate members.
// The string is a view that the library owns, valid until the next call on `session`. Copy what you keep.
int32_t sdrkit_problem(SDRKIT_HANDLE session,
                       struct sdrkit_string *text);

// Writes the number of commands of the object of `handle` to `count`. Command ids go from 0 to `count` - 1.
int32_t sdrkit_commands_count(SDRKIT_HANDLE handle,
                              uint32_t *count);

// Writes the id of the command with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no command has the key.
int32_t sdrkit_command_find(SDRKIT_HANDLE handle,
                            const char *key,
                            sdrkit_id *id);

// Fills `info` with the declaration of command `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the command does not exist.
// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_command_decl(SDRKIT_HANDLE handle,
                            sdrkit_id id,
                            struct sdrkit_command_info *info);

// Runs command `id` of the object of `handle` with the `count` arguments in `args`, and writes the number of reply fields
// to `reply_fields`. An argument that is not in `args` has its default value. The call checks the arguments against the
// declaration before the command runs. `sdrkit_reply` reads the reply. Returns `SDRKIT_E_NO_SUCH_ID`,
// `SDRKIT_E_MISSING_ARGUMENT`, a setting code for a refused argument (the error detail holds its id), `SDRKIT_E_NOT_NOW`,
// `SDRKIT_E_NO_SUCH_ROW`, `SDRKIT_E_TRANSMIT_OFF` or `SDRKIT_E_COMMAND_FAILED`. For a command that a family header declares, its typed function
// (`sdrkit_<stack>_<command>`) is the first choice.
int32_t sdrkit_command_invoke(SDRKIT_HANDLE handle,
                              sdrkit_id id,
                              const struct sdrkit_arg *args,
                              size_t count,
                              uint32_t *reply_fields);

// Writes reply field `id` of the last command that ran on `handle` to `value`. Returns `SDRKIT_E_NO_SUCH_ID` if the field
// does not exist.
// A string or bytes in `value` is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_reply(SDRKIT_HANDLE handle,
                     sdrkit_id id,
                     struct sdrkit_variant *value);

// Writes the number of settings of the object of `handle` to `count`. Setting ids go from 0 to `count` - 1.
int32_t sdrkit_settings_count(SDRKIT_HANDLE handle,
                              uint32_t *count);

// Writes the id of the setting with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no setting has the key.
int32_t sdrkit_setting_find(SDRKIT_HANDLE handle,
                            const char *key,
                            sdrkit_id *id);

// Fills `decl` with the declaration of setting `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the setting does not exist.
// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_setting_decl(SDRKIT_HANDLE handle,
                            sdrkit_id id,
                            struct sdrkit_decl *decl);

// Fills `option` with option `n` of the setting `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the setting or the option does not
// exist.
// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_setting_option(SDRKIT_HANDLE handle,
                              sdrkit_id id,
                              uint32_t n,
                              struct sdrkit_option *option);

// Copies the values of setting `id` that people normally use into `values`, at most `capacity`, and writes the number of
// values that the setting has to `count`. A list that depends on another setting (the filter widths of a demodulator) is
// the one for the present value of that setting. A setting without such values has a count of 0. `values` can be NULL
// with a `capacity` of 0. Returns `SDRKIT_E_NO_SUCH_ID` if the setting does not exist.
int32_t sdrkit_setting_common(SDRKIT_HANDLE handle,
                              sdrkit_id id,
                              double *values,
                              uint32_t capacity,
                              uint32_t *count);

// Writes to `count` the number of settings that `sdrkit_radio_open()` takes for `address`: those of the object
// `driver.<key>` of the scheme of the address, with the same ids, then those that the radio at the address adds and
// that are known before it opens, such as the settings of the test signal of the stack of `synth:<stack>`. Setting ids
// go from 0 to `count` - 1. Returns the codes of `sdrkit_radio_open()` for an unknown driver or stack.
int32_t sdrkit_radio_open_settings_count(SDRKIT_HANDLE engine,
                                         const char *address,
                                         uint32_t *count);

// Writes to `id` the id of the setting with the key `key` that `sdrkit_radio_open()` takes for `address`. Returns
// `SDRKIT_E_NO_SUCH_KEY` if no such setting has the key.
int32_t sdrkit_radio_open_setting_find(SDRKIT_HANDLE engine,
                                       const char *address,
                                       const char *key,
                                       sdrkit_id *id);

// Fills `decl` with the declaration of setting `id` that `sdrkit_radio_open()` takes for `address`. `common` is 0: the
// open radio gives the common values with `sdrkit_setting_common()`. Returns `SDRKIT_E_NO_SUCH_ID` if the setting does
// not exist.
// The strings of `decl` are views that the library owns, valid until the next call on `engine`. Copy what you keep.
int32_t sdrkit_radio_open_setting_decl(SDRKIT_HANDLE engine,
                                       const char *address,
                                       sdrkit_id id,
                                       struct sdrkit_decl *decl);

// Fills `option` with option `n` of setting `id` that `sdrkit_radio_open()` takes for `address`. Returns
// `SDRKIT_E_NO_SUCH_ID` if the setting or the option does not exist.
// The strings of `option` are views that the library owns, valid until the next call on `engine`. Copy what you keep.
int32_t sdrkit_radio_open_setting_option(SDRKIT_HANDLE engine,
                                         const char *address,
                                         sdrkit_id id,
                                         uint32_t n,
                                         struct sdrkit_option *option);

// Writes the id of the argument with the key `key` of command `command` to `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the
// command does not exist, `SDRKIT_E_NO_SUCH_KEY` if no argument has the key.
int32_t sdrkit_argument_find(SDRKIT_HANDLE handle,
                             sdrkit_id command,
                             const char *key,
                             sdrkit_id *id);

// Fills `decl` with the declaration of argument `id` of command `command`. Returns `SDRKIT_E_NO_SUCH_ID` if the command or
// the argument does not exist.
// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_argument_decl(SDRKIT_HANDLE handle,
                             sdrkit_id command,
                             sdrkit_id id,
                             struct sdrkit_decl *decl);

// Fills `option` with option `n` of argument `id` of command `command`. Returns `SDRKIT_E_NO_SUCH_ID` if the command, the
// argument or the option does not exist.
// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_argument_option(SDRKIT_HANDLE handle,
                               sdrkit_id command,
                               sdrkit_id id,
                               uint32_t n,
                               struct sdrkit_option *option);

// Writes the id of the reply field with the key `key` of command `command` to `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the
// command does not exist, `SDRKIT_E_NO_SUCH_KEY` if no reply field has the key.
int32_t sdrkit_reply_field_find(SDRKIT_HANDLE handle,
                                sdrkit_id command,
                                const char *key,
                                sdrkit_id *id);

// Fills `decl` with the declaration of reply field `id` of command `command`. Returns `SDRKIT_E_NO_SUCH_ID` if the command
// or the field does not exist.
// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_reply_field_decl(SDRKIT_HANDLE handle,
                                sdrkit_id command,
                                sdrkit_id id,
                                struct sdrkit_decl *decl);

// Fills `option` with option `n` of reply field `id` of command `command`. Returns `SDRKIT_E_NO_SUCH_ID` if the command,
// the field or the option does not exist.
// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_reply_field_option(SDRKIT_HANDLE handle,
                                  sdrkit_id command,
                                  sdrkit_id id,
                                  uint32_t n,
                                  struct sdrkit_option *option);

// Writes the id of the result field with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no field has the key.
int32_t sdrkit_field_find(SDRKIT_HANDLE handle,
                          const char *key,
                          sdrkit_id *id);

// Fills `decl` with the declaration of result field `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the field does not exist.
// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_field_decl(SDRKIT_HANDLE handle,
                          sdrkit_id id,
                          struct sdrkit_decl *decl);

// Fills `option` with option `n` of result field `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the field or the option does not
// exist.
// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_field_option(SDRKIT_HANDLE handle,
                            sdrkit_id id,
                            uint32_t n,
                            struct sdrkit_option *option);

// Writes the id of the column with the key `key` of table `table` to `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the table does
// not exist, `SDRKIT_E_NO_SUCH_KEY` if no column has the key.
int32_t sdrkit_column_find(SDRKIT_HANDLE handle,
                           sdrkit_id table,
                           const char *key,
                           sdrkit_id *id);

// Fills `decl` with the declaration of column `id` of table `table`. Returns `SDRKIT_E_NO_SUCH_ID` if the table or the
// column does not exist.
// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_column_decl(SDRKIT_HANDLE handle,
                           sdrkit_id table,
                           sdrkit_id id,
                           struct sdrkit_decl *decl);

// Fills `option` with option `n` of column `id` of table `table`. Returns `SDRKIT_E_NO_SUCH_ID` if the table, the column or
// the option does not exist.
// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_column_option(SDRKIT_HANDLE handle,
                             sdrkit_id table,
                             sdrkit_id id,
                             uint32_t n,
                             struct sdrkit_option *option);

// Writes the id of the field with the key `key` of event type `event_type` to `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the
// event type does not exist, `SDRKIT_E_NO_SUCH_KEY` if no field has the key.
int32_t sdrkit_event_field_find(SDRKIT_HANDLE handle,
                                sdrkit_id event_type,
                                const char *key,
                                sdrkit_id *id);

// Fills `decl` with the declaration of field `id` of event type `event_type`. Returns `SDRKIT_E_NO_SUCH_ID` if the event
// type or the field does not exist.
// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_event_field_decl(SDRKIT_HANDLE handle,
                                sdrkit_id event_type,
                                sdrkit_id id,
                                struct sdrkit_decl *decl);

// Fills `option` with option `n` of field `id` of event type `event_type`. Returns `SDRKIT_E_NO_SUCH_ID` if the event type,
// the field or the option does not exist.
// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_event_field_option(SDRKIT_HANDLE handle,
                                  sdrkit_id event_type,
                                  sdrkit_id id,
                                  uint32_t n,
                                  struct sdrkit_option *option);

// Writes the version of the library, `major.minor.patch`, to `version`.
// The string belongs to the library and is valid while the library is loaded.
int32_t sdrkit_version(struct sdrkit_string *version);

// Makes an engine that keeps its files in the folder `folder`, and writes its handle to `engine`. The call makes the
// folder if it does not exist. If `folder` is NULL, the engine keeps its files in memory, and they are lost when the
// engine is freed. Returns `SDRKIT_E_NOT_FOUND`, `SDRKIT_E_DENIED` or another storage code if it cannot make the folder.
// On error, `sdrkit_error_message(0, ...)` returns the message.
int32_t sdrkit_engine_new(const char *folder,
                          SDRKIT_HANDLE *engine);

// Frees `handle`. Freeing the engine stops every session, closes every radio and frees every handle of the engine. Freeing
// an object handle frees the handle only: a session runs until `sdrkit_session_stop`. Freeing a graph frees the graph.
int32_t sdrkit_free(SDRKIT_HANDLE handle);

// Writes a new handle of the object with the key `key` to `object`. `handle` is any handle of the engine. Returns
// `SDRKIT_E_BAD_KEY` if `key` is not an object key, `SDRKIT_E_NO_SUCH_KEY` if no object has the key.
int32_t sdrkit_object_find(SDRKIT_HANDLE handle,
                           const char *key,
                           SDRKIT_HANDLE *object);

// Writes a new handle of the stack of the stack session `session` to `object`. Returns `SDRKIT_E_WRONG_HANDLE` if `session`
// is not a session.
int32_t sdrkit_object_stack(SDRKIT_HANDLE session,
                            SDRKIT_HANDLE *object);

// Writes a new handle of the component named `component` of the graph of `session` to `object`. Returns
// `SDRKIT_E_WRONG_HANDLE` if `session` is not a session, `SDRKIT_E_NO_SUCH_KEY` if its graph has no component of that name.
int32_t sdrkit_object_component(SDRKIT_HANDLE session,
                                const char *component,
                                SDRKIT_HANDLE *object);

// Writes the key of the object of `handle` to `key`, as `sdrkit_object_find` reads it. The key is empty for a stack or a
// component of a session and for a graph.
// The string is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_object_key(SDRKIT_HANDLE handle,
                          struct sdrkit_string *key);

// Writes the type of the object of `handle`, an `SDRKIT_OBJECT_*` constant, to `object_type`.
int32_t sdrkit_object_type(SDRKIT_HANDLE handle, int32_t *object_type);

// Takes a list of the live objects of the engine of `handle` and writes their number to `count`: the engine, the radios,
// the sessions, the services, then the stack, driver and component types. `sdrkit_object` reads the entries.
int32_t sdrkit_objects(SDRKIT_HANDLE handle,
                       uint32_t *count);

// Writes the key and the `SDRKIT_OBJECT_*` type of entry `n` of the object list of the handle to `key` and `object_type`.
// Returns `SDRKIT_E_NO_SUCH_ID` if `n` is not in the list.
// The string is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_object(SDRKIT_HANDLE handle,
                      uint32_t n,
                      struct sdrkit_string *key,
                      int32_t *object_type);

// Returns the input of a session that reads the open radio `radio`.
struct sdrkit_input sdrkit_input_radio(SDRKIT_HANDLE radio);

// Returns the input of a session that reads the host feed `feed`. Any number of sessions can read one feed.
struct sdrkit_input sdrkit_input_host(SDRKIT_HANDLE feed);

// Opens a host feed of complex samples at `rate_hz` around `centre_hz` and writes its handle to `feed`. Sessions read it
// with `sdrkit_input_host`, and the caller writes it with `sdrkit_samples_push`. `sdrkit_free` closes it.
int32_t sdrkit_feed_open(SDRKIT_HANDLE engine,
                         double rate_hz,
                         double centre_hz,
                         SDRKIT_HANDLE *feed);

// Returns the input of a session that reads no samples, for a graph whose component makes the signal.
struct sdrkit_input sdrkit_input_inside(void);

// Opens the radio at `address` and writes its handle to `radio`. The scheme of the address selects the driver:
// `rtl_tcp:<host>:<port>`, `synth:<stack>` for a test signal, `file:<path>` for a recording in the folder of the engine.
// `options` is NULL for the defaults; its settings are those that `sdrkit_radio_open_setting_decl()` declares for the
// address. Returns `SDRKIT_E_NO_SUCH_KEY` for an unknown driver and for the test signal of an
// unknown stack (the member `key` of the error detail holds the driver key or the stack id), a setting code for a
// refused setting, `SDRKIT_E_RADIO_OPEN` if a radio of this address is open and the device has one receiver, or
// `SDRKIT_E_DRIVER`.
int32_t sdrkit_radio_open(SDRKIT_HANDLE engine,
                          const char *address,
                          const struct sdrkit_radio_options *options,
                          SDRKIT_HANDLE *radio);

// Stops and closes the radio of `radio`. Returns `SDRKIT_E_RADIO_IN_USE` while sessions read it.
int32_t sdrkit_radio_close(SDRKIT_HANDLE radio);

// Starts a session of the stack `stack` on `input`, and writes its handle to `session`. `options` is NULL for the defaults.
// On a radio, the session is tuned to `options->centre_hz` or to the default frequency of the stack. If no other session
// reads the radio, the radio is first set to the stack's sample rate and to that frequency. A radio with one fixed
// frequency (a recording) is not tuned. Returns `SDRKIT_E_BAD_ARGUMENT` for a negative or not finite `centre_hz`,
// `SDRKIT_E_NO_SUCH_KEY` for an unknown stack, a setting code for a refused setting or centre, `SDRKIT_E_CONFIG`, or
// `SDRKIT_E_NO_SUCH_OBJECT` for a closed radio.
int32_t sdrkit_stack_start(SDRKIT_HANDLE engine,
                           const char *stack,
                           const struct sdrkit_stack_options *options,
                           const struct sdrkit_input *input,
                           SDRKIT_HANDLE *session);

// Writes to `need` what the stack `stack` in `submode` (NULL for the default) with its default settings needs on this
// device, by its recorded cost, before a session starts it: the cores, the cores free, and the threads that the engine
// would use. The call never waits. Returns `SDRKIT_E_NOT_NOW` while the calibration of the device has not finished (the
// first tens of milliseconds after the engine started), `SDRKIT_E_NO_SUCH_KEY` for an unknown stack, and
// `SDRKIT_E_CONFIG` if the stack cannot be made in `submode`.
int32_t sdrkit_stack_need(SDRKIT_HANDLE engine,
                          const char *stack,
                          const char *submode,
                          struct sdrkit_stack_need_info *need);

// Starts a session of the graph `graph` on `input`, and writes its handle to `session`. The graph can start more sessions.
// Returns `SDRKIT_E_GRAPH`, `SDRKIT_E_CONNECT` or `SDRKIT_E_CONFIG` if the graph does not build.
int32_t sdrkit_graph_start(SDRKIT_HANDLE graph,
                           const struct sdrkit_input *input,
                           SDRKIT_HANDLE *session);

// Stops the session of `session`. Its results stay until `sdrkit_session_close`.
int32_t sdrkit_session_stop(SDRKIT_HANDLE session);

// Closes the stopped session of `session`. The handle stays until `sdrkit_free`. Returns `SDRKIT_E_STILL_RUNNING` if the
// session was not stopped.
int32_t sdrkit_session_close(SDRKIT_HANDLE session);

// Returns the name of the error code `code`, such as `"SDRKIT_E_NO_SUCH_OBJECT"`, as a static NUL-terminated string. Returns
// an empty string if `code` is not an error code. Never returns NULL.
const char *sdrkit_code_name(int32_t code);

// Writes the code of the last error of the engine of `handle` to `code`: `SDRKIT_OK` if no call failed. If `handle` is 0,
// writes the code of the last error of the calls that take no handle.
int32_t sdrkit_error_code(SDRKIT_HANDLE handle,
                          int32_t *code);

// Fills `detail` with the detail of the last error of the engine of `handle`. If `handle` is 0, fills it with the detail
// of the last error of the calls that take no handle (`sdrkit_engine_new`); its strings are then valid until the next
// call on handle 0 in the same thread.
int32_t sdrkit_error_detail(SDRKIT_HANDLE handle,
                            struct sdrkit_error_info *detail);

// Writes the last error of the engine of `handle` to `message` as one English sentence, for a log. If `handle` is 0,
// writes the last error of the calls that take no handle (`sdrkit_engine_new`); the string is then valid until the next
// call on handle 0 in the same thread.
int32_t sdrkit_error_message(SDRKIT_HANDLE handle,
                             struct sdrkit_string *message);

// Writes the number of event types in the catalog of the engine to `count`. Event type ids go from 0 to `count` - 1.
int32_t sdrkit_event_types_count(SDRKIT_HANDLE handle,
                                 uint32_t *count);

// Writes the id of the event type with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no event type has the key.
int32_t sdrkit_event_type_find(SDRKIT_HANDLE handle,
                               const char *key,
                               sdrkit_id *id);

// Fills `info` with the declaration of event type `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the event type does not exist.
// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_event_type_decl(SDRKIT_HANDLE handle,
                               sdrkit_id id,
                               struct sdrkit_event_type_info *info);

// Takes the waiting events of the object of `handle` and writes their number to `count`. The engine, radios, sessions and
// services have events. The events of the previous take are dropped.
int32_t sdrkit_events_take(SDRKIT_HANDLE handle,
                           uint32_t *count);

// Fills `event` with event `n` of the last take. Returns `SDRKIT_E_NO_SUCH_ID` if the event does not exist.
int32_t sdrkit_event(SDRKIT_HANDLE handle,
                     uint32_t n,
                     struct sdrkit_event_info *event);

// Writes field `id` of event `n` of the last take to `value`. Returns `SDRKIT_E_NO_SUCH_ID` if the event or the field does
// not exist.
// A string or bytes in `value` is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
// For a event type that a family header declares, the typed function (`sdrkit_<type>_event_read`) is the first choice.
int32_t sdrkit_event_field(SDRKIT_HANDLE handle,
                           uint32_t n,
                           sdrkit_id id,
                           struct sdrkit_variant *value);

// Makes a new, empty graph of the engine of `handle` and writes its handle to `graph`.
int32_t sdrkit_graph_new(SDRKIT_HANDLE handle, SDRKIT_HANDLE *graph);

// Adds a component of the type `component_type` with the name `name` (letters, digits, `_` and `-`) and its default
// settings. The settings of the component are those of the object `component.<component_type>`. Returns
// `SDRKIT_E_NO_SUCH_KEY` for an unknown type, `SDRKIT_E_GRAPH` for a bad or used name.
int32_t sdrkit_graph_add(SDRKIT_HANDLE graph,
                         const char *name,
                         const char *component_type);

// Sets setting `id` of component `component` (its name) to `value` before the graph starts. Returns
// `SDRKIT_E_NO_SUCH_KEY` for an unknown component, `SDRKIT_E_NO_SUCH_ID` for an unknown setting, or a setting code.
int32_t sdrkit_graph_set(SDRKIT_HANDLE graph,
                         const char *component,
                         sdrkit_id id,
                         const struct sdrkit_variant *value);

// Writes the number of ports of component `component` (its name) to `count`: its inputs, then its outputs. Returns
// `SDRKIT_E_NO_SUCH_KEY` for an unknown component.
int32_t sdrkit_graph_ports(SDRKIT_HANDLE graph,
                           const char *component,
                           uint32_t *count);

// Fills `port` with port `n` of component `component` (its name). The inputs come first, then the outputs. Returns
// `SDRKIT_E_NO_SUCH_KEY` for an unknown component, `SDRKIT_E_NO_SUCH_ID` if the port does not exist.
// The strings of `port` are views that the library owns, valid until the next call on `graph`. Copy what you keep.
int32_t sdrkit_graph_port(SDRKIT_HANDLE graph,
                          const char *component,
                          uint32_t n,
                          struct sdrkit_port_info *port);

// Connects the output `from` to the input `to`. Each is `"<component>.<port>"`, or `"<component>"` for a component with
// one port on that side. `from` is `"input"` for the input of the graph. Returns `SDRKIT_E_NO_SUCH_KEY` for an unknown
// component or port, or `SDRKIT_E_CONNECT` with the components, the ports and the stream types in the error detail.
int32_t sdrkit_graph_connect(SDRKIT_HANDLE graph,
                             const char *from,
                             const char *to);

// Builds the graph for complex samples at `rate_hz` around `centre_hz`, resolves the format of every port, inserts the
// adapters that it needs, and fills `report` with the size of the report. Returns `SDRKIT_E_GRAPH`, `SDRKIT_E_CONNECT` or
// `SDRKIT_E_CONFIG` if the graph does not build.
int32_t sdrkit_graph_validate(SDRKIT_HANDLE graph,
                              double rate_hz,
                              double centre_hz,
                              struct sdrkit_report_info *report);

// Fills `port` with port `n` of the last report. Returns `SDRKIT_E_NO_SUCH_ID` if the port does not exist.
// The strings of `port` are views that the library owns, valid until the next call on `graph`. Copy what you keep.
int32_t sdrkit_graph_report_port(SDRKIT_HANDLE graph,
                                 uint32_t n,
                                 struct sdrkit_report_port *port);

// Fills `adapter` with adapter `n` of the last report. Returns `SDRKIT_E_NO_SUCH_ID` if the adapter does not exist.
// The strings of `adapter` are views that the library owns, valid until the next call on `graph`. Copy what you keep.
int32_t sdrkit_graph_report_adapter(SDRKIT_HANDLE graph,
                                    uint32_t n,
                                    struct sdrkit_report_adapter *adapter);

// Fills `info` with the size of the results of the object of `handle`.
// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_results_decl(SDRKIT_HANDLE handle,
                            struct sdrkit_results_info *info);

// Writes the id of the table with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no table has the key.
int32_t sdrkit_table_find(SDRKIT_HANDLE handle,
                          const char *key,
                          sdrkit_id *id);

// Fills `info` with the declaration of table `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the table does not exist.
// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_table_decl(SDRKIT_HANDLE handle,
                          sdrkit_id id,
                          struct sdrkit_table_info *info);

// Writes the id of the series with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no series has the key.
int32_t sdrkit_series_find(SDRKIT_HANDLE handle,
                           const char *key,
                           sdrkit_id *id);

// Fills `info` with the declaration of series `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the series does not exist.
// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_series_decl(SDRKIT_HANDLE handle,
                           sdrkit_id id,
                           struct sdrkit_plot_info *info);

// Writes the id of the raster with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no raster has the key.
int32_t sdrkit_raster_find(SDRKIT_HANDLE handle,
                           const char *key,
                           sdrkit_id *id);

// Fills `info` with the declaration of raster `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the raster does not exist.
// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_raster_decl(SDRKIT_HANDLE handle,
                           sdrkit_id id,
                           struct sdrkit_plot_info *info);

// Takes a snapshot of the results of the object of `handle` and fills `info`. The snapshot holds the state, the log, the
// map points, every result field if one changed after the change counter `since`, and the rows of each table that changed
// after `since`. Pass 0 to take everything, and `info->counter` on the next call. If a table in your copy holds more rows
// than `total` after you merge the changed rows, rows were removed: take a snapshot with `since` 0.
// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_snapshot_read(SDRKIT_HANDLE handle,
                             uint64_t since,
                             struct sdrkit_snapshot_info *info);

// Writes result field `id` of the snapshot to `value`. Returns `SDRKIT_E_NO_SUCH_ID` if the field does not exist or no
// snapshot was taken.
// A string or bytes in `value` is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
// For a result set that a family header declares, the typed function (`sdrkit_<stack>_fields_read`) is the first choice.
int32_t sdrkit_snapshot_field(SDRKIT_HANDLE handle,
                              sdrkit_id id,
                              struct sdrkit_variant *value);

// Fills `rows` with the rows of table `table` in the snapshot. Returns `SDRKIT_E_NO_SUCH_ID` if the table does not exist.
int32_t sdrkit_snapshot_table(SDRKIT_HANDLE handle,
                              sdrkit_id table,
                              struct sdrkit_rows_info *rows);

// Writes the key of changed row `n` of table `table` in the snapshot to `key`. Returns `SDRKIT_E_NO_SUCH_ID` if the table
// or the row does not exist.
// The string is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_snapshot_row(SDRKIT_HANDLE handle,
                            sdrkit_id table,
                            uint32_t n,
                            struct sdrkit_string *key);

// Writes column `column` of changed row `n` of table `table` in the snapshot to `value`. Returns `SDRKIT_E_NO_SUCH_ID` if
// the table, the row or the column does not exist.
// A string or bytes in `value` is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
// For a table that a family header declares, the typed function (`sdrkit_<stack>_<table>_rows`) is the first choice.
int32_t sdrkit_snapshot_cell(SDRKIT_HANDLE handle,
                             sdrkit_id table,
                             uint32_t n,
                             sdrkit_id column,
                             struct sdrkit_variant *value);

// Writes log line `n` of the snapshot to `line`. Line 0 is the oldest. Returns `SDRKIT_E_NO_SUCH_ID` if the line does not
// exist.
// The string is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_snapshot_log(SDRKIT_HANDLE handle,
                            uint32_t n,
                            struct sdrkit_string *line);

// Fills `point` with map point `n` of the snapshot. Returns `SDRKIT_E_NO_SUCH_ID` if the point does not exist.
// The strings of `point` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_snapshot_point(SDRKIT_HANDLE handle,
                              uint32_t n,
                              struct sdrkit_point *point);

// Copies the points of series `id` into the snapshot if the series changed after the change counter `since`, and fills
// `info`. Returns `SDRKIT_E_NO_SUCH_ID` if the series does not exist.
// The arrays and strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_snapshot_series(SDRKIT_HANDLE handle,
                               sdrkit_id id,
                               uint64_t since,
                               struct sdrkit_series_info *info);

// Copies the values of raster `id` into the snapshot if the raster changed after the change counter `since`, and fills
// `info`. Returns `SDRKIT_E_NO_SUCH_ID` if the raster does not exist.
// The arrays and strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_snapshot_raster(SDRKIT_HANDLE handle,
                               sdrkit_id id,
                               uint64_t since,
                               struct sdrkit_raster_info *info);

// Writes the value of setting `id` of the object of `handle` to `value`. For a type of a catalog, the value is the default.
// Returns `SDRKIT_E_NO_SUCH_ID` if the setting does not exist. For a setting that a family header declares, its typed
// function (`sdrkit_<stack>_get_<key>`) is the first choice. This call is for a program that shows any setting from its
// declaration.
// A string or bytes in `value` is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_setting_get(SDRKIT_HANDLE handle,
                           sdrkit_id id,
                           struct sdrkit_variant *value);

// Sets setting `id` of the object of `handle` to `value`. The value must match the declaration: its type, range and
// options. A refused value changes nothing. Returns a setting code on error: `SDRKIT_E_WRONG_TYPE`,
// `SDRKIT_E_OUT_OF_RANGE`, `SDRKIT_E_NOT_AN_OPTION` and others. For a setting that a family header declares, its typed
// function (`sdrkit_<stack>_set_<key>`) is the first choice.
int32_t sdrkit_setting_set(SDRKIT_HANDLE handle,
                           sdrkit_id id,
                           const struct sdrkit_variant *value);

// Writes the settings of every object of the engine to `blob` as saved settings: bytes that the host stores as they are
// and gives to `sdrkit_settings_import` in a later run. Only the library reads them. `transmit_allowed` is not saved.
// The bytes are a view that the library owns, valid until the next call on `handle`. Copy what you keep.
int32_t sdrkit_settings_export(SDRKIT_HANDLE handle,
                               struct sdrkit_bytes *blob);

// Applies the `size` bytes at `data` that `sdrkit_settings_export` wrote, in this run or an earlier one. An object or a
// setting that the engine does not have now is skipped. A setting of a saved object that the bytes do not hold takes its
// default. Returns `SDRKIT_E_VERSION` if a later version of the library wrote the bytes, and `SDRKIT_E_IMPORT` if they
// are not saved settings, are damaged, or hold a value that a setting refuses; the call then changes nothing.
int32_t sdrkit_settings_import(SDRKIT_HANDLE handle,
                               const uint8_t *data,
                               size_t size);

// Makes an engine that keeps its files in the storage of the callbacks in `storage`, and writes its handle to `engine`. The
// engine copies the table. Returns `SDRKIT_E_NULL_POINTER` if a callback other than `release` is NULL.
int32_t sdrkit_engine_new_with_storage(const struct sdrkit_storage *storage,
                                       SDRKIT_HANDLE *engine);

// Copies the waiting audio of `session` into `out`, at most `capacity` floats, and fills `info`. The call copies whole
// interleaved frames of one rate and one channel count. Audio that the caller does not read in time is lost, and the
// result field `session.audio_lost` counts it.
int32_t sdrkit_audio_read(SDRKIT_HANDLE session,
                          float *out,
                          size_t capacity,
                          struct sdrkit_audio_info *info);

// Gives `session` `frames` frames of audio at `rate_hz`, such as a microphone, for the host audio input (`host.in`)
// of its graph. A frame has `channels` floats: 1 for mono, or 2 for stereo as left then right, so `samples` holds
// `frames` times `channels` floats. `samples` can be NULL when `frames` is 0. The engine converts the audio to the
// rate and the channel count of that input: mono audio goes to both channels of a stereo input, and stereo audio
// becomes the mean of left and right for a mono input. The channel count and the rate can change from call to call;
// the audio from before that the graph has not taken is then left out. The session keeps 32768 floats (0.68 s of
// mono or 0.34 s of stereo at 48 kHz): audio that the graph does not take in time is lost, oldest first, and the
// result field `session.audio_in_lost` counts it. A session that transmits asks for audio: the field
// `session.audio_in_rate_hz` has a value while it can take it, and `session.audio_in_wanted` is true while a
// transmission that sends it runs or push-to-talk is pressed. Open the microphone only then.
//
// The call never waits and allocates nothing, so a real-time audio thread can make it, also while another thread uses
// the handle. It takes only the lock of the handle table of the library, which no thread of the engine takes, and no
// lock of the engine or of the session. One thread at a time writes the audio of a session; stop the writes before
// the handle or the engine is freed. Returns `SDRKIT_E_UNSUPPORTED` for a channel count that is not 1 or 2, with
// the count in the member `value` of the error detail, and `SDRKIT_E_CONFIG` for a rate that is not above 0 or not
// finite, with the rate in the member `value`; an error is kept as the last error of the engine, which takes a lock
// and can allocate.
int32_t sdrkit_session_write_audio(SDRKIT_HANDLE session,
                                   const float *samples,
                                   uint32_t frames,
                                   uint16_t channels,
                                   double rate_hz);

// Copies the samples that the component `tx.sink` of the graph of `session` wrote into `out`, at most `capacity` samples of
// two floats (I then Q), and fills `info`. Samples that the caller does not read in time are lost, and the result field
// `session.transmit_lost` counts them. Returns `SDRKIT_E_TO_RADIO` if the engine sends the samples to the radio.
int32_t sdrkit_transmit_read(SDRKIT_HANDLE session,
                             float *out,
                             size_t capacity,
                             struct sdrkit_iq_info *info);

// Writes `size` bytes of samples of the `SDRKIT_SAMPLE_*` format `format` to the host feed `feed`, for every session that
// reads it. The call does not wait. Returns `SDRKIT_E_PART_SAMPLE`, or `SDRKIT_E_BEHIND` if a session of the feed has
// not used the previous samples: the call then writes no samples, and the caller tries again later.
int32_t sdrkit_samples_push(SDRKIT_HANDLE feed,
                            int32_t format,
                            const void *data,
                            size_t size);

// Fills `info` with the first media packet of `session` after the counter `after`. Pass 0 for the oldest packet. The
// session keeps the coded packets of the last ten seconds, at most 32 MiB, and decoded video frames of the last half
// second, at most 16 MiB; the result field `session.media_lost` counts those that left unread. Pictures are not media
// packets.
// The arrays and strings of `info` are views that the library owns, valid until the next call on `session`. Copy what you keep.
int32_t sdrkit_media_read(SDRKIT_HANDLE session,
                          uint64_t after,
                          struct sdrkit_media_info *info);

// Fills `info` with the first picture of `session` after the counter `after`. Pass 0 for the oldest picture. A picture is
// compressed bytes, or raw pixels in the layout of `width`, `height` and `strides`. `picture_id` says which picture
// the packet holds, `picture_state` whether more of it can come, and `picture_name` its name. A picture that grows
// comes again with the same `picture_id` and all of its bytes; the session keeps the newest state of each of its
// last 16 pictures, so a state that the caller did not read leaves when the next state of that picture comes, and
// counts in `missed`. A picture that grows when the session takes another stack, or ends, comes once more, finished.
// The arrays and strings of `info` are views that the library owns, valid until the next call on `session`. Copy what you keep.
int32_t sdrkit_picture_read(SDRKIT_HANDLE session,
                            uint64_t after,
                            struct sdrkit_media_info *info);

// Copies the newest spectrum frame of `session` into `bins`, lowest frequency first, if a frame came after the last
// call, and fills `frame`. `bins` holds `capacity` values in dB. If the frame has more bins, the call copies the
// first `capacity` and writes the number of bins of the frame to `frame.total`: read the next frame with a buffer of
// that size. The setting `spectrum` of the session turns the spectrum on, and `spectrum_size` sets the number of
// bins. `bins` can be NULL with a `capacity` of 0.
int32_t sdrkit_spectrum_read(SDRKIT_HANDLE session,
                             float *bins,
                             uint32_t capacity,
                             struct sdrkit_spectrum_frame *frame);

// Opens a tap on the output `port` (`"<component>.<port>"`, or `"input"`) of the graph of `session`, and writes its id to
// `tap`. A tap copies the stream at that point into at most `chunks` buffers until the caller reads them. Returns
// `SDRKIT_E_NO_GRAPH` or `SDRKIT_E_NO_SUCH_KEY`.
int32_t sdrkit_tap_open(SDRKIT_HANDLE session,
                        const char *port,
                        size_t chunks,
                        sdrkit_id *tap);

// Copies the waiting values of tap `tap` of `session` into `out`, at most `capacity` floats, and fills `info`. Returns
// `SDRKIT_E_NO_SUCH_ID` if the tap does not exist.
int32_t sdrkit_tap_read(SDRKIT_HANDLE session,
                        sdrkit_id tap,
                        float *out,
                        size_t capacity,
                        struct sdrkit_tap_info *info);

// Closes tap `tap` of `session`. Returns `SDRKIT_E_NO_SUCH_ID` if the tap does not exist.
int32_t sdrkit_tap_close(SDRKIT_HANDLE session, sdrkit_id tap);

// Writes the version of the C interface of this library to `major` and `minor`. The base header that a program was
// built with has the version `SDRKIT_INTERFACE_VERSION_MAJOR`.`SDRKIT_INTERFACE_VERSION_MINOR`: the program runs with a
// library of the same major version and of the same or a higher minor version.
int32_t sdrkit_interface_version(uint32_t *major,
                                 uint32_t *minor);

// Writes the number of stack families that the library holds a typed interface of to `count`.
int32_t sdrkit_families_count(uint32_t *count);

// Writes the key of family `n` to `key` (a string of the library, valid while the library is loaded), and the version
// of its interface to `major` and `minor`. Returns `SDRKIT_E_NO_SUCH_ID` if the family does not exist.
int32_t sdrkit_family(uint32_t n,
                      struct sdrkit_string *key,
                      uint32_t *major,
                      uint32_t *minor);

// Writes the version of the interface of the stack family `family` in this library to `major` and `minor`. Returns
// `SDRKIT_E_NO_SUCH_KEY` if the library was built without the family.
int32_t sdrkit_family_version(const char *family,
                              uint32_t *major,
                              uint32_t *minor);

// Fills `rows` with the changed rows of the table `table` in the snapshot of `handle`, newest first, at most
// `capacity` structs of `row_size` bytes each, and fills `info` as `sdrkit_snapshot_table` does. `type_name` names the
// struct. The functions of a family header call this with the right names and size: use them. Returns
// `SDRKIT_E_NO_SUCH_KEY` if the library has no such struct, and `SDRKIT_E_WRONG_HANDLE` if the snapshot does not hold
// the table with the members of the struct.
int32_t sdrkit_typed_rows(SDRKIT_HANDLE handle,
                          const char *type_name,
                          const char *table,
                          void *rows,
                          size_t row_size,
                          uint32_t capacity,
                          struct sdrkit_rows_info *info);

// Fills the struct of `size` bytes at `fields` with the result fields in the snapshot of `handle`. `type_name` names
// the struct. With `@<component>` after the name, the fields are those of the named component of the stack, which a
// generic consumer reads under the keys `<component>.<key>`. The functions of a family header call this with the right name and size: use them. Returns
// `SDRKIT_E_NO_SUCH_KEY` if the library has no such struct, and `SDRKIT_E_WRONG_HANDLE` if the snapshot does not hold
// the fields of the struct.
int32_t sdrkit_typed_fields(SDRKIT_HANDLE handle,
                            const char *type_name,
                            void *fields,
                            size_t size);

// Fills the struct of `size` bytes at `event` with event `n` of the last `sdrkit_events_take` on `handle`.
// `event_type` is the key of the event type and `type_name` names the struct. The functions of a family header call
// this with the right names and size: use them. Returns `SDRKIT_E_NO_SUCH_ID` if the event does not exist,
// `SDRKIT_E_WRONG_TYPE` if it is of another type, `SDRKIT_E_NO_SUCH_KEY` if the library has no such struct, and
// `SDRKIT_E_WRONG_HANDLE` if the event does not hold the members of the struct.
int32_t sdrkit_typed_event(SDRKIT_HANDLE handle,
                           uint32_t n,
                           const char *event_type,
                           const char *type_name,
                           void *event,
                           size_t size);

// Runs the command with the key `command` of the object of `handle`. `args` is the arguments struct that `args_type`
// names, with its `size` set, or NULL with a NULL `args_type` for a command without arguments. An optional argument
// whose `has_` member is false, or that lies after the `size` bytes of the struct, takes its default. If `reply` is not
// NULL, the call fills the struct of `reply_size` bytes that `reply_type` names with the reply. The functions of a
// family header call this with the right names and sizes: use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object does
// not have the command with the arguments and the reply of the structs, `SDRKIT_E_NO_SUCH_KEY` if the library has no
// such struct, or the codes of `sdrkit_command_invoke`.
int32_t sdrkit_typed_command(SDRKIT_HANDLE handle,
                             const char *command,
                             const char *args_type,
                             const void *args,
                             const char *reply_type,
                             void *reply,
                             size_t reply_size);

// Writes the value of the setting `key` of the object of `handle` to `value`: a switch. For a type of a catalog,
// the value is the default. The functions of a family header call this with the key and the type of each setting:
// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
// setting is of another type.
int32_t sdrkit_typed_setting_get_bool(SDRKIT_HANDLE handle,
                                      const char *key,
                                      bool *value);

// Sets the setting `key` of the object of `handle` to `value`: a switch. The value must match the declaration of the
// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
int32_t sdrkit_typed_setting_set_bool(SDRKIT_HANDLE handle,
                                      const char *key,
                                      bool value);

// Writes the value of the setting `key` of the object of `handle` to `value`: a whole number. For a type of a catalog,
// the value is the default. The functions of a family header call this with the key and the type of each setting:
// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
// setting is of another type.
int32_t sdrkit_typed_setting_get_i64(SDRKIT_HANDLE handle,
                                     const char *key,
                                     int64_t *value);

// Sets the setting `key` of the object of `handle` to `value`: a whole number. The value must match the declaration of the
// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
int32_t sdrkit_typed_setting_set_i64(SDRKIT_HANDLE handle,
                                     const char *key,
                                     int64_t value);

// Writes the value of the setting `key` of the object of `handle` to `value`: a number, a frequency in Hz, a time or a length of time in seconds. For a type of a catalog,
// the value is the default. The functions of a family header call this with the key and the type of each setting:
// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
// setting is of another type.
int32_t sdrkit_typed_setting_get_f64(SDRKIT_HANDLE handle,
                                     const char *key,
                                     double *value);

// Sets the setting `key` of the object of `handle` to `value`: a number, a frequency in Hz, a time or a length of time in seconds. The value must match the declaration of the
// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
int32_t sdrkit_typed_setting_set_f64(SDRKIT_HANDLE handle,
                                     const char *key,
                                     double value);

// Writes the value of the setting `key` of the object of `handle` to `value`: the index of an option. For a type of a catalog,
// the value is the default. The functions of a family header call this with the key and the type of each setting:
// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
// setting is of another type.
int32_t sdrkit_typed_setting_get_enum(SDRKIT_HANDLE handle,
                                      const char *key,
                                      int32_t *value);

// Sets the setting `key` of the object of `handle` to `value`: the index of an option. The value must match the declaration of the
// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
int32_t sdrkit_typed_setting_set_enum(SDRKIT_HANDLE handle,
                                      const char *key,
                                      int32_t value);

// Writes the value of the setting `key` of the object of `handle` to `value`: a string. The string is valid until the next call on `handle`. For a type of a catalog,
// the value is the default. The functions of a family header call this with the key and the type of each setting:
// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
// setting is of another type.
int32_t sdrkit_typed_setting_get_string(SDRKIT_HANDLE handle,
                                        const char *key,
                                        struct sdrkit_string *value);

// Sets the setting `key` of the object of `handle` to `value`: a string. The value must match the declaration of the
// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
int32_t sdrkit_typed_setting_set_string(SDRKIT_HANDLE handle,
                                        const char *key,
                                        struct sdrkit_string value);

// Writes the value of the setting `key` of the object of `handle` to `value`: a position. For a type of a catalog,
// the value is the default. The functions of a family header call this with the key and the type of each setting:
// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
// setting is of another type.
int32_t sdrkit_typed_setting_get_position(SDRKIT_HANDLE handle,
                                          const char *key,
                                          struct sdrkit_position *value);

// Sets the setting `key` of the object of `handle` to `value`: a position. The value must match the declaration of the
// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
int32_t sdrkit_typed_setting_set_position(SDRKIT_HANDLE handle,
                                          const char *key,
                                          struct sdrkit_position value);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* SDRKIT_H */

/*
 * The initialisers of the structs with a size, then the typed settings, results, events and commands of the objects
 * that every host has. Each header is written from the declarations of its object.
 *
 * A change of frequency. sdrkit_session_set_tuned_hz(session, hz) tunes a running session. If the frequency lies inside
 * the samples of the input, only the session moves. If it lies outside, the radio of the session moves its centre.
 * The handle of the session stays valid and the session keeps running. sdrkit_session_get_tuned_hz() returns the
 * frequency. What the change does to the results, the tables and the events of the session:
 *
 * - From the return of the call until the receiver has taken the frequency, a snapshot holds no result of the stack:
 *   its fields have no value, and its tables, series, rasters, map points and log are empty. A result of the
 *   frequency before is never in a snapshot with the count of changes of before.
 * - After that, a snapshot holds what the receiver holds. The receiver forgets what it decoded on the frequency
 *   before: each component of the stack that has a "clear" command runs it, so its results start empty and fill from
 *   the new signal. A table or a result set that the description of the stack marks as kept (a scan list, a
 *   database of stations) stays: it is in the snapshot again, with every row marked as changed. The registry record
 *   of the family lists the kept items of a stack as "<stack>.kept.<key>". A component without a "clear" command
 *   keeps its results.
 * - The fields of the session itself (sdrkit_session_fields) stay, and its counts go on. The member `retunes` counts
 *   the changes of frequency that the receiver has taken: those of the host, of a service and of the stack itself.
 *   A session on a radio counts its first tuning at the start. If the radio moves its centre for the change, the receiver is told again
 *   when the samples of the new centre arrive, and clears again; `retunes` does not count that.
 * - Events stay. Each event holds the frequency of its signal (`freq_hz`), and the events that were not yet taken are
 *   taken as before.
 * - Audio that waits is not removed.
 *
 * A change of frequency by a service (a scan) or by the stack itself reaches the receiver without the first step and
 * without the second: the results stay in view, and the stack clears what it must. A change of the centre of the
 * radio alone (sdrkit_radio_set_centre_hz()) is a change by the host for a session that follows the centre (its
 * setting "tuned" is 0): the receiver forgets as in the second step, without the first step. A session with a
 * frequency of its own keeps that frequency and its results.
 */
#include "sdrkit/init.h"
#include "sdrkit/session.h"
#include "sdrkit/scan.h"
#include "sdrkit/record.h"
#include "sdrkit/radio.h"
#include "sdrkit/engine.h"
