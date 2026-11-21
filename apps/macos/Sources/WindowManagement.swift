import AppKit

extension AppDelegate {
    func queueWindowAction(_ surface:RemoteSurface,_ action:String,fields:[String:Any]=[:]) {
        guard supportsWindowManagement,!readOnly,surface.presentation?.enabled==true else{return}
        if action=="resize" {controlRequests.removeAll{($0["id"] as? UInt64)==surface.id && $0["action"] as? String=="resize"}}
        if action=="activate" {controlRequests.removeAll{$0["action"] as? String=="activate"}}
        if ["close","minimize","maximize","restore"].contains(action) {controlRequests.removeAll{($0["id"] as? UInt64)==surface.id && $0["action"] as? String==action}}
        var packet:[String:Any]=["command":"window_action","id":surface.id,"token":surface.token,"action":action]
        for (key,value) in fields {packet[key]=value}
        controlRequests.append(packet)
    }
    func windowShouldClose(_ sender:NSWindow)->Bool {
        guard !removingWindows,let sid=sessionID(for:sender) else{return true}
        withSession(sid){if let s=surface(for:sender) {dismissViewer(s)}}
        return false
    }
    func rememberViewer(_ s:RemoteSurface) {
        session.viewerPreferences[s.id]=ViewerPreferences(identity:s.nativeIdentity,closed:s.locallyClosed,mode:s.resizeMode,frame:s.window.frame,offset:s.localOffset)
    }
    func dismissViewer(_ s:RemoteSurface) {
        var closing:Set<UInt64>=[s.id]
        for _ in 0..<surfaces.count {
            for child in surfaces.values where child.role != "independent" {
                if let parent=child.presentation?.owner ?? (child.id != rootID ? rootID:nil),closing.contains(parent) {closing.insert(child.id)}
            }
        }
        for id in closing {
            guard let item=surfaces[id],!item.locallyClosed else{continue}
            inputEvents.removeAll{($0["id"] as? UInt64)==id}
            clipboardRequests=clipboardRequests.filter{$0.value.id != id}
            cancelViewerResize(item)
            item.resetInput();enqueue(["kind":"release"],id:id,token:item.token)
            item.locallyClosed=true;item.view.inputEnabled=false
            item.heldFeedback += playbackFeedback.filter{($0["session_id"] as? UInt64)==contextSessionID && ($0["id"] as? UInt64)==id}
            playbackFeedback.removeAll{($0["session_id"] as? UInt64)==contextSessionID && ($0["id"] as? UInt64)==id}
            item.window.isExcludedFromWindowsMenu=true;item.window.parent?.removeChildWindow(item.window);item.window.orderOut(nil);rememberViewer(item)
        }
    }
    func restoreViewers() {
        let previous=applyingWindowState;applyingWindowState=true
        defer{applyingWindowState=previous}
        for s in surfaces.values where s.locallyClosed {
            s.locallyClosed=false;playbackFeedback += s.heldFeedback;s.heldFeedback.removeAll()
            if let p=s.presentation {s.configure(p,readOnly:readOnly)} else {s.view.inputEnabled = !readOnly;s.window.isExcludedFromWindowsMenu=s.role != "independent"}
            if s.window.isMiniaturized {s.window.deminiaturize(nil)}
            s.window.orderFront(nil)
            rememberViewer(s)
        }
        layoutChildren();poll()
    }
    @objc func restoreClosedViewers() {
        let preferred=focusSessionID ?? lastActiveSessionID
        guard let sid=preferred.flatMap({sessions[$0]?.surfaces.values.contains{$0.locallyClosed} == true ? $0:nil}) ?? sessions.values.first(where:{$0.surfaces.values.contains{$0.locallyClosed}})?.id else{return}
        withSession(sid){restoreViewers()}
    }
    func activeViewer()->(UInt64,RemoteSurface)? {
        guard let sid=focusSessionID ?? lastActiveSessionID,let state=sessions[sid] else{return nil}
        let s=state.surfaces.values.first{$0.window.isKeyWindow} ?? state.metricWindowID.flatMap{state.surfaces[$0]} ?? state.rootID.flatMap{state.surfaces[$0]}
        return s.map{(sid,$0)}
    }
    func cancelViewerResize(_ s:RemoteSurface) {
        controlRequests.removeAll{($0["id"] as? UInt64)==s.id && $0["command"] as? String=="window_action"}
        s.resizeGesture=nil;s.resizeSample=nil;s.resizePending=nil;s.resizeSettlesAt=nil
    }
    func hostResizeUnavailable(_ s:RemoteSurface)->String? {
        if !supportsWindowManagement {return L("viewer.host_upgrade")}
        if readOnly {return L("viewer.host_read_only")}
        if s.presentation?.enabled != true {return L("viewer.host_blocked")}
        if s.presentation?.canResize != true {return L("viewer.host_fixed")}
        return nil
    }
    func updateViewerControls(_ s:RemoteSurface) {
        let reason=hostResizeUnavailable(s)
        if s.resizeMode == .hostWindow,reason != nil {
            cancelViewerResize(s);s.resizeMode = .scale
        }
        if s.role != "transient",s.window.styleMask.contains(.titled) {
            if !s.window.titlebarAccessoryViewControllers.contains(where:{$0 === s.resizeControl}) {s.window.addTitlebarAccessoryViewController(s.resizeControl)}
        }
        s.resizeControl.update(mode:s.resizeMode,hostAllowed:reason==nil,reason:reason)
        let source=s.displaySourceSize
        let aspect=s.resizeMode == .scale && source.width>0 && source.height>0 ? NSSize(width:source.width/source.height,height:1):.zero
        if aspect != .zero {
            if s.window.contentAspectRatio != aspect {s.window.contentAspectRatio=aspect}
        } else {
            // Cancel the aspect constraint with resize increments. Assigning
            // (0,0) leaves AppKit's zoom animator with a zero divisor.
            s.window.contentResizeIncrements=NSSize(width:1,height:1)
        }
    }
    func setResizeMode(_ mode:ViewerResizeMode,for s:RemoteSurface) {
        guard !s.locallyClosed,mode == .scale || hostResizeUnavailable(s)==nil else{updateViewerControls(s);return}
        cancelViewerResize(s);s.resizeMode=mode;s.hostGeometryChanged=true;updateLocalScale(s)
        updateViewerControls(s)
        if let p=s.presentation {s.configure(p,readOnly:readOnly)}
        layoutChildren();rememberViewer(s)
        s.view.restoreTextInputFocus()
    }
    @objc func changeResizeMode(_ item:NSMenuItem) {
        guard let (sid,s)=activeViewer(),let mode=ViewerResizeMode(rawValue:item.tag) else{return}
        withSession(sid){setResizeMode(mode,for:s)}
    }
    @objc func changeViewerZoom(_ item:NSMenuItem) {
        guard let (sid,s)=activeViewer(),[50,75,100,125,150,200].contains(item.tag),s.bounds.width>0 else{return}
        withSession(sid) {
            cancelViewerResize(s)
            let scale=Double(item.tag)/100*96/s.hostDPI
            let source=s.displaySourceSize
            var rect=s.window.contentRect(forFrameRect:s.window.frame)
            rect.origin.y=rect.maxY-source.height*scale;rect.size=NSSize(width:source.width*scale,height:source.height*scale)
            let previous=applyingWindowState;applyingWindowState=true
            s.window.setFrame(s.window.frameRect(forContentRect:rect),display:true);applyingWindowState=previous
            s.displayScale=scale;s.clientGeometry=true;resizedAt=Date();rememberViewer(s);layoutChildren()
        }
    }
    func fitLocalCanvas(_ s:RemoteSurface) {
        var content=s.window.contentRect(forFrameRect:s.window.frame)
        let size=ViewerAspect.fit(content.size,source:s.displaySourceSize)
        guard abs(content.width-size.width)>0.01 || abs(content.height-size.height)>0.01 else{return}
        content.origin.y=content.maxY-size.height;content.size=size
        let previous=applyingWindowState;applyingWindowState=true
        s.window.setFrame(s.window.frameRect(forContentRect:content),display:true)
        applyingWindowState=previous
    }
    func finishLocalGeometry(_ s:RemoteSurface) {
        fitLocalCanvas(s);updateLocalScale(s)
        if s.role != "independent",let expected=managedExpectedRect(s) {
            let actual=s.window.contentRect(forFrameRect:s.window.frame)
            s.localOffset=NSPoint(x:actual.minX-expected.minX,y:actual.maxY-expected.maxY)
        }
        resizedAt=Date();rememberViewer(s);layoutChildren()
    }
    func updateLocalScale(_ s:RemoteSurface) {
        guard s.bounds.width>0,s.bounds.height>0 else{return}
        let rect=s.window.contentRect(forFrameRect:s.window.frame)
        s.displayScale=min(rect.width/s.displaySourceSize.width,rect.height/s.displaySourceSize.height);s.clientGeometry=true
    }
    // Title-bar controls belong to the viewer. Only an explicit live border
    // gesture in hostWindow mode may send a remote size request.
    func windowShouldZoom(_ window:NSWindow,toFrame:NSRect)->Bool {
        if let sid=sessionID(for:window) {withSession(sid){
            if let s=surface(for:window) {cancelViewerResize(s)}
        }}
        return true
    }
    func windowWillResize(_ window:NSWindow,to proposedFrame:NSSize)->NSSize {
        guard let sid=sessionID(for:window),let s=sessions[sid]?.surfaces.values.first(where:{$0.window===window}),
              !applyingWindowState,(s.resizeMode == .scale || s.localGeometryAction) else{return proposedFrame}
        let proposed=window.contentRect(forFrameRect:NSRect(origin:.zero,size:proposedFrame)).size
        let current=window.contentRect(forFrameRect:window.frame).size
        let size=s.localGeometryAction ? ViewerAspect.fit(proposed,source:s.displaySourceSize):ViewerAspect.resize(proposed,from:current,source:s.displaySourceSize)
        return window.frameRect(forContentRect:NSRect(origin:.zero,size:size)).size
    }
    func windowWillUseStandardFrame(_ window:NSWindow,defaultFrame frame:NSRect)->NSRect {
        guard let sid=sessionID(for:window),let s=sessions[sid]?.surfaces.values.first(where:{$0.window===window}) else{return frame}
        var content=window.contentRect(forFrameRect:frame)
        let size=ViewerAspect.fit(content.size,source:s.displaySourceSize)
        content.origin.y=content.maxY-size.height;content.size=size
        return window.frameRect(forContentRect:content)
    }
    @objc func endViewingApplication() {
        guard let sid=focusSessionID ?? lastActiveSessionID,sessions[sid] != nil else{return}
        withSession(sid){resumeToken=nil;clearSelection();controlRequests.removeAll();controlRequests.append(["command":"close_window"]);poll()}
    }
    func windowWillStartLiveResize(_ note:Notification) {
        guard !applyingWindowState,supportsWindowManagement,!readOnly,let window=note.object as? NSWindow,let sid=sessionID(for:window) else{return}
        withSession(sid) {
            guard let s=surface(for:window),!s.localGeometryAction,s.resizeMode == .hostWindow,let p=s.presentation,p.enabled,p.canResize,s.bounds.width>0,s.bounds.height>0,s.view.imageRect.width>0 else{return}
            let rect=window.contentRect(forFrameRect:window.frame)
            let ratio=NSSize(width:s.bounds.width/s.view.imageRect.width,height:s.bounds.height/s.view.imageRect.height)
            s.resizeGesture=WindowResizeGesture(source:s.bounds,content:rect,pixelsPerPoint:ratio)
            s.displayScale=1/ratio.width;s.resizeSettlesAt=nil;s.resizeSample=nil
        }
    }
    func updateLiveResize(_ s:RemoteSurface) {
        guard s.resizeMode == .hostWindow,let gesture=s.resizeGesture else{return}
        s.resizeSample=gesture.target(s.window.contentRect(forFrameRect:s.window.frame))
        sendResizeSample(s,final:false)
    }
    func sendResizeSample(_ s:RemoteSurface,final:Bool) {
        guard let sample=s.resizeSample,final || Date().timeIntervalSince(s.lastResizeSent)>=0.1 else{return}
        s.resizeRevision+=1;s.lastResizeSent=Date();s.resizePending=s.resizeRevision;s.resizeSettlesAt=nil
        var fields:[String:Any]=sample;fields["revision"]=s.resizeRevision;fields["final"]=final
        s.resizeSample=nil
        queueWindowAction(s,"resize",fields:fields)
    }
    func windowDidEndLiveResize(_ note:Notification) {
        guard let window=note.object as? NSWindow,let sid=sessionID(for:window) else{return}
        withSession(sid) {
            guard let s=surface(for:window) else{return}
            if s.resizeMode == .scale || readOnly {finishLocalGeometry(s);return}
            guard let gesture=s.resizeGesture else{return}
            s.resizeSample=gesture.target(window.contentRect(forFrameRect:window.frame))
            s.resizeGesture=nil;sendResizeSample(s,final:true);resizedAt=Date();rememberViewer(s)
        }
    }
    func applyPresentation(_ value:WindowPresentation,to s:RemoteSurface) {
        s.configure(value,readOnly:readOnly)
        updateViewerControls(s)
        if let until=s.resizeSettlesAt,Date()>=until,s.resizeGesture==nil {s.resizePending=nil;s.resizeSettlesAt=nil}
        if !value.enabled {s.resetInput()}
    }
    func managedParent(_ surface:RemoteSurface)->RemoteSurface? {
        guard surface.role != "independent",let owner=surface.presentation?.owner,owner != surface.id else{return nil}
        // Native owner chains should be acyclic. Reject corrupt/stale snapshots.
        var visited:Set<UInt64>=[surface.id];var cursor:UInt64?=owner
        while let id=cursor,let next=surfaces[id] {if !visited.insert(id).inserted{return nil};cursor=next.presentation?.owner}
        return surfaces[owner]
    }
    func managedExpectedRect(_ s:RemoteSurface)->NSRect? {
        guard let rootID,let anchor=managedParent(s) ?? surfaces[rootID],anchor !== s,anchor.bounds.width>0 else{return nil}
        let rect=anchor.view.image != nil ? anchor.window.convertToScreen(anchor.view.convert(anchor.view.imageRect,to:nil)):anchor.window.contentRect(forFrameRect:anchor.window.frame)
        return RemoteWindowLayout.childRect(rootScreen:rect,rootBounds:anchor.bounds,childBounds:s.bounds)
    }
    func layoutManagedWindows() {
        guard !layoutInProgress else{return}
        layoutInProgress=true
        let previous=applyingWindowState;applyingWindowState=true
        defer{layoutInProgress=false;applyingWindowState=previous}
        for s in surfaces.values {
            let parent=s.locallyClosed ? nil:managedParent(s)?.window
            if s.window.parent !== parent {s.window.parent?.removeChildWindow(s.window);parent?.addChildWindow(s.window,ordered:.above)}
        }
        // Owners first; nested panels settle without making siblings children of root.
        let ordered=surfaces.values.sorted{($0.presentation?.owner == nil ? 0:1)<($1.presentation?.owner == nil ? 0:1)}
        for s in ordered where s.presentation != nil && s.bounds.width>0 && s.bounds.height>0 {
            guard !s.locallyClosed,!s.localGeometryAction,!s.geometryHeld,!s.window.isMiniaturized else{continue}
            if s.displayScale==nil {
                let zoom=managedParent(s)?.localZoom ?? rootID.flatMap{surfaces[$0]}.flatMap{$0 === s ? nil:$0.localZoom} ?? 1
                s.displayScale=zoom*96/s.hostDPI
            }
            var content=s.window.contentRect(forFrameRect:s.window.frame)
            var size=content.size
            if !s.clientGeometry || (s.resizeMode == .hostWindow && s.hostGeometryChanged) {
                var scale=s.displayScale ?? (96/s.hostDPI)
                if (!s.clientGeometry || s.presentation?.maximized==true),let screen=(s.window.screen ?? NSScreen.main)?.visibleFrame {
                    scale=min(scale,(screen.width-40)/s.bounds.width,(screen.height-80)/s.bounds.height)
                }
                size=NSSize(width:max(1,s.bounds.width*scale),height:max(1,s.bounds.height*scale))
            }
            if s.resizeMode == .scale {
                let source=s.displaySourceSize
                if s.clientGeometry,s.fittedSourceSize != source,source.width>0,source.height>0 {
                    size=NSSize(width:size.width,height:size.width*source.height/source.width)
                    if let screen=(s.window.screen ?? NSScreen.main)?.visibleFrame {
                        let maximum=s.window.contentRect(forFrameRect:screen).size
                        let cap=min(1,maximum.width/size.width,maximum.height/size.height)
                        size=NSSize(width:size.width*cap,height:size.height*cap)
                    }
                }
                size=ViewerAspect.fit(size,source:source)
            }
            s.fittedSourceSize=s.displaySourceSize
            if managedParent(s) != nil || !s.positioned {
                if let expected=managedExpectedRect(s) {
                    content=NSRect(x:expected.minX+s.localOffset.x,y:expected.maxY-size.height+s.localOffset.y,width:size.width,height:size.height)
                } else {content=NSRect(x:content.minX,y:content.maxY-size.height,width:size.width,height:size.height)}
            } else {content=NSRect(x:content.minX,y:content.maxY-size.height,width:size.width,height:size.height)}
            s.positioned=true;s.hostGeometryChanged=false
            let frame=s.window.frameRect(forContentRect:content)
            if frame != s.window.frame {s.window.setFrame(frame,display:true)}
            if s.resizeMode == .scale {updateLocalScale(s)} else {s.clientGeometry=true}
        }
    }
}

extension AppDelegate:NSMenuItemValidation {
    func validateMenuItem(_ item:NSMenuItem)->Bool {
        if item.action == #selector(restoreClosedViewers) {return sessions.values.contains{$0.surfaces.values.contains{$0.locallyClosed}}}
        if item.action == #selector(changeResizeMode(_:)) {
            guard let (_,s)=activeViewer(),!s.locallyClosed else{return false}
            item.state=s.resizeMode.rawValue==item.tag ? .on:.off
            return item.tag==ViewerResizeMode.scale.rawValue || (supportsWindowManagement && !readOnly && s.presentation?.enabled==true && s.presentation?.canResize==true)
        }
        if item.action == #selector(changeViewerZoom(_:)) {
            guard let (_,s)=activeViewer(),!s.locallyClosed else{return false}
            item.state=abs(s.localZoom*100-Double(item.tag))<1 ? .on:.off
            return s.role != "transient"
        }
        return true
    }
}
