#pragma once
#include "HostCharacterResult.h"
#include <cstdint>
#include <utility>

namespace msime::tsf {
// 与现有 FIFO 的一个回放 token 共生；本地会话推进后，最终文本仍归 Server。
class WubiCommitOwner {
public:
    enum class Worker { Early, Ready, Invalid };
    void begin(uint64_t token, uint64_t now) { clear(); token_ = token; started_ = now; }
    void clear() { token_ = started_ = 0; early_ = 0; applied_ = needs_worker_ = local_done_ = editing_ = false; }
    bool active() const { return token_ != 0; }
    bool current(uint64_t token) const { return active() && token_ == token; }
    uint64_t token() const { return token_; }
    bool applied() const { return applied_; }
    bool observe(const EngineResult &result) {
        if (!active()) return false;
        applied_ = true;
        needs_worker_ = result.has_commit;
        return needs_worker_;
    }
    bool finish_local() {
        local_done_ = true;
        return needs_worker_;
    }
    bool has_early_worker() const { return early_ != 0; }
    uint32_t take_early_worker() { const auto value = early_; early_ = 0; return value; }
    Worker worker(uint32_t message) {
        if (!active() || editing_ || !message) return Worker::Invalid;
        if (!local_done_) {
            if (early_) return Worker::Invalid;
            early_ = message;
            return Worker::Early;
        }
        if (!needs_worker_) return Worker::Invalid;
        editing_ = true;
        return Worker::Ready;
    }
    bool editing(uint64_t token) const { return current(token) && editing_; }
    bool expired(uint64_t now) const { return active() && now - started_ >= 2000; }
private:
    uint64_t token_ = 0, started_ = 0;
    uint32_t early_ = 0;
    bool applied_ = false, needs_worker_ = false, local_done_ = false, editing_ = false;
};

// 余码来自已经推进的本地视图；Server 的最终文本可能经过脚本和简繁转换。
inline EngineResult ServerWubiCommit(EngineResult live, std::string finalText) {
    live.handled = true;
    live.has_commit = true;
    live.commit = std::move(finalText);
    return live;
}
}
