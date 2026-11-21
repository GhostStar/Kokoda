setbuf(stdout,nil)
let app=NSApplication.shared
let c=AppDelegate();c.supportsWindowManagement=true;c.rootID=1;c.selected=true
func entry(_ id:UInt64,_ role:String,_ owner:UInt64?=nil,width:Double=600,height:Double=400,enabled:Bool=true)->[String:Any] {
    var p:[String:Any]=["role":role,"enabled":enabled,"can_resize":true,"dpi":96]
    if let owner {p["owner"]=owner}
    return ["id":id,"token":id,"title":"Kokoda resize test","bounds":["left":100.0,"top":100.0,"right":100+width,"bottom":100+height],"presentation":p]
}
c.syncGroup(["root_id":UInt64(1),"windows":[entry(1,"independent"),entry(2,"floating",1,width:300,height:200),entry(3,"transient",1,width:100,height:60)]])
let root=c.surfaces[1]!,panel=c.surfaces[2]!,tip=c.surfaces[3]!
for s in c.surfaces.values {s.view.image=NSImage(size:s.bounds.size);c.updateViewerControls(s)}
c.layoutChildren()
func fullImage(_ s:RemoteSurface) {
    let bounds=s.view.bounds,image=s.view.imageRect
    precondition(image.minX<=0.01 && image.minY<=0.01 && image.maxX>=bounds.maxX-0.01 && image.maxY>=bounds.maxY-0.01,"Viewer has padding: \(bounds), image \(image)")
    precondition(abs(image.width/image.height-s.displaySourceSize.width/s.displaySourceSize.height)<0.0001,"Image was stretched")
}
precondition(root.window.titlebarAccessoryViewControllers.contains{$0 === root.resizeControl})
precondition(panel.window.titlebarAccessoryViewControllers.contains{$0 === panel.resizeControl})
precondition(tip.resizeControl.view.window==nil,"Transient gained a title bar")
precondition(root.resizeControl.popup.title==L("viewer.scale_local"))
root.window.makeKeyAndOrderFront(nil);root.window.makeFirstResponder(root.resizeControl.popup)
// Title-bar actions target their owning surface, not the last focused window.
panel.resizeControl.popup.selectItem(at:1);panel.resizeControl.changed()
precondition(panel.resizeMode == .hostWindow && root.resizeMode == .scale)
precondition(panel.window.contentAspectRatio == .zero)
precondition(panel.window.firstResponder === panel.view)
panel.resizeControl.popup.selectItem(at:0);panel.resizeControl.changed()
precondition(panel.window.contentAspectRatio.width/panel.window.contentAspectRatio.height==1.5)
print("PASS: per-window title-bar mode, owning-window routing, native aspect constraint and IME focus restoration")

c.controlRequests.removeAll()
func resizeLocal(_ width:Double,_ height:Double) {
    let current=root.window.contentRect(forFrameRect:root.window.frame)
    let proposed=root.window.frameRect(forContentRect:NSRect(origin:current.origin,size:NSSize(width:width,height:height)))
    let constrained=c.windowWillResize(root.window,to:proposed.size)
    var frame=root.window.frame;frame.origin.y=frame.maxY-constrained.height;frame.size=constrained
    root.window.setFrame(frame,display:true)
    c.windowDidEndLiveResize(Notification(name:NSWindow.didEndLiveResizeNotification,object:root.window))
    fullImage(root)
}
resizeLocal(840,400)
precondition(abs(root.view.bounds.width-840)<0.5 && abs(root.view.bounds.height-560)<0.5)
resizeLocal(840,420)
precondition(abs(root.view.bounds.width-630)<0.5 && abs(root.view.bounds.height-420)<0.5)
precondition(!c.controlRequests.contains{$0["action"] as? String=="resize"})
let saved=root.window.frame;c.layoutChildren();precondition(root.window.frame==saved)
print("PASS: horizontal and vertical edge resize preserve content aspect, fill canvas and never resize host")

root.resizeControl.popup.selectItem(at:1);root.resizeControl.changed()
let arbitrary=NSSize(width:810,height:610)
precondition(c.windowWillResize(root.window,to:arbitrary)==arbitrary,"Host resize was constrained to local image aspect")
c.windowWillStartLiveResize(Notification(name:NSWindow.willStartLiveResizeNotification,object:root.window))
precondition(root.resizeGesture != nil)
root.window.setContentSize(NSSize(width:730,height:470))
c.windowDidResize(Notification(name:NSWindow.didResizeNotification,object:root.window))
c.windowDidEndLiveResize(Notification(name:NSWindow.didEndLiveResizeNotification,object:root.window))
precondition(c.controlRequests.contains{$0["action"] as? String=="resize" && $0["final"] as? Bool==true})
root.resizeControl.popup.selectItem(at:0);root.resizeControl.changed();fullImage(root)
precondition(!root.geometryHeld && !c.controlRequests.contains{$0["action"] as? String=="resize"})
print("PASS: explicit host mode unlocks independent dimensions; returning local clears resize state and padding")

// Bounds may lead decoded video. Fit the frame actually being displayed.
root.bounds.size=NSSize(width:1200,height:1600);c.layoutChildren();fullImage(root)
c.presentFrame(["sequence":UInt64(10)],surface:root,image:NSImage(size:NSSize(width:1200,height:1600)),width:1200,height:1600,payloadBytes:1)
fullImage(root)
precondition(abs(root.window.contentAspectRatio.width-0.75)<0.001)
c.presentFrame(["sequence":UInt64(11)],surface:root,image:NSImage(size:NSSize(width:1600,height:400)),width:1600,height:400,payloadBytes:1)
fullImage(root)
precondition(abs(root.window.contentAspectRatio.width-4)<0.001)
root.resizeControl.popup.selectItem(at:1);root.resizeControl.changed();c.controlRequests.removeAll()
root.window.performZoom(nil);fullImage(root)
panel.resizeControl.popup.selectItem(at:1);panel.resizeControl.changed()
panel.window.orderFront(nil);panel.window.performZoom(nil);fullImage(panel)
precondition(!c.controlRequests.contains{$0["action"] as? String=="resize"},"Native zoom resized the host")
print("PASS: source aspect changes and normal/panel native maximize fit the visible image without letterboxing")

c.readOnly=true;c.updateViewerControls(root)
precondition(root.resizeMode == .scale && !root.resizeControl.popup.item(at:1)!.isEnabled)
precondition(root.resizeControl.popup.toolTip!.contains(L("viewer.host_read_only")))
root.resizeControl.popup.selectItem(at:1);root.resizeControl.changed()
precondition(root.resizeMode == .scale && root.resizeControl.popup.indexOfSelectedItem==0)
c.readOnly=false
c.applyPresentation(WindowPresentation(["role":"independent","enabled":false,"can_resize":true]),to:root)
precondition(!root.resizeControl.popup.item(at:1)!.isEnabled && root.resizeControl.popup.item(at:0)!.isEnabled)
print("PASS: unavailable host resizing explains why and safely returns to proportional local scaling")
c.disconnectHost()
