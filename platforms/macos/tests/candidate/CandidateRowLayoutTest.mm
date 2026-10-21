#import "../settings/TestPreferenceSuite.h"
// The candidate page layout as the panel applies it. CandidateItemLayoutTest covers the arithmetic; this renders real pages through the controller, because what matters is the frame a candidate button ends up with and the runs it draws: the card is at most half the screen's visible width (a horizontal page may grow past it to keep its glosses on one line), text, 辅助码 and glosses wider than their column wrap inside it, rows take their own heights, and a horizontal page breaks onto a new line instead of squeezing its candidates.
//
// It stands apart from shortcut-test, which imports the same controller, so a failure elsewhere in that suite cannot hide the layout assertions.
#import "../../src/input/InputController.mm"
#import <ImageIO/ImageIO.h>

#include <cassert>
#include <filesystem>
#include <fstream>

@interface RowLayoutClient : NSObject <MSIMETextClient>
@property(nonatomic) NSRect caret;
@property(nonatomic, copy) NSDictionary *colors;
@property(nonatomic) NSUInteger attributeReads;
@end
@implementation RowLayoutClient
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index lineHeightRectangle:(NSRect *)rect
{
    (void)index;
    *rect = self.caret;
    ++self.attributeReads;
    return self.colors ?: @{};
}
- (void)insertText:(id)text replacementRange:(NSRange)range
{
    (void)text;
    (void)range;
}
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement
{
    (void)text;
    (void)selection;
    (void)replacement;
}
@end

// Keep the panel off the screen while it still lays its content out.
@interface RowLayoutPanel : MSIMECandidatePanel
@end
@implementation RowLayoutPanel
- (void)orderFrontRegardless
{
}
@end

// 专门模拟正在显示的窗口，检查同次组字的历史宽度不会覆盖收紧开关。
@interface VisibleRowLayoutPanel : RowLayoutPanel
@end
@implementation VisibleRowLayoutPanel
- (BOOL)isVisible { return YES; }
@end

static MSIMECandidateButton *CandidateButton(NSView *content, NSInteger tag)
{
    for (NSView *view in MSIMECandidateControlsView(content).subviews)
        if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag == tag) return (MSIMECandidateButton *)view;
    return nil;
}

static void Spin(NSTimeInterval seconds)
{
    [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:seconds]];
}

// 按完成状态等待主队列回调；单次 runUntilDate 返回不保证到期回调已经执行。
static bool SpinUntil(BOOL (^finished)(void))
{
    const CFTimeInterval deadline = CACurrentMediaTime() + 2.0;
    while (!finished() && CACurrentMediaTime() < deadline) Spin(0.01);
    return finished();
}

// 图层在窗口根图层坐标系里的纵坐标，带上它此刻的模型变换。
static CGFloat RootY(CALayer *layer)
{
    CALayer *root = layer;
    while (root.superlayer) root = root.superlayer;
    return [layer convertRect:layer.bounds toLayer:root].origin.y;
}

// 把 transform 临时设成图层的模型变换，返回图层在屏幕方向上被抬起多少点（负数是向下）。up 是根图层坐标系里屏幕向上的方向（+1 或 -1）。
static CGFloat ScreenLift(CALayer *layer, CATransform3D transform, CGFloat up)
{
    const CGFloat rest = RootY(layer);
    const CATransform3D saved = layer.transform;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    layer.transform = transform;
    const CGFloat moved = RootY(layer);
    layer.transform = saved;
    [CATransaction commit];
    return (moved - rest) * up;
}

static NSDictionary *MotionView(NSString *editing, NSUInteger count, NSUInteger highlighted)
{
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSUInteger slot = 0; slot < count; ++slot)
        [candidates addObject:@{@"text": [NSString stringWithFormat:@"候选%lu", (unsigned long)slot], @"highlighted": @(slot == highlighted)}];
    return @{@"focused": @YES, @"editing_text": editing, @"page": @0, @"page_count": @1, @"candidates": candidates};
}

