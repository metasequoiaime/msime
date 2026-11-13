#import "../../src/input/ClientKeyboardLayout.h"
#include <cassert>

@interface LayoutClient : NSObject
@property(nonatomic, copy) NSString *layout;
@property(nonatomic) NSUInteger calls;
@end

@implementation LayoutClient
- (void)overrideKeyboardWithKeyboardNamed:(NSString *)identifier {
    self.layout = identifier;
    self.calls += 1;
}
@end

static NSUInteger overrideCalls = 0;
static NSString *overriddenLayout;
static OSStatus RecordOverride(TISInputSourceRef source) {
    overriddenLayout = [(__bridge NSString *)TISGetInputSourceProperty(source, kTISPropertyInputSourceID) copy];
    ++overrideCalls;
    return noErr;
}

int main() {
    @autoreleasepool {
        LayoutClient *client = [LayoutClient new];
        MSIMEOverrideClientKeyboardLayout(client, @"com.apple.keylayout.Dvorak");
        assert([client.layout isEqualToString:@"com.apple.keylayout.Dvorak"]);
        assert(client.calls == 1);
        MSIMEOverrideClientKeyboardLayout(client, @"com.apple.keylayout.Colemak");
        assert([client.layout isEqualToString:@"com.apple.keylayout.Colemak"]);
        assert(client.calls == 2);
        MSIMEOverrideClientKeyboardLayout(client, nil);
        MSIMEOverrideClientKeyboardLayout(client, @"");
        assert(client.calls == 2);
        MSIMEPrepareClientKeyboardLayout(nil, RecordOverride);
        MSIMEPrepareClientKeyboardLayout([NSObject new], RecordOverride);
        assert(overrideCalls == 0);
        TISInputSourceRef source = TISCopyCurrentASCIICapableKeyboardLayoutInputSource();
        if (source) {
            NSString *expected = (__bridge NSString *)TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
            MSIMEPrepareClientKeyboardLayout(client, RecordOverride);
            assert([client.layout isEqualToString:expected]);
            assert([overriddenLayout isEqualToString:expected]);
            assert(client.calls == 3);
            assert(overrideCalls == 1);
            MSIMEPrepareClientKeyboardLayout(client, RecordOverride);
            assert(client.calls == 4);
            assert(overrideCalls == 2);
            CFRelease(source);
        }
    }
}
