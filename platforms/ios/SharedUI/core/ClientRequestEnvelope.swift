import Foundation
import Darwin

@_silgen_name("msime_client_string_free")
private func msimeClientRequestStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// Send a JSON request to a client-core entry point and return a successful envelope's value.
/// Callers choose whether an unavailable or failed response has any visible effect.
enum ClientRequestEnvelope {
  @discardableResult
  static func call(_ function: (UnsafePointer<UInt8>?, UInt) -> UnsafeMutablePointer<CChar>?,
                   _ request: [String: Any]) -> Any? {
    guard let body = try? JSONSerialization.data(withJSONObject: request) else { return nil }
    let raw = body.withUnsafeBytes { function($0.bindMemory(to: UInt8.self).baseAddress, UInt(body.count)) }
    guard let raw else { return nil }
    defer { msimeClientRequestStringFree(raw) }
    guard let envelope = try? JSONSerialization.jsonObject(with: Data(bytes: raw, count: strlen(raw))) as? [String: Any],
          envelope["ok"] as? Bool == true else { return nil }
    return envelope["value"]
  }
}
