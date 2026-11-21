// Normalize the supplied artwork for application-icon packaging without redrawing it.
import AppKit
import ImageIO

let arguments=CommandLine.arguments
precondition(arguments.count==3,"Usage: prepare-icon.swift SOURCE OUTPUT")
let source=URL(fileURLWithPath:arguments[1]),destination=URL(fileURLWithPath:arguments[2])
guard let imageSource=CGImageSourceCreateWithURL(source as CFURL,nil),
      let image=CGImageSourceCreateImageAtIndex(imageSource,0,nil),
      let colorSpace=CGColorSpace(name:CGColorSpace.sRGB),
      let scan=CGContext(data:nil,width:image.width,height:image.height,bitsPerComponent:8,
                         bytesPerRow:image.width*4,space:colorSpace,
                         bitmapInfo:CGImageAlphaInfo.premultipliedLast.rawValue) else {fatalError("Cannot read icon")}
scan.draw(image,in:CGRect(x:0,y:0,width:image.width,height:image.height))
let pixels=scan.data!.assumingMemoryBound(to:UInt8.self)
var left=image.width,right=0,bottom=image.height,top=0
for y in 0..<image.height {for x in 0..<image.width {
    // Ignore nearly invisible cutout noise when finding the visual bounds.
    if pixels[(y*image.width+x)*4+3]>8 {
        left=min(left,x);right=max(right,x);bottom=min(bottom,y);top=max(top,y)
    }
}}
precondition(right>left && top>bottom,"Icon has no visible artwork")
// Keep a few source pixels around antialiased edges and shadows.
left=max(0,left-3);right=min(image.width-1,right+3)
bottom=max(0,bottom-3);top=min(image.height-1,top+3)
// Crop the same bitmap that was scanned, avoiding orientation/metadata ambiguity.
guard let bitmap=scan.makeImage(),
      let cropped=bitmap.cropping(to:CGRect(x:left,y:bottom,width:right-left+1,height:top-bottom+1)),
      let canvas=CGContext(data:nil,width:1024,height:1024,bitsPerComponent:8,bytesPerRow:4096,
                           space:colorSpace,bitmapInfo:CGImageAlphaInfo.premultipliedLast.rawValue) else {fatalError("Cannot prepare icon")}
let scale=896.0/Double(max(cropped.width,cropped.height))
let width=Double(cropped.width)*scale,height=Double(cropped.height)*scale
canvas.interpolationQuality = .high
canvas.draw(cropped,in:CGRect(x:(1024-width)/2,y:(1024-height)/2,width:width,height:height))
guard let result=canvas.makeImage(),
      let png=CGImageDestinationCreateWithURL(destination as CFURL,"public.png" as CFString,1,nil) else {fatalError("Cannot write icon")}
CGImageDestinationAddImage(png,result,nil)
precondition(CGImageDestinationFinalize(png),"Cannot finalize PNG")
print("Prepared icon: source bounds \(right-left+1)×\(top-bottom+1), artwork \(Int(width.rounded()))×\(Int(height.rounded())) on transparent 1024×1024 canvas")
