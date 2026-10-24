#pragma once
#import <AppKit/AppKit.h>
#include <IOKit/hidsystem/IOLLEvent.h>

// Native modifier events use device-side flags, not keyDown/keyUp. Match the
// Windows written chord order (Control first), adapting Win to Command.
class MSIMEVoiceHoldShortcut {
public:
    enum class Action { None, Toggle, Cancel };
    struct Result { bool consumed = false; Action action = Action::None; bool onRelease = false; };
    struct Options { bool rightOption; bool controlCommand; bool rightControlOption; bool spaceLock; };
    void reset() { *this = MSIMEVoiceHoldShortcut{}; }
    // Space during the hold locked the recording: releasing the hold no longer stops it.
    bool locked() const { return locked_; }
    Result observe(NSEvent *event, Options options, bool recording, NSEventModifierFlags physicalModifierFlags = 0) {
        if (!recording) locked_ = false;
        const auto type = event.type;
        const auto key = event.keyCode;
        if (key == 53 && (type == NSEventTypeKeyDown || type == NSEventTypeKeyUp)) {
            if (type == NSEventTypeKeyDown && escape_) return {true};
            if (type == NSEventTypeKeyDown && recording) { escape_ = true; return {true, Action::Cancel}; }
            if (type == NSEventTypeKeyUp && escape_) { escape_ = false; return {true}; }
        }
        if (key == 49 && (type == NSEventTypeKeyDown || type == NSEventTypeKeyUp)) {
            if (type == NSEventTypeKeyDown && space_) return {true};
            if (type == NSEventTypeKeyDown && hold_ != Hold::None && options.spaceLock) {
                space_ = true; if (recording) locked_ = true; return {true};
            }
            if (type == NSEventTypeKeyUp && space_) { space_ = false; return {true}; }
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
