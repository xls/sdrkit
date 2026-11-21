/*
 * sdrkit: the typed interface of the shared interface "bcdata", whose items the stacks of several families show, version 1.0. MIT licence.
 *
 * Written from the interface description of the family. Do not edit.
 *
 * Structs. Each struct starts with `size`. A call that fills a struct writes at most the size of the struct that
 * the caller was compiled with, and sets `size` to the number of bytes that it wrote. A struct only grows at its
 * end: a library that is newer than the caller fills the members that the caller knows, and a library that is
 * older leaves the members that it does not know at 0, with `size` below the size of the struct. The caller
 * sets `size` of an arguments struct to its sizeof before the call: use the *_INIT initialiser.
 *
 * Values. A member with a `has_` member before it holds a value only if the `has_` member is true. A string or
 * bytes member that the library writes is a view into a buffer of the handle, valid until the next call on the
 * same handle. A member of an enumeration holds a value of the enum that its comment names.
 *
 * Versions. The minor version rises when the family gets a new stack, table, member, setting, command or event
 * type. The major version rises when one is removed or renamed, or its type or unit changes.
 * sdrkit_family_version() returns the version of the family in the library, or SDRKIT_E_NO_SUCH_KEY if the
 * library was built without the family. sdrkit_object_find() with "stack.<id>" finds out if an engine has a
 * stack.
 *
 * Results. Take a snapshot with sdrkit_snapshot_read() on the handle of a session, then call the *_rows() and
 * *_read() functions on the same handle. Events. Take the events with sdrkit_events_take(), then call the
 * *_event_read() function of the type of each event; it returns SDRKIT_E_WRONG_TYPE for an event of another type.
 * Settings and commands. Call the *_get_*(), *_set_*() and command functions on the handle of a session of the
 * stack; the getters also take the handle of the stack type ("stack.<id>") and return the defaults. One call
 * sets one setting. A function returns SDRKIT_E_WRONG_HANDLE if the object of the handle does not have the
 * table, the fields, the setting or the command with the types of this header.
 */

#ifndef SDRKIT_BCDATA_H
#define SDRKIT_BCDATA_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_BCDATA_VERSION_MAJOR 1
#define SDRKIT_BCDATA_VERSION_MINOR 0

// The keys of the event types.

// The radio text and whether its item is running.
typedef struct sdrkit_bcdata_label_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `text` holds a value.
  bool has_text;
  // The radio text.
  sdrkit_string text;
  // True if `item_running` holds a value.
  bool has_item_running;
  // The tagged item (a song) is playing.
  bool item_running;
} sdrkit_bcdata_label_fields;

// The slide shown.
typedef struct sdrkit_bcdata_slide_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `slide_name` holds a value.
  bool has_slide_name;
  // The name of the slide (its MOT ContentName).
  sdrkit_string slide_name;
  // True if `slide_category` holds a value.
  bool has_slide_category;
  // The category title of the slide.
  sdrkit_string slide_category;
  // True if `slide_link` holds a value.
  bool has_slide_link;
  // The click-through URL of the slide.
  sdrkit_string slide_link;
  // The slide is an emergency alert.
  bool slide_alert;
} sdrkit_bcdata_slide_fields;

// The counts of the playout since the start or the last change of frequency, written once a second while sound plays. No value before the first report.
typedef struct sdrkit_bcdata_playout_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `concealed` holds a value.
  bool has_concealed;
  // Time filled by concealment above silence. A length of time in seconds.
  double concealed;
  // True if `muted` holds a value.
  bool has_muted;
  // Time of silence (faded concealment, or waiting to start). A length of time in seconds.
  double muted;
  // True if `concealments` holds a value.
  bool has_concealments;
  // Concealments started.
  int64_t concealments;
  // True if `underruns` holds a value.
  bool has_underruns;
  // Times the queue ran empty.
  int64_t underruns;
} sdrkit_bcdata_playout_fields;

