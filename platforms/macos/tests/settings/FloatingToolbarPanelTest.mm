#import "../../src/core/FloatingToolbarPanel.h"
#import "../../src/candidate/CandidateSkinAppearance.h"

#include <cassert>
#include <cmath>

@interface FloatingToolbarTestDelegate : NSObject <MSIMEFloatingToolbarDelegate>
@property(nonatomic) NSUInteger inputModeToggles;
@property(nonatomic) NSUInteger punctuationToggles;
@property(nonatomic) NSUInteger fullWidthToggles;
@property(nonatomic) NSUInteger traditionalToggles;
@property(nonatomic) NSUInteger characterPaletteRequests;
@property(nonatomic) NSUInteger emojiRequests;
@property(nonatomic) NSUInteger handwritingRequests;
@property(nonatomic) NSUInteger keyboardRequests;
@property(nonatomic) NSUInteger voiceToggles;
@property(nonatomic) NSUInteger settingsRequests;
@property(nonatomic) NSUInteger updateRequests;
@property(nonatomic) NSUInteger websiteRequests;
@property(nonatomic) NSUInteger hideRequests;
@end

@implementation FloatingToolbarTestDelegate
- (void)floatingToolbarDidRequestToggleInputMode:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_inputModeToggles; }
- (void)floatingToolbarDidRequestTogglePunctuation:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_punctuationToggles; }
- (void)floatingToolbarDidRequestToggleFullWidth:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_fullWidthToggles; }
- (void)floatingToolbarDidRequestToggleTraditionalOutput:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_traditionalToggles; }
- (void)floatingToolbarDidRequestOpenCharacterPalette:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_characterPaletteRequests; }
- (void)floatingToolbarDidRequestOpenEmoji:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_emojiRequests; }
- (void)floatingToolbarDidRequestOpenHandwriting:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_handwritingRequests; }
- (void)floatingToolbarDidRequestOpenScreenKeyboard:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_keyboardRequests; }
- (void)floatingToolbarDidRequestToggleVoice:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_voiceToggles; }
- (void)floatingToolbarDidRequestOpenSettings:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_settingsRequests; }
- (void)floatingToolbarDidRequestCheckForUpdates:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_updateRequests; }
- (void)floatingToolbarDidRequestOpenWebsite:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_websiteRequests; }
- (void)floatingToolbarDidRequestHide:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; ++_hideRequests; }
@end

static NSButton *FindButton(NSView *view, NSString *identifier) {
    if ([view isKindOfClass:NSButton.class] && [view.accessibilityIdentifier isEqualToString:identifier]) {
        return (NSButton *)view;
    }
    for (NSView *subview in view.subviews) {
        NSButton *button = FindButton(subview, identifier);
        if (button) return button;
    }
    return nil;
}

static NSView *FindView(NSView *view, NSString *identifier) {
    if ([view.accessibilityIdentifier isEqualToString:identifier]) return view;
    for (NSView *subview in view.subviews) {
        NSView *found = FindView(subview, identifier);
        if (found) return found;
    }
    return nil;
}

// Mirrors the panel's width: buttons and their gaps, the 20pt trailing run, and the 67.2pt leading run of logo (32), gap (6), grip (18), gap (2), divider (1.2) and gap (8).
static double ExpectedWidth(double count, double fontSize, double factor) {
    const double gaps = count > 0 ? count - 1 : 0;
    return std::ceil((count * (fontSize + 18.0) + gaps * 8.0 + 20.0 + (32.0 + 6.0 + 18.0 + 2.0 + 1.2 + 8.0)) * factor);
}

