import AppKit
import VideoToolbox
import CoreImage
import ImageIO

let application = NSApplication.shared
let delegate = AppDelegate()
application.delegate = delegate
application.setActivationPolicy(.regular)
application.run()
