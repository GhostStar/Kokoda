let app=NSApplication.shared
let controller=AppDelegate()
func response(_ object:[String:Any]) {
 controller.buffer.append(try! JSONSerialization.data(withJSONObject:object));controller.buffer.append(10);controller.consume()
}
controller.wantsConnection=true;controller.authenticated=true;controller.resumeToken="opaque-ticket"
controller.inputEvents=[["event":["kind":"clipboard","action":"paste","text":"must not replay"]]]
controller.controlRequests=[["command":"input"]]
controller.clipboardRequests[1]=(1,1,0)
controller.connectionFailed("network",retryable:true)
precondition(controller.retryAt != nil && controller.reconnectAttempt==1)
precondition(controller.inputEvents.isEmpty && controller.controlRequests.isEmpty && controller.clipboardRequests.isEmpty)
precondition(controller.resumeToken=="opaque-ticket")
controller.restoreAfterConnect=true
response(["type":"windows","windows":[["id":1,"title":"Generic app"]]])
precondition(controller.controlRequests.count==1 && controller.controlRequests[0]["command"] as? String=="resume_window")
controller.controlRequests.removeAll()
response(["type":"windows","windows":[["id":1,"title":"Generic app"]]])
precondition(controller.controlRequests.isEmpty,"Refreshing the list must not reopen the stream")
print("PASS: loss discards input and clipboard queues; only window ticket restores once")
response(["type":"error","command":"resume_window","selection_lost":true,"message":"expired"])
precondition(controller.resumeToken==nil && !controller.selected)
controller.wantsConnection=true
response(["type":"bridge_error","category":"authentication","retryable":false])
precondition(controller.retryAt==nil && !controller.wantsConnection)
controller.wantsConnection=true;controller.reconnectAttempt=8
controller.connectionFailed("network",retryable:true)
precondition(controller.retryAt==nil && !controller.wantsConnection)
controller.wantsConnection=true;controller.retryAt=Date();controller.resumeToken="ticket"
controller.disconnectHost()
precondition(controller.retryAt==nil && controller.resumeToken==nil && !controller.wantsConnection)
print("PASS: invalid restore, auth failure, retry ceiling and explicit disconnect stop recovery")
controller.authenticated=true;controller.heartbeatMS=12.5
controller.statsFrames=30;controller.statsBytes=125000;controller.statsStart=Date(timeIntervalSince1970:100)
controller.updateMetrics(now:Date(timeIntervalSince1970:101))
precondition(controller.metrics.stringValue.contains("30.0 FPS") && controller.metrics.stringValue.contains("1.00 Mbps"))
controller.updateMetrics(now:Date(timeIntervalSince1970:102))
precondition(controller.metrics.stringValue.contains("0.0 FPS") && controller.metrics.stringValue.contains("0.00 Mbps"))
controller.installMenus()
let quit=app.mainMenu!.items[0].submenu!.items.first{$0.keyEquivalent=="q"}!
precondition(quit.action == #selector(NSApplication.terminate(_:)) && quit.target === app)
controller.toggleStats();precondition(controller.metrics.isHidden)
controller.toggleReconnect();precondition(!controller.reconnectEnabled)
print("PASS: idle statistics fall to zero; native quit and connection menu actions wired")
// An owned bridge which is blocked on stdin must actually terminate.
let bridge=Process();bridge.executableURL=URL(fileURLWithPath:"/bin/cat")
let input=Pipe();bridge.standardInput=input;bridge.standardOutput=Pipe();try bridge.run()
controller.process=bridge;controller.input=input.fileHandleForWriting
controller.disconnectHost()
let deadline=Date().addingTimeInterval(2)
while bridge.isRunning && Date()<deadline {RunLoop.current.run(until:Date().addingTimeInterval(0.05))}
precondition(!bridge.isRunning)
print("PASS: explicit disconnect terminates owned bridge")

final class MenuProbe:NSObject {var called=false;@objc func invoke(_ sender:Any?) {called=true}}
let probe=MenuProbe();quit.target=probe;quit.action=#selector(MenuProbe.invoke(_:))
let surface=RemoteSurface(id:99,token:1,root:true)
surface.window.makeFirstResponder(surface.view)
var sent=0;surface.view.sendInput={_ in sent += 1}
let commandQ=NSEvent.keyEvent(with:.keyDown,location:.zero,modifierFlags:.command,timestamp:0,windowNumber:surface.window.windowNumber,context:nil,characters:"q",charactersIgnoringModifiers:"q",isARepeat:false,keyCode:12)!
precondition(surface.view.performKeyEquivalent(with:commandQ))
precondition(probe.called && sent==0,"Native menu shortcut leaked to remote keyboard")
surface.window.close()
print("PASS: native Command-Q takes priority over remote shortcut forwarding")

let sleeper=AppDelegate();sleeper.applicationDidFinishLaunching(Notification(name:NSApplication.didFinishLaunchingNotification))
sleeper.wantsConnection=true;sleeper.resumeToken="sleep-ticket"
NSWorkspace.shared.notificationCenter.post(name:NSWorkspace.willSleepNotification,object:nil)
precondition(sleeper.sleeping && sleeper.retryAt==nil && sleeper.resumeToken=="sleep-ticket")
NSWorkspace.shared.notificationCenter.post(name:NSWorkspace.didWakeNotification,object:nil)
precondition(!sleeper.sleeping && sleeper.retryAt != nil)
sleeper.toggleReconnect()
NSWorkspace.shared.notificationCenter.post(name:NSWorkspace.willSleepNotification,object:nil)
NSWorkspace.shared.notificationCenter.post(name:NSWorkspace.didWakeNotification,object:nil)
precondition(sleeper.retryAt==nil)
sleeper.disconnectHost();sleeper.timer?.invalidate();sleeper.applicationWillTerminate(Notification(name:NSApplication.willTerminateNotification));sleeper.window.close()
print("PASS: sleep pauses recovery; wake respects automatic recovery setting")
let geometry=AppDelegate()
let snapshot:[String:Any]=["root_bounds":["left":0,"top":0,"right":800,"bottom":600],"windows":[["id":1,"token":1,"title":"Generic window","bounds":["left":0,"top":0,"right":800,"bottom":600]]]]
geometry.rootID=1;geometry.selected=true;geometry.syncGroup(snapshot)
geometry.surfaces[1]!.window.setFrame(NSRect(x:120,y:180,width:680,height:460),display:false)
let beforeFrame=geometry.surfaces[1]!.window.frame
geometry.wantsConnection=true;geometry.resumeToken="ticket";geometry.connectionFailed("network",retryable:true)
precondition(geometry.resumeWindowFrame==beforeFrame)
geometry.rootID=1;geometry.selected=true;geometry.syncGroup(snapshot)
precondition(geometry.surfaces[1]!.window.frame==beforeFrame)
geometry.disconnectHost()
print("PASS: restored window keeps previous Mac position and size")

let diagnostics=AppDelegate()
diagnostics.pending=true;diagnostics.pendingCommand="window_group"
diagnostics.requestStarted=Date().addingTimeInterval(-35)
diagnostics.lastResponseAt=Date().addingTimeInterval(-31)
diagnostics.errors=Data("Control response timed out".utf8)
diagnostics.connectionFailed("请求超时",retryable:false)
precondition(diagnostics.connectionDiagnostics.last!.contains("window_group"))
precondition(diagnostics.connectionDiagnostics.last!.contains("Control response timed out"))
precondition(diagnostics.errors.isEmpty && diagnostics.connectionDiagnostics.count==1,"Disconnect erased its cause")
for _ in 0..<12 { diagnostics.recordConnectionFailure("temporary network error") }
precondition(diagnostics.connectionDiagnostics.count==8,"Diagnostic history must remain bounded")
diagnostics.disconnect()
precondition(diagnostics.connectionDiagnostics.count==8,"Reconnect cleanup erased diagnostics")
print("PASS: disconnect reason and pending command survive cleanup; history remains bounded")
