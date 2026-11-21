let app=NSApplication.shared
let controller=AppDelegate()
let oldAddress=UserDefaults.standard.object(forKey:"hostAddress")
controller.clipboardBoard=NSPasteboard(name:NSPasteboard.Name("Kokoda.PopupTest.\(UUID().uuidString)"))
controller.applicationDidFinishLaunching(Notification(name:NSApplication.didFinishLaunchingNotification))
defer {
    controller.disconnectHost();controller.timer?.invalidate();controller.window.orderOut(nil)
    controller.password.stringValue="";controller.clipboardBoard.releaseGlobally()
    if let oldAddress {UserDefaults.standard.set(oldAddress,forKey:"hostAddress")}
    else {UserDefaults.standard.removeObject(forKey:"hostAddress")}
}
func waitFor(_ message:String,_ condition:()->Bool) throws {
    let deadline=Date().addingTimeInterval(30)
    while !condition(),Date()<deadline {RunLoop.current.run(until:Date().addingTimeInterval(0.02))}
    guard condition() else {throw NSError(domain:"PopupLiveTests",code:1,userInfo:[NSLocalizedDescriptionKey:message+"; status="+controller.status.stringValue+"; errors="+String(decoding:controller.errors.suffix(1024),as:UTF8.self)])}
}
let secret=try JSONSerialization.jsonObject(with:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[3]))) as! [String:String]
controller.host.stringValue=CommandLine.arguments[2];controller.password.stringValue=secret["password"]!
controller.connectHost()
try waitFor("Host authentication/capability") {controller.authenticated && controller.supportsWindowVideoAlpha}
try waitFor("Synthetic fixture discovery") {controller.choices.itemArray.contains{$0.title=="Kokoda Popup Video Root"}}
controller.choices.select(controller.choices.itemArray.first{$0.title=="Kokoda Popup Video Root"}!)
controller.openWindow()
try waitFor("All native viewers rendered") {controller.sessions.values.contains{$0.surfaces.count==4 && $0.surfaces.values.allSatisfy{$0.view.image != nil}}}
let state=controller.sessions.values.first{$0.surfaces.count==4}!
try waitFor("Per-window quality feedback and Retina budget negotiation") {controller.supportsResolutionBitrate && state.surfaces.values.allSatisfy{!$0.rateText.isEmpty}}
let popup=state.surfaces.values.first{$0.window.title=="Kokoda Popup Video Opaque"}!
try waitFor("Independent popup Retina viewport") {popup.lastViewport != nil}
let before=popup.sequence
let origin=popup.window.frame.origin
var ticks=0
let tracking=Timer(timeInterval:0.033,repeats:true) {_ in
    ticks += 1
    popup.window.setFrameOrigin(NSPoint(x:origin.x+Double(ticks%30)*2,y:origin.y))
}
CFRunLoopAddCommonMode(CFRunLoopGetMain(),CFRunLoopMode(rawValue:RunLoop.Mode.eventTracking.rawValue as CFString))
RunLoop.main.add(tracking,forMode:.eventTracking)
let deadline=Date().addingTimeInterval(2)
while Date()<deadline {RunLoop.current.run(mode:.eventTracking,before:Date().addingTimeInterval(0.04))}
tracking.invalidate()
precondition(ticks>=20,"Native tracking timer stalled: ticks=\(ticks), frames=\(popup.sequence-before)")
precondition(popup.sequence>before+5,"Video stalled in native window tracking mode")
let dragged=popup.window.frame
RunLoop.current.run(until:Date().addingTimeInterval(0.4))
precondition(abs(popup.window.frame.minX-dragged.minX)<2,"Snapshot snapped local drag back")
precondition(controller.authenticated && controller.connectionDiagnostics.isEmpty,"Tracking caused a reconnect")
print("PASS: native password client, four viewers, popup viewport, \(ticks) native tracking ticks, \(popup.sequence-before) video frames during tracking/recovery, stable local drag and no reconnect")

let oldGeneration=controller.generation
controller.process?.terminate()
try waitFor("New authenticated connection restores viewers") {
    controller.generation>oldGeneration && controller.authenticated && controller.supportsPlaybackFeedback && controller.sessions.values.contains{$0.surfaces.count==4 && $0.surfaces.values.allSatisfy{$0.view.image != nil}}
}
print("PASS: automatic reconnect restores consumption-credit negotiation and all four native viewers")
