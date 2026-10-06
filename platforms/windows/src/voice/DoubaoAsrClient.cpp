#include "DoubaoAsrClient.h"
#include "DoubaoTranscript.h"
#include "../../../../shared/voice/DoubaoAuth.h"
#include "msime_client.h"

#include <nlohmann/json.hpp>
#include <windows.h>
#include <winhttp.h>

#include <algorithm>
#include <array>
#include <chrono>
#include <memory>
#include <utility>

namespace
{
constexpr std::size_t kPcmChunkBytes = 6400; // 200 ms, 16 kHz, signed 16-bit mono.
constexpr std::size_t kMaximumQueuedBytes = 16000 * 2 * 10; // 10 seconds of PCM.
// Match the shared Doubao frame decoder's limits. A remote WebSocket may send
// arbitrarily many fragments or a tiny gzip stream that expands far beyond a
// transcript; neither may exhaust the Server process.
constexpr std::size_t kMaximumResponseBytes = 1024 * 1024;

struct WinHttpHandle
{
    HINTERNET value = nullptr;
    ~WinHttpHandle()
    {
        if (value)
            WinHttpCloseHandle(value);
    }
    WinHttpHandle() = default;
    explicit WinHttpHandle(HINTERNET handle) : value(handle)
    {
    }
    WinHttpHandle(const WinHttpHandle &) = delete;
    WinHttpHandle &operator=(const WinHttpHandle &) = delete;
};

std::wstring Utf8ToWide(const std::string &value)
{
    if (value.empty())
        return {};
    const int size = MultiByteToWideChar(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), nullptr, 0);
    std::wstring result(static_cast<std::size_t>(size), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), result.data(), size);
    return result;
}

// Doubao v1 frames are built and decoded by the shared host library (crates/client-core/src/voice/doubao_frame.rs), the codec every other host uses. Each builder is called once with no buffer to learn the frame's size and once more to write it.
template <typename Build> std::vector<std::uint8_t> BuildFrame(Build build)
{
    std::size_t length = 0;
    if (build(nullptr, 0, &length) || length == 0)
        return {};
    std::vector<std::uint8_t> frame(length);
    if (!build(frame.data(), frame.size(), &length) || length != frame.size())
        return {};
    return frame;
}

std::vector<std::uint8_t> StartFrame(bool enable_itn, bool enable_punc, bool enable_ddc, const std::string &boosting_table_id)
{
    return BuildFrame([&](std::uint8_t *output, std::size_t capacity, std::size_t *length) {
        return msime_client_doubao_start_frame(enable_itn, enable_punc, enable_ddc,
                                               reinterpret_cast<const std::uint8_t *>(boosting_table_id.data()),
                                               boosting_table_id.size(), output, capacity, length);
    });
}

// The final frame negates the sequence and sets the last-packet flag.
std::vector<std::uint8_t> AudioFrame(std::int32_t sequence, const std::uint8_t *pcm, std::size_t size, bool final_chunk)
{
    return BuildFrame([&](std::uint8_t *output, std::size_t capacity, std::size_t *length) {
        return msime_client_doubao_audio_frame(sequence, pcm, size, final_chunk, output, capacity, length);
    });
}

bool SendBinary(HINTERNET websocket, const std::vector<std::uint8_t> &packet)
{
    return !packet.empty() && WinHttpWebSocketSend(websocket, WINHTTP_WEB_SOCKET_BINARY_MESSAGE_BUFFER_TYPE,
                                                   const_cast<std::uint8_t *>(packet.data()),
                                                   static_cast<DWORD>(packet.size())) == NO_ERROR;
}

struct ParsedResponse
{
    bool last = false;
    int code = 0;
    std::string text;
};

// A message the decoder refuses (another message type, a truncated or oversized frame, a payload that is not gzip JSON) carries no transcript and does not end the exchange.
ParsedResponse ParseResponse(const std::vector<std::uint8_t> &message)
{
    ParsedResponse response;
    if (message.empty())
        return response;
    const std::unique_ptr<char, decltype(&msime_client_string_free)> decoded(
        msime_client_doubao_decode_frame(message.data(), message.size()), msime_client_string_free);
    if (!decoded)
        return response;
    try
    {
        const auto reply = nlohmann::json::parse(decoded.get());
        if (!reply.value("ok", false))
            return response;
        const auto &value = reply.at("value");
        if (value.contains("error_code"))
        {
            response.code = value.at("error_code").get<int>();
            return response;
        }
        response.last = value.at("last").get<bool>();
        response.text = msime::windows::doubao_transcript(nlohmann::json::parse(value.at("payload").get<std::string>()));
    }
    catch (...)
    {
    }
    return response;
}

