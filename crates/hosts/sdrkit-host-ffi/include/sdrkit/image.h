/*
 * sdrkit: the typed interface of the shared interface "image", whose items the stacks of several families show, version 1.0. MIT licence.
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

#ifndef SDRKIT_IMAGE_H
#define SDRKIT_IMAGE_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_IMAGE_VERSION_MAJOR 1
#define SDRKIT_IMAGE_VERSION_MINOR 0

// The keys of the event types.

// The result set of the picture component.
typedef struct sdrkit_image_picture_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The rows of the picture shown.
  int64_t rows;
  // The strips kept, over all instruments.
  int64_t strips;
  // The strips refused because their samples did not match their size.
  int64_t bad_strips;
} sdrkit_image_picture_fields;

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_image_picture_fields_read(SDRKIT_HANDLE handle, sdrkit_image_picture_fields *fields) {
  return sdrkit_typed_fields(handle, "image.PictureFields", fields, sizeof(sdrkit_image_picture_fields));
}

#endif
