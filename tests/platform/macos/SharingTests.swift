let app = NSApplication.shared
let controller = AppDelegate()
controller.clipboardBoard = NSPasteboard(name: NSPasteboard.Name("Kokoda.SharingTest.\(UUID().uuidString)"))
defer {controller.clipboardBoard.releaseGlobally()}
controller.handleResponse(["windows":[],"read_only":true],type:"windows")
precondition(controller.readOnly && !controller.syncRemoteClipboard.isEnabled)
controller.rootID=1;controller.selected=true
controller.syncGroup(["root_bounds":["left":0,"top":0,"right":400,"bottom":200],"windows":[["id":1,"token":1,"title":"Synthetic shared window","bounds":["left":0,"top":0,"right":400,"bottom":200]]]])
let surface=controller.surfaces[1]!
precondition(surface.window.title==L("sharing.viewer_title","Synthetic shared window"))
controller.clipboardBoard.setString("Unchanged local test clipboard",forType:.string)
let before=controller.clipboardBoard.changeCount
for event:[String:Any] in [["kind":"text","text":"must not arrive"],["kind":"clipboard","action":"paste"],["kind":"move","x":0.2,"y":0.2]] {
 controller.enqueue(event,id:1,token:1)
}
precondition(controller.inputEvents.isEmpty && controller.clipboardRequests.isEmpty)
precondition(controller.clipboardBoard.changeCount==before && !controller.pollClipboardIfNeeded())
surface.view.setMarkedText("pinyin",selectedRange:NSRange(location:0,length:0),replacementRange:NSRange(location:NSNotFound,length:0))
precondition(!surface.view.hasMarkedText())
controller.clearSelection()
controller.handleResponse(["windows":[],"read_only":false],type:"windows")
precondition(!controller.readOnly && controller.syncRemoteClipboard.isEnabled)
print("PASS: read-only viewers label access, block input/IME and leave the local clipboard untouched")
