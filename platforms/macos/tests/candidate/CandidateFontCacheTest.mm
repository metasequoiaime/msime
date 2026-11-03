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

static void TestUnavailableFamilyKeysAreReleased() {
    // 不运行主循环、也不变更系统字体集，单独验证配置持续换名时缓存本身会回收旧 key。
    [NSNotificationCenter.defaultCenter postNotificationName:NSFontSetChangedNotification object:nil];
    __weak NSString *oldFamily;
    @autoreleasepool {
        NSString *family = [[NSString alloc] initWithFormat:@"MSIME Synthetic Cache Capacity Family %lu", (unsigned long)0];
        oldFamily = family;
        assert(!MSIMEInstalledFontFamilyDescriptor(family));
    }
    assert(oldFamily != nil);

    __weak NSString *newestFamily;
    for (NSUInteger index = 1; index <= 128; ++index) {
        @autoreleasepool {
            NSString *family = [[NSString alloc] initWithFormat:@"MSIME Synthetic Cache Capacity Family %lu", (unsigned long)index];
            newestFamily = family;
            assert(!MSIMEInstalledFontFamilyDescriptor(family));
        }
    }
    assert(oldFamily == nil && "旧的字体族名仍被缓存永久持有");
    assert(newestFamily != nil);
    [NSNotificationCenter.defaultCenter postNotificationName:NSFontSetChangedNotification object:nil];
    assert(newestFamily == nil);
}

static void TestWeightedFamilies() {
    // 常规字重就是原来的族匹配，连缓存里的描述符都是同一个，默认画法不变。
    assert(MSIMEInstalledFontFamilyDescriptorWithWeight(@"Helvetica", NSFontWeightRegular) == MSIMEInstalledFontFamilyDescriptor(@"Helvetica"));
    // 选了粗体就落到这个族自己的粗体字面，结果按族和字重一起缓存。
    NSFontDescriptor *bold = MSIMEInstalledFontFamilyDescriptorWithWeight(@"Helvetica", NSFontWeightBold);
    assert(bold);
    NSFont *boldFont = [NSFont fontWithDescriptor:bold size:18];
    assert([boldFont.familyName isEqual:@"Helvetica"]);
    assert(boldFont.fontDescriptor.symbolicTraits & NSFontDescriptorTraitBold);
    assert(![[NSFont fontWithDescriptor:MSIMEInstalledFontFamilyDescriptor(@"Helvetica") size:18].fontName isEqual:boldFont.fontName]);
    assert(MSIMEInstalledFontFamilyDescriptorWithWeight(@"Helvetica", NSFontWeightBold) == bold);
    // 族里没有这一档字面时退回族里就近的字面，不换成别的族，也不合成加粗。
    NSFontDescriptor *regularOnly = MSIMEInstalledFontFamilyDescriptor(@"Academy Engraved LET");
    assert(regularOnly);
    NSFontDescriptor *nearest = MSIMEInstalledFontFamilyDescriptorWithWeight(@"Academy Engraved LET", NSFontWeightBold);
    assert(nearest);
    assert([[NSFont fontWithDescriptor:nearest size:18].fontName isEqual:[NSFont fontWithDescriptor:regularOnly size:18].fontName]);
    // 没装的族不论字重都是 nil。
    assert(!MSIMEInstalledFontFamilyDescriptorWithWeight(@"MSIME Synthetic Unavailable Family", NSFontWeightBold));

    // 按 CSS 的就近规则挑字面，与 DirectWrite、fontconfig 一致：只有常规和粗体的族选中等时退到常规，不像 CoreText 的非必需键匹配那样画成粗体；半粗先找更粗的字面；斜体字面不算。
    const auto faceName = [](NSString *family, NSFontWeight weight) {
        return [NSFont fontWithDescriptor:MSIMEInstalledFontFamilyDescriptorWithWeight(family, weight) size:18].fontName;
    };
    assert([faceName(@"Helvetica", NSFontWeightMedium) isEqual:faceName(@"Helvetica", NSFontWeightRegular)]);
    assert([faceName(@"Menlo", NSFontWeightMedium) isEqual:@"Menlo-Regular"]);
    assert([faceName(@"Menlo", NSFontWeightSemibold) isEqual:@"Menlo-Bold"]);
    assert([faceName(@"Menlo", NSFontWeightBold) isEqual:@"Menlo-Bold"]);
    assert([faceName(@"Helvetica", NSFontWeightSemibold) isEqual:@"Helvetica-Bold"]);
    // 族里真有这一档时就用它（苹方有中等和半粗字面）。
    assert([faceName(@"PingFang SC", NSFontWeightMedium) isEqual:@"PingFangSC-Medium"]);
    assert([faceName(@"PingFang SC", NSFontWeightSemibold) isEqual:@"PingFangSC-Semibold"]);
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

        TestUnavailableFamilyKeysAreReleased();
        TestWeightedFamilies();

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
