import AppKit

final class ManagerWindow:NSWindow {
    var find:(()->Void)?
    override func performKeyEquivalent(with event:NSEvent)->Bool {
        if event.modifierFlags.intersection(.deviceIndependentFlagsMask) == .command,event.keyCode==40 {find?();return true}
        return super.performKeyEquivalent(with:event)
    }
}
final class ManagerTable: NSTableView {
    var activate:(()->Void)?
    var find:(()->Void)?
    override func keyDown(with event:NSEvent) {
        if event.keyCode==36 || event.keyCode==76 {activate?();return}
        super.keyDown(with:event)
    }
    override func performKeyEquivalent(with event:NSEvent)->Bool {
        if event.modifierFlags.intersection(.deviceIndependentFlagsMask) == .command,event.keyCode==40 {find?();return true}
        return super.performKeyEquivalent(with:event)
    }
}
final class ManagerButton:NSButton {var rowKey=""}

final class ManagerController:NSObject,NSTableViewDataSource,NSTableViewDelegate,NSSearchFieldDelegate {
    weak var app:AppDelegate?
    let search=NSSearchField()
    let tabs=NSSegmentedControl(labels:[L("manager.available"),L("manager.viewing")],trackingMode:.selectOne,target:nil,action:nil)
    let table=ManagerTable()
    let scroll=NSScrollView()
    let empty=NSTextField(wrappingLabelWithString:"")
    let connection=NSButton(title:L("connection.connect"),target:nil,action:nil)
    let summary=NSTextField(labelWithString:"")
    let refreshButton=NSButton(title:L("windows.refresh"),target:nil,action:nil)
    let primary=NSButton(title:L("windows.open"),target:nil,action:nil)
    let hideButton=NSButton(title:L("manager.hide"),target:nil,action:nil)
    let endButton=NSButton(title:L("menu.end_viewing"),target:nil,action:nil)
    let settingsButton=NSButton(title:L("manager.options"),target:nil,action:nil)
    let form=NSStackView()
    let options=NSStackView()
    let reconnect=NSButton(checkboxWithTitle:L("menu.auto_reconnect"),target:nil,action:nil)
    var rows:[ManagerRow]=[]
    var expanded=Set<String>()
    private var lastRows:[ManagerRow]=[]
    private var allRows:[ManagerRow]=[]
    private var lastRefresh=Date.distantPast
    private var lastReadOnly=false
    private var built=false
    var selected:ManagerRow? {rows.indices.contains(table.selectedRow) ? rows[table.selectedRow]:nil}
    init(app:AppDelegate) {self.app=app;super.init()}

