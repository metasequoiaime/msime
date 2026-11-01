#pragma once

#import <Foundation/Foundation.h>

// How the gloss lines under a candidate are read, drawn after each line and never committed.
//
// 释义每个翻译目标一行，按目标顺序用 "\n" 连起来（MSIMEJoinedTranslations 保留中间的空行）。每一行按它的第一个词条读：英、法、西、俄、德、韩交给共享的离线读音表（msime_client_pronunciation_request，英、法、西、俄、德回答 IPA，韩文回答罗马字），日文用系统分词器读成罗马字，汉字和假名都能读。没有读音的行保持原样。

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

// 除英文外，共享表还能读哪些目标语言的释义行（target-pronunciations/<language>-phonetic.db）。
static inline BOOL MSIMEIsSharedReadingTarget(NSString *target)
{
    return [@[ @"fr", @"es", @"ru", @"de", @"ko" ] containsObject:target ?: @""];
}

// 释义第 `index` 行按哪种语言读：这一行的目标语言，再对照文字核对。假名不论出现在哪一行都是日文。拉丁字母的行只有在英语目标上才是英文：法文的 "chat" 也是普通字母，按英文读会教错音；只有汉字的行只有在日语目标上才是日文。法、西、俄、德、韩目标上的行就是那种语言：它的表里只有那种语言的词，不是其中之一的行只是没有读音。
static inline NSString *MSIMEGlossLineLanguage(NSString *line, NSUInteger index, NSArray<NSString *> *targets)
{
    NSString *term = MSIMEGlossFirstTerm(line);
    if (term.length == 0) return nil;
    if (MSIMEContainsKana(term)) return @"ja";
    NSString *target = index < targets.count ? targets[index] : nil;
    if ([target isEqual:@"en"]) return MSIMEIsPlainEnglish(term) ? @"en" : nil;
    if (MSIMEIsSharedReadingTarget(target)) return target;
    return [target isEqual:@"ja"] ? @"ja" : nil;
}

// 共享表的回答存放用的键：查询的语言和文字。同样的字母在别的语言里是另一个词（法文的 "chat"），释义行里也不会有换行。
static inline NSString *MSIMEPronunciationKey(NSString *language, NSString *text)
{
    return [NSString stringWithFormat:@"%@\n%@", language, text];
}

// 日文的罗马字，由系统分词器逐词给出，词与词之间一个空格：今日は天気がいいですね 读作 "kyou wa tenki ga ii desu ne"，学习者能和词一一对上，连在一起就没法读了。助词 は、へ、を 按实际发音读作 wa、e、o，こんにちは 这类问候语末尾的 は 也一样。某个词没有拉丁转写时（不是分词器认识的日文）返回空串：不显示半截读音。
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

// 候选释义逐行的读音，与各行一一对应、用 "\n" 连起来；没有任何一行有读音时为空。`shared` 把交给共享表的文字的 MSIMEPronunciationKey(language, text) 映射到它的回答；英文候选在英文那一行读它自己的文字，因为那一行是它的中文释义。`romaji` 读一个日文词条（MSIMEJapaneseRomaji 加缓存的包装）；为 nil 时直接读。
static inline NSString *MSIMEGlossPronunciation(NSString *candidateText, NSString *translation, NSArray<NSString *> *targets,
                                                NSDictionary<NSString *, NSString *> *shared,
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
            reading = shared[MSIMEPronunciationKey(@"en", candidateText)] ?: @"";
        else if ([language isEqual:@"ja"])
            reading = (romaji ? romaji(MSIMEGlossFirstTerm(line)) : MSIMEJapaneseRomaji(MSIMEGlossFirstTerm(line))) ?: @"";
        else if (language)
            reading = shared[MSIMEPronunciationKey(language, line)] ?: @"";
        any = any || reading.length;
        [readings addObject:reading];
    }
    return any ? [readings componentsJoinedByString:@"\n"] : @"";
}

// 一个候选要向共享表查的内容，即 {text, language} 请求项：英文候选是它自己的文字（按英文），否则是英、法、西、俄、德、韩的每一行释义。整行发送，由共享层取第一个词条。日文行改由系统分词器读。
static inline NSArray<NSDictionary<NSString *, NSString *> *> *MSIMEGlossSharedItems(NSString *candidateText, NSString *translation,
                                                                                 NSArray<NSString *> *targets)
{
    NSMutableArray<NSDictionary<NSString *, NSString *> *> *items = [NSMutableArray array];
    if (translation.length == 0) return items;
    if (MSIMEIsPlainEnglish(candidateText)) {
        [items addObject:@{ @"text" : candidateText, @"language" : @"en" }];
        return items;
    }
    NSArray<NSString *> *lines = [translation componentsSeparatedByString:@"\n"];
    for (NSUInteger index = 0; index < lines.count; ++index) {
        NSString *language = MSIMEGlossLineLanguage(lines[index], index, targets);
        if (language && ![language isEqual:@"ja"]) [items addObject:@{ @"text" : lines[index], @"language" : language }];
    }
    return items;
}

// The gloss as drawn: each line, then its reading on a line of its own under it. The reading ranges are reported so they can be styled apart, and `lineStarts` gives where each gloss line begins in the result, for underlining an armed column.
//
// 读音单独占一行，不再接在释义后面：英文释义加上 IPA 常常比候选本身宽出一倍，横排时整格跟着这一行变宽，读音的末尾紧挨下一格的释义，读起来像压在了相邻候选上；格子被挤窄时这一行又在音标中间折开。单独一行后格宽取释义和读音里较宽的一行，没有读音的页高度不变。
static inline NSString *MSIMECandidateGlossDisplay(NSString *translation, NSString *pronunciation,
                                                   NSMutableArray<NSValue *> *readingRanges,
                                                   NSMutableArray<NSNumber *> *lineStarts)
{
    NSArray<NSString *> *lines = translation.length ? [translation componentsSeparatedByString:@"\n"] : @[];
    NSArray<NSString *> *readings = pronunciation.length ? [pronunciation componentsSeparatedByString:@"\n"] : @[];
    NSMutableString *display = [NSMutableString string];
    for (NSUInteger index = 0; index < lines.count; ++index) {
        if (index) [display appendString:@"\n"];
        [lineStarts addObject:@(display.length)];
        [display appendString:lines[index]];
        NSString *reading = index < readings.count ? readings[index] : @"";
        if (lines[index].length && reading.length) {
            [display appendString:@"\n"];
            [readingRanges addObject:[NSValue valueWithRange:NSMakeRange(display.length, reading.length)]];
            [display appendString:reading];
        }
    }
    return display;
}
