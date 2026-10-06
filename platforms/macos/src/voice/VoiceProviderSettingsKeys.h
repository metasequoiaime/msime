#pragma once
#import <Foundation/Foundation.h>
#import "../core/SharedVoicePreferences.h"

// The native voice window keeps its own "voiceInput" dictionary and the input method reads the
// MSIMEClientVoice* defaults. For a long time only the first of those was written, so every choice made in
// that window was stored and then ignored - the window looked like it worked and no recording ever used it.
//
// These two functions are the pairing, in one place so the window and its test agree: what a saved field is
// written to, and which of the two stores wins when the window is reopened. A field added to the window
// without a line here is a field the runtime will not see.

// The window's private key for each editable string, and the shared default the input method reads.
static inline NSDictionary<NSString *, NSString *> *MSIMEVoiceProviderSharedKeys(void)
{
    return @{
        @"provider" : @"MSIMEClientVoiceASRProvider",
        @"endpoint" : @"MSIMEClientVoiceASREndpoint",
        @"model" : @"MSIMEClientVoiceASRModel",
        @"modelPath" : @"MSIMEClientVoiceASRModelPath",
        @"token" : @"MSIMEClientVoiceASRToken",
        @"polishEndpoint" : @"MSIMEClientVoicePolishEndpoint",
        @"polishModel" : @"MSIMEClientVoicePolishModel",
        @"polishToken" : @"MSIMEClientVoicePolishToken",
        @"polishPromptID" : @"MSIMEClientVoicePolishPromptID",
        @"polishPromptCustom1" : @"MSIMEClientVoicePolishPromptCustom1",
        @"polishPromptCustom2" : @"MSIMEClientVoicePolishPromptCustom2",
        @"polishPromptCustom3" : @"MSIMEClientVoicePolishPromptCustom3",
        @"captureDevice" : @"MSIMEClientVoiceCaptureDevice",
    };
}

// The shared default is the one store for these fields: the Tauri settings page writes only that, and this window writes it on save. `fallback` covers a default that was never written.
//
// `sharedValue` is read leniently on purpose: an out-of-band edit (defaults write, a managed preference, a corrupt plist) that leaves a number where a string belongs used to reach -length and kill the input method on the next Control+Option+V.
static inline NSString *MSIMEVoiceProviderSharedSetting(id sharedValue, NSString *fallback)
{
    if ([sharedValue isKindOfClass:NSString.class] && [(NSString *)sharedValue length] > 0)
        return (NSString *)sharedValue;
    return fallback;
}

// Match the shared settings page: selecting another provider replaces values
// that are empty or one of our known defaults, but never overwrites a custom
// endpoint/model the user deliberately entered.
static inline NSString *MSIMEVoiceProviderValueAfterSelection(NSString *current,
                                                              NSString *selectedDefault,
                                                              NSArray<NSString *> *knownDefaults)
{
    if (current.length == 0 || [knownDefaults containsObject:current])
        return selectedDefault ?: @"";
    return current;
}

// Update only one provider's credential slot. Empty tokens and providers that
// do not use a service have no slot, exactly like the shared React editor.
static inline NSDictionary *MSIMEVoiceProviderTokenSlotsByUpdating(id current,
                                                                   NSString *provider,
                                                                   NSString *token,
                                                                   BOOL usesService)
{
    NSMutableDictionary *slots = [MSIMEValidVoiceTokenSlots(current) mutableCopy];
    if (!slots) slots = [NSMutableDictionary dictionary];
    if (!provider.length || provider.length > 128) return [slots copy];
    if (usesService && [token isKindOfClass:NSString.class] && token.length > 0 && token.length <= 16384)
        slots[provider] = token;
    else
        [slots removeObjectForKey:provider];
    return [slots copy];
}

// Keychain accounts include the provider as well as the endpoint origin. Two
// providers may intentionally share an OpenAI-compatible origin but must never
// read each other's bearer token. Paths do not affect the credential origin.
static inline NSString *MSIMEVoiceProviderCredentialAccount(NSString *kind,
                                                             NSString *provider,
                                                             NSString *endpoint)
{
    NSURLComponents *url = [NSURLComponents componentsWithString:endpoint ?: @""];
    NSString *scheme = url.scheme.lowercaseString ?: @"";
    NSString *host = url.host.lowercaseString ?: @"";
    NSNumber *port = url.port ?: @443;
    return [NSString stringWithFormat:@"%@|%@|%@://%@:%@", kind ?: @"", provider.lowercaseString ?: @"",
                                      scheme, host, port];
}

static inline BOOL MSIMEVoiceProviderShouldDeletePreviousCredential(NSString *previousProvider,
                                                                    NSString *previousEndpoint,
                                                                    NSString *provider,
                                                                    NSString *endpoint)
{
    if (!previousProvider.length || ![previousProvider.lowercaseString isEqual:provider.lowercaseString])
        return NO;
    NSString *before = MSIMEVoiceProviderCredentialAccount(@"asr", previousProvider, previousEndpoint);
    NSString *after = MSIMEVoiceProviderCredentialAccount(@"asr", provider, endpoint);
    return ![before isEqual:after];
}
