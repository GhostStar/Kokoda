import AppKit
let board = NSPasteboard.general
let backup = (board.pasteboardItems ?? []).map { item in item.types.compactMap { type -> (NSPasteboard.PasteboardType,Data)? in item.data(forType:type).map { (type,$0) } } }
let sample = "Kokoda clipboard GUI 中文🙂"
defer {
 if let current = board.string(forType:.string), current == sample || current == "Kokoda clipboard sentinel" {
  board.clearContents()
  let items = backup.map { values -> NSPasteboardItem in let item=NSPasteboardItem();for (type,data) in values {item.setData(data,forType:type)};return item }
  board.writeObjects(items)
 }
}
func script(_ source: String) throws {
 let p=Process();p.executableURL=URL(fileURLWithPath:"/usr/bin/osascript");p.arguments=["-e",source];try p.run();p.waitUntilExit();if p.terminationStatus != 0 {throw NSError(domain:"UI",code:1)}
}
func keys(_ source: String) throws {try script("tell application \"System Events\" to tell process \"Kokoda\"\n"+source+"\nend tell")}
try keys("set frontmost to true\nperform action \"AXRaise\" of window \"Kokoda Clipboard Test\"\ndelay 0.3\nset pos to position of window \"Kokoda Clipboard Test\"\nclick at {(item 1 of pos) + 100, (item 2 of pos) + 150}")
Thread.sleep(forTimeInterval:1)
board.clearContents();board.setString(sample,forType:.string)
try keys("keystroke \"a\" using command down\nkeystroke \"v\" using command down")
Thread.sleep(forTimeInterval:1)
try keys("keystroke \"a\" using command down")
Thread.sleep(forTimeInterval:0.5)
board.clearContents();board.setString("Kokoda clipboard sentinel",forType:.string)
try keys("key code 100") // F8 triggers native TextBox.Copy, not Kokoda's clipboard shortcut.
var passed=false
for _ in 0..<30 {Thread.sleep(forTimeInterval:0.1);if board.string(forType:.string)==sample {passed=true;break}}
print(passed ? "PASS: native Windows copy -> GUI poll -> Mac pasteboard" : "FAIL: GUI clipboard sync did not arrive")
if !passed {throw NSError(domain:"ClipboardTest",code:2)}
