#import "../../src/core/AISettingsSnapshot.h"
#include <cassert>

int main() {
    @autoreleasepool {
        NSDictionary *originalAI = @{
            @"enabled": @YES, @"provider": @"openai", @"model": @"old-model",
            @"candidate_limit": @3,
            @"token": @"synthetic-legacy", @"token_openai": @"synthetic-openai",
            @"token_deepseek": @"synthetic-deepseek",
            @"token_siliconflow": @"synthetic-siliconflow", @"token_groq": @"synthetic-groq",
            @"prompt_id": @"custom_2", @"prompt_custom_1": @"synthetic-one",
            @"prompt_custom_2": @"synthetic-two", @"prompt_custom_3": @"synthetic-three",
            @"future_field": @"preserve"
        };
        NSDictionary *original = @{@"ai_assistant": originalAI, @"learning": @YES};
        NSDictionary *edits = @{
            @"enabled": @NO, @"provider": @"deepseek", @"model": @"new-model",
            @"endpoint": @"https://synthetic.invalid/chat", @"candidate_limit": @7,
            @"prompt_custom_1": @"new-one",
            @"prompt_custom_2": @"new-two", @"prompt_custom_3": @"new-three"
        };
        NSDictionary *merged = MSIMEAISettingsMerge(original, edits);
        for (NSString *key in edits) assert([merged[@"ai_assistant"][key] isEqual:edits[key]]);
        for (NSString *key in originalAI) {
            if (!edits[key]) assert([merged[@"ai_assistant"][key] isEqual:originalAI[key]]);
        }
        assert([merged[@"learning"] isEqual:@YES]);
        assert([original[@"ai_assistant"][@"model"] isEqual:@"old-model"]);
        NSDictionary *unowned = MSIMEAISettingsMerge(original, @{@"token": @"must-not-overwrite"});
        assert([unowned isEqual:original]);
        assert([MSIMEAISettingsMerge(@{}, edits)[@"ai_assistant"] isEqual:edits]);
    }
}
