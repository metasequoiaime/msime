#import "../../src/candidate/CandidateCanvas.h"
#import <ImageIO/ImageIO.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#include <cassert>
#include <filesystem>

// 只固定当前测试进程的动态效果策略，不依赖或改写机器的辅助功能设置。
static BOOL TestReduceMotion = NO;
static BOOL ReadTestReduceMotion(id workspace, SEL selector)
{
    (void)workspace;
    (void)selector;
    return TestReduceMotion;
}
static NSColor *Pixel(CALayer *layer, NSUInteger x, NSUInteger y)
{
    const size_t width = 40, height = 40;
    uint8_t pixels[width * height * 4] = {};
    CGColorSpaceRef space = CGColorSpaceCreateDeviceRGB();
    CGContextRef context = CGBitmapContextCreate(pixels, width, height, 8, width * 4, space, kCGImageAlphaPremultipliedLast);
    [layer renderInContext:context];
    // AppKit 的根图层翻转了栅格行；调用方仍使用未翻转的候选窗坐标。
    const NSUInteger row = height - 1 - y;
    const uint8_t *pixel = pixels + (row * width + x) * 4;
    NSColor *color = [NSColor colorWithSRGBRed:pixel[0] / 255.0 green:pixel[1] / 255.0 blue:pixel[2] / 255.0 alpha:pixel[3] / 255.0];
    CGContextRelease(context);
    CGColorSpaceRelease(space);
    return color;
}
static void Color(NSColor *color, NSInteger red, NSInteger green, NSInteger blue, NSInteger alpha = 255)
{
    if (std::abs(color.redComponent * 255 - red) >= 3)
        fprintf(stderr, "pixel %.0f %.0f %.0f %.0f expected %ld %ld %ld %ld\n", color.redComponent * 255, color.greenComponent * 255,
                color.blueComponent * 255, color.alphaComponent * 255, (long)red, (long)green, (long)blue, (long)alpha);
    assert(std::abs(color.redComponent * 255 - red) < 3);
    assert(std::abs(color.greenComponent * 255 - green) < 3);
    assert(std::abs(color.blueComponent * 255 - blue) < 3);
    assert(std::abs(color.alphaComponent * 255 - alpha) < 3);
}
static void WritePanel(const std::filesystem::path &path)
{
    uint8_t pixels[6 * 6 * 4] = {};
    for (NSInteger y = 0; y < 6; ++y)
        for (NSInteger x = 0; x < 6; ++x)
        {
            const BOOL edgeX = x < 2 || x >= 4, edgeY = y < 2 || y >= 4;
            const NSUInteger channel = edgeX && edgeY ? 0 : edgeX || edgeY ? 1 : 2;
            pixels[(y * 6 + x) * 4 + channel] = 255;
            if (edgeX && edgeY) {
                // 四角异色，避免上下或左右镜像仍能通过九宫格像素检查。
                pixels[(y * 6 + x) * 4] = y < 2 || x >= 4 ? 255 : 0;
                pixels[(y * 6 + x) * 4 + 1] = (x >= 4) != (y >= 4) ? 255 : 0;
                pixels[(y * 6 + x) * 4 + 2] = y >= 4 ? 255 : 0;
            }
            pixels[(y * 6 + x) * 4 + 3] = 255;
        }
    CGColorSpaceRef space = CGColorSpaceCreateDeviceRGB();
    CGContextRef context = CGBitmapContextCreate(pixels, 6, 6, 8, 24, space, kCGImageAlphaPremultipliedLast);
    CGImageRef image = CGBitmapContextCreateImage(context);
    CGImageDestinationRef destination =
        CGImageDestinationCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(path.c_str())], CFSTR("public.png"), 1, nullptr);
    CGImageDestinationAddImage(destination, image, nullptr);
    assert(CGImageDestinationFinalize(destination));
    CFRelease(destination);
    CGImageRelease(image);
    CGContextRelease(context);
    CGColorSpaceRelease(space);
}
static void TestCanvas(void)
{
    @autoreleasepool
    {
        [NSApplication sharedApplication];
        const auto root = std::filesystem::temp_directory_path() / ("msime-canvas-" + std::string(NSUUID.UUID.UUIDString.UTF8String));
        std::filesystem::create_directory(root);
        WritePanel(root / "panel.png");
        msime::mac::SkinCanvas canvas;
        msime::mac::SkinBorderImage border;
        border.source = (root / "panel.png").string();
        border.slice = {2, 2, 2, 2};
        border.width = {4, 4, 4, 4};
        canvas.border = border;
        MSIMECandidateCanvasAssets *assets = [MSIMECandidateCanvasAssets assetsForCanvas:canvas dark:NO];
        assert(assets.panelLoaded);
        MSIMECandidateCanvasView *view = [[MSIMECandidateCanvasView alloc] initWithFrame:NSMakeRect(0, 0, 40, 40)
                                                                                  assets:assets
                                                                                  canvas:canvas
                                                                                    card:NSMakeRect(6, 8, 24, 20)
                                                                                   level:0];
        Color(Pixel(view.layer, 7, 9), 0, 255, 255);
        Color(Pixel(view.layer, 9, 11), 0, 255, 255);
        Color(Pixel(view.layer, 7, 26), 255, 0, 0);
        Color(Pixel(view.layer, 28, 26), 255, 255, 0);
        Color(Pixel(view.layer, 28, 9), 255, 0, 255);
        Color(Pixel(view.layer, 14, 9), 0, 255, 0);
        Color(Pixel(view.layer, 14, 17), 0, 0, 255);
        Color(Pixel(view.layer, 1, 1), 0, 0, 0, 0);
        assert([view hitTest:NSMakePoint(14, 17)] == nil);
        canvas.border->fill = false;
        view = [[MSIMECandidateCanvasView alloc] initWithFrame:NSMakeRect(0, 0, 40, 40)
                                                        assets:assets
                                                        canvas:canvas
                                                          card:NSMakeRect(6, 8, 24, 20)
                                                         level:0];
        Color(Pixel(view.layer, 14, 17), 0, 0, 0, 0);
        Color(Pixel(view.layer, 7, 9), 0, 255, 255);
        // 不透明度只淡化面板；实际栅格的颜色和 alpha 同时变化。
        canvas.panelOpacity = 0.5;
        view = [[MSIMECandidateCanvasView alloc] initWithFrame:NSMakeRect(0, 0, 40, 40)
                                                        assets:assets
                                                        canvas:canvas
                                                          card:NSMakeRect(6, 8, 24, 20)
                                                         level:0];
        Color(Pixel(view.layer, 7, 9), 0, 128, 128, 128);
        // 自由挂件按声明框 contain，不拦截候选点击；正的 Y 偏移向屏幕下方。
        canvas.decorations.push_back({(root / "panel.png").string(), 6, 12, "top-right", 2, 3, 2});
        assets = [MSIMECandidateCanvasAssets assetsForCanvas:canvas dark:YES];
        view = [[MSIMECandidateCanvasView alloc] initWithFrame:NSMakeRect(0, 0, 40, 40)
                                                        assets:assets
                                                        canvas:canvas
                                                          card:NSMakeRect(6, 8, 24, 20)
                                                         level:1];
        assert(view.layer.sublayers.count == 1);
        assert(view.layer.sublayers.firstObject.opacity == 1.0f);
        assert(NSEqualRects(view.layer.sublayers.firstObject.frame, NSMakeRect(26, 16, 6, 6)));
        canvas.panelOpacity = 1.0;
        canvas.border->source = (root / "missing.png").string();
        assets = [MSIMECandidateCanvasAssets assetsForCanvas:canvas dark:YES];
        assert(!assets.panelLoaded);
        // APNG、GIF 与 WebP 读出真实帧时长，减弱动态效果或收起面板会撤掉帧动画。
        NSMutableDictionary *moving = [@{
            @"png" : @"iVBORw0KGgoAAAANSUhEUgAAAAwAAAAMCAYAAABWdVznAAAACGFjVEwAAAACAAAAAPONk3AAAAAaZmNUTAAAAAAAAAAMAAAADAAAAAAAAAAAALQD6AAA"
                     @"8a7+pwAAABxJREFUeJxj/M/"
                     @"A8J+BBMBEimIQGNVADBgOoQQArIUCFiFVnUUAAAAaZmNUTAAAAAEAAAAMAAAADAAAAAAAAAAAALQD6AAAat0UcwAAACBmZEFUAAAAAnicY2Rg+P+"
                     @"fgQTARIpiEBjVQAwYDqEEAKqHAhZF3Mt4AAAAAElFTkSuQmCC",
            @"gif" : @"R0lGODlhDAAMAIEAAP8AAAAAAAAAAAAAACH/"
                     @"C05FVFNDQVBFMi4wAwEAAAAh+QQAEgAAACwAAAAADAAMAAAIFgABCBxIsKDBgwgTKlzIsKHDhxARBgQAIfkEARIAAQAsAAAAAAwADACBAAD/"
                     @"AAAAAAAAAAAACBYAAQgcSLCgwYMIEypcyLChw4cQEQYEADs=",
            @"webp" : @"UklGRoQAAABXRUJQVlA4WAoAAAACAAAACwAACwAAQU5JTQYAAAAAAAAAAABBTk1GKAAAAAAAAAAAAAsAAAsAALQAAAJWUDhMDwAAAC8LwAIABxD9j/"
                      @"4HIqL/AQBBTk1GKAAAAAAAAAAAAAsAAAsAALQAAABWUDhMDwAAAC8LwAIABxDR//4HIqL/AQA="
        } mutableCopy];
        NSMutableData *gif = [[[NSData alloc] initWithBase64EncodedString:moving[@"gif"] options:0] mutableCopy];
        const uint8_t loopHeader[] = {0x21, 0xff, 0x0b, 'N', 'E', 'T', 'S', 'C', 'A', 'P', 'E', '2', '.', '0'};
        const NSRange loop = [gif rangeOfData:[NSData dataWithBytes:loopHeader length:sizeof(loopHeader)]
                                      options:0
                                        range:NSMakeRange(0, gif.length)];
        assert(loop.location != NSNotFound);
        NSMutableData *once = [gif mutableCopy];
        [once replaceBytesInRange:NSMakeRange(loop.location, 19) withBytes:nullptr length:0];
        moving[@"gif-once"] = [once base64EncodedStringWithOptions:0];
        const uint8_t repeats = 2;
        [gif replaceBytesInRange:NSMakeRange(loop.location + 16, 1) withBytes:&repeats];
        moving[@"gif-repeat"] = [gif base64EncodedStringWithOptions:0];
        for (NSUInteger at = 0; at + 7 < gif.length; ++at)
        {
            const uint8_t *bytes = static_cast<const uint8_t *>(gif.bytes);
            if (bytes[at] == 0x21 && bytes[at + 1] == 0xf9 && bytes[at + 2] == 4)
            {
                const uint8_t delay[] = {1, 0};
                [gif replaceBytesInRange:NSMakeRange(at + 4, 2) withBytes:delay];
            }
        }
        moving[@"gif-fast"] = [gif base64EncodedStringWithOptions:0];
        for (NSString *extension in moving)
        {
            NSString *path = [@(root.c_str()) stringByAppendingPathComponent:[@"moving." stringByAppendingString:extension]];
            assert([[[NSData alloc] initWithBase64EncodedString:moving[extension] options:0] writeToFile:path atomically:YES]);
            canvas.border->source = path.UTF8String;
            canvas.border->slice = {0, 0, 0, 0};
            canvas.border->width = {0, 0, 0, 0};
            canvas.border->fill = true;
            assets = [MSIMECandidateCanvasAssets assetsForCanvas:canvas dark:YES];
            assert(assets.panelLoaded);
            view = [[MSIMECandidateCanvasView alloc] initWithFrame:NSMakeRect(0, 0, 40, 40)
                                                            assets:assets
                                                            canvas:canvas
                                                              card:NSMakeRect(0, 0, 24, 20)
                                                             level:0];
            Color(Pixel(view.layer, 12, 10), 255, 0, 0);
            [view setPlaybackVisible:YES anchor:CACurrentMediaTime()];
            CALayer *sprite = view.layer.sublayers.firstObject;
            CAKeyframeAnimation *animation = (id)[sprite animationForKey:@"msime-canvas-frames"];
            if (!NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion)
            {
                assert(animation.values.count == 3 && animation.keyTimes.count == 3);
                assert(std::abs(animation.duration - ([extension isEqual:@"gif-fast"] ? 0.2 : 0.36)) < 0.01);
                const float loops = [extension isEqual:@"gif-once"]                                        ? 1
                                    : [extension isEqual:@"gif-repeat"] || [extension isEqual:@"gif-fast"] ? 3
                                                                                                           : HUGE_VALF;
                if (animation.repeatCount != loops)
                    fprintf(stderr, "%s repeat=%g expected=%g\n", extension.UTF8String, animation.repeatCount, loops);
                assert(animation.repeatCount == loops);
                assert(!animation.removedOnCompletion && [animation.fillMode isEqual:kCAFillModeForwards]);
                MSIMECandidateCanvasView *sample = [[MSIMECandidateCanvasView alloc] initWithFrame:NSMakeRect(0, 0, 40, 40)];
                sample.wantsLayer = YES;
                CALayer *frame = [CALayer layer];
                frame.frame = sprite.frame;
                frame.contents = animation.values[1];
                [sample.layer addSublayer:frame];
                Color(Pixel(sample.layer, 12, 10), 0, 0, 255);
            }
            TestReduceMotion = YES;
            [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification
                                                                          object:nil];
            assert([sprite animationForKey:@"msime-canvas-frames"] == nil);
            Color(Pixel(view.layer, 12, 10), 255, 0, 0);
            TestReduceMotion = NO;
            [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification
                                                                          object:nil];
            assert([sprite animationForKey:@"msime-canvas-frames"] != nil);
            [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceScreensDidSleepNotification object:nil];
            assert([sprite animationForKey:@"msime-canvas-frames"] == nil);
            [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceScreensDidWakeNotification object:nil];
            assert([sprite animationForKey:@"msime-canvas-frames"] != nil);
            [view setPlaybackVisible:NO anchor:0];
            assert([sprite animationForKey:@"msime-canvas-frames"] == nil);
            Color(Pixel(view.layer, 12, 10), 255, 0, 0);
        }
        std::filesystem::remove_all(root);
    }
}

int main(void)
{
    @autoreleasepool
    {
        Method method = class_getInstanceMethod(NSWorkspace.class, @selector(accessibilityDisplayShouldReduceMotion));
        IMP original = method_setImplementation(method, (IMP)ReadTestReduceMotion);
        TestCanvas();
        method_setImplementation(method, original);
    }
    return 0;
}
