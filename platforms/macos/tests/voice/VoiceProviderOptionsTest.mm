#import "../../src/core/SharedVoicePreferences.h"
#import "../../src/voice/VoiceProviderOptions.h"
#include <cassert>
#import "../settings/TestPreferenceSuite.h"

int main() {
    @autoreleasepool {
        NSString *suite = [@"app.msime.test.provider." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        // Every provider offered as an HTTPS multipart preset must use the batch request path.
        // Falling through starts macOS Speech and silently ignores the selected endpoint and token.
        for (NSString *provider in @[@"openai", @"groq", @"siliconflow", @"everyapi", @"mistral"])
            assert(MSIMEVoiceUsesNativeHTTPProvider(provider, NO));
        for (NSString *provider in @[@"doubao", @"system", @"unknown", @"", @"local", @"cloud"])
            assert(!MSIMEVoiceUsesNativeHTTPProvider(provider, NO));
        assert(!MSIMEVoiceUsesNativeHTTPProvider(@"everyapi", YES));
        // An installed model directory streams through the helper; any other local path, another provider or an external socket does not.
        assert(MSIMEVoiceUsesLocalModelHelper(@"local", NO, YES) && MSIMEVoiceUsesLocalModelHelper(@"Local", NO, YES));
        assert(!MSIMEVoiceUsesLocalModelHelper(@"local", NO, NO) && !MSIMEVoiceUsesLocalModelHelper(@"local", YES, YES));
        for (NSString *provider in @[@"doubao", @"system", @"openai", @""])
            assert(!MSIMEVoiceUsesLocalModelHelper(provider, NO, YES));
        // `local` naming no installed model directory (a Whisper .bin from before the model catalog, say) refuses to record instead of reaching a network or system recognizer; a model directory or an external socket does not.
        assert(MSIMEVoiceLocalModelMissing(@"local", NO, NO) && MSIMEVoiceLocalModelMissing(@"Local", NO, NO));
        assert(!MSIMEVoiceLocalModelMissing(@"local", NO, YES) && !MSIMEVoiceLocalModelMissing(@"local", YES, NO));
        for (NSString *provider in @[@"doubao", @"system", @"openai", @""])
            assert(!MSIMEVoiceLocalModelMissing(provider, NO, NO));
        assert(!MSIMEVoiceUsesNativeHTTPProvider(@"local", NO) && !MSIMEVoiceUsesLocalModelHelper(@"local", NO, NO));
        // Every provider this host calls with the stored token asks for one before recording; an unset provider preference means Doubao.
        for (NSString *provider in @[@"openai", @"groq", @"siliconflow", @"everyapi", @"mistral", @"doubao", @"Doubao"]) {
            assert(MSIMEVoiceASRTokenMissing(provider, nil, NO) && MSIMEVoiceASRTokenMissing(provider, @"", NO));
            assert(!MSIMEVoiceASRTokenMissing(provider, @"synthetic-token", NO));
            assert(!MSIMEVoiceASRTokenMissing(provider, nil, YES));
        }
        assert(MSIMEVoiceASRTokenMissing(nil, nil, NO));
        for (NSString *provider in @[@"local", @"system", @"", @"unknown"])
            assert(!MSIMEVoiceASRTokenMissing(provider, nil, NO));
        NSDictionary *base = @{@"generation": @42, @"language": @"en-us", @"asr_provider": @"doubao"};
        NSDictionary *query = MSIMEVoiceProviderOptions(base, defaults);
        assert([query[@"commit_mode"] isEqual:@"tsf"]);
        for (NSString *mode in @[@"tsf", @"sendinput", @"ctrl_v"]) {
            MSIMEApplySharedVoicePreferences(@{@"commit_mode": mode}, defaults);
            assert([MSIMEVoiceProviderOptions(base, defaults)[@"commit_mode"] isEqual:mode]);
        }
        for (id invalid in @[@"unknown", @42, @[]]) {
            [defaults setObject:invalid forKey:@"MSIMEClientVoiceCommitMode"];
            assert([MSIMEVoiceProviderOptions(base, defaults)[@"commit_mode"] isEqual:@"tsf"]);
        }
        assert([query[@"polish_text"] isEqual:@YES]);
        assert(!query[@"doubao_auth_mode"]);
        assert([query[@"doubao_enable_itn"] isEqual:@YES]);
        assert([query[@"doubao_enable_punc"] isEqual:@YES]);
        assert([query[@"doubao_enable_ddc"] isEqual:@YES]);
        assert(MSIMEVoiceMuteSystemAudioEnabled(defaults));
        MSIMEApplySharedVoicePreferences(@{@"mute_system_audio": @NO}, defaults);
        assert(!MSIMEVoiceMuteSystemAudioEnabled(defaults));
        // Test the actual shared-settings -> native preferences -> request path.
        MSIMEApplySharedVoicePreferences(@{@"doubao_auth_mode": @"api_key",
            @"doubao_enable_itn": @NO, @"doubao_enable_punc": @NO, @"doubao_enable_ddc": @NO}, defaults);
        query = MSIMEVoiceProviderOptions(base, defaults);
        assert([query[@"doubao_auth_mode"] isEqual:@"api_key"]);
        assert([query[@"doubao_enable_itn"] isEqual:@NO]);
        assert([query[@"doubao_enable_punc"] isEqual:@NO]);
        assert([query[@"doubao_enable_ddc"] isEqual:@NO]);
        for (NSString *key in base) assert([query[key] isEqual:base[key]]);
        assert(base.count == 3 && [NSJSONSerialization isValidJSONObject:query]);
        MSIMEApplySharedVoicePreferences(@{@"polish_text": @YES, @"polish_enabled": @NO}, defaults);
        assert([MSIMEVoiceProviderOptions(base, defaults)[@"polish_text"] isEqual:@YES]);
        MSIMEApplySharedVoicePreferences(@{@"polish_text": @NO}, defaults);
        assert([MSIMEVoiceProviderOptions(base, defaults)[@"polish_text"] isEqual:@NO]);
        MSIMEApplySharedVoicePreferences(@{@"doubao_auth_mode": @"legacy"}, defaults);
        assert([MSIMEVoiceProviderOptions(base, defaults)[@"doubao_auth_mode"] isEqual:@"legacy"]);
        for (id invalid in @[@"", @"unknown", @42, @[]]) {
            [defaults setObject:invalid forKey:@"MSIMEClientVoiceDoubaoAuthMode"];
            assert(!MSIMEVoiceProviderOptions(base, defaults)[@"doubao_auth_mode"]);
        }
        [defaults setObject:@"false" forKey:@"MSIMEClientVoiceDoubaoEnableITN"];
        [defaults setObject:@[] forKey:@"MSIMEClientVoiceDoubaoEnablePunctuation"];
        [defaults setObject:@"false" forKey:@"MSIMEClientVoiceDoubaoEnableDDC"];
        query = MSIMEVoiceProviderOptions(base, defaults);
        assert([query[@"doubao_enable_itn"] isEqual:@YES]);
        assert([query[@"doubao_enable_punc"] isEqual:@YES]);
        assert([query[@"doubao_enable_ddc"] isEqual:@YES]);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
}
