#include <jni.h>
#include "msime_client.h"
// The polish presets carry their own prompt-injection wording, and there are already four copies
// of that text in this repository. This host reads the shared one rather than adding a fifth.
#include "voice/PolishPrompt.h"
// On-device recognition is the shared sherpa-onnx recognizer every desktop host uses, compiled into this library; the runtime itself (libsherpa-onnx-c-api.so and libonnxruntime.so from the pinned .aar) is loaded on first use, by name when the package carries it, otherwise from the downloaded voice-runtime resource pack at the path Java hands to localSpeechRuntimeRaw.
#include "voice/LocalAsr.h"
#include <atomic>
#include <chrono>
#include <cstring>
#include <exception>
#include <functional>
#include <memory>
#include <mutex>
#include <vector>
#include <fstream>
#include <limits>
#include <string>

struct SnapshotReader {
    explicit SnapshotReader(const std::string &path) : input(path, std::ios::in | std::ios::binary) {}
    std::ifstream input;
};

static intptr_t snapshotNext(void *context, uint8_t *buffer, size_t capacity) noexcept {
    auto *reader = static_cast<SnapshotReader *>(context);
    if (!reader || !buffer || capacity == 0) return -1;
    for (;;) {
        size_t length = 0;
        bool ended = false;
        // Reserve one byte for the NUL terminator used by the lightweight record discriminator.
        // A full buffer is still a malformed overlong line, never a reason to write past it.
        while (length + 1 < capacity) {
            const int value = reader->input.get();
            if (value == EOF) {
                if (!reader->input.eof()) return -1;
                ended = true;
                break;
            }
            if (value == '\n') {
                ended = true;
                break;
            }
            if (value == '\r' && reader->input.peek() == '\n') {
                reader->input.get();
                ended = true;
                break;
            }
            buffer[length++] = static_cast<uint8_t>(value);
        }
        if (!ended || length == 0) return ended && length == 0 ? 0 : -1;
        buffer[length] = 0;
        const char *type = std::strstr(reinterpret_cast<const char *>(buffer), "\"type\":\"");
        if (!type) return -1;
        type += 8;
        const bool engine_record = std::strncmp(type, "overlay\"", 8) == 0
            || std::strncmp(type, "position\"", 9) == 0
            || std::strncmp(type, "selection\"", 10) == 0;
        if (engine_record) return static_cast<intptr_t>(length);
        if (std::strncmp(type, "header\"", 7) == 0
                || std::strncmp(type, "entry\"", 6) == 0
                || std::strncmp(type, "footer\"", 7) == 0) continue;
        return -1;
    }
}

// Use UTF-8 byte arrays, not JNI modified UTF-8: supplementary characters in
// candidates and resource paths must survive the Java/native boundary unchanged.
static jbyteArray response(JNIEnv *env, char *value) {
    if (!value) return nullptr;
    size_t length = std::strlen(value);
    if (length > static_cast<size_t>(std::numeric_limits<jsize>::max())) {
        msime_client_string_free(value);
        env->ThrowNew(env->FindClass("java/lang/IllegalStateException"), "Native response too large");
        return nullptr;
    }
    jbyteArray output = env->NewByteArray(static_cast<jsize>(length));
    if (output) env->SetByteArrayRegion(output, 0, static_cast<jsize>(length), reinterpret_cast<const jbyte *>(value));
    msime_client_string_free(value);
    return output;
}

namespace {
template <typename Call> jbyteArray bounded_request(JNIEnv *env, jbyteArray request, jsize limit, Call call);
constexpr jsize kThemeRequestLimit = 1 * 1024 * 1024;
constexpr jsize kTypingStatisticsRequestLimit = 65536;
constexpr jsize kTypingStatisticsDirectoryLimit = 16384;
constexpr jsize kPersonalDictionaryRequestLimit = 2248576;
constexpr jsize kCreateOptionsLimit = 1 * 1024 * 1024;
constexpr jsize kMobileClipboardHistoryRequestLimit = 524288;
constexpr jsize kVocabularyReviewRequestLimit = 8 * 1024 * 1024;
constexpr jsize kPrepareHostRequestLimit = 16384;
constexpr jsize kSavePreferencesDirectoryLimit = 16384;
constexpr jsize kSavePreferencesSnapshotLimit = 1 * 1024 * 1024;
constexpr jsize kMobileVoiceDirectoryLimit = 16384;
constexpr jsize kLoadPreferencesDirectoryLimit = 16384;
constexpr jsize kRefreshHostPathLimit = 4096;
constexpr jsize kSnapshotVersionRequestLimit = 1 * 1024 * 1024;
constexpr jsize kSnapshotPrepareRequestLimit = 1 * 1024 * 1024;
constexpr jsize kSnapshotPreparePathLimit = 16384;
constexpr jsize kUpdatePreferencesSnapshotLimit = 1 * 1024 * 1024;
constexpr jsize kSmallJsonRequestLimit = 16 * 1024;
constexpr jsize kEmojiQueryLimit = 16384;
constexpr jsize kEmojiResourcesLimit = 4096;
constexpr jsize kCandidateGlossRequestLimit = 262144;
constexpr jsize kCandidateGlossResourcesLimit = 4096;
constexpr jsize kEnglishCompletionRequestLimit = 16384;
constexpr jsize kEnglishCompletionResourcesLimit = 4096;
constexpr jsize kApplyTranslationsLimit = 1 * 1024 * 1024;
constexpr jsize kShuangpinProfileLimit = 64;
constexpr jsize kSmartPunctuationRequestLimit = 4096;
// 更长的请求 msime_client_glide 自己也会拒绝；这里先查，免得复制一个超大的数组。
constexpr jsize kGlideRequestLimit = 65536;
constexpr jsize kTraditionalConversionLimit = 1 * 1024 * 1024;
constexpr jsize kOnlineQueryLimit = 16384;
constexpr jsize kOnlineBodyLimit = 262144;
constexpr jsize kOnlineCandidatesLimit = 16384;
constexpr jsize kDoubaoAudioPcmLimit = 1 * 1024 * 1024;
constexpr jsize kDoubaoBoostingLimit = 4096;
constexpr jsize kPolishPromptIdLimit = 256;
constexpr jsize kPolishPromptCustomLimit = 8192;
}

