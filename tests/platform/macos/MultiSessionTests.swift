let app=NSApplication.shared
let c=AppDelegate()
func deliver(_ packet:[String:Any]) {c.buffer.append(try! JSONSerialization.data(withJSONObject:packet));c.buffer.append(10);c.consume();let deadline=Date().addingTimeInterval(3);while c.decodingFrame,Date()<deadline {RunLoop.current.run(until:Date().addingTimeInterval(0.01))};precondition(!c.decodingFrame)}
func entry(_ id:UInt64,_ token:UInt64)->[String:Any] {return ["id":id,"token":token,"title":"Synthetic app", "bounds":["left":0,"top":0,"right":600,"bottom":400]]}
for sid:UInt64 in [1,2] {
 let state=ApplicationSession(id:sid);c.sessions[sid]=state
 deliver(["type":"selected","session_id":sid,"id":sid*100,"window_groups":true,"resume_token":"ticket-\(sid)"])
 deliver(["type":"window_group","session_id":sid,"root_bounds":["left":0,"top":0,"right":600,"bottom":400],"windows":[entry(sid*100,1),entry(sid*100+1,2)]])
}
let a=c.sessions[1]!,b=c.sessions[2]!
precondition(a.surfaces.count==2 && b.surfaces.count==2)
precondition(a.surfaces[101]!.window.parent===a.surfaces[100]!.window)
precondition(b.surfaces[201]!.window.parent===b.surfaces[200]!.window)
a.surfaces[100]!.view.sendInput?(["kind":"text","text":"A"])
b.surfaces[200]!.view.sendInput?(["kind":"text","text":"B"])
precondition((a.inputEvents.last?["event"] as? [String:Any])?["text"] as? String=="A")
precondition((b.inputEvents.last?["event"] as? [String:Any])?["text"] as? String=="B")
let pixel=NSBitmapImageRep(bitmapDataPlanes:nil,pixelsWide:2,pixelsHigh:2,bitsPerSample:8,samplesPerPixel:4,hasAlpha:true,isPlanar:false,colorSpaceName:.deviceRGB,bytesPerRow:0,bitsPerPixel:0)!
let image=pixel.representation(using:.png,properties:[:])!.base64EncodedString()
deliver(["type":"frame","session_id":2,"id":100,"token":1,"sequence":9,"image":image,"width":2,"height":2])
precondition(a.surfaces[100]!.view.image==nil && b.surfaces[200]!.view.image==nil)
deliver(["type":"frame","session_id":1,"id":100,"token":1,"sequence":1,"image":image,"width":600,"height":400])
precondition(a.surfaces[100]!.view.image != nil && b.surfaces[200]!.view.image==nil)
print("PASS: parentage, input callbacks and frames are isolated by session")
// Exercise scheduler through a real pipe; /bin/cat echoes requests without a network peer.
a.inputEvents.removeAll();b.inputEvents.removeAll();c.lastHeartbeat=Date();c.authenticated=true
let child=Process();child.executableURL=URL(fileURLWithPath:"/bin/cat")
let input=Pipe(),output=Pipe();child.standardInput=input;child.standardOutput=output;try child.run()
c.process=child;c.input=input.fileHandleForWriting
var seen:[UInt64]=[]
for _ in 0..<6 {
 c.poll();precondition(c.pending)
 let request=try! JSONSerialization.jsonObject(with:output.fileHandleForReading.availableData) as! [String:Any]
 seen.append((request["session_id"] as! NSNumber).uint64Value)
 c.pending=false
}
precondition(seen==[1,2,1,2,1,2],"One application starved the other")
input.fileHandleForWriting.closeFile();child.terminate();child.waitUntilExit();c.process=nil;c.input=nil
print("PASS: shared connection schedules both application sessions fairly")
c.wantsConnection=true;c.connectionFailed("network",retryable:true)
precondition(a.surfaces.isEmpty && b.surfaces.isEmpty && a.inputEvents.isEmpty && b.inputEvents.isEmpty)
c.applyQuality(3) // changed while disconnected
c.restoreAfterConnect=true
deliver(["type":"windows","live_video_settings":true,"multi_session":true,"windows":[]])
precondition(a.controlRequests.first?["resume_token"] as? String=="ticket-1")
precondition(b.controlRequests.first?["resume_token"] as? String=="ticket-2")
a.controlRequests.removeAll();b.controlRequests.removeAll()
for sid:UInt64 in [1,2] {
 deliver(["type":"selected","session_id":sid,"id":sid*100,"window_groups":true,"resume_token":"new-\(sid)"])
 deliver(["type":"window_group","session_id":sid,"root_bounds":["left":0,"top":0,"right":600,"bottom":400],"windows":[entry(sid*100,1)]])
}
for state in [a, b] {
 precondition(state.controlRequests.count == 1 && state.controlRequests[0]["fps"] as? Int == 60)
 state.controlRequests.removeAll()
}
let originalWindow=a.surfaces[100]!.window
let originalFrame=originalWindow.frame
originalWindow.makeKeyAndOrderFront(nil)
a.inputEvents=[["id":UInt64(100),"token":UInt64(1),"event":["kind":"text","text":"preserved"]]]
c.lastActiveSessionID=1;c.supportsLiveSettings=true;c.pending=true;c.requestStarted=Date()
c.applyQuality(3);c.applyQuality(1)
precondition(a.surfaces[100]!.window === originalWindow && originalWindow.frame == originalFrame)
precondition(a.selected && b.selected && b.controlRequests.count==1)
precondition(a.inputEvents.count==1 && a.controlRequests.count==1)
precondition(a.controlRequests[0]["command"] as? String=="video_settings")
precondition(a.controlRequests[0]["bitrate"] as? Int==2_000_000)
precondition(b.controlRequests[0]["bitrate"] as? Int==2_000_000 && b.controlRequests[0]["id"] as? UInt64==200)
b.controlRequests.removeAll();a.controlRequests.removeAll();a.inputEvents.removeAll();c.pending=false
print("PASS: live quality changes preserve window identity, geometry, pending input and update both sessions")
a.surfaces[100]!.window.close()
precondition(a.selected && a.surfaces[100]!.locallyClosed && a.controlRequests.isEmpty)
c.lastActiveSessionID=1;c.withSession(1){c.endViewingApplication()}
precondition(!a.selected && a.controlRequests.last?["command"] as? String=="close_window")
precondition(b.selected && b.surfaces[200] != nil && b.controlRequests.isEmpty)
deliver(["type":"closed","session_id":1])
precondition(c.sessions[1]==nil && c.sessions[2]===b)
deliver(["type":"error","session_id":1,"selection_lost":true])
precondition(b.selected)
c.disconnectHost();precondition(c.sessions.count==1 && !c.selected)
print("PASS: both sessions restore; closing one and late replies do not affect the other")
c.pending=true;c.pendingSessionID=0
c.buffer.append(try JSONSerialization.data(withJSONObject:["type":"frame","event":true,"session_id":999,"id":999]));c.buffer.append(10);c.consume()
precondition(c.pending,"Unsolicited video must not complete an outstanding control request")
c.buffer.append(try JSONSerialization.data(withJSONObject:["type":"pong","session_id":0]));c.buffer.append(10);c.consume()
precondition(!c.pending)
print("PASS: pushed frames do not consume pending control responses")

c.supportsPushVideo=false
c.buffer.append(try JSONSerialization.data(withJSONObject:["type":"stream_started","session_id":999]));c.buffer.append(10);c.consume()
precondition(c.supportsPushVideo,"Connection-level stream acknowledgement must survive a removed window session")
print("PASS: stream acknowledgement is connection-scoped even with an obsolete session ID")

// The same fixture applies to Windows and Linux controller adapters.
let contractData = try Data(contentsOf: URL(fileURLWithPath: "tests/contracts/video-profiles.json"))
let contract = try JSONSerialization.jsonObject(with: contractData) as! [String: Any]
let profiles = contract["profiles"] as! [[String: Any]]
let sample = ApplicationSession(id: 42)
precondition(sample.preset == contract["defaultIndex"] as? Int)
for (index, expected) in profiles.enumerated() {
    sample.preset = index
    var actual = c.selectionRequest(id: 42, state: sample)
    for key in ["command", "id", "scope", "alpha_video"] { actual.removeValue(forKey: key) }
    precondition(NSDictionary(dictionary: actual).isEqual(to: expected))
}
print("PASS: controller defaults and selection requests match shared video profiles")
