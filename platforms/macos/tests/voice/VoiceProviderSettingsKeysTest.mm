#import "../../src/voice/VoiceProviderSettings.h"
#import "../../src/voice/VoiceProviderSettingsKeys.h"
#import "../../src/voice/VoiceSettingsEntry.h"

#import <objc/runtime.h>

#include <cassert>
#include <cstdio>

@interface VoiceSettingsEntryFixture : NSObject
@property(class, readonly) VoiceSettingsEntryFixture *sharedController;
@property NSUInteger presentations;
- (void)showAndActivate;
@end
@implementation VoiceSettingsEntryFixture
+ (VoiceSettingsEntryFixture *)sharedController {
    static VoiceSettingsEntryFixture *fixture;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ fixture = [VoiceSettingsEntryFixture new]; });
    return fixture;
}
- (void)showAndActivate { ++self.presentations; }
@end

@interface VoiceSettingsEntryMissingPresentationFixture : NSObject
+ (id)sharedController;
@end
@implementation VoiceSettingsEntryMissingPresentationFixture
+ (id)sharedController { return [NSObject new]; }
@end

// Every text field the window edits has to reach the default the input method reads. The window used to
// write its own dictionary alone, so provider, endpoint, model and model path were stored and then ignored
// by every recording. Ask the runtime what the class actually carries rather than repeating a list here:
// a property added to the window without a shared key fails this.
static void TestEveryEditableFieldHasASharedKey()
{
    NSDictionary<NSString *, NSString *> *keys = MSIMEVoiceProviderSharedKeys();
    unsigned int count = 0;
    objc_property_t *properties = class_copyPropertyList(MetasequoiaVoiceProviderSettings.class, &count);
    NSUInteger strings = 0;
    for (unsigned int index = 0; index < count; ++index) {
        NSString *name = @(property_getName(properties[index]));
        const char *attributes = property_getAttributes(properties[index]);
        // The switch is a BOOL and is saved on its own; everything else the window edits is text.
        if (!strstr(attributes, "T@\"NSString\"")) continue;
        ++strings;
        assert(keys[name].length && "a text field with no shared key never reaches a recording");
    }
    free(properties);
    assert(strings == keys.count);

    NSMutableSet *seen = [NSMutableSet set];
    for (NSString *field in keys) {
        assert([keys[field] hasPrefix:@"MSIMEClientVoice"]);
        // Two fields sharing one default would make the second silently overwrite the first.
        assert(![seen containsObject:keys[field]]);
        [seen addObject:keys[field]];
    }
}

static void TestSharedSettingReadsTheSharedStoreAndToleratesJunk()
{
    assert([MSIMEVoiceProviderSharedSetting(@"openai", @"doubao") isEqual:@"openai"]);
    // An empty shared value is not a choice; it is a default that was never written.
    assert([MSIMEVoiceProviderSharedSetting(@"", @"fallback") isEqual:@"fallback"]);
    assert([MSIMEVoiceProviderSharedSetting(nil, @"fallback") isEqual:@"fallback"]);
    // A number where a string belongs used to reach -length and take the input method down on the next Control+Option+V, so the value is checked rather than trusted.
    assert([MSIMEVoiceProviderSharedSetting(@7, @"doubao") isEqual:@"doubao"]);
}

static void TestProviderCredentialIsolation()
{
    NSArray *knownEndpoints = @[
        MSIMEVoiceASRProviderDefaultEndpoint(@"openai"),
        MSIMEVoiceASRProviderDefaultEndpoint(@"groq")
    ];
    assert([MSIMEVoiceProviderValueAfterSelection(@"", @"next-default", knownEndpoints)
        isEqual:@"next-default"]);
    assert([MSIMEVoiceProviderValueAfterSelection(knownEndpoints[0], @"next-default", knownEndpoints)
        isEqual:@"next-default"]);
    assert([MSIMEVoiceProviderValueAfterSelection(@"https://private.example/asr", @"next-default", knownEndpoints)
        isEqual:@"https://private.example/asr"]);

    NSDictionary *slots = MSIMEVoiceProviderTokenSlotsByUpdating(
        @{@"openai":@"openai-secret", @"groq":@"old-groq"}, @"groq", @"groq-secret", YES);
    assert([slots[@"openai"] isEqual:@"openai-secret"]);
    assert([slots[@"groq"] isEqual:@"groq-secret"]);
    slots = MSIMEVoiceProviderTokenSlotsByUpdating(slots, @"system", @"must-not-survive", NO);
    assert(!slots[@"system"] && [slots[@"openai"] isEqual:@"openai-secret"]);

    NSString *openAI = MSIMEVoiceProviderCredentialAccount(
        @"asr", @"openai", @"https://api.example.test/v1/audio");
    NSString *groq = MSIMEVoiceProviderCredentialAccount(
        @"asr", @"groq", @"https://api.example.test/v1/audio");
    NSString *openAISecondPath = MSIMEVoiceProviderCredentialAccount(
        @"asr", @"openai", @"https://api.example.test/v2/audio");
    assert(![openAI isEqual:groq]);
    assert([openAI isEqual:openAISecondPath]);
    assert(!MSIMEVoiceProviderShouldDeletePreviousCredential(
        @"openai", @"https://old.example/asr", @"groq", @"https://new.example/asr"));
    assert(MSIMEVoiceProviderShouldDeletePreviousCredential(
        @"openai", @"https://old.example/asr", @"openai", @"https://new.example/asr"));
    assert(!MSIMEVoiceProviderShouldDeletePreviousCredential(
        @"openai", @"https://same.example/v1", @"openai", @"https://same.example/v2"));
}

