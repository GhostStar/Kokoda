import AppKit

final class HostDiscovery:NSObject {
    weak var app:AppDelegate?
    let choices=NSPopUpButton()
    let refreshButton=NSButton(title:L("discovery.refresh"),target:nil,action:nil)
    let hint=NSTextField(wrappingLabelWithString:L("discovery.hint"))
    private var active=false
    private var attempted=false
    private var lastPoll=Date.distantPast
    private var signature=""
    var disabledForTests=ProcessInfo.processInfo.environment["KOKODA_DISABLE_DISCOVERY"]=="1"
    init(app:AppDelegate) {
        self.app=app;super.init()
        choices.addItem(withTitle:L("discovery.searching"));choices.target=self;choices.action=#selector(selectHost)
        choices.setAccessibilityLabel(L("discovery.hosts"));choices.setContentCompressionResistancePriority(.defaultLow,for:.horizontal)
        refreshButton.target=self;refreshButton.action=#selector(refresh);refreshButton.bezelStyle = .rounded
        hint.font = .systemFont(ofSize:11);hint.textColor = .secondaryLabelColor
    }
    func stop() {
        if active {app?.workflow.dispatch(["type":"discovery_stop"])}
        active=false;attempted=true
        // Preserve discovered endpoints and selection while connecting. A stopped
        // browser must not look active or restart because authentication failed.
        if choices.numberOfItems==1 && choices.itemTitle(at:0)==L("discovery.searching") {
            choices.removeAllItems();choices.addItem(withTitle:L("discovery.choose"))
        }
    }
    @objc func refresh() {
        guard let app=app,!app.authenticated,app.process==nil,!app.wantsConnection else{return}
        stop();attempted=false;signature="";lastPoll = .distantPast
        choices.removeAllItems();choices.addItem(withTitle:L("discovery.searching"));tick()
    }
    func tick() {
        guard let app=app else{return}
        let busy=app.authenticated || app.process != nil || app.wantsConnection
        choices.isEnabled = !busy;refreshButton.isEnabled = !busy
        if busy {stop();return}
        guard !disabledForTests,Date().timeIntervalSince(lastPoll)>1 else{return};lastPoll=Date()
        if !active {
            guard !attempted else{return};attempted=true
            let response=app.workflow.dispatch(["type":"discovery_start"])
            if response["unavailable"] as? Bool==true {hint.stringValue=L("discovery.unavailable");return}
            active=true;hint.stringValue=L("discovery.hint")
        }
        apply(app.workflow.dispatch(["type":"discovery_poll"])["hosts"] as? [[String:Any]] ?? [])
    }
    func apply(_ hosts:[[String:Any]]) {
        var entries:[(String,String,String)]=[]
        for host in hosts {
            guard let id=host["id"] as? String,let name=host["name"] as? String,let addresses=host["addresses"] as? [String] else{continue}
            for address in addresses {entries.append((id+"|"+address,"\(name) — \(address)",address))}
        }
        entries.sort{$0.1.localizedStandardCompare($1.1) == .orderedAscending}
        let key=entries.map{$0.0+":"+$0.1}.joined(separator:"\n")
        guard signature != key || choices.numberOfItems==1 else{return};signature=key
        let selected=choices.selectedItem?.representedObject as? [String:String]
        choices.removeAllItems();choices.addItem(withTitle:entries.isEmpty ? L("discovery.none"):L("discovery.choose"))
        for (id,title,address) in entries {
            let item=NSMenuItem(title:title,action:nil,keyEquivalent:"");item.representedObject=["id":id,"address":address]
            choices.menu?.addItem(item)
            if selected?["id"]==id {choices.select(item)}
        }
    }
    @objc func selectHost() {
        guard let app=app,!app.authenticated,app.process==nil,!app.wantsConnection,
              let item=choices.selectedItem?.representedObject as? [String:String],let address=item["address"] else{return}
        if app.host.stringValue != address {app.password.stringValue=""}
        app.host.stringValue=address
        app.window.makeFirstResponder(app.password)
        // Selecting an untrusted discovery hint never initiates authentication.
    }
}
