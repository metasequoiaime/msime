#import "../../src/voice/VoiceProviderSettings.h"
#include "../../../../shared/voice/VoiceProviderCatalog.h"

#include <cassert>
#include <cstdio>

static NSString *CatalogText(std::string_view value)
{
    return [[NSString alloc] initWithBytes:value.data() length:value.size() encoding:NSUTF8StringEncoding];
}

int main()
{
    @autoreleasepool {
        // 顺序属于 macOS；名称和默认值必须实际读取共享目录。
        NSArray *expected = @[@"doubao", @"openai", @"siliconflow", @"groq", @"everyapi",
                              @"mistral", @"bailian", @"zhipu", @"minimax", @"custom",
                              @"system", @"local"];
        NSArray *identifiers = MSIMEVoiceASRProviderIDs();
        NSArray *titles = MSIMEVoiceASRProviderTitles();
        assert([identifiers isEqual:expected]);
        assert(titles.count == identifiers.count);
        for (const auto &preset : msime::voice::kAsrProviders) {
            NSString *identifier = CatalogText(preset.id);
            NSUInteger index = [identifiers indexOfObject:identifier];
            assert(index != NSNotFound);
            assert([titles[index] isEqual:CatalogText(preset.title)]);
            assert([MSIMEVoiceASRProviderDefaultEndpoint(identifier) isEqual:CatalogText(preset.endpoint)]);
            assert([MSIMEVoiceASRProviderDefaultModel(identifier) isEqual:CatalogText(preset.model)]);
            assert(MSIMEVoiceASRProviderUsesService(identifier));
        }
        assert([titles[titles.count - 2] isEqual:@"macOS 系统识别"]);
        assert([titles.lastObject isEqual:@"本地模型"]);
        for (NSString *identifier in @[@"system", @"local"]) {
            assert(MSIMEVoiceASRProviderDefaultEndpoint(identifier).length == 0);
            assert(MSIMEVoiceASRProviderDefaultModel(identifier).length == 0);
            assert(!MSIMEVoiceASRProviderUsesService(identifier));
        }
        // 保持旧公开接口的大小写、空白、未知与空指针语义。
        assert([MSIMEVoiceASRProviderDefaultEndpoint(@"OPENAI")
            isEqual:MSIMEVoiceASRProviderDefaultEndpoint(@"openai")]);
        assert([MSIMEVoiceASRProviderDefaultModel(@"OPENAI")
            isEqual:MSIMEVoiceASRProviderDefaultModel(@"openai")]);
        assert(MSIMEVoiceASRProviderUsesService(@"OPENAI"));
        for (id value in @[[NSNull null], @"", @" openai ", @"synthetic-unknown"]) {
            NSString *identifier = value == [NSNull null] ? nil : value;
            assert(MSIMEVoiceASRProviderDefaultEndpoint(identifier).length == 0);
            assert(MSIMEVoiceASRProviderDefaultModel(identifier).length == 0);
            assert(!MSIMEVoiceASRProviderUsesService(identifier));
        }
    }
    std::puts("macOS voice provider catalog passed.");
}
