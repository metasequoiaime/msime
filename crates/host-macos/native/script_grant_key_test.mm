#import <Foundation/Foundation.h>

#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <cstdint>

extern "C" bool msime_macos_script_grant_partitions_include(const char *, const char *);
extern "C" int32_t msime_macos_script_grant_key(const char *, const char *const *, size_t, bool, uint8_t *, size_t);

// 按钥匙串存分区说明的样子编码：XML 属性列表 {"Partitions": [...]} 的十六进制。
static NSString *Description(NSArray *partitions) {
    NSData *data = [NSPropertyListSerialization dataWithPropertyList:@{@"Partitions": partitions}
                                                              format:NSPropertyListXMLFormat_v1_0
                                                             options:0
                                                               error:nil];
    NSMutableString *hex = [NSMutableString stringWithCapacity:data.length * 2];
    const uint8_t *bytes = static_cast<const uint8_t *>(data.bytes);
    for (NSUInteger index = 0; index < data.length; ++index) [hex appendFormat:@"%02X", bytes[index]];
    return hex;
}

static bool Includes(NSString *description, const char *team) {
    return msime_macos_script_grant_partitions_include(description.UTF8String, team);
}

int main() {
    @autoreleasepool {
        // 本团队创建的项；用户之后允许过别的程序时分区里会多出几项，仍然是本团队的。
        assert(Includes(Description(@[@"teamid:ABCDE12345"]), "ABCDE12345"));
        assert(Includes(Description(@[@"apple-tool:", @"apple:", @"teamid:ABCDE12345"]), "ABCDE12345"));
        // 同一用户下别的程序抢先放进来的项：没签名的、用系统工具写的、别的团队的，都不认。
        assert(!Includes(Description(@[@"unsigned:"]), "ABCDE12345"));
        assert(!Includes(Description(@[@"apple-tool:", @"apple:"]), "ABCDE12345"));
        assert(!Includes(Description(@[@"teamid:ZZZZZ99999"]), "ABCDE12345"));
        assert(!Includes(Description(@[@"teamid:ABCDE123456"]), "ABCDE12345"));
        assert(!Includes(Description(@[]), "ABCDE12345"));
        // 说明读不懂时不认。
        assert(!Includes(@"", "ABCDE12345"));
        assert(!Includes(@"3C3F78", "ABCDE12345"));
        assert(!Includes(@"not hex at all", "ABCDE12345"));
        assert(!Includes(Description(@[@"teamid:ABCDE12345"]), ""));
        assert(!msime_macos_script_grant_partitions_include(nullptr, "ABCDE12345"));
        // 参数不对时不碰钥匙串，直接报错并清空输出。
        uint8_t key[32];
        uint8_t short_key[31];
        assert(msime_macos_script_grant_key(nullptr, nullptr, 0, false, key, sizeof key) != 0);
        assert(msime_macos_script_grant_key("", nullptr, 0, false, key, sizeof key) != 0);
        assert(msime_macos_script_grant_key("app.msime.test.script-grants", nullptr, 0, false, short_key, sizeof short_key) != 0);
        assert(msime_macos_script_grant_key("app.msime.test.script-grants", nullptr, 1, false, key, sizeof key) != 0);
    }
    std::puts("macOS script grant key boundary passed.");
    return EXIT_SUCCESS;
}
