#include "msime_client.h"
#include "../../common/HostApiString.h"
#include "key_sound_render.h"
#include <napi/native_api.h>
#include <zlib.h>
#include <cerrno>
#include <cstring>
#include <fcntl.h>
#include <limits>
#include <sys/stat.h>
#include <string>
#include <unistd.h>
#include <vector>

// Mirrors platforms/android/native/client_jni.cpp. The shared C ABI takes UTF-8 JSON in and returns
// UTF-8 JSON out, so every binding here is the same three steps: read the arguments, call one
// msime_client_* entry point, hand the response back and free it.

struct SnapshotReader {
    explicit SnapshotReader(const std::string &path)
        : descriptor(::open(path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK)) {
        struct stat metadata{};
        if (descriptor < 0 || ::fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode)) {
            if (descriptor >= 0) ::close(descriptor);
            descriptor = -1;
        }
    }
    ~SnapshotReader() { if (descriptor >= 0) ::close(descriptor); }
    SnapshotReader(const SnapshotReader &) = delete;
    SnapshotReader &operator=(const SnapshotReader &) = delete;

    int next() noexcept {
        if (pending != kNoPending) {
            const int value = pending;
            pending = kNoPending;
            return value;
        }
        for (;;) {
            if (offset < available) return buffer[offset++];
            const ssize_t count = ::read(descriptor, buffer, sizeof(buffer));
            if (count > 0) {
                offset = 0;
                available = static_cast<size_t>(count);
                continue;
            }
            if (count == 0) return kEof;
            if (errno == EINTR) continue;
            return kError;
        }
    }

    int peek() noexcept {
        if (pending == kNoPending) pending = next();
        return pending;
    }

    static constexpr int kNoPending = -3;
    static constexpr int kEof = -1;
    static constexpr int kError = -2;
    int descriptor = -1;
    unsigned char buffer[4096]{};
    size_t offset = 0;
    size_t available = 0;
    int pending = kNoPending;
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
            const int value = reader->next();
            if (value == SnapshotReader::kEof) {
                ended = true;
                break;
            }
            if (value == SnapshotReader::kError) return -1;
            if (value == '\n') {
                ended = true;
                break;
            }
            if (value == '\r' && reader->peek() == '\n') {
                reader->next();
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

// The Rust side owns the response buffer until it is handed back, so every exit path frees it.
static napi_value response(napi_env env, char *value) {
    auto owned = msime::host_api::own_string(value);
    if (!owned) return nullptr;
    const size_t length = std::strlen(owned.get());
    napi_value output = nullptr;
    const napi_status status = napi_create_string_utf8(env, owned.get(), length, &output);
    return status == napi_ok ? output : nullptr;
}

// Arguments arrive as ArkTS strings holding UTF-8 JSON. napi_create/get_string_utf8 is plain UTF-8,
// unlike JNI's modified form, so supplementary characters in candidates and resource paths cross
// unchanged without a byte-array detour.
// Keep the bridge from copying an attacker-controlled ArkTS string before the
// shared C ABI can apply the per-operation request limit.  The largest JSON
// request accepted by that ABI is dictionary collections (17 MiB); larger
// inputs are malformed for every N-API entry point and are forwarded as the
// same empty buffer used for other argument failures.
constexpr size_t kMaxTextArgumentBytes = 17 * 1024 * 1024;

static bool argumentText(napi_env env, napi_value value, std::string &out) {
    size_t length = 0;
    if (napi_get_value_string_utf8(env, value, nullptr, 0, &length) != napi_ok) return false;
    if (length > kMaxTextArgumentBytes) return false;
    out.assign(length, '\0');
    size_t written = 0;
    if (napi_get_value_string_utf8(env, value, out.data(), length + 1, &written) != napi_ok) {
        return false;
    }
    out.resize(written);
    return true;
}

static bool argumentHandle(napi_env env, napi_value value, uint64_t &out) {
    int64_t handle = 0;
    if (napi_get_value_int64(env, value, &handle) != napi_ok || handle < 0) return false;
    out = static_cast<uint64_t>(handle);
    return true;
}

static bool argumentFlag(napi_env env, napi_value value, bool &out) {
    return napi_get_value_bool(env, value, &out) == napi_ok;
}

static bool argumentIndex(napi_env env, napi_value value, size_t &out) {
    int64_t index = 0;
    if (napi_get_value_int64(env, value, &index) != napi_ok) return false;
    if (index < 0 || static_cast<uint64_t>(index) > std::numeric_limits<size_t>::max()) return false;
    out = static_cast<size_t>(index);
    return true;
}

static bool argumentInt32(napi_env env, napi_value value, int32_t &out) {
    return napi_get_value_int32(env, value, &out) == napi_ok;
}

static bool argumentArrayBuffer(napi_env env, napi_value value, std::vector<uint8_t> &out,
                               size_t maximum = std::numeric_limits<size_t>::max()) {
    bool is_array_buffer = false;
    if (napi_is_arraybuffer(env, value, &is_array_buffer) != napi_ok || !is_array_buffer) return false;
    void *data = nullptr;
    size_t length = 0;
    if (napi_get_arraybuffer_info(env, value, &data, &length) != napi_ok) return false;
    // N-API may expose a zero-length ArrayBuffer with a null data pointer. Empty PCM is a
    // legitimate final audio frame, so only require storage when there are bytes to copy.
    // Check the byte length before copying.  The callers that decode provider frames have a
    // one-megabyte protocol limit; copying an untrusted ArrayBuffer first would let a malformed
    // bridge call allocate an arbitrary amount of native memory before it is rejected.
    if (length > maximum) return false;
    if (length == 0) {
        out.clear();
        return true;
    }
    if (!data) return false;
    out.assign(static_cast<const uint8_t *>(data), static_cast<const uint8_t *>(data) + length);
    return true;
}

static bool gzipCompress(const std::vector<uint8_t> &input, std::vector<uint8_t> &output) {
    if (input.size() > static_cast<size_t>(std::numeric_limits<uInt>::max())) return false;
    z_stream stream{};
    if (deflateInit2(&stream, Z_DEFAULT_COMPRESSION, Z_DEFLATED, 15 + 16, 8,
            Z_DEFAULT_STRATEGY) != Z_OK) return false;
    const size_t capacity = compressBound(static_cast<uLong>(input.size())) + 32;
    output.assign(capacity, 0);
    stream.next_in = const_cast<Bytef *>(reinterpret_cast<const Bytef *>(input.data()));
    stream.avail_in = static_cast<uInt>(input.size());
    stream.next_out = reinterpret_cast<Bytef *>(output.data());
    stream.avail_out = static_cast<uInt>(output.size());
    const int status = deflate(&stream, Z_FINISH);
    const bool ok = status == Z_STREAM_END;
    if (ok) output.resize(stream.total_out);
    deflateEnd(&stream);
    return ok;
}

static bool gzipDecompress(const uint8_t *input, size_t input_length, std::vector<uint8_t> &output) {
    constexpr size_t max_output = 1024 * 1024;
    if (!input || input_length == 0 || input_length > max_output) return false;
    z_stream stream{};
    if (inflateInit2(&stream, 15 + 16) != Z_OK) return false;
    output.assign(max_output, 0);
    stream.next_in = const_cast<Bytef *>(reinterpret_cast<const Bytef *>(input));
    stream.avail_in = static_cast<uInt>(input_length);
    stream.next_out = reinterpret_cast<Bytef *>(output.data());
    stream.avail_out = static_cast<uInt>(output.size());
    const int status = inflate(&stream, Z_FINISH);
    const bool ok = status == Z_STREAM_END && stream.avail_in == 0;
    if (ok) output.resize(stream.total_out);
    inflateEnd(&stream);
    return ok;
}

static bool arguments(napi_env env, napi_callback_info info, size_t expected,
                      std::vector<napi_value> &out) {
    size_t count = expected;
    out.assign(expected, nullptr);
    if (napi_get_cb_info(env, info, &count, out.data(), nullptr, nullptr) != napi_ok) return false;
    return count >= expected;
}

static napi_value invalid(napi_env env, const char *message) {
    napi_throw_error(env, nullptr, message);
    return nullptr;
}

// A malformed document reaches the Rust side as an empty buffer, which answers with the same
// structured error every other host sees rather than a native crash.
#define TEXT_ENTRY(name, call)                                                                     \
    static napi_value name(napi_env env, napi_callback_info info) {                                 \
        std::vector<napi_value> argv;                                                               \
        std::string text;                                                                           \
        if (!arguments(env, info, 1, argv) || !argumentText(env, argv[0], text)) {                  \
            return response(env, call(nullptr, 0));                                                 \
        }                                                                                           \
        return response(env,                                                                        \
            call(reinterpret_cast<const uint8_t *>(text.data()), text.size()));                     \
    }

TEXT_ENTRY(LoadPreferences, msime_client_load_preferences)
TEXT_ENTRY(SkinCatalog, msime_client_skin_catalog)
TEXT_ENTRY(ResolveTheme, msime_client_resolve_theme)
// 按宿主本地月份和深色模式给出应用主题的颜色；它是纯函数，所以设置页和键盘都在主线程上解析。
TEXT_ENTRY(ResolveAppTheme, msime_client_resolve_app_theme)
TEXT_ENTRY(DictionaryManifest, msime_client_dictionary_manifest)
TEXT_ENTRY(SkinResource, msime_client_skin_resource)
TEXT_ENTRY(SkinToolbarStylesheet, msime_client_skin_toolbar_stylesheet)
TEXT_ENTRY(KeyboardSkinTrial, msime_client_keyboard_skin_trial)
TEXT_ENTRY(AiSkinPlan, msime_client_ai_skin_plan)
TEXT_ENTRY(Dictionary, msime_client_dictionary)
TEXT_ENTRY(TypingStatistics, msime_client_typing_statistics)
TEXT_ENTRY(MobileClipboardHistory, msime_client_mobile_clipboard_history)
TEXT_ENTRY(PersonalDictionarySync, msime_client_personal_dictionary_sync)
TEXT_ENTRY(PersonalDictionaryRequest, msime_client_personal_dictionary_request)
// The keyboard's flush after each personal-dictionary drain: it moves at most one queue batch, so it stays synchronous on the keyboard's own thread like the drain it follows.
TEXT_ENTRY(DictionaryCollections, msime_client_dictionary_collections)
TEXT_ENTRY(PrepareHost, msime_client_prepare_host)
TEXT_ENTRY(SnapshotVersion, msime_client_snapshot_version)
TEXT_ENTRY(SnapshotInspect, msime_client_snapshot_inspect)
TEXT_ENTRY(SnapshotQueue, msime_client_snapshot_queue)
TEXT_ENTRY(CloudRequestUrl, msime_client_cloud_request_url)
TEXT_ENTRY(KeySoundPack, msime_client_key_sound_pack)
TEXT_ENTRY(MusicPack, msime_client_music_pack)
TEXT_ENTRY(Plugins, msime_client_plugins)
TEXT_ENTRY(Create, msime_client_create)
// Usage reporting without the network: each touches a few small files under the telemetry directory, so the crash observer can call record_crash before the process goes.
TEXT_ENTRY(TelemetryBegin, msime_client_telemetry_begin)
TEXT_ENTRY(TelemetryEnd, msime_client_telemetry_end)
TEXT_ENTRY(TelemetryRecordCrash, msime_client_telemetry_record_crash)
TEXT_ENTRY(TelemetryClear, msime_client_telemetry_clear)
TEXT_ENTRY(NoticeDismiss, msime_client_notice_dismiss)

struct SnapshotRestoreWork {
    napi_async_work work = nullptr;
    napi_deferred deferred = nullptr;
    std::string request;
    std::string file;
    char *result = nullptr;
};

static void executeSnapshotRestore(napi_env, void *data) {
    auto *work = static_cast<SnapshotRestoreWork *>(data);
    work->result = msime_client_snapshot_restore(
        reinterpret_cast<const uint8_t *>(work->request.data()), work->request.size(),
        reinterpret_cast<const uint8_t *>(work->file.data()), work->file.size());
}

static void completeSnapshotRestore(napi_env env, napi_status status, void *data) {
    auto *work = static_cast<SnapshotRestoreWork *>(data);
    auto owned = msime::host_api::own_string(work->result);
    napi_value value = nullptr;
    bool resolved = status == napi_ok && owned
        && napi_create_string_utf8(env, owned.get(), std::strlen(owned.get()), &value) == napi_ok;
    if (resolved) {
        napi_resolve_deferred(env, work->deferred, value);
    } else {
        napi_value message = nullptr;
        napi_value error = nullptr;
        if (napi_create_string_utf8(env, "Snapshot restore worker failed", NAPI_AUTO_LENGTH,
                &message) == napi_ok
                && napi_create_error(env, nullptr, message, &error) == napi_ok) {
            napi_reject_deferred(env, work->deferred, error);
        } else {
            napi_get_undefined(env, &error);
            napi_reject_deferred(env, work->deferred, error);
        }
    }
    napi_delete_async_work(env, work->work);
    delete work;
}

static napi_value SnapshotRestore(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    auto *work = new SnapshotRestoreWork();
    if (!arguments(env, info, 2, argv) || !argumentText(env, argv[0], work->request)
            || !argumentText(env, argv[1], work->file)) {
        delete work;
        return invalid(env, "Expected a snapshot restore request and private file path");
    }
    napi_value promise = nullptr;
    napi_value resource = nullptr;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok
            || napi_create_string_utf8(env, "MSIME snapshot restore", NAPI_AUTO_LENGTH,
                &resource) != napi_ok
            || napi_create_async_work(env, nullptr, resource, executeSnapshotRestore,
                completeSnapshotRestore, work, &work->work) != napi_ok) {
        delete work;
        return invalid(env, "Unable to create snapshot restore worker");
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_delete_async_work(env, work->work);
        delete work;
        return invalid(env, "Unable to queue snapshot restore worker");
    }
    return promise;
}

