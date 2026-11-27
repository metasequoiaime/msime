#import "../../src/core/LeftOptionHold.h"
#include <cassert>
#include <cmath>

// #11859：长按左 Option 打开符号面板的手势判定。只驱动状态机，不需要图形会话。
int main() {
    @autoreleasepool {
        using E = MSIMELeftOptionHold::Event;
        const NSEventModifierFlags lo = NSEventModifierFlagOption | NX_DEVICELALTKEYMASK;
        const NSEventModifierFlags ro = NSEventModifierFlagOption | NX_DEVICERALTKEYMASK;
        const NSEventModifierFlags shift = NSEventModifierFlagShift | NX_DEVICELSHIFTKEYMASK;
        const NSEventModifierFlags command = NSEventModifierFlagCommand | NX_DEVICELCMDKEYMASK;
        MSIMELeftOptionHold hold;
        auto press = [&](double time, NSEventModifierFlags extra = 0) {
            return hold.observe(E::FlagsChanged, 58, lo | extra, time, true);
        };
        auto release = [&](double time, NSEventModifierFlags remaining = 0) {
            return hold.observe(E::FlagsChanged, 58, remaining, time, true);
        };

        // 按满 400 毫秒：按下交给应用，到期打开，打开后吞掉这次松开，下一次松开不再吞。
        assert(!press(10.0));
        assert(hold.pending() && std::fabs(hold.deadline() - 10.4) < 1e-9);
        assert(!hold.due(10.39, lo)); // 还没到。
        assert(hold.pending());
        assert(hold.due(hold.deadline(), lo));
        hold.didOpen(true);
        assert(hold.opened() && !hold.pending());
        assert(release(10.6));
        assert(!hold.opened());
        assert(!release(10.7)); // 重复的松开快照不再吞。

        // 短按：到期前松开，交给应用，之后到期也不打开。
        assert(!press(20.0));
        assert(!release(20.2));
        assert(!hold.pending() && !hold.due(20.5, lo));

        // 面板没打开（没有焦点客户端、宿主拒绝）：松开照常交给应用。
        assert(!press(30.0));
        assert(hold.due(30.5, lo));
        hold.didOpen(false);
        assert(!hold.opened());
        assert(!release(30.6));

        // 按住期间按了别的键：组合键，不打开，松开也不吞。KeyUp（按住前就按着的键这时抬起）同样算组合。
        assert(!press(40.0));
        assert(!hold.observe(E::KeyDown, 0, lo, 40.1, true));
        assert(!hold.pending() && !hold.due(40.5, lo));
        assert(!release(40.6));
        assert(!press(41.0));
        assert(!hold.observe(E::KeyUp, 0, lo, 41.1, true));
        assert(!hold.due(41.5, lo));
        assert(!release(41.6));

        // 按住期间加了别的修饰键：组合键。别的修饰键先松开、左 Option 继续按满也不打开。
        assert(!press(50.0));
        assert(!hold.observe(E::FlagsChanged, 56, lo | shift, 50.1, true));
        assert(!hold.observe(E::FlagsChanged, 56, lo, 50.2, true));
        assert(!hold.due(50.6, lo));
        assert(!release(50.7));

        // 带着别的修饰键按下左 Option，或者右 Option 已经按着：从一开始就是组合键。
        assert(!press(60.0, command));
        assert(!hold.pending() && !hold.due(60.5, lo | command));
        assert(!release(60.6, command));
        assert(!press(61.0, ro));
        assert(!hold.pending());
        assert(!release(61.6, ro));

        // 右 Option 自己从不触发：它的事件不是 keyCode 58。
        assert(!hold.observe(E::FlagsChanged, 61, ro, 70.0, true));
        assert(!hold.pending() && !hold.due(70.5, ro));
        assert(!hold.observe(E::FlagsChanged, 61, 0, 70.6, true));

        // 只有通用 Option 位、没有侧别位的 keyCode 58 事件按左 Option 处理。
        assert(!hold.observe(E::FlagsChanged, 58, NSEventModifierFlagOption, 80.0, true));
        assert(hold.pending());
        assert(hold.due(hold.deadline(), NSEventModifierFlagOption));
        hold.didOpen(true);
        assert(release(80.5));

        // 大写锁定开着不影响。
        assert(!press(85.0, NSEventModifierFlagCapsLock));
        assert(hold.due(hold.deadline(), lo | NSEventModifierFlagCapsLock));
        hold.didOpen(true);
        assert(release(85.5, NSEventModifierFlagCapsLock));

        // 同一次按住的重复按下快照不重新计时。
        assert(!press(90.0));
        assert(!press(90.3));
        assert(std::fabs(hold.deadline() - 90.4) < 1e-9);
        assert(hold.due(hold.deadline(), lo));
        hold.didOpen(false);
        assert(!release(90.5));

        // 松开没送到：到期时会话里左 Option 已经抬起，不打开；迟到的松开交给应用。
        assert(!press(100.0));
        assert(!hold.due(100.5, 0));
        assert(!hold.pending());
        assert(!release(100.6));
        // 到期时会话里多了别的修饰键（按下事件没送到这里）：组合键。
        assert(!press(101.0));
        assert(!hold.due(101.5, lo | shift));
        assert(!release(101.6, shift));

        // 鼠标按下（Option+点按、拖动）取消这次按住。
        assert(!press(110.0));
        hold.cancel();
        assert(!hold.due(110.5, lo));
        assert(!release(110.6));

        // 面板打开后焦点去了面板，松开没送到；下一次按下是新的一次，照常计时。
        assert(!press(120.0));
        assert(hold.due(hold.deadline(), lo));
        hold.didOpen(true);
        assert(!press(125.0));
        assert(hold.pending() && std::fabs(hold.deadline() - 125.4) < 1e-9);
        assert(!release(125.1));
        assert(!hold.opened());

        // 左 Option 的松开随别的修饰键事件反映出来：按住结束，这个事件不吞，之后的左 Option 松开快照也不吞。
        assert(!press(130.0));
        assert(hold.due(hold.deadline(), lo));
        hold.didOpen(true);
        assert(!hold.observe(E::FlagsChanged, 56, shift, 130.5, true));
        assert(!hold.opened());
        assert(!release(130.6));

        // 开关关着或安全输入期间：什么都不做，事件全部交给应用，已经在等的按住作废。
        assert(!hold.observe(E::FlagsChanged, 58, lo, 140.0, false));
        assert(!hold.pending() && !hold.due(140.5, lo));
        assert(!hold.observe(E::FlagsChanged, 58, 0, 140.6, false));
        assert(!press(141.0));
        assert(!hold.observe(E::FlagsChanged, 58, lo, 141.1, false)); // 中途进入安全输入。
        assert(!hold.due(141.5, lo));
        assert(!release(141.6));
        assert(!press(142.0));
        assert(hold.due(hold.deadline(), lo));
        hold.didOpen(true);
        assert(!hold.observe(E::FlagsChanged, 58, 0, 142.5, false)); // 打开后开关被关掉，松开也交给应用。

        // reset（焦点切换）丢掉一切状态。
        assert(!press(150.0));
        hold.reset();
        assert(!hold.pending() && std::isnan(hold.deadline()));
    }
    return 0;
}
