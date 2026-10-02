#pragma once
#import <Foundation/Foundation.h>

// This page owns only these eight fields. Provider tokens, prompt selection and
// custom slots must survive saving an unrelated model or candidate limit.
static inline NSDictionary *MSIMEAISettingsMerge(NSDictionary *preferences, NSDictionary *edits) {
    NSMutableDictionary *result = [preferences mutableCopy];
    id original = preferences[@"ai_assistant"];
    NSMutableDictionary *ai = [original isKindOfClass:NSDictionary.class]
        ? [original mutableCopy] : [NSMutableDictionary dictionary];
    for (NSString *key in @[
        @"enabled", @"provider", @"model", @"endpoint", @"candidate_limit",
        @"prompt_custom_1", @"prompt_custom_2", @"prompt_custom_3"
    ]) {
        if (edits[key]) ai[key] = edits[key];
    }
    result[@"ai_assistant"] = ai;
    return result;
}
