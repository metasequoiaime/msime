#pragma once
#import <AppKit/AppKit.h>
#include <IOKit/hidsystem/IOLLEvent.h>

// Native modifier events use device-side flags, not keyDown/keyUp. Match the
// Windows written chord order (Control first), adapting Win to Command.
class MSIMEVoiceHoldShortcut {
public:
    enum class Action { None, Toggle, Cancel, ChordCancel };
    struct Result { bool consumed = false; Action action = Action::None; bool onRelease = false; };
    struct Options { bool rightOption; bool controlCommand; bool rightControlOption; bool spaceLock; };
    void reset() { *this = MSIMEVoiceHoldShortcut{}; }
    // Space during the hold locked the recording: releasing the hold no longer stops it.
    bool locked() const { return locked_; }
    // 按住还没结束：宿主据此在按住期间轮询（poll），松开通知没送到也能收尾。
    bool holding() const { return hold_ != Hold::None; }
    // 这次按住开始的录音还没锁定，按住一结束就该停。宿主丢掉按住状态（reset）之前用它判断要不要先把录音停下。
    bool finishesOnRelease() const { return hold_ != Hold::None && finishOnRelease_ && !locked_; }
    // 松开的 flagsChanged 可能根本送不到输入法，送到的也可能还是按下时的快照；只靠事件，按住就永远结束不了，录音一直开着，下一次按下还会被当成重复通知吞掉（2026-11-03 设备验收）。
    // 宿主在按住期间定时拿会话里的修饰键状态（CGEventSourceFlagsState）来问：确定按住的键已经抬起，就和收到松开一样结束按住。
    // 只在确定抬起时才结束：通用修饰位没了，或者侧别位在场而按住的那一侧不在。只有通用位、看不出哪一侧时当作还按着，宁可晚停，也不在用户还按着时提前停。
    // 迟到的松开事件照旧被吞（suppressed_ 留着），应用从没收到过这次按下。
    Result poll(NSEventModifierFlags sessionFlags, bool recording) {
        if (!recording) locked_ = false;
        if (hold_ == Hold::None || !released(sessionFlags)) return {};
        hold_ = Hold::None;
        // 换成会话里的状态，下一次按下才构成新的按下沿，不会被当成这次按住的重复通知。
        flags_ = sessionFlags;
        return {false, recording && finishOnRelease_ && !locked_ ? Action::Toggle : Action::None, true};
    }
    Result observe(NSEvent *event, Options options, bool recording, NSEventModifierFlags physicalModifierFlags = 0) {
        if (!recording) locked_ = false;
        const auto type = event.type;
        const auto key = event.keyCode;
        // 被吞掉的 Esc、锁定用的空格，它们的 keyUp 不一定送到（验收里 Esc 在一次焦点内只有第一次能取消录音）。
        // 所以标记只拦自动重复；新的一次按下说明上一次早已抬起，先清掉标记再照常处理，不能从此把这个键一直吞掉。
        if (key == 53 && (type == NSEventTypeKeyDown || type == NSEventTypeKeyUp)) {
            if (type == NSEventTypeKeyDown && escape_ && event.isARepeat) return {true};
            if (type == NSEventTypeKeyDown) escape_ = false;
            if (type == NSEventTypeKeyDown && recording) { escape_ = true; return {true, Action::Cancel}; }
            if (type == NSEventTypeKeyUp && escape_) { escape_ = false; return {true}; }
        }
        if (key == 49 && (type == NSEventTypeKeyDown || type == NSEventTypeKeyUp)) {
            if (type == NSEventTypeKeyDown && space_ && event.isARepeat) return {true};
            if (type == NSEventTypeKeyDown) space_ = false;
            if (type == NSEventTypeKeyDown && hold_ != Hold::None && options.spaceLock) {
                space_ = true; if (recording) locked_ = true; return {true};
            }
            if (type == NSEventTypeKeyUp && space_) { space_ = false; return {true}; }
        }
        // 按住右 Option 时又新按下一个键，用户按的是组合键而不是要说话（#10580，对齐 Windows #10581）：按住就此结束，只有这次按住自己开始、还没锁定的录音报 ChordCancel。
        // 按键本身不吞，它的 modifierFlags 里本来就带着 Option，应用照常收到组合键，不必像 Windows 那样补发修饰键。
        // suppressed_ 留着，右 Option 的松开仍被吞掉：应用从没收到过它的按下。空格锁定开着时空格在上面已经处理，Esc 保持取消的含义，自动重复不是新按下的键。
        // Control+Command 吞的是 Command，和 Windows 的 Ctrl+Win 一样不在此列。
        if (type == NSEventTypeKeyDown && !event.isARepeat && key != 53 && !locked_ &&
            (hold_ == Hold::RightOption || hold_ == Hold::RightControlOption)) {
            const bool started = recording && finishOnRelease_;
            hold_ = Hold::None;
            return {false, started ? Action::ChordCancel : Action::None};
        }
        if (type != NSEventTypeFlagsChanged) return {};
        const auto previous = flags_;
        flags_ = event.modifierFlags;
        const auto optionSides = NX_DEVICELALTKEYMASK | NX_DEVICERALTKEYMASK;
        // 只补本次变化的侧别，避免较晚的实体快照提前占用另一侧的按下沿。
        if ((flags_ & NSEventModifierFlagOption) && !(flags_ & optionSides)) {
            const auto changedSide = key == 61 ? NX_DEVICERALTKEYMASK : key == 58 ? NX_DEVICELALTKEYMASK : 0;
            flags_ |= (previous & optionSides & ~changedSide) | (physicalModifierFlags & changedSide);
        }
        bool consumed = false;
        if (suppressed_ == key && !down(key, flags_)) { consumed = true; suppressed_ = 0; }
        if (hold_ != Hold::None && !held()) {
            hold_ = Hold::None;
            return {consumed, recording && finishOnRelease_ && !locked_ ? Action::Toggle : Action::None, true};
        }
        if (hold_ != Hold::None) return {key == suppressed_};
        if (!down(key, flags_) || down(key, previous)) return {consumed};
        Hold next = Hold::None;
        if (key == 61) {
            if (options.rightControlOption && (previous & NX_DEVICERCTLKEYMASK) && (flags_ & NX_DEVICERCTLKEYMASK)) next = Hold::RightControlOption;
            else if (options.rightOption) next = Hold::RightOption;
        } else if ((key == 54 || key == 55) && options.controlCommand && (previous & NSEventModifierFlagControl) && (flags_ & NSEventModifierFlagControl)) {
            next = Hold::ControlCommand;
        }
        if (next == Hold::None) return {consumed};
        hold_ = next; suppressed_ = key; locked_ = false;
        finishOnRelease_ = !recording;
        return {true, Action::Toggle};
    }
private:
    enum class Hold { None, RightOption, ControlCommand, RightControlOption };
    static bool down(unsigned short key, NSEventModifierFlags flags) {
        switch (key) {
        case 61: return flags & NX_DEVICERALTKEYMASK;
        case 54: return flags & NX_DEVICERCMDKEYMASK;
        case 55: return flags & NX_DEVICELCMDKEYMASK;
        default: return false;
        }
    }
    // 一侧修饰键确定已经抬起：通用位没了，或者有侧别位而没有这一侧。
    static bool sideReleased(NSEventModifierFlags flags, NSEventModifierFlags generic, NSEventModifierFlags sides, NSEventModifierFlags side) {
        return !(flags & generic) || ((flags & sides) && !(flags & side));
    }
    bool released(NSEventModifierFlags flags) const {
        const bool rightOption = sideReleased(flags, NSEventModifierFlagOption, NX_DEVICELALTKEYMASK | NX_DEVICERALTKEYMASK, NX_DEVICERALTKEYMASK);
        switch (hold_) {
        case Hold::RightOption: return rightOption;
        case Hold::RightControlOption:
            return rightOption || sideReleased(flags, NSEventModifierFlagControl, NX_DEVICELCTLKEYMASK | NX_DEVICERCTLKEYMASK, NX_DEVICERCTLKEYMASK);
        case Hold::ControlCommand: return !(flags & NSEventModifierFlagControl) || !(flags & NSEventModifierFlagCommand);
        default: return false;
        }
    }
    bool held() const {
        switch (hold_) {
        case Hold::RightOption: return flags_ & NX_DEVICERALTKEYMASK;
        case Hold::RightControlOption: return (flags_ & NX_DEVICERCTLKEYMASK) && (flags_ & NX_DEVICERALTKEYMASK);
        case Hold::ControlCommand: return (flags_ & NSEventModifierFlagControl) && (flags_ & NSEventModifierFlagCommand);
        default: return false;
        }
    }
    Hold hold_ = Hold::None;
    NSEventModifierFlags flags_ = 0;
    unsigned short suppressed_ = 0;
    bool locked_ = false;
    bool space_ = false;
    bool escape_ = false;
    bool finishOnRelease_ = false;
};