static void TestProviderWindowRestoresTheMatchingDraft()
{
    [NSApplication sharedApplication];
    MetasequoiaVoiceProviderSettingsWindow *window = [MetasequoiaVoiceProviderSettingsWindow new];
    // The controls belong to the form, which is also the 语音输入 page of the settings window.
    id form = [window valueForKey:@"form"];
    NSPopUpButton *provider = [form valueForKey:@"provider"];
    NSTextField *endpoint = [form valueForKey:@"endpoint"];
    NSTextField *model = [form valueForKey:@"model"];
    NSSecureTextField *token = [form valueForKey:@"token"];
    [form setValue:@"openai" forKey:@"loadedProvider"];
    [form setValue:[@{@"groq":@"", @"mistral":@""} mutableCopy] forKey:@"tokenDrafts"];
    endpoint.stringValue = MSIMEVoiceASRProviderDefaultEndpoint(@"openai");
    model.stringValue = MSIMEVoiceASRProviderDefaultModel(@"openai");
    token.stringValue = @"openai-secret";

    [provider selectItemAtIndex:[MSIMEVoiceASRProviderIDs() indexOfObject:@"groq"]];
    [NSApp sendAction:provider.action to:provider.target from:provider];
    assert([endpoint.stringValue isEqual:MSIMEVoiceASRProviderDefaultEndpoint(@"groq")]);
    token.stringValue = @"groq-secret";
    [provider selectItemAtIndex:[MSIMEVoiceASRProviderIDs() indexOfObject:@"openai"]];
    [NSApp sendAction:provider.action to:provider.target from:provider];
    assert([token.stringValue isEqual:@"openai-secret"]);
    assert([endpoint.stringValue isEqual:MSIMEVoiceASRProviderDefaultEndpoint(@"openai")]);

    endpoint.stringValue = @"https://private.example/asr";
    [provider selectItemAtIndex:[MSIMEVoiceASRProviderIDs() indexOfObject:@"mistral"]];
    [NSApp sendAction:provider.action to:provider.target from:provider];
    assert([endpoint.stringValue isEqual:@"https://private.example/asr"]);
    NSDictionary *drafts = [form valueForKey:@"tokenDrafts"];
    assert([drafts[@"openai"] isEqual:@"openai-secret"]);
    assert([drafts[@"groq"] isEqual:@"groq-secret"]);
    [window close];
}

static void TestLoadUsesTheSelectedProviderSlots()
{
    NSUserDefaults *defaults = NSUserDefaults.standardUserDefaults;
    NSDictionary *oldArguments = [defaults volatileDomainForName:NSArgumentDomain];
    [defaults setVolatileDomain:@{
        @"MSIMEClientVoiceASRProvider":@"groq",
        @"MSIMEClientVoiceASRTokens":@{@"openai":@"openai-secret", @"groq":@"groq-secret"},
        @"MSIMEClientVoiceASRToken":@"wrong-flat-secret",
        @"MSIMEClientVoicePolishProvider":@"deepseek",
        @"MSIMEClientVoicePolishTokens":@{@"deepseek":@"polish-secret"},
        @"MSIMEClientVoicePolishToken":@"wrong-flat-polish"
    } forName:NSArgumentDomain];
    MetasequoiaVoiceProviderSettings *settings = [MetasequoiaVoiceProviderSettings loadSettings];
    assert([settings.provider isEqual:@"groq"]);
    assert([settings.token isEqual:@"groq-secret"]);
    assert([settings.tokenSlots[@"openai"] isEqual:@"openai-secret"]);
    assert([settings.polishToken isEqual:@"polish-secret"]);

    [defaults setVolatileDomain:@{
        @"MSIMEClientVoiceASRProvider":@"system",
        @"MSIMEClientVoiceASRToken":@"stale-cloud-secret"
    } forName:NSArgumentDomain];
    settings = [MetasequoiaVoiceProviderSettings loadSettings];
    assert([settings.provider isEqual:@"system"]);
    assert(settings.token.length == 0);
    [defaults setVolatileDomain:oldArguments forName:NSArgumentDomain];
}

