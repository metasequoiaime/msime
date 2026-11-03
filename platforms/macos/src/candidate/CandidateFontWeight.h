#pragma once
#import <AppKit/AppKit.h>

// 共享偏好 `candidate_font_weight`（client-core 的 `CandidateFontWeight`）在 macOS 上的字重。四档的 OpenType 数值是 400、500、600、700，这里换成 AppKit 对应的 `NSFontWeight` 常量；不认识的值按常规，与这个字段出现之前的画法相同。这份表抄自 Rust，scripts/test-candidate-font-weight-parity.py 核对它与 Rust 和另外两端一致。
inline NSArray<NSString *> *MSIMECandidateFontWeightIds() {
    return @[@"regular", @"medium", @"semibold", @"bold"];
}

inline NSFontWeight MSIMECandidateFontWeightValue(NSString *weight) {
    if ([weight isEqual:@"medium"]) return NSFontWeightMedium;
    if ([weight isEqual:@"semibold"]) return NSFontWeightSemibold;
    if ([weight isEqual:@"bold"]) return NSFontWeightBold;
    return NSFontWeightRegular;
}

// 预编辑行一直是半粗，候选文字选得更粗时跟到同一档（`CandidateFontWeight::preedit_value`）。
inline NSFontWeight MSIMECandidatePreeditFontWeightValue(NSString *weight) {
    return MAX(NSFontWeightSemibold, MSIMECandidateFontWeightValue(weight));
}
