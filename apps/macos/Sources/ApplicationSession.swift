import AppKit
import VideoToolbox
import CoreImage
import ImageIO

final class ApplicationSession {
    let id: UInt64
    var requestedRoot: UInt64?
    var ending = false
    static let defaultPreset = 2
    var needsQualityUpdate = false
    var preset = ApplicationSession.defaultPreset
    var adaptive = true
    var applicationScope = true
    init(id:UInt64) {self.id=id}
    var lastViewport: NSSize?
    var resizedAt = Date.distantPast
    var encodedSizeText = ""
    var videoMode = false
    var statsStart = Date()
    var encoderText = ""
    var statsText = ""
    var surfaces: [UInt64:RemoteSurface] = [:]
    var viewerPreferences:[UInt64:ViewerPreferences]=[:]
    var rootID: UInt64?
    var rootBounds = NSRect.zero
    var order: [UInt64] = []
    var metricWindowID: UInt64?
    var remoteDragging = false
    var selected = false
    var inputEvents: [[String:Any]] = []
    var clipboardRequests: [UInt64:(id:UInt64,token:UInt64,changeCount:Int)] = [:]
    var lastClipboardPoll = Date.distantPast
    var clipboardPollNeedsBaseline = true
    var controlRequests: [[String:Any]] = []
    var lastGroup = Date.distantPast
    var frameCursor = 0
    var remoteForeground: UInt64?
    var layoutInProgress = false
    var resumeToken: String?
    var resumeWindowFrame: NSRect?
    var sourceSizeText = ""
    var statsFrames = 0
    var statsBytes = 0
}
