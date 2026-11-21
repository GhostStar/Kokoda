let app=NSApplication.shared
let c=AppDelegate();c.applicationDidFinishLaunching(Notification(name:NSApplication.didFinishLaunchingNotification));c.timer?.invalidate()
let d=c.manager!.discovery!
precondition(d.disabledForTests)
c.host.stringValue="192.0.2.20:47831";c.password.stringValue="Synthetic-password"
d.apply([["id":"first","name":"Same name","addresses":["192.0.2.10:47832"]],["id":"second","name":"Same name","addresses":["192.0.2.11:47832"]]])
precondition(d.choices.numberOfItems==3)
let selected=d.choices.itemArray.first{($0.representedObject as? [String:String])?["address"]=="192.0.2.11:47832"}!
d.choices.select(selected);d.selectHost()
precondition(c.host.stringValue=="192.0.2.11:47832" && c.password.stringValue.isEmpty && c.process==nil && !c.wantsConnection)
c.password.stringValue="Synthetic-password"
d.apply([["id":"second","name":"Renamed","addresses":["192.0.2.11:47832"]]])
precondition((d.choices.selectedItem?.representedObject as? [String:String])?["address"]=="192.0.2.11:47832")
c.wantsConnection=true;c.host.stringValue="192.0.2.20:47831";d.selectHost();precondition(c.host.stringValue=="192.0.2.20:47831")
c.wantsConnection=false;d.apply([]);precondition(d.choices.numberOfItems==1)
d.selectHost();precondition(c.host.stringValue=="192.0.2.20:47831")
c.manager!.options.isHidden=false;c.window.contentView!.layoutSubtreeIfNeeded()
let content=c.window.contentView!;let stack=content.subviews.first as! NSStackView
for view in stack.arrangedSubviews where !view.isHidden {
 let rect=view.convert(view.bounds,to:content)
 precondition(rect.minY>=0 && rect.maxY<=content.bounds.height+1,"Disconnected discovery settings overflow")
}
c.applicationWillTerminate(Notification(name:NSApplication.willTerminateNotification));c.window.orderOut(nil)
print("PASS: discovery hints preserve identity, never auto-connect, clear credentials on host changes and keep manual entry usable")
