#import "../../src/candidate/CandidatePanel.h"
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
        manifest << R"toml(schema_version = 1
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
)toml";
        Require(static_cast<bool>(manifest), "Failed to write candidate panel skin fixture.");
        manifest.close();
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
        Require(MetasequoiaResolveStoredTheme(NO, YES).candidateSkin == "wide-card" &&
                    MetasequoiaResolveStoredTheme(NO, NO).candidateSkin.empty(),
                "A vertical-only package was not drawn in exactly the vertical layout.");
        [panel hide];
        [NSUserDefaults.standardUserDefaults removeObjectForKey:@"MSIMEClientCustomCandidateSkin"];
        MetasequoiaSetStoredGlobalTheme(@"system");
        if (savedHome.empty()) unsetenv("HOME");
        else setenv("HOME", savedHome.c_str(), 1);
        std::filesystem::remove_all(temporary);
    }
}
