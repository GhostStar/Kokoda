import AppKit
import VideoToolbox
import CoreImage
import ImageIO

enum RemoteWindowLayout {
    static func childRect(rootScreen: NSRect, rootBounds: NSRect, childBounds: NSRect) -> NSRect {
        guard rootBounds.width > 0, rootBounds.height > 0 else { return .zero }
        let scale = rootScreen.width / rootBounds.width
        return NSRect(x:rootScreen.minX+(childBounds.minX-rootBounds.minX)*scale,
                      y:rootScreen.maxY-(childBounds.minY-rootBounds.minY+childBounds.height)*scale,
                      width:childBounds.width*scale,height:childBounds.height*scale)
    }
}

final class RemoteNativeWindow: NSWindow {
    // AppKit also emits live-resize notifications during native zoom. Keep
    // those distinct from a user dragging the viewer border.
    var localGeometryDepth=0
    var didChangeLocalGeometry:(()->Void)?
    override func performZoom(_ sender:Any?) {
        localGeometryDepth+=1;defer{localGeometryDepth-=1;if localGeometryDepth==0 {didChangeLocalGeometry?()}};super.performZoom(sender)
    }
    override func zoom(_ sender:Any?) {
        localGeometryDepth+=1;defer{localGeometryDepth-=1;if localGeometryDepth==0 {didChangeLocalGeometry?()}};super.zoom(sender)
    }
    var acceptsFocus=true
    override var canBecomeKey:Bool {acceptsFocus}
    override var canBecomeMain:Bool {acceptsFocus}
}
final class RemotePanel: NSPanel {
    // AppKit also emits live-resize notifications during native zoom. Keep
    // those distinct from a user dragging the viewer border.
    var localGeometryDepth=0
    var didChangeLocalGeometry:(()->Void)?
    override func performZoom(_ sender:Any?) {
        localGeometryDepth+=1;defer{localGeometryDepth-=1;if localGeometryDepth==0 {didChangeLocalGeometry?()}};super.performZoom(sender)
    }
    override func zoom(_ sender:Any?) {
        localGeometryDepth+=1;defer{localGeometryDepth-=1;if localGeometryDepth==0 {didChangeLocalGeometry?()}};super.zoom(sender)
    }
    var acceptsFocus=true
    var independent=false
    override var canBecomeKey: Bool { acceptsFocus }
    override var canBecomeMain: Bool { acceptsFocus && independent }
}
struct WindowPresentation {
    let dpi:Double
    let role:String
    let owner:UInt64?
    let enabled:Bool
    let minimized:Bool
    let maximized:Bool
    let canMaximize:Bool
    let canClose:Bool
    let canMinimize:Bool
    let canResize:Bool
    init(_ value:[String:Any]) {
        dpi=max(48,min(768,(value["dpi"] as? NSNumber)?.doubleValue ?? 96))
        role=value["role"] as? String ?? "independent"
        owner=(value["owner"] as? NSNumber)?.uint64Value
        enabled=value["enabled"] as? Bool ?? false
        minimized=value["minimized"] as? Bool ?? false
        maximized=value["maximized"] as? Bool ?? false
        canMaximize=value["can_maximize"] as? Bool ?? false
        canClose=value["can_close"] as? Bool ?? false
        canMinimize=value["can_minimize"] as? Bool ?? false
        canResize=value["can_resize"] as? Bool ?? false
    }
}
enum ViewerResizeMode:Int {case scale,hostWindow}
struct ViewerPreferences {
    let identity:String
    let closed:Bool
    let mode:ViewerResizeMode
    let frame:NSRect
    let offset:NSPoint
}
struct WindowResizeGesture {
    let source:NSRect
    let content:NSRect
    let pixelsPerPoint:NSSize
    func target(_ current:NSRect)->[String:Int] {
        ["width":max(64,min(8192,Int((source.width+(current.width-content.width)*pixelsPerPoint.width).rounded()))),
         "height":max(64,min(8192,Int((source.height+(current.height-content.height)*pixelsPerPoint.height).rounded()))),
         "left":Int((source.minX+(current.minX-content.minX)*pixelsPerPoint.width).rounded()),
         "top":Int((source.minY+(content.maxY-current.maxY)*pixelsPerPoint.height).rounded())]
    }
}

