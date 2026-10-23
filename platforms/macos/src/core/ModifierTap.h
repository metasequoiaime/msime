#pragma once
#import <AppKit/AppKit.h>
#include <cmath>
#include <set>

// Observe the complete key stream; only a short, isolated modifier release toggles.
// Engine composition remains owned by the normal host mode-switch operation.
class MSIMEModifierTap {
public:
    /// Whether a key code is being held on the keyboard right now. A test drives a synthetic
    /// keyboard and has to answer for it; the default reads the HID system's own view.
    using KeyHeldProbe = bool (*)(unsigned short);
    void reset() {
        const auto probe = held_;
        *this = MSIMEModifierTap{};
        held_ = probe;
    }
    void setKeyHeldProbe(KeyHeldProbe probe) { held_ = probe ? probe : &MSIMEModifierTap::physicallyHeld; }
    // Shift 被其他字符打断后，同一轮按住期间的 Shift+空格应交回正常输入。
    bool shiftChordInterrupted() const { return shiftChordInterrupted_; }
    bool observe(NSEvent *event, bool shiftEnabled, bool controlEnabled) {
        const auto mask = NSEventModifierFlagShift | NSEventModifierFlagControl |
            NSEventModifierFlagOption | NSEventModifierFlagCommand | NSEventModifierFlagFunction;
        const auto flags = event.modifierFlags & mask;
        const auto previous = modifiers_;
        modifiers_ = flags;
        // 释放通知可能丢失；普通按键报告的修饰键状态也能确认这一轮已结束。
        if (!(flags & NSEventModifierFlagShift)) shiftChordInterrupted_ = false;
        if (event.type == NSEventTypeKeyDown) {
            const bool modifier = event.keyCode == 54 || event.keyCode == 55 ||
                event.keyCode == 56 || event.keyCode == 58 || event.keyCode == 59 ||
                event.keyCode == 60 || event.keyCode == 61 || event.keyCode == 62 ||
                event.keyCode == 63;
            const auto competing = NSEventModifierFlagControl | NSEventModifierFlagOption |
                NSEventModifierFlagCommand | NSEventModifierFlagFunction;
            // 空格本身由调用方决定是否是快捷键；带其他修饰键的组合也不属于纯 Shift 轮次。
            if ((flags & NSEventModifierFlagShift) && !(flags & competing) &&
                event.keyCode != 49 && !modifier)
                shiftChordInterrupted_ = true;
            keys_.insert(event.keyCode);
            armed_ = false;
            return false;
        }
        if (event.type == NSEventTypeKeyUp) {
            keys_.erase(event.keyCode);
            armed_ = false;
            return false;
        }
        if (event.type != NSEventTypeFlagsChanged) return false;
        const bool shift = event.keyCode == 56 || event.keyCode == 60;
        const bool control = event.keyCode == 59 || event.keyCode == 62;
        const bool enabled = (shift && shiftEnabled) || (control && controlEnabled);
        const auto required = shift ? NSEventModifierFlagShift : NSEventModifierFlagControl;
        const double time = event.timestamp;
        const bool sameModifier = (shift || control) && required == armedModifier_;
        const bool toggle = armed_ && sameModifier && flags == 0 &&
            previous == required && enabled && !anyKeyHeld() && std::isfinite(time) &&
            time >= pressedAt_ && time - pressedAt_ < 0.5;
        // Both sides of the same modifier form one tap, timed from the first
        // press and completed only when the last side is released.
        if (armed_ && sameModifier && flags == required && enabled) return false;
        armed_ = false;
        // No rearming while another modifier is held. Chords and interrupted
        // taps remain cancelled even after the competing modifier is released.
        if (enabled && previous == 0 && flags == required && !anyKeyHeld() && std::isfinite(time)) {
            armed_ = true;
            armedModifier_ = required;
            pressedAt_ = time;
        }
        return toggle;
    }
private:
    /// Whether a key other than the modifier is being held right now.
    ///
    /// The set is fed by the key events this host receives, and a key-up can simply not arrive: the
    /// user switches away with a key down, an application takes the release, or - as happened while
    /// this was being investigated - the host asks for a narrower set of events and stops being
    /// told about releases at all. Any of those leaves an entry behind forever, and the tap is then
    /// refused for the rest of the session with nothing to show for it.
    ///
    /// So the set is checked against what is actually held, rather than trusted. The state query
    /// reads the HID system's view of the keyboard and needs no permission; the set still decides
    /// which keys are worth asking about, so an empty one costs nothing.
    bool anyKeyHeld() const {
        for (const auto key : keys_)
            if (held_(key)) return true;
        return false;
    }
    static bool physicallyHeld(unsigned short key) {
        return CGEventSourceKeyState(kCGEventSourceStateHIDSystemState, key) != false;
    }
    KeyHeldProbe held_ = &MSIMEModifierTap::physicallyHeld;
    std::set<unsigned short> keys_;
    NSEventModifierFlags modifiers_ = 0;
    NSEventModifierFlags armedModifier_ = 0;
    double pressedAt_ = 0;
    bool armed_ = false;
    bool shiftChordInterrupted_ = false;
};
