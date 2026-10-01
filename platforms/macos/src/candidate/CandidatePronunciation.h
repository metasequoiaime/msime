#pragma once

#import <Foundation/Foundation.h>

// How the gloss lines under a candidate are read, drawn after each line and never committed.
//
// A gloss is one line per translation target, joined with "\n" in target order (MSIMEJoinedTranslations keeps the
// empty lines in between). Each line is pronounced by its first term: English through the shared offline table
// (msime_client_pronunciation_request), Japanese as romaji from the system tokenizer, which reads kanji as well as kana.
// Nothing else is pronounced; a line without a pronunciation is left as it is.

// The part of a gloss line before its first separator, which is the word the line teaches.
static inline NSString *MSIMEGlossFirstTerm(NSString *line)
{
    if (line.length == 0) return @"";
    NSCharacterSet *separators = [NSCharacterSet characterSetWithCharactersInString:@";；,，、/"];
    const NSRange range = [line rangeOfCharacterFromSet:separators];
    NSString *term = range.location == NSNotFound ? line : [line substringToIndex:range.location];
    return [term stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceCharacterSet];
}

static inline BOOL MSIMEContainsKana(NSString *text)
{
    for (NSUInteger index = 0; index < text.length; ++index) {
        const unichar character = [text characterAtIndex:index];
        if ((character >= 0x3041 && character <= 0x309F) || (character >= 0x30A0 && character <= 0x30FF)) return YES;
    }
    return NO;
}

