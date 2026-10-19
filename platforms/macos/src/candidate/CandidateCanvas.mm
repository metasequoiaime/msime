#import "CandidateCanvas.h"
#import <ImageIO/ImageIO.h>
#import <QuartzCore/QuartzCore.h>
#include <array>
#include <cmath>

@interface MSIMECanvasFrames : NSObject
@property(nonatomic, copy) NSArray *images;
@property(nonatomic, copy) NSArray<NSNumber *> *times;
@property(nonatomic) double duration;
@property(nonatomic) NSUInteger loops;
@end
@implementation MSIMECanvasFrames
@end

namespace
{
// 与共享 catalog 的上限一致，防止扫描完成后被替换的图片在原生解码时突破预算。
MSIMECanvasFrames *LoadFrames(const std::string &path, uint64_t &remaining)
{
    if (path.empty())
        return nil;
    CGImageSourceRef source = CGImageSourceCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(path.c_str())], nullptr);
    if (!source)
        return nil;
    const size_t count = CGImageSourceGetCount(source);
    if (count == 0 || count > 120)
    {
        CFRelease(source);
        return nil;
    }
    NSMutableArray *images = [NSMutableArray array];
    NSMutableArray<NSNumber *> *times = [NSMutableArray array];
    double elapsed = 0;
    NSDictionary *container = CFBridgingRelease(CGImageSourceCopyProperties(source, nullptr));
    NSNumber *loops = container[@"{PNG}"][@"LoopCount"] ?: container[@"{GIF}"][@"LoopCount"] ?: container[@"{WebP}"][@"LoopCount"];
    const BOOL gif = [(__bridge NSString *)CGImageSourceGetType(source) isEqual:@"com.compuserve.gif"];
    for (size_t at = 0; at < count; ++at)
    {
        NSDictionary *properties = CFBridgingRelease(CGImageSourceCopyPropertiesAtIndex(source, at, nullptr));
        const uint64_t width = [properties[(__bridge NSString *)kCGImagePropertyPixelWidth] unsignedLongLongValue];
        const uint64_t height = [properties[(__bridge NSString *)kCGImagePropertyPixelHeight] unsignedLongLongValue];
        if (!width || !height || width > 2048 || height > 2048 || width * height > remaining)
        {
            CFRelease(source);
            return nil;
        }
        remaining -= width * height;
        CGImageRef image = CGImageSourceCreateImageAtIndex(
            source, at,
            (__bridge CFDictionaryRef)
                @{(__bridge NSString *)kCGImageSourceShouldCacheImmediately : @YES});
        if (!image)
        {
            CFRelease(source);
            return nil;
        }
        [images addObject:CFBridgingRelease(image)];
        [times addObject:@(elapsed)];
        NSDictionary *animation = properties[@"{PNG}"] ?: properties[@"{GIF}"] ?: properties[@"{WebP}"];
        NSNumber *delay = animation[@"UnclampedDelayTime"] ?: animation[@"DelayTime"];
        const double seconds = [delay isKindOfClass:NSNumber.class] ? delay.doubleValue : 0;
        // 与 msime-windows 和浏览器一致：10ms 及以下的帧按 100ms 播放。
        const double milliseconds = std::isfinite(seconds) && seconds > 0 ? std::round(seconds * 1000) : 0;
        elapsed += milliseconds > 10 ? milliseconds / 1000 : 0.1;
    }
    CFRelease(source);
    for (NSUInteger at = 0; at < times.count; ++at)
        times[at] = @([times[at] doubleValue] / elapsed);
    [times addObject:@1];
    // 离散动画的终点显式保留最后一帧，有限循环结束后不会闪回第一帧。
    [images addObject:images.lastObject];
    MSIMECanvasFrames *frames = [MSIMECanvasFrames new];
    frames.images = images;
    frames.times = times;
    frames.duration = elapsed;
    frames.loops = [loops isKindOfClass:NSNumber.class] ? loops.unsignedIntegerValue : 0;
    // ImageIO 已将 GIF 扩展中的重复次数换成总轮数；没有扩展只播放一轮。
    if (gif && !loops)
        frames.loops = 1;
    return frames;
}
std::array<double, 4> Axis(double length, double before, double after)
{
    const double factor = before + after > length ? length / (before + after) : 1;
    return {0, before * factor, length - after * factor, length};
}
MSIMECanvasFrames *Crop(MSIMECanvasFrames *source, CGRect rect)
{
    if (rect.size.width <= 0 || rect.size.height <= 0)
        return nil;
    NSMutableArray *images = [NSMutableArray array];
    for (id raw in source.images)
    {
        CGImageRef cropped = CGImageCreateWithImageInRect((__bridge CGImageRef)raw, rect);
        if (!cropped)
            return nil;
        [images addObject:CFBridgingRelease(cropped)];
    }
    MSIMECanvasFrames *result = [MSIMECanvasFrames new];
    result.images = images;
    result.times = source.times;
    result.duration = source.duration;
    result.loops = source.loops;
    return result;
}
}