TEXT_ENTRY(VoiceHotwordCorrect, msime_client_voice_hotword_correct)
TEXT_ENTRY(VoiceLocalModels, msime_client_voice_local_models)
TEXT_ENTRY(VoiceLocalModelCancel, msime_client_voice_local_model_cancel)

static void rejectWith(napi_env env, napi_deferred deferred, const char *text) {
    napi_value message = nullptr;
    napi_value error = nullptr;
    if (napi_create_string_utf8(env, text, NAPI_AUTO_LENGTH, &message) != napi_ok
            || napi_create_error(env, nullptr, message, &error) != napi_ok) {
        napi_get_undefined(env, &error);
    }
    napi_reject_deferred(env, deferred, error);
}

// Settles a voice promise with the Rust response and frees it; a missing response is the only rejection, every refusal arrives as {"ok":false} like the synchronous calls.
static void settleVoicePromise(napi_env env, napi_status status, napi_deferred deferred, char *result,
                               const char *failure) {
    auto owned = msime::host_api::own_string(result);
    napi_value value = nullptr;
    const bool resolved = status == napi_ok && owned
        && napi_create_string_utf8(env, owned.get(), std::strlen(owned.get()), &value) == napi_ok;
    if (resolved) {
        napi_resolve_deferred(env, deferred, value);
    } else {
        rejectWith(env, deferred, failure);
    }
}

// The hotword read touches the user dictionary store, which the host API refuses on the UI thread, so it runs as async work and answers through a promise.
struct VoiceHotwordsWork {
    napi_async_work work = nullptr;
    napi_deferred deferred = nullptr;
    std::string request;
    char *result = nullptr;
};

static void executeVoiceHotwords(napi_env, void *data) {
    auto *work = static_cast<VoiceHotwordsWork *>(data);
    work->result = msime_client_voice_hotwords(
        reinterpret_cast<const uint8_t *>(work->request.data()), work->request.size());
}

static void completeVoiceHotwords(napi_env env, napi_status status, void *data) {
    auto *work = static_cast<VoiceHotwordsWork *>(data);
    settleVoicePromise(env, status, work->deferred, work->result, "Voice hotword worker failed");
    napi_delete_async_work(env, work->work);
    delete work;
}

static napi_value VoiceHotwords(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    auto *work = new VoiceHotwordsWork();
    if (!arguments(env, info, 1, argv) || !argumentText(env, argv[0], work->request)) {
        delete work;
        return invalid(env, "Expected a voice hotword request");
    }
    napi_value promise = nullptr;
    napi_value resource = nullptr;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok
            || napi_create_string_utf8(env, "MSIME voice hotwords", NAPI_AUTO_LENGTH,
                &resource) != napi_ok
            || napi_create_async_work(env, nullptr, resource, executeVoiceHotwords,
                completeVoiceHotwords, work, &work->work) != napi_ok) {
        delete work;
        return invalid(env, "Unable to create voice hotword worker");
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_delete_async_work(env, work->work);
        delete work;
        return invalid(env, "Unable to queue voice hotword worker");
    }
    return promise;
}

