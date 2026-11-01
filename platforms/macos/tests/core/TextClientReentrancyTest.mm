#import "TextClient.h"
#include <cstdio>

@interface ReentrantDocumentClient : NSObject <MSIMETextClient>
@property(nonatomic, strong) NSMutableString *document;
@property(nonatomic) NSRange markedRange;
@property(nonatomic) NSUInteger caret;
@property(nonatomic, copy) void (^onInsert)(void);
@property(nonatomic, copy) void (^onMark)(void);
@property(nonatomic, copy) void (^onRead)(void);
@property(nonatomic, copy) void (^onEmptyInsert)(void);
@property(nonatomic) BOOL unknownContext;
@property(nonatomic) BOOL rewrittenContext;
@property(nonatomic) BOOL rejectPosition;
@property(nonatomic) BOOL ignorePosition;
@end

@implementation ReentrantDocumentClient
- (NSRange)replacement:(NSRange)range {
    if (range.location != NSNotFound) return range;
    return self.markedRange.location != NSNotFound ? self.markedRange : NSMakeRange(self.caret, 0);
}
- (void)insertText:(id)value replacementRange:(NSRange)range {
    NSString *text = [value isKindOfClass:NSAttributedString.class] ? [value string] : value;
    range = [self replacement:range];
    [self.document replaceCharactersInRange:range withString:text];
    self.caret = range.location + text.length;
    self.markedRange = NSMakeRange(NSNotFound, 0);
    void (^callback)(void) = text.length ? self.onInsert : self.onEmptyInsert;
    if (text.length) self.onInsert = nil;
    else self.onEmptyInsert = nil;
    if (callback) callback();
}
- (void)setMarkedText:(id)value selectionRange:(NSRange)selection replacementRange:(NSRange)range {
    NSString *text = [value isKindOfClass:NSAttributedString.class] ? [value string] : value;
    if (range.location != NSNotFound && self.rejectPosition) @throw [NSException exceptionWithName:@"range" reason:nil userInfo:nil];
    if (self.ignorePosition) range = NSMakeRange(NSNotFound, 0);
    range = [self replacement:range];
    [self.document replaceCharactersInRange:range withString:text];
    self.caret = range.location + selection.location;
    self.markedRange = text.length ? NSMakeRange(range.location, text.length) : NSMakeRange(NSNotFound, 0);
    void (^callback)(void) = self.onMark;
    self.onMark = nil;
    if (callback) callback();
}
- (NSRange)selectedRange {
    const NSRange range = self.unknownContext ? NSMakeRange(NSNotFound, 0) : NSMakeRange(self.caret, 0);
    void (^callback)(void) = self.onRead;
    self.onRead = nil;
    if (callback) callback();
    return range;
}
- (NSAttributedString *)attributedSubstringFromRange:(NSRange)range {
    if (NSMaxRange(range) > self.document.length) return nil;
    if (self.rewrittenContext) return [[NSAttributedString alloc] initWithString:@"changed"];
    return [[NSAttributedString alloc] initWithString:[self.document substringWithRange:range]];
}
@end

static NSDictionary *View(NSString *text) {
    return @{@"editing_text": text, @"caret_position": @(text.length)};
}

static void Apply(NSDictionary *transition, ReentrantDocumentClient *client, BOOL *hasMarked, uint64_t *sequence) {
    const uint64_t current = ++*sequence;
    MSIMEApplyTransitionWithWriteGuard(transition, client, MSIMEInlinePreeditStylePinyin, nil, hasMarked,
        ^BOOL { return current == *sequence; });
}

static ReentrantDocumentClient *Document() {
    ReentrantDocumentClient *client = [ReentrantDocumentClient new];
    client.document = [@"prefix|suffix" mutableCopy];
    client.caret = 6;
    client.markedRange = NSMakeRange(NSNotFound, 0);
    return client;
}

