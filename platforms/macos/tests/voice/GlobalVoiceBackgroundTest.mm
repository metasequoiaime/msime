#import "../../src/voice/GlobalVoiceBackground.h"
#import "../settings/TestPreferenceSuite.h"
#include <cassert>
@interface BackgroundService : NSObject
@property(nonatomic) SMAppServiceStatus status;
@property(nonatomic) NSUInteger registrations;
@property(nonatomic) NSUInteger removals;
@property(nonatomic) BOOL failRegister;
@property(nonatomic, copy) void (^removal)(NSError *);
@end
@implementation BackgroundService
- (BOOL)registerAndReturnError:(NSError **)error {
    ++_registrations;
    if (_failRegister) { if (error) *error = [NSError errorWithDomain:@"synthetic" code:1 userInfo:nil]; return NO; }
    _status = SMAppServiceStatusEnabled; return YES;
}
- (BOOL)unregisterAndReturnError:(NSError **)error { (void)error; ++_removals; _status = SMAppServiceStatusNotRegistered; return YES; }
- (void)unregisterWithCompletionHandler:(void (^)(NSError *))reply { ++_removals; _removal = reply; }
@end
static void Pump() { [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]]; }
int main() { @autoreleasepool {
    NSString *suite = [@"msime.synthetic.background." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEGlobalVoiceBackground *registration = [MSIMEGlobalVoiceBackground new];
    BackgroundService *service = [BackgroundService new]; service.status = SMAppServiceStatusNotFound;
    __block NSUInteger changes = 0;
    void (^changed)(void) = ^{ ++changes; };
    auto configure = [&](BOOL enabled, NSString *revision) {
        [registration configureService:(id)service enabled:enabled revision:revision defaults:defaults changed:changed];
    };
    configure(YES, @"A"); configure(YES, @"A");
    assert(service.registrations == 1 && !registration.busy && !registration.error);
    configure(YES, @"B");
    assert(registration.busy && service.registrations == 1 && service.removals == 1);
    configure(YES, @"B"); assert(service.removals == 1);
    // 注销完成之前不能登记新版，完成后再经过主线程一轮才登记。
    service.status = SMAppServiceStatusEnabled; service.removal(nil);
    assert(service.registrations == 1); Pump();
    assert(service.registrations == 2 && changes == 1 && !registration.busy);
    configure(YES, @"B"); assert(service.registrations == 2);
    service.status = SMAppServiceStatusRequiresApproval;
    configure(YES, @"C");
    assert(service.registrations == 2 && service.removals == 1);
    configure(NO, @"C"); assert(service.removals == 2);
    service.failRegister = YES; configure(YES, @"C");
    assert(registration.error && service.registrations == 3);
    service.failRegister = NO; configure(YES, @"C");
    assert(!registration.error && service.registrations == 4);
    configure(YES, @"D"); assert(registration.busy);
    // 更新登记未完成时用户关闭功能，迟到的完成不能把后台项重新启用。
    configure(NO, @"D");
    service.status = SMAppServiceStatusNotRegistered; service.removal(nil); Pump();
    assert(service.registrations == 4 && !registration.busy);
    configure(YES, @"D"); assert(service.registrations == 5);
    configure(YES, @"E");
    service.removal([NSError errorWithDomain:@"synthetic" code:2 userInfo:nil]); Pump();
    assert(registration.error && !registration.busy && service.registrations == 5);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
} }