// Registering the anonymous account waits on the network, so it runs as async work and answers through a promise; a refusal or a failed request arrives as {"ok":false}.
struct AnonymousAccountWork {
    napi_async_work work = nullptr;
    napi_deferred deferred = nullptr;
    std::string directory;
    char *result = nullptr;
};

static void executeAnonymousAccount(napi_env, void *data) {
    auto *work = static_cast<AnonymousAccountWork *>(data);
    work->result = msime_client_ensure_anonymous_account(
        reinterpret_cast<const uint8_t *>(work->directory.data()), work->directory.size());
}

static void completeAnonymousAccount(napi_env env, napi_status status, void *data) {
    auto *work = static_cast<AnonymousAccountWork *>(data);
    settleVoicePromise(env, status, work->deferred, work->result, "Anonymous account worker failed");
    napi_delete_async_work(env, work->work);
    delete work;
}

static napi_value EnsureAnonymousAccount(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    auto *work = new AnonymousAccountWork();
    if (!arguments(env, info, 1, argv) || !argumentText(env, argv[0], work->directory)) {
        delete work;
        return invalid(env, "Expected an account directory");
    }
    napi_value promise = nullptr;
    napi_value resource = nullptr;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok
            || napi_create_string_utf8(env, "MSIME anonymous account", NAPI_AUTO_LENGTH,
                &resource) != napi_ok
            || napi_create_async_work(env, nullptr, resource, executeAnonymousAccount,
                completeAnonymousAccount, work, &work->work) != napi_ok) {
        delete work;
        return invalid(env, "Unable to create anonymous account worker");
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_delete_async_work(env, work->work);
        delete work;
        return invalid(env, "Unable to queue anonymous account worker");
    }
    return promise;
}

// 较慢的单文档请求共用一个工作线程：上报、公告和检查更新要等网络，自定义皮肤库要在锁内读写有上限的数 MB 文件。
using RequestCall = char *(*)(const uint8_t *, size_t);

struct RequestWork {
    napi_async_work work = nullptr;
    napi_deferred deferred = nullptr;
    RequestCall call = nullptr;
    std::string request;
    char *result = nullptr;
};

static void executeRequest(napi_env, void *data) {
    auto *work = static_cast<RequestWork *>(data);
    work->result = work->call(
        reinterpret_cast<const uint8_t *>(work->request.data()), work->request.size());
}

static void completeRequest(napi_env env, napi_status status, void *data) {
    auto *work = static_cast<RequestWork *>(data);
    settleVoicePromise(env, status, work->deferred, work->result, "Request worker failed");
    napi_delete_async_work(env, work->work);
    delete work;
}

static napi_value queueRequest(napi_env env, napi_callback_info info, RequestCall call,
                               const char *name) {
    std::vector<napi_value> argv;
    auto *work = new RequestWork();
    work->call = call;
    if (!arguments(env, info, 1, argv) || !argumentText(env, argv[0], work->request)) {
        delete work;
        return invalid(env, "Expected a request document");
    }
    napi_value promise = nullptr;
    napi_value resource = nullptr;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok
            || napi_create_string_utf8(env, name, NAPI_AUTO_LENGTH, &resource) != napi_ok
            || napi_create_async_work(env, nullptr, resource, executeRequest,
                completeRequest, work, &work->work) != napi_ok) {
        delete work;
        return invalid(env, "Unable to create request worker");
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_delete_async_work(env, work->work);
        delete work;
        return invalid(env, "Unable to queue request worker");
    }
    return promise;
}

static napi_value TelemetryFlush(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_telemetry_flush, "MSIME telemetry flush");
}

static napi_value Notices(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_notices, "MSIME notices");
}

// 「关于」页的检查更新：读取 GitHub 发布列表最多要十秒，所以绝不能在 UI 线程上执行。
static napi_value UpdateCheck(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_update_check, "MSIME update check");
}

// A named design can carry a photo, so both reading and mutating the library can parse and
// serialize megabytes while holding its file lock. The settings page awaits this worker.
static napi_value CustomSkinLibrary(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_custom_skin_library, "MSIME custom skin library");
}

// Reply templates are bounded but can still occupy several megabytes across the saved library;
// keep validation, locking and atomic replacement off the ArkTS thread as well.
static napi_value CommunityResourceLibrary(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_community_resource_library,
        "MSIME community resource library");
}

// Wordbook imports are bounded at several megabytes and rewrite the selected book and review
// progress. Keep parsing, file locks and atomic replacement off the ArkTS thread.
static napi_value VocabularyReview(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_vocabulary_review, "MSIME vocabulary review");
}

// 「统计」标签页的概览要读取整份统计文档、推导每个时间窗口，并在键盘进程同样会获取的文件锁下写入新达成的徽章，所以设置应用把它放到 ArkTS 线程之外运行，就像 Android 放在 worker 上运行一样。键盘的记录量小，继续使用同步的 `typingStatistics`。
static napi_value TypingStatisticsAsync(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_typing_statistics, "MSIME typing statistics");
}

// Picked skins are checked against the manifest and copied with a bounded tree budget. Run that
// validation and replacement off the ArkTS thread so a large folder cannot freeze settings.
static napi_value SkinImport(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_skin_import, "MSIME skin import");
}

// Named dictionaries: an import parses up to 16 MiB of text and every operation rewrites the collection files under their lock, so the settings page runs them on a worker and awaits the promise. The keyboard's small flush uses the synchronous `dictionaryCollections`.
static napi_value DictionaryCollectionsAsync(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_dictionary_collections,
        "MSIME dictionary collections");
}

// The settings page's 修复配置文件: copies the unreadable document aside and rewrites it under the preferences writer lock, so it runs on a worker like every other call that may wait on that lock.
static napi_value RepairPreferences(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_repair_preferences, "MSIME repair preferences");
}

// 键盘的常用语存放在一个文件里，设置进程和键盘进程都会在锁下重写它，装上短语包后文档可达数 MB，所以每个操作都在 ArkTS 线程之外运行。
static napi_value CommonPhrases(napi_env env, napi_callback_info info) {
    return queueRequest(env, info, msime_client_common_phrases, "MSIME common phrases");
}

// A pack import extracts or copies up to a music pack's size and validates it before swapping it into place, which the header says belongs on a worker thread, so it runs as async work and answers through a promise. The small catalog, remove and name-list calls stay on the synchronous `plugins` entry.
struct PluginsWork {
    napi_async_work work = nullptr;
    napi_deferred deferred = nullptr;
    std::string request;
    char *result = nullptr;
};

static void executePlugins(napi_env, void *data) {
    auto *work = static_cast<PluginsWork *>(data);
    work->result = msime_client_plugins(
        reinterpret_cast<const uint8_t *>(work->request.data()), work->request.size());
}

static void completePlugins(napi_env env, napi_status status, void *data) {
    auto *work = static_cast<PluginsWork *>(data);
    settleVoicePromise(env, status, work->deferred, work->result, "Plugin worker failed");
    napi_delete_async_work(env, work->work);
    delete work;
}

static napi_value PluginsAsync(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    auto *work = new PluginsWork();
    if (!arguments(env, info, 1, argv) || !argumentText(env, argv[0], work->request)) {
        delete work;
        return invalid(env, "Expected a plugins request");
    }
    napi_value promise = nullptr;
    napi_value resource = nullptr;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok
            || napi_create_string_utf8(env, "MSIME plugins", NAPI_AUTO_LENGTH, &resource) != napi_ok
            || napi_create_async_work(env, nullptr, resource, executePlugins, completePlugins, work,
                &work->work) != napi_ok) {
        delete work;
        return invalid(env, "Unable to create plugin worker");
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_delete_async_work(env, work->work);
        delete work;
        return invalid(env, "Unable to queue plugin worker");
    }
    return promise;
}

// Community skin designs can carry several megabytes of validated image data and are written to
// both the trial and library stores. Keep the import and its file locks off the ArkTS thread.
struct CommunitySkinInstallWork {
    napi_async_work work = nullptr;
    napi_deferred deferred = nullptr;
    std::string request;
    char *result = nullptr;
};

static void executeCommunitySkinInstall(napi_env, void *data) {
    auto *work = static_cast<CommunitySkinInstallWork *>(data);
    work->result = msime_client_community_skin_install(
        reinterpret_cast<const uint8_t *>(work->request.data()), work->request.size());
}

