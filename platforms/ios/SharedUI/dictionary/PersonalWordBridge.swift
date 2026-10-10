import Foundation
import CoreFoundation

extension PersonalWordKind {
  /// The name the Engine's dictionary requests use, which spells the quick-phrase kind in snake case.
  var bridgeName: String { self == .quickPhrase ? "quick_phrase" : rawValue }
  init?(bridgeName: String) { self.init(rawValue: bridgeName == "quick_phrase" ? "quickPhrase" : bridgeName) }
}

extension PersonalWord {
  var bridgeValue: [String: Any] {
    var value: [String: Any] = ["kind": kind.bridgeName, "key": key, "value": self.value, "weight": weight]
    if let source { value["source"] = source.rawValue }
    return value
  }
  init(bridgeValue: [String: Any]) throws {
    guard let raw = bridgeValue["kind"] as? String,
          let kind = PersonalWordKind(bridgeName: raw),
          let key = bridgeValue["key"] as? String, let value = bridgeValue["value"] as? String,
          let weight = bridgeValue["weight"] as? NSNumber,
          let weightValue = SharedNumber.strictInt64(weight) else { throw PersonalDictionaryStore.StoreError.invalidState }
    self.init(kind: kind, key: key, value: value, weight: weightValue,
              source: bridgeValue["source"] as? String == PersonalWordSource.bundled.rawValue ? .bundled : nil)
  }

  /// The word as the Engine will store it as a user word. The source is dropped, so a file or form cannot mark a word bundled; only a row the keyboard listed carries that mark.
  func validated() throws -> Self {
    var user = self
    user.source = nil
    return try Self(bridgeValue: PersonalDictionaryBridge.validateEntry(user.bridgeValue))
  }
}
