#import "ToolbarImageCanvas.h"
#import <QuartzCore/QuartzCore.h>
#include "../../../common/SkinImageAnimation.h"
#include <cmath>

@interface MSIMEToolbarImageAsset : NSObject
+ (instancetype)load:(const std::filesystem::path &)directory relative:(NSString *)relative remaining:(uint64_t &)remaining;
- (void)drawInRect:(NSRect)rect elapsed:(uint64_t)elapsed;
- (BOOL)opaqueAt:(NSPoint)point size:(NSSize)size;
@end
@implementation MSIMEToolbarImageAsset {
    std::optional<msime::skin::ImageAnimation> _decoded;
    NSArray<NSImage *> *_images;
}
+ (instancetype)load:(const std::filesystem::path &)directory relative:(NSString *)relative remaining:(uint64_t &)remaining {
    if (![relative isKindOfClass:NSString.class]) return nil;
    auto decoded = msime::skin::ImageAnimation::load(directory, relative.UTF8String, remaining);
    if (!decoded) return nil;
    MSIMEToolbarImageAsset *asset = [self new];
    NSMutableArray<NSImage *> *images = [NSMutableArray array];
    CGColorSpaceRef color = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    for (auto &frame : decoded->frames) {
        CFDataRef bytes = CFDataCreate(kCFAllocatorDefault, frame.rgba.data(), frame.rgba.size());
        CGDataProviderRef provider = CGDataProviderCreateWithCFData(bytes);
        CGImageRef image = CGImageCreate(decoded->width, decoded->height, 8, 32, decoded->width * 4,
            color, kCGImageAlphaLast | kCGBitmapByteOrderDefault, provider, nullptr, YES, kCGRenderingIntentDefault);
        if (image) {
            [images addObject:[[NSImage alloc] initWithCGImage:image size:NSZeroSize]];
            CGImageRelease(image);
        }
        CGDataProviderRelease(provider);
        CFRelease(bytes);
        // 像素已经由 CGImage 持有，播放规则只需要时长，不再留第二份 RGBA。
        std::vector<uint8_t>().swap(frame.rgba);
    }
    CGColorSpaceRelease(color);
    if (images.count != decoded->frames.size()) return nil;
    asset->_decoded = std::move(decoded);
    asset->_images = images;
    return asset;
}
- (void)drawInRect:(NSRect)rect elapsed:(uint64_t)elapsed {
    const size_t at = _decoded->frame_at(elapsed, NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion);
    [_images[at] drawInRect:rect fromRect:NSZeroRect operation:NSCompositingOperationSourceOver
                   fraction:1 respectFlipped:YES hints:nil];
}
- (BOOL)opaqueAt:(NSPoint)point size:(NSSize)size {
    if (point.x < 0 || point.y < 0 || point.x >= size.width || point.y >= size.height) return NO;
    const size_t x = static_cast<size_t>(point.x / size.width * _decoded->width);
    const size_t y = static_cast<size_t>(point.y / size.height * _decoded->height);
    return _decoded->alpha[y * _decoded->width + x] > 0;
}
@end

@interface MSIMEImageToolbarButton : NSButton
@property(nonatomic, strong) MSIMEToolbarImageAsset *backgroundAsset;
@property(nonatomic, strong) MSIMEToolbarImageAsset *faceAsset;
@property(nonatomic) uint64_t elapsed;
@end
@implementation MSIMEImageToolbarButton
- (BOOL)isFlipped { return YES; }
- (BOOL)mouseDownCanMoveWindow { return NO; }
- (void)drawRect:(NSRect)dirtyRect {
    [self.backgroundAsset drawInRect:self.bounds elapsed:self.elapsed];
    if (self.faceAsset) [self.faceAsset drawInRect:self.bounds elapsed:self.elapsed];
    else [super drawRect:dirtyRect];
}
@end

