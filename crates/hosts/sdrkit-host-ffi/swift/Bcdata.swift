// sdrkit: the typed interface of the shared interface "bcdata", whose items the stacks of several families show, version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum BcdataInterface {
    public static let major = 1
    public static let minor = 0
}

/// The radio text and whether its item is running.
public struct BcdataLabelFields {
    /// The radio text.
    public var text: String?
    /// The tagged item (a song) is playing.
    public var itemRunning: Bool?

    init(_ c: sdrkit_bcdata_label_fields) {
        text = c.has_text ? sdrkitText(c.text) : nil
        itemRunning = c.has_item_running ? c.item_running : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BcdataLabelFields, SDRKitCode> {
        var c = sdrkit_bcdata_label_fields()
        let code = sdrkit_bcdata_label_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BcdataLabelFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The slide shown.
public struct BcdataSlideFields {
    /// The name of the slide (its MOT ContentName).
    public var slideName: String?
    /// The category title of the slide.
    public var slideCategory: String?
    /// The click-through URL of the slide.
    public var slideLink: String?
    /// The slide is an emergency alert.
    public var slideAlert: Bool

    init(_ c: sdrkit_bcdata_slide_fields) {
        slideName = c.has_slide_name ? sdrkitText(c.slide_name) : nil
        slideCategory = c.has_slide_category ? sdrkitText(c.slide_category) : nil
        slideLink = c.has_slide_link ? sdrkitText(c.slide_link) : nil
        slideAlert = c.slide_alert
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BcdataSlideFields, SDRKitCode> {
        var c = sdrkit_bcdata_slide_fields()
        let code = sdrkit_bcdata_slide_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BcdataSlideFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The counts of the playout since the start or the last change of frequency, written once a second while sound plays. No value before the first report.
public struct BcdataPlayoutFields {
    /// Time filled by concealment above silence.
    public var concealed: Double?
    /// Time of silence (faded concealment, or waiting to start).
    public var muted: Double?
    /// Concealments started.
    public var concealments: Int64?
    /// Times the queue ran empty.
    public var underruns: Int64?

    init(_ c: sdrkit_bcdata_playout_fields) {
        concealed = c.has_concealed ? c.concealed : nil
        muted = c.has_muted ? c.muted : nil
        concealments = c.has_concealments ? c.concealments : nil
        underruns = c.has_underruns ? c.underruns : nil
    }
}

/// A tag of the radio text. The row key is the class name of the content type (`ITEM.TITLE`), or its code for a reserved type.
public struct BcdataLabelTagRow {
    /// The row key.
    public var key: String
    /// The content type code (0 to 127).
    public var code: Int64
    /// What people call the content type ("Title").
    public var type: String?
    /// The tagged text.
    public var text: String

    init(_ c: sdrkit_bcdata_label_tag_row) {
        key = sdrkitText(c.key)
        code = c.code
        type = c.has_type ? sdrkitText(c.type) : nil
        text = sdrkitText(c.text)
    }
}

/// A service of the service information. The row key is the SId in hexadecimal, or the place of a service without a bearer.
public struct BcdataGuideServiceRow {
    /// The row key.
    public var key: String
    /// The name.
    public var name: String?
    /// The short name.
    public var shortName: String?
    /// The description.
    public var description: String?
    /// The service identifier of the first bearer.
    public var sid: Int64?
    /// The ensemble identifier of the first bearer.
    public var eid: Int64?
    /// The genre links, separated by spaces.
    public var genres: String?
    /// The URL of the first logo.
    public var logoUrl: String?

    init(_ c: sdrkit_bcdata_guide_service_row) {
        key = sdrkitText(c.key)
        name = c.has_name ? sdrkitText(c.name) : nil
        shortName = c.has_short_name ? sdrkitText(c.short_name) : nil
        description = c.has_description ? sdrkitText(c.description) : nil
        sid = c.has_sid ? c.sid : nil
        eid = c.has_eid ? c.eid : nil
        genres = c.has_genres ? sdrkitText(c.genres) : nil
        logoUrl = c.has_logo_url ? sdrkitText(c.logo_url) : nil
    }
}

/// A programme of the programme information. The row key is `<crid>@<start>`.
public struct BcdataGuideProgrammeRow {
    /// The row key.
    public var key: String
    /// The programme id (CRID).
    public var crid: String
    /// The name.
    public var name: String?
    /// The start time.
    public var start: Double
    /// The duration.
    public var duration: Double
    /// The description.
    public var description: String?
    /// The genre links, separated by spaces.
    public var genres: String?

    init(_ c: sdrkit_bcdata_guide_programme_row) {
        key = sdrkitText(c.key)
        crid = sdrkitText(c.crid)
        name = c.has_name ? sdrkitText(c.name) : nil
        start = c.start
        duration = c.duration
        description = c.has_description ? sdrkitText(c.description) : nil
        genres = c.has_genres ? sdrkitText(c.genres) : nil
    }
}

/// A programme group. The row key is its name.
public struct BcdataGuideGroupRow {
    /// The row key.
    public var key: String
    /// The name.
    public var name: String

    init(_ c: sdrkit_bcdata_guide_group_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
    }
}

/// A slide of a categorised slideshow. The row key is `<CategoryID>/<SlideID>`.
public struct BcdataSlideCategoryRow {
    /// The row key.
    public var key: String
    /// The CategoryID.
    public var category: Int64
    /// The CategoryTitle.
    public var title: String?
    /// The SlideID.
    public var slide: Int64
    /// The name of the slide.
    public var name: String

    init(_ c: sdrkit_bcdata_slide_category_row) {
        key = sdrkitText(c.key)
        category = c.category
        title = c.has_title ? sdrkitText(c.title) : nil
        slide = c.slide
        name = sdrkitText(c.name)
    }
}

/// A logo. The row key is its MOT ContentName. A row holds the picture in the member of its format.
public struct BcdataLogoRow {
    /// The row key.
    public var key: String
    /// A PNG picture.
    public var png: [UInt8]?
    /// A JPEG picture.
    public var jpeg: [UInt8]?

    init(_ c: sdrkit_bcdata_logo_row) {
        key = sdrkitText(c.key)
        png = c.has_png ? [UInt8](UnsafeBufferPointer(start: c.png.ptr, count: c.png.len)) : nil
        jpeg = c.has_jpeg ? [UInt8](UnsafeBufferPointer(start: c.jpeg.ptr, count: c.jpeg.len)) : nil
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum BcdataTables {
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum BcdataCommands {
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum BcdataSettings {
}
