#import "../../src/core/ChineseTextConversion.h"

#import <Foundation/Foundation.h>

#include <stdexcept>

namespace
{
void require(bool condition, const char *message)
{
    if (!condition)
    {
        throw std::runtime_error(message);
    }
}
} // namespace

int main()
{
    @autoreleasepool
    {
        NSString *simplified = @"开发软件，后台里面";
        require([[MetasequoiaChineseOutputString(simplified, YES) description] isEqualToString:@"開發軟件，後臺裏面"],
                "Traditional output did not convert the visible Chinese text.");
        require(MetasequoiaChineseOutputString(simplified, NO) == simplified,
                "Simplified output unnecessarily copied or transformed its text.");
        require([MetasequoiaChineseOutputString(@"MSIME 123 😀", YES) isEqualToString:@"MSIME 123 😀"],
                "Traditional output changed non-Chinese text.");
        require([MetasequoiaChineseOutputString(@"", YES) isEqualToString:@""],
                "Traditional output did not preserve an empty string.");
        NSString *convertedSimplifiedGan = MetasequoiaChineseOutputString(@"干", YES);
        NSString *convertedTraditionalGan = MetasequoiaChineseOutputString(@"乾", YES);
        require([convertedSimplifiedGan isEqualToString:@"幹"] && [convertedTraditionalGan isEqualToString:@"乾"],
                "The OpenCC s2t 干/乾 fixture changed: standalone 干 is 幹 and 乾 stays 乾.");
        NSDictionary<NSString *, NSString *> *phrases = @{
            @"头发" : @"頭髮",
            @"蓬松" : @"蓬鬆",
            @"余下" : @"餘下",
            @"答复" : @"答覆",
            @"凭借" : @"憑藉",
        };
        for (NSString *source in phrases)
        {
            require([MetasequoiaChineseOutputString(source, YES) isEqualToString:phrases[source]],
                    "Traditional output did not use the OpenCC s2t phrase tables.");
        }
        // Mirrors the reference server's test_candidate_text_policy.cpp: word-to-character takes the first or last Han character, skipping everything else.
        require([MetasequoiaEdgeHanCharacter(@"中文", YES) isEqualToString:@"中"] &&
                    [MetasequoiaEdgeHanCharacter(@"中文", NO) isEqualToString:@"文"],
                "Edge extraction did not take the first and last Han characters.");
        require([MetasequoiaEdgeHanCharacter(@"a1中b2文c", YES) isEqualToString:@"中"] &&
                    [MetasequoiaEdgeHanCharacter(@"a1中b2文c", NO) isEqualToString:@"文"],
                "Edge extraction did not skip non-Han characters.");
        require([MetasequoiaEdgeHanCharacter(@"\U00020000x", YES) isEqualToString:@"\U00020000"] &&
                    [MetasequoiaEdgeHanCharacter(@"x中\U00020000", NO) isEqualToString:@"\U00020000"],
                "Edge extraction split a non-BMP Han character.");
        require([MetasequoiaEdgeHanCharacter(@"〇", YES) isEqualToString:@"〇"] &&
                    [MetasequoiaEdgeHanCharacter(@"\U000323AF", NO) isEqualToString:@"\U000323AF"],
                "Edge extraction missed a Han range the reference counts.");
        require(MetasequoiaEdgeHanCharacter(@"abc 123 😀，", YES) == nil && MetasequoiaEdgeHanCharacter(@"", NO) == nil &&
                    MetasequoiaEdgeHanCharacter(nil, YES) == nil,
                "Edge extraction returned a character from text without Han.");
        require([MetasequoiaEdgeHanCharacter(MetasequoiaChineseOutputString(@"头发", YES), NO) isEqualToString:@"髮"] &&
                    [MetasequoiaEdgeHanCharacter(MetasequoiaChineseOutputString(@"皇后", YES), NO) isEqualToString:@"后"],
                "Edge extraction over the converted phrase lost the phrase-level character.");
        require([MetasequoiaChineseOutputString(@"吃", YES, 0) isEqualToString:@"喫"] &&
                    [MetasequoiaChineseOutputString(@"吃", YES, 1) isEqualToString:@"吃"] &&
                    [MetasequoiaChineseOutputString(@"里面", YES, 0) isEqualToString:@"裏面"] &&
                    [MetasequoiaChineseOutputString(@"里面", YES, 1) isEqualToString:@"裡面"] &&
                    [MetasequoiaChineseOutputString(@"软件", YES, 1) isEqualToString:@"軟件"] &&
                    [MetasequoiaChineseOutputString(@"软件", YES, 2) isEqualToString:@"軟體"],
                "Traditional variants did not follow OpenCC s2t / s2tw / s2twp.");
        require([MetasequoiaChineseOutputString(@"软件", YES, 3) isEqualToString:@"软件"],
                "An unknown variant should keep the original text.");
    }
    return 0;
}
