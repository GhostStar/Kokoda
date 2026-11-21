import AppKit
import VideoToolbox
import CoreImage
import ImageIO

// PNG is the wire format. Inspect metadata before allocating decoded pixels.
enum ClipboardImage {
    static let maxBytes = 8 * 1024 * 1024
    static func normalizedPNG(_ data: Data) -> Data? {
        guard data.count <= 64 * 1024 * 1024,
              let source = CGImageSourceCreateWithData(data as CFData,nil),
              let properties = CGImageSourceCopyPropertiesAtIndex(source,0,nil) as? [CFString:Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int,
              let height = properties[kCGImagePropertyPixelHeight] as? Int,
              width > 0, height > 0, width <= 16384, height <= 16384, width * height <= 16 * 1024 * 1024,
              let image = CGImageSourceCreateImageAtIndex(source,0,[kCGImageSourceShouldCache:false] as CFDictionary),
              let png = NSBitmapImageRep(cgImage:image).representation(using:.png,properties:[:]),
              png.count <= maxBytes else { return nil }
        return png
    }
}

// Each window owns a decoder. Parameter-set changes (including resize) recreate
// its session; packets cannot be dropped after encoding because they reference
// earlier pictures. Only the host's unencoded capture slot may be superseded.
