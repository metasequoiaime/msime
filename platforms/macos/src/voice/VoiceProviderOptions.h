#pragma once
#import <Foundation/Foundation.h>

// Decide the in-process recognition transport from the persisted provider id.
// Keep this pure so the settings surface and controller cannot silently drift:
// every HTTPS multipart preset belongs to the batch request path, while Doubao
// and system Speech have dedicated transports. An external provider socket owns
// all provider routing when present.
// `local` never takes this path: an installed on-device model directory runs in the helper (MSIMEVoiceUsesLocalModelHelper), and anything else the preference holds for it, such as a Whisper model file from before the model catalog, is refused before recording (MSIMEVoiceLocalModelMissing).
static inline BOOL MSIMEVoiceUsesNativeHTTPProvider(NSString *provider, BOOL providerSocketAvailable) {
    if (providerSocketAvailable) return NO;
    NSString *identifier = provider.lowercaseString ?: @"";
    return [@[@"openai", @"groq", @"siliconflow", @"everyapi", @"mistral"]
        containsObject:identifier];
}

// `local` with an installed model directory streams through the msime-voice-local helper process, never through a recognizer loaded into the input method, on the same controller path as Doubao so its partial text shows the same way. An external provider socket still owns all routing when present.
static inline BOOL MSIMEVoiceUsesLocalModelHelper(NSString *provider, BOOL providerSocketAvailable, BOOL modelDirectory) {
    return !providerSocketAvailable && modelDirectory && [provider.lowercaseString isEqual:@"local"];
}

// `local` without an installed model directory - no path, a Whisper model file saved before the model catalog, or a model deleted since - has nothing to run. Recording is refused before capture and the user is pointed at the voice settings, as Android's LocalAsrPolicy refuses a Whisper file, rather than the audio going to a recognizer the user did not pick. An external provider socket still owns all routing when present.
static inline BOOL MSIMEVoiceLocalModelMissing(NSString *provider, BOOL providerSocketAvailable, BOOL modelDirectory) {
    return !providerSocketAvailable && !modelDirectory && [provider.lowercaseString isEqual:@"local"];
}

// MSIME-Windows StartRecording refuses to record when the current ASR provider has no API token, and tells the user where to fill it in. Here that covers the providers this host calls itself with a token - the HTTPS presets and Doubao, which an unset provider preference means - and not the on-device, system Speech or external-socket paths, which take none from this preference.
static inline BOOL MSIMEVoiceASRTokenMissing(NSString *provider, NSString *token, BOOL providerSocketAvailable) {
    if (providerSocketAvailable || token.length) return NO;
    return !provider || [provider.lowercaseString isEqual:@"doubao"] ||
        MSIMEVoiceUsesNativeHTTPProvider(provider, NO);
}

// Adapt native preferences to the provider contract. An unset Doubao authentication mode means the API key route. The boolean fallbacks match the shared macOS first-run defaults (`source_voice_default` in client-core).
static inline NSDictionary *MSIMEVoiceProviderOptions(NSDictionary *query, NSUserDefaults *defaults) {
    NSMutableDictionary *result = [query mutableCopy];
    id commitMode = [defaults objectForKey:@"MSIMEClientVoiceCommitMode"];
    result[@"commit_mode"] = [commitMode isKindOfClass:NSString.class] &&
        [@[@"tsf", @"sendinput", @"ctrl_v"] containsObject:commitMode] ? commitMode : @"tsf";
    id mode = [defaults objectForKey:@"MSIMEClientVoiceDoubaoAuthMode"];
    if ([mode isKindOfClass:NSString.class] && [@[@"api_key", @"legacy"] containsObject:mode])
        result[@"doubao_auth_mode"] = mode;
    else
        [result removeObjectForKey:@"doubao_auth_mode"];
    NSArray *options = @[
        @[@"polish_text", @"MSIMEClientVoicePolishText", @YES],
        @[@"doubao_enable_itn", @"MSIMEClientVoiceDoubaoEnableITN", @YES],
        @[@"doubao_enable_punc", @"MSIMEClientVoiceDoubaoEnablePunctuation", @YES],
        @[@"doubao_enable_ddc", @"MSIMEClientVoiceDoubaoEnableDDC", @YES]
    ];
    for (NSArray *option in options) {
        id value = [defaults objectForKey:option[1]];
        BOOL valid = [value isKindOfClass:NSNumber.class] &&
            CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID();
        result[option[0]] = valid ? value : option[2];
    }
    return [result copy];
}
