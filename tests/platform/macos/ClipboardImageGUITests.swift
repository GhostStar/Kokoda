import AppKit
let board = NSPasteboard.general
let backup = (board.pasteboardItems ?? []).map { item in item.types.compactMap { type -> (NSPasteboard.PasteboardType,Data)? in item.data(forType:type).map { (type,$0) } } }
let sample = Data(base64Encoded:"iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP4z8DwHwgbGMA0EAAAP9cIeY8TiooAAAAASUVORK5CYII=")!
var lastTestChange = board.changeCount
defer {
 if board.changeCount == lastTestChange {
  board.clearContents()
  let items = backup.map { values -> NSPasteboardItem in let item=NSPasteboardItem();for (type,data) in values {item.setData(data,forType:type)};return item }
  board.writeObjects(items)
 }
}
func script(_ source: String) throws {
 let p=Process();p.executableURL=URL(fileURLWithPath:"/usr/bin/osascript");p.arguments=["-e",source];try p.run();p.waitUntilExit();if p.terminationStatus != 0 {throw NSError(domain:"UI",code:1)}
}
func keys(_ source: String) throws {try script("tell application \"System Events\" to tell process \"Kokoda\"\n"+source+"\nend tell")}
try keys("set frontmost to true\nperform action \"AXRaise\" of window \"Kokoda Image Clipboard Test\"\ndelay 0.3\nset pos to position of window \"Kokoda Image Clipboard Test\"\nclick at {(item 1 of pos) + 100, (item 2 of pos) + 150}")
Thread.sleep(forTimeInterval:1)
board.clearContents();board.setData(sample,forType:.png);lastTestChange = board.changeCount
try keys("keystroke \"v\" using command down")
Thread.sleep(forTimeInterval:1)

Thread.sleep(forTimeInterval:0.5)
board.clearContents();board.setString("Kokoda clipboard sentinel",forType:.string);lastTestChange = board.changeCount
try keys("key code 101") // F9 publishes native PNG + Bitmap without Kokoda clipboard commands.
var passed=false
for _ in 0..<30 {
 Thread.sleep(forTimeInterval:0.1)
 if let png=board.data(forType:.png),let image=NSBitmapImageRep(data:png),image.pixelsWide==2,image.pixelsHigh==2 {
  var pixel=[Int](repeating:0,count:4);image.getPixel(&pixel,atX:0,y:0)
  if pixel == [255,0,0,255] {passed=true;lastTestChange=board.changeCount;break}
 }
}
print(passed ? "PASS: native Windows image copy -> GUI poll -> Mac PNG pasteboard" : "FAIL: GUI clipboard sync did not arrive")
if !passed {throw NSError(domain:"ClipboardTest",code:2)}