// With nothing chosen yet the window falls back to the shared macOS first-run polish service, which follows the source: DeepSeek with `deepseek-v4-flash`. The provider decides which token slot is read, so it is checked through the slot it selects.
static void TestUnsetPolishServiceFallsBackToTheSharedDefault()
{
    NSUserDefaults *defaults = NSUserDefaults.standardUserDefaults;
    NSDictionary *oldArguments = [defaults volatileDomainForName:NSArgumentDomain];
    // The argument domain cannot hide a persisted provider, and this test binary never persists one.
    assert([defaults objectForKey:@"MSIMEClientVoicePolishProvider"] == nil);
    // An empty shared value is treated as never written, so these stand in for unset keys.
    [defaults setVolatileDomain:@{
        @"voiceInput":@{},
        @"MSIMEClientVoicePolishEndpoint":@"",
        @"MSIMEClientVoicePolishModel":@"",
        @"MSIMEClientVoicePolishTokens":@{@"deepseek":@"deepseek-secret", @"siliconflow":@"siliconflow-secret"}
    } forName:NSArgumentDomain];
    MetasequoiaVoiceProviderSettings *settings = [MetasequoiaVoiceProviderSettings loadSettings];
    assert([settings.polishEndpoint isEqual:@"https://api.deepseek.com/chat/completions"]);
    assert([settings.polishModel isEqual:@"deepseek-v4-flash"]);
    assert([settings.polishToken isEqual:@"deepseek-secret"]);
    assert([MSIMEVoicePolishDefaultProvider isEqual:@"deepseek"]);
    [defaults setVolatileDomain:oldArguments forName:NSArgumentDomain];
}

static void TestEveryRuntimeProviderIsEditable()
{
    NSArray *providers = MSIMEVoiceASRProviderIDs();
    NSArray *expected = @[ @"doubao", @"openai", @"siliconflow", @"groq", @"everyapi", @"mistral",
                           @"bailian", @"system", @"local" ];
    assert([providers isEqual:expected]);
    assert(MSIMEVoiceASRProviderTitles().count == providers.count);
    NSDictionary *defaults = @{
        @"everyapi" : @[ @"https://api.everyapi.ai/v1/audio/transcriptions", @"openai/whisper-large-v3-turbo" ],
        @"mistral" : @[ @"https://api.mistral.ai/v1/audio/transcriptions", @"voxtral-mini-latest" ],
        @"bailian" : @[ @"https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions", @"qwen3-asr-flash" ]
    };
    for (NSString *provider in defaults) {
        assert([MSIMEVoiceASRProviderDefaultEndpoint(provider) isEqual:defaults[provider][0]]);
        assert([MSIMEVoiceASRProviderDefaultModel(provider) isEqual:defaults[provider][1]]);
        MetasequoiaVoiceProviderSettings *settings = [MetasequoiaVoiceProviderSettings new];
        settings.provider = provider;
        settings.endpoint = defaults[provider][0];
        settings.model = defaults[provider][1];
        settings.token = @"synthetic-token";
        settings.modelPath = @"";
        settings.polishEnabled = NO;
        assert([settings validate:nil]);
    }
    MetasequoiaVoiceProviderSettings *doubao = [MetasequoiaVoiceProviderSettings new];
    doubao.provider = @"doubao";
    doubao.endpoint = MSIMEVoiceASRProviderDefaultEndpoint(@"doubao");
    doubao.model = @"";
    doubao.token = @"synthetic-token";
    doubao.modelPath = @"";
    doubao.polishEnabled = NO;
    assert([doubao validate:nil]);

    MetasequoiaVoiceProviderSettings *system = [MetasequoiaVoiceProviderSettings new];
    system.provider = @"system";
    system.endpoint = @"";
    system.model = @"";
    system.token = @"";
    system.modelPath = @"";
    system.polishEnabled = NO;
    assert([system validate:nil]);
    assert(!MSIMEVoiceASRProviderUsesService(@"system"));
    assert(!MSIMEVoiceASRProviderUsesService(@"local"));
}

static void TestNativeSettingsEntryUsesTheProviderWindowContract()
{
    VoiceSettingsEntryFixture *fixture = VoiceSettingsEntryFixture.sharedController;
    assert(MSIMEShowVoiceSettingsWindow(VoiceSettingsEntryFixture.class));
    assert(fixture.presentations == 1);
    assert(!MSIMEShowVoiceSettingsWindow(Nil));
    assert(!MSIMEShowVoiceSettingsWindow(NSObject.class));
    assert(!MSIMEShowVoiceSettingsWindow(VoiceSettingsEntryMissingPresentationFixture.class));
}

int main()
{
    @autoreleasepool {
        TestEveryEditableFieldHasASharedKey();
        TestSharedSettingReadsTheSharedStoreAndToleratesJunk();
        TestProviderCredentialIsolation();
        TestProviderWindowRestoresTheMatchingDraft();
        TestLoadUsesTheSelectedProviderSlots();
        TestUnsetPolishServiceFallsBackToTheSharedDefault();
        TestEveryRuntimeProviderIsEditable();
        TestNativeSettingsEntryUsesTheProviderWindowContract();
    }
    std::puts("macOS voice provider settings keys passed.");
    return 0;
}
