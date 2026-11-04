#pragma once
#include <cstdint>
#include <string>
#include <utility>

namespace msime::mac {
// 输入源恢复与录音阶段无关：收到最终结果或完整取消后才释放临时选择。
class GlobalVoiceState {
public:
    enum class Phase { Idle, Waiting, Active, Finishing };
    bool begin(std::string source, int pid, double now) {
        if (phase != Phase::Idle || pid < 0) return false;
        original = std::move(source); targetPID = pid; deadline = now + 3;
        selected.clear(); selectedObserved = false; manual = false; phase = Phase::Waiting; ++generation;
        return true;
    }
    bool activate(int pid, std::string source) {
        if (phase != Phase::Waiting || pid != targetPID) return false;
        selected = std::move(source); phase = Phase::Active; return true;
    }
    // TIS 接受请求不代表选择已完成；第一次到达之前仍看到原输入源属于正常等待。
    bool observeWaitingSource(const std::string &source, bool ours) {
        if (phase != Phase::Waiting || source.empty()) return false;
        if (ours) { selectedObserved = true; hostSelectedSource(source); return true; }
        if (selectedObserved || source != original) manual = true;
        return false;
    }
    void hostSelectedSource(const std::string &source) {
        if (phase != Phase::Idle && !manual && !source.empty()) selected = source;
    }
    void sourceChanged(const std::string &source) {
        if (phase != Phase::Idle && !selected.empty() && source != selected) manual = true;
    }
    bool timedOut(double now) const { return phase == Phase::Waiting && now >= deadline; }
    bool canRestore(const std::string &current) const {
        return phase != Phase::Idle && !manual && !original.empty() && !selected.empty() && original != selected && (current == selected ||
            (!selectedObserved && current == original));
    }
    void clear() { phase = Phase::Idle; original.clear(); selected.clear(); targetPID = 0; selectedObserved = false; }
    Phase phase = Phase::Idle;
    std::string original, selected;
    int targetPID = 0;
    bool manual = false;
    bool selectedObserved = false;
    double deadline = 0;
    uint64_t generation = 0;
};
}
