#include "DoubaoAsrClient.h"
#include "DoubaoTranscript.h"
#include "../../../../shared/voice/DoubaoAuth.h"

#include <nlohmann/json.hpp>
#include <windows.h>
#include <winhttp.h>
#include <zlib.h>

#include <algorithm>
#include <array>
#include <chrono>
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

void AppendBigEndian32(std::vector<std::uint8_t> &output, std::int32_t value)
{
    const auto unsigned_value = static_cast<std::uint32_t>(value);
    output.push_back(static_cast<std::uint8_t>(unsigned_value >> 24));
    output.push_back(static_cast<std::uint8_t>(unsigned_value >> 16));
    output.push_back(static_cast<std::uint8_t>(unsigned_value >> 8));
    output.push_back(static_cast<std::uint8_t>(unsigned_value));
}

std::uint32_t ReadBigEndian32(const std::uint8_t *data)
{
    return (static_cast<std::uint32_t>(data[0]) << 24) | (static_cast<std::uint32_t>(data[1]) << 16) |
           (static_cast<std::uint32_t>(data[2]) << 8) | static_cast<std::uint32_t>(data[3]);
}

std::vector<std::uint8_t> GzipCompress(const std::uint8_t *data, std::size_t size)
{
    z_stream stream{};
    if (deflateInit2(&stream, Z_DEFAULT_COMPRESSION, Z_DEFLATED, MAX_WBITS + 16, 8, Z_DEFAULT_STRATEGY) != Z_OK)
        return {};
    const std::size_t compressed_capacity = static_cast<std::size_t>(compressBound(static_cast<uLong>(size))) + 32;
    std::vector<std::uint8_t> output((std::max)(std::size_t{64}, compressed_capacity));
    stream.next_in = const_cast<Bytef *>(reinterpret_cast<const Bytef *>(data));
    stream.avail_in = static_cast<uInt>(size);
    stream.next_out = output.data();
    stream.avail_out = static_cast<uInt>(output.size());
    const int status = deflate(&stream, Z_FINISH);
    if (status != Z_STREAM_END)
    {
        deflateEnd(&stream);
        return {};
    }
    output.resize(stream.total_out);
    deflateEnd(&stream);
    return output;
}

std::vector<std::uint8_t> GzipDecompress(const std::uint8_t *data, std::size_t size)
{
    z_stream stream{};
    if (inflateInit2(&stream, MAX_WBITS + 16) != Z_OK)
        return {};
    stream.next_in = const_cast<Bytef *>(reinterpret_cast<const Bytef *>(data));
    stream.avail_in = static_cast<uInt>(size);
    std::vector<std::uint8_t> output;
    std::array<std::uint8_t, 8192> buffer{};
    int status = Z_OK;
    while (status == Z_OK)
    {
        stream.next_out = buffer.data();
        stream.avail_out = static_cast<uInt>(buffer.size());
        status = inflate(&stream, Z_NO_FLUSH);
        const auto produced = buffer.size() - stream.avail_out;
        if (produced > kMaximumResponseBytes - output.size())
        {
            inflateEnd(&stream);
            return {};
        }
        output.insert(output.end(), buffer.begin(),
                      buffer.begin() + static_cast<std::ptrdiff_t>(produced));
    }
    inflateEnd(&stream);
    return status == Z_STREAM_END ? output : std::vector<std::uint8_t>{};
}

std::vector<std::uint8_t> BuildPacket(std::uint8_t message_type, std::uint8_t flags, std::int32_t sequence,
                                      const std::uint8_t *payload, std::size_t payload_size)
{
    const auto compressed = GzipCompress(payload, payload_size);
    if (compressed.empty())
        return {};
    std::vector<std::uint8_t> packet;
    packet.reserve(12 + compressed.size());
    packet.push_back(0x11); // Protocol v1, 4-byte header.
    packet.push_back(static_cast<std::uint8_t>((message_type << 4) | flags));
    packet.push_back(0x11); // JSON serialization + gzip (also required by the audio frame protocol).
    packet.push_back(0x00);
    AppendBigEndian32(packet, sequence);
    AppendBigEndian32(packet, static_cast<std::int32_t>(compressed.size()));
    packet.insert(packet.end(), compressed.begin(), compressed.end());
    return packet;
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

ParsedResponse ParseResponse(const std::vector<std::uint8_t> &message)
{
    ParsedResponse response;
    if (message.size() < 4)
        return response;
    const std::size_t header_size = (message[0] & 0x0f) * 4;
    if (header_size > message.size())
        return response;
    const std::uint8_t message_type = message[1] >> 4;
    const std::uint8_t flags = message[1] & 0x0f;
    const std::uint8_t serialization = message[2] >> 4;
    const std::uint8_t compression = message[2] & 0x0f;
    response.last = (flags & 0x02) != 0;
    std::size_t offset = header_size;
    if (flags & 0x01)
        offset += 4;
    if (flags & 0x04)
        offset += 4;
    if (offset > message.size())
        return response;
    std::size_t payload_size = 0;
    if (message_type == 0x09)
    {
        if (offset + 4 > message.size())
            return response;
        payload_size = ReadBigEndian32(message.data() + offset);
        offset += 4;
    }
    else if (message_type == 0x0f)
    {
        if (offset + 8 > message.size())
            return response;
        response.code = static_cast<int>(ReadBigEndian32(message.data() + offset));
        payload_size = ReadBigEndian32(message.data() + offset + 4);
        offset += 8;
    }
    else
        return response;
    payload_size = (std::min)(payload_size, message.size() - offset);
    std::vector<std::uint8_t> payload(message.begin() + static_cast<std::ptrdiff_t>(offset),
                                      message.begin() + static_cast<std::ptrdiff_t>(offset + payload_size));
    if (compression == 0x01)
        payload = GzipDecompress(payload.data(), payload.size());
    if (serialization != 0x01 || payload.empty())
        return response;
    try
    {
        const auto json = nlohmann::json::parse(payload.begin(), payload.end());
        response.text = msime::windows::doubao_transcript(json);
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

    nlohmann::json request_options = {{"model_name", "bigmodel"},    {"enable_itn", enable_itn_},
                                      {"enable_punc", enable_punc_}, {"enable_ddc", enable_ddc_},
                                      {"show_utterances", false},    {"result_type", "full"}};
    if (!boosting_table_id_.empty())
        request_options["corpus"] = {{"boosting_table_id", boosting_table_id_}};

    const nlohmann::json request_json = {
        {"user", {{"uid", "metasequoia-ime"}}},
        {"audio", {{"format", "pcm"}, {"codec", "raw"}, {"rate", 16000}, {"bits", 16}, {"channel", 1}}},
        {"request", std::move(request_options)}};
    const std::string request_text = request_json.dump();
    std::int32_t sequence = 1;
    if (!SendBinary(websocket.value,
                    BuildPacket(0x01, 0x01, sequence++, reinterpret_cast<const std::uint8_t *>(request_text.data()),
                                request_text.size())))
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
            if (!SendBinary(websocket.value, BuildPacket(0x02, 0x01, sequence++, pending.data(), kPcmChunkBytes)))
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
            if (!SendBinary(websocket.value, BuildPacket(0x02, 0x01, sequence++, pending.data(), kPcmChunkBytes)))
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
        const std::int32_t last_sequence = -sequence;
        if (!SendBinary(websocket.value, BuildPacket(0x02, 0x03, last_sequence, pending.data(), pending.size())))
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
