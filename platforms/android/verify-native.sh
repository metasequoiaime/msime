#!/usr/bin/env bash
set -euo pipefail
readelf_tool=${1:?usage: verify-native.sh <llvm-readelf> <library-directory> <abi>}
library_dir=${2:?library directory required}
abi=${3:?ABI required}
case "$abi" in
  arm64-v8a) machine=AArch64 ;;
  x86_64) machine='Advanced Micro Devices X86-64' ;;
  *) echo "Unsupported ABI" >&2; exit 1 ;;
esac
# The two speech runtime libraries are prebuilt upstream (pinned in resources/voice-runtime.lock.json) and link libc++ statically; they are held to the same architecture and 16KB alignment, with an allowlist that adds only what sherpa-onnx itself needs.
# 三个宿主库必须在；语音运行库在 build-native.sh --omit-voice-runtime 时不在目录里（改由资源包 voice-runtime 下载），在的时候照样逐项检查。两个库要么都在要么都不在：libsherpa-onnx-c-api.so 按 soname 依赖 libonnxruntime.so，只带一个时本地识别必然加载失败。
libraries=(libmsime_host_api.so libmsime_android.so libc++_shared.so)
runtime_present=()
for name in libsherpa-onnx-c-api.so libonnxruntime.so; do
  if [[ -e "$library_dir/$name" ]]; then runtime_present+=("$name"); fi
done
case "${#runtime_present[@]}" in
  2) libraries+=("${runtime_present[@]}") ;;
  0) echo "speech runtime libraries absent from $library_dir; checking the host libraries only" ;;
  *) echo "Only ${runtime_present[0]} of the speech runtime pair is in $library_dir" >&2; exit 1 ;;
esac
for name in "${libraries[@]}"; do
  library="$library_dir/$name"
  header=$("$readelf_tool" -h "$library")
  [[ "$header" == *ELF64* && "$header" == *"$machine"* ]] || { echo "Wrong ELF architecture: $name" >&2; exit 1; }
  segments=$("$readelf_tool" -l --wide "$library" | awk '$1 == "LOAD" {print $NF}')
  [[ -n "$segments" ]] || exit 1
  while IFS= read -r alignment; do
    [[ "$alignment" =~ ^0x[[:xdigit:]]+$ ]] && (( alignment >= 16384 )) || { echo "LOAD alignment below 16KB: $name" >&2; exit 1; }
  done <<< "$segments"
  dependencies=$("$readelf_tool" -d "$library" | awk '/NEEDED/ {gsub(/[][]/, "", $NF); print $NF}')
  while IFS= read -r dependency; do
    case "$dependency" in
      libc.so|libm.so|libdl.so|liblog.so|libc++_shared.so|libmsime_host_api.so|'') ;;
      libandroid.so|libonnxruntime.so) [[ "$name" == libsherpa-onnx-c-api.so ]] || { echo "Unexpected dynamic dependency: $name -> $dependency" >&2; exit 1; } ;;
      # Microphone capture in host-api goes through cpal, which links AAudio where the Engine's miniaudio used to dlopen it. AAudio is a public NDK library from API 26, below this host's minSdk 28.
      libaaudio.so) [[ "$name" == libmsime_host_api.so ]] || { echo "Unexpected dynamic dependency: $name -> $dependency" >&2; exit 1; } ;;
      *) echo "Unexpected dynamic dependency: $name -> $dependency" >&2; exit 1 ;;
    esac
  done <<< "$dependencies"
  echo "$name: $abi ELF, 16KB LOAD alignment and dependency allowlist passed"
