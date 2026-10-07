import CoreFoundation
import Foundation

private typealias MSIMEByte = UInt8

@_silgen_name("msime_client_prepare_host")
private func msimeClientPrepareHost(_ options: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_create")
private func msimeClientCreate(_ options: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_destroy")
private func msimeClientDestroy(_ session: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_focus")
private func msimeClientFocus(_ session: UInt64, _ focused: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientStringFree(_ value: UnsafeMutablePointer<CChar>?)
@_silgen_name("msime_client_character")
private func msimeClientCharacter(_ session: UInt64, _ value: MSIMEByte, _ shift: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_punctuation")
private func msimeClientPunctuation(_ session: UInt64, _ value: MSIMEByte) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_punctuation_with_context")
private func msimeClientPunctuationWithContext(
  _ session: UInt64, _ value: MSIMEByte, _ preceding: UInt32
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_command")
private func msimeClientCommand(_ session: UInt64, _ command: UInt32) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_reset_cache")
private func msimeClientResetCache(_ session: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_select")
private func msimeClientSelect(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_select_any_candidate")
private func msimeClientSelectAnyCandidate(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_select_edge")
private func msimeClientSelectEdge(_ session: UInt64, _ generation: UInt64, _ index: UInt, _ edge: UInt8) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_pin_candidate")
private func msimeClientPinCandidate(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_remove_candidate")
private func msimeClientRemoveCandidate(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_fix_candidate_position")
private func msimeClientFixCandidatePosition(_ session: UInt64, _ generation: UInt64, _ index: UInt, _ position: UInt8) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_clear_candidate_position")
private func msimeClientClearCandidatePosition(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_balance_paired_punctuation_after_auto_close")
private func msimeClientBalancePairedPunctuationAfterAutoClose(_ session: UInt64, _ opening: MSIMEByte) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_smart_punctuation_arm")
private func msimeClientSmartPunctuationArm(_ session: UInt64, _ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_smart_punctuation_decide")
private func msimeClientSmartPunctuationDecide(_ session: UInt64, _ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_shuangpin_key_hints")
private func msimeClientShuangpinKeyHints(_ profile: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_choose_nine_key_spelling")
private func msimeClientChooseNineKeySpelling(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_nine_key_mode")
private func msimeClientSetNineKeyMode(_ session: UInt64, _ enabled: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_chinese_punctuation")
private func msimeClientSetChinesePunctuation(_ session: UInt64, _ enabled: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_ai_credential")
private func msimeClientSetAICredential(_ session: UInt64, _ token: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_character_width")
private func msimeClientSetCharacterWidth(_ session: UInt64, _ fullwidth: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_update_preferences")
private func msimeClientUpdatePreferences(_ session: UInt64, _ snapshot: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_load_preferences")
private func msimeClientLoadPreferences(_ directory: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_save_preferences")
private func msimeClientSavePreferences(
  _ directory: UnsafePointer<MSIMEByte>?, _ directoryLength: UInt, _ expectedRevision: UInt64,
  _ snapshot: UnsafePointer<MSIMEByte>?, _ snapshotLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_view")
private func msimeClientView(_ session: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_all_candidates")
private func msimeClientAllCandidates(_ session: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_english_completions")
private func msimeClientEnglishCompletions(
  _ session: UInt64, _ prefix: UnsafePointer<MSIMEByte>?, _ prefixLength: UInt, _ limit: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_candidate_gloss_request")
private func msimeClientCandidateGlossRequest(
  _ request: UnsafePointer<MSIMEByte>?, _ requestLength: UInt,
  _ resources: UnsafePointer<MSIMEByte>?, _ resourcesLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_emoji_catalog_request")
private func msimeClientEmojiCatalogRequest(
  _ request: UnsafePointer<MSIMEByte>?, _ requestLength: UInt,
  _ resources: UnsafePointer<MSIMEByte>?, _ resourcesLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_apply_translations")
private func msimeClientApplyTranslations(
  _ session: UInt64, _ generation: UInt64,
  _ translations: UnsafePointer<MSIMEByte>?, _ translationsLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_online_query")
private func msimeClientOnlineQuery(_ session: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_cloud_request_url")
private func msimeClientCloudRequestURL(_ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_apply_cloud_response")
private func msimeClientApplyCloudResponse(
  _ session: UInt64, _ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt,
  _ body: UnsafePointer<MSIMEByte>?, _ bodyLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_ai_request_for_query")
private func msimeClientAIRequestForQuery(
  _ session: UInt64, _ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_parse_ai_response")
private func msimeClientParseAIResponse(_ body: UnsafePointer<MSIMEByte>?, _ length: UInt, _ limit: UInt8) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_tencent_translation_http_request")
private func msimeClientTencentTranslationHTTPRequest(_ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_niutrans_translation_http_request")
private func msimeClientNiuTransTranslationHTTPRequest(_ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_custom_translation_http_request")
private func msimeClientCustomTranslationHTTPRequest(_ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_parse_tencent_translation_response")
private func msimeClientParseTencentTranslationResponse(_ body: UnsafePointer<MSIMEByte>?, _ length: UInt, _ expected: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_parse_niutrans_translation_response")
private func msimeClientParseNiuTransTranslationResponse(_ body: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_parse_custom_translation_response")
private func msimeClientParseCustomTranslationResponse(_ body: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_apply_online_candidates")
private func msimeClientApplyOnlineCandidates(
  _ session: UInt64, _ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt,
  _ candidates: UnsafePointer<MSIMEByte>?, _ candidatesLength: UInt, _ source: UInt8
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_dictionary")
private func msimeClientDictionary(_ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_snapshot_version")
private func msimeClientSnapshotVersion(_ options: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_snapshot_activate")
private func msimeClientSnapshotActivate(_ handle: UInt64, _ expected: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

enum MetasequoiaCandidateAction: Equatable {
  /// `position` is 1...5, the range the shared layer accepts and the desktop candidate menu offers.
  case promote, remove, fix(position: UInt8), clearPosition
}

enum MetasequoiaFrequencyAdjustmentMode: UInt8 {
  case pin, halve, linear, promote, disabled
}

struct MetasequoiaInputSnapshot: Equatable, Sendable {
  let isHandled: Bool
  let commitText: String?
  let preedit: String
  let reading: String
  /// 已经选中的那一段，运行时替这个宿主留在组字里而不是立刻上屏。
  ///
  /// 候选只吃掉部分输入时，引擎会继续组字并把选中的那一段交回来。留住它的宿主必须画出来：请求了
  /// 却不画，用户已经选中的字既不在文档里也不在屏幕上，而组字在他按取消之前不会结束。
  let phrasePrefix: String
  let candidates: [String]
  let candidateCodes: [String]
  let candidateGlosses: [String]
  /// The Engine's display suffix for each candidate, aligned with `candidates`: its helpcode when the scheme's "show helpcode" setting is on, the spelling a typo correction replaced, or a Korean Hanja's 훈음. Never part of the committed text.
  let candidateAnnotations: [String]
  /// The Engine `CandidateSource` (cloud, AI, dictionary...) and pinned slot of each candidate, aligned with `candidates`; a source of -1 means the row carried none, and a fixed position of zero is a word ranked by use.
  let candidateSources: [Int]
  let candidateFixedPositions: [Int]
  let candidatePageCount: Int
  let answeredByPinyinFallback: Bool
  let diagnosticText: String?
  /// Engine's own local-mode name, carried rather than asked for again.
  ///
  /// Every field below this point was already in the response this snapshot was decoded from. The
  /// keyboard used to drop them and then call back through the C ABI for each one, which
  /// serialises the whole view to JSON in Rust and parses it again in Swift - a keystroke was
  /// paying for that several times over.
  let localMode: String
  let nineKeySpellings: [String]
  /// The Engine's ASCII spelling and the caret inside it, as a byte offset. The caret leaves the end only when the user moves it (dragging the space bar while composing), which is the Windows host's ← / → editing of the input string.
  let editingText: String
  let caretPosition: Int

  var isInLocalMode: Bool { !localMode.isEmpty && localMode != "none" }

  /// The spelling with a bar where the caret sits, or nil while the caret is at the end, where the strip keeps showing the segmented pinyin. Windows draws the same caret inside the composition. The offset indexes characters directly because the editing text is ASCII; anything else is not split.
  var editingTextWithCaret: String? {
    guard caretPosition >= 0, caretPosition < editingText.count, editingText.allSatisfy(\.isASCII) else { return nil }
    let caret = editingText.index(editingText.startIndex, offsetBy: caretPosition)
    return editingText[..<caret] + "|" + editingText[caret...]
  }

  init(isHandled: Bool = false, commitText: String? = nil, preedit: String = "", reading: String = "",
       phrasePrefix: String = "",
       candidates: [String] = [], candidateCodes: [String] = [], candidateGlosses: [String] = [],
       candidateAnnotations: [String] = [], candidateSources: [Int] = [], candidateFixedPositions: [Int] = [],
       candidatePageCount: Int = 0, answeredByPinyinFallback: Bool = false,
       diagnosticText: String? = nil, localMode: String = "none",
       nineKeySpellings: [String] = [], editingText: String = "", caretPosition: Int = 0) {
    self.isHandled = isHandled
    self.commitText = commitText
    self.preedit = preedit
    self.reading = reading
    self.phrasePrefix = phrasePrefix
    self.candidates = candidates
    self.candidateCodes = candidateCodes
    self.candidateGlosses = candidateGlosses
    self.candidateAnnotations = candidateAnnotations
    self.candidateSources = candidateSources
    self.candidateFixedPositions = candidateFixedPositions
    self.candidatePageCount = candidatePageCount
    self.answeredByPinyinFallback = answeredByPinyinFallback
    self.diagnosticText = diagnosticText
    self.localMode = localMode
    self.nineKeySpellings = nineKeySpellings
    self.editingText = editingText
    self.caretPosition = caretPosition
  }
}

private enum InputBridgeFailure: LocalizedError {
  case response(String)
  case invalidResponse
  case unavailable

  var errorDescription: String? {
    switch self {
    case .response(let message): return message
    case .invalidResponse: return "输入运行时返回了无效响应。"
    case .unavailable: return "输入运行时尚未准备完成。"
    }
  }
}

/// Swift keyboard host for the shared Rust/C ABI. The extension owns one session
/// and never keeps Engine pointers or input text outside the returned snapshot.
final class MetasequoiaInputSessionBridge: @unchecked Sendable {
  private var handle: UInt64 = 0
  private var options: [String: Any]
  private var stateRoot: String?
  private var initializationDiagnostic: String?
  private var revision: UInt64 = 0
  /// The shared document's revision as this bridge last saw it.
  ///
  /// Kept apart from `revision`, which counts the snapshots handed to the session: every local change bumps that one, so a document the settings app had just saved - one step past what the keyboard last read - compared lower and was dropped as stale.
  private var documentRevision: UInt64 = 0
  private var appliedFuzzyPinyinRules: UInt32?
  private var suspended = false
  // Nine-key lives on the session, not in the preferences the options carry, so a rebuilt session
  // starts back on the 26-key layout unless it is told again.
  private var nineKeyEnabled = false
  // The width, like nine-key, is session state the options do not carry, so a rebuilt session is told it again.
  private var fullwidth = false
  /// The keyboard's 中文标点 switch; nil until it is first set, so a rebuilt session keeps the document's value.
  private var chinesePunctuation: Bool?
  /// Kept only in memory so a recreated focused session gets it back; it never reaches the shared document.
  private var aiCredential: String?

  init(resources: URL? = nil, stateRoot: URL? = nil) {
    options = [:]
    self.stateRoot = nil
    self.appliedFuzzyPinyinRules = nil
    do {
      let bootstrap = Self.bootstrapOptions(resources: resources, stateRoot: stateRoot)
      options = try Self.callOptions(msimeClientPrepareHost,
                                     bootstrap)
      self.stateRoot = options["preferences_directory"] as? String
        ?? bootstrap["state_root"] as? String
      options["preferences"] = Self.hostOverrides(
        applyingTo: options["preferences"] as? [String: Any] ?? [:])
    } catch {
      // 不吞掉底层原因。准备失败的理由几乎全是设备侧才成立的（资源校验、词库换代时的复制、共享容器不可用），
      // 只留一句中文的话，设备上就再也没有别的地方能读到它。
      initializationDiagnostic = "输入运行时准备失败：\(error.localizedDescription)"
      return
    }
    do {
      try createFocusedSession()
    } catch {
      initializationDiagnostic = "输入运行时创建或激活失败：\(error.localizedDescription)"
    }
  }

  deinit {
    if handle != 0 { _ = try? Self.decode(msimeClientDestroy(handle)) }
  }

  /// The directory this session reads its preference document from; nil when preparing the runtime failed.
  var stateDirectory: String? { stateRoot }
  /// Whether the runtime failed to prepare or start, the case the diagnostic log records without its (possibly path-bearing) reason.
  var initializationFailed: Bool { initializationDiagnostic != nil }

  var isInLocalMode: Bool {
    guard let mode = try? localMode() else { return false }
    return !mode.isEmpty && mode != "none"
  }

  /// 组字中或本地模式里，Engine 把这个单个 ASCII 符号列在 `spelling_symbols` 里时它是 Engine 的输入，要作为字符交给会话，而不是选候选或标点：网址模式的数字和网址符号、`www` 之后的 `.`、U 模式的十六进制数字。没有组字时列出的 `/` 和 `@` 不在此列。
  func engineSpellsWhileComposing(_ symbol: String) -> Bool {
    guard symbol.count == 1, let scalar = symbol.unicodeScalars.first,
          scalar.value > 0x20, scalar.value < 0x7f,
          let current = try? view() else { return false }
    let mode = current["local_mode"] as? String ?? "none"
    let editing = current["editing_text"] as? String ?? ""
    let symbols = current["spelling_symbols"] as? String ?? ""
    return (mode != "none" || !editing.isEmpty) && symbols.contains(symbol)
  }

  /// Reload the canonical PreferencesStore written by the Tauri settings host.
  /// Disk and lock work stays off the keyboard thread; the accepted snapshot is
  /// applied on the session's owning (main) thread before the callback returns.
  func reloadSharedPreferences(completion: @escaping (Bool) -> Void) {
    guard handle != 0, let stateRoot else { completion(false); return }
    let path = Data(stateRoot.utf8)
    // Keep the bridge alive until the main-thread callback has applied the snapshot. A weak capture here can drop the bridge while the worker is reading, leaving callers waiting forever for a completion that is never delivered. Capturing it strongly in both closures also keeps the final release on the main queue, where the session handle is owned.
    // The visible keyboard waits on this read, so it runs at userInitiated: Darwin throttles utility-QoS disk I/O whenever other I/O is busy, and that stalled the lock and the small document read for over 15 seconds on a loaded simulator.
    DispatchQueue.global(qos: .userInitiated).async { [self, path] in
      let snapshot: [String: Any]?
      do {
        snapshot = try path.withUnsafeBytes { bytes in
          try MetasequoiaInputSessionBridge.decode(msimeClientLoadPreferences(
            bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(path.count))) as? [String: Any]
        }
      } catch {
        snapshot = nil
      }
      DispatchQueue.main.async { [self] in
        guard let snapshot,
              let preferences = snapshot["preferences"] as? [String: Any],
              let revision = snapshot["revision"] as? NSNumber else {
          completion(false)
          return
        }
        guard let revision = Self.strictUInt64(revision), revision >= self.documentRevision else {
          completion(false)
          return
        }
        // Keep the session's engine configuration stable while the keyboard is
        // visible; applyLearningPreferences will update only the fuzzy-pinyin
        // contract below. This avoids changing the selected scheme underneath
        // UIKit while a Tauri settings write is being observed.
        let applied = (self.options["preferences"] as? [String: Any]) ?? [:]
        let overridden = Self.hostOverrides(applyingTo: preferences)
        self.options["preferences"] = overridden
        self.revision = max(self.revision, revision)
        self.documentRevision = revision
        self.applyAppEditedPreferences(from: overridden, over: applied)
        completion(true)
      }
    }
  }

  /// The fields the settings app edits on its punctuation, candidate, helpcode and local-mode pages.
  ///
  /// Unlike the scheme, the session applies them as soon as it is idle rather than when it is built, so a change made in the settings app has to reach the live session: the keyboard extension process outlives many appearances, and waiting for its next session meant a switch the user had just turned off kept working.
  private static let appEditedKeys = [
    "smart_punctuation", "smart_punctuation_repeat", "smart_punctuation_space_convert",
    "smart_punctuation_direct_digit", "smart_punctuation_direct_letter",
    "paired_punctuation", "punctuation_lock",
    // 「双拼显示原始按键」 on the candidate page: the session rebuilds its Engine with it once idle, like the punctuation fields.
    "shuangpin_preedit_uses_raw",
    // Whole objects: the app merges single fields into them, and the document's copy is the one it wrote.
    "quanpin", "mixed_input", "quanpin_helpcode", "shuangpin_helpcode", "local_modes",
    // 整句联想 on the 输入 page: the session attaches or drops the keyboard sentence model once idle, so turning 增强 off stops it in the keyboard that is already open.
    "sentence_association",
    // Laid over the document by `hostOverrides` from the iOS switch, so a change to that switch reaches the live session too.
    "cloud_candidates",
    // Laid over the document from the iOS page size, like cloud candidates; the session applies it once idle, so an open composition keeps its page.
    "candidate_page_size",
  ]

  /// Hand the reloaded app-edited fields to the session, leaving every other field as the session has it. The session queues the change behind an open composition, so this never interrupts typing.
  private func applyAppEditedPreferences(from reloaded: [String: Any], over applied: [String: Any]) {
    var next = applied
    for key in Self.appEditedKeys { next[key] = reloaded[key] }
    guard handle != 0, !NSDictionary(dictionary: next).isEqual(to: applied) else { return }
    revision &+= 1
    let snapshot: [String: Any] = ["format_version": 1, "revision": revision, "preferences": next]
    _ = try? Self.callUpdate(msimeClientUpdatePreferences, handle, snapshot)
  }

  var fuzzyPinyinRulesApplied: UInt32? { appliedFuzzyPinyinRules }

  /// The latest canonical PreferencesStore document, exposed as a read-only
  /// value for native UI settings that still use App Group compatibility keys.
  /// Callers must remain on the session's owning thread when reading it.
  var sharedPreferences: [String: Any]? {
    options["preferences"] as? [String: Any]
  }

  /// Show touch keyboard geometry on the live session while it is being dragged. The shared snapshot is the source that is reloaded when the extension appears.
  @discardableResult
  func setTouchKeyboardGeometry(keySpacing: Double, rowSpacing: Double,
                                heightAdjustment: Double, voiceEnabled: Bool) -> Bool {
    guard let mapping = Self.geometryMapping(keySpacing: keySpacing, rowSpacing: rowSpacing,
                                             heightAdjustment: heightAdjustment,
                                             voiceEnabled: voiceEnabled) else { return false }
    return updatePreferences(mapping)
  }

  /// Commit the geometry the drag has been previewing.
  ///
  /// The drag emits on every gesture frame, so only the live session follows it; the shared
  /// document is written once, when the user lets go of the grip or flips the switch.
  @discardableResult
  func persistTouchKeyboardGeometry(keySpacing: Double, rowSpacing: Double,
                                    heightAdjustment: Double, voiceEnabled: Bool) -> Bool {
    guard let mapping = Self.geometryMapping(keySpacing: keySpacing, rowSpacing: rowSpacing,
                                             heightAdjustment: heightAdjustment,
                                             voiceEnabled: voiceEnabled) else { return false }
    return updateAndPersist(mapping)
  }

  static func geometryMapping(keySpacing: Double, rowSpacing: Double,
                                      heightAdjustment: Double,
                                      voiceEnabled: Bool) -> ((inout [String: Any]) -> Void)? {
    guard keySpacing.isFinite, rowSpacing.isFinite, heightAdjustment.isFinite else { return nil }
    let keySpacingTenths = Int((min(6, max(3, keySpacing)) * 10).rounded())
    let rowSpacingTenths = Int((min(10, max(4, rowSpacing)) * 10).rounded())
    let clampedHeight = Int(min(48, max(-12, heightAdjustment)).rounded())
    return { preferences in
      preferences["touch_key_spacing_tenths"] = keySpacingTenths
      preferences["touch_row_spacing_tenths"] = rowSpacingTenths
      preferences["touch_keyboard_height_adjustment"] = clampedHeight
      preferences["touch_voice_shortcut"] = voiceEnabled
    }
  }

  /// Remove touch geometry overrides so canonical defaults are used again.
  @discardableResult
  func resetTouchKeyboardGeometry() -> Bool {
    updateAndPersist { preferences in
      preferences.removeValue(forKey: "touch_key_spacing_tenths")
      preferences.removeValue(forKey: "touch_row_spacing_tenths")
      preferences.removeValue(forKey: "touch_keyboard_height_adjustment")
      preferences.removeValue(forKey: "touch_voice_shortcut")
    }
  }

  /// Persist the selected touch scheme and its presentation mapping in one canonical snapshot.
  @discardableResult
  func setTouchKeyboardScheme(_ scheme: ChineseInputScheme,
                              enabledSchemes: [ChineseInputScheme]) -> Bool {
    guard let mapping = Self.schemeMapping(scheme, enabledSchemes: enabledSchemes) else { return false }
    return updateAndPersist(mapping)
  }

  /// The document fields a scheme selection writes; nil when no scheme is enabled. The settings app writes the same fields, so the keyboard does not put its own older selection back.
  static func schemeMapping(_ scheme: ChineseInputScheme,
                            enabledSchemes: [ChineseInputScheme]) -> ((inout [String: Any]) -> Void)? {
    // 本版本不提供的入口不写进共享文档：写进去的方案 host-api 会按版本回退，文档里记的和 Engine 跑的就对不上了。full 提供全部入口，这里不过滤任何东西。
    let enabled = ChineseInputScheme.allCases.filter { enabledSchemes.contains($0) && $0.isOfferedByEdition }
    guard !enabled.isEmpty else { return nil }
    let selected = enabled.contains(scheme) ? scheme : enabled[0]
    // 手写写的是本版本的默认方案（full 是全拼）：识别由平台识别器完成，手写面板背后的 Engine 只需要跑一个本版本提供的方案，否则 host-api 会把它当作本版本不含的方案回退，偏好里记的和 Engine 跑的就对不上了。
    let engineScheme = selected == .handwriting ? MSIMEAppEdition.defaultScheme : selected.engineScheme
    let layout: String
    switch selected {
    case .nineKey, .japaneseNineKey: layout = "nine_key"
    case .handwriting: layout = "handwriting"
    default: layout = "twenty_six_key"
    }
    // Spelled the way the shared schema spells them. The Swift raw values stay camel case for the
    // App Group mirror, and a document that carries those instead is rejected outright - which
    // silently failed every scheme change the keyboard made, so the selection never left the live
    // session and every new session started on the layout the document still held.
    let selectedID = selected.sharedIdentifier
    let mapping: (inout [String: Any]) -> Void = { preferences in
      preferences["scheme"] = engineScheme
      // `last_chinese_scheme` 是切回中文时要回到的方案，所以写其他语言的日语、韩语、越南语和藏文从不替换它；粤拼、注音和笔画是中文方案，会替换。
      if !["japanese", "korean", "vietnamese", "tibetan"].contains(engineScheme) {
        preferences["last_chinese_scheme"] = engineScheme
      }
      if let profile = selected.shuangpinProfile {
        preferences["shuangpin_profile"] = profile
      }
      // 五笔版本 `wubi_profile` 不随方案选择写入：文档里已有的版本原样保留，切到五笔就沿用它；只有设置页的版本选项会改它（`WubiProfilePreference.save`）。
      preferences["touch_keyboard_layout"] = layout
      preferences["touch_keyboard_schemes"] = [
        "enabled": enabled.map(\.sharedIdentifier),
        "selected": selectedID,
      ]
    }
    return mapping
  }

  /// Apply a selection the user made in this keyboard to both places it has to hold.
  ///
  /// The live session answers this keyboard; the shared document answers every session created
  /// after it, including the ones this bridge rebuilds for dictionary maintenance. A selection
  /// that reached only the first was forgotten the moment the session went away.
  @discardableResult
  private func updateAndPersist(_ mutate: (inout [String: Any]) -> Void) -> Bool {
    guard updatePreferences(mutate) else { return false }
    persistSharedPreferences(mutate)
    return true
  }

  /// Record a host-owned selection in the document the next session is created from.
  ///
  /// The live session keeps the selection until it is destroyed, but a rebuilt one is created from
  /// this file, and iOS had never written the touch layout to it: a cold keyboard created its
  /// session on 26 keys whatever the user had picked, and every reload of this document put the
  /// stale layout back. Only the fields the mapping touches are written - the rest of the document,
  /// including the values this host overrides for its own session, is left as the settings app
  /// wrote it. A lost compare-and-swap leaves the live session alone; the next selection retries.
  @discardableResult
  private func persistSharedPreferences(_ mutate: (inout [String: Any]) -> Void) -> Bool {
    guard let stateRoot,
          let revision = Self.persistSharedPreferences(stateRoot: stateRoot, mutate) else { return false }
    self.revision = max(self.revision, revision)
    documentRevision = max(documentRevision, revision)
    return true
  }

  /// The shared document as the settings app reads it: without a session, so opening a settings page does not load the Engine.
  ///
  /// `stateRoot` is for tests; the app and the keyboard share the App Group one.
  static func loadSharedPreferences(stateRoot: URL? = nil) -> [String: Any]? {
    let directory = Data(sharedStateRoot(stateRoot).utf8)
    guard !directory.isEmpty, directory.count <= 16_384 else { return nil }
    return (try? callDirectory(msimeClientLoadPreferences, directory))?["preferences"] as? [String: Any]
  }

  /// Change fields of the shared document from the settings app.
  ///
  /// The keyboard picks the change up the next time it appears (`reloadSharedPreferences`). A write that loses the compare-and-swap to the keyboard returns false and changes nothing.
  @discardableResult
  static func updateSharedPreferences(stateRoot: URL? = nil,
                                      _ mutate: (inout [String: Any]) -> Void) -> Bool {
    persistSharedPreferences(stateRoot: sharedStateRoot(stateRoot), mutate) != nil
  }

  /// `msime_client_save_preferences`'s snapshot bound: large enough for a custom skin's photo.
  private static let preferencesDocumentLimit = 1_048_576

  /// The App Group directory holding the shared preference document, where the keyboard also keeps its diagnostic log.
  static var sharedStateDirectory: String { sharedStateRoot(nil) }

  private static func sharedStateRoot(_ override: URL?) -> String {
    bootstrapOptions(resources: nil, stateRoot: override)["state_root"] as? String ?? ""
  }

  /// Returns the revision the document is at afterwards, or nil when nothing was written.
  private static func persistSharedPreferences(stateRoot: String,
                                               _ mutate: (inout [String: Any]) -> Void) -> UInt64? {
    let directory = Data(stateRoot.utf8)
    guard !directory.isEmpty, directory.count <= 16_384 else { return nil }
    guard let stored = try? callDirectory(msimeClientLoadPreferences, directory),
          let storedRevision = Self.strictUInt64(stored["revision"]),
          let previous = stored["preferences"] as? [String: Any] else { return nil }
    var preferences = previous
    mutate(&preferences)
    guard !NSDictionary(dictionary: preferences).isEqual(to: previous) else {
      return storedRevision
    }
    let document: [String: Any] = ["format_version": 1, "revision": storedRevision,
                                   "preferences": preferences]
    guard JSONSerialization.isValidJSONObject(document),
          let snapshot = try? JSONSerialization.data(withJSONObject: document),
          snapshot.count <= Self.preferencesDocumentLimit else { return nil }
    let saved: [String: Any]
    do {
      saved = try directory.withUnsafeBytes { directoryBytes -> [String: Any] in
        try snapshot.withUnsafeBytes { snapshotBytes -> [String: Any] in
          let value = try decode(msimeClientSavePreferences(
            directoryBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(directory.count),
            storedRevision,
            snapshotBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(snapshot.count)))
          guard let dictionary = value as? [String: Any] else {
            throw InputBridgeFailure.invalidResponse
          }
          return dictionary
        }
      }
    } catch {
      return nil
    }
    return Self.strictUInt64(saved["revision"]) ?? storedRevision
  }

  /// Apply a global theme change made in this keyboard (a GlobalThemePreference mapping) to the live session and the shared document. The caller mirrors the document into the App Group afterwards.
  @discardableResult
  func updateTheme(_ mapping: (inout [String: Any]) -> Void) -> Bool {
    updateAndPersist(mapping)
  }

  /// Persist the touch host's Chinese output mode in the canonical snapshot.
  @discardableResult
  func setTraditionalChineseOutput(_ enabled: Bool) -> Bool {
    updateAndPersist { preferences in
      preferences["traditional_chinese_output"] = enabled
    }
  }

  func handleCharacter(_ character: String, shifted: Bool = false) -> MetasequoiaInputSnapshot {
    dispatch { pointer(for: character, shift: shifted) }
  }

  func handleCandidateKey(_ character: String) -> MetasequoiaInputSnapshot {
    guard let index = Int(character), (1...9).contains(index) else { return diagnostic("候选编号无效") }
    return selectCandidate(at: UInt(index - 1))
  }

  func handlePunctuation(_ character: String) -> MetasequoiaInputSnapshot {
    guard let byte = Self.ascii(character) else { return diagnostic("标点输入无效") }
    return dispatch { msimeClientPunctuation(handle, byte) }
  }

  func handlePunctuationWithContext(_ character: String,
                                    preceding: UInt32) -> MetasequoiaInputSnapshot {
    guard let byte = Self.ascii(character) else { return diagnostic("标点输入无效") }
    return dispatch { msimeClientPunctuationWithContext(handle, byte, preceding) }
  }

  /// Tell the Engine the keyboard wrote the closing half of a pair it opened. Only book titles need it: the Engine nests 《 then 〈 until it sees a 》, and an auto-closed 》 never passes through it, so without this the next < would open 〈.
  @discardableResult
  func balancePairedPunctuationAfterAutoClose(opening: String) -> Bool {
    guard let byte = Self.ascii(opening), handle != 0 else { return false }
    return (try? Self.decode(msimeClientBalancePairedPunctuationAfterAutoClose(handle, byte))) != nil
  }

  /// What the commit just made arms, if anything.
  ///
  /// The snapshots belong to the host's editor rather than to Engine, so the keyboard holds them
  /// and hands them back on the next press. The switches that gate them live in the shared
  /// preferences, which is why this asks the session instead of reading a second copy here.
  func smartPunctuationArming(ascii: String, commit: String, timestampMilliseconds: UInt64,
                              editorGeneration: UInt64,
                              autoClosedPair: Bool) -> [String: Any] {
    guard let byte = Self.ascii(ascii), handle != 0 else { return [:] }
    let request: [String: Any] = ["ascii": byte, "commit": commit,
                                  "timestamp_ms": timestampMilliseconds,
                                  "editor_generation": editorGeneration,
                                  "auto_closed_pair": autoClosedPair]
    return (try? Self.callUpdate(msimeClientSmartPunctuationArm, handle, request)) ?? [:]
  }

  /// What this press should do about a previously armed gesture.
  ///
  /// `preceding` is what the editor holds before the caret right now. Both decisions re-read it
  /// and decline when it disagrees with the arming, so a snapshot left over from an edit the
  /// keyboard never saw cannot rewrite the wrong character.
  func smartPunctuationDecision(character: String, preceding: String?,
                                timestampMilliseconds: UInt64, editorGeneration: UInt64,
                                repeatSnapshot: Any?, spaceSnapshot: Any?) -> [String: Any] {
    guard let byte = Self.ascii(character), handle != 0 else { return [:] }
    let request: [String: Any] = ["character": byte, "preceding": preceding ?? NSNull(),
                                  "timestamp_ms": timestampMilliseconds,
                                  "editor_generation": editorGeneration,
                                  "repeat": repeatSnapshot ?? NSNull(),
                                  "space": spaceSnapshot ?? NSNull()]
    return (try? Self.callUpdate(msimeClientSmartPunctuationDecide, handle, request)) ?? [:]
  }

  func handleBackspace() -> MetasequoiaInputSnapshot { command(0) }
  func commitCandidate() -> MetasequoiaInputSnapshot { command(1) }
  func commitRaw() -> MetasequoiaInputSnapshot { command(2) }
  func cancel() -> MetasequoiaInputSnapshot { command(3) }
  func finishComposition() -> MetasequoiaInputSnapshot { command(9) }
  func cycleKanaVariant() -> MetasequoiaInputSnapshot { command(10) }
  func commitReading() -> MetasequoiaInputSnapshot { command(11) }
  func moveCaretLeft() -> MetasequoiaInputSnapshot { command(4) }
  func moveCaretRight() -> MetasequoiaInputSnapshot { command(5) }
  /// The segment edits the Windows composition binds to Ctrl+Backspace and Ctrl+← / →: a whole syllable (or a held phrase) at a time, on the unit boundaries the Engine owns.
  func segmentBackspace() -> MetasequoiaInputSnapshot { command(12) }
  func moveCaretLeftBySegment() -> MetasequoiaInputSnapshot { command(13) }
  func moveCaretRightBySegment() -> MetasequoiaInputSnapshot { command(14) }
  /// The Home, End and Delete keys of the Windows composition: the caret to either end of the spelling, and the letter after the caret removed.
  func moveCaretToStart() -> MetasequoiaInputSnapshot { command(6) }
  func moveCaretToEnd() -> MetasequoiaInputSnapshot { command(7) }
  func deleteForward() -> MetasequoiaInputSnapshot { command(8) }
  /// MSIME_CONVERT_HANJA (msime_client.h): lists the Hanja of the composing Korean syllable as candidates, or closes that list when it is open. Unhandled when nothing is composing or the composition is a lone jamo, which has no Hanja.
  func convertHanja() -> MetasequoiaInputSnapshot { command(16) }
  /// Drops the Engine's cached candidate lookups, which Windows does on Ctrl+Shift+Alt+C.
  func resetCache() -> MetasequoiaInputSnapshot { dispatch { msimeClientResetCache(handle) } }

  func selectCandidate(at index: UInt) -> MetasequoiaInputSnapshot {
    guard let rows = try? currentCandidates(), rows.indices.contains(Int(index)),
          let identity = rows[Int(index)]["id"] as? [String: Any],
          let generation = Self.strictUInt64(identity["generation"]),
          let globalIndex = Self.strictUInt64(identity["index"]) else { return diagnostic("候选已失效") }
    return selectCandidate(
      generation: generation, globalIndex: globalIndex)
  }

  func selectCandidate(generation: UInt64, globalIndex: UInt64) -> MetasequoiaInputSnapshot {
    guard let index = UInt(exactly: globalIndex) else { return diagnostic("候选已失效") }
    return dispatch { msimeClientSelect(handle, generation, index) }
  }

  /// Select an entry of the expanded panel. Panel positions index the Engine's whole answer, and `selectCandidate(generation:globalIndex:)` only accepts the page the strip is showing, so anything past the ninth candidate came back as stale.
  func selectAnyCandidate(generation: UInt64, globalIndex: UInt64) -> MetasequoiaInputSnapshot {
    guard let index = UInt(exactly: globalIndex) else { return diagnostic("候选已失效") }
    return dispatch { msimeClientSelectAnyCandidate(handle, generation, index) }
  }

  /// Whether a whole-answer candidate sits on the page the strip is showing. Pin, remove, fix and 以词定字 are page-bounded in the runtime, so the expanded panel offers them only for these entries.
  func isOnCurrentPage(generation: UInt64, globalIndex: UInt64) -> Bool {
    guard let rows = try? currentCandidates() else { return false }
    return rows.contains { row in
      guard let identity = row["id"] as? [String: Any] else { return false }
      return Self.strictUInt64(identity["generation"]) == generation
        && Self.strictUInt64(identity["index"]) == globalIndex
    }
  }

  /// Commit only the first or the last Han character of a candidate (以词定字); the Engine ends the composition with it.
  func selectCandidateEdge(at index: UInt, last: Bool) -> MetasequoiaInputSnapshot {
    guard let rows = try? currentCandidates(), rows.indices.contains(Int(index)),
          let identity = rows[Int(index)]["id"] as? [String: Any],
          let generation = Self.strictUInt64(identity["generation"]),
          let globalIndex = Self.strictUInt64(identity["index"]) else { return diagnostic("候选已失效") }
    return selectCandidateEdge(generation: generation, globalIndex: globalIndex, last: last)
  }

  func selectCandidateEdge(generation: UInt64, globalIndex: UInt64, last: Bool) -> MetasequoiaInputSnapshot {
    guard let index = UInt(exactly: globalIndex) else { return diagnostic("候选已失效") }
    return dispatch { msimeClientSelectEdge(handle, generation, index, last ? 1 : 0) }
  }

  func allCandidates() throws -> [String: Any] {
    try Self.callHandle(msimeClientAllCandidates, handle)
  }

  /// Read-only English completion lookup for the word immediately before the cursor.
  /// The caller owns the document-context parsing; the Engine session remains untouched.
  func englishCompletions(forPrefix prefix: String, limit: Int) -> [String] {
    guard handle != 0, (1...32).contains(limit),
          !prefix.isEmpty, prefix.utf8.count <= 64,
          prefix.utf8.allSatisfy({ ($0 >= 65 && $0 <= 90) || ($0 >= 97 && $0 <= 122) })
    else { return [] }
    let bytes = Array(prefix.utf8)
    do {
      let response: Any = try bytes.withUnsafeBufferPointer { buffer in
        try Self.decode(msimeClientEnglishCompletions(
          handle, buffer.baseAddress, UInt(buffer.count), UInt(limit)))
      }
      guard let value = response as? [String: Any],
            let completions = value["completions"] as? [String] else { return [] }
      return completions
    } catch {
      return []
    }
  }

  func candidateGlossResources() -> String? {
    options["resources"] as? String
  }

  static func candidateGlosses(request: Data, resources: String) throws -> [String: Any] {
    let resourceData = Data(resources.utf8)
    guard NSString(string: resources).isAbsolutePath, resourceData.count <= 4096 else {
      throw InputBridgeFailure.invalidResponse
    }
    return try request.withUnsafeBytes { requestBytes in
      try resourceData.withUnsafeBytes { resourceBytes in
        let value = try decode(msimeClientCandidateGlossRequest(
          requestBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(request.count),
          resourceBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(resourceData.count)))
        guard let dictionary = value as? [String: Any] else { throw InputBridgeFailure.invalidResponse }
        return dictionary
      }
    }
  }

  /// Reads one bounded page from the verified packaged Emoji catalog. This has no live-session
  /// dependency and is safe to invoke from the keyboard's serial catalog worker.
  static func emojiCatalog(request: Data, resources: String) throws -> [String: Any] {
    let resourceData = Data(resources.utf8)
    guard !request.isEmpty, request.count <= 16_384,
          NSString(string: resources).isAbsolutePath, resourceData.count <= 4096 else {
      throw InputBridgeFailure.invalidResponse
    }
    return try request.withUnsafeBytes { requestBytes in
      try resourceData.withUnsafeBytes { resourceBytes in
        let value = try decode(msimeClientEmojiCatalogRequest(
          requestBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(request.count),
          resourceBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(resourceData.count)))
        guard let dictionary = value as? [String: Any] else {
          throw InputBridgeFailure.invalidResponse
        }
        return dictionary
      }
    }
  }

  func applyTranslations(generation: UInt64, translations: Data) throws -> [String: Any] {
    try translations.withUnsafeBytes { bytes in
      let value = try Self.decode(msimeClientApplyTranslations(
        handle, generation, bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(translations.count)))
      guard let dictionary = value as? [String: Any] else { throw InputBridgeFailure.invalidResponse }
      return dictionary
    }
  }

  /// The composition the cloud and AI providers could answer, as the session's JSON document, or nil when neither could.
  ///
  /// The document is handed back unchanged with a provider's result; the session refuses a result for a composition that has since moved on.
  func onlineQuery() -> Data? {
    guard handle != 0, let value = try? Self.decode(msimeClientOnlineQuery(handle)),
          let query = value as? [String: Any],
          let data = try? JSONSerialization.data(withJSONObject: query) else { return nil }
    return data
  }

  /// The HTTPS cloud candidate URL the shared host builds for an eligible query.
  static func cloudRequestURL(query: Data) -> URL? {
    guard !query.isEmpty, query.count <= 16_384 else { return nil }
    let value = query.withUnsafeBytes { bytes in
      try? decode(msimeClientCloudRequestURL(bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(query.count)))
    }
    guard let text = value as? String, let url = URL(string: text), url.scheme == "https" else { return nil }
    return url
  }

  /// Hand a fetched cloud reply to the shared parser. Returns `{applied, view}`.
  func applyCloudResponse(query: Data, body: Data) throws -> [String: Any] {
    guard handle != 0 else { throw InputBridgeFailure.unavailable }
    guard !query.isEmpty, query.count <= 16_384, !body.isEmpty, body.count <= 262_144 else {
      throw InputBridgeFailure.invalidResponse
    }
    return try query.withUnsafeBytes { queryBytes in
      try body.withUnsafeBytes { bodyBytes in
        guard let dictionary = try Self.decode(msimeClientApplyCloudResponse(
          handle, queryBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(query.count),
          bodyBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(body.count))) as? [String: Any] else {
          throw InputBridgeFailure.invalidResponse
        }
        return dictionary
      }
    }
  }

  /// The AI HTTPS request descriptor for a query, or nil when the assistant's settings no longer match it. It carries the provider credential: never log it.
  func aiRequest(query: Data) -> [String: Any]? {
    guard handle != 0, !query.isEmpty, query.count <= 16_384 else { return nil }
    return query.withUnsafeBytes { bytes in
      (try? Self.decode(msimeClientAIRequestForQuery(
        handle, bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(query.count)))) as? [String: Any]
    }
  }

  /// The candidate texts in an AI reply, by the shared parser; empty when it supplies none.
  static func parseAIResponse(_ body: Data, limit: Int) -> [String] {
    guard !body.isEmpty, body.count <= 1_048_576, (1...10).contains(limit) else { return [] }
    return body.withUnsafeBytes { bytes in
      (try? decode(msimeClientParseAIResponse(
        bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(body.count), UInt8(limit)))) as? [String] ?? []
    }
  }

  /// The HTTP request the shared layer builds for one translation provider (`tencent`, `niutrans` or `custom`), or nil when the provider is off or the request is invalid. The descriptor carries credentials and must never be logged.
  static func translationRequest(provider: String, _ request: [String: Any]) -> [String: Any]? {
    let function: (UnsafePointer<MSIMEByte>?, UInt) -> UnsafeMutablePointer<CChar>?
    switch provider {
    case "tencent": function = msimeClientTencentTranslationHTTPRequest
    case "niutrans": function = msimeClientNiuTransTranslationHTTPRequest
    case "custom": function = msimeClientCustomTranslationHTTPRequest
    default: return nil
    }
    guard JSONSerialization.isValidJSONObject(request),
          let data = try? JSONSerialization.data(withJSONObject: request), data.count <= 16_384 else { return nil }
    return data.withUnsafeBytes { bytes in
      (try? decode(function(bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(data.count)))) as? [String: Any]
    }
  }

  /// One gloss per requested text, in order, from a provider's reply; nil when the reply is unusable as a whole. Tencent answers a batch of `expected` texts, the others one text each.
  static func parseTranslationResponse(provider: String, body: Data, expected: Int) -> [String?]? {
    guard !body.isEmpty, body.count <= 1_048_576 else { return nil }
    return body.withUnsafeBytes { bytes -> [String?]? in
      let base = bytes.bindMemory(to: MSIMEByte.self).baseAddress
      switch provider {
      case "tencent":
        guard (1...9).contains(expected),
              let values = (try? decode(msimeClientParseTencentTranslationResponse(base, UInt(body.count), UInt(expected)))) as? [Any],
              values.count == expected else { return nil }
        return values.map { $0 as? String }
      case "niutrans", "custom":
        guard expected == 1 else { return nil }
        let pointer = provider == "niutrans"
          ? msimeClientParseNiuTransTranslationResponse(base, UInt(body.count))
          : msimeClientParseCustomTranslationResponse(base, UInt(body.count))
        guard let value = (try? decode(pointer)) as? String else { return nil }
        return [value]
      default:
        return nil
      }
    }
  }

  /// Apply one provider's candidates (cloud 0, AI 1) to the query they answer. Returns `{applied, view}`.
  func applyOnlineCandidates(query: Data, candidates: [String], source: UInt8) throws -> [String: Any] {
    guard handle != 0 else { throw InputBridgeFailure.unavailable }
    let payload = try JSONSerialization.data(withJSONObject: candidates)
    guard !query.isEmpty, query.count <= 16_384, payload.count <= 16_384, source <= 1 else {
      throw InputBridgeFailure.invalidResponse
    }
    return try query.withUnsafeBytes { queryBytes in
      try payload.withUnsafeBytes { payloadBytes in
        guard let dictionary = try Self.decode(msimeClientApplyOnlineCandidates(
          handle, queryBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(query.count),
          payloadBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(payload.count), source)) as? [String: Any] else {
          throw InputBridgeFailure.invalidResponse
        }
        return dictionary
      }
    }
  }

  func snapshot(from value: [String: Any]) throws -> MetasequoiaInputSnapshot {
    try Self.snapshot(value)
  }

  func chooseNineKeySpelling(at index: UInt) -> MetasequoiaInputSnapshot {
    let generation = (try? view()).flatMap { Self.strictUInt64($0["generation"]) } ?? 0
    return dispatch { msimeClientChooseNineKeySpelling(handle, generation, index) }
  }

  @discardableResult func setLearningEnabled(_ enabled: Bool) -> Bool {
    updatePreferences { $0["learning"] = enabled }
  }

  @discardableResult func setFuzzyPinyinRules(_ rules: UInt32) -> Bool {
    let names = ["z-zh", "c-ch", "s-sh", "n-l", "f-h", "r-l", "an-ang", "en-eng", "in-ing", "ian-iang", "uan-uang"]
    let applied = updatePreferences { prefs in
      prefs["fuzzy_pinyin"] = ["enabled": rules != 0, "seeded": true,
                                "rules": names.enumerated().compactMap { rules & (1 << $0.offset) == 0 ? nil : $0.element }]
    }
    if applied { appliedFuzzyPinyinRules = rules }
    return applied
  }

  func setWubiMixedPinyin(_ enabled: Bool) {
    _ = updatePreferences { $0["wubi_mixed_pinyin"] = enabled }
  }

  @discardableResult func setFrequencyAdjustmentMode(_ mode: MetasequoiaFrequencyAdjustmentMode,
                                                      triggerCount: Int, linearStep: Int) -> Bool {
    let names = ["pin", "halve", "linear", "promote", "disabled"]
    return updatePreferences { $0["frequency"] = ["mode": names[Int(mode.rawValue)],
                                                    "trigger_count": triggerCount,
                                                    "linear_step": linearStep] }
  }

  func `switch`(toShuangpin enabled: Bool) -> MetasequoiaInputSnapshot {
    switchScheme(enabled ? "shuangpin" : "quanpin", profile: nil)
  }
  func `switch`(toShuangpinProfile profile: String) -> MetasequoiaInputSnapshot {
    switchScheme("shuangpin", profile: profile)
  }
  func switchToNineKey() -> MetasequoiaInputSnapshot {
    guard updatePreferences({ $0["scheme"] = "quanpin" }) else { return diagnostic("九键模式切换失败") }
    nineKeyEnabled = true
    return dispatch { msimeClientSetNineKeyMode(handle, true) }
  }
  /// Tell the runtime the width it commits in; from then on every commit it completes arrives already converted.
  @discardableResult func setCharacterWidth(fullwidth: Bool) -> MetasequoiaInputSnapshot {
    self.fullwidth = fullwidth
    return dispatch { msimeClientSetCharacterWidth(handle, fullwidth) }
  }
  /// Chinese or ASCII marks for this session, on top of the document's `chinese_punctuation`; `punctuation_lock` still wins.
  @discardableResult func setChinesePunctuation(_ enabled: Bool) -> MetasequoiaInputSnapshot {
    chinesePunctuation = enabled
    return dispatch { msimeClientSetChinesePunctuation(handle, enabled) }
  }
  /// Hand the runtime the AI provider key from the Keychain for this session's candidate-bar AI requests; nil clears it. The key is never written to the shared document.
  @discardableResult func setAICredential(_ token: String?) -> Bool {
    aiCredential = token
    return applyAICredential()
  }

  private func applyAICredential() -> Bool {
    guard handle != 0 else { return false }
    let bytes = Array((aiCredential ?? "").utf8)
    let response: Any? = try? bytes.withUnsafeBufferPointer { buffer -> Any in
      try Self.decode(msimeClientSetAICredential(handle, buffer.baseAddress, UInt(buffer.count)))
    }
    return response as? Bool == true
  }
  /// 只改 `scheme`，会话里的 `wubi_profile` 不动，所以切过去仍是当前的 86 或 98 五笔。
  func switchToWubi() -> MetasequoiaInputSnapshot { switchScheme("wubi", profile: nil) }
  /// 手写面板背后跑本版本的默认方案，与 `schemeMapping` 写进偏好的一致；full 是全拼。
  func switchToHandwriting() -> MetasequoiaInputSnapshot { switchScheme(MSIMEAppEdition.defaultScheme, profile: nil) }
  func switchToJapanese() -> MetasequoiaInputSnapshot { switchScheme("japanese", profile: nil) }
  /// Korean Hangul (Dubeolsik). Switching discards an open syllable, so callers finish the composition first.
  func switchToKorean() -> MetasequoiaInputSnapshot { switchScheme("korean", profile: nil) }
  /// Cantonese Jyutping. Without msime-cantonese.db beside EngineResources the runtime keeps running the last Chinese scheme instead, so the keyboard only offers it when the file is installed.
  func switchToCantonese() -> MetasequoiaInputSnapshot { switchScheme("cantonese", profile: nil) }
  /// Dachen Zhuyin with Traditional output. Without msime-zhuyin.db beside EngineResources the runtime keeps running the last Chinese scheme instead, so the keyboard only offers it when the file is installed. Switching discards an open conversion, so callers finish the composition first.
  func switchToZhuyin() -> MetasequoiaInputSnapshot { switchScheme("zhuyin", profile: nil) }
  /// Vietnamese Telex and VNI, composed in place with no candidates. Switching discards an open word, so callers finish the composition first.
  func switchToVietnamese() -> MetasequoiaInputSnapshot { switchScheme("vietnamese", profile: nil) }
  /// 藏文 EWTS 威利转写，就地组字，没有候选。切换会丢掉正在组的音节，所以调用方先结束组字。
  func switchToTibetan() -> MetasequoiaInputSnapshot { switchScheme("tibetan", profile: nil) }
  /// 笔画输入，按笔顺查单字。缺少 language-dictionaries 里的 msime-stroke.db 时运行时继续用最近一次的中文方案，所以键盘只在文件在时提供它。
  func switchToStroke() -> MetasequoiaInputSnapshot { switchScheme("stroke", profile: nil) }

  func editCandidate(at index: UInt, expectedWord: String, action: MetasequoiaCandidateAction) -> MetasequoiaInputSnapshot {
    guard let row = (try? currentCandidates())?[safe: Int(index)],
          row["text"] as? String == expectedWord,
          let identity = row["id"] as? [String: Any],
          let generation = Self.strictUInt64(identity["generation"]),
          let globalIndex = Self.strictUInt64(identity["index"]) else {
      return diagnostic("候选已失效")
    }
    return editCandidate(generation: generation, globalIndex: globalIndex,
                         action: action)
  }

  /// Edit a candidate the caller already holds an engine identity for.
  ///
  /// The expanded panel lists every candidate the engine returned, not the nine on the strip, so
  /// its positions are not the visible indexes the overload above resolves. It carries the
  /// generation and global index the snapshot gave it, which is what the engine wanted all along.
  func editCandidate(generation: UInt64, globalIndex: UInt64,
                     action: MetasequoiaCandidateAction) -> MetasequoiaInputSnapshot {
    guard let indexValue = UInt(exactly: globalIndex) else { return diagnostic("候选已失效") }
    switch action {
    case .promote:
      return dispatch { msimeClientPinCandidate(handle, generation, indexValue) }
    case .remove:
      return dispatch { msimeClientRemoveCandidate(handle, generation, indexValue) }
    case .fix(let position):
      return dispatch { msimeClientFixCandidatePosition(handle, generation, indexValue, position) }
    case .clearPosition:
      return dispatch { msimeClientClearCandidatePosition(handle, generation, indexValue) }
    }
  }

  func openLocalMode(_ trigger: String) -> MetasequoiaInputSnapshot {
    guard let byte = Self.ascii(trigger) else { return diagnostic("本地输入模式无效") }
    return dispatch { pointer(for: String(UnicodeScalar(byte)), shift: true) }
  }

  func nineKeySpellings() -> [String] {
    ((try? view()["nine_key_spellings"] as? [String]) ?? [])
  }

  /// Per-key double-pinyin hint text for the profile this session is running.
  ///
  /// The keymap is read out of the Engine rather than kept here. A host-side copy
  /// drifts from the scheme the session runs - this one had lost Xiaohe's `uai`
  /// from K, so the key that types `guai` carried no sign of it - and the Engine
  /// is the only place that knows which units a profile puts on which key. The
  /// shared layer answers with an empty face for a profile it does not run, which
  /// is what a Quanpin or Wubi session reports.
  func shuangpinKeyHints() -> [String: String] {
    // The view names a profile whatever the scheme, because the Engine is built with one
    // either way; only the scheme says whether the keys are running it.
    guard let snapshot = try? view(),
          Self.strictUInt64(snapshot["scheme"]) == UInt64(Self.shuangpinSchemeCode),
          let profile = snapshot["shuangpin_profile"] as? String, !profile.isEmpty,
          let data = profile.data(using: .utf8) else { return [:] }
    let response = try? data.withUnsafeBytes { bytes -> Any in
      try Self.decode(msimeClientShuangpinKeyHints(
        bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(data.count)))
    }
    return (response as? [String: String]) ?? [:]
  }

  // The engine keeps its dictionary access from creation until destroy, and those lock files sit in
  // the App Group container. iOS terminates an extension that is suspended while holding a lock
  // there, which it reports as 0xdead10cc, so putting the keyboard away has to hand the access back
  // rather than only pause learning. Destroying also ends learning writes, which is what pausing
  // was for.
  func suspendDictionarySession() -> Bool {
    guard !suspended else { return true }
    guard !hasComposition else { return false }
    if handle != 0 {
      guard (try? Self.decode(msimeClientDestroy(handle))) != nil else { return false }
      handle = 0
    }
    suspended = true
    return true
  }

  // Recreated from the options prepared when this process started. Preparation streams every pinned
  // resource through SHA-256, so repeating it for each presentation would cost far more than the
  // session itself; the options stay valid for the life of the process.
  func resumeDictionarySession() throws {
    guard suspended else { return }
    guard !options.isEmpty else { throw InputBridgeFailure.unavailable }
    try createFocusedSession()
    suspended = false
  }

  /// Create and focus a session, then restore the state the prepared options do not carry.
  ///
  /// Nine-key lives on the session, so every path that destroys and rebuilds one has to set it
  /// again. Dictionary maintenance and snapshot activation rebuild as often as resuming does:
  /// leaving the replay to the caller left the host drawing the nine-key layout over a 26-key
  /// engine after the first personal-dictionary refresh of a keyboard appearance.
  private func createFocusedSession() throws {
    // 选中只吃掉部分输入的候选时，让运行时把已选的那一段留在组字里而不是立刻上屏：用户还在打后
    // 半截，前半截已经进了文档的话，搜索框会拿半个词去搜，编辑器为它记一次撤销。键盘把它画在候选
    // 条的读音前面，两件事必须一起做（scripts/test-phrase-preedit-hosts.py 守的就是这一半状态）。
    var requested = options
    requested["phrase_preedit"] = true
    handle = try Self.callCreateFocused(requested)
    if nineKeyEnabled { _ = dispatch { msimeClientSetNineKeyMode(handle, true) } }
    if fullwidth { _ = dispatch { msimeClientSetCharacterWidth(handle, true) } }
    if let chinesePunctuation { _ = dispatch { msimeClientSetChinesePunctuation(handle, chinesePunctuation) } }
    if aiCredential != nil { _ = applyAICredential() }
  }

  func localDictionaryStateVersion() throws -> String {
    // A runtime that failed to prepare has no options to describe, and asking the snapshot API about the empty dictionary only answers "invalid snapshot options" - which the snapshot worker then shows in place of the real reason on the keyboard's diagnostic line.
    if let initializationDiagnostic, options["user_data"] == nil {
      throw InputBridgeFailure.response(initializationDiagnostic)
    }
    let result = try Self.callOptions(msimeClientSnapshotVersion, options)
    guard let version = result["version"] as? String, version.utf8.count == 64,
          let generation = result["generation"] as? String,
          generation == "legacy" || UUID(uuidString: generation)?.uuidString == generation else {
      throw InputBridgeFailure.invalidResponse
    }
    return "local-v1:\(generation):\(version)"
  }

  func dictionarySnapshotContext() throws -> [String: Any] {
    guard let resources = options["resources"] as? String, let user = options["user_data"] as? String,
          let dictionaries = options["dictionaries"] as? String else { throw InputBridgeFailure.invalidResponse }
    return ["resources": URL(fileURLWithPath: resources), "user": URL(fileURLWithPath: user),
            "contentIdentifier": dictionaries,
            "preparedOptions": try JSONSerialization.data(withJSONObject: options)]
  }

  func activateDictionarySnapshot(_ snapshot: MSIMEPreparedDictionarySnapshot,
                                  expectedVersion: String) throws {
    guard handle != 0 else { throw InputBridgeFailure.unavailable }
    let fields = expectedVersion.split(separator: ":", omittingEmptySubsequences: false)
    guard fields.count == 3, fields[0] == "local-v1", fields[2].utf8.count == 64 else {
      throw InputBridgeFailure.invalidResponse
    }
    let expected = Data(fields[2].utf8)
    // Release the shared dictionary lease before asking the host for its
    // exclusive activation lease, then recreate the session against the
    // published generation. If activation fails, restore the old session.
    let oldHandle = handle
    _ = try? Self.decode(msimeClientDestroy(oldHandle))
    handle = 0
    do {
      let response = try expected.withUnsafeBytes { bytes in
        try Self.decode(msimeClientSnapshotActivate(snapshot.handle,
                                                     bytes.bindMemory(to: MSIMEByte.self).baseAddress,
                                                     UInt(expected.count)))
      }
      _ = response
      try createFocusedSession()
      DictionarySnapshotBridge.forget(snapshot.identifier)
      snapshot.markConsumed()
    } catch {
      try createFocusedSession()
      throw error
    }
  }

  /// The core's transient lease conflict; the text is fixed by crates/host-api/src/dictionary_snapshot.rs.
  static func isSnapshotBusy(_ error: Error) -> Bool {
    if case .response("snapshot access busy") = error as? InputBridgeFailure { return true }
    return false
  }

  func applyPersonalPrevious(_ previous: [String: Any]?, replacement: [String: Any]?, requestID: String) throws {
    var action: [String: Any] = ["operation": "edit", "request_id": requestID]
    action["previous"] = previous ?? NSNull()
    action["replacement"] = replacement ?? NSNull()
    let request: [String: Any] = ["options": options, "action": action]
    let result = try withDictionaryMaintenance {
      try Self.callOptions(msimeClientDictionary, request)
    }
    guard (result["applied"] as? Bool) == true else { throw InputBridgeFailure.response("个人词条未能应用") }
  }

  /// One page of the dictionary. Without a kind it is the user's own words; a code prefix within one kind, or the quick phrases, is looked up in the dictionary itself, so bundled rows come back marked `source: bundled` and can be re-weighted or deleted through the queue.
  func personalEntries(atOffset offset: UInt, kind: PersonalWordKind? = nil, query: String = "") throws -> [String: Any] {
    var action: [String: Any] = ["operation": "list", "offset": offset, "limit": 100]
    if let kind { action["kind"] = kind.bridgeName }
    if !query.isEmpty { action["query"] = query }
    let request: [String: Any] = ["options": options, "action": action]
    var result = try withDictionaryMaintenance {
      try Self.callOptions(msimeClientDictionary, request)
    }
    if let hasMore = result.removeValue(forKey: "has_more") { result["hasMore"] = hasMore }
    return result
  }

  /// One dictionary in a shared text layout, read a page at a time inside one maintenance window so the session is reopened once rather than per page. It stops at `maximumPages` pages or `PersonalDictionaryStore.maximumExportBytes`, whichever comes first, and says so with `complete`. Pinyin carries the weights of bundled words the user changed or taught, and omits single characters, as the Windows export does.
  func personalExport(kind: PersonalWordKind, format: String, maximumPages: Int = 200) throws -> PersonalExportText {
    try withDictionaryMaintenance {
      var text = ""
      for page in 0..<maximumPages {
        let action: [String: Any] = ["operation": "export", "kind": kind.bridgeName, "format": format,
                                     "offset": page * 1000, "limit": 1000]
        let result = try Self.callOptions(msimeClientDictionary, ["options": options, "action": action])
        guard let chunk = result["text"] as? String, let hasMore = result["has_more"] as? Bool else {
          throw InputBridgeFailure.invalidResponse
        }
        text += chunk
        if text.utf8.count > PersonalDictionaryStore.maximumExportBytes {
          // Keep whole rows up to the cap, so the file still imports.
          let bytes = Array(text.utf8.prefix(PersonalDictionaryStore.maximumExportBytes))
          let end = bytes.lastIndex(of: UInt8(ascii: "\n")).map { $0 + 1 } ?? 0
          return PersonalExportText(text: String(decoding: bytes[..<end], as: UTF8.self), complete: false)
        }
        if !hasMore { return PersonalExportText(text: text, complete: true) }
      }
      return PersonalExportText(text: text, complete: false)
    }
  }

  private func withDictionaryMaintenance<T>(_ operation: () throws -> T) throws -> T {
    guard handle != 0 else { throw InputBridgeFailure.unavailable }
    guard !hasComposition else { throw InputBridgeFailure.response("请先结束当前输入再同步个人词库") }
    let previousHandle = handle
    _ = try Self.decode(msimeClientDestroy(previousHandle))
    handle = 0
    let result = Result { try operation() }
    do {
      try createFocusedSession()
      initializationDiagnostic = nil
    } catch {
      initializationDiagnostic = "词库维护后输入运行时恢复失败。"
      throw error
    }
    return try result.get()
  }

  private var hasComposition: Bool { (try? (view()["preedit"] as? String ?? "")).map { !$0.isEmpty } ?? false }

  private func localMode() throws -> String { try view()["local_mode"] as? String ?? "" }

  private func switchScheme(_ scheme: String, profile: String?) -> MetasequoiaInputSnapshot {
    let updated = updatePreferences { prefs in
      prefs["scheme"] = scheme
      if let profile { prefs["shuangpin_profile"] = profile }
    }
    guard updated else { return diagnostic("输入方案切换失败") }
    nineKeyEnabled = false
    return dispatch { msimeClientSetNineKeyMode(handle, false) }
  }

  private func command(_ value: UInt32) -> MetasequoiaInputSnapshot {
    dispatch { msimeClientCommand(handle, value) }
  }

  private func pointer(for character: String, shift: Bool) -> UnsafeMutablePointer<CChar>? {
    guard let byte = Self.ascii(character) else { return nil }
    return msimeClientCharacter(handle, byte, shift)
  }

  private func dispatch(_ operation: () -> UnsafeMutablePointer<CChar>?) -> MetasequoiaInputSnapshot {
    guard handle != 0 else {
      return diagnostic(initializationDiagnostic ?? "输入运行时尚未准备完成。")
    }
    do {
      guard let value = try Self.decode(operation()) as? [String: Any] else { throw InputBridgeFailure.invalidResponse }
      return try Self.snapshot(value)
    }
    catch { return diagnostic(error.localizedDescription) }
  }

  private func diagnostic(_ message: String) -> MetasequoiaInputSnapshot {
    MetasequoiaInputSnapshot(diagnosticText: message)
  }

  private func view() throws -> [String: Any] {
    try Self.callHandle(msimeClientView, handle)
  }

  private func currentCandidates() throws -> [[String: Any]] {
    (try view()["candidates"] as? [[String: Any]]) ?? []
  }

  private func updatePreferences(_ mutate: (inout [String: Any]) -> Void) -> Bool {
    guard handle != 0 else { return false }
    let previous = (options["preferences"] as? [String: Any]) ?? [:]
    var prefs = previous
    mutate(&prefs)
    options["preferences"] = prefs
    revision &+= 1
    let snapshot: [String: Any] = ["format_version": 1, "revision": revision, "preferences": prefs]
    do {
      let response = try Self.callUpdate(msimeClientUpdatePreferences, handle, snapshot)
      return try Self.preferencesUpdateSucceeded(response)
    } catch {
      options["preferences"] = previous
      revision &-= 1
      return false
    }
  }

  /// The keyboard host's own contract, laid over whatever the shared document holds.
  ///
  /// Both fields come from iOS-only switches in the App Group rather than the document, and have to hold for every session this host creates, including the ones created after the shared document replaces the session's preferences. A reload used to drop the page size, which left the engine paging candidates by the desktop's count while the strip numbered its own.
  private static func hostOverrides(applyingTo preferences: [String: Any]) -> [String: Any] {
    var preferences = preferences
    // See CandidatePageSizePreference. Never persisted: the shared document keeps the desktop's value.
    preferences["candidate_page_size"] = CandidatePageSizePreference.size
    // Cloud candidates are opt-in on iOS; see CloudCandidatePreference. Never persisted: the shared document keeps the desktop's value.
    preferences["cloud_candidates"] = CloudCandidatePreference.enabled
    return preferences
  }

  private static func bootstrapOptions(resources resourceOverride: URL?, stateRoot stateOverride: URL?) -> [String: Any] {
    let fm = FileManager.default
    let group = fm.containerURL(forSecurityApplicationGroupIdentifier: MSIMEAppEdition.appGroupIdentifier)
      ?? fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    let root = stateOverride ?? group.appendingPathComponent("MSIME", isDirectory: true)
    let resources = resourceOverride
      ?? Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true)
      ?? root.appendingPathComponent("resources", isDirectory: true)
    var options: [String: Any] = ["resources": resources.path, "state_root": root.path]
    // full 不传版本，请求与引入版本之前相同；其他版本让 host-api 按版本收窄方案并写进 HostOptions。
    if !MSIMEAppEdition.isFull { options["edition"] = MSIMEAppEdition.identifier }
    return options
  }

  /// `View.scheme` for double pinyin, as the shared runtime numbers the Engine's schemes.
  private static let shuangpinSchemeCode: UInt8 = 1

  private static func ascii(_ value: String) -> MSIMEByte? {
    guard value.utf8.count == 1, let byte = value.utf8.first else { return nil }
    return byte
  }

  private static func number(_ value: Any?) throws -> UInt64 {
    guard let value = strictUInt64(value) else { throw InputBridgeFailure.invalidResponse }
    return value
  }

  /// NSNumber's unsigned accessors truncate fractions and convert booleans; bridge identities and
  /// revisions are protocol integers, so malformed values must be rejected at the boundary.
  static func strictUInt64(_ value: Any?) -> UInt64? {
    guard let number = value as? NSNumber else { return nil }
    return CandidateGlossModel.integerValue(number, maximum: UInt64.max)
  }

  /// The runtime always reports whether a preference update was deferred. A missing or
  /// non-boolean field is a malformed protocol response, not an applied update.
  static func preferencesUpdateSucceeded(_ response: [String: Any]) throws -> Bool {
    guard let deferred = response["deferred"] as? Bool else {
      throw InputBridgeFailure.invalidResponse
    }
    return !deferred
  }

  /// Runtime state flags are protocol booleans. Keep absent optional fields on
  /// their documented defaults, but never let NSNumber or strings coerce into
  /// a state transition.
  private static func strictBool(_ value: Any?, fallback: Bool) throws -> Bool {
    guard let value else { return fallback }
    guard let boolean = value as? Bool else { throw InputBridgeFailure.invalidResponse }
    return boolean
  }

  private static func snapshot(_ value: [String: Any]) throws -> MetasequoiaInputSnapshot {
    let view = value["view"] as? [String: Any] ?? [:]
    let rows = view["candidates"] as? [[String: Any]] ?? []
    guard rows.allSatisfy({ $0["text"] is String }) else {
      throw InputBridgeFailure.invalidResponse
    }
    let candidateSources = try rows.map { try strictInt($0["source"], fallback: -1, range: 0...255) }
    let candidateFixedPositions = try rows.map { try strictInt($0["fixed_position"], fallback: 0, range: 0...255) }
    let pageCount = try strictInt(view["page_count"], fallback: 0, range: 0...Int.max)
    let editingText = view["editing_text"] as? String ?? ""
    let caretPosition = try strictInt(view["caret_position"], fallback: 0, range: 0...editingText.utf8.count)
    return MetasequoiaInputSnapshot(isHandled: try strictBool(value["handled"], fallback: false),
      commitText: value["commit"] as? String, preedit: view["preedit"] as? String ?? "",
      reading: view["reading"] as? String ?? "",
      phrasePrefix: view["phrase_prefix"] as? String ?? "",
      candidates: rows.compactMap { $0["text"] as? String },
      candidateCodes: rows.map { $0["code"] as? String ?? "" },
      candidateGlosses: rows.map { $0["translation"] as? String ?? "" },
      candidateAnnotations: rows.map { $0["annotation"] as? String ?? "" },
      candidateSources: candidateSources,
      candidateFixedPositions: candidateFixedPositions,
      candidatePageCount: pageCount,
      answeredByPinyinFallback: try strictBool(view["answered_by_pinyin_fallback"], fallback: false),
      diagnosticText: value["diagnostic"] as? String,
      localMode: view["local_mode"] as? String ?? "none",
      nineKeySpellings: view["nine_key_spellings"] as? [String] ?? [],
      editingText: editingText,
      caretPosition: caretPosition)
  }

  private static func strictInt(_ value: Any?, fallback: Int, range: ClosedRange<Int>) throws -> Int {
    guard let value else { return fallback }
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID(),
          let integer = Int(number.stringValue),
          NSNumber(value: integer).compare(number) == .orderedSame,
          range.contains(integer) else { throw InputBridgeFailure.invalidResponse }
    return integer
  }

  static func decodeResponse(
    _ pointer: UnsafeMutablePointer<CChar>?,
    release: (UnsafeMutablePointer<CChar>) -> Void
  ) throws -> Any {
    guard let pointer else { throw InputBridgeFailure.unavailable }
    defer { release(pointer) }
    // Parse the response where it already is. Going through `String(cString:)` and then
    // `.data(using:)` copies the whole document twice - and validates its UTF-8 on the way - before
    // the parser has seen a byte of it. Every keystroke carries a view with nine candidates, their
    // codes and their glosses, so those copies are on the path a person feels.
    let envelope: [String: Any]
    do {
      let length = strlen(pointer)
      let parsed = try pointer.withMemoryRebound(to: UInt8.self, capacity: length) { bytes in
        try JSONSerialization.jsonObject(
          with: Data(bytesNoCopy: UnsafeMutableRawPointer(mutating: bytes), count: length,
                     deallocator: .none))
      }
      guard let object = parsed as? [String: Any] else { throw InputBridgeFailure.invalidResponse }
      envelope = object
    } catch let failure as InputBridgeFailure {
      throw failure
    } catch {
      throw InputBridgeFailure.invalidResponse
    }
    guard envelope["ok"] as? Bool == true else {
      throw InputBridgeFailure.response(envelope["error"] as? String ?? "输入运行时调用失败")
    }
    return envelope["value"] ?? NSNull()
  }

  private static func decode(_ pointer: UnsafeMutablePointer<CChar>?) throws -> Any {
    try decodeResponse(pointer, release: msimeClientStringFree)
  }

  private static func callCreate(_ options: [String: Any]) throws -> [String: Any] {
    try callOptions(msimeClientCreate, options)
  }

  private static func callCreateFocused(_ options: [String: Any]) throws -> UInt64 {
    let handle = try number(try callCreate(options)["session"])
    do {
      _ = try decode(msimeClientFocus(handle, true))
      return handle
    } catch {
      _ = try? decode(msimeClientDestroy(handle))
      throw error
    }
  }

  private static func callOptions(_ function: (UnsafePointer<MSIMEByte>?, UInt) -> UnsafeMutablePointer<CChar>?,
                                  _ object: Any) throws -> [String: Any] {
    let data = try JSONSerialization.data(withJSONObject: object)
    return try data.withUnsafeBytes { bytes in
      let value = try decode(function(bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(data.count)))
      guard let dictionary = value as? [String: Any] else { throw InputBridgeFailure.invalidResponse }
      return dictionary
    }
  }

  /// Call a function that takes a raw UTF-8 path rather than a JSON document.
  private static func callDirectory(_ function: (UnsafePointer<MSIMEByte>?, UInt) -> UnsafeMutablePointer<CChar>?,
                                    _ directory: Data) throws -> [String: Any] {
    try directory.withUnsafeBytes { bytes in
      let value = try decode(function(bytes.bindMemory(to: MSIMEByte.self).baseAddress,
                                      UInt(directory.count)))
      guard let dictionary = value as? [String: Any] else { throw InputBridgeFailure.invalidResponse }
      return dictionary
    }
  }

  private static func callHandle(_ function: (UInt64) -> UnsafeMutablePointer<CChar>?,
                                 _ handle: UInt64) throws -> [String: Any] {
    let value = try decode(function(handle))
    guard let dictionary = value as? [String: Any] else { throw InputBridgeFailure.invalidResponse }
    return dictionary
  }

  private static func callUpdate(_ function: (UInt64, UnsafePointer<MSIMEByte>?, UInt) -> UnsafeMutablePointer<CChar>?,
                                 _ handle: UInt64, _ object: Any) throws -> [String: Any] {
    let data = try JSONSerialization.data(withJSONObject: object)
    return try data.withUnsafeBytes { bytes in
      let value = try decode(function(handle, bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(data.count)))
      guard let dictionary = value as? [String: Any] else { throw InputBridgeFailure.invalidResponse }
      return dictionary
    }
  }
}

private extension Array {
  subscript(safe index: Index) -> Element? {
    indices.contains(index) ? self[index] : nil
  }
}
