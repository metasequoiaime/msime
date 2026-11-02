import CoreFoundation
import Foundation

private typealias MSIMEByte = UInt8

@_silgen_name("msime_client_prepare_host")
private func msimeClientPrepareHost(_ options: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_create")
private func msimeClientCreateRaw(_ options: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_destroy")
private func msimeClientDestroyRaw(_ session: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_focus")
private func msimeClientFocusRaw(_ session: UInt64, _ focused: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientStringFree(_ value: UnsafeMutablePointer<CChar>?)
@_silgen_name("msime_client_character")
private func msimeClientCharacterRaw(_ session: UInt64, _ value: MSIMEByte, _ shift: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_key_geometry")
private func msimeClientSetKeyGeometryRaw(_ session: UInt64, _ json: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_touch_character")
private func msimeClientTouchCharacterRaw(
  _ session: UInt64, _ value: MSIMEByte, _ shift: Bool, _ geometryID: UInt32, _ x: Float, _ y: Float
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_glide")
private func msimeClientGlideRaw(_ session: UInt64, _ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_punctuation")
private func msimeClientPunctuationRaw(_ session: UInt64, _ value: MSIMEByte) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_punctuation_with_context")
private func msimeClientPunctuationWithContextRaw(
  _ session: UInt64, _ value: MSIMEByte, _ preceding: UInt32
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_command")
private func msimeClientCommandRaw(_ session: UInt64, _ command: UInt32) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_reset_cache")
private func msimeClientResetCacheRaw(_ session: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_select")
private func msimeClientSelectRaw(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_select_any_candidate")
private func msimeClientSelectAnyCandidateRaw(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_select_edge")
private func msimeClientSelectEdgeRaw(_ session: UInt64, _ generation: UInt64, _ index: UInt, _ edge: UInt8) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_pin_candidate")
private func msimeClientPinCandidateRaw(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_remove_candidate")
private func msimeClientRemoveCandidateRaw(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_fix_candidate_position")
private func msimeClientFixCandidatePositionRaw(_ session: UInt64, _ generation: UInt64, _ index: UInt, _ position: UInt8) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_clear_candidate_position")
private func msimeClientClearCandidatePositionRaw(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_balance_paired_punctuation_after_auto_close")
private func msimeClientBalancePairedPunctuationAfterAutoCloseRaw(_ session: UInt64, _ opening: MSIMEByte) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_smart_punctuation_arm")
private func msimeClientSmartPunctuationArmRaw(_ session: UInt64, _ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_smart_punctuation_decide")
private func msimeClientSmartPunctuationDecideRaw(_ session: UInt64, _ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_nine_key_shuangpin_layout")
private func msimeClientNineKeyShuangpinLayout(_ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_shuangpin_key_hints")
private func msimeClientShuangpinKeyHints(_ profile: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_choose_nine_key_spelling")
private func msimeClientChooseNineKeySpellingRaw(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_nine_key_filter")
private func msimeClientSetNineKeyFilterRaw(_ session: UInt64, _ singleCharacter: Bool, _ strokes: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_key_grid")
private func msimeClientSetKeyGridRaw(_ session: UInt64, _ grid: UInt8) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_grid_key")
private func msimeClientGridKeyRaw(_ session: UInt64, _ letter: MSIMEByte) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_chinese_punctuation")
private func msimeClientSetChinesePunctuationRaw(_ session: UInt64, _ enabled: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_ai_credential")
private func msimeClientSetAICredentialRaw(_ session: UInt64, _ token: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_character_width")
private func msimeClientSetCharacterWidthRaw(_ session: UInt64, _ fullwidth: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_set_private_session")
private func msimeClientSetPrivateSessionRaw(_ session: UInt64, _ enabled: Bool) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_update_preferences")
private func msimeClientUpdatePreferencesRaw(_ session: UInt64, _ snapshot: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_load_preferences")
private func msimeClientLoadPreferences(_ directory: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_save_preferences")
private func msimeClientSavePreferences(
  _ directory: UnsafePointer<MSIMEByte>?, _ directoryLength: UInt, _ expectedRevision: UInt64,
  _ snapshot: UnsafePointer<MSIMEByte>?, _ snapshotLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_view")
private func msimeClientViewRaw(_ session: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_all_candidates")
private func msimeClientAllCandidatesRaw(_ session: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_english_completions")
private func msimeClientEnglishCompletionsRaw(
  _ session: UInt64, _ prefix: UnsafePointer<MSIMEByte>?, _ prefixLength: UInt, _ limit: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_english_completions_request")
private func msimeClientEnglishCompletionsRequest(
  _ request: UnsafePointer<MSIMEByte>?, _ requestLength: UInt,
  _ resources: UnsafePointer<MSIMEByte>?, _ resourcesLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_candidate_gloss_request")
private func msimeClientCandidateGlossRequest(
  _ request: UnsafePointer<MSIMEByte>?, _ requestLength: UInt,
  _ resources: UnsafePointer<MSIMEByte>?, _ resourcesLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_pronunciation_request")
private func msimeClientPronunciationRequest(
  _ request: UnsafePointer<MSIMEByte>?, _ requestLength: UInt,
  _ resources: UnsafePointer<MSIMEByte>?, _ resourcesLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_emoji_catalog_request")
private func msimeClientEmojiCatalogRequest(
  _ request: UnsafePointer<MSIMEByte>?, _ requestLength: UInt,
  _ resources: UnsafePointer<MSIMEByte>?, _ resourcesLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_apply_translations")
private func msimeClientApplyTranslationsRaw(
  _ session: UInt64, _ generation: UInt64,
  _ translations: UnsafePointer<MSIMEByte>?, _ translationsLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_online_query")
private func msimeClientOnlineQueryRaw(_ session: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_cloud_request_url")
private func msimeClientCloudRequestURL(_ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_apply_cloud_response")
private func msimeClientApplyCloudResponseRaw(
  _ session: UInt64, _ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt,
  _ body: UnsafePointer<MSIMEByte>?, _ bodyLength: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_ai_request_for_query")
private func msimeClientAIRequestForQueryRaw(
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
@_silgen_name("msime_client_ai_translation_http_request")
private func msimeClientAITranslationHTTPRequest(_ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_parse_ai_translation_response")
private func msimeClientParseAITranslationResponse(_ body: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_parse_tencent_translation_response")
private func msimeClientParseTencentTranslationResponse(_ body: UnsafePointer<MSIMEByte>?, _ length: UInt, _ expected: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_parse_niutrans_translation_response")
private func msimeClientParseNiuTransTranslationResponse(_ body: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_parse_custom_translation_response")
private func msimeClientParseCustomTranslationResponse(_ body: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_apply_online_candidates")
private func msimeClientApplyOnlineCandidatesRaw(
  _ session: UInt64, _ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt,
  _ candidates: UnsafePointer<MSIMEByte>?, _ candidatesLength: UInt, _ source: UInt8
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_resolve_online_candidates")
private func msimeClientResolveOnlineCandidatesRaw(
  _ session: UInt64, _ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt, _ source: UInt8
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_dictionary")
private func msimeClientDictionary(_ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_plugins")
private func msimeClientPlugins(_ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_key_sound_pack")
private func msimeClientKeySoundPack(_ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_snapshot_version")
private func msimeClientSnapshotVersion(_ options: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_snapshot_activate")
private func msimeClientSnapshotActivate(_ handle: UInt64, _ expected: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

/// C ABI会话绑定创建线程；串行队列可能换线程，因此所有会话调用由这一条常驻线程拥有。
final class MSIMEInputThread: @unchecked Sendable {
  static let shared = MSIMEInputThread()
  private let condition = NSCondition()
  private var tasks: [() -> Void] = []
  private var thread: Thread!

  private init() {
    thread = Thread { [self] in
      while true {
        condition.lock()
        while tasks.isEmpty { condition.wait() }
        let task = tasks.removeFirst()
        condition.unlock()
        autoreleasepool { task() }
      }
    }
    thread.name = "app.msime.ios.input"
    thread.qualityOfService = .userInteractive
    thread.start()
  }

  func async(_ operation: @escaping () -> Void) {
    condition.lock()
    tasks.append(operation)
    condition.signal()
    condition.unlock()
  }

  func sync<Value>(_ operation: @escaping () -> Value) -> Value {
    if Thread.current === thread { return operation() }
    let completed = DispatchSemaphore(value: 0)
    var result: Value?
    async {
      result = operation()
      completed.signal()
    }
    completed.wait()
    return result!
  }
}

private func msimeClientCreate(_ options: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientCreateRaw(options, length) }
}

private func msimeClientDestroy(_ session: UInt64) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientDestroyRaw(session) }
}

private func msimeClientFocus(_ session: UInt64, _ focused: Bool) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientFocusRaw(session, focused) }
}

private func msimeClientCharacter(_ session: UInt64, _ value: MSIMEByte, _ shift: Bool) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientCharacterRaw(session, value, shift) }
}

private func msimeClientSetKeyGeometry(_ session: UInt64, _ json: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSetKeyGeometryRaw(session, json, length) }
}

private func msimeClientTouchCharacter(
  _ session: UInt64, _ value: MSIMEByte, _ shift: Bool, _ geometryID: UInt32, _ x: Float, _ y: Float
) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientTouchCharacterRaw(session, value, shift, geometryID, x, y) }
}

private func msimeClientGlide(_ session: UInt64, _ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientGlideRaw(session, request, length) }
}

private func msimeClientPunctuation(_ session: UInt64, _ value: MSIMEByte) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientPunctuationRaw(session, value) }
}

private func msimeClientPunctuationWithContext(
  _ session: UInt64, _ value: MSIMEByte, _ preceding: UInt32
) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientPunctuationWithContextRaw(session, value, preceding) }
}

private func msimeClientCommand(_ session: UInt64, _ command: UInt32) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientCommandRaw(session, command) }
}

private func msimeClientResetCache(_ session: UInt64) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientResetCacheRaw(session) }
}

private func msimeClientSelect(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSelectRaw(session, generation, index) }
}

private func msimeClientSelectAnyCandidate(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSelectAnyCandidateRaw(session, generation, index) }
}

private func msimeClientSelectEdge(_ session: UInt64, _ generation: UInt64, _ index: UInt, _ edge: UInt8) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSelectEdgeRaw(session, generation, index, edge) }
}

private func msimeClientPinCandidate(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientPinCandidateRaw(session, generation, index) }
}

private func msimeClientRemoveCandidate(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientRemoveCandidateRaw(session, generation, index) }
}

private func msimeClientFixCandidatePosition(_ session: UInt64, _ generation: UInt64, _ index: UInt, _ position: UInt8) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientFixCandidatePositionRaw(session, generation, index, position) }
}

private func msimeClientClearCandidatePosition(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientClearCandidatePositionRaw(session, generation, index) }
}

private func msimeClientBalancePairedPunctuationAfterAutoClose(_ session: UInt64, _ opening: MSIMEByte) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientBalancePairedPunctuationAfterAutoCloseRaw(session, opening) }
}

private func msimeClientSmartPunctuationArm(_ session: UInt64, _ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSmartPunctuationArmRaw(session, request, length) }
}

private func msimeClientSmartPunctuationDecide(_ session: UInt64, _ request: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSmartPunctuationDecideRaw(session, request, length) }
}

private func msimeClientChooseNineKeySpelling(_ session: UInt64, _ generation: UInt64, _ index: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientChooseNineKeySpellingRaw(session, generation, index) }
}

private func msimeClientSetNineKeyFilter(_ session: UInt64, _ singleCharacter: Bool, _ strokes: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSetNineKeyFilterRaw(session, singleCharacter, strokes, length) }
}

private func msimeClientSetKeyGrid(_ session: UInt64, _ grid: UInt8) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSetKeyGridRaw(session, grid) }
}

private func msimeClientGridKey(_ session: UInt64, _ letter: MSIMEByte) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientGridKeyRaw(session, letter) }
}

private func msimeClientSetChinesePunctuation(_ session: UInt64, _ enabled: Bool) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSetChinesePunctuationRaw(session, enabled) }
}

private func msimeClientSetAICredential(_ session: UInt64, _ token: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSetAICredentialRaw(session, token, length) }
}

private func msimeClientSetCharacterWidth(_ session: UInt64, _ fullwidth: Bool) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSetCharacterWidthRaw(session, fullwidth) }
}

private func msimeClientSetPrivateSession(_ session: UInt64, _ enabled: Bool) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientSetPrivateSessionRaw(session, enabled) }
}

private func msimeClientUpdatePreferences(_ session: UInt64, _ snapshot: UnsafePointer<MSIMEByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientUpdatePreferencesRaw(session, snapshot, length) }
}

private func msimeClientView(_ session: UInt64) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientViewRaw(session) }
}

private func msimeClientAllCandidates(_ session: UInt64) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientAllCandidatesRaw(session) }
}

private func msimeClientEnglishCompletions(
  _ session: UInt64, _ prefix: UnsafePointer<MSIMEByte>?, _ prefixLength: UInt, _ limit: UInt
) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientEnglishCompletionsRaw(session, prefix, prefixLength, limit) }
}

