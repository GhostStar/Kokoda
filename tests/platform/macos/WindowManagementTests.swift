let app=NSApplication.shared
let c=AppDelegate();c.supportsWindowManagement=true;c.rootID=1;c.selected=true
func entry(_ id:UInt64,_ role:String,_ owner:UInt64?=nil,enabled:Bool=true,width:Double=600,height:Double=400,dpi:Double=96)->[String:Any] {
    var p:[String:Any]=["dpi":dpi,"role":role,"enabled":enabled,"minimized":false,"can_close":enabled,"can_minimize":role=="independent","can_resize":role != "transient"]
    if let owner {p["owner"]=owner}
    return ["id":id,"token":id,"title":"Synthetic role \(id)","bounds":["left":Double(id)*100,"top":100.0,"right":Double(id)*100+width,"bottom":100+height],"presentation":p]
}
var entries=[entry(1,"independent"),entry(2,"independent"),entry(3,"floating",2),entry(4,"modal",1),entry(5,"transient",2)]
func sync(){c.syncGroup(["root_id":1,"root_bounds":["left":100,"top":100,"right":700,"bottom":500],"windows":entries])}
sync()
for s in c.surfaces.values {s.view.image=NSImage(size:s.bounds.size)}
c.layoutChildren()
let root=c.surfaces[1]!,independent=c.surfaces[2]!,panel=c.surfaces[3]!,modal=c.surfaces[4]!,transient=c.surfaces[5]!
precondition(!(independent.window is NSPanel) && independent.window.parent==nil && !independent.window.isExcludedFromWindowsMenu)
precondition(panel.window is NSPanel && panel.window.parent===independent.window && panel.window.isExcludedFromWindowsMenu)
precondition(modal.window.parent===root.window && modal.window.isExcludedFromWindowsMenu)
precondition(transient.window.styleMask==[.borderless] && transient.window.isExcludedFromWindowsMenu)
precondition(independent.window.styleMask.contains(.resizable) && panel.window.styleMask.contains(.resizable))
let panelSize=panel.window.frame.size
precondition(panelSize.width>100 && panelSize.height>100,"First popup frame inherited an empty viewport")
root.window.setContentSize(NSSize(width:700,height:500));c.layoutChildren()
precondition(panel.window.frame.size==panelSize,"Independent owner panel followed unrelated root scaling")
print("PASS: independent native windows, actual owner panels, transient decoration and task-switch eligibility")
entries[0]=entry(1,"independent",enabled:false);sync();c.controlRequests.removeAll();c.inputEvents.removeAll()
precondition(!root.view.inputEnabled && !root.window.canBecomeKey && root.window.styleMask.contains(.resizable))
c.enqueue(["kind":"text","text":"blocked"],id:1,token:1)
precondition(c.inputEvents.isEmpty && !c.windowShouldClose(root.window) && c.controlRequests.isEmpty)
precondition(root.locallyClosed && modal.locallyClosed && !independent.locallyClosed)
sync();precondition(root.locallyClosed && !root.window.isVisible && root.window.standardWindowButton(.closeButton)!.isEnabled)
c.restoreViewers()
entries[0]=entry(1,"independent");sync();precondition(root.view.inputEnabled && root.window.canBecomeKey)
print("PASS: disabled modal owner rejects remote input but permits local close and restore")

c.controlRequests.removeAll()
let sourceBounds=independent.bounds
independent.window.setContentSize(NSSize(width:420,height:310));c.layoutChildren()
let localFrame=independent.window.frame;sync()
precondition(independent.window.frame==localFrame && independent.bounds==sourceBounds && c.controlRequests.isEmpty)
c.windowWillStartLiveResize(Notification(name:NSWindow.willStartLiveResizeNotification,object:independent.window))
precondition(independent.resizeGesture==nil)
print("PASS: default local border scaling preserves geometry across snapshots without host resize")
independent.resizeMode = .hostWindow;independent.hostGeometryChanged=true;c.layoutChildren()
let note=Notification(name:NSWindow.willStartLiveResizeNotification,object:independent.window)
c.windowWillStartLiveResize(note);let gesture=independent.resizeGesture!
for n in 1...20 {
    independent.window.setContentSize(NSSize(width:gesture.content.width+Double(n)*4,height:gesture.content.height+Double(n)*2))
    c.resizeSession(Notification(name:NSWindow.didResizeNotification,object:independent.window))
}
c.windowDidEndLiveResize(Notification(name:NSWindow.didEndLiveResizeNotification,object:independent.window))
let finalFrame=independent.window.frame
let expected=gesture.target(independent.window.contentRect(forFrameRect:finalFrame))
let sizes=c.controlRequests.filter{$0["action"] as? String=="resize"}
precondition(sizes.count==1 && sizes[0]["final"] as? Bool==true)
precondition(sizes[0]["width"] as? Int==expected["width"] && sizes[0]["height"] as? Int==expected["height"],"Resize incorrectly applied Retina backing scale")
sync();precondition(independent.window.frame==finalFrame,"Stale geometry snapped a pending gesture back")
let revision=independent.resizePending!
c.handleResponse(["id":UInt64(2),"token":UInt64(2),"action":"resize","revision":revision-1,"final":true],type:"window_action_ok")
precondition(independent.resizeSettlesAt==nil)
c.handleResponse(["id":UInt64(2),"token":UInt64(2),"action":"resize","revision":revision,"final":true],type:"window_action_ok")
precondition(independent.resizeSettlesAt != nil)
independent.resizeSettlesAt = .distantPast
entries[1]=entry(2,"independent",width:Double(expected["width"]!),height:Double(expected["height"]!));c.controlRequests.removeAll();sync()
precondition(!independent.geometryHeld && c.controlRequests.isEmpty,"State reconciliation echoed a user resize")
print("PASS: bounded final resize, frozen point/pixel conversion, stale snapshot/revision rejection and authoritative reconciliation")

