#pragma once
#import <AppKit/AppKit.h>
FOUNDATION_EXPORT NSNotificationName const MSIMEVoiceSettingsDidChangeNotification;

// The prompt presets, as an identifier the preference stores and a name the user reads.
//
// The popup that offered these used to be built from the identifiers alone, so it showed `cleanup` and `zh2en` as menu items and wrote back whichever string was on screen. The names are the reference's own (`PolishPromptPreset` in voice_providers.cpp); the three custom slots are this client's.
static inline NSArray<NSString *> *MSIMEPolishPromptIdentifiers(void)
{
    return @[@"cleanup", @"faithful", @"zh2en", @"casual", @"custom_1", @"custom_2", @"custom_3"];
}

static inline NSArray<NSString *> *MSIMEPolishPromptTitles(void)
{
    return @[@"精炼整理", @"忠实校对", @"中翻英", @"口语整理", @"自定义一", @"自定义二", @"自定义三"];
}

static inline NSString *MSIMEPolishPromptIdentifierForIndex(NSInteger index)
{
    NSArray<NSString *> *identifiers = MSIMEPolishPromptIdentifiers();
    return index >= 0 && (NSUInteger)index < identifiers.count ? identifiers[(NSUInteger)index]
                                                               : identifiers.firstObject;
}

/// The default a preset identifier falls back to. An identifier from another build, or one an out-of-band edit left behind, is not a preset this client can show, and selecting nothing in the popup would write the first item back the next time anything else on the card changed.
static inline NSString *MSIMEPolishPromptIdentifierOrDefault(NSString *identifier)
{
    return [MSIMEPolishPromptIdentifiers() containsObject:identifier ?: @""] ? identifier
                                                                             : MSIMEPolishPromptIdentifiers().firstObject;
}

