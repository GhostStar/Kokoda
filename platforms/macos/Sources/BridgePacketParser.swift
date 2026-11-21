import Foundation

/// Owned by one connection's serial parsing queue. Never used on the UI thread.
final class BridgePacketParser {
    private var buffer=Data()
    func append(_ bytes:Data,receivedAt:Double=ProcessInfo.processInfo.systemUptime) throws -> [[String:Any]] {
        guard buffer.count+bytes.count<=24*1024*1024 else {throw CocoaError(.fileReadTooLarge)}
        buffer.append(bytes)
        var messages:[[String:Any]]=[]
        while let newline=buffer.firstIndex(of:10) {
            let line=buffer[..<newline]
            guard var message=try JSONSerialization.jsonObject(with:line) as? [String:Any] else {throw CocoaError(.propertyListReadCorrupt)}
            buffer.removeSubrange(...newline)
            if message["type"] as? String == "frame",let encoded=message["image"] as? String {
                message["_client_received_at"]=receivedAt
                guard let data=Data(base64Encoded:encoded) else {throw CocoaError(.propertyListReadCorrupt)}
                message["binary_image"]=data;message.removeValue(forKey:"image")
                if let alpha=try WindowVideoDecoder.alphaData(message["alpha_png"]) {message["binary_alpha"]=alpha}
                message.removeValue(forKey:"alpha_png")
            }
            messages.append(message)
        }
        return messages
    }
}
