#pragma once
#import <AppKit/AppKit.h>
#include "../../../../shared/voice/PolishPromptCatalog.h"
FOUNDATION_EXPORT NSNotificationName const MSIMEVoiceSettingsDidChangeNotification;

// 内置方案的 id、名称和顺序读取共享 Rust 静态目录；三个自定义槽位由宿主追加。
static inline NSArray<NSString *> *MSIMEPolishPromptIdentifiers(void)
{
    NSMutableArray<NSString *> *identifiers = [NSMutableArray arrayWithCapacity:msime::windows::kPolishPromptPresets.size() + 3];
    for (const auto &preset : msime::windows::kPolishPromptPresets)
        [identifiers addObject:[[NSString alloc] initWithBytes:preset.id.data() length:preset.id.size() encoding:NSUTF8StringEncoding]];
    [identifiers addObjectsFromArray:@[@"custom_1", @"custom_2", @"custom_3"]];
    return [identifiers copy];
}

static inline NSArray<NSString *> *MSIMEPolishPromptTitles(void)
{
    NSMutableArray<NSString *> *titles = [NSMutableArray arrayWithCapacity:msime::windows::kPolishPromptPresets.size() + 3];
    for (const auto &preset : msime::windows::kPolishPromptPresets)
        [titles addObject:[[NSString alloc] initWithBytes:preset.title.data() length:preset.title.size() encoding:NSUTF8StringEncoding]];
    [titles addObjectsFromArray:@[@"自定义一", @"自定义二", @"自定义三"]];
    return [titles copy];
}

static inline NSString *MSIMEPolishPromptIdentifierForIndex(NSInteger index)
{
    NSArray<NSString *> *identifiers = MSIMEPolishPromptIdentifiers();
    return index >= 0 && (NSUInteger)index < identifiers.count ? identifiers[(NSUInteger)index]
                                                               : identifiers.firstObject;
}

/// The default a preset identifier falls back to. An identifier from another build, or one an out-of-band edit left behind, is not a preset this client can show, and selecting nothing in the popup would write the first item back the next time anything else on the card changed.
static inline NSString *MSIMEPolishPromptIdentifierOrDefault(NSString *identifier)
{
    return [MSIMEPolishPromptIdentifiers() containsObject:identifier ?: @""] ? identifier
                                                                             : MSIMEPolishPromptIdentifiers().firstObject;
}