extern "C" {
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_loadPreferencesRaw(JNIEnv *env, jclass, jbyteArray directory) {
    return bounded_request(env, directory, kLoadPreferencesDirectoryLimit, msime_client_load_preferences);
}
// Usage reporting and notices: one UTF-8 JSON request in, the shared envelope out (msime_client.h documents each request).
static jbyteArray json_call(JNIEnv *env, jbyteArray request, char *(*call)(const uint8_t *, size_t)) {
    if (!request) return response(env, call(nullptr, 0));
    jsize length = env->GetArrayLength(request);
    if (length > kSmallJsonRequestLimit) return response(env, call(nullptr, 0));
    jbyte *bytes = env->GetByteArrayElements(request, nullptr);
    if (!bytes) return nullptr;
    char *result = call(reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(request, bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_telemetryBeginRaw(JNIEnv *env, jclass, jbyteArray request) {
    return json_call(env, request, msime_client_telemetry_begin);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_telemetryEndRaw(JNIEnv *env, jclass, jbyteArray request) {
    return json_call(env, request, msime_client_telemetry_end);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_telemetryFlushRaw(JNIEnv *env, jclass, jbyteArray request) {
    return json_call(env, request, msime_client_telemetry_flush);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_telemetryClearRaw(JNIEnv *env, jclass, jbyteArray request) {
    return json_call(env, request, msime_client_telemetry_clear);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_noticesRaw(JNIEnv *env, jclass, jbyteArray request) {
    return json_call(env, request, msime_client_notices);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_noticeDismissRaw(JNIEnv *env, jclass, jbyteArray request) {
    return json_call(env, request, msime_client_notice_dismiss);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_themeCatalogRaw(JNIEnv *env, jclass) {
    return response(env, msime_client_theme_catalog());
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_resolveThemeRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kThemeRequestLimit, msime_client_resolve_theme);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_typingStatisticsRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kTypingStatisticsRequestLimit, msime_client_typing_statistics);
}
// 1 enabled, 0 disabled or missing, -1 invalid directory or unreadable document; the Java side treats anything but 1 as off.
JNIEXPORT jint JNICALL Java_app_msime_android_NativeClient_typingStatisticsEnabledRaw(JNIEnv *env, jclass, jbyteArray directory) {
    if (!directory) return -1;
    jsize length = env->GetArrayLength(directory);
    if (length > kTypingStatisticsDirectoryLimit) return -1;
    jbyte *bytes = env->GetByteArrayElements(directory, nullptr);
    if (!bytes) return -1;
    int32_t result = msime_client_typing_statistics_enabled(
        reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(directory, bytes, JNI_ABORT);
    return static_cast<jint>(result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_vocabularyReviewRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kVocabularyReviewRequestLimit,
        msime_client_vocabulary_review);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_emojiCatalogRaw(JNIEnv *env, jclass, jbyteArray query, jbyteArray resources) {
    if (!query || !resources) {
        return response(env, msime_client_emoji_catalog_request(nullptr, 0, nullptr, 0));
    }
    jsize query_length = env->GetArrayLength(query);
    jsize resources_length = env->GetArrayLength(resources);
    if (query_length > kEmojiQueryLimit || resources_length > kEmojiResourcesLimit) {
        return response(env, msime_client_emoji_catalog_request(nullptr, 0, nullptr, 0));
    }
    jbyte *query_bytes = env->GetByteArrayElements(query, nullptr);
    if (!query_bytes) return nullptr;
    jbyte *resources_bytes = env->GetByteArrayElements(resources, nullptr);
    if (!resources_bytes) {
        env->ReleaseByteArrayElements(query, query_bytes, JNI_ABORT);
        return nullptr;
    }
    char *result = msime_client_emoji_catalog_request(
        reinterpret_cast<const uint8_t *>(query_bytes), static_cast<size_t>(query_length),
        reinterpret_cast<const uint8_t *>(resources_bytes), static_cast<size_t>(resources_length));
    env->ReleaseByteArrayElements(resources, resources_bytes, JNI_ABORT);
    env->ReleaseByteArrayElements(query, query_bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_candidateGlossesRaw(JNIEnv *env, jclass, jbyteArray request, jbyteArray resources) {
    if (!request || !resources) {
        return response(env, msime_client_candidate_gloss_request(nullptr, 0, nullptr, 0));
    }
    jsize request_length = env->GetArrayLength(request);
    jsize resources_length = env->GetArrayLength(resources);
    if (request_length > kCandidateGlossRequestLimit
            || resources_length > kCandidateGlossResourcesLimit) {
        return response(env, msime_client_candidate_gloss_request(nullptr, 0, nullptr, 0));
    }
    jbyte *request_bytes = env->GetByteArrayElements(request, nullptr);
    if (!request_bytes) return nullptr;
    jbyte *resources_bytes = env->GetByteArrayElements(resources, nullptr);
    if (!resources_bytes) {
        env->ReleaseByteArrayElements(request, request_bytes, JNI_ABORT);
        return nullptr;
    }
    char *result = msime_client_candidate_gloss_request(
        reinterpret_cast<const uint8_t *>(request_bytes), static_cast<size_t>(request_length),
        reinterpret_cast<const uint8_t *>(resources_bytes), static_cast<size_t>(resources_length));
    env->ReleaseByteArrayElements(resources, resources_bytes, JNI_ABORT);
    env->ReleaseByteArrayElements(request, request_bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_englishCompletionsRaw(JNIEnv *env, jclass, jbyteArray request, jbyteArray resources) {
    if (!request || !resources) {
        return response(env, msime_client_english_completions_request(nullptr, 0, nullptr, 0));
    }
    jsize request_length = env->GetArrayLength(request);
    jsize resources_length = env->GetArrayLength(resources);
    if (request_length > kEnglishCompletionRequestLimit
            || resources_length > kEnglishCompletionResourcesLimit) {
        return response(env, msime_client_english_completions_request(nullptr, 0, nullptr, 0));
    }
    jbyte *request_bytes = env->GetByteArrayElements(request, nullptr);
    if (!request_bytes) return nullptr;
    jbyte *resources_bytes = env->GetByteArrayElements(resources, nullptr);
    if (!resources_bytes) {
        env->ReleaseByteArrayElements(request, request_bytes, JNI_ABORT);
        return nullptr;
    }
    char *result = msime_client_english_completions_request(
        reinterpret_cast<const uint8_t *>(request_bytes), static_cast<size_t>(request_length),
        reinterpret_cast<const uint8_t *>(resources_bytes), static_cast<size_t>(resources_length));
    env->ReleaseByteArrayElements(resources, resources_bytes, JNI_ABORT);
    env->ReleaseByteArrayElements(request, request_bytes, JNI_ABORT);
    return response(env, result);
}
static std::string utf8(JNIEnv *env, jbyteArray value) {
    if (!value) return {};
    jsize length = env->GetArrayLength(value);
    if (length <= 0) return {};
    jbyte *bytes = env->GetByteArrayElements(value, nullptr);
    if (!bytes) return {};
    std::string text(reinterpret_cast<const char *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(value, bytes, JNI_ABORT);
    return text;
}
// Which prompt the selected slot resolves to, decided by the shared header rather than here: slot precedence has been wrong on individual hosts before, and it is one rule.
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_polishPromptRaw(JNIEnv *env, jclass, jbyteArray id, jbyteArray custom1, jbyteArray custom2, jbyteArray custom3) {
    if ((id && env->GetArrayLength(id) > kPolishPromptIdLimit)
            || (custom1 && env->GetArrayLength(custom1) > kPolishPromptCustomLimit)
            || (custom2 && env->GetArrayLength(custom2) > kPolishPromptCustomLimit)
            || (custom3 && env->GetArrayLength(custom3) > kPolishPromptCustomLimit)) {
        return env->NewByteArray(0);
    }
    msime::windows::PolishPromptSlots slots;
    slots.id = utf8(env, id);
    slots.custom_1 = utf8(env, custom1);
    slots.custom_2 = utf8(env, custom2);
    slots.custom_3 = utf8(env, custom3);
    const std::string prompt = msime::windows::polish_prompt_for(slots);
    if (prompt.size() > static_cast<size_t>(std::numeric_limits<jsize>::max())) return nullptr;
    jbyteArray out = env->NewByteArray(static_cast<jsize>(prompt.size()));
    if (out) {
        env->SetByteArrayRegion(out, 0, static_cast<jsize>(prompt.size()),
                                reinterpret_cast<const jbyte *>(prompt.data()));
    }
    return out;
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_mobileClipboardHistoryRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kMobileClipboardHistoryRequestLimit,
        msime_client_mobile_clipboard_history);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_doubaoDecodeFrameRaw(JNIEnv *env, jclass, jbyteArray frame) {
    if (!frame) return response(env, msime_client_doubao_decode_frame(nullptr, 0));
    jsize length = env->GetArrayLength(frame);
    // The shared decoder rejects frames above one MiB. Check before asking JNI for a native view:
    // GetByteArrayElements may copy the entire Java array, so doing this after the call briefly
    // doubles an attacker-controlled oversized response and defeats the decoder's allocation
    // bound.
    if (length > 1024 * 1024) {
        return response(env, msime_client_doubao_decode_frame(nullptr, 0));
    }
    jbyte *bytes = env->GetByteArrayElements(frame, nullptr);
    if (!bytes) return nullptr;
    char *result = msime_client_doubao_decode_frame(
        reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(frame, bytes, JNI_ABORT);
    return response(env, result);
}
// The two frame builders write into a caller-owned buffer and report the size they need when it is
// too small. Ask first, then allocate exactly that: the alternative is a guessed ceiling that is
// either wasteful for a start frame or silently truncating for a long PCM chunk.
static jbyteArray build_frame(JNIEnv *env, const std::function<bool(uint8_t *, size_t, size_t *)> &build) {
    size_t needed = 0;
    if (!build(nullptr, 0, &needed) && needed == 0) return nullptr;
    if (needed == 0 || needed > static_cast<size_t>(std::numeric_limits<jsize>::max())) return nullptr;
    std::vector<uint8_t> buffer(needed);
    size_t written = 0;
    if (!build(buffer.data(), buffer.size(), &written) || written == 0 || written > buffer.size()) {
        return nullptr;
    }
    jbyteArray out = env->NewByteArray(static_cast<jsize>(written));
    if (out) {
        env->SetByteArrayRegion(out, 0, static_cast<jsize>(written),
                                reinterpret_cast<const jbyte *>(buffer.data()));
    }
    return out;
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_doubaoStartFrameRaw(JNIEnv *env, jclass, jboolean itn, jboolean punc, jboolean ddc, jbyteArray boosting) {
    jsize boostingLength = boosting ? env->GetArrayLength(boosting) : 0;
    if (boostingLength > kDoubaoBoostingLimit) return nullptr;
    std::string table = utf8(env, boosting);
    return build_frame(env, [&](uint8_t *out, size_t capacity, size_t *length) {
        return msime_client_doubao_start_frame(
            itn == JNI_TRUE, punc == JNI_TRUE, ddc == JNI_TRUE,
            table.empty() ? nullptr : reinterpret_cast<const uint8_t *>(table.data()),
            table.size(), out, capacity, length);
    });
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_doubaoAudioFrameRaw(JNIEnv *env, jclass, jint sequence, jbyteArray pcm, jint pcmLength, jboolean finalChunk) {
    jsize available = pcm ? env->GetArrayLength(pcm) : 0;
    if (pcmLength < 0 || pcmLength > available || pcmLength > kDoubaoAudioPcmLimit) return nullptr;
    jbyte *bytes = pcm && pcmLength > 0 ? env->GetByteArrayElements(pcm, nullptr) : nullptr;
    if (pcmLength > 0 && !bytes) return nullptr;
    jbyteArray out = build_frame(env, [&](uint8_t *buffer, size_t capacity, size_t *length) {
        return msime_client_doubao_audio_frame(
            static_cast<int32_t>(sequence), reinterpret_cast<const uint8_t *>(bytes),
            static_cast<size_t>(pcmLength), finalChunk == JNI_TRUE, buffer, capacity, length);
    });
    if (bytes) env->ReleaseByteArrayElements(pcm, bytes, JNI_ABORT);
    return out;
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_shuangpinKeyHintsRaw(JNIEnv *env, jclass, jbyteArray profile) {
    return bounded_request(env, profile, kShuangpinProfileLimit,
        msime_client_shuangpin_key_hints);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_savePreferencesRaw(JNIEnv *env, jclass, jbyteArray directory, jlong expected_revision, jbyteArray snapshot) {
    if (!directory || !snapshot || expected_revision < 0) {
        return response(env, msime_client_save_preferences(nullptr, 0, 0, nullptr, 0));
    }
    jsize directory_length = env->GetArrayLength(directory);
    jsize snapshot_length = env->GetArrayLength(snapshot);
    if (directory_length > kSavePreferencesDirectoryLimit
            || snapshot_length > kSavePreferencesSnapshotLimit) {
        return response(env, msime_client_save_preferences(nullptr, 0, 0, nullptr, 0));
    }
    jbyte *directory_bytes = env->GetByteArrayElements(directory, nullptr);
    if (!directory_bytes) return nullptr;
    jbyte *snapshot_bytes = env->GetByteArrayElements(snapshot, nullptr);
    if (!snapshot_bytes) {
        env->ReleaseByteArrayElements(directory, directory_bytes, JNI_ABORT);
        return nullptr;
    }
    char *result = msime_client_save_preferences(
        reinterpret_cast<const uint8_t *>(directory_bytes), static_cast<size_t>(directory_length),
        static_cast<uint64_t>(expected_revision),
        reinterpret_cast<const uint8_t *>(snapshot_bytes), static_cast<size_t>(snapshot_length));
    env->ReleaseByteArrayElements(snapshot, snapshot_bytes, JNI_ABORT);
    env->ReleaseByteArrayElements(directory, directory_bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_personalDictionarySyncRaw(JNIEnv *env, jclass, jbyteArray options) {
    return bounded_request(env, options, kPersonalDictionaryRequestLimit,
        msime_client_personal_dictionary_sync);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_prepareHostRaw(JNIEnv *env, jclass, jbyteArray options) {
    return bounded_request(env, options, kPrepareHostRequestLimit, msime_client_prepare_host);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_refreshHostRaw(JNIEnv *env, jclass, jbyteArray path) {
    return bounded_request(env, path, kRefreshHostPathLimit, msime_client_refresh_host);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_snapshotVersionRaw(JNIEnv *env, jclass, jbyteArray options) {
    return bounded_request(env, options, kSnapshotVersionRequestLimit, msime_client_snapshot_version);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_snapshotPrepareRaw(JNIEnv *env, jclass, jbyteArray request, jbyteArray file) {
    if (!request || !file) return response(env, msime_client_snapshot_prepare(nullptr, 0, nullptr, nullptr));
    jsize request_length = env->GetArrayLength(request);
    jsize file_length = env->GetArrayLength(file);
    if (request_length > kSnapshotPrepareRequestLimit || file_length > kSnapshotPreparePathLimit) {
        return response(env, msime_client_snapshot_prepare(nullptr, 0, nullptr, nullptr));
    }
    jbyte *request_bytes = env->GetByteArrayElements(request, nullptr);
    if (!request_bytes) return nullptr;
    jbyte *file_bytes = env->GetByteArrayElements(file, nullptr);
    if (!file_bytes) {
        env->ReleaseByteArrayElements(request, request_bytes, JNI_ABORT);
        return nullptr;
    }
    std::string path(reinterpret_cast<const char *>(file_bytes), static_cast<size_t>(file_length));
    SnapshotReader reader(path);
    char *result = msime_client_snapshot_prepare(
        reinterpret_cast<const uint8_t *>(request_bytes), static_cast<size_t>(request_length),
        snapshotNext, &reader);
    env->ReleaseByteArrayElements(file, file_bytes, JNI_ABORT);
    env->ReleaseByteArrayElements(request, request_bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_snapshotDiscardRaw(JNIEnv *env, jclass, jlong handle) {
    if (handle <= 0) return response(env, msime_client_snapshot_discard(0));
    return response(env, msime_client_snapshot_discard(static_cast<uint64_t>(handle)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_snapshotActivateRaw(JNIEnv *env, jclass, jlong handle, jbyteArray expected) {
    if (!expected || handle <= 0) return response(env, msime_client_snapshot_activate(0, nullptr, 0));
    jsize length = env->GetArrayLength(expected);
    if (length != 64) return response(env, msime_client_snapshot_activate(0, nullptr, 0));
    jbyte *bytes = env->GetByteArrayElements(expected, nullptr);
    if (!bytes) return nullptr;
    char *result = msime_client_snapshot_activate(
        static_cast<uint64_t>(handle), reinterpret_cast<const uint8_t *>(bytes),
        static_cast<size_t>(length));
    env->ReleaseByteArrayElements(expected, bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_updatePreferencesRaw(JNIEnv *env, jclass, jlong handle, jbyteArray snapshot) {
    if (!snapshot) return response(env, msime_client_update_preferences(static_cast<uint64_t>(handle), nullptr, 0));
    jsize length = env->GetArrayLength(snapshot);
    if (length > kUpdatePreferencesSnapshotLimit) {
        return response(env, msime_client_update_preferences(static_cast<uint64_t>(handle), nullptr, 0));
    }
    jbyte *bytes = env->GetByteArrayElements(snapshot, nullptr);
    if (!bytes) return nullptr;
    char *result = msime_client_update_preferences(static_cast<uint64_t>(handle), reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(snapshot, bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_createRaw(JNIEnv *env, jclass, jbyteArray options) {
    return bounded_request(env, options, kCreateOptionsLimit, msime_client_create);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_focusRaw(JNIEnv *env, jclass, jlong handle, jboolean focused) {
    return response(env, msime_client_focus(static_cast<uint64_t>(handle), focused == JNI_TRUE));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_setNineKeyModeRaw(JNIEnv *env, jclass, jlong handle, jboolean enabled) {
    return response(env, msime_client_set_nine_key_mode(static_cast<uint64_t>(handle), enabled == JNI_TRUE));
}

JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_setPrivateSessionRaw(JNIEnv *env, jclass, jlong handle, jboolean enabled) {
    return response(env, msime_client_set_private_session(static_cast<uint64_t>(handle), enabled == JNI_TRUE));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_setEnglishModeRaw(JNIEnv *env, jclass, jlong handle, jboolean enabled) {
    return response(env, msime_client_set_english_mode(static_cast<uint64_t>(handle), enabled == JNI_TRUE));
}
// Returns the converted text directly rather than a JSON envelope, which is why it reuses the
// same response helper: both are NUL-terminated strings this side must free. A null answer means
// the text was not convertible and the caller keeps the original.
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_mobileVoiceConfigurationRaw(JNIEnv *env, jclass, jbyteArray directory) {
    return bounded_request(env, directory, kMobileVoiceDirectoryLimit,
        msime_client_mobile_voice_configuration);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_simplifiedToTraditionalRaw(JNIEnv *env, jclass, jbyteArray text) {
    if (!text) return nullptr;
    jsize length = env->GetArrayLength(text);
    if (length <= 0 || length > kTraditionalConversionLimit) return nullptr;
    jbyte *bytes = env->GetByteArrayElements(text, nullptr);
    if (!bytes) return nullptr;
    char *result = msime_client_simplified_to_traditional(
        reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(text, bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_resetCacheRaw(JNIEnv *env, jclass, jlong handle) {
    return response(env, msime_client_reset_cache(static_cast<uint64_t>(handle)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_setChinesePunctuationRaw(JNIEnv *env, jclass, jlong handle, jboolean enabled) {
    return response(env, msime_client_set_chinese_punctuation(static_cast<uint64_t>(handle), enabled == JNI_TRUE));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_setCharacterWidthRaw(JNIEnv *env, jclass, jlong handle, jboolean fullwidth) {
    return response(env, msime_client_set_character_width(static_cast<uint64_t>(handle), fullwidth == JNI_TRUE));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_characterRaw(JNIEnv *env, jclass, jlong handle, jint ascii, jboolean shift) {
    if (ascii < 0 || ascii > 127) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Engine character must be ASCII");
        return nullptr;
    }
    return response(env, msime_client_character(static_cast<uint64_t>(handle), static_cast<uint8_t>(ascii), shift == JNI_TRUE));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_punctuationWithContextRaw(JNIEnv *env, jclass, jlong handle, jint ascii, jint preceding) {
    if (ascii < 0 || ascii > 127 || preceding < 0) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Invalid punctuation context");
        return nullptr;
    }
    return response(env, msime_client_punctuation_with_context(static_cast<uint64_t>(handle), static_cast<uint8_t>(ascii), static_cast<uint32_t>(preceding)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_glideRaw(JNIEnv *env, jclass, jlong handle, jbyteArray request) {
    if (!request) return response(env, msime_client_glide(static_cast<uint64_t>(handle), nullptr, 0));
    jsize length = env->GetArrayLength(request);
    if (length > kGlideRequestLimit) {
        return response(env, msime_client_glide(static_cast<uint64_t>(handle), nullptr, 0));
    }
    jbyte *bytes = env->GetByteArrayElements(request, nullptr);
    if (!bytes) return nullptr;
    char *result = msime_client_glide(static_cast<uint64_t>(handle),
        reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(request, bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_smartPunctuationArmRaw(JNIEnv *env, jclass, jlong handle, jbyteArray request) {
    if (!request) return response(env, msime_client_smart_punctuation_arm(static_cast<uint64_t>(handle), nullptr, 0));
    jsize length = env->GetArrayLength(request);
    if (length > kSmartPunctuationRequestLimit) {
        return response(env, msime_client_smart_punctuation_arm(static_cast<uint64_t>(handle), nullptr, 0));
    }
    jbyte *bytes = env->GetByteArrayElements(request, nullptr);
    if (!bytes) return nullptr;
    char *result = msime_client_smart_punctuation_arm(static_cast<uint64_t>(handle),
        reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(request, bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_smartPunctuationDecideRaw(JNIEnv *env, jclass, jlong handle, jbyteArray request) {
    if (!request) return response(env, msime_client_smart_punctuation_decide(static_cast<uint64_t>(handle), nullptr, 0));
    jsize length = env->GetArrayLength(request);
    if (length > kSmartPunctuationRequestLimit) {
        return response(env, msime_client_smart_punctuation_decide(static_cast<uint64_t>(handle), nullptr, 0));
    }
    jbyte *bytes = env->GetByteArrayElements(request, nullptr);
    if (!bytes) return nullptr;
    char *result = msime_client_smart_punctuation_decide(static_cast<uint64_t>(handle),
        reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(request, bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_commandRaw(JNIEnv *env, jclass, jlong handle, jint command) {
    return response(env, msime_client_command(static_cast<uint64_t>(handle), static_cast<uint32_t>(command)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_selectRaw(JNIEnv *env, jclass, jlong handle, jlong generation, jlong index) {
    if (index < 0 || static_cast<uint64_t>(index) > std::numeric_limits<size_t>::max()) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Invalid candidate index");
        return nullptr;
    }
    return response(env, msime_client_select(static_cast<uint64_t>(handle), static_cast<uint64_t>(generation), static_cast<size_t>(index)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_selectAnyCandidateRaw(JNIEnv *env, jclass, jlong handle, jlong generation, jlong index) {
    if (index < 0 || static_cast<uint64_t>(index) > std::numeric_limits<size_t>::max()) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Invalid candidate index");
        return nullptr;
    }
    return response(env, msime_client_select_any_candidate(static_cast<uint64_t>(handle), static_cast<uint64_t>(generation), static_cast<size_t>(index)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_pinCandidateRaw(JNIEnv *env, jclass, jlong handle, jlong generation, jlong index) {
    if (index < 0 || static_cast<uint64_t>(index) > std::numeric_limits<size_t>::max()) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Invalid candidate index");
        return nullptr;
    }
    return response(env, msime_client_pin_candidate(static_cast<uint64_t>(handle), static_cast<uint64_t>(generation), static_cast<size_t>(index)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_fixCandidatePositionRaw(JNIEnv *env, jclass, jlong handle, jlong generation, jlong index, jint position) {
    if (index < 0 || static_cast<uint64_t>(index) > std::numeric_limits<size_t>::max()) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Invalid candidate index");
        return nullptr;
    }
    if (position < 1 || position > 5) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Candidate position must be between 1 and 5");
        return nullptr;
    }
    return response(env, msime_client_fix_candidate_position(static_cast<uint64_t>(handle), static_cast<uint64_t>(generation), static_cast<size_t>(index), static_cast<uint8_t>(position)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_clearCandidatePositionRaw(JNIEnv *env, jclass, jlong handle, jlong generation, jlong index) {
    if (index < 0 || static_cast<uint64_t>(index) > std::numeric_limits<size_t>::max()) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Invalid candidate index");
        return nullptr;
    }
    return response(env, msime_client_clear_candidate_position(static_cast<uint64_t>(handle), static_cast<uint64_t>(generation), static_cast<size_t>(index)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_removeCandidateRaw(JNIEnv *env, jclass, jlong handle, jlong generation, jlong index) {
    if (index < 0 || static_cast<uint64_t>(index) > std::numeric_limits<size_t>::max()) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Invalid candidate index");
        return nullptr;
    }
    return response(env, msime_client_remove_candidate(static_cast<uint64_t>(handle), static_cast<uint64_t>(generation), static_cast<size_t>(index)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_selectEdgeRaw(JNIEnv *env, jclass, jlong handle, jlong generation, jlong index, jint edge) {
    if (index < 0 || static_cast<uint64_t>(index) > std::numeric_limits<size_t>::max()) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Invalid candidate index");
        return nullptr;
    }
    if (edge != MSIME_FIRST_HAN && edge != MSIME_LAST_HAN) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Invalid candidate edge");
        return nullptr;
    }
    return response(env, msime_client_select_edge(static_cast<uint64_t>(handle), static_cast<uint64_t>(generation), static_cast<size_t>(index), static_cast<uint8_t>(edge)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_chooseNineKeySpellingRaw(JNIEnv *env, jclass, jlong handle, jlong generation, jlong index) {
    if (index < 0 || static_cast<uint64_t>(index) > std::numeric_limits<size_t>::max()) {
        env->ThrowNew(env->FindClass("java/lang/IllegalArgumentException"), "Invalid nine-key spelling index");
        return nullptr;
    }
    return response(env, msime_client_choose_nine_key_spelling(static_cast<uint64_t>(handle), static_cast<uint64_t>(generation), static_cast<size_t>(index)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_viewRaw(JNIEnv *env, jclass, jlong handle) {
    return response(env, msime_client_view(static_cast<uint64_t>(handle)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_allCandidatesRaw(JNIEnv *env, jclass, jlong handle) {
    return response(env, msime_client_all_candidates(static_cast<uint64_t>(handle)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_applyTranslationsRaw(JNIEnv *env, jclass, jlong handle, jlong generation, jbyteArray translations) {
    if (!translations || generation < 0) {
        return response(env, msime_client_apply_translations(static_cast<uint64_t>(handle),
            static_cast<uint64_t>(generation), nullptr, 0));
    }
    jsize length = env->GetArrayLength(translations);
    if (length > kApplyTranslationsLimit) {
        return response(env, msime_client_apply_translations(static_cast<uint64_t>(handle),
            static_cast<uint64_t>(generation), nullptr, 0));
    }
    jbyte *bytes = env->GetByteArrayElements(translations, nullptr);
    if (!bytes) return nullptr;
    char *result = msime_client_apply_translations(static_cast<uint64_t>(handle),
        static_cast<uint64_t>(generation), reinterpret_cast<const uint8_t *>(bytes),
        static_cast<size_t>(length));
    env->ReleaseByteArrayElements(translations, bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_onlineQueryRaw(JNIEnv *env, jclass, jlong handle) {
    return response(env, msime_client_online_query(static_cast<uint64_t>(handle)));
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_cloudRequestUrlRaw(JNIEnv *env, jclass, jbyteArray query) {
    return bounded_request(env, query, kOnlineQueryLimit, msime_client_cloud_request_url);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_aiRequestForQueryRaw(JNIEnv *env, jclass, jlong handle, jbyteArray query) {
    if (!query) {
        return response(env, msime_client_ai_request_for_query(
            static_cast<uint64_t>(handle), nullptr, 0));
    }
    jsize length = env->GetArrayLength(query);
    if (length > kOnlineQueryLimit) {
        return response(env, msime_client_ai_request_for_query(
            static_cast<uint64_t>(handle), nullptr, 0));
    }
    jbyte *bytes = env->GetByteArrayElements(query, nullptr);
    if (!bytes) return nullptr;
    char *result = msime_client_ai_request_for_query(static_cast<uint64_t>(handle),
        reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(query, bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_applyCloudResponseRaw(JNIEnv *env, jclass, jlong handle, jbyteArray query, jbyteArray body) {
    if (!query || !body) {
        return response(env, msime_client_apply_cloud_response(
            static_cast<uint64_t>(handle), nullptr, 0, nullptr, 0));
    }
    jsize queryLength = env->GetArrayLength(query);
    jsize bodyLength = env->GetArrayLength(body);
    if (queryLength > kOnlineQueryLimit || bodyLength > kOnlineBodyLimit) {
        return response(env, msime_client_apply_cloud_response(
            static_cast<uint64_t>(handle), nullptr, 0, nullptr, 0));
    }
    jbyte *queryBytes = env->GetByteArrayElements(query, nullptr);
    if (!queryBytes) return nullptr;
    jbyte *bodyBytes = env->GetByteArrayElements(body, nullptr);
    if (!bodyBytes) {
        env->ReleaseByteArrayElements(query, queryBytes, JNI_ABORT);
        return nullptr;
    }
    char *result = msime_client_apply_cloud_response(static_cast<uint64_t>(handle),
        reinterpret_cast<const uint8_t *>(queryBytes), static_cast<size_t>(queryLength),
        reinterpret_cast<const uint8_t *>(bodyBytes), static_cast<size_t>(bodyLength));
    env->ReleaseByteArrayElements(body, bodyBytes, JNI_ABORT);
    env->ReleaseByteArrayElements(query, queryBytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_applyOnlineCandidatesRaw(JNIEnv *env, jclass, jlong handle, jbyteArray query, jbyteArray candidates, jint source) {
    if (!query || !candidates || source < 0 || source > 255) {
        return response(env, msime_client_apply_online_candidates(
            static_cast<uint64_t>(handle), nullptr, 0, nullptr, 0, 0));
    }
    jsize queryLength = env->GetArrayLength(query);
    jsize candidatesLength = env->GetArrayLength(candidates);
    if (queryLength > kOnlineQueryLimit || candidatesLength > kOnlineCandidatesLimit) {
        return response(env, msime_client_apply_online_candidates(
            static_cast<uint64_t>(handle), nullptr, 0, nullptr, 0, 0));
    }
    jbyte *queryBytes = env->GetByteArrayElements(query, nullptr);
    if (!queryBytes) return nullptr;
    jbyte *candidateBytes = env->GetByteArrayElements(candidates, nullptr);
    if (!candidateBytes) {
        env->ReleaseByteArrayElements(query, queryBytes, JNI_ABORT);
        return nullptr;
    }
    char *result = msime_client_apply_online_candidates(static_cast<uint64_t>(handle),
        reinterpret_cast<const uint8_t *>(queryBytes), static_cast<size_t>(queryLength),
        reinterpret_cast<const uint8_t *>(candidateBytes), static_cast<size_t>(candidatesLength),
        static_cast<uint8_t>(source));
    env->ReleaseByteArrayElements(candidates, candidateBytes, JNI_ABORT);
    env->ReleaseByteArrayElements(query, queryBytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_destroyRaw(JNIEnv *env, jclass, jlong handle) {
    return response(env, msime_client_destroy(static_cast<uint64_t>(handle)));
}
}

// ---- on-device speech recognition ----

namespace {
// One dictation. The cancel flag is shared with the recognizer so a cancel from the window's thread stops a decode running on the worker; everything else is touched only by the worker that created the session.
struct LocalSpeech {
    std::shared_ptr<std::atomic_bool> cancelled = std::make_shared<std::atomic_bool>(false);
    std::unique_ptr<msime::voice::LocalAsrSession> session;
    std::mutex partial_mutex;
    std::string partial;
    bool partial_changed = false;
};
constexpr jint kLocalSpeechChunkLimit = 1600;
constexpr jsize kLocalSpeechModelPathLimit = 4096;
constexpr jsize kLocalSpeechLanguageLimit = 256;
constexpr jsize kLocalSpeechHotwordsLimit = 256 * 1024;

jbyteArray bytes_of(JNIEnv *env, const std::string &text) {
    if (text.size() > static_cast<size_t>(std::numeric_limits<jsize>::max())) return nullptr;
    jbyteArray out = env->NewByteArray(static_cast<jsize>(text.size()));
    if (out) env->SetByteArrayRegion(out, 0, static_cast<jsize>(text.size()), reinterpret_cast<const jbyte *>(text.data()));
    return out;
}

bool bounded_utf8(JNIEnv *env, jbyteArray value, jsize limit, std::string &out) {
    out.clear();
    if (!value) return true;
    jsize length = env->GetArrayLength(value);
    if (length > limit) return false;
    jbyte *bytes = env->GetByteArrayElements(value, nullptr);
    if (!bytes) return false;
    out.assign(reinterpret_cast<const char *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(value, bytes, JNI_ABORT);
    return true;
}

void throw_state(JNIEnv *env, const char *message) {
    jclass type = env->FindClass("java/lang/IllegalStateException");
    if (type) env->ThrowNew(type, message);
}

LocalSpeech *speech(jlong handle) {
    if (handle <= 0) return nullptr;
    return reinterpret_cast<LocalSpeech *>(static_cast<intptr_t>(handle));
}

// 先按头文件写明的上限检查长度：超限的请求在取得本地视图之前就交给宿主以空请求拒绝，避免 GetByteArrayElements 复制一份超大数组。
template <typename Call> jbyteArray bounded_request(JNIEnv *env, jbyteArray request, jsize limit, Call call) {
    if (!request) return response(env, call(nullptr, 0));
    jsize length = env->GetArrayLength(request);
    if (length > limit) return response(env, call(nullptr, 0));
    jbyte *bytes = env->GetByteArrayElements(request, nullptr);
    if (!bytes) return nullptr;
    char *result = call(reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(request, bytes, JNI_ABORT);
    return response(env, result);
}

// 两段 UTF-8 输入（请求与资源目录）的宿主调用；任一段超限或为空引用时交给宿主以空请求拒绝。
template <typename Call> jbyteArray bounded_pair(JNIEnv *env, jbyteArray first, jsize first_limit, jbyteArray second, jsize second_limit, Call call) {
    if (!first || !second) return response(env, call(nullptr, 0, nullptr, 0));
    jsize first_length = env->GetArrayLength(first);
    jsize second_length = env->GetArrayLength(second);
    if (first_length > first_limit || second_length > second_limit) {
        return response(env, call(nullptr, 0, nullptr, 0));
    }
    jbyte *first_bytes = env->GetByteArrayElements(first, nullptr);
    if (!first_bytes) return nullptr;
    jbyte *second_bytes = env->GetByteArrayElements(second, nullptr);
    if (!second_bytes) {
        env->ReleaseByteArrayElements(first, first_bytes, JNI_ABORT);
        return nullptr;
    }
    char *result = call(
        reinterpret_cast<const uint8_t *>(first_bytes), static_cast<size_t>(first_length),
        reinterpret_cast<const uint8_t *>(second_bytes), static_cast<size_t>(second_length));
    env->ReleaseByteArrayElements(second, second_bytes, JNI_ABORT);
    env->ReleaseByteArrayElements(first, first_bytes, JNI_ABORT);
    return response(env, result);
}

// 各宿主调用在头文件里写明的请求上限（字节），JNI 只按它们挡掉明显超限的输入，校验仍由宿主完成。
constexpr jsize kPathLimit = 16384;
constexpr jsize kDictionaryRequestLimit = 2248576;
constexpr jsize kHansTextLimit = 64 * 1024;
constexpr jsize kImportRequestLimit = 1200000;
constexpr jsize kPluginsRequestLimit = 2 * 1024 * 1024;
constexpr jsize kCommunityLibraryLimit = 4000000;
constexpr jsize kAiSkinPlanLimit = 12 * 1024 * 1024;
constexpr jsize kSkinTrialLimit = 16384;
constexpr jsize kCommunitySkinInstallLimit = 9000000;
constexpr jsize kKeySoundPackLimit = 65536;
constexpr jsize kCommonPhrasesLimit = 4 * 1024 * 1024;
constexpr jsize kDictionaryCollectionsLimit = 17 * 1024 * 1024;
constexpr jsize kDiagnosticBundleLimit = 65536;
constexpr jsize kAccountSettingsLimit = 4 * 1024 * 1024;
constexpr jsize kAppThemeRequestLimit = 4096;
constexpr jsize kPlatformLimit = 64;
constexpr jsize kVoiceRequestLimit = 1 * 1024 * 1024;
constexpr jsize kResourcePackRequestLimit = 16384;
constexpr jsize kResourcePackIdLimit = 256;

// 资源包安装的进度回调上下文：回调在调用 msime_client_resource_pack_install 的同一线程上触发，所以可以直接用这个线程的 JNIEnv。
struct ResourcePackProgress {
    JNIEnv *env;
    jobject listener;
    jmethodID method;
};

// 把一次进度转给 Java 的 listener。listener 抛出异常后不再调用它（异常挂起时不能再调 JNI），异常留到安装返回后交给 Java。每次的阶段字符串都及时释放，下载大文件时回调次数很多，不释放会撑满局部引用表。
void resource_pack_progress(void *context, const char *phase, uint64_t done, uint64_t total) {
    auto *progress = static_cast<ResourcePackProgress *>(context);
    if (!progress || !phase || progress->env->ExceptionCheck()) return;
    jstring name = progress->env->NewStringUTF(phase);
    if (!name) return;
    const auto bounded = [](uint64_t value) {
        return static_cast<jlong>(value > static_cast<uint64_t>(std::numeric_limits<jlong>::max())
            ? std::numeric_limits<jlong>::max() : value);
    };
    progress->env->CallVoidMethod(progress->listener, progress->method, name, bounded(done), bounded(total));
    progress->env->DeleteLocalRef(name);
}
} // namespace

extern "C" {
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_voiceHotwordsRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kVoiceRequestLimit, msime_client_voice_hotwords);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_voiceHotwordCorrectRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kVoiceRequestLimit, msime_client_voice_hotword_correct);
}
// 应用主题目录与当季颜色：纯计算。
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_appThemeCatalogRaw(JNIEnv *env, jclass) {
    return response(env, msime_client_app_theme_catalog());
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_resolveAppThemeRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kAppThemeRequestLimit, msime_client_resolve_app_theme);
}
// 「重置所有设置」：在偏好锁里按修订号比较并交换写回默认偏好。
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_restoreDefaultPreferencesRaw(JNIEnv *env, jclass, jbyteArray directory, jlong expected_revision) {
    if (!directory || expected_revision < 0) {
        return response(env, msime_client_restore_default_preferences(nullptr, 0, 0));
    }
    jsize length = env->GetArrayLength(directory);
    if (length > kPathLimit) return response(env, msime_client_restore_default_preferences(nullptr, 0, 0));
    jbyte *bytes = env->GetByteArrayElements(directory, nullptr);
    if (!bytes) return nullptr;
    char *result = msime_client_restore_default_preferences(
        reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length),
        static_cast<uint64_t>(expected_revision));
    env->ReleaseByteArrayElements(directory, bytes, JNI_ABORT);
    return response(env, result);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_defaultPreferencesRaw(JNIEnv *env, jclass) {
    return response(env, msime_client_default_preferences());
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_hostCapabilitiesRaw(JNIEnv *env, jclass, jbyteArray platform) {
    return bounded_request(env, platform, kPlatformLimit, msime_client_host_capabilities);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_dictionaryRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kDictionaryRequestLimit, msime_client_dictionary);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_personalDictionaryRequestRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kDictionaryRequestLimit, msime_client_personal_dictionary_request);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_dictionaryHansEntriesRaw(JNIEnv *env, jclass, jbyteArray text, jbyteArray resources) {
    return bounded_pair(env, text, kHansTextLimit, resources, kPathLimit, msime_client_dictionary_hans_entries);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_dictionaryImportEntriesRaw(JNIEnv *env, jclass, jbyteArray request, jbyteArray resources) {
    return bounded_pair(env, request, kImportRequestLimit, resources, kPathLimit, msime_client_dictionary_import_entries);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_dictionaryManifestRaw(JNIEnv *env, jclass, jbyteArray resources) {
    return bounded_request(env, resources, kPathLimit, msime_client_dictionary_manifest);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_pluginsRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kPluginsRequestLimit, msime_client_plugins);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_communityResourceLibraryRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kCommunityLibraryLimit, msime_client_community_resource_library);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_aiSkinPlanRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kAiSkinPlanLimit, msime_client_ai_skin_plan);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_keyboardSkinTrialRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kSkinTrialLimit, msime_client_keyboard_skin_trial);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_communitySkinInstallRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kCommunitySkinInstallLimit, msime_client_community_skin_install);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_keySoundPackRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kKeySoundPackLimit, msime_client_key_sound_pack);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_commonPhrasesRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kCommonPhrasesLimit, msime_client_common_phrases);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_dictionaryCollectionsRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kDictionaryCollectionsLimit, msime_client_dictionary_collections);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_diagnosticBundleRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kDiagnosticBundleLimit, msime_client_diagnostic_bundle);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_accountSettingsExportRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kAccountSettingsLimit, msime_client_account_settings_export);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_accountSettingsApplyRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kAccountSettingsLimit, msime_client_account_settings_apply);
}
// 按需下载的资源包（msime_client.h 里 msime_client_resource_pack_* 一节）。列出只读几个文件属性；安装和收编阻塞，只在主进程的工作线程上调用。
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_resourcePacksRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kResourcePackRequestLimit, msime_client_resource_packs);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_resourcePackAdoptRaw(JNIEnv *env, jclass, jbyteArray request) {
    return bounded_request(env, request, kResourcePackRequestLimit, msime_client_resource_pack_adopt);
}
// listener 可以为 null；不为 null 时在本线程上收到 onProgress(phase, done, total)。listener 抛出的异常在安装结束后原样抛给调用方，此时不返回结果。
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_resourcePackInstallRaw(JNIEnv *env, jclass, jbyteArray request, jobject listener) {
    ResourcePackProgress progress{env, listener, nullptr};
    if (listener) {
        jclass type = env->GetObjectClass(listener);
        if (!type) return nullptr;
        progress.method = env->GetMethodID(type, "onProgress", "(Ljava/lang/String;JJ)V");
        env->DeleteLocalRef(type);
        if (!progress.method) return nullptr;
    }
    const auto install = [&](const uint8_t *bytes, size_t length) {
        return msime_client_resource_pack_install(bytes, length,
            listener ? resource_pack_progress : nullptr, listener ? &progress : nullptr);
    };
    if (!request) return response(env, install(nullptr, 0));
    jsize length = env->GetArrayLength(request);
    if (length > kResourcePackRequestLimit) return response(env, install(nullptr, 0));
    jbyte *bytes = env->GetByteArrayElements(request, nullptr);
    if (!bytes) return nullptr;
    char *result = install(reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(request, bytes, JNI_ABORT);
    if (env->ExceptionCheck()) {
        msime_client_string_free(result);
        return nullptr;
    }
    return response(env, result);
}
// 任何线程都可以调用，立即返回；pack 为 null 时停下本进程里所有资源包的安装。
JNIEXPORT void JNICALL Java_app_msime_android_NativeClient_resourcePackCancelRaw(JNIEnv *env, jclass, jbyteArray pack) {
    if (!pack) {
        msime_client_resource_pack_cancel(nullptr, 0);
        return;
    }
    jsize length = env->GetArrayLength(pack);
    if (length > kResourcePackIdLimit) return;
    jbyte *bytes = env->GetByteArrayElements(pack, nullptr);
    if (!bytes) return;
    msime_client_resource_pack_cancel(reinterpret_cast<const uint8_t *>(bytes), static_cast<size_t>(length));
    env->ReleaseByteArrayElements(pack, bytes, JNI_ABORT);
}
// 指定 libsherpa-onnx-c-api.so 的绝对路径（下载的 voice-runtime 资源包）。之前没加载成功的结果作废，下一次 localSpeechAvailableRaw 按新路径重试；已经加载成功后不再改变。
JNIEXPORT void JNICALL Java_app_msime_android_NativeClient_localSpeechRuntimeRaw(JNIEnv *env, jclass, jbyteArray path) {
    msime::voice::set_sherpa_library_path(utf8(env, path));
}
JNIEXPORT jboolean JNICALL Java_app_msime_android_NativeClient_localSpeechAvailableRaw(JNIEnv *, jclass) {
    return msime::voice::sherpa_runtime_available() ? JNI_TRUE : JNI_FALSE;
}
JNIEXPORT jlong JNICALL Java_app_msime_android_NativeClient_localSpeechCreateRaw(JNIEnv *, jclass) {
    return static_cast<jlong>(reinterpret_cast<intptr_t>(new LocalSpeech()));
}
// Loads the model (seconds on a phone the first time; cached afterwards) and opens the session. Hotwords arrive newline-separated. Returns null on success, else a message for logs.
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_localSpeechStartRaw(JNIEnv *env, jclass, jlong handle, jbyteArray model, jbyteArray language, jbyteArray hotwords, jint threads) {
    LocalSpeech *state = speech(handle);
    if (!state || state->session) return bytes_of(env, "invalid local speech session");
    std::string model_text;
    std::string language_text;
    std::string hotwords_text;
    if (!bounded_utf8(env, model, kLocalSpeechModelPathLimit, model_text)
            || !bounded_utf8(env, language, kLocalSpeechLanguageLimit, language_text)
            || !bounded_utf8(env, hotwords, kLocalSpeechHotwordsLimit, hotwords_text)) {
        return bytes_of(env, "invalid local speech input");
    }
    msime::voice::LocalAsrOptions options;
    options.model_dir = model_text;
    options.language = language_text;
    options.threads = threads < 0 ? 0 : threads;
    const std::string &words = hotwords_text;
    for (size_t start = 0; start < words.size();) {
        size_t end = words.find('\n', start);
        if (end == std::string::npos) end = words.size();
        if (end > start) options.hotwords.emplace_back(words.substr(start, end - start));
        start = end + 1;
    }
    try {
        state->session = std::make_unique<msime::voice::LocalAsrSession>(
            options,
            [state](const std::string &text) {
                std::lock_guard<std::mutex> lock(state->partial_mutex);
                state->partial = text;
                state->partial_changed = true;
            },
            state->cancelled);
        return nullptr;
    } catch (const std::exception &error) {
        return bytes_of(env, error.what());
    }
}
// Feeds 16 kHz mono PCM16. Returns the whole transcript so far when it changed, else null. Throws IllegalStateException when the session was cancelled or the recognizer failed.
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_localSpeechAcceptRaw(JNIEnv *env, jclass, jlong handle, jshortArray pcm, jint count) {
    LocalSpeech *state = speech(handle);
    if (!state || !state->session || !pcm || count < 0 || count > kLocalSpeechChunkLimit
            || count > env->GetArrayLength(pcm)) {
        throw_state(env, "invalid local speech input");
        return nullptr;
    }
    std::vector<jshort> samples(static_cast<size_t>(count));
    env->GetShortArrayRegion(pcm, 0, count, samples.data());
    if (env->ExceptionCheck()) return nullptr;
    std::vector<float> floats(samples.size());
    for (size_t index = 0; index < samples.size(); index++) floats[index] = static_cast<float>(samples[index]) / 32768.0f;
    try {
        state->session->accept(floats.data(), floats.size());
    } catch (const std::exception &error) {
        throw_state(env, error.what());
        return nullptr;
    }
    std::lock_guard<std::mutex> lock(state->partial_mutex);
    if (!state->partial_changed) return nullptr;
    state->partial_changed = false;
    return bytes_of(env, state->partial);
}
JNIEXPORT jbyteArray JNICALL Java_app_msime_android_NativeClient_localSpeechFinishRaw(JNIEnv *env, jclass, jlong handle) {
    LocalSpeech *state = speech(handle);
    if (!state || !state->session) {
        throw_state(env, "invalid local speech session");
        return nullptr;
    }
    try {
        return bytes_of(env, state->session->finish());
    } catch (const std::exception &error) {
        throw_state(env, error.what());
        return nullptr;
    }
}
// Any thread: a decode in progress on the worker stops at its next check.
JNIEXPORT void JNICALL Java_app_msime_android_NativeClient_localSpeechCancelRaw(JNIEnv *, jclass, jlong handle) {
    if (LocalSpeech *state = speech(handle)) state->cancelled->store(true);
}
JNIEXPORT void JNICALL Java_app_msime_android_NativeClient_localSpeechDestroyRaw(JNIEnv *, jclass, jlong handle) {
    if (handle > 0) delete speech(handle);
}
// Drops models no session has used for `idleMillis`; 0 drops every model not in use. Returns how many were dropped.
JNIEXPORT jint JNICALL Java_app_msime_android_NativeClient_localSpeechReleaseRaw(JNIEnv *, jclass, jlong idleMillis) {
    const size_t released = idleMillis <= 0
        ? msime::voice::release_local_models()
        : msime::voice::release_idle_local_models(std::chrono::milliseconds(idleMillis));
    return static_cast<jint>(released);
}
}
