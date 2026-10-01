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
for name in libmsime_host_api.so libmsime_android.so libc++_shared.so libsherpa-onnx-c-api.so libonnxruntime.so; do
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
for symbol in msime_client_prepare_host msime_client_refresh_host msime_client_create msime_client_snapshot_version msime_client_snapshot_prepare msime_client_snapshot_discard msime_client_snapshot_activate msime_client_character msime_client_punctuation_with_context msime_client_load_preferences msime_client_save_preferences msime_client_theme_catalog msime_client_resolve_theme msime_client_update_preferences msime_client_set_nine_key_mode msime_client_mobile_voice_configuration msime_client_simplified_to_traditional msime_client_reset_cache msime_client_mobile_clipboard_history msime_client_set_chinese_punctuation msime_client_set_character_width msime_client_select_edge msime_client_doubao_start_frame msime_client_doubao_audio_frame msime_client_doubao_decode_frame msime_client_choose_nine_key_spelling msime_client_pin_candidate msime_client_fix_candidate_position msime_client_clear_candidate_position msime_client_remove_candidate msime_client_emoji_catalog_request msime_client_candidate_gloss_request msime_client_apply_translations msime_client_online_query msime_client_cloud_request_url msime_client_ai_request_for_query msime_client_apply_cloud_response msime_client_apply_online_candidates msime_client_vocabulary_review msime_client_voice_hotwords msime_client_voice_hotword_correct msime_client_telemetry_begin msime_client_telemetry_end msime_client_telemetry_flush msime_client_telemetry_clear msime_client_notices msime_client_notice_dismiss msime_client_string_free; do
  grep -Eq "GLOBAL +DEFAULT +[0-9]+ +${symbol}$" <<< "$symbols" || { echo "Missing host export: $symbol" >&2; exit 1; }
done
symbols=$("$readelf_tool" --dyn-syms --wide "$library_dir/libmsime_android.so")
for method in prepareHostRaw refreshHostRaw snapshotVersionRaw snapshotPrepareRaw snapshotDiscardRaw snapshotActivateRaw loadPreferencesRaw savePreferencesRaw themeCatalogRaw resolveThemeRaw createRaw characterRaw punctuationWithContextRaw setNineKeyModeRaw mobileVoiceConfigurationRaw simplifiedToTraditionalRaw resetCacheRaw mobileClipboardHistoryRaw setChinesePunctuationRaw setCharacterWidthRaw selectEdgeRaw doubaoStartFrameRaw doubaoAudioFrameRaw doubaoDecodeFrameRaw chooseNineKeySpellingRaw updatePreferencesRaw pinCandidateRaw fixCandidatePositionRaw clearCandidatePositionRaw removeCandidateRaw emojiCatalogRaw candidateGlossesRaw applyTranslationsRaw onlineQueryRaw cloudRequestUrlRaw aiRequestForQueryRaw applyCloudResponseRaw applyOnlineCandidatesRaw vocabularyReviewRaw voiceHotwordsRaw voiceHotwordCorrectRaw localSpeechAvailableRaw localSpeechCreateRaw localSpeechStartRaw localSpeechAcceptRaw localSpeechFinishRaw localSpeechCancelRaw localSpeechDestroyRaw localSpeechReleaseRaw telemetryBeginRaw telemetryEndRaw telemetryFlushRaw telemetryClearRaw noticesRaw noticeDismissRaw destroyRaw; do
  grep -Eq "GLOBAL +DEFAULT +[0-9]+ +Java_app_msime_android_NativeClient_${method}$" <<< "$symbols" || { echo "Missing JNI export: $method" >&2; exit 1; }
done
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