static void SendButton(NSButton *button) {
    assert(button != nil && button.target != nil && button.action != nullptr);
    [NSApp sendAction:button.action to:button.target from:button];
}

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];

        NSRect visible = NSMakeRect(-1200.0, -800.0, 1920.0, 1080.0);
        NSRect defaultFrame = MSIMEFloatingToolbarFrame(NSMakeRect(0.0, 0.0, 1.0, 1.0), visible, NO);
        // Five buttons: emoji, handwriting, voice and the screen keyboard are opt-in.
        assert(defaultFrame.size.width == 330.0 && defaultFrame.size.height == 44.0);
        assert(defaultFrame.size.width == ExpectedWidth(5, 24, 1));
        assert(defaultFrame.origin.x == NSMaxX(visible) - 350.0 && defaultFrame.origin.y == NSMinY(visible) + 20.0);
        NSRect restored = MSIMEFloatingToolbarFrame(NSMakeRect(-4000.0, 4000.0, 1.0, 1.0), visible, YES);
        assert(restored.origin.x == NSMinX(visible) + 12.0 && restored.origin.y == NSMaxY(visible) - 56.0);
        assert(MetasequoiaFloatingToolbarShouldShow(YES, YES, NO));
        assert(!MetasequoiaFloatingToolbarShouldShow(NO, YES, NO));
        assert(!MetasequoiaFloatingToolbarShouldShow(YES, NO, NO));
        assert(!MetasequoiaFloatingToolbarShouldShow(YES, YES, YES));
        const CGRect display = CGRectMake(-1440.0, 0.0, 1440.0, 900.0);
        assert(MetasequoiaWindowCoversDisplay(display, display));
        assert(MetasequoiaWindowCoversDisplay(CGRectMake(-1441.0, -1.0, 1442.0, 902.0), display));
        assert(!MetasequoiaWindowCoversDisplay(CGRectMake(-1440.0, 22.0, 1440.0, 878.0), display));
        assert(!MetasequoiaWindowCoversDisplay(CGRectMake(-720.0, 0.0, 1440.0, 900.0), display));
        assert(!MetasequoiaWindowCoversDisplay(CGRectZero, display));

        MSIMEFloatingToolbarPanel *panel = [[MSIMEFloatingToolbarPanel alloc] init];
        assert(panel != nil && !panel.canBecomeKeyWindow && !panel.canBecomeMainWindow);
        assert((panel.collectionBehavior & NSWindowCollectionBehaviorCanJoinAllSpaces) != 0);
        assert((panel.collectionBehavior & NSWindowCollectionBehaviorFullScreenAuxiliary) == 0);
        [panel applyThemePreferences:@{}];
        assert(panel.appearance == nil);
        [panel applyThemePreferences:@{@"theme": @"light", @"toolbar_theme": @"follow"}];
        assert([panel.appearance.name isEqualToString:NSAppearanceNameAqua]);
        [panel applyThemePreferences:@{@"theme": @"light", @"toolbar_theme": @"dark"}];
        assert([panel.appearance.name isEqualToString:NSAppearanceNameDarkAqua]);
        [panel applyThemePreferences:@{@"theme": @"dark", @"toolbar_theme": @"light", @"candidate_theme": @"dark", @"settings_theme": @"dark"}];
        assert([panel.appearance.name isEqualToString:NSAppearanceNameAqua]);
        [panel applyThemePreferences:@{@"theme": @"system", @"toolbar_theme": @"light"}];
        assert([panel.appearance.name isEqualToString:NSAppearanceNameAqua]);
        [panel applyThemePreferences:@{@"theme": @"system", @"toolbar_theme": @"follow"}];
        assert(panel.appearance == nil);
        assert([panel.frameAutosaveName isEqualToString:@"MetasequoiaFloatingToolbarFrame"]);
        {
            // Resizing a hidden toolbar must not write a saved frame. The window has an autosave name, so
            // any setFrame: here is persisted, and setVisible: reads the presence of that default as "the
            // user placed it" - which would strand a toolbar the user has never seen in the corner this
            // path clamps to, instead of the default placement on the screen holding the pointer.
            NSString *key = @"NSWindow Frame MetasequoiaFloatingToolbarFrame";
            [NSUserDefaults.standardUserDefaults removeObjectForKey:key];
            assert(!panel.visible);
            [panel applySizingPreferences:@{@"floating_toolbar": @{@"scale_percent": @150, @"font_size": @28}}];
            assert([NSUserDefaults.standardUserDefaults objectForKey:key] == nil);
            // The size still took effect; it is applied when the toolbar is shown.
            const NSSize hiddenPreferred = [[panel valueForKey:@"preferredSize"] sizeValue];
            assert(hiddenPreferred.width > 0 && hiddenPreferred.height > 0);
            [panel applySizingPreferences:@{@"floating_toolbar": @{@"scale_percent": @100, @"font_size": @24}}];
        }
        [panel setFrameAutosaveName:@""]; // Geometry tests must not persist window placement.
        msime::mac::SkinTokens light{}, dark{};
        light.surface = {0.8, 0.7, 0.6, 1};
        light.border = {0.4, 0.3, 0.2, 1};
        light.text = {0.1, 0.2, 0.3, 1};
        dark.surface = {0.1, 0.2, 0.3, 1};
        dark.border = {0.3, 0.4, 0.5, 1};
        dark.text = {0.9, 0.8, 0.7, 1};
        [panel applyLightSkin:light darkSkin:dark];
        for (NSString *mode in @[@"light", @"dark"]) {
            const auto expected = [mode isEqual:@"dark"] ? dark : light;
            [panel applyThemePreferences:@{@"toolbar_theme": mode}];
            // Legacy notifications and size updates must not replace host-supplied colors.
            [NSNotificationCenter.defaultCenter postNotificationName:MetasequoiaCandidateSkinDidChangeNotification object:nil];
            [panel applySizingPreferences:@{}];
            id chrome = [panel valueForKey:@"chrome"];
            assert([[chrome valueForKey:@"fillColor"] isEqual:MetasequoiaColorFromRgba(expected.surface)]);
            assert([[chrome valueForKey:@"strokeColor"] isEqual:MetasequoiaColorFromRgba(expected.border)]);
            NSButton *button = FindButton(panel.contentView, @"MetasequoiaFloatingToolbarInputMode");
            assert([button.contentTintColor isEqual:MetasequoiaColorFromRgba(expected.text)]);
        }
        dark.surface = {0.3, 0.1, 0.2, 1};
        [panel applyLightSkin:light darkSkin:dark];
        assert([[[panel valueForKey:@"chrome"] valueForKey:@"fillColor"] isEqual:MetasequoiaColorFromRgba(dark.surface)]);

        // Candidate card colors and toolbar colors are separate host inputs.
        // A toolbar theme change must continue using the toolbar palette after
        // the candidate palette has been supplied.
        msime::mac::SkinTokens toolbarLight = light;
        msime::mac::SkinTokens toolbarDark = dark;
        toolbarLight.surface = {0.2, 0.4, 0.6, 1};
        toolbarDark.surface = {0.6, 0.2, 0.4, 1};
        [panel applyLightToolbarSkin:toolbarLight darkSkin:toolbarDark];
        [panel applyThemePreferences:@{@"toolbar_theme": @"dark"}];
        assert([[[panel valueForKey:@"chrome"] valueForKey:@"fillColor"] isEqual:MetasequoiaColorFromRgba(toolbarDark.surface)]);

        NSButton *inputMode = FindButton(panel.contentView, @"MetasequoiaFloatingToolbarInputMode");
        NSButton *punctuation = FindButton(panel.contentView, @"MetasequoiaFloatingToolbarPunctuation");
        NSButton *fullWidth = FindButton(panel.contentView, @"MetasequoiaFloatingToolbarFullWidth");
        NSButton *traditional = FindButton(panel.contentView, @"MetasequoiaFloatingToolbarTraditionalOutput");
        NSButton *settings = FindButton(panel.contentView, @"MetasequoiaFloatingToolbarSettings");
        NSButton *emoji = FindButton(panel.contentView, @"MetasequoiaFloatingToolbarEmoji");
        NSButton *handwriting = FindButton(panel.contentView, @"MetasequoiaFloatingToolbarHandwriting");
        NSButton *keyboard = FindButton(panel.contentView, @"MetasequoiaFloatingToolbarScreenKeyboard");
        NSButton *voice = FindButton(panel.contentView, @"MetasequoiaFloatingToolbarVoice");
        NSView *logo = FindView(panel.contentView, @"MetasequoiaFloatingToolbarLogo");
        assert(logo != nil && [logo.accessibilityLabel isEqualToString:@"水杉输入法"]);
        // The four opt-in buttons, after the empty snapshot above: built, labelled and off the row until
        // a snapshot turns them on. Their labels and images exist either way, which is what makes turning
        // one on a visibility change and nothing else.
        assert(keyboard && keyboard.image && keyboard.hidden);
        assert([keyboard.accessibilityLabel isEqualToString:@"打开水杉屏幕键盘"]);
        assert([keyboard.toolTip isEqualToString:keyboard.accessibilityLabel]);
        assert(emoji && emoji.image && emoji.hidden);
        assert([emoji.accessibilityLabel isEqualToString:@"打开水杉表情面板"]);
        assert([emoji.toolTip isEqualToString:emoji.accessibilityLabel]);
        assert(handwriting && handwriting.image && handwriting.hidden);
        assert([handwriting.accessibilityLabel isEqualToString:@"打开水杉手写识别板"]);
        assert([handwriting.toolTip isEqualToString:handwriting.accessibilityLabel]);
        assert(voice && voice.image && voice.hidden);
        assert([voice.accessibilityLabel isEqualToString:@"开始或结束语音输入"]);
        assert([voice.toolTip isEqualToString:voice.accessibilityLabel]);
        assert(inputMode && punctuation && fullWidth && traditional && handwriting && voice && settings);

        // The reference's ToolbarDragHandle and ToolbarDivider lead the row. Dragging the grip moves the panel; pressing a button never does.
        NSView *grip = FindView(panel.contentView, @"MetasequoiaFloatingToolbarGrip");
        NSView *divider = FindView(panel.contentView, @"MetasequoiaFloatingToolbarDivider");
        assert(grip != nil && divider != nil && grip.mouseDownCanMoveWindow && panel.movableByWindowBackground);
        [panel.contentView layoutSubtreeIfNeeded];
        {
            const NSRect gripRect = [grip convertRect:grip.bounds toView:panel.contentView];
            const NSRect dividerRect = [divider convertRect:divider.bounds toView:panel.contentView];
            const NSRect firstRect = [inputMode convertRect:inputMode.bounds toView:panel.contentView];
            assert(NSMaxX(gripRect) <= NSMinX(dividerRect) && NSMaxX(dividerRect) <= NSMinX(firstRect));
        }
        for (NSButton *button in @[inputMode, punctuation, fullWidth, traditional, emoji, handwriting, keyboard, voice, settings]) {
            assert(!button.mouseDownCanMoveWindow);
            [button updateTrackingAreas];
            BOOL alwaysActive = NO;
            for (NSTrackingArea *area in button.trackingAreas)
                if ((area.options & NSTrackingActiveAlways) != 0 && (area.options & NSTrackingMouseEnteredAndExited) != 0) alwaysActive = YES;
            assert(alwaysActive);
        }

        // Hover and pressed fill: the reference's ToolbarIconButton constants, white 0.10 on dark and black 0.08 on light, never the candidate-row hover token.
        NSEvent *entered = [NSEvent enterExitEventWithType:NSEventTypeMouseEntered location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:panel.windowNumber context:nil eventNumber:0 trackingNumber:0 userData:NULL];
        NSEvent *exited = [NSEvent enterExitEventWithType:NSEventTypeMouseExited location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:panel.windowNumber context:nil eventNumber:0 trackingNumber:0 userData:NULL];
        [inputMode mouseEntered:entered];
        assert([[inputMode valueForKey:@"hovered"] boolValue]);
        [inputMode mouseExited:exited];
        assert(![[inputMode valueForKey:@"hovered"] boolValue]);
        // A hidden window gets no mouseExited:, so hiding the toolbar clears the hover itself.
        [inputMode mouseEntered:entered];
        [panel orderOut:nil];
        assert(![[inputMode valueForKey:@"hovered"] boolValue]);
        [panel applyThemePreferences:@{@"toolbar_theme": @"dark"}];
        assert([[inputMode valueForKey:@"hoverFillColor"] isEqual:[NSColor colorWithSRGBRed:1 green:1 blue:1 alpha:0.10]]);
        assert([[divider valueForKey:@"fillColor"] isEqual:[NSColor colorWithSRGBRed:1 green:1 blue:1 alpha:0.15]]);
        [panel applyThemePreferences:@{@"toolbar_theme": @"light"}];
        assert([[inputMode valueForKey:@"hoverFillColor"] isEqual:[NSColor colorWithSRGBRed:0 green:0 blue:0 alpha:0.08]]);
        assert([[divider valueForKey:@"fillColor"] isEqual:[NSColor colorWithSRGBRed:0 green:0 blue:0 alpha:0.12]]);
        for (NSNumber *scale in @[@75, @100, @125, @150]) {
            for (NSNumber *size in @[@16, @18, @20, @22, @24, @26, @28]) {
                NSDictionary *preferences = @{@"floating_toolbar": @{@"scale_percent": scale, @"font_size": size}};
                [panel applySizingPreferences:preferences];
                const double factor = scale.doubleValue / 100.0;
                // The panel is hidden here, so the size it will take lives in preferredSize rather than in
                // the frame: setVisible: applies it when the toolbar is placed. Resizing a hidden window
                // would otherwise persist a frame through its autosave name.
                const NSSize preferred = [[panel valueForKey:@"preferredSize"] sizeValue];
                assert(preferred.width == ExpectedWidth(5, size.doubleValue, factor));
                assert(preferred.height == std::ceil((size.doubleValue + 20.0) * factor));
                // Every size here scales to a multiple of half a point, which is a whole pixel on a Retina screen and not on a 1x one - the CI runner's display - where AppKit rounds the first item too.
                assert(std::abs(inputMode.frame.size.width - (size.doubleValue + 18.0) * factor) <= 1.0 / panel.backingScaleFactor);
                assert(std::abs(inputMode.frame.size.height - (size.doubleValue + 8.0) * factor) <= 1.0 / panel.backingScaleFactor);
                assert(std::abs(inputMode.font.pointSize - size.doubleValue * factor * 0.833) < 0.01);
                // AppKit aligns the later stack items to backing pixels at fractional positions. Settings
                // rather than emoji: emoji is opt-in and off here, and a hidden stack item has no laid-out
                // frame to measure. Its width constraint below is set either way.
                assert(std::abs(settings.frame.size.width - (size.doubleValue + 18.0) * factor) <= 1.0 / panel.backingScaleFactor);
                for (NSLayoutConstraint *constraint in emoji.constraints) {
                    if ([constraint.identifier isEqualToString:@"ToolbarButtonWidth"])
                        assert(constraint.constant == (size.doubleValue + 18.0) * factor);
                }
                assert([emoji.contentTintColor isEqual:settings.contentTintColor]);
                assert(emoji.symbolConfiguration != nil);
                NSImage *expectedEmoji = [emoji.image imageWithSymbolConfiguration:
                    [NSImageSymbolConfiguration configurationWithPointSize:size.doubleValue * factor weight:NSFontWeightRegular]];
                NSImage *configuredEmoji = [emoji.image imageWithSymbolConfiguration:emoji.symbolConfiguration];
                assert(NSEqualSizes(expectedEmoji.size, configuredEmoji.size));
                NSRect stable = panel.frame;
                const NSSize stablePreferred = [[panel valueForKey:@"preferredSize"] sizeValue];
                [panel applySizingPreferences:preferences];
                assert(NSEqualRects(stable, panel.frame));
                assert(NSEqualSizes(stablePreferred, [[panel valueForKey:@"preferredSize"] sizeValue]));
                [panel applySizingPreferences:@{@"floating_toolbar": @{@"scale_percent": scale, @"font_size": size, @"screen_keyboard": @YES}}];
                assert(!keyboard.hidden && keyboard.superview != nil);
                assert([[panel valueForKey:@"preferredSize"] sizeValue].width == ExpectedWidth(6, size.doubleValue, factor));
                assert([keyboard.contentTintColor isEqual:settings.contentTintColor]);
                NSImage *expectedKeyboard = [keyboard.image imageWithSymbolConfiguration:
                    [NSImageSymbolConfiguration configurationWithPointSize:size.doubleValue * factor weight:NSFontWeightRegular]];
                NSImage *configuredKeyboard = [keyboard.image imageWithSymbolConfiguration:keyboard.symbolConfiguration];
                assert(NSEqualSizes(expectedKeyboard.size, configuredKeyboard.size));
            }
        }
        [panel applySizingPreferences:@{@"floating_toolbar": @{@"scale_percent": @999, @"font_size": @(-1)}}];
        assert(NSEqualSizes([[panel valueForKey:@"preferredSize"] sizeValue], NSMakeSize(330.0, 44.0)));
        [panel applySizingPreferences:@{@"floating_toolbar": @{@"scale_percent": @150, @"font_size": @28}}];
        FloatingToolbarTestDelegate *sizingDelegate = [FloatingToolbarTestDelegate new];
        // Configured while hidden, so it is the preferred size that carries it; showing the toolbar is what
        // puts it on the frame, and it has to survive being hidden and shown again.
        const NSSize configuredSize = [[panel valueForKey:@"preferredSize"] sizeValue];
        [panel activateForDelegate:sizingDelegate visible:YES];
        assert(NSEqualSizes(panel.frame.size, configuredSize));
        [panel setVisible:NO forDelegate:sizingDelegate];
        [panel setVisible:YES forDelegate:sizingDelegate];
        assert(NSEqualSizes(panel.frame.size, configuredSize));
        [panel deactivateForDelegate:sizingDelegate];
        [panel applySizingPreferences:@{}];
        NSArray<NSButton *> *optionalButtons = @[inputMode, punctuation, fullWidth, traditional, emoji, keyboard, settings];
        NSArray<NSString *> *keys = @[@"english_mode", @"punctuation", @"fullwidth", @"character_set", @"emoji", @"screen_keyboard", @"settings"];
        for (NSUInteger mask = 0; mask < 128; ++mask) {
            NSMutableDictionary *components = [@{@"scale_percent": @150, @"font_size": @28, @"english_mode": @NO} mutableCopy];
            // Handwriting and voice are absent from every mask below, and absent now means off, so they
            // add nothing to the count.
            NSUInteger count = 0;
            for (NSUInteger index = 0; index < keys.count; ++index) {
                const BOOL enabled = (mask & (1u << index)) != 0;
                components[keys[index]] = @(enabled);
                if (enabled) ++count;
            }
            [panel applySizingPreferences:@{@"floating_toolbar": components}];
            for (NSUInteger index = 0; index < keys.count; ++index)
                assert(optionalButtons[index].hidden == ((mask & (1u << index)) == 0));
            assert([[panel valueForKey:@"preferredSize"] sizeValue].width == ExpectedWidth(count, 28, 1.5));
            assert(inputMode.superview != nil);
            // The logo, grip and divider lead the row; every visible button sits right of them.
            const NSRect logoRect = [logo convertRect:logo.bounds toView:panel.contentView];
            const NSRect gripRect = [grip convertRect:grip.bounds toView:panel.contentView];
            const NSRect dividerRect = [divider convertRect:divider.bounds toView:panel.contentView];
            // mask 0 leaves the row empty, and an empty row drops the divider - the logo and grip stay.
            assert(divider.hidden == (count == 0) && std::abs(NSMinX(logoRect)) < 0.01 && std::abs(NSWidth(logoRect) - 57.0) <= 1.0 / panel.backingScaleFactor);
            assert(std::abs(NSMinX(gripRect) - NSMaxX(logoRect)) <= 1.0 / panel.backingScaleFactor && std::abs(NSWidth(gripRect) - 27.0) <= 1.0 / panel.backingScaleFactor);
            assert(NSMinX(dividerRect) >= NSMaxX(gripRect) && std::abs(NSWidth(dividerRect) - 1.8) <= 1.0 / panel.backingScaleFactor);
            CGFloat previousRight = NSMaxX(dividerRect);
            for (NSButton *button in @[inputMode, punctuation, fullWidth, traditional, emoji, handwriting, keyboard, voice, settings]) {
                if (button.hidden) continue;
                const NSRect rect = [button convertRect:button.bounds toView:panel.contentView];
                assert(NSMinX(rect) >= previousRight);
                assert(NSMaxX(rect) <= panel.contentView.bounds.size.width);
                previousRight = NSMaxX(rect);
            }
        }
        // With every button off the divider goes, but the grip stays so the panel can still be dragged.
        [panel applySizingPreferences:@{@"floating_toolbar": @{@"english_mode": @NO, @"punctuation": @NO, @"fullwidth": @NO, @"character_set": @NO, @"emoji": @NO, @"handwriting": @NO, @"screen_keyboard": @NO, @"voice": @NO, @"settings": @NO}}];
        assert(FindView(panel.contentView, @"MetasequoiaFloatingToolbarGrip") == grip && !grip.hidden && divider.hidden);
        assert([[panel valueForKey:@"preferredSize"] sizeValue].width == ExpectedWidth(0, 24, 1));
        [panel applySizingPreferences:@{}];
        assert(!divider.hidden);
        for (NSButton *button in optionalButtons) {
            assert(button.hidden == (button == keyboard || button == emoji));
            if (!button.hidden) assert(button.superview != nil);
        }
        assert([[panel valueForKey:@"preferredSize"] sizeValue].width == 330.0);
        [panel applySizingPreferences:@{@"floating_toolbar": @{@"screen_keyboard": @"invalid"}}];
        assert(keyboard.hidden && [[panel valueForKey:@"preferredSize"] sizeValue].width == 330.0);
        [panel applySizingPreferences:@{@"floating_toolbar": @{@"screen_keyboard": @YES}}];
        assert(!keyboard.hidden && [[panel valueForKey:@"preferredSize"] sizeValue].width == ExpectedWidth(6, 24, 1));

        [panel updateEnglishInputMode:YES chinesePunctuationEnabled:NO fullWidthEnabled:YES traditionalChineseOutputEnabled:YES];
        assert([inputMode.title isEqualToString:@"英"] && [punctuation.title isEqualToString:@"."] &&
               [fullWidth.title isEqualToString:@"全"] && [traditional.title isEqualToString:@"繁"]);
        assert([inputMode.toolTip isEqualToString:inputMode.accessibilityLabel]);
        assert([punctuation.toolTip isEqualToString:punctuation.accessibilityLabel]);
        assert([fullWidth.toolTip isEqualToString:fullWidth.accessibilityLabel]);
        assert([traditional.toolTip isEqualToString:traditional.accessibilityLabel]);

        [panel updateEnglishInputMode:NO
                    japaneseInputMode:YES
                             capsLock:NO
                chinesePunctuationEnabled:YES
                         fullWidthEnabled:NO
          traditionalChineseOutputEnabled:NO];
        assert([inputMode.title isEqualToString:@"日"]);
        [panel updateEnglishInputMode:NO
                    japaneseInputMode:YES
                             capsLock:YES
                chinesePunctuationEnabled:YES
                         fullWidthEnabled:NO
          traditionalChineseOutputEnabled:NO];
        assert([inputMode.title isEqualToString:@"A"]);
        [panel updateEnglishInputMode:YES
                    japaneseInputMode:YES
                             capsLock:NO
                chinesePunctuationEnabled:YES
                         fullWidthEnabled:NO
          traditionalChineseOutputEnabled:NO];
        assert([inputMode.title isEqualToString:@"英"]);
        [panel updateEnglishInputMode:NO
                 englishCandidateMode:YES
                    japaneseInputMode:YES
                             capsLock:NO
                chinesePunctuationEnabled:YES
                         fullWidthEnabled:NO
          traditionalChineseOutputEnabled:NO];
        assert([inputMode.title isEqualToString:@"En"]);

        FloatingToolbarTestDelegate *delegate = [FloatingToolbarTestDelegate new];
        panel.toolbarDelegate = delegate;
        SendButton(inputMode);
        SendButton(punctuation);
        SendButton(fullWidth);
        SendButton(traditional);
        SendButton(emoji);
        assert(delegate.emojiRequests == 1 && delegate.characterPaletteRequests == 0);
        SendButton(handwriting);
        assert(delegate.handwritingRequests == 1);
        SendButton(keyboard);
        assert(delegate.keyboardRequests == 1 && delegate.emojiRequests == 1);
        SendButton(voice);
        assert(delegate.voiceToggles == 1);
        // The gear opens settings on click; the utility menu is only its right-click menu.
        SendButton(settings);
        assert(delegate.settingsRequests == 1);
        assert(settings.menu.numberOfItems == 10 && [settings.menu itemAtIndex:1].action == @selector(openSettings:));
        assert([settings.accessibilityLabel isEqualToString:@"打开水杉输入法设置"]);
        for (NSString *selectorName in @[@"openCharacterPalette:", @"openSettings:", @"checkForUpdates:",
                                         @"openWebsite:", @"dismissFloatingToolbar:"]) {
            [NSApp sendAction:NSSelectorFromString(selectorName) to:panel from:nil];
        }
        assert(delegate.inputModeToggles == 1 && delegate.punctuationToggles == 1 &&
               delegate.fullWidthToggles == 1 && delegate.traditionalToggles == 1 &&
               delegate.characterPaletteRequests == 1 && delegate.settingsRequests == 2 &&
               delegate.updateRequests == 1 && delegate.websiteRequests == 1 && delegate.hideRequests == 1);

        NSMenu *menu = CreateMSIMEFloatingToolbarUtilityMenu(panel);
        [menu update];
        assert(menu.numberOfItems == 10);
        assert([menu itemAtIndex:0].action == @selector(openCharacterPalette:) &&
               [menu itemAtIndex:1].action == @selector(openSettings:) &&
               [menu itemAtIndex:2].action == @selector(checkForUpdates:) &&
               [menu itemAtIndex:4].action == @selector(openWebsite:) &&
               [menu itemAtIndex:5].action == @selector(openHelp:) &&
               [menu itemAtIndex:6].action == @selector(openAbout:) &&
               [menu itemAtIndex:7].action == @selector(openFeedback:) &&
               [menu itemAtIndex:9].action == @selector(dismissFloatingToolbar:));
        for (NSMenuItem *item in menu.itemArray) {
            if (!item.isSeparatorItem) assert(item.enabled && item.target == panel);
        }

        [panel setVisible:NO forDelegate:delegate];
        assert(!panel.visible && panel.toolbarDelegate == delegate);
        [panel setVisible:YES forDelegate:[FloatingToolbarTestDelegate new]];
        assert(!panel.visible);
        [panel deactivateForDelegate:[FloatingToolbarTestDelegate new]];
        assert(panel.toolbarDelegate == delegate);
        [panel deactivateForDelegate:delegate];
        assert(!panel.visible && panel.toolbarDelegate == nil);
        SendButton(keyboard);
        assert(delegate.keyboardRequests == 1);
        SendButton(emoji);
        assert(delegate.emojiRequests == 1);
        FloatingToolbarTestDelegate *newDelegate = [FloatingToolbarTestDelegate new];
        [panel activateForDelegate:newDelegate visible:NO];
        SendButton(keyboard);
        assert(newDelegate.keyboardRequests == 1 && delegate.keyboardRequests == 1);
        SendButton(emoji);
        assert(newDelegate.emojiRequests == 1 && delegate.emojiRequests == 1);
        [panel deactivateForDelegate:newDelegate];

        // Selecting another input source is the reference's WM_IMEDEACTIVATE: the toolbar hides and lets go of whichever controller owned it, so that controller can no longer show it again. Whether the first show actually puts the panel on screen depends on the frontmost app being fullscreen, so the reactivation is compared against it rather than against a fixed value.
        FloatingToolbarTestDelegate *residentOwner = [FloatingToolbarTestDelegate new];
        [panel activateForDelegate:residentOwner visible:YES];
        const BOOL shownWhenActive = panel.visible;
        assert(panel.toolbarDelegate == residentOwner);
        [panel deactivateForInputSourceSwitch];
        assert(!panel.visible && panel.toolbarDelegate == nil);
        [panel setVisible:YES forDelegate:residentOwner];
        assert(!panel.visible && panel.toolbarDelegate == nil);
        SendButton(keyboard);
        assert(residentOwner.keyboardRequests == 0);

        // The next activation, which is the user selecting MSIME again, shows it as before.
        FloatingToolbarTestDelegate *returningOwner = [FloatingToolbarTestDelegate new];
        [panel activateForDelegate:returningOwner visible:YES];
        assert(panel.toolbarDelegate == returningOwner && panel.visible == shownWhenActive);
        [panel deactivateForInputSourceSwitch];
        assert(!panel.visible && panel.toolbarDelegate == nil);

        // A resident toolbar whose owner is freed (the client app quit) hides on the next refresh, which every application activation triggers, instead of leaving buttons that reach nobody.
        @autoreleasepool {
            FloatingToolbarTestDelegate *transientOwner = [FloatingToolbarTestDelegate new];
            [panel activateForDelegate:transientOwner visible:YES];
            assert(panel.visible == shownWhenActive);
        }
        assert(panel.toolbarDelegate == nil);
        [[NSWorkspace sharedWorkspace].notificationCenter postNotificationName:NSWorkspaceDidActivateApplicationNotification object:[NSWorkspace sharedWorkspace]];
        assert(!panel.visible);
    }
}
