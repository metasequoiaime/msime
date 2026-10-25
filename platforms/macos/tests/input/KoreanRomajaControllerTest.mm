#import "../../src/input/InputController.mm"
#import "../settings/TestPreferenceSuite.h"
#include <cassert>
#import <objc/runtime.h>

// 使用 AppKit 自己的 marked range、文档和光标；不以桩会话或字符串拼接模拟引擎。
@interface KoreanTextView : NSTextView <MSIMETextClient>
@end
@implementation KoreanTextView
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement {
    [super setMarkedText:text selectedRange:selection replacementRange:replacement];
}
- (NSAttributedString *)attributedSubstringFromRange:(NSRange)range {
    if (NSMaxRange(range) > self.string.length) return nil;
    return [self.textStorage attributedSubstringFromRange:range];
}
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index lineHeightRectangle:(NSRect *)rect {
    (void)index;
    *rect = [self.window convertRectToScreen:[self convertRect:NSMakeRect(8, 8, 1, 20) toView:nil]];
    return @{};
}
@end

// 单进程夹具没有 IMK 的远端客户端连接；只隔离框架最后一次 IPC，记录生产控制器确实调用它。
// Rust 会话、marked text、候选和失焦提交仍走生产实现；系统跨进程投递须另行验收。
static NSUInteger BaseDeactivations;
static void RecordBaseDeactivation(id controller, SEL selector, id sender) {
    (void)controller; (void)selector; (void)sender;
    ++BaseDeactivations;
}

static NSDictionary *KoreanOptions;
@interface KoreanController : MSIMEInputController
@end
@implementation KoreanController
- (NSDictionary *)runtimeOptions { return KoreanOptions; }
- (BOOL)secureEventInputActive { return NO; }
@end

static NSEvent *Key(NSString *text, unsigned short code, NSEventModifierFlags flags = 0) {
    return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0
        windowNumber:0 context:nil characters:text charactersIgnoringModifiers:text.lowercaseString
        isARepeat:NO keyCode:code];
}

static void Type(KoreanController *controller, KoreanTextView *client, NSString *text) {
    NSDictionary *codes = @{@"a":@0, @"b":@11, @"g":@5, @"h":@4, @"k":@40, @"n":@45,
        @"q":@12, @"r":@15, @"s":@1, @"u":@32, @"v":@9, @"w":@13, @"x":@7, @"z":@6};
    for (NSUInteger index = 0; index < text.length; ++index) {
        NSString *letter = [text substringWithRange:NSMakeRange(index, 1)];
        NSNumber *code = codes[letter.lowercaseString];
        assert(code);
        assert([controller handleEvent:Key(letter, code.unsignedShortValue,
            [letter isEqual:letter.uppercaseString] ? NSEventModifierFlagShift : 0) client:client]);
    }
}

