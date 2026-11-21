let app = NSApplication.shared
let controller = AppDelegate()
controller.clipboardBoard = NSPasteboard(name:NSPasteboard.Name("Kokoda.ClipboardTest.\(UUID().uuidString)"))
defer {controller.clipboardBoard.releaseGlobally()}
let process = Process()
process.executableURL=URL(fileURLWithPath:CommandLine.arguments[1])
process.arguments=[CommandLine.arguments[2],CommandLine.arguments[3],"--bridge"]
let outgoing=Pipe(),incoming=Pipe()
process.standardInput=outgoing;process.standardOutput=incoming
try process.run()
controller.process=process;controller.input=outgoing.fileHandleForWriting
var wire=Data()
func readReply()->Data {
    while wire.firstIndex(of:10)==nil {let bytes=incoming.fileHandleForReading.availableData;precondition(!bytes.isEmpty,"Bridge exited");wire.append(bytes)}
    let end=wire.firstIndex(of:10)!,line=Data(wire[...end]);wire.removeSubrange(...end);return line
}
func exchange(_ message:[String:Any])->[String:Any] {
    var bytes=try! JSONSerialization.data(withJSONObject:message);bytes.append(10)
    outgoing.fileHandleForWriting.write(bytes)
    return try! JSONSerialization.jsonObject(with:readReply()) as! [String:Any]
}
func drain() {
    var replies=0
    while controller.pending || !controller.inputEvents.isEmpty {
        controller.lastGroup=Date()
        if !controller.pending {controller.poll()}
        precondition(controller.pending)
        let data=readReply()
        let reply=try! JSONSerialization.jsonObject(with:data) as! [String:Any]
        precondition(reply["type"] as? String != "error","Remote clipboard/input command failed")
        controller.buffer.append(data);controller.consume()
        precondition(!controller.status.stringValue.contains("error"),"Remote input error")
        replies+=1;precondition(replies<50,"Input queue did not drain")
    }
}
let windows=exchange(["command":"list_windows"])["windows"] as! [[String:Any]]
let fixture=windows.first {$0["title"] as? String == "Kokoda Clipboard Test"}!
let id=(fixture["id"] as! NSNumber).uint64Value
let selected=exchange(["command":"select_window","id":id,"scope":"application","codec":"h264"])
precondition(selected["type"] as? String == "selected")
controller.rootID=id;controller.selected=true
controller.syncGroup(exchange(["command":"window_group"]))
let surface=controller.surfaces[id]!,token=surface.token
let dimensions=surface.bounds
controller.lastGroup=Date()
func event(_ item:[String:Any]) {controller.lastGroup=Date();controller.enqueue(item,id:id,token:token);drain()}
event(["kind":"button","x":0.5,"y":0.4,"width":Int(dimensions.width),"height":Int(dimensions.height),"button":0,"down":true])
event(["kind":"button","x":0.5,"y":0.4,"width":Int(dimensions.width),"height":Int(dimensions.height),"button":0,"down":false])
func shortcut(_ code:UInt16) {
    controller.lastGroup=Date()
    let down=NSEvent.keyEvent(with:.keyDown,location:.zero,modifierFlags:.command,timestamp:0,windowNumber:surface.window.windowNumber,context:nil,characters:"",charactersIgnoringModifiers:"",isARepeat:false,keyCode:code)!
    let up=NSEvent.keyEvent(with:.keyUp,location:.zero,modifierFlags:.command,timestamp:0,windowNumber:surface.window.windowNumber,context:nil,characters:"",charactersIgnoringModifiers:"",isARepeat:false,keyCode:code)!
    surface.view.keyDown(with:down);surface.view.keyUp(with:up);drain()
}
func setLocal(_ text:String) {controller.clipboardBoard.clearContents();controller.clipboardBoard.setString(text,forType:.string)}
func remoteText()->String {Thread.sleep(forTimeInterval:0.15);return (try! String(contentsOfFile:CommandLine.arguments[4],encoding:.utf8)).replacingOccurrences(of:"\r\n",with:"\n")}
let text="Kokoda clipboard 中文🙂\n第二行\tTabbed\n"
shortcut(0);setLocal(text);shortcut(9)
precondition(remoteText()==text,"Mac clipboard paste failed")
print("PASS: Mac shortcut -> Windows clipboard -> multiline Unicode paste")
setLocal("Local sentinel");shortcut(0);shortcut(8)
precondition(controller.clipboardBoard.string(forType:.string)==text,"Remote copy did not reach Mac clipboard")
print("PASS: Windows copy -> Mac clipboard")
shortcut(0);shortcut(7)
precondition(remoteText().isEmpty && controller.clipboardBoard.string(forType:.string)==text,"Cut failed")
shortcut(9);precondition(remoteText()==text)
print("PASS: cut and paste-back")
// Native Ctrl+C follows the same Win32 clipboard path as a context-menu copy,
// without invoking the dedicated Kokoda clipboard shortcut transaction.
func pollClipboard(_ extra:[String:Any] = [:])->[String:Any] {
    var request:[String:Any] = ["command":"clipboard_poll","id":id,"token":token,"request_id":12345]
    request.merge(extra) { _, new in new }
    return exchange(request)
}
precondition(pollClipboard()["text"] is NSNull, "Our paste echoed back")
func nativeCopy() {
    shortcut(0)
    for (vk,down) in [(17,true),(67,true),(67,false),(17,false)] { event(["kind":"key","vk":vk,"down":down]) }
    Thread.sleep(forTimeInterval:0.15)
}
nativeCopy()
let nativeReply = pollClipboard()
precondition(nativeReply["text"] as? String == text, "Native app clipboard change was not detected")
// A poll response must not change the local clipboard while the app is inactive.
setLocal("Inactive local sentinel")
controller.clipboardRequests[12345] = (id,token,controller.clipboardBoard.changeCount)
controller.buffer.append(try! JSONSerialization.data(withJSONObject:nativeReply));controller.buffer.append(10);controller.consume()
precondition(controller.clipboardBoard.string(forType:.string) == "Inactive local sentinel")
precondition(pollClipboard()["text"] is NSNull, "Clipboard change repeated")
nativeCopy()
precondition(pollClipboard(["discard":true])["text"] is NSNull, "Focus baseline imported old content")
precondition(pollClipboard()["text"] is NSNull)
precondition(pollClipboard(["token":token+999])["type"] as? String == "error")
print("PASS: native copy detected once; own paste, focus baseline and stale target filtered")
event(["kind":"key","vk":39,"down":true]);event(["kind":"key","vk":39,"down":false])
setLocal("Unselected copy sentinel");shortcut(8)
precondition(controller.clipboardBoard.string(forType:.string)=="Unselected copy sentinel","Old clipboard leaked on empty selection")
print("PASS: empty selection does not import stale Windows clipboard")
let rejected=exchange(["command":"clipboard","id":id,"token":token+999,"action":"paste","text":"must not paste","request_id":999])
precondition(rejected["type"] as? String == "error" && remoteText()==text)
let requestID:UInt64=9999
controller.clipboardRequests[requestID]=(id,token,controller.clipboardBoard.changeCount)
setLocal("New local copy while waiting")
let reply:[String:Any]=["type":"clipboard_result","request_id":requestID,"action":"copy","text":"older remote result"]
controller.buffer.append(try! JSONSerialization.data(withJSONObject:reply));controller.buffer.append(10);controller.consume()
precondition(controller.clipboardBoard.string(forType:.string)=="New local copy while waiting")
print("PASS: stale target rejected; newer local clipboard never overwritten")
controller.clipboardBoard.clearContents();controller.clipboardBoard.setData(Data([1,2,3]),forType:.png)
shortcut(9)
precondition(controller.inputEvents.isEmpty && remoteText()==text,"Non-text Mac clipboard pasted stale Windows contents")
let oversized=exchange(["command":"clipboard","id":id,"token":token,"action":"paste","text":String(repeating:"x",count:1_048_577),"request_id":10000])
precondition(oversized["type"] as? String == "error" && remoteText()==text)
print("PASS: non-text paste and oversized payload rejected without changing target text")

_ = exchange(["command":"close_window"])
controller.clearSelection();outgoing.fileHandleForWriting.closeFile();process.waitUntilExit();controller.process=nil;controller.input=nil
