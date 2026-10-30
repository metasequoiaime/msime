#import "../../src/candidate/CandidatePanel.h"
#import "../../src/candidate/CandidateMotion.h"
#import "../../src/candidate/CandidateSkinAppearance.h"
#import "../../src/candidate/CandidateTypography.h"
#import <ImageIO/ImageIO.h>
#import <QuartzCore/QuartzCore.h>
#include <cmath>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <stdexcept>
#include <unistd.h>

static void Require(bool condition, const char *message)
{
    if (!condition)
        throw std::runtime_error(message);
}

// 让 dispatch_after 安排的进场、退场和选中结束回调有机会运行。
static void Spin(NSTimeInterval seconds)
{
    [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:seconds]];
}

static NSButton *CandidateButton(MetasequoiaCandidatePanel *panel, NSInteger tag)
{
    for (NSView *view in panel.window.contentView.subviews)
        if ([view isKindOfClass:NSButton.class] && view.tag == tag)
            return (NSButton *)view;
    return nil;
}

static bool CardEntering(MetasequoiaCandidatePanel *panel)
{
    return [panel.window.contentView.layer animationForKey:@"msime-candidate-enter-opacity"] != nil;
}

// 候选界面动效一律不播：显示不透明、无进场，换选中无装饰，收起没有退场窗。
static void RequireInstant(MetasequoiaCandidatePanel *panel, MSIMECandidateMotion *motion, const char *message)
{
    [panel hide];
    Spin(0.5);
    [panel show:kIMKLocateCandidatesBelowHint];
    Require(panel.isVisible && panel.window.alphaValue == 1.0 && !CardEntering(panel) && panel.window.hasShadow, message);
    [panel selectCandidateWithIdentifier:panel.selectedCandidate == 0 ? 1 : 0];
    Require(motion.selectionLayer == nil, message);
    [panel hide];
    Require(!panel.isVisible && !motion.exiting, message);
}

// 写一张 width×height、frames 帧、每帧 delay 秒的 APNG，各帧颜色不同；loops 为 0 时无限循环。
static void WriteApng(const std::filesystem::path &path, size_t width, size_t height, size_t frames, double delay, unsigned loops)
{
    NSURL *url = [NSURL fileURLWithPath:@(path.c_str())];
    CGImageDestinationRef destination = CGImageDestinationCreateWithURL((__bridge CFURLRef)url, CFSTR("public.png"), frames, nullptr);
    Require(destination != nullptr, "Failed to create the APNG fixture.");
    CGImageDestinationSetProperties(destination, (__bridge CFDictionaryRef) @{
        (__bridge NSString *)kCGImagePropertyPNGDictionary : @{(__bridge NSString *)kCGImagePropertyAPNGLoopCount : @(loops)}
    });
    CGColorSpaceRef space = CGColorSpaceCreateDeviceRGB();
    for (size_t index = 0; index < frames; ++index)
    {
        CGContextRef context = CGBitmapContextCreate(nullptr, width, height, 8, 0, space, kCGImageAlphaPremultipliedLast);
        CGContextSetRGBFillColor(context, (index % 3) == 0, (index % 3) == 1, (index % 3) == 2, 1.0);
        CGContextFillRect(context, CGRectMake(0, 0, width, height));
        CGImageRef image = CGBitmapContextCreateImage(context);
        CGImageDestinationAddImage(destination, image, (__bridge CFDictionaryRef) @{
            (__bridge NSString *)kCGImagePropertyPNGDictionary : @{(__bridge NSString *)kCGImagePropertyAPNGDelayTime : @(delay)}
        });
        CGImageRelease(image);
        CGContextRelease(context);
    }
    CGColorSpaceRelease(space);
    Require(CGImageDestinationFinalize(destination), "Failed to write the APNG fixture.");
    CFRelease(destination);
}

static CAKeyframeAnimation *DecorationAnimation(NSView *decoration, NSString *key)
{
    return (CAKeyframeAnimation *)[decoration.layer animationForKey:key];
}

