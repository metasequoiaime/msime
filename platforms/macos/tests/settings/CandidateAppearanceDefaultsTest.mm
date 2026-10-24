// 候选窗外观的出厂回退值：本机旧键和共享文档都没给值时，原生窗口读到的值必须与共享
// `Preferences::default()` 一致。两条曾经漂移过：
// - 主字体回退 `Noto Sans SC`（共享默认），不是 Windows 的 `Segoe UI`；
// - 预编辑字号回退 15（共享与 Windows 都是 15），不是 16。
// 只构造偏好对象、只读 getter，不碰 `-window`，所以没有图形会话也能跑。
#import "../../src/settings/AppearancePreferences.h"
#import "TestPreferenceSuite.h"
#include <cstdio>

static int failures = 0;

static void expectString(NSString *actual, NSString *expected, const char *what) {
    if ([actual isEqualToString:expected]) return;
    std::fprintf(stderr, "%s: 期望 %s，实际 %s\n", what, expected.UTF8String,
                 actual.UTF8String ?: "(nil)");
    ++failures;
}

static void expectUnsigned(NSUInteger actual, NSUInteger expected, const char *what) {
    if (actual == expected) return;
    std::fprintf(stderr, "%s: 期望 %lu，实际 %lu\n", what, (unsigned long)expected,
                 (unsigned long)actual);
    ++failures;
}

int main() {
    @autoreleasepool {
        NSString *suite = [@"msime.candidate-appearance-defaults." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        NSString *root =
            [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        MSIMEAppearancePreferences *prefs =
            [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
                                                       skinsRoot:[NSURL fileURLWithPath:root]];
        expectString(prefs.fontFamily, @"Noto Sans SC", "候选主字体回退");
        expectUnsigned(prefs.preeditFontSize, 15, "预编辑字号回退");
        expectUnsigned(prefs.translationFontSize, 12, "译文字号回退");
        for (NSNumber *size in @[@12, @21, @32]) {
            [prefs applySharedCandidatePreferences:@{@"candidate_translation_font_size": size}];
            expectUnsigned(prefs.translationFontSize, size.unsignedIntegerValue, "共享译文字号");
        }
        [prefs applySharedCandidatePreferences:@{@"candidate_translation_font_size": @33}];
        expectUnsigned(prefs.translationFontSize, 32, "无效字号不覆盖已加载值");
        [prefs applySharedCandidatePreferences:@{@"candidate_translation_font_size": NSNull.null}];
        expectUnsigned(prefs.translationFontSize, 12, "null 恢复默认译文字号");
        [prefs applySharedCandidatePreferences:@{@"candidate_translation_font_size": @21}];
        [prefs applySharedCandidatePreferences:@{}];
        expectUnsigned(prefs.translationFontSize, 12, "省略字段恢复默认译文字号");
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    return failures == 0 ? 0 : 1;
}
