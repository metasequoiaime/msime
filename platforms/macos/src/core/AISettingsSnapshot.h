#pragma once
#import <Foundation/Foundation.h>

// 这个窗口只管下面七个字段。服务商令牌、提示词方案和候选数量都不归它管，保存模型等无关字段时必须原样保留：macOS 的候选数量固定为 3，宿主不读存下的值，这里也不写它，免得把撤掉的设置又写回文档。
static inline NSDictionary *MSIMEAISettingsMerge(NSDictionary *preferences, NSDictionary *edits) {
    NSMutableDictionary *result = [preferences mutableCopy];
    id original = preferences[@"ai_assistant"];
    NSMutableDictionary *ai = [original isKindOfClass:NSDictionary.class]
        ? [original mutableCopy] : [NSMutableDictionary dictionary];
    for (NSString *key in @[
        @"enabled", @"provider", @"model", @"endpoint",
        @"prompt_custom_1", @"prompt_custom_2", @"prompt_custom_3"
    ]) {
        if (edits[key]) ai[key] = edits[key];
    }
    result[@"ai_assistant"] = ai;
    return result;
}
