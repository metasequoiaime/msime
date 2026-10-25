// 单按 Caps Lock 切换中英文。判定规则是三个桌面宿主共用的 shared/input/CapsLockTap.h，这里逐条钉住；TSF 里的接线要整个 TIP 才编得起来，所以和 paired_punctuation_wiring 一样检查源码。从仓库根目录运行，或传入 TSF 源码目录。
#include "../../../../../shared/input/CapsLockTap.h"

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

namespace {
int failures = 0;

void check(bool condition, const char *what) {
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}

std::string read(const std::string &path) {
    std::ifstream file(path);
    if (!file) {
        std::fprintf(stderr, "FAIL: cannot open %s\n", path.c_str());
        std::exit(EXIT_FAILURE);
    }
    std::ostringstream text;
    text << file.rdbuf();
    return text.str();
}

void expect(const std::string &text, const char *needle, bool present, const char *what) {
    check((text.find(needle) != std::string::npos) == present, what);
}

void policy() {
    using msime::input::CapsLockTap;
    using msime::input::kCapsLockTapThresholdMs;
    check(kCapsLockTapThresholdMs == 300, "the tap threshold is 300 ms on every desktop");

    CapsLockTap tap;
    tap.press(1000, true, false, false);
    check(tap.armed() && !tap.heldTooLong(1300), "a press with the switch on arms");
    check(tap.release(1300), "a release within the threshold is a tap");
    check(!tap.release(1300), "one press is one tap");

    tap.press(2000, true, false, false);
    check(tap.heldTooLong(2301), "held past the threshold is a long press");
    check(!tap.release(2301), "a long press keeps Caps Lock");

    tap.press(3000, false, false, false);
    check(!tap.armed() && !tap.release(3010), "the switch off leaves Caps Lock alone");

    tap.press(4000, true, true, false);
    check(!tap.release(4010), "pressing Caps Lock while it is on turns it off without switching");

    tap.press(5000, true, false, true);
    check(!tap.release(5010), "Caps Lock with another key held is a chord");

    tap.press(6000, true, false, false);
    tap.interrupt();
    check(!tap.release(6010), "typing while Caps Lock is held uses the lock");

    tap.press(7000, true, false, false);
    check(!tap.release(6999), "a clock that went backwards is not a tap");

    tap.press(8000, true, false, false);
    tap.reset();
    check(!tap.armed() && !tap.release(8010), "a reset drops the pending tap");
}

void wiring(const std::string &root) {
    const std::string sink = read(root + "/Key/KeyEventSink.cpp");
    expect(sink, "_QueueCapsLockEdge();", true, "第一次按下排队读取真实大写状态");
    expect(sink, "_ArmCapsLockTap(!enabled);", true, "从切换后的真实状态还原轻点前状态");
    // 「按 SHIFT 键关闭 Caps Lock」时注入的 Caps Lock 关不掉大写锁定，SendInput 却报告成功，所以那里不布防。
    expect(sink, "_IsKeyboardDisabled() && !CapsLockOffNeedsShift();", true,
           "the tap is not armed when only Shift can turn Caps Lock off");
    expect(sink, "L\"Keyboard Layout\", L\"Attributes\"", true, "the Shift-to-turn-off setting is read from the layout attributes");
    expect(sink, "(attributes & KLF_SHIFTLOCK) != 0", true, "KLF_SHIFTLOCK is the bit that means Shift turns Caps Lock off");
    expect(sink, "_capsLockTap.interrupt();", true, "another key while Caps Lock is held cancels the tap");
    expect(sink, "if (_QueueCapsLockTap(pContext, wParam))", true, "the release queues the toggle");
    check(sink.find("if (_QueueCapsLockTap(pContext, wParam))") !=
              sink.rfind("if (_QueueCapsLockTap(pContext, wParam))"),
          "both OnTestKeyUp and OnKeyUp complete the tap");
    const auto release = sink.find("if (!ReleaseCapsLockAfterTap())");
    const auto queue = sink.find("_QueueInputHotkey(pContext, Global::MetasequoiaIMEGuidImeModePreserveKey, &queued);");
    check(release != std::string::npos && queue != std::string::npos && release < queue,
          "Caps Lock is released before the language switches, and a failed release switches nothing");
    expect(sink, "inputs[0].ki.dwExtraInfo = CAPS_LOCK_SENDINPUT_EXTRA_INFO;", true,
           "the synthetic Caps Lock carries its marker");
    // 撤回的那一下被 OnTestKeyDown 当作自注入放行，TIP 要自己把大写锁定记成关，否则语言栏一直显示大写锁定。
    const auto stored = sink.find("Global::CapsLockEnabled.store(false, std::memory_order_relaxed);", release);
    check(release != std::string::npos && stored != std::string::npos && stored < queue,
          "a successful release records Caps Lock as off in the TIP");

    // Server 的低级钩子要在跳过注入按键之前计入 Caps Lock，否则撤回的那一下漏掉，Server 报告的大写锁定从此反着。
    const std::string hook = read(root + "/../src/input/MaintenanceHotkey.cpp");
    const auto counted = hook.find("self->caps_.press()");
    const auto skipped = hook.find("if (injected)\n    return CallNextHookEx");
    check(counted != std::string::npos && skipped != std::string::npos && counted < skipped,
          "the Server's hook counts the injected Caps Lock before skipping injected strokes");

    const std::string header = read(root + "/IME/MetasequoiaIME.h");
    expect(header, "extraInfo == CAPS_LOCK_SENDINPUT_EXTRA_INFO", true,
           "the synthetic Caps Lock is passed through rather than taken as another tap");

    const std::string utils = read(root + "/Utils/FanyUtils.cpp");
    expect(utils, "keybindings.value(\"switch_language_caps_lock\", false)", true,
           "the switch is read from the shared keybindings and defaults off");
}
} // namespace

int main(int argc, char **argv) {
    policy();
    wiring(argc > 1 ? argv[1] : "platforms/windows/tsf");
    if (failures != 0) return EXIT_FAILURE;
    std::puts("caps lock tap: ok");
    return EXIT_SUCCESS;
}
