let application=NSApplication.shared
let c=AppDelegate()
c.applicationDidFinishLaunching(Notification(name:NSApplication.didFinishLaunchingNotification))
c.timer?.invalidate()
let manager=c.manager!
func deliver(_ packet:[String:Any]) {
    c.buffer.append(try! JSONSerialization.data(withJSONObject:packet));c.buffer.append(10);c.consume()
}
func catalog(_ entries:[[String:Any]]) {deliver(["type":"windows","multi_session":true,"window_management":true,"windows":entries])}
func entry(_ id:UInt64,_ title:String,pid:Int=10)->[String:Any] {["id":id,"process_id":pid,"thread_id":11,"title":title]}
func member(_ id:UInt64,_ token:UInt64,_ title:String,role:String="independent",owner:UInt64?=nil,enabled:Bool=true)->[String:Any] {
    var p:[String:Any]=["dpi":192,"role":role,"enabled":enabled,"can_resize":true]
    if let owner=owner {p["owner"]=owner}
    return ["id":id,"token":token,"title":title,"identity":"synthetic-\(id)-\(token)","presentation":p,"bounds":["left":0,"top":0,"right":800,"bottom":600]]
}
func group(_ members:[[String:Any]]) {deliver(["type":"window_group","session_id":1,"root_bounds":["left":0,"top":0,"right":800,"bottom":600],"windows":members])}
func choose(_ id:UInt64) {
    manager.refresh(force:true)
    let index=manager.rows.firstIndex{$0.id==id}!
    manager.table.selectRowIndexes(IndexSet(integer:index),byExtendingSelection:false)
}
precondition(c.process==nil && !c.authenticated && manager.rows.isEmpty)
precondition(!manager.form.isHidden && !manager.primary.isEnabled)
c.toggleReconnect();manager.refresh(force:true)
precondition(manager.reconnect.state == .off)
c.toggleReconnect();manager.refresh(force:true)
precondition(manager.reconnect.state == .on)
c.host.stringValue="192.0.2.10:47832" // documentation-only test address; never connected
catalog([entry(1,"Same title"),entry(2,"Same title"),entry(3,"")])
precondition(manager.rows.count==3 && manager.form.isHidden)
precondition(manager.rows.first{$0.id==3}!.title==L("windows.untitled"))
choose(2)
let selectedKey=manager.selected!.key
catalog([entry(3,""),entry(2,"Renamed"),entry(1,"Same title")])
precondition(manager.selected?.key==selectedKey && manager.selected?.id==2)
let stale=manager.selected!
catalog([entry(1,"Same title"),entry(3,"")])
precondition(manager.selected==nil && !manager.primary.isEnabled)
c.activateManagerRow(stale);precondition(c.sessions.count==1)
catalog([entry(2,"Renamed",pid:20),entry(1,"Same title"),entry(3,"")])
c.activateManagerRow(stale);precondition(c.sessions.count==1)
print("PASS: empty titles, duplicate titles, stable refresh selection, disappearance and reused catalog handles")
choose(1);manager.activateSelected();precondition(c.sessions[1]?.requestedRoot==1)
let count=c.sessions.count
c.openManagerWindow(1);precondition(c.sessions.count==count)
precondition(c.managerRows().first{$0.id==1}!.state=="opening")
precondition(c.sessions[1]!.controlRequests.count==1)
deliver(["type":"selected","session_id":1,"id":1,"window_groups":true])
c.sessions[1]!.controlRequests.removeAll()
let initial=[member(1,1,"Same title"),member(4,4,"Independent sibling"),member(5,5,"Owned panel",role:"floating",owner:1),member(6,6,"Menu",role:"transient",owner:1)]
group(initial)
precondition(!c.managerRows().contains{$0.id==6})
manager.tabs.selectedSegment=1;manager.refresh(force:true)
precondition(manager.rows.map{$0.id}.sorted()==[1,4])
let rootRow=c.managerRows().first{$0.id==1}!
manager.expanded.insert(rootRow.key);manager.refresh(force:true)
precondition(manager.rows.contains{$0.id==5})
let root=c.sessions[1]!.surfaces[1]!,sibling=c.sessions[1]!.surfaces[4]!,panel=c.sessions[1]!.surfaces[5]!
c.withSession(1) {c.dismissViewer(root);c.dismissViewer(sibling)}
precondition(root.locallyClosed && panel.locallyClosed && sibling.locallyClosed)
c.activateManagerRow(rootRow)
precondition(!root.locallyClosed && !panel.locallyClosed && sibling.locallyClosed)
precondition(c.sessions.count==count)
precondition(!c.sessions[1]!.controlRequests.contains{$0["action"] as? String=="close"})
print("PASS: deduplicated opening, owned expansion, transient exclusion, targeted local restoration and independent siblings")
group([member(1,1,"Same title",enabled:false),member(4,4,"Independent sibling"),member(7,7,"Confirm",role:"modal",owner:1)])
let blocked=c.managerRows().first{$0.id==1}!
precondition(blocked.action=="modal")
let modal=c.sessions[1]!.surfaces[7]!
c.withSession(1){c.dismissViewer(modal)}
c.activateManagerRow(blocked)
precondition(!modal.locallyClosed && root.view.inputEnabled==false)
print("PASS: blocked owner restores its authorized modal instead of injecting into the owner")
manager.search.stringValue="Confirm";manager.refresh(force:true)
precondition(manager.rows.count==1 && manager.rows[0].id==7)
manager.search.stringValue="no-synthetic-match";manager.refresh(force:true)
precondition(manager.rows.isEmpty && !manager.empty.isHidden)
manager.search.stringValue="";manager.tabs.selectedSegment=0;manager.refresh(force:true)
choose(2);manager.activateSelected()
let opening=c.sessions.values.first{$0.requestedRoot==2}!
c.pending=true;c.pendingSessionID=opening.id;c.pendingCommand="select_window";c.requestStarted=Date()
c.endManagerSession(opening.id)
precondition(opening.ending && opening.surfaces.isEmpty)
deliver(["type":"selected","session_id":opening.id,"id":2,"window_groups":true])
deliver(["type":"window_group","session_id":opening.id,"windows":[member(2,2,"Late")]])
precondition(!opening.selected && opening.surfaces.isEmpty)
precondition(opening.controlRequests.last?["command"] as? String=="close_window")
deliver(["type":"closed","session_id":opening.id])
precondition(c.sessions[opening.id]==nil && c.sessions[1] != nil)
print("PASS: search includes owned windows; cancel during opening rejects late selection and group replies")
let tokenRow=c.managerRows().first{$0.id==4}!
group([member(1,1,"Same title"),member(4,44,"Independent sibling")])
let replacement=c.sessions[1]!.surfaces[4]!
c.withSession(1){c.dismissViewer(replacement)}
c.activateManagerRow(tokenRow);precondition(replacement.locallyClosed)
print("PASS: stale viewer tokens cannot restore a replacement window")
// Revocation clears previously listed titles and actions, not only video surfaces.
let revokedRow=c.managerRows().first{$0.id==1}!
deliver(["type":"error","session_id":1,"selection_lost":true,"message":"Synthetic revoke"])
precondition(!c.managerRows().contains{$0.id==1 || $0.id==4})
c.activateManagerRow(revokedRow);precondition(c.sessions[1]==nil)
print("PASS: revocation removes private catalog metadata and disables stale actions")
// Rebuild harmless synthetic previews after revocation.
catalog([entry(20,"Notes — Project plan"),entry(21,"Browser — Documentation"),entry(22,"")])
c.openManagerWindow(20)
let previewSID=c.sessions.values.first{$0.requestedRoot==20}!.id
deliver(["type":"selected","session_id":previewSID,"id":20,"window_groups":true])
deliver(["type":"window_group","session_id":previewSID,"root_bounds":["left":0,"top":0,"right":800,"bottom":600],"windows":[member(20,20,"Notes — Project plan")]])
// Capture the synthetic manager in each requested language, with no private windows.
manager.search.stringValue="";manager.tabs.selectedSegment=0;manager.refresh(force:true)
c.status.stringValue=L("status.choose_window")
c.updateMetrics()
c.window.contentView!.layoutSubtreeIfNeeded()
let content=c.window.contentView!
let stack=content.subviews.first as! NSStackView
for view in stack.arrangedSubviews where !view.isHidden {
    let rect=view.convert(view.bounds,to:content)
    precondition(rect.minX>=0 && rect.maxX<=content.bounds.width+1 && rect.minY>=0 && rect.maxY<=content.bounds.height+1,"Manager control overflow")
}
manager.options.isHidden=false
c.window.contentView!.layoutSubtreeIfNeeded()
for view in stack.arrangedSubviews where !view.isHidden {
    let rect=view.convert(view.bounds,to:content)
    precondition(rect.minY>=0 && rect.maxY<=content.bounds.height+1,"Expanded options overflow")
}
manager.options.isHidden=true
c.window.makeKeyAndOrderFront(nil)
RunLoop.current.run(until:Date().addingTimeInterval(0.3))
c.window.displayIfNeeded()
if let output=ProcessInfo.processInfo.environment["KOKODA_MANAGER_SCREENSHOT"],let bitmap=content.bitmapImageRepForCachingDisplay(in:content.bounds) {
    content.cacheDisplay(in:content.bounds,to:bitmap)
    try bitmap.representation(using:.png,properties:[:])!.write(to:URL(fileURLWithPath:output))
    let capture=Process();capture.executableURL=URL(fileURLWithPath:"/usr/sbin/screencapture")
    capture.arguments=["-x","-l",String(c.window.windowNumber),output+".screen.png"]
    try capture.run();capture.waitUntilExit()

}
let disconnectedRow=c.managerRows().first!
c.disconnectHost();precondition(c.managerRows().isEmpty && manager.rows.isEmpty && c.windows.isEmpty)
c.activateManagerRow(disconnectedRow);precondition(!c.selected && c.sessions.count==1)
c.applicationWillTerminate(Notification(name:NSApplication.willTerminateNotification));c.window.orderOut(nil)
print("PASS: manager layout and disconnect privacy; stale actions cannot reopen another connection")
