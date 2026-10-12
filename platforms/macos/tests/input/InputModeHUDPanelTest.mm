#import "../../src/input/InputModeHUDPanel.h"
#import "../../src/candidate/CandidateWindowLevel.h"
#import <AppKit/AppKit.h>
#include <cmath>
#include <cstdio>
#include <initializer_list>
#include <stdexcept>

namespace {
void Require(bool condition, const char *message) { if (!condition) throw std::runtime_error(message); }
}

int main() {
    @autoreleasepool {
        try {
            Require([MSIMEInputModeHUDText(YES) isEqualToString:@"英"] && [MSIMEInputModeHUDText(NO) isEqualToString:@"中"], "HUD text mismatch");
            const NSRect screen = NSMakeRect(0, 0, 1440, 900);
            const NSSize panel = NSMakeSize(100, 56);
            const NSRect caret = NSMakeRect(700, 500, 2, 20);
            const NSRect under = MSIMEInputModeHUDFrame(caret, panel, screen);
            Require(NSMaxY(under) < NSMinY(caret) && std::abs(NSMidX(under) - NSMidX(caret)) < 0.5, "HUD did not stay below caret");
            const NSRect low = MSIMEInputModeHUDFrame(NSMakeRect(700, 12, 2, 20), panel, screen);
            Require(NSMinY(low) > NSMaxY(NSMakeRect(700, 12, 2, 20)), "HUD did not move above low caret");
            for (const NSRect edge : {NSMakeRect(-40, 500, 2, 20), NSMakeRect(1480, 500, 2, 20), NSMakeRect(700, 1200, 2, 20)}) {
                NSRect frame = MSIMEInputModeHUDFrame(edge, panel, screen);
                Require(NSMinX(frame) >= NSMinX(screen) && NSMaxX(frame) <= NSMaxX(screen) && NSMinY(frame) >= NSMinY(screen) && NSMaxY(frame) <= NSMaxY(screen), "HUD escaped screen");
            }
            Require(!MSIMEInputModeHUDUsableCaretRect(NSZeroRect) && !MSIMEInputModeHUDUsableCaretRect(NSMakeRect(0, 0, 1, 0)) && MSIMEInputModeHUDUsableCaretRect(caret), "caret validation mismatch");
            MSIMEInputModeHUDPanel *hud = MSIMEInputModeHUDPanel.sharedPanel;
            Require(hud == MSIMEInputModeHUDPanel.sharedPanel && hud.ignoresMouseEvents && hud.floatingPanel && !hud.opaque, "HUD panel contract mismatch");
            // 和候选窗同层级，截图工具的遮罩盖不住它（#7326）。
            Require(hud.level == MSIMECandidateWindowLevel() && hud.level > NSScreenSaverWindowLevel, "HUD is not at the candidate window level");
            Require(hud.displayedText == nil, "HUD was visible before use");
            // The badge is the floating toolbar's height, (font + 20) x scale: 44pt at the toolbar's default 24pt and 100%.
            Require(hud.panelSize.height == 44.0 && hud.panelSize.width < 100.0, "HUD default size is not the toolbar's");
            const CGFloat defaultWidth = hud.panelSize.width;
            [hud applySizingPreferences:@{@"floating_toolbar" : @{@"font_size" : @16, @"scale_percent" : @150}}];
            Require(hud.panelSize.height == 54.0 && hud.panelSize.width > defaultWidth, "HUD did not follow the toolbar's font size and scale");
            [hud applySizingPreferences:@{@"floating_toolbar" : @{@"font_size" : @99, @"scale_percent" : @7}}];
            Require(hud.panelSize.height == 44.0 && hud.panelSize.width == defaultWidth, "HUD did not fall back to the toolbar's defaults");
            // Until the controller hands it the theme's palette the badge is the system window surface, not a fixed brand colour.
            Require([hud.surfaceColor isEqual:NSColor.windowBackgroundColor] && [hud.textColor isEqual:NSColor.labelColor], "HUD default colours are not the system ones");
            NSColor *surface = [NSColor colorWithSRGBRed:0xF4 / 255.0 green:0xF8 / 255.0 blue:1.0 alpha:1.0];
            NSColor *border = [NSColor colorWithSRGBRed:0.16 green:0.21 blue:0.44 alpha:0.24];
            NSColor *text = [NSColor colorWithSRGBRed:0x1C / 255.0 green:0x25 / 255.0 blue:0x50 / 255.0 alpha:1.0];
            [hud setSurfaceColor:surface borderColor:border textColor:text];
            Require([hud.surfaceColor isEqual:surface] && [hud.borderColor isEqual:border] && [hud.textColor isEqual:text], "HUD did not keep the theme colours");
            Require(CGColorEqualToColor(hud.contentView.layer.backgroundColor, surface.CGColor) && hud.contentView.layer.borderWidth > 0, "HUD surface or outline not drawn in the theme colours");
            // The badge is drawn in the toolbar's mode: toolbar_theme when it names one, then theme, then the system.
            [hud applyThemePreferences:@{@"theme" : @"light", @"toolbar_theme" : @"dark"}];
            Require([hud.appearance.name isEqualToString:NSAppearanceNameDarkAqua], "HUD ignored the toolbar's own mode");
            [hud applyThemePreferences:@{@"theme" : @"dark", @"toolbar_theme" : @"follow"}];
            Require([hud.appearance.name isEqualToString:NSAppearanceNameDarkAqua], "HUD ignored the global mode");
            [hud applyThemePreferences:@{@"theme" : @"light"}];
            Require([hud.appearance.name isEqualToString:NSAppearanceNameAqua], "HUD ignored the light mode");
            NSColor *darkSurface = [NSColor colorWithSRGBRed:0x14 / 255.0 green:0x1B / 255.0 blue:0x33 / 255.0 alpha:1.0];
            NSColor *dynamicSurface = [NSColor colorWithName:nil dynamicProvider:^NSColor *(NSAppearance *appearance) {
                return [appearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]] == NSAppearanceNameDarkAqua ? darkSurface : surface;
            }];
            [hud setSurfaceColor:dynamicSurface borderColor:border textColor:text];
            [hud applyThemePreferences:@{@"theme" : @"system", @"toolbar_theme" : @"dark"}];
            Require(CGColorEqualToColor(hud.contentView.layer.backgroundColor, darkSurface.CGColor), "HUD did not draw the dark palette in the toolbar's dark mode");
            [hud applyThemePreferences:@{@"theme" : @"system"}];
            Require(hud.appearance == nil, "HUD did not return to the system appearance");
            [hud setSurfaceColor:surface borderColor:border textColor:text];
            [hud showEnglishInputMode:YES nearCaretRect:caret];
            Require([hud.displayedText isEqualToString:@"英"], "English HUD missing");
            [hud showEnglishInputMode:NO nearCaretRect:caret];
            Require([hud.displayedText isEqualToString:@"中"], "Chinese HUD missing");
            [hud orderOut:nil];
        } catch (const std::exception &error) { std::fprintf(stderr, "%s\n", error.what()); return 1; }
    }
    return 0;
}
