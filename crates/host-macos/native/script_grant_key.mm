// 脚本插件授权的 MAC 密钥（见 client-core 的 `plugins::script_grants`）：登录钥匙串里的一个通用密码项，32 字节随机数。输入法进程（platforms/macos 的 input_method_main.mm）在启动时、设置应用（crates/host-macos 的 `script_grant_key`）在第一次读写授权之前各取一次；还没有这一项时由先到的一方生成并写入。两边编译的是同一个文件。
//
// - 只放在登录钥匙串（旧式的文件钥匙串）里。数据保护钥匙串靠 keychain-access-groups 在两个应用之间共享，那是要描述文件背书的受限权利，Developer ID 签名的输入法带上它会被 AMFI 拒绝启动（platforms/macos/tests/settings/entitlements_guard.py）。旧式钥匙串的访问控制表按代码签名信任应用：创建者信任自己和对方（输入法信任设置应用，设置应用信任输入法），别的程序读它时系统会弹出授权框。输入法进程不允许弹框，读不到就当没有密钥，那样没有任何授权条目生效。
// - 只在登录钥匙串里查找，不走搜索列表：同一用户下的程序能新建一个它知道密码的钥匙串，在里面放一个同名的项、改成任意分区，再把它排到搜索列表前面。
// - 读到的项还要核对分区（partition ID）。系统在创建一项时按创建者的签名写下分区（Developer ID 签名的程序是 `teamid:<团队标识>`），改分区要钥匙串的密码，登录钥匙串的密码就是用户的登录密码。本进程有团队标识时只认分区里有本团队的项：同一用户下别的程序抢先放进来的项（分区是 `unsigned:`、`apple-tool:` 或别的团队）不认，否则它知道这把密钥，就能给自己写的授权签名。本进程没有团队标识（ad-hoc 签名或没签名的开发构建）时这一项检查跳过，开发构建不防这种预先放置。
//
// 这些都是 SecKeychain 时代的接口，macOS 10.10 起标为弃用但仍是旧式钥匙串访问控制唯一的公开接口，所以整个文件关掉弃用告警。

#import <Foundation/Foundation.h>
#import <Security/Security.h>

#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <unistd.h>

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"

