let application = NSApplication.shared
let view = RemoteView(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 800, height: 600), styleMask: [.titled], backing: .buffered, defer: false)
window.contentView = view
precondition(view.inputContext != nil, "Native input context unavailable")
var events: [[String: Any]] = []
view.sendInput = { events.append($0) }
view.inputAnchor = NSPoint(x: 120, y: 80)
view.setMarkedText("zhongwen", selectedRange: NSRange(location: 8, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
precondition(events.isEmpty, "Composition leaked to Windows")
precondition(view.hasMarkedText())
let rect = view.firstRect(forCharacterRange: NSRange(location: 0, length: 1), actualRange: nil)
let expected = window.convertToScreen(view.convert(NSRect(x: 120, y: 80, width: 2, height: 24), to: nil))
precondition(rect == expected, "Candidate placement not in screen coordinates")
view.insertText("中文🙂", replacementRange: NSRange(location: NSNotFound, length: 0))
precondition(!view.hasMarkedText())
precondition(events.count == 1 && events[0]["kind"] as? String == "text" && events[0]["text"] as? String == "中文🙂")
view.setMarkedText("cancel", selectedRange: NSRange(location: 6, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
view.cancelComposition()
precondition(!view.hasMarkedText() && events.count == 1, "Cancellation committed text")
print("PASS: composition isolation, committed Unicode, candidate screen coordinates, cancellation")

let rootPixels = NSRect(x:1000,y:500,width:1600,height:1200)
let rightPopup = NSRect(x:2600,y:700,width:400,height:300)
let rootOnMac = NSRect(x:100,y:200,width:800,height:600)
precondition(RemoteWindowLayout.childRect(rootScreen:rootOnMac,rootBounds:rootPixels,childBounds:rightPopup)
    == NSRect(x:900,y:550,width:200,height:150), "Popup offset/scale or top-left conversion incorrect")
let leftPopup = NSRect(x:900,y:400,width:400,height:300)
precondition(RemoteWindowLayout.childRect(rootScreen:rootOnMac,rootBounds:rootPixels,childBounds:leftPopup)
    == NSRect(x:50,y:700,width:200,height:150), "Popup outside root was clipped or misplaced")
print("PASS: popup DPI scaling, relative placement and outside-root bounds")

view.setMarkedText("weishuru",selectedRange:NSRange(location:8,length:0),replacementRange:NSRange(location:NSNotFound,length:0))
let pasteEvent=NSEvent.keyEvent(with:.keyDown,location:.zero,modifierFlags:.command,timestamp:0,windowNumber:window.windowNumber,context:nil,characters:"v",charactersIgnoringModifiers:"v",isARepeat:false,keyCode:9)!
view.keyDown(with:pasteEvent)
precondition(!view.hasMarkedText() && events.last?["kind"] as? String == "clipboard" && events.last?["action"] as? String == "paste")
precondition(events.filter {$0["kind"] as? String == "text"}.count == 1,"Paste must not commit unfinished composition")
print("PASS: paste during IME composition cancels marked text and uses clipboard transaction")

view.image=NSImage(size:NSSize(width:800,height:600))
let countBeforeHover=events.count
let hover=NSEvent.mouseEvent(with:.mouseMoved,location:NSPoint(x:50,y:50),modifierFlags:[],timestamp:0,windowNumber:window.windowNumber,context:nil,eventNumber:0,clickCount:0,pressure:0)!
precondition(!window.isKeyWindow)
view.mouseMoved(with:hover)
precondition(events.count==countBeforeHover,"Background window hover moved the remote cursor")
print("PASS: background window hover does not send remote pointer movement")

// A stale/missing responder must be repaired without changing the window or
// sending the marked (uncommitted) text to Windows.
window.makeFirstResponder(nil)
view.restoreTextInputFocus()
precondition(window.firstResponder === view,"IME recovery did not restore its input view")
view.setMarkedText("zhong",selectedRange:NSRange(location:5,length:0),replacementRange:NSRange(location:NSNotFound,length:0))
let beforeRecovery=events.count
view.restoreTextInputFocus()
precondition(view.hasMarkedText() && events.count==beforeRecovery,"Focus repair discarded or submitted composition")
view.insertText("中",replacementRange:NSRange(location:NSNotFound,length:0))
precondition(events.count==beforeRecovery+1 && events.last?["text"] as? String == "中")
print("PASS: missing responder recovers; repeat focus recovery preserves marked text and submits once")

// A wheel operation carries its own target point and physical source dimensions.
view.inputEnabled=true;view.positionedWheel=true;events.removeAll()
view.scroll(point:NSPoint(x:200,y:150),delta:-120)
precondition(events.count==1 && events[0]["kind"] as? String=="wheel_at")
precondition(events[0]["x"] as? CGFloat==0.25 && events[0]["y"] as? CGFloat==0.75)
precondition(events[0]["width"] as? Int==800 && events[0]["height"] as? Int==600)
view.inputEnabled=false;view.scroll(point:NSPoint(x:200,y:150),delta:120)
precondition(events.count==1)
view.inputEnabled=true;view.positionedWheel=false;events.removeAll()
view.scroll(point:NSPoint(x:200,y:150),delta:120)
precondition(events.count==2 && events[0]["kind"] as? String=="move" && events[1]["kind"] as? String=="wheel")
view.scroll(point:NSPoint(x:-10,y:150),delta:120);precondition(events.count==2)
print("PASS: atomic wheel coordinates, legacy fallback, disabled input and outside-image rejection")