private func msimeClientApplyTranslations(
  _ session: UInt64, _ generation: UInt64,
  _ translations: UnsafePointer<MSIMEByte>?, _ translationsLength: UInt
) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientApplyTranslationsRaw(session, generation, translations, translationsLength) }
}

private func msimeClientOnlineQuery(_ session: UInt64) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientOnlineQueryRaw(session) }
}

private func msimeClientApplyCloudResponse(
  _ session: UInt64, _ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt,
  _ body: UnsafePointer<MSIMEByte>?, _ bodyLength: UInt
) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientApplyCloudResponseRaw(session, query, queryLength, body, bodyLength) }
}

private func msimeClientAIRequestForQuery(
  _ session: UInt64, _ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt
) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientAIRequestForQueryRaw(session, query, queryLength) }
}

private func msimeClientApplyOnlineCandidates(
  _ session: UInt64, _ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt,
  _ candidates: UnsafePointer<MSIMEByte>?, _ candidatesLength: UInt, _ source: UInt8
) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientApplyOnlineCandidatesRaw(session, query, queryLength, candidates, candidatesLength, source) }
}

private func msimeClientResolveOnlineCandidates(
  _ session: UInt64, _ query: UnsafePointer<MSIMEByte>?, _ queryLength: UInt, _ source: UInt8
) -> UnsafeMutablePointer<CChar>? {
  MSIMEInputThread.shared.sync { msimeClientResolveOnlineCandidatesRaw(session, query, queryLength, source) }
}

enum MetasequoiaCandidateAction: Equatable {
  /// `position` is 1...5, the range the shared layer accepts and the desktop candidate menu offers.
  case promote, remove, fix(position: UInt8), clearPosition
}

enum MetasequoiaFrequencyAdjustmentMode: UInt8 {
  case pin, halve, linear, promote, disabled
}

struct MetasequoiaCandidateIdentity: Equatable, Sendable {
  let generation: UInt64
  let index: UInt64
}

struct MetasequoiaInputSnapshot: Equatable, Sendable {
  let isHandled: Bool
  let commitText: String?
  /// 上屏后光标右边的正文；nil 表示没有定位请求，空串表示正文末尾。
  let commitCursorSuffix: String?
  /// Runtime 已经按候选来源分段转换时为 false，宿主不能再次把整行转成繁体。
  let scriptConversion: Bool
  let preedit: String
  let reading: String
  /// 已经选中的那一段，运行时替这个宿主留在组字里而不是立刻上屏。
  ///
  /// 候选只吃掉部分输入时，引擎会继续组字并把选中的那一段交回来。留住它的宿主必须画出来：请求了
  /// 却不画，用户已经选中的字既不在文档里也不在屏幕上，而组字在他按取消之前不会结束。
  let phrasePrefix: String
  let candidates: [String]
  let candidateCodes: [String]
  /// 画面上每个候选的身份；异步输入之后不能用引擎最新页解释旧按钮的位置。
  let candidateIdentities: [MetasequoiaCandidateIdentity?]
  let generation: UInt64?
  let candidateGlosses: [String]
  /// The Engine's display suffix for each candidate, aligned with `candidates`: its helpcode when the scheme's "show helpcode" setting is on, the spelling a typo correction replaced, or a Korean Hanja's 훈음. Never part of the committed text.
  let candidateAnnotations: [String]
  /// The Engine `CandidateSource` (cloud, AI, dictionary...) and pinned slot of each candidate, aligned with `candidates`; a source of -1 means the row carried none, and a fixed position of zero is a word ranked by use.
  let candidateSources: [Int]
  let candidateFixedPositions: [Int]
  /// 引擎当前页的高亮状态，跟随候选一起过滤与换页。
  let candidateHighlighted: [Bool]
  /// 引擎按双拼触点首选规则让这一行纠错坐了首选（View 的 `confident_correction`，只在为真时出现），与 `candidates` 对齐。候选条和展开网格据此用强调色画它，读屏念「已纠正」；纠错前的拼写照旧在 `candidateAnnotations` 里。
  let candidateConfidentCorrections: [Bool]
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
  /// 全拼九键和 14 键组字时首选候选覆盖的按键写成拼音（`ning'bai`；首选是英文词时是这个词的字母），给读音行显示；五笔 14 键是首选编码的前几位（与 `preedit` 相同，键 QW QW 首选「你」时是 `wq`）。其他情况为空，这时照旧显示 `preedit`（九键下是数字，14 键下是组码字母）。
  let nineKeyReading: String
  /// 九键候选当前的筛选：只留单字，以及首字笔顺前缀（`hspnz`，空表示不按笔画）。组字结束时 Engine 把两者都清掉。
  let nineKeySingleCharacter: Bool
  let nineKeyStrokes: String
  /// The Engine's ASCII spelling and the caret inside it, as a byte offset. The caret leaves the end only when the user moves it (dragging the space bar while composing), which is the Windows host's ← / → editing of the input string.
  let editingText: String
  let caretPosition: Int
  /// View 的 `punctuation_marks`：中文标点下 `'` 出「」、`"` 出『』（直角引号），`[` `]` 出〖〗（空心括号）。host-api 已按繁体输出开关解析好 `corner_when_traditional`，键盘画键面和补全成对标点时读它，不自己解析偏好。
  let cornerQuotes: Bool
  let hollowBrackets: Bool
  /// View 的 `punctuation_marks`：顿号 `\` 键与 `/` 键各自开着与否（契约 `enumerationComma` 段，host-api 已归一「至少一个开」），符号面板选顿号时按它挑发给 Engine 的键。
  let enumerationCommaBackslash: Bool
  let enumerationCommaSlash: Bool

  var isInLocalMode: Bool { !localMode.isEmpty && localMode != "none" }

  /// The spelling with a bar where the caret sits, or nil while the caret is at the end, where the strip keeps showing the segmented pinyin. Windows draws the same caret inside the composition. The offset indexes characters directly because the editing text is ASCII; anything else is not split.
  var editingTextWithCaret: String? {
    guard caretPosition >= 0, caretPosition < editingText.count, editingText.allSatisfy(\.isASCII) else { return nil }
    let caret = editingText.index(editingText.startIndex, offsetBy: caretPosition)
    return editingText[..<caret] + "|" + editingText[caret...]
  }