static void Clear(KoreanTextView *client) {
    assert(!client.hasMarkedText);
    client.string = @"";
    [client setSelectedRange:NSMakeRange(0, 0)];
}

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        for (NSNumber *romaja in @[@NO, @YES]) {
            NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
            NSMutableDictionary *preferences = [@{@"scheme":@"korean", @"default_ime_mode":@"chinese",
                @"candidate_page_size":@5, @"chinese_punctuation":@YES,
                @"learning":@NO, @"cloud_candidates":@NO, @"candidate_translations":@NO,
                @"floating_toolbar":@{@"enabled":@NO}, @"tsf_preedit_style":@"empty"} mutableCopy];
            if (romaja.boolValue) preferences[@"korean"] = @{@"layout":@"romaja"};
            NSMutableDictionary *options = [@{@"api_version":@1, @"preferences":preferences} mutableCopy];
            for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
                NSString *path = [root stringByAppendingPathComponent:name];
                assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
                options[name] = path;
            }
            KoreanOptions = options;
            NSString *suite = [@"msime.korean-flow." stringByAppendingString:NSUUID.UUID.UUIDString];
            NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
            MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
                skinsRoot:[NSURL fileURLWithPath:root]];
            [appearance applySharedInputPreferences:preferences];
            NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(100, 100, 640, 240)
                styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
            window.releasedWhenClosed = NO;
            KoreanTextView *client = [[KoreanTextView alloc] initWithFrame:NSMakeRect(0, 0, 640, 240)];
            window.contentView = client;
            [window makeFirstResponder:client];
            KoreanController *controller = [KoreanController alloc];
            [controller setValue:appearance forKey:@"appearance"];
            [controller setValue:client forKey:@"activeClient"];
            [controller setValue:[[MSIMECandidatePanel alloc] init] forKey:@"panel"];
            [controller prepareSession];
            MSIMEClientSession *session = [controller valueForKey:@"session"];
            assert(session && [session.hostOptions[@"korean_romaja"] isEqual:@YES]);
            [controller apply:[session setFocused:YES error:nil]];

            // 两套式缺省仍有效；罗马字的 ng 后续改写不能留下重复文档文字。
            Type(controller, client, romaja.boolValue ? @"hang" : @"gks");
            assert(client.hasMarkedText && [client.string isEqual:romaja.boolValue ? @"항" : @"한"]);
            Type(controller, client, romaja.boolValue ? @"ug" : @"rnr");
            assert([client.string isEqual:@"한국"]);
            NSEvent *enter = Key(@"\r", 36);
            assert(![controller handleEvent:enter client:client]);
            assert(!client.hasMarkedText && [client.string isEqual:@"한국"]);
            // 宿主把未处理的 Return 留给应用，应用随后插入换行。
            [client insertText:@"\n" replacementRange:NSMakeRange(NSNotFound, 0)];
            assert([client.string isEqual:@"한국\n"]);
            Clear(client);

            if (romaja.boolValue) {
                Type(controller, client, @"bang");
                assert([client.string isEqual:@"방"]);
                assert([controller handleEvent:Key(@"\b", 51) client:client]);
                assert([client.string isEqual:@"반"]);
                assert([controller handleEvent:Key(@"\e", 53) client:client]);
                assert(!client.hasMarkedText && client.string.length == 0);
                Type(controller, client, @"gwa");
                assert([controller handleEvent:Key(@"\b", 51) client:client]);
                assert([client.string isEqual:@"구"]);
                assert([controller handleEvent:Key(@"\e", 53) client:client]);
                Type(controller, client, @"xa");
                assert([client.string isEqual:@"아"]);
                assert([controller handleEvent:Key(@"\e", 53) client:client]);
                Type(controller, client, @"Ga");
                assert([client.string isEqual:@"까"]);
                assert([controller handleEvent:Key(@",", 43) client:client]);
                assert(!client.hasMarkedText && [client.string isEqual:@"까,"]);
                Clear(client);
                Type(controller, client, @"hanqvz");
                assert(!client.hasMarkedText && [client.string isEqual:@"한qvz"]);
                Clear(client);
            }

            // 实际候选窗读取引擎汉字列表；Option+Return 打开，数字选择写回 AppKit 文档。
            Type(controller, client, romaja.boolValue ? @"han" : @"gks");
            assert([controller handleEvent:Key(@"\r", 36, NSEventModifierFlagOption) client:client]);
            NSDictionary *view = [controller valueForKey:@"view"];
            assert([view[@"candidates"][0][@"text"] isEqual:@"韓"]);
            assert([(MSIMECandidatePanel *)[controller valueForKey:@"panel"] isVisible]);
            assert([controller handleEvent:Key(@"1", 18) client:client]);
            assert(!client.hasMarkedText && [client.string isEqual:@"韓"]);
            Clear(client);

            // 失焦把当前韩文留在原文档；另一个编辑框不得继承这次组字。
            Type(controller, client, romaja.boolValue ? @"han" : @"gks");
            Method base = class_getInstanceMethod(IMKInputController.class, @selector(deactivateServer:));
            assert(base);
            BaseDeactivations = 0;
            IMP original = method_setImplementation(base, (IMP)RecordBaseDeactivation);
            [controller deactivateServer:client];
            method_setImplementation(base, original);
            assert(BaseDeactivations == 1);
            assert(!client.hasMarkedText && [client.string isEqual:@"한"]);
            KoreanTextView *second = [[KoreanTextView alloc] initWithFrame:client.frame];
            [controller setValue:second forKey:@"activeClient"];
            [controller apply:[session setFocused:YES error:nil]];
            Type(controller, second, romaja.boolValue ? @"hangug" : @"gksrnr");
            assert([second.string isEqual:@"한국"] && [client.string isEqual:@"한"]);
            [controller commitComposition:second];
            assert(!second.hasMarkedText);
            assert([session closeWithError:nil]);
            [window close];
            MSIMERemoveTestPreferenceSuite(defaults, suite);
            [NSFileManager.defaultManager removeItemAtPath:root error:nil];
        }
    }
    return 0;
}
