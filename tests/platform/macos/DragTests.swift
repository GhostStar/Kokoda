let app=NSApplication.shared
let view=RemoteView(frame:NSRect(x:0,y:0,width:800,height:450))
let window=NSWindow(contentRect:NSRect(x:100,y:100,width:800,height:450),styleMask:[.titled,.resizable],backing:.buffered,defer:false)
window.contentView=view
view.image=NSImage(size:NSSize(width:1600,height:900));view.anchoredPointer=true
var events:[[String:Any]]=[]
view.sendInput={events.append($0)}
view.pointer(kind:"button",down:true,point:NSPoint(x:100,y:400),screen:NSPoint(x:1000,y:600))
precondition(events.last?["kind"] as? String == "drag_begin")
let gesture=events.last?["gesture"] as! UInt64
// Incoming remote geometry moves/resizes the local viewer while the mouse moves
// only 10x20 screen points. Neither change can feed into the gesture delta.
window.setFrameOrigin(NSPoint(x:400,y:300));window.setContentSize(NSSize(width:400,height:400))
view.pointer(kind:"move",point:NSPoint(x:-100,y:1000),screen:NSPoint(x:1010,y:620))
precondition(events.last?["dx"] as? CGFloat == 20 && events.last?["dy"] as? CGFloat == 40)
precondition(events.last?["gesture"] as? UInt64 == gesture)
view.pointer(kind:"move",point:.zero,screen:NSPoint(x:1010,y:620))
precondition(events.last?["dx"] as? CGFloat == 20,"Repeated sample accumulated drift")
view.pointer(kind:"button",down:false,point:NSPoint(x:-100,y:1000),screen:NSPoint(x:990,y:650))
precondition(events.last?["kind"] as? String == "drag_end" && events.last?["dx"] as? CGFloat == -20 && events.last?["dy"] as? CGFloat == 100)
precondition(view.pointerGesture == nil,"Release outside the image left a drag active")
view.pointer(kind:"button",down:true,point:CGPoint(x:200,y:200),screen:.zero)
precondition(events.last?["gesture"] as! UInt64 > gesture)
view.cancelPointerDrag();precondition(view.pointerGesture == nil)
view.anchoredPointer=false
view.pointer(kind:"button",down:true,point:CGPoint(x:200,y:200),screen:.zero)
precondition(events.last?["kind"] as? String == "button","Legacy host compatibility")
let count=events.count;view.inputEnabled=false
view.pointer(kind:"button",down:true,point:CGPoint(x:200,y:200),screen:.zero)
precondition(events.count==count)
window.orderOut(nil)
print("PASS: anchored screen deltas survive viewer movement/resize, repeated samples do not drift, outside release, gesture identity and legacy/read-only behavior")
