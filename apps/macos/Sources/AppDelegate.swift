import AppKit
import VideoToolbox
import CoreImage
import ImageIO

final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var sessions: [UInt64:ApplicationSession] = [0:ApplicationSession(id:0)]
    var contextSessionID: UInt64 = 0
    var nextSessionID: UInt64 = 0
    var scheduleCursor = 0
    var lastActiveSessionID: UInt64?
    let workflow = SharedController()
    var globalPreset: Int { workflow.preset }
    var supportsPipeline = false
    var supportsPlaybackFeedback = false
    var asynchronousRequests: [Date] = []
    var playbackFeedback: [[String:Any]] = []
    var supportsPositionedWheel = false
    var supportsAnchoredPointer = false
    var supportsWindowVideoAlpha = false
    var supportsLiveSettings = false
    var supportsPushVideo = false
    var supportsMultiSession = false
    var supportsWindowManagement = false
    var supportsResolutionBitrate = false
    var applyingWindowState = false
    var readOnly = false
    var connectionRequests: [[String:Any]] = []
    var pendingSessionID: UInt64 = 0
    var session: ApplicationSession {sessions[contextSessionID] ?? sessions[0]!}
    @discardableResult func withSession<T>(_ id:UInt64,_ body:()->T)->T {
        let old=contextSessionID;contextSessionID=id;defer{contextSessionID=old};return body()
    }
    func sessionID(for window:NSWindow?) -> UInt64? {
        sessions.first { $0.value.surfaces.values.contains { $0.window === window } }?.key
    }
    var focusSessionID: UInt64? {sessions.first{$0.value.surfaces.values.contains{$0.window.isKeyWindow}}?.key}
    func rememberFrames() {
        for state in sessions.values where state.resumeToken != nil {
            if let root=state.rootID,let surface=state.surfaces[root] {state.resumeWindowFrame=surface.window.frame}
            withSession(state.id){for s in surfaces.values {rememberViewer(s)}}
        }
    }
    func forgetRecovery() {for state in sessions.values {state.resumeToken=nil;state.resumeWindowFrame=nil;state.requestedRoot=nil;state.viewerPreferences.removeAll()}}
    let window = ManagerWindow(contentRect:NSRect(x:200,y:300,width:640,height:620),
                          styleMask:[.titled,.closable,.miniaturizable,.resizable],backing:.buffered,defer:false)
    let host = NSTextField(string:UserDefaults.standard.string(forKey:"hostAddress") ?? "127.0.0.1:47831")
    let password = NSSecureTextField(string: "")
    let status = NSTextField(labelWithString:L("status.enter_password"))
    var manager: ManagerController?
    var managerCatalogIDs: [UInt64:(serial:UInt64, identity:String)] = [:]
    var managerCatalogSerial: UInt64 = 0
    let quality = NSPopUpButton()
    let adaptiveResolution = NSButton(checkboxWithTitle:L("display.adaptive"),target:nil,action:nil)
    var lastViewport: NSSize? { get { session.lastViewport } set { session.lastViewport=newValue } }
    var resizedAt: Date { get { session.resizedAt } set { session.resizedAt=newValue } }
    var encodedSizeText: String { get { session.encodedSizeText } set { session.encodedSizeText=newValue } }
    var videoMode: Bool { get { session.videoMode } set { session.videoMode=newValue } }
    var statsStart: Date { get { session.statsStart } set { session.statsStart=newValue } }
    var statsFrames: Int { get { session.statsFrames } set { session.statsFrames=newValue } }
    var statsBytes: Int { get { session.statsBytes } set { session.statsBytes=newValue } }
    var statsText: String { get { session.statsText } set { session.statsText=newValue } }
    let includeApplication = NSButton(checkboxWithTitle:L("windows.include_app"),target:nil,action:nil)
    var process: Process?
    var input: FileHandle?
    var windows: [[String:Any]] = []
    var surfaces: [UInt64:RemoteSurface] { get { session.surfaces } set { session.surfaces=newValue } }
    var rootID: UInt64? { get { session.rootID } set { session.rootID=newValue } }
    var rootBounds: NSRect { get { session.rootBounds } set { session.rootBounds=newValue } }
    var order: [UInt64] { get { session.order } set { session.order=newValue } }
    var decodingCount = 0
    var decodingBytes = 0
    var decodingFrame: Bool {decodingCount > 0}
    var incomingAcks: [()->Void] = []
    func acknowledgeIncoming() {let callbacks=incomingAcks;incomingAcks.removeAll();for callback in callbacks {callback()}}
    var buffer = Data()
    var parsedResponses: [[String:Any]] = []
    var decodeSlots = DispatchSemaphore(value:2)
    var errors = Data()
    var generation = 0
    var pending = false
    var selected: Bool { get { session.selected } set { session.selected=newValue } }
    var timer: Timer?
    var inputEvents: [[String:Any]] { get { session.inputEvents } set { session.inputEvents=newValue } }
    var clipboardBoard = NSPasteboard.general
    var clipboardRequests: [UInt64:(id:UInt64,token:UInt64,changeCount:Int)] { get { session.clipboardRequests } set { session.clipboardRequests=newValue } }
    var nextClipboardRequest: UInt64 = 0
    var lastClipboardPoll: Date { get { session.lastClipboardPoll } set { session.lastClipboardPoll=newValue } }
    var clipboardPollNeedsBaseline: Bool { get { session.clipboardPollNeedsBaseline } set { session.clipboardPollNeedsBaseline=newValue } }
    let syncRemoteClipboard = NSButton(checkboxWithTitle:L("clipboard.sync"),target:nil,action:nil)
    var controlRequests: [[String:Any]] { get { session.controlRequests } set { session.controlRequests=newValue } }
    var lastGroup: Date { get { session.lastGroup } set { session.lastGroup=newValue } }
    var frameCursor: Int { get { session.frameCursor } set { session.frameCursor=newValue } }
    var remoteForeground: UInt64? { get { session.remoteForeground } set { session.remoteForeground=newValue } }
    var removingWindows = false
    var layoutInProgress: Bool { get { session.layoutInProgress } set { session.layoutInProgress=newValue } }
    let metrics = NSTextField(labelWithString:L("status.not_connected"))
    var wantsConnection = false
    var authenticated = false
    var reconnectEnabled = true
    var reconnectAttempt = 0
    var retryAt: Date?
    var sleeping = false
    var quitting = false
    var resumeToken: String? { get { session.resumeToken } set { session.resumeToken=newValue } }
    var restoreAfterConnect = false
    var resumeWindowFrame: NSRect? { get { session.resumeWindowFrame } set { session.resumeWindowFrame=newValue } }
    var requestStarted = Date.distantPast
    var lastHeartbeat = Date.distantPast
    var heartbeatMS: Double?
    var sourceSizeText: String { get { session.sourceSizeText } set { session.sourceSizeText=newValue } }
    var pendingCommand = ""
    var lastResponseAt = Date.distantPast
    var connectionDiagnostics: [String] = []
    var wakeObservers: [NSObjectProtocol] = []
    var statsMenu: NSMenuItem?
    var reconnectMenu: NSMenuItem?


    func applicationDidFinishLaunching(_ notification: Notification) {
        window.title = L("window.title")
        window.isReleasedWhenClosed = false
        includeApplication.state = .on
        installMenus()
        wakeObservers.append(NSWorkspace.shared.notificationCenter.addObserver(forName:NSWorkspace.willSleepNotification,object:nil,queue:.main) { [weak self] _ in
            guard let self=self else{return};self.sleeping=true;self.retryAt=nil;self.disconnect()
            self.status.stringValue=L("status.sleep")
        })
        wakeObservers.append(NSWorkspace.shared.notificationCenter.addObserver(forName:NSWorkspace.didWakeNotification,object:nil,queue:.main) { [weak self] _ in
            guard let self=self else{return};self.sleeping=false
            if self.wantsConnection && self.reconnectEnabled {self.reconnectAttempt=0;self.retryAt=Date().addingTimeInterval(1);self.status.stringValue=L("status.wake")}
        })
        manager=ManagerController(app:self)
        manager?.build()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps:true)
        let pollTimer = Timer(timeInterval:0.008,repeats:true) { [weak self] _ in self?.poll() }
        RunLoop.main.add(pollTimer,forMode:.common); timer = pollTimer
    }
    func installMenus() {
        let bar=NSMenu();NSApp.mainMenu=bar
        func menu(_ title:String)->NSMenu {let item=NSMenuItem();item.title=title;let menu=NSMenu(title:title);item.submenu=menu;bar.addItem(item);return menu}
        func add(_ menu:NSMenu,_ title:String,_ action:Selector,_ key:String="",_ target:AnyObject?=nil)->NSMenuItem {
            let item=NSMenuItem(title:title,action:action,keyEquivalent:key);item.target=target ?? self;menu.addItem(item);return item
        }
        let app=menu("Kokoda")
        _=add(app,L("menu.about"),#selector(NSApplication.orderFrontStandardAboutPanel(_:)),"",NSApp)
        _=add(app,L("menu.settings"),#selector(showSettings),",")
        app.addItem(.separator())
        _=add(app,L("menu.hide"),#selector(NSApplication.hide(_:)),"h",NSApp)
        _=add(app,L("menu.quit"),#selector(NSApplication.terminate(_:)),"q",NSApp)
        let connection=menu(L("menu.connection"))
        _=add(connection,L("connection.connect"),#selector(connectHost))
        _=add(connection,L("menu.reconnect"),#selector(reconnectHost))
        _=add(connection,L("menu.disconnect"),#selector(disconnectHost))
        _=add(connection,L("menu.refresh"),#selector(refreshWindows),"r")
        _=add(connection,L("menu.diagnostics"),#selector(showConnectionDiagnostics))
        reconnectMenu=add(connection,L("menu.auto_reconnect"),#selector(toggleReconnect));reconnectMenu?.state = .on
        let view=menu(L("menu.display"))
        statsMenu=add(view,L("menu.stats"),#selector(toggleStats));statsMenu?.state = .on
        let presets=NSMenu(title:L("menu.quality"))
        let qualityItem=NSMenuItem(title:L("menu.quality"),action:nil,keyEquivalent:"");qualityItem.submenu=presets;view.addItem(qualityItem)
        for (index,title) in [L("quality.original_short"),L("quality.smooth_short"),L("quality.clear_short"),L("quality.fast_short")].enumerated() {
            let item=add(presets,title,#selector(changeQuality(_:)));item.tag=index
        }
        let windowsMenu=menu(L("menu.window"));NSApp.windowsMenu=windowsMenu
        _=add(windowsMenu,L("menu.settings_short"),#selector(showSettings))
        _=add(windowsMenu,L("manager.find"),#selector(findManagerWindow))
        _=add(windowsMenu,L("menu.restore_viewers"),#selector(restoreClosedViewers))
        let behavior=NSMenu(title:L("menu.resize_behavior"))
        let behaviorItem=NSMenuItem(title:L("menu.resize_behavior"),action:nil,keyEquivalent:"");behaviorItem.submenu=behavior;windowsMenu.addItem(behaviorItem)
        add(behavior,L("menu.resize_local"),#selector(changeResizeMode(_:))).tag=ViewerResizeMode.scale.rawValue
        add(behavior,L("menu.resize_host"),#selector(changeResizeMode(_:))).tag=ViewerResizeMode.hostWindow.rawValue
        let zoom=NSMenu(title:L("menu.viewer_zoom"))
        let zoomItem=NSMenuItem(title:L("menu.viewer_zoom"),action:nil,keyEquivalent:"");zoomItem.submenu=zoom;windowsMenu.addItem(zoomItem)
        for percent in [50,75,100,125,150,200] {add(zoom,"\(percent)%",#selector(changeViewerZoom(_:))).tag=percent}
        windowsMenu.addItem(.separator())
        _=add(windowsMenu,L("menu.end_viewing"),#selector(endViewingApplication))
        add(windowsMenu,L("menu.minimize"),#selector(NSWindow.performMiniaturize(_:)),"m",nil).target=nil
        add(windowsMenu,L("menu.close_window"),#selector(NSWindow.performClose(_:)),"w",nil).target=nil
    }
    @objc func findManagerWindow() {showSettings();manager?.focusSearch()}
    @objc func showSettings() { window.makeKeyAndOrderFront(nil);NSApp.activate(ignoringOtherApps:true) }
    func applicationShouldHandleReopen(_ sender:NSApplication,hasVisibleWindows flag:Bool)->Bool {showSettings();return true}
    @objc func toggleStats() {metrics.isHidden.toggle();statsMenu?.state=metrics.isHidden ? .off : .on}
    @objc func toggleReconnect() {
        reconnectEnabled.toggle();reconnectMenu?.state=reconnectEnabled ? .on : .off
        if !reconnectEnabled {retryAt=nil;if process==nil {wantsConnection=false;status.stringValue=L("status.reconnect_off")}}
    }
    @objc func qualityChanged() { applyQuality(quality.indexOfSelectedItem) }
    @objc func changeQuality(_ item:NSMenuItem) {applyQuality(item.tag)}
    func applyQuality(_ preset:Int) {
        guard (0...3).contains(preset) else{return}
        quality.selectItem(at:preset)
        let effects = workflow.quality(preset)
        for state in sessions.values { state.needsQualityUpdate = true }
        for effect in effects {
            guard effect["type"] as? String == "quality",let id=(effect["id"] as? NSNumber)?.uint64Value,
                  let generation=(effect["generation"] as? NSNumber)?.uint64Value else {continue}
            for state in sessions.values where state.rootID == id && state.surfaces[id]?.token == generation {
                queueGlobalQuality(for: state)
            }
        }
        if sessions.values.contains(where: { $0.selected }) {
            status.stringValue = supportsLiveSettings ? L("quality.updating") : L("status.upgrade_quality")
        }
        poll()
    }
    func queueGlobalQuality(for state: ApplicationSession) {
        guard supportsLiveSettings, state.needsQualityUpdate, state.selected,
              let root = state.rootID, let surface = state.surfaces[root] else { return }
        state.preset = globalPreset
        var settings = selectionRequest(id: root, state: state)
        settings["command"] = "video_settings"
        settings["token"] = surface.token
        settings.removeValue(forKey: "scope")
        state.controlRequests.removeAll { $0["command"] as? String == "video_settings" }
        state.controlRequests.append(settings)
        state.needsQualityUpdate = false
    }
    func selectionRequest(id:UInt64,state:ApplicationSession)->[String:Any] {
        var selection:[String:Any]=["command":"select_window","id":id,"scope":state.applicationScope ? "application" : "window"]
        selection["alpha_video"]=supportsWindowVideoAlpha
        if supportsPlaybackFeedback {selection["playback_feedback"]=true}
        if supportsWindowManagement {selection["window_management"]=true}
        if supportsResolutionBitrate {selection["resolution_bitrate"]=true}
        state.videoMode=state.preset>0
        for (key,value) in workflow.profile(state.preset) {selection[key]=value}
        return selection
    }
    @objc func disconnectHost() {forgetRecovery();wantsConnection=false;retryAt=nil;reconnectAttempt=0;disconnect();status.stringValue=L("status.disconnected")}
    @objc func reconnectHost() {wantsConnection=true;retryAt=nil;reconnectAttempt=0;startConnection()}
    func recordConnectionFailure(_ message:String, now:Date=Date()) {
        let stamp=ISO8601DateFormatter().string(from:now)
        let waiting=pending ? String(format:L("diagnostics.seconds"),max(0,now.timeIntervalSince(requestStarted))) : L("diagnostics.none")
        let received=lastResponseAt == .distantPast ? L("diagnostics.no_response") : String(format:L("diagnostics.ago"),max(0,now.timeIntervalSince(lastResponseAt)))
        let stderr=String(decoding:errors.suffix(2048),as:UTF8.self).trimmingCharacters(in:.whitespacesAndNewlines)
        let detail=stderr.isEmpty ? "" : L("diagnostics.bridge",String(stderr.prefix(512)))
        connectionDiagnostics.append(L("diagnostics.entry",stamp,String(message.prefix(512)),pending ? pendingCommand : L("diagnostics.none"),waiting,received,detail))
        connectionDiagnostics=Array(connectionDiagnostics.suffix(8))
    }
    @objc func showConnectionDiagnostics() {
        let text=connectionDiagnostics.isEmpty ? L("diagnostics.empty") : connectionDiagnostics.reversed().joined(separator:"\n\n")
        let alert=NSAlert();alert.messageText=L("diagnostics.title")
        alert.informativeText=L("diagnostics.description")
        let scroll=NSScrollView(frame:NSRect(x:0,y:0,width:620,height:300));scroll.hasVerticalScroller=true
        let view=NSTextView(frame:scroll.bounds);view.isEditable=false;view.isSelectable=true
        view.autoresizingMask=[.width];view.isVerticallyResizable=true;view.textContainer?.widthTracksTextView=true;view.string=text;scroll.documentView=view;alert.accessoryView=scroll
        alert.addButton(withTitle:L("dialog.close"));alert.addButton(withTitle:L("diagnostics.copy"))
        alert.beginSheetModal(for:NSApp.keyWindow ?? window) { result in
            if result == .alertSecondButtonReturn {NSPasteboard.general.clearContents();NSPasteboard.general.setString(text,forType:.string)}
        }
    }
    func connectionFailed(_ message:String, retryable:Bool) {
        recordConnectionFailure(message)
        rememberFrames()
        disconnect()
        guard wantsConnection, reconnectEnabled, retryable, !sleeping, reconnectAttempt<8 else {
            retryAt=nil;wantsConnection=false;status.stringValue=message;return
        }
        let delay=min(30.0,pow(2.0,Double(reconnectAttempt)))
        reconnectAttempt += 1;retryAt=Date().addingTimeInterval(delay)
        status.stringValue=L("status.retry",message,Int(delay),reconnectAttempt)
    }
    func updateMetrics(now:Date=Date()) {
        var changed=false
        for state in sessions.values {
            let elapsed=now.timeIntervalSince(state.statsStart)
            if elapsed>=1 {
                state.statsText=String(format:L("stats.rate"),Double(state.statsFrames)/elapsed,Double(state.statsBytes)*8/elapsed/1_000_000)
                state.statsFrames=0;state.statsBytes=0;state.statsStart=now;changed=true
            }
        }
        guard changed else{return}
        let state=(focusSessionID ?? lastActiveSessionID).flatMap{sessions[$0]} ?? sessions.sorted{$0.key<$1.key}.first(where:{$0.value.selected})?.value ?? session
        let latency=heartbeatMS.map{String(format:L("stats.rtt"),$0)} ?? L("stats.rtt_unknown")
        metrics.stringValue=authenticated ? L("stats.summary",latency,sessions.values.filter{$0.selected}.count,sessions.values.reduce(0){$0+$1.surfaces.values.filter{!$0.locallyClosed}.count},state.statsText,state.videoMode ? "H.264" : "JPEG / PNG",supportsPushVideo ? L("stats.push") : L("stats.pull"),state.encoderText,state.sourceSizeText.isEmpty ? "—" : state.sourceSizeText,state.encodedSizeText.isEmpty ? "—" : state.encodedSizeText) : L("stats.disconnected")
        if authenticated,state.videoMode,let id=state.metricWindowID ?? state.rootID,let surface=state.surfaces[id],!surface.rateText.isEmpty {metrics.stringValue += "\n"+surface.rateText}
    }
    func removeSurface(_ id: UInt64) {
        guard let surface = surfaces.removeValue(forKey:id) else { return }
        session.viewerPreferences.removeValue(forKey:id)
        removingWindows = true
        surface.resetInput()
        if id == rootID {
            lastViewport = nil
            for child in surface.window.childWindows ?? [] { surface.window.removeChildWindow(child) }
        }
        surface.window.parent?.removeChildWindow(surface.window)
        surface.window.close()
        removingWindows = false
        inputEvents.removeAll { ($0["id"] as? UInt64) == id }
        if controlRequests.contains(where:{($0["id"] as? NSNumber)?.uint64Value==id && $0["command"] as? String=="video_settings"}) {session.needsQualityUpdate=true}
        controlRequests.removeAll { ($0["id"] as? NSNumber)?.uint64Value == id }
        playbackFeedback.removeAll { ($0["session_id"] as? UInt64)==contextSessionID && ($0["id"] as? UInt64)==id }
    }
    func clearSelection() {
        session.remoteDragging=false
        session.viewerPreferences.removeAll()
        workflow.forget(session:contextSessionID)
        selected = false
        clipboardRequests.removeAll()
        lastClipboardPoll = .distantPast
        clipboardPollNeedsBaseline = true
        lastViewport = nil
        for id in Array(surfaces.keys).filter({$0 != rootID}) { removeSurface(id) }
        for id in Array(surfaces.keys) { removeSurface(id) }
        rootID = nil; order.removeAll(); inputEvents.removeAll()
    }
    func disconnect() {
        rememberFrames()
        workflow.disconnect()
        supportsPushVideo=false
        generation += 1
        decodingCount=0;decodingBytes=0;decodeSlots=DispatchSemaphore(value:2);acknowledgeIncoming()
        supportsPipeline=false;supportsPlaybackFeedback=false;supportsResolutionBitrate=false;asynchronousRequests.removeAll();playbackFeedback.removeAll()
        input?.closeFile(); input = nil
        if let child=process { stopBridge(child) }; process = nil
        windows.removeAll();managerCatalogIDs.removeAll()
        manager?.rows.removeAll()
        readOnly=false;syncRemoteClipboard.isEnabled=true
        supportsPositionedWheel=false;supportsAnchoredPointer=false;supportsWindowManagement=false
        supportsWindowVideoAlpha=false
        authenticated=false;heartbeatMS=nil;lastHeartbeat = .distantPast
        pending = false
        for id in Array(sessions.keys) {withSession(id) {
            statsFrames=0;statsBytes=0;statsStart=Date();sourceSizeText="";encodedSizeText=""
            let preferences=session.resumeToken != nil ? session.viewerPreferences:[:]
            clearSelection();controlRequests.removeAll();session.viewerPreferences=preferences
        }}
        sessions=sessions.filter{$0.key==0 || $0.value.resumeToken != nil}
        connectionRequests.removeAll();buffer.removeAll();parsedResponses.removeAll();errors.removeAll();lastResponseAt = .distantPast
        manager?.refresh(force:true)
    }
    func stopBridge(_ child:Process) {
        if child.isRunning { child.terminate() }
        DispatchQueue.main.asyncAfter(deadline:.now()+1) {
            if child.isRunning { kill(child.processIdentifier,SIGKILL) }
        }
    }
    @objc func connectHost() {
        forgetRecovery();wantsConnection=true;retryAt=nil;reconnectAttempt=0
        startConnection()
    }
    func startConnection() {
        guard wantsConnection,!sleeping else{return}
        guard (8...128).contains(password.stringValue.unicodeScalars.count), !password.stringValue.unicodeScalars.contains(where: {CharacterSet.controlCharacters.contains($0)}), !password.stringValue.trimmingCharacters(in:.whitespacesAndNewlines).isEmpty else {
            connectionFailed(L("error.password_required"),retryable:false);return
        }
        rememberFrames()
        restoreAfterConnect = sessions.values.contains{$0.resumeToken != nil}
        UserDefaults.standard.set(host.stringValue,forKey:"hostAddress")
        UserDefaults.standard.removeObject(forKey:"pairingDirectory")
        disconnect()
        let current = generation
        let process = Process()
        process.executableURL = Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/kokoda-bridge")
        process.arguments = [host.stringValue,"--password-stdin","--stream-bridge"]
        let incoming = Pipe(), outgoing = Pipe(), errorPipe = Pipe()
        process.standardInput = outgoing; process.standardOutput = incoming; process.standardError = errorPipe
        let deliverySlots=DispatchSemaphore(value:2)
        let parser=BridgePacketParser()
        let parserQueue=DispatchQueue(label:"Kokoda.bridge.parse",qos:.userInitiated,autoreleaseFrequency:.workItem)
        incoming.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data=handle.availableData
            if data.isEmpty {handle.readabilityHandler=nil;return}
            let receivedAt=ProcessInfo.processInfo.systemUptime
            deliverySlots.wait()
            parserQueue.async {
                let result=Result {try parser.append(data,receivedAt:receivedAt)}
                RunLoop.main.perform(inModes:[.common]) {
                    guard let self=self,self.generation==current else {deliverySlots.signal();return}
                    self.incomingAcks.append{deliverySlots.signal()}
                    switch result {
                    case .success(let messages):self.parsedResponses.append(contentsOf:messages);self.consume()
                    case .failure:self.connectionFailed(L("error.invalid_response"),retryable:false)
                    }
                }
            }
        }
        errorPipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            if data.isEmpty { handle.readabilityHandler = nil; return }
            DispatchQueue.main.async {
                guard let self = self, self.generation == current else { return }
                self.errors.append(data)
                if self.errors.count > 8192 {self.errors=Data(self.errors.suffix(8192))}
            }
        }
        process.terminationHandler = { [weak self] terminated in
            DispatchQueue.main.asyncAfter(deadline:.now()+0.1) {
                incoming.fileHandleForReading.readabilityHandler = nil
                errorPipe.fileHandleForReading.readabilityHandler = nil
                guard let self = self, self.generation == current else { return }
                self.connectionFailed(L("error.bridge_exit",terminated.terminationStatus),retryable:self.authenticated)
            }
        }
        do {
            try process.run(); self.process = process; input = outgoing.fileHandleForWriting
            var credential = try JSONSerialization.data(withJSONObject:["password":password.stringValue]);credential.append(10)
            defer {credential.resetBytes(in:0..<credential.count)}
            try outgoing.fileHandleForWriting.write(contentsOf:credential)
            status.stringValue = L("status.connecting"); request(["command":"list_windows"])
        } catch { connectionFailed(L("error.bridge_start",error.localizedDescription),retryable:false) }
    }
    func request(_ object: [String:Any], asynchronous:Bool=false) {
        guard let process = process, process.isRunning, (asynchronous ? asynchronousRequests.count<16 : !pending), let input = input else { return }
        do {
            var packet=object
            let command=object["command"] as? String ?? ""
            if command=="video_feedback",let received=packet.removeValue(forKey:"_client_received_at") as? Double {
                let decode=packet["decode_ms"] as? Double ?? 0
                packet["queue_ms"]=min(60000,max(0,(ProcessInfo.processInfo.systemUptime-received)*1000-decode))
            }
            if !["ping","list_windows","start_stream"].contains(command),packet["session_id"] == nil {packet["session_id"]=contextSessionID}
            if command == "start_stream" {packet["session_id"]=0}
            if asynchronous {packet["bridge_async"]=true}
            else {pendingSessionID=(packet["session_id"] as? NSNumber)?.uint64Value ?? 0}
            var data = try JSONSerialization.data(withJSONObject:packet); data.append(10)
            if asynchronous {asynchronousRequests.append(Date())}
            else {pending = true;requestStarted=Date();pendingCommand=command}
            try input.write(contentsOf:data)
        } catch { connectionFailed(L("error.write",error.localizedDescription),retryable:true) }
    }
    @objc func refreshWindows() {
        guard authenticated else{return}
        connectionRequests.removeAll { $0["command"] as? String == "list_windows" }
        connectionRequests.append(["command":"list_windows"]); poll()
    }
    @objc func openWindow() {
        manager?.activateSelected()
    }
    func enqueue(_ incoming: [String:Any], id: UInt64, token: UInt64) {
        guard !readOnly else {status.stringValue=L("sharing.read_only");return}
        guard selected, surfaces[id]?.token == token else { return }
        if (surfaces[id]?.locallyClosed==true || (supportsWindowManagement && surfaces[id]?.presentation?.enabled==false)),incoming["kind"] as? String != "release" {return}
        if incoming["kind"] as? String == "button",incoming["button"] as? Int == 0 {session.remoteDragging=incoming["down"] as? Bool == true}
        if incoming["kind"] as? String == "drag_begin" {session.remoteDragging=true}
        if ["release","drag_end"].contains(incoming["kind"] as? String ?? "") {session.remoteDragging=false}
        var event = incoming
        if event["kind"] as? String == "clipboard" {
            if event["action"] as? String == "paste" {
                if clipboardBoard.types?.contains(.fileURL) == true {
                    status.stringValue = L("clipboard.files_unsupported"); return
                }
                if let data = clipboardBoard.data(forType:.png) ?? clipboardBoard.data(forType:.tiff) {
                    guard let png = ClipboardImage.normalizedPNG(data) else {
                        status.stringValue = L("clipboard.image_limit"); return
                    }
                    event["image_png"] = png.base64EncodedString()
                } else if let text = clipboardBoard.string(forType:.string) {
                    guard text.utf8.count <= 1_048_576, !text.contains("\0") else {
                        status.stringValue = L("clipboard.text_limit"); return
                    }
                    event["text"] = text
                } else {
                    status.stringValue = L("clipboard.empty"); return
                }
            }
            nextClipboardRequest += 1
            event["request_id"] = nextClipboardRequest
            clipboardRequests[nextClipboardRequest] = (id,token,clipboardBoard.changeCount)
        }
        if inputEvents.count >= 1024 { disconnect(); status.stringValue = L("error.input_backlog"); return }
        let item: [String:Any] = ["id":id,"token":token,"event":event]
        if ["move","drag_move"].contains(event["kind"] as? String ?? ""), let last = inputEvents.last,
           last["id"] as? UInt64 == id, let previous=last["event"] as? [String:Any],
           previous["kind"] as? String == event["kind"] as? String,
           previous["gesture"] as? UInt64 == event["gesture"] as? UInt64 {
            inputEvents[inputEvents.count-1] = item
        } else { inputEvents.append(item) }
        poll()
    }
    func surface(for window: NSWindow?) -> RemoteSurface? { surfaces.values.first { $0.window === window } }
    func windowDidBecomeKey(_ notification:Notification) {
        guard let target=notification.object as? NSWindow, let sid=sessionID(for:target) else{return}
        lastActiveSessionID=sid
        withSession(sid) {
            if let surface=surface(for:target) {
                if session.metricWindowID != surface.id {session.metricWindowID=surface.id;statsFrames=0;statsBytes=0;statsStart=Date()}
                surface.view.restoreTextInputFocus()
                if !applyingWindowState && !removingWindows {queueWindowAction(surface,"activate")}
            }
        }
    }
    func applicationDidBecomeActive(_ notification:Notification) {
        guard let target=NSApp.keyWindow, let sid=sessionID(for:target) else{return}
        withSession(sid) {
            if let surface=surface(for:target) {
                if session.metricWindowID != surface.id {session.metricWindowID=surface.id;statsFrames=0;statsBytes=0;statsStart=Date()}
                surface.view.restoreTextInputFocus()
            }
        }
    }
    func windowDidResignKey(_ notification: Notification) {
        guard let sid=sessionID(for:notification.object as? NSWindow) else{return}
        withSession(sid) { resignKey(notification) }
    }
    func resignKey(_ notification:Notification) {
        guard !removingWindows, let surface = surface(for:notification.object as? NSWindow) else { return }
        surface.resetInput()
        enqueue(["kind":"release"],id:surface.id,token:surface.token)
    }
    func windowWillClose(_ notification: Notification) {
        guard let sid=sessionID(for:notification.object as? NSWindow) else{return}
        withSession(sid) { closeSessionWindow(notification) }
    }
    func closeSessionWindow(_ notification:Notification) {
        guard !removingWindows,let surface=surface(for:notification.object as? NSWindow) else{return}
        dismissViewer(surface)
    }

    func windowDidMove(_ notification: Notification) {
        guard let sid=sessionID(for:notification.object as? NSWindow) else{return}
        withSession(sid) {
            guard !layoutInProgress,!applyingWindowState,let moved=surface(for:notification.object as? NSWindow) else{return}
            if supportsWindowManagement {
                if moved.resizeGesture==nil,moved.role != "independent",let expected=managedExpectedRect(moved) {
                    let actual=moved.window.contentRect(forFrameRect:moved.window.frame)
                    moved.localOffset=NSPoint(x:actual.minX-expected.minX,y:actual.maxY-expected.maxY)
                }
                return
            }
            if moved.id != rootID, let rootID=rootID, let root=surfaces[rootID], root.view.image != nil {
                let rootRect=root.window.convertToScreen(root.view.convert(root.view.imageRect,to:nil))
                let expected=RemoteWindowLayout.childRect(rootScreen:rootRect,rootBounds:rootBounds,childBounds:moved.bounds)
                let actual=moved.window.contentRect(forFrameRect:moved.window.frame)
                moved.localOffset=NSPoint(x:actual.minX-expected.minX,y:actual.minY-expected.minY)
            }
            layoutChildren()
        }
    }
    func windowDidResize(_ notification: Notification) {
        guard let sid=sessionID(for:notification.object as? NSWindow) else{return}
        withSession(sid) { resizeSession(notification) }
    }
    func resizeSession(_ notification:Notification) {
        if supportsWindowManagement {
            guard !applyingWindowState,!layoutInProgress,let s=surface(for:notification.object as? NSWindow) else{return}
            resizedAt=Date()
            if s.resizeMode == .scale || readOnly {
                if !s.localGeometryAction {fitLocalCanvas(s)}
                updateLocalScale(s);rememberViewer(s);layoutChildren()
            }
            else if s.resizeGesture != nil {updateLiveResize(s)}
            else {updateLocalScale(s);rememberViewer(s)}
            return
        }
        if surface(for:notification.object as? NSWindow)?.id == rootID { resizedAt = Date() }
        layoutChildren()
    }
    func windowDidChangeBackingProperties(_ notification: Notification) {if let sid=sessionID(for:notification.object as? NSWindow) {withSession(sid) {resizedAt=Date();lastViewport=nil}}}
    func syncViewport() -> Bool {
        guard videoMode, selected, Date().timeIntervalSince(resizedAt) >= 0.25,
              let rootID = rootID else { return false }
        let root = surfaces[rootID].map{[$0]} ?? []
        let candidates = supportsWindowVideoAlpha ? root + surfaces.values.filter{$0.id != rootID}.sorted{$0.id<$1.id} : root
        for target in candidates where !target.locallyClosed && target.view.image != nil && !target.geometryHeld && target.presentation?.minimized != true {
            let backing = target.view.convertToBacking(target.view.imageRect)
            guard backing.width > 0, backing.height > 0 else {continue}
            let size = (contextSessionID == 0 ? adaptiveResolution.state == .on : session.adaptive)
                ? NSSize(width:min(8192,max(1,backing.width.rounded())),height:min(8192,max(1,backing.height.rounded()))) : .zero
            let previous = target.id == rootID ? lastViewport : target.lastViewport
            if previous == size {continue}
            target.lastViewport=size
            if target.id == rootID {lastViewport=size}
            request(["command":"video_viewport","id":target.id,"token":target.token,"width":Int(size.width),"height":Int(size.height)])
            return true
        }
        return false
    }

    func pollClipboardIfNeeded() -> Bool {
        if !readOnly, selected, syncRemoteClipboard.state == .on, NSApp.isActive,
           (clipboardPollNeedsBaseline || Date().timeIntervalSince(lastClipboardPoll) >= 0.5),
           let surface = surfaces.values.first(where: { $0.window.isKeyWindow }) {
            lastClipboardPoll = Date()
            nextClipboardRequest += 1
            clipboardRequests[nextClipboardRequest] = (surface.id,surface.token,clipboardBoard.changeCount)
            request(["command":"clipboard_poll","id":surface.id,"token":surface.token,"request_id":nextClipboardRequest,"discard":clipboardPollNeedsBaseline])
            clipboardPollNeedsBaseline = false
            return true
        }
        return false
    }
    func poll() {
        updateMetrics()
        manager?.refresh()
        if let next=retryAt,Date()>=next,!sleeping {retryAt=nil;startConnection();return}
        if pending,Date().timeIntervalSince(requestStarted)>35 {
            connectionFailed(L("error.timeout"),retryable:authenticated);return
        }
        guard process != nil else{return}
        if let started=asynchronousRequests.first,Date().timeIntervalSince(started)>35 {connectionFailed(L("error.timeout"),retryable:authenticated);return}
        if supportsPipeline && authenticated {
            // Input can pass read-only maintenance requests, never a clipboard or
            // selection barrier. Keep the same global order for focus and keys.
            let inputAllowed = !pending || ["ping","window_group","video_viewport","video_settings","list_windows"].contains(pendingCommand)
            if inputAllowed && asynchronousRequests.count<12 {
                let ids=sessions.keys.sorted()
                for sid in ids {
                    withSession(sid) {
                        if selected && !inputEvents.isEmpty && (inputEvents.first?["event"] as? [String:Any])?["kind"] as? String != "clipboard" {
                            sendInputBatch(asynchronous:true)
                        }
                    }
                }
            }
            while asynchronousRequests.count<16 && !playbackFeedback.isEmpty {
                let packet=playbackFeedback.removeFirst()
                request(packet,asynchronous:true)
            }
        }
        guard !pending else{return}
        if !connectionRequests.isEmpty {request(connectionRequests.removeFirst());return}
        if authenticated,Date().timeIntervalSince(lastHeartbeat)>=1 {lastHeartbeat=Date();request(["command":"ping"]);return}
        let ids=sessions.keys.sorted()
        guard !ids.isEmpty else{return}
        for offset in 0..<ids.count {
            let index=(scheduleCursor+offset)%ids.count
            withSession(ids[index]) {pollSession()}
            if pending {scheduleCursor=(index+1)%ids.count;return}
        }
    }
    func sendInputBatch(asynchronous:Bool) {
        guard !inputEvents.isEmpty, !asynchronous || asynchronousRequests.count<12 else{return}
        let first=inputEvents.removeFirst(),id=first["id"] as! UInt64,token=first["token"] as! UInt64
        var batch=[first["event"]!]
        while batch.count<256, let next=inputEvents.first,next["id"] as? UInt64==id,next["token"] as? UInt64==token,
              (next["event"] as? [String:Any])?["kind"] as? String != "clipboard" {batch.append(inputEvents.removeFirst()["event"]!)}
        request(["command":"input","id":id,"token":token,"events":batch],asynchronous:asynchronous)
    }
    func pollSession() {
        if !NSApp.isActive || !surfaces.values.contains(where: { $0.window.isKeyWindow }) || syncRemoteClipboard.state != .on {
            clipboardPollNeedsBaseline = true
        }
        if supportsWindowManagement {for s in surfaces.values {sendResizeSample(s,final:false)}}
        if !controlRequests.isEmpty { request(controlRequests.removeFirst()); return }
        if selected && Date().timeIntervalSince(lastGroup) >= (session.remoteDragging ? 0.033 : (supportsPushVideo ? 0.2 : (videoMode ? 0.05 : 0.3))) {
            lastGroup = Date(); request(["command":"window_group"]); return
        }
        if clipboardPollNeedsBaseline && pollClipboardIfNeeded() { return }
        let frameDue = surfaces.values.allSatisfy { Date().timeIntervalSince($0.lastFrame) >= 0.15 }
        if !inputEvents.isEmpty && (supportsPushVideo || !frameDue) {
            let first = inputEvents.removeFirst()
            let id = first["id"] as! UInt64, token = first["token"] as! UInt64
            if var event = first["event"] as? [String:Any], event["kind"] as? String == "clipboard" {
                event.removeValue(forKey:"kind"); event["command"] = "clipboard"; event["id"] = id; event["token"] = token
                request(event); return
            }
            inputEvents.insert(first,at:0)
            sendInputBatch(asynchronous:supportsPipeline);return
        }
        if pollClipboardIfNeeded() { return }
        if syncViewport() { return }
        let available = order.filter { surfaces[$0] != nil }
        if !supportsPushVideo && selected && !available.isEmpty {
            let key = surfaces.values.first { $0.window.isKeyWindow }?.id
            var schedule = available
            if let key = key, available.count > 1 { schedule.append(key) }
            let id = schedule[frameCursor % schedule.count]; frameCursor += 1
            let surface = surfaces[id]!
            surface.lastFrame = Date()
            request(["command":"frame","id":id,"token":surface.token,"after":surface.sequence]); return
        }
        if supportsPushVideo {return}
        // Release events may remain after a transient window hide.
        if !inputEvents.isEmpty { inputEvents.removeAll() }
    }
    func rectangle(_ value: Any?) -> NSRect {
        guard let value = value as? [String:Any], let left = value["left"] as? Double,
              let top = value["top"] as? Double, let right = value["right"] as? Double,
              let bottom = value["bottom"] as? Double else { return .zero }
        return NSRect(x:left,y:top,width:right-left,height:bottom-top)
    }
    func layoutChildren() {
        if supportsWindowManagement {layoutManagedWindows();return}
        guard !layoutInProgress, let rootID = rootID, let root = surfaces[rootID],
              rootBounds.width > 0, rootBounds.height > 0, root.view.image != nil else { return }
        layoutInProgress = true
        defer { layoutInProgress = false }
        let rect = root.window.convertToScreen(root.view.convert(root.view.imageRect,to:nil))
        for surface in surfaces.values where surface.id != rootID {
            let content = RemoteWindowLayout.childRect(rootScreen:rect,rootBounds:rootBounds,childBounds:surface.bounds)
                .offsetBy(dx:surface.localOffset.x,dy:surface.localOffset.y)
            if content.width > 0 && content.height > 0 {
                let frame=surface.window.frameRect(forContentRect:content)
                if surface.window.frame != frame { surface.window.setFrame(frame,display:true) }
            }
        }
    }
    func syncGroup(_ response: [String:Any]) {
        let previous=applyingWindowState;applyingWindowState=true
        defer{applyingWindowState=previous}
        if supportsWindowManagement,let anchor=(response["root_id"] as? NSNumber)?.uint64Value {if rootID != anchor {lastViewport=nil};rootID=anchor;session.requestedRoot=anchor}
        guard let root = rootID else { return }
        rootBounds = rectangle(response["root_bounds"])
        let entries = response["windows"] as? [[String:Any]] ?? []
        var ids = Set<UInt64>()
        order = entries.compactMap { ($0["id"] as? NSNumber)?.uint64Value }
        let foreground = (response["foreground"] as? NSNumber)?.uint64Value
        remoteForeground = foreground
        for entry in entries {
            guard let id = (entry["id"] as? NSNumber)?.uint64Value,
                  let token = (entry["token"] as? NSNumber)?.uint64Value else { continue }
            ids.insert(id)
            if let old = surfaces[id], old.token != token { removeSurface(id) }
            if surfaces[id] == nil {
                let presentation=supportsWindowManagement ? (entry["presentation"] as? [String:Any]).map(WindowPresentation.init):nil
                let surface = RemoteSurface(id:id,token:token,root:id == root,presentation:presentation)
                let identity=(entry["process_id"] as? NSNumber).map{"\($0.uint64Value):\((entry["thread_id"] as? NSNumber)?.uint64Value ?? 0)"} ?? "token:\(token)"
                surface.nativeIdentity=identity
                if let prefs=session.viewerPreferences[id],prefs.identity==identity {
                    surface.resizeMode=prefs.mode;surface.locallyClosed=prefs.closed;surface.localOffset=prefs.offset
                    surface.window.setFrame(prefs.frame,display:false);surface.clientGeometry=true;surface.positioned=true
                }
                surfaces[id] = surface; surface.window.delegate = self
                let sid=contextSessionID
                let geometryChanged:()->Void = { [weak self,weak surface] in
                    guard let self,let surface,self.sessions[sid]?.surfaces[id] === surface else{return}
                    self.withSession(sid){self.finishLocalGeometry(surface)}
                }
                surface.resizeControl.selectMode = { [weak self,weak surface] mode in
                    guard let self,let surface,self.sessions[sid]?.surfaces[id] === surface else{return}
                    self.withSession(sid){self.setResizeMode(mode,for:surface)}
                }
                (surface.window as? RemoteNativeWindow)?.didChangeLocalGeometry=geometryChanged
                (surface.window as? RemotePanel)?.didChangeLocalGeometry=geometryChanged
                surface.view.sendInput = { [weak self] event in
                    guard let self=self,self.sessions[sid] != nil else{return}
                    self.lastActiveSessionID=sid
                    self.withSession(sid) {self.enqueue(event,id:id,token:token)}
                }
                if id == root {
                    if !surface.clientGeometry,let frame=resumeWindowFrame {surface.window.setFrame(frame,display:false);surface.clientGeometry=true}
                    resumeWindowFrame=nil
                    if !surface.locallyClosed {surface.window.makeKeyAndOrderFront(nil);surface.view.restoreTextInputFocus()}
                }
            }
            let surface = surfaces[id]!
            let bounds=rectangle(entry["bounds"])
            if surface.bounds.size != bounds.size {surface.hostGeometryChanged=true}
            surface.bounds = bounds
            let title=entry["title"] as? String ?? L("windows.untitled")
            surface.managerTitle=title
            surface.window.title = readOnly ? L("sharing.viewer_title",title) : title
            surface.view.inputEnabled = !readOnly && !surface.locallyClosed
            surface.view.anchoredPointer = supportsAnchoredPointer
            surface.view.positionedWheel = supportsPositionedWheel
            if supportsWindowManagement,let value=entry["presentation"] as? [String:Any] {applyPresentation(WindowPresentation(value),to:surface)}
            else {updateViewerControls(surface)}
        }
        for id in Array(surfaces.keys) where !ids.contains(id) { removeSurface(id) }
        session.viewerPreferences=session.viewerPreferences.filter{ids.contains($0.key)}
        // New owned viewers inherit a locally hidden owner; independent siblings stay visible.
        for s in surfaces.values where s.locallyClosed {dismissViewer(s)}
        // Attach auxiliaries only after the root exists, regardless of native z-order.
        if !supportsWindowManagement,let root = surfaces[root] {
            for id in order.reversed() where id != root.id {
                guard let surface = surfaces[id],!surface.locallyClosed else { continue }
                if surface.window.parent == nil { root.window.addChildWindow(surface.window,ordered:.above) }
                // Show only once a real frame is available; don't expose uninitialized surfaces.
                if !surface.locallyClosed && surface.view.image != nil && !surface.window.isVisible {
                    surface.window.orderFront(nil)
                    if id == foreground && NSApp.isActive { surface.window.makeKey(); surface.view.restoreTextInputFocus() }
                }
            }
        }
        layoutChildren()
        if response["truncated"] as? Bool == true { status.stringValue = L("windows.group_limit") }
    }
    func consume() {
        defer {if supportsPlaybackFeedback || !decodingFrame {acknowledgeIncoming()}}
        while let newline=buffer.firstIndex(of:10) {
            let line=buffer[..<newline];buffer.removeSubrange(...newline)
            guard let response=try? JSONSerialization.jsonObject(with:line) as? [String:Any] else {disconnect();status.stringValue=L("error.invalid_response");return}
            parsedResponses.append(response)
        }
        while (supportsPlaybackFeedback || !decodingFrame), !parsedResponses.isEmpty {
            let response=parsedResponses.removeFirst()
            guard let type=response["type"] as? String else {disconnect();status.stringValue=L("error.invalid_response");return}
            lastResponseAt=Date()
            if response["event"] as? Bool == true && type == "frame" {supportsPushVideo=true}
            if response["bridge_async"] as? Bool == true {
                if !asynchronousRequests.isEmpty {asynchronousRequests.removeFirst()}
            } else if response["event"] as? Bool != true {pending = false}
            let sid=(response["session_id"] as? NSNumber)?.uint64Value ?? pendingSessionID
            if ["windows","pong","bridge_error","stream_started"].contains(type) {
                withSession(0) {handleResponse(response,type:type)}
            } else if sessions[sid] != nil {
                withSession(sid) {handleResponse(response,type:type)}
            }
        }
    }
    func queueFrame(_ response:[String:Any],surface:RemoteSurface) {
        let encoded=response["image"] as? String
        guard let data=(response["binary_image"] as? Data) ?? encoded.flatMap({Data(base64Encoded:$0)}),
              let width=response["width"] as? Double,let height=response["height"] as? Double else{return}
        let epoch=(response["stream_epoch"] as? NSNumber)?.uint64Value ?? 0
        let sequence=(response["sequence"] as? NSNumber)?.uint64Value ?? 0
        guard sequence>surface.receivedSequence else{return}
        let packetBytes=data.count+(encoded?.utf8.count ?? 0)+((response["alpha_png"] as? String)?.utf8.count ?? 0)+((response["binary_alpha"] as? Data)?.count ?? 0)
        guard decodingBytes+packetBytes<96*1024*1024,decodingCount<64,surface.queuedFrames<4 else {
            connectionFailed(L("error.frame_limit"),retryable:false);return
        }
        if epoch != surface.streamEpoch {surface.videoDecoder=WindowVideoDecoder();surface.streamEpoch=epoch}
        surface.receivedSequence=sequence;surface.queuedFrames+=1
        decodingCount+=1;decodingBytes+=packetBytes
        let current=generation,sid=contextSessionID,id=surface.id,decoder=surface.videoDecoder
        let received=response["_client_received_at"] as? Double ?? ProcessInfo.processInfo.systemUptime,slots=decodeSlots
        surface.decodeQueue.async { [weak self] in
            slots.wait()
            let began=ProcessInfo.processInfo.systemUptime
            let decoded:Result<NSImage?,Error> = Result {
                if response["format"] as? String == "h264" {return try decoder.image(data,alphaPNG:(response["binary_alpha"] as? Data) ?? WindowVideoDecoder.alphaData(response["alpha_png"]))}
                return try WindowVideoDecoder.stillImage(data)
            }
            let decodeMS=(ProcessInfo.processInfo.systemUptime-began)*1000
            slots.signal()
            RunLoop.main.perform(inModes:[.common]) { [weak self] in
                guard let self=self,self.generation==current else{return}
                self.decodingCount-=1;self.decodingBytes-=packetBytes;surface.queuedFrames-=1
                if self.sessions[sid]?.surfaces[id] === surface {
                    self.withSession(sid) {
                        switch decoded {
                        case .success(let image):
                            if surface.streamEpoch==epoch {
                                surface.sequence=sequence
                                if let image {self.presentFrame(response,surface:surface,image:image,width:width,height:height,payloadBytes:data.count)}
                            }
                            if self.supportsPlaybackFeedback {
                                let feedback:[String:Any]=["command":"video_feedback","session_id":sid,"id":id,"token":surface.token,"stream_epoch":epoch,"sequence":sequence,"decode_ms":min(60000,decodeMS),"queue_ms":min(60000,max(0,(ProcessInfo.processInfo.systemUptime-received)*1000-decodeMS)),"_client_received_at":received]
                                if surface.locallyClosed {surface.heldFeedback.append(feedback)} else {self.playbackFeedback.append(feedback)}
                            }
                        case .failure(let error):
                            self.controlRequests.append(["command":"close_window"]);self.clearSelection()
                            self.status.stringValue=L("error.decode",error.localizedDescription)
                        }
                    }
                }
                self.consume();self.poll()
            }
        }
    }
    func presentFrame(_ response:[String:Any],surface:RemoteSurface,image:NSImage,width:Double,height:Double,payloadBytes:Int) {
        guard !surface.locallyClosed else{return}
        let id=surface.id
        let metricID = session.metricWindowID.flatMap{surfaces[$0] != nil ? $0 : nil} ?? rootID
        if id == metricID {
            if let backend=response["encoder"] as? String, let ms=response["encode_ms"] as? Double {session.encoderText=String(format:L("stats.encode"),backend == "nvenc" ? "NVIDIA NVENC" : (backend == "amf" ? "AMD AMF" : (backend == "qsv" ? "Intel QSV" : (backend == "hardware-mf" ? L("encoder.hardware") : (backend == "software-mf" ? L("encoder.software") : L("encoder.image"))))),ms)}
            statsFrames += 1
            sourceSizeText="\(Int(width))×\(Int(height))"
            if let w = response["encoded_width"] as? Int, let h = response["encoded_height"] as? Int { encodedSizeText = " \(w)×\(h)" }
        }; statsBytes += payloadBytes + ((response["binary_alpha"] as? Data)?.count ?? (response["alpha_png"] as? String).map{$0.utf8.count*3/4} ?? 0)
        let sizeChanged = surface.view.image?.size != NSSize(width:width,height:height)
        image.size = NSSize(width:width,height:height)
        surface.view.image = image
        surface.sequence = (response["sequence"] as? NSNumber)?.uint64Value ?? 0
        if sizeChanged {updateViewerControls(surface);layoutChildren()}
        if !surface.window.isVisible && (supportsWindowManagement || id != rootID) && surface.presentation?.minimized != true && surface.canPresent {
            surface.window.orderFront(nil)
            if id == remoteForeground && NSApp.isActive {
                let previous=applyingWindowState;applyingWindowState=true
                surface.window.makeKey(); surface.view.restoreTextInputFocus();applyingWindowState=previous
            }
        }
        status.stringValue = L("status.connected_counts",sessions.values.filter{$0.selected}.count,sessions.values.reduce(0){$0+$1.surfaces.values.filter{!$0.locallyClosed}.count})
    }
    func handleResponse(_ response:[String:Any],type:String) {
        defer {if ["windows","selected","window_group","closed","error","bridge_error"].contains(type) {manager?.refresh(force:true)}}
        if session.ending && !["closed","error","windows","pong","stream_started","bridge_error"].contains(type) {return}
            switch type {
            case "bridge_error":
                status.toolTip=response["message"] as? String
                let category=response["category"] as? String ?? "protocol"
                let explanation:[String:String]=["permission":L("error.permission"),"authentication":L("error.authentication"),"network":L("error.network"),"protocol":L("error.protocol")]
                connectionFailed(explanation[category] ?? L("error.connection"),retryable:response["retryable"] as? Bool == true)
                return
            case "video_feedback_ok":
                if let id=(response["id"] as? NSNumber)?.uint64Value,let surface=surfaces[id],
                   (response["token"] as? NSNumber)?.uint64Value==surface.token,
                   let fps=response["fps"] as? Int,let bitrate=response["bitrate"] as? Double,let ceiling=response["bitrate_ceiling"] as? Double {
                    let reason=response["adaptation"] as? String ?? "stable"
                    let reasons=["stable":L("stats.adaptation.stable"),"client_busy":L("stats.adaptation.client_busy"),"delivery_backlog":L("stats.adaptation.delivery_backlog"),"recovering":L("stats.adaptation.recovering")]
                    surface.rateText=String(format:L("stats.target_rate"),fps,bitrate/1_000_000,ceiling/1_000_000,reasons[reason] ?? L("stats.adaptation.stable"))
                }
            case "video_settings_ok":
                videoMode=response["codec"] as? String == "h264";lastViewport=nil
                for surface in surfaces.values {surface.lastViewport=nil;surface.rateText=""}
                status.stringValue=L("quality.updated")
            case "stream_started": supportsPushVideo=true
            case "pong":
                heartbeatMS=response["round_trip_ms"] as? Double
            case "windows":
                authenticated=true;reconnectAttempt=0
                readOnly=response["read_only"] as? Bool ?? false
                syncRemoteClipboard.isEnabled = !readOnly
                supportsPipeline=response["bridge_pipeline"] as? Bool == true
                supportsPlaybackFeedback=supportsPipeline && response["playback_feedback"] as? Bool == true
                supportsLiveSettings=response["live_video_settings"] as? Bool == true
                supportsWindowVideoAlpha=response["window_video_alpha"] as? Bool == true
                supportsPositionedWheel=response["positioned_wheel"] as? Bool == true
                supportsAnchoredPointer=response["anchored_pointer"] as? Bool == true
                supportsMultiSession=response["multi_session"] as? Bool == true
                supportsWindowManagement=response["window_management"] as? Bool == true
                supportsResolutionBitrate=response["resolution_bitrate"] as? Bool == true
                if response["push_video"] as? Bool == true && !supportsPushVideo {connectionRequests.insert(["command":"start_stream"],at:0)}
                replaceManagerCatalog(response["windows"] as? [[String:Any]] ?? [])
                status.stringValue = readOnly ? L("sharing.read_only") : L("status.choose_window")
                if restoreAfterConnect {
                    restoreAfterConnect=false
                    for state in sessions.values {if let ticket=state.resumeToken {state.needsQualityUpdate=true;state.controlRequests.append(["command":"resume_window","resume_token":ticket])}}
                    status.stringValue=L("status.restoring")
                }
            case "selected":
                if let value=response["read_only"] as? Bool {readOnly=value;syncRemoteClipboard.isEnabled = !value}
                resumeToken=response["resume_token"] as? String
                guard response["window_groups"] as? Bool == true else { status.stringValue = L("windows.upgrade_groups"); return }
                rootID = (response["id"] as? NSNumber)?.uint64Value
                session.requestedRoot=rootID
                selected = true; lastActiveSessionID=contextSessionID; lastGroup = .distantPast; frameCursor = 0
                statsFrames=0;statsBytes=0;statsStart=Date()
                status.stringValue = L("status.discovering")
            case "window_action_ok":
                if let id=(response["id"] as? NSNumber)?.uint64Value,let s=surfaces[id],(response["token"] as? NSNumber)?.uint64Value==s.token {
                    if response["action"] as? String=="resize",response["final"] as? Bool==true,(response["revision"] as? NSNumber)?.uint64Value==s.resizePending {s.resizeSettlesAt=Date().addingTimeInterval(0.25)}
                    lastGroup = .distantPast
                }
            case "closed":
                workflow.forget(session:contextSessionID)
                session.requestedRoot=nil;resumeToken=nil
                if contextSessionID != 0 {sessions.removeValue(forKey:contextSessionID)}
            case "window_group":
                lastGroup=Date();syncGroup(response)
                if let root=rootID,let surface=surfaces[root] {workflow.track(session:contextSessionID,id:root,generation:surface.token)}
                queueGlobalQuality(for: session)
            case "frame", "unchanged":
                guard let id = (response["id"] as? NSNumber)?.uint64Value,
                      let token = (response["token"] as? NSNumber)?.uint64Value,
                      let surface = surfaces[id], surface.token == token else { return }
                if type == "frame" {queueFrame(response,surface:surface)}
                if !surface.view.hasMarkedText(), let caret = response["caret"] as? [Double], caret.count == 2 {
                    let rect = surface.view.imageRect
                    surface.view.inputAnchor = NSPoint(x:rect.minX+caret[0]*rect.width,y:rect.maxY-caret[1]*rect.height)
                    surface.view.inputContext?.invalidateCharacterCoordinates()
                }
            case "clipboard_result":
                guard let requestID = (response["request_id"] as? NSNumber)?.uint64Value,
                      let request = clipboardRequests.removeValue(forKey:requestID),
                      surfaces[request.id]?.token == request.token else { return }
                if response["action"] as? String == "paste" { status.stringValue = L("clipboard.pasted"); return }
                if response["action"] as? String == "poll" {
                    guard syncRemoteClipboard.state == .on, NSApp.isActive,
                          surfaces[request.id]?.window.isKeyWindow == true, (response["text"] is String || response["image_png"] is String) else { return }
                }
                guard clipboardBoard.changeCount == request.changeCount else {
                    status.stringValue = L("clipboard.changed"); return
                }
                if let encoded = response["image_png"] as? String {
                    guard encoded.utf8.count <= ((ClipboardImage.maxBytes + 2) / 3) * 4,
                          let bytes = Data(base64Encoded:encoded), let png = ClipboardImage.normalizedPNG(bytes) else {
                        status.stringValue = L("clipboard.remote_image_invalid"); return
                    }
                    let item = NSPasteboardItem(); item.setData(png,forType:.png)
                    if let bitmap = NSBitmapImageRep(data:png), let tiff = bitmap.tiffRepresentation { item.setData(tiff,forType:.tiff) }
                    clipboardBoard.clearContents(); clipboardBoard.writeObjects([item])
                    status.stringValue = L("clipboard.copied_image")
                } else if let text = response["text"] as? String {
                    clipboardBoard.clearContents(); clipboardBoard.setString(text,forType:.string)
                    status.stringValue = L("clipboard.copied_text")
                } else if response["action"] as? String != "poll" {
                    status.stringValue = L("clipboard.no_change")
                }
            case "error":
                if response["command"] as? String=="window_action",let id=(response["window_id"] as? NSNumber)?.uint64Value,let s=surfaces[id] {s.resizeGesture=nil;s.resizeSample=nil;s.resizePending=nil;s.resizeSettlesAt=nil}
                if let requestID = (response["request_id"] as? NSNumber)?.uint64Value { clipboardRequests.removeValue(forKey:requestID) }
                if response["window_lost"] as? Bool == true,
                   let id = (response["window_id"] as? NSNumber)?.uint64Value,
                   let token = (response["token"] as? NSNumber)?.uint64Value {
                    if surfaces[id]?.token == token {
                        windows.removeAll{($0["id"] as? NSNumber)?.uint64Value==id};managerCatalogIDs.removeValue(forKey:id)
                        removeSurface(id)
                    }
                    lastGroup = .distantPast
                    status.stringValue = L("status.connected_counts",sessions.values.filter{$0.selected}.count,sessions.values.reduce(0){$0+$1.surfaces.values.filter{!$0.locallyClosed}.count})
                    return
                }
                if response["selection_lost"] as? Bool == true || (!selected && session.requestedRoot != nil) {
                    let lost=Set(surfaces.keys).union(session.requestedRoot.map{[$0]} ?? [])
                    windows.removeAll{($0["id"] as? NSNumber).map{lost.contains($0.uint64Value)} ?? false}
                    for id in lost {managerCatalogIDs.removeValue(forKey:id)}
                    session.requestedRoot=nil;resumeWindowFrame=nil;resumeToken=nil;clearSelection()
                    if contextSessionID != 0 {sessions.removeValue(forKey:contextSessionID)}
                }
                status.stringValue = response["command"] as? String=="window_action" ? L("error.window_action") : (response["message"] as? String ?? L("error.remote"))
                lastGroup = .distantPast
            default: break
            }
    }
    func applicationShouldTerminate(_ sender:NSApplication)->NSApplication.TerminateReply {
        if quitting {return .terminateLater};quitting=true
        forgetRecovery();wantsConnection=false;retryAt=nil;timer?.invalidate()
        let child=process;disconnect()
        guard let child=child,child.isRunning else{return .terminateNow}
        DispatchQueue.main.asyncAfter(deadline:.now()+1.2) {
            if child.isRunning {kill(child.processIdentifier,SIGKILL)}
            DispatchQueue.global().async {child.waitUntilExit();DispatchQueue.main.async {sender.reply(toApplicationShouldTerminate:true)}}
        }
        return .terminateLater
    }
    func applicationWillTerminate(_ notification: Notification) {
        manager?.discovery.stop()
        for observer in wakeObservers {NSWorkspace.shared.notificationCenter.removeObserver(observer)}
        wantsConnection=false;retryAt=nil;disconnect()
    }
}
