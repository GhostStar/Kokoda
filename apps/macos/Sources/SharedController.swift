import Foundation

// Native views retain AppKit resources; Kokoda's Rust core owns shared viewer/quality state.
final class SharedController {
    private let handle: UnsafeMutableRawPointer
    private(set) var preset = 2
    init() { handle = kokoda_client_new()! }
    deinit { kokoda_client_free(handle) }
    @discardableResult func dispatch(_ event: [String:Any]) -> [String:Any] {
        precondition(Thread.isMainThread)
        let data = try! JSONSerialization.data(withJSONObject: event)
        let input = String(data:data,encoding:.utf8)!
        guard let result = input.withCString({kokoda_client_dispatch(handle,$0)}) else { preconditionFailure("Missing core response") }
        defer {kokoda_client_string_free(result)}
        let response = try! JSONSerialization.jsonObject(with: Data(String(cString:result).utf8)) as! [String:Any]
        precondition(response["error"] == nil,"Invalid native/core contract")
        if let value = response["preset"] as? Int { preset=value }
        return response
    }
    func profile(_ preset:Int)->[String:Any] { dispatch(["type":"profile","preset":preset]) }
    func track(session:UInt64,id:UInt64,generation:UInt64) {dispatch(["type":"track","session":session,"id":id,"generation":generation])}
    func forget(session:UInt64) {dispatch(["type":"forget","session":session])}
    func disconnect() {dispatch(["type":"disconnect"])}
    func quality(_ preset:Int) -> [[String:Any]] { dispatch(["type":"quality","preset":preset])["actions"] as! [[String:Any]] }
}
