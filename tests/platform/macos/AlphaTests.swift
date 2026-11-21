let decoder=WindowVideoDecoder()
let values:[UInt8] = [0,64,128,255]
let pixels=Data(values.flatMap{[$0,$0,$0,255]})
let provider=CGDataProvider(data:pixels as CFData)!
let maskCG=CGImage(width:4,height:1,bitsPerComponent:8,bitsPerPixel:32,bytesPerRow:16,space:CGColorSpace(name:CGColorSpace.sRGB)!,bitmapInfo:CGBitmapInfo(rawValue:CGImageAlphaInfo.last.rawValue),provider:provider,decode:nil,shouldInterpolate:false,intent:.defaultIntent)!
let png=NSBitmapImageRep(cgImage:maskCG).representation(using:.png,properties:[:])!
let color=CIImage(color:CIColor(red:1,green:0,blue:0,alpha:1)).cropped(to:CGRect(x:0,y:0,width:4,height:1))
let context=CIContext()
for _ in 0..<2 {
    let output=try decoder.composite(color,alphaPNG:png)
    var rgba=[UInt8](repeating:0,count:16)
    context.render(output,toBitmap:&rgba,rowBytes:16,bounds:output.extent,format:.RGBA8,colorSpace:CGColorSpace(name:CGColorSpace.sRGB))
    for (x,level) in values.enumerated() {
        precondition(abs(Int(rgba[x*4+3])-Int(level))<=3,"Alpha luminance/cache regression: \(rgba)")
    }
}
let opaque=try decoder.composite(color,alphaPNG:nil)
precondition(opaque.extent==color.extent)
do {_ = try decoder.composite(color,alphaPNG:Data([1,2,3]));preconditionFailure("Invalid mask accepted")} catch {}
let resized=try decoder.composite(color.transformed(by:CGAffineTransform(scaleX:2,y:2)),alphaPNG:png)
precondition(resized.extent.width==8 && resized.extent.height==2)
print("PASS: opaque frames, exact/partial transparency, cached masks, scaled masks and malformed-mask rejection")

let absent=try WindowVideoDecoder.alphaData(nil)
precondition(absent == nil)
do {_ = try WindowVideoDecoder.alphaData("invalid-base64");preconditionFailure("Malformed mask silently accepted")} catch {}
