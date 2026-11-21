// sdrkit: the typed interface of the shared interface "image", whose items the stacks of several families show, version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum ImageInterface {
    public static let major = 1
    public static let minor = 0
}

/// The result set of the picture component.
public struct ImagePictureFields {
    /// The rows of the picture shown.
    public var rows: Int64
    /// The strips kept, over all instruments.
    public var strips: Int64
    /// The strips refused because their samples did not match their size.
    public var badStrips: Int64

    init(_ c: sdrkit_image_picture_fields) {
        rows = c.rows
        strips = c.strips
        badStrips = c.bad_strips
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<ImagePictureFields, SDRKitCode> {
        var c = sdrkit_image_picture_fields()
        let code = sdrkit_image_picture_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(ImagePictureFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum ImageTables {
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum ImageCommands {
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum ImageSettings {
}
