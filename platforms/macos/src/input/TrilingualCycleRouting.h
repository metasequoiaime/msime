#pragma once
#import <Foundation/Foundation.h>

// 中日英循环切换在 macOS 这一侧只做两件事：把控制器的状态翻译成
// `msime_client_language_cycle` 的入参，把它的答复翻译回控制器要执行的动作。
// 循环顺序本身不在这里：它在 `crates/client-core/src/language_cycle.rs`，
// 由四个宿主共用同一份（目前只有 macOS 接上，见 `.agents/notes/implemented/feature/2026-10-21-trilingual-cycle.md`）。
namespace msime::mac::cycle
{
// `msime_client_language_cycle` 的 mode：0 中文、1 日文、2 英文。
enum class Mode
{
    Chinese = 0,
    Japanese = 1,
    English = 2,
};

// 按下循环键那一刻的模式。英文是「键交给应用」那一态，优先于方案；日语方案是日文。
// 其它方案（韩文、越南文、藏文等）都算中文那一态：它们是中文宿主里的外语方案，
// Engine 仍会组字，循环键先把组好的原文上屏再走。
constexpr int ModeCode(bool english, bool japanese)
{
    return english ? static_cast<int>(Mode::English) : japanese ? static_cast<int>(Mode::Japanese) : static_cast<int>(Mode::Chinese);
}

// `Preferences::last_chinese_scheme` 的方案名，换成 `--` 入口接受的编号；不是中文方案时 -1
// （还没记下回去的路）。编号与 `InputScheme` 的 Engine 序号一致。
static inline int8_t LastChineseSchemeCode(const char *scheme)
{
    if (scheme == nullptr)
        return -1;
    NSString *name = @(scheme);
    NSArray<NSString *> *chinese = @[ @"quanpin", @"shuangpin", @"wubi", @"cantonese", @"zhuyin", @"stroke" ];
    NSArray<NSNumber *> *codes = @[ @0, @1, @2, @5, @6, @9 ];
    const NSUInteger index = [chinese indexOfObject:name];
    return index == NSNotFound ? -1 : codes[index].charValue;
}

// 一次循环的答复。`claimed` 为假表示这一键不循环，照旧按二态切换处理。
struct Reply
{
    bool claimed = false;
    int mode = 0;
    // -1 表示只换中英模式，不换方案。
    int scheme = -1;
    bool commitRaw = false;
};

// 解析 `msime_client_language_cycle` 的 `value`：null 是不接管，字典是 {mode, scheme, commit_raw}。
// 形状不对时按不接管处理，控制器退回二态切换，绝不按猜出来的目标换模式。
inline Reply ParseReply(id value)
{
    Reply reply;
    if (![value isKindOfClass:NSDictionary.class])
        return reply;
    NSDictionary *document = value;
    id mode = document[@"mode"];
    id scheme = document[@"scheme"];
    id commitRaw = document[@"commit_raw"];
    if (![mode isKindOfClass:NSNumber.class] || ![commitRaw isKindOfClass:NSNumber.class])
        return reply;
    const int modeCode = [mode intValue];
    if (modeCode < 0 || modeCode > 2)
        return reply;
    reply.claimed = true;
    reply.mode = modeCode;
    reply.commitRaw = [commitRaw boolValue];
    if ([scheme isKindOfClass:NSNumber.class])
        reply.scheme = [scheme intValue];
    else if (scheme != nil && ![scheme isKindOfClass:NSNull.class])
        reply.claimed = false;
    return reply;
}
} // namespace msime::mac::cycle
