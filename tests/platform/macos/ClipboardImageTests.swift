setbuf(stdout,nil)
let app = NSApplication.shared
let controller = AppDelegate()
controller.clipboardBoard = NSPasteboard(name:NSPasteboard.Name("Kokoda.ImageClipboardTest.\(UUID().uuidString)"))
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
let fixture=windows.first {$0["title"] as? String == "Kokoda Image Clipboard Test"}!
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
let png = Data(base64Encoded:"iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP4z8DwHwgbGMA0EAAAP9cIeY8TiooAAAAASUVORK5CYII=")!
func setImage(_ data:Data, _ type:NSPasteboard.PasteboardType = .png) {controller.clipboardBoard.clearContents();controller.clipboardBoard.setData(data,forType:type)}
func received()->[String:Any] {Thread.sleep(forTimeInterval:0.2);return try! JSONSerialization.jsonObject(with:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[4]))) as! [String:Any]}
func checkImage(_ data:Data, alpha:Bool=true) {
 let image=NSBitmapImageRep(data:data)!;precondition(image.pixelsWide==2 && image.pixelsHigh==2)
 precondition(image.bitsPerSample==8 && image.samplesPerPixel==4)
 var pixel=[Int](repeating:0,count:4)
 image.getPixel(&pixel,atX:0,y:0)
 precondition(pixel==[255,0,0,255], "Encoded red pixel changed")
 if alpha {
  image.getPixel(&pixel,atX:1,y:0);precondition(abs(pixel[3]-128)<=1)
  image.getPixel(&pixel,atX:0,y:1);precondition(pixel[3]==0)
 }
}
setImage(png);shortcut(9)
let value=received();precondition(value["width"] as? Int==2 && value["height"] as? Int==2)
let colors=value["pixels"] as! [[Int]]
precondition(colors[0]==[255,0,0,255] && colors[1][3]==128 && colors[2][3]==0)
print("PASS: Mac PNG -> Windows native clipboard paste; colors and alpha preserved")
setLocal("Image copy sentinel");shortcut(8)
checkImage(controller.clipboardBoard.data(forType:.png)!)
precondition(controller.clipboardBoard.data(forType:.tiff) != nil)
print("PASS: Windows PNG copy -> Mac PNG and TIFF pasteboard")
let tiff=NSBitmapImageRep(data:png)!.tiffRepresentation!
setImage(tiff,.tiff);shortcut(9)
precondition((received()["pixels"] as! [[Int]])[1][3]==128)
print("PASS: Mac TIFF screenshot format -> Windows image paste")
func nativeCopy(_ vk:Int)->[String:Any] {
 event(["kind":"key","vk":vk,"down":true]);event(["kind":"key","vk":vk,"down":false])
 Thread.sleep(forTimeInterval:0.2)
 return exchange(["command":"clipboard_poll","id":id,"token":token,"request_id":800])
}
let nativePNG=nativeCopy(120)
checkImage(Data(base64Encoded:nativePNG["image_png"] as! String)!)
let dib=nativeCopy(119)
checkImage(Data(base64Encoded:dib["image_png"] as! String)!,alpha:false)
precondition(exchange(["command":"clipboard_poll","id":id,"token":token])["image_png"] is NSNull)
print("PASS: native PNG and bitmap-only copy detected; no repeat")
let badNative=nativeCopy(118)
precondition(badNative["type"] as? String=="error")
let retry=exchange(["command":"clipboard_poll","id":id,"token":token])
precondition(retry["type"] as? String=="clipboard_result" && retry["image_png"] is NSNull)
checkImage(Data(base64Encoded:nativeCopy(120)["image_png"] as! String)!)
print("PASS: malformed native PNG reported once; next valid copy recovers")
let before=received()
for invalid in ["invalid-base64",Data([1,2,3]).base64EncodedString()] {
 let result=exchange(["command":"clipboard","id":id,"token":token,"action":"paste","image_png":invalid])
 precondition(result["type"] as? String=="error")
}
let tooLarge=String(repeating:"A",count:((8*1024*1024+2)/3)*4+4)
precondition(exchange(["command":"clipboard","id":id,"token":token,"action":"paste","image_png":tooLarge])["type"] as? String=="error")
setImage(Data([1,2,3]));shortcut(9)
precondition(controller.inputEvents.isEmpty)
controller.clipboardBoard.clearContents();controller.clipboardBoard.setString("file:///tmp/test.png",forType:.fileURL);shortcut(9)
precondition(controller.inputEvents.isEmpty)
precondition(NSDictionary(dictionary:before).isEqual(to:received()))
let stale=exchange(["command":"clipboard","id":id,"token":token+999,"action":"paste","image_png":png.base64EncodedString()])
precondition(stale["type"] as? String=="error")
controller.clipboardRequests[900]=(id,token,controller.clipboardBoard.changeCount)
setLocal("New local content")
let reply:[String:Any]=["type":"clipboard_result","request_id":900,"action":"copy","image_png":png.base64EncodedString()]
controller.buffer.append(try! JSONSerialization.data(withJSONObject:reply));controller.buffer.append(10);controller.consume()
precondition(controller.clipboardBoard.string(forType:.string)=="New local content")
print("PASS: malformed image, files, stale target and local overwrite protection")
_ = exchange(["command":"close_window"])
controller.clearSelection();outgoing.fileHandleForWriting.closeFile();process.waitUntilExit();controller.process=nil;controller.input=nil
