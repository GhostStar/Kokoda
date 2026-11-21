let testUnits = WindowVideoDecoder.units(Data([0,0,0,1,0x67,2,3,0,0,1,0x68,4,0,0,0,1,0x65,5]))
precondition(testUnits == [Data([0x67,2,3]),Data([0x68,4]),Data([0x65,5])])
let path = CommandLine.arguments[1]
var decoder = WindowVideoDecoder()
var epoch: UInt64?
let lines = try String(contentsOfFile:path,encoding:.utf8).split(separator:"\n")
var count = 0, last: NSImage?
let start = Date()
for line in lines {
    let item = try JSONSerialization.jsonObject(with:Data(line.utf8)) as! [String:Any]
    let bytes = Data(base64Encoded:item["image"] as! String)!
    let next=(item["stream_epoch"] as? NSNumber)?.uint64Value ?? 0
    if epoch != next {decoder=WindowVideoDecoder();epoch=next}
    if let image = try decoder.image(bytes,alphaPNG:(item["alpha_png"] as? String).flatMap{Data(base64Encoded:$0)}) { count += 1; last = image }
}
precondition(count == lines.count,"Every encoded picture must decode; lost references or latency detected")
let bitmap = NSBitmapImageRep(data:last!.tiffRepresentation!)!
try bitmap.representation(using:.png,properties:[:])!.write(to:URL(fileURLWithPath:CommandLine.arguments[2]))
print("PASS: \(count) H.264 frames decoded, \(last!.size), \(String(format:"%.2f",Date().timeIntervalSince(start))) seconds; Annex B parsing passed")

if CommandLine.arguments.count == 8 {
    let values=CommandLine.arguments[3...7].map { Int($0)! }
    let color=bitmap.colorAt(x:values[0],y:values[1])!
    let actual=[color.redComponent*255,color.greenComponent*255,color.blueComponent*255]
    for i in 0..<3 { precondition(abs(actual[i]-Double(values[i+2])) <= 4,"Color matrix/transfer-function regression") }
    print("PASS: known synthetic color matches within 4/255 per channel: \(actual)")
}

if let text=ProcessInfo.processInfo.environment["KOKODA_EXPECT_ALPHA"],let alpha=Double(text) {
    let actual=bitmap.colorAt(x:bitmap.pixelsWide/2,y:bitmap.pixelsHigh/2)!.alphaComponent*255
    precondition(abs(actual-alpha)<=2,"Native uniform window alpha was not preserved")
    print("PASS: captured native window alpha \(actual)/255")
}