static void completeCommunitySkinInstall(napi_env env, napi_status status, void *data) {
    auto *work = static_cast<CommunitySkinInstallWork *>(data);
    settleVoicePromise(env, status, work->deferred, work->result,
        "Community skin install worker failed");
    napi_delete_async_work(env, work->work);
    delete work;
}

static napi_value CommunitySkinInstall(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    auto *work = new CommunitySkinInstallWork();
    if (!arguments(env, info, 1, argv) || !argumentText(env, argv[0], work->request)) {
        delete work;
        return invalid(env, "Expected a community skin install request");
    }
    napi_value promise = nullptr;
    napi_value resource = nullptr;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok
            || napi_create_string_utf8(env, "MSIME community skin install", NAPI_AUTO_LENGTH,
                &resource) != napi_ok
            || napi_create_async_work(env, nullptr, resource, executeCommunitySkinInstall,
                completeCommunitySkinInstall, work, &work->work) != napi_ok) {
        delete work;
        return invalid(env, "Unable to create community skin install worker");
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_delete_async_work(env, work->work);
        delete work;
        return invalid(env, "Unable to queue community skin install worker");
    }
    return promise;
}

// A model install blocks for the whole download, so it runs as async work; progress is reported on that worker thread and crosses to the JS thread through a thread-safe function holding a copy of each document.
struct VoiceModelInstallWork {
    napi_async_work work = nullptr;
    napi_deferred deferred = nullptr;
    napi_threadsafe_function progress = nullptr;
    std::string request;
    char *result = nullptr;
};

static void callVoiceModelProgress(napi_env env, napi_value callback, void *, void *data) {
    auto *document = static_cast<std::string *>(data);
    if (env != nullptr && callback != nullptr) {
        napi_value argument = nullptr;
        napi_value receiver = nullptr;
        if (napi_create_string_utf8(env, document->data(), document->size(), &argument) == napi_ok
                && napi_get_undefined(env, &receiver) == napi_ok) {
            napi_call_function(env, receiver, callback, 1, &argument, nullptr);
        }
    }
    delete document;
}

static void reportVoiceModelProgress(const uint8_t *json, size_t length, void *context) {
    auto *work = static_cast<VoiceModelInstallWork *>(context);
    if (work->progress == nullptr || json == nullptr) return;
    // The buffer belongs to the caller only for the duration of this call.
    auto *document = new std::string(reinterpret_cast<const char *>(json), length);
    if (napi_call_threadsafe_function(work->progress, document, napi_tsfn_nonblocking) != napi_ok) {
        delete document;
    }
}

static void executeVoiceModelInstall(napi_env, void *data) {
    auto *work = static_cast<VoiceModelInstallWork *>(data);
    work->result = msime_client_voice_local_model_install(
        reinterpret_cast<const uint8_t *>(work->request.data()), work->request.size(),
        work->progress != nullptr ? reportVoiceModelProgress : nullptr, work);
}

static void completeVoiceModelInstall(napi_env env, napi_status status, void *data) {
    auto *work = static_cast<VoiceModelInstallWork *>(data);
    if (work->progress != nullptr) {
        napi_release_threadsafe_function(work->progress, napi_tsfn_release);
        work->progress = nullptr;
    }
    settleVoicePromise(env, status, work->deferred, work->result, "Voice model install worker failed");
    napi_delete_async_work(env, work->work);
    delete work;
}

static napi_value VoiceLocalModelInstall(napi_env env, napi_callback_info info) {
    size_t count = 2;
    napi_value argv[2] = { nullptr, nullptr };
    if (napi_get_cb_info(env, info, &count, argv, nullptr, nullptr) != napi_ok || count < 1) {
        return invalid(env, "Expected a voice model install request");
    }
    auto *work = new VoiceModelInstallWork();
    if (!argumentText(env, argv[0], work->request)) {
        delete work;
        return invalid(env, "Expected a voice model install request");
    }
    napi_value resource = nullptr;
    if (napi_create_string_utf8(env, "MSIME voice model install", NAPI_AUTO_LENGTH, &resource)
            != napi_ok) {
        delete work;
        return invalid(env, "Unable to create voice model install worker");
    }
    napi_valuetype callbackType = napi_undefined;
    if (count >= 2 && napi_typeof(env, argv[1], &callbackType) == napi_ok
            && callbackType == napi_function
            && napi_create_threadsafe_function(env, argv[1], nullptr, resource, 0, 1, nullptr,
                nullptr, nullptr, callVoiceModelProgress, &work->progress) != napi_ok) {
        delete work;
        return invalid(env, "Unable to create voice model progress channel");
    }
    napi_value promise = nullptr;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok
            || napi_create_async_work(env, nullptr, resource, executeVoiceModelInstall,
                completeVoiceModelInstall, work, &work->work) != napi_ok) {
        if (work->progress != nullptr) napi_release_threadsafe_function(work->progress, napi_tsfn_release);
        delete work;
        return invalid(env, "Unable to create voice model install worker");
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_delete_async_work(env, work->work);
        if (work->progress != nullptr) napi_release_threadsafe_function(work->progress, napi_tsfn_release);
        delete work;
        return invalid(env, "Unable to queue voice model install worker");
    }
    return promise;
}

// Removing a model recursively deletes a user-downloaded directory that can be larger than a
// gigabyte. Keep the filesystem walk off the ArkTS thread just like installation; the short
// cancel call remains synchronous so it can set the native cancellation flag immediately.
struct VoiceLocalModelRemoveWork {
    napi_async_work work = nullptr;
    napi_deferred deferred = nullptr;
    std::string request;
    char *result = nullptr;
};

static void executeVoiceLocalModelRemove(napi_env, void *data) {
    auto *work = static_cast<VoiceLocalModelRemoveWork *>(data);
    work->result = msime_client_voice_local_model_remove(
        reinterpret_cast<const uint8_t *>(work->request.data()), work->request.size());
}

static void completeVoiceLocalModelRemove(napi_env env, napi_status status, void *data) {
    auto *work = static_cast<VoiceLocalModelRemoveWork *>(data);
    settleVoicePromise(env, status, work->deferred, work->result,
        "Voice model removal worker failed");
    napi_delete_async_work(env, work->work);
    delete work;
}

static napi_value VoiceLocalModelRemove(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    auto *work = new VoiceLocalModelRemoveWork();
    if (!arguments(env, info, 1, argv) || !argumentText(env, argv[0], work->request)) {
        delete work;
        return invalid(env, "Expected a local voice model removal request");
    }
    napi_value promise = nullptr;
    napi_value resource = nullptr;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok
            || napi_create_string_utf8(env, "MSIME voice model removal", NAPI_AUTO_LENGTH,
                &resource) != napi_ok
            || napi_create_async_work(env, nullptr, resource, executeVoiceLocalModelRemove,
                completeVoiceLocalModelRemove, work, &work->work) != napi_ok) {
        delete work;
        return invalid(env, "Unable to create voice model removal worker");
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_delete_async_work(env, work->work);
        delete work;
        return invalid(env, "Unable to queue voice model removal worker");
    }
    return promise;
}

#define PAIR_ENTRY(name, call)                                                                     \
    static napi_value name(napi_env env, napi_callback_info info) {                                 \
        std::vector<napi_value> argv;                                                               \
        std::string first;                                                                          \
        std::string second;                                                                         \
        if (!arguments(env, info, 2, argv) || !argumentText(env, argv[0], first)                    \
                || !argumentText(env, argv[1], second)) {                                           \
            return response(env, call(nullptr, 0, nullptr, 0));                                     \
        }                                                                                           \
        return response(env,                                                                        \
            call(reinterpret_cast<const uint8_t *>(first.data()), first.size(),                     \
                 reinterpret_cast<const uint8_t *>(second.data()), second.size()));                 \
    }

PAIR_ENTRY(EmojiCatalog, msime_client_emoji_catalog_request)
PAIR_ENTRY(CandidateGlosses, msime_client_candidate_gloss_request)
PAIR_ENTRY(EnglishCompletions, msime_client_english_completions_request)
PAIR_ENTRY(TranslationGlossSave, msime_client_translation_gloss_save)
TEXT_ENTRY(TranslationPlan, msime_client_custom_translation_plan)
TEXT_ENTRY(GoogleLoopback, msime_client_google_loopback)
TEXT_ENTRY(TencentTranslationHttpRequest, msime_client_tencent_translation_http_request)
TEXT_ENTRY(NiuTransTranslationHttpRequest, msime_client_niutrans_translation_http_request)
TEXT_ENTRY(CustomTranslationHttpRequest, msime_client_custom_translation_http_request)
TEXT_ENTRY(ParseNiuTransTranslationResponse, msime_client_parse_niutrans_translation_response)
TEXT_ENTRY(ParseCustomTranslationResponse, msime_client_parse_custom_translation_response)

