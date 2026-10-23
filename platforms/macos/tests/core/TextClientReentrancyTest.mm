#import "TextClient.h"
#include <cstdio>

@interface ReentrantDocumentClient : NSObject <MSIMETextClient>
@property(nonatomic, strong) NSMutableString *document;
@property(nonatomic) NSRange markedRange;
@property(nonatomic) NSUInteger caret;
@property(nonatomic, copy) void (^onInsert)(void);
@property(nonatomic, copy) void (^onMark)(void);
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
    void (^callback)(void) = self.onInsert;
    self.onInsert = nil;
    if (callback) callback();
}
- (void)setMarkedText:(id)value selectionRange:(NSRange)selection replacementRange:(NSRange)range {
    NSString *text = [value isKindOfClass:NSAttributedString.class] ? [value string] : value;
    range = [self replacement:range];
    [self.document replaceCharactersInRange:range withString:text];
    self.caret = range.location + selection.location;
    self.markedRange = text.length ? NSMakeRange(range.location, text.length) : NSMakeRange(NSNotFound, 0);
    void (^callback)(void) = self.onMark;
    self.onMark = nil;
    if (callback) callback();
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

int main() {
    @autoreleasepool {
        bool passed = TestCommitReentrancy(NO, NO);
        passed = TestCommitReentrancy(YES, NO) && passed;
        passed = TestCommitReentrancy(NO, YES) && passed;
        passed = TestMarkedTextReentrancy() && passed;
        return passed ? 0 : 1;
    }
}
