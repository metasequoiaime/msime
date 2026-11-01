// 画在释义行后面的读音：哪一行是哪种语言、哪些文字交给共享表、系统分词器给的罗马字，以及按钮据以绘制读音范围和行首的显示字符串。
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
        Check([MSIMEGlossLineLanguage(@"chat, chatte", 0, @[ @"fr" ]) isEqual:@"fr"], "French letters are read as French, not English");
        Check([MSIMEGlossLineLanguage(@"chat, chatte", 0, @[ @"fr", @"en" ]) isEqual:@"fr"], "only the English target's line is English");
        Check([MSIMEGlossLineLanguage(@"кошка, кот", 1, @[ @"en", @"ru" ]) isEqual:@"ru"], "a line follows its own target");
        Check([MSIMEGlossLineLanguage(@"고양이", 0, @[ @"ko" ]) isEqual:@"ko"], "Korean is read from the Korean table");
        Check(MSIMEGlossLineLanguage(@"chat", 0, @[ @"zh" ]) == nil, "a target without a table is not read");
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

        NSDictionary *english = @{
            MSIMEPronunciationKey(@"en", @"love; affection") : @"/lʌv/",
            MSIMEPronunciationKey(@"en", @"hello") : @"/həˈləu/",
            MSIMEPronunciationKey(@"fr", @"chat, chatte; félin") : @"/ʃa/",
            MSIMEPronunciationKey(@"de", @"Katze") : @"/ˈkatsə/",
        };
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

        // 法文和德文行从各自的表里读；同样的字母放在英文那一行就按英文读。
        Check([MSIMEGlossPronunciation(@"猫", @"chat, chatte; félin\nKatze", @[ @"fr", @"de" ], english, nil) isEqual:@"/ʃa/\n/ˈkatsə/"],
              "each target line reads from its own language");
        Check([MSIMEGlossPronunciation(@"猫", @"chat, chatte; félin", @[ @"en" ], english, nil) isEqual:@""],
              "a French answer is not an English one");

        NSArray<NSDictionary *> *items = MSIMEGlossSharedItems(@"爱", @"love; affection\n愛する", enJa);
        Check(items.count == 1 && [items[0] isEqual:(@{ @"text" : @"love; affection", @"language" : @"en" })], "English gloss lines are asked for");
        items = MSIMEGlossSharedItems(@"hello", @"你好", @[ @"en" ]);
        Check(items.count == 1 && [items[0] isEqual:(@{ @"text" : @"hello", @"language" : @"en" })], "an English candidate asks for itself");
        items = MSIMEGlossSharedItems(@"猫", @"chat\n猫\nKatze", @[ @"fr", @"ja", @"de" ]);
        Check(items.count == 2 && [items[0] isEqual:(@{ @"text" : @"chat", @"language" : @"fr" })] &&
                  [items[1] isEqual:(@{ @"text" : @"Katze", @"language" : @"de" })],
              "other target lines are asked for in their language; Japanese is read locally");
        Check(MSIMEGlossSharedItems(@"爱", @"", enJa).count == 0, "no gloss asks for nothing");

        NSMutableArray<NSValue *> *ranges = [NSMutableArray array];
        NSMutableArray<NSNumber *> *starts = [NSMutableArray array];
        NSString *display = MSIMECandidateGlossDisplay(@"love; affection\n愛する", @"/lʌv/\naisuru", ranges, starts);
        Check([display isEqual:@"love; affection\n/lʌv/\n愛する\naisuru"], "each reading takes its own line under its gloss line");
        Check(ranges.count == 2 && [[display substringWithRange:ranges[0].rangeValue] isEqual:@"/lʌv/"] &&
                  [[display substringWithRange:ranges[1].rangeValue] isEqual:@"aisuru"],
              "reading ranges cover exactly the readings");
        Check(starts.count == 2 && starts[0].unsignedIntegerValue == 0 &&
                  [[display substringFromIndex:starts[1].unsignedIntegerValue] hasPrefix:@"愛する"],
              "line starts point at each gloss line");
        [ranges removeAllObjects];
        [starts removeAllObjects];
        Check([MSIMECandidateGlossDisplay(@"sky", @"", ranges, starts) isEqual:@"sky"] && ranges.count == 0,
              "no reading leaves the gloss as it is");
        Check([MSIMECandidateGlossDisplay(@"sky", @"/skai/", nil, nil) isEqual:@"sky\n/skai/"], "ranges are optional");
        Check([MSIMECandidateGlossDisplay(@"sky\n\nsora", @"/skai/\n\nsora", nil, nil) isEqual:@"sky\n/skai/\n\nsora\nsora"],
              "an empty target line stays empty and gets no reading line");
    }
    if (failures) std::fprintf(stderr, "%d failure(s)\n", failures);
    return failures ? EXIT_FAILURE : EXIT_SUCCESS;
}