static napi_value ParseTencentTranslationResponse(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    std::string body;
    size_t expected = 0;
    if (!arguments(env, info, 2, argv) || !argumentText(env, argv[0], body)
            || !argumentIndex(env, argv[1], expected)) {
        return response(env, msime_client_parse_tencent_translation_response(nullptr, 0, 0));
    }
    return response(env, msime_client_parse_tencent_translation_response(
        reinterpret_cast<const uint8_t *>(body.data()), body.size(), expected));
}

static napi_value OnlineQuery(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    if (!arguments(env, info, 1, argv) || !argumentHandle(env, argv[0], handle)) {
        return response(env, msime_client_online_query(0));
    }
    return response(env, msime_client_online_query(handle));
}

static napi_value AiRequestForQuery(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    std::string query;
    if (!arguments(env, info, 2, argv) || !argumentHandle(env, argv[0], handle)
            || !argumentText(env, argv[1], query)) {
        return response(env, msime_client_ai_request_for_query(0, nullptr, 0));
    }
    return response(env, msime_client_ai_request_for_query(
        handle, reinterpret_cast<const uint8_t *>(query.data()), query.size()));
}

static napi_value AiHttpRequest(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    std::string request;
    if (!arguments(env, info, 1, argv) || !argumentText(env, argv[0], request)) {
        return response(env, msime_client_ai_http_request(nullptr, 0));
    }
    return response(env, msime_client_ai_http_request(
        reinterpret_cast<const uint8_t *>(request.data()), request.size()));
}

static napi_value ParseAiResponse(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    std::string body;
    size_t limit = 0;
    if (!arguments(env, info, 2, argv) || !argumentText(env, argv[0], body)
            || !argumentIndex(env, argv[1], limit) || limit > 255) {
        return response(env, msime_client_parse_ai_response(nullptr, 0, 0));
    }
    return response(env, msime_client_parse_ai_response(
        reinterpret_cast<const uint8_t *>(body.data()), body.size(), static_cast<uint8_t>(limit)));
}

static napi_value ApplyCloudResponse(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    std::string query;
    std::string body;
    if (!arguments(env, info, 3, argv) || !argumentHandle(env, argv[0], handle)
            || !argumentText(env, argv[1], query) || !argumentText(env, argv[2], body)) {
        return response(env, msime_client_apply_cloud_response(0, nullptr, 0, nullptr, 0));
    }
    return response(env, msime_client_apply_cloud_response(
        handle, reinterpret_cast<const uint8_t *>(query.data()), query.size(),
        reinterpret_cast<const uint8_t *>(body.data()), body.size()));
}

static napi_value ApplyOnlineCandidates(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    size_t source = 0;
    std::string query;
    std::string candidates;
    if (!arguments(env, info, 4, argv) || !argumentHandle(env, argv[0], handle)
            || !argumentText(env, argv[1], query) || !argumentText(env, argv[2], candidates)
            || !argumentIndex(env, argv[3], source) || source > 1) {
        return response(env, msime_client_apply_online_candidates(
            0, nullptr, 0, nullptr, 0, 2));
    }
    return response(env, msime_client_apply_online_candidates(
        handle, reinterpret_cast<const uint8_t *>(query.data()), query.size(),
        reinterpret_cast<const uint8_t *>(candidates.data()), candidates.size(),
        static_cast<uint8_t>(source)));
}

#define HANDLE_ENTRY(name, call)                                                                   \
    static napi_value name(napi_env env, napi_callback_info info) {                                 \
        std::vector<napi_value> argv;                                                               \
        uint64_t handle = 0;                                                                        \
        if (!arguments(env, info, 1, argv) || !argumentHandle(env, argv[0], handle)) {              \
            return invalid(env, "Session handle must be a non-negative integer");                    \
        }                                                                                           \
        return response(env, call(handle));                                                         \
    }

HANDLE_ENTRY(SnapshotDiscard, msime_client_snapshot_discard)
HANDLE_ENTRY(View, msime_client_view)
HANDLE_ENTRY(ResetCache, msime_client_reset_cache)
HANDLE_ENTRY(AllCandidates, msime_client_all_candidates)
HANDLE_ENTRY(TranslationQuery, msime_client_translation_query)
HANDLE_ENTRY(Destroy, msime_client_destroy)
HANDLE_ENTRY(VoiceStart, msime_client_voice_start)
HANDLE_ENTRY(VoiceCancel, msime_client_voice_cancel)

#define FLAG_ENTRY(name, call)                                                                      \
    static napi_value name(napi_env env, napi_callback_info info) {                                 \
        std::vector<napi_value> argv;                                                               \
        uint64_t handle = 0;                                                                        \
        bool enabled = false;                                                                       \
        if (!arguments(env, info, 2, argv) || !argumentHandle(env, argv[0], handle)                 \
                || !argumentFlag(env, argv[1], enabled)) {                                          \
            return invalid(env, "Expected a session handle and a boolean");                          \
        }                                                                                           \
        return response(env, call(handle, enabled));                                                \
    }

FLAG_ENTRY(Focus, msime_client_focus)
FLAG_ENTRY(SetNineKeyMode, msime_client_set_nine_key_mode)
FLAG_ENTRY(SetEnglishMode, msime_client_set_english_mode)
FLAG_ENTRY(SetCharacterWidth, msime_client_set_character_width)
FLAG_ENTRY(SetPrivateSession, msime_client_set_private_session)

// Candidate identity is the generation plus the index, so a stale page cannot act on a fresh one.
#define CANDIDATE_ENTRY(name, call, message)                                                        \
    static napi_value name(napi_env env, napi_callback_info info) {                                 \
        std::vector<napi_value> argv;                                                               \
        uint64_t handle = 0;                                                                        \
        uint64_t generation = 0;                                                                    \
        size_t index = 0;                                                                           \
        if (!arguments(env, info, 3, argv) || !argumentHandle(env, argv[0], handle)                 \
                || !argumentHandle(env, argv[1], generation)                                        \
                || !argumentIndex(env, argv[2], index)) {                                           \
            return invalid(env, message);                                                           \
        }                                                                                           \
        return response(env, call(handle, generation, index));                                      \
    }

CANDIDATE_ENTRY(Select, msime_client_select, "Invalid candidate index")

static napi_value SelectEdge(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    uint64_t generation = 0;
    size_t index = 0;
    int32_t edge = 0;
    if (!arguments(env, info, 4, argv) || !argumentHandle(env, argv[0], handle)
            || !argumentHandle(env, argv[1], generation) || !argumentIndex(env, argv[2], index)
            || napi_get_value_int32(env, argv[3], &edge) != napi_ok) {
        return invalid(env, "Invalid candidate edge");
    }
    if (edge < 0 || edge > 1) return invalid(env, "Invalid candidate edge");
    return response(env, msime_client_select_edge(
        handle, generation, index, static_cast<uint8_t>(edge)));
}

CANDIDATE_ENTRY(SelectAnyCandidate, msime_client_select_any_candidate, "Invalid candidate index")
CANDIDATE_ENTRY(PinCandidate, msime_client_pin_candidate, "Invalid candidate index")
CANDIDATE_ENTRY(ClearCandidatePosition, msime_client_clear_candidate_position,
                "Invalid candidate index")
CANDIDATE_ENTRY(RemoveCandidate, msime_client_remove_candidate, "Invalid candidate index")
CANDIDATE_ENTRY(ChooseNineKeySpelling, msime_client_choose_nine_key_spelling,
                "Invalid nine-key spelling index")

// 九宫格候选筛选。笔画前缀以 ASCII 字符串传入，长度和字节是否都是 hspnz 由 C ABI 校验，这里只转发。
static napi_value SetNineKeyFilter(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    bool single_character = false;
    std::string strokes;
    if (!arguments(env, info, 3, argv) || !argumentHandle(env, argv[0], handle)
            || !argumentFlag(env, argv[1], single_character)
            || !argumentText(env, argv[2], strokes)) {
        return invalid(env, "Expected a session handle, a boolean and a stroke string");
    }
    return response(env, msime_client_set_nine_key_filter(handle, single_character,
        reinterpret_cast<const uint8_t *>(strokes.data()), strokes.size()));
}

