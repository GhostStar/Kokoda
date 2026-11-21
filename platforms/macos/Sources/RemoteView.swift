import AppKit
import VideoToolbox
import CoreImage
import ImageIO

struct PointerGesture {
    let id: UInt64
    let screenOrigin: NSPoint // Quartz screen points, top-left origin.
    let pixelsPerPoint: NSSize
    func delta(to screen: NSPoint) -> NSPoint {
        NSPoint(x:(screen.x-screenOrigin.x)*pixelsPerPoint.width,
                y:(screen.y-screenOrigin.y)*pixelsPerPoint.height)
    }
}

final class RemoteView: NSView, NSTextInputClient {
    var inputEnabled = true { didSet {if !inputEnabled {cancelComposition();cancelPointerDrag()}} }
    var anchoredPointer = false
    var pointerGesture: PointerGesture?
    var nextPointerGesture: UInt64 = 0
    func cancelPointerDrag() {pointerGesture=nil}
    var composition = NSAttributedString(string: "")
    var compositionSelection = NSRange(location: 0, length: 0)
    var inputAnchor = NSPoint(x: 20, y: 40)
    var forwardedKeys = Set<UInt16>()
    var transparent = false
    var image: NSImage? { didSet { needsDisplay = true } }
    var sendInput: (([String: Any]) -> Void)?
    var modifiers: NSEvent.ModifierFlags = []
    var tracking: NSTrackingArea?
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking = tracking { removeTrackingArea(tracking) }
        let area = NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeInActiveApp, .inVisibleRect, .enabledDuringMouseDrag], owner: self)
        addTrackingArea(area)
        tracking = area
    }
    override func mouseMoved(with event: NSEvent) {
        // Background/covered surfaces must not steer the Windows cursor.
        guard window?.isKeyWindow == true else { return }
        pointer(event, kind:"move")
    }
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    var imageRect: NSRect {
        guard let image = image else { return .zero }
        var scale = min(bounds.width / image.size.width, bounds.height / image.size.height)
        // AppKit rounds native frames to whole points. Cover only that rounding
        // seam proportionally; genuine aspect differences still use letterboxing.
        if bounds.width-image.size.width*scale<=1.01 && bounds.height-image.size.height*scale<=1.01 {
            scale=max(bounds.width/image.size.width,bounds.height/image.size.height)
        }
        let size = NSSize(width: image.size.width * scale, height: image.size.height * scale)
        return NSRect(x: (bounds.width - size.width) / 2, y: (bounds.height - size.height) / 2,
                      width: size.width, height: size.height)
    }
    override func draw(_ dirtyRect: NSRect) {
        if transparent { NSColor.clear.setFill(); bounds.fill(using:.copy) }
        else { NSColor.windowBackgroundColor.setFill(); bounds.fill() }
        NSGraphicsContext.current?.imageInterpolation = .high
        image?.draw(in: imageRect)
        if hasMarkedText() {
            let text = NSAttributedString(string: composition.string, attributes: [
                .font: NSFont.systemFont(ofSize: 16), .foregroundColor: NSColor.labelColor,
                .underlineStyle: NSUnderlineStyle.single.rawValue
            ])
            let size = text.size()
            let box = NSRect(x: inputAnchor.x, y: inputAnchor.y, width: size.width + 12, height: size.height + 8)
            NSColor.textBackgroundColor.setFill()
            box.fill()
            text.draw(at: NSPoint(x: box.minX + 6, y: box.minY + 4))
        }
    }
    func pointer(_ event: NSEvent, kind: String, down: Bool = false, button: Int = 0) {
        let point = convert(event.locationInWindow, from: nil)
        // CGEvent retains screen coordinates independently of the moving NSWindow.
        let mouse=NSEvent.mouseLocation
        let screen=event.cgEvent?.location ?? NSPoint(x:mouse.x,y:(NSScreen.screens.first?.frame.maxY ?? 0)-mouse.y)
        pointer(kind:kind,down:down,button:button,point:point,screen:screen)
    }
    func pointer(kind:String,down:Bool=false,button:Int=0,point:NSPoint,screen:NSPoint) {
        guard inputEnabled,let image=image else{return}
        if let gesture=pointerGesture,kind=="move" || (kind=="button" && !down && button==0) {
            let delta=gesture.delta(to:screen)
            let ending=kind=="button"
            sendInput?(["kind":ending ? "drag_end" : "drag_move","gesture":gesture.id,"dx":delta.x,"dy":delta.y])
            if ending {pointerGesture=nil}
            return
        }
        let rect = imageRect
        guard rect.contains(point) else {
            if kind == "button" && !down { sendInput?(["kind":"release"]) }
            return
        }
        var message: [String: Any] = [
            "kind":kind, "x":min(0.999999, max(0, (point.x - rect.minX) / rect.width)),
            "y":min(0.999999, max(0, 1 - (point.y - rect.minY) / rect.height)),
            "width":Int(image.size.width), "height":Int(image.size.height)
        ]
        if anchoredPointer,kind=="button",button==0,down {
            guard nextPointerGesture<UInt64.max else{return}
            nextPointerGesture+=1
            pointerGesture=PointerGesture(id:nextPointerGesture,screenOrigin:screen,
                pixelsPerPoint:NSSize(width:image.size.width/rect.width,height:image.size.height/rect.height))
            message["kind"]="drag_begin";message["gesture"]=nextPointerGesture
            sendInput?(message);return
        }
        if kind == "button" { message["button"] = button; message["down"] = down }
        sendInput?(message)
    }
    // Re-enter the local IME even when AppKit kept this view as first responder
    // across a window/app switch. Do not discard an in-progress composition.
    func restoreTextInputFocus() {
        guard let window=window else { return }
        if window.firstResponder !== self {
            guard window.makeFirstResponder(self) else { return }
        }
        if window.isKeyWindow && NSApp.isActive { inputContext?.activate() }
        inputContext?.invalidateCharacterCoordinates()
    }
    override func mouseDown(with event: NSEvent) {
        cancelComposition()
        window?.makeKey()
        restoreTextInputFocus()
        inputAnchor = convert(event.locationInWindow, from: nil)
        inputContext?.invalidateCharacterCoordinates()
        pointer(event, kind:"button", down:true)
    }
    override func mouseUp(with event: NSEvent) { pointer(event, kind:"button", down:false) }
    override func mouseDragged(with event: NSEvent) { pointer(event, kind:"move") }
    override func rightMouseDown(with event: NSEvent) { pointer(event, kind:"button", down:true, button:1) }
    override func rightMouseUp(with event: NSEvent) { pointer(event, kind:"button", down:false, button:1) }
    override func rightMouseDragged(with event: NSEvent) { pointer(event, kind:"move") }
    override func scrollWheel(with event: NSEvent) {
        guard image != nil, imageRect.contains(convert(event.locationInWindow, from: nil)) else { return }
        pointer(event, kind:"move")
        let delta = max(-1200, min(1200, Int(event.scrollingDeltaY * (event.hasPreciseScrollingDeltas ? 12 : 120))))
        sendInput?(["kind":"wheel", "delta":delta])
    }
    static let virtualKeys: [UInt16: Int] = [
        0:65,1:83,2:68,3:70,4:72,5:71,6:90,7:88,8:67,9:86,11:66,
        12:81,13:87,14:69,15:82,16:89,17:84,18:49,19:50,20:51,21:52,
        22:54,23:53,24:187,25:57,26:55,27:189,28:56,29:48,30:221,
        31:79,32:85,33:219,34:73,35:80,36:13,37:76,38:74,39:222,
        40:75,41:186,42:220,43:188,44:191,45:78,46:77,47:190,
        48:9,49:32,50:192,51:8,53:27,65:110,67:106,69:107,75:111,
        76:13,78:109,82:96,83:97,84:98,85:99,86:100,87:101,88:102,
        89:103,91:104,92:105,96:116,97:117,98:118,99:114,100:119,
        101:120,103:122,109:121,111:123,115:36,116:33,117:46,
        118:115,119:35,120:113,121:34,122:112,123:37,124:39,125:40,126:38
    ]
    // Printable text is composed on macOS. Only committed text crosses the network.
    override func keyDown(with event: NSEvent) {
        guard inputEnabled else {return}
        if !event.modifierFlags.intersection([.command,.control]).isEmpty,
           event.modifierFlags.intersection([.shift,.option]).isEmpty,
           let action = [UInt16(8):"copy",UInt16(9):"paste",UInt16(7):"cut"][event.keyCode] {
            if hasMarkedText() {
                guard action == "paste" else { return }
                cancelComposition()
            }
            if !event.isARepeat { sendInput?(["kind":"clipboard","action":action]) }
            return
        }
        if !hasMarkedText() && !event.modifierFlags.intersection([.command, .control]).isEmpty {
            forwardKey(event)
        } else {
            interpretKeyEvents([event])
        }
    }
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard window?.firstResponder === self, event.modifierFlags.contains(.command) else { return false }
        if NSApp.mainMenu?.performKeyEquivalent(with:event) == true { return true }
        keyDown(with: event)
        return true
    }
    func forwardKey(_ event: NSEvent) {
        guard let vk = Self.virtualKeys[event.keyCode] else { return }
        let flags = event.modifierFlags
        for (key, down) in [(16, flags.contains(.shift)),
                            (17, !flags.intersection([.command, .control]).isEmpty),
                            (18, flags.contains(.option))] {
            sendInput?(["kind":"key", "vk":key, "down":down])
        }
        forwardedKeys.insert(event.keyCode)
        sendInput?(["kind":"key", "vk":vk, "down":true])
    }
    override func keyUp(with event: NSEvent) {
        if forwardedKeys.remove(event.keyCode) != nil, let vk = Self.virtualKeys[event.keyCode] {
            sendInput?(["kind":"key", "vk":vk, "down":false])
            if forwardedKeys.isEmpty { sendInput?(["kind":"release"]) }
        }
    }
    override func flagsChanged(with event: NSEvent) { modifiers = event.modifierFlags }
    func cancelComposition() {
        composition = NSAttributedString(string: "")
        compositionSelection = NSRange(location: 0, length: 0)
        setAccessibilityValue("")
        inputContext?.discardMarkedText()
        needsDisplay = true
    }
    func insertText(_ string: Any, replacementRange: NSRange) {
        guard inputEnabled else {return}
        let text = (string as? NSAttributedString)?.string ?? (string as? String ?? "")
        cancelComposition()
        if !text.isEmpty { sendInput?(["kind":"text", "text":text]) }
    }
    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        guard inputEnabled else {return}
        composition = (string as? NSAttributedString) ?? NSAttributedString(string: string as? String ?? "")
        compositionSelection = selectedRange
        setAccessibilityValue(composition.string)
        needsDisplay = true
        inputContext?.invalidateCharacterCoordinates()
    }
    func unmarkText() { composition = NSAttributedString(string: ""); setAccessibilityValue(""); needsDisplay = true }
    func hasMarkedText() -> Bool { composition.length > 0 }
    func markedRange() -> NSRange { NSRange(location: hasMarkedText() ? 0 : NSNotFound, length: composition.length) }
    func selectedRange() -> NSRange { compositionSelection }
    func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }
    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        guard range.location != NSNotFound, range.location <= composition.length else { return nil }
        let safe = NSIntersectionRange(range, NSRange(location: 0, length: composition.length))
        actualRange?.pointee = safe
        return composition.attributedSubstring(from: safe)
    }
    func characterIndex(for point: NSPoint) -> Int { NSNotFound }
    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        actualRange?.pointee = markedRange()
        let local = NSRect(x: min(max(0, inputAnchor.x), max(0, bounds.width - 20)),
                           y: min(max(0, inputAnchor.y), max(0, bounds.height - 24)), width: 2, height: 24)
        return window?.convertToScreen(convert(local, to: nil)) ?? .zero
    }
    override func doCommand(by selector: Selector) {
        if let event = NSApp.currentEvent, event.type == .keyDown { forwardKey(event) }
    }

}
