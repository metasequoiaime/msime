#import "../../src/cloud/CloudDictionaryClient.h"

#include <cassert>

static void assertRejected(void (^request)(MSIMECloudDictionaryCompletion)) {
    __block BOOL completed = NO;
    request(^(NSData *data, NSInteger status, NSError *error) {
        assert(data == nil);
        assert(status == 400);
        assert(error != nil);
        completed = YES;
    });
    assert(completed);
}

int main() {
    @autoreleasepool {
        NSData *oversized = [NSMutableData dataWithLength:65537];
        assertRejected(^(MSIMECloudDictionaryCompletion completion) {
            MSIMEMutateCloudDictionary(@"POST", @"pinyin", nil, oversized, @"synthetic-token", completion);
        });
        assertRejected(^(MSIMECloudDictionaryCompletion completion) {
            MSIMEMutateCloudFixedPosition(@"PUT", oversized, @"synthetic-token", completion);
        });
        assertRejected(^(MSIMECloudDictionaryCompletion completion) {
            MSIMEMutateCloudFixedPosition(@"PUT", [NSData data], nil, completion);
        });
        assertRejected(^(MSIMECloudDictionaryCompletion completion) {
            MSIMEMutateCloudDictionary(@"POST", @"../other", nil, [NSData data], @"synthetic-token", completion);
        });
        assertRejected(^(MSIMECloudDictionaryCompletion completion) {
            MSIMEMutateCloudDictionary(@"DELETE", @"pinyin", @"../logout", [NSData data], @"synthetic-token", completion);
        });
        assertRejected(^(MSIMECloudDictionaryCompletion completion) {
            MSIMEFetchCloudDictionaryCatalog(@"pinyin", @"ni", 0, @"\n", @"xiaohe", @"synthetic-token", completion);
        });
    }
    return 0;
}
