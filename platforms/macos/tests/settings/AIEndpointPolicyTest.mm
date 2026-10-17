// 经 msime_client_ai_endpoint_check 跑各平台共用的用例：设置窗口和发请求前用的都是 Rust 的结论，这里核对 C 接口的回答被正确读出。
#import "../../src/core/AIEndpointPolicy.h"
#include <cassert>
#include <cstdio>

int main() {
    @autoreleasepool {
        NSString *path = [[@(__FILE__) stringByDeletingLastPathComponent] stringByAppendingPathComponent:@"../../../../shared/contracts/ai-endpoint/cases.json"];
        NSData *data = [NSData dataWithContentsOfFile:path];
        assert(data);
        NSDictionary *root = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
        NSArray<NSDictionary *> *cases = root[@"cases"];
        assert(cases.count > 40);
        int failures = 0;
        for (NSDictionary *item in cases) {
            NSString *endpoint = item[@"endpoint"];
            MSIMEAIEndpointProblem problem = MSIMEAIEndpointCheck(endpoint);
            NSString *result = problem == MSIMEAIEndpointProblemNone ? @"allowed" : problem == MSIMEAIEndpointProblemInvalid ? @"invalid" : @"cleartext_public";
            id expectedOrigin = item[@"origin"];
            NSString *origin = MSIMEAIEndpointOrigin(endpoint);
            BOOL originMatches = expectedOrigin == NSNull.null ? origin == nil : [origin isEqualToString:expectedOrigin];
            if (![result isEqualToString:item[@"result"]] || !originMatches) {
                fprintf(stderr, "mismatch: %s -> %s %s\n", endpoint.UTF8String, result.UTF8String, origin.UTF8String ?: "nil");
                ++failures;
            }
        }
        // 带前导零的点分写法在系统解析里按八进制处理（`010.0.0.1` 是 8.0.0.1），不能当成 10/8 放行。Rust 只收按规范写法书写的明文主机，这类写法是不合规，而不是公网明文（原来的 Objective-C 副本把它们当成公网域名）。
        assert(MSIMEAIEndpointCheck(@"http://010.0.0.1/v1") == MSIMEAIEndpointProblemInvalid);
        assert(MSIMEAIEndpointCheck(@"http://0x7f000001/v1") == MSIMEAIEndpointProblemInvalid);
        assert(MSIMEAIEndpointCheck(nil) == MSIMEAIEndpointProblemInvalid && MSIMEAIEndpointOrigin(@"") == nil);
        // 明文地址的主机不收百分号编码和空方括号：`host` 是解码后的，`evil.com%00.local` 解码后以 `.local` 结尾。
        for (NSString *endpoint in @[@"http://evil.com%00.local/v1", @"http://[fe80::1%25en0]/v1", @"http://%31%32%37.0.0.1/v1", @"http://[]/v1"]) {
            assert(MSIMEAIEndpointCheck(endpoint) == MSIMEAIEndpointProblemInvalid);
        }
        assert(failures == 0);
    }
    return 0;
}
