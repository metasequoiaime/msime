#import "../../src/input/InputController.mm"
#import "../settings/TestPreferenceSuite.h"

#include <cassert>
#include <cstdio>
#include <sqlite3.h>

// A document the controller can read back, which is what the conversion depends on: it re-reads the mark
// before rewriting it rather than trusting what it believes it committed.
@interface SpaceConvertClient : NSObject <MSIMETextClient>
@property(nonatomic, copy) NSString *document;
@property(nonatomic) NSRange selection;
@property(nonatomic) NSUInteger replacements;
@end

@implementation SpaceConvertClient
- (NSRange)selectedRange { return self.selection; }
- (NSAttributedString *)attributedSubstringFromRange:(NSRange)range {
    if (range.location == NSNotFound || range.location > self.document.length ||
        range.length > self.document.length - range.location)
        return nil;
    return [[NSAttributedString alloc] initWithString:[self.document substringWithRange:range]];
}
- (void)insertText:(id)text replacementRange:(NSRange)range {
    if (![text isKindOfClass:NSString.class]) return;
    ++self.replacements;
    NSRange target = range.location == NSNotFound ? self.selection : range;
    self.document = [self.document stringByReplacingCharactersInRange:target withString:text];
    self.selection = NSMakeRange(target.location + [text length], 0);
}
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement {
    (void)text; (void)selection; (void)replacement;
}
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index lineHeightRectangle:(NSRect *)rect {
    (void)index;
    *rect = NSMakeRect(100, 100, 1, 16);
    return @{};
}
@end

@interface SpaceConvertController : MSIMEInputController
@end
@implementation SpaceConvertController
- (void)ensureAppearance {}
@end

// The candidate panel is driven for real but never put on screen.
@interface SpaceConvertHiddenPanel : MSIMECandidatePanel
@property(nonatomic) BOOL requestedVisible;
@end
@implementation SpaceConvertHiddenPanel
- (BOOL)isVisible { return self.requestedVisible; }
- (void)orderFrontRegardless { self.requestedVisible = YES; }
- (void)orderOut:(id)sender { (void)sender; self.requestedVisible = NO; }
@end

static NSEvent *KeyWithFlags(NSString *characters, NSEventModifierFlags flags) {
    return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0
                        windowNumber:0 context:nil characters:characters
     charactersIgnoringModifiers:characters isARepeat:NO keyCode:0];
}
static NSEvent *Key(NSString *characters) { return KeyWithFlags(characters, 0); }

// Arm the controller the way a committed Chinese mark does, without running the Engine.
static void Arm(SpaceConvertController *controller, unichar mark, id client) {
    [controller setValue:@(mark) forKey:@"spaceConvertMark"];
    [controller setValue:client forKey:@"spaceConvertClient"];
}

static SpaceConvertClient *ClientWith(NSString *document) {
    SpaceConvertClient *client = [SpaceConvertClient new];
    client.document = document;
    client.selection = NSMakeRange(document.length, 0);
    return client;
}

static NSEvent *PhysicalKey(unsigned short code, NSString *characters, NSEventModifierFlags flags) {
    return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0
                        windowNumber:0 context:nil characters:characters
     charactersIgnoringModifiers:characters isARepeat:NO keyCode:code];
}

