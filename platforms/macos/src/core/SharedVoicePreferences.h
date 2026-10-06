#pragma once
#import <Foundation/Foundation.h>

// Match the shared default while preserving explicit disablement.
static inline BOOL MSIMEVoiceInputEnabled(NSUserDefaults *defaults) {
    return [defaults objectForKey:@"MSIMEClientVoiceEnabled"] == nil ||
        [defaults boolForKey:@"MSIMEClientVoiceEnabled"];
}

// Muting other audio is on out of the box, like the source and the shared macOS default.
static inline BOOL MSIMEVoiceMuteSystemAudioEnabled(NSUserDefaults *defaults) {
    return [defaults objectForKey:@"MSIMEClientVoiceMuteSystemAudio"] == nil ||
        [defaults boolForKey:@"MSIMEClientVoiceMuteSystemAudio"];
}

// Native fallbacks for an unset polish service. They match the shared macOS first-run default (`default_polish_service` in crates/client-core/src/preferences.rs), which follows the source template: DeepSeek with `deepseek-v4-flash`.
static NSString *const MSIMEVoicePolishDefaultProvider = @"deepseek";
static NSString *const MSIMEVoicePolishDefaultEndpoint = @"https://api.deepseek.com/chat/completions";
static NSString *const MSIMEVoicePolishDefaultModel = @"deepseek-v4-flash";

static inline BOOL MSIMEVoiceCueEnabled(NSUserDefaults *defaults, BOOL start) {
    NSString *key = start ? @"MSIMEClientVoiceStartSound" : @"MSIMEClientVoiceEndSound";
    return ([defaults objectForKey:@"MSIMEClientVoiceSoundEnabled"] == nil ||
            [defaults boolForKey:@"MSIMEClientVoiceSoundEnabled"]) &&
           ([defaults objectForKey:key] == nil || [defaults boolForKey:key]);
}

// macOS always records through CoreAudio. Keep the shared backend explicit so
// a setting synced from another desktop is never silently reinterpreted here.
static inline BOOL MSIMEVoiceCaptureBackendSupported(id backend) {
    if (backend == nil) return YES;
    if (![backend isKindOfClass:NSString.class]) return NO;
    return ![(NSString *)backend length] ||
        [@[@"auto", @"macos"] containsObject:[(NSString *)backend lowercaseString]];
}

// Token maps are user preferences, not arbitrary JSON. Keep only string
// provider names and string values before putting them in NSUserDefaults;
// malformed snapshots must not make the native fallback throw while saving.
static inline NSDictionary *MSIMEValidVoiceTokenSlots(id value) {
    if (![value isKindOfClass:NSDictionary.class]) return nil;
    NSMutableDictionary *slots = [NSMutableDictionary dictionary];
    for (id provider in (NSDictionary *)value) {
        id token = ((NSDictionary *)value)[provider];
        if (![provider isKindOfClass:NSString.class] ||
            [(NSString *)provider length] == 0 ||
            [(NSString *)provider length] > 128 ||
            ![token isKindOfClass:NSString.class] ||
            [(NSString *)token length] > 16384)
            continue;
        slots[provider] = token;
    }
    return [slots copy];
}

static inline NSDictionary *MSIMEVoiceTokenSlotsFromDefaults(NSUserDefaults *defaults,
                                                               NSString *key) {
    return defaults && key.length
        ? MSIMEValidVoiceTokenSlots([defaults objectForKey:key]) : nil;
}

static inline NSString *MSIMEVoiceTokenForProvider(NSUserDefaults *defaults,
                                                    NSString *key,
                                                    NSString *provider,
                                                    NSString *legacyValue) {
    NSDictionary *slots = MSIMEVoiceTokenSlotsFromDefaults(defaults, key);
    if (!slots) return legacyValue ?: @"";
    NSString *token = slots[provider];
    return [token isKindOfClass:NSString.class] ? token : @"";
}

static inline BOOL MSIMESaveVoiceTokenSlot(NSUserDefaults *defaults, NSString *key,
                                           NSString *provider, NSString *token) {
    if (!defaults || !key.length || !provider.length || provider.length > 128 ||
        ![token isKindOfClass:NSString.class] || token.length > 16384)
        return NO;
    NSMutableDictionary *slots = [MSIMEVoiceTokenSlotsFromDefaults(defaults, key) mutableCopy];
    if (!slots) slots = [NSMutableDictionary dictionary];
    if ([slots[provider] isEqual:token]) return NO;
    slots[provider] = token;
    [defaults setObject:slots forKey:key];
    return YES;
}

