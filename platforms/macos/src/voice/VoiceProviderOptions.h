#pragma once
#import <Foundation/Foundation.h>

// 按持久化的 provider id 决定进程内用哪条识别通道。保持纯函数，免得设置页和控制器悄悄走偏：所有 HTTPS 整句预设（multipart，以及阿里云百炼的 chat_audio）都走整句请求路径，豆包和系统语音各有专用通道。有外部 provider socket 时，provider 路由全部交给它。
// `local` never takes this path: an installed on-device model directory runs in the helper (MSIMEVoiceUsesLocalModelHelper), and anything else the preference holds for it, such as a Whisper model file from before the model catalog, is refused before recording (MSIMEVoiceLocalModelMissing).
static inline BOOL MSIMEVoiceUsesNativeHTTPProvider(NSString *provider, BOOL providerSocketAvailable) {
    if (providerSocketAvailable) return NO;
    NSString *identifier = provider.lowercaseString ?: @"";
    return [@[@"openai", @"groq", @"siliconflow", @"everyapi", @"mistral", @"bailian",
              @"zhipu", @"minimax", @"custom"]
        containsObject:identifier];
}

// `local` with an installed model directory streams through the msime-voice-local helper process, never through a recognizer loaded into the input method, on the same controller path as Doubao so its partial text shows the same way. An external provider socket still owns all routing when present.
static inline BOOL MSIMEVoiceUsesLocalModelHelper(NSString *provider, BOOL providerSocketAvailable, BOOL modelDirectory) {
    return !providerSocketAvailable && modelDirectory && [provider.lowercaseString isEqual:@"local"];
}

// `local` without an installed model directory - no path, a Whisper model file saved before the model catalog, or a model deleted since - has nothing to run. Recording is refused before capture and the user is pointed at the voice settings, as Android's LocalAsrPolicy refuses a Whisper file, rather than the audio going to a recognizer the user did not pick. 未设置的服务按出厂默认 `local` 算（client-core 的 `VoiceInputPreferences::default`）。An external provider socket still owns all routing when present.
static inline BOOL MSIMEVoiceLocalModelMissing(NSString *provider, BOOL providerSocketAvailable, BOOL modelDirectory) {
    return !providerSocketAvailable && !modelDirectory &&
        (!provider || [provider.lowercaseString isEqual:@"local"]);
}

// MSIME-Windows StartRecording 在当前识别服务没有 API Token 时拒绝录音，并告诉用户去哪里填。这里覆盖本宿主自己带 Token 调用的服务——HTTPS 预设和豆包；本地模型（未设置时的出厂默认）、系统语音和外部 socket 都不从这项偏好取 Token，不在此列。
static inline BOOL MSIMEVoiceASRTokenMissing(NSString *provider, NSString *token, BOOL providerSocketAvailable) {
    if (providerSocketAvailable || token.length) return NO;
    return [provider.lowercaseString isEqual:@"doubao"] ||
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
