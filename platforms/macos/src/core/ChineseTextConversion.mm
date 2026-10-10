#import "ChineseTextConversion.h"

#include "../../../common/ChineseTextConversion.h"

NSString *MetasequoiaChineseOutputString(NSString *text, BOOL traditionalOutput) {
    if (!traditionalOutput || text.length == 0) return text;
    // The shared OpenCC s2t tables, the same phrase-level conversion the reference server ships. A character table cannot tell 头发 (頭髮) from 发展 (發展); CFStringTransform was one.
    const char *utf8 = text.UTF8String;
    if (!utf8) return text;
    const auto converted = msime::host_api::simplified_to_traditional(
        std::string_view(utf8, strlen(utf8)));
    if (!converted) return text;
    NSString *result = [NSString stringWithUTF8String:converted->c_str()];
    return result ?: text;
}

static BOOL MetasequoiaIsHanCodePoint(UTF32Char codePoint) {
    return codePoint == 0x3007 || (codePoint >= 0x3400 && codePoint <= 0x4DBF) ||
        (codePoint >= 0x4E00 && codePoint <= 0x9FFF) || (codePoint >= 0xF900 && codePoint <= 0xFAFF) ||
        (codePoint >= 0x20000 && codePoint <= 0x2FA1F) || (codePoint >= 0x30000 && codePoint <= 0x323AF);
}

NSString *MetasequoiaEdgeHanCharacter(NSString *text, BOOL first) {
    if (![text isKindOfClass:NSString.class]) return nil;
    NSString *found = nil;
    const NSUInteger length = text.length;
    for (NSUInteger index = 0; index < length;) {
        const unichar unit = [text characterAtIndex:index];
        UTF32Char codePoint = unit;
        NSUInteger width = 1;
        if (CFStringIsSurrogateHighCharacter(unit) && index + 1 < length &&
            CFStringIsSurrogateLowCharacter([text characterAtIndex:index + 1])) {
            codePoint = CFStringGetLongCharacterForSurrogatePair(unit, [text characterAtIndex:index + 1]);
            width = 2;
        }
        if (MetasequoiaIsHanCodePoint(codePoint)) {
            found = [text substringWithRange:NSMakeRange(index, width)];
            if (first) return found;
        }
        index += width;
    }
    return found;
}
