#import "CandidateFontCache.h"
#import <CoreText/CoreText.h>
#include <os/lock.h>
#include <cmath>
#include <utility>

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

static NSDictionary *CandidateFaceTraits(NSFontDescriptor *face) {
    return (__bridge_transfer NSDictionary *)CTFontDescriptorCopyAttribute((__bridge CTFontDescriptorRef)face, kCTFontTraitsAttribute);
}

// 按 CSS Fonts 的字重匹配规则在族里挑字面，与 DirectWrite、fontconfig 的就近选法一致：要的字重在常规到中等之间（400–500）时，先取不超过中等、不轻于它的字面，再取更轻的，最后才取更粗的；要的字重比中等粗时先取更粗的，再取更轻的。CoreText 把字重当非必需键匹配时直接取更粗的字面，只有常规和粗体的族（Helvetica、Menlo、宋体）选中等会画成粗体，和另外两端不一样。只在与常规字面同宽、同为正体的字面里挑，斜体和窄体不算。
static NSFontDescriptor *CandidateWeightedFace(NSString *family, NSFontDescriptor *regularFace, NSFontWeight weight) {
    // 同一档字重在不同字体里可能差在小数点后几位，比较时留一点余量。
    const CGFloat epsilon = 0.01;
    const auto traitValue = [](NSDictionary *traits, CFStringRef key) { return [traits[(__bridge NSString *)key] doubleValue]; };
    // 排序键：先比所在的优先区间（0 最好），同一区间里比离要的字重有多远。
    const auto rank = [&](CGFloat faceWeight) -> std::pair<int, CGFloat> {
        const CGFloat distance = std::fabs(faceWeight - weight);
        if (distance <= epsilon) return {0, 0};
        if (weight <= NSFontWeightMedium + epsilon) {
            if (faceWeight > weight && faceWeight <= NSFontWeightMedium + epsilon) return {1, distance};
            return {faceWeight < weight ? 2 : 3, distance};
        }
        return {faceWeight > weight ? 1 : 2, distance};
    };
    NSDictionary *regularTraits = CandidateFaceTraits(regularFace);
    const CGFloat regularWidth = traitValue(regularTraits, kCTFontWidthTrait);
    NSFontDescriptor *best = regularFace;
    auto bestRank = rank(traitValue(regularTraits, kCTFontWeightTrait));
    NSFontDescriptor *query = [NSFontDescriptor fontDescriptorWithFontAttributes:@{NSFontFamilyAttribute:family}];
    for (NSFontDescriptor *face in [query matchingFontDescriptorsWithMandatoryKeys:[NSSet setWithObject:NSFontFamilyAttribute]]) {
        NSDictionary *traits = CandidateFaceTraits(face);
        if ([traits[(__bridge NSString *)kCTFontSymbolicTrait] unsignedIntValue] & kCTFontTraitItalic) continue;
        if (std::fabs(traitValue(traits, kCTFontWidthTrait) - regularWidth) > epsilon) continue;
        const auto faceRank = rank(traitValue(traits, kCTFontWeightTrait));
        if (faceRank < bestRank) {
            best = face;
            bestRank = faceRank;
        }
    }
    return best;
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
        NSFontDescriptor *requested = [NSFontDescriptor fontDescriptorWithFontAttributes:@{NSFontFamilyAttribute:family}];
        NSFontDescriptor *matched = [available containsObject:family]
            ? [requested matchingFontDescriptorWithMandatoryKeys:[NSSet setWithObject:NSFontFamilyAttribute]]
            : nil;
        // 指定字重时在同一个族里按 CSS 的就近规则挑字面（CandidateWeightedFace），不换族，也不合成加粗。
        if (matched && !regular) matched = CandidateWeightedFace(family, matched, weight);
        cached = matched ?: NSNull.null;
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
