#import "../../src/voice/VoiceInputService.h"
#include <cassert>

@interface MalformedVoiceSession : MSIMEClientSession
@property(nonatomic, strong) id generationValue;
@end
@implementation MalformedVoiceSession
- (NSDictionary *)startVoiceWithError:(NSError **)error {
    (void)error;
    return @{@"generation":self.generationValue};
}
@end

static void Pump(BOOL *done) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5];
    while (!*done && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    assert(*done);
}

int main() {
    @autoreleasepool {
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSMutableDictionary *options = [@{@"api_version": @1, @"preferences": @{@"scheme": @"quanpin", @"candidate_page_size": @5, @"learning": @NO, @"chinese_punctuation": @YES}} mutableCopy];
        for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
            NSString *path = [root stringByAppendingPathComponent:name];
            assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
            options[name] = path;
        }
        for (id invalid in @[@YES, @1.5, @(-1)]) {
            MalformedVoiceSession *session = [[MalformedVoiceSession alloc] initWithOptions:options error:nil];
            assert(session);
            session.generationValue = invalid;
            MSIMEVoiceInputService *service = [MSIMEVoiceInputService new];
            uint64_t generation = 0;
            assert(![service startWithSession:session generation:&generation error:nil] && !service.active);
        }
        NSError *error = nil;
        MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
        assert(session && !error);
        MSIMEVoiceInputService *service = [MSIMEVoiceInputService new];
        uint64_t generation = 0;
        assert([service startWithSession:session generation:&generation error:&error] && generation);
        __block BOOL done = NO;
        [service applyText:@"synthetic delivery" generation:generation completion:^(NSDictionary *result, NSError *failure) {
            assert(NSThread.isMainThread && !failure);
            assert([result[@"commit"] isEqual:@"synthetic delivery"]);
            done = YES;
        }];
        Pump(&done);
        assert([service cancelWithError:&error]);
        assert([service startWithSession:session generation:&generation error:&error]);
        done = NO;
        [service applyText:@"synthetic cancelled" generation:generation completion:^(NSDictionary *result, NSError *failure) {
            assert(NSThread.isMainThread && !failure && !result);
            done = YES;
        }];
        assert([service cancelWithError:&error]);
        assert([service startWithSession:session generation:&generation error:&error]);
        Pump(&done);
        assert([[[session applyVoiceText:@"synthetic successor" generation:generation error:&error] objectForKey:@"commit"] isEqual:@"synthetic successor"]);
        assert([service cancelWithError:&error]);
        assert([session closeWithError:&error]);
        assert([NSFileManager.defaultManager removeItemAtPath:root error:&error]);
    }
}
