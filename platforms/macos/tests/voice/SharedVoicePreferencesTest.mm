#import "../../src/core/SharedVoicePreferences.h"
#include <cassert>
#import "../settings/TestPreferenceSuite.h"

int main() {
    @autoreleasepool {
        NSString *suite = [@"app.msime.test.voice." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        assert(MSIMEVoiceInputEnabled(defaults));
        assert(MSIMEVoiceCuesEnabled(defaults));
        NSDictionary *voice = @{
            @"enabled": @NO, @"hotkey_rctrl_ralt": @YES,
            @"language": @"en-US", @"asr_provider": @"openai",
            @"asr_endpoint": @"https://example.invalid/asr", @"asr_model": @"fixture-model",
            @"asr_model_path": @"/Users/fixture/Library/Application Support/app.msime/voice-models/x-asr-zh-en-streaming",
            @"asr_token": @"fixture-only", @"capture_backend": @"macos",
            @"capture_device": @"fixture-device",
            @"asr_app_key": @"fixture-app", @"asr_resource_id": @"fixture-resource",
            @"doubao_boosting_table_id": @"fixture-table", @"polish_provider": @"groq",
            @"polish_endpoint": @"https://example.invalid/polish", @"polish_model": @"fixture-polish",
            @"polish_token": @"fixture-only", @"polish_prompt_id": @"custom1",
            @"polish_prompt_custom_1": @"one",
            @"polish_prompt_custom_2": @"two", @"polish_prompt_custom_3": @"three",
            @"sound_enabled": @NO, @"mute_system_audio": @YES, @"stream_inline_preedit": @NO,
            @"polish_enabled": @YES, @"polish_text": @YES, @"hotkey_ctrl_f9": @NO,
            @"hotkey_ralt": @YES, @"hotkey_ctrl_win": @YES
        };
        assert(MSIMEApplySharedVoicePreferences(voice, defaults));
        assert(!MSIMEVoiceInputEnabled(defaults));
        assert([defaults boolForKey:@"MSIMEClientVoiceHotkeyCtrlOption"]);
        assert([[defaults stringForKey:@"MSIMEClientVoiceCaptureBackend"] isEqual:@"macos"]);
        assert(MSIMEVoiceCaptureBackendSupported([defaults objectForKey:@"MSIMEClientVoiceCaptureBackend"]));
        assert([[defaults stringForKey:@"MSIMEClientVoiceCaptureDevice"] isEqual:@"fixture-device"]);
        assert([[defaults stringForKey:@"MSIMEClientVoiceASRProvider"] isEqual:@"openai"]);
        assert([[defaults stringForKey:@"MSIMEClientVoiceASREndpoint"] isEqual:voice[@"asr_endpoint"]]);
        // The model the settings page picks is what the IMK's local recognizer loads.
        assert([[defaults stringForKey:@"MSIMEClientVoiceASRModelPath"] isEqual:voice[@"asr_model_path"]]);
        assert([[defaults stringForKey:@"MSIMEClientVoicePolishPromptCustom3"] isEqual:@"three"]);
        assert([defaults boolForKey:@"MSIMEClientVoicePolish"]);
        assert([defaults boolForKey:@"MSIMEClientVoicePolishText"]);
        assert([defaults boolForKey:@"MSIMEClientVoiceHotkeyCtrlCommand"]);
        assert(![defaults boolForKey:@"MSIMEClientVoiceSoundEnabled"]);
        NSDictionary *captured = MSIMEVoicePreferencesFromDefaults(defaults);
        assert([captured[@"asr_provider"] isEqual:@"openai"]);
        assert([captured[@"capture_backend"] isEqual:@"macos"]);
        assert([captured[@"asr_model_path"] isEqual:voice[@"asr_model_path"]]);
        assert([captured[@"capture_device"] isEqual:@"fixture-device"]);
        assert([captured[@"hotkey_ctrl_win"] isEqual:@YES]);
        assert([captured[@"polish_prompt_custom_3"] isEqual:@"three"]);
        assert([MSIMEVoicePreferencesFromDefaults(nil) isEqual:@{}]);
        NSDictionary *saved = [defaults persistentDomainForName:suite];
        assert(saved.count == voice.count);
        assert(!MSIMEApplySharedVoicePreferences(voice, defaults));
        assert(!MSIMEApplySharedVoicePreferences(nil, defaults));
        assert(!MSIMEApplySharedVoicePreferences(NSNull.null, defaults));
        assert(!MSIMEApplySharedVoicePreferences(@[], defaults));
        MSIMEApplySharedVoicePreferences(@{@"asr_token": NSNull.null, @"capture_backend": @42,
                                           @"capture_device": @42,
                                           @"polish_enabled": @0, @"polish_text": @1, @"sound_enabled": @"true",
                                           @"enabled": @"true", @"hotkey_rctrl_ralt": @0}, defaults);
        assert([[defaults persistentDomainForName:suite] isEqual:saved]);
        for (id supported in @[@"", @"auto", @"AUTO", @"macos", @"MACOS"])
            assert(MSIMEVoiceCaptureBackendSupported(supported));
        for (id unsupported in @[@"windows", @"pulse", @"pipewire", @"alsa", @42, @[]])
            assert(!MSIMEVoiceCaptureBackendSupported(unsupported));
        MSIMEApplySharedVoicePreferences(@{@"capture_device": @"", @"asr_token": @"",
                                           @"polish_enabled": @NO, @"polish_text": @NO}, defaults);
        assert([[defaults stringForKey:@"MSIMEClientVoiceCaptureDevice"] isEqual:@""]);
        assert([[defaults stringForKey:@"MSIMEClientVoiceASRToken"] isEqual:@""]);
        assert(![defaults boolForKey:@"MSIMEClientVoicePolish"]);
        assert(![defaults boolForKey:@"MSIMEClientVoicePolishText"]);
        MSIMEApplySharedVoicePreferences(@{@"enabled": @YES, @"hotkey_rctrl_ralt": @NO}, defaults);
        assert(MSIMEVoiceInputEnabled(defaults));
        assert(![defaults boolForKey:@"MSIMEClientVoiceHotkeyCtrlOption"]);
        NSDictionary *providerSlots = @{
            @"asr_provider": @"openai",
            @"asr_tokens": @{@"openai": @"slot-asr"},
            @"polish_provider": @"groq",
            @"polish_tokens": @{@"groq": @"slot-polish"}
        };
        assert(MSIMEApplySharedVoicePreferences(providerSlots, defaults));
        assert([[defaults stringForKey:@"MSIMEClientVoiceASRToken"] isEqual:@"slot-asr"]);
        assert([[defaults stringForKey:@"MSIMEClientVoicePolishToken"] isEqual:@"slot-polish"]);
        NSDictionary *capturedSlots = MSIMEVoicePreferencesFromDefaults(defaults);
        assert([capturedSlots[@"asr_tokens"][@"openai"] isEqual:@"slot-asr"]);
        assert([capturedSlots[@"polish_tokens"][@"groq"] isEqual:@"slot-polish"]);
        assert([MSIMEVoiceTokenForProvider(defaults, @"MSIMEClientVoiceASRTokens",
                                           @"openai", @"legacy") isEqual:@"slot-asr"]);
        assert([MSIMEVoiceTokenForProvider(defaults, @"MSIMEClientVoiceASRTokens",
                                           @"groq", @"legacy") isEqual:@""]);
        assert(MSIMESaveVoiceTokenSlot(defaults, @"MSIMEClientVoiceASRTokens",
                                       @"groq", @"slot-groq"));
        assert([MSIMEVoiceTokenForProvider(defaults, @"MSIMEClientVoiceASRTokens",
                                           @"groq", @"legacy") isEqual:@"slot-groq"]);

        // macOS 只读提示音总开关：开始、结束提示音和空格锁定不再搬进本机设置，存下的 false 不再起作用，也不会从本机设置写回共享文档。
        for (NSNumber *master in @[@NO, @YES]) {
            for (NSNumber *start in @[@NO, @YES]) {
                for (NSNumber *end in @[@NO, @YES]) {
                    MSIMEApplySharedVoicePreferences(@{@"sound_enabled": master, @"start_sound": start, @"end_sound": end}, defaults);
                    assert(MSIMEVoiceCuesEnabled(defaults) == master.boolValue);
                }
            }
        }
        assert(!MSIMEApplySharedVoicePreferences(@{@"start_sound": @NO, @"end_sound": @NO, @"hotkey_hold_space_lock": @NO}, defaults));
        assert(![defaults objectForKey:@"MSIMEClientVoiceStartSound"] && ![defaults objectForKey:@"MSIMEClientVoiceEndSound"]);
        assert(![defaults objectForKey:@"MSIMEClientVoiceHotkeyHoldSpace"]);
        [defaults setBool:NO forKey:@"MSIMEClientVoiceStartSound"];
        [defaults setBool:NO forKey:@"MSIMEClientVoiceHotkeyHoldSpace"];
        NSDictionary *withoutCutKeys = MSIMEVoicePreferencesFromDefaults(defaults);
        assert(!withoutCutKeys[@"start_sound"] && !withoutCutKeys[@"end_sound"] && !withoutCutKeys[@"hotkey_hold_space_lock"]);
        [defaults removeObjectForKey:@"MSIMEClientVoiceStartSound"];
        [defaults removeObjectForKey:@"MSIMEClientVoiceHotkeyHoldSpace"];

        // 别的平台写下的录音后端（比如 Linux 的 pulse）不再让录音被拒绝：连同它的设备一起当作系统默认设备。
        MSIMEApplySharedVoicePreferences(@{@"capture_backend": @"pulse", @"capture_device": @"alsa_input.fixture"}, defaults);
        assert(MSIMEVoiceCaptureDeviceUID(defaults) == nil);
        assert([[defaults stringForKey:@"MSIMEClientVoiceCaptureBackend"] isEqual:@"pulse"]);
        MSIMEApplySharedVoicePreferences(@{@"capture_backend": @"macos", @"capture_device": @"fixture-uid"}, defaults);
        assert([MSIMEVoiceCaptureDeviceUID(defaults) isEqual:@"fixture-uid"]);
        MSIMEApplySharedVoicePreferences(@{@"capture_backend": @"", @"capture_device": @""}, defaults);
        assert([MSIMEVoiceCaptureDeviceUID(defaults) isEqual:@""]);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
}