static napi_value SavePreferences(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    std::string directory;
    std::string snapshot;
    uint64_t revision = 0;
    if (!arguments(env, info, 3, argv) || !argumentText(env, argv[0], directory)
            || !argumentHandle(env, argv[1], revision)
            || !argumentText(env, argv[2], snapshot)) {
        return response(env, msime_client_save_preferences(nullptr, 0, 0, nullptr, 0));
    }
    return response(env, msime_client_save_preferences(
        reinterpret_cast<const uint8_t *>(directory.data()), directory.size(), revision,
        reinterpret_cast<const uint8_t *>(snapshot.data()), snapshot.size()));
}

static napi_value SnapshotPrepare(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    std::string request;
    std::string file;
    if (!arguments(env, info, 2, argv) || !argumentText(env, argv[0], request)
            || !argumentText(env, argv[1], file)) {
        return response(env, msime_client_snapshot_prepare(nullptr, 0, nullptr, nullptr));
    }
    SnapshotReader reader(file);
    return response(env, msime_client_snapshot_prepare(
        reinterpret_cast<const uint8_t *>(request.data()), request.size(), snapshotNext, &reader));
}

static napi_value SnapshotActivate(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    std::string expected;
    if (!arguments(env, info, 2, argv) || !argumentHandle(env, argv[0], handle)
            || !argumentText(env, argv[1], expected)) {
        return response(env, msime_client_snapshot_activate(0, nullptr, 0));
    }
    return response(env, msime_client_snapshot_activate(
        handle, reinterpret_cast<const uint8_t *>(expected.data()), expected.size()));
}

static napi_value UpdatePreferences(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    std::string snapshot;
    if (!arguments(env, info, 2, argv) || !argumentHandle(env, argv[0], handle)) {
        return invalid(env, "Session handle must be a non-negative integer");
    }
    if (!argumentText(env, argv[1], snapshot)) {
        return response(env, msime_client_update_preferences(handle, nullptr, 0));
    }
    return response(env, msime_client_update_preferences(
        handle, reinterpret_cast<const uint8_t *>(snapshot.data()), snapshot.size()));
}

static napi_value Character(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    int32_t ascii = 0;
    bool shift = false;
    if (!arguments(env, info, 3, argv) || !argumentHandle(env, argv[0], handle)
            || napi_get_value_int32(env, argv[1], &ascii) != napi_ok
            || !argumentFlag(env, argv[2], shift)) {
        return invalid(env, "Expected a session handle, an ASCII code and a boolean");
    }
    if (ascii < 0 || ascii > 127) return invalid(env, "Engine character must be ASCII");
    return response(env,
        msime_client_character(handle, static_cast<uint8_t>(ascii), shift));
}

// One glide stroke (滑行输入) as the JSON request msime_client_glide reads. A request that is not a string, or is too long to copy, reaches the Rust side as an empty buffer and is refused there with the same structured error as a malformed one.
static napi_value Glide(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    std::string request;
    if (!arguments(env, info, 2, argv) || !argumentHandle(env, argv[0], handle)) {
        return invalid(env, "Session handle must be a non-negative integer");
    }
    if (!argumentText(env, argv[1], request)) {
        return response(env, msime_client_glide(handle, nullptr, 0));
    }
    return response(env, msime_client_glide(
        handle, reinterpret_cast<const uint8_t *>(request.data()), request.size()));
}

static napi_value PunctuationWithContext(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    int32_t ascii = 0;
    int32_t preceding = 0;
    if (!arguments(env, info, 3, argv) || !argumentHandle(env, argv[0], handle)
            || napi_get_value_int32(env, argv[1], &ascii) != napi_ok
            || napi_get_value_int32(env, argv[2], &preceding) != napi_ok) {
        return invalid(env, "Expected a session handle, an ASCII code and a preceding code point");
    }
    if (ascii < 0 || ascii > 127 || preceding < 0) {
        return invalid(env, "Invalid punctuation context");
    }
    return response(env, msime_client_punctuation_with_context(
        handle, static_cast<uint8_t>(ascii), static_cast<uint32_t>(preceding)));
}

static napi_value BalancePairedPunctuationAfterAutoClose(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    int32_t opening = 0;
    if (!arguments(env, info, 2, argv) || !argumentHandle(env, argv[0], handle)
            || napi_get_value_int32(env, argv[1], &opening) != napi_ok) {
        return invalid(env, "Expected a session handle and an ASCII opening mark");
    }
    if (opening < 0 || opening > 127) return invalid(env, "Invalid punctuation opening");
    return response(env, msime_client_balance_paired_punctuation_after_auto_close(
        handle, static_cast<uint8_t>(opening)));
}

static napi_value Command(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    uint32_t command = 0;
    if (!arguments(env, info, 2, argv) || !argumentHandle(env, argv[0], handle)
            || napi_get_value_uint32(env, argv[1], &command) != napi_ok) {
        return invalid(env, "Expected a session handle and a command code");
    }
    return response(env, msime_client_command(handle, command));
}

// The typing effect of one key or commit: the packed integer msime_client.h documents. Called on the key path, so it allocates nothing beyond the returned number.
static napi_value TypingEffect(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    uint32_t event = 0;
    if (!arguments(env, info, 2, argv) || !argumentHandle(env, argv[0], handle)
            || napi_get_value_uint32(env, argv[1], &event) != napi_ok) {
        return invalid(env, "Expected a session handle and a typing effect event");
    }
    napi_value output = nullptr;
    if (napi_create_uint32(env, msime_client_typing_effect(handle, event), &output) != napi_ok) return nullptr;
    return output;
}

// The session's resolved typing effect as JSON: read after the preferences change or a field gains focus, never per key.
static napi_value TypingEffectSettings(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    if (!arguments(env, info, 1, argv) || !argumentHandle(env, argv[0], handle)) {
        return invalid(env, "Expected a session handle");
    }
    return response(env, msime_client_typing_effect_settings(handle));
}

static napi_value FixCandidatePosition(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    uint64_t generation = 0;
    size_t index = 0;
    int32_t position = 0;
    if (!arguments(env, info, 4, argv) || !argumentHandle(env, argv[0], handle)
            || !argumentHandle(env, argv[1], generation) || !argumentIndex(env, argv[2], index)
            || napi_get_value_int32(env, argv[3], &position) != napi_ok) {
        return invalid(env, "Invalid candidate index");
    }
    if (position < 1 || position > 5) {
        return invalid(env, "Candidate position must be between 1 and 5");
    }
    return response(env, msime_client_fix_candidate_position(
        handle, generation, index, static_cast<uint8_t>(position)));
}

static napi_value ApplyTranslations(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    uint64_t generation = 0;
    std::string translations;
    if (!arguments(env, info, 3, argv) || !argumentHandle(env, argv[0], handle)
            || !argumentHandle(env, argv[1], generation)
            || !argumentText(env, argv[2], translations)) {
        return response(env, msime_client_apply_translations(0, 0, nullptr, 0));
    }
    return response(env, msime_client_apply_translations(handle, generation,
        reinterpret_cast<const uint8_t *>(translations.data()), translations.size()));
}

static napi_value VoiceApply(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    uint64_t handle = 0;
    uint64_t generation = 0;
    std::string text;
    if (!arguments(env, info, 3, argv) || !argumentHandle(env, argv[0], handle)
            || !argumentHandle(env, argv[1], generation) || !argumentText(env, argv[2], text)) {
        return response(env, msime_client_voice_apply(0, 0, nullptr, 0));
    }
    return response(env, msime_client_voice_apply(handle, generation,
        reinterpret_cast<const uint8_t *>(text.data()), text.size()));
}

// The statistics master switch as a number, not a JSON reply: the keyboard asks on every focus and needs only the one bit. A missing or non-text argument reads as an invalid directory, -1.
static napi_value TypingStatisticsEnabled(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    std::string directory;
    int32_t enabled = -1;
    if (arguments(env, info, 1, argv) && argumentText(env, argv[0], directory)) {
        enabled = msime_client_typing_statistics_enabled(
            reinterpret_cast<const uint8_t *>(directory.data()), directory.size());
    }
    napi_value output = nullptr;
    if (napi_create_int32(env, enabled, &output) != napi_ok) return nullptr;
    return output;
}