// A tag of the radio text. The row key is the class name of the content type (`ITEM.TITLE`), or its code for a reserved type.
typedef struct sdrkit_bcdata_label_tag_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The content type code (0 to 127).
  int64_t code;
  // True if `type` holds a value.
  bool has_type;
  // What people call the content type ("Title").
  sdrkit_string type;
  // The tagged text.
  sdrkit_string text;
} sdrkit_bcdata_label_tag_row;

// A service of the service information. The row key is the SId in hexadecimal, or the place of a service without a bearer.
typedef struct sdrkit_bcdata_guide_service_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `name` holds a value.
  bool has_name;
  // The name.
  sdrkit_string name;
  // True if `short_name` holds a value.
  bool has_short_name;
  // The short name.
  sdrkit_string short_name;
  // True if `description` holds a value.
  bool has_description;
  // The description.
  sdrkit_string description;
  // True if `sid` holds a value.
  bool has_sid;
  // The service identifier of the first bearer.
  int64_t sid;
  // True if `eid` holds a value.
  bool has_eid;
  // The ensemble identifier of the first bearer.
  int64_t eid;
  // True if `genres` holds a value.
  bool has_genres;
  // The genre links, separated by spaces.
  sdrkit_string genres;
  // True if `logo_url` holds a value.
  bool has_logo_url;
  // The URL of the first logo.
  sdrkit_string logo_url;
} sdrkit_bcdata_guide_service_row;

// A programme of the programme information. The row key is `<crid>@<start>`.
typedef struct sdrkit_bcdata_guide_programme_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The programme id (CRID).
  sdrkit_string crid;
  // True if `name` holds a value.
  bool has_name;
  // The name.
  sdrkit_string name;
  // The start time. Seconds since 1970 (UTC).
  double start;
  // The duration. A length of time in seconds.
  double duration;
  // True if `description` holds a value.
  bool has_description;
  // The description.
  sdrkit_string description;
  // True if `genres` holds a value.
  bool has_genres;
  // The genre links, separated by spaces.
  sdrkit_string genres;
} sdrkit_bcdata_guide_programme_row;

// A programme group. The row key is its name.
typedef struct sdrkit_bcdata_guide_group_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name.
  sdrkit_string name;
} sdrkit_bcdata_guide_group_row;

// A slide of a categorised slideshow. The row key is `<CategoryID>/<SlideID>`.
typedef struct sdrkit_bcdata_slide_category_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The CategoryID.
  int64_t category;
  // True if `title` holds a value.
  bool has_title;
  // The CategoryTitle.
  sdrkit_string title;
  // The SlideID.
  int64_t slide;
  // The name of the slide.
  sdrkit_string name;
} sdrkit_bcdata_slide_category_row;

// A logo. The row key is its MOT ContentName. A row holds the picture in the member of its format.
typedef struct sdrkit_bcdata_logo_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `png` holds a value.
  bool has_png;
  // A PNG picture.
  sdrkit_bytes png;
  // True if `jpeg` holds a value.
  bool has_jpeg;
  // A JPEG picture.
  sdrkit_bytes jpeg;
} sdrkit_bcdata_logo_row;

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_bcdata_label_fields_read(SDRKIT_HANDLE handle, sdrkit_bcdata_label_fields *fields) {
  return sdrkit_typed_fields(handle, "bcdata.LabelFields", fields, sizeof(sdrkit_bcdata_label_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_bcdata_slide_fields_read(SDRKIT_HANDLE handle, sdrkit_bcdata_slide_fields *fields) {
  return sdrkit_typed_fields(handle, "bcdata.SlideFields", fields, sizeof(sdrkit_bcdata_slide_fields));
}

// Fills `fields` with the result fields of the component "playout" of the stack "bcdata" in the snapshot of
// `handle`. A generic consumer reads them under the keys "playout.<key>".
static inline int32_t sdrkit_bcdata_playout_fields_read(SDRKIT_HANDLE handle, sdrkit_bcdata_playout_fields *fields) {
  return sdrkit_typed_fields(handle, "bcdata.PlayoutFields@playout", fields, sizeof(sdrkit_bcdata_playout_fields));
}

#endif