@implementation MSIMEToolbarImageCanvas {
    msime::mac::SkinToolbarImage _skin;
    NSDictionary *_manifest;
    NSMutableDictionary<NSString *, id> *_assets;
    NSSize _backgroundSize;
    NSSize _preferredSize;
    MSIMEToolbarImageAsset *_background;
    CFTimeInterval _anchor;
    uint64_t _elapsed;
}
- (BOOL)isFlipped { return YES; }
- (BOOL)mouseDownCanMoveWindow { return YES; }
- (NSSize)preferredSize { return _preferredSize; }
- (void)applySkin:(const msime::mac::SkinToolbarImage &)skin
           shown:(NSArray<NSArray<NSString *> *> *)shown
         buttons:(NSDictionary<NSString *, NSButton *> *)buttons
           scale:(CGFloat)scale color:(NSColor *)color {
    if (_skin.json != skin.json || _skin.directory != skin.directory || !_manifest) {
        _skin = skin;
        NSData *data = [@(skin.json.c_str()) dataUsingEncoding:NSUTF8StringEncoding];
        id manifest = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
        _manifest = [manifest isKindOfClass:NSDictionary.class] ? manifest : nil;
        _assets = [NSMutableDictionary dictionary];
        _anchor = CACurrentMediaTime();
        uint64_t remaining = 8000000;
        auto load = [&](id relative) {
            if (![relative isKindOfClass:NSString.class] || _assets[relative]) return;
            _assets[relative] = [MSIMEToolbarImageAsset load:skin.directory relative:relative remaining:remaining] ?: (id)NSNull.null;
        };
        load(_manifest[@"image"]);
        load(_manifest[@"buttonDefaults"][@"background"]);
        for (NSDictionary *button in [_manifest[@"buttons"] allValues]) {
            load(button[@"background"]);
            load(button[@"image"]);
            for (id relative in [button[@"images"] allValues]) load(relative);
        }
    }
    [self.subviews makeObjectsPerformSelector:@selector(removeFromSuperview)];
    _preferredSize = NSZeroSize;
    _backgroundSize = NSZeroSize;
    _background = nil;
    if (!_manifest) return;
    NSData *request = [NSJSONSerialization dataWithJSONObject:@{@"toolbar":_manifest, @"shown":shown} options:0 error:nil];
    auto owned = msime::host_api::own_string(msime_client_skin_toolbar_image_layout(
        static_cast<const uint8_t *>(request.bytes), request.length));
    if (!owned) return;
    NSData *reply = [NSData dataWithBytes:owned.get() length:std::char_traits<char>::length(owned.get())];
    NSDictionary *envelope = [NSJSONSerialization JSONObjectWithData:reply options:0 error:nil];
    if (![envelope[@"ok"] isEqual:@YES]) return;
    NSDictionary *layout = envelope[@"value"];
    _preferredSize = NSMakeSize(std::ceil([layout[@"widthDip"] doubleValue] * scale),
                                std::ceil([layout[@"heightDip"] doubleValue] * scale));
    _backgroundSize = NSMakeSize([_manifest[@"imageWidthDip"] doubleValue] * scale,
                                 [_manifest[@"imageHeightDip"] doubleValue] * scale);
    _background = [self asset:_manifest[@"image"]];
    for (NSDictionary *slot in layout[@"buttons"]) {
        NSButton *source = buttons[slot[@"key"]];
        if (!source) continue;
        MSIMEImageToolbarButton *button = [MSIMEImageToolbarButton buttonWithTitle:source.title target:source.target action:source.action];
        button.frame = NSMakeRect([slot[@"xDip"] doubleValue] * scale, [slot[@"yDip"] doubleValue] * scale,
                                  [slot[@"widthDip"] doubleValue] * scale, [slot[@"heightDip"] doubleValue] * scale);
        button.bordered = NO;
        button.image = source.image;
        button.imagePosition = source.imagePosition;
        button.font = [NSFont systemFontOfSize:[slot[@"iconSizeDip"] doubleValue] * scale weight:NSFontWeightRegular];
        button.symbolConfiguration = [NSImageSymbolConfiguration configurationWithPointSize:button.font.pointSize weight:NSFontWeightRegular];
        button.contentTintColor = color;
        button.attributedTitle = [[NSAttributedString alloc] initWithString:source.title attributes:@{NSFontAttributeName:button.font, NSForegroundColorAttributeName:color}];
        button.accessibilityIdentifier = source.accessibilityIdentifier;
        button.accessibilityLabel = source.accessibilityLabel;
        button.toolTip = source.toolTip;
        button.menu = source.menu;
        button.backgroundAsset = [self asset:slot[@"background"]];
        button.faceAsset = [self asset:slot[@"image"]];
        button.elapsed = _elapsed;
        [self addSubview:button];
    }
    self.needsDisplay = YES;
}
- (MSIMEToolbarImageAsset *)asset:(id)path {
    if (![path isKindOfClass:NSString.class]) return nil;
    id asset = _assets[path];
    return [asset isKindOfClass:MSIMEToolbarImageAsset.class] ? asset : nil;
}
- (BOOL)containsInteractivePoint:(NSPoint)point {
    for (NSView *button in self.subviews) if (NSPointInRect(point, button.frame)) return YES;
    if (!NSPointInRect(point, NSMakeRect(0, 0, _backgroundSize.width, _backgroundSize.height))) return NO;
    return !_background || [_background opaqueAt:point size:_backgroundSize];
}
- (NSView *)hitTest:(NSPoint)point {
    const NSPoint local = [self convertPoint:point fromView:self.superview];
    if (![self containsInteractivePoint:local]) return nil;
    return [super hitTest:point];
}
- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    [_background drawInRect:NSMakeRect(0, 0, _backgroundSize.width, _backgroundSize.height) elapsed:_elapsed];
}
- (void)advanceAnimation {
    _elapsed = static_cast<uint64_t>((CACurrentMediaTime() - _anchor) * 1000);
    self.needsDisplay = YES;
    for (MSIMEImageToolbarButton *button in self.subviews) {
        button.elapsed = _elapsed;
        button.needsDisplay = YES;
    }
}
@end