static napi_value AbiVersion(napi_env env, napi_callback_info) {
    napi_value output = nullptr;
    if (napi_create_uint32(env, msime_client_abi_version(), &output) != napi_ok) return nullptr;
    return output;
}

// The global theme picker takes no argument: the ids, titles and palettes all come from the shared catalog.
static napi_value ThemeCatalog(napi_env env, napi_callback_info) {
    return response(env, msime_client_theme_catalog());
}

// 应用主题选择器同样不接受参数：五个 id、标题、季节和两套配色都来自共享目录，其中「四季」画成「秋杉」，不随时间变化。
static napi_value AppThemeCatalog(napi_env env, napi_callback_info) {
    return response(env, msime_client_app_theme_catalog());
}

static napi_value HostCapabilities(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    std::string platform;
    if (!arguments(env, info, 1, argv) || !argumentText(env, argv[0], platform)) {
        return response(env, msime_client_host_capabilities(nullptr, 0));
    }
    return response(env, msime_client_host_capabilities(
        reinterpret_cast<const uint8_t *>(platform.data()), platform.size()));
}

// The shared OpenCC s2t conversion every other host uses. Unlike the JSON entry points this one answers with the converted text itself, and with null when the C ABI refuses the input, so the caller keeps its own text.
static napi_value SimplifiedToTraditional(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    std::string text;
    char *converted = nullptr;
    if (arguments(env, info, 1, argv) && argumentText(env, argv[0], text)) {
        converted = msime_client_simplified_to_traditional(
            reinterpret_cast<const uint8_t *>(text.data()), text.size());
    }
    if (!converted) {
        napi_value none = nullptr;
        napi_get_null(env, &none);
        return none;
    }
    return response(env, converted);
}

static napi_value DoubaoEncodeFrame(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    int32_t message_type = 0;
    int32_t flags = 0;
    int32_t sequence = 0;
    std::vector<uint8_t> payload;
    constexpr size_t kMaxFramePayload = 1024 * 1024;
    if (!arguments(env, info, 4, argv) || !argumentInt32(env, argv[0], message_type)
            || !argumentInt32(env, argv[1], flags) || !argumentInt32(env, argv[2], sequence)
            || !argumentArrayBuffer(env, argv[3], payload, kMaxFramePayload)
            || message_type < 0 || message_type > 15 || flags < 0 || flags > 15
            || payload.size() > kMaxFramePayload) {
        return invalid(env, "Invalid Doubao frame");
    }
    std::vector<uint8_t> compressed;
    if (!gzipCompress(payload, compressed)
            || compressed.size() > static_cast<size_t>(std::numeric_limits<int32_t>::max())) {
        return invalid(env, "Unable to encode Doubao frame");
    }
    std::vector<uint8_t> frame(12 + compressed.size());
    frame[0] = 0x11;
    frame[1] = static_cast<uint8_t>((message_type << 4) | flags);
    frame[2] = 0x11;
    frame[3] = 0;
    const uint32_t sequence_bits = static_cast<uint32_t>(sequence);
    const uint32_t compressed_size = static_cast<uint32_t>(compressed.size());
    for (size_t index = 0; index < 4; ++index) {
        frame[4 + index] = static_cast<uint8_t>(sequence_bits >> (24 - index * 8));
        frame[8 + index] = static_cast<uint8_t>(compressed_size >> (24 - index * 8));
    }
    std::memcpy(frame.data() + 12, compressed.data(), compressed.size());
    napi_value output = nullptr;
    void *data = nullptr;
    if (napi_create_arraybuffer(env, frame.size(), &data, &output) != napi_ok || !data) return nullptr;
    std::memcpy(data, frame.data(), frame.size());
    return output;
}

// ArkTS sees a NULL napi_value as undefined; the d.ts promises null on refusal.
static napi_value nullValue(napi_env env) {
    napi_value none = nullptr;
    napi_get_null(env, &none);
    return none;
}

static napi_value DoubaoDecodeFrame(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    std::vector<uint8_t> frame;
    constexpr size_t kMaxFrameBytes = 1024 * 1024;
    if (!arguments(env, info, 1, argv) || !argumentArrayBuffer(env, argv[0], frame, kMaxFrameBytes)
            || frame.size() < 12 || frame.size() > 1024 * 1024 || frame[0] != 0x11
            || (frame[1] >> 4) != 0x09 || frame[2] != 0x11) return nullValue(env);
    const uint8_t flags = frame[1] & 0x0f;
    size_t offset = 4;
    if ((flags & 0x01) != 0) offset += 4;
    if ((flags & 0x04) != 0) offset += 4;
    if (offset + 4 > frame.size()) return nullValue(env);
    uint32_t compressed_size = 0;
    for (size_t index = 0; index < 4; ++index) {
        compressed_size = (compressed_size << 8) | frame[offset + index];
    }
    offset += 4;
    if (compressed_size != frame.size() - offset) return nullValue(env);
    std::vector<uint8_t> payload;
    if (!gzipDecompress(frame.data() + offset, compressed_size, payload)) return nullValue(env);
    std::string text(reinterpret_cast<const char *>(payload.data()), payload.size());
    napi_value result = nullptr;
    napi_value last = nullptr;
    napi_value body = nullptr;
    if (napi_create_object(env, &result) != napi_ok
            || napi_get_boolean(env, (flags & 0x02) != 0, &last) != napi_ok
            || napi_create_string_utf8(env, text.data(), text.size(), &body) != napi_ok
            || napi_set_named_property(env, result, "last", last) != napi_ok
            || napi_set_named_property(env, result, "payload", body) != napi_ok) return nullValue(env);
    return result;
}

// Decoding and pitching a pack's WAV samples reads files and runs a sample-rate converter, so it runs as async work and answers through a promise: the paths of the notes it wrote, or a rejection naming what was wrong with the sample.
struct KeySoundRenderWork {
    napi_async_work work = nullptr;
    napi_deferred deferred = nullptr;
    std::string sample;
    std::vector<int32_t> semitones;
    std::string directory;
    uint32_t max_millis = 0;
    KeySoundRender result;
};

static void executeKeySoundRender(napi_env, void *data) {
    auto *work = static_cast<KeySoundRenderWork *>(data);
    work->result = renderKeySoundNotes(work->sample, work->semitones, work->directory,
        work->max_millis);
}

static void completeKeySoundRender(napi_env env, napi_status status, void *data) {
    auto *work = static_cast<KeySoundRenderWork *>(data);
    napi_value files = nullptr;
    bool resolved = status == napi_ok && work->result.ok
        && napi_create_array_with_length(env, work->result.files.size(), &files) == napi_ok;
    for (size_t index = 0; resolved && index < work->result.files.size(); ++index) {
        napi_value file = nullptr;
        const std::string &path = work->result.files[index];
        resolved = napi_create_string_utf8(env, path.data(), path.size(), &file) == napi_ok
            && napi_set_element(env, files, static_cast<uint32_t>(index), file) == napi_ok;
    }
    if (resolved) {
        napi_resolve_deferred(env, work->deferred, files);
    } else {
        rejectWith(env, work->deferred, work->result.error.empty()
            ? "Key sound render worker failed" : work->result.error.c_str());
    }
    napi_delete_async_work(env, work->work);
    delete work;
}

static napi_value KeySoundRenderNotes(napi_env env, napi_callback_info info) {
    std::vector<napi_value> argv;
    auto *work = new KeySoundRenderWork();
    uint32_t count = 0;
    bool is_array = false;
    if (!arguments(env, info, 4, argv) || !argumentText(env, argv[0], work->sample)
            || napi_is_array(env, argv[1], &is_array) != napi_ok || !is_array
            || napi_get_array_length(env, argv[1], &count) != napi_ok
            || count == 0 || count > kKeySoundMaxNotes
            || !argumentText(env, argv[2], work->directory)
            || napi_get_value_uint32(env, argv[3], &work->max_millis) != napi_ok) {
        delete work;
        return invalid(env, "Expected a sample path, semitones, a directory and a length bound");
    }
    for (uint32_t index = 0; index < count; ++index) {
        napi_value element = nullptr;
        int32_t semitone = 0;
        if (napi_get_element(env, argv[1], index, &element) != napi_ok
                || !argumentInt32(env, element, semitone)) {
            delete work;
            return invalid(env, "Semitones must be integers");
        }
        work->semitones.push_back(semitone);
    }
    napi_value promise = nullptr;
    napi_value resource = nullptr;
    if (napi_create_promise(env, &work->deferred, &promise) != napi_ok
            || napi_create_string_utf8(env, "MSIME key sound render", NAPI_AUTO_LENGTH,
                &resource) != napi_ok
            || napi_create_async_work(env, nullptr, resource, executeKeySoundRender,
                completeKeySoundRender, work, &work->work) != napi_ok) {
        delete work;
        return invalid(env, "Unable to create key sound render worker");
    }
    if (napi_queue_async_work(env, work->work) != napi_ok) {
        napi_delete_async_work(env, work->work);
        delete work;
        return invalid(env, "Unable to queue key sound render worker");
    }
    return promise;
}

