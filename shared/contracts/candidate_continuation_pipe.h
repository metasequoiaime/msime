#pragma once
#include "windows_ipc.h"
#include <algorithm>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace FanyImeCandidateContinuationPipe {
constexpr std::size_t HeaderChars = 3;
constexpr std::size_t ChunkChars = FanyImePipeLimits::CandidateTextCapacity - HeaderChars - 1;
constexpr std::size_t MaxPayloadChars = 32768;
constexpr std::size_t MaxFrames = (MaxPayloadChars + ChunkChars - 1) / ChunkChars;

// ASCII JSON 在固定 UTF-16 包里逐字符传输，不截断，也不依赖宿主 wchar_t 的宽度。
inline std::optional<std::vector<FanyImeNamedpipeDataToTsfWorkerThread>>
Encode(std::string_view payload, uint16_t transaction,
       uint32_t type = FanyImeWorkerReplyType::CandidateContinuation,
       std::size_t limit = MaxPayloadChars) {
    if (!transaction || payload.empty() || payload.size() > limit ||
        std::any_of(payload.begin(), payload.end(), [](unsigned char c) { return c == 0 || c > 0x7f; }))
        return std::nullopt;
    const auto count = (payload.size() + ChunkChars - 1) / ChunkChars;
    std::vector<FanyImeNamedpipeDataToTsfWorkerThread> frames(count);
    for (std::size_t index = 0; index < count; ++index) {
        auto &frame = frames[index];
        frame.msg_type = type;
        frame.data[0] = static_cast<FanyImeWireChar>(transaction);
        frame.data[1] = static_cast<FanyImeWireChar>(index + 1);
        frame.data[2] = static_cast<FanyImeWireChar>(count);
        const auto offset = index * ChunkChars;
        const auto length = (std::min)(ChunkChars, payload.size() - offset);
        for (std::size_t n = 0; n < length; ++n)
            frame.data[HeaderChars + n] = static_cast<FanyImeWireChar>(payload[offset + n]);
    }
    return frames;
}

struct Owner { uint64_t focus; uint64_t epoch; };

struct Completed { std::string text; uint64_t focus; uint64_t epoch; };

template<std::size_t Limit = MaxPayloadChars>
class BasicAssembler {
public:
    uint64_t focus() const { return focus_; }
    uint64_t epoch() const { return epoch_; }
    bool active() const { return next_ != 0; }
    const std::optional<Owner> &failed_owner() const { return failedOwner_; }
    void reset() { failedOwner_.reset(); text_.clear(); transaction_ = 0; next_ = 0; count_ = 0; focus_ = 0; epoch_ = 0; }

    // 仅完整、有序且全程属于同一焦点与组字 epoch 的载荷才能离开 worker 线程。
    template<class Char>
    std::optional<Completed> accept(const Char *data, std::size_t size, uint64_t focus, uint64_t epoch) {
        failedOwner_.reset();
        if (!data || size < HeaderChars + 1 || !focus || !epoch) { reject(); return std::nullopt; }
        const auto transaction = static_cast<uint16_t>(data[0]);
        const auto index = static_cast<uint16_t>(data[1]);
        const auto count = static_cast<uint16_t>(data[2]);
        if (!transaction || !count || count > (Limit + ChunkChars - 1) / ChunkChars || !index || index > count) {
            reject(); return std::nullopt;
        }
        if (index == 1 && next_ != 0) { reject(); return std::nullopt; }
        if (index == 1) {
            reset(); transaction_ = transaction; count_ = count; next_ = 1; focus_ = focus; epoch_ = epoch;
        }
        if (transaction != transaction_ || count != count_ || index != next_ || focus != focus_ || epoch != epoch_) {
            reject(); return std::nullopt;
        }
        std::size_t end = HeaderChars;
        while (end < size && data[end] != 0) {
            if (static_cast<uint32_t>(data[end]) > 0x7f) { reject(); return std::nullopt; }
            ++end;
        }
        const auto length = end - HeaderChars;
        if (end == size || length == 0 || (index != count && length != ChunkChars) ||
            text_.size() + length > Limit) { reject(); return std::nullopt; }
        for (std::size_t n = HeaderChars; n < end; ++n) text_.push_back(static_cast<char>(data[n]));
        ++next_;
        if (index != count) return std::nullopt;
        Completed completed{std::move(text_), focus_, epoch_};
        reset();
        return completed;
    }
private:
    void reject() {
        const auto owner = active() ? std::optional<Owner>{{focus_, epoch_}} : std::nullopt;
        reset(); failedOwner_ = owner;
    }
    std::optional<Owner> failedOwner_;
    std::string text_;
    uint16_t transaction_ = 0;
    std::size_t next_ = 0, count_ = 0;
    uint64_t focus_ = 0, epoch_ = 0;
};
using Assembler = BasicAssembler<>;
} // namespace FanyImeCandidateContinuationPipe
