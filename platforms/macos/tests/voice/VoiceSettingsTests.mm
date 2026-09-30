// Validation of the voice form's stored configuration. validate: reads only the properties, so this never touches the Keychain or the defaults the form saves to.
#import "../../src/voice/VoiceProviderSettings.h"
#include <cstdio>
#include <cstdlib>

namespace
{
void Require(bool value, const char *message)
{
    if (!value)
    {
        std::fprintf(stderr, "%s\n", message);
        std::exit(1);
    }
}
} // namespace
int main()
{
    @autoreleasepool
    {
        MetasequoiaVoiceProviderSettings *settings = [MetasequoiaVoiceProviderSettings new];
        settings.provider = @"openai";
        settings.endpoint = @"https://example.test/v1/audio/transcriptions";
        settings.model = @"fixture-model";
        settings.token = @"fixture-token";
        settings.modelPath = @"";
        settings.polishEnabled = NO;
        settings.polishEndpoint = @"";
        settings.polishModel = @"";
        settings.polishToken = @"";
        Require([settings validate:nil], "valid cloud configuration rejected");
        for (NSString *invalid in @[
                 @"http://example.test/asr", @"https:///", @"https://user:password@example.test/asr",
                 @"https://example.test/asr#fragment", @"file:///tmp/asr"
             ])
        {
            settings.endpoint = invalid;
            NSError *error = nil;
            Require(![settings validate:&error] && error != nil, "unsafe endpoint accepted");
        }
        settings.endpoint = @"https://example.test/asr";
        settings.token = @"";
        Require(![settings validate:nil], "missing ASR token accepted");
        settings.token = @"fixture-token";
        settings.model = @"";
        Require(![settings validate:nil], "missing ASR model accepted");
        settings.model = @"fixture-model";
        // Doubao streams over a WebSocket, so an HTTPS address is not one it can use.
        settings.provider = @"doubao";
        Require(![settings validate:nil], "HTTPS endpoint accepted for the streaming provider");
        settings.endpoint = @"wss://example.test/asr";
        Require([settings validate:nil], "valid streaming configuration rejected");
        settings.provider = @"openai";
        settings.endpoint = @"https://example.test/asr";
        settings.polishEnabled = YES;
        Require(![settings validate:nil], "incomplete opt-in polish accepted");
        settings.polishEndpoint = @"https://polish.test/v1/chat/completions";
        settings.polishModel = @"fixture-model";
        settings.polishToken = @"fixture-token";
        Require([settings validate:nil], "valid polish rejected");
        settings.polishEnabled = NO;
        settings.provider = @"local";
        settings.token = @"";
        settings.modelPath = @"/nonexistent/msime-voice-model";
        Require(![settings validate:nil], "missing local model accepted");
        settings.modelPath = NSTemporaryDirectory();
        Require(![settings validate:nil], "directory without a model manifest accepted as model");
        NSString *path = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        Require([@"model fixture" writeToFile:path atomically:YES encoding:NSUTF8StringEncoding error:nil],
                "model fixture creation failed");
        settings.modelPath = path;
        Require(![settings validate:nil], "a model file accepted in place of an installed model directory");
        [[NSFileManager defaultManager] removeItemAtPath:path error:nil];
        Require([[NSFileManager defaultManager] createDirectoryAtPath:path withIntermediateDirectories:NO attributes:nil error:nil],
                "model directory fixture creation failed");
        Require([@"{}" writeToFile:[path stringByAppendingPathComponent:@"msime-model.json"] atomically:YES
                          encoding:NSUTF8StringEncoding error:nil], "model manifest fixture creation failed");
        settings.modelPath = path;
        Require([settings validate:nil], "local mode requires unused cloud credentials");
        [[NSFileManager defaultManager] removeItemAtPath:path error:nil];
        NSString *external = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        Require([[NSFileManager defaultManager] createDirectoryAtPath:external withIntermediateDirectories:NO attributes:nil error:nil],
                "external model directory creation failed");
        Require([@"{}" writeToFile:[external stringByAppendingPathComponent:@"msime-model.json"] atomically:YES
                          encoding:NSUTF8StringEncoding error:nil], "external manifest creation failed");
        NSString *linked = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        Require([[NSFileManager defaultManager] createSymbolicLinkAtPath:linked withDestinationPath:external error:nil],
                "linked model directory creation failed");
        settings.modelPath = linked;
        Require(![settings validate:nil], "symlinked model directory accepted");
        [[NSFileManager defaultManager] removeItemAtPath:linked error:nil];
        Require([[NSFileManager defaultManager] createDirectoryAtPath:linked withIntermediateDirectories:NO attributes:nil error:nil],
                "model directory creation failed");
        Require([[NSFileManager defaultManager] createSymbolicLinkAtPath:[linked stringByAppendingPathComponent:@"msime-model.json"]
                                                       withDestinationPath:[external stringByAppendingPathComponent:@"msime-model.json"] error:nil],
                "linked manifest creation failed");
        settings.modelPath = linked;
        Require(![settings validate:nil], "symlinked model manifest accepted");
        [[NSFileManager defaultManager] removeItemAtPath:linked error:nil];
        [[NSFileManager defaultManager] removeItemAtPath:external error:nil];
        settings.provider = @"unknown";
        Require(![settings validate:nil], "unknown provider accepted");
    }
    return 0;
}