// 装饰图动画从哪个时刻起算（CACurrentMediaTime 的时刻）。
static CFTimeInterval DecorationBegin(NSView *decoration, NSString *key)
{
    return [decoration.layer convertTime:DecorationAnimation(decoration, key).beginTime toLayer:nil];
}

@interface CandidatePanelTestDelegate : NSObject <MetasequoiaCandidatePanelDelegate>
@property(nonatomic, strong) NSAttributedString *selection;
@property(nonatomic) NSUInteger nextPages;
@end
@implementation CandidatePanelTestDelegate
- (void)candidateSelected:(NSAttributedString *)candidate
{
    self.selection = candidate;
}
- (void)candidatePanelNextPage
{
    ++self.nextPages;
}
- (void)candidatePanelPreviousPage
{
}
@end

int main()
{
    @autoreleasepool
    {
        [NSApplication sharedApplication];
        char temporary[] = "/tmp/msime-candidate-panel-XXXXXX";
        Require(mkdtemp(temporary) != nullptr, "Failed to create candidate panel fixture directory.");
        const std::filesystem::path fixtureRoot = std::filesystem::path(temporary) / "Library" / "Application Support" /
                                                   "app.msime.macos" / "skins" / "wide-card";
        std::filesystem::create_directories(fixtureRoot);
        std::ofstream manifest(fixtureRoot / "skin.toml");
        manifest << R"toml(schema_version = 2
id = "wide-card"
name = "Wide Card"
version = "1"
base = "system"
[supports]
layouts = ["vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 240
[candidate_window.decoration]
top_inset_dip = 0
width_dip = 0
[animation]
enter = "fade_scale"
exit = "fade"
selection = "spring"
duration_ms = 180
stagger_ms = 14
)toml";
        Require(static_cast<bool>(manifest), "Failed to write candidate panel skin fixture.");
        manifest.close();
        // 同样是 v2，但没写 [animation]：共享解析器补上的活泼默认组合在 macOS 上不播放（#7760）。
        const std::filesystem::path implicitRoot = fixtureRoot.parent_path() / "implicit-card";
        std::filesystem::create_directories(implicitRoot);
        std::ofstream implicitManifest(implicitRoot / "skin.toml");
        implicitManifest << R"toml(schema_version = 2
id = "implicit-card"
name = "Implicit Card"
version = "1"
base = "system"
[supports]
layouts = ["vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 240
)toml";
        Require(static_cast<bool>(implicitManifest), "Failed to write the implicit-motion skin fixture.");
        implicitManifest.close();
        const char *oldHome = std::getenv("HOME");
        const std::string savedHome = oldHome == nullptr ? std::string() : std::string(oldHome);
        Require(setenv("HOME", temporary, 1) == 0, "Failed to isolate candidate panel skin root.");
        MetasequoiaSetStoredGlobalTheme(@"system");
        MetasequoiaCandidatePanel *panel = [MetasequoiaCandidatePanel new];
        CandidatePanelTestDelegate *delegate = [CandidatePanelTestDelegate new];
        panel.delegate = delegate;
        NSMutableArray *candidates = [NSMutableArray array];
        for (NSUInteger index = 0; index < 9; ++index)
            [candidates addObject:[[NSAttributedString alloc]
                                      initWithString:[NSString stringWithFormat:@"候选%lu", (unsigned long)index]]];
        panel.panelType = kIMKSingleColumnScrollingCandidatePanel;
        [panel setCandidateData:candidates];
        const CGFloat nineHeight = panel.candidateFrame.size.height;
        [panel setCandidateData:[candidates subarrayWithRange:NSMakeRange(0, 7)]];
        const CGFloat sevenHeight = panel.candidateFrame.size.height;
        [panel setCandidateData:[candidates subarrayWithRange:NSMakeRange(0, 5)]];
        const CGFloat fiveHeight = panel.candidateFrame.size.height;
        Require(fiveHeight < sevenHeight && sevenHeight < nineHeight,
                "Vertical window did not shrink with candidate count.");
        Require(fiveHeight < nineHeight * 0.75, "Five-candidate window retained substantial empty row space.");
        [panel setCandidateData:@[ candidates[0] ]];
        Require(panel.candidateFrame.size.height < fiveHeight * 0.5, "A partial page retained the full page height.");
        panel.panelType = kIMKSingleRowSteppingCandidatePanel;
        [panel setCandidateData:candidates];
        const CGFloat nineWidth = panel.candidateFrame.size.width;
        [panel setCandidateData:[candidates subarrayWithRange:NSMakeRange(0, 5)]];
        Require(panel.candidateFrame.size.width < nineWidth, "Horizontal window did not shrink with candidate count.");
        const CGFloat smallHeight = panel.candidateFrame.size.height;
        [panel setAttributes:@{NSFontAttributeName : [NSFont systemFontOfSize:20]}];
        Require(panel.candidateFrame.size.height > smallHeight, "Candidate font size did not relayout the window.");
        Require(!panel.window.canBecomeKeyWindow && !panel.window.canBecomeMainWindow,
                "The candidate window can steal input focus.");
        Require([panel selectCandidateWithIdentifier:3] && panel.selectedCandidate == 3 &&
                    [panel.selectedCandidateString isEqual:candidates[3]],
                "Selection and displayed highlight disagree.");
        NSButton *hoverButton = nil;
        for (NSView *view in panel.window.contentView.subviews)
            if ([view isKindOfClass:NSButton.class] && view.tag == 0)
                hoverButton = (NSButton *)view;
        Require(hoverButton != nil && [[hoverButton valueForKey:@"hoverColor"] alphaComponent] > 0.01,
                "Candidate rows did not receive the skin hover color.");
        NSEvent *hoverMove = [NSEvent mouseEventWithType:NSEventTypeMouseMoved
            location:[hoverButton convertPoint:NSMakePoint(10, 10) toView:nil] modifierFlags:0
            timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:panel.window.windowNumber
            context:nil eventNumber:0 clickCount:0 pressure:0];
        [(id)hoverButton performSelector:@selector(mouseEntered:) withObject:[NSObject new]];
        Require(![[hoverButton valueForKey:@"candidateHovered"] boolValue], "进入消息不能激活静止指针的悬停。");
        [hoverButton mouseMoved:hoverMove];
        Require([[hoverButton valueForKey:@"candidateHovered"] boolValue], "移动指针后应显示候选悬停。");
        [hoverButton updateTrackingAreas];
        Require(![[hoverButton valueForKey:@"candidateHovered"] boolValue], "重建跟踪区域应清除旧悬停。");
        [hoverButton mouseMoved:hoverMove];
        Require([[hoverButton valueForKey:@"candidateHovered"] boolValue], "重建后移动指针应恢复悬停。");
        [(id)hoverButton performSelector:@selector(mouseExited:) withObject:[NSObject new]];
        Require(![[hoverButton valueForKey:@"candidateHovered"] boolValue], "Candidate hover state did not clear.");
        NSColor *selectedHover = [hoverButton valueForKey:@"selectedHoverColor"];
        Require(selectedHover != nil && selectedHover.alphaComponent > 0.01,
                "Selected candidate rows did not receive a separate hover color.");
        Require(![panel selectCandidateWithIdentifier:8], "A nonvisible candidate could be selected.");
        NSButton *candidateButton = nil;
        for (NSView *view in panel.window.contentView.subviews)
            if ([view isKindOfClass:NSButton.class] && view.tag == 3)
                candidateButton = (NSButton *)view;
        Require(candidateButton != nil, "No clickable candidate was rendered.");
        [candidateButton performClick:nil];
        Require([delegate.selection isEqual:candidates[3]], "Click did not return the original attributed candidate.");
        panel.hasNextPage = YES;
        for (NSView *view in panel.window.contentView.subviews)
            if ([view isKindOfClass:NSButton.class] && view.tag == -2)
                [(NSButton *)view performClick:nil];
        Require(delegate.nextPages == 1, "The visible next-page button did not route to the controller.");
        NSMutableString *longText = [NSMutableString string];
        for (NSUInteger index = 0; index < 200; ++index)
            [longText appendString:@"长候选"];
        NSAttributedString *longCandidate = [[NSAttributedString alloc] initWithString:longText];
        [panel setCandidateData:@[ longCandidate, longCandidate, longCandidate, longCandidate, longCandidate ]];
        Require(panel.candidateFrame.size.width <= NSScreen.mainScreen.visibleFrame.size.width,
                "Long candidates pushed the window beyond the screen width.");
        panel.panelType = kIMKSingleRowSteppingCandidatePanel;
        NSAttributedString *annotated = [[NSAttributedString alloc] initWithString:@"水杉(Ss)"];
        [panel setCandidateData:@[ annotated, annotated, annotated ]];
        NSButton *annotatedButton = nil;
        for (NSView *view in panel.window.contentView.subviews)
            if ([view isKindOfClass:NSButton.class] && view.tag == 0)
                annotatedButton = (NSButton *)view;
        Require(annotatedButton != nil, "The annotated candidate was not rendered.");
        NSFont *numberFont = [annotatedButton valueForKey:@"numberFont"];
        Require(numberFont != nil && fabs(numberFont.pointSize - annotatedButton.font.pointSize * MSIMECandidateNumberScale) < 0.01,
                "Candidate number font did not use the Windows 80% scale.");
        NSDictionary *measure = @{NSFontAttributeName : annotatedButton.font};
        NSDictionary *numberMeasure = @{NSFontAttributeName : numberFont};
        // The 6pt accent-bar gutter exists only for a palette that draws the bar; the native system theme does not.
        const CGFloat barGutter = MetasequoiaResolveStoredTheme(NO, NO).tokens.showSelectedBar ? 6.0 : 0.0;
        const CGFloat needed = 8.0 + barGutter + [@"1" sizeWithAttributes:numberMeasure].width + MSIMECandidateNumberGap +
                               [@"水杉(Ss)" sizeWithAttributes:measure].width + 8.0;
        Require(annotatedButton.frame.size.width + 0.5 >= needed, "Horizontal layout truncated helpcode annotations.");
        [panel setCandidateData:[candidates subarrayWithRange:NSMakeRange(0, 5)]];
        for (NSScreen *screen in NSScreen.screens)
        {
            NSRect bounds = screen.visibleFrame;
            panel.caretRect = NSMakeRect(NSMaxX(bounds) - 2, NSMinY(bounds) + 2, 0, 20);
            [panel show:kIMKLocateCandidatesBelowHint];
            Require(NSContainsRect(bounds, panel.candidateFrame),
                    "A zero-width edge caret positioned the panel outside its screen.");
            [panel hide];
        }
        NSRect bounds = NSScreen.mainScreen.visibleFrame;
        panel.panelType = kIMKSingleColumnScrollingCandidatePanel;
        panel.caretRect = NSMakeRect(NSMidX(bounds), NSMinY(bounds) + 100, 0, 20);
        [panel setCandidateData:candidates];
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(NSMinY(panel.candidateFrame) > NSMaxY(panel.caretRect),
                "A tall vertical page did not flip above a low caret.");
        [panel setCandidateData:@[ candidates[0] ]];
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(NSMinY(panel.candidateFrame) > NSMaxY(panel.caretRect),
                "A shorter page changed sides after a tall page had flipped.");
        [panel hide];
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(NSMaxY(panel.candidateFrame) < NSMinY(panel.caretRect),
                "Hiding the candidate panel did not reset placement memory.");
        panel.caretRect = NSZeroRect;
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(!panel.isVisible, "An invalid caret displayed a misplaced candidate window.");
        [panel setCandidateData:@[]];
        Require(!panel.isVisible && panel.selectedCandidate == NSNotFound, "Empty data retained a visible selection.");
        // A package is drawn as the custom theme's candidate skin.
        [NSUserDefaults.standardUserDefaults setObject:@"wide-card" forKey:@"MSIMEClientCustomCandidateSkin"];
        MetasequoiaSetStoredGlobalTheme(@"custom");
        panel.panelType = kIMKSingleColumnScrollingCandidatePanel;
        [panel setCandidateData:@[ [[NSAttributedString alloc] initWithString:@"短"] ,
                                   [[NSAttributedString alloc] initWithString:@"窄"] ]];
        NSButton *wideButton = nil;
        for (NSView *view in panel.window.contentView.subviews)
            if ([view isKindOfClass:NSButton.class] && view.tag == 0)
                wideButton = (NSButton *)view;
        Require(wideButton != nil, "The wide-card candidate was not rendered.");
        const CGFloat cardWidth = panel.window.contentView.bounds.size.width;
        const CGFloat contentInset = MetasequoiaResolveStoredTheme(NO, YES).tokens.pad;
        Require(cardWidth >= 240.0 && wideButton.frame.size.width >= cardWidth - 2.0 * contentInset - 0.5,
                "Vertical candidate highlighting did not fill the final card width.");
        Require(NSMaxX(wideButton.frame) >= cardWidth - contentInset - 0.5,
                "Vertical candidate row did not reach the card's content edge.");
        panel.caretRect = NSMakeRect(300, 300, 1, 20);
        const auto declared = MetasequoiaResolveStoredTheme(NO, YES).animation;
        Require(declared.enter == metasequoia::mac::SkinAnimationPreset::fade_scale &&
                    declared.exit == metasequoia::mac::SkinAnimationPreset::fade &&
                    declared.selection == metasequoia::mac::SkinAnimationPreset::spring && declared.durationMs == 180.0,
                "清单显式写出的动效没有传到 macOS。");
        MSIMECandidateMotion *motion = [panel valueForKey:@"motion"];
        // 不依赖这台机器的系统设置。
        motion.reduceMotion = NO;
        NSPanel *window = panel.window;
        // 显式声明的动效也不执行，候选与高亮直接呈现最终状态。
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(panel.isVisible && window.alphaValue == 1.0 && window.animationBehavior == NSWindowAnimationBehaviorNone,
                "候选窗应立即以不透明状态显示。");
        Require(!CardEntering(panel) && window.hasShadow, "显式皮肤不应播放进场或延迟显示阴影。");
        Require([CandidateButton(panel, 1).layer animationForKey:@"msime-candidate-row-reveal"] == nil,
                "候选行不应逐行浮现。");
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(!CardEntering(panel), "重建候选时不应播放过渡。");
        Require([panel selectCandidateWithIdentifier:1] && panel.selectedCandidate == 1, "高亮应立即移动。");
        NSButton *selectedWideButton = CandidateButton(panel, 1);
        Require([[selectedWideButton valueForKey:@"candidateHighlighted"] boolValue]
                    && ![[CandidateButton(panel, 0) valueForKey:@"candidateHighlighted"] boolValue], "高亮状态未更新。");
        Require(motion.selectionLayer == nil && ![[selectedWideButton valueForKey:@"selectionFillDetached"] boolValue],
                "显式选中预设不应创建动画图层或分离静态高亮。");
        [panel hide];
        Require(!panel.isVisible && !window.isVisible && !motion.exiting && !motion.exitWindow.isVisible,
                "收起候选不应保留退场窗口。");
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(panel.isVisible && !CardEntering(panel), "再次显示候选应立即完成。");
        RequireInstant(panel, motion, "显式声明动效的皮肤也应即时显隐。");
        motion.reduceMotion = YES;
        RequireInstant(panel, motion, "减弱动态效果时也应即时显隐。");
        motion.reduceMotion = NO;
        Require(MetasequoiaResolveStoredTheme(NO, YES).candidateSkin == "wide-card" &&
                    MetasequoiaResolveStoredTheme(NO, NO).candidateSkin.empty(),
                "A vertical-only package was not drawn in exactly the vertical layout.");
        [panel hide];
        // v2 包省略 [animation] 时得到的是解析器补上的默认组合，不是皮肤要求的动效。
        [NSUserDefaults.standardUserDefaults setObject:@"implicit-card" forKey:@"MSIMEClientCustomCandidateSkin"];
        MetasequoiaSetStoredGlobalTheme(@"custom");
        Require(MetasequoiaResolveStoredTheme(NO, YES).candidateSkin == "implicit-card", "The implicit-motion package was not drawn.");
        RequireInstant(panel, motion, "没写 [animation] 的 v2 皮肤在 macOS 上播放了默认动效。");
        [NSUserDefaults.standardUserDefaults removeObjectForKey:@"MSIMEClientCustomCandidateSkin"];
        MetasequoiaSetStoredGlobalTheme(@"system");
        RequireInstant(panel, motion, "默认主题的候选窗应即时显隐。");

        // 装饰图的待机动效和 APNG 帧动画：候选窗在屏幕上时才动，收起、减弱动态效果和屏幕睡眠时停下，重排时相位接着走。
        const std::filesystem::path whaleRoot = fixtureRoot.parent_path() / "whale-card";
        std::filesystem::create_directories(whaleRoot);
        WriteApng(whaleRoot / "whale.png", 64, 48, 4, 0.1, 0);
        {
            std::ofstream whale(whaleRoot / "skin.toml");
            whale << R"toml(schema_version = 2
id = "whale-card"
name = "Whale Card"
version = "1"
base = "system"
[supports]
layouts = ["vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 200
[candidate_window.decoration]
image = "whale.png"
top_inset_dip = 40
width_dip = 60
motion = "float"
motion_period_ms = 2000
)toml";
            Require(static_cast<bool>(whale), "Failed to write the decoration motion fixture.");
        }
        [NSUserDefaults.standardUserDefaults setObject:@"whale-card" forKey:@"MSIMEClientCustomCandidateSkin"];
        MetasequoiaSetStoredGlobalTheme(@"custom");
        const auto whaleSkin = MetasequoiaResolveStoredTheme(NO, YES);
        Require(whaleSkin.candidateSkin == "whale-card" && whaleSkin.decorationMotion == metasequoia::mac::DecorationMotion::floating &&
                    whaleSkin.decorationMotionPeriodMs == 2000.0 && !whaleSkin.decorationPath.empty(),
                "清单的 decoration.motion 没有传到 macOS。");
        motion.reduceMotion = NO;
        [panel hide];
        Spin(0.5);
        Require(motion.animatedDecoration == nil, "候选窗不在屏幕上时装饰图不应该动。");
        [panel setCandidateData:[candidates subarrayWithRange:NSMakeRange(0, 3)]];
        [panel show:kIMKLocateCandidatesBelowHint];
        MSIMECandidateDecorationView *decoration = motion.animatedDecoration;
        Require(decoration != nil && decoration.window == panel.window && decoration.frames.images.count == 4,
                "候选窗显示时装饰图没有动起来。");
        CAKeyframeAnimation *idle = DecorationAnimation(decoration, @"msime-decoration-motion");
        CAKeyframeAnimation *playback = DecorationAnimation(decoration, @"msime-decoration-frames");
        Require(idle != nil && idle.repeatCount == HUGE_VALF && std::fabs(idle.duration - 2.0) < 1e-6,
                "float 待机动效应按清单的周期无限循环。");
        Require(playback != nil && playback.values.count == 4 && [playback.calculationMode isEqual:kCAAnimationDiscrete] &&
                    std::fabs(playback.duration - 0.4) < 0.01 && playback.repeatCount == HUGE_VALF,
                "APNG 装饰图应按文件的帧时长逐帧无限播放。");
        // 浮动的最高点不越过窗口顶边：装饰图顶边离窗口顶边留着余量，浮动幅度不超过它。
        const CGFloat windowTop = NSHeight(panel.window.contentView.bounds);
        Require(NSMaxY(decoration.frame) <= windowTop - metasequoia::mac::kDecorationMotionMargin + 0.5,
                "有待机动效的装饰图没有给动效留出余量，会被窗口顶边裁掉。");
        CGFloat highest = 0.0;
        for (NSValue *value in idle.values)
        {
            const CATransform3D transform = value.CATransform3DValue;
            highest = MAX(highest, transform.m42);
        }
        Require(highest > 1.0 && highest <= metasequoia::mac::kDecorationMotionMargin + 0.01, "float 的幅度应在余量之内、又看得出来。");
        const CFTimeInterval anchor = motion.decorationAnchor;
        Require(std::fabs(DecorationBegin(decoration, @"msime-decoration-motion") - anchor) < 1e-3 &&
                    std::fabs(DecorationBegin(decoration, @"msime-decoration-frames") - anchor) < 1e-3,
                "装饰图的两种动画应从同一时刻起算。");
        // 面板还在屏幕上时重排：动画重新挂上，但仍从同一时刻起算，打字时装饰图不会跳回原位。
        Spin(0.3);
        [panel setCandidateData:[candidates subarrayWithRange:NSMakeRange(0, 4)]];
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(motion.animatedDecoration == decoration && motion.decorationAnchor == anchor &&
                    std::fabs(DecorationBegin(decoration, @"msime-decoration-motion") - anchor) < 1e-3,
                "重排后装饰图的待机动效从头开始了。");
        // 收起时立即停下；同一段连续输入里再出现，接着原来的相位。
        [panel hide];
        Require(motion.animatedDecoration == nil && DecorationAnimation(decoration, @"msime-decoration-motion") == nil &&
                    DecorationAnimation(decoration, @"msime-decoration-frames") == nil,
                "候选窗收起后装饰图的动画还挂着。");
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(motion.animatedDecoration != nil && motion.decorationAnchor == anchor, "连续输入中重新出现时装饰图的相位不该重来。");
        // 停下来之后再出现：从原位重新开始。
        [panel hide];
        Spin(0.5);
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(motion.animatedDecoration != nil && motion.decorationAnchor > anchor, "停顿后重新出现时装饰图应从原位开始。");
        // 减弱动态效果：不动，APNG 停在第一帧。
        decoration = motion.animatedDecoration;
        motion.reduceMotion = YES;
        Require(motion.animatedDecoration == nil && decoration.layer.contents == decoration.frames.images.firstObject,
                "减弱动态效果时装饰图仍在动，或者没有停在第一帧。");
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(motion.animatedDecoration == nil, "减弱动态效果时重排又让装饰图动了起来。");
        motion.reduceMotion = NO;
        Require(motion.animatedDecoration == decoration, "关掉减弱动态效果后装饰图没有接着动。");
        // 屏幕睡眠时停下，醒来接着动。
        [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceScreensDidSleepNotification
                                                                      object:NSWorkspace.sharedWorkspace];
        Require(motion.animatedDecoration == nil, "屏幕睡眠时装饰图还在动。");
        [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:NSWorkspaceScreensDidWakeNotification
                                                                      object:NSWorkspace.sharedWorkspace];
        Require(motion.animatedDecoration == decoration, "屏幕醒来后装饰图没有接着动。");
        [panel hide];
        // 帧数超过上限的包由共享加载器拒绝，原因说清楚是帧数。
        const std::filesystem::path framesRoot = fixtureRoot.parent_path() / "too-many-frames";
        std::filesystem::create_directories(framesRoot);
        WriteApng(framesRoot / "whale.png", 2, 2, 121, 0.05, 0);
        {
            std::ofstream frames(framesRoot / "skin.toml");
            frames << R"toml(schema_version = 2
id = "too-many-frames"
name = "Too Many Frames"
version = "1"
base = "system"
[supports]
layouts = ["vertical"]
themes = ["light"]
[candidate_window]
[candidate_window.decoration]
image = "whale.png"
top_inset_dip = 40
width_dip = 60
)toml";
        }
        std::string framesError;
        Require(!metasequoia::mac::LoadSkinPackage(fixtureRoot.parent_path(), "too-many-frames", &framesError) &&
                    framesError.find("120") != std::string::npos,
                "帧数超过上限的 APNG 包没有被共享加载器以帧数为由拒绝。");
        [NSUserDefaults.standardUserDefaults removeObjectForKey:@"MSIMEClientCustomCandidateSkin"];
        MetasequoiaSetStoredGlobalTheme(@"system");
        if (savedHome.empty()) unsetenv("HOME");
        else setenv("HOME", savedHome.c_str(), 1);
        std::filesystem::remove_all(temporary);
    }
}
