#include "DictionaryRuntime.h"
#import "../settings/RuntimeOptions.h"
#include "../../../../shared/apple-bridge/DictionaryInstallation.h"
#include <stdexcept>
#include <fstream>
#include <array>
#import <CommonCrypto/CommonDigest.h>

NSURL *MetasequoiaDictionaryUserDirectory()
{
    const auto legacy = metasequoia::RuntimePaths::legacy();
    return [NSURL fileURLWithFileSystemRepresentation:legacy.user_data.c_str() isDirectory:YES relativeToURL:nil];
}

static NSString *MetasequoiaEngineResourcesDirectory()
{
    NSDictionary *runtime = MSIMELoadRuntimeOptions();
    NSString *resources = [runtime[@"resources"] isKindOfClass:NSString.class] ? runtime[@"resources"] : nil;
    if (resources.length > 0 && resources.isAbsolutePath &&
        [[NSFileManager defaultManager] fileExistsAtPath:resources isDirectory:nil])
        return resources;
    return NSBundle.mainBundle.resourceURL.path;
}

metasequoia::RuntimePaths MetasequoiaCurrentDictionaryPaths()
{
    NSURL *user = MetasequoiaDictionaryUserDirectory();
    NSString *identifier = metasequoia::apple::ActiveDictionarySnapshotIdentifier(user);
    if (identifier.length == 0)
        return metasequoia::RuntimePaths::legacy();
    NSURL *generation =
        [[user URLByAppendingPathComponent:@"snapshot-generations"] URLByAppendingPathComponent:identifier];
    NSURL *journal = [generation URLByAppendingPathComponent:@"user"];
    NSString *content = [[NSString
        stringWithContentsOfURL:[journal URLByAppendingPathComponent:@"active-dictionary-generation"]
                       encoding:NSUTF8StringEncoding
                          error:nil] stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
    if (content.length != 128 ||
        [content rangeOfCharacterFromSet:[[NSCharacterSet characterSetWithCharactersInString:@"0123456789abcdef"]
                                             invertedSet]]
                .location != NSNotFound)
        throw std::runtime_error("Invalid desktop dictionary generation");
    metasequoia::RuntimePaths paths{
        MetasequoiaEngineResourcesDirectory().fileSystemRepresentation, journal.fileSystemRepresentation,
        [generation URLByAppendingPathComponent:@"cache"].fileSystemRepresentation,
        [[journal URLByAppendingPathComponent:@"dictionaries"] URLByAppendingPathComponent:content]
            .fileSystemRepresentation};
    paths.validate();
    for (const char *file : {"msime.db", "english.db", ".ready"})
        if (!std::filesystem::is_regular_file(paths.dictionaries / file))
            throw std::runtime_error("Desktop dictionary generation is incomplete");
    if (!std::filesystem::is_regular_file(paths.user_data / "msime_user.db"))
        throw std::runtime_error("Desktop dictionary journal is missing");
    return paths;
}

#include "DictionaryInstaller.h"
#include "../../../../shared/apple-bridge/DictionarySnapshotBridge.h"
#include "../../../../shared/apple-bridge/DictionarySessionLease.h"

namespace
{
std::string ResourceDigest(NSURL *url)
{
    std::ifstream input(url.fileSystemRepresentation, std::ios::binary);
    if (!input)
        throw std::runtime_error("Cannot read snapshot resource");
    CC_SHA256_CTX state;
    CC_SHA256_Init(&state);
    std::array<char, 65536> buffer;
    while (input)
    {
        input.read(buffer.data(), buffer.size());
        CC_SHA256_Update(&state, buffer.data(), static_cast<CC_LONG>(input.gcount()));
    }
    if (!input.eof())
        throw std::runtime_error("Cannot finish snapshot resource verification");
    unsigned char digest[CC_SHA256_DIGEST_LENGTH];
    CC_SHA256_Final(digest, &state);
    std::string result;
    for (auto byte : digest)
    {
        result += "0123456789abcdef"[byte >> 4];
        result += "0123456789abcdef"[byte & 15];
    }
    return result;
}

NSString *RuntimeVersion()
{
    NSString *identifier = metasequoia::apple::ActiveDictionarySnapshotIdentifier(MetasequoiaDictionaryUserDirectory());
    const auto revision = metasequoia::apple::DictionaryStateRevision(MetasequoiaCurrentDictionaryPaths());
    return [NSString stringWithFormat:@"local-v1:%@:%s", identifier, revision.c_str()];
}
NSDictionary *RuntimeFailure(NSInteger code, NSString *message)
{
    return @{
        @"error" : [NSError errorWithDomain:@"app.msime.snapshot"
                                       code:code
                                   userInfo:@{NSLocalizedDescriptionKey : message}]
    };
}
} // namespace

@interface MSIMEMacDictionarySync : NSObject
+ (NSDictionary *)context;
+ (NSDictionary *)prepare:(NSDictionary *)parameters;
+ (NSDictionary *)activate:(NSDictionary *)parameters;
@end

@implementation MSIMEMacDictionarySync
+ (NSDictionary *)context
{
    if (!NSThread.isMainThread)
        return RuntimeFailure(400, @"请在输入法主线程读取同步状态。");
    try
    {
        NSError *error = nil;
        if (!std::filesystem::is_regular_file(metasequoia::RuntimePaths::legacy().dictionaries / "msime.db") &&
            !EnsureMetasequoiaDictionary(&error))
            return @{@"error" : error ? error : [NSError errorWithDomain:@"app.msime.snapshot" code:1 userInfo:nil]};
        NSURL *resources = NSBundle.mainBundle.resourceURL;
        NSMutableString *content = [NSMutableString string];
        for (NSString *name in @[ @"msime.db.sha256", @"english.db.sha256" ])
        {
            NSString *digest = [[NSString stringWithContentsOfURL:[resources URLByAppendingPathComponent:name]
                                                         encoding:NSUTF8StringEncoding
                                                            error:nil]
                stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
            if (digest.length != 64 ||
                [digest rangeOfCharacterFromSet:[[NSCharacterSet characterSetWithCharactersInString:@"0123456789abcdef"]
                                                    invertedSet]]
                        .location != NSNotFound)
                return RuntimeFailure(400, @"安装包缺少完整词库摘要，请更新输入法。");
            [content appendString:digest];
        }
        return @{
            @"resources" : resources,
            @"user" : MetasequoiaDictionaryUserDirectory(),
            @"contentIdentifier" : content,
            @"version" : RuntimeVersion()
        };
    }
    catch (const std::exception &)
    {
        return RuntimeFailure(500, @"无法读取本机词库状态。");
    }
}
+ (NSDictionary *)prepare:(NSDictionary *)parameters
{
    NSDictionary *context = parameters[@"context"];
    NSError *error = nil;
    MSIMESnapshotNextRecord next = parameters[@"nextRecord"];
    if (![context isKindOfClass:NSDictionary.class] || !next)
        return RuntimeFailure(400, @"快照准备参数无效。");
    try
    {
        NSURL *resources = context[@"resources"];
        NSString *content = context[@"contentIdentifier"];
        if (![resources isKindOfClass:NSURL.class] || !resources.isFileURL || ![content isKindOfClass:NSString.class])
            return RuntimeFailure(400, @"词库资源参数无效。");
        const auto digest = ResourceDigest([resources URLByAppendingPathComponent:@"msime.db"]) +
                            ResourceDigest([resources URLByAppendingPathComponent:@"english.db"]);
        if (digest != content.UTF8String)
            return RuntimeFailure(400, @"安装包词库完整性校验失败，请重新安装输入法。");
    }
    catch (const std::exception &)
    {
        return RuntimeFailure(400, @"无法校验安装包词库，请重新安装输入法。");
    }
    MSIMEPreparedDictionarySnapshot *prepared =
        [DictionarySnapshotBridge prepareResources:context[@"resources"]
                                     userDirectory:context[@"user"]
                                        identifier:parameters[@"identifier"]
                                 contentIdentifier:context[@"contentIdentifier"]
                                    maximumRecords:[parameters[@"maximumRecords"] unsignedIntegerValue]
                                        nextRecord:next
                                             error:&error];
    if (!prepared)
        return @{@"error" : error ? error : [NSError errorWithDomain:@"app.msime.snapshot" code:1 userInfo:nil]};
    return @{@"prepared" : prepared};
}
+ (NSDictionary *)discard:(NSDictionary *)parameters
{
    NSError *error = nil;
    if (![DictionarySnapshotBridge discardInactiveIdentifier:parameters[@"identifier"]
                                               userDirectory:parameters[@"user"]
                                                       error:&error])
        return @{@"error" : error ? error : [NSError errorWithDomain:@"app.msime.snapshot" code:1 userInfo:nil]};
    return @{};
}
+ (NSDictionary *)reset
{
    NSDictionary *context = [self context];
    if (context[@"error"])
        return context;
    NSString *identifier = NSUUID.UUID.UUIDString;
    MSIMESnapshotNextRecord empty = ^NSDictionary *(NSError **error) {
      (void)error;
      return nil;
    };
    NSDictionary *staged = [self
        prepare:@{@"context" : context, @"identifier" : identifier, @"maximumRecords" : @1, @"nextRecord" : empty}];
    if (staged[@"error"])
        return staged;
    NSDictionary *result = [self activate:@{@"prepared" : staged[@"prepared"], @"version" : context[@"version"]}];
    [self discard:@{@"identifier" : identifier, @"user" : context[@"user"]}];
    return result;
}
+ (NSDictionary *)activate:(NSDictionary *)parameters
{
    if (!NSThread.isMainThread)
        return RuntimeFailure(400, @"请在输入法主线程应用词库。");
    MSIMEPreparedDictionarySnapshot *prepared = parameters[@"prepared"];
    NSString *expected = parameters[@"version"];
    if (![prepared isKindOfClass:MSIMEPreparedDictionarySnapshot.class] || ![expected isKindOfClass:NSString.class])
        return RuntimeFailure(400, @"快照应用参数无效。");
    Class controller = NSClassFromString(@"MetasequoiaInputController");
    SEL suspend = NSSelectorFromString(@"suspendForCloudDictionarySwitch");
    if (!controller || ![controller respondsToSelector:suspend])
        return RuntimeFailure(423, @"输入会话暂不可用。");
    using Suspend = NSNumber *(*)(id, SEL);
    if (!reinterpret_cast<Suspend>([controller methodForSelector:suspend])(controller, suspend).boolValue)
        return RuntimeFailure(423, @"请先完成正在输入的内容，再应用云词库。");
    try
    {
        NSURL *user = MetasequoiaDictionaryUserDirectory();
        metasequoia::apple::DictionarySessionLease lease(user);
        NSDictionary *result = nil;
        if (!lease.exclusively([&] {
                NSString *active = metasequoia::apple::ActiveDictionarySnapshotIdentifier(user);
                if ([active isEqualToString:prepared.identifier])
                {
                    result = @{@"version" : RuntimeVersion()};
                    return;
                }
                if (![RuntimeVersion() isEqualToString:expected])
                {
                    result = RuntimeFailure(409, @"本地词库已变化，请重新下载并确认。");
                    return;
                }
                metasequoia::SessionOptions options;
                options.paths = [prepared runtimePaths];
                options.learning = false;
                metasequoia::Session probe(options);
                metasequoia::apple::PublishDictionaryInstallation(user, prepared.identifier, options.paths, active);
                result = @{@"version" : RuntimeVersion()};
            }))
            return RuntimeFailure(423, @"另一个输入进程正在使用词库，请稍后重试。");
        return result;
    }
    catch (const std::exception &)
    {
        return RuntimeFailure(500, @"快照应用未完成，请重新检查本机词库状态。");
    }
}
@end
