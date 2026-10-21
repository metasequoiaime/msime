// 语音快捷键的出厂回退值：本机旧键和共享文档都没给值时，原生窗口读到的值必须与共享
// `VoiceInputPreferences::default()` 一致。三个按住录音的组合键里只有 `hotkey_ralt`（按住右
// Option）默认打开，另外两个默认关；识别语言回退共享的 BCP 47 区域形式 `zh-CN`，小写旧值也按
// 它显示，比较不因大小写失配。只读 getter，不碰 `-window`，所以没有图形会话也能跑。
#import "../../src/settings/AppearancePreferences.h"
#import "TestPreferenceSuite.h"
#include <cstdio>

static int failures = 0;

static void expectBool(BOOL actual, BOOL expected, const char *what) {
    if (actual == expected) return;
    std::fprintf(stderr, "%s: 期望 %s，实际 %s\n", what, expected ? "开" : "关", actual ? "开" : "关");
    ++failures;
}

static void expectString(NSString *actual, NSString *expected, const char *what) {
    if ([actual isEqualToString:expected]) return;
    std::fprintf(stderr, "%s: 期望 %s，实际 %s\n", what, expected.UTF8String,
                 actual.UTF8String ?: "(nil)");
    ++failures;
}

int main() {
    @autoreleasepool {
        NSString *suite = [@"msime.voice-hold-defaults." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        NSString *root =
            [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        MSIMEAppearancePreferences *prefs =
            [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
                                                       skinsRoot:[NSURL fileURLWithPath:root]];
        expectBool(prefs.voiceHotkeyRightAlt, YES, "按住右 Option 录音回退");
        expectBool(prefs.voiceHotkeyCtrlOption, NO, "按住右 Control+Option 录音回退");
        expectBool(prefs.voiceHotkeyCtrlCommand, NO, "按住 Control+Command 录音回退");
        // 系统识别器写区域形式；存着短形式时也按 zh-CN 显示，不因大小写失配。
        [defaults setObject:@"zh-cn" forKey:@"MSIMEClientVoiceLanguage"];
        expectString(prefs.voiceLanguage, @"zh-CN", "识别语言回退");
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    return failures == 0 ? 0 : 1;
}
