#pragma once
#import <Foundation/Foundation.h>

// Pure merge: no window controller/defaults reads on the persistence worker.
// Nested overrides own only their named fields; preserve other host settings.
static inline NSDictionary *MSIMEMergePreferenceSnapshot(NSDictionary *base, NSDictionary *overrides) {
    if (![base isKindOfClass:NSDictionary.class] || ![overrides isKindOfClass:NSDictionary.class]) return nil;
    NSMutableDictionary *merged = [base mutableCopy];
    for (NSString *key in overrides) {
        id value = overrides[key];
        // 应用例外是一张以应用标识为键的表，宿主写来的就是整张表：逐项合并的话，移除的规则会一直留在文档里。
        if ([value isKindOfClass:NSDictionary.class] && ![key isEqual:@"app_input_mode_rules"]) {
            id original = base[key] ?: @{};
            NSDictionary *nested = MSIMEMergePreferenceSnapshot(original, value);
            if (!nested) return nil;
            merged[key] = nested;
        } else {
            merged[key] = value;
        }
    }
    return [merged copy];
}
