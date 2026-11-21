let app=NSApplication.shared
let c=AppDelegate()
c.rootID=1;c.selected=true
c.syncGroup(["root_bounds":["left":0,"top":0,"right":100,"bottom":100],"windows":[["id":1,"token":1,"title":"Synthetic pipeline","bounds":["left":0,"top":0,"right":100,"bottom":100]]]])
let child=Process();child.executableURL=URL(fileURLWithPath:"/bin/cat")
let incoming=Pipe(),outgoing=Pipe();child.standardInput=incoming;child.standardOutput=outgoing;try child.run()
c.process=child;c.input=incoming.fileHandleForWriting
c.supportsPipeline=true;c.authenticated=true;c.pending=true;c.pendingCommand="ping";c.requestStarted=Date()
for i in 0..<13 {c.enqueue(["kind":"text","text":"\(i)"],id:1,token:1)}
precondition(c.asynchronousRequests.count==12 && c.inputEvents.count==1,"Input pipeline did not honor its bounded outstanding limit")
var bytes=Data()
while bytes.filter({$0==10}).count<12 {bytes.append(outgoing.fileHandleForReading.availableData)}
let packets=bytes.split(separator:10).map{try! JSONSerialization.jsonObject(with:Data($0)) as! [String:Any]}
precondition(packets.count==12)
for (i,packet) in packets.enumerated() {
    precondition(packet["bridge_async"] as? Bool == true)
    precondition((packet["events"] as? [[String:Any]])?.first?["text"] as? String == "\(i)","Input reordered")
}
func reply(_ packet:[String:Any]) {c.buffer.append(try! JSONSerialization.data(withJSONObject:packet));c.buffer.append(10);c.consume()}
reply(["type":"input_ok","bridge_async":true,"session_id":0])
precondition(c.pending && c.pendingCommand=="ping" && c.asynchronousRequests.count==11,"Input ACK consumed a different request")
c.pendingCommand="clipboard";c.poll()
precondition(c.inputEvents.count==1,"Input crossed a clipboard barrier")
reply(["type":"pong","round_trip_ms":3])
precondition(!c.pending && c.heartbeatMS==3)
c.process=nil;c.input=nil
incoming.fileHandleForWriting.closeFile();child.terminate();child.waitUntilExit()
c.disconnect()
precondition(c.asynchronousRequests.isEmpty && c.inputEvents.isEmpty)
print("PASS: 12 ordered input batches sent before replies, bounded backlog, ACK correlation, clipboard barrier and disconnect cleanup")

let parser=BridgePacketParser()
var framed=try JSONSerialization.data(withJSONObject:["type":"frame","image":Data([1,2,3]).base64EncodedString(),"alpha_png":Data([4,5]).base64EncodedString()]);framed.append(10)
let partial=try parser.append(Data(framed.prefix(7)));precondition(partial.isEmpty)
var tail=Data(framed.dropFirst(7));tail.append(Data("{\"type\":\"pong\"}\n".utf8))
let parsed=try parser.append(tail)
precondition(parsed.count==2 && parsed[1]["type"] as? String=="pong")
precondition(parsed[0]["binary_image"] as? Data==Data([1,2,3]) && parsed[0]["binary_alpha"] as? Data==Data([4,5]))
precondition(parsed[0]["image"]==nil && parsed[0]["alpha_png"]==nil)
do {_ = try BridgePacketParser().append(Data(repeating:32,count:24*1024*1024+1));preconditionFailure("Unbounded IPC input accepted")} catch {}
print("PASS: background IPC parsing preserves fragmented message order, decodes binary payloads and rejects oversized input")

let stamp=ProcessInfo.processInfo.systemUptime-0.2
let timed=try BridgePacketParser().append(framed,receivedAt:stamp)
precondition(timed[0]["_client_received_at"] as? Double==stamp)
let feedbackChild=Process();feedbackChild.executableURL=URL(fileURLWithPath:"/bin/cat")
let feedbackIn=Pipe(),feedbackOut=Pipe();feedbackChild.standardInput=feedbackIn;feedbackChild.standardOutput=feedbackOut;try feedbackChild.run()
c.process=feedbackChild;c.input=feedbackIn.fileHandleForWriting
c.request(["command":"video_feedback","decode_ms":5.0,"queue_ms":0.0,"_client_received_at":stamp],asynchronous:true)
let feedbackBytes=feedbackOut.fileHandleForReading.availableData
let sent=try JSONSerialization.jsonObject(with:feedbackBytes) as! [String:Any]
precondition(sent["_client_received_at"]==nil && (sent["queue_ms"] as? Double ?? 0)>=195,"Local scheduling time leaked or was omitted")
c.process=nil;c.input=nil;feedbackIn.fileHandleForWriting.closeFile();feedbackChild.terminate();feedbackChild.waitUntilExit();c.disconnect()
print("PASS: receiver-owned parsing timestamp and final feedback queue accounting without transmitting local timestamps")