// 产品路径（InputController）在皮肤包写了动效时的行为：每次排版都换一个新的内容视图，显示和装饰在同一轮事件里发生，候选行这时还没有自己的图层。
static void TestDeclaredMotion(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-candidate-motion-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    std::filesystem::create_directory(root / "declared-motion");
    {
        std::ofstream manifest(root / "declared-motion" / "skin.toml");
        manifest << R"toml(schema_version = 2
id = "declared-motion"
name = "Declared Motion"
version = "1"
base = "system"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 240
[animation]
enter = "fade_scale"
exit = "fade"
selection = "spring"
duration_ms = 400
stagger_ms = 20
)toml";
        assert(manifest.good());
    }
    NSString *suite = [@"app.msime.test.motion." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:@"declared-motion" forKey:@"MSIMEClientCustomCandidateSkin"];
    MSIMEAppearancePreferences *appearance =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    appearance.vertical = YES;
    const auto declared = [appearance candidateWindowSkinForDark:NO].animation;
    assert(declared.enter == msime::mac::SkinAnimationPreset::fade_scale && declared.selection == msime::mac::SkinAnimationPreset::spring &&
           declared.durationMs == 400.0 && declared.staggerMs == 20.0);
    // A canvas base carries the same v2 motion contract as any other base; canvas only adds the nine-slice and widget fields.
    std::filesystem::create_directory(root / "canvas-motion");
    {
        std::ofstream manifest(root / "canvas-motion" / "skin.toml");
        manifest << R"toml(schema_version = 2
id = "canvas-motion"
name = "Canvas Motion"
version = "1"
base = "canvas"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 240
[animation]
enter = "slide_up"
exit = "fade"
selection = "glow"
duration_ms = 240
stagger_ms = 10
)toml";
        assert(manifest.good());
    }
    {
        NSString *canvasSuite = [@"app.msime.test.motion.canvas." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *canvasDefaults = [[NSUserDefaults alloc] initWithSuiteName:canvasSuite];
        [canvasDefaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [canvasDefaults setObject:@"canvas-motion" forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *canvasAppearance =
            [[MSIMEAppearancePreferences alloc] initWithDefaults:canvasDefaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        const auto canvas = [canvasAppearance candidateWindowSkinForDark:NO].animation;
        assert(canvas.enter == msime::mac::SkinAnimationPreset::slide_up && canvas.selection == msime::mac::SkinAnimationPreset::glow &&
               canvas.durationMs == 240.0 && canvas.staggerMs == 10.0);
        MSIMERemoveTestPreferenceSuite(canvasDefaults, canvasSuite);
    }
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    MSIMECandidateMotion *motion = [MSIMECandidateMotion new];
    // 不依赖这台机器的「减弱动态效果」。
    motion.reduceMotion = NO;
    [controller setValue:motion forKey:@"candidateMotion"];

    // 进场：卡片和每一行都在动，错峰总延迟封顶 80ms，行从下方 3 点升起。
    [controller setValue:MotionView(@"ce", 9, 0) forKey:@"view"];
    [controller renderCandidates];
    NSPanel *panel = [controller valueForKey:@"panel"];
    NSView *content = panel.contentView;
    assert(panel.isVisible && panel.alphaValue == 1.0);
    assert([content.layer animationForKey:@"msime-candidate-enter-opacity"] != nil);
    CFTimeInterval firstBegin = 0, lastBegin = 0;
    for (NSInteger tag = 0; tag < 9; ++tag) {
        CALayer *rowLayer = CandidateButton(content, tag).layer;
        CAAnimationGroup *reveal = (id)[rowLayer animationForKey:@"msime-candidate-row-reveal"];
        assert(rowLayer.superlayer == CandidateButton(content, tag).superview.layer && reveal != nil);
        const CFTimeInterval begin = [rowLayer convertTime:reveal.beginTime toLayer:nil];
        if (tag == 0) firstBegin = begin;
        lastBegin = begin;
    }
    assert(lastBegin - firstBegin > 0.0 && lastBegin - firstBegin <= 0.08 + 1e-6);
    CALayer *row0 = CandidateButton(content, 0).layer;
    CALayer *row1 = CandidateButton(content, 1).layer;
    assert(NSMinY(CandidateButton(content, 0).frame) > NSMinY(CandidateButton(content, 1).frame));
    const CGFloat up = RootY(row0) > RootY(row1) ? 1.0 : -1.0;
    CAAnimationGroup *reveal = (id)[row1 animationForKey:@"msime-candidate-row-reveal"];
    CABasicAnimation *lift = (id)reveal.animations[1];
    assert(std::abs(ScreenLift(row1, [lift.fromValue CATransform3DValue], up) + 3.0) < 0.01);

    // 进场途中换高亮（整页重建）：高亮同步换好，新高亮行的底色交给选中装饰，新内容视图接着同一条进场时间线。
    [controller setValue:MotionView(@"ce", 9, 2) forKey:@"view"];
    [controller renderCandidates];
    content = panel.contentView;
    MSIMECandidateButton *selected = CandidateButton(content, 2);
    assert(selected.candidateHighlighted && !CandidateButton(content, 0).candidateHighlighted);
    assert(motion.selectionLayer != nil && motion.selectionLayer.superlayer == selected.superview.layer && selected.selectionFillDetached);
    assert([motion.selectionLayer animationForKey:@"msime-candidate-selection"] != nil);
    assert([selected.layer animationForKey:@"msime-candidate-selection"] == nil);
    assert([CandidateButton(content, 8).layer animationForKey:@"msime-candidate-row-reveal"] != nil);
    assert(SpinUntil(^BOOL {
        return motion.selectionLayer == nil && !CandidateButton(panel.contentView, 2).selectionFillDetached;
    }));

    // 打字让 Engine 把高亮放回第一格：那不是选中，不装饰。
    [controller setValue:MotionView(@"ces", 9, 0) forKey:@"view"];
    [controller renderCandidates];
    assert(motion.selectionLayer == nil);
    // 同一个输入串里再移一格才装饰。
    [controller setValue:MotionView(@"ces", 9, 1) forKey:@"view"];
    [controller renderCandidates];
    assert(motion.selectionLayer != nil && CandidateButton(panel.contentView, 1).selectionFillDetached);

    // 进场途中换成行数更多的一页：后来的行也播完错峰，不被只按第一页算的结束时间提前撤掉。第一页一行时进场 400ms 结束，九行的页要到 480ms。
    [controller hideCandidatePanel:"empty"];
    assert(!panel.isVisible);
    Spin(0.6);
    const CFTimeInterval shown = CACurrentMediaTime();
    [controller setValue:MotionView(@"ce", 1, 0) forKey:@"view"];
    [controller renderCandidates];
    assert([panel.contentView.layer animationForKey:@"msime-candidate-enter-opacity"] != nil);
    [controller setValue:MotionView(@"ceshi", 9, 0) forKey:@"view"];
    [controller renderCandidates];
    Spin(0.44 - (CACurrentMediaTime() - shown));
    if (CACurrentMediaTime() - shown < 0.475)
        assert([CandidateButton(panel.contentView, 8).layer animationForKey:@"msime-candidate-row-reveal"] != nil);
    Spin(0.2);
    assert([panel.contentView.layer animationForKey:@"msime-candidate-enter-opacity"] == nil);

    // 卡片的 slide_up 从下方升起，退场向下沉。
    [controller hideCandidatePanel:"focus_out"];
    Spin(0.6);
    msime::mac::SkinAnimation slideUp = declared;
    slideUp.enter = msime::mac::SkinAnimationPreset::slide_up;
    [motion showPanel:panel animation:slideUp rows:@[]];
    CABasicAnimation *rise = (id)[panel.contentView.layer animationForKey:@"msime-candidate-enter-transform"];
    assert(rise != nil && ScreenLift(panel.contentView.layer, [rise.fromValue CATransform3DValue], up) < -1.0);

    // 减弱动态效果打开时什么都不播。
    motion.reduceMotion = YES;
    [motion stop];
    [panel orderOut:nil];
    Spin(0.6);
    [controller setValue:MotionView(@"ce", 9, 0) forKey:@"view"];
    [controller renderCandidates];
    assert(panel.isVisible && [panel.contentView.layer animationForKey:@"msime-candidate-enter-opacity"] == nil);
    [controller setValue:MotionView(@"ce", 9, 1) forKey:@"view"];
    [controller renderCandidates];
    assert(motion.selectionLayer == nil);
    [controller hideCandidatePanel:"empty"];
    assert(!panel.isVisible && !motion.exiting);
    [motion stop];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    std::filesystem::remove_all(root);
}

// 写一张 width×height、frames 帧、每帧 delay 秒、无限循环的 APNG，各帧颜色不同。
static void WriteApng(const std::filesystem::path &path, size_t width, size_t height, size_t frames, double delay)
{
    CGImageDestinationRef destination =
        CGImageDestinationCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(path.c_str())], CFSTR("public.png"), frames, nullptr);
    assert(destination != nullptr);
    CGColorSpaceRef space = CGColorSpaceCreateDeviceRGB();
    for (size_t index = 0; index < frames; ++index)
    {
        CGContextRef context = CGBitmapContextCreate(nullptr, width, height, 8, 0, space, kCGImageAlphaPremultipliedLast);
        CGContextSetRGBFillColor(context, index % 2, 0.4, (index + 1) % 2, 1.0);
        CGContextFillRect(context, CGRectMake(0, 0, width, height));
        CGImageRef image = CGBitmapContextCreateImage(context);
        CGImageDestinationAddImage(destination, image, (__bridge CFDictionaryRef) @{
            (__bridge NSString *)kCGImagePropertyPNGDictionary : @{(__bridge NSString *)kCGImagePropertyAPNGDelayTime : @(delay)}
        });
        CGImageRelease(image);
        CGContextRelease(context);
    }
    CGColorSpaceRelease(space);
    assert(CGImageDestinationFinalize(destination));
    CFRelease(destination);
}

static MSIMECandidateDecorationView *DecorationView(NSView *content)
{
    for (NSView *view in content.subviews)
        if ([view isKindOfClass:MSIMECandidateDecorationView.class]) return (MSIMECandidateDecorationView *)view;
    return nil;
}

// 产品路径的装饰图待机动效和 APNG 帧动画：每次排版都换新的内容视图和装饰视图，动画跟着挂到新视图上，从同一时刻起算，所以打字时装饰图不会每个键都跳回原位；旧视图上的动画撤掉；收起和减弱动态效果时不动。
static void TestDecorationMotion(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-decoration-motion-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    std::filesystem::create_directory(root / "whale");
    WriteApng(root / "whale" / "whale.png", 60, 80, 6, 0.12);
    {
        std::ofstream manifest(root / "whale" / "skin.toml");
        manifest << R"toml(schema_version = 2
id = "whale"
name = "Whale"
version = "1"
base = "system"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 240
[candidate_window.decoration]
image = "whale.png"
top_inset_dip = 48
width_dip = 60
motion = "sway"
motion_period_ms = 2400
)toml";
        assert(manifest.good());
    }
    NSString *suite = [@"app.msime.test.decoration." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:@"whale" forKey:@"MSIMEClientCustomCandidateSkin"];
    MSIMEAppearancePreferences *appearance =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    appearance.vertical = YES;
    const auto skin = [appearance candidateWindowSkinForDark:NO];
    assert(skin.decorationMotion == msime::mac::DecorationMotion::sway && skin.decorationMotionPeriodMs == 2400.0);
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    MSIMECandidateMotion *motion = [MSIMECandidateMotion new];
    motion.reduceMotion = NO;
    [controller setValue:motion forKey:@"candidateMotion"];

    [controller setValue:MotionView(@"ce", 5, 0) forKey:@"view"];
    [controller renderCandidates];
    NSPanel *panel = [controller valueForKey:@"panel"];
    MSIMECandidateDecorationView *first = DecorationView(panel.contentView);
    assert(panel.isVisible && first != nil && motion.animatedDecoration == first && first.frames.images.count == 6);
    CAKeyframeAnimation *sway = (id)[first.layer animationForKey:@"msime-decoration-motion"];
    CAKeyframeAnimation *frames = (id)[first.layer animationForKey:@"msime-decoration-frames"];
    assert(sway != nil && std::abs(sway.duration - 2.4) < 1e-6 && sway.repeatCount == HUGE_VALF);
    assert(frames != nil && frames.values.count == 6 && std::abs(frames.duration - 0.72) < 0.01);
    // 摇摆以底边中点为轴：最大角度时底边中点不动，顶边横移不超过留出的余量。
    const CATransform3D tilted = [sway.values[1] CATransform3DValue];
    const CGRect bounds = first.layer.bounds;
    const CGPoint anchor = CGPointMake(CGRectGetMinX(bounds) + first.layer.anchorPoint.x * CGRectGetWidth(bounds),
                                       CGRectGetMinY(bounds) + first.layer.anchorPoint.y * CGRectGetHeight(bounds));
    const CGPoint bottom = CGPointApplyAffineTransform(CGPointMake(CGRectGetMidX(bounds) - anchor.x, CGRectGetMinY(bounds) - anchor.y),
                                                       CATransform3DGetAffineTransform(tilted));
    assert(std::abs(bottom.x - (CGRectGetMidX(bounds) - anchor.x)) < 0.01 && std::abs(bottom.y - (CGRectGetMinY(bounds) - anchor.y)) < 0.01);
    const CGPoint top = CGPointApplyAffineTransform(CGPointMake(CGRectGetMidX(bounds) - anchor.x, CGRectGetMaxY(bounds) - anchor.y),
                                                    CATransform3DGetAffineTransform(tilted));
    const CGFloat shift = std::abs(top.x - (CGRectGetMidX(bounds) - anchor.x));
    assert(shift > 1.0 && shift <= msime::mac::kDecorationMotionMargin + 0.01);
    // 装饰图四周留着余量，最大摇摆时也不越过窗口左右边和顶边。
    assert(NSMinX(first.frame) >= msime::mac::kDecorationMotionMargin - 0.01 &&
           NSMaxX(first.frame) <= NSWidth(panel.contentView.bounds) - msime::mac::kDecorationMotionMargin + 0.01 &&
           NSMaxY(first.frame) <= NSHeight(panel.contentView.bounds) - msime::mac::kDecorationMotionMargin + 0.01);
    const CFTimeInterval begin = [first.layer convertTime:sway.beginTime toLayer:nil];
    assert(std::abs(begin - motion.decorationAnchor) < 1e-3);

    // 打字：整页重排换了新的装饰视图，动画挂到新视图上、从同一时刻起算，旧视图上的撤掉。
    Spin(0.2);
    [controller setValue:MotionView(@"ces", 5, 0) forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateDecorationView *second = DecorationView(panel.contentView);
    assert(second != nil && second != first && motion.animatedDecoration == second);
    assert([first.layer animationForKey:@"msime-decoration-motion"] == nil);
    CAKeyframeAnimation *continued = (id)[second.layer animationForKey:@"msime-decoration-motion"];
    CAKeyframeAnimation *continuedFrames = (id)[second.layer animationForKey:@"msime-decoration-frames"];
    assert(std::abs([second.layer convertTime:continued.beginTime toLayer:nil] - begin) < 1e-3);
    assert(std::abs([second.layer convertTime:continuedFrames.beginTime toLayer:nil] - begin) < 1e-3);

    // 收起后不动。
    [controller hideCandidatePanel:"empty"];
    assert(!panel.isVisible && motion.animatedDecoration == nil && [second.layer animationForKey:@"msime-decoration-motion"] == nil);

    // 减弱动态效果时不动，APNG 停在第一帧。
    motion.reduceMotion = YES;
    Spin(0.5);
    [controller setValue:MotionView(@"ce", 5, 0) forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateDecorationView *still = DecorationView(panel.contentView);
    assert(panel.isVisible && still != nil && motion.animatedDecoration == nil && still.layer.animationKeys.count == 0 &&
           still.layer.contents == still.frames.images.firstObject);
    [controller hideCandidatePanel:"focus_out"];
    [motion stop];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    std::filesystem::remove_all(root);
}

// 读音单独占释义下面的一行。横排时格宽取候选、释义、读音三行里最宽的一行，长音标不再跟在释义后面把这一格撑到下一格的释义边上；同一行的各格一样高，释义都在同一高度；这一页没有读音、或者「显示读音」关着时高度和从前一样。
static void TestPronunciationLine(MSIMEInputController *controller, NSWindow *panel, MSIMEAppearancePreferences *appearance)
{
    appearance.vertical = NO;
    appearance.candidateTranslations = YES;
    NSString *ipa = @"/.ænɪˈmeɪʃən prəʊˈtægənɪst/";
    NSDictionary *view = @{@"focused": @YES, @"editing_text": @"donghuazhuti", @"page": @0, @"page_count": @1,
                           @"candidates": @[@{@"text": @"动画主体", @"translation": @"Animation protagonist", @"highlighted": @YES},
                                            @{@"text": @"东华主题", @"translation": @"Donghua theme"}]};
    [controller setValue:nil forKey:@"pronunciationTargets"];
    [controller setValue:[@{@"Animation protagonist": ipa} mutableCopy] forKey:@"englishPronunciations"];
    [controller setValue:view forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateButton *plain = CandidateButton(panel.contentView, 0);
    assert(plain && plain.pronunciation.length == 0);
    const CGFloat plainHeight = NSHeight(plain.frame);
    const CGFloat glossLine = plain.itemLayout.translation.height;
    const CGFloat plainCard = NSHeight(panel.frame);

    // 打开读音：读音在释义下面单独一行，释义行的位置不变。
    [controller setValue:@[@"en"] forKey:@"pronunciationTargets"];
    [controller renderCandidates];
    MSIMECandidateButton *read = CandidateButton(panel.contentView, 0);
    MSIMECandidateButton *neighbour = CandidateButton(panel.contentView, 1);
    assert(read && neighbour && [read.pronunciation isEqual:ipa] && neighbour.pronunciation.length == 0);
    NSFont *glossFont = read.translationFont;
    const CGFloat glossWidth = MSIMECandidateSingleLineWidth(@"Animation protagonist", glossFont);
    const CGFloat ipaWidth = MSIMECandidateSingleLineWidth(ipa, glossFont);
    const msime::candidate::CandidateRunBox &run = read.itemLayout.translation;
    assert(run.below && fabs(run.y - plain.itemLayout.translation.y) < 0.5);
    assert(run.height > glossLine * 1.5 && run.height < glossLine * 2.5);
    // 每一行都放得下，不折行：格子按较宽的那一行量，不按「释义  读音」连在一起的宽度。
    assert(run.width >= glossWidth && run.width >= ipaWidth);
    assert(fabs(run.width - MAX(glossWidth, ipaWidth)) < 1.0);
    const CGFloat joined = MSIMECandidateSingleLineWidth([NSString stringWithFormat:@"Animation protagonist  %@", ipa], glossFont);
    assert(NSWidth(read.frame) < read.contentLeft + joined);
    // 画出来的释义和读音都在自己的格子里，两格不重叠。
    assert(read.contentLeft + run.x + run.width <= NSWidth(read.frame) + 0.5);
    assert(NSMaxX(read.frame) <= NSMinX(neighbour.frame) + 0.5);
    assert(neighbour.contentLeft + neighbour.itemLayout.translation.x + neighbour.itemLayout.translation.width <= NSWidth(neighbour.frame) + 0.5);
    // 有读音的这一页多出一行高，同一行的格子一样高、释义在同一高度，选中底色盖住整格连同读音那一行。
    assert(NSHeight(read.frame) > plainHeight + glossLine * 0.5 && NSHeight(panel.frame) > plainCard + glossLine * 0.5);
    assert(fabs(NSHeight(read.frame) - NSHeight(neighbour.frame)) < 0.5 && fabs(NSMinY(read.frame) - NSMinY(neighbour.frame)) < 0.5);
    assert(fabs(neighbour.itemLayout.translation.y - run.y) < 0.5);
    assert(NSHeight(read.frame) >= run.y + run.height - 0.5);
    NSBitmapImageRep *bitmap = [read bitmapImageRepForCachingDisplayInRect:read.bounds];
    [read cacheDisplayInRect:read.bounds toBitmapImageRep:bitmap];

    // 这一页没有读音时高度不变。
    NSMutableDictionary *unread = [view mutableCopy];
    unread[@"candidates"] = @[@{@"text": @"东华主题", @"translation": @"Donghua theme", @"highlighted": @YES}, @{@"text": @"冬化", @"translation": @"winterize"}];
    [controller setValue:[unread copy] forKey:@"view"];
    [controller renderCandidates];
    assert(fabs(NSHeight(CandidateButton(panel.contentView, 0).frame) - plainHeight) < 0.5);
    // 「显示读音」关掉：没有第三行，回到原来的高度。
    [controller setValue:view forKey:@"view"];
    [controller setValue:nil forKey:@"pronunciationTargets"];
    [controller renderCandidates];
    assert(CandidateButton(panel.contentView, 0).pronunciation.length == 0);
    assert(fabs(NSHeight(CandidateButton(panel.contentView, 0).frame) - plainHeight) < 0.5 && fabs(NSHeight(panel.frame) - plainCard) < 0.5);

    // 只有释义变了、格子没变时按钮原地重画，读音跟着新释义换，不留上一条释义的读音。
    [controller setValue:@[@"en"] forKey:@"pronunciationTargets"];
    [controller setValue:[@{@"ab": @"/ab/", @"ba": @"/ba/"} mutableCopy] forKey:@"englishPronunciations"];
    NSDictionary *(^swapView)(NSString *) = ^NSDictionary *(NSString *gloss) {
        return @{@"session": @1, @"generation": @3, @"focused": @YES, @"editing_text": @"d", @"page": @0, @"page_count": @1,
                 @"candidates": @[@{@"id": @{@"session": @1, @"generation": @3, @"index": @0}, @"text": @"的", @"translation": gloss, @"highlighted": @YES}]};
    };
    [controller setValue:swapView(@"ab") forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateButton *before = CandidateButton(panel.contentView, 0);
    assert([before.pronunciation isEqual:@"/ab/"]);
    [controller setValue:swapView(@"ba") forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateButton *after = CandidateButton(panel.contentView, 0);
    assert(after == before && [after.translation isEqual:@"ba"] && [after.pronunciation isEqual:@"/ba/"]);

    // 竖排：释义仍在候选同一行，读音在释义下面，行高容得下两行。
    appearance.vertical = YES;
    [controller setValue:view forKey:@"view"];
    [controller setValue:[@{@"Animation protagonist": ipa} mutableCopy] forKey:@"englishPronunciations"];
    [controller renderCandidates];
    MSIMECandidateButton *vertical = CandidateButton(panel.contentView, 0);
    assert([vertical.pronunciation isEqual:ipa] && !vertical.itemLayout.translation.below);
    assert(vertical.itemLayout.translation.height > glossLine * 1.5 && NSHeight(vertical.frame) >= vertical.itemLayout.height - 0.5);
    assert(vertical.itemLayout.translation.width >= ipaWidth);
    [controller setValue:nil forKey:@"pronunciationTargets"];
    appearance.vertical = NO;
}

// 前景按它的不透明度叠到不透明的底色上后，和底色的 WCAG 对比度。
static double CandidateContrast(NSColor *foreground, NSColor *fill)
{
    NSColor *fore = [foreground colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    NSColor *back = [fill colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    auto linear = [](double c) { return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4); };
    auto luminance = [&](double r, double g, double b) { return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b); };
    const double a = fore.alphaComponent;
    const double over = luminance(fore.redComponent * a + back.redComponent * (1 - a), fore.greenComponent * a + back.greenComponent * (1 - a),
                                  fore.blueComponent * a + back.blueComponent * (1 - a));
    const double under = luminance(back.redComponent, back.greenComponent, back.blueComponent);
    return (MAX(over, under) + 0.05) / (MIN(over, under) + 0.05);
}

// 皮肤的选中底色不透明、释义色和它同一色系时，选中行的释义和读音换成按底色挑出的颜色，和底色至少 3:1；底色半透明时仍是皮肤的释义色。
static void TestSelectedGlossColour(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-selected-gloss-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    struct Palette { const char *id, *selected, *translation; bool opaque; };
    for (const Palette &palette : {Palette{"blue-on-blue", "#4D6BFE", "#2F5BD3", true}, Palette{"slate", "#23262F", "#6B7180", true},
                                   Palette{"translucent", "#2F5BD329", "#2F5BD3", false}}) {
        std::filesystem::create_directories(root / palette.id);
        {
            std::ofstream manifest(root / palette.id / "skin.toml");
            manifest << "schema_version = 2\nid = \"" << palette.id << "\"\nname = \"Selected Gloss\"\nversion = \"1\"\nbase = \"system\"\n"
                     << "[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n[candidate_window]\nmin_width_dip = 0\n";
            for (const char *mode : {"light", "dark"})
                manifest << "[candidate." << mode << "]\nselected = \"" << palette.selected << "\"\ntranslation = \"" << palette.translation << "\"\n";
            assert(manifest.good());
        }
        NSString *suite = [@"app.msime.test.selected-gloss." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(palette.id) forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *appearance =
            [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.vertical = NO;
        appearance.candidateTranslations = YES;
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                                                                    backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];
        [controller setValue:@[@"en"] forKey:@"pronunciationTargets"];
        [controller setValue:[@{@"to": @"/tuː/"} mutableCopy] forKey:@"englishPronunciations"];
        [controller setValue:@{@"focused": @YES, @"editing_text": @"dao", @"page": @0, @"page_count": @1,
                               @"candidates": @[@{@"text": @"到", @"translation": @"to", @"highlighted": @YES}, @{@"text": @"道", @"translation": @"to"}]}
                      forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *selected = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *other = CandidateButton(panel.contentView, 1);
        assert(selected.candidateHighlighted && !other.candidateHighlighted && [selected.pronunciation isEqual:@"/tuː/"]);
        const auto tokens = [appearance candidateWindowSkinForDark:NO].tokens;
        assert(tokens.translation && tokens.selectedTranslation);
        assert([other.translationColor isEqual:SkinColor(*tokens.translation)]);
        if (palette.opaque) {
            assert(selected.fillColor.alphaComponent >= 0.85 && CandidateContrast(selected.translationColor, selected.fillColor) >= 3.0);
            assert(![selected.translationColor isEqual:SkinColor(*tokens.translation)]);
        } else {
            assert([selected.translationColor isEqual:SkinColor(*tokens.translation)]);
        }
        NSBitmapImageRep *bitmap = [selected bitmapImageRepForCachingDisplayInRect:selected.bounds];
        [selected cacheDisplayInRect:selected.bounds toBitmapImageRep:bitmap];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

// 皮肤的收紧和横排均分必须到达真实控制器，不能只在清单读模型里保留字段。
static void TestFactoryFitAndJustify(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-factory-row-layout-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    CGFloat regularWidth = 0;
    for (const char *identifier : {"regular", "compact", "justified"}) {
        const bool compact = std::string(identifier) == "compact";
        const bool justified = std::string(identifier) == "justified";
        std::filesystem::create_directory(root / identifier);
        {
            std::ofstream manifest(root / identifier / "skin.toml");
            manifest << "schema_version = 1\nid = \"" << identifier << "\"\nname = \"Synthetic Layout\"\nversion = \"1\"\nbase = \"microsoft\"\n"
                     << "[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n"
                     << "[candidate_window]\nmin_width_dip = " << (justified ? 260 : 0)
                     << "\nexpand_rows = " << (compact ? 2 : justified ? 8 : 4)
                     << "\nfit_content = " << (compact ? "true" : "false")
                     << "\n[candidate_window.horizontal]\nitem_gap = 11\njustify_rows = " << (justified ? "true" : "false") << "\n";
            assert(manifest.good());
        }
        NSString *suite = [@"app.msime.test.factory-row-layout." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(identifier) forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
            skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.vertical = NO;
        appearance.showsAppLogo = NO;
        appearance.showsCandidatePreedit = NO;
        appearance.showsCandidatePageNumber = NO;
        appearance.showsCandidateExpandButton = NO;
        appearance.candidateTranslations = NO;
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        RowLayoutPanel *panel = [[VisibleRowLayoutPanel alloc] initWithContentRect:NSZeroRect
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];
        [controller setValue:@{@"focused": @YES, @"editing_text": @"ce", @"page": @0, @"page_count": @1,
            @"candidates": justified ? @[@{@"text": @"甲", @"highlighted": @YES}, @{@"text": @"乙"}]
                                      : @[@{@"text": @"甲", @"highlighted": @YES}]} forKey:@"view"];
        [controller renderCandidates];
        const CGFloat initialWidth = NSWidth(panel.contentView.frame);
        if (!compact) assert(initialWidth >= 159);
        else assert(initialWidth < 140);
        [controller setValue:@400 forKey:@"widestHorizontalCandidateWidth"];
        [controller renderCandidates];
        MSIMECandidateButton *first = CandidateButton(panel.contentView, 0);
        assert(first && first.hasSkinLayout);
        const CGFloat width = NSWidth(panel.contentView.frame);
        if (compact) {
            assert(width < regularWidth - 20 && fabs(width - initialWidth) < 1);
            assert(fabs(first.textLeftInset - first.textRightInset) < 0.1);
        } else if (justified) {
            MSIMECandidateButton *last = CandidateButton(panel.contentView, 1);
            assert(last && fabs(NSMaxX(last.frame) - (width - NSMinX(first.frame))) < 1);
            assert(NSWidth(first.frame) > 100 && NSWidth(last.frame) > 100);
        } else {
            regularWidth = width;
            assert(width >= 399);
        }
        if (!justified) {
            [controller setValue:@{@"focused": @YES, @"editing_text": @"ce", @"page": @0, @"page_count": @1,
                @"candidates": @[@{@"text": @"甲", @"highlighted": @YES}, @{@"text": @"乙"}]} forKey:@"view"];
            [controller renderCandidates];
            first = CandidateButton(panel.contentView, 0);
            MSIMECandidateButton *second = CandidateButton(panel.contentView, 1);
            assert(first && second && fabs(NSMinX(second.frame) - NSMaxX(first.frame) - (compact ? 11 : 17)) < 0.1);
        }
        // 可见行数来自同一份共享皮肤；末行仍能滚入视口，点击按钮只为可见行创建。
        appearance.pageSize = 3;
        NSMutableArray *expanded = [NSMutableArray array];
        for (NSUInteger index = 0; index < 30; ++index)
            [expanded addObject:@{@"text": @"合成", @"highlighted": @(index == 0)}];
        [controller setValue:expanded forKey:@"expandedCandidates"];
        [controller setValue:@0 forKey:@"expandedCursor"];
        [controller setValue:@0 forKey:@"expandedFirstRow"];
        const NSUInteger expectedRows = 1 + (compact ? 2 : justified ? 8 : 4);
        assert([controller renderExpandedCandidates]);
        assert([[controller valueForKey:@"expandedVisibleRows"] unsignedIntegerValue] == expectedRows);
        assert(CandidateButton(panel.contentView, expectedRows * 3 - 1));
        assert(!CandidateButton(panel.contentView, expectedRows * 3));
        [controller setValue:@29 forKey:@"expandedCursor"];
        assert([controller renderExpandedCandidates]);
        assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 10 - expectedRows);
        assert(CandidateButton(panel.contentView, 29) && !panel.hasNextPage && panel.hasPreviousPage);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

// 从真实候选视图检查四角及动效轮廓，不用读取清单字段代替绘制验收。
static void TestFactoryCornerRadii(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-candidate-corners-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    for (const char *identifier : {"follow", "plain", "square", "canvas-corners"}) {
        const bool canvas = std::string(identifier) == "canvas-corners";
        const bool square = std::string(identifier) == "square";
        const bool follow = std::string(identifier) != "plain";
        std::filesystem::create_directory(root / identifier);
        {
            std::ofstream manifest(root / identifier / "skin.toml");
            manifest << "schema_version = 2\nid = \"" << identifier << "\"\nname = \"Synthetic Corners\"\nversion = \"1\"\nbase = \""
                     << (canvas ? "canvas" : "microsoft") << "\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n"
                     << "[candidate_window]\ncorner_radius_dip = " << (square ? 0 : 16)
                     << "\nitem_corner_radius_dip = 2\nborder_width_dip = 0\npadding = 0\nitem_gap = 2\npreedit_margin_dip = [0]\n"
                     << "item_corners_follow_card = " << (follow ? "true" : "false") << "\n";
            if (canvas) manifest << "content_insets_dip = { top = 5, right = 9, bottom = 7, left = 11 }\n";
            assert(manifest.good());
        }
        NSString *suite = [@"app.msime.test.candidate-corners." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(identifier) forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
            skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.showsAppLogo = NO;
        appearance.showsCandidatePageNumber = NO;
        appearance.showsCandidateExpandButton = NO;
        appearance.candidateTranslations = NO;
        appearance.fontSize = 16;
        appearance.themeMode = @"light";
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        MSIMECandidatePanel *panel = [[MSIMECandidatePanel alloc] initWithContentRect:NSZeroRect
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];
        NSArray *candidates = @[@{@"text": @"合成", @"highlighted": @YES},
            @{@"text": [@"长候选" stringByPaddingToLength:1024 withString:@"长候选" startingAtIndex:0]}, @{@"text": @"末"}];
        [controller setValue:@{@"focused": @YES, @"editing_text": @"synthetic", @"page": @0, @"page_count": @1,
            @"candidates": candidates} forKey:@"view"];
        for (NSNumber *scaling in @[@100, @150]) for (NSNumber *vertical in @[@NO, @YES]) for (NSNumber *preedit in @[@NO, @YES])
            for (NSNumber *expanded in vertical.boolValue ? @[@NO] : @[@NO, @YES]) {
            appearance.candidateScalePercent = scaling.integerValue;
            appearance.vertical = vertical.boolValue;
            appearance.showsCandidatePreedit = preedit.boolValue;
            [controller setValue:expanded.boolValue ? candidates : nil forKey:@"expandedCandidates"];
            if (expanded.boolValue) assert([controller renderExpandedCandidates]);
            else [controller renderCandidates];
            auto skin = [appearance candidateWindowSkinForDark:NO];
            const double scale = scaling.doubleValue / 100.0;
            const double outer = MAX(0.01, skin.tokens.radius);
            MSIMECandidateChromeView *chrome = (id)panel.contentView;
            MSIMECandidateButton *first = CandidateButton(chrome, 0), *last = CandidateButton(chrome, 2);
            assert(first && last && first.hasCornerRadii == follow && last.hasCornerRadii == follow);
            CGPathRef path = [first createSelectionFillPath];
            if (!follow) { assert(path == nullptr); continue; }
            assert(path && CGRectEqualToRect(CGPathGetBoundingBox(path), NSInsetRect(first.bounds, 1, 1)));
            assert(fabs(first.cornerRadii[0] - (preedit.boolValue ? first.cornerRadius : outer)) < 0.01);
            if (fabs(last.cornerRadii[3] - outer) >= 0.01)
                fprintf(stderr, "corner geometry skin=%s scale=%.1f vertical=%d preedit=%d expanded=%d area=%s first=%s last=%s radius=%.2f expected=%.2f\n",
                    identifier, scale, vertical.boolValue, preedit.boolValue, expanded.boolValue, NSStringFromRect(chrome.candidateRowsRect).UTF8String,
                    NSStringFromRect(first.frame).UTF8String, NSStringFromRect(last.frame).UTF8String, last.cornerRadii[3], outer);
            assert(fabs(last.cornerRadii[3] - outer) < 0.01);
            if (vertical.boolValue) {
                assert(fabs(first.cornerRadii[1] - (preedit.boolValue ? first.cornerRadius : outer)) < 0.01);
                assert(fabs(last.cornerRadii[2] - outer) < 0.01);
            } else {
                // 末行只有一项，右缘没有贴满卡片，不能凭“末候选”借右下角。
                assert(NSMaxX(last.frame) + scale < NSMaxX(chrome.candidateRowsRect));
                assert(fabs(last.cornerRadii[2] - last.cornerRadius) < 0.01);
            }
            // 动效必须拿到同一路径，glow 也沿路径描边，不能退回统一 CALayer 圆角。
            MSIMECandidateMotion *motion = [MSIMECandidateMotion new];
            motion.reduceMotion = NO;
            msime::mac::SkinAnimation animation;
            animation.selection = msime::mac::SkinAnimationPreset::glow;
            animation.durationMs = 100;
            [motion decorateSelectionOfRow:first fill:NSColor.redColor cornerRadius:first.cornerRadius accent:NSColor.blueColor animation:animation];
            assert([motion.selectionLayer isKindOfClass:CAShapeLayer.class]);
            CAShapeLayer *shape = (id)motion.selectionLayer;
            assert(CGPathEqualToPath(shape.path, path));
            assert(CGRectEqualToRect(shape.frame, first.layer.frame));
            CABasicAnimation *glow = (id)[shape animationForKey:@"msime-candidate-selection"];
            assert([glow.keyPath isEqual:@"strokeColor"] && shape.lineWidth == 2);
            assert(first.selectionFillDetached);
            [motion endSelection];
            assert(!first.selectionFillDetached);
            CGPathRelease(path);
        }
        [controller setValue:nil forKey:@"expandedCandidates"];
        [panel orderOut:nil];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

// 对真实视图矩形检查预编辑外边距，避免只改窗口高度而文字仍挤在原位。
static void TestFactoryPreeditMargins(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-preedit-margin-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    struct Gaps { CGFloat top = 0, bottom = 0; } baseline[2][2][2];
    for (const char *identifier : {"plain", "margin", "canvas-margin", "canvas-zero"}) {
        const bool plain = std::string(identifier) == "plain";
        const bool zeroGap = std::string(identifier) == "canvas-zero";
        const bool canvas = zeroGap || std::string(identifier) == "canvas-margin";
        std::filesystem::create_directory(root / identifier);
        {
            std::ofstream manifest(root / identifier / "skin.toml");
            manifest << "schema_version = 2\nid = \"" << identifier << "\"\nname = \"Synthetic Margin\"\nversion = \"1\"\nbase = \""
                     << (canvas ? "canvas" : "microsoft") << "\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n"
                     << "[candidate_window]\npreedit_margin_dip = " << (plain ? "[0]" : "[7, 11, 13, 17]") << "\n";
            if (canvas) manifest << "line_gap_dip = " << (zeroGap ? 0 : 6) << "\n";
            manifest << "[candidate_window.vertical]\npreedit_margin_dip = " << (plain ? "[0]" : "[3, 5, 7, 9]") << "\n";
            assert(manifest.good());
        }
        NSString *suite = [@"app.msime.test.preedit-margin." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(identifier) forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
            skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.showsAppLogo = NO;
        appearance.showsCandidatePageNumber = NO;
        appearance.showsCandidateExpandButton = NO;
        appearance.showsCandidatePreedit = YES;
        appearance.candidateTranslations = NO;
        appearance.fontSize = 16;
        appearance.preeditFontSize = 12;
        appearance.themeMode = @"light";
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];
        NSString *preedit = @"synthetic012345678901234567890";
        NSArray *candidates = @[@{@"text": @"甲", @"highlighted": @YES}, @{@"text": @"乙"}];
        [controller setValue:@{@"focused": @YES, @"editing_text": preedit, @"page": @0, @"page_count": @1,
            @"candidates": candidates} forKey:@"view"];
        for (NSUInteger scaling = 0; scaling < 2; ++scaling) for (NSUInteger vertical = 0; vertical < 2; ++vertical)
            for (NSUInteger expanded = 0; expanded < (vertical ? 1 : 2); ++expanded) {
                appearance.candidateScalePercent = scaling ? 150 : 100;
                appearance.vertical = vertical;
                const CGFloat scale = scaling ? 1.5 : 1.0;
                [controller setValue:expanded ? candidates : nil forKey:@"expandedCandidates"];
                if (expanded) assert([controller renderExpandedCandidates]);
                else [controller renderCandidates];
                MSIMECandidateButton *first = CandidateButton(panel.contentView, 0);
                NSTextField *label = nil;
                for (NSView *child in MSIMECandidateControlsView(panel.contentView).subviews)
                    if ([child.identifier isEqual:@"candidate-preedit"]) label = (id)child;
                assert(first && first.hasSkinLayout && label);
                const CGFloat inset = NSMinX(first.frame);
                const CGFloat top = NSHeight(panel.contentView.bounds) - inset - NSMaxY(label.frame);
                const CGFloat bottom = NSMinY(label.frame) - NSMaxY(first.frame);
                if (plain) baseline[scaling][vertical][expanded] = {top, bottom};
                const auto before = baseline[scaling][vertical][expanded];
                const CGFloat marginTop = plain ? 0 : vertical ? 3 : 7;
                const CGFloat marginRight = plain ? 0 : vertical ? 5 : 11;
                const CGFloat marginBottom = canvas ? (zeroGap ? 0 : 6) : plain ? 0 : vertical ? 7 : 13;
                const CGFloat marginLeft = plain ? 0 : vertical ? 9 : 17;
                assert(fabs(NSMinX(label.frame) - inset - marginLeft * scale) < 0.1);
                assert(fabs(NSWidth(panel.contentView.bounds) - inset - NSMaxX(label.frame) - marginRight * scale) < 0.1);
                // 画布调整窗口后按背板像素对齐，顶部允许一像素的取整差；子视图之间仍精确比较。
                const CGFloat edgeTolerance = 1.0 / panel.backingScaleFactor + 0.01;
                if (fabs(top - before.top - marginTop * scale) > edgeTolerance)
                    fprintf(stderr, "preedit margin: skin=%s scale=%.1f vertical=%lu expanded=%lu top=%.2f baseline=%.2f expected=%.2f\n",
                        identifier, scale, (unsigned long)vertical, (unsigned long)expanded, top, before.top, marginTop * scale);
                assert(fabs(top - before.top - marginTop * scale) <= edgeTolerance);
                assert(fabs(bottom - before.bottom - marginBottom * scale) < 0.1);
                assert(NSWidth(label.frame) + 0.1 >= [preedit sizeWithAttributes:@{NSFontAttributeName:label.font}].width);
            }
        if (!plain && !canvas) {
            // 首行翻页和展开收起控件在外边距以内居中，不跟着总高度再次偏移。
            appearance.candidateScalePercent = 100;
            appearance.vertical = NO;
            appearance.showsCandidatePageNumber = YES;
            [controller setValue:@{@"focused": @YES, @"editing_text": preedit, @"page": @0, @"page_count": @3,
                @"candidates": candidates} forKey:@"view"];
            for (BOOL expanded : {NO, YES}) {
                [controller setValue:expanded ? candidates : nil forKey:@"expandedCandidates"];
                if (expanded) assert([controller renderExpandedCandidates]);
                else [controller renderCandidates];
                NSView *label = nil;
                for (NSView *child in MSIMECandidateControlsView(panel.contentView).subviews)
                    if ([child.identifier isEqual:@"candidate-preedit"]) label = child;
                MSIMECandidateButton *control = CandidateButton(panel.contentView, expanded ? -3 : -1);
                assert(label && control && fabs(NSMidY(label.frame) - NSMidY(control.frame)) < 0.6);
            }
            appearance.showsCandidatePageNumber = NO;
        }
        // 隐藏预编辑时不残留其外边距，原生布局原本就不产生隐藏行间隙。
        appearance.showsCandidatePreedit = NO;
        [controller setValue:nil forKey:@"expandedCandidates"];
        [controller renderCandidates];
        MSIMECandidateButton *first = CandidateButton(panel.contentView, 0);
        assert(fabs(NSHeight(panel.contentView.bounds) - NSMinX(first.frame) - NSMaxY(first.frame)) < 0.1);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

// 真正经过普通候选与展开渲染，验证明暗来自客户端颜色且没有增加跨进程读取。
static void TestHostAppearance(BOOL visual)
{
    NSString *suite = [@"msime.host-appearance." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.globalTheme = @"system";
    appearance.themeMode = @"system";
    appearance.candidateTheme = @"follow";
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    RowLayoutClient *client = [RowLayoutClient new];
    client.caret = NSMakeRect(400, 500, 1, 24);
    MSIMECandidatePanel *panel = visual ? [[MSIMECandidatePanel alloc] initWithContentRect:NSZeroRect styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO] :
        [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    NSDictionary *view = @{@"focused":@YES, @"editing_text":@"d", @"page":@0, @"page_count":@12,
        @"candidates":@[@{@"text":@"在", @"translation":@"in", @"highlighted":@YES}, @{@"text":@"石", @"translation":@"stone"}, @{@"text":@"帮助", @"translation":@"assistance"}]};
    [controller setValue:view forKey:@"view"];
    NSWindow *backdrop = nil;
    if (visual) {
        backdrop = [[NSWindow alloc] initWithContentRect:NSMakeRect(300, 200, 900, 500) styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
        backdrop.title = @"合成输入区明暗验收";
        [backdrop makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
    }
    if (@available(macOS 26.0, *)) {
        for (NSNumber *dark in @[@YES, @NO, @YES]) {
            NSColor *background = dark.boolValue ? [NSColor colorWithSRGBRed:0.1 green:0.1 blue:0.15 alpha:1] : NSColor.whiteColor;
            client.colors = @{NSBackgroundColorAttributeName:background};
            const NSUInteger reads = client.attributeReads;
            [controller renderCandidates];
            assert(client.attributeReads == reads + 1);
            assert([panel.appearance.name isEqual:dark.boolValue ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua]);
            MSIMECandidateChromeView *chrome = (id)panel.contentView;
            assert(chrome.liquidGlassEnabled);
            auto tokens = [appearance candidateWindowSkinForDark:dark.boolValue].tokens;
            if (chrome.liquidGlassActive) msime::mac::ApplyNativeGlassCandidateContrast(tokens, dark.boolValue);
            assert([CandidateButton(chrome, 1).numberColor isEqual:SkinColor(tokens.number)]);
            assert([CandidateButton(chrome, 1).translationColor isEqual:SkinColor(tokens.number)]);
            if (chrome.liquidGlassActive) {
                MSIMECandidateButton *selected = CandidateButton(chrome, 0);
                assert(CandidateContrast(selected.titleColor, selected.fillColor) >= 4.5);
            }
            if (visual) {
                backdrop.backgroundColor = background;
                [panel setFrameOrigin:NSMakePoint(400, 400)];
                printf("HOST visual window=%ld background=%s\n", (long)panel.windowNumber, dark.boolValue ? "dark" : "light");
                fflush(stdout);
                Spin(3);
            }
        }
        client.colors = @{NSForegroundColorAttributeName:NSColor.whiteColor};
        [controller renderCandidates];
        assert([panel.appearance.name isEqual:NSAppearanceNameDarkAqua]);
        client.colors = @{};
        [controller renderCandidates];
        assert(panel.appearance == nil);
        client.colors = @{NSBackgroundColorAttributeName:NSColor.blackColor};
        appearance.candidateTheme = @"light";
        [controller renderCandidates];
        assert([panel.appearance.name isEqual:NSAppearanceNameAqua]);
        appearance.candidateTheme = @"follow";
        appearance.themeMode = @"light";
        [controller renderCandidates];
        assert([panel.appearance.name isEqual:NSAppearanceNameAqua]);
        appearance.themeMode = @"system";
        NSMutableArray *expanded = [NSMutableArray array];
        for (NSUInteger at = 0; at < 9; ++at) [expanded addObject:@{@"text":@"合成", @"highlighted":@(at == 0)}];
        [controller setValue:expanded forKey:@"expandedCandidates"];
        [controller setValue:@0 forKey:@"expandedCursor"];
        assert([controller renderExpandedCandidates]);
        assert([panel.appearance.name isEqual:NSAppearanceNameDarkAqua]);
        client.colors = @{NSBackgroundColorAttributeName:NSColor.whiteColor};
        assert([controller renderExpandedCandidates]);
        assert([panel.appearance.name isEqual:NSAppearanceNameAqua]);
        for (NSDictionary *invalid in @[@{NSBackgroundColorAttributeName:@"not a color"}, @{NSBackgroundColorAttributeName:NSColor.clearColor}, @{NSForegroundColorAttributeName:NSColor.redColor}])
            assert(MSIMECandidateHostAppearance(invalid) == nil);
        assert([MSIMECandidateHostAppearance(@{NSBackgroundColorAttributeName:NSColor.whiteColor, NSForegroundColorAttributeName:NSColor.whiteColor}).name isEqual:NSAppearanceNameAqua]);
    }
    [panel orderOut:nil];
    [backdrop orderOut:nil];
    [controller setValue:nil forKey:@"activeClient"];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 使用生产控制器渲染 canvas，点击与候选身份仍通过原来的内容容器。
static void TestCanvasHost(const std::filesystem::path &root, NSString *identifier, NSString *output)
{
    NSString *suite = [@"app.msime.test.canvas-host." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:identifier forKey:@"MSIMEClientCustomCandidateSkin"];
    [defaults setObject:identifier forKey:@"MSIMEClientCustomCandidateSkinDark"];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
        skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    appearance.showsAppLogo = NO;
    appearance.showsCandidatePreedit = YES;
    appearance.showsCandidatePageNumber = NO;
    appearance.candidateTranslations = YES;
    appearance.candidateTheme = @"follow";
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    RowLayoutClient *client = [RowLayoutClient new];
    const NSRect screen = NSScreen.mainScreen.visibleFrame;
    client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
    RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    NSMutableArray *rows = [NSMutableArray array];
    for (NSUInteger at = 0; at < 3; ++at) [rows addObject:@{
        @"id":@{@"session":@1, @"generation":@7, @"index":@(at)}, @"text":@[@"合成", @"测试", @"候选"][at],
        @"translation":@[@"synthetic", @"test", @"candidate"][at], @"highlighted":@(at == 0)}];
    NSDictionary *view = @{@"session":@1, @"generation":@7, @"focused":@YES,
        @"editing_text":@"ceshi", @"page":@0, @"page_count":@1, @"candidates":rows};
    NSEvent *rightClick = [NSEvent mouseEventWithType:NSEventTypeRightMouseDown location:NSZeroPoint modifierFlags:0
        timestamp:0 windowNumber:0 context:nil eventNumber:1 clickCount:1 pressure:1];
    for (NSNumber *percent in @[@100, @150]) for (NSNumber *vertical in @[@NO, @YES]) for (NSNumber *dark in @[@NO, @YES]) {
        appearance.candidateScalePercent = percent.integerValue;
        appearance.candidateOpacityPercent = percent.integerValue == 150 ? 75 : 100;
        appearance.vertical = vertical.boolValue;
        appearance.themeMode = dark.boolValue ? @"dark" : @"light";
        [controller setValue:nil forKey:@"expandedCandidates"];
        [controller setValue:view forKey:@"view"];
        const auto skin = [appearance candidateWindowSkinForDark:dark.boolValue];
        assert(skin.canvas && skin.canvasPanelDrawn);
        assert([appearance canvasAssetsForDark:dark.boolValue] == [appearance canvasAssetsForDark:dark.boolValue]);
        const auto padding = msime::mac::CanvasContentInsets(*skin.canvas, msime::mac::CandidateInset(skin.tokens));
        for (NSNumber *expanded in @[@NO, @YES]) {
            if (expanded.boolValue) {
                [controller setValue:rows forKey:@"expandedCandidates"];
                [controller setValue:@0 forKey:@"expandedCursor"];
                assert([controller renderExpandedCandidates]);
            } else [controller renderCandidates];
            MSIMECandidateChromeView *chrome = (id)panel.contentView;
            assert(chrome.canvasPanelLoaded && !chrome.liquidGlassActive && !panel.hasShadow);
            const NSRect card = chrome.cardBounds;
            // NSWindow 按物理像素对齐，150% 的外留白可能落在半像素上。
            const CGFloat positionTolerance = 1.0 / panel.backingScaleFactor + 0.01;
            if (std::abs(NSMinX(panel.frame) + NSMinX(card) - NSMinX(client.caret)) > positionTolerance)
                fprintf(stderr, "canvas x window=%.3f card=%.3f caret=%.3f backing=%.3f percent=%ld expanded=%d\n",
                    NSMinX(panel.frame), NSMinX(card), NSMinX(client.caret), panel.backingScaleFactor,
                    (long)percent.integerValue, expanded.boolValue);
            assert(std::abs(NSMinX(panel.frame) + NSMinX(card) - NSMinX(client.caret)) <= positionTolerance);
            assert(std::abs(NSMinX(card) - skin.canvas->outer.left) < 0.01);
            assert(std::abs(NSMinY(card) - skin.canvas->outer.bottom) < 0.01);
            assert(std::abs(NSWidth(chrome.bounds) - NSWidth(card) - skin.canvas->outer.left - skin.canvas->outer.right) < 0.01);
            assert(std::abs(NSHeight(chrome.bounds) - NSHeight(card) - skin.canvas->outer.top - skin.canvas->outer.bottom) < 0.01);
            assert(NSEqualRects(chrome.candidateContentView.frame, card));
            MSIMECandidateButton *first = CandidateButton(chrome, 0);
            assert(first.superview == chrome.candidateContentView);
            const CGFloat itemRadius = [identifier isEqual:@"canvas-host"] ? 3.5 : [identifier isEqual:@"green_leaf"] ? 5 : 6;
            assert(std::abs(first.cornerRadius - itemRadius * percent.doubleValue / 100) < 0.01);
            assert(std::abs(NSMinX(first.frame) - padding.left) < 0.01);
            CGFloat bottom = CGFLOAT_MAX;
            for (NSInteger tag = 0; tag < 3; ++tag) {
                MSIMECandidateButton *button = CandidateButton(chrome, tag);
                assert(button && NSMaxX(button.frame) <= NSWidth(card) - padding.right + 0.01);
                bottom = MIN(bottom, NSMinY(button.frame));
            }
            assert(std::abs(bottom - padding.bottom) < 0.01);
            const NSPoint hit = [first convertPoint:NSMakePoint(NSMidX(first.bounds), NSMidY(first.bounds)) toView:chrome];
            assert([chrome hitTest:hit] == first);
            assert([first.titleColor isEqual:SkinColor(skin.tokens.selectedText)]);
            assert([CandidateButton(chrome, 1).titleColor isEqual:SkinColor(skin.tokens.text)]);
            assert([first.barColor isEqual:SkinColor(skin.tokens.selectedBar.value_or(skin.tokens.accent))]);
            if (expanded.boolValue) assert(first.action == @selector(selectExpandedCandidate:));
            else assert([first menuForEvent:rightClick] != nil);
            MSIMECandidatePreeditField *preedit = nil;
            NSMutableArray *levels = [NSMutableArray array];
            for (NSView *child in chrome.candidateContentView.subviews)
                if ([child.identifier isEqual:@"candidate-preedit"]) preedit = (id)child;
            assert(preedit && [preedit.textColor isEqual:SkinColor(skin.tokens.preeditText.value_or(skin.tokens.accent))]);
            assert([preedit.caretColor isEqual:SkinColor(skin.tokens.preeditCaret.value_or(skin.tokens.accent))]);
            // 预编辑在整条首行内居中；字号小于展开按钮时，下缘还包含居中偏移。
            const CGFloat scale = MSIMECandidateScale(appearance);
            const BOOL headerHasControls = expanded.boolValue || (appearance.showsCandidateExpandButton && [controller expandedCandidatesOffered]);
            const CGFloat headerHeight = MAX(NSHeight(preedit.frame), headerHasControls ? MSIMECandidateHeaderHeight * scale : 0);
            const CGFloat centeredOffset = floor((headerHeight - NSHeight(preedit.frame)) / 2);
            const auto preeditMargin = MSIMECandidatePreeditMargins(skin, YES, scale);
            assert(std::abs(NSMinY(preedit.frame) - NSMaxY(first.frame) -
                skin.canvas->lineGap.value_or(0) - preeditMargin.bottom - centeredOffset) < 0.01);
            for (NSView *child in chrome.subviews) {
                if ([child isKindOfClass:MSIMECandidateCanvasView.class]) {
                    [levels addObject:child.identifier];
                    assert([child hitTest:hit] == nil);
                    for (CALayer *sprite in child.layer.sublayers)
                        assert(std::abs(sprite.opacity - ([child.identifier isEqual:@"candidate-canvas-0"] ? skin.canvas->panelOpacity : 1.0)) < 0.01);
                } else if ([child isKindOfClass:MSIMECandidateCardBackgroundView.class]) [levels addObject:@"background"];
                else if (child == chrome.candidateContentView) [levels addObject:@"controls"];
            }
            assert(([levels isEqual:@[@"candidate-canvas--1", @"background", @"candidate-canvas-0", @"controls", @"candidate-canvas-1"]]));
            // 系统切换明暗而包 ID 不变时，候选按钮保持身份，贴图和文字一起换槽位。
            NSAppearance *originalAppearance = chrome.appearance;
            chrome.appearance = [NSAppearance appearanceNamed:dark.boolValue ? NSAppearanceNameAqua : NSAppearanceNameDarkAqua];
            [controller refreshCandidateSkin];
            assert(panel.contentView == chrome && CandidateButton(chrome, 0) == first);
            assert(chrome.canvasAssets == [appearance canvasAssetsForDark:!dark.boolValue]);
            const auto otherSkin = [appearance candidateWindowSkinForDark:!dark.boolValue];
            assert([first.titleColor isEqual:SkinColor(otherSkin.tokens.selectedText)]);
            if ([identifier isEqual:@"canvas-host"]) {
                for (NSView *child in chrome.subviews) if ([child.identifier isEqual:@"candidate-canvas-0"])
                    assert(CGImageGetWidth((__bridge CGImageRef)child.layer.sublayers[4].contents) == (dark.boolValue ? 10 : 8));
            }
            chrome.appearance = originalAppearance;
            [controller refreshCandidateSkin];
            assert(chrome.canvasAssets == [appearance canvasAssetsForDark:dark.boolValue]);
            NSUInteger canvasViews = 0;
            for (NSView *child in chrome.subviews) if ([child isKindOfClass:MSIMECandidateCanvasView.class]) ++canvasViews;
            assert(canvasViews == 3);
            if (output && percent.integerValue == 100) {
                chrome.wantsLayer = YES;
                [chrome displayIfNeeded];
                NSBitmapImageRep *bitmap = [chrome bitmapImageRepForCachingDisplayInRect:chrome.bounds];
                [chrome cacheDisplayInRect:chrome.bounds toBitmapImageRep:bitmap];
                NSString *file = [NSString stringWithFormat:@"%@-%@-%@-%@.png", identifier,
                    dark.boolValue ? @"dark" : @"light", vertical.boolValue ? @"vertical" : @"horizontal", expanded.boolValue ? @"expanded" : @"page"];
                assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
                    writeToFile:[output stringByAppendingPathComponent:file] atomically:YES]);
            }
        }
        [controller setValue:nil forKey:@"expandedCandidates"];
        [controller renderCandidates];
        MSIMECandidateButton *stale = CandidateButton(panel.contentView, 0);
        [controller renderCandidates];
        assert([stale menuForEvent:rightClick] == nil);
    }
    // 没有预编辑和翻页首行时，line_gap 不得在卡片顶部留下空行。
    appearance.showsCandidatePreedit = NO;
    client.caret = NSMakeRect(NSMidX(screen), NSMinY(screen) + 1, 1, 20);
    [controller setValue:nil forKey:@"expandedCandidates"];
    for (NSNumber *expanded in @[@NO, @YES]) {
        if (expanded.boolValue) {
            [controller setValue:rows forKey:@"expandedCandidates"];
            assert([controller renderExpandedCandidates]);
        } else [controller renderCandidates];
        MSIMECandidateChromeView *chrome = (id)panel.contentView;
        const auto skin = [appearance candidateWindowSkinForDark:YES];
        // 底部挂件留白较大时，整窗夹紧会把卡片向上推，不能用未夹紧的光标位置作期望。
        const CGFloat cardBottom = MAX(NSMaxY(client.caret) + 4, NSMinY(screen) + NSMinY(chrome.cardBounds));
        assert(std::abs(NSMinY(panel.frame) + NSMinY(chrome.cardBounds) - cardBottom) <= 1.0 / panel.backingScaleFactor + 0.01);
        assert(std::abs(NSMaxY(CandidateButton(chrome, 0).frame) - NSHeight(chrome.cardBounds) + skin.canvas->content->top) < 0.01);
    }
    [controller hideCandidatePanel:"empty"];
    [controller setValue:nil forKey:@"activeClient"];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestCanvasHostFixture()
{
    const auto root = std::filesystem::temp_directory_path() / ("msime-canvas-host-" + std::string(NSUUID.UUID.UUIDString.UTF8String));
    std::filesystem::create_directories(root / "canvas-host");
    WriteApng(root / "canvas-host" / "panel.png", 12, 12, 1, 0.1);
    WriteApng(root / "canvas-host" / "panel-light.png", 14, 12, 1, 0.1);
    {
        std::ofstream manifest(root / "canvas-host" / "skin.toml");
        manifest << R"toml(schema_version = 2
id = "canvas-host"
name = "合成 canvas 控制器夹具"
version = "1"
base = "canvas"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 240
line_gap_dip = 9
item_corner_radius_dip = 3.5
[candidate_window.border_image]
source = "panel.png"
light_source = "panel-light.png"
slice_dip = [2]
width_dip = [4]
[candidate_window.content_insets_dip]
top = 17
right = 21
bottom = 19
left = 23
[candidate_window.outer_insets_dip]
top = 11
right = 13
bottom = 7
left = 5
[[candidate_window.decorations]]
image = "panel.png"
width_dip = 12
height_dip = 18
anchor = "top-left"
z_index = 0
[[candidate_window.decorations]]
image = "panel.png"
width_dip = 16
height_dip = 12
anchor = "bottom-right"
z_index = 2
[candidate.light]
text = "#123456"
preedit_text = "#aabbcc"
preedit_caret = "#778899"
selected = "#112233"
selected_text = "#ffffff"
selected_bar = "#eecc77"
[candidate.dark]
text = "#abcdef"
preedit_text = "#ccbbaa"
preedit_caret = "#998877"
selected = "#112233"
selected_text = "#ffffff"
selected_bar = "#77ccee"
)toml";
        assert(manifest.good());
    }
    TestCanvasHost(root, @"canvas-host", nil);
    std::filesystem::remove_all(root);
}

int main(int argc, char **argv)
{
    @autoreleasepool
    {
        [NSApplication sharedApplication];
        if (argc == 2 && std::string(argv[1]) == "--glass-host-visual") { TestHostAppearance(YES); return 0; }
        if (argc == 4 && std::string(argv[1]) == "--canvas-upstream") {
            NSString *output = @(argv[3]);
            assert([NSFileManager.defaultManager createDirectoryAtPath:output withIntermediateDirectories:YES attributes:nil error:nil]);
            TestCanvasHost(std::filesystem::path(argv[2]), @"green_leaf", output);
            TestCanvasHost(std::filesystem::path(argv[2]), @"sakura-night", output);
            return 0;
        }
        TestCanvasHostFixture();
        TestHostAppearance(NO);
        NSString *suite = [@"app.msime.test.rowfit." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        appearance.vertical = NO;
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        RowLayoutClient *client = [RowLayoutClient new];
        NSRect screen = NSScreen.mainScreen.visibleFrame;
        client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
        RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
                                                                  styleMask:NSWindowStyleMaskBorderless |
                                                                            NSWindowStyleMaskNonactivatingPanel
                                                                    backing:NSBackingStoreBuffered
                                                                      defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];

        NSDictionary *singleView = @{@"focused": @YES, @"editing_text": @"ceshi", @"page": @0, @"page_count": @1,
                                     @"candidates": @[@{@"text": @"测试", @"highlighted": @YES}]};
        [controller setValue:singleView forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *measured = CandidateButton(panel.contentView, 0);
        assert(measured);
        // 实际产品渲染入口启用系统材质，切到自定义主题后恢复原来的皮肤底板。
        assert(((MSIMECandidateChromeView *)panel.contentView).liquidGlassEnabled);
        appearance.globalTheme = @"custom";
        [controller renderCandidates];
        assert(!((MSIMECandidateChromeView *)panel.contentView).liquidGlassEnabled);
        appearance.globalTheme = @"system";
        [controller renderCandidates];
        measured = CandidateButton(panel.contentView, 0);
        NSFont *rowFont = measured.font;
        const CGFloat glyphWidth = [@"水" sizeWithAttributes:@{NSFontAttributeName: rowFont}].width;
        assert(glyphWidth > 0);

        const CGFloat halfScreen = MAX(80, floor(screen.size.width * 0.5));

        // A one-character page still gets a card at least seven times the candidate font wide, as the Windows card and the source skins' `min-width: 7em` do.
        NSMutableDictionary *oneCharacterView = [singleView mutableCopy];
        oneCharacterView[@"candidates"] = @[@{@"text": @"的", @"highlighted": @YES}];
        [controller setValue:[oneCharacterView copy] forKey:@"view"];
        [controller renderCandidates];
        assert(panel.contentView.frame.size.width >= MIN(halfScreen, 7 * rowFont.pointSize) - 0.5);
        appearance.vertical = YES;
        [controller renderCandidates];
        assert(panel.contentView.frame.size.width >= MIN(halfScreen, 7 * rowFont.pointSize) - 0.5);
        // A vertical row stretches across the whole card.
        MSIMECandidateButton *oneCharacter = CandidateButton(panel.contentView, 0);
        assert(oneCharacter && fabs(NSWidth(oneCharacter.frame) - (panel.contentView.frame.size.width - 2 * NSMinX(oneCharacter.frame))) <= 1.0);
        appearance.vertical = NO;
        NSString *(^glyphs)(CGFloat) = ^NSString *(CGFloat points) {
            return [@"" stringByPaddingToLength:(NSUInteger)MAX(2.0, floor(points / glyphWidth))
                                     withString:@"水杉输入法" startingAtIndex:0];
        };

        // A sentence worth four fifths of half the screen, then eight candidates that together take far more than a whole screen line: the card grows past half the screen towards the screen less its margins, and the page still breaks onto new lines.
        NSString *sentence = glyphs(halfScreen * 0.8 - 60);
        NSString *shortCandidate = glyphs(halfScreen * 0.24);
        NSMutableArray *page = [NSMutableArray arrayWithObject:@{@"text": sentence, @"highlighted": @YES}];
        while (page.count < 9) [page addObject:@{@"text": shortCandidate}];
        NSMutableDictionary *pageView = [singleView mutableCopy];
        pageView[@"candidates"] = [page copy];
        [controller setValue:[pageView copy] forKey:@"view"];
        [controller renderCandidates];

        MSIMECandidateButton *sentenceButton = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *tailButton = CandidateButton(panel.contentView, 8);
        assert(sentenceButton && tailButton);
        const CGFloat sentenceWidth = ceil([sentence sizeWithAttributes:@{NSFontAttributeName: rowFont}].width);
        // The head keeps its full width and every candidate keeps its natural width; the ones that do not fit start new lines below.
        assert(sentenceButton.frame.size.width >= sentenceWidth);
        assert(!sentenceButton.itemLayout.textWrapped);
        assert(NSMaxY(tailButton.frame) <= NSMinY(sentenceButton.frame) + 0.5);
        assert(!tailButton.itemLayout.textWrapped);
        assert(fabs(tailButton.frame.size.width - CandidateButton(panel.contentView, 1).frame.size.width) <= 1.0);
        for (NSInteger tag = 0; tag < 9; ++tag) {
            NSRect frame = CandidateButton(panel.contentView, tag).frame;
            assert(NSMinX(frame) >= 0 && NSMaxX(frame) <= panel.frame.size.width + 0.5 && NSMinY(frame) >= 0);
        }
        assert(panel.frame.size.width <= MAX(halfScreen, floor(screen.size.width - 2 * MSIMECandidateScreenMargin)) + 0.5);

        // A page that fits keeps every candidate at its natural width on one line.
        NSMutableDictionary *narrowView = [singleView mutableCopy];
        narrowView[@"candidates"] = @[@{@"text": @"测试", @"highlighted": @YES}, @{@"text": @"测试测试"}];
        [controller setValue:[narrowView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *shorter = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *longer = CandidateButton(panel.contentView, 1);
        assert(shorter && longer);
        assert(longer.frame.size.width > shorter.frame.size.width);
        assert(longer.frame.size.width - shorter.frame.size.width >= 2 * glyphWidth - 1.0);
        assert(shorter.frame.origin.y == longer.frame.origin.y && NSMaxX(shorter.frame) == NSMinX(longer.frame));
        const CGFloat oneLine = shorter.frame.size.height;

        // One candidate wider than a whole line is narrowed to the line and wraps inside it rather than being cut off.
        NSString *paragraph = glyphs(screen.size.width * 3);
        NSMutableDictionary *wideView = [singleView mutableCopy];
        wideView[@"candidates"] = @[@{@"text": paragraph, @"highlighted": @YES}, @{@"text": @"测试"}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *wrapped = CandidateButton(panel.contentView, 0);
        assert(wrapped.itemLayout.textWrapped && wrapped.frame.size.height > oneLine * 1.5);
        assert(panel.frame.size.width <= MAX(halfScreen, floor(screen.size.width - 2 * MSIMECandidateScreenMargin)) + 0.5 && NSMaxX(wrapped.frame) <= panel.frame.size.width + 0.5);
        assert(NSMaxY(CandidateButton(panel.contentView, 1).frame) <= NSMinY(wrapped.frame) + 0.5);

        // A horizontal page whose glosses are wider than half the screen grows past it instead of wrapping them, as long as the screen has room: every gloss keeps the single line a short one gets.
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"translation": @"Chinese", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *shortGloss = CandidateButton(panel.contentView, 0);
        const CGFloat glossLine = shortGloss.itemLayout.translation.height;
        assert(glossLine > 0 && shortGloss.translationFont);
        // Six glosses of a quarter of half the screen each: half again as wide as a half-screen card, still well inside the screen.
        const CGFloat glossUnit = [@"gloss " sizeWithAttributes:@{NSFontAttributeName: shortGloss.translationFont}].width;
        NSString *sentenceGloss = [@"" stringByPaddingToLength:(NSUInteger)MAX(8.0, ceil(halfScreen / 4 / glossUnit * 6)) withString:@"gloss " startingAtIndex:0];
        NSMutableArray *glossPage = [NSMutableArray array];
        while (glossPage.count < 6) [glossPage addObject:@{@"text": @"测试", @"translation": sentenceGloss}];
        glossPage[0] = @{@"text": @"测试", @"translation": sentenceGloss, @"highlighted": @YES};
        wideView[@"candidates"] = [glossPage copy];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        CGFloat glossTotal = 0;
        for (NSInteger tag = 0; tag < 6; ++tag) {
            MSIMECandidateButton *button = CandidateButton(panel.contentView, tag);
            assert(button && fabs(button.itemLayout.translation.height - glossLine) < 0.5);
            assert(fabs(NSMinY(button.frame) - NSMinY(CandidateButton(panel.contentView, 0).frame)) < 0.5);
            glossTotal += button.itemLayout.translation.width;
        }
        assert(glossTotal > halfScreen && panel.frame.size.width > halfScreen + 0.5);
        assert(panel.frame.size.width <= screen.size.width + 0.5);

        // Vertical: the card is capped at half the screen, a long sentence wraps into a taller row than its neighbours, and rows stack at their own heights.
        appearance.vertical = YES;
        wideView[@"candidates"] = @[@{@"text": @"测试", @"highlighted": @YES}, @{@"text": paragraph}, @{@"text": @"测试测试"}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *first = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *tall = CandidateButton(panel.contentView, 1);
        MSIMECandidateButton *last = CandidateButton(panel.contentView, 2);
        assert(first && tall && last);
        assert(panel.frame.size.width <= halfScreen + 0.5);
        assert(tall.itemLayout.textWrapped && tall.frame.size.height > first.frame.size.height * 2);
        assert(fabs(first.frame.size.height - last.frame.size.height) < 0.5);
        assert(fabs(NSMinY(first.frame) - NSMaxY(tall.frame)) < 0.5 && fabs(NSMinY(tall.frame) - NSMaxY(last.frame)) < 0.5);
        assert(first.frame.size.width == tall.frame.size.width && tall.frame.size.width == last.frame.size.width);

        // The 辅助码 is a run of its own: the button's text is the candidate alone, the annotation is drawn after it, and the tooltip and accessibility label keep the combined form.
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"annotation": @"(aB)", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *annotated = CandidateButton(panel.contentView, 0);
        assert(![annotated.title containsString:@"(aB)"] && [annotated.title containsString:@"汉语"]);
        assert([annotated.annotation isEqual:@"(aB)"] && !annotated.itemLayout.annotation.below);
        assert([annotated.toolTip isEqual:@"汉语(aB)"] && [annotated.accessibilityLabel containsString:@"汉语(aB)"]);
        // A text that fills the column pushes the annotation under it, where it takes a line of its own.
        wideView[@"candidates"] = @[@{@"text": paragraph, @"annotation": @"(aB)", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        annotated = CandidateButton(panel.contentView, 0);
        assert(annotated.itemLayout.annotation.below && annotated.itemLayout.annotation.y >= annotated.itemLayout.textHeight);
        assert(annotated.frame.size.height >= annotated.itemLayout.height - 0.5);

        // A vertical gloss stays on the candidate's line when it fits and wraps under it when it does not.
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"translation": @"Chinese", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        assert(!CandidateButton(panel.contentView, 0).itemLayout.translation.below);
        NSString *longGloss = [@"" stringByPaddingToLength:(NSUInteger)(screen.size.width / 4) withString:@"gloss " startingAtIndex:0];
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"translation": longGloss, @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *glossed = CandidateButton(panel.contentView, 0);
        assert(glossed.itemLayout.translation.below && glossed.translationBelow);
        assert(glossed.itemLayout.translation.height > glossed.itemLayout.textHeight);
        assert(panel.frame.size.width <= halfScreen + 0.5);
        NSBitmapImageRep *bitmap = [glossed bitmapImageRepForCachingDisplayInRect:glossed.bounds];
        assert(bitmap);
        [glossed cacheDisplayInRect:glossed.bounds toBitmapImageRep:bitmap];

        // A Korean Hanja row shows only the Hanja on its line. Its 훈음 is drawn on the gloss line under it whatever the translation switches say, never as `translation`, which is what the gloss chords commit; the tooltip and accessibility label keep it apart from the Hanja.
        appearance.vertical = NO;
        appearance.candidateTranslations = NO;
        appearance.candidateEnglishGloss = NO;
        NSString *reading = @"나라 이름 한, 한나라 한";
        NSDictionary *hanjaBase = @{@"focused": @YES, @"scheme": @(msime::mac::KoreanScheme), @"local_mode": @"none", @"editing_text": @"한",
                                    @"candidate_list_open": @YES, @"page": @0, @"page_count": @1};
        NSMutableDictionary *hanjaView = [hanjaBase mutableCopy];
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"annotation": reading, @"highlighted": @YES}, @{@"text": @"漢", @"annotation": @"한수 한"}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *hanja = CandidateButton(panel.contentView, 0);
        assert(hanja && [hanja.title isEqual:@"1  韓"] && hanja.annotation.length == 0 && hanja.itemLayout.annotation.width == 0);
        assert([hanja.glossReading isEqual:reading] && hanja.translation.length == 0);
        assert(hanja.itemLayout.translation.width > 0 && hanja.itemLayout.translation.below && hanja.translationBelow);
        assert(hanja.itemLayout.translation.y >= hanja.itemLayout.textHeight);
        assert([hanja.toolTip isEqual:[@"韓\n" stringByAppendingString:reading]]);
        assert([hanja.accessibilityLabel isEqual:[@"1  韓 " stringByAppendingString:reading]]);
        // The Hanja's own column is no wider than the reading line under it needs: an inline 훈음 at the candidate size made it several times wider.
        const CGFloat readingWidth = ceil([reading sizeWithAttributes:@{NSFontAttributeName: hanja.translationFont}].width);
        const CGFloat inlineWidth = ceil([reading sizeWithAttributes:@{NSFontAttributeName: rowFont}].width);
        assert(hanja.itemLayout.translation.width <= readingWidth + 0.5 && readingWidth < inlineWidth);
        bitmap = [hanja bitmapImageRepForCachingDisplayInRect:hanja.bounds];
        [hanja cacheDisplayInRect:hanja.bounds toBitmapImageRep:bitmap];

        // The 훈음 line is reserved with both translation switches off, so a Hanja with no 훈음 is as tall as one with it and the card does not jump; a Chinese page reserves nothing then.
        const CGFloat hanjaHeight = NSHeight(hanja.frame);
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"highlighted": @YES}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *bare = CandidateButton(panel.contentView, 0);
        assert(bare.glossReading.length == 0 && fabs(NSHeight(bare.frame) - hanjaHeight) < 0.5);
        [controller setValue:@{@"focused": @YES, @"editing_text": @"han", @"page": @0, @"page_count": @1,
                               @"candidates": @[@{@"text": @"韩", @"highlighted": @YES}]} forKey:@"view"];
        [controller renderCandidates];
        assert(NSHeight(CandidateButton(panel.contentView, 0).frame) + glossLine * 0.5 < hanjaHeight);

        // A real translation goes on the line after the 훈음, and only it is the row's `translation`.
        appearance.candidateTranslations = YES;
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"annotation": reading, @"translation": @"Korea", @"highlighted": @YES}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *translated = CandidateButton(panel.contentView, 0);
        assert([translated.translation isEqual:@"Korea"] && [translated.glossReading isEqual:reading]);
        assert(translated.itemLayout.translation.height > glossLine * 1.5);
        NSString *translatedTip = [NSString stringWithFormat:@"韓\n%@\nKorea", reading];
        assert([translated.toolTip isEqual:translatedTip]);
        // An armed translation column underlines the translation, not the reading above it.
        translated.armedGlossColumn = 1;
        bitmap = [translated bitmapImageRepForCachingDisplayInRect:translated.bounds];
        [translated cacheDisplayInRect:translated.bounds toBitmapImageRep:bitmap];

        // Vertical follows the vertical gloss rule: the 훈음 stays beside the Hanja when it fits, and nothing is reserved.
        appearance.vertical = YES;
        appearance.candidateTranslations = NO;
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"annotation": reading, @"highlighted": @YES}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *verticalHanja = CandidateButton(panel.contentView, 0);
        assert(verticalHanja.itemLayout.translation.width > 0 && !verticalHanja.itemLayout.translation.below && !verticalHanja.translationBelow);
        assert(verticalHanja.annotation.length == 0 && [verticalHanja.glossReading isEqual:reading]);

        // 脚本插件的注释收拾成一行再画：换行、U+2028、双向控制符和零宽字符连成的一段变成一个空格，首尾空白去掉，不是字符串时为空，超过 32 个 UTF-16 码元时在完整字符处截断补「…」，不拆开代理对。
        assert([MSIMECandidateScriptAnnotationRun(@"甲\n乙\u2028丙\u202E丁\u200B戊\u2029") isEqual:@"甲 乙 丙 丁 戊"]);
        assert([MSIMECandidateScriptAnnotationRun(@"\r\n 常用 \t") isEqual:@"常用"]);
        assert(MSIMECandidateScriptAnnotationRun(@42).length == 0 && MSIMECandidateScriptAnnotationRun(nil).length == 0);
        assert(MSIMECandidateScriptAnnotationRun(@"\u202E \u200B").length == 0);
        NSString *thirtyTwo = [@"" stringByPaddingToLength:32 withString:@"水" startingAtIndex:0];
        assert([MSIMECandidateScriptAnnotationRun(thirtyTwo) isEqual:thirtyTwo]);
        NSString *cutCJK = MSIMECandidateScriptAnnotationRun([thirtyTwo stringByAppendingString:@"杉杉"]);
        assert(cutCJK.length == 32 && [cutCJK hasSuffix:@"…"] && [cutCJK hasPrefix:[thirtyTwo substringToIndex:31]]);
        NSString *emoji = [@"" stringByPaddingToLength:40 withString:@"😀" startingAtIndex:0];
        NSString *cutEmoji = MSIMECandidateScriptAnnotationRun(emoji);
        assert(cutEmoji.length == 31 && [cutEmoji isEqual:[[emoji substringToIndex:30] stringByAppendingString:@"…"]]);

        // 横排：插件注释是释义行的最后一行，排在译文之后；它不进引擎辅助码那一段，也不是按列上屏的 `translation`。悬停提示带着它，读屏标签说明它是插件注释；没有注释的行什么都不多。
        appearance.vertical = NO;
        appearance.candidateTranslations = YES;
        NSDictionary *scriptBase = @{@"focused": @YES, @"editing_text": @"hanyu", @"page": @0, @"page_count": @1};
        NSMutableDictionary *scriptView = [scriptBase mutableCopy];
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"annotation": @"(aB)", @"translation": @"Chinese", @"script_annotation": @"常用词", @"highlighted": @YES},
                                      @{@"text": @"韩语", @"annotation": @"(aB)", @"translation": @"Korean"}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *scripted = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *unscripted = CandidateButton(panel.contentView, 1);
        assert(scripted && unscripted);
        assert([scripted.title isEqual:@"1  汉语"] && [scripted.annotation isEqual:@"(aB)"] && [scripted.translation isEqual:@"Chinese"]);
        assert([scripted.scriptAnnotation isEqual:@"常用词"] && unscripted.scriptAnnotation.length == 0);
        assert(scripted.itemLayout.translation.below && scripted.translationBelow);
        assert(scripted.itemLayout.translation.height > unscripted.itemLayout.translation.height + glossLine * 0.5);
        assert([scripted.toolTip isEqual:@"汉语(aB)\nChinese\n常用词"] && [unscripted.toolTip isEqual:@"韩语(aB)\nKorean"]);
        assert([scripted.accessibilityLabel isEqual:@"1  汉语(aB)，插件注释：常用词"]);
        assert([unscripted.accessibilityLabel isEqual:@"2  韩语(aB)"]);
        bitmap = [scripted bitmapImageRepForCachingDisplayInRect:scripted.bounds];
        [scripted cacheDisplayInRect:scripted.bounds toBitmapImageRep:bitmap];

        // 没有译文时插件注释自己占释义行；插件写进去的换行和零宽字符画不出第二行。
        appearance.candidateTranslations = NO;
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"script_annotation": @"第一行\n\u200B第二行", @"highlighted": @YES}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *alone = CandidateButton(panel.contentView, 0);
        assert([alone.scriptAnnotation isEqual:@"第一行 第二行"] && alone.translation.length == 0);
        assert(alone.itemLayout.translation.width > 0 && alone.itemLayout.translation.below);
        assert(alone.itemLayout.translation.height < glossLine * 1.5);
        assert([alone.toolTip isEqual:@"汉语\n第一行 第二行"]);

        // 只有注释变了的视图不走只重画译文的捷径，按钮换成新的注释。
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"script_annotation": @"别的注释", @"highlighted": @YES}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        assert([CandidateButton(panel.contentView, 0).scriptAnnotation isEqual:@"别的注释"]);

        // 竖排：注释放得下时和释义一样留在候选那一行；比整列还长时在列内折行，不撑出卡片。
        appearance.vertical = YES;
        [controller renderCandidates];
        MSIMECandidateButton *verticalScripted = CandidateButton(panel.contentView, 0);
        assert(!verticalScripted.itemLayout.translation.below && !verticalScripted.translationBelow);
        NSString *wideAnnotation = [@"" stringByPaddingToLength:32 withString:@"﷽" startingAtIndex:0];
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"script_annotation": wideAnnotation, @"highlighted": @YES}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *wideScripted = CandidateButton(panel.contentView, 0);
        assert([wideScripted.scriptAnnotation isEqual:wideAnnotation]);
        assert(panel.frame.size.width <= halfScreen + 0.5 && NSMaxX(wideScripted.frame) <= panel.frame.size.width + 0.5);
        assert(wideScripted.itemLayout.translation.x + wideScripted.itemLayout.translation.width <= NSWidth(wideScripted.frame) + 0.5);
        assert(NSHeight(wideScripted.frame) >= wideScripted.itemLayout.height - 0.5);
        bitmap = [wideScripted bitmapImageRepForCachingDisplayInRect:wideScripted.bounds];
        [wideScripted cacheDisplayInRect:wideScripted.bounds toBitmapImageRep:bitmap];

        // 读屏：高亮的那一行报告自己被选中，别的行和翻页按钮都不报告；每画一页就请求朗读高亮的那一行。AI 占位行（来源 12，共享内核已经不再产出）读成它的意思，不读省略号或破折号。
        appearance.vertical = NO;
        NSDictionary *accessibleBase = @{@"focused": @YES, @"editing_text": @"ceshi", @"page": @0, @"page_count": @2};
        NSArray<NSDictionary *> *(^accessibleRows)(NSUInteger, NSString *) = ^NSArray<NSDictionary *> *(NSUInteger highlighted, NSString *placeholder) {
            NSArray<NSDictionary *> *rows = @[@{@"text": @"测试"}, @{@"text": @"侧室"}, @{@"text": placeholder, @"source": @12}];
            NSMutableArray<NSDictionary *> *marked = [NSMutableArray array];
            for (NSUInteger index = 0; index < rows.count; ++index) {
                NSMutableDictionary *row = [rows[index] mutableCopy];
                row[@"highlighted"] = @(index == highlighted);
                [marked addObject:row];
            }
            return marked;
        };
        void (^renderAccessible)(NSUInteger, NSString *) = ^(NSUInteger highlighted, NSString *placeholder) {
            NSMutableDictionary *view = [accessibleBase mutableCopy];
            view[@"candidates"] = accessibleRows(highlighted, placeholder);
            [controller setValue:[view copy] forKey:@"view"];
            [controller renderCandidates];
            for (NSInteger tag = 0; tag < 3; ++tag)
                assert(CandidateButton(panel.contentView, tag).isAccessibilitySelected == (tag == (NSInteger)highlighted));
            for (NSInteger tag : {-1, -2})
                if (MSIMECandidateButton *arrow = CandidateButton(panel.contentView, tag)) assert(!arrow.isAccessibilitySelected);
        };
        renderAccessible(0, @"…");
        assert([[controller valueForKey:@"announcedCandidate"] isEqual:@"1  测试"]);
        assert([CandidateButton(panel.contentView, 2).accessibilityLabel isEqual:@"3  AI 候选等待中"]);
        renderAccessible(1, @"…");
        assert([[controller valueForKey:@"announcedCandidate"] isEqual:@"2  侧室"]);
        renderAccessible(2, @"…");
        assert([[controller valueForKey:@"announcedCandidate"] isEqual:@"3  AI 候选等待中"]);
        renderAccessible(2, @"—");
        assert([[controller valueForKey:@"announcedCandidate"] isEqual:@"3  AI 候选没有结果"]);
        // 没有画皮肤包时原生控制器立即显示与隐藏：没有进场、选中装饰和退场。皮肤包写了动效时的行为由 candidate-panel 测试覆盖。
        assert(panel.alphaValue == 1.0);
        assert(panel.animationBehavior == NSWindowAnimationBehaviorNone);
        assert(CATransform3DIsIdentity(panel.contentView.layer.transform));
        assert([panel.contentView.layer animationForKey:@"msime-candidate-enter-opacity"] == nil);
        for (NSView *row in MSIMECandidateControlsView(panel.contentView).subviews)
            assert([row.layer animationForKey:@"msime-candidate-selection"] == nil);
        [controller hideCandidatePanel:"empty"];
        assert(!panel.isVisible);
        [controller renderCandidates];
        assert(panel.alphaValue == 1.0);
        // 使用有效的合成候选身份，验证按需创建、帧内复用和旧按钮拒绝菜单。
        NSDictionary *menuView = @{@"session": @1, @"generation": @7, @"focused": @YES,
                                  @"editing_text": @"d", @"page": @0, @"page_count": @1,
                                  @"candidates": @[@{@"id": @{@"session": @1, @"generation": @7, @"index": @0},
                                                     @"text": @"合成", @"highlighted": @YES}]};
        [controller setValue:menuView forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *menuRow = CandidateButton(panel.contentView, 0);
        assert(menuRow.menu == nil);
        NSEvent *rightClick = [NSEvent mouseEventWithType:NSEventTypeRightMouseDown location:NSZeroPoint modifierFlags:0
                                               timestamp:0 windowNumber:0 context:nil eventNumber:1 clickCount:1 pressure:1];
        NSMenu *menu = [menuRow menuForEvent:rightClick];
        assert(menu && menu.numberOfItems == 3);
        assert([menuRow menuForEvent:rightClick] == menu);
        [controller renderCandidates];
        assert([menuRow menuForEvent:rightClick] == nil);
        assert(CandidateButton(panel.contentView, 0).menu == nil);

        // 可选的合成显隐样本，仅输出耗时与菜单构建次数，不输出输入内容。
        if (getenv("MSIME_CANDIDATE_BENCHMARK")) {
            NSMutableArray *sample = [NSMutableArray array];
            for (NSUInteger slot = 0; slot < 9; ++slot)
                [sample addObject:@{@"id": @{@"session": @1, @"generation": @7, @"index": @(slot)},
                                    @"text": @"合成", @"highlighted": @(slot == 0)}];
            NSDictionary *shown = @{@"session": @1, @"generation": @7, @"focused": @YES,
                                    @"editing_text": @"d", @"candidates": sample, @"page": @0, @"page_count": @1};
            NSDictionary *hidden = @{@"candidates": @[]};
            std::vector<double> timings;
            for (NSUInteger iteration = 0; iteration < 60; ++iteration) {
                @autoreleasepool {
                    const uint64_t started = clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
                    [controller setValue:shown forKey:@"view"];
                    [controller renderCandidates];
                    [controller setValue:hidden forKey:@"view"];
                    [controller renderCandidates];
                    timings.push_back((clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - started) / 1e6);
                }
            }
            std::sort(timings.begin(), timings.end());
            printf("candidate show/hide: median=%.3fms p95=%.3fms\n", timings[30], timings[57]);
        }
        // 首选与悬停行按相同内边距推导圆角，覆盖极端设置和缩放。内边距 0 时窗口只留 1pt 细边框的宽度（#7974），边框不随缩放变粗，所以各档缩放都只扣 1pt。
        appearance.candidateCornerRadius = @32;
        appearance.candidatePadding = @0;
        for (NSNumber *scale in @[@50, @100, @150, @200]) {
            appearance.candidateScalePercent = scale.integerValue;
            renderAccessible(0, @"…");
            MSIMECandidateButton *selected = CandidateButton(panel.contentView, 0);
            MSIMECandidateButton *hovered = CandidateButton(panel.contentView, 1);
            hovered.candidateHovered = YES;
            assert(selected.cornerRadius == 32.0 * scale.doubleValue / 100.0 - 1.0);
            assert(hovered.cornerRadius == selected.cornerRadius);
            assert(std::abs(NSMinX(selected.frame) - 1.0) < 0.001);
        }
        // 前一组覆盖到 200% 缩放，首行布局断言使用 100% 的坐标。
        appearance.candidateScalePercent = 100;
        // 直接驱动实际展开渲染路径，避免其它快捷键测试的失败遮住布局回归。
        NSMutableArray *expanded = [NSMutableArray array];
        for (NSUInteger slot = 0; slot < 9; ++slot)
            [expanded addObject:@{@"text": @"合成", @"highlighted": @(slot == 0)}];
        [controller setValue:expanded forKey:@"expandedCandidates"];
        [controller setValue:@0 forKey:@"expandedCursor"];
        appearance.vertical = NO;
        appearance.showsCandidatePageNumber = NO;
        appearance.showsAppLogo = NO;
        appearance.showsCandidatePreedit = NO;
        assert([controller renderExpandedCandidates]);
        assert(((MSIMECandidateChromeView *)panel.contentView).liquidGlassEnabled);
        for (NSView *row in MSIMECandidateControlsView(panel.contentView).subviews) {
            assert(![row.identifier isEqual:@"candidate-page-indicator"]);
            assert(![row.identifier isEqual:@"candidate-collapse"]);
        }
        const CGFloat inset = msime::mac::CandidateInset([appearance candidateWindowSkinForDark:NO].tokens);
        assert(std::abs(NSMaxY(CandidateButton(panel.contentView, 0).frame) - (NSHeight(panel.contentView.bounds) - inset)) < .01);
        appearance.showsCandidatePreedit = YES;
        assert([controller renderExpandedCandidates]);
        assert(NSMaxY(CandidateButton(panel.contentView, 0).frame) < NSHeight(panel.contentView.bounds) - inset);
        appearance.showsCandidatePageNumber = YES;
        assert([controller renderExpandedCandidates]);
        assert([CandidateButton(panel.contentView, -3).identifier isEqual:@"candidate-collapse"]);
        TestPronunciationLine(controller, panel, appearance);
        TestSelectedGlossColour(client);
        TestFactoryFitAndJustify(client);
        TestFactoryPreeditMargins(client);
        TestFactoryCornerRadii(client);
        TestDeclaredMotion(client);
        TestDecorationMotion(client);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    return 0;
}
