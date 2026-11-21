let app = NSApplication.shared
let controller = AppDelegate()
controller.rootID = 1
controller.selected = true
let rootBounds: [String:Any] = ["left":1000.0,"top":500.0,"right":2600.0,"bottom":1700.0]
let cardBounds: [String:Any] = ["left":2600.0,"top":700.0,"right":3000.0,"bottom":1000.0]
func entry(_ id:UInt64,_ token:UInt64,_ bounds:[String:Any]) -> [String:Any] {
    ["id":NSNumber(value:id),"token":NSNumber(value:token),"title":"Kokoda UI test \(id)","bounds":bounds]
}
func sync(_ entries:[[String:Any]]) {
    controller.syncGroup(["root_bounds":rootBounds,"windows":entries,"foreground":NSNumber(value:1)])
}
sync([entry(2,2,cardBounds),entry(1,1,rootBounds)])
precondition(controller.surfaces.count == 2)
let root = controller.surfaces[1]!, card = controller.surfaces[2]!
root.window.makeFirstResponder(nil)
controller.windowDidBecomeKey(Notification(name:NSWindow.didBecomeKeyNotification,object:root.window))
precondition(root.window.firstResponder === root.view,"Returning to root failed to restore IME input view")
card.window.makeFirstResponder(nil)
controller.windowDidBecomeKey(Notification(name:NSWindow.didBecomeKeyNotification,object:card.window))
precondition(card.window.firstResponder === card.view,"Returning to popup failed to restore IME input view")
precondition(root.window.firstResponder === root.view,"Popup focus changed another window's responder")
print("PASS: root and popup key-window callbacks restore their own text input view")

precondition(card.window.parent === root.window, "Out-of-order snapshot failed to attach child")
root.view.image = NSImage(size:NSSize(width:1600,height:1200))
controller.layoutChildren()
let rootRect = root.window.convertToScreen(root.view.convert(root.view.imageRect,to:nil))
precondition(abs(card.window.frame.minX-rootRect.maxX) <= 1.0, "Child must extend outside parent at right edge")
precondition(card.window.styleMask.contains(.titled), "Popup needs a native drag title bar")
let oldFrame=card.window.frame
// AppKit may constrain requested frame origins on a smaller CI display. Check
// the actual content delta rather than assuming the requested move was applied.
let expectedContent=RemoteWindowLayout.childRect(rootScreen:rootRect,rootBounds:controller.rootBounds,childBounds:card.bounds)
card.window.setFrameOrigin(NSPoint(x:oldFrame.minX+83,y:oldFrame.minY-47))
controller.windowDidMove(Notification(name:NSWindow.didMoveNotification,object:card.window))
let draggedFrame=card.window.frame
let draggedContent=card.window.contentRect(forFrameRect:draggedFrame)
precondition(abs(draggedFrame.minX-oldFrame.minX)>1 || abs(draggedFrame.minY-oldFrame.minY)>1,"Fixture must actually move")
sync([entry(2,2,cardBounds),entry(1,1,rootBounds)])
precondition(card.window.frame == draggedFrame, "Group refresh snapped locally dragged popup back")
precondition(abs(card.localOffset.x-(draggedContent.minX-expectedContent.minX))<1 && abs(card.localOffset.y-(draggedContent.minY-expectedContent.minY))<1,"Local offset must track the actual native content position")
print("PASS: native popup title bar and local drag position survive group refresh")
let widthBefore = card.window.frame.width
let parentSize=root.window.contentRect(forFrameRect:root.window.frame).size
root.window.setContentSize(NSSize(width:parentSize.width*0.7,height:parentSize.height*0.7))
controller.layoutChildren()
precondition(card.window.frame.width < widthBefore, "Child didn't rescale with parent")
controller.videoMode = true
controller.adaptiveResolution.state = .on
controller.resizedAt = Date()
precondition(!controller.syncViewport(),"Live resize must be debounced")
controller.resizedAt = .distantPast
precondition(controller.syncViewport())
let pixels=root.view.convertToBacking(root.view.imageRect)
precondition(controller.lastViewport == NSSize(width:pixels.width.rounded(),height:pixels.height.rounded()),"Viewport must use backing pixels, not logical points")
precondition(!controller.syncViewport(),"Unchanged size must not rebuild encoder")
controller.adaptiveResolution.state = .off
precondition(controller.syncViewport() && controller.lastViewport == .zero)
controller.supportsWindowVideoAlpha = true
card.view.image = NSImage(size:NSSize(width:400,height:300))
precondition(controller.syncViewport() && card.lastViewport == .zero,"Each popup needs its own viewport")
precondition(!controller.syncViewport(),"Unchanged popup must not rebuild encoder")
controller.adaptiveResolution.state = .on
precondition(controller.syncViewport())
precondition(controller.syncViewport())
let cardPixels=card.view.convertToBacking(card.view.imageRect)
precondition(card.lastViewport == NSSize(width:cardPixels.width.rounded(),height:cardPixels.height.rounded()))
controller.enqueue(["kind":"button","button":0,"down":true],id:card.id,token:card.token)
precondition(controller.session.remoteDragging)
controller.enqueue(["kind":"release"],id:card.id,token:card.token)
precondition(!controller.session.remoteDragging)
controller.videoMode = false
print("PASS: independent popup viewport and drag/release tracking")
controller.inputEvents.removeAll()
controller.enqueue(["kind":"drag_begin","gesture":UInt64(1)],id:card.id,token:card.token)
precondition(controller.session.remoteDragging)
for dx in [10,20,30] {
    controller.enqueue(["kind":"drag_move","gesture":UInt64(1),"dx":dx,"dy":0],id:card.id,token:card.token)
}
precondition(controller.inputEvents.count==2,"Cumulative moves must replace queued samples")
precondition((controller.inputEvents.last?["event"] as? [String:Any])?["dx"] as? Int == 30)
controller.enqueue(["kind":"drag_end","gesture":UInt64(1),"dx":40,"dy":0],id:card.id,token:card.token)
precondition(!controller.session.remoteDragging)
controller.enqueue(["kind":"drag_begin","gesture":UInt64(2)],id:card.id,token:card.token)
controller.enqueue(["kind":"drag_move","gesture":UInt64(2),"dx":5,"dy":0],id:card.id,token:card.token)
precondition(controller.inputEvents.count==5,"Coalescing must preserve release/start boundaries")
card.view.pointerGesture=PointerGesture(id:2,screenOrigin:.zero,pixelsPerPoint:NSSize(width:1,height:1))
controller.resignKey(Notification(name:NSWindow.didResignKeyNotification,object:card.window))
precondition(card.view.pointerGesture==nil && !controller.session.remoteDragging)
precondition((controller.inputEvents.last?["event"] as? [String:Any])?["kind"] as? String == "release")
controller.inputEvents.removeAll()
print("PASS: anchored drag coalescing preserves gesture boundaries and focus loss releases input")
print("PASS: Retina viewport, resize debounce, unchanged-size suppression and fixed-mode reset")

