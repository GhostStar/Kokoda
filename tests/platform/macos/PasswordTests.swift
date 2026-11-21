let app=NSApplication.shared
let controller=AppDelegate()
controller.applicationDidFinishLaunching(Notification(name:NSApplication.didFinishLaunchingNotification))
controller.timer?.invalidate()
precondition(controller.password.cell is NSSecureTextFieldCell)
precondition(controller.password.stringValue.isEmpty)
precondition(controller.process == nil)
controller.password.stringValue="short"
controller.connectHost()
precondition(controller.process == nil && !controller.wantsConnection)
precondition(controller.status.stringValue == L("error.password_required"))
controller.password.stringValue="        "
controller.connectHost()
precondition(controller.process == nil && !controller.wantsConnection)
precondition(UserDefaults.standard.string(forKey:"connectionPassword") == nil)
controller.password.stringValue=""
controller.applicationWillTerminate(Notification(name:NSApplication.willTerminateNotification))
controller.window.orderOut(nil)
print("PASS: native secure password field, empty launch, invalid password blocked before launch, no plaintext preference")