  init(isHandled: Bool = false, commitText: String? = nil, commitCursorSuffix: String? = nil,
       scriptConversion: Bool = true, preedit: String = "", reading: String = "",
       phrasePrefix: String = "",
       candidates: [String] = [], candidateCodes: [String] = [],
       candidateIdentities: [MetasequoiaCandidateIdentity?] = [], generation: UInt64? = nil,
       candidateGlosses: [String] = [],
       candidateAnnotations: [String] = [], candidateSources: [Int] = [], candidateFixedPositions: [Int] = [],
       candidateHighlighted: [Bool] = [], candidateConfidentCorrections: [Bool] = [],
       candidatePageCount: Int = 0, answeredByPinyinFallback: Bool = false,
       diagnosticText: String? = nil, localMode: String = "none",
       nineKeySpellings: [String] = [], nineKeyReading: String = "", nineKeySingleCharacter: Bool = false,
       nineKeyStrokes: String = "", editingText: String = "", caretPosition: Int = 0,
       cornerQuotes: Bool = false, hollowBrackets: Bool = false,
       enumerationCommaBackslash: Bool = true, enumerationCommaSlash: Bool = false) {
    self.isHandled = isHandled
    self.commitText = commitText
    self.commitCursorSuffix = commitCursorSuffix
    self.scriptConversion = scriptConversion
    self.preedit = preedit
    self.reading = reading
    self.phrasePrefix = phrasePrefix
    self.candidates = candidates
    self.candidateCodes = candidateCodes
    self.candidateIdentities = candidateIdentities
    self.generation = generation
    self.candidateGlosses = candidateGlosses
    self.candidateAnnotations = candidateAnnotations
    self.candidateSources = candidateSources
    self.candidateFixedPositions = candidateFixedPositions
    self.candidateHighlighted = candidateHighlighted
    self.candidateConfidentCorrections = candidateConfidentCorrections
    self.candidatePageCount = candidatePageCount
    self.answeredByPinyinFallback = answeredByPinyinFallback
    self.diagnosticText = diagnosticText
    self.localMode = localMode
    self.nineKeySpellings = nineKeySpellings
    self.nineKeyReading = nineKeyReading
    self.nineKeySingleCharacter = nineKeySingleCharacter
    self.nineKeyStrokes = nineKeyStrokes
    self.editingText = editingText
    self.caretPosition = caretPosition
    self.cornerQuotes = cornerQuotes
    self.hollowBrackets = hollowBrackets
    self.enumerationCommaBackslash = enumerationCommaBackslash
    self.enumerationCommaSlash = enumerationCommaSlash
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
  private final class PendingCharacter {
    var snapshot: MetasequoiaInputSnapshot?
    let completion: (MetasequoiaInputSnapshot) -> Void
    init(_ completion: @escaping (MetasequoiaInputSnapshot) -> Void) { self.completion = completion }
  }
  private let pendingCharactersLock = NSLock()
  private var pendingCharacters: [PendingCharacter] = []

  var hasPendingCharacters: Bool {
    pendingCharactersLock.lock()
    defer { pendingCharactersLock.unlock() }
    return !pendingCharacters.isEmpty
  }

  /// 26 键双拼一键的按下点（#10018），含义与 `handleTouchCharacter` 的参数相同。
  struct TouchPoint: Equatable {
    let geometryID: UInt32
    let x: Float
    let y: Float
  }

  /// 能交给会话线程异步处理的按键：九键网格下的数字 2–9，不用网格（26 键）时的小写字母和微软双拼的 `;`；按下点只在 26 键上带。其余按键（分词、大写、标点、其他网格）仍走同步入口。
  static func acceptsAsynchronousCharacter(_ byte: UInt8, grid: KeyGrid, touched: Bool) -> Bool {
    switch grid {
    case .nineKey: return !touched && (50...57).contains(byte)
    case .none: return (97...122).contains(byte) || byte == 59
    default: return false
    }
  }

  /// 普通全拼九键数字，或 26 键全拼、双拼的字母（#10031）；方案由键盘判断，这里只按网格和字节把关。字节、句柄与按下点按值移交，所有结果仍按投递顺序交给主线程。
  @discardableResult
  func enqueueCharacter(_ character: String, touch: TouchPoint? = nil,
                        completion: @escaping (MetasequoiaInputSnapshot) -> Void) -> Bool {
    precondition(Thread.isMainThread)
    guard handle != 0, let byte = Self.ascii(character),
          Self.acceptsAsynchronousCharacter(byte, grid: keyGrid, touched: touch != nil) else { return false }
    let pending = PendingCharacter(completion)
    let sessionHandle = handle
    pendingCharactersLock.lock()
    pendingCharacters.append(pending)
    pendingCharactersLock.unlock()
    MSIMEInputThread.shared.async { [weak self] in
      guard let self else { return }
      let snapshot: MetasequoiaInputSnapshot
      do {
        let response = touch.map {
          msimeClientTouchCharacter(sessionHandle, byte, false, $0.geometryID, $0.x, $0.y)
        } ?? msimeClientCharacter(sessionHandle, byte, false)
        guard let value = try Self.decode(response) as? [String: Any]
        else { throw InputBridgeFailure.invalidResponse }
        snapshot = try Self.snapshot(value)
      } catch {
        snapshot = MetasequoiaInputSnapshot(diagnosticText: error.localizedDescription)
      }
      pendingCharactersLock.lock()
      pending.snapshot = snapshot
      pendingCharactersLock.unlock()
      DispatchQueue.main.async { [weak self] in self?.deliverCompletedCharacters() }
    }
    return true
  }

  /// 空格、删除、设置与焦点边界先完成此前数字；回调不持锁执行，允许渲染过程中再次读会话。
  func flushPendingCharacters() {
    guard hasPendingCharacters else { return }
    precondition(Thread.isMainThread)
    repeat {
      MSIMEInputThread.shared.sync {}
      deliverCompletedCharacters()
    } while hasPendingCharacters
  }

  private func deliverCompletedCharacters() {
    precondition(Thread.isMainThread)
    while true {
      pendingCharactersLock.lock()
      guard let pending = pendingCharacters.first, let snapshot = pending.snapshot else {
        pendingCharactersLock.unlock()
        return
      }
      pendingCharacters.removeFirst()
      pendingCharactersLock.unlock()
      // 每次只移交一项；回调中的控制命令仍能先排空剩余结果，再改变会话状态。
      pending.completion(snapshot)
    }
  }
  private var options: [String: Any]
  private var stateRoot: String?
  /// The session reads the App Group document rather than a test's own directory, so its writes are the user's settings changes that cloud sync uploads.
  private let sharesStateRoot: Bool
  private var initializationDiagnostic: String?
  private var revision: UInt64 = 0
  /// The shared document's revision as this bridge last saw it.
  ///
  /// Kept apart from `revision`, which counts the snapshots handed to the session: every local change bumps that one, so a document the settings app had just saved - one step past what the keyboard last read - compared lower and was dropped as stale.
  private var documentRevision: UInt64 = 0
  private var appliedFuzzyPinyinRules: UInt32?
  private var suspended = false
  /// 引擎的组码网格，取值与 `msime_client_set_key_grid` 的 `grid` 相同：0 不用网格（26 键），1 九键，2 当前方案的 14 键，3 九键双拼，4 全拼 18 键，5 双拼 18 键。
  enum KeyGrid: UInt8 {
    case none = 0
    case nineKey = 1
    case fourteenKey = 2
    case nineKeyShuangpin = 3
    case eighteenKey = 4
    case eighteenKeyShuangpin = 5
  }
  // 网格（九键、14 键或 18 键）是会话状态，不在选项带的偏好里，所以重建的会话要再告诉一次，否则会回到 26 键。
  private var keyGrid = KeyGrid.none
  // The width, like nine-key, is session state the options do not carry, so a rebuilt session is told it again.
  private var fullwidth = false
  /// The keyboard's 中文标点 switch; nil until it is first set, so a rebuilt session keeps the document's value.
  private var chinesePunctuation: Bool?
  /// Kept only in memory so a recreated focused session gets it back; it never reaches the shared document.
  private var aiCredential: String?
  /// 隐私会话标记和九键一样是会话状态，选项里不带，重建的会话要再告诉一次。
  private var privateSession = false
  /// 运行时已确认带着 `privateSession` 的那个会话句柄；0 表示当前会话还没确认过。键盘每次空闲渲染都会再设一次隐私标记，值没变、会话也没重建时就不再跨到输入线程。
  private var privateSessionHandle: UInt64 = 0
  /// 当前会话最后一次接受并已经生效（没被组字推迟）的偏好，即交给它的 `engineBound` 文档；nil 表示不确定，下一次偏好更新照常发送。
  ///
  /// 键盘每次空闲渲染（每次上屏、每个中文标点、中文模式下按住删除键时每秒十几次）都会把学习和调频设置再推一遍，以前每次都要把整份偏好文档编成 JSON、同步跨到输入线程、在 Rust 里解析校验比较，再把整个 view 编回来解析，值却一次也没变。会话的偏好只经这个桥的 `msime_client_update_preferences` 改动，所以与这份完全相同的请求不会改变会话的任何状态，可以不发。被推迟的那次不记：组字结束后同样的请求还要再发一次，才能如实报告已经生效。
  private var appliedPreferences: [String: Any]?

  init(resources: URL? = nil, stateRoot: URL? = nil) {
    options = [:]
    self.stateRoot = nil
    sharesStateRoot = stateRoot == nil
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

  /// 现在按 `/` 能不能打开 / 指令：没有组字、不在本地模式里，并且 Engine 把 `/` 列在 `spelling_symbols` 里（`local_modes.command` 开着、中文标点、方案能打开它时才列）。键盘的「本地输入 → 指令」点下去之前问一次，不在按键路径上调用。
  func slashOpensCommandMode() -> Bool {
    guard let current = try? view() else { return false }
    let mode = current["local_mode"] as? String ?? "none"
    let editing = current["editing_text"] as? String ?? ""
    let symbols = current["spelling_symbols"] as? String ?? ""
    return mode == "none" && editing.isEmpty && symbols.contains("/")
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
    // 输入页的「只出单字」：同样在会话空闲时重建 Engine。
    "single_character_only",
    // 标点页的「中英文之间加空格」：会话在上屏时读最新请求的偏好，交给活着的会话后下一次上屏就生效。
    "chinese_english_spacing",
    // 标点页的引号样式、方头括号样式：会话空闲时重建 Engine 换字，View 的 `punctuation_marks` 随之更新键面。
    "quote_style", "bracket_style",
    // Whole objects: the app merges single fields into them, and the document's copy is the one it wrote.
    "quanpin", "mixed_input", "multilingual_input", "quanpin_helpcode", "shuangpin_helpcode", "local_modes",
    // 整句联想 on the 输入 page: the session attaches or drops the keyboard sentence model once idle, so turning 增强 off stops it in the keyboard that is already open.
    "sentence_association",
    // 输入页五笔组的混拼与四码唯一自动上屏：都是引擎字段，会话空闲时重建 Engine 应用它，所以要交给活着的会话，而不等键盘扩展被杀掉后重建会话。
    "wubi_mixed_pinyin",
    "wubi_auto_commit_unique",
    "wubi_candidate_selection",
    // 退格回退已选词段由 Runtime 立即应用，不等待组字结束。
    "backspace_reverts_selection",
    // Laid over the document by `hostOverrides` from the iOS switch, so a change to that switch reaches the live session too.
    "cloud_candidates",
    // Laid over the document from the iOS page size, like cloud candidates; the session applies it once idle, so an open composition keeps its page.
    "candidate_page_size",
    // 辅助码页选用导入的码表（#7176）写的是 `plugins.helpcode_pack_*`：会话空闲时用新选择重建 Engine，聚焦时再按插件目录的戳换表，所以要交给活着的会话。iOS 键盘自己不写 `plugins` 里的任何键，整个对象以文档为准。
    "plugins",
  ]

  /// Hand the reloaded app-edited fields to the session, leaving every other field as the session has it. The session queues the change behind an open composition, so this never interrupts typing.
  private func applyAppEditedPreferences(from reloaded: [String: Any], over applied: [String: Any]) {
    var next = applied
    for key in Self.appEditedKeys { next[key] = reloaded[key] }
    guard handle != 0, !NSDictionary(dictionary: next).isEqual(to: applied) else { return }
    revision &+= 1
    let sent = Self.engineBound(next)
    let snapshot: [String: Any] = ["format_version": 1, "revision": revision, "preferences": sent]
    appliedPreferences = nil
    if let response = try? Self.callUpdate(msimeClientUpdatePreferences, handle, snapshot),
       (try? Self.preferencesUpdateSucceeded(response)) == true {
      appliedPreferences = sent
    }
  }

  var fuzzyPinyinRulesApplied: UInt32? { appliedFuzzyPinyinRules }

  /// 最新的规范 PreferencesStore 文档，只读地暴露给键盘自己的读取（五笔剩余编码提示）和原生设置。
  /// Swift桥接状态由调用方串行访问，C会话的线程归属由固定会话线程独立保证。
  var sharedPreferences: [String: Any]? {
    options["preferences"] as? [String: Any]
  }

  /// Show touch keyboard geometry on the live session while it is being dragged. The shared snapshot is the source that is reloaded when the extension appears.
  @discardableResult
  func setTouchKeyboardGeometry(keySpacing: Double, rowSpacing: Double, heightAdjustment: Double) -> Bool {
    guard let mapping = Self.geometryMapping(keySpacing: keySpacing, rowSpacing: rowSpacing,
                                             heightAdjustment: heightAdjustment) else { return false }
    return updatePreferences(mapping)
  }

  /// Commit the geometry the drag has been previewing.
  ///
  /// The drag emits on every gesture frame, so only the live session follows it; the shared
  /// document is written once, when the user lets go of the grip or flips the switch.
  ///
  /// 返回值是共享文档有没有写成。组字中会话会把偏好推迟到空闲时再应用（`deferred`），这不是失败：键距、行距、高度只由宿主自己读，文档照样要写。原先推迟了就不写文档，下次唤出键盘时文档里的旧高度又把这次调整盖了回去。
  @discardableResult
  func persistTouchKeyboardGeometry(keySpacing: Double, rowSpacing: Double, heightAdjustment: Double) -> Bool {
    guard let mapping = Self.geometryMapping(keySpacing: keySpacing, rowSpacing: rowSpacing,
                                             heightAdjustment: heightAdjustment) else { return false }
    _ = updatePreferences(mapping)
    return persistSharedPreferences(mapping)
  }

  /// 只写传了值的那几项。设置 App 一次只改一项，只写这一项，就不会把它页面上停留着的旧值（比如键盘里刚调过的高度）写回文档。`touch_voice_shortcut` 是 Android 的工具栏语音按钮，iOS 不写它，免得键盘里调一下高度就把别的设备同步来的值盖掉。
  static func geometryMapping(keySpacing: Double? = nil, rowSpacing: Double? = nil,
                              heightAdjustment: Double? = nil) -> ((inout [String: Any]) -> Void)? {
    guard [keySpacing, rowSpacing, heightAdjustment].allSatisfy({ $0?.isFinite ?? true }) else { return nil }
    let keySpacingTenths = keySpacing.map { Int((SharedNumber.clamped($0, to: 3...6) * 10).rounded()) }
    let rowSpacingTenths = rowSpacing.map { Int((SharedNumber.clamped($0, to: 4...10) * 10).rounded()) }
    let clampedHeight = heightAdjustment.map { Int(SharedNumber.clamped($0, to: -12...48).rounded()) }
    return { preferences in
      if let keySpacingTenths { preferences["touch_key_spacing_tenths"] = keySpacingTenths }
      if let rowSpacingTenths { preferences["touch_row_spacing_tenths"] = rowSpacingTenths }
      if let clampedHeight { preferences["touch_keyboard_height_adjustment"] = clampedHeight }
    }
  }

  /// Remove touch geometry overrides so canonical defaults are used again.
  @discardableResult
  func resetTouchKeyboardGeometry() -> Bool {
    updateAndPersist { preferences in
      preferences.removeValue(forKey: "touch_key_spacing_tenths")
      preferences.removeValue(forKey: "touch_row_spacing_tenths")
      preferences.removeValue(forKey: "touch_keyboard_height_adjustment")
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
    case .fourteenKey: layout = "fourteen_key"
    case .eighteenKey: layout = "eighteen_key"
    case .nineKeyShuangpin: layout = "nine_key_shuangpin"
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
          let revision = Self.persistSharedPreferences(stateRoot: stateRoot, marksCloudSync: sharesStateRoot, mutate)
    else { return false }
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

  /// 首次引导也准备键盘实际使用的宿主资源和偏好；只校验、准备，不创建输入会话，不要求完全访问。
  static func prepareOnboardingResources() -> Bool {
    guard let resources = PersonalDictionaryBridge.packagedResources else { return false }
    return (try? callOptions(msimeClientPrepareHost,
      bootstrapOptions(resources: resources, stateRoot: nil))) != nil
  }

  /// Change fields of the shared document from the settings app.
  ///
  /// The keyboard picks the change up the next time it appears (`reloadSharedPreferences`). A write that loses the compare-and-swap to the keyboard returns false and changes nothing.
  @discardableResult
  static func updateSharedPreferences(stateRoot: URL? = nil,
                                      _ mutate: (inout [String: Any]) -> Void) -> Bool {
    persistSharedPreferences(stateRoot: sharedStateRoot(stateRoot), marksCloudSync: stateRoot == nil, mutate) != nil
  }

  /// `msime_client_plugins` 失败时的失败码和 client-services 给的原因（码表不合格时带文件名和行号），见 `include/msime_client.h`。
  struct PluginFailure: Error, Equatable {
    let code: String
    var detail: String? = nil
  }

  /// 插件目录的一次操作（`msime_client_plugins`），给设置 App 导入、列出和删除辅助码表、列出按键音效用，不建输入会话。
  ///
  /// `state_root` 是 App Group 里的共享状态目录，键盘的 `preferences_directory` 就是它，所以这里装进 `<state_root>/plugins` 的包键盘读得到。`sound_packs` 是随包内置的音效包目录（`KeySoundPackPolicy.bundledDirectory`），`catalog` 因此也列出内置的按键音包；找不到时传 null，只列装在状态目录里的包。会读写文件，导入时还要校验整张码表：调用方放到主线程之外。`stateRoot` 只给测试用。
  static func plugins(_ action: [String: Any], stateRoot: URL? = nil) -> Result<Any, PluginFailure> {
    let root = sharedStateRoot(stateRoot)
    let request: [String: Any] = ["state_root": root, "sound_packs": bundledSoundPacks, "action": action]
    return pluginCall(msimeClientPlugins, request, valid: !root.isEmpty)
  }

  /// 一个按键音包里校验过的样本（`msime_client_key_sound_pack`），给自己播放按键音的键盘和设置 App 的试听用。内置包从随包的 `sound-packs/` 找，其余的 id 在共享状态目录的 `plugins/` 里找。value 是 `{id, name, mode, sounds:{default, space, enter, backspace, …: 绝对路径|null}, …}`，见 `include/msime_client.h`；由 `KeySoundPackPolicy.Samples` 读。要读包的清单和文件：不要在按键路径上调用。`stateRoot` 和 `soundPacks` 只给测试用。
  static func keySoundPack(_ pack: String, stateRoot: URL? = nil, soundPacks: URL? = nil) -> Result<Any, PluginFailure> {
    let root = sharedStateRoot(stateRoot)
    let request: [String: Any] = ["state_root": root.isEmpty ? NSNull() : root,
                                  "sound_packs": soundPacks?.path ?? bundledSoundPacks, "pack": pack]
    return pluginCall(msimeClientKeySoundPack, request, valid: !pack.isEmpty)
  }

  /// 内置音效包目录的绝对路径，没有时是 JSON null。
  private static var bundledSoundPacks: Any { KeySoundPackPolicy.bundledDirectory()?.path ?? NSNull() }

  /// `msime_client_plugins` 一类入口的一次调用：请求编成 JSON 交给 `entry`，读回标准应答。
  private static func pluginCall(_ entry: (UnsafePointer<MSIMEByte>?, UInt) -> UnsafeMutablePointer<CChar>?,
                                 _ request: [String: Any], valid: Bool) -> Result<Any, PluginFailure> {
    guard valid, JSONSerialization.isValidJSONObject(request),
          let data = try? JSONSerialization.data(withJSONObject: request) else {
      return .failure(PluginFailure(code: "invalid"))
    }
    let pointer = data.withUnsafeBytes { bytes in
      entry(bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(data.count))
    }
    guard let pointer else { return .failure(PluginFailure(code: "unavailable")) }
    defer { msimeClientStringFree(pointer) }
    guard let envelope = (try? JSONSerialization.jsonObject(with: Data(bytes: pointer, count: strlen(pointer))))
      as? [String: Any] else { return .failure(PluginFailure(code: "invalid_response")) }
    if envelope["ok"] as? Bool == true { return .success(envelope["value"] ?? NSNull()) }
    return .failure(PluginFailure(code: envelope["error"] as? String ?? "", detail: envelope["detail"] as? String))
  }

  /// `msime_client_save_preferences`'s snapshot bound: large enough for a custom skin's photo.
  private static let preferencesDocumentLimit = 1_048_576

  /// The App Group directory holding the shared preference document, where the keyboard also keeps its diagnostic log.
  static var sharedStateDirectory: String { sharedStateRoot(nil) }

  private static func sharedStateRoot(_ override: URL?) -> String {
    bootstrapOptions(resources: nil, stateRoot: override)["state_root"] as? String ?? ""
  }

  /// Returns the revision the document is at afterwards, or nil when nothing was written.
  ///
  /// Every write that changes the App Group document (`marksCloudSync`) is a settings change, from the settings app or from the keyboard, so it marks the settings section for the next cloud sync. A mutation that leaves the document as it was writes nothing and marks nothing; cloud sync applying the cloud's settings runs inside `CloudSyncState.suppressingMarks`.
  private static func persistSharedPreferences(stateRoot: String, marksCloudSync: Bool,
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
    if marksCloudSync { CloudSyncState.shared.markDirty(.settings) }
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

  /// 键盘功能菜单里的模糊音：在运行中的会话和共享文档里切换 `fuzzy_pinyin.enabled`，保留设置 app 选好的规则。如果文档里一条规则都没选，或者从未初始化过，打开时会启用全部规则，这样开关总会改变 Engine 接受的输入。
  ///
  /// 新对象只构建一次，能读到文档时以文档为基础，同一个对象同时交给两处：开关关闭时会话自己那份会丢掉规则列表（`setFuzzyPinyinRules` 传给它的是空集），基于它构建就会把全部规则重新打开，盖掉用户的选择。没有完全访问权限时文档写入会失败，改动只留在运行中的会话里。
  @MainActor
  @discardableResult
  func setFuzzyPinyinEnabled(_ enabled: Bool) -> Bool {
    let document = stateRoot.flatMap(Self.storedPreferences) ?? sharedPreferences
    let stored = document?[FuzzyPinyinPreference.documentKey] as? [String: Any] ?? [:]
    var rules = (stored["rules"] as? [String] ?? []).filter(FuzzyPinyinPreference.ruleIDs.contains)
    var fuzzy = stored
    // `PreferencesStore` 在从未初始化的文档第一次启用时会选中全部规则；这里照做，会话那份才与存储写入的内容一致。
    if enabled && (rules.isEmpty || stored["seeded"] as? Bool != true) {
      rules = FuzzyPinyinPreference.ruleIDs
      fuzzy["seeded"] = true
    }
    fuzzy["enabled"] = enabled
    fuzzy["rules"] = rules
    let object = fuzzy
    guard updateAndPersist({ $0[FuzzyPinyinPreference.documentKey] = object }) else { return false }
    // 会话现在已经在用这些规则；记下来，免得键盘下一轮学习时又把这些位交给它一遍。
    appliedFuzzyPinyinRules = FuzzyPinyinPreference.settings(in: sharedPreferences)?.bits ?? 0
    return true
  }

  /// `stateRoot` 里按原样存储的共享文档，不经过会话。
  private static func storedPreferences(stateRoot: String) -> [String: Any]? {
    let directory = Data(stateRoot.utf8)
    guard !directory.isEmpty, directory.count <= 16_384 else { return nil }
    return (try? callDirectory(msimeClientLoadPreferences, directory))?["preferences"] as? [String: Any]
  }

  func handleCharacter(_ character: String, shifted: Bool = false) -> MetasequoiaInputSnapshot {
    dispatch { pointer(for: character, shift: shifted) }
  }

  /// 会话里注册着的键位几何代次（#10018）；还没注册、注册被拒，或会话重建后句柄换了时为 nil，键盘要先重新注册再带落点打字。
  var keyGeometryID: UInt32? {
    guard let keyGeometry, keyGeometry.handle == handle, handle != 0 else { return nil }
    return keyGeometry.id
  }
  private var keyGeometry: (handle: UInt64, id: UInt32)?

  /// 注册 26 键的字母键几何（`TouchPointPolicy.request` 拼出的 JSON，代次是 `id`），只在布局变化后调用。引擎拒绝时返回 false，`keyGeometryID` 随之为 nil：引擎里原来那份几何虽然还在，键盘不再按任何代次带落点，下一键重新注册。
  @discardableResult
  func setKeyGeometry(_ request: Data, id: UInt32) -> Bool {
    flushPendingCharacters()
    keyGeometry = nil
    guard handle != 0, id != 0, !request.isEmpty, request.count <= 16_384 else { return false }
    let accepted = request.withUnsafeBytes { bytes in
      (try? Self.decode(msimeClientSetKeyGeometry(
        handle, bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(request.count)))) != nil
    }
    if accepted { keyGeometry = (handle, id) }
    return accepted
  }

  /// 26 键双拼的一键带上按下点 (x, y)（键区坐标，与 `geometryID` 那份几何同一个坐标系）。代次不符、坐标不是有限数或当前不是 26 键双拼时，引擎的回应与 `handleCharacter(character)` 逐字节相同。
  func handleTouchCharacter(_ character: String, geometryID: UInt32, x: Float, y: Float) -> MetasequoiaInputSnapshot {
    dispatch {
      guard let byte = Self.ascii(character) else { return nil }
      return msimeClientTouchCharacter(handle, byte, false, geometryID, x, y)
    }
  }

  /// 滑行输入：抬手时把一笔滑行的请求（`GlideTyping.request`）交给 Engine，回应与 `handleCharacter` 的一样。方案不是全拼、处于本地模式或这一笔解不出音节时 Engine 不处理（`isHandled` 为 false），宿主丢掉这一笔。
  func glide(_ request: Data) -> MetasequoiaInputSnapshot {
    guard !request.isEmpty, request.count <= 65_536 else { return diagnostic("滑行输入无效") }
    return request.withUnsafeBytes { bytes in
      dispatch { msimeClientGlide(handle, bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(request.count)) }
    }
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
    flushPendingCharacters()
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
    flushPendingCharacters()
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
    flushPendingCharacters()
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
    return dispatch(preservingViewOnRejection: true) { msimeClientSelect(handle, generation, index) }
  }

  /// Select an entry of the expanded panel. Panel positions index the Engine's whole answer, and `selectCandidate(generation:globalIndex:)` only accepts the page the strip is showing, so anything past the ninth candidate came back as stale.
  func selectAnyCandidate(generation: UInt64, globalIndex: UInt64) -> MetasequoiaInputSnapshot {
    guard let index = UInt(exactly: globalIndex) else { return diagnostic("候选已失效") }
    return dispatch(preservingViewOnRejection: true) { msimeClientSelectAnyCandidate(handle, generation, index) }
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
    return dispatch(preservingViewOnRejection: true) { msimeClientSelectEdge(handle, generation, index, last ? 1 : 0) }
  }

  func allCandidates() throws -> [String: Any] {
    flushPendingCharacters()
    return try Self.callHandle(msimeClientAllCandidates, handle)
  }

  /// Read-only English completion lookup for the word immediately before the cursor.
  /// The caller owns the document-context parsing; the Engine session remains untouched.
  func englishCompletions(forPrefix prefix: String, limit: Int) -> [String] {
    flushPendingCharacters()
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

  /// 英文纠错在输入线程上的状态。Engine 把纠错词典放在调用线程的线程局部变量里，所以查询一律在 `MSIMEInputThread` 这一条常驻线程上做：主线程不再另开一份词典连接，也不在每个字母上跑整段纠错搜索。
  private final class EnglishCorrectionWork: @unchecked Sendable {
    private let lock = NSLock()
    private var latestRequest: UInt64 = 0
    /// 只在输入线程上读写：最近一次算出的结果。打完一个词按空格或标点时，同一个词刚在最后一个字母上算过，这里直接取用，不再算第二遍。
    var memo: (resources: String, prefix: String, items: [String])?
    var suggestionMemo: (resources: String, prefix: String, items: [String], corrected: Bool)?

    func begin() -> UInt64 {
      lock.lock()
      defer { lock.unlock() }
      latestRequest &+= 1
      return latestRequest
    }

    func isLatest(_ request: UInt64) -> Bool {
      lock.lock()
      defer { lock.unlock() }
      return request == latestRequest
    }
  }
  private let englishCorrectionWork = EnglishCorrectionWork()

  /// 从共享引擎读取原文和唯一纠错；查询不改变键盘组字，也不学习当前词。同步版本，在输入线程上算完再返回，空格、标点自动纠错要在上屏前拿到结果。
  func englishCorrections(forPrefix prefix: String) -> [String] {
    guard let resources = englishCorrectionResources(forPrefix: prefix) else { return [] }
    let work = englishCorrectionWork
    return MSIMEInputThread.shared.sync { Self.englishCorrections(prefix: prefix, resources: resources, work: work) }
  }

  /// 异步版本：在输入线程上算，结果回到主线程交给 `completion`。快速连打时排在队里、已被更新请求取代的查询直接跳过、不回调，只有最后一次请求的结果会送回；调用方仍要在回调里核对当前的词。
  func englishCorrections(forPrefix prefix: String, completion: @escaping ([String]) -> Void) {
    precondition(Thread.isMainThread)
    let work = englishCorrectionWork
    let request = work.begin()
    guard let resources = englishCorrectionResources(forPrefix: prefix) else {
      completion([])
      return
    }
    MSIMEInputThread.shared.async {
      guard work.isLatest(request) else { return }
      let items = Self.englishCorrections(prefix: prefix, resources: resources, work: work)
      DispatchQueue.main.async {
        guard work.isLatest(request) else { return }
        completion(items)
      }
    }
  }

  private func englishCorrectionResources(forPrefix prefix: String) -> String? {
    guard handle != 0, !prefix.isEmpty, prefix.utf8.count <= 128,
          let resources = options["resources"] as? String,
          NSString(string: resources).isAbsolutePath else { return nil }
    return resources
  }

  /// 只在输入线程上调用。
  private static func englishCorrections(prefix: String, resources: String,
                                         work: EnglishCorrectionWork) -> [String] {
    if let memo = work.memo, memo.resources == resources, memo.prefix == prefix { return memo.items }
    let items = englishCorrections(prefix: prefix, resources: resources)
    work.memo = (resources, prefix, items)
    return items
  }

  private static func englishCorrections(prefix: String, resources: String) -> [String] {
    englishSuggestionRequest(prefix: prefix, resources: resources, completions: false)?.items ?? []
  }

  /// 候选栏的合并展示上限：原文加至多十一个前缀补全，与 Android、HarmonyOS 相同。
  static let englishSuggestionDisplayLimit = 12

  /// 完整展示仍在同一输入线程上计算，保持共享引擎的纠错与补全规则。
  func englishSuggestions(forPrefix prefix: String) -> (items: [String], corrected: Bool) {
    guard let resources = englishCorrectionResources(forPrefix: prefix) else { return ([], false) }
    let work = englishCorrectionWork
    return MSIMEInputThread.shared.sync { Self.englishSuggestions(prefix: prefix, resources: resources, work: work) }
  }

  /// 异步候选展示保留纠错标记；过期请求在计算前和回主线程后均丢弃。
  func englishSuggestions(forPrefix prefix: String,
                          completion: @escaping ((items: [String], corrected: Bool)) -> Void) {
    precondition(Thread.isMainThread)
    let work = englishCorrectionWork
    let request = work.begin()
    guard let resources = englishCorrectionResources(forPrefix: prefix) else {
      completion(([], false))
      return
    }
    MSIMEInputThread.shared.async {
      guard work.isLatest(request) else { return }
      let result = Self.englishSuggestions(prefix: prefix, resources: resources, work: work)
      DispatchQueue.main.async {
        guard work.isLatest(request) else { return }
        completion(result)
      }
    }
  }

  /// 仅输入线程访问缓存；候选展示的唯一纠错也供随后的空格、标点边界复用。
  private static func englishSuggestions(prefix: String, resources: String,
                                        work: EnglishCorrectionWork) -> (items: [String], corrected: Bool) {
    if let memo = work.suggestionMemo, memo.resources == resources, memo.prefix == prefix {
      return (memo.items, memo.corrected)
    }
    let result = englishSuggestionRequest(prefix: prefix, resources: resources, completions: true) ?? ([], false)
    work.suggestionMemo = (resources, prefix, result.0, result.1)
    work.memo = (resources, prefix, result.0.isEmpty ? [] : (result.1 ? result.0 : [prefix]))
    return result
  }

  private static func englishSuggestionRequest(prefix: String, resources: String,
                                              completions: Bool) -> (items: [String], corrected: Bool)? {
    let limit = completions ? englishSuggestionDisplayLimit : 2
    var fields: [String: Any] = ["prefix": prefix, "limit": limit, "correction": true]
    if completions { fields["completions"] = true }
    do {
      let request = try JSONSerialization.data(withJSONObject: fields)
      let resourceData = Data(resources.utf8)
      let response = try request.withUnsafeBytes { requestBytes in
        try resourceData.withUnsafeBytes { resourceBytes in
          try Self.decode(msimeClientEnglishCompletionsRequest(
            requestBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(request.count),
            resourceBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(resourceData.count)))
        }
      }
      guard let value = response as? [String: Any], value["prefix"] as? String == prefix,
            let items = value["items"] as? [String], items.first == prefix, items.count <= limit else { return nil }
      let corrected = completions && value["corrected"] as? Bool == true
      if corrected && items.count != 2 { return nil }
      return (items, corrected)
    } catch { return nil }
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

  /// 候选英文音标（`CandidatePronunciationModel.request`）：不持有会话，只读本机的音标表，在键盘的释义工作队列上调用。
  static func candidatePronunciations(request: Data, resources: String) throws -> [String: Any] {
    let resourceData = Data(resources.utf8)
    guard !request.isEmpty, request.count <= CandidateGlossModel.maxRequestBytes,
          NSString(string: resources).isAbsolutePath, resourceData.count <= 4096 else {
      throw InputBridgeFailure.invalidResponse
    }
    return try request.withUnsafeBytes { requestBytes in
      try resourceData.withUnsafeBytes { resourceBytes in
        let value = try decode(msimeClientPronunciationRequest(
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
    flushPendingCharacters()
    return try translations.withUnsafeBytes { bytes in
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
    flushPendingCharacters()
    guard handle != 0, let value = try? Self.decode(msimeClientOnlineQuery(handle)),
          let query = value as? [String: Any],
          let data = try? JSONSerialization.data(withJSONObject: query) else { return nil }
    return data
  }

  /// 兼容 GET 查询的共享云候选地址；完整请求通过在线查询描述符执行。
  static func cloudRequestURL(query: Data) -> URL? {
    guard !query.isEmpty, query.count <= 131_072 else { return nil }
    let value = query.withUnsafeBytes { bytes in
      try? decode(msimeClientCloudRequestURL(bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(query.count)))
    }
    guard let text = value as? String, let url = URL(string: text), url.scheme == "https" else { return nil }
    return url
  }

  /// Hand a fetched cloud reply to the shared parser. Returns `{applied, view}`.
  func applyCloudResponse(query: Data, body: Data) throws -> [String: Any] {
    flushPendingCharacters()
    guard handle != 0 else { throw InputBridgeFailure.unavailable }
    guard !query.isEmpty, query.count <= 131_072, !body.isEmpty, body.count <= 262_144 else {
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
    flushPendingCharacters()
    guard handle != 0, !query.isEmpty, query.count <= 131_072 else { return nil }
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

  static func aiTranslationRequest(request: [String: Any]) -> [String: Any]? {
    guard JSONSerialization.isValidJSONObject(request),
          let data = try? JSONSerialization.data(withJSONObject: request), data.count <= 65_536 else { return nil }
    return data.withUnsafeBytes { bytes in
      (try? decode(msimeClientAITranslationHTTPRequest(bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(data.count)))) as? [String: Any]
    }
  }

  static func parseAITranslationResponse(body: Data) -> String? {
    guard !body.isEmpty, body.count <= 1_048_576 else { return nil }
    return body.withUnsafeBytes { bytes in
      (try? decode(msimeClientParseAITranslationResponse(bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(body.count)))) as? String
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
    flushPendingCharacters()
    guard handle != 0 else { throw InputBridgeFailure.unavailable }
    let payload = try JSONSerialization.data(withJSONObject: candidates)
    guard !query.isEmpty, query.count <= 131_072, payload.count <= 16_384, source <= 1 else {
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

  /// AI 回答为空、失败或被拒时调用（`source` 只认 1）。共享内核已经不放 AI 占位行，这个调用不改候选列表，总是答 `applied:false`；保留它只为宿主和 C ABI 不变。Returns `{applied, view}`.
  func resolveOnlineCandidates(query: Data, source: UInt8) throws -> [String: Any] {
    flushPendingCharacters()
    guard handle != 0 else { throw InputBridgeFailure.unavailable }
    guard !query.isEmpty, query.count <= 131_072, source <= 1 else {
      throw InputBridgeFailure.invalidResponse
    }
    return try query.withUnsafeBytes { queryBytes in
      guard let dictionary = try Self.decode(msimeClientResolveOnlineCandidates(
        handle, queryBytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(query.count), source))
        as? [String: Any] else {
        throw InputBridgeFailure.invalidResponse
      }
      return dictionary
    }
  }

  /// 偏好投影变更后读取新候选，不发送按键或改变组字。
  func currentSnapshot() -> MetasequoiaInputSnapshot {
    do { return try Self.snapshot(["view": view()]) }
    catch { return diagnostic(error.localizedDescription) }
  }

  func snapshot(from value: [String: Any]) throws -> MetasequoiaInputSnapshot {
    try Self.snapshot(value)
  }

  func chooseNineKeySpelling(at index: UInt) -> MetasequoiaInputSnapshot {
    let generation = (try? view()).flatMap { Self.strictUInt64($0["generation"]) } ?? 0
    return chooseNineKeySpelling(generation: generation, at: index)
  }

  func chooseNineKeySpelling(generation: UInt64, at index: UInt) -> MetasequoiaInputSnapshot {
    dispatch(preservingViewOnRejection: true) { msimeClientChooseNineKeySpelling(handle, generation, index) }
  }

  /// 九键展开面板的候选筛选：`singleCharacter` 只留单字，`strokes` 是首字笔顺前缀（h 横、s 竖、p 撇、n 点、z 折，最多 64 笔，空串表示不按笔画）。两项一起交给 Engine，作用到这次组字结束；当前状态从快照的 `nineKeySingleCharacter`、`nineKeyStrokes` 读。
  func setNineKeyFilter(singleCharacter: Bool, strokes: String) -> MetasequoiaInputSnapshot {
    let bytes = Array(strokes.utf8)
    guard bytes.count <= Self.nineKeyStrokeLimit else { return diagnostic("笔画太多") }
    return bytes.withUnsafeBufferPointer { buffer in
      dispatch { msimeClientSetNineKeyFilter(handle, singleCharacter, buffer.baseAddress, UInt(buffer.count)) }
    }
  }

  /// 与 host-api 的 `NINE_KEY_STROKE_LIMIT` 相同。
  static let nineKeyStrokeLimit = 64

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
  /// 双拼 14 键：先切到选定的双拼方案，再打开共享引擎的 14 键网格。
  func switchToFourteenKeyShuangpin(profile: String) -> MetasequoiaInputSnapshot {
    switchToShuangpinGrid(.fourteenKey, profile: profile)
  }
  /// 双拼 18 键：先切到选定的双拼方案，再打开共享引擎的双拼 18 键网格；横滑的精确字母经 `gridKey` 以大写送入。
  func switchToEighteenKeyShuangpin(profile: String) -> MetasequoiaInputSnapshot {
    switchToShuangpinGrid(.eighteenKeyShuangpin, profile: profile)
  }
  private func switchToShuangpinGrid(_ grid: KeyGrid, profile: String) -> MetasequoiaInputSnapshot {
    let snapshot = self.switch(toShuangpinProfile: profile)
    guard snapshot.diagnosticText == nil else { return snapshot }
    keyGrid = grid
    return dispatch { msimeClientSetKeyGrid(handle, grid.rawValue) }
  }
  func switchToNineKeyShuangpin() -> MetasequoiaInputSnapshot { switchToKeyGrid(.nineKeyShuangpin) }

  func switchToNineKey() -> MetasequoiaInputSnapshot { switchToKeyGrid(.nineKey) }
  /// 全拼 14 键：方案是全拼，按键经 `gridKey` 送组码。
  func switchToFourteenKey() -> MetasequoiaInputSnapshot { switchToKeyGrid(.fourteenKey) }
  /// 全拼 18 键：方案是全拼，按键经 `gridKey` 送组码。
  func switchToEighteenKey() -> MetasequoiaInputSnapshot { switchToKeyGrid(.eighteenKey) }

  /// 先把方案切到全拼，再开网格：引擎只给全拼开九键、14 键和 18 键，方案改变时还会丢掉宿主之前设下的网格。只在组字结束后调用。
  private func switchToKeyGrid(_ grid: KeyGrid) -> MetasequoiaInputSnapshot {
    guard updatePreferences({ $0["scheme"] = "quanpin" }) else {
      let failure = switch grid {
      case .fourteenKey: "14 键模式切换失败"
      case .eighteenKey: "18 键模式切换失败"
      default: "九键模式切换失败"
      }
      return diagnostic(failure)
    }
    keyGrid = grid
    return dispatch { msimeClientSetKeyGrid(handle, grid.rawValue) }
  }

  /// 字母网格的一键：`letter` 是这一组的首字母（14 键 q e t u o a d g j l z c b m，18 键是各组的首字母），引擎把整组当作一个组码；双拼 18 键另收大写字母，表示横滑选中的精确字母。不在字母网格下、在本地模式里或 26 键正在组字时引擎不处理（`isHandled` 为 false），会话不变。
  func gridKey(_ letter: String) -> MetasequoiaInputSnapshot {
    dispatch { Self.ascii(letter).flatMap { msimeClientGridKey(handle, $0) } }
  }
  /// Tell the runtime the width it commits in; from then on every commit it completes arrives already converted.
  @discardableResult func setCharacterWidth(fullwidth: Bool) -> MetasequoiaInputSnapshot {
    self.fullwidth = fullwidth
    return dispatch { msimeClientSetCharacterWidth(handle, fullwidth) }
  }
  /// 把隐私模式告诉运行时（与 Android 的 `markPrivateSession` 相同）：隐私会话不记选词位置和上屏效率。学习本身仍由偏好里的 `learning` 决定。返回运行时是否接受。
  @discardableResult func setPrivateSession(_ enabled: Bool) -> Bool {
    flushPendingCharacters()
    if handle != 0, privateSessionHandle == handle, enabled == privateSession { return true }
    privateSession = enabled
    guard handle != 0 else { return false }
    let accepted = (try? Self.decode(msimeClientSetPrivateSession(handle, enabled))) != nil
    privateSessionHandle = accepted ? handle : 0
    return accepted
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
    flushPendingCharacters()
    guard handle != 0 else { return false }
    let bytes = Array((aiCredential ?? "").utf8)
    let response: Any? = try? bytes.withUnsafeBufferPointer { buffer -> Any in
      try Self.decode(msimeClientSetAICredential(handle, buffer.baseAddress, UInt(buffer.count)))
    }
    return response as? Bool == true
  }
  /// 只改 `scheme`，会话里的 `wubi_profile` 不动，所以切过去仍是当前的 86 或 98 五笔。
  func switchToWubi() -> MetasequoiaInputSnapshot { switchScheme("wubi", profile: nil) }
  /// 五笔 14 键：先切到五笔（86/98 码表照旧），再打开共享引擎的 14 键网格，引擎按组展开码表；按键经 `gridKey` 送各组首字母。网格记在 `keyGrid` 里，重建会话时重放。
  func switchToFourteenKeyWubi() -> MetasequoiaInputSnapshot {
    let snapshot = switchToWubi()
    guard snapshot.diagnosticText == nil else { return snapshot }
    keyGrid = .fourteenKey
    return dispatch { msimeClientSetKeyGrid(handle, KeyGrid.fourteenKey.rawValue) }
  }
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
      return dispatch(preservingViewOnRejection: true) { msimeClientPinCandidate(handle, generation, indexValue) }
    case .remove:
      return dispatch(preservingViewOnRejection: true) { msimeClientRemoveCandidate(handle, generation, indexValue) }
    case .fix(let position):
      return dispatch(preservingViewOnRejection: true) { msimeClientFixCandidatePosition(handle, generation, indexValue, position) }
    case .clearPosition:
      return dispatch(preservingViewOnRejection: true) { msimeClientClearCandidatePosition(handle, generation, indexValue) }
    }
  }

  func openLocalMode(_ trigger: String) -> MetasequoiaInputSnapshot {
    guard let byte = Self.ascii(trigger) else { return diagnostic("本地输入模式无效") }
    return dispatch { pointer(for: String(UnicodeScalar(byte)), shift: true) }
  }

  /// 设置与键面共用引擎的方案表和带位置的校验结果，不创建输入会话。
  static func nineKeyShuangpinLayout(profile: String, table: NineKeyShuangpinLayout.Table? = nil) throws -> NineKeyShuangpinLayout {
    var request: [String: Any] = ["profile": profile]
    if let table { request["table"] = table }
    let data = try JSONSerialization.data(withJSONObject: request)
    let value = try data.withUnsafeBytes { bytes in
      try decode(msimeClientNineKeyShuangpinLayout(bytes.bindMemory(to: MSIMEByte.self).baseAddress, UInt(data.count)))
    }
    return try JSONDecoder().decode(NineKeyShuangpinLayout.self, from: JSONSerialization.data(withJSONObject: value))
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

  /// 粤语双拼的键位提示，和 `shuangpinKeyHints` 一样从引擎取：引擎按粤语双拼编码用的同一张表生成，宿主不另存键位。粤语双拼不是普通话双拼方案，没有方案名，用保留名 `cantonese` 取。会话跑的不是粤拼时（比如缺了 `msime-cantonese.db`，运行时留在上一个中文方案）给空表，免得把键标成没在跑的方案。
  func cantoneseShuangpinKeyHints() -> [String: String] {
    guard let snapshot = try? view(),
          Self.strictUInt64(snapshot["scheme"]) == UInt64(Self.cantoneseSchemeCode),
          let data = "cantonese".data(using: .utf8) else { return [:] }
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
    flushPendingCharacters()
    guard !suspended else { return true }
    guard !hasComposition else { return false }
    if handle != 0 {
      guard (try? Self.decode(msimeClientDestroy(handle))) != nil else { return false }
      handle = 0
    }
    suspended = true
    return true
  }

  /// 会话在收起时销毁、还没在下一次出现时重建：这时 `resumeDictionarySession` 要同步建一整个会话。
  var dictionarySessionSuspended: Bool { suspended }

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
  /// 九键和 14 键的网格在会话上，所以每条销毁再重建会话的路径都要把它再设一次。词库维护（应用编辑、导出）和快照激活也会重建会话：把重放留给调用方时，键盘出现后第一次刷新个人词库（那时列表也走维护路径），宿主画的还是九键，引擎却已经回到 26 键。
  private func createFocusedSession() throws {
    // 选中只吃掉部分输入的候选时，让运行时把已选的那一段留在组字里而不是立刻上屏：用户还在打后
    // 半截，前半截已经进了文档的话，搜索框会拿半个词去搜，编辑器为它记一次撤销。键盘把它画在候选
    // 条的读音前面，两件事必须一起做（scripts/test-phrase-preedit-hosts.py 守的就是这一半状态）。
    var requested = engineOptions
    requested["phrase_preedit"] = true
    // 新会话的偏好来自建会话的选项，不是上一个会话最后接受的那份；隐私标记也要重新确认。
    appliedPreferences = nil
    privateSessionHandle = 0
    handle = try Self.callCreateFocused(requested)
    if keyGrid != .none { _ = dispatch { msimeClientSetKeyGrid(handle, keyGrid.rawValue) } }
    if fullwidth { _ = dispatch { msimeClientSetCharacterWidth(handle, true) } }
    if let chinesePunctuation { _ = dispatch { msimeClientSetChinesePunctuation(handle, chinesePunctuation) } }
    if aiCredential != nil { _ = applyAICredential() }
    if privateSession {
      if (try? Self.decode(msimeClientSetPrivateSession(handle, true))) != nil { privateSessionHandle = handle }
    } else {
      // 新会话本来就不是隐私会话。
      privateSessionHandle = handle
    }
  }

  /// 主线程复制选项；后台版本读取不访问可变会话，也不占用按键线程。
  func dictionaryStateVersionOptions() throws -> Data {
    // 初始化失败时保留原诊断，不让空选项的版本错误盖掉它。
    if let initializationDiagnostic, options["user_data"] == nil {
      throw InputBridgeFailure.response(initializationDiagnostic)
    }
    return try JSONSerialization.data(withJSONObject: engineOptions)
  }

  func localDictionaryStateVersion() throws -> String {
    try Self.localDictionaryStateVersion(preparedOptions: dictionaryStateVersionOptions())
  }

  /// 这个 C ABI 只读文件版本，不使用线程绑定的会话句柄。
  static func localDictionaryStateVersion(preparedOptions: Data) throws -> String {
    let result = try callDirectory(msimeClientSnapshotVersion, preparedOptions)
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
            "preparedOptions": try JSONSerialization.data(withJSONObject: engineOptions)]
  }

  func activateDictionarySnapshot(_ snapshot: MSIMEPreparedDictionarySnapshot,
                                  expectedVersion: String) throws {
    flushPendingCharacters()
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
    let request: [String: Any] = ["options": engineOptions, "action": action]
    let result = try withDictionaryMaintenance {
      try Self.callOptions(msimeClientDictionary, request)
    }
    guard (result["applied"] as? Bool) == true else { throw InputBridgeFailure.response("个人词条未能应用") }
  }

  /// 设置主应用的更新入口不建 Engine 会话，不取得键盘的维护锁；后台任务只写独立的资源代次。
  static func dictionaryUpdates(_ command: String, enabled: Bool? = nil) throws -> [String: Any] {
    guard let resources = PersonalDictionaryBridge.packagedResources else { throw InputBridgeFailure.unavailable }
    let root = sharedStateDirectory
    var options: [String: Any] = ["api_version": 1, "resources": resources.path,
      "user_data": root + "/user", "cache": root + "/cache", "dictionaries": root + "/user/dictionaries",
      "preferences_directory": root, "preferences": loadSharedPreferences() ?? [:]]
    if !MSIMEAppEdition.isFull { options["edition"] = MSIMEAppEdition.identifier }
    var action: [String: Any] = ["operation": "updates", "command": command]
    if let enabled { action["enabled"] = enabled }
    return try callOptions(msimeClientDictionary, ["options": options, "action": action])
  }

  /// 「清除学习记录」失败的两种情况：键盘会话还开着（Rust 报 `dictionary maintenance busy`），或者别的原因。
  enum LearnedDataResetFailure: Error, Equatable {
    case busy
    case failed
  }

  /// 设置 App「词库 › 学习」的「清除学习记录」：与五笔查码、词库同步一样经 `msime_client_prepare_host` 解析出键盘用的那套词库路径，发词库请求 `reset`，换回随包的工作词库、清空学习日志（调频、置顶、固定位置、选词计数、整句联想、拼写纠错和自造词），共享偏好、常用语、剪贴板和打字统计不动。Rust 要独占的维护权；键盘扩展收起时就销毁会话放掉它，键盘还开着时是 `.busy`，什么也不换。键盘扩展有没有完全访问，状态目录都是 App Group 里的同一个 `MSIME`（`bootstrapOptions`），没有另一份：没开完全访问时键盘写不了 App Group，它学到的东西本来就落不了盘，所以这里清的就是键盘下次读的那一份。读写文件，调用方放到主线程之外。
  static func resetLearnedData(resources: URL? = PersonalDictionaryBridge.packagedResources,
                               stateRoot: URL? = nil) throws {
    guard let resources else { throw LearnedDataResetFailure.failed }
    let result: [String: Any]
    do {
      let options = try callOptions(msimeClientPrepareHost, bootstrapOptions(resources: resources, stateRoot: stateRoot))
      result = try callOptions(msimeClientDictionary, ["options": options, "action": ["operation": "reset"]])
    } catch InputBridgeFailure.response("dictionary maintenance busy") {
      throw LearnedDataResetFailure.busy
    } catch {
      throw LearnedDataResetFailure.failed
    }
    guard result["reset"] as? Bool == true else { throw LearnedDataResetFailure.failed }
  }

  /// 五笔查码（#6789）：`text` 里每个不同汉字（以及整段作为一个词）在 86 和 98 码表里的编码。给设置 App 用，不建输入会话：经 `msime_client_prepare_host` 解析出选项后发词库请求 `wubi_codes`，host-api 只读打开会话用的同一份五笔码表，不拿词库维护租约，键盘开着也能查。
  ///
  /// `resources` 默认是 `PersonalDictionaryBridge.packagedResources`：App 自己不带词库，它找到的是键盘扩展包里的 `EngineResources`，和键盘准备宿主时是同一个目录，所以状态目录里的资源校验标记两边一致，查一次码不会让键盘下次启动重新哈希整套词库。在磁盘上读码表，调用方放到主线程之外。
  static func wubiCodes(_ text: String, resources: URL? = PersonalDictionaryBridge.packagedResources,
                        stateRoot: URL? = nil) throws -> WubiCodeLookup {
    guard let resources else { throw InputBridgeFailure.unavailable }
    let options = try callOptions(msimeClientPrepareHost, bootstrapOptions(resources: resources, stateRoot: stateRoot))
    let action: [String: Any] = ["operation": "wubi_codes", "text": text]
    return try WubiCodeLookup.decode(callOptions(msimeClientDictionary, ["options": options, "action": action]))
  }

  /// 词库同步（`IOSDictionarySync`）在设置 App 里要的 HostOptions，编成 JSON：与五笔查码相同，经 `msime_client_prepare_host` 解析，不建输入会话、不拿词库维护租约，资源目录是键盘扩展包里的 `EngineResources`，所以 Engine 的词库路径与键盘的相同。键盘自己用会话准备好的那一份（`dictionaryStateVersionOptions`）。读文件，调用方放到主线程之外。
  static func preparedHostOptions(resources: URL? = PersonalDictionaryBridge.packagedResources,
                                  stateRoot: URL? = nil) throws -> Data {
    guard let resources else { throw InputBridgeFailure.unavailable }
    let options = try callOptions(msimeClientPrepareHost, bootstrapOptions(resources: resources, stateRoot: stateRoot))
    return try JSONSerialization.data(withJSONObject: options)
  }

  /// 词库的一页。不带种类时是用户自己的词条；在某个种类里按编码前缀查，或者列快捷短语时，查的是整份词库，内置行带 `source: bundled` 返回，可以经队列调权重或删除。
  ///
  /// 列表不经 `withDictionaryMaintenance`，活着的会话原样保留：host-api 的 `list` 只取共享租约（`DictionaryAccess::try_session`），和会话自己持有的共享租约并存，只有维护者在场时才报 `dictionary maintenance busy`；它读的个人词条行由会话在上屏时直接写进日志，不在会话的延迟写队列里，销毁会话也不会让列表多出什么。键盘每次出现都强制刷新一次这个列表，原来每次都要为它销毁、重建并聚焦一遍刚在 `viewWillAppear` 建好的会话。
  func personalEntries(atOffset offset: UInt, kind: PersonalWordKind? = nil, query: String = "") throws -> [String: Any] {
    try Self.personalEntries(options: engineOptions, offset: offset, kind: kind, query: query)
  }

  /// 同上，但用调用方给的 HostOptions、不经会话：设置 App 用 `preparedHostOptions` 的那一份，在词库同步把改动写进 Engine 之后重读「我的词条」（`PersonalDictionaryStore.refreshPage`）。`list` 只取共享租约，键盘的会话开着也能读，只有维护者在场时报 `dictionary maintenance busy`。读文件，调用方放到主线程之外。
  static func personalEntries(options: [String: Any], offset: UInt, kind: PersonalWordKind? = nil,
                              query: String = "") throws -> [String: Any] {
    var action: [String: Any] = ["operation": "list", "offset": offset, "limit": 100]
    if let kind { action["kind"] = kind.bridgeName }
    if !query.isEmpty { action["query"] = query }
    let request: [String: Any] = ["options": options, "action": action]
    var result = try callOptions(msimeClientDictionary, request)
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
        let result = try Self.callOptions(msimeClientDictionary, ["options": engineOptions, "action": action])
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
    keyGrid = .none
    return dispatch { msimeClientSetKeyGrid(handle, KeyGrid.none.rawValue) }
  }

  private func command(_ value: UInt32) -> MetasequoiaInputSnapshot {
    dispatch { msimeClientCommand(handle, value) }
  }

  private func pointer(for character: String, shift: Bool) -> UnsafeMutablePointer<CChar>? {
    guard let byte = Self.ascii(character) else { return nil }
    return msimeClientCharacter(handle, byte, shift)
  }

  private func dispatch(preservingViewOnRejection: Bool = false,
                        _ operation: () -> UnsafeMutablePointer<CChar>?) -> MetasequoiaInputSnapshot {
    flushPendingCharacters()
    guard handle != 0 else {
      return diagnostic(initializationDiagnostic ?? "输入运行时尚未准备完成。")
    }
    do {
      guard let value = try Self.decode(operation()) as? [String: Any] else { throw InputBridgeFailure.invalidResponse }
      return try Self.snapshot(value)
    }
    catch InputBridgeFailure.response(let message) {
      // 旧画面的身份被运行时拒绝时，组字仍在；返回当前视图，避免把错误误画成清空。
      if preservingViewOnRejection,
         let current = try? Self.callHandle(msimeClientView, handle),
         let snapshot = try? Self.snapshot(["view": current, "diagnostic": message]) {
        return snapshot
      }
      return diagnostic(message)
    } catch { return diagnostic(error.localizedDescription) }
  }

  private func diagnostic(_ message: String) -> MetasequoiaInputSnapshot {
    MetasequoiaInputSnapshot(diagnosticText: message)
  }

  private func view() throws -> [String: Any] {
    flushPendingCharacters()
    return try Self.callHandle(msimeClientView, handle)
  }

  private func currentCandidates() throws -> [[String: Any]] {
    (try view()["candidates"] as? [[String: Any]]) ?? []
  }

  private func updatePreferences(_ mutate: (inout [String: Any]) -> Void) -> Bool {
    flushPendingCharacters()
    guard handle != 0 else { return false }
    let previous = (options["preferences"] as? [String: Any]) ?? [:]
    var prefs = previous
    mutate(&prefs)
    options["preferences"] = prefs
    let sent = Self.engineBound(prefs)
    // 会话已经接受并应用了完全相同的偏好：再发一次不改变任何状态，见 `appliedPreferences`。
    if let appliedPreferences, NSDictionary(dictionary: appliedPreferences).isEqual(to: sent) { return true }
    revision &+= 1
    let snapshot: [String: Any] = ["format_version": 1, "revision": revision, "preferences": sent]
    appliedPreferences = nil
    do {
      let response = try Self.callUpdate(msimeClientUpdatePreferences, handle, snapshot)
      let applied = try Self.preferencesUpdateSucceeded(response)
      if applied { appliedPreferences = sent }
      return applied
    } catch {
      options["preferences"] = previous
      revision &-= 1
      return false
    }
  }

  /// 交给 Rust 会话和词库调用的偏好：去掉 `custom_theme.keyboard.photo`。
  ///
  /// 照片皮肤的照片按 base64 内联在共享文档里，最大约 680 KB。会话和词库都不画键盘皮肤（主题由 `ResolvedTheme` 另外解析），带着它的话每次建会话、改偏好、列个人词库、查词库版本都要在主线程把它整段编一遍 JSON，Rust 再解析一遍、按 base64 解一遍做校验。所有交给会话的偏好都经过这里，会话里的偏好因此始终一致，换照片也不会让会话以为偏好变了而重建 Engine。`options["preferences"]` 仍是完整文档，`sharedPreferences` 的读者（键盘皮肤、候选配色）照旧读得到照片；写回共享文档的路径都先从磁盘重新读文档（`persistSharedPreferences`），不从这里写回，所以文档里的照片不会因此丢失。
  static func engineBound(_ preferences: [String: Any]) -> [String: Any] {
    guard var custom = preferences["custom_theme"] as? [String: Any],
          var keyboard = custom["keyboard"] as? [String: Any],
          keyboard["photo"] is String else { return preferences }
    keyboard.removeValue(forKey: "photo")
    custom["keyboard"] = keyboard
    var stripped = preferences
    stripped["custom_theme"] = custom
    return stripped
  }

  /// `options` with its preferences passed through `engineBound`, for every call that hands the prepared options to Rust.
  private var engineOptions: [String: Any] {
    guard let preferences = options["preferences"] as? [String: Any] else { return options }
    var engine = options
    engine["preferences"] = Self.engineBound(preferences)
    return engine
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
    let group = IOSSharedPaths.sharedContainerOrApplicationSupport(for: MSIMEAppEdition.appGroupIdentifier)
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
  /// `View.scheme` for Cantonese Jyutping, which also runs Cantonese shuangpin.
  private static let cantoneseSchemeCode: UInt8 = 5

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
    let marks = view["punctuation_marks"] as? [String: Any] ?? [:]
    let commitContext = value["commit_context"] as? [String: Any]
    let scriptConversion = try strictBool(commitContext?["script_conversion"], fallback: true)
    return MetasequoiaInputSnapshot(isHandled: try strictBool(value["handled"], fallback: false),
      commitText: value["commit"] as? String, commitCursorSuffix: value["commit_cursor_suffix"] as? String,
      scriptConversion: scriptConversion,
      preedit: view["preedit"] as? String ?? "",
      reading: view["reading"] as? String ?? "",
      phrasePrefix: view["phrase_prefix"] as? String ?? "",
      candidates: rows.compactMap { $0["text"] as? String },
      candidateCodes: rows.map { $0["code"] as? String ?? "" },
      candidateIdentities: try rows.map { row in
        guard let raw = row["id"] else { return nil }
        guard let identity = raw as? [String: Any],
              let generation = strictUInt64(identity["generation"]),
              let index = strictUInt64(identity["index"]) else { throw InputBridgeFailure.invalidResponse }
        return MetasequoiaCandidateIdentity(generation: generation, index: index)
      },
      generation: try view["generation"].map { try number($0) },
      candidateGlosses: rows.map { $0["translation"] as? String ?? "" },
      candidateAnnotations: rows.map { $0["annotation"] as? String ?? "" },
      candidateSources: candidateSources,
      candidateFixedPositions: candidateFixedPositions,
      candidateHighlighted: try rows.map { try strictBool($0["highlighted"], fallback: false) },
      candidateConfidentCorrections: try rows.map { try strictBool($0["confident_correction"], fallback: false) },
      candidatePageCount: pageCount,
      answeredByPinyinFallback: try strictBool(view["answered_by_pinyin_fallback"], fallback: false),
      diagnosticText: value["diagnostic"] as? String,
      localMode: view["local_mode"] as? String ?? "none",
      nineKeySpellings: view["nine_key_spellings"] as? [String] ?? [],
      nineKeyReading: view["nine_key_reading"] as? String ?? "",
      nineKeySingleCharacter: try strictBool(view["nine_key_single_character"], fallback: false),
      nineKeyStrokes: view["nine_key_strokes"] as? String ?? "",
      editingText: editingText,
      caretPosition: caretPosition,
      cornerQuotes: marks["quotes"] as? String == "corner",
      hollowBrackets: marks["brackets"] as? String == "hollow",
      enumerationCommaBackslash: marks["enumeration_comma_backslash"] as? Bool ?? true,
      enumerationCommaSlash: marks["enumeration_comma_slash"] as? Bool ?? false)
  }

  private static func strictInt(_ value: Any?, fallback: Int, range: ClosedRange<Int>) throws -> Int {
    guard let value else { return fallback }
    guard let integer = SharedNumber.strictInt(value),
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
