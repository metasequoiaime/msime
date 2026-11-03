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
        if (phase != Phase::Idle || source.empty() || pid <= 0) return false;
        original = std::move(source); targetPID = pid; deadline = now + 3;
        selected.clear(); manual = false; phase = Phase::Waiting; ++generation;
        return true;
    }
    bool activate(int pid, std::string source) {
        if (phase != Phase::Waiting || pid != targetPID || source.empty()) return false;
        selected = std::move(source); phase = Phase::Active; return true;
    }
    void hostSelectedSource(const std::string &source) {
        if (phase != Phase::Idle && !manual && !source.empty()) selected = source;
    }
    void sourceChanged(const std::string &source) {
        if (phase != Phase::Idle && !selected.empty() && source != selected) manual = true;
    }
    bool timedOut(double now) const { return phase == Phase::Waiting && now >= deadline; }
    bool canRestore(const std::string &current) const {
        return phase != Phase::Idle && !manual && !selected.empty() && current == selected && original != selected;
    }
    void clear() { phase = Phase::Idle; original.clear(); selected.clear(); targetPID = 0; }
    Phase phase = Phase::Idle;
    std::string original, selected;
    int targetPID = 0;
    bool manual = false;
    double deadline = 0;
    uint64_t generation = 0;
};
}
