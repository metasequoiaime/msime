#import <Foundation/Foundation.h>
#include "../../src/input/InputModeIcons.h"
#include <cassert>
#include <cstdio>

static void require(bool condition, const char *message) {
    if (!condition) { std::fprintf(stderr, "%s\n", message); std::abort(); }
}

int main() {
    @autoreleasepool {
        NSString *root = @MSIME_MACOS_SOURCE_DIR;
        NSString *source = [NSString stringWithContentsOfFile:[root stringByAppendingPathComponent:@"Info.plist.in"]
                                                   encoding:NSUTF8StringEncoding error:nil];
        for (NSString *placeholder in @[@"${MSIME_INPUT_SOURCE_REVISION_PLIST}", @"${MSIME_SPARKLE_PLIST_KEYS}"])
            source = [source stringByReplacingOccurrencesOfString:placeholder withString:@""];
        NSDictionary *info = [NSPropertyListSerialization propertyListWithData:[source dataUsingEncoding:NSUTF8StringEncoding]
                                                                     options:NSPropertyListImmutable format:nullptr error:nil];
        require(info != nil, "实际 Info.plist 模板解析失败");
        // 按当前静态注册格式解析，不把 SDK 的其他模式键当成同一契约。
        NSDictionary *modes = info[@"ComponentInputModeDict"];
        require([modes isKindOfClass:NSDictionary.class], "实际 bundle 的模式字典缺失");
        NSDictionary *before = [modes copy];
        NSBundle *resources = [NSBundle bundleWithPath:[root stringByAppendingPathComponent:@"resources"]];
        BOOL (^exists)(NSString *) = ^BOOL(NSString *file) { return [resources pathForResource:file ofType:nil] != nil; };
        NSDictionary *on = @{@"input_mode_glyph_icons": @YES};

        require(MSIMEInputModesWithGlyphIcons(modes, info, @{}, exists) == modes, "缺字段改变了品牌默认值");
        for (id invalid in @[@NO, @1, @"true", NSNull.null, @[], @{}])
            require(MSIMEInputModesWithGlyphIcons(modes, info, @{@"input_mode_glyph_icons": invalid}, exists) == modes,
                    "非 JSON true 选择了字形图标");
        NSDictionary *changed = MSIMEInputModesWithGlyphIcons(modes, info, on, exists);
        NSDictionary *names = @{@"Hans": @"Chinese", @"Shuangpin": @"Shuangpin", @"Wubi": @"Wubi", @"Cantonese": @"Cantonese",
            @"Zhuyin": @"Zhuyin", @"Japanese": @"Japanese", @"Korean": @"Korean", @"Vietnamese": @"Vietnamese",
            @"Tibetan": @"Tibetan", @"Stroke": @"Stroke", @"Roman": @"English"};
        require([changed[@"tsInputModeListKey"] count] == names.count, "模式数改变或测试漏了模式");
        for (NSString *suffix in names) {
            NSString *identifier = [MSIMEFullInputMethodBundleIdentifier stringByAppendingFormat:@".%@", suffix];
            NSString *file = [NSString stringWithFormat:@"MSIMEClientInputMethodMenuIcon%@.tiff", names[suffix]];
            require(exists(file), "实际字形资源缺失");
            NSMutableDictionary *expected = [modes[@"tsInputModeListKey"][identifier] mutableCopy];
            expected[@"tsInputModeMenuIconFileKey"] = file;
            expected[@"tsInputModePaletteIconFileKey"] = file;
            require([changed[@"tsInputModeListKey"][identifier] isEqual:expected], "模式内容除图标以外发生变化");
        }
        NSMutableDictionary *expectedModes = [modes mutableCopy];
        expectedModes[@"tsInputModeListKey"] = changed[@"tsInputModeListKey"];
        require([changed isEqual:expectedModes] && [modes isEqual:before], "顺序、其他字段或原字典被改变");

        NSDictionary *schemeFiles = @{@"quanpin": @"Chinese", @"shuangpin": @"Shuangpin", @"wubi": @"Wubi",
            @"japanese": @"Japanese", @"korean": @"Korean", @"cantonese": @"Cantonese", @"zhuyin": @"Zhuyin",
            @"vietnamese": @"Vietnamese", @"tibetan": @"Tibetan", @"stroke": @"Stroke"};
        for (NSString *scheme in schemeFiles) {
            NSString *bundle = [@"app.test.synthetic." stringByAppendingString:scheme];
            NSString *main = [bundle stringByAppendingString:@".Hans"];
            NSString *roman = [bundle stringByAppendingString:@".Roman"];
            NSDictionary *edition = @{@"MSIMEEdition": scheme, @"CFBundleIdentifier": bundle, @"MSIMEDefaultScheme": scheme};
            NSDictionary *entries = @{@"tsInputModeListKey": @{main: @{@"name": @"合成主模式"}, roman: @{@"name": @"合成英文"}},
                                      @"tsVisibleInputModeOrderedArrayKey": @[main, roman], @"unknown": @42};
            NSDictionary *result = MSIMEInputModesWithGlyphIcons(entries, edition, on, exists);
            require([result[@"tsInputModeListKey"][main][@"tsInputModeMenuIconFileKey"] isEqualToString:
                     [NSString stringWithFormat:@"MSIMEClientInputMethodMenuIcon%@.tiff", schemeFiles[scheme]]],
                    "单方案版主模式的字形与实际方案不符");
            require([result[@"tsInputModeListKey"][roman][@"tsInputModePaletteIconFileKey"] isEqualToString:
                     @"MSIMEClientInputMethodMenuIconEnglish.tiff"], "单方案英文图标不正确");
            require([result[@"tsVisibleInputModeOrderedArrayKey"] isEqual:entries[@"tsVisibleInputModeOrderedArrayKey"]] &&
                    [result[@"unknown"] isEqual:@42], "单方案版的顺序或扩展字段丢失");
        }
        NSString *unknown = [MSIMEFullInputMethodBundleIdentifier stringByAppendingString:@".Future"];
        NSString *nested = [MSIMEFullInputMethodBundleIdentifier stringByAppendingString:@".Future.Hans"];
        NSDictionary *extensions = @{@"tsInputModeListKey": @{
            unknown: @{@"future": @1}, nested: @{@"nested": @1}, @"app.other.Hans": @{@"foreign": @1},
            MSIMEChineseInputModeID: @{@"tsInputModeMenuIconFileKey": @"original.tiff", @"extra": @42},
            MSIMEEnglishInputModeID: NSNull.null}};
        NSDictionary *updated = MSIMEInputModesWithGlyphIcons(extensions, info, on, exists);
        for (NSString *identifier in @[unknown, nested, @"app.other.Hans", MSIMEEnglishInputModeID])
            require([updated[@"tsInputModeListKey"][identifier] isEqual:extensions[@"tsInputModeListKey"][identifier]],
                    "未知、外部或无效模式被改写");
        require([updated[@"tsInputModeListKey"][MSIMEChineseInputModeID][@"extra"] isEqual:@42], "模式扩展字段丢失");
        require([MSIMEInputModesWithGlyphIcons(extensions, info, on, ^BOOL(NSString *) { return NO; }) isEqual:extensions],
                "缺资源时没有保留原图标");
        require([MSIMEInputModesWithGlyphIcons(@{@"tsInputModeListKey": @[]}, info, on, exists)
                 isEqual:@{@"tsInputModeListKey": @[]}], "无效列表没有安全回退");
        require(MSIMEInputModesWithGlyphIcons(nil, info, on, exists) == nil &&
                MSIMEInputModesWithGlyphIcons(@42, info, on, exists) == nil, "无效模式字典没有安全回退");
        require(MSIMEInputModesWithGlyphIcons(modes, info, @[], exists) == modes &&
                MSIMEInputModesWithGlyphIcons(modes, info, on, nil) == modes, "缺少配置或资源解析器时没有回退");
        NSDictionary *futureEdition = @{@"MSIMEEdition": @"synthetic", @"MSIMEDefaultScheme": @"future"};
        NSDictionary *futureModes = @{@"tsInputModeListKey": @{MSIMEChineseInputModeID: @{@"original": @42}}};
        require([MSIMEInputModesWithGlyphIcons(futureModes, futureEdition, on, exists) isEqual:futureModes],
                "未知版本方案被错误标成全拼字形");
        std::puts("input-mode-icons: 11 个模式、10 个版本及兼容回退通过");
    }
}