bool ReceiveMessage(HINTERNET websocket, std::vector<std::uint8_t> &message)
{
    message.clear();
    std::array<std::uint8_t, 8192> buffer{};
    message.reserve(buffer.size());
    for (;;)
    {
        DWORD bytes_read = 0;
        WINHTTP_WEB_SOCKET_BUFFER_TYPE type = WINHTTP_WEB_SOCKET_BINARY_FRAGMENT_BUFFER_TYPE;
        const DWORD error =
            WinHttpWebSocketReceive(websocket, buffer.data(), static_cast<DWORD>(buffer.size()), &bytes_read, &type);
        if (error != NO_ERROR)
            return false;
        if (bytes_read > kMaximumResponseBytes - message.size())
            return false;
        message.insert(message.end(), buffer.begin(), buffer.begin() + static_cast<std::ptrdiff_t>(bytes_read));
        if (type == WINHTTP_WEB_SOCKET_BINARY_MESSAGE_BUFFER_TYPE)
            return true;
        if (type == WINHTTP_WEB_SOCKET_CLOSE_BUFFER_TYPE)
            return false;
        if (type != WINHTTP_WEB_SOCKET_BINARY_FRAGMENT_BUFFER_TYPE)
            return false;
    }
}

HINTERNET ConnectWebSocket(const std::string &endpoint, const std::string &auth_mode, const std::string &app_key, const std::string &access_key,
                           const std::string &resource_id, WinHttpHandle &session, WinHttpHandle &connection)
{
    const auto auth = msime::voice::doubao_auth_headers(auth_mode, app_key, access_key, resource_id);
    if (!auth)
        return nullptr;
    std::string crackable_endpoint = endpoint;
    if (crackable_endpoint.rfind("wss://", 0) == 0)
        crackable_endpoint.replace(0, 6, "https://");
    const std::wstring url = Utf8ToWide(crackable_endpoint);
    URL_COMPONENTS components{};
    components.dwStructSize = sizeof(components);
    components.dwHostNameLength = static_cast<DWORD>(-1);
    components.dwUrlPathLength = static_cast<DWORD>(-1);
    components.dwExtraInfoLength = static_cast<DWORD>(-1);
    if (!WinHttpCrackUrl(url.c_str(), static_cast<DWORD>(url.size()), 0, &components))
        return nullptr;
    const std::wstring host(components.lpszHostName, components.dwHostNameLength);
    std::wstring path(components.lpszUrlPath, components.dwUrlPathLength);
    if (components.dwExtraInfoLength)
        path.append(components.lpszExtraInfo, components.dwExtraInfoLength);
    session.value = WinHttpOpen(L"MetasequoiaImeServer/1.0", WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_NO_PROXY_NAME,
                                WINHTTP_NO_PROXY_BYPASS, 0);
    if (!session.value)
        return nullptr;
    WinHttpSetTimeouts(session.value, 10000, 10000, 10000, 30000);
    connection.value = WinHttpConnect(session.value, host.c_str(), components.nPort, 0);
    if (!connection.value)
        return nullptr;
    WinHttpHandle request(WinHttpOpenRequest(connection.value, L"GET", path.c_str(), nullptr, WINHTTP_NO_REFERER,
                                             WINHTTP_DEFAULT_ACCEPT_TYPES, WINHTTP_FLAG_SECURE));
    if (!request.value)
        return nullptr;
    // The handshake carries provider credentials in its headers. Never replay them after a
    // redirect to a different endpoint or protocol.
    DWORD redirect_policy = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
    if (!WinHttpSetOption(request.value, WINHTTP_OPTION_REDIRECT_POLICY,
                          &redirect_policy, sizeof(redirect_policy)))
        return nullptr;
    if (!WinHttpSetOption(request.value, WINHTTP_OPTION_UPGRADE_TO_WEB_SOCKET, nullptr, 0))
        return nullptr;
    const std::wstring headers = Utf8ToWide(*auth);
    if (!WinHttpAddRequestHeaders(request.value, headers.c_str(), static_cast<DWORD>(-1),
                                  WINHTTP_ADDREQ_FLAG_ADD | WINHTTP_ADDREQ_FLAG_REPLACE) ||
        !WinHttpSendRequest(request.value, WINHTTP_NO_ADDITIONAL_HEADERS, 0, WINHTTP_NO_REQUEST_DATA, 0, 0, 0) ||
        !WinHttpReceiveResponse(request.value, nullptr))
        return nullptr;
    return WinHttpWebSocketCompleteUpgrade(request.value, 0);
}
} // namespace

