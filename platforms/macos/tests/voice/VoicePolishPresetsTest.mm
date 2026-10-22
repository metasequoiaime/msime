// 选择表读取共享目录，索引和未知方案继续使用宿主原有回退。
#import "../../src/voice/VoiceSettings.h"
#include "../../../../shared/voice/PolishPromptCatalog.h"
#include <cstdio>
#include <cstdlib>

static void Require(bool value, const char *message)
{
    if (!value)
    {
        std::fprintf(stderr, "%s\n", message);
        std::exit(1);
    }
}

int main()
{
    @autoreleasepool
    {
        NSArray<NSString *> *identifiers = MSIMEPolishPromptIdentifiers();
        NSArray<NSString *> *titles = MSIMEPolishPromptTitles();
        NSArray<NSString *> *expectedIdentifiers = @[@"cleanup", @"faithful", @"zh2en", @"casual", @"custom_1", @"custom_2", @"custom_3"];
        NSArray<NSString *> *expectedTitles = @[@"精炼整理", @"忠实校对", @"中翻英", @"口语整理", @"自定义一", @"自定义二", @"自定义三"];
        Require([identifiers isEqualToArray:expectedIdentifiers], "七项方案 id 和顺序保持既有值");
        Require([titles isEqualToArray:expectedTitles], "七项方案名称和顺序保持既有值");
        NSUInteger index = 0;
        for (const auto &preset : msime::windows::kPolishPromptPresets)
        {
            NSString *identifier = [[NSString alloc] initWithBytes:preset.id.data() length:preset.id.size() encoding:NSUTF8StringEncoding];
            NSString *title = [[NSString alloc] initWithBytes:preset.title.data() length:preset.title.size() encoding:NSUTF8StringEncoding];
            Require([identifiers[index] isEqualToString:identifier], "内置方案 id 必须读取共享目录");
            Require([titles[index] isEqualToString:title], "内置方案名称必须读取共享目录");
            ++index;
        }
        for (NSUInteger position = 0; position < identifiers.count; ++position)
        {
            Require([MSIMEPolishPromptIdentifierForIndex((NSInteger)position) isEqualToString:identifiers[position]],
                    "有效下标映射到原方案 id");
            Require([MSIMEPolishPromptIdentifierOrDefault(identifiers[position]) isEqualToString:identifiers[position]],
                    "已知内置和自定义方案均原样读回");
        }
        for (NSInteger invalid : {(NSInteger)-1, (NSInteger)identifiers.count, NSIntegerMax})
            Require([MSIMEPolishPromptIdentifierForIndex(invalid) isEqualToString:@"cleanup"], "越界下标回退到第一项");
        for (NSString *unknown in @[@"", @"synthetic-unknown"])
            Require([MSIMEPolishPromptIdentifierOrDefault(unknown) isEqualToString:@"cleanup"], "未知方案回退到第一项");
        Require([MSIMEPolishPromptIdentifierOrDefault(nil) isEqualToString:@"cleanup"], "缺失方案回退到第一项");
    }
    return 0;
}
