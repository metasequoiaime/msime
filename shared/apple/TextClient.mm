#import "TextClient.h"

#if TARGET_OS_OSX
// NSUnderlineStyle* and the marked clause attribute are AppKit's, not Foundation's.
#import <AppKit/AppKit.h>
#endif

static BOOL MSIMEPreeditSeparator(unichar character) {
    return character == '\'' || character == ' ';
}

static BOOL MSIMEStrictCaretPosition(id value, NSUInteger *result) {
    if (![value isKindOfClass:NSNumber.class] ||
        CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID() ||
        CFNumberIsFloatType((__bridge CFNumberRef)value)) return NO;
    NSNumber *number = (NSNumber *)value;
    if ([number compare:@0] == NSOrderedAscending) return NO;
    uint64_t position = number.unsignedLongLongValue;
    if ([number compare:@(position)] != NSOrderedSame || position > NSUIntegerMax) return NO;
    if (result) *result = (NSUInteger)position;
    return YES;
}

static NSString *MSIMEPreeditLetters(NSString *text) {
    NSMutableString *letters = [NSMutableString string];
    for (NSUInteger i = 0; i < text.length; ++i) {
        unichar character = [text characterAtIndex:i];
        if (MSIMEPreeditSeparator(character)) continue;
        if (!((character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z'))) return nil;
        [letters appendString:[text substringWithRange:NSMakeRange(i, 1)]];
    }
    return letters;
}

NSUInteger MSIMEPreeditCaretPosition(NSString *editing, NSString *preedit, id position) {
    NSUInteger rawCaret = 0;
    if (!MSIMEStrictCaretPosition(position, &rawCaret)) return preedit.length;
    rawCaret = MIN(rawCaret, editing.length);
    if ([preedit isEqual:editing]) return rawCaret;
    // Only map lossless separator formatting. Expanded shuangpin, converted words
    // and corrections need an Engine-provided offset map, not host-side guesses.
    NSString *letters = MSIMEPreeditLetters(editing);
    if (!letters.length || ![letters isEqual:MSIMEPreeditLetters(preedit)]) return preedit.length;
    if (rawCaret == editing.length) return preedit.length;
    NSUInteger remaining = 0;
    for (NSUInteger i = 0; i < rawCaret; ++i)
        if (!MSIMEPreeditSeparator([editing characterAtIndex:i])) ++remaining;
    NSUInteger displayCaret = 0;
    while (displayCaret < preedit.length && remaining) {
        if (!MSIMEPreeditSeparator([preedit characterAtIndex:displayCaret])) --remaining;
        ++displayCaret;
    }
    // Preserve the two sides of an explicitly typed syllable separator, matching
    // Windows GetPreeditWithCaretMarker at the pinned product reference.
    if (rawCaret && MSIMEPreeditSeparator([editing characterAtIndex:rawCaret - 1]))
        while (displayCaret < preedit.length && MSIMEPreeditSeparator([preedit characterAtIndex:displayCaret])) ++displayCaret;
    return displayCaret;
}

NSString *MSIMETextClientFollowingCharacter(id<MSIMETextClient> client) {
    if (!client || ![client respondsToSelector:@selector(selectedRange)] ||
        ![client respondsToSelector:@selector(attributedSubstringFromRange:)]) return nil;
    NSRange selected = [client selectedRange];
    if (selected.location == NSNotFound || selected.length != 0) return nil;
    NSAttributedString *substring = [client attributedSubstringFromRange:NSMakeRange(selected.location, 1)];
    NSString *text = substring.string;
    return text.length == 1 ? [text substringWithRange:NSMakeRange(0, 1)] : nil;
}

uint32_t MSIMETextClientPrecedingUnicodeScalar(id<MSIMETextClient> client) {
    if (!client || ![client respondsToSelector:@selector(selectedRange)] ||
        ![client respondsToSelector:@selector(attributedSubstringFromRange:)]) return 0;
    NSRange selected = [client selectedRange];
    if (selected.location == NSNotFound || selected.length != 0 || selected.location == 0) return 0;
    NSUInteger start = selected.location - 1;
    NSAttributedString *one = [client attributedSubstringFromRange:NSMakeRange(start, 1)];
    NSString *text = one.string;
    if (!text.length) return 0;
    unichar tail = [text characterAtIndex:text.length - 1];
    if (tail >= 0xDC00 && tail <= 0xDFFF && start > 0) {
        NSAttributedString *pair = [client attributedSubstringFromRange:NSMakeRange(start - 1, 2)];
        NSString *pairText = pair.string;
        if (pairText.length == 2) {
            unichar high = [pairText characterAtIndex:0];
            if (high >= 0xD800 && high <= 0xDBFF)
                return CFStringGetLongCharacterForSurrogatePair(high, tail);
        }
    }
    return tail;
}

void MSIMEApplyTransitionWithPreeditStyle(NSDictionary *transition, id<MSIMETextClient> client,
                                          MSIMEInlinePreeditStyle style) {
    MSIMEApplyTransitionWithPendingClosing(transition, client, style, nil);
}

void MSIMEApplyTransitionWithPendingClosing(NSDictionary *transition, id<MSIMETextClient> client,
                                            MSIMEInlinePreeditStyle style, NSString *closing) {
    MSIMEApplyTransitionTrackingMarkedText(transition, client, style, closing, NULL);
}

void MSIMEApplyTransitionTrackingMarkedText(NSDictionary *transition, id<MSIMETextClient> client,
                                            MSIMEInlinePreeditStyle style, NSString *closing,
                                            BOOL *clientHasMarkedText) {
    if (!closing.length) closing = nil;
    id commit = transition[@"commit"];
    // A commit ends the pair: the closing mark goes in with the text it was holding open, and the
    // caret lands after it, where the next character belongs.
    if ([commit isKindOfClass:NSString.class] && closing)
        commit = [(NSString *)commit stringByAppendingString:closing];
    if ([commit isKindOfClass:NSString.class]) [client insertText:commit replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    NSDictionary *view = transition[@"view"];
    if (![view isKindOfClass:NSDictionary.class]) return;
    NSString *editing = view[@"editing_text"];
    if (![editing isKindOfClass:NSString.class]) editing = @"";
    NSString *preedit = view[@"preedit"];
    if (![preedit isKindOfClass:NSString.class]) preedit = editing;
    id position = view[@"caret_position"];
    // A Japanese composition is かな, not romaji, and a Korean one is Hangul, not the Dubeolsik key letters in `editing_text`.
    //
    // The Engine hands over both - `editing_text` is the letters that were typed and `reading` the
    // kana they convert to - and every Japanese input method shows the kana: it is what the user
    // means, what the candidates are for, and what Enter commits here. Showing the letters instead
    // leaves the composition saying `nihon` while the commit says にほん.
    //
    // The exception is a caret the user has moved into the middle of the letters. The Engine's
    // offset is into the romaji and there is no map from it into the kana - the same reason
    // MSIMEPreeditCaretPosition refuses to guess for shuangpin - so rather than draw the caret in
    // the wrong place, that case keeps showing what the caret belongs to. Typing never reaches it:
    // the caret sits at the end until an arrow key moves it.
    NSString *reading = view[@"reading"];
    NSUInteger rawCaret = 0;
    BOOL validCaret = MSIMEStrictCaretPosition(position, &rawCaret);
    if ([reading isKindOfClass:NSString.class] && reading.length &&
        (!validCaret || rawCaret >= editing.length)) {
        editing = reading;
        preedit = reading;
        position = @(reading.length);
        rawCaret = reading.length;
        validCaret = YES;
    }
    NSString *marked = preedit;
    NSUInteger caret = MSIMEPreeditCaretPosition(editing, preedit, position);
    if (style == MSIMEInlinePreeditStyleRaw) {
        marked = editing;
        caret = validCaret ? MIN(rawCaret, editing.length) : editing.length;
    } else if (style == MSIMEInlinePreeditStyleEmpty) {
        marked = @"";
        caret = 0;
    }
    // A phrase being put together out of several selections keeps the part already chosen in the
    // composition instead of sending it to the document. It leads the marked text and the caret
    // moves past it, the way the reference prepends word_for_creating_word to the reading and
    // offsets the display caret by its length. The runtime hands it over separately because
    // caret_position is an offset into the editing text in this host's own string unit.
    NSString *phrase = view[@"phrase_prefix"];
    NSUInteger phraseLength = 0;
    if ([phrase isKindOfClass:NSString.class] && phrase.length && style != MSIMEInlinePreeditStyleEmpty) {
        marked = [phrase stringByAppendingString:marked];
        caret += phrase.length;
        phraseLength = phrase.length;
    }
    // While the pair is open the closing mark is the tail of the marked text, so it stays visible and
    // stays after the caret. A commit above has already consumed it.
    if (closing && ![commit isKindOfClass:NSString.class]) marked = [marked stringByAppendingString:closing];
    id displayed = marked;
#if TARGET_OS_OSX
    // Two clauses, drawn differently, once a phrase is being assembled: the piece already chosen is
    // settled and the reading after it is still being typed. The reference draws the same
    // distinction with TSF display attributes - its input clause carries a dotted underline and its
    // converted clause none - and the macOS convention for it is the other way round in weight: the
    // settled clause keeps a thin underline and the clause still being worked on a thick one, which
    // is what every Japanese input method here does. Without this the two run together as one
    // stretch of underlined text and nothing says where what the user already chose ends.
    //
    // AppKit only; UIKit's document proxy takes a plain string, and no UIKit host holds a phrase.
    if (phraseLength > 0 && phraseLength < marked.length) {
        NSMutableAttributedString *clauses = [[NSMutableAttributedString alloc] initWithString:marked];
        [clauses addAttributes:@{NSUnderlineStyleAttributeName: @(NSUnderlineStyleSingle),
                                 NSMarkedClauseSegmentAttributeName: @0}
                         range:NSMakeRange(0, phraseLength)];
        [clauses addAttributes:@{NSUnderlineStyleAttributeName: @(NSUnderlineStyleThick),
                                 NSMarkedClauseSegmentAttributeName: @1}
                         range:NSMakeRange(phraseLength, marked.length - phraseLength)];
        displayed = clauses;
    }
#endif
    // Only the clear of a composition that is not there is skipped; after a commit the clear still goes out, as it always has.
    if (clientHasMarkedText && !*clientHasMarkedText && !marked.length && ![commit isKindOfClass:NSString.class]) return;
    // Recorded before the write: IMK can service the next key inside it, and that nested write lands in the client after this one, so it must also be the one whose state is left recorded.
    if (clientHasMarkedText) *clientHasMarkedText = marked.length > 0;
    [client setMarkedText:displayed selectionRange:NSMakeRange(caret, 0) replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
}

void MSIMEApplyTransition(NSDictionary *transition, id<MSIMETextClient> client) {
    MSIMEApplyTransitionWithPreeditStyle(transition, client, MSIMEInlinePreeditStylePinyin);
}
