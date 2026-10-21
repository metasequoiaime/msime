#import "../../src/input/TrilingualCycleRouting.h"
#include <cassert>

// 只测这一侧的翻译：循环顺序本身在 Rust（crates/client-core/src/language_cycle.rs）里，
// 由那边的单测覆盖；这里覆盖「控制器状态 → 入口参数」和「答复 → 动作」两处转换。
int main() {
    using namespace msime::mac::cycle;

    // 模式：英文优先于方案；日语方案是日文；其它外语方案都算中文那一态。
    assert(ModeCode(false, false) == 0);
    assert(ModeCode(false, true) == 1);
    assert(ModeCode(true, false) == 2);
    assert(ModeCode(true, true) == 2);

    // 回去的中文方案：只有中文方案有编号，其余（含没记下）-1。
    assert(LastChineseSchemeCode("quanpin") == 0);
    assert(LastChineseSchemeCode("shuangpin") == 1);
    assert(LastChineseSchemeCode("wubi") == 2);
    assert(LastChineseSchemeCode("cantonese") == 5);
    assert(LastChineseSchemeCode("zhuyin") == 6);
    assert(LastChineseSchemeCode("stroke") == 9);
    assert(LastChineseSchemeCode("japanese") == -1);
    assert(LastChineseSchemeCode("korean") == -1);
    assert(LastChineseSchemeCode(nullptr) == -1);

    // null 是不接管：开关关着、英文模式下或专用英文候选开着。
    assert(!ParseReply(nil).claimed);
    assert(!ParseReply(NSNull.null).claimed);
    // 形状不对也按不接管：绝不按猜出来的目标换模式。
    assert(!ParseReply(@{@"mode": @1}).claimed);
    assert(!ParseReply(@{@"mode": @7, @"scheme": NSNull.null, @"commit_raw": @NO}).claimed);
    assert(!ParseReply(@{@"mode": @1, @"scheme": @"wubi", @"commit_raw": @NO}).claimed);
    assert(!ParseReply(@{@"mode": @"1", @"scheme": NSNull.null, @"commit_raw": @NO}).claimed);

    // 中 → 日：带日语方案，没组字时不上屏。
    const Reply japanese = ParseReply(@{@"mode": @1, @"scheme": @3, @"commit_raw": @NO});
    assert(japanese.claimed && japanese.mode == 1 && japanese.scheme == 3 && !japanese.commitRaw);
    // 有组字时先上屏。
    const Reply composing = ParseReply(@{@"mode": @1, @"scheme": @3, @"commit_raw": @YES});
    assert(composing.claimed && composing.commitRaw);
    // 日 → 英：scheme 是 null，只换中英模式。
    const Reply english = ParseReply(@{@"mode": @2, @"scheme": NSNull.null, @"commit_raw": @NO});
    assert(english.claimed && english.mode == 2 && english.scheme == -1);
    // 英 → 中：回到记下的中文方案。
    const Reply chinese = ParseReply(@{@"mode": @0, @"scheme": @1, @"commit_raw": @NO});
    assert(chinese.claimed && chinese.mode == 0 && chinese.scheme == 1);
    return 0;
}
