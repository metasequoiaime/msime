#import "../../src/input/TypingEffectPanel.h"
#import "../../src/candidate/CandidateWindowLevel.h"
#import <AppKit/AppKit.h>
#include <cstdio>
#include <stdexcept>

namespace {
void Require(bool condition, const char *message) { if (!condition) throw std::runtime_error(message); }

// msime_client_typing_effect's packing: bits 0-15 the combo, bit 16 tier-up, bits 17-19 the style, bit 20 the tier sound.
uint32_t Pack(uint32_t combo, bool tierUp, MSIMETypingEffectStyle style) {
    return combo | (tierUp ? MSIMETypingEffectTierUp : 0) | (uint32_t)style << 17 | (tierUp ? 0x100000u : 0);
}
}

int main() {
    @autoreleasepool {
        try {
            const MSIMETypingEffect decoded = MSIMETypingEffectDecode(Pack(25, true, MSIMETypingEffectStylePowerMode));
            Require(decoded.combo == 25 && decoded.tierUp && decoded.style == MSIMETypingEffectStylePowerMode, "answer not decoded");
            const MSIMETypingEffect off = MSIMETypingEffectDecode(0);
            Require(off.combo == 0 && !off.tierUp && off.style == MSIMETypingEffectStyleOff, "zero answer is not off");
            Require(MSIMETypingEffectDecode(7u << 17).style == MSIMETypingEffectStyleOff, "unknown style code was drawn");
            Require(MSIMETypingEffectDecode(0xFFFF).combo == 65535, "saturated combo not decoded");

            Require(MSIMETypingEffectDrawnStyle(MSIMETypingEffectStyleSparks, NO, NO) == MSIMETypingEffectStyleSparks, "sparks degraded without cause");
            Require(MSIMETypingEffectDrawnStyle(MSIMETypingEffectStyleSparks, YES, NO) == MSIMETypingEffectStyleFlash, "Reduce Motion did not degrade sparks");
            Require(MSIMETypingEffectDrawnStyle(MSIMETypingEffectStylePowerMode, NO, YES) == MSIMETypingEffectStyleFlash, "Low Power did not degrade power mode");
            Require(MSIMETypingEffectDrawnStyle(MSIMETypingEffectStyleFlash, YES, YES) == MSIMETypingEffectStyleFlash, "flash changed under Reduce Motion");
            Require(MSIMETypingEffectDrawnStyle(MSIMETypingEffectStyleOff, YES, YES) == MSIMETypingEffectStyleOff, "off became visible");

            Require(MSIMETypingEffectComboText(0) == nil && MSIMETypingEffectComboText(1) == nil, "a single key showed a combo");
            Require([MSIMETypingEffectComboText(12) isEqualToString:@"连击 ×12"], "combo text mismatch");

            MSIMETypingEffectPanel *panel = MSIMETypingEffectPanel.sharedPanel;
            Require(panel == MSIMETypingEffectPanel.sharedPanel, "panel is not shared");
            Require(panel.ignoresMouseEvents && panel.floatingPanel && !panel.opaque && !panel.canBecomeKeyWindow && !panel.canBecomeMainWindow, "panel could take clicks or focus");
            Require((panel.styleMask & NSWindowStyleMaskNonactivatingPanel) != 0, "panel would activate the input method");
            Require(panel.level == MSIMECandidateWindowLevel() - 1, "panel is not just under the candidate window");
            Require(panel.level > NSScreenSaverWindowLevel, "panel is under screenshot overlays");
            Require((panel.collectionBehavior & NSWindowCollectionBehaviorFullScreenAuxiliary) != 0, "panel cannot appear beside a full-screen window");
            Require(!panel.configured && panel.intensity == 50 && !panel.isVisible && !panel.emitting, "panel not idle before use");

            // 强度在 macOS 上固定为 50：文档里存的旧强度（包括越界的值）一律不读。
            [panel applyPreferences:@{@"plugins" : @{@"effect_style" : @"sparks", @"effect_intensity" : @80}}];
            Require(panel.configured && panel.intensity == 50, "a stored intensity was honoured");
            // 特效包换上的强度：没有 plugins 的文档不动它，带 plugins 的文档把它放回固定的 50。
            [panel applySettings:@{@"intensity" : @70}];
            Require(panel.intensity == 70, "a pack intensity was not applied");
            [panel applyPreferences:@{@"theme" : @"dark"}];
            Require(panel.configured && panel.intensity == 70, "a document without plugins reset the effect");
            [panel applyPreferences:@{@"plugins" : @{@"effect_style" : @"sparks"}}];
            Require(panel.configured && panel.intensity == 50, "a plugins document did not return to the fixed intensity");
            [panel applyPreferences:@{@"plugins" : @{@"effect_style" : @"lasers", @"combo_counter" : @YES, @"effect_intensity" : @400}}];
            Require(panel.configured && panel.intensity == 50, "unknown style not bounded or a stored intensity was honoured");
            [panel applyPreferences:@{@"plugins" : @{@"effect_style" : @"sparks", @"combo_counter" : @YES}}];

            const NSRect caret = NSMakeRect(600, 400, 2, 20);
            [panel presentEffect:Pack(1, false, MSIMETypingEffectStyleSparks) commit:NO caretRect:caret candidateView:nil cardRect:NSZeroRect cornerRadius:0 reduceMotion:NO lowPower:NO];
            Require(panel.emitting && panel.isVisible && panel.drawnStyle == MSIMETypingEffectStyleSparks && panel.displayedCombo == nil, "sparks not drawn at the caret");
            Require(NSPointInRect(NSMakePoint(NSMinX(caret), NSMidY(caret)), panel.frame), "panel does not cover the caret");

            [panel presentEffect:Pack(10, true, MSIMETypingEffectStyleSparks) commit:NO caretRect:caret candidateView:nil cardRect:NSZeroRect cornerRadius:0 reduceMotion:NO lowPower:NO];
            Require([panel.displayedCombo isEqualToString:@"连击 ×10"], "combo badge missing");

            [panel settle];
            Require(!panel.emitting && !panel.isVisible && panel.displayedCombo == nil, "settle left the panel on screen");

            [panel presentEffect:Pack(3, false, MSIMETypingEffectStyleSparks) commit:YES caretRect:caret candidateView:nil cardRect:NSZeroRect cornerRadius:0 reduceMotion:YES lowPower:NO];
            Require(!panel.emitting && panel.drawnStyle == MSIMETypingEffectStyleFlash, "Reduce Motion still emitted sparks");

            // A backspace ends the combo: the answer has no count and the style off, and the badge comes down at once.
            [panel presentEffect:Pack(0, false, MSIMETypingEffectStyleOff) commit:NO caretRect:caret candidateView:nil cardRect:NSZeroRect cornerRadius:0 reduceMotion:NO lowPower:NO];
            Require(panel.displayedCombo == nil && !panel.isVisible, "an ended combo left the badge");

            // The card flash lies over the candidate card, takes no clicks, and leaves with the panel's idle state.
            NSPanel *candidate = [[NSPanel alloc] initWithContentRect:NSMakeRect(600, 300, 240, 80)
                                                            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                                                              backing:NSBackingStoreBuffered
                                                                defer:NO];
            candidate.releasedWhenClosed = NO;
            NSView *content = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 240, 80)];
            [content addSubview:[[NSView alloc] initWithFrame:NSMakeRect(0, 0, 240, 72)]];
            candidate.contentView = content;
            [candidate orderFrontRegardless];
            [panel applyPreferences:@{@"plugins" : @{@"effect_style" : @"flash", @"combo_counter" : @NO}}];
            [panel presentEffect:Pack(0, false, MSIMETypingEffectStyleFlash) commit:NO caretRect:caret candidateView:content cardRect:NSMakeRect(0, 0, 240, 72) cornerRadius:8 reduceMotion:NO lowPower:NO];
            NSView *flash = content.subviews.lastObject;
            Require([flash isKindOfClass:MSIMETypingEffectFlashView.class] && [flash.identifier isEqualToString:@"candidate-typing-flash"], "card flash not on top of the card");
            Require(NSEqualRects(flash.frame, NSMakeRect(0, 0, 240, 72)) && [flash hitTest:NSMakePoint(10, 10)] == nil, "card flash misplaced or takes clicks");
            Require(!panel.isVisible && !panel.emitting, "flash alone put the spark panel on screen");
            [panel presentEffect:Pack(0, false, MSIMETypingEffectStyleFlash) commit:YES caretRect:caret candidateView:content cardRect:NSMakeRect(0, 0, 240, 72) cornerRadius:8 reduceMotion:NO lowPower:NO];
            NSUInteger flashes = 0;
            for (NSView *subview in content.subviews) flashes += [subview isKindOfClass:MSIMETypingEffectFlashView.class] ? 1 : 0;
            Require(flashes == 1, "a second flash stacked another overlay");
            [panel settle];
            Require(![content.subviews.lastObject isKindOfClass:MSIMETypingEffectFlashView.class], "settle left the card flash");
            [candidate orderOut:nil];

            [panel applyPreferences:@{@"plugins" : @{@"effect_style" : @"off", @"combo_counter" : @NO}}];
            Require(!panel.configured && !panel.isVisible, "turning effects off left something drawn");

            // An effect pack selected with the style off still asks the library: the pack's style is the session's to resolve.
            [panel applyPreferences:@{@"plugins" : @{@"effect_style" : @"off", @"effect_pack" : @"neon", @"combo_counter" : @NO}}];
            Require(panel.configured, "a selected effect pack left the effect unconfigured");
            Require(MSIMETypingEffectColor(@"#FF8800") != nil && MSIMETypingEffectColor(@"FF8800") == nil && MSIMETypingEffectColor(@"#GG8800") == nil &&
                        MSIMETypingEffectColor(@3) == nil,
                    "pack colour parsing");
            [panel applySettings:@{@"pack" : @"neon", @"style" : @"sparks", @"intensity" : @70, @"colors" : @[@"#FF0000", @"bad", @"#00FF00"],
                                   @"duration_ms" : @400, @"particles" : @12, @"combo_counter" : @NO}];
            Require(panel.intensity == 70 && panel.effectColors.count == 2 && panel.effectDuration == 0.4 && panel.effectParticles == 12, "pack settings not applied");
            [panel presentEffect:Pack(2, false, MSIMETypingEffectStyleSparks) commit:NO caretRect:caret candidateView:nil cardRect:NSZeroRect cornerRadius:0 reduceMotion:NO lowPower:NO];
            Require(panel.emitting, "pack sparks not drawn");
            [panel settle];
            [panel applySettings:@{@"pack" : @"quiet", @"style" : @"sparks", @"intensity" : @50, @"colors" : @[], @"duration_ms" : [NSNull null], @"particles" : @0, @"combo_counter" : @NO}];
            [panel presentEffect:Pack(1, false, MSIMETypingEffectStyleSparks) commit:NO caretRect:caret candidateView:nil cardRect:NSZeroRect cornerRadius:0 reduceMotion:NO lowPower:NO];
            Require(!panel.emitting && panel.effectDuration == 0 && panel.effectColors.count == 0, "a pack with no particles emitted sparks");
            [panel settle];
            [panel applySettings:@{@"intensity" : @900, @"duration_ms" : @5, @"particles" : @500}];
            Require(panel.intensity == 100 && panel.effectDuration == 0.06 && panel.effectParticles == 64, "pack settings not bounded");
            [panel applySettings:nil];
            Require(panel.effectColors.count == 0 && panel.effectDuration == 0 && panel.effectParticles == -1, "no settings kept a pack's hints");
            [panel applyPreferences:@{@"plugins" : @{@"effect_style" : @"off", @"effect_pack" : @"", @"combo_counter" : @NO}}];
            Require(!panel.configured, "an empty effect pack counted as selected");
        } catch (const std::exception &error) { std::fprintf(stderr, "%s\n", error.what()); return 1; }
    }
    return 0;
}
