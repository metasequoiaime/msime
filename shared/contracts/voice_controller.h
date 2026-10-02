#pragma once

#include <cstddef>
#include <cstdint>
#include <type_traits>

// Dedicated controller protocol; never sent on TSF Main/reverse/Aux pipes.
// v1 remains unchanged. All integers are little endian, payload is UTF-8.
namespace FanyImeVoiceController
{
inline constexpr uint32_t Magic = 0x3243564d; // bytes "MVC2"
inline constexpr uint32_t Version = 2;
inline constexpr uint32_t MaxLanguageBytes = 64;
inline constexpr uint32_t MaxTextBytes = 65536;
inline constexpr uint32_t MaxLevel = 1000;

enum class Operation : uint32_t
{
    Hello = 0,
    Start = 1,
    Stop = 2,
    Cancel = 3,
    Poll = 4
};
enum class Status : uint32_t
{
    Ok = 0,
    Invalid = 1,
    Denied = 2,
    Busy = 3,
    Stale = 4,
    Unavailable = 5
};
enum class Phase : uint32_t
{
    Idle = 0,
    Recording = 1,
    Recognizing = 2,
    Processing = 3,
    Complete = 4,
    Cancelled = 5,
    Failed = 6
};

// First message is Hello with a nonzero controller_id (PID in high 32 bits).
// It identifies the controller, NEVER the TSF target. The server authenticates
// the retained process, account and OS session before acknowledging Hello.
struct alignas(8) Request
{
    uint32_t magic = Magic;
    uint32_t version = Version;
    Operation operation = Operation::Hello;
    uint32_t language_bytes = 0;
    uint64_t request_id = 0;
    uint64_t session_id = 0;
    uint64_t controller_id = 0;
    uint64_t reserved = 0;
};

// Exactly one reply per request. No unsolicited frames. Start captures a
// server-owned focus lease and returns a connection-bound nonzero session ID.
// The controller receives reviewed text; it is the only final-submit owner.
// Native Server must not also commit that session through TSF/SendInput.
struct alignas(8) Reply
{
    uint32_t magic = Magic;
    uint32_t version = Version;
    Status status = Status::Ok;
    Phase phase = Phase::Idle;
    uint64_t request_id = 0;
    uint64_t session_id = 0;
    uint32_t text_bytes = 0;
    uint32_t level = 0;
};

constexpr bool valid_request(const Request &request, size_t message_bytes)
{
    if (request.magic != Magic || request.version != Version || request.reserved || !request.request_id ||
        !request.controller_id || !(request.controller_id >> 32) || request.operation > Operation::Poll ||
        request.language_bytes > MaxLanguageBytes || message_bytes != sizeof(Request) + request.language_bytes)
        return false;
    if (request.operation == Operation::Start)
        return request.session_id == 0 && request.language_bytes != 0;
    if (request.language_bytes != 0)
        return false;
    return request.operation == Operation::Hello ? request.session_id == 0 : request.session_id != 0;
}

constexpr bool valid_reply(const Reply &reply, size_t message_bytes)
{
    if (reply.magic != Magic || reply.version != Version || !reply.request_id || reply.status > Status::Unavailable ||
        reply.phase > Phase::Failed || reply.text_bytes > MaxTextBytes || reply.level > MaxLevel ||
        message_bytes != sizeof(Reply) + reply.text_bytes)
        return false;
    if (reply.status != Status::Ok)
        return reply.text_bytes == 0 && reply.level == 0;
    if (reply.phase == Phase::Idle)
        return reply.session_id == 0 && reply.text_bytes == 0 && reply.level == 0;
    if (!reply.session_id)
        return false;
    if (reply.phase == Phase::Cancelled || reply.phase == Phase::Failed)
        return reply.text_bytes == 0 && reply.level == 0;
    return reply.phase == Phase::Recording || reply.level == 0;
}

static_assert(sizeof(Request) == 48 && alignof(Request) == 8);
static_assert(offsetof(Request, request_id) == 16);
static_assert(offsetof(Request, controller_id) == 32);
static_assert(sizeof(Reply) == 40 && alignof(Reply) == 8);
static_assert(offsetof(Reply, request_id) == 16);
static_assert(offsetof(Reply, text_bytes) == 32);
static_assert(std::is_trivially_copyable_v<Request> && std::is_trivially_copyable_v<Reply>);
} // namespace FanyImeVoiceController