    func build() {
        guard let app=app,!built else{return};built=true
        let root=app.window.contentView!
        let stack=NSStackView();stack.orientation = .vertical;stack.alignment = .leading;stack.spacing=12
        stack.translatesAutoresizingMaskIntoConstraints=false;root.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo:root.leadingAnchor,constant:20),
            stack.trailingAnchor.constraint(equalTo:root.trailingAnchor,constant:-20),
            stack.topAnchor.constraint(equalTo:root.topAnchor,constant:20),
            stack.bottomAnchor.constraint(equalTo:root.bottomAnchor,constant:-16)])
        func add(_ view:NSView) {stack.addArrangedSubview(view);view.widthAnchor.constraint(equalTo:stack.widthAnchor).isActive=true}
        func button(_ b:NSButton,_ action:Selector) {b.target=self;b.action=action;b.bezelStyle = .rounded}
        button(connection,#selector(connectionAction));button(settingsButton,#selector(toggleOptions))
        summary.font = .systemFont(ofSize:15,weight:.semibold);summary.lineBreakMode = .byTruncatingMiddle
        summary.setContentCompressionResistancePriority(.defaultLow,for:.horizontal)
        let spacer=NSView();spacer.setContentHuggingPriority(.defaultLow,for:.horizontal)
        add(NSStackView(views:[summary,spacer,connection,settingsButton]))
        form.orientation = .vertical;form.alignment = .leading;form.spacing=8
        app.host.placeholderString=L("connection.address");app.host.setAccessibilityLabel(L("connection.address"))
        app.password.placeholderString=L("connection.password");app.password.setAccessibilityLabel(L("connection.password"))
        app.password.target=self;app.password.action=#selector(connectionAction)
        for field in [app.host,app.password] {form.addArrangedSubview(field);field.widthAnchor.constraint(equalTo:form.widthAnchor).isActive=true}
        add(form)
        app.quality.addItems(withTitles:[L("quality.original"),L("quality.smooth"),L("quality.clear"),L("quality.fast")])
        app.quality.selectItem(at:app.globalPreset);app.quality.target=app;app.quality.action=#selector(AppDelegate.qualityChanged)
        app.quality.toolTip=L("quality.tip");app.quality.setAccessibilityLabel(L("menu.quality"))
        let qualityRow=NSStackView(views:[NSTextField(labelWithString:L("menu.quality")),app.quality])
        qualityRow.alignment = .centerY;add(qualityRow)
        options.orientation = .vertical;options.alignment = .leading;options.spacing=6
        app.adaptiveResolution.state = .on;app.adaptiveResolution.toolTip=L("display.adaptive_tip")
        app.syncRemoteClipboard.state = .on;app.syncRemoteClipboard.toolTip=L("clipboard.sync_tip")
        app.includeApplication.toolTip=L("manager.scope_tip")
        for v in [app.adaptiveResolution,app.syncRemoteClipboard,app.includeApplication] {options.addArrangedSubview(v)}
        reconnect.target=self;reconnect.action=#selector(toggleReconnect(_:))
        reconnect.state=app.reconnectEnabled ? .on:.off;options.addArrangedSubview(reconnect)
        let notice=NSTextField(wrappingLabelWithString:L("manager.scope_tip"));notice.font = .systemFont(ofSize:11);notice.textColor = .secondaryLabelColor
        options.addArrangedSubview(notice);notice.widthAnchor.constraint(equalTo:options.widthAnchor).isActive=true
        let diagnostics=NSButton(title:L("menu.diagnostics"),target:app,action:#selector(AppDelegate.showConnectionDiagnostics));diagnostics.bezelStyle = .rounded
        options.addArrangedSubview(diagnostics);add(options);options.isHidden=true
        tabs.selectedSegment=0;tabs.target=self;tabs.action=#selector(changeTab)
        search.placeholderString=L("manager.search");search.setAccessibilityLabel(L("manager.search"));search.delegate=self
        search.sendsSearchStringImmediately=true
        search.widthAnchor.constraint(greaterThanOrEqualToConstant:160).isActive=true
        button(refreshButton,#selector(refreshCatalog))
        let browse=NSStackView(views:[tabs,search,refreshButton]);browse.distribution = .fill;add(browse)
        for (name,title,width) in [("window",L("manager.window"),340.0),("state",L("manager.state"),120.0),("action",L("manager.action"),120.0)] {
            let c=NSTableColumn(identifier:NSUserInterfaceItemIdentifier(name));c.title=title;c.width=width
            c.minWidth=name=="window" ? 180:100
            c.resizingMask=name=="window" ? .autoresizingMask:[];table.addTableColumn(c)
        }
        table.columnAutoresizingStyle = .lastColumnOnlyAutoresizingStyle
        table.rowHeight=48;table.usesAlternatingRowBackgroundColors=true;table.allowsMultipleSelection=false
        table.dataSource=self;table.delegate=self;table.target=self;table.doubleAction=#selector(activateSelected)
        table.setAccessibilityLabel(L("manager.window_list"))
        table.activate={[weak self] in self?.activateSelected()};table.find={[weak self] in self?.focusSearch()}
        scroll.documentView=table;scroll.hasVerticalScroller=true;scroll.borderType = .bezelBorder
        add(scroll);scroll.heightAnchor.constraint(greaterThanOrEqualToConstant:160).isActive=true
        scroll.setContentHuggingPriority(.defaultLow,for:.vertical)
        empty.textColor = .secondaryLabelColor;empty.maximumNumberOfLines=2;add(empty)
        button(primary,#selector(activateSelected));button(hideButton,#selector(hideSelected));button(endButton,#selector(endSelected))
        let actions=NSStackView(views:[primary,hideButton,endButton]);add(actions)
        app.status.maximumNumberOfLines=2;app.status.lineBreakMode = .byWordWrapping;add(app.status)
        app.metrics.maximumNumberOfLines=3;app.metrics.lineBreakMode = .byWordWrapping;app.metrics.textColor = .secondaryLabelColor
        app.metrics.font = .systemFont(ofSize:11);app.metrics.toolTip=L("stats.tip");add(app.metrics)
        app.window.find={[weak self] in self?.focusSearch()}
        app.window.minSize=NSSize(width:760,height:680)
        app.window.setContentSize(NSSize(width:820,height:720))
        refresh(force:true)
    }
    func focusSearch() {app?.window.makeKeyAndOrderFront(nil);app?.window.makeFirstResponder(search)}
    @objc func connectionAction() {
        guard let app=app else{return}
        if app.authenticated || app.process != nil || app.wantsConnection {app.disconnectHost()}
        else {app.connectHost()}
        refresh(force:true)
    }
    @objc func toggleOptions() {options.isHidden.toggle()}
    @objc func toggleReconnect(_ sender:NSButton) {
        guard let app=app else{return}
        if (sender.state == .on) != app.reconnectEnabled {app.toggleReconnect()}
    }
    @objc func refreshCatalog() {guard app?.authenticated==true else{return};app?.refreshWindows()}
    @objc func changeTab() {refresh(force:true)}
    func controlTextDidChange(_ obj:Notification) {refresh(force:true)}
    func control(_ control:NSControl,textView:NSTextView,doCommandBy command:Selector)->Bool {
        if textView.hasMarkedText() {return false}
        if command == #selector(NSResponder.insertNewline(_:)) {
            if selected==nil,!rows.isEmpty {table.selectRowIndexes(IndexSet(integer:0),byExtendingSelection:false)}
            activateSelected();return true
        }
        if command == #selector(NSResponder.moveDown(_:)),!rows.isEmpty {table.selectRowIndexes(IndexSet(integer:0),byExtendingSelection:false);app?.window.makeFirstResponder(table);return true}
        return false
    }
    @objc func activateSelected() {if let row=selected {app?.activateManagerRow(row)}}
    @objc func rowAction(_ button:ManagerButton) {
        if let row=rows.first(where:{$0.key==button.rowKey}) {app?.activateManagerRow(row)}
    }
    @objc func expandRow(_ button:ManagerButton) {
        if expanded.contains(button.rowKey) {expanded.remove(button.rowKey)} else {expanded.insert(button.rowKey)}
        refresh(force:true)
    }
    @objc func hideSelected() {
        guard let row=selected,let app=app,let current=app.managerRows().first(where:{$0.key==row.key}),
              let sid=current.sessionID,let s=app.sessions[sid]?.surfaces[current.id],!s.locallyClosed else{return}
        app.withSession(sid){app.dismissViewer(s)};refresh(force:true)
    }
    @objc func endSelected() {
        guard let row=selected,let app=app,let current=app.managerRows().first(where:{$0.key==row.key}),let sid=current.sessionID else{return}
        app.endManagerSession(sid);refresh(force:true)
    }
    func refresh(force:Bool=false) {
        guard built,let app=app else{return}
        if !force,Date().timeIntervalSince(lastRefresh)<0.2 {return};lastRefresh=Date()
        let busy=app.process != nil || app.wantsConnection
        form.isHidden=app.authenticated;app.host.isEnabled = !busy;app.password.isEnabled = !busy
        summary.stringValue=app.authenticated ? app.host.stringValue:(busy ? L("manager.connecting"):L("status.not_connected"))
        summary.toolTip=app.authenticated ? app.host.stringValue:nil
        connection.title=app.authenticated ? L("connection.disconnect"):(busy ? (app.retryAt != nil ? L("manager.stop_reconnect"):L("manager.cancel")):L("connection.connect"))
        refreshButton.isEnabled=app.authenticated
        reconnect.state=app.reconnectEnabled ? .on:.off
        let all=app.managerRows(query:search.stringValue)
        allRows=all
        expanded.formIntersection(Set(all.map{$0.key}))
        let independent=all.filter{isRoot($0,in:all)}
        tabs.setLabel(L("manager.available_count",independent.count),forSegment:0)
        tabs.setLabel(L("manager.viewing_count",independent.filter{$0.viewing}.count),forSegment:1)
        let candidates=all.filter{tabs.selectedSegment==0 || $0.viewing}
        var displayed:[ManagerRow]=[]
        func visit(_ row:ManagerRow,_ seen:Set<String>) {
            guard !seen.contains(row.key) else{return}
            var seen=seen;seen.insert(row.key)
            if row.rank>=0 {displayed.append(row)}
            if expanded.contains(row.key) || !search.stringValue.isEmpty {
                for child in candidates where child.parent==row.id && child.sessionID==row.sessionID && child.role != "independent" {visit(child,seen)}
            }
        }
        for row in candidates where isRoot(row,in:candidates) {visit(row,[])}
        if !search.stringValue.isEmpty {
            // Matching children remain discoverable even if their owner does not match.
            for row in candidates where row.rank>=0 && !displayed.contains(where:{$0.key==row.key}) {displayed.append(row)}
        }
        if displayed != lastRows || lastReadOnly != app.readOnly {
            let selection=selected?.key
            let position=scroll.contentView.bounds.origin
            rows=displayed;lastRows=displayed;lastReadOnly=app.readOnly;table.reloadData()
            if let key=selection,let index=rows.firstIndex(where:{$0.key==key}) {table.selectRowIndexes(IndexSet(integer:index),byExtendingSelection:false)}
            else {table.deselectAll(nil)}
            scroll.contentView.scroll(to:position);scroll.reflectScrolledClipView(scroll.contentView)
        }
        empty.isHidden = !rows.isEmpty
        empty.stringValue = !app.authenticated ? L("manager.connect_hint") : (!search.stringValue.isEmpty ? L("manager.no_matches"):(tabs.selectedSegment==1 ? L("manager.no_viewers"):L("manager.no_windows")))
        updateActions()
    }
    func isRoot(_ row:ManagerRow,in all:[ManagerRow])->Bool {
        row.role=="independent" || row.parent==nil || !all.contains{$0.id==row.parent && $0.sessionID==row.sessionID}
    }
    func actionTitle(_ action:String)->String {
        switch action {
        case "open":return L("windows.open")
        case "focus":return L("manager.switch")
        case "restore":return L("manager.restore")
        case "modal":return L("manager.show_dialog")
        default:return L("manager.unavailable")
        }
    }
    func stateTitle(_ state:String)->String {
        switch state {
        case "opening":return L("manager.opening")
        case "ending":return L("manager.ending")
        case "blocked":return L("manager.blocked")
        case "hidden":return L("manager.hidden")
        case "minimized":return L("manager.minimized")
        case "viewing":return L("manager.viewing")
        default:return app?.readOnly==true ? L("manager.read_only"):L("manager.available")
        }
    }
    func updateActions() {
        let row=selected
        primary.title=actionTitle(row?.action ?? "open");primary.isEnabled=row != nil && row?.action != "none"
        hideButton.isEnabled=row?.token != nil && row?.state != "hidden"
        endButton.isEnabled=row?.sessionID != nil
    }
    func tableViewSelectionDidChange(_ notification:Notification) {updateActions()}
    func numberOfRows(in tableView:NSTableView)->Int {rows.count}
    func tableView(_ tableView:NSTableView,viewFor column:NSTableColumn?,row index:Int)->NSView? {
        guard rows.indices.contains(index) else{return nil}
        let row=rows[index]
        switch column?.identifier.rawValue {
        case "action":
            let b=ManagerButton(title:actionTitle(row.action),target:self,action:#selector(rowAction(_:)))
            b.rowKey=row.key;b.bezelStyle = .rounded;b.controlSize = .small;b.isEnabled=row.action != "none"
            b.setAccessibilityLabel("\(b.title): \(row.title)");return b
        case "state":
            let label=stateTitle(row.state)+(app?.readOnly==true && row.viewing ? " · "+L("manager.read_only"):"")
            let t=NSTextField(labelWithString:label);t.textColor = .secondaryLabelColor;t.font = .systemFont(ofSize:12)
            t.lineBreakMode = .byTruncatingTail;t.toolTip=label;t.translatesAutoresizingMaskIntoConstraints=false
            let cell=NSView();cell.addSubview(t)
            NSLayoutConstraint.activate([t.centerYAnchor.constraint(equalTo:cell.centerYAnchor),t.leadingAnchor.constraint(equalTo:cell.leadingAnchor,constant:6),t.trailingAnchor.constraint(equalTo:cell.trailingAnchor,constant:-4)])
            return cell
        default:
            let box=NSStackView();box.spacing=8;box.alignment = .centerY
            let all=allRows
            let children=all.contains{$0.parent==row.id && $0.sessionID==row.sessionID && $0.role != "independent"}
            if children {
                let b=ManagerButton(title:expanded.contains(row.key) ? "▾":"▸",target:self,action:#selector(expandRow(_:)))
                b.isBordered=false;b.rowKey=row.key;b.widthAnchor.constraint(equalToConstant:20).isActive=true
                b.setAccessibilityLabel(L("manager.related"));box.addArrangedSubview(b)
            } else {
                let icon=NSImageView(image:NSImage(systemSymbolName:isRoot(row,in:all) ? "macwindow":"arrow.turn.down.right",accessibilityDescription:nil) ?? NSImage())
                icon.contentTintColor = .secondaryLabelColor;icon.widthAnchor.constraint(equalToConstant:20).isActive=true;box.addArrangedSubview(icon)
            }
            let title=NSTextField(labelWithString:row.title);title.lineBreakMode = .byTruncatingTail;title.toolTip=row.title
            title.font = .systemFont(ofSize:13,weight:row.viewing ? .medium:.regular)
            title.setContentCompressionResistancePriority(.defaultLow,for:.horizontal)
            box.addArrangedSubview(title);return box
        }
    }
}