// Capture the native fallback's valid voice fields when it writes the shared
// Preferences snapshot. The Tauri settings page remains the primary editor,
// but the fallback window must not leave a second, silently diverging config
// behind. Missing defaults are omitted so a shared value from another host is
// preserved; malformed defaults are ignored rather than poisoning the next
// shared snapshot.
static inline NSDictionary *MSIMEVoicePreferencesFromDefaults(NSUserDefaults *defaults) {
    if (!defaults) return @{};
    NSMutableDictionary *voice = [NSMutableDictionary dictionary];
    NSDictionary *strings = @{
        @"language": @"MSIMEClientVoiceLanguage",
        @"asr_provider": @"MSIMEClientVoiceASRProvider",
        @"asr_endpoint": @"MSIMEClientVoiceASREndpoint",
        @"asr_model": @"MSIMEClientVoiceASRModel",
        @"asr_model_path": @"MSIMEClientVoiceASRModelPath",
        @"asr_token": @"MSIMEClientVoiceASRToken",
        @"capture_backend": @"MSIMEClientVoiceCaptureBackend",
        @"capture_device": @"MSIMEClientVoiceCaptureDevice",
        @"commit_mode": @"MSIMEClientVoiceCommitMode",
        @"asr_app_key": @"MSIMEClientVoiceDoubaoAppKey",
        @"asr_resource_id": @"MSIMEClientVoiceDoubaoResourceID",
        @"doubao_auth_mode": @"MSIMEClientVoiceDoubaoAuthMode",
        @"doubao_boosting_table_id": @"MSIMEClientVoiceDoubaoBoostingTableID",
        @"polish_provider": @"MSIMEClientVoicePolishProvider",
        @"polish_endpoint": @"MSIMEClientVoicePolishEndpoint",
        @"polish_model": @"MSIMEClientVoicePolishModel",
        @"polish_token": @"MSIMEClientVoicePolishToken",
        @"polish_prompt_id": @"MSIMEClientVoicePolishPromptID",
        @"polish_prompt_custom_1": @"MSIMEClientVoicePolishPromptCustom1",
        @"polish_prompt_custom_2": @"MSIMEClientVoicePolishPromptCustom2",
        @"polish_prompt_custom_3": @"MSIMEClientVoicePolishPromptCustom3"
    };
    for (NSString *field in strings) {
        id value = [defaults objectForKey:strings[field]];
        if ([value isKindOfClass:NSString.class]) voice[field] = value;
    }
    NSDictionary *booleans = @{
        @"enabled": @"MSIMEClientVoiceEnabled",
        @"sound_enabled": @"MSIMEClientVoiceSoundEnabled",
        @"start_sound": @"MSIMEClientVoiceStartSound",
        @"end_sound": @"MSIMEClientVoiceEndSound",
        @"mute_system_audio": @"MSIMEClientVoiceMuteSystemAudio",
        @"stream_inline_preedit": @"MSIMEClientVoiceStreamInlinePreedit",
        @"polish_enabled": @"MSIMEClientVoicePolish",
        @"polish_text": @"MSIMEClientVoicePolishText",
        @"doubao_enable_itn": @"MSIMEClientVoiceDoubaoEnableITN",
        @"doubao_enable_punc": @"MSIMEClientVoiceDoubaoEnablePunctuation",
        @"doubao_enable_ddc": @"MSIMEClientVoiceDoubaoEnableDDC",
        @"hotkey_ctrl_f9": @"MSIMEClientVoiceHotkeyCtrlF9",
        @"hotkey_hold_space_lock": @"MSIMEClientVoiceHotkeyHoldSpace",
        @"hotkey_ralt": @"MSIMEClientVoiceHotkeyRightAlt",
        @"hotkey_rctrl_ralt": @"MSIMEClientVoiceHotkeyCtrlOption",
        @"hotkey_ctrl_win": @"MSIMEClientVoiceHotkeyCtrlCommand"
    };
    for (NSString *field in booleans) {
        id value = [defaults objectForKey:booleans[field]];
        if ([value isKindOfClass:NSNumber.class] &&
            CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID())
            voice[field] = value;
    }
    NSDictionary *tokenKeys = @{
        @"asr_tokens": @"MSIMEClientVoiceASRTokens",
        @"polish_tokens": @"MSIMEClientVoicePolishTokens"
    };
    for (NSString *field in tokenKeys) {
        NSDictionary *slots = MSIMEValidVoiceTokenSlots([defaults objectForKey:tokenKeys[field]]);
        if (slots) voice[field] = slots;
    }
    return [voice copy];
}

