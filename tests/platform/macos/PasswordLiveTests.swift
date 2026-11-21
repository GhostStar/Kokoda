// Run only against the isolated "Kokoda Password Fixture" and temporary credentials.
let app = NSApplication.shared
let controller = AppDelegate()
let oldAddress = UserDefaults.standard.object(forKey:"hostAddress")
controller.clipboardBoard = NSPasteboard(name:NSPasteboard.Name("Kokoda.PasswordTest.\(UUID().uuidString)"))
controller.applicationDidFinishLaunching(Notification(name:NSApplication.didFinishLaunchingNotification))
defer {
    controller.disconnectHost()
    controller.timer?.invalidate()
    controller.password.stringValue = ""
    controller.window.orderOut(nil)
    controller.clipboardBoard.releaseGlobally()
    if let oldAddress { UserDefaults.standard.set(oldAddress,forKey:"hostAddress") }
    else { UserDefaults.standard.removeObject(forKey:"hostAddress") }
}
func waitFor(_ label:String,seconds:Double=25,_ condition:()->Bool) throws {
    let deadline=Date().addingTimeInterval(seconds)
    while !condition(),Date()<deadline {RunLoop.current.run(until:Date().addingTimeInterval(0.02))}
    guard condition() else {throw NSError(domain:"PasswordLiveTests",code:1,userInfo:[NSLocalizedDescriptionKey:label])}
}
let secret = try JSONSerialization.jsonObject(with:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[3]))) as! [String:String]
controller.host.stringValue=CommandLine.arguments[2]
controller.password.stringValue=secret["password"]!
controller.connectHost()
try waitFor("Password connection did not authenticate") {controller.authenticated}
precondition(controller.process?.arguments?.contains("--password-stdin") == true)
precondition(controller.process?.arguments?.contains(secret["password"]!) == false)
try waitFor("Isolated fixture unavailable") {controller.windows.contains{$0["title"] as? String == "Kokoda Password Fixture"}}
controller.activateManagerRow(controller.managerRows().first{$0.title == "Kokoda Password Fixture"}!)
try waitFor("Authenticated window did not render") {controller.sessions.values.contains{$0.surfaces.values.contains{$0.view.image != nil}}}
let original=controller.process!
original.terminate()
try waitFor("Reconnect did not authenticate and restore window",seconds:40) {
    controller.process !== original && controller.authenticated && controller.sessions.values.contains{$0.surfaces.values.contains{$0.view.image != nil}}
}
controller.disconnectHost()
controller.password.stringValue="Wrong-temporary-password"
controller.connectHost()
try waitFor("Wrong password did not fail closed") {!controller.wantsConnection && controller.process == nil}
precondition(!controller.authenticated && controller.retryAt == nil)
precondition(controller.status.stringValue == L("error.authentication"))
precondition(!controller.connectionDiagnostics.joined().contains(secret["password"]!))
print("PASS: Mac → Windows password login, native window rendering, automatic reconnect/restoration, wrong-password rejection, no secret in arguments/diagnostics")