// Plain English: ASCII letters with the spaces, hyphens and apostrophes words carry, and at least one letter.
static inline BOOL MSIMEIsPlainEnglish(NSString *text)
{
    BOOL letter = NO;
    for (NSUInteger index = 0; index < text.length; ++index) {
        const unichar character = [text characterAtIndex:index];
        if ((character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z')) letter = YES;
        else if (character != ' ' && character != '-' && character != '\'') return NO;
    }
    return letter;
}

// Which language line `index` of a gloss is in, for reading it: the line's target, checked against the text. Kana is
// Japanese wherever it appears. A Latin line is English only on the English target — French "chat" is plain letters too,
// and reading it as English would teach the wrong sound — and a kanji-only line is Japanese only on the Japanese target.
static inline NSString *MSIMEGlossLineLanguage(NSString *line, NSUInteger index, NSArray<NSString *> *targets)
{
    NSString *term = MSIMEGlossFirstTerm(line);
    if (term.length == 0) return nil;
    if (MSIMEContainsKana(term)) return @"ja";
    NSString *target = index < targets.count ? targets[index] : nil;
    if ([target isEqual:@"en"]) return MSIMEIsPlainEnglish(term) ? @"en" : nil;
    return [target isEqual:@"ja"] ? @"ja" : nil;
}

// Romaji of Japanese text, word by word from the system tokenizer, one space between words: 今日は天気がいいですね reads
// "kyou wa tenki ga ii desu ne", which a learner can match to the words; run together it cannot be read. The particles
// は, へ and を are read wa, e and o, as they are spoken, and so is the は that ends a greeting such as こんにちは. Empty
// when a token has no Latin transcription (the text is not Japanese the tokenizer knows): half a reading is not shown.
static inline NSString *MSIMEJapaneseRomaji(NSString *text)
{
    if (text.length == 0) return @"";
    static NSDictionary<NSString *, NSString *> *spoken = @{
        @"は" : @"wa", @"へ" : @"e", @"を" : @"o",
        @"こんにちは" : @"konnichiwa", @"こんばんは" : @"konbanwa",
    };
    CFLocaleRef locale = CFLocaleCreate(kCFAllocatorDefault, CFSTR("ja"));
    CFStringTokenizerRef tokenizer = CFStringTokenizerCreate(kCFAllocatorDefault, (__bridge CFStringRef)text,
                                                             CFRangeMake(0, (CFIndex)text.length), kCFStringTokenizerUnitWord, locale);
    if (locale) CFRelease(locale);
    if (!tokenizer) return @"";
    NSMutableArray<NSString *> *words = [NSMutableArray array];
    BOOL complete = YES;
    while (CFStringTokenizerAdvanceToNextToken(tokenizer) != kCFStringTokenizerTokenNone) {
        const CFRange range = CFStringTokenizerGetCurrentTokenRange(tokenizer);
        NSString *surface = [text substringWithRange:NSMakeRange((NSUInteger)range.location, (NSUInteger)range.length)];
        if (spoken[surface]) {
            [words addObject:spoken[surface]];
            continue;
        }
        CFTypeRef latin = CFStringTokenizerCopyCurrentTokenAttribute(tokenizer, kCFStringTokenizerAttributeLatinTranscription);
        if (latin && CFGetTypeID(latin) == CFStringGetTypeID() && CFStringGetLength((CFStringRef)latin) > 0)
            [words addObject:(__bridge NSString *)latin];
        else
            complete = NO;
        if (latin) CFRelease(latin);
    }
    CFRelease(tokenizer);
    return complete ? [words componentsJoinedByString:@" "] : @"";
}

// The per-line pronunciation of a candidate's gloss, "\n"-joined parallel to its lines, or empty when no line has one.
// `english` maps a text sent to the shared table to its "/…/" answer; English candidates are pronounced by their own
// text on the English line, since that line is their Chinese gloss. `romaji` reads a Japanese term (a caching wrapper
// of MSIMEJapaneseRomaji); nil reads it directly.
static inline NSString *MSIMEGlossPronunciation(NSString *candidateText, NSString *translation, NSArray<NSString *> *targets,
                                                NSDictionary<NSString *, NSString *> *english,
                                                NSString *(^romaji)(NSString *term))
{
    if (translation.length == 0) return @"";
    NSArray<NSString *> *lines = [translation componentsSeparatedByString:@"\n"];
    NSMutableArray<NSString *> *readings = [NSMutableArray arrayWithCapacity:lines.count];
    BOOL any = NO;
    const BOOL englishCandidate = MSIMEIsPlainEnglish(candidateText);
    for (NSUInteger index = 0; index < lines.count; ++index) {
        NSString *line = lines[index];
        NSString *reading = @"";
        NSString *language = MSIMEGlossLineLanguage(line, index, targets);
        if (englishCandidate && line.length && (index < targets.count ? [targets[index] isEqual:@"en"] : index == 0))
            reading = english[candidateText] ?: @"";
        else if ([language isEqual:@"en"])
            reading = english[line] ?: @"";
        else if ([language isEqual:@"ja"])
            reading = (romaji ? romaji(MSIMEGlossFirstTerm(line)) : MSIMEJapaneseRomaji(MSIMEGlossFirstTerm(line))) ?: @"";
        any = any || reading.length;
        [readings addObject:reading];
    }
    return any ? [readings componentsJoinedByString:@"\n"] : @"";
}

// The English texts a candidate needs the shared table for: its own text when it is an English candidate, otherwise its
// English gloss lines. Sent as whole lines; the shared side picks the first term.
static inline NSArray<NSString *> *MSIMEGlossEnglishTexts(NSString *candidateText, NSString *translation, NSArray<NSString *> *targets)
{
    NSMutableArray<NSString *> *texts = [NSMutableArray array];
    if (translation.length == 0) return texts;
    if (MSIMEIsPlainEnglish(candidateText)) {
        [texts addObject:candidateText];
        return texts;
    }
    NSArray<NSString *> *lines = [translation componentsSeparatedByString:@"\n"];
    for (NSUInteger index = 0; index < lines.count; ++index)
        if ([MSIMEGlossLineLanguage(lines[index], index, targets) isEqual:@"en"]) [texts addObject:lines[index]];
    return texts;
}

// The gloss as drawn: each line followed by its reading. The reading ranges are reported so they can be styled apart,
// and `lineStarts` gives where each gloss line begins in the result, for underlining an armed column.
static inline NSString *MSIMECandidateGlossDisplay(NSString *translation, NSString *pronunciation,
                                                   NSMutableArray<NSValue *> *readingRanges,
                                                   NSMutableArray<NSNumber *> *lineStarts)
{
    NSArray<NSString *> *lines = [translation ?: @"" componentsSeparatedByString:@"\n"];
    NSArray<NSString *> *readings = pronunciation.length ? [pronunciation componentsSeparatedByString:@"\n"] : @[];
    NSMutableString *display = [NSMutableString string];
    for (NSUInteger index = 0; index < lines.count; ++index) {
        if (index) [display appendString:@"\n"];
        [lineStarts addObject:@(display.length)];
        [display appendString:lines[index]];
        NSString *reading = index < readings.count ? readings[index] : @"";
        if (lines[index].length && reading.length) {
            [display appendString:@"  "];
            [readingRanges addObject:[NSValue valueWithRange:NSMakeRange(display.length, reading.length)]];
            [display appendString:reading];
        }
    }
    return display;
}
