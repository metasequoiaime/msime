// The readings drawn after gloss lines: which line is in which language, which texts go to the shared English table,
// romaji from the system tokenizer, and the display string whose reading ranges and line starts the button draws from.
#import "../../src/candidate/CandidatePronunciation.h"
#include <cstdio>
#include <cstdlib>

static int failures = 0;
static void Check(BOOL condition, const char *message)
{
    if (!condition) {
        std::fprintf(stderr, "FAIL %s\n", message);
        ++failures;
    }
}

int main()
{
    @autoreleasepool {
        Check([MSIMEGlossFirstTerm(@"love; affection") isEqual:@"love"], "first term before ';'");
        Check([MSIMEGlossFirstTerm(@"愛する, 恋する; 愛") isEqual:@"愛する"], "first term before ','");
        Check([MSIMEGlossFirstTerm(@"  学校  ") isEqual:@"学校"], "first term trimmed");
        Check([MSIMEGlossFirstTerm(@"") isEqual:@""], "empty line has no term");

        NSArray *enJa = @[ @"en", @"ja" ];
        Check([MSIMEGlossLineLanguage(@"love; affection", 0, enJa) isEqual:@"en"], "letters are English");
        Check([MSIMEGlossLineLanguage(@"愛する, 恋する", 1, enJa) isEqual:@"ja"], "kana is Japanese");
        Check([MSIMEGlossLineLanguage(@"学校; 大学", 1, enJa) isEqual:@"ja"], "kanji-only follows the Japanese target");
        Check(MSIMEGlossLineLanguage(@"学校; 大学", 0, enJa) == nil, "kanji on a non-Japanese target is not guessed");
        Check(MSIMEGlossLineLanguage(@"chat, chatte", 0, @[ @"fr" ]) == nil, "French letters are not read as English");
        Check(MSIMEGlossLineLanguage(@"chat, chatte", 0, @[ @"fr", @"en" ]) == nil, "only the English target's line is English");
        Check(MSIMEGlossLineLanguage(@"", 0, enJa) == nil, "an empty line has no language");
        Check(MSIMEGlossLineLanguage(@"你好; 您好", 0, enJa) == nil, "a Chinese gloss has no reading of its own");

        // Romaji comes from the system tokenizer, kanji included.
        Check([MSIMEJapaneseRomaji(@"愛する") isEqual:@"aisuru"], "愛する -> aisuru");
        Check([MSIMEJapaneseRomaji(@"学校") isEqual:@"gakkou"], "学校 -> gakkou");
        Check([MSIMEJapaneseRomaji(@"ありがとう") isEqual:@"arigatou"], "kana -> romaji");
        Check([MSIMEJapaneseRomaji(@"") isEqual:@""], "empty -> empty");
        // Words are spaced so a learner can match them; particles and greetings read as spoken.
        Check([MSIMEJapaneseRomaji(@"今日は天気がいいですね") isEqual:@"kyou wa tenki ga ii desu ne"], "a sentence is spaced by word");
        Check([MSIMEJapaneseRomaji(@"東京へ行きます") isEqual:@"toukyou e iki masu"], "the particle へ reads e");
        Check([MSIMEJapaneseRomaji(@"私を") isEqual:@"watakushi o"], "the particle を reads o");
        Check([MSIMEJapaneseRomaji(@"こんにちは") isEqual:@"konnichiwa"], "a greeting's final は reads wa");
        Check([MSIMEJapaneseRomaji(@"はな") isEqual:@"hana"], "は inside a word keeps ha");

        NSDictionary *english = @{ @"love; affection" : @"/lʌv/", @"hello" : @"/həˈləu/" };
        // A Chinese candidate with an English and a Japanese line: one reading per line.
        Check([MSIMEGlossPronunciation(@"爱", @"love; affection\n愛する, 恋する", enJa, english, nil) isEqual:@"/lʌv/\naisuru"],
              "Chinese candidate reads both lines");
        // An English candidate's English line is its Chinese gloss; the reading is of the candidate itself.
        Check([MSIMEGlossPronunciation(@"hello", @"你好; 您好", @[ @"en" ], english, nil) isEqual:@"/həˈləu/"],
              "English candidate reads its own text");
        // Unknown English: nothing, and no reading at all is an empty string rather than a blank line.
        Check([MSIMEGlossPronunciation(@"天", @"sky; heaven", @[ @"en" ], english, nil) isEqual:@""], "unknown English is empty");
        Check([MSIMEGlossPronunciation(@"天", @"", @[ @"en" ], english, nil) isEqual:@""], "no gloss, no reading");
        // A missing first line keeps the second aligned.
        Check([MSIMEGlossPronunciation(@"猫", @"\n猫", enJa, english, ^NSString *(NSString *term) { return [term isEqual:@"猫"] ? @"neko" : @""; })
                  isEqual:@"\nneko"],
              "readings stay aligned with their lines");

        NSArray *texts = MSIMEGlossEnglishTexts(@"爱", @"love; affection\n愛する", enJa);
        Check(texts.count == 1 && [texts[0] isEqual:@"love; affection"], "English gloss lines are asked for");
        texts = MSIMEGlossEnglishTexts(@"hello", @"你好", @[ @"en" ]);
        Check(texts.count == 1 && [texts[0] isEqual:@"hello"], "an English candidate asks for itself");
        Check(MSIMEGlossEnglishTexts(@"爱", @"", enJa).count == 0, "no gloss asks for nothing");

        NSMutableArray<NSValue *> *ranges = [NSMutableArray array];
        NSMutableArray<NSNumber *> *starts = [NSMutableArray array];
        NSString *display = MSIMECandidateGlossDisplay(@"love; affection\n愛する", @"/lʌv/\naisuru", nil, ranges, starts);
        Check([display isEqual:@"love; affection  /lʌv/\n愛する  aisuru"], "display appends each reading to its line");
        Check(ranges.count == 2 && [[display substringWithRange:ranges[0].rangeValue] isEqual:@"/lʌv/"] &&
                  [[display substringWithRange:ranges[1].rangeValue] isEqual:@"aisuru"],
              "reading ranges cover exactly the readings");
        Check(starts.count == 2 && starts[0].unsignedIntegerValue == 0 &&
                  [[display substringFromIndex:starts[1].unsignedIntegerValue] hasPrefix:@"愛する"],
              "line starts point at each gloss line");
        [ranges removeAllObjects];
        [starts removeAllObjects];
        Check([MSIMECandidateGlossDisplay(@"sky", @"", nil, ranges, starts) isEqual:@"sky"] && ranges.count == 0,
              "no reading leaves the gloss as it is");
        Check([MSIMECandidateGlossDisplay(@"sky", @"/skai/", nil, nil, nil) isEqual:@"sky  /skai/"], "ranges are optional");
        [ranges removeAllObjects];
        [starts removeAllObjects];
        display = MSIMECandidateGlossDisplay(@"I like you", @"/aɪ laɪk juː/", @"我 I · 喜欢 to like · 你 you", ranges, starts);
        Check([display isEqual:@"I like you  /aɪ laɪk juː/\n我 I · 喜欢 to like · 你 you"], "the breakdown takes a line of its own after the glosses");
        Check(ranges.count == 2 && [[display substringWithRange:ranges[1].rangeValue] isEqual:@"我 I · 喜欢 to like · 你 you"],
              "the breakdown range covers the breakdown");
        Check(starts.count == 1, "the breakdown is not a gloss column");
        Check([MSIMECandidateGlossDisplay(@"", @"", @"我 I · 你 you", nil, nil) isEqual:@"我 I · 你 you"],
              "a breakdown shows without any gloss");
    }
    if (failures) std::fprintf(stderr, "%d failure(s)\n", failures);
    return failures ? EXIT_FAILURE : EXIT_SUCCESS;
}