#define ENTRY(exported, function)                                                                  \
    { exported, nullptr, function, nullptr, nullptr, nullptr, napi_default, nullptr }

static napi_value Init(napi_env env, napi_value exports) {
    napi_property_descriptor properties[] = {
        ENTRY("abiVersion", AbiVersion),
        ENTRY("hostCapabilities", HostCapabilities),
        ENTRY("simplifiedToTraditional", SimplifiedToTraditional),
        ENTRY("loadPreferences", LoadPreferences),
        ENTRY("skinCatalog", SkinCatalog),
        ENTRY("themeCatalog", ThemeCatalog),
        ENTRY("resolveTheme", ResolveTheme),
        ENTRY("appThemeCatalog", AppThemeCatalog),
        ENTRY("resolveAppTheme", ResolveAppTheme),
        ENTRY("dictionaryManifest", DictionaryManifest),
        ENTRY("skinResource", SkinResource),
        ENTRY("skinToolbarStylesheet", SkinToolbarStylesheet),
        ENTRY("skinImport", SkinImport),
        ENTRY("customSkinLibrary", CustomSkinLibrary),
        ENTRY("communitySkinInstall", CommunitySkinInstall),
        ENTRY("keyboardSkinTrial", KeyboardSkinTrial),
        ENTRY("communityResourceLibrary", CommunityResourceLibrary),
        ENTRY("aiSkinPlan", AiSkinPlan),
        ENTRY("dictionary", Dictionary),
        ENTRY("savePreferences", SavePreferences),
        ENTRY("updatePreferences", UpdatePreferences),
        ENTRY("typingStatistics", TypingStatistics),
        ENTRY("typingStatisticsEnabled", TypingStatisticsEnabled),
        ENTRY("typingStatisticsAsync", TypingStatisticsAsync),
        ENTRY("vocabularyReview", VocabularyReview),
        ENTRY("mobileClipboardHistory", MobileClipboardHistory),
        ENTRY("emojiCatalog", EmojiCatalog),
        ENTRY("candidateGlosses", CandidateGlosses),
        ENTRY("englishCompletions", EnglishCompletions),
        ENTRY("resetCache", ResetCache),
        ENTRY("translationGlossSave", TranslationGlossSave),
        ENTRY("translationPlan", TranslationPlan),
        ENTRY("googleLoopback", GoogleLoopback),
        ENTRY("tencentTranslationHttpRequest", TencentTranslationHttpRequest),
        ENTRY("niuTransTranslationHttpRequest", NiuTransTranslationHttpRequest),
        ENTRY("customTranslationHttpRequest", CustomTranslationHttpRequest),
        ENTRY("parseTencentTranslationResponse", ParseTencentTranslationResponse),
        ENTRY("parseNiuTransTranslationResponse", ParseNiuTransTranslationResponse),
        ENTRY("parseCustomTranslationResponse", ParseCustomTranslationResponse),
        ENTRY("onlineQuery", OnlineQuery),
        ENTRY("cloudRequestUrl", CloudRequestUrl),
        ENTRY("keySoundPack", KeySoundPack),
        ENTRY("musicPack", MusicPack),
        ENTRY("plugins", Plugins),
        ENTRY("pluginsAsync", PluginsAsync),
        ENTRY("commonPhrases", CommonPhrases),
        ENTRY("repairPreferences", RepairPreferences),
        ENTRY("ensureAnonymousAccount", EnsureAnonymousAccount),
        ENTRY("telemetryBegin", TelemetryBegin),
        ENTRY("telemetryEnd", TelemetryEnd),
        ENTRY("telemetryRecordCrash", TelemetryRecordCrash),
        ENTRY("telemetryFlush", TelemetryFlush),
        ENTRY("telemetryClear", TelemetryClear),
        ENTRY("notices", Notices),
        ENTRY("updateCheck", UpdateCheck),
        ENTRY("noticeDismiss", NoticeDismiss),
        ENTRY("keySoundRenderNotes", KeySoundRenderNotes),
        ENTRY("aiRequestForQuery", AiRequestForQuery),
        ENTRY("aiHttpRequest", AiHttpRequest),
        ENTRY("parseAiResponse", ParseAiResponse),
        ENTRY("applyCloudResponse", ApplyCloudResponse),
        ENTRY("applyOnlineCandidates", ApplyOnlineCandidates),
        ENTRY("personalDictionarySync", PersonalDictionarySync),
        ENTRY("personalDictionaryRequest", PersonalDictionaryRequest),
        ENTRY("dictionaryCollections", DictionaryCollections),
        ENTRY("dictionaryCollectionsAsync", DictionaryCollectionsAsync),
        ENTRY("prepareHost", PrepareHost),
        ENTRY("snapshotVersion", SnapshotVersion),
        ENTRY("snapshotInspect", SnapshotInspect),
        ENTRY("snapshotQueue", SnapshotQueue),
        ENTRY("snapshotRestore", SnapshotRestore),
        ENTRY("snapshotPrepare", SnapshotPrepare),
        ENTRY("snapshotDiscard", SnapshotDiscard),
        ENTRY("snapshotActivate", SnapshotActivate),
        ENTRY("create", Create),
        ENTRY("destroy", Destroy),
        ENTRY("focus", Focus),
        ENTRY("setNineKeyMode", SetNineKeyMode),
        ENTRY("setEnglishMode", SetEnglishMode),
        ENTRY("setCharacterWidth", SetCharacterWidth),
        ENTRY("setPrivateSession", SetPrivateSession),
        ENTRY("character", Character),
        ENTRY("glide", Glide),
        ENTRY("punctuationWithContext", PunctuationWithContext),
        ENTRY("balancePairedPunctuationAfterAutoClose", BalancePairedPunctuationAfterAutoClose),
        ENTRY("command", Command),
        ENTRY("typingEffect", TypingEffect),
        ENTRY("typingEffectSettings", TypingEffectSettings),
        ENTRY("select", Select),
        ENTRY("selectEdge", SelectEdge),
        ENTRY("selectAnyCandidate", SelectAnyCandidate),
        ENTRY("pinCandidate", PinCandidate),
        ENTRY("fixCandidatePosition", FixCandidatePosition),
        ENTRY("clearCandidatePosition", ClearCandidatePosition),
        ENTRY("removeCandidate", RemoveCandidate),
        ENTRY("chooseNineKeySpelling", ChooseNineKeySpelling),
        ENTRY("setNineKeyFilter", SetNineKeyFilter),
        ENTRY("view", View),
        ENTRY("allCandidates", AllCandidates),
        ENTRY("translationQuery", TranslationQuery),
        ENTRY("applyTranslations", ApplyTranslations),
        ENTRY("voiceStart", VoiceStart),
        ENTRY("voiceCancel", VoiceCancel),
        ENTRY("voiceApply", VoiceApply),
        ENTRY("voiceHotwords", VoiceHotwords),
        ENTRY("voiceHotwordCorrect", VoiceHotwordCorrect),
        ENTRY("voiceLocalModels", VoiceLocalModels),
        ENTRY("voiceLocalModelInstall", VoiceLocalModelInstall),
        ENTRY("voiceLocalModelCancel", VoiceLocalModelCancel),
        ENTRY("voiceLocalModelRemove", VoiceLocalModelRemove),
        ENTRY("doubaoEncodeFrame", DoubaoEncodeFrame),
        ENTRY("doubaoDecodeFrame", DoubaoDecodeFrame),
    };
    if (napi_define_properties(env, exports,
            sizeof(properties) / sizeof(properties[0]), properties) != napi_ok) {
        return nullptr;
    }
    return exports;
}

static napi_module client_module = {
    .nm_version = 1,
    .nm_flags = 0,
    .nm_filename = nullptr,
    .nm_register_func = Init,
    .nm_modname = "msimeclient",
    .nm_priv = nullptr,
    .reserved = { nullptr },
};

extern "C" __attribute__((constructor)) void RegisterClientModule(void) {
    napi_module_register(&client_module);
}
