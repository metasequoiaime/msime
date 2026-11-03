#import "CandidateFontCache.h"
#import <CoreText/CoreText.h>
#include <os/lock.h>

static os_unfair_lock CandidateFontCacheLock = OS_UNFAIR_LOCK_INIT;
// 缓存键是字体族名（常规字重）或「族名\n字重」，值是匹配到的 NSFontDescriptor，没装的族是 NSNull。族名不含控制字符，换行不会和族名撞上。
static NSMutableDictionary<NSString *, id> *CandidateFontCache;
static NSMutableOrderedSet<NSString *> *CandidateFontCacheOrder;
static const NSUInteger CandidateFontCacheCapacity = 128;
// Bumped on every clear, so a match that started before a font change cannot store its now stale answer after the clear.
static uint64_t CandidateFontCacheGeneration;
// 本机已激活、可以直接使用的字体族名，nil 表示还没读过。可下载的系统字体（楷体、圆体等）在下载之前不在其中。
static NSSet<NSString *> *CandidateAvailableFontFamilies;

static void ClearCandidateFontCache() {
    os_unfair_lock_lock(&CandidateFontCacheLock);
    [CandidateFontCache removeAllObjects];
    [CandidateFontCacheOrder removeAllObjects];
    CandidateAvailableFontFamilies = nil;
    CandidateFontCacheGeneration++;
    os_unfair_lock_unlock(&CandidateFontCacheLock);
}

NSFontDescriptor *MSIMEInstalledFontFamilyDescriptor(NSString *family) {
    return MSIMEInstalledFontFamilyDescriptorWithWeight(family, NSFontWeightRegular);
}

NSFontDescriptor *MSIMEInstalledFontFamilyDescriptorWithWeight(NSString *family, NSFontWeight weight) {
    // 常规字重沿用原来只按族名匹配的写法和缓存键，默认画法与加入字重之前逐字节相同。
    const BOOL regular = weight == NSFontWeightRegular;
    NSString *key = regular ? family : [NSString stringWithFormat:@"%@\n%.3f", family, weight];
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        CandidateFontCache = [NSMutableDictionary dictionary];
        CandidateFontCacheOrder = [NSMutableOrderedSet orderedSet];
        NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
        // Both fire, on the main thread, for a font registered or removed in this process or in any other, persistent user-wide installs included. Measured on macOS 27, AppKit's NSFontSetChangedNotification only starts once the process has created an NSFont, so CoreText's own notification is observed as well.
        [center addObserverForName:(__bridge NSString *)kCTFontManagerRegisteredFontsChangedNotification object:nil queue:nil usingBlock:^(NSNotification *) { ClearCandidateFontCache(); }];
        [center addObserverForName:NSFontSetChangedNotification object:nil queue:nil usingBlock:^(NSNotification *) { ClearCandidateFontCache(); }];
    });
    os_unfair_lock_lock(&CandidateFontCacheLock);
    id cached = CandidateFontCache[key];
    NSSet<NSString *> *available = CandidateAvailableFontFamilies;
    const uint64_t generation = CandidateFontCacheGeneration;
    if (cached) {
        [CandidateFontCacheOrder removeObject:key];
        [CandidateFontCacheOrder addObject:key];
    }
    os_unfair_lock_unlock(&CandidateFontCacheLock);
    if (!cached) {
        // 只拿已激活的字体族去匹配。可下载但还没下载的系统字体（如 CI 的 macOS 15 上的「Kaiti SC」「Yuanti SC」）一经匹配，CoreText 就走 TDownloadableFontManager::Download，经 FontRegistryUI 同步等待下载确认，调用线程一直卡住：设置窗口的字体预设、以及把候选字体设成这类字体后的每次候选绘制都会因此挂起。这里只回答装没装，不该触发下载。
        if (!available) {
            available = [NSSet setWithArray:(__bridge_transfer NSArray<NSString *> *)CTFontManagerCopyAvailableFontFamilyNames()];
            os_unfair_lock_lock(&CandidateFontCacheLock);
            if (generation == CandidateFontCacheGeneration) CandidateAvailableFontFamilies = available;
            os_unfair_lock_unlock(&CandidateFontCacheLock);
        }
        // The match runs outside the lock because it is the slow part; two threads racing on one family only store equal answers.
        // 指定字重时族名仍是必须匹配的键，字重只是偏好：族里没有这一档就落到 CoreText 选的就近字面，不会换族，也不会合成加粗。
        NSDictionary *attributes = regular ? @{NSFontFamilyAttribute:family}
                                           : @{NSFontFamilyAttribute:family, NSFontTraitsAttribute:@{NSFontWeightTrait:@(weight)}};
        NSFontDescriptor *requested = [NSFontDescriptor fontDescriptorWithFontAttributes:attributes];
        cached = [available containsObject:family]
            ? [requested matchingFontDescriptorWithMandatoryKeys:[NSSet setWithObject:NSFontFamilyAttribute]] ?: NSNull.null
            : NSNull.null;
        os_unfair_lock_lock(&CandidateFontCacheLock);
        if (generation == CandidateFontCacheGeneration) {
            [CandidateFontCacheOrder removeObject:key];
            CandidateFontCache[key] = cached;
            [CandidateFontCacheOrder addObject:key];
            while (CandidateFontCacheOrder.count > CandidateFontCacheCapacity) {
                NSString *oldest = CandidateFontCacheOrder.firstObject;
                [CandidateFontCacheOrder removeObjectAtIndex:0];
                [CandidateFontCache removeObjectForKey:oldest];
            }
        }
        os_unfair_lock_unlock(&CandidateFontCacheLock);
    }
    return cached == NSNull.null ? nil : cached;
}