static bool TestCommitReentrancy(BOOL nestedCommit, BOOL outerRemainder) {
    ReentrantDocumentClient *client = Document();
    __block BOOL hasMarked = NO;
    __block uint64_t sequence = 0;
    Apply(@{@"view": View(@"old")}, client, &hasMarked, &sequence);
    __weak ReentrantDocumentClient *weakClient = client;
    client.onInsert = ^{
        NSMutableDictionary *next = [@{@"view": View(@"ne")} mutableCopy];
        if (nestedCommit) next[@"commit"] = @"新";
        Apply(next, weakClient, &hasMarked, &sequence);
    };
    Apply(@{@"commit": @"词", @"view": View(outerRemainder ? @"tail" : @"")}, client, &hasMarked, &sequence);
    NSString *expected = nestedCommit ? @"prefix词新ne|suffix" : @"prefix词ne|suffix";
    if (![client.document isEqualToString:expected] || !hasMarked) {
        std::fprintf(stderr, "上屏重入丢失较新组字：nestedCommit=%d remainder=%d document=%s marked=%d\n",
            nestedCommit, outerRemainder, client.document.UTF8String, hasMarked);
        return false;
    }
    return true;
}

static bool TestMarkedTextReentrancy() {
    ReentrantDocumentClient *client = Document();
    __block BOOL hasMarked = NO;
    __block uint64_t sequence = 0;
    __weak ReentrantDocumentClient *weakClient = client;
    client.onMark = ^{ Apply(@{@"view": View(@"ne")}, weakClient, &hasMarked, &sequence); };
    Apply(@{@"view": View(@"old")}, client, &hasMarked, &sequence);
    return [client.document isEqualToString:@"prefixne|suffix"] && hasMarked;
}

static bool TestCursorReentrancy(NSUInteger stage) {
    ReentrantDocumentClient *client = Document();
    __block BOOL hasMarked = NO;
    __block uint64_t sequence = 0;
    __weak ReentrantDocumentClient *weakClient = client;
    void (^nested)(void) = ^{ Apply(@{@"view":View(@"ne")}, weakClient, &hasMarked, &sequence); };
    if (stage == 0) client.onInsert = nested;
    if (stage == 1) client.onRead = nested;
    if (stage == 2) client.onMark = nested;
    if (stage == 3) client.onEmptyInsert = nested;
    Apply(@{@"commit":@"title:\"\"", @"commit_cursor_suffix":@"\"", @"view":View(@"stale")}, client, &hasMarked, &sequence);
    NSString *expected = stage < 2 ? @"prefixtitle:\"\"ne|suffix" : @"prefixtitle:\"ne\"|suffix";
    return [client.document isEqual:expected] && hasMarked;
}

static bool TestCursorFallback(NSUInteger stage) {
    ReentrantDocumentClient *client = Document();
    BOOL hasMarked = NO;
    uint64_t sequence = 0;
    client.unknownContext = stage == 0;
    client.rejectPosition = stage == 1;
    client.ignorePosition = stage == 2;
    client.rewrittenContext = stage == 4;
    Apply(@{@"commit":@"title:\"\"", @"commit_cursor_suffix":stage == 3 ? @"wrong" : @"\"", @"view":View(@"n")}, client, &hasMarked, &sequence);
    return [client.document isEqual:@"prefixtitle:\"\"n|suffix"] && hasMarked;
}

int main() {
    @autoreleasepool {
        bool passed = TestCommitReentrancy(NO, NO);
        passed = TestCommitReentrancy(YES, NO) && passed;
        passed = TestCommitReentrancy(NO, YES) && passed;
        passed = TestMarkedTextReentrancy() && passed;
        for (NSUInteger stage = 0; stage < 4; ++stage)
            passed = TestCursorReentrancy(stage) && passed;
        for (NSUInteger stage = 0; stage < 5; ++stage)
            passed = TestCursorFallback(stage) && passed;
        return passed ? 0 : 1;
    }
}
