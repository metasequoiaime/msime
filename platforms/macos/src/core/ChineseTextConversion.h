#pragma once
#import <Foundation/Foundation.h>
#include <stdint.h>
#ifdef __cplusplus
// `variant` 是 `msime::host_api::traditional_variant_code` 的编号：0 s2t、1 s2tw、2 s2twp。缺省 0，与只走标准繁体的旧调用兼容。
FOUNDATION_EXPORT NSString *MetasequoiaChineseOutputString(NSString *text, BOOL traditionalOutput,
                                                           uint32_t variant = 0);
#else
FOUNDATION_EXPORT NSString *MetasequoiaChineseOutputString(NSString *text, BOOL traditionalOutput,
                                                           uint32_t variant);
#endif
#define MSIMEChineseOutputString MetasequoiaChineseOutputString
// The first or last Han character of text, or nil when it has none. Non-Han characters are skipped and a non-BMP character is returned whole. The Han ranges are the reference server's IsHanCodePoint (candidate_text_policy.h), which word-to-character ([ ]) applies to the phrase after output conversion.
FOUNDATION_EXPORT NSString *MetasequoiaEdgeHanCharacter(NSString *text, BOOL first);
#define MSIMEEdgeHanCharacter MetasequoiaEdgeHanCharacter
