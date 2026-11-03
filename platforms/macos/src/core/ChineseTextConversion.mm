#import "ChineseTextConversion.h"

#include "../../../common/ChineseTextConversion.h"

NSString *MetasequoiaChineseOutputString(NSString *text, BOOL traditionalOutput, uint32_t variant) {
    if (!traditionalOutput || text.length == 0) return text;
    // 共享 OpenCC 词表：`variant` 0 是 s2t（与旧调用相同），1 s2tw、2 s2twp。逐字表分不清「头发」和「发展」；CFStringTransform 也分不清。
    const char *utf8 = text.UTF8String;
    if (!utf8) return text;
    const auto converted = msime::host_api::convert_to_traditional(
        std::string_view(utf8, strlen(utf8)), variant);
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
