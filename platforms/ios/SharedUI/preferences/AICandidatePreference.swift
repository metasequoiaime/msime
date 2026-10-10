import Foundation
import CoreFoundation

/// 「候选栏 AI 候选」: the shared document's `ai_assistant`, which the runtime turns into candidate-bar AI requests, written from the keyboard AI configuration the settings app already saved.
///
/// The desktop keeps the provider key inside `ai_assistant`. On iOS the key lives in the Keychain the keyboard shares with the app, so the document only ever carries where to send and what to ask for; the keyboard hands the key to its own runtime session in memory, and only for the endpoint the document names.
enum AICandidatePreference {
  static let limits = 1...10
  static let defaultLimit = 3

  static func isEnabled(_ preferences: [String: Any]?) -> Bool {
    (preferences?["ai_assistant"] as? [String: Any])?["enabled"] as? Bool ?? false
  }

  static func limit(_ preferences: [String: Any]?) -> Int {
    let value = SharedNumber.strictInt((preferences?["ai_assistant"] as? [String: Any])?["candidate_limit"])
    return value.flatMap { limits.contains($0) ? $0 : nil } ?? defaultLimit
  }

  /// The document's `ai_assistant` after turning the candidate bar on or off for a saved keyboard configuration. Fields this page does not own (prompts, other providers' entries) are kept; a key that reached the document from elsewhere is left alone rather than copied or erased.
  static func assistant(_ existing: [String: Any]?, enabled: Bool, limit: Int,
                        provider: String, endpoint: String, model: String) -> [String: Any] {
    var assistant = existing ?? [:]
    assistant["enabled"] = enabled
    assistant["candidate_limit"] = SharedNumber.clamped(limit, to: limits)
    if enabled {
      assistant["provider"] = provider.lowercased()
      assistant["endpoint"] = endpoint.trimmingCharacters(in: .whitespacesAndNewlines)
      assistant["model"] = model.trimmingCharacters(in: .whitespacesAndNewlines)
    }
    return assistant
  }

  /// The desktop's three prompt slots (自定义一/二/三). `prompt_id` names the one in use; the shared layer sends the built-in prompt when that slot is empty.
  static let promptSlots: [(id: String, title: String)] = [
    ("custom_1", "自定义一"), ("custom_2", "自定义二"), ("custom_3", "自定义三"),
  ]

  static func promptID(_ preferences: [String: Any]?) -> String {
    let id = (preferences?["ai_assistant"] as? [String: Any])?["prompt_id"] as? String
    return promptSlots.first { $0.id == id }?.id ?? promptSlots[0].id
  }

  /// A slot's text; empty means the built-in prompt.
  static func prompt(_ preferences: [String: Any]?, slot: String) -> String {
    (preferences?["ai_assistant"] as? [String: Any])?["prompt_\(slot)"] as? String ?? ""
  }

  /// The document's `ai_assistant` with `slot` in use and holding `text`. Text that is only whitespace is stored empty so the built-in prompt applies.
  static func assistant(_ existing: [String: Any]?, promptSlot slot: String, text: String) -> [String: Any] {
    var assistant = existing ?? [:]
    guard promptSlots.contains(where: { $0.id == slot }) else { return assistant }
    assistant["prompt_id"] = slot
    assistant["prompt_\(slot)"] = text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "" : text
    return assistant
  }

  /// The endpoint a Keychain key may be handed over for: the document's own, and only while the candidate bar is on.
  static func credentialEndpoint(_ preferences: [String: Any]?, configuredEndpoint: String?) -> String? {
    guard isEnabled(preferences),
          let shared = (preferences?["ai_assistant"] as? [String: Any])?["endpoint"] as? String,
          let configuredEndpoint else { return nil }
    let endpoint = shared.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !endpoint.isEmpty,
          endpoint == configuredEndpoint.trimmingCharacters(in: .whitespacesAndNewlines) else { return nil }
    return endpoint
  }
}