done
symbols=$("$readelf_tool" --dyn-syms --wide "$library_dir/libmsime_host_api.so")
for symbol in msime_client_language_cycle msime_client_ai_translation_http_request msime_client_parse_ai_translation_response msime_client_ai_translation_reply_failed msime_client_prepare_host msime_client_refresh_host msime_client_create msime_client_snapshot_version msime_client_snapshot_prepare msime_client_snapshot_discard msime_client_snapshot_activate msime_client_character msime_client_set_key_geometry msime_client_touch_character msime_client_glide msime_client_punctuation_with_context msime_client_balance_paired_punctuation_after_auto_close msime_client_load_preferences msime_client_toggle_hotkey_chords msime_client_save_preferences msime_client_migrate_android_key_layout msime_client_theme_catalog msime_client_resolve_theme msime_client_update_preferences msime_client_set_nine_key_mode msime_client_set_key_grid msime_client_grid_key msime_client_nine_key_shuangpin_layout msime_client_set_private_session msime_client_mobile_voice_configuration msime_client_mobile_voice_status msime_client_voice_provider_catalog msime_client_voice_provider_update msime_client_voice_endpoint_issue msime_client_voice_credential_issue msime_client_voice_websocket_endpoint msime_client_simplified_to_traditional msime_client_reset_cache msime_client_release_process_cache msime_client_mobile_clipboard_history msime_client_cloud_clipboard_e2e msime_client_cloud_clipboard_source msime_client_relay_cloud_clipboard msime_client_relay_cloud_clipboard_exchange msime_client_cloud_clipboard_webdav_probe msime_client_dictionary_sync msime_client_set_chinese_punctuation msime_client_set_character_width msime_client_select_edge msime_client_doubao_start_frame msime_client_doubao_audio_frame msime_client_doubao_decode_frame msime_client_choose_nine_key_spelling msime_client_set_nine_key_filter msime_client_pin_candidate msime_client_fix_candidate_position msime_client_clear_candidate_position msime_client_remove_candidate msime_client_emoji_catalog_request msime_client_candidate_gloss_request msime_client_pronunciation_request msime_client_apply_translations msime_client_online_query msime_client_cloud_request_url msime_client_ai_request_for_query msime_client_apply_cloud_response msime_client_apply_online_candidates msime_client_clear_online_candidates msime_client_resolve_online_candidates msime_client_vocabulary_review msime_client_personal_wordbook_add msime_client_voice_hotwords msime_client_voice_hotword_correct msime_client_voice_normalize_numbers msime_client_telemetry_begin msime_client_telemetry_end msime_client_telemetry_flush msime_client_telemetry_clear msime_client_notices msime_client_notice_dismiss msime_client_app_theme_catalog msime_client_resolve_app_theme msime_client_restore_default_preferences msime_client_default_preferences msime_client_host_capabilities msime_client_dictionary msime_client_personal_dictionary_request msime_client_dictionary_hans_entries msime_client_dictionary_import_entries msime_client_dictionary_manifest msime_client_plugins msime_client_community_resource_library msime_client_ai_skin_plan msime_client_keyboard_skin_trial msime_client_community_skin_install msime_client_key_sound_pack msime_client_world_clock msime_client_common_phrases msime_client_dictionary_collections msime_client_diagnostic_bundle msime_client_account_settings_export msime_client_account_settings_apply msime_client_resource_packs msime_client_resource_pack_install msime_client_resource_pack_cancel msime_client_resource_pack_adopt msime_client_resource_pack_remove msime_client_custom_translation_http_request msime_client_niutrans_translation_http_request msime_client_tencent_translation_http_request msime_client_parse_custom_translation_response msime_client_parse_niutrans_translation_response msime_client_parse_tencent_translation_response msime_client_custom_translation_reply_failed msime_client_niutrans_translation_reply_failed msime_client_translation_endpoint_issue msime_client_test_api_credential msime_client_ai_endpoint_check msime_client_session_store_register msime_client_session_store_unregister msime_client_client_agent msime_client_account_name msime_client_account msime_client_account_access_token msime_client_account_cloud_clipboard msime_client_account_chat_stream msime_client_account_chat_cancel msime_client_account_gloss msime_client_community msime_client_ai_skin_generate msime_client_ai_skin_cancel msime_client_ai_skin_job msime_client_cloud_dictionary msime_client_snapshot_queue msime_client_cloud_sync msime_client_string_free; do
  grep -Eq "GLOBAL +DEFAULT +[0-9]+ +${symbol}$" <<< "$symbols" || { echo "Missing host export: $symbol" >&2; exit 1; }