final class RemoteSurface {
    let id: UInt64
    let token: UInt64
    let window: NSWindow
    let view = RemoteView()
    let resizeControl=ViewerResizeControl()
    var fittedSourceSize:NSSize?
    var displaySourceSize:NSSize {view.image?.size ?? bounds.size}
    var videoDecoder = WindowVideoDecoder()
    let decodeQueue = DispatchQueue(label:"Kokoda.window.decode",qos:.userInitiated,autoreleaseFrequency:.workItem)
    var queuedFrames = 0
    var receivedSequence: UInt64 = 0
    var streamEpoch: UInt64 = 0
    var sequence: UInt64 = 0
    var bounds = NSRect.zero // Windows physical pixels, top-left origin.
    var lastFrame = Date.distantPast
    var lastViewport: NSSize?
    var rateText = ""
    var localOffset = NSPoint.zero
    var presentation:WindowPresentation?
    var positioned=false
    var displayScale:CGFloat?
    var resizeMode:ViewerResizeMode = .scale
    var clientGeometry=false
    var hostGeometryChanged=false
    var locallyClosed=false
    var nativeIdentity=""
    var heldFeedback:[[String:Any]]=[]
    var hostDPI:Double {presentation?.dpi ?? 96}
    var localZoom:Double {(displayScale ?? (96/hostDPI))*hostDPI/96}
    var resizeGesture:WindowResizeGesture?
    var resizeRevision:UInt64=0
    var resizePending:UInt64?
    var resizeSettlesAt:Date?
    var resizeSample: [String:Int]?
    var lastResizeSent=Date.distantPast
    var role:String {presentation?.role ?? (window is RemotePanel ? "floating":"independent")}
    var localGeometryAction:Bool {((window as? RemoteNativeWindow)?.localGeometryDepth ?? (window as? RemotePanel)?.localGeometryDepth ?? 0)>0}
    var canPresent:Bool {
        guard !locallyClosed else{return false}
        var current:NSWindow?=window
        while let item=current {if item.isMiniaturized{return false};current=item.parent}
        return true
    }
    var geometryHeld:Bool {resizeGesture != nil || resizePending != nil}
    init(id: UInt64, token: UInt64, root: Bool, presentation:WindowPresentation?=nil) {
        self.id = id; self.token = token;self.presentation=presentation
        if presentation?.role == "independent" || (presentation == nil && root) {
            window = RemoteNativeWindow(contentRect: NSRect(x:250,y:150,width:1000,height:700),
                              styleMask:[.titled,.closable,.miniaturizable,.resizable],backing:.buffered,defer:false)
        } else {
            let panel = RemotePanel(contentRect:root ? NSRect(x:250,y:150,width:1000,height:700):.zero,styleMask:[.titled,.closable],backing:.buffered,defer:false)
            panel.isFloatingPanel = false
            panel.hidesOnDeactivate = false
            panel.isOpaque = false
            panel.backgroundColor = .clear
            panel.hasShadow = true
            view.transparent = true
            window = panel
        }
        positioned=root
        window.isReleasedWhenClosed = false
        window.contentView = view
        view.setAccessibilityElement(true)
        view.setAccessibilityRole(.textArea)
        view.setAccessibilityLabel(L("accessibility.input"))
    }
    func configure(_ value:WindowPresentation,readOnly:Bool) {
        presentation=value
        var style:NSWindow.StyleMask=value.role=="transient" ? [.borderless]:[.titled,.closable]
        if value.role != "transient" {
            if value.role=="independent" {style.insert(.miniaturizable)}
            style.insert(.resizable)
        }
        if !style.contains(.titled),window.styleMask.contains(.titled) {
            if let index=window.titlebarAccessoryViewControllers.firstIndex(where:{$0 === resizeControl}) {window.removeTitlebarAccessoryViewController(at:index)}
        }
        if window.styleMask != style {window.styleMask=style}
        window.isExcludedFromWindowsMenu=locallyClosed || value.role != "independent"
        window.collectionBehavior=value.role == "independent" ? [.managed,.participatesInCycle]:[.transient,.ignoresCycle]
        window.standardWindowButton(.closeButton)?.isEnabled=true
        window.standardWindowButton(.miniaturizeButton)?.isEnabled=true
        window.standardWindowButton(.zoomButton)?.isEnabled=true
        if value.role=="independent" {window.collectionBehavior.insert(.fullScreenNone)}
        (window as? RemoteNativeWindow)?.acceptsFocus=value.enabled
        if let panel=window as? RemotePanel {panel.acceptsFocus=value.enabled;panel.independent=value.role=="independent"}
        view.inputEnabled = !readOnly && value.enabled && !locallyClosed
    }
    func resetInput() { view.modifiers = []; view.forwardedKeys.removeAll(); view.cancelComposition(); view.cancelPointerDrag() }
}

// All HWNDs, queues and recovery state belong to one application session.
