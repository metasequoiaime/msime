#import "../../src/core/WindowPresentation.h"
#include <cassert>

@interface RecordingWindow : NSWindow
@property(nonatomic) NSUInteger keyOrders;
@property(nonatomic) NSUInteger regardlessOrders;
@end
@implementation RecordingWindow
- (void)makeKeyAndOrderFront:(id)sender { (void)sender; ++self.keyOrders; }
- (void)orderFrontRegardless { ++self.regardlessOrders; }
@end

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];
        // The input method is never the active application, so ordering the window among its own windows is not enough: it has to go above the editor's even when activation is refused.
        RecordingWindow *window = [[RecordingWindow alloc] initWithContentRect:NSMakeRect(0, 0, 100, 100)
            styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:YES];
        MSIMEPresentWindow(window);
        assert(window.keyOrders == 1 && window.regardlessOrders == 1);
        // A background-only process cannot become active at all; presenting a window lifts it to Accessory, never to Regular.
        assert(NSApp.activationPolicy == NSApplicationActivationPolicyAccessory);
        MSIMEPresentWindow(nil);
    }
    return 0;
}