@interface MSIMECandidateCanvasAssets ()
@property(nonatomic, copy) NSArray *tiles;
@property(nonatomic, copy) NSArray *widgets;
@end
@implementation MSIMECandidateCanvasAssets
+ (instancetype)assetsForCanvas:(const msime::mac::SkinCanvas &)canvas dark:(BOOL)dark
{
    MSIMECandidateCanvasAssets *assets = [self new];
    uint64_t remaining = 8000000;
    NSMutableDictionary<NSString *, MSIMECanvasFrames *> *loaded = [NSMutableDictionary dictionary];
    auto load = [&](const std::string &path) -> MSIMECanvasFrames * {
        NSString *key = @(path.c_str());
        MSIMECanvasFrames *frames = loaded[key];
        if (!frames)
        {
            frames = LoadFrames(path, remaining);
            if (frames)
                loaded[key] = frames;
        }
        return frames;
    };
    NSMutableArray *tiles = [NSMutableArray array];
    if (canvas.border)
    {
        const auto &border = *canvas.border;
        MSIMECanvasFrames *panel = load(!dark && !border.lightSource.empty() ? border.lightSource : border.source);
        if (panel)
        {
            CGImageRef image = (__bridge CGImageRef)panel.images.firstObject;
            const auto x = Axis(CGImageGetWidth(image), border.slice[3], border.slice[1]);
            const auto y = Axis(CGImageGetHeight(image), border.slice[0], border.slice[2]);
            for (NSUInteger row = 0; row < 3; ++row)
                for (NSUInteger col = 0; col < 3; ++col)
                {
                    MSIMECanvasFrames *tile = Crop(panel, CGRectMake(x[col], y[row], x[col + 1] - x[col], y[row + 1] - y[row]));
                    [tiles addObject:tile ?: (id)NSNull.null];
                }
        }
    }
    assets.tiles = tiles;
    NSMutableArray *widgets = [NSMutableArray array];
    for (const auto &decoration : canvas.decorations)
        [widgets addObject:load(decoration.image) ?: (id)NSNull.null];
    assets.widgets = widgets;
    return assets;
}
- (BOOL)panelLoaded
{
    return self.tiles.count == 9;
}
@end