DoubaoAsrClient::DoubaoAsrClient(std::string endpoint, std::string auth_mode, std::string app_key, std::string access_key,
                                 std::string resource_id, bool enable_itn, bool enable_punc, bool enable_ddc,
                                 std::string boosting_table_id, TranscriptCallback transcript_callback)
    : endpoint_(std::move(endpoint)), auth_mode_(std::move(auth_mode)), app_key_(std::move(app_key)), access_key_(std::move(access_key)),
      resource_id_(std::move(resource_id)), enable_itn_(enable_itn), enable_punc_(enable_punc), enable_ddc_(enable_ddc),
      boosting_table_id_(std::move(boosting_table_id)), transcript_callback_(std::move(transcript_callback))
{
}

DoubaoAsrClient::~DoubaoAsrClient()
{
    Cancel();
}

bool DoubaoAsrClient::Start()
{
    std::lock_guard<std::mutex> lock(mutex_);
    if (started_ || endpoint_.empty() || access_key_.empty() || resource_id_.empty())
        return false;
    started_ = true;
    worker_ = std::thread(&DoubaoAsrClient::Run, this);
    return true;
}

void DoubaoAsrClient::PushFloatSamples(const float *samples, std::size_t count)
{
    if (!samples || !count)
        return;
    std::vector<std::uint8_t> pcm;
    pcm.reserve(count * 2);
    for (std::size_t i = 0; i < count; ++i)
    {
        const float clamped = (std::max)(-1.0f, (std::min)(1.0f, samples[i]));
        const auto value = static_cast<std::int16_t>(clamped * 32767.0f);
        pcm.push_back(static_cast<std::uint8_t>(value));
        pcm.push_back(static_cast<std::uint8_t>(static_cast<std::uint16_t>(value) >> 8));
    }
    {
        std::lock_guard<std::mutex> lock(mutex_);
        if (!started_ || finishing_ || canceled_)
            return;
        if (pcm.size() > kMaximumQueuedBytes || queued_bytes_ > kMaximumQueuedBytes - pcm.size())
        {
            canceled_ = true;
            {
                std::lock_guard<std::mutex> result_lock(result_mutex_);
                error_ = "豆包语音识别音频队列已满。";
            }
            cv_.notify_one();
            return;
        }
        queued_bytes_ += pcm.size();
        audio_queue_.push_back(std::move(pcm));
    }
    cv_.notify_one();
}

std::string DoubaoAsrClient::Finish()
{
    {
        std::lock_guard<std::mutex> lock(mutex_);
        finishing_ = true;
    }
    cv_.notify_one();
    std::lock_guard<std::mutex> lifecycle_lock(lifecycle_mutex_);
    if (worker_.joinable())
        worker_.join();
    std::lock_guard<std::mutex> result_lock(result_mutex_);
    return result_;
}

std::string DoubaoAsrClient::LastError() const
{
    std::lock_guard<std::mutex> lock(result_mutex_);
    return error_;
}

void DoubaoAsrClient::Cancel()
{
    {
        std::lock_guard<std::mutex> lock(mutex_);
        canceled_ = true;
    }
    cv_.notify_one();
    std::lock_guard<std::mutex> lifecycle_lock(lifecycle_mutex_);
    if (worker_.joinable())
        worker_.join();
}