namespace {

constexpr size_t kKeyBytes = 32;
NSString *const kAccount = @"script-grants-mac-key";
NSString *const kLabel = @"水杉输入法脚本插件授权密钥";

NSData *DataFromHex(NSString *hex) {
    if (hex.length % 2 != 0) return nil;
    NSMutableData *data = [NSMutableData dataWithCapacity:hex.length / 2];
    const char *characters = hex.UTF8String;
    if (!characters) return nil;
    for (NSUInteger index = 0; index < hex.length; index += 2) {
        char pair[3] = {characters[index], characters[index + 1], 0};
        char *end = nullptr;
        const long value = strtol(pair, &end, 16);
        if (end != pair + 2) return nil;
        const uint8_t byte = static_cast<uint8_t>(value);
        [data appendBytes:&byte length:1];
    }
    return data;
}

// 一条分区说明（ACL 条目的描述）是不是列着 `teamid:<team>`。描述是十六进制编码的 XML 属性列表 {"Partitions": [...]}，`security dump-keychain -a` 打出来的就是它。
bool PartitionsInclude(NSString *description, NSString *team) {
    if (!description.length || !team.length) return false;
    NSData *data = DataFromHex(description);
    if (!data) return false;
    id plist = [NSPropertyListSerialization propertyListWithData:data options:NSPropertyListImmutable format:nil error:nil];
    if (![plist isKindOfClass:NSDictionary.class]) return false;
    NSDictionary *dictionary = plist;
    id partitions = dictionary[@"Partitions"];
    if (![partitions isKindOfClass:NSArray.class]) return false;
    NSArray *list = partitions;
    NSString *expected = [@"teamid:" stringByAppendingString:team];
    for (id partition in list) {
        if ([partition isKindOfClass:NSString.class] && [partition isEqualToString:expected]) return true;
    }
    return false;
}

// 本进程签名里的团队标识；ad-hoc 签名或没签名时为 nil。
NSString *OwnTeam() {
    SecCodeRef code = nullptr;
    if (SecCodeCopySelf(kSecCSDefaultFlags, &code) != errSecSuccess || !code) return nil;
    SecStaticCodeRef staticCode = nullptr;
    const OSStatus copied = SecCodeCopyStaticCode(code, kSecCSDefaultFlags, &staticCode);
    CFRelease(code);
    if (copied != errSecSuccess || !staticCode) return nil;
    CFDictionaryRef information = nullptr;
    const OSStatus read = SecCodeCopySigningInformation(staticCode, kSecCSSigningInformation, &information);
    CFRelease(staticCode);
    if (read != errSecSuccess || !information) return nil;
    NSDictionary *info = CFBridgingRelease(information);
    id team = info[(__bridge NSString *)kSecCodeInfoTeamIdentifier];
    return [team isKindOfClass:NSString.class] && [team length] ? team : nil;
}

// 这一项的分区里有没有本团队。
bool ItemBelongsToTeam(SecKeychainItemRef item, NSString *team) {
    SecAccessRef access = nullptr;
    if (SecKeychainItemCopyAccess(item, &access) != errSecSuccess || !access) return false;
    CFArrayRef entries = SecAccessCopyMatchingACLList(access, kSecACLAuthorizationPartitionID);
    CFRelease(access);
    if (!entries) return false;
    bool belongs = false;
    for (id entry in (__bridge NSArray *)entries) {
        CFArrayRef applications = nullptr;
        CFStringRef description = nullptr;
        SecKeychainPromptSelector selector = 0;
        if (SecACLCopyContents((__bridge SecACLRef)entry, &applications, &description, &selector) != errSecSuccess) continue;
        if (applications) CFRelease(applications);
        if (description) {
            belongs = belongs || PartitionsInclude((__bridge NSString *)description, team);
            CFRelease(description);
        }
    }
    CFRelease(entries);
    return belongs;
}

// 登录钥匙串。macOS 10.12 起文件名是 login.keychain-db。
SecKeychainRef CopyLoginKeychain() {
    NSString *directory = [NSHomeDirectory() stringByAppendingPathComponent:@"Library/Keychains"];
    for (NSString *name in @[@"login.keychain-db", @"login.keychain"]) {
        NSString *path = [directory stringByAppendingPathComponent:name];
        if (![NSFileManager.defaultManager fileExistsAtPath:path]) continue;
        SecKeychainRef keychain = nullptr;
        if (SecKeychainOpen(path.fileSystemRepresentation, &keychain) == errSecSuccess && keychain) return keychain;
    }
    return nullptr;
}

OSStatus ReadKey(SecKeychainRef keychain, NSString *service, NSString *team, bool allowInteraction, uint8_t *out) {
    NSMutableDictionary *query = [@{
        (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecAttrService: service,
        (__bridge id)kSecAttrAccount: kAccount,
        (__bridge id)kSecMatchSearchList: @[(__bridge id)keychain],
        (__bridge id)kSecMatchLimit: (__bridge id)kSecMatchLimitOne,
        (__bridge id)kSecReturnAttributes: @YES,
        (__bridge id)kSecReturnData: @YES,
        (__bridge id)kSecReturnRef: @YES,
    } mutableCopy];
    if (!allowInteraction) query[(__bridge id)kSecUseAuthenticationUI] = (__bridge id)kSecUseAuthenticationUIFail;
    CFTypeRef result = nullptr;
    const OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &result);
    if (status != errSecSuccess) return status;
    NSDictionary *item = CFBridgingRelease(result);
    if (![item isKindOfClass:NSDictionary.class]) return errSecDecode;
    NSData *data = item[(__bridge id)kSecValueData];
    id reference = item[(__bridge id)kSecValueRef];
    if (![data isKindOfClass:NSData.class] || data.length != kKeyBytes || !reference) return errSecDecode;
    if (team && !ItemBelongsToTeam((__bridge SecKeychainItemRef)reference, team)) return errSecAuthFailed;
    std::memcpy(out, data.bytes, kKeyBytes);
    return errSecSuccess;
}

OSStatus CreateKey(SecKeychainRef keychain, NSString *service, const char *const *peers, size_t peerCount, uint8_t *out) {
    uint8_t key[kKeyBytes];
    if (SecRandomCopyBytes(kSecRandomDefault, kKeyBytes, key) != errSecSuccess) return errSecAllocate;
    NSMutableArray *trusted = [NSMutableArray array];
    SecTrustedApplicationRef me = nullptr;
    if (SecTrustedApplicationCreateFromPath(nullptr, &me) == errSecSuccess && me) [trusted addObject:CFBridgingRelease(me)];
    for (size_t index = 0; index < peerCount; ++index) {
        if (!peers[index] || access(peers[index], F_OK) != 0) continue;
        SecTrustedApplicationRef peer = nullptr;
        if (SecTrustedApplicationCreateFromPath(peers[index], &peer) == errSecSuccess && peer) [trusted addObject:CFBridgingRelease(peer)];
    }
    SecAccessRef accessRef = nullptr;
    OSStatus status = SecAccessCreate((__bridge CFStringRef)kLabel, (__bridge CFArrayRef)trusted, &accessRef);
    if (status != errSecSuccess || !accessRef) {
        std::memset(key, 0, sizeof key);
        return status != errSecSuccess ? status : errSecAllocate;
    }
    NSDictionary *item = @{
        (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecAttrService: service,
        (__bridge id)kSecAttrAccount: kAccount,
        (__bridge id)kSecAttrLabel: kLabel,
        (__bridge id)kSecAttrAccess: (__bridge id)accessRef,
        (__bridge id)kSecUseKeychain: (__bridge id)keychain,
        (__bridge id)kSecValueData: [NSData dataWithBytes:key length:kKeyBytes],
    };
    status = SecItemAdd((__bridge CFDictionaryRef)item, nullptr);
    CFRelease(accessRef);
    if (status == errSecSuccess) std::memcpy(out, key, kKeyBytes);
    std::memset(key, 0, sizeof key);
    return status;
}

}  // namespace

// 测试用：一条分区说明里有没有 `teamid:<team>`。
extern "C" bool msime_macos_script_grant_partitions_include(const char *description, const char *team) {
    if (!description || !team) return false;
    @autoreleasepool {
        NSString *text = [NSString stringWithUTF8String:description];
        NSString *teamText = [NSString stringWithUTF8String:team];
        return text && teamText && PartitionsInclude(text, teamText);
    }
}

// 取出（没有就生成并写入）`service` 下的授权密钥，成功时把 32 字节写进 `out` 并返回 0，否则返回 OSStatus。`peers` 是要一起信任的对方应用 bundle 的路径，只在生成时用到，不存在的路径跳过。`allow_interaction` 为 false 时（输入法进程）不让系统弹出钥匙串授权框。
extern "C" int32_t msime_macos_script_grant_key(const char *service, const char *const *peers, size_t peer_count,
                                                bool allow_interaction, uint8_t *out, size_t out_length) {
    if (!service || !*service || !out || out_length != kKeyBytes || (peer_count > 0 && !peers)) return errSecParam;
    @autoreleasepool {
        NSString *serviceName = [NSString stringWithUTF8String:service];
        if (!serviceName) return errSecParam;
        SecKeychainRef keychain = CopyLoginKeychain();
        if (!keychain) return errSecNoSuchKeychain;
        NSString *team = OwnTeam();
        OSStatus status = ReadKey(keychain, serviceName, team, allow_interaction, out);
        if (status == errSecItemNotFound) {
            status = CreateKey(keychain, serviceName, peers, peer_count, out);
            // 另一方刚好同时写入了：用它写的那一把。
            if (status == errSecDuplicateItem) status = ReadKey(keychain, serviceName, team, allow_interaction, out);
        }
        CFRelease(keychain);
        if (status != errSecSuccess) std::memset(out, 0, out_length);
        return status;
    }
}

#pragma clang diagnostic pop