sync([entry(1,1,rootBounds)])
precondition(controller.surfaces[2] == nil && controller.surfaces[1] === root, "Closing card removed root")
precondition(card.window.parent == nil && !card.window.isVisible)
sync([entry(1,1,rootBounds),entry(2,3,cardBounds)])
let bitmap = NSBitmapImageRep(bitmapDataPlanes:nil,pixelsWide:2,pixelsHigh:2,bitsPerSample:8,samplesPerPixel:4,
                             hasAlpha:true,isPlanar:false,colorSpaceName:.deviceRGB,bytesPerRow:0,bitsPerPixel:0)!
let encoded = bitmap.representation(using:.png,properties:[:])!.base64EncodedString()
func deliver(token:UInt64) {
    let packet: [String:Any] = ["type":"frame","id":NSNumber(value:2),"token":NSNumber(value:token),"sequence":1,
                              "image":encoded,"width":400,"height":300]
    controller.buffer.append(try! JSONSerialization.data(withJSONObject:packet));controller.buffer.append(10)
    controller.consume()
    let deadline=Date().addingTimeInterval(3)
    while controller.decodingFrame,Date()<deadline {RunLoop.current.run(until:Date().addingTimeInterval(0.01))}
    precondition(!controller.decodingFrame)
}
deliver(token:2)
precondition(controller.surfaces[2]!.view.image == nil, "Stale frame reached recreated window")
deliver(token:3)
precondition(controller.surfaces[2]!.view.image != nil)
let legacyCard=controller.surfaces[2]!
precondition(legacyCard.window.standardWindowButton(.closeButton)?.isEnabled==true)
legacyCard.window.performClose(nil)
sync([entry(1,1,rootBounds),entry(2,3,cardBounds)])
precondition(legacyCard.locallyClosed && !legacyCard.window.isVisible,"Legacy popup reopened after local close")
controller.restoreViewers();precondition(!legacyCard.locallyClosed)
sync([entry(2,3,cardBounds)])
precondition(controller.surfaces[2]!.window.parent == nil, "Hidden root left dangling parent")
controller.clearSelection()
precondition(controller.surfaces.isEmpty)
print("PASS: group attach, outside-root layout, scaling, hide cleanup, stale-frame rejection, teardown")
let duplicateTitles: [String:Any] = ["type":"windows","windows":[
    ["id":1,"title":"QQ"],["id":2,"title":"QQ"],["id":3,"title":"Another window"]]]
controller.buffer.append(try! JSONSerialization.data(withJSONObject:duplicateTitles));controller.buffer.append(10)
controller.consume()
precondition(controller.managerRows().count == 3,"Duplicate titles must retain separate rows")
let row=controller.managerRows().first{$0.id==2}!
controller.activateManagerRow(row)
precondition((controller.controlRequests.last?["id"] as? NSNumber)?.uint64Value == 2,"Selection must use bound HWND, not row index")
print("PASS: duplicate titles preserve identity and selection routes its bound HWND")