void DoubaoAsrClient::Run()
{
    WinHttpHandle session;
    WinHttpHandle connection;
    WinHttpHandle websocket(ConnectWebSocket(endpoint_, auth_mode_, app_key_, access_key_, resource_id_, session, connection));
    if (!websocket.value)
    {
        std::lock_guard<std::mutex> lock(result_mutex_);
        error_ = "无法连接豆包语音识别。请检查鉴权方式、凭据、资源 ID 和接口地址。";
        return;
    }

    // The full client request is sequence 1; audio sequences start at 2.
    std::int32_t sequence = 2;
    if (!SendBinary(websocket.value, StartFrame(enable_itn_, enable_punc_, enable_ddc_, boosting_table_id_)))
    {
        std::lock_guard<std::mutex> lock(result_mutex_);
        error_ = "豆包语音识别握手失败。";
        return;
    }

    // WinHTTP WebSocket supports one concurrent send and one concurrent receive.
    // Receiving here, instead of waiting for Finish(), makes partial ASR text available live.
    std::thread receiver([this, websocket_handle = websocket.value] {
        std::vector<std::uint8_t> message;
        std::string last_notified_text;
        while (ReceiveMessage(websocket_handle, message))
        {
            const ParsedResponse response = ParseResponse(message);
            if (!response.text.empty())
            {
                {
                    std::lock_guard<std::mutex> lock(result_mutex_);
                    result_ = response.text;
                }
                if (response.text != last_notified_text)
                {
                    last_notified_text = response.text;
                    if (transcript_callback_)
                    {
                        // The callback crosses back into the Server/TSF
                        // boundary. A provider update must never be allowed
                        // to escape this receiver thread: an allocation or a
                        // host callback failure otherwise invokes
                        // std::terminate and takes down the input method.
                        try
                        {
                            transcript_callback_(response.text);
                        }
                        catch (...)
                        {
                            std::lock_guard<std::mutex> lock(result_mutex_);
                            if (error_.empty())
                                error_ = "豆包语音识别结果处理失败。";
                        }
                    }
                }
            }
            if (response.last || response.code != 0)
            {
                if (response.code != 0)
                {
                    std::lock_guard<std::mutex> lock(result_mutex_);
                    if (error_.empty())
                        error_ = "豆包语音识别失败（code " + std::to_string(response.code) + "）。请检查 Access Token。";
                }
                break;
            }
        }
    });

    auto close_and_join_receiver = [&] {
        WinHttpWebSocketClose(websocket.value, WINHTTP_WEB_SOCKET_SUCCESS_CLOSE_STATUS, nullptr, 0);
        if (receiver.joinable())
            receiver.join();
    };

    std::vector<std::uint8_t> pending;
    for (;;)
    {
        bool finishing = false;
        {
            std::unique_lock<std::mutex> lock(mutex_);
            cv_.wait(lock, [this] { return canceled_ || finishing_ || !audio_queue_.empty(); });
            if (canceled_)
            {
                close_and_join_receiver();
                return;
            }
            while (!audio_queue_.empty())
            {
                auto chunk = std::move(audio_queue_.front());
                audio_queue_.pop_front();
                queued_bytes_ -= chunk.size();
                pending.insert(pending.end(), chunk.begin(), chunk.end());
            }
            finishing = finishing_;
        }
        while (pending.size() >= kPcmChunkBytes && !finishing)
        {
            if (!SendBinary(websocket.value, AudioFrame(sequence++, pending.data(), kPcmChunkBytes, false)))
            {
                {
                    std::lock_guard<std::mutex> lock(result_mutex_);
                    error_ = "豆包语音识别音频发送失败。";
                }
                close_and_join_receiver();
                return;
            }
            pending.erase(pending.begin(), pending.begin() + static_cast<std::ptrdiff_t>(kPcmChunkBytes));
        }
        if (!finishing)
            continue;
        // Drain complete chunks, leaving the final chunk for the negative sequence packet.
        while (pending.size() > kPcmChunkBytes)
        {
            if (!SendBinary(websocket.value, AudioFrame(sequence++, pending.data(), kPcmChunkBytes, false)))
            {
                {
                    std::lock_guard<std::mutex> lock(result_mutex_);
                    error_ = "豆包语音识别音频发送失败。";
                }
                close_and_join_receiver();
                return;
            }
            pending.erase(pending.begin(), pending.begin() + static_cast<std::ptrdiff_t>(kPcmChunkBytes));
        }
        if (!SendBinary(websocket.value, AudioFrame(sequence, pending.data(), pending.size(), true)))
        {
            {
                std::lock_guard<std::mutex> lock(result_mutex_);
                error_ = "豆包语音识别结束帧发送失败。";
            }
            close_and_join_receiver();
            return;
        }
        break;
    }
    if (receiver.joinable())
        receiver.join();
    WinHttpWebSocketClose(websocket.value, WINHTTP_WEB_SOCKET_SUCCESS_CLOSE_STATUS, nullptr, 0);
}
