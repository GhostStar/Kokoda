import AppKit
import VideoToolbox
import CoreImage
import ImageIO

final class WindowVideoDecoder {
    private var session: VTDecompressionSession?
    private var format: CMVideoFormatDescription?
    private var sps = Data(), pps = Data()
    private var decoded: CVPixelBuffer?
    private var decodeStatus: OSStatus = noErr
    private let context = CIContext(options:[.cacheIntermediates:false])
    deinit { if let session = session { VTDecompressionSessionInvalidate(session) } }
    static func units(_ data: Data) -> [Data] {
        let bytes = [UInt8](data)
        var starts: [(Int,Int)] = [], i = 0
        while i+3 <= bytes.count {
            if bytes[i] == 0 && bytes[i+1] == 0 {
                if bytes[i+2] == 1 { starts.append((i,i+3)); i += 3; continue }
                if i+4 <= bytes.count && bytes[i+2] == 0 && bytes[i+3] == 1 { starts.append((i,i+4)); i += 4; continue }
            }
            i += 1
        }
        return starts.enumerated().compactMap { index, range in
            let end = index+1 < starts.count ? starts[index+1].0 : bytes.count
            return end > range.1 ? Data(bytes[range.1..<end]) : nil
        }
    }
    private func check(_ code:OSStatus) throws {
        if code != noErr { throw NSError(domain:"Kokoda.VideoToolbox",code:Int(code)) }
    }
    private var maskData: Data?
    private var maskImage: CIImage?
    static func stillImage(_ data:Data) throws -> NSImage? {
        guard let source=CGImageSourceCreateWithData(data as CFData,nil),
              let props=CGImageSourceCopyPropertiesAtIndex(source,0,nil) as? [CFString:Any],
              let w=props[kCGImagePropertyPixelWidth] as? Int,let h=props[kCGImagePropertyPixelHeight] as? Int,
              w>0,h>0,w<=33177600/h,
              let image=CGImageSourceCreateImageAtIndex(source,0,[kCGImageSourceShouldCacheImmediately:true] as CFDictionary) else {
            throw NSError(domain:"Kokoda.StillImage",code:1)
        }
        return NSImage(cgImage:image,size:NSSize(width:w,height:h))
    }
    static func alphaData(_ value:Any?) throws -> Data? {
        guard let value, !(value is NSNull) else {return nil}
        guard let text=value as? String,text.utf8.count<=8*1024*1024,
              let data=Data(base64Encoded:text),data.count<=6*1024*1024 else {
            throw NSError(domain:"Kokoda.VideoAlpha",code:2)
        }
        return data
    }
    func composite(_ color: CIImage, alphaPNG: Data?) throws -> CIImage {
        guard let alphaPNG else { maskData=nil;maskImage=nil;return color }
        if alphaPNG != maskData {
            guard alphaPNG.count <= 6*1024*1024,
                  let source=CGImageSourceCreateWithData(alphaPNG as CFData,nil),
                  CGImageSourceGetType(source) as String? == "public.png",
                  let properties=CGImageSourceCopyPropertiesAtIndex(source,0,nil) as? [CFString:Any],
                  let w=properties[kCGImagePropertyPixelWidth] as? Int,
                  let h=properties[kCGImagePropertyPixelHeight] as? Int,
                  w>0,h>0,w<=33177600/h,
                  let image=CGImageSourceCreateImageAtIndex(source,0,nil) else {
                throw NSError(domain:"Kokoda.VideoAlpha",code:1)
            }
            maskImage=CIImage(cgImage:image,options:[.colorSpace:NSNull()]);maskData=alphaPNG
        }
        let mask=maskImage!.transformed(by:CGAffineTransform(scaleX:color.extent.width/maskImage!.extent.width,y:color.extent.height/maskImage!.extent.height))
        return color.applyingFilter("CIBlendWithMask",parameters:[kCIInputBackgroundImageKey:CIImage(color:.clear).cropped(to:color.extent),kCIInputMaskImageKey:mask]).cropped(to:color.extent)
    }
    func image(_ data:Data, alphaPNG:Data?=nil) throws -> NSImage? {
        let units = Self.units(data)
        guard !units.isEmpty else { throw NSError(domain:"Kokoda.H264.AnnexB",code:1) }
        let nextSPS = units.first { ($0.first! & 31) == 7 } ?? sps
        let nextPPS = units.first { ($0.first! & 31) == 8 } ?? pps
        if session == nil || nextSPS != sps || nextPPS != pps {
            guard !nextSPS.isEmpty && !nextPPS.isEmpty else { throw NSError(domain:"Kokoda.H264.Parameters",code:1) }
            if let session = session { VTDecompressionSessionInvalidate(session) }
            session = nil; format = nil; sps = nextSPS; pps = nextPPS
            let result = sps.withUnsafeBytes { s in pps.withUnsafeBytes { p -> OSStatus in
                let pointers = [s.bindMemory(to:UInt8.self).baseAddress!,p.bindMemory(to:UInt8.self).baseAddress!]
                let sizes = [sps.count,pps.count]
                return CMVideoFormatDescriptionCreateFromH264ParameterSets(allocator:kCFAllocatorDefault,
                    parameterSetCount:2,parameterSetPointers:pointers,parameterSetSizes:sizes,
                    nalUnitHeaderLength:4,formatDescriptionOut:&format)
            }}
            try check(result)
            var callback = VTDecompressionOutputCallbackRecord(decompressionOutputCallback: { ref,_,status,_,buffer,_,_ in
                guard let ref = ref else { return }
                let decoder = Unmanaged<WindowVideoDecoder>.fromOpaque(ref).takeUnretainedValue()
                decoder.decodeStatus = status
                decoder.decoded = buffer
            },decompressionOutputRefCon:Unmanaged.passUnretained(self).toOpaque())
            let spec = [kVTVideoDecoderSpecification_EnableHardwareAcceleratedVideoDecoder:true] as CFDictionary
            let attributes = [kCVPixelBufferPixelFormatTypeKey:kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange] as CFDictionary
            try check(VTDecompressionSessionCreate(allocator:kCFAllocatorDefault,formatDescription:format!,
                decoderSpecification:spec,imageBufferAttributes:attributes,outputCallback:&callback,decompressionSessionOut:&session))
            if let session = session { VTSessionSetProperty(session,key:kVTDecompressionPropertyKey_RealTime,value:kCFBooleanTrue) }
        }
        var avcc = Data()
        for unit in units where ![7,8,9].contains(unit.first! & 31) {
            var length = UInt32(unit.count).bigEndian
            withUnsafeBytes(of:&length) { avcc.append(contentsOf:$0) }; avcc.append(unit)
        }
        guard !avcc.isEmpty else { return nil }
        var block: CMBlockBuffer?
        try check(CMBlockBufferCreateWithMemoryBlock(allocator:kCFAllocatorDefault,memoryBlock:nil,blockLength:avcc.count,
            blockAllocator:kCFAllocatorDefault,customBlockSource:nil,offsetToData:0,dataLength:avcc.count,flags:0,blockBufferOut:&block))
        try avcc.withUnsafeBytes { bytes in
            try check(CMBlockBufferReplaceDataBytes(with:bytes.baseAddress!,blockBuffer:block!,offsetIntoDestination:0,dataLength:avcc.count))
        }
        var sample: CMSampleBuffer?
        var length = avcc.count
        try check(CMSampleBufferCreateReady(allocator:kCFAllocatorDefault,dataBuffer:block,formatDescription:format,
            sampleCount:1,sampleTimingEntryCount:0,sampleTimingArray:nil,sampleSizeEntryCount:1,sampleSizeArray:&length,sampleBufferOut:&sample))
        decoded = nil; decodeStatus = noErr
        try check(VTDecompressionSessionDecodeFrame(session!,sampleBuffer:sample!,flags:[],frameRefcon:nil,infoFlagsOut:nil))
        try check(VTDecompressionSessionWaitForAsynchronousFrames(session!))
        try check(decodeStatus)
        guard let decoded = decoded else { return nil }
        // The Microsoft software MFT may omit VUI color fields despite media-type
        // attributes. Our wire format explicitly uses sRGB / BT.601 limited NV12.
        // Preserve YUV until Core Image, rather than letting VT guess an HD matrix.
        CVBufferSetAttachment(decoded,kCVImageBufferYCbCrMatrixKey,kCVImageBufferYCbCrMatrix_ITU_R_601_4,.shouldPropagate)
        CVBufferSetAttachment(decoded,kCVImageBufferColorPrimariesKey,kCVImageBufferColorPrimaries_ITU_R_709_2,.shouldPropagate)
        CVBufferSetAttachment(decoded,kCVImageBufferTransferFunctionKey,kCVImageBufferTransferFunction_sRGB,.shouldPropagate)
        let ci = CIImage(cvPixelBuffer:decoded,options:[.colorSpace:CGColorSpace(name:CGColorSpace.sRGB)!])
        let composed = try composite(ci,alphaPNG:alphaPNG)
        guard let cg = context.createCGImage(composed,from:ci.extent,format:.RGBA8,colorSpace:CGColorSpace(name:CGColorSpace.sRGB)) else { return nil }
        return NSImage(cgImage:cg,size:NSSize(width:cg.width,height:cg.height))
    }
}
