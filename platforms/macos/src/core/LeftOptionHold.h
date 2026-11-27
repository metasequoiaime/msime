#pragma once
#import <AppKit/AppKit.h>
#include <IOKit/hidsystem/IOLLEvent.h>
#include <cmath>

// 长按左 Option 打开表情与符号面板（#11859，偏好 `keybindings.symbol_panel_left_option_hold`）的手势判定。
// 它不读时钟、不碰界面：宿主把每个按键事件交给 observe，到点后用会话里的修饰键状态问 due，再用 didOpen 告诉它面板有没有真的打开。这样整套时序和组合键判定可以在没有图形会话的单测里跑。
// 只认左 Option（keyCode 58）；右 Option 是语音按住说话的键，这里从不处理它。按下和短按松开都交给应用，只有面板确实打开之后，这一次按住的松开才被吞掉。
class MSIMELeftOptionHold {
public:
    static constexpr double holdSeconds = 0.4;
    static constexpr unsigned short leftOptionKey = 58;
    enum class Event { FlagsChanged, KeyDown, KeyUp };

    void reset() { *this = MSIMELeftOptionHold{}; }
    // 正在等按满 400 毫秒：宿主据此安排到期检查。
    bool pending() const { return state_ == State::Pending; }
    // 到期时刻，和事件时间戳同一个时钟（系统启动以来的秒数）；没有在等时是 NaN。
    double deadline() const { return pending() ? pressedAt_ + holdSeconds : NAN; }
    // 面板已经由这次按住打开，还在等松开。
    bool opened() const { return state_ == State::Opened; }

    // 返回 true 表示宿主要吞掉这个事件，只有面板打开之后那次按住的松开会这样。enabled 为假（开关关着、安全输入期间）时整个手势作废，事件全部交给应用。
    bool observe(Event type, unsigned short key, NSEventModifierFlags flags, double time, bool enabled) {
        if (!enabled) {
            reset();
            return false;
        }
        if (type != Event::FlagsChanged) {
            // 按住左 Option 期间按了别的键（包括按住前就按着、这时才抬起的键），是组合键，不是长按。
            if (state_ == State::Pending) state_ = State::Cancelled;
            return false;
        }
        if (key == leftOptionKey) {
            if (leftDown(flags)) {
                // 同一次按住的重复快照不算新的按下。面板打开后焦点去了面板，松开可能送不到这里，所以打开之后再收到按下就是新的一次。
                if (down_ && state_ != State::Opened) return false;
                down_ = true;
                pressedAt_ = time;
                state_ = alone(flags) && std::isfinite(time) ? State::Pending : State::Cancelled;
                return false;
            }
            const bool swallow = state_ == State::Opened;
            down_ = false;
            state_ = State::Idle;
            return swallow;
        }
        // 别的修饰键变了：按住期间加上 Shift、Control、Command、Fn 或右 Option 都是组合键。
        if (state_ == State::Pending) state_ = State::Cancelled;
        // 左 Option 的松开没有单独送到、而是随别的修饰键事件一起反映出来时，按住到此结束；这个事件属于别的键，不吞。
        if (down_ && !(flags & NSEventModifierFlagOption)) {
            down_ = false;
            state_ = State::Idle;
        }
        return false;
    }

    // 鼠标按下（Option+点按、Option+拖动）不经过输入法的按键流，宿主看到后调用它。
    void cancel() {
        if (state_ == State::Pending) state_ = State::Cancelled;
    }

    // 到期检查：now 是当前时刻，sessionFlags 是会话里此刻的修饰键状态。返回 true 表示现在该打开面板，宿主接着必须调用 didOpen。
    // 松开事件可能没送到，所以这里再用会话状态确认左 Option 还单独按着；已经抬起就当这次按住结束。
    bool due(double now, NSEventModifierFlags sessionFlags) {
        if (state_ != State::Pending || !(now >= pressedAt_ + holdSeconds)) return false;
        if (!leftDown(sessionFlags)) {
            down_ = false;
            state_ = State::Idle;
            return false;
        }
        if (!alone(sessionFlags)) {
            state_ = State::Cancelled;
            return false;
        }
        state_ = State::Opening;
        return true;
    }

    // 面板是否真的打开了。没打开时这次按住的松开照常交给应用。
    void didOpen(bool success) {
        if (state_ == State::Opening) state_ = success ? State::Opened : State::Cancelled;
    }

private:
    enum class State { Idle, Pending, Cancelled, Opening, Opened };
    static constexpr NSEventModifierFlags optionSides = NX_DEVICELALTKEYMASK | NX_DEVICERALTKEYMASK;
    // 有侧别位时按侧别判断；只有通用 Option 位、看不出哪一侧时，按左侧处理（宿主只在 keyCode 58 的事件和会话状态上问它）。
    static bool leftDown(NSEventModifierFlags flags) {
        if (!(flags & NSEventModifierFlagOption)) return false;
        return (flags & NX_DEVICELALTKEYMASK) || !(flags & optionSides);
    }
    // 除了左 Option 没有别的修饰键。大写锁定是锁定状态，不算按着的键。
    static bool alone(NSEventModifierFlags flags) {
        const NSEventModifierFlags others = NSEventModifierFlagShift | NSEventModifierFlagControl |
            NSEventModifierFlagCommand | NSEventModifierFlagFunction;
        return !(flags & others) && !(flags & NX_DEVICERALTKEYMASK);
    }

    State state_ = State::Idle;
    bool down_ = false;
    double pressedAt_ = 0;
};