c.controlRequests.removeAll();precondition(!c.windowShouldClose(panel.window))
precondition(c.controlRequests.isEmpty && panel.locallyClosed && !panel.window.isVisible)
precondition(c.surfaces[3]===panel)
sync();c.presentFrame(["sequence":UInt64(100)],surface:panel,image:NSImage(size:panel.bounds.size),width:panel.bounds.width,height:panel.bounds.height,payloadBytes:10)
precondition(!panel.window.isVisible && panel.locallyClosed,"Snapshot or late video reopened a locally closed viewer")
c.restoreViewers();precondition(panel.window.isVisible && !panel.locallyClosed)
precondition(c.windowShouldClose(panel.window)==false)
entries[2]["token"]=UInt64(30);sync();precondition(c.surfaces[3] !== panel && !c.surfaces[3]!.locallyClosed,"New member inherited stale dismissal")
entries.removeAll{($0["id"] as? UInt64)==3};sync()
precondition(c.surfaces[3]==nil && c.surfaces[2]===independent && c.surfaces[1]===root)
c.controlRequests.removeAll()
independent.window.orderFront(nil);independent.window.miniaturize(nil)
RunLoop.current.run(until:Date().addingTimeInterval(0.5))
precondition(c.controlRequests.isEmpty && independent.presentation?.minimized==false)
sync();precondition(independent.window.isMiniaturized,"Host snapshot restored a locally minimized viewer")
independent.window.deminiaturize(nil)
independent.window.performZoom(nil)
RunLoop.current.run(until:Date().addingTimeInterval(0.3))
let zoomFrame=independent.window.frame;sync()
precondition(independent.window.frame==zoomFrame && c.controlRequests.isEmpty,"Local zoom affected the host or was reverted")
print("PASS: local close/restore/replacement identity; native minimize and zoom never mutate host")

// Source physical pixels / host DPI define logical display size, not Mac Retina pixels.
let dpiController=AppDelegate();dpiController.supportsWindowManagement=true;dpiController.rootID=10;dpiController.selected=true
dpiController.syncGroup(["root_id":UInt64(10),"windows":[entry(10,"independent",width:1200,height:800,dpi:240)]])
let scaled=dpiController.surfaces[10]!
precondition(abs(scaled.window.contentRect(forFrameRect:scaled.window.frame).width-480)<1)
scaled.view.image=NSImage(size:scaled.bounds.size)
let logical=scaled.window.contentRect(forFrameRect:scaled.window.frame)
let resize=WindowResizeGesture(source:scaled.bounds,content:logical,pixelsPerPoint:NSSize(width:2.5,height:2.5))
var enlarged=logical;enlarged.size.width+=80
precondition(resize.target(enlarged)["width"]==1400,"Host scale must be applied once without Retina")
dpiController.lastActiveSessionID=0;dpiController.session.metricWindowID=10
let zoom=NSMenuItem();zoom.tag=150;dpiController.changeViewerZoom(zoom)
precondition(abs(scaled.window.contentRect(forFrameRect:scaled.window.frame).width-720)<1 && dpiController.controlRequests.isEmpty)
dpiController.readOnly=true;scaled.configure(scaled.presentation!,readOnly:true)
precondition(scaled.window.standardWindowButton(.closeButton)!.isEnabled)
dpiController.resumeToken="synthetic";dpiController.dismissViewer(scaled)
let savedFrame=scaled.window.frame
dpiController.disconnect();dpiController.supportsWindowManagement=true;dpiController.rootID=10;dpiController.selected=true
dpiController.syncGroup(["root_id":UInt64(10),"windows":[entry(10,"independent",width:1200,height:800,dpi:240)]])
precondition(dpiController.surfaces[10]!.locallyClosed && dpiController.surfaces[10]!.window.frame==savedFrame)
dpiController.restoreViewers();precondition(!dpiController.surfaces[10]!.locallyClosed)
dpiController.disconnectHost()
print("PASS: 250% host scaling, local 150% zoom, read-only close and reconnect preferences")

c.readOnly=true;c.queueWindowAction(independent,"resize",fields:["width":900,"height":700]);precondition(c.controlRequests.isEmpty)
c.readOnly=false
c.syncGroup(["root_id":UInt64(2),"root_bounds":entries[1]["bounds"]!,"windows":[entries[1]]])
precondition(c.rootID==2 && c.surfaces[2]===independent && c.surfaces.count==1,"Anchor promotion destroyed the independent window")
c.disconnect();precondition(c.controlRequests.isEmpty && c.surfaces.isEmpty)
print("PASS: read-only suppression, anchor promotion and disconnect clear window operations")