// Adapt shared settings to the legacy native consumers. Missing or malformed
// fields preserve local values; explicit false and empty strings clear them.
// Reloading settings must not restart an active recording or emit save events.
static inline BOOL MSIMEApplySharedVoicePreferences(id voice, NSUserDefaults *defaults) {
    if (![voice isKindOfClass:NSDictionary.class]) return NO;
    BOOL changed = NO;
    NSDictionary *strings = @{
        @"language": @"Language",
        @"asr_provider": @"ASRProvider", @"asr_endpoint": @"ASREndpoint",
        @"asr_model": @"ASRModel", @"asr_model_path": @"ASRModelPath", @"asr_token": @"ASRToken",
        @"capture_backend": @"CaptureBackend", @"capture_device": @"CaptureDevice",
        @"commit_mode": @"CommitMode",
        @"asr_app_key": @"DoubaoAppKey", @"asr_resource_id": @"DoubaoResourceID",
        @"doubao_auth_mode": @"DoubaoAuthMode",
        @"doubao_boosting_table_id": @"DoubaoBoostingTableID",
        @"polish_provider": @"PolishProvider", @"polish_endpoint": @"PolishEndpoint",
        @"polish_model": @"PolishModel", @"polish_token": @"PolishToken",
        @"polish_prompt_id": @"PolishPromptID",
        @"polish_prompt_custom_1": @"PolishPromptCustom1",
        @"polish_prompt_custom_2": @"PolishPromptCustom2",
        @"polish_prompt_custom_3": @"PolishPromptCustom3"
    };
    for (NSString *field in strings) {
        id value = voice[field];
        if ([value isKindOfClass:NSString.class]) {
            NSString *key = [@"MSIMEClientVoice" stringByAppendingString:strings[field]];
            if (![[defaults objectForKey:key] isEqual:value]) {
                [defaults setObject:value forKey:key]; changed = YES;
            }
        }
    }
    NSDictionary *booleans = @{
        @"enabled": @"Enabled",
        @"start_sound": @"StartSound", @"end_sound": @"EndSound",
        @"sound_enabled": @"SoundEnabled", @"mute_system_audio": @"MuteSystemAudio",
        @"stream_inline_preedit": @"StreamInlinePreedit", @"polish_enabled": @"Polish",
        @"polish_text": @"PolishText",
        @"doubao_enable_itn": @"DoubaoEnableITN",
        @"doubao_enable_punc": @"DoubaoEnablePunctuation",
        @"doubao_enable_ddc": @"DoubaoEnableDDC",
        @"hotkey_ctrl_f9": @"HotkeyCtrlF9", @"hotkey_hold_space_lock": @"HotkeyHoldSpace",
        @"hotkey_ralt": @"HotkeyRightAlt",
        @"hotkey_rctrl_ralt": @"HotkeyCtrlOption",
        // The shared Windows modifier is Command on macOS.
        @"hotkey_ctrl_win": @"HotkeyCtrlCommand"
    };
    for (NSString *field in booleans) {
        id value = voice[field];
        if ([value isKindOfClass:NSNumber.class] &&
            CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID()) {
            NSString *key = [@"MSIMEClientVoice" stringByAppendingString:booleans[field]];
            if (![[defaults objectForKey:key] isEqual:value]) {
                [defaults setObject:value forKey:key]; changed = YES;
            }
        }
    }

    NSDictionary *tokenKeys = @{
        @"asr_tokens": @"MSIMEClientVoiceASRTokens",
        @"polish_tokens": @"MSIMEClientVoicePolishTokens"
    };
    for (NSString *mapField in tokenKeys) {
        NSDictionary *slots = MSIMEValidVoiceTokenSlots(voice[mapField]);
        if (!slots) continue;
        NSString *key = tokenKeys[mapField];
        if (![[defaults objectForKey:key] isEqual:slots]) {
            [defaults setObject:slots forKey:key];
            changed = YES;
        }
    }

    // The shared contract keeps one credential slot per provider. The native
    // fallback consumes a flat token, so select the slot for the provider that
    // is active in this snapshot instead of accidentally reusing a token from
    // the previously selected provider. Missing slots deliberately preserve
    // the local value for compatibility with older snapshots.
    NSDictionary *tokenSlots = @{
        @"asr_tokens": @{@"provider": @"asr_provider", @"key": @"ASRToken"},
        @"polish_tokens": @{@"provider": @"polish_provider", @"key": @"PolishToken"}
    };
    for (NSString *mapField in tokenSlots) {
        NSDictionary *configuration = tokenSlots[mapField];
        id provider = voice[configuration[@"provider"]];
        NSDictionary *slots = MSIMEValidVoiceTokenSlots(voice[mapField]);
        if (![provider isKindOfClass:NSString.class] ||
            [(NSString *)provider length] == 0 ||
            ![slots isKindOfClass:NSDictionary.class])
            continue;
        id token = slots[provider];
        if (![token isKindOfClass:NSString.class]) continue;
        NSString *key = [@"MSIMEClientVoice" stringByAppendingString:configuration[@"key"]];
        if (![[defaults objectForKey:key] isEqual:token]) {
            [defaults setObject:token forKey:key];
            changed = YES;
        }
    }
    return changed;
}