// The arm is made where the commit happens, so these cases run real keys through handleEvent: and a real Engine session over the fixture dictionary (the tbl_2_n schema create_fixture_dictionary.py writes), rather than arming by hand.
static void TestRealKeyArming(MSIMEAppearancePreferences *appearance) {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *options = [@{@"api_version" : @1,
        @"preferences" : @{@"scheme" : @"quanpin", @"default_ime_mode" : @"chinese", @"candidate_page_size" : @5,
                           @"learning" : @NO, @"chinese_punctuation" : @YES}} mutableCopy];
    for (NSString *name in @[ @"resources", @"user_data", @"cache", @"dictionaries" ]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    sqlite3 *database = nullptr;
    assert(sqlite3_open([[options[@"dictionaries"] stringByAppendingPathComponent:@"msime-pinyin.db"] fileSystemRepresentation], &database) == SQLITE_OK);
    assert(sqlite3_exec(database, "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
        "INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100);", nullptr, nullptr, nullptr) == SQLITE_OK);
    assert(sqlite3_close(database) == SQLITE_OK);
    NSError *error = nil;
    MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(session && !error);
    assert([session setFocused:YES error:&error] && !error);

    SpaceConvertController *controller = [SpaceConvertController alloc];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[[SpaceConvertHiddenPanel alloc] init] forKey:@"panel"];
    [controller setValue:@{@"editing_text" : @"", @"candidates" : @[]} forKey:@"view"];
    appearance.smartPunctuation = YES;
    appearance.smartPunctuationSpaceConvert = YES;
    appearance.smartPunctuationRepeatToChinese = NO;
    appearance.pairedPunctuation = NO;
    appearance.fullWidthInput = NO;
    // Comma/period paging would claim the comma while candidates are up, so it is off here: the case is the comma that ends a composition.
    const BOOL commaPaging = [appearance navigationEnabled:@"comma_period"];
    [appearance setNavigation:@"comma_period" enabled:NO];
    NSEvent *space = PhysicalKey(49, @" ", 0);
    auto Focus = [&](SpaceConvertClient *client) {
        [controller setValue:client forKey:@"activeClient"];
        [controller clearSmartPunctuationSpaceConversion];
        [controller resetSmartPunctuationState];
    };
    auto TypeNihao = [&](SpaceConvertClient *client) {
        for (NSArray *stroke in @[ @[ @45, @"n" ], @[ @34, @"i" ], @[ @4, @"h" ], @[ @0, @"a" ], @[ @31, @"o" ] ])
            assert([controller handleEvent:PhysicalKey([stroke[0] unsignedShortValue], stroke[1], 0) client:client]);
        assert([[controller valueForKey:@"view"][@"editing_text"] length]);
        assert([client.document isEqual:@""]);
    };

    // (a) The comma that ends a composition commits the candidate together with the mark. That commit arms, as the reference's beforeChar/tail fingerprint does, so Space turns 你好， into 你好, and is consumed.
    SpaceConvertClient *composed = ClientWith(@"");
    Focus(composed);
    TypeNihao(composed);
    assert([controller handleEvent:PhysicalKey(43, @",", 0) client:composed]);
    assert([composed.document isEqual:@"你好，"]);
    assert([[controller valueForKey:@"spaceConvertMark"] integerValue] == ',');
    const NSUInteger composedReplacements = composed.replacements;
    assert([controller handleEvent:space client:composed]);
    assert([composed.document isEqual:@"你好,"] && composed.replacements == composedReplacements + 1);

    // (b) The ASCII target comes from the mark map, not the key: the backslash key gives 、, and Space turns it into /.
    SpaceConvertClient *enumeration = ClientWith(@"");
    Focus(enumeration);
    assert([controller handleEvent:PhysicalKey(42, @"\\", 0) client:enumeration]);
    assert([enumeration.document isEqual:@"、"]);
    assert([[controller valueForKey:@"spaceConvertMark"] integerValue] == '/');
    assert([controller handleEvent:space client:enumeration]);
    assert([enumeration.document isEqual:@"/"]);

    // Shift is how several marks are typed; an idle Shift+1 commits ！ and arms !.
    SpaceConvertClient *shifted = ClientWith(@"abc");
    Focus(shifted);
    assert([controller handleEvent:PhysicalKey(18, @"!", NSEventModifierFlagShift) client:shifted]);
    assert([shifted.document isEqual:@"abc！"]);
    assert([controller handleEvent:space client:shifted]);
    assert([shifted.document isEqual:@"abc!"]);

    // (c) An auto-closed pair does not arm: the two halves sit either side of the caret, so rewriting the left one alone would orphan the right. Space is left to the application.
    appearance.pairedPunctuation = YES;
    SpaceConvertClient *paired = ClientWith(@"");
    Focus(paired);
    assert([controller handleEvent:PhysicalKey(25, @"(", NSEventModifierFlagShift) client:paired]);
    assert([paired.document isEqual:@"（"]);
    assert([[controller valueForKey:@"pendingPairedClosing"] isEqual:@"）"]);
    assert([[controller valueForKey:@"spaceConvertMark"] integerValue] == 0);
    assert(![controller convertSmartPunctuationSpace:space client:paired]);
    assert([paired.document isEqual:@"（"]);
    [controller flushPendingPairedClosing];
    appearance.pairedPunctuation = NO;

    // (d) With the space conversion off, the same composition-ending comma commits but does not arm.
    appearance.smartPunctuationSpaceConvert = NO;
    SpaceConvertClient *disabled = ClientWith(@"");
    Focus(disabled);
    TypeNihao(disabled);
    assert([controller handleEvent:PhysicalKey(43, @",", 0) client:disabled]);
    assert([disabled.document isEqual:@"你好，"]);
    assert([[controller valueForKey:@"spaceConvertMark"] integerValue] == 0);
    appearance.smartPunctuationSpaceConvert = YES;
    assert(![controller convertSmartPunctuationSpace:space client:disabled]);
    assert([disabled.document isEqual:@"你好，"]);

    // A candidate picked with Space is not a punctuation commit and never arms, as in the reference, whose punctuation handler is the only caller.
    SpaceConvertClient *picked = ClientWith(@"");
    Focus(picked);
    TypeNihao(picked);
    assert([controller handleEvent:space client:picked]);
    assert([picked.document isEqual:@"你好"]);
    assert([[controller valueForKey:@"spaceConvertMark"] integerValue] == 0);

    [controller clearSmartPunctuationSpaceConversion];
    [controller resetSmartPunctuationState];
    [appearance setNavigation:@"comma_period" enabled:commaPaging];
    assert([session closeWithError:&error] && !error);
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
}

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSString *suite = [@"msime.smart.space." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        // The whole family is off in the source, so both the parent switch and the space rewrite have to be asked for before anything rewrites text.
        assert(!appearance.smartPunctuation && !appearance.smartPunctuationSpaceConvert);

        SpaceConvertController *controller = [SpaceConvertController alloc];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:@{@"editing_text" : @"", @"candidates" : @[]} forKey:@"view"];

        // Disabled: the arm is dropped and the mark the user saw land stays put.
        SpaceConvertClient *client = ClientWith(@"测试，");
        Arm(controller, ',', client);
        assert(![controller convertSmartPunctuationSpace:Key(@" ") client:client]);
        assert([client.document isEqual:@"测试，"] && client.replacements == 0);

        appearance.smartPunctuation = YES;
        appearance.smartPunctuationSpaceConvert = YES;
        assert(appearance.smartPunctuation && appearance.smartPunctuationSpaceConvert);

        // Enabled: every standalone Chinese mark from the source mapping becomes its ASCII form. The space is the gesture and is consumed; it is not document content after a successful rewrite. 、 is covered with the real backslash key in TestRealKeyArming, since its ASCII target is not the key that types it.
        for (NSString *pair in @[ @",，", @".。", @":：", @"!！", @"?？", @";；",
                                  @"\"“", @"\"”", @"'‘", @"'’", @"[【", @"]】", @"<《",
                                  @">》", @"(（", @")）" ]) {
            const unichar ascii = [pair characterAtIndex:0];
            SpaceConvertClient *editor = ClientWith([@"abc" stringByAppendingString:[pair substringFromIndex:1]]);
            Arm(controller, ascii, editor);
            assert([controller convertSmartPunctuationSpace:Key(@" ") client:editor]);
            NSString *expected = [@"abc" stringByAppendingString:[NSString stringWithCharacters:&ascii length:1]];
            assert([editor.document isEqual:expected]);
            assert(editor.replacements == 1);
        }

        // One space only. A second finds nothing armed and leaves the ASCII mark alone.
        SpaceConvertClient *once = ClientWith(@"abc，");
        Arm(controller, ',', once);
        assert([controller convertSmartPunctuationSpace:Key(@" ") client:once]);
        assert([once.document isEqual:@"abc,"]);
        assert(![controller convertSmartPunctuationSpace:Key(@" ") client:once]);
        assert([once.document isEqual:@"abc,"] && once.replacements == 1);

        // Any other key disarms: the conversion is for the space that immediately follows the mark.
        SpaceConvertClient *interrupted = ClientWith(@"abc，");
        Arm(controller, ',', interrupted);
        assert(![controller convertSmartPunctuationSpace:Key(@"x") client:interrupted]);
        assert(![controller convertSmartPunctuationSpace:Key(@" ") client:interrupted]);
        assert([interrupted.document isEqual:@"abc，"] && interrupted.replacements == 0);

        // A space in a different editor must not rewrite anything: the arm belongs to one client.
        SpaceConvertClient *armed = ClientWith(@"abc，");
        SpaceConvertClient *other = ClientWith(@"other，");
        Arm(controller, ',', armed);
        assert(![controller convertSmartPunctuationSpace:Key(@" ") client:other]);
        assert([armed.document isEqual:@"abc，"] && [other.document isEqual:@"other，"]);
        assert(armed.replacements == 0 && other.replacements == 0);

        // The document is authoritative. If the mark is no longer there - the user moved the caret, or
        // something else edited - nothing is rewritten.
        SpaceConvertClient *moved = ClientWith(@"abc，def");
        Arm(controller, ',', moved);
        assert(![controller convertSmartPunctuationSpace:Key(@" ") client:moved]);
        assert([moved.document isEqual:@"abc，def"] && moved.replacements == 0);

        // A composition owns the keyboard; the mark behind it is not this feature's to touch.
        [controller setValue:@{@"editing_text" : @"ni", @"candidates" : @[]} forKey:@"view"];
        SpaceConvertClient *composing = ClientWith(@"abc，");
        Arm(controller, ',', composing);
        assert(![controller convertSmartPunctuationSpace:Key(@" ") client:composing]);
        assert([composing.document isEqual:@"abc，"] && composing.replacements == 0);
        [controller setValue:@{@"editing_text" : @"", @"candidates" : @[]} forKey:@"view"];

        // Full-width mode owns the punctuation form and does not run the ASCII conversion gesture.
        appearance.fullWidthInput = YES;
        SpaceConvertClient *wide = ClientWith(@"abc\u3002");
        Arm(controller, '.', wide);
        assert(![controller convertSmartPunctuationSpace:Key(@" ") client:wide]);
        assert([wide.document isEqual:@"abc\u3002"] && wide.replacements == 0);
        appearance.fullWidthInput = NO;

        // An auto-completed pair sits around the caret. Rewriting only its left half would produce a
        // mixed pair, so the gesture is disabled while the host owns a pending closing mark.
        SpaceConvertClient *paired = ClientWith(@"abc（");
        Arm(controller, '(', paired);
        [controller setValue:@"）" forKey:@"pendingPairedClosing"];
        assert(![controller convertSmartPunctuationSpace:Key(@" ") client:paired]);
        assert([paired.document isEqual:@"abc（"] && paired.replacements == 0);
        [controller setValue:nil forKey:@"pendingPairedClosing"];

        // The Japanese scheme leaves both reversible gestures unclaimed, as KeyEventSink does behind !JapaneseInputModeEnabled: a space after 。 is an ordinary space, and a repeated . is not rewritten back to Chinese. The same setup under the Chinese scheme still rewrites, so the gate is the scheme and nothing else.
        appearance.smartPunctuationRepeatToChinese = YES;
        appearance.pairedPunctuation = YES;
        for (NSNumber *scheme in @[ @3, @0 ]) {
            const BOOL japanese = scheme.integerValue == 3;
            [controller setValue:@{@"scheme" : scheme, @"editing_text" : @"", @"candidates" : @[]} forKey:@"view"];

            SpaceConvertClient *spaced = ClientWith(@"テスト。");
            Arm(controller, '.', spaced);
            assert([controller convertSmartPunctuationSpace:Key(@" ") client:spaced] == !japanese);
            assert([spaced.document isEqual:(japanese ? @"テスト。" : @"テスト.")]);
            assert(spaced.replacements == (japanese ? 0u : 1u));
            assert([[controller valueForKey:@"spaceConvertMark"] integerValue] == 0);

            // A committed mark does not arm the space conversion in the Japanese scheme, so the arm cannot outlive a switch back to Chinese.
            SpaceConvertClient *arming = ClientWith(@"abc！");
            [controller noteCommittedChinesePunctuation:@{@"commit" : @"！"} client:arming];
            assert([[controller valueForKey:@"spaceConvertMark"] integerValue] == (japanese ? 0 : '!'));
            [controller clearSmartPunctuationSpaceConversion];

            SpaceConvertClient *repeated = ClientWith(@"1.");
            [controller setValue:@((unichar)'.') forKey:@"lastSmartPunctuation"];
            [controller setValue:@(NSProcessInfo.processInfo.systemUptime) forKey:@"lastSmartPunctuationTime"];
            [controller setValue:@NO forKey:@"smartPunctuationRejected"];
            [controller setValue:repeated forKey:@"smartPunctuationClient"];
            assert([controller handleSmartPunctuation:Key(@".") client:repeated] == !japanese);
            assert([repeated.document isEqual:(japanese ? @"1." : @"1。")]);
            assert(repeated.replacements == (japanese ? 0u : 1u));
            [controller clearSmartPunctuationSpaceConversion];
            [controller resetSmartPunctuationState];
        }
        [controller setValue:@{@"editing_text" : @"", @"candidates" : @[]} forKey:@"view"];

        // Repeat-to-Chinese stands on its own, as in the reference, which never reads PairedPunctuation on this path: with paired punctuation off, a second , within the window still turns the direct ASCII , into ，. The repeat keys , . : never open a pair, so pairing has no say here.
        appearance.smartPunctuation = YES;
        appearance.smartPunctuationRepeatToChinese = YES;
        appearance.pairedPunctuation = NO;
        auto ArmRepeat = [&](unichar mark, id repeatClient) {
            [controller setValue:@(mark) forKey:@"lastSmartPunctuation"];
            [controller setValue:@(NSProcessInfo.processInfo.systemUptime) forKey:@"lastSmartPunctuationTime"];
            [controller setValue:@NO forKey:@"smartPunctuationRejected"];
            [controller setValue:repeatClient forKey:@"smartPunctuationClient"];
        };
        SpaceConvertClient *unpaired = ClientWith(@"1,");
        ArmRepeat(',', unpaired);
        assert([controller handleSmartPunctuation:Key(@",") client:unpaired]);
        assert([unpaired.document isEqual:@"1，"] && unpaired.replacements == 1);
        assert([[controller valueForKey:@"lastSmartPunctuation"] integerValue] == 0);
        [controller clearSmartPunctuationSpaceConversion];

        // The feature's own switch is still the gate: off, the repeat is an ordinary key and the document is left alone.
        appearance.smartPunctuationRepeatToChinese = NO;
        SpaceConvertClient *switchedOff = ClientWith(@"1,");
        ArmRepeat(',', switchedOff);
        assert(![controller handleSmartPunctuation:Key(@",") client:switchedOff]);
        assert([switchedOff.document isEqual:@"1,"] && switchedOff.replacements == 0);
        [controller clearSmartPunctuationSpaceConversion];
        [controller resetSmartPunctuationState];

        // The arm belongs to the client it was made in, matching the reference's focus check: a repeat in another client rewrites nothing.
        appearance.smartPunctuationRepeatToChinese = YES;
        SpaceConvertClient *armedRepeat = ClientWith(@"1,");
        SpaceConvertClient *otherRepeat = ClientWith(@"2,");
        ArmRepeat(',', armedRepeat);
        assert(![controller handleSmartPunctuation:Key(@",") client:otherRepeat]);
        assert([otherRepeat.document isEqual:@"2,"] && otherRepeat.replacements == 0);
        assert([armedRepeat.document isEqual:@"1,"] && armedRepeat.replacements == 0);
        [controller clearSmartPunctuationSpaceConversion];
        [controller resetSmartPunctuationState];

        // Same key after the space conversion: the ASCII character goes back to the Chinese mark it replaced (Composition.cpp Kind::AsciiConverted, _CanInterceptSmartPunctuationRevert). The whole family is covered, and the mark comes back as the one read back, so ” stays ” rather than becoming “.
        appearance.smartPunctuation = YES;
        appearance.smartPunctuationSpaceConvert = YES;
        appearance.smartPunctuationRepeatToChinese = YES;
        appearance.pairedPunctuation = NO;
        [controller setValue:@{@"editing_text" : @"", @"candidates" : @[]} forKey:@"view"];
        [controller resetSmartPunctuationState];
        [controller clearSmartPunctuationSpaceConversion];
        [controller clearSmartPunctuationSpaceRevert];
        auto Converted = [&](NSString *pair) {
            SpaceConvertClient *editor = ClientWith([@"abc" stringByAppendingString:[pair substringFromIndex:1]]);
            Arm(controller, [pair characterAtIndex:0], editor);
            assert([controller convertSmartPunctuationSpace:Key(@" ") client:editor]);
            assert(editor.replacements == 1);
            return editor;
        };
        auto Ascii = [](NSString *pair) { return [pair substringToIndex:1]; };
        for (NSString *pair in @[ @",，", @".。", @":：", @"!！", @"?？", @";；", @"/、",
                                  @"\"“", @"\"”", @"'‘", @"'’", @"[【", @"]】", @"<《",
                                  @">》", @"(（", @")）" ]) {
            SpaceConvertClient *editor = Converted(pair);
            assert([editor.document isEqual:[@"abc" stringByAppendingString:Ascii(pair)]]);
            assert([controller revertSmartPunctuationSpace:Key(Ascii(pair)) client:editor]);
            assert([editor.document isEqual:[@"abc" stringByAppendingString:[pair substringFromIndex:1]]]);
            assert(editor.replacements == 2);
        }

        // Shift is how several of these marks are typed, so it does not block the revert.
        SpaceConvertClient *shiftRevert = Converted(@";；");
        assert([controller revertSmartPunctuationSpace:KeyWithFlags(@";", NSEventModifierFlagShift) client:shiftRevert]);
        assert([shiftRevert.document isEqual:@"abc；"] && shiftRevert.replacements == 2);

        // Control, Option or Command make it a chord, not the punctuation key.
        SpaceConvertClient *chord = Converted(@",，");
        assert(![controller revertSmartPunctuationSpace:KeyWithFlags(@",", NSEventModifierFlagCommand) client:chord]);
        assert([chord.document isEqual:@"abc,"] && chord.replacements == 1);

        // Any other key disarms; the same key afterwards is an ordinary key.
        SpaceConvertClient *otherKey = Converted(@",，");
        assert(![controller revertSmartPunctuationSpace:Key(@"x") client:otherKey]);
        assert(![controller revertSmartPunctuationSpace:Key(@",") client:otherKey]);
        assert([otherKey.document isEqual:@"abc,"] && otherKey.replacements == 1);

        // One-shot: after a revert, the same key again does not revert a second time.
        SpaceConvertClient *twice = Converted(@",，");
        assert([controller revertSmartPunctuationSpace:Key(@",") client:twice]);
        assert(![controller revertSmartPunctuationSpace:Key(@",") client:twice]);
        assert([twice.document isEqual:@"abc，"] && twice.replacements == 2);

        // The arm belongs to the client the conversion happened in.
        SpaceConvertClient *revertArmed = Converted(@",，");
        SpaceConvertClient *revertOther = ClientWith(@"xyz,");
        assert(![controller revertSmartPunctuationSpace:Key(@",") client:revertOther]);
        assert([revertOther.document isEqual:@"xyz,"] && revertOther.replacements == 0);
        assert([revertArmed.document isEqual:@"abc,"] && revertArmed.replacements == 1);

        // The document is authoritative: with the caret moved off the ASCII character, nothing is rewritten.
        SpaceConvertClient *caretMoved = Converted(@",，");
        caretMoved.selection = NSMakeRange(1, 0);
        assert(![controller revertSmartPunctuationSpace:Key(@",") client:caretMoved]);
        assert([caretMoved.document isEqual:@"abc,"] && caretMoved.replacements == 1);

        // Past the 2 s repeat window the key is ordinary.
        SpaceConvertClient *late = Converted(@",，");
        [controller setValue:@(NSProcessInfo.processInfo.systemUptime - 3.0) forKey:@"spaceRevertTime"];
        assert(![controller revertSmartPunctuationSpace:Key(@",") client:late]);
        assert([late.document isEqual:@"abc,"] && late.replacements == 1);

        // A composition owns the keyboard.
        SpaceConvertClient *revertComposing = Converted(@",，");
        [controller setValue:@{@"editing_text" : @"ni", @"candidates" : @[]} forKey:@"view"];
        assert(![controller revertSmartPunctuationSpace:Key(@",") client:revertComposing]);
        assert([revertComposing.document isEqual:@"abc,"] && revertComposing.replacements == 1);
        [controller setValue:@{@"editing_text" : @"", @"candidates" : @[]} forKey:@"view"];

        // Full-width input owns the punctuation form.
        SpaceConvertClient *revertWide = Converted(@".。");
        appearance.fullWidthInput = YES;
        assert(![controller revertSmartPunctuationSpace:Key(@".") client:revertWide]);
        assert([revertWide.document isEqual:@"abc."] && revertWide.replacements == 1);
        appearance.fullWidthInput = NO;

        // A pending paired closing around the caret blocks the rewrite.
        SpaceConvertClient *revertPaired = Converted(@",，");
        [controller setValue:@"）" forKey:@"pendingPairedClosing"];
        assert(![controller revertSmartPunctuationSpace:Key(@",") client:revertPaired]);
        assert([revertPaired.document isEqual:@"abc,"] && revertPaired.replacements == 1);
        [controller setValue:nil forKey:@"pendingPairedClosing"];

        // With repeat-to-Chinese off the conversion does not arm a revert at all.
        appearance.smartPunctuationRepeatToChinese = NO;
        SpaceConvertClient *noRepeat = Converted(@",，");
        assert([[controller valueForKey:@"spaceRevertKey"] integerValue] == 0);
        assert(![controller revertSmartPunctuationSpace:Key(@",") client:noRepeat]);
        assert([noRepeat.document isEqual:@"abc,"] && noRepeat.replacements == 1);
        // Turning it off after arming also blocks the revert.
        appearance.smartPunctuationRepeatToChinese = YES;
        SpaceConvertClient *offAfter = Converted(@",，");
        appearance.smartPunctuationRepeatToChinese = NO;
        assert(![controller revertSmartPunctuationSpace:Key(@",") client:offAfter]);
        assert([offAfter.document isEqual:@"abc,"] && offAfter.replacements == 1);

        [controller clearSmartPunctuationSpaceRevert];
        TestRealKeyArming(appearance);

        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::puts("macOS smart punctuation space conversion passed.");
    return 0;
}
