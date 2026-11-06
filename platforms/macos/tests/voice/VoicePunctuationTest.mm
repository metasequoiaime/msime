#import "../../src/voice/VoicePunctuation.h"
#include <cassert>

int main() {
    @autoreleasepool {
        // 缺键、空串、非字符串都按出厂默认 auto；不认识的取值原样保留，交给共享层按 auto 处理。
        assert([MSIMEVoicePunctuationMode(nil) isEqual:@"auto"]);
        assert([MSIMEVoicePunctuationMode(@{}) isEqual:@"auto"]);
        assert([MSIMEVoicePunctuationMode(@{@"punctuation_mode" : @""}) isEqual:@"auto"]);
        assert([MSIMEVoicePunctuationMode(@{@"punctuation_mode" : @1}) isEqual:@"auto"]);
        assert([MSIMEVoicePunctuationMode(@[]) isEqual:@"auto"]);
        assert([MSIMEVoicePunctuationMode(@{@"punctuation_mode" : @"omit_all"}) isEqual:@"omit_all"]);
        assert([MSIMEVoicePunctuationMode(@{@"punctuation_mode" : @"from_a_newer_version"}) isEqual:@"from_a_newer_version"]);

        NSString *text = @"天啊！他是谁？";
        assert([MSIMEVoicePunctuated(text, @"auto") isEqual:text]);
        assert([MSIMEVoicePunctuated(text, @"omit_trailing") isEqual:@"天啊！他是谁"]);
        assert([MSIMEVoicePunctuated(text, @"omit_all") isEqual:@"天啊他是谁"]);
        assert([MSIMEVoicePunctuated(text, @"space") isEqual:@"天啊 他是谁"]);
        assert([MSIMEVoicePunctuated(text, @"from_a_newer_version") isEqual:text]);
        assert(MSIMEVoicePunctuated(@"", @"omit_all").length == 0);
        // 整段只有标点时处理后为空，由调用方按没有识别到文字提示。
        assert(MSIMEVoicePunctuated(@"。", @"omit_trailing").length == 0);
        // 共享层拒绝的请求（超过文本上限）保留原文。
        NSString *tooLong = [@"" stringByPaddingToLength:10001 withString:@"好" startingAtIndex:0];
        assert([MSIMEVoicePunctuated(tooLong, @"omit_all") isEqual:tooLong]);
    }
    return 0;
}
