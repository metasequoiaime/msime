#import "../../src/candidate/CandidateFontCache.h"
#import <CoreText/CoreText.h>
#include <cassert>

// A family no machine has installed: a copy of a stock macOS font whose family name is rewritten in place, so the test can register and remove a real font without touching the user's font set.
static NSString *const SyntheticFamily = @"MsimeFc Engraved LET";

static NSURL *WriteSyntheticFont(NSURL *directory) {
    NSData *stock = [NSData dataWithContentsOfFile:@"/System/Library/Fonts/Supplemental/Academy Engraved LET Fonts.ttf"];
    assert(stock.length);
    NSMutableData *font = [stock mutableCopy];
    NSData *from = [@"Academy" dataUsingEncoding:NSASCIIStringEncoding];
    NSData *to = [@"MsimeFc" dataUsingEncoding:NSASCIIStringEncoding];
    NSUInteger replaced = 0;
    for (NSRange range = [font rangeOfData:from options:0 range:NSMakeRange(0, font.length)]; range.location != NSNotFound;
         range = [font rangeOfData:from options:0 range:NSMakeRange(NSMaxRange(range), font.length - NSMaxRange(range))]) {
        [font replaceBytesInRange:range withBytes:to.bytes];
        replaced++;
    }
    assert(replaced > 0);
    NSURL *url = [directory URLByAppendingPathComponent:@"Synthetic.ttf"];
    assert([font writeToURL:url atomically:YES]);
    return url;
}

// Runs the main run loop, where CoreText delivers its font-change notification, until the condition holds.
static bool SpinUntil(bool (^condition)(void)) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:10];
    while (!condition() && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runMode:NSDefaultRunLoopMode beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
    return condition();
}

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];

        // A hit answers with the descriptor matched the first time, and the font built from it is the uncached one.
        NSFontDescriptor *menlo = MSIMEInstalledFontFamilyDescriptor(@"Menlo");
        assert(menlo);
        assert(MSIMEInstalledFontFamilyDescriptor(@"Menlo") == menlo);
        NSFontDescriptor *uncached = [[NSFontDescriptor fontDescriptorWithFontAttributes:@{NSFontFamilyAttribute:@"Menlo"}] matchingFontDescriptorWithMandatoryKeys:[NSSet setWithObject:NSFontFamilyAttribute]];
        assert([[NSFont fontWithDescriptor:menlo size:18] isEqual:[NSFont fontWithDescriptor:uncached size:18]]);
        assert([[NSFont fontWithDescriptor:menlo size:18].familyName isEqual:@"Menlo"]);

        // 可下载但还没下载的系统字体（字体预设里的楷体、圆体等）不拿去匹配：匹配会让 CoreText 经 FontRegistryUI 同步等待下载确认，调用线程就此挂起，CI 的 macOS 15 上设置窗口就是这样卡死的。答案只看本机已激活的字体族，所以这台机器下载过的照常命中，没下载的立即答 nil。
        NSSet<NSString *> *available = [NSSet setWithArray:(__bridge_transfer NSArray<NSString *> *)CTFontManagerCopyAvailableFontFamilyNames()];
        for (NSString *family in @[@"Songti SC", @"PingFang SC", @"Kaiti SC", @"STKaiti", @"Yuanti SC"])
            assert((MSIMEInstalledFontFamilyDescriptor(family) != nil) == [available containsObject:family]);

        // A family that is not installed resolves to nil, and so does every later lookup of it.
        assert(!MSIMEInstalledFontFamilyDescriptor(SyntheticFamily));
        assert(!MSIMEInstalledFontFamilyDescriptor(SyntheticFamily));

        NSURL *directory = [NSFileManager.defaultManager URLForDirectory:NSItemReplacementDirectory inDomain:NSUserDomainMask appropriateForURL:[NSURL fileURLWithPath:NSTemporaryDirectory()] create:YES error:nil];
        assert(directory);
        NSURL *fontURL = WriteSyntheticFont(directory);
        CFErrorRef error = NULL;
        assert(CTFontManagerRegisterFontsForURL((__bridge CFURLRef)fontURL, kCTFontManagerScopeProcess, &error));

        // The miss is cached: the system announces the new font through the run loop, which has not run yet, so the lookup still says missing.
        assert(!MSIMEInstalledFontFamilyDescriptor(SyntheticFamily));
        // AppKit's font-set notification clears the cache too.
        [NSNotificationCenter.defaultCenter postNotificationName:NSFontSetChangedNotification object:nil];
        NSFontDescriptor *installed = MSIMEInstalledFontFamilyDescriptor(SyntheticFamily);
        assert(installed);
        assert([[NSFont fontWithDescriptor:installed size:18].familyName isEqual:SyntheticFamily]);

        // Removing the font is picked up from the system's own font-change notifications, with nothing posted by hand.
        assert(CTFontManagerUnregisterFontsForURL((__bridge CFURLRef)fontURL, kCTFontManagerScopeProcess, &error));
        assert(MSIMEInstalledFontFamilyDescriptor(SyntheticFamily) == installed);
        assert(SpinUntil(^{ return MSIMEInstalledFontFamilyDescriptor(SyntheticFamily) == nil; }));
        // Menlo was dropped with everything else and matches again.
        assert([MSIMEInstalledFontFamilyDescriptor(@"Menlo") isEqual:menlo]);
        [NSFileManager.defaultManager removeItemAtURL:directory error:nil];

        // Lookups and clears from many threads at once neither crash nor return a wrong family.
        NSFontDescriptor *helvetica = MSIMEInstalledFontFamilyDescriptor(@"Helvetica");
        assert(helvetica);
        dispatch_apply(2000, dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^(size_t index) {
            @autoreleasepool {
                if (index % 50 == 0) [NSNotificationCenter.defaultCenter postNotificationName:NSFontSetChangedNotification object:nil];
                assert([MSIMEInstalledFontFamilyDescriptor(index % 2 ? @"Menlo" : @"Helvetica") isEqual:index % 2 ? menlo : helvetica]);
                assert(!MSIMEInstalledFontFamilyDescriptor(@"MSIME Synthetic Unavailable Family"));
            }
        });
    }
    return 0;
}