@implementation MSIMECandidateCanvasView
{
    NSMutableArray<CALayer *> *_sprites;
    NSMutableArray<MSIMECanvasFrames *> *_frames;
    BOOL _playbackVisible;
    BOOL _asleep;
    CFTimeInterval _anchor;
}
- (instancetype)initWithFrame:(NSRect)frame
                       assets:(MSIMECandidateCanvasAssets *)assets
                       canvas:(const msime::mac::SkinCanvas &)canvas
                         card:(NSRect)card
                        level:(NSInteger)level
{
    self = [super initWithFrame:frame];
    if (!self)
        return nil;
    self.wantsLayer = YES;
    _sprites = [NSMutableArray array];
    _frames = [NSMutableArray array];
    auto add = [&](id raw, NSRect rect) {
        if (![raw isKindOfClass:MSIMECanvasFrames.class] || NSIsEmptyRect(rect))
            return;
        MSIMECanvasFrames *frames = raw;
        CALayer *sprite = [CALayer layer];
        sprite.frame = rect;
        sprite.contents = frames.images.firstObject;
        sprite.contentsGravity = kCAGravityResize;
        sprite.opacity = level == 0 ? static_cast<float>(canvas.panelOpacity) : 1.0f;
        [self.layer addSublayer:sprite];
        [_sprites addObject:sprite];
        [_frames addObject:frames];
    };
    if (level == 0 && assets.panelLoaded && canvas.border)
    {
        const auto &border = *canvas.border;
        const auto x = Axis(card.size.width, border.width[3], border.width[1]);
        const auto y = Axis(card.size.height, border.width[0], border.width[2]);
        for (NSUInteger row = 0; row < 3; ++row)
            for (NSUInteger col = 0; col < 3; ++col)
            {
                if (row == 1 && col == 1 && !border.fill)
                    continue;
                add(assets.tiles[row * 3 + col],
                    NSMakeRect(card.origin.x + x[col], NSMaxY(card) - y[row + 1], x[col + 1] - x[col], y[row + 1] - y[row]));
            }
    }
    if (level != 0)
        for (NSUInteger at = 0; at < canvas.decorations.size(); ++at)
        {
            const auto &deco = canvas.decorations[at];
            if ((deco.zIndex < 1 ? -1 : 1) != level)
                continue;
            id raw = assets.widgets[at];
            if (![raw isKindOfClass:MSIMECanvasFrames.class])
                continue;
            CGImageRef image = (__bridge CGImageRef)((MSIMECanvasFrames *)raw).images.firstObject;
            const double ratio = std::min(deco.width / CGImageGetWidth(image), deco.height / CGImageGetHeight(image));
            const CGFloat width = CGImageGetWidth(image) * ratio, height = CGImageGetHeight(image) * ratio;
            const bool left = deco.anchor.find("left") != std::string::npos;
            const bool right = deco.anchor.find("right") != std::string::npos;
            const bool top = deco.anchor.find("top") != std::string::npos;
            const bool bottom = deco.anchor.find("bottom") != std::string::npos;
            const CGFloat boxX = left ? NSMinX(card) : right ? NSMaxX(card) - deco.width : NSMidX(card) - deco.width / 2;
            const CGFloat boxY = top ? NSMaxY(card) - deco.height : bottom ? NSMinY(card) : NSMidY(card) - deco.height / 2;
            add(raw, NSMakeRect(boxX + deco.offsetX + (deco.width - width) / 2, boxY - deco.offsetY + (deco.height - height) / 2, width,
                                height));
        }
    NSNotificationCenter *center = NSWorkspace.sharedWorkspace.notificationCenter;
    [center addObserver:self
               selector:@selector(motionChanged:)
                   name:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification
                 object:nil];
    [center addObserver:self selector:@selector(sleep:) name:NSWorkspaceScreensDidSleepNotification object:nil];
    [center addObserver:self selector:@selector(wake:) name:NSWorkspaceScreensDidWakeNotification object:nil];
    return self;
}
- (void)dealloc
{
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
}
- (BOOL)isOpaque
{
    return NO;
}
- (NSView *)hitTest:(NSPoint)point
{
    (void)point;
    return nil;
}
- (void)motionChanged:(NSNotification *)notification
{
    (void)notification;
    [self refreshPlayback];
}
- (void)sleep:(NSNotification *)notification
{
    (void)notification;
    _asleep = YES;
    [self refreshPlayback];
}
- (void)wake:(NSNotification *)notification
{
    (void)notification;
    _asleep = NO;
    [self refreshPlayback];
}
- (void)setPlaybackVisible:(BOOL)visible anchor:(CFTimeInterval)anchor
{
    _playbackVisible = visible;
    _anchor = anchor;
    [self refreshPlayback];
}
- (void)refreshPlayback
{
    const BOOL play = _playbackVisible && !_asleep && !NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion;
    for (NSUInteger at = 0; at < _sprites.count; ++at)
    {
        CALayer *sprite = _sprites[at];
        MSIMECanvasFrames *frames = _frames[at];
        [sprite removeAnimationForKey:@"msime-canvas-frames"];
        if (!play || frames.images.count < 3)
            continue;
        CAKeyframeAnimation *animation = [CAKeyframeAnimation animationWithKeyPath:@"contents"];
        animation.values = frames.images;
        animation.keyTimes = frames.times;
        animation.duration = frames.duration;
        animation.calculationMode = kCAAnimationDiscrete;
        animation.repeatCount = frames.loops == 0 ? HUGE_VALF : static_cast<float>(frames.loops);
        animation.beginTime = [sprite convertTime:_anchor fromLayer:nil];
        animation.fillMode = kCAFillModeForwards;
        animation.removedOnCompletion = NO;
        [sprite addAnimation:animation forKey:@"msime-canvas-frames"];
    }
}
@end
