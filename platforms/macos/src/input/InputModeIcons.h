#pragma once
#import <Foundation/Foundation.h>
#import <CoreFoundation/CoreFoundation.h>
#include "InputModeIdentifiers.h"

// 只替换公开 IMK modes: 字典里的图标引用；静态注册信息、模式名称、排序和状态项均不改。
// 系统切换条是否读取动态字典不属于这个转换函数的保证。
static inline NSDictionary *MSIMEInputModesWithGlyphIcons(id modes, NSDictionary *info, id preferences,
                                                        BOOL (^resourceExists)(NSString *)) {
    if (![modes isKindOfClass:NSDictionary.class]) return nil;
    id enabled = [preferences isKindOfClass:NSDictionary.class] ? preferences[@"input_mode_glyph_icons"] : nil;
    if (!enabled || CFGetTypeID((__bridge CFTypeRef)enabled) != CFBooleanGetTypeID() || ![enabled boolValue]) return modes;
    id list = modes[@"tsInputModeListKey"];
    if (![list isKindOfClass:NSDictionary.class] || !resourceExists) return modes;

    NSMutableDictionary *updatedList = [list mutableCopy];
    NSString *prefix = [MSIMEInputMethodBundleIdentifierIn(info) stringByAppendingString:@"."];
    for (id identifier in list) {
        if (![identifier isKindOfClass:NSString.class] || ![identifier hasPrefix:prefix]) continue;
        id entry = list[identifier];
        if (![entry isKindOfClass:NSDictionary.class]) continue;
        NSString *suffix = [identifier substringFromIndex:prefix.length];
        if ([suffix containsString:@"."]) continue;
        NSString *glyphID = identifier;
        // 单方案版复用 .Hans 注册主模式，其字形取版本方案，而不是固定的「中」。
        if (!MSIMEEditionIsFullIn(info) && [suffix isEqualToString:@"Hans"]) {
            NSString *scheme = MSIMEEditionDefaultSchemeIn(info);
            if (![MSIMEInputSchemeNames() containsObject:scheme]) continue;
            glyphID = MSIMEInputModeID(MSIMEInputModeFor(NO, scheme));
        }
        NSString *file = MSIMEInputModeGlyphFile(glyphID);
        if (!file || !resourceExists(file)) continue;
        NSMutableDictionary *updatedEntry = [entry mutableCopy];
        updatedEntry[@"tsInputModeMenuIconFileKey"] = file;
        updatedEntry[@"tsInputModePaletteIconFileKey"] = file;
        updatedList[identifier] = updatedEntry;
    }
    NSMutableDictionary *updated = [modes mutableCopy];
    updated[@"tsInputModeListKey"] = updatedList;
    return updated;
}
