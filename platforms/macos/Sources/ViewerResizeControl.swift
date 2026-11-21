import AppKit

// A native title-bar control belongs to its own viewer, including inactive panels.
final class ViewerResizeControl:NSTitlebarAccessoryViewController {
    let popup=NSPopUpButton(frame:.zero,pullsDown:false)
    var selectMode:((ViewerResizeMode)->Void)?
    init() {
        super.init(nibName:nil,bundle:nil)
        layoutAttribute = .right
        view=NSView(frame:NSRect(x:0,y:0,width:126,height:24))
        popup.frame=NSRect(x:0,y:2,width:120,height:20)
        popup.controlSize = .small;popup.font = .systemFont(ofSize:11)
        popup.addItems(withTitles:[L("viewer.scale_local"),L("viewer.resize_host")])
        popup.menu?.autoenablesItems=false
        popup.target=self;popup.action=#selector(changed)
        popup.setAccessibilityLabel(L("viewer.resize_mode"))
        view.addSubview(popup)
    }
    required init?(coder:NSCoder){fatalError("init(coder:) has not been implemented")}
    func update(mode:ViewerResizeMode,hostAllowed:Bool,reason:String?) {
        popup.selectItem(at:mode.rawValue)
        popup.item(at:1)?.isEnabled=hostAllowed
        popup.item(at:0)?.toolTip=L("viewer.scale_hint")
        popup.item(at:1)?.toolTip=reason ?? L("viewer.host_hint")
        popup.toolTip=mode == .scale ? L("viewer.scale_hint")+(reason.map{"\n"+$0} ?? ""):(reason ?? L("viewer.host_hint"))
    }
    @objc func changed() {
        guard let mode=ViewerResizeMode(rawValue:popup.indexOfSelectedItem) else{return}
        selectMode?(mode)
    }
}

enum ViewerAspect {
    static func fit(_ available:NSSize,source:NSSize)->NSSize {
        guard source.width>0,source.height>0,available.width>0,available.height>0 else{return available}
        let scale=min(available.width/source.width,available.height/source.height)
        return NSSize(width:source.width*scale,height:source.height*scale)
    }
    static func resize(_ proposed:NSSize,from current:NSSize,source:NSSize)->NSSize {
        guard source.width>0,source.height>0 else{return proposed}
        let ratio=source.width/source.height
        // Follow the edge the user moved, rather than repeatedly shrinking to fit.
        if abs(proposed.width-current.width)/max(1,current.width) >= abs(proposed.height-current.height)/max(1,current.height) {
            return NSSize(width:proposed.width,height:proposed.width/ratio)
        }
        return NSSize(width:proposed.height*ratio,height:proposed.height)
    }
}
