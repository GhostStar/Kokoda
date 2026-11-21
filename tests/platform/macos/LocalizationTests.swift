let app=NSApplication.shared
let controller=AppDelegate()
controller.applicationDidFinishLaunching(Notification(name:NSApplication.didFinishLaunchingNotification))
controller.timer?.invalidate()
controller.metrics.stringValue=L("stats.summary",L("stats.rtt_unknown"),2,4,"30 FPS · 12 Mbps","H.264",L("stats.push"),"hardware-ffmpeg-nvenc","3840×2160","3840×2160")+"\n"+String(format:L("stats.target_rate"),30,18.0,20.0,L("stats.adaptation.delivery_backlog"))
controller.window.contentView!.layoutSubtreeIfNeeded()
let content=controller.window.contentView!
let stack=content.subviews.first as! NSStackView
precondition(stack.frame.minY >= 0 && stack.frame.maxY <= content.bounds.height,"Settings overflow the window")
for view in stack.arrangedSubviews {
    let rect=view.convert(view.bounds,to:content)
    precondition(rect.minX >= 0 && rect.maxX <= content.bounds.width,"Localized control overflows the window")
}
precondition(!L("status.connected_counts",2,3).contains("%"))
precondition(!L("status.connected_counts",2,3).contains("status.connected_counts"))
let requestedLanguage=ProcessInfo.processInfo.environment["KOKODA_TEST_LANGUAGE"] ?? Bundle.main.preferredLocalizations.first ?? "en"
let chinese=requestedLanguage.hasPrefix("zh")
precondition(L("menu.quit") == (chinese ? "退出 Kokoda" : "Quit Kokoda"),"Requested native language was not loaded")
precondition(controller.process == nil,"Settings must not connect without an explicit action")
controller.applicationWillTerminate(Notification(name:NSApplication.willTerminateNotification))
controller.window.orderOut(nil)
print("PASS: localized settings fit, format arguments resolve, and launch does not connect")
