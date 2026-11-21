import AppKit

struct ManagerRow: Equatable {
    let key: String
    let id: UInt64
    let sessionID: UInt64?
    let token: UInt64?
    let title: String
    let parent: UInt64?
    let role: String
    let state: String
    let action: String
    let rank: Int
    var viewing: Bool { sessionID != nil }
}

extension AppDelegate {
    // Catalog handles have no native generation on the current wire. Keep an
    // observed incarnation, invalidate disappearance/process changes, and let
    // the host revalidate every selection. Never restore by title or row index.
    func replaceManagerCatalog(_ entries: [[String:Any]]) {
        var next: [UInt64:(serial:UInt64, identity:String)] = [:]
        for entry in entries {
            guard let id=(entry["id"] as? NSNumber)?.uint64Value else {continue}
            let identity="\(entry["process_id"] ?? ""):\(entry["thread_id"] ?? "")"
            if let old=managerCatalogIDs[id],old.identity==identity {next[id]=old}
            else {managerCatalogSerial += 1;next[id]=(managerCatalogSerial,identity)}
        }
        managerCatalogIDs=next
        windows=entries
    }
    func managerRows(query:String="") -> [ManagerRow] {
        guard authenticated else{return []}
        var result:[ManagerRow]=[]
        var represented=Set<UInt64>()
        func append(id:UInt64,sid:UInt64?,surface:RemoteSurface?,title:String,opening:Bool=false,ending:Bool=false) {
            let role=surface?.role ?? "independent"
            guard role != "transient" else{return}
            let title=title.isEmpty ? L("windows.untitled"):title
            let blocked=surface?.presentation?.enabled == false
            let modal=sid.flatMap{sessions[$0]}.flatMap{modalViewer(for:id,in:$0)}
            let model=workflow.dispatch(["type":"manager_row","connected":authenticated,
                "title":String(title.prefix(4096)),"query":String(query.prefix(1024)),"viewing":sid != nil,
                "opening":opening,"ending":ending,"closed":surface?.locallyClosed ?? false,
                "minimized":surface?.window.isMiniaturized ?? false,"blocked":blocked,"modal_available":modal != nil])
            let key:String
            if let sid=sid {key="\(generation):s\(sid):w\(id):\(surface.map{String($0.token)} ?? "opening")"}
            else {key="\(generation):c\(managerCatalogIDs[id]?.serial ?? 0):w\(id)"}
            result.append(ManagerRow(key:key,id:id,sessionID:sid,token:surface?.token,title:title,
                parent:surface?.presentation?.owner,role:role,state:model["state"] as! String,
                action:model["action"] as! String,rank:model["rank"] as! Int))
        }
        for state in sessions.values.sorted(by:{$0.id<$1.id}) {
            if state.ending {continue}
            for surface in state.surfaces.values.sorted(by:{$0.id<$1.id}) {
                represented.insert(surface.id)
                append(id:surface.id,sid:state.id,surface:surface,title:surface.managerTitle)
            }
            if let root=state.requestedRoot,!represented.contains(root) {
                represented.insert(root)
                let title=windows.first{($0["id"] as? NSNumber)?.uint64Value==root}?["title"] as? String ?? ""
                append(id:root,sid:state.id,surface:nil,title:title,opening:true)
            }
        }
        for entry in windows {
            guard let id=(entry["id"] as? NSNumber)?.uint64Value,!represented.contains(id) else{continue}
            // The catalog does not yet carry window roles. Retain selectable
            // entries instead of guessing ownership from process names.
            let ending=sessions.values.contains{$0.ending && $0.requestedRoot==id}
            append(id:id,sid:nil,surface:nil,title:entry["title"] as? String ?? "",ending:ending)
        }
        return result.sorted {
            if $0.rank != $1.rank {return $0.rank>$1.rank}
            if $0.viewing != $1.viewing {return $0.viewing}
            let order=$0.title.localizedStandardCompare($1.title)
            return order == .orderedSame ? $0.key<$1.key:order == .orderedAscending
        }
    }
    func modalViewer(for id:UInt64,in state:ApplicationSession) -> RemoteSurface? {
        var ancestors:Set<UInt64>=[id]
        for _ in 0..<state.surfaces.count {
            let matches=state.surfaces.values.filter{$0.role=="modal" && $0.presentation?.owner.map{ancestors.contains($0)} == true}
            if let enabled=matches.filter({$0.presentation?.enabled==true}).sorted(by:{$0.id<$1.id}).first {return enabled}
            let old=ancestors.count
            ancestors.formUnion(matches.map{$0.id})
            if old==ancestors.count {break}
        }
        return nil
    }
    func activateManagerRow(_ row:ManagerRow) {
        guard let current=managerRows().first(where:{$0.key==row.key}),current.action != "none" else{return}
        if let sid=current.sessionID,let state=sessions[sid],let s=state.surfaces[current.id],s.token==current.token {
            let target=current.action=="modal" ? modalViewer(for:s.id,in:state):s
            guard let target=target else{return}
            withSession(sid) {
                restoreViewers(only:target.id)
                if target.window.isMiniaturized {target.window.deminiaturize(nil)}
                target.window.makeKeyAndOrderFront(nil);target.view.restoreTextInputFocus()
                lastActiveSessionID=sid
            }
        } else if current.action=="open" {openManagerWindow(current.id)}
        manager?.refresh(force:true)
    }
    func openManagerWindow(_ root:UInt64) {
        guard authenticated,windows.contains(where:{($0["id"] as? NSNumber)?.uint64Value==root}) else{return}
        if let row=managerRows().first(where:{$0.id==root && $0.sessionID != nil}) {activateManagerRow(row);return}
        guard !sessions.values.contains(where:{$0.ending && $0.requestedRoot==root}) else{return}
        let live=sessions.values.filter{$0.selected || !$0.controlRequests.isEmpty || $0.requestedRoot != nil}
        guard live.count<8 else{status.stringValue=L("windows.session_limit");return}
        guard supportsMultiSession || live.isEmpty else{status.stringValue=L("windows.upgrade_multi");return}
        nextSessionID += 1
        let id=supportsMultiSession ? nextSessionID:0
        let state=ApplicationSession(id:id);state.requestedRoot=root
        state.preset=globalPreset;state.adaptive=adaptiveResolution.state == .on;state.applicationScope=includeApplication.state == .on
        sessions[id]=state;contextSessionID=id
        state.controlRequests.append(selectionRequest(id:root,state:state))
        manager?.refresh(force:true);poll()
    }
    func endManagerSession(_ sid:UInt64) {
        guard let state=sessions[sid],!state.ending else{return}
        let inFlight=pending && pendingSessionID==sid
        let needsClose=state.selected || inFlight
        withSession(sid) {
            state.ending=true;resumeToken=nil;clearSelection();controlRequests.removeAll()
            if needsClose {controlRequests.append(["command":"close_window"])}
            else {state.requestedRoot=nil;if sid != 0 {sessions.removeValue(forKey:sid)}}
        }
        manager?.refresh(force:true);poll()
    }
}
