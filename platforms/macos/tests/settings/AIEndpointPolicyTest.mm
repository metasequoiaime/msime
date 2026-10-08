// 与 crates/client-core/src/ai/endpoint.rs 跑同一组用例：两边结论不一致时，设置窗口会放行输入法拒绝发出的地址，或者反过来。
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
            MSIMEAIEndpointProblem problem = MSIMEAIEndpointCheck(endpoint, nil);
            NSString *result = problem == MSIMEAIEndpointProblemNone ? @"allowed" : problem == MSIMEAIEndpointProblemInvalid ? @"invalid" : @"cleartext_public";
            id expectedOrigin = item[@"origin"];
            NSString *origin = MSIMEAIEndpointOrigin(endpoint);
            BOOL originMatches = expectedOrigin == NSNull.null ? origin == nil : [origin isEqualToString:expectedOrigin];
            if (![result isEqualToString:item[@"result"]] || !originMatches) {
                fprintf(stderr, "mismatch: %s -> %s %s\n", endpoint.UTF8String, result.UTF8String, origin.UTF8String ?: "nil");
                ++failures;
            }
        }
        // 带前导零的点分写法在系统解析里按八进制处理（`010.0.0.1` 是 8.0.0.1），不能当成 10/8 放行。
        assert(MSIMEAIEndpointCheck(@"http://010.0.0.1/v1", nil) == MSIMEAIEndpointProblemCleartextPublicHost);
        assert(MSIMEAIEndpointCheck(@"http://0x7f000001/v1", nil) == MSIMEAIEndpointProblemCleartextPublicHost);
        assert(failures == 0);
    }
    return 0;
}
