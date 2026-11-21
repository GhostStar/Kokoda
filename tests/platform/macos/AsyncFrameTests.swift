let app=NSApplication.shared
let c=AppDelegate()
func select(_ token:UInt64) {
    c.rootID=1;c.selected=true
    c.syncGroup(["root_bounds":["left":0,"top":0,"right":100,"bottom":100],"windows":[["id":1,"token":token,"title":"Synthetic async frame","bounds":["left":0,"top":0,"right":100,"bottom":100]]]])
}
let bitmap=NSBitmapImageRep(bitmapDataPlanes:nil,pixelsWide:2,pixelsHigh:2,bitsPerSample:8,samplesPerPixel:4,hasAlpha:true,isPlanar:false,colorSpaceName:.deviceRGB,bytesPerRow:0,bitsPerPixel:0)!
let image=bitmap.representation(using:.png,properties:[:])!.base64EncodedString()
func packet(_ sequence:UInt64,_ token:UInt64=1) {
    c.buffer.append(try! JSONSerialization.data(withJSONObject:["type":"frame","event":true,"id":1,"token":token,"sequence":sequence,"image":image,"width":100,"height":100]))
    c.buffer.append(10)
}
func drain() {
    let deadline=Date().addingTimeInterval(5)
    while c.decodingFrame,Date()<deadline {RunLoop.current.run(until:Date().addingTimeInterval(0.01))}
    precondition(!c.decodingFrame,"Decode completion stalled")
}
select(1)
let gate=DispatchSemaphore(value:0)
c.surfaces[1]!.decodeQueue.async {gate.wait()}
var acknowledgements=0
c.incomingAcks.append{acknowledgements+=1}
packet(1);packet(2);c.consume()
precondition(c.decodingFrame && acknowledgements==0,"Read backpressure released before decode")
gate.signal();drain()
precondition(c.surfaces[1]!.sequence==2 && acknowledgements==1 && c.buffer.isEmpty)
let next=DispatchSemaphore(value:0)
c.surfaces[1]!.decodeQueue.async {next.wait()}
packet(3);c.incomingAcks.append{acknowledgements+=1};c.consume()
precondition(c.decodingFrame)
c.disconnect()
precondition(acknowledgements==2,"Disconnect did not release blocked reader")
select(2)
next.signal()
RunLoop.current.run(until:Date().addingTimeInterval(0.3))
precondition(c.surfaces[1]!.view.image==nil,"Old decode reached replacement connection")
packet(4,2);c.consume();drain()
precondition(c.surfaces[1]!.sequence==4)
c.clearSelection()
print("PASS: ordered background frames, bounded reader backpressure, disconnect release and stale-decode rejection")

// Negotiated consumption credits permit parsing controls during blocked decode.
select(3);c.supportsPlaybackFeedback=true
let slow=DispatchSemaphore(value:0)
c.surfaces[1]!.decodeQueue.async {slow.wait()}
c.pending=true;c.requestStarted=Date();c.pendingCommand="ping"
packet(5,3)
c.buffer.append(try! JSONSerialization.data(withJSONObject:["type":"pong","round_trip_ms":7]));c.buffer.append(10)
c.incomingAcks.append{acknowledgements+=1};c.consume()
precondition(c.decodingFrame && !c.pending && c.heartbeatMS==7 && c.buffer.isEmpty,"Control waited for video decode")
precondition(acknowledgements==3,"Negotiated input parsing retained pipe permits")
let independent=RemoteSurface(id:2,token:4,root:false);c.surfaces[2]=independent
c.buffer.append(try! JSONSerialization.data(withJSONObject:["type":"frame","event":true,"id":2,"token":4,"sequence":1,"image":image,"width":100,"height":100]));c.buffer.append(10);c.consume()
let independentDeadline=Date().addingTimeInterval(2)
while independent.sequence==0,Date()<independentDeadline {RunLoop.current.run(until:Date().addingTimeInterval(0.01))}
precondition(independent.sequence==1 && c.decodingFrame,"One blocked decoder stalled another window")
slow.signal();drain()
precondition(c.playbackFeedback.last?["sequence"] as? UInt64 == 5)
c.clearSelection()
print("PASS: control response bypasses blocked decode and consumption feedback follows presentation")

// Include parsing/UI/feedback scheduling, not just the serial decode queue.
select(4);c.supportsPlaybackFeedback=true
c.queueFrame(["id":1,"token":4,"sequence":6,"image":image,"width":100.0,"height":100.0,"_client_received_at":ProcessInfo.processInfo.systemUptime-0.25],surface:c.surfaces[1]!)
Thread.sleep(forTimeInterval:0.1)
drain()
precondition((c.playbackFeedback.last?["queue_ms"] as? Double ?? 0)>=250,"Client UI delay was misreported as network time: \(String(describing:c.playbackFeedback.last))")
c.handleResponse(["id":1,"token":4,"fps":24,"bitrate":4800000.0,"bitrate_ceiling":6000000.0,"adaptation":"client_busy"],type:"video_feedback_ok")
precondition(c.surfaces[1]!.rateText.contains(L("stats.adaptation.client_busy")))
let oldRate=c.surfaces[1]!.rateText
c.handleResponse(["id":1,"token":99,"fps":10,"bitrate":1000000.0,"bitrate_ceiling":6000000.0,"adaptation":"delivery_backlog"],type:"video_feedback_ok")
precondition(c.surfaces[1]!.rateText==oldRate,"Stale feedback replaced live diagnostics")
c.supportsResolutionBitrate=true
precondition(c.selectionRequest(id:1,state:c.session)["resolution_bitrate"] as? Bool == true)
c.supportsResolutionBitrate=false
precondition(c.selectionRequest(id:1,state:c.session)["resolution_bitrate"]==nil)
c.clearSelection()
print("PASS: local receive/presentation timing, generation-safe rate diagnostics and negotiated Retina bitrate budget")
