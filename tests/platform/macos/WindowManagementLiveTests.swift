setbuf(stdout,nil)
let app=NSApplication.shared
let c=AppDelegate()
let oldAddress=UserDefaults.standard.object(forKey:"hostAddress")
c.clipboardBoard=NSPasteboard(name:NSPasteboard.Name("Kokoda.WindowRoles.\(UUID().uuidString)"))
c.applicationDidFinishLaunching(Notification(name:NSApplication.didFinishLaunchingNotification))
let secretURL=URL(fileURLWithPath:CommandLine.arguments[3])
let command=secretURL.deletingLastPathComponent().appendingPathComponent("roles-command.txt")
func trigger(_ name:String)throws {try "\(UUID().uuidString):\(name)".write(to:command,atomically:true,encoding:.utf8)}
defer {
    try? trigger("stop")
    c.disconnectHost();c.timer?.invalidate();c.window.orderOut(nil);c.password.stringValue="";c.clipboardBoard.releaseGlobally()
    if let oldAddress {UserDefaults.standard.set(oldAddress,forKey:"hostAddress")} else {UserDefaults.standard.removeObject(forKey:"hostAddress")}
}
func wait(_ message:String,_ predicate:()->Bool)throws {
    let deadline=Date().addingTimeInterval(30)
    while !predicate(),Date()<deadline {RunLoop.current.run(until:Date().addingTimeInterval(0.02))}
    guard predicate() else{throw NSError(domain:"WindowManagementLiveTests",code:1,userInfo:[NSLocalizedDescriptionKey:message+"; status="+c.status.stringValue+"; bridge="+String(decoding:c.errors.suffix(1024),as:UTF8.self)])}
}
let secret=try JSONSerialization.jsonObject(with:Data(contentsOf:secretURL)) as! [String:String]
c.host.stringValue=CommandLine.arguments[2];c.password.stringValue=secret["password"]!;c.connectHost()
try wait("Authentication and role capability"){c.authenticated && c.supportsWindowManagement}
try wait("Root discovery"){c.managerRows().contains{$0.title=="Kokoda Roles Root"}}
try trigger("animate")
c.activateManagerRow(c.managerRows().first{$0.title=="Kokoda Roles Root"}!)
try wait("All viewers rendered"){c.sessions.values.contains{$0.surfaces.count==5 && $0.surfaces.values.allSatisfy{$0.view.image != nil}}}
let state=c.sessions.values.first{$0.surfaces.count==5}!
func surface(_ title:String)->RemoteSurface? {state.surfaces.values.first{$0.window.title==title}}
let root=surface("Kokoda Roles Root")!,independent=surface("Kokoda Roles Independent")!,tool=surface("Kokoda Roles Tool")!,fixed=surface("Kokoda Roles Fixed")!,owned=surface("Kokoda Roles Owned App")!
precondition(!(independent.window is NSPanel) && independent.window.parent==nil && !independent.window.isExcludedFromWindowsMenu)
precondition(owned.window.parent==nil && !owned.window.isExcludedFromWindowsMenu)
precondition(tool.window.parent===root.window && tool.window.isExcludedFromWindowsMenu)
precondition(fixed.window.styleMask.contains(.resizable),"Fixed host window must permit local scaling")
print("PASS: real Windows roles produce separate native windows, owned panels and fixed-size controls")
let initialBounds=tool.bounds
let initialFrame=tool.window.contentRect(forFrameRect:tool.window.frame)
precondition(abs(initialFrame.width-initialBounds.width*96/tool.hostDPI)<2,"Initial display scale must account for host monitor DPI")
let proposed=tool.window.frameRect(forContentRect:NSRect(origin:initialFrame.origin,size:NSSize(width:initialFrame.width+100,height:initialFrame.height)))
let constrained=c.windowWillResize(tool.window,to:proposed.size)
var resizedFrame=tool.window.frame;resizedFrame.origin.y=resizedFrame.maxY-constrained.height;resizedFrame.size=constrained
tool.window.setFrame(resizedFrame,display:true)
c.windowDidResize(Notification(name:NSWindow.didResizeNotification,object:tool.window))
let localFrame=tool.window.frame
func fillsCanvas(_ s:RemoteSurface) {
    let image=s.view.imageRect,bounds=s.view.bounds
    precondition(image.minX<=0.01 && image.minY<=0.01 && image.maxX>=bounds.maxX-0.01 && image.maxY>=bounds.maxY-0.01,"Local viewer has padding")
}
fillsCanvas(tool)
let until=Date().addingTimeInterval(1)
while Date()<until {RunLoop.current.run(until:Date().addingTimeInterval(0.02))}
precondition(tool.bounds==initialBounds && tool.window.frame==localFrame,"Local scaling changed host or snapped back")
print("PASS: local border scaling leaves actual Windows bounds unchanged; monitor DPI converts physical capture to logical points")
tool.resizeControl.popup.selectItem(at:1);tool.resizeControl.changed()
precondition(tool.resizeMode == .hostWindow && root.resizeMode == .scale && tool.window.firstResponder === tool.view)
precondition(tool.window.contentAspectRatio == .zero)
let begin=Notification(name:NSWindow.willStartLiveResizeNotification,object:tool.window)
c.windowWillStartLiveResize(begin);let gesture=tool.resizeGesture!
for n in 1...8 {
    tool.window.setContentSize(NSSize(width:gesture.content.width+Double(n)*10,height:gesture.content.height+Double(n)*6))
    c.windowDidResize(Notification(name:NSWindow.didResizeNotification,object:tool.window))
    RunLoop.current.run(until:Date().addingTimeInterval(0.035))
}
let expected=gesture.target(tool.window.contentRect(forFrameRect:tool.window.frame))
c.windowDidEndLiveResize(Notification(name:NSWindow.didEndLiveResizeNotification,object:tool.window))
try wait("Remote resize and reconciliation"){!tool.geometryHeld && abs(tool.bounds.width-Double(expected["width"]!))<=2 && abs(tool.bounds.height-Double(expected["height"]!))<=2}
precondition(state.surfaces[independent.id]===independent)
print("PASS: native popup border resize changes the Windows source bounds without replacing siblings")
let before=independent.sequence
let unchangedBounds=independent.bounds
independent.window.miniaturize(nil)
RunLoop.current.run(until:Date().addingTimeInterval(1))
precondition(independent.window.isMiniaturized && independent.presentation?.minimized==false)
precondition(state.surfaces[independent.id]===independent,"Minimize replaced the surface")
independent.window.deminiaturize(nil)
try wait("Local restore resumes visible frames"){independent.sequence>before && !independent.window.isMiniaturized}
independent.resizeControl.popup.selectItem(at:1);independent.resizeControl.changed()
independent.window.performZoom(nil)
let zoomFrame=independent.window.frame;fillsCanvas(independent)
RunLoop.current.run(until:Date().addingTimeInterval(1))
precondition(independent.bounds==unchangedBounds && independent.presentation?.maximized==false && independent.window.frame==zoomFrame,"Local maximize changed host geometry or snapped back")
independent.window.performZoom(nil)
let toolBounds=tool.bounds
tool.window.performZoom(nil)
let toolZoomFrame=tool.window.frame;fillsCanvas(tool)
RunLoop.current.run(until:Date().addingTimeInterval(0.5))
precondition(tool.bounds==toolBounds && tool.window.frame==toolZoomFrame,"Panel zoom escaped local control")
tool.window.performZoom(nil)
print("PASS: native minimize, restore and normal/panel zoom stay local even in host-resize mode")
try trigger("modal")
try wait("Modal shown and owner blocked"){surface("Kokoda Roles Modal")?.view.image != nil && !root.view.inputEnabled}
let modal=surface("Kokoda Roles Modal")!
precondition(modal.role=="modal" && modal.window.parent===root.window && !root.window.canBecomeKey)
modal.window.performClose(nil)
try wait("Modal locally closed"){modal.locallyClosed && !modal.window.isVisible}
RunLoop.current.run(until:Date().addingTimeInterval(1))
precondition(surface("Kokoda Roles Modal")===modal && !root.view.inputEnabled,"Local close must leave the host modal intact")
let stoppedSequence=modal.sequence
RunLoop.current.run(until:Date().addingTimeInterval(0.5))
precondition(modal.sequence==stoppedSequence && modal.heldFeedback.count<=2,"Hidden viewer did not stop consuming credits")
c.withSession(state.id){c.restoreViewers()}
try wait("Restored modal resumes video"){modal.window.isVisible && modal.sequence>stoppedSequence}
c.withSession(state.id){c.queueWindowAction(modal,"close")}
try wait("Explicit remote modal close enables owner"){surface("Kokoda Roles Modal")==nil && root.view.inputEnabled}
try trigger("child")
try wait("New process native window"){surface("Kokoda Roles Child Process")?.view.image != nil}
let child=surface("Kokoda Roles Child Process")!
precondition(child.role=="independent" && child.window.parent==nil && !child.window.isExcludedFromWindowsMenu)
child.window.performClose(nil)
try wait("Child local close"){child.locallyClosed && !child.window.isVisible}
RunLoop.current.run(until:Date().addingTimeInterval(0.5))
precondition(surface("Kokoda Roles Child Process")===child)
c.withSession(state.id){c.restoreViewers();c.queueWindowAction(child,"close")}
try wait("Explicit child remote close"){surface("Kokoda Roles Child Process")==nil}
print("PASS: local modal close preserves host blocking, bounds hidden credits, resumes video; descendant window stays independent")
root.window.performClose(nil)
try wait("Root locally closed"){root.locallyClosed && tool.locallyClosed && !independent.locallyClosed}
RunLoop.current.run(until:Date().addingTimeInterval(0.5))
precondition(state.surfaces[root.id]===root && independent.window.isVisible)
c.withSession(state.id){c.restoreViewers();c.queueWindowAction(root,"close")}
try wait("Independent survives original root close"){state.rootID != root.id && state.surfaces[independent.id]===independent && state.surfaces[root.id]==nil}
c.lastActiveSessionID=state.id;c.endViewingApplication()
try wait("Viewing ended"){state.surfaces.isEmpty && !state.selected && !c.pending}
c.refreshWindows()
try wait("Remote application remains running"){!c.pending && c.windows.contains{($0["id"] as? NSNumber)?.uint64Value==independent.id}}
print("PASS: root close promotes surviving independent window; End Viewing leaves the remote application running")
