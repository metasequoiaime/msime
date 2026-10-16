#import "../../src/candidate/CandidatePanel.h"
#import "../../src/candidate/CandidateMotion.h"
#import "../../src/candidate/CandidateSkinAppearance.h"
#import "../../src/candidate/CandidateTypography.h"
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

// 皮肤没写的动效一律不播：显示不透明、无进场，换选中无装饰，收起没有退场窗。
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
        [(id)hoverButton performSelector:@selector(mouseEntered:) withObject:[NSObject new]];
        Require([[hoverButton valueForKey:@"candidateHovered"] boolValue], "Candidate hover state did not activate.");
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
        // 距上次收起已经很久：候选窗重新出现时播放皮肤写了的进场，窗口本身照样立即以完整透明度显示。
        Spin(0.5);
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(panel.isVisible && window.alphaValue == 1.0 && window.animationBehavior == NSWindowAnimationBehaviorNone,
                "候选窗应立即以不透明状态显示。");
        Require(CardEntering(panel) &&
                    [window.contentView.layer animationForKey:@"msime-candidate-enter-transform"] != nil &&
                    window.contentView.layer.opacity == 1.0f,
                "写了 fade_scale 进场的皮肤没有在卡片图层上播放进场。");
        Require([CandidateButton(panel, 1).layer animationForKey:@"msime-candidate-row-reveal"] != nil,
                "fade_scale 进场没有逐行错峰。");
        // 进场途中再排一次：CandidatePanel 在同一张卡片里重建候选行，新行接着同一条时间线错峰，与 InputController 换新内容视图时一样。
        NSButton *revealedBefore = CandidateButton(panel, 1);
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(CandidateButton(panel, 1) != revealedBefore && CardEntering(panel) &&
                    [CandidateButton(panel, 1).layer animationForKey:@"msime-candidate-row-reveal"] != nil,
                "进场途中重建的候选行没有接着错峰。");
        Require(!window.hasShadow, "进场期间窗口阴影应先关掉。");
        Spin(0.35);
        Require(!CardEntering(panel) && window.hasShadow, "进场结束后没有撤掉动画、还原阴影。");
        // 选中：高亮和选中状态同步移动，动效只装饰新高亮行的底色。
        Require([panel selectCandidateWithIdentifier:1] && panel.selectedCandidate == 1, "Selection did not move.");
        NSButton *selectedWideButton = CandidateButton(panel, 1);
        Require(selectedWideButton != nil, "The second wide-card candidate was not rendered.");
        Require([[selectedWideButton valueForKey:@"candidateHighlighted"] boolValue] &&
                    ![[CandidateButton(panel, 0) valueForKey:@"candidateHighlighted"] boolValue],
                "选中动效不应推迟高亮。");
        Require(motion.selectionLayer != nil && motion.selectionLayer.superlayer == selectedWideButton.layer.superlayer &&
                    [motion.selectionLayer animationForKey:@"msime-candidate-selection"] != nil &&
                    [[selectedWideButton valueForKey:@"selectionFillDetached"] boolValue],
                "写了 spring 选中的皮肤没有装饰新高亮行的底色。");
        Require([selectedWideButton.layer animationForKey:@"msime-candidate-selection"] == nil,
                "选中动效不应缩放或淡化行里的文字。");
        Spin(0.3);
        Require(motion.selectionLayer == nil && ![[selectedWideButton valueForKey:@"selectionFillDetached"] boolValue],
                "选中装饰结束后行没有重新自己画底色。");
        // 退场：面板立即不可见，旧卡片在不接收鼠标的退场窗里淡出。
        [panel hide];
        Require(!panel.isVisible && !window.isVisible, "隐藏候选窗不应等待退出动画。");
        Require(motion.exiting && motion.exitWindow.isVisible && motion.exitWindow.ignoresMouseEvents &&
                    [motion.exitWindow.contentView.layer animationForKey:@"msime-candidate-exit-opacity"] != nil &&
                    window.contentView != motion.exitWindow.contentView,
                "写了 fade 退场的皮肤没有播放退场。");
        // 退场中重新显示：立即撤掉退场，以完整不透明度显示，不播进场；迟到的完成回调不能收起它。
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(panel.isVisible && window.alphaValue == 1.0 && !motion.exiting && !motion.exitWindow.isVisible && !CardEntering(panel),
                "退场中重新显示没有立即以不透明状态出现。");
        Spin(0.4);
        Require(panel.isVisible && window.isVisible, "迟到的退场完成回调收起了重新显示的候选窗。");
        // 没有被打断的退场在动画结束后才撤掉退场窗。
        [panel hide];
        Require(!panel.isVisible && motion.exiting, "退场没有开始。");
        Spin(0.25);
        Require(!motion.exiting && !motion.exitWindow.isVisible, "退场结束后退场窗没有收起。");
        // 同一段连续输入（收起后不到 400ms 又出现）不播进场。
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(panel.isVisible && !CardEntering(panel), "连续输入中重新出现的候选窗不应播放进场。");
        // 进场途中打开「减弱动态效果」：动画立即撤掉，之后什么都不播。
        [panel hide];
        Spin(0.5);
        [panel show:kIMKLocateCandidatesBelowHint];
        Require(CardEntering(panel), "长时间收起后重新出现应播放进场。");
        motion.reduceMotion = YES;
        Require(!CardEntering(panel) && window.hasShadow, "打开减弱动态效果后进场没有立即结束。");
        RequireInstant(panel, motion, "减弱动态效果时仍然播放了皮肤动画。");
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
        if (savedHome.empty()) unsetenv("HOME");
        else setenv("HOME", savedHome.c_str(), 1);
        std::filesystem::remove_all(temporary);
    }
}