done
symbols=$("$readelf_tool" --dyn-syms --wide "$library_dir/libmsime_android.so")
for method in textTrackpadOptionsRaw worldClockRaw aiProviderCatalogRaw aiProviderUpdateRaw aiModelsRaw skinPackageRaw skinResourceRaw aiTranslationHttpRequestRaw parseAiTranslationResponseRaw aiTranslationReplyFailedRaw prepareHostRaw refreshHostRaw snapshotVersionRaw loadPreferencesRaw savePreferencesRaw migrateKeyLayoutRaw themeCatalogRaw resolveThemeRaw createRaw characterRaw setKeyGeometryRaw touchCharacterRaw glideRaw punctuationWithContextRaw balancePairedPunctuationAfterAutoCloseRaw setNineKeyModeRaw setKeyGridRaw gridKeyRaw nineKeyShuangpinLayoutRaw setPrivateSessionRaw mobileVoiceConfigurationRaw mobileVoiceStatusRaw mobileVoicePolishStatusRaw mobileVoicePolicyRaw voiceProviderCatalogRaw voiceProviderUpdateRaw voiceEndpointIssueRaw voiceCredentialIssueRaw voiceWebSocketEndpointRaw simplifiedToTraditionalRaw resetCacheRaw releaseProcessCacheRaw mobileClipboardHistoryRaw cloudClipboardE2eRaw cloudClipboardSourceRaw cloudClipboardRelayRaw cloudClipboardRelayExchangeRaw cloudClipboardWebDavProbeRaw dictionarySyncRaw setChinesePunctuationRaw setCharacterWidthRaw selectEdgeRaw doubaoStartFrameRaw doubaoAudioFrameRaw doubaoDecodeFrameRaw chooseNineKeySpellingRaw setNineKeyFilterRaw updatePreferencesRaw pinCandidateRaw fixCandidatePositionRaw clearCandidatePositionRaw removeCandidateRaw emojiCatalogRaw candidateGlossesRaw candidatePronunciationsRaw applyTranslationsRaw onlineQueryRaw cloudRequestUrlRaw aiRequestForQueryRaw applyCloudResponseRaw applyOnlineCandidatesRaw clearOnlineCandidatesRaw resolveOnlineCandidatesRaw vocabularyReviewRaw personalWordbookAddRaw voiceHotwordsRaw voiceHotwordCorrectRaw voiceNormalizeNumbersRaw localSpeechAvailableRaw localSpeechCreateRaw localSpeechStartRaw localSpeechAcceptRaw localSpeechFinishRaw localSpeechCancelRaw localSpeechDestroyRaw localSpeechReleaseRaw localSpeechSpeakingRaw telemetryBeginRaw telemetryEndRaw telemetryFlushRaw telemetryClearRaw noticesRaw noticeDismissRaw destroyRaw allCandidatesRaw candidateRangeRaw appThemeCatalogRaw commandRaw communityResourceLibraryRaw communitySkinInstallRaw defaultPreferencesRaw dictionaryHansEntriesRaw dictionaryImportEntriesRaw dictionaryManifestRaw dictionaryRaw englishCompletionsRaw focusRaw hostCapabilitiesRaw keyboardSkinTrialRaw keySoundPackRaw personalDictionaryRequestRaw personalDictionarySyncRaw pluginsRaw polishPromptRaw resolveAppThemeRaw restoreDefaultPreferencesRaw selectAnyCandidateRaw selectRaw setEnglishModeRaw shuangpinKeyHintsRaw smartPunctuationArmRaw smartPunctuationDecideRaw typingStatisticsEnabledRaw typingStatisticsRaw viewRaw commonPhrasesRaw dictionaryCollectionsRaw diagnosticBundleRaw accountSettingsExportRaw accountSettingsApplyRaw localSpeechRuntimeRaw resourcePacksRaw resourcePackInstallRaw resourcePackCancelRaw resourcePackVerifyRaw resourcePackAdoptRaw resourcePackRemoveRaw testVoiceCredentialRaw voiceLocalModelsRaw voiceLocalModelInstallRaw voiceLocalModelCancelRaw voiceLocalModelRemoveRaw customTranslationHttpRequestRaw niutransTranslationHttpRequestRaw tencentTranslationHttpRequestRaw parseCustomTranslationResponseRaw parseNiuTransTranslationResponseRaw parseTencentTranslationResponseRaw customTranslationReplyFailedRaw niutransTranslationReplyFailedRaw aiTranslationReplyFailedRaw translationEndpointIssueRaw testApiCredentialRaw aiEndpointCheckRaw sessionStoreRegisterRaw sessionStoreUnregisterRaw clientAgentRaw accountNameRaw accountRaw accountAccessTokenRaw accountCloudClipboardRaw accountChatStreamRaw accountChatCancelRaw accountGlossRaw communityRaw aiSkinGenerateRaw aiSkinCancelRaw aiSkinJobRaw cloudDictionaryRaw snapshotQueueRaw cloudSyncRaw; do
  grep -Eq "GLOBAL +DEFAULT +[0-9]+ +Java_app_msime_android_NativeClient_${method}$" <<< "$symbols" || { echo "Missing JNI export: $method" >&2; exit 1; }
done
# 注入的会话存储的回调可能在 Rust 自己起的线程上发生，要经 JNI_OnLoad 记下的 JavaVM 取 env；没有导出它时 ART 不会调用，回调一律按失败返回。
grep -Eq "GLOBAL +DEFAULT +[0-9]+ +JNI_OnLoad$" <<< "$symbols" || { echo "Missing JNI export: JNI_OnLoad" >&2; exit 1; }
nm_tool="${readelf_tool%/llvm-readelf}/llvm-nm"
[[ -x "$nm_tool" ]] || { echo "llvm-nm is required beside llvm-readelf" >&2; exit 1; }
host_symbols=$("$nm_tool" -C --defined-only "$library_dir/libmsime_host_api.so")
# The candidate ordering policy of msime-engine's handwriting module is shared with Android; the model-backed recognizer and its zinnia port (the recognizer, model and features submodules) are compiled out there. The ordering policy has to be present, or the symbols were stripped and the exclusion below would prove nothing.
if ! grep -q 'msime_engine::handwriting::order_handwriting_candidates' <<< "$host_symbols"; then
  echo "libmsime_host_api.so carries no msime-engine symbol names; the recognizer exclusion cannot be checked" >&2; exit 1
fi
if grep -Eq 'handwriting_recognize|msime_engine::handwriting::(recognizer|model|features)::' <<< "$host_symbols"; then
  echo "Android host unexpectedly contains the Engine handwriting recognizer" >&2; exit 1
fi
echo "libmsime_host_api.so: Engine handwriting recognizer excluded for Android"
