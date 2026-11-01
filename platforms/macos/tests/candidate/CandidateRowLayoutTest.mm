#import "../settings/TestPreferenceSuite.h"
// The candidate page layout as the panel applies it. CandidateItemLayoutTest covers the arithmetic; this renders real pages through the controller, because what matters is the frame a candidate button ends up with and the runs it draws: the card is at most half the screen's visible width (a horizontal page may grow past it to keep its glosses on one line), text, 辅助码 and glosses wider than their column wrap inside it, rows take their own heights, and a horizontal page breaks onto a new line instead of squeezing its candidates.
//
// It stands apart from shortcut-test, which imports the same controller, so a failure elsewhere in that suite cannot hide the layout assertions.
#import "../../src/input/InputController.mm"
#import <ImageIO/ImageIO.h>

#include <cassert>
#include <filesystem>
#include <fstream>

@interface RowLayoutClient : NSObject <MSIMETextClient>
@property(nonatomic) NSRect caret;
@property(nonatomic, copy) NSDictionary *colors;
@property(nonatomic) NSUInteger attributeReads;
@end
@implementation RowLayoutClient
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index lineHeightRectangle:(NSRect *)rect
{
    (void)index;
    *rect = self.caret;
    ++self.attributeReads;
    return self.colors ?: @{};
}
- (void)insertText:(id)text replacementRange:(NSRange)range
{
    (void)text;
    (void)range;
}
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement
{
    (void)text;
    (void)selection;
    (void)replacement;
}
@end

// Keep the panel off the screen while it still lays its content out.
@interface RowLayoutPanel : MSIMECandidatePanel
@property(nonatomic, strong) NSMutableArray<NSValue *> *resizedSizes;
@end
@implementation RowLayoutPanel
- (void)setFrame:(NSRect)frame display:(BOOL)display
{
    const NSSize previous = self.frame.size;
    [super setFrame:frame display:display];
    if (!NSEqualSizes(previous, self.frame.size))
        [self.resizedSizes addObject:[NSValue valueWithSize:self.frame.size]];
}
- (void)orderFrontRegardless
{
}
@end

// 翻页测试只记录经真实身份检查后派发的命令，不创建词库或改动用户数据。
@interface PagerTestSession : NSObject
@property(nonatomic) NSUInteger commands;
@property(nonatomic) uint32_t lastCommand;
@end
@implementation PagerTestSession
- (NSDictionary *)command:(uint32_t)command error:(NSError **)error {
    (void)error; ++self.commands; self.lastCommand = command; return @{};
}
@end
@interface PagerTestController : MSIMEInputController
@property(nonatomic) NSUInteger pointerReports;
@end
@implementation PagerTestController
- (void)reportPointerToLifecycle { ++self.pointerReports; }
- (void)apply:(NSDictionary *)transition { (void)transition; }
- (void)applyCandidateFrame:(NSRect)frame {
    [super applyCandidateFrame:frame];
}
@end

// 专门模拟正在显示的窗口，检查同次组字的历史宽度不会覆盖收紧开关。
@interface VisibleRowLayoutPanel : RowLayoutPanel
@end
@implementation VisibleRowLayoutPanel
- (BOOL)isVisible { return YES; }
@end

@interface AdaptiveLayoutSession : NSObject
@property(nonatomic, copy) NSArray<NSString *> *texts;
@property(nonatomic) NSUInteger cursor;
@property(nonatomic) NSUInteger selected;
@property(nonatomic, copy) NSIndexSet *pendingIndices;
// 联想候选：视图带 prediction，翻页命令照 runtime 清掉联想。
@property(nonatomic) BOOL prediction;
@property(nonatomic) NSMutableArray<NSNumber *> *commands;
// 最近一次报来的显示页座位；nil 表示没报过，空数组表示清掉。
@property(nonatomic, copy) NSArray<NSNumber *> *displayPage;
// 落进引擎的字符键：选词键该被宿主拦下时，这里应当是空的。
@property(nonatomic) NSMutableString *typed;
@end
@implementation AdaptiveLayoutSession
- (NSArray *)entries {
    NSMutableArray *entries = [NSMutableArray array];
    for (NSUInteger index = 0; index < self.texts.count; ++index)
        [entries addObject:@{@"text":self.texts[index], @"highlighted":@(index == self.cursor),
            @"source":@([self.pendingIndices containsIndex:index] ? 15 : 0),
            @"id":@{@"session":@1, @"generation":@7, @"index":@(index)}}];
    return entries;
}
- (NSDictionary *)viewWithError:(NSError **)error {
    (void)error;
    if (!self.texts.count) return @{@"session":@1, @"generation":@7, @"focused":@YES, @"candidates":@[]};
    const NSUInteger start = self.cursor / 6 * 6;
    NSMutableDictionary *view = [@{@"session":@1, @"generation":@7, @"focused":@YES, @"editing_text":@"synthetic",
        @"page":@(self.cursor / 6), @"page_count":@((self.texts.count + 5) / 6),
        @"candidates":[[self entries] subarrayWithRange:NSMakeRange(start, MIN((NSUInteger)6, self.texts.count - start))]} mutableCopy];
    if (self.prediction) view[@"prediction"] = @YES;
    return view;
}
- (NSDictionary *)allCandidatesWithError:(NSError **)error {
    (void)error;
    return @{@"session":@1, @"generation":@7, @"candidates":[self entries]};
}
- (NSDictionary *)setDisplayPageGeneration:(uint64_t)generation indices:(NSArray<NSNumber *> *)indices error:(NSError **)error {
    (void)error;
    assert(generation == 7);
    self.displayPage = indices;
    return @{@"applied":@YES};
}
- (NSDictionary *)command:(uint32_t)command error:(NSError **)error {
    [self.commands addObject:@(command)];
    // runtime 对联想状态的翻页与移动高亮都会清掉联想。
    if (self.prediction && (command == MSIME_NEXT_PAGE || command == MSIME_PREVIOUS_PAGE ||
                            command == MSIME_NEXT_CANDIDATE || command == MSIME_PREVIOUS_CANDIDATE)) {
        self.prediction = NO;
        self.texts = @[];
        return @{@"view":[self viewWithError:error], @"handled":@NO};
    }
    if (command == MSIME_NEXT_CANDIDATE && self.cursor + 1 < self.texts.count) ++self.cursor;
    if (command == MSIME_PREVIOUS_CANDIDATE && self.cursor > 0) --self.cursor;
    return @{@"view":[self viewWithError:error], @"handled":@YES};
}
- (NSDictionary *)selectAnyCandidateGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error {
    (void)error;
    assert(generation == 7 && index < self.texts.count);
    self.selected = index;
    return nil;
}
@end

@interface AdaptiveLayoutController : MSIMEInputController
// 普通候选窗一次测量里最多的项数，用来确认按需分页没有量全部候选。
@property(nonatomic) NSUInteger largestMeasurement;
// 展开网格累计测量的项数（网格测量不为释义预留，普通候选窗预留），用来确认移动光标不重排。
@property(nonatomic) NSUInteger gridMeasuredItems;
@end
@implementation AdaptiveLayoutController
- (void)reportPointerToLifecycle {}
- (MSIMECandidatePageGeometry)candidatePageGeometry:(NSArray *)candidates font:(NSFont *)font glossFont:(NSFont *)glossFont
                                     showSelectedBar:(BOOL)showSelectedBar inset:(CGFloat)inset paging:(BOOL)paging
                                             visible:(NSRect)visible preeditWidth:(CGFloat)preeditWidth
                                        minimumWidth:(CGFloat)minimumWidth reservesGloss:(BOOL)reservesGloss pagerWidth:(CGFloat)pagerWidth
                                     expandedColumns:(NSUInteger)columnCount
                                       sharedColumns:(const std::vector<double> *)sharedColumns {
    if (!reservesGloss) self.gridMeasuredItems += candidates.count;
    return [super candidatePageGeometry:candidates font:font glossFont:glossFont showSelectedBar:showSelectedBar inset:inset
        paging:paging visible:visible preeditWidth:preeditWidth minimumWidth:minimumWidth reservesGloss:reservesGloss
        pagerWidth:pagerWidth expandedColumns:columnCount sharedColumns:sharedColumns];
}
- (MSIMECandidatePageGeometry)candidatePageGeometry:(NSArray *)candidates font:(NSFont *)font glossFont:(NSFont *)glossFont
                                     showSelectedBar:(BOOL)showSelectedBar inset:(CGFloat)inset paging:(BOOL)paging
                                             visible:(NSRect)visible preeditWidth:(CGFloat)preeditWidth
                                        minimumWidth:(CGFloat)minimumWidth reservesGloss:(BOOL)reservesGloss {
    if (reservesGloss) self.largestMeasurement = MAX(self.largestMeasurement, candidates.count);
    return [super candidatePageGeometry:candidates font:font glossFont:glossFont showSelectedBar:showSelectedBar inset:inset
        paging:paging visible:visible preeditWidth:preeditWidth minimumWidth:minimumWidth reservesGloss:reservesGloss];
}
- (void)applyCandidateFrame:(NSRect)frame {
    [super applyCandidateFrame:frame];
}
@end

static MSIMECandidateButton *CandidateButton(NSView *content, NSInteger tag)
{
    for (NSView *view in MSIMECandidateControlSubviews(content))
        if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag == tag) return (MSIMECandidateButton *)view;
    return nil;
}

static NSRect CandidateFrame(NSView *view) {
    NSView *chrome = view.superview;
    while (chrome && ![chrome isKindOfClass:MSIMECandidateChromeView.class]) chrome = chrome.superview;
    return chrome ? [view convertRect:view.bounds toView:MSIMECandidateControlsView(chrome)] : view.frame;
}

// 候选相对窗口左上角的位置；按候选身份找，不按按钮序号。
static void AssertNoCandidateLayoutAnimation(NSView *content) {
    assert(![content.layer.mask animationForKey:@"candidate-expand-size"]);
    for (NSView *view in MSIMECandidateControlSubviews(content))
        if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag >= 0)
            assert(![view.layer animationForKey:@"candidate-layout-move"]);
}

// 走完整按键入口用的替身：没实现的会话方法按真实会话的签名回零值并记下来；字符与标点记进 typed，
// 用来确认选词键没有落进引擎。只给按键测试用，其余测试的替身仍然对缺失的方法直接报错。
@interface AdaptiveKeySession : AdaptiveLayoutSession
@property(nonatomic) NSMutableArray<NSString *> *forwarded;
@end
@implementation AdaptiveKeySession
- (NSMethodSignature *)methodSignatureForSelector:(SEL)selector {
    return [super methodSignatureForSelector:selector] ?: [MSIMEClientSession instanceMethodSignatureForSelector:selector];
}
- (void)forwardInvocation:(NSInvocation *)invocation {
    [self.forwarded addObject:NSStringFromSelector(invocation.selector)];
    char zero[64] = {};
    if (invocation.methodSignature.methodReturnLength) [invocation setReturnValue:zero];
}
- (NSDictionary *)typeASCII:(uint8_t)character shift:(BOOL)shift error:(NSError **)error {
    (void)shift; (void)error; [self.typed appendFormat:@"%c", character]; return nil;
}
- (NSDictionary *)punctuationASCII:(uint8_t)ascii error:(NSError **)error {
    (void)error; [self.typed appendFormat:@"%c", ascii]; return nil;
}
- (NSDictionary *)punctuation:(uint8_t)ascii error:(NSError **)error {
    (void)error; [self.typed appendFormat:@"%c", ascii]; return nil;
}
- (NSDictionary *)punctuation:(uint8_t)ascii preceding:(uint32_t)preceding error:(NSError **)error {
    (void)preceding; (void)error; [self.typed appendFormat:@"%c", ascii]; return nil;
}
@end

// 显示和收起都只改记下的可见状态，不真的上屏：收起后的按键路由要看到候选窗确实不在了。
@interface TogglingRowLayoutPanel : RowLayoutPanel
@property(nonatomic) BOOL shown;
@end
@implementation TogglingRowLayoutPanel
- (BOOL)isVisible { return self.shown; }
- (void)orderFrontRegardless { self.shown = YES; }
- (void)orderOut:(id)sender { (void)sender; self.shown = NO; }
@end

// PR 审查补的边界：显示页上报、释义按显示行请求、收起后与联想状态的翻页、窄宽度大缩放的首行控件。
static NSView *HeaderChild(NSView *content, NSString *identifier);

static void TestFilteredFixedPages(RowLayoutClient *client) {
    NSString *suite = [@"app.msime.test.filtered-pages." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"fixed",
        @"candidate_page_size":@6}];
    appearance.showsCandidatePreedit = NO;
    appearance.showsCandidatePageNumber = YES;
    appearance.showsAppLogo = NO;
    appearance.candidateTranslations = NO;
    AdaptiveKeySession *session = [AdaptiveKeySession new];
    session.forwarded = [NSMutableArray array];
    session.typed = [NSMutableString string];
    session.commands = [NSMutableArray array];
    AdaptiveLayoutController *controller = [[AdaptiveLayoutController alloc] init];
    MSIMECandidatePanel *panel = [[TogglingRowLayoutPanel alloc] initWithContentRect:NSZeroRect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:panel forKey:@"panel"];
    for (BOOL vertical : {NO, YES}) for (NSUInteger count : {7, 13}) {
        appearance.vertical = vertical;
        NSMutableArray *texts = [NSMutableArray array];
        for (NSUInteger index = 0; index < count; ++index) [texts addObject:@"合成候选"];
        session.texts = texts;
        session.pendingIndices = [NSIndexSet indexSetWithIndex:1];
        session.cursor = 0;
        [controller setValue:[session viewWithError:nil] forKey:@"view"];
        [controller renderCandidates];
        assert(CandidateButton(panel.contentView, 5));
        assert(CandidateButton(panel.contentView, 6) == nil);
        assert([CandidateButton(panel.contentView, 5).candidateID[@"index"] isEqual:@6]);
        assert(([session.displayPage isEqual:@[@0, @2, @3, @4, @5, @6]]));
        assert([CandidateButton(panel.contentView, 1).title hasPrefix:@"2  "]);
        NSEvent *six = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0
            windowNumber:0 context:nil characters:@"6" charactersIgnoringModifiers:@"6" isARepeat:NO keyCode:22];
        session.selected = NSNotFound;
        assert([controller handleEvent:six client:client]);
        assert(session.selected == 6 && session.typed.length == 0);
        [controller apply:[controller candidateCommand:MSIME_NEXT_CANDIDATE]];
        assert(session.cursor == 2);
        [controller apply:[controller candidateCommand:MSIME_PREVIOUS_CANDIDATE]];
        assert(session.cursor == 0);
        if (count == 7) {
            assert(!HeaderChild(panel.contentView, @"candidate-page-indicator"));
            assert(!CandidateButton(panel.contentView, -2));
        } else {
            NSTextField *indicator = (id)HeaderChild(panel.contentView, @"candidate-page-indicator");
            assert([indicator.stringValue isEqual:@"1 / 2"]);
            [controller changeCandidatePage:CandidateButton(panel.contentView, -2)];
            assert(session.cursor == 7);
            indicator = (id)HeaderChild(panel.contentView, @"candidate-page-indicator");
            assert([indicator.stringValue isEqual:@"2 / 2"]);
            assert(([session.displayPage isEqual:@[@7, @8, @9, @10, @11, @12]]));
            [controller selectCandidate:CandidateButton(panel.contentView, 5)];
            assert(session.selected == 12);
            session.selected = NSNotFound;
            assert([controller handleEvent:six client:client]);
            assert(session.selected == 12 && session.typed.length == 0);
            [controller changeCandidatePage:CandidateButton(panel.contentView, -1)];
            assert(session.cursor == 0);
        }
        session.pendingIndices = nil;
        [controller setValue:[session viewWithError:nil] forKey:@"view"];
        [controller renderCandidates];
        assert([CandidateButton(panel.contentView, 5).candidateID[@"index"] isEqual:@5]);
        assert(session.displayPage.count == 0);
        NSTextField *restoredIndicator = (id)HeaderChild(panel.contentView, @"candidate-page-indicator");
        assert([restoredIndicator.stringValue isEqual:count == 7 ? @"1 / 2" : @"1 / 3"]);
    }
    session.texts = @[@"合成首项", @"等待", @"等待", @"等待", @"等待", @"等待", @"等待", @"等待", @"等待", @"等待", @"等待", @"等待", @"合成尾项"];
    session.pendingIndices = [NSIndexSet indexSetWithIndexesInRange:NSMakeRange(1, 11)];
    session.cursor = 0;
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [controller renderCandidates];
    assert(([session.displayPage isEqual:@[@0, @12]]));
    [controller apply:[controller candidateCommand:MSIME_NEXT_CANDIDATE]];
    assert(session.cursor == 12);
    [controller apply:[controller candidateCommand:MSIME_PREVIOUS_CANDIDATE]];
    assert(session.cursor == 0);
    session.pendingIndices = [NSIndexSet indexSetWithIndexesInRange:NSMakeRange(0, session.texts.count)];
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [controller renderCandidates];
    assert(session.displayPage.count == 0 && !panel.isVisible);
    [controller setValue:nil forKey:@"session"];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestAdaptiveReviewFixes(RowLayoutClient *client) {
    NSString *suite = [@"app.msime.test.adaptive-review." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"adaptive",
        @"candidate_max_width":@400}];
    appearance.showsCandidatePreedit = NO;
    appearance.showsCandidatePageNumber = NO;
    appearance.showsAppLogo = NO;
    appearance.expandCandidates = YES;
    appearance.candidateTranslations = NO;
    AdaptiveKeySession *session = [AdaptiveKeySession new];
    session.forwarded = [NSMutableArray array];
    session.commands = [NSMutableArray array];
    session.typed = [NSMutableString string];
    NSMutableArray *texts = [NSMutableArray array];
    for (NSUInteger index = 0; index < 40; ++index) [texts addObject:@"合成词组"];
    session.texts = texts;
    AdaptiveLayoutController *controller = [[AdaptiveLayoutController alloc] init];
    MSIMECandidatePanel *panel = [[TogglingRowLayoutPanel alloc] initWithContentRect:NSZeroRect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [controller setValue:panel forKey:@"panel"];
    auto indices = [](NSArray *candidates) {
        NSMutableArray *seats = [NSMutableArray array];
        for (NSDictionary *candidate in candidates) [seats addObject:candidate[@"id"][@"index"]];
        return seats;
    };

    // 显示页：每次画出一行就把它的座位报给运行时；翻到第二行（跨过引擎每页 6 个的边界）跟着更新。
    [controller renderCandidates];
    NSArray *first = [controller valueForKey:@"adaptivePageCandidates"];
    assert(first.count > 1 && first.count < 6);
    assert([session.displayPage isEqual:indices(first)]);
    NSDictionary *next = [controller candidateCommand:MSIME_NEXT_PAGE];
    [controller setValue:next[@"view"] forKey:@"view"];
    [controller renderCandidates];
    NSArray *second = [controller valueForKey:@"adaptivePageCandidates"];
    assert([session.displayPage isEqual:indices(second)]);
    // 服务看到的视图换成显示的这一行：本地释义、目标语言词典、自定义翻译都按看得见的候选请求。
    NSDictionary *serviceView = [controller serviceSnapshotView];
    assert([indices(serviceView[@"candidates"]) isEqual:indices(second)]);
    assert(![indices([session viewWithError:nil][@"candidates"]) isEqual:indices(second)]);
    // 同一代里换回固定数量：清掉显示页，服务视图回到引擎的这一页。
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"fixed"}];
    [controller renderCandidates];
    assert(session.displayPage && session.displayPage.count == 0);
    assert([[controller serviceSnapshotView][@"candidates"] isEqual:[session viewWithError:nil][@"candidates"]]);
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"adaptive",
        @"candidate_max_width":@400}];

    // 引擎每页 10 个、这一行只显示几个：`0` 仍归选词，这一行没有第十格就吞掉，不能落进引擎去选它固定页里看不见的第十个。
    appearance.pageSize = 10;
    session.cursor = 0;
    session.selected = NSNotFound;
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [controller renderCandidates];
    assert([[controller valueForKey:@"adaptivePageCandidates"] count] < 10);
    [session.commands removeAllObjects];
    NSEvent *zero = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0
        windowNumber:0 context:nil characters:@"0" charactersIgnoringModifiers:@"0" isARepeat:NO keyCode:29];
    assert([controller handleEvent:zero client:client]);
    assert(session.selected == NSNotFound && session.typed.length == 0);
    appearance.pageSize = 6;

    // 候选窗收起后（上屏、取消），上一行留下的分页不能吞掉应用自己的翻页键：照原样交给引擎。
    session.cursor = 0;
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [controller renderCandidates];
    [controller hideCandidatePanel:"test"];
    [session.commands removeAllObjects];
    [controller candidateCommand:MSIME_PREVIOUS_PAGE];
    assert([session.commands isEqual:@[@(MSIME_PREVIOUS_PAGE)]]);

    // 联想候选超过一行时翻页：runtime 清掉联想，用原来的单次命令，不逐个挪高亮。
    session.prediction = YES;
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [controller renderCandidates];
    assert(panel.isVisible && [[controller valueForKey:@"adaptivePageCandidates"] count] < session.texts.count);
    [session.commands removeAllObjects];
    NSDictionary *cleared = [controller candidateCommand:MSIME_NEXT_PAGE];
    assert([session.commands isEqual:@[@(MSIME_NEXT_PAGE)]]);
    assert(cleared && [cleared[@"view"][@"candidates"] count] == 0);
    session.texts = texts;

    // 最大宽度 160pt、缩放 200%，首行同时有翻页、展开和 logo：控件都留在卡片里，互不重叠。
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"adaptive",
        @"candidate_max_width":@160}];
    appearance.showsCandidatePageNumber = YES;
    appearance.showsAppLogo = YES;
    appearance.showsCandidateExpandButton = YES;
    appearance.candidateScalePercent = 200;
    session.cursor = 0;
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateChromeView *chrome = (id)panel.contentView;
    const NSRect card = chrome.cardBounds;
    NSMutableArray<NSView *> *header = [NSMutableArray array];
    for (NSView *view in MSIMECandidateControlSubviews(panel.contentView))
        if ((view.tag == -1 || view.tag == -2 || view.tag == -3) && [view isKindOfClass:NSButton.class]) [header addObject:view];
        else if ([view.identifier isEqual:@"candidate-logo"]) [header addObject:view];
    assert(header.count >= 3);
    for (NSView *view in header) {
        const NSRect frame = [view convertRect:view.bounds toView:chrome];
        assert(NSMinX(frame) >= NSMinX(card) - 0.5 && NSMaxX(frame) <= NSMaxX(card) + 0.5);
        for (NSView *other in header) {
            if (other == view) continue;
            const NSRect otherFrame = [other convertRect:other.bounds toView:chrome];
            assert(NSWidth(NSIntersectionRect(NSInsetRect(frame, 0.5, 0.5), otherFrame)) <= 0);
        }
    }
    appearance.candidateScalePercent = 100;
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestAdaptiveCandidateLayout(RowLayoutClient *client) {
    NSString *suite = [@"app.msime.test.adaptive." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"adaptive"}];
    appearance.showsCandidatePreedit = NO;
    appearance.showsCandidatePageNumber = NO;
    appearance.showsAppLogo = NO;
    appearance.expandCandidates = YES;
    appearance.candidateTranslations = NO;
    AdaptiveLayoutSession *session = [AdaptiveLayoutSession new];
    NSMutableArray *texts = [NSMutableArray array];
    for (NSUInteger index = 0; index < 30; ++index)
        [texts addObject:index == 2 ? @"合成较长候选" : [NSString stringWithFormat:@"%C", (unichar)(0x4e00 + index)]];
    texts[1] = texts[0];
    session.texts = texts;
    AdaptiveLayoutController *controller = [[AdaptiveLayoutController alloc] init];
    MSIMECandidatePanel *panel = [[VisibleRowLayoutPanel alloc] initWithContentRect:NSZeroRect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    auto width = [&] { return NSWidth(((MSIMECandidateChromeView *)panel.contentView).cardBounds); };
    [controller setValue:panel forKey:@"panel"];
    auto assertTightColumns = [&] {
        NSArray *page = [controller valueForKey:@"adaptivePageCandidates"];
        for (NSUInteger index = 0; index < page.count; ++index) {
            MSIMECandidateButton *cell = CandidateButton(panel.contentView, index);
            const CGFloat right = cell.hasSkinLayout ? cell.textRightInset : MSIMECandidateTextRight * MSIMECandidateScale(appearance);
            assert(fabs(NSWidth(cell.frame) - cell.contentLeft - cell.itemLayout.textWidth - right) <= 1);
        }
    };
    CGFloat firstWidth = 0;
    for (NSUInteger count : {3, 30, 300}) {
        NSMutableArray *synthetic = [NSMutableArray array];
        for (NSUInteger index = 0; index < count; ++index) [synthetic addObject:@"合成词组"];
        session.texts = synthetic;
        [controller setValue:[session viewWithError:nil] forKey:@"view"];
        [controller renderCandidates];
        assertTightColumns();
        const CGFloat cellWidth = NSWidth(CandidateButton(panel.contentView, 0).frame);
        if (firstWidth) assert(fabs(cellWidth - firstWidth) < 0.1);
        firstWidth = cellWidth;
    }
    const NSUInteger originalFontSize = appearance.fontSize;
    NSString *originalFontFamily = appearance.fontFamily;
    NSString *originalEnglishFont = appearance.candidateEnglishFont;
    NSArray *originalFallbackFonts = appearance.fallbackFonts;
    appearance.fontFamily = @"MSIME Synthetic Missing Font";
    appearance.candidateEnglishFont = nil;
    appearance.fallbackFonts = @[];
    for (NSString *mode in @[@"adaptive", @"fixed"])
    for (NSUInteger fontSize : {18, 32}) for (NSUInteger scale : {50, 100, 200}) {
        [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":mode}];
        appearance.fontSize = fontSize;
        appearance.candidateScalePercent = scale;
        CGFloat firstContentLeft = 0;
        for (NSUInteger count = 1; count <= 9; ++count) {
            NSMutableArray *synthetic = [NSMutableArray array];
            for (NSUInteger index = 0; index < count; ++index) [synthetic addObject:@"合成"];
            session.texts = synthetic;
            [controller setValue:[session viewWithError:nil] forKey:@"view"];
            [controller renderCandidates];
            assertTightColumns();
            const CGFloat contentLeft = CandidateButton(panel.contentView, 0).contentLeft;
            if (firstContentLeft) assert(fabs(contentLeft - firstContentLeft) < 0.01);
            firstContentLeft = contentLeft;
        }
    }
    appearance.fontSize = originalFontSize;
    appearance.fontFamily = originalFontFamily;
    appearance.candidateEnglishFont = originalEnglishFont;
    appearance.fallbackFonts = originalFallbackFonts;
    appearance.candidateScalePercent = 100;
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"adaptive"}];
    NSMutableArray *fourCharacterCandidates = [NSMutableArray array];
    for (NSUInteger index = 0; index < 30; ++index) [fourCharacterCandidates addObject:@"合成词组"];
    session.texts = fourCharacterCandidates;
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [appearance applySharedCandidatePreferences:@{@"candidate_count_mode":@"adaptive", @"candidate_max_width":@600}];
    [controller renderCandidates];
    const NSRect firstCell = CandidateButton(panel.contentView, 0).frame;
    const CGFloat gap = NSMinX(CandidateButton(panel.contentView, 1).frame) - NSMaxX(firstCell);
    NSView *expandButton = CandidateButton(panel.contentView, -3);
    assert(expandButton);
    const NSUInteger fourCandidateWidth = (NSUInteger)ceil(2 * NSMinX(firstCell) +
        4 * NSWidth(firstCell) + 3 * gap + NSWidth(expandButton.frame));
    [appearance applySharedCandidatePreferences:@{@"candidate_count_mode":@"adaptive", @"candidate_max_width":@(fourCandidateWidth)}];
    [controller renderCandidates];
    assert([[controller valueForKey:@"adaptivePageCandidates"] count] >= 4);
    assert(NSMaxX(CandidateButton(panel.contentView, 3).frame) <=
        NSMinX(CandidateButton(panel.contentView, -3).frame) + 0.5);
    [appearance applySharedCandidatePreferences:@{@"candidate_count_mode":@"adaptive", @"candidate_max_width":@(fourCandidateWidth - 1)}];
    [controller renderCandidates];
    assert([[controller valueForKey:@"adaptivePageCandidates"] count] == 3);
    for (NSUInteger maximum : {400, 420, 430}) {
        [appearance applySharedCandidatePreferences:@{@"candidate_count_mode":@"adaptive", @"candidate_max_width":@(maximum)}];
        [controller renderCandidates];
        const NSUInteger expected = (NSUInteger)floor((maximum - 2 * NSMinX(firstCell) -
            NSWidth(expandButton.frame) + gap) / (NSWidth(firstCell) + gap));
        assert([[controller valueForKey:@"adaptivePageCandidates"] count] == expected);
    }
    // 按需分页：300 项只量到首行排完；只显示 ‹ ›、不显示总页数；逐页翻到末页，页与页首尾相接。
    {
        appearance.showsCandidatePageNumber = YES;
        [appearance applySharedCandidatePreferences:@{@"candidate_count_mode":@"adaptive", @"candidate_max_width":@400}];
        auto assertArrows = [&](BOOL previous, BOOL next) {
            for (NSView *child in MSIMECandidateControlSubviews(panel.contentView))
                assert(![child.identifier isEqual:@"candidate-page-indicator"]);
            MSIMECandidateButton *back = CandidateButton(panel.contentView, -1);
            MSIMECandidateButton *forward = CandidateButton(panel.contentView, -2);
            assert(back && forward && back.enabled == previous && forward.enabled == next);
        };
        NSMutableArray *many = [NSMutableArray array];
        for (NSUInteger index = 0; index < 300; ++index) [many addObject:@"合成词组"];
        session.texts = many;
        session.cursor = 0;
        [controller setValue:[session viewWithError:nil] forKey:@"view"];
        controller.largestMeasurement = 0;
        [controller renderCandidates];
        assert(controller.largestMeasurement > 0 && controller.largestMeasurement <= MSIMEAdaptiveCandidateMeasureChunk);
        assertArrows(NO, YES);
        session.texts = [many subarrayWithRange:NSMakeRange(0, 40)];
        [controller setValue:[session viewWithError:nil] forKey:@"view"];
        [controller renderCandidates];
        NSUInteger expectedStart = 0, pages = 1;
        for (;;) {
            NSArray *page = [controller valueForKey:@"adaptivePageCandidates"];
            assert(page.count && [page.firstObject[@"id"][@"index"] unsignedIntegerValue] == expectedStart);
            expectedStart += page.count;
            if (!CandidateButton(panel.contentView, -2).enabled) break;
            assertArrows(pages > 1, YES);
            NSDictionary *next = [controller candidateCommand:MSIME_NEXT_PAGE];
            assert(next && session.cursor == expectedStart);
            [controller setValue:next[@"view"] forKey:@"view"];
            [controller renderCandidates];
            assert(++pages < 40);
        }
        assert(pages > 2 && expectedStart == 40);
        assertArrows(YES, NO);
        appearance.showsCandidatePageNumber = NO;
        session.cursor = 0;
    }
    // 展开网格里移动光标只换高亮和视口：逐行几何沿用上一次，几千个候选也不每步重测；外观变了照常重排。
    for (NSString *mode in @[@"adaptive", @"fixed"]) {
        [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":mode,
            @"candidate_max_width":@400}];
        NSArray *shapes = @[@"合", @"合成", @"合成词", @"合成词组"];
        NSMutableArray *many = [NSMutableArray array];
        for (NSUInteger index = 0; index < 120; ++index) [many addObject:shapes[index % shapes.count]];
        session.texts = many;
        session.cursor = 0;
        [controller setValue:[session viewWithError:nil] forKey:@"view"];
        [controller renderCandidates];
        assert([controller openExpandedCandidates]);
        auto key = [](unsigned short code) {
            return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0
                windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:code];
        };
        auto highlightedIndex = [&] {
            for (NSView *view in MSIMECandidateControlSubviews(panel.contentView))
                if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag >= 0 && ((MSIMECandidateButton *)view).candidateHighlighted)
                    return [((MSIMECandidateButton *)view).candidateID[@"index"] unsignedIntegerValue];
            return (NSUInteger)NSNotFound;
        };
        const CGFloat gridWidth = NSWidth(panel.frame);
        for (unsigned short code : {125, 125, 124, 126}) {
            controller.gridMeasuredItems = 0;
            assert([controller handleExpandedCandidateEvent:key(code) client:client]);
            // 已排的行一律沿用；自动适应下移可能追加新行，只量新增的那一批，不重测已排的。
            if ([mode isEqual:@"adaptive"] && code == 125)
                assert(controller.gridMeasuredItems <= 2 * MSIMEExpandedGridMeasureChunk);
            else
                assert(controller.gridMeasuredItems == 0);
            assert(highlightedIndex() == [[controller valueForKey:@"expandedCursor"] unsignedIntegerValue]);
            assert(fabs(NSWidth(panel.frame) - gridWidth) < 0.5);
        }
        assert([[controller valueForKey:@"expandedCursor"] unsignedIntegerValue] > 0);
        // 外观变了，下一次移动光标照常重排，网格换成新宽度。
        [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":mode,
            @"candidate_max_width":@600, @"candidate_columns":@3}];
        appearance.pageSize = 4;
        controller.gridMeasuredItems = 0;
        assert([controller handleExpandedCandidateEvent:key(125) client:client]);
        // 固定数量的网格全量重排；自动适应按需排版，只重排到视口之后几行。
        if ([mode isEqual:@"adaptive"]) {
            assert(controller.gridMeasuredItems > 0 && controller.gridMeasuredItems < 2 * many.count);
            assert(NSWidth(panel.frame) > gridWidth + 100);
        } else {
            assert(controller.gridMeasuredItems >= many.count);
        }
        assert(highlightedIndex() == [[controller valueForKey:@"expandedCursor"] unsignedIntegerValue]);
        // 收起时引擎高亮会跟到光标那一项；替身会话不处理那条路径，先把光标放回首项。
        [controller setValue:@0 forKey:@"expandedCursor"];
        [controller closeExpandedCandidates];
        appearance.pageSize = 6;
        [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"adaptive",
            @"candidate_max_width":@400, @"candidate_columns":@6}];
    }
    // 自动适应的展开网格按需排版：三千项展开时只排 20 行；不显示行号，滚动条照常；往下走、往下滚时接着排，画面不跳。
    {
        appearance.showsCandidatePageNumber = YES;
        NSArray *shapes = @[@"合", @"合成", @"合成词", @"合成词组"];
        NSMutableArray *many = [NSMutableArray array];
        for (NSUInteger index = 0; index < 3000; ++index) [many addObject:shapes[index % shapes.count]];
        session.texts = many;
        session.cursor = 0;
        [controller setValue:[session viewWithError:nil] forKey:@"view"];
        [controller renderCandidates];
        controller.gridMeasuredItems = 0;
        assert([controller openExpandedCandidates]);
        assert(controller.gridMeasuredItems > 0 && controller.gridMeasuredItems < 400);
        for (NSView *child in MSIMECandidateControlSubviews(panel.contentView))
            assert(![child.identifier isEqual:@"candidate-page-indicator"]);
        MSIMEExpandedCandidateScrollView *grid = [controller valueForKey:@"expandedScrollView"];
        assert(grid && grid.hasVerticalScroller && grid.hasMoreRows);
        // 展开后立刻排出 20 行：暂时关掉追加，滚到底数出已排的行数，再滚回顶部。
        grid.hasMoreRows = NO;
        [grid scrollToTopOffset:CGFLOAT_MAX];
        assert(NSMaxRange(grid.visibleRowRange) == 20);
        grid.hasMoreRows = YES;
        [grid scrollToTopOffset:0];
        assert(grid.visibleRowRange.location == 0);
        auto key = [](unsigned short code) {
            return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0
                windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:code];
        };
        auto highlighted = [&] {
            for (NSView *view in MSIMECandidateControlSubviews(panel.contentView))
                if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag >= 0 && ((MSIMECandidateButton *)view).candidateHighlighted)
                    return (NSInteger)view.tag;
            return (NSInteger)-1;
        };
        // 一行一行往下走到远超首次排出的范围，每一步高亮都跟着光标。
        NSUInteger previous = 0;
        for (NSUInteger step = 0; step < 60; ++step) {
            assert([controller handleExpandedCandidateEvent:key(125) client:client]);
            const NSUInteger cursor = [[controller valueForKey:@"expandedCursor"] unsignedIntegerValue];
            assert(cursor > previous && highlighted() == (NSInteger)cursor);
            previous = cursor;
        }
        // 翻屏同样不会停在已排的末行。
        for (NSUInteger step = 0; step < 5; ++step) {
            assert([controller handleExpandedCandidateEvent:key(121) client:client]);
            const NSUInteger cursor = [[controller valueForKey:@"expandedCursor"] unsignedIntegerValue];
            assert(cursor > previous && highlighted() == (NSInteger)cursor);
            previous = cursor;
        }
        assert(controller.gridMeasuredItems < many.count);
        for (NSView *child in MSIMECandidateControlSubviews(panel.contentView))
            assert(![child.identifier isEqual:@"candidate-page-indicator"]);
        // 滚到已排的末尾：追加行，文档变高，看到的首行不变。
        grid = [controller valueForKey:@"expandedScrollView"];
        const CGFloat before = NSHeight(grid.documentView.frame);
        const CGFloat bottom = before - NSHeight(grid.contentView.bounds);
        [grid scrollToTopOffset:bottom];
        assert(NSHeight(grid.documentView.frame) > before);
        const NSRange visibleRows = grid.visibleRowRange;
        const CGFloat top = NSHeight(grid.documentView.frame) - NSMaxY(grid.documentVisibleRect);
        assert(fabs(top - bottom) < 1 && visibleRows.length > 0);
        // 看得见的按钮都在视口里，没有因为文档变高而错位。
        const NSRect shown = grid.documentVisibleRect;
        NSUInteger inside = 0;
        for (NSView *view in grid.documentView.subviews)
            if (NSIntersectsRect(view.frame, shown)) ++inside;
        assert(inside > 0);
        for (NSView *view in grid.documentView.subviews)
            assert(NSMinY(view.frame) >= -1 && NSMaxY(view.frame) <= NSHeight(grid.documentView.frame) + 1);
        [controller setValue:@0 forKey:@"expandedCursor"];
        [controller closeExpandedCandidates];
        appearance.showsCandidatePageNumber = NO;
    }
    session.cursor = 0;
    [appearance applySharedCandidatePreferences:@{@"candidate_count_mode":@"adaptive", @"candidate_max_width":@400}];
    session.texts = texts;
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [controller renderCandidates];
    assert(fabs(width() - 400) < 1);
    NSArray *first = [controller valueForKey:@"adaptivePageCandidates"];
    assert(first.count > 1 && first.count < session.texts.count);
    assert(CandidateButton(panel.contentView, (NSInteger)first.count) == nil);
    assertTightColumns();
    NSDictionary *next = [controller candidateCommand:MSIME_NEXT_PAGE];
    assert(session.cursor == first.count);
    [controller setValue:next[@"view"] forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateButton *button = CandidateButton(panel.contentView, 0);
    [controller selectCandidate:button];
    assert(session.selected == first.count);
    session.selected = NSNotFound;
    NSMutableDictionary *staleID = [button.candidateID mutableCopy];
    staleID[@"generation"] = @6;
    button.candidateID = staleID;
    [controller selectCandidate:button];
    assert(session.selected == NSNotFound);
    [controller candidateCommand:MSIME_PREVIOUS_PAGE];
    [controller renderCandidates];
    assert(session.cursor == 0);
    const CGFloat collapsedHeight = NSHeight(panel.frame);
    const NSInteger windowNumber = panel.windowNumber;
    assert([controller openExpandedCandidates]);
    assert(NSHeight(panel.frame) > collapsedHeight && panel.windowNumber == windowNumber);
    assert(fabs(width() - 400) < 1);
    MSIMEExpandedCandidateScrollView *viewport = [controller valueForKey:@"expandedScrollView"];
    assert(viewport);
    MSIMECandidateButton *wide = CandidateButton(panel.contentView, 2);
    MSIMECandidateButton *shortWord = CandidateButton(panel.contentView, 0);
    assert(wide && shortWord && NSWidth(wide.frame) >= 2 * NSWidth(shortWord.frame) - 1);
    const auto chrome = (MSIMECandidateChromeView *)panel.contentView;
    assert(NSMaxX(chrome.candidateRowsRect) <= NSWidth(chrome.cardBounds) - MSIMEExpandedCandidateScrollerPadding(1));
    AssertNoCandidateLayoutAnimation(panel.contentView);
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"adaptive",
        @"candidate_max_width":@500}];
    [controller renderCandidates];
    assert(fabs(width() - 500) < 1);
    AssertNoCandidateLayoutAnimation(panel.contentView);
    [controller setValue:@20 forKey:@"expandedCursor"];
    [controller renderCandidates];
    AssertNoCandidateLayoutAnimation(panel.contentView);
    [controller setValue:@0 forKey:@"expandedCursor"];
    [controller renderCandidates];
    AssertNoCandidateLayoutAnimation(panel.contentView);
    const CGFloat expandedHeight = NSHeight(panel.frame);
    MSIMECandidateMotion *layoutMotion = [controller valueForKey:@"candidateMotion"];
    [controller closeExpandedCandidates];
    assert(NSHeight(panel.frame) < expandedHeight && panel.windowNumber == windowNumber);
    [texts exchangeObjectAtIndex:0 withObjectAtIndex:2];
    session.texts = texts;
    NSMutableDictionary *typingView = [[session viewWithError:nil] mutableCopy];
    typingView[@"editing_text"] = @"synthetic-update";
    [controller setValue:typingView forKey:@"view"];
    [controller renderCandidates];
    AssertNoCandidateLayoutAnimation(panel.contentView);
    assert(!layoutMotion.exiting && !layoutMotion.exitWindow.isVisible);
    // 快速切换与两种系统动态效果设置都直接到最终尺寸，不创建中间窗口。
    for (BOOL reduced : {NO, YES}) {
        layoutMotion.reduceMotion = reduced;
        for (NSUInteger repeat = 0; repeat < 3; ++repeat) {
            const NSRect collapsed = panel.frame;
            assert([controller openExpandedCandidates]);
            assert(NSHeight(panel.frame) > NSHeight(collapsed) && panel.windowNumber == windowNumber);
            AssertNoCandidateLayoutAnimation(panel.contentView);
            [controller closeExpandedCandidates];
            assert(NSEqualRects(panel.frame, collapsed) && panel.windowNumber == windowNumber);
            AssertNoCandidateLayoutAnimation(panel.contentView);
        }
        [controller hideCandidatePanel:"deactivate"];
        // 此夹具固定报告可见；实际窗口显隐由 candidate-panel 和声明皮肤用例验证。
        assert(!layoutMotion.exiting && !layoutMotion.exitWindow.isVisible);
        [controller renderCandidates];
    }
    layoutMotion.reduceMotion = NO;
    for (NSNumber *maximum in @[@160, @400, @600]) {
        for (NSNumber *scale in @[@50, @100, @200]) {
            for (NSNumber *columns in @[@3, @6, @10]) {
                appearance.candidateScalePercent = scale.integerValue;
                [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"adaptive",
                    @"candidate_max_width":maximum, @"candidate_columns":columns}];
                [controller renderCandidates];
                assert(fabs(width() - maximum.doubleValue) < 1);
                for (NSView *cell in MSIMECandidateControlSubviews(panel.contentView))
                    if ([cell isKindOfClass:MSIMECandidateButton.class] && cell.tag >= 0)
                        assert(NSMinX(cell.frame) >= 0 && NSMaxX(cell.frame) <= width() + 0.5);
                assert([controller openExpandedCandidates]);
                assert(fabs(width() - maximum.doubleValue) < 1);
                for (NSView *cell in MSIMECandidateControlSubviews(panel.contentView))
                    if ([cell isKindOfClass:MSIMECandidateButton.class] && cell.tag >= 0)
                        assert(NSMinX(cell.frame) >= 0 && NSMaxX(cell.frame) <= width() + 0.5);
                [controller closeExpandedCandidates];
            }
        }
    }
    appearance.candidateScalePercent = 100;
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"horizontal", @"candidate_count_mode":@"adaptive"}];
    session.texts = @[@"甲"];
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [controller renderCandidates];
    assert(width() < 160);
    const CGFloat shortWidth = width();
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"vertical", @"candidate_count_mode":@"adaptive"}];
    [controller renderCandidates];
    const CGFloat verticalWidth = width();
    [appearance applySharedCandidatePreferences:@{@"candidate_layout":@"vertical", @"candidate_count_mode":@"fixed"}];
    [controller renderCandidates];
    assert(fabs(width() - verticalWidth) < 0.1 && width() > shortWidth);
    [layoutMotion stop];
    [panel orderOut:nil];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 直接检查真实绘制方法的四条直边，布局矩形贴边不能代替背景像素贴边。
static void TestCandidateFillEdges()
{
    for (NSInteger backingScale : {1, 2}) for (BOOL fourCorners : {NO, YES}) for (BOOL selected : {NO, YES}) {
        MSIMECandidateButton *button = [[MSIMECandidateButton alloc] initWithFrame:NSMakeRect(0, 0, 64, 32)];
        button.tag = 0;
        button.title = @"";
        button.font = [NSFont systemFontOfSize:16];
        button.cornerRadius = 6;
        button.hasCornerRadii = fourCorners;
        button.cornerRadii = {8, 6, 4, 2};
        button.candidateHighlighted = selected;
        button.candidateHovered = !selected;
        button.fillColor = NSColor.redColor;
        button.hoverColor = NSColor.blueColor;
        const NSInteger width = 64 * backingScale, height = 32 * backingScale;
        NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr
            pixelsWide:width pixelsHigh:height bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES
            isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
        assert(bitmap);
        [NSGraphicsContext saveGraphicsState];
        NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap];
        CGContextScaleCTM(NSGraphicsContext.currentContext.CGContext, backingScale, backingScale);
        [button drawRect:button.bounds];
        [NSGraphicsContext restoreGraphicsState];
        for (const NSPoint point : {NSMakePoint(0, height / 2), NSMakePoint(width - 1, height / 2),
                                   NSMakePoint(width / 2, 0), NSMakePoint(width / 2, height - 1)}) {
            NSColor *pixel = [[bitmap colorAtX:point.x y:point.y] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
            assert(pixel.alphaComponent > 0.99);
            assert(fabs(pixel.redComponent - (selected ? 1.0 : 0.0)) < 0.05);
            assert(fabs(pixel.blueComponent - (selected ? 0.0 : 1.0)) < 0.05);
            assert(pixel.greenComponent < 0.05);
        }
    }
}

static void Spin(NSTimeInterval seconds)
{
    [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:seconds]];
}

static NSDictionary *MotionView(NSString *editing, NSUInteger count, NSUInteger highlighted)
{
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSUInteger slot = 0; slot < count; ++slot)
        [candidates addObject:@{@"text": [NSString stringWithFormat:@"候选%lu", (unsigned long)slot], @"highlighted": @(slot == highlighted)}];
    return @{@"focused": @YES, @"editing_text": editing, @"page": @0, @"page_count": @1, @"candidates": candidates};
}

// 产品路径（InputController）在皮肤包写了动效时的行为：每次排版都换一个新的内容视图，显示和装饰在同一轮事件里发生，候选行这时还没有自己的图层。
static void TestDeclaredMotion(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-candidate-motion-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    std::filesystem::create_directory(root / "declared-motion");
    {
        std::ofstream manifest(root / "declared-motion" / "skin.toml");
        manifest << R"toml(schema_version = 2
id = "declared-motion"
name = "Declared Motion"
version = "1"
base = "system"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 240
[animation]
enter = "fade_scale"
exit = "fade"
selection = "spring"
duration_ms = 400
stagger_ms = 20
)toml";
        assert(manifest.good());
    }
    NSString *suite = [@"app.msime.test.motion." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:@"declared-motion" forKey:@"MSIMEClientCustomCandidateSkin"];
    MSIMEAppearancePreferences *appearance =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    appearance.vertical = YES;
    const auto declared = [appearance candidateWindowSkinForDark:NO].animation;
    assert(declared.enter == msime::mac::SkinAnimationPreset::fade_scale && declared.selection == msime::mac::SkinAnimationPreset::spring &&
           declared.durationMs == 400.0 && declared.staggerMs == 20.0);
    // A canvas base carries the same v2 motion contract as any other base; canvas only adds the nine-slice and widget fields.
    std::filesystem::create_directory(root / "canvas-motion");
    {
        std::ofstream manifest(root / "canvas-motion" / "skin.toml");
        manifest << R"toml(schema_version = 2
id = "canvas-motion"
name = "Canvas Motion"
version = "1"
base = "canvas"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 240
[animation]
enter = "slide_up"
exit = "fade"
selection = "glow"
duration_ms = 240
stagger_ms = 10
)toml";
        assert(manifest.good());
    }
    {
        NSString *canvasSuite = [@"app.msime.test.motion.canvas." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *canvasDefaults = [[NSUserDefaults alloc] initWithSuiteName:canvasSuite];
        [canvasDefaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [canvasDefaults setObject:@"canvas-motion" forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *canvasAppearance =
            [[MSIMEAppearancePreferences alloc] initWithDefaults:canvasDefaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        const auto canvas = [canvasAppearance candidateWindowSkinForDark:NO].animation;
        assert(canvas.enter == msime::mac::SkinAnimationPreset::slide_up && canvas.selection == msime::mac::SkinAnimationPreset::glow &&
               canvas.durationMs == 240.0 && canvas.staggerMs == 10.0);
        MSIMERemoveTestPreferenceSuite(canvasDefaults, canvasSuite);
    }
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    MSIMECandidateMotion *motion = [MSIMECandidateMotion new];
    // 不依赖这台机器的「减弱动态效果」。
    motion.reduceMotion = NO;
    [controller setValue:motion forKey:@"candidateMotion"];

    // 两种系统动态效果设置下，显式皮肤预设都不能延迟候选和高亮。
    for (BOOL reduced : {NO, YES}) {
        motion.reduceMotion = reduced;
        [controller setValue:MotionView(@"ce", 9, 0) forKey:@"view"];
        [controller renderCandidates];
        NSPanel *panel = [controller valueForKey:@"panel"];
        assert(panel.isVisible && panel.alphaValue == 1.0);
        assert([panel.contentView.layer animationForKey:@"msime-candidate-enter-opacity"] == nil);
        assert([panel.contentView.layer animationForKey:@"msime-candidate-enter-transform"] == nil);
        for (NSInteger tag = 0; tag < 9; ++tag)
            assert([CandidateButton(panel.contentView, tag).layer animationForKey:@"msime-candidate-row-reveal"] == nil);
        for (NSUInteger highlighted : {2u, 0u, 1u}) {
            [controller setValue:MotionView(@"ce", 9, highlighted) forKey:@"view"];
            [controller renderCandidates];
            MSIMECandidateButton *selected = CandidateButton(panel.contentView, highlighted);
            assert(selected.candidateHighlighted && !selected.selectionFillDetached);
            assert(motion.selectionLayer == nil);
        }
        [controller hideCandidatePanel:"empty"];
        assert(!panel.isVisible && !motion.exiting && !motion.exitWindow.isVisible);
        [controller setValue:MotionView(@"ceshi", 1, 0) forKey:@"view"];
        [controller renderCandidates];
        assert(panel.isVisible && panel.alphaValue == 1.0);
        assert([panel.contentView.layer animationForKey:@"msime-candidate-enter-opacity"] == nil);
        [controller hideCandidatePanel:"focus_out"];
        assert(!panel.isVisible && !motion.exiting);
    }
    [motion stop];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    std::filesystem::remove_all(root);
}

// 写一张 width×height、frames 帧、每帧 delay 秒、无限循环的 APNG，各帧颜色不同。
static void WriteApng(const std::filesystem::path &path, size_t width, size_t height, size_t frames, double delay)
{
    CGImageDestinationRef destination =
        CGImageDestinationCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(path.c_str())], CFSTR("public.png"), frames, nullptr);
    assert(destination != nullptr);
    CGColorSpaceRef space = CGColorSpaceCreateDeviceRGB();
    for (size_t index = 0; index < frames; ++index)
    {
        CGContextRef context = CGBitmapContextCreate(nullptr, width, height, 8, 0, space, kCGImageAlphaPremultipliedLast);
        CGContextSetRGBFillColor(context, index % 2, 0.4, (index + 1) % 2, 1.0);
        CGContextFillRect(context, CGRectMake(0, 0, width, height));
        CGImageRef image = CGBitmapContextCreateImage(context);
        CGImageDestinationAddImage(destination, image, (__bridge CFDictionaryRef) @{
            (__bridge NSString *)kCGImagePropertyPNGDictionary : @{(__bridge NSString *)kCGImagePropertyAPNGDelayTime : @(delay)}
        });
        CGImageRelease(image);
        CGContextRelease(context);
    }
    CGColorSpaceRelease(space);
    assert(CGImageDestinationFinalize(destination));
    CFRelease(destination);
}

static MSIMECandidateDecorationView *DecorationView(NSView *content)
{
    for (NSView *view in content.subviews)
        if ([view isKindOfClass:MSIMECandidateDecorationView.class]) return (MSIMECandidateDecorationView *)view;
    return nil;
}

// 产品路径的装饰图待机动效和 APNG 帧动画：每次排版都换新的内容视图和装饰视图，动画跟着挂到新视图上，从同一时刻起算，所以打字时装饰图不会每个键都跳回原位；旧视图上的动画撤掉；收起和减弱动态效果时不动。
static void TestDecorationMotion(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-decoration-motion-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    std::filesystem::create_directory(root / "whale");
    WriteApng(root / "whale" / "whale.png", 60, 80, 6, 0.12);
    {
        std::ofstream manifest(root / "whale" / "skin.toml");
        manifest << R"toml(schema_version = 2
id = "whale"
name = "Whale"
version = "1"
base = "system"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 240
[candidate_window.decoration]
image = "whale.png"
top_inset_dip = 48
width_dip = 60
motion = "sway"
motion_period_ms = 2400
)toml";
        assert(manifest.good());
    }
    NSString *suite = [@"app.msime.test.decoration." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:@"whale" forKey:@"MSIMEClientCustomCandidateSkin"];
    MSIMEAppearancePreferences *appearance =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    appearance.vertical = YES;
    const auto skin = [appearance candidateWindowSkinForDark:NO];
    assert(skin.decorationMotion == msime::mac::DecorationMotion::sway && skin.decorationMotionPeriodMs == 2400.0);
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    MSIMECandidateMotion *motion = [MSIMECandidateMotion new];
    motion.reduceMotion = NO;
    [controller setValue:motion forKey:@"candidateMotion"];

    [controller setValue:MotionView(@"ce", 5, 0) forKey:@"view"];
    [controller renderCandidates];
    NSPanel *panel = [controller valueForKey:@"panel"];
    MSIMECandidateDecorationView *first = DecorationView(panel.contentView);
    assert(panel.isVisible && first != nil && motion.animatedDecoration == first && first.frames.images.count == 6);
    CAKeyframeAnimation *sway = (id)[first.layer animationForKey:@"msime-decoration-motion"];
    CAKeyframeAnimation *frames = (id)[first.layer animationForKey:@"msime-decoration-frames"];
    assert(sway != nil && std::abs(sway.duration - 2.4) < 1e-6 && sway.repeatCount == HUGE_VALF);
    assert(frames != nil && frames.values.count == 6 && std::abs(frames.duration - 0.72) < 0.01);
    // 摇摆以底边中点为轴：最大角度时底边中点不动，顶边横移不超过留出的余量。
    const CATransform3D tilted = [sway.values[1] CATransform3DValue];
    const CGRect bounds = first.layer.bounds;
    const CGPoint anchor = CGPointMake(CGRectGetMinX(bounds) + first.layer.anchorPoint.x * CGRectGetWidth(bounds),
                                       CGRectGetMinY(bounds) + first.layer.anchorPoint.y * CGRectGetHeight(bounds));
    const CGPoint bottom = CGPointApplyAffineTransform(CGPointMake(CGRectGetMidX(bounds) - anchor.x, CGRectGetMinY(bounds) - anchor.y),
                                                       CATransform3DGetAffineTransform(tilted));
    assert(std::abs(bottom.x - (CGRectGetMidX(bounds) - anchor.x)) < 0.01 && std::abs(bottom.y - (CGRectGetMinY(bounds) - anchor.y)) < 0.01);
    const CGPoint top = CGPointApplyAffineTransform(CGPointMake(CGRectGetMidX(bounds) - anchor.x, CGRectGetMaxY(bounds) - anchor.y),
                                                    CATransform3DGetAffineTransform(tilted));
    const CGFloat shift = std::abs(top.x - (CGRectGetMidX(bounds) - anchor.x));
    assert(shift > 1.0 && shift <= msime::mac::kDecorationMotionMargin + 0.01);
    // 装饰图四周留着余量，最大摇摆时也不越过窗口左右边和顶边。
    assert(NSMinX(first.frame) >= msime::mac::kDecorationMotionMargin - 0.01 &&
           NSMaxX(first.frame) <= NSWidth(panel.contentView.bounds) - msime::mac::kDecorationMotionMargin + 0.01 &&
           NSMaxY(first.frame) <= NSHeight(panel.contentView.bounds) - msime::mac::kDecorationMotionMargin + 0.01);
    const CFTimeInterval begin = [first.layer convertTime:sway.beginTime toLayer:nil];
    assert(std::abs(begin - motion.decorationAnchor) < 1e-3);

    // 打字：整页重排换了新的装饰视图，动画挂到新视图上、从同一时刻起算，旧视图上的撤掉。
    Spin(0.2);
    [controller setValue:MotionView(@"ces", 5, 0) forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateDecorationView *second = DecorationView(panel.contentView);
    assert(second != nil && second != first && motion.animatedDecoration == second);
    assert([first.layer animationForKey:@"msime-decoration-motion"] == nil);
    CAKeyframeAnimation *continued = (id)[second.layer animationForKey:@"msime-decoration-motion"];
    CAKeyframeAnimation *continuedFrames = (id)[second.layer animationForKey:@"msime-decoration-frames"];
    assert(std::abs([second.layer convertTime:continued.beginTime toLayer:nil] - begin) < 1e-3);
    assert(std::abs([second.layer convertTime:continuedFrames.beginTime toLayer:nil] - begin) < 1e-3);

    // 收起后不动。
    [controller hideCandidatePanel:"empty"];
    assert(!panel.isVisible && motion.animatedDecoration == nil && [second.layer animationForKey:@"msime-decoration-motion"] == nil);

    // 减弱动态效果时不动，APNG 停在第一帧。
    motion.reduceMotion = YES;
    Spin(0.5);
    [controller setValue:MotionView(@"ce", 5, 0) forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateDecorationView *still = DecorationView(panel.contentView);
    assert(panel.isVisible && still != nil && motion.animatedDecoration == nil && still.layer.animationKeys.count == 0 &&
           still.layer.contents == still.frames.images.firstObject);
    [controller hideCandidatePanel:"focus_out"];
    [motion stop];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    std::filesystem::remove_all(root);
}

// 读音单独占释义下面的一行。横排时格宽取候选、释义、读音三行里最宽的一行，长音标不再跟在释义后面把这一格撑到下一格的释义边上；同一行的各格一样高，释义都在同一高度。「显示读音」开着时横排一直为读音留出这一行：读音还没查回来、这一页没有读音，卡片都和有读音时一样高，读音到达时窗口不长高，光标靠近屏幕底边时也就不会从光标下方翻到上方；关着时高度和从前一样。
static void TestPronunciationLine(MSIMEInputController *controller, NSWindow *panel, MSIMEAppearancePreferences *appearance)
{
    appearance.vertical = NO;
    appearance.candidateTranslations = YES;
    NSString *ipa = @"/.ænɪˈmeɪʃən prəʊˈtægənɪst/";
    NSDictionary *view = @{@"focused": @YES, @"editing_text": @"donghuazhuti", @"page": @0, @"page_count": @1,
                           @"candidates": @[@{@"text": @"动画主体", @"translation": @"Animation protagonist", @"highlighted": @YES},
                                            @{@"text": @"东华主题", @"translation": @"Donghua theme"}]};
    [controller setValue:nil forKey:@"pronunciationTargets"];
    [controller setValue:[@{MSIMEPronunciationKey(@"en", @"Animation protagonist"): ipa} mutableCopy] forKey:@"sharedPronunciations"];
    [controller setValue:view forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateButton *plain = CandidateButton(panel.contentView, 0);
    assert(plain && plain.pronunciation.length == 0);
    const CGFloat plainHeight = NSHeight(plain.frame);
    const CGFloat glossLine = plain.itemLayout.translation.height;
    const CGFloat plainCard = NSHeight(panel.frame);

    // 打开读音：读音在释义下面单独一行，释义行的位置不变。
    [controller setValue:@[@"en"] forKey:@"pronunciationTargets"];
    [controller renderCandidates];
    MSIMECandidateButton *read = CandidateButton(panel.contentView, 0);
    MSIMECandidateButton *neighbour = CandidateButton(panel.contentView, 1);
    assert(read && neighbour && [read.pronunciation isEqual:ipa] && neighbour.pronunciation.length == 0);
    NSFont *glossFont = read.translationFont;
    const CGFloat glossWidth = MSIMECandidateSingleLineWidth(@"Animation protagonist", glossFont);
    const CGFloat ipaWidth = MSIMECandidateSingleLineWidth(ipa, glossFont);
    const msime::candidate::CandidateRunBox &run = read.itemLayout.translation;
    assert(run.below && fabs(run.y - plain.itemLayout.translation.y) < 0.5);
    assert(run.height > glossLine * 1.5 && run.height < glossLine * 2.5);
    // 每一行都放得下，不折行：格子按较宽的那一行量，不按「释义  读音」连在一起的宽度。
    assert(run.width >= glossWidth && run.width >= ipaWidth);
    assert(fabs(run.width - MAX(glossWidth, ipaWidth)) < 1.0);
    const CGFloat joined = MSIMECandidateSingleLineWidth([NSString stringWithFormat:@"Animation protagonist  %@", ipa], glossFont);
    assert(NSWidth(read.frame) < read.contentLeft + joined);
    // 画出来的释义和读音都在自己的格子里，两格不重叠。
    assert(read.contentLeft + run.x + run.width <= NSWidth(read.frame) + 0.5);
    assert(NSMaxX(read.frame) <= NSMinX(neighbour.frame) + 0.5);
    assert(neighbour.contentLeft + neighbour.itemLayout.translation.x + neighbour.itemLayout.translation.width <= NSWidth(neighbour.frame) + 0.5);
    // 有读音的这一页多出一行高，同一行的格子一样高、释义在同一高度，选中底色盖住整格连同读音那一行。
    assert(NSHeight(read.frame) > plainHeight + glossLine * 0.5 && NSHeight(panel.frame) > plainCard + glossLine * 0.5);
    assert(fabs(NSHeight(read.frame) - NSHeight(neighbour.frame)) < 0.5 && fabs(NSMinY(read.frame) - NSMinY(neighbour.frame)) < 0.5);
    assert(fabs(neighbour.itemLayout.translation.y - run.y) < 0.5);
    assert(NSHeight(read.frame) >= run.y + run.height - 0.5);
    NSBitmapImageRep *bitmap = [read bitmapImageRepForCachingDisplayInRect:read.bounds];
    [read cacheDisplayInRect:read.bounds toBitmapImageRep:bitmap];

    const CGFloat readHeight = NSHeight(read.frame);
    const CGFloat readCard = NSHeight(panel.frame);

    // 读音还没查回来：已经按有读音的高度留好那一行，查回来时卡片不长高。
    [controller setValue:[NSMutableDictionary dictionary] forKey:@"sharedPronunciations"];
    [controller renderCandidates];
    assert(CandidateButton(panel.contentView, 0).pronunciation.length == 0);
    assert(fabs(NSHeight(CandidateButton(panel.contentView, 0).frame) - readHeight) < 0.5 && fabs(NSHeight(panel.frame) - readCard) < 0.5);
    [controller setValue:[@{MSIMEPronunciationKey(@"en", @"Animation protagonist"): ipa} mutableCopy] forKey:@"sharedPronunciations"];

    // 这一页没有读音时也留着读音那一行，和有读音的页一样高，逐键换页时卡片高度不跳。
    NSMutableDictionary *unread = [view mutableCopy];
    unread[@"candidates"] = @[@{@"text": @"东华主题", @"translation": @"Donghua theme", @"highlighted": @YES}, @{@"text": @"冬化", @"translation": @"winterize"}];
    [controller setValue:[unread copy] forKey:@"view"];
    [controller renderCandidates];
    assert(fabs(NSHeight(CandidateButton(panel.contentView, 0).frame) - readHeight) < 0.5 && fabs(NSHeight(panel.frame) - readCard) < 0.5);
    // 「显示读音」关掉：没有第三行，回到原来的高度。
    [controller setValue:view forKey:@"view"];
    [controller setValue:nil forKey:@"pronunciationTargets"];
    [controller renderCandidates];
    assert(CandidateButton(panel.contentView, 0).pronunciation.length == 0);
    assert(fabs(NSHeight(CandidateButton(panel.contentView, 0).frame) - plainHeight) < 0.5 && fabs(NSHeight(panel.frame) - plainCard) < 0.5);

    // 只有释义变了、格子没变时按钮原地重画，读音跟着新释义换，不留上一条释义的读音。
    [controller setValue:@[@"en"] forKey:@"pronunciationTargets"];
    [controller setValue:[@{MSIMEPronunciationKey(@"en", @"ab"): @"/ab/", MSIMEPronunciationKey(@"en", @"ba"): @"/ba/"} mutableCopy] forKey:@"sharedPronunciations"];
    NSDictionary *(^swapView)(NSString *) = ^NSDictionary *(NSString *gloss) {
        return @{@"session": @1, @"generation": @3, @"focused": @YES, @"editing_text": @"d", @"page": @0, @"page_count": @1,
                 @"candidates": @[@{@"id": @{@"session": @1, @"generation": @3, @"index": @0}, @"text": @"的", @"translation": gloss, @"highlighted": @YES}]};
    };
    [controller setValue:swapView(@"ab") forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateButton *before = CandidateButton(panel.contentView, 0);
    assert([before.pronunciation isEqual:@"/ab/"]);
    [controller setValue:swapView(@"ba") forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateButton *after = CandidateButton(panel.contentView, 0);
    assert(after == before && [after.translation isEqual:@"ba"] && [after.pronunciation isEqual:@"/ba/"]);

    // 竖排：释义仍在候选同一行，读音在释义下面，行高容得下两行。
    appearance.vertical = YES;
    [controller setValue:view forKey:@"view"];
    [controller setValue:[@{MSIMEPronunciationKey(@"en", @"Animation protagonist"): ipa} mutableCopy] forKey:@"sharedPronunciations"];
    [controller renderCandidates];
    MSIMECandidateButton *vertical = CandidateButton(panel.contentView, 0);
    assert([vertical.pronunciation isEqual:ipa] && !vertical.itemLayout.translation.below);
    assert(vertical.itemLayout.translation.height > glossLine * 1.5 && NSHeight(vertical.frame) >= vertical.itemLayout.height - 0.5);
    assert(vertical.itemLayout.translation.width >= ipaWidth);
    [controller setValue:nil forKey:@"pronunciationTargets"];
    appearance.vertical = NO;
}

// 候选词翻译范围「仅当前高亮」：竖排时视图给高亮行标出 translation_reserved，这一行一直留出译文的高度（两种目标语言时两行，与横排的预留行数相同），回答没到、高亮移动、回答到了，窗口都一样高。
static void TestVerticalReservedGlossRow(MSIMEInputController *controller, NSWindow *panel, MSIMEAppearancePreferences *appearance)
{
    appearance.vertical = YES;
    appearance.candidateTranslations = YES;
    // 竖排的译文画在候选同一行，默认字号下两行译文还没有候选行高；放大译文字号，让两行译文比候选行高。
    [appearance applySharedCandidatePreferences:@{@"candidate_translation_font_size": @22}];
    [controller setValue:@[@"en", @"fr"] forKey:@"glossTargetLanguages"];
    NSArray<NSString *> *words = @[@"测试", @"侧室", @"策士"];
    NSDictionary *(^page)(NSUInteger, BOOL, NSString *) = ^NSDictionary *(NSUInteger highlighted, BOOL reserved, NSString *translation) {
        NSMutableArray *candidates = [NSMutableArray array];
        for (NSUInteger index = 0; index < words.count; ++index) {
            NSMutableDictionary *candidate = [@{@"text": words[index], @"highlighted": @(index == highlighted)} mutableCopy];
            if (index == highlighted && reserved) candidate[@"translation_reserved"] = @YES;
            if (index == highlighted && translation) candidate[@"translation"] = translation;
            [candidates addObject:candidate];
        }
        return @{@"focused": @YES, @"editing_text": @"ceshi", @"page": @0, @"page_count": @1, @"candidates": candidates};
    };
    CGFloat (^cardHeight)(NSDictionary *) = ^CGFloat(NSDictionary *view) {
        [controller setValue:view forKey:@"view"];
        [controller renderCandidates];
        return NSHeight(panel.frame);
    };
    // 两行译文比候选行高：不留的话，回答一到这一行就变高。
    const CGFloat plain = cardHeight(page(0, NO, nil));
    const CGFloat plainRow = NSHeight(CandidateButton(panel.contentView, 0).frame);
    const CGFloat glossed = cardHeight(page(0, NO, @"test\u2028essai"));
    const CGFloat glossedRow = NSHeight(CandidateButton(panel.contentView, 0).frame);
    assert(!CandidateButton(panel.contentView, 0).itemLayout.translation.below);
    assert(glossed > plain + 0.5 && glossedRow > plainRow + 0.5);
    // 留出的那一行和回答到了以后一样高，其他行不变。
    for (NSUInteger highlighted = 0; highlighted < words.count; ++highlighted) {
        assert(fabs(cardHeight(page(highlighted, YES, nil)) - glossed) < 0.5);
        for (NSUInteger index = 0; index < words.count; ++index) {
            const CGFloat height = NSHeight(CandidateButton(panel.contentView, (NSInteger)index).frame);
            assert(fabs(height - (index == highlighted ? glossedRow : plainRow)) < 0.5);
        }
        assert(fabs(cardHeight(page(highlighted, YES, @"test\u2028essai")) - glossed) < 0.5);
    }
    // 只有一种目标语言时只留一行，和一行译文到了以后一样高。
    [controller setValue:@[@"en"] forKey:@"glossTargetLanguages"];
    const CGFloat oneLine = cardHeight(page(1, NO, @"test"));
    assert(fabs(cardHeight(page(1, YES, nil)) - oneLine) < 0.5 && oneLine < glossed - 0.5);
    [controller setValue:nil forKey:@"glossTargetLanguages"];
    [appearance applySharedCandidatePreferences:@{@"candidate_translation_font_size": NSNull.null}];
    appearance.vertical = NO;
}

// 前景按它的不透明度叠到不透明的底色上后，和底色的 WCAG 对比度。
static double CandidateContrast(NSColor *foreground, NSColor *fill)
{
    NSColor *fore = [foreground colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    NSColor *back = [fill colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    auto linear = [](double c) { return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4); };
    auto luminance = [&](double r, double g, double b) { return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b); };
    const double a = fore.alphaComponent;
    const double over = luminance(fore.redComponent * a + back.redComponent * (1 - a), fore.greenComponent * a + back.greenComponent * (1 - a),
                                  fore.blueComponent * a + back.blueComponent * (1 - a));
    const double under = luminance(back.redComponent, back.greenComponent, back.blueComponent);
    return (MAX(over, under) + 0.05) / (MIN(over, under) + 0.05);
}

// 皮肤的选中底色不透明、释义色和它同一色系时，选中行的释义和读音换成按底色挑出的颜色，和底色至少 3:1；底色半透明时仍是皮肤的释义色。
static void TestSelectedGlossColour(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-selected-gloss-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    struct Palette { const char *id, *selected, *translation; bool opaque; };
    for (const Palette &palette : {Palette{"blue-on-blue", "#4D6BFE", "#2F5BD3", true}, Palette{"slate", "#23262F", "#6B7180", true},
                                   Palette{"translucent", "#2F5BD329", "#2F5BD3", false}}) {
        std::filesystem::create_directories(root / palette.id);
        {
            std::ofstream manifest(root / palette.id / "skin.toml");
            manifest << "schema_version = 2\nid = \"" << palette.id << "\"\nname = \"Selected Gloss\"\nversion = \"1\"\nbase = \"system\"\n"
                     << "[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n[candidate_window]\nmin_width_dip = 0\n";
            for (const char *mode : {"light", "dark"})
                manifest << "[candidate." << mode << "]\nselected = \"" << palette.selected << "\"\ntranslation = \"" << palette.translation << "\"\n";
            assert(manifest.good());
        }
        NSString *suite = [@"app.msime.test.selected-gloss." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(palette.id) forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *appearance =
            [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.vertical = NO;
        appearance.candidateTranslations = YES;
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                                                                    backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];
        [controller setValue:@[@"en"] forKey:@"pronunciationTargets"];
        [controller setValue:[@{MSIMEPronunciationKey(@"en", @"to"): @"/tuː/"} mutableCopy] forKey:@"sharedPronunciations"];
        [controller setValue:@{@"focused": @YES, @"editing_text": @"dao", @"page": @0, @"page_count": @1,
                               @"candidates": @[@{@"text": @"到", @"translation": @"to", @"highlighted": @YES}, @{@"text": @"道", @"translation": @"to"}]}
                      forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *selected = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *other = CandidateButton(panel.contentView, 1);
        assert(selected.candidateHighlighted && !other.candidateHighlighted && [selected.pronunciation isEqual:@"/tuː/"]);
        const auto tokens = [appearance candidateWindowSkinForDark:NO].tokens;
        assert(tokens.translation && tokens.selectedTranslation);
        assert([other.translationColor isEqual:SkinColor(*tokens.translation)]);
        if (palette.opaque) {
            assert(selected.fillColor.alphaComponent >= 0.85 && CandidateContrast(selected.translationColor, selected.fillColor) >= 3.0);
            assert(![selected.translationColor isEqual:SkinColor(*tokens.translation)]);
        } else {
            assert([selected.translationColor isEqual:SkinColor(*tokens.translation)]);
        }
        NSBitmapImageRep *bitmap = [selected bitmapImageRepForCachingDisplayInRect:selected.bounds];
        [selected cacheDisplayInRect:selected.bounds toBitmapImageRep:bitmap];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

// 皮肤的收紧和横排均分必须到达真实控制器，不能只在清单读模型里保留字段。
static void TestFactoryFitAndJustify(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-factory-row-layout-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    CGFloat regularWidth = 0;
    for (const char *identifier : {"regular", "compact", "justified"}) {
        const bool compact = std::string(identifier) == "compact";
        const bool justified = std::string(identifier) == "justified";
        std::filesystem::create_directory(root / identifier);
        {
            std::ofstream manifest(root / identifier / "skin.toml");
            manifest << "schema_version = 1\nid = \"" << identifier << "\"\nname = \"Synthetic Layout\"\nversion = \"1\"\nbase = \"microsoft\"\n"
                     << "[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n"
                     << "[candidate_window]\nmin_width_dip = " << (justified ? 260 : 0)
                     << "\nexpand_rows = " << (compact ? 2 : justified ? 8 : 4)
                     << "\nfit_content = " << (compact ? "true" : "false")
                     << "\n[candidate_window.horizontal]\nitem_gap = 11\njustify_rows = " << (justified ? "true" : "false") << "\n";
            assert(manifest.good());
        }
        NSString *suite = [@"app.msime.test.factory-row-layout." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(identifier) forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
            skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.vertical = NO;
        appearance.showsAppLogo = NO;
        appearance.showsCandidatePreedit = NO;
        appearance.showsCandidatePageNumber = NO;
        appearance.showsCandidateExpandButton = NO;
        appearance.candidateTranslations = NO;
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        RowLayoutPanel *panel = [[VisibleRowLayoutPanel alloc] initWithContentRect:NSZeroRect
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];
        [controller setValue:@{@"focused": @YES, @"editing_text": @"ce", @"page": @0, @"page_count": @1,
            @"candidates": justified ? @[@{@"text": @"甲", @"highlighted": @YES}, @{@"text": @"乙"}]
                                      : @[@{@"text": @"甲", @"highlighted": @YES}]} forKey:@"view"];
        [controller renderCandidates];
        const CGFloat initialWidth = NSWidth(((MSIMECandidateChromeView *)panel.contentView).cardBounds);
        assert(initialWidth < 160);
        NSDictionary *initialView = [controller valueForKey:@"view"];
        [controller setValue:@{@"focused": @YES, @"editing_text": @"ce", @"page": @0, @"page_count": @1,
            @"candidates": @[@{@"text": @"合成候选窗口宽度回归测试合成候选窗口宽度回归测试", @"highlighted": @YES}]} forKey:@"view"];
        [controller renderCandidates];
        assert(NSWidth(((MSIMECandidateChromeView *)panel.contentView).cardBounds) > initialWidth + 20);
        // 通过真实长短候选切换验证收窄，不能再注入已删除的历史最大宽度字段。
        [controller setValue:initialView forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *first = CandidateButton(panel.contentView, 0);
        assert(first && first.hasSkinLayout);
        const CGFloat width = NSWidth(((MSIMECandidateChromeView *)panel.contentView).cardBounds);
        if (compact) {
            assert(fabs(width - regularWidth) < 1 && fabs(width - initialWidth) < 1);
            assert(fabs(first.textLeftInset - first.textRightInset) < 0.1);
        } else if (justified) {
            MSIMECandidateButton *last = CandidateButton(panel.contentView, 1);
            assert(last && fabs(NSMaxX(last.frame) - (width - NSMinX(first.frame))) < 1);
            assert(NSWidth(first.frame) < 100 && NSWidth(last.frame) < 100);
        } else {
            regularWidth = width;
            assert(width < 160 && fabs(width - initialWidth) < 1);
        }
        if (!justified) {
            [controller setValue:@{@"focused": @YES, @"editing_text": @"ce", @"page": @0, @"page_count": @1,
                @"candidates": @[@{@"text": @"甲", @"highlighted": @YES}, @{@"text": @"乙"}]} forKey:@"view"];
            [controller renderCandidates];
            first = CandidateButton(panel.contentView, 0);
            MSIMECandidateButton *second = CandidateButton(panel.contentView, 1);
            assert(first && second && fabs(NSMinX(second.frame) - NSMaxX(first.frame) - 11) < 0.1);
        }
        // 可见行数来自同一份共享皮肤；末行仍能滚入视口，点击按钮只为可见行创建。
        appearance.pageSize = 3;
        NSMutableArray *expanded = [NSMutableArray array];
        for (NSUInteger index = 0; index < 30; ++index)
            [expanded addObject:@{@"text": @"合成", @"highlighted": @(index == 0)}];
        [controller setValue:expanded forKey:@"expandedCandidates"];
        [controller setValue:@0 forKey:@"expandedCursor"];
        [controller setValue:@0 forKey:@"expandedFirstRow"];
        const NSUInteger expectedRows = 1 + (compact ? 2 : justified ? 8 : 4);
        assert([controller renderExpandedCandidates]);
        assert([[controller valueForKey:@"expandedVisibleRows"] unsignedIntegerValue] == expectedRows);
        assert(CandidateButton(panel.contentView, expectedRows * 3 - 1));
        assert(!CandidateButton(panel.contentView, expectedRows * 3));
        [controller setValue:@29 forKey:@"expandedCursor"];
        assert([controller renderExpandedCandidates]);
        assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 10 - expectedRows);
        assert(CandidateButton(panel.contentView, 29) && !panel.hasNextPage && panel.hasPreviousPage);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

// 从真实候选视图检查四角及动效轮廓，不用读取清单字段代替绘制验收。
static void TestFactoryCornerRadii(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-candidate-corners-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    for (const char *identifier : {"follow", "plain", "square", "canvas-corners"}) {
        const bool canvas = std::string(identifier) == "canvas-corners";
        const bool square = std::string(identifier) == "square";
        const bool follow = std::string(identifier) != "plain";
        std::filesystem::create_directory(root / identifier);
        {
            std::ofstream manifest(root / identifier / "skin.toml");
            manifest << "schema_version = 2\nid = \"" << identifier << "\"\nname = \"Synthetic Corners\"\nversion = \"1\"\nbase = \""
                     << (canvas ? "canvas" : "microsoft") << "\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n"
                     << "[candidate_window]\ncorner_radius_dip = " << (square ? 0 : 16)
                     << "\nitem_corner_radius_dip = 2\nborder_width_dip = 0\npadding = 0\nitem_gap = 2\npreedit_margin_dip = [0]\n"
                     << "item_corners_follow_card = " << (follow ? "true" : "false") << "\n";
            if (canvas) manifest << "content_insets_dip = { top = 5, right = 9, bottom = 7, left = 11 }\n";
            assert(manifest.good());
        }
        NSString *suite = [@"app.msime.test.candidate-corners." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(identifier) forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
            skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.showsAppLogo = NO;
        appearance.showsCandidatePageNumber = NO;
        appearance.showsCandidateExpandButton = NO;
        appearance.candidateTranslations = NO;
        appearance.fontSize = 16;
        appearance.themeMode = @"light";
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        MSIMECandidatePanel *panel = [[MSIMECandidatePanel alloc] initWithContentRect:NSZeroRect
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];
        NSArray *candidates = @[@{@"text": @"合成", @"highlighted": @YES},
            @{@"text": [@"长候选" stringByPaddingToLength:1024 withString:@"长候选" startingAtIndex:0]}, @{@"text": @"末"}];
        [controller setValue:@{@"focused": @YES, @"editing_text": @"synthetic", @"page": @0, @"page_count": @1,
            @"candidates": candidates} forKey:@"view"];
        for (NSNumber *scaling in @[@100, @150]) for (NSNumber *vertical in @[@NO, @YES]) for (NSNumber *preedit in @[@NO, @YES])
            for (NSNumber *expanded in vertical.boolValue ? @[@NO] : @[@NO, @YES]) {
            appearance.candidateScalePercent = scaling.integerValue;
            appearance.vertical = vertical.boolValue;
            appearance.showsCandidatePreedit = preedit.boolValue;
            [controller setValue:expanded.boolValue ? candidates : nil forKey:@"expandedCandidates"];
            if (expanded.boolValue) assert([controller renderExpandedCandidates]);
            else [controller renderCandidates];
            auto skin = [appearance candidateWindowSkinForDark:NO];
            const double scale = scaling.doubleValue / 100.0;
            const double outer = MAX(0.01, skin.tokens.radius);
            MSIMECandidateChromeView *chrome = (id)panel.contentView;
            MSIMECandidateButton *first = CandidateButton(chrome, 0), *last = CandidateButton(chrome, 2);
            assert(first && last && first.hasCornerRadii == follow && last.hasCornerRadii == follow);
            CGPathRef path = [first createSelectionFillPath];
            if (!follow) { assert(path == nullptr); continue; }
            assert(path && CGRectEqualToRect(CGPathGetBoundingBox(path), first.bounds));
            assert(fabs(first.cornerRadii[0] - (preedit.boolValue ? first.cornerRadius : outer)) < 0.01);
            const double lastBottomLeft = expanded.boolValue ? last.cornerRadius : outer;
            if (expanded.boolValue) {
                // 展开共享列处于同一行，只有第一列贴着左下边界。
                assert(fabs(NSMinY(CandidateFrame(first)) - NSMinY(CandidateFrame(last))) < 0.01);
                assert(fabs(first.cornerRadii[3] - outer) < 0.01);
            }
            if (fabs(last.cornerRadii[3] - lastBottomLeft) >= 0.01)
                fprintf(stderr, "corner geometry skin=%s scale=%.1f vertical=%d preedit=%d expanded=%d area=%s first=%s last=%s radius=%.2f expected=%.2f\n",
                    identifier, scale, vertical.boolValue, preedit.boolValue, expanded.boolValue, NSStringFromRect(chrome.candidateRowsRect).UTF8String,
                    NSStringFromRect(CandidateFrame(first)).UTF8String, NSStringFromRect(CandidateFrame(last)).UTF8String, last.cornerRadii[3], lastBottomLeft);
            assert(fabs(last.cornerRadii[3] - lastBottomLeft) < 0.01);
            if (vertical.boolValue) {
                assert(fabs(first.cornerRadii[1] - (preedit.boolValue ? first.cornerRadius : outer)) < 0.01);
                assert(fabs(last.cornerRadii[2] - outer) < 0.01);
            } else {
                // 此夹具右缘没有贴满候选区域，不能凭“末候选”借右下角。
                assert(NSMaxX(CandidateFrame(last)) + scale < NSMaxX(chrome.candidateRowsRect));
                assert(fabs(last.cornerRadii[2] - last.cornerRadius) < 0.01);
            }
            // 显式高亮动效也不接管静态四角高亮。
            MSIMECandidateMotion *motion = [MSIMECandidateMotion new];
            motion.reduceMotion = NO;
            msime::mac::SkinAnimation animation;
            animation.selection = msime::mac::SkinAnimationPreset::glow;
            animation.durationMs = 100;
            [motion decorateSelectionOfRow:first fill:NSColor.redColor cornerRadius:first.cornerRadius accent:NSColor.blueColor animation:animation];
            assert(motion.selectionLayer == nil && !first.selectionFillDetached);
            [motion endSelection];
            assert(!first.selectionFillDetached);
            CGPathRelease(path);
        }
        [controller setValue:nil forKey:@"expandedCandidates"];
        [panel orderOut:nil];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

// 实际行间矩形检查基础行距、隐藏收紧和 canvas 显式行距的优先级，包含翻页首行。
static void TestSkinPreeditGap(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-hidden-preedit-gap-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    for (const char *identifier : {"loose", "tight", "override", "follow", "square", "canvas-loose", "canvas-tight"}) {
        const std::string name(identifier);
        const bool canvas = name.find("canvas-") == 0;
        const bool follow = name == "follow" || name == "square";
        const bool tight = name == "tight" || name == "canvas-tight" || name == "override";
        std::filesystem::create_directory(root / identifier);
        {
            std::ofstream manifest(root / identifier / "skin.toml");
            manifest << "schema_version = 2\nid = \"" << identifier << "\"\nname = \"Synthetic Hidden Gap\"\nversion = \"1\"\nbase = \""
                     << (canvas ? "canvas" : "microsoft") << "\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n"
                     << "[candidate_window]\ncorner_radius_dip = " << (name == "square" ? 0 : 16)
                     << "\npadding = 8\npage_arrows = true\nexpand_panel = false\npreedit_margin_dip = [7, 11, 13, 17]\nitem_corners_follow_card = " << (follow ? "true" : "false")
                     << "\ntrim_hidden_preedit_gap = " << (tight ? "true" : "false") << "\n";
            if (canvas) manifest << "line_gap_dip = 19\ncontent_insets_dip = { top = 5, right = 9, bottom = 13, left = 17 }\n";
            if (name == "canvas-loose") manifest << "page_arrow_style = \"triangle\"\n";
            if (name == "override") manifest << "[candidate_window.vertical]\ntrim_hidden_preedit_gap = false\n";
            assert(manifest.good());
        }
        NSString *suite = [@"app.msime.test.hidden-preedit-gap." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(identifier) forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
            skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.showsCandidatePageNumber = NO;
        appearance.showsCandidateExpandButton = NO;
        appearance.showsCandidatePreedit = NO;
        appearance.candidateTranslations = NO;
        appearance.preeditFontSize = 24;
        appearance.themeMode = @"light";
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];
        NSArray *candidates = @[@{@"text": @"甲", @"highlighted": @YES}, @{@"text": @"乙"}];
        [controller setValue:@{@"focused": @YES, @"editing_text": @"synthetic", @"page": @0, @"page_count": @3,
            @"candidates": candidates} forKey:@"view"];
        for (NSUInteger scaling = 0; scaling < 2; ++scaling) for (NSUInteger vertical = 0; vertical < 2; ++vertical)
            for (NSUInteger expanded = 0; expanded < (vertical ? 1 : 2); ++expanded) for (BOOL controls : {NO, YES}) for (BOOL shown : {NO, YES}) {
                appearance.candidateScalePercent = scaling ? 150 : 100;
                appearance.vertical = vertical;
                appearance.showsAppLogo = NO;
                appearance.showsCandidatePageNumber = controls;
                appearance.showsCandidatePreedit = shown;
                const CGFloat scale = scaling ? 1.5 : 1.0;
                [controller setValue:expanded ? candidates : nil forKey:@"expandedCandidates"];
                if (expanded) assert([controller renderExpandedCandidates]);
                else [controller renderCandidates];
                MSIMECandidateChromeView *chrome = (id)panel.contentView;
                MSIMECandidateButton *first = CandidateButton(chrome, 0);
                assert(first && first.hasSkinLayout);
                const CGFloat padding = (canvas ? 5 : 8) * scale;
                const bool trimmed = follow || (tight && !(name == "override" && vertical));
                const CGFloat expectedGap = (trimmed ? 0 : 2) * scale;
                const CGFloat header = expanded && controls ? MSIMECandidateHeaderHeight * scale : 0;
                NSTextField *preedit = nil;
                for (NSView *child in chrome.candidateContentView.subviews)
                    if ([child.identifier isEqual:@"candidate-preedit"]) preedit = (id)child;
                assert((preedit != nil) == shown);
                const CGFloat actualGap = shown ? NSMinY(preedit.frame) - NSMaxY(CandidateFrame(first)) - (canvas ? 0 : 13 * scale)
                    : NSHeight(chrome.cardBounds) - padding - NSMaxY(CandidateFrame(first)) - header;
                const CGFloat expected = shown ? (canvas ? 19 : 2) * scale : expectedGap;
                const CGFloat tolerance = 1.0 / panel.backingScaleFactor + 0.01;
                if (fabs(actualGap - expected) > tolerance)
                    fprintf(stderr, "preedit row gap: skin=%s scale=%.1f vertical=%lu expanded=%lu controls=%d shown=%d actual=%.2f expected=%.2f\n",
                        identifier, scale, (unsigned long)vertical, (unsigned long)expanded, controls, shown, actualGap, expected);
                assert(fabs(actualGap - expected) <= tolerance);
                assert(chrome.candidateRowsTouchTop == (!shown && !(expanded && controls) && trimmed));
                if (controls && !expanded) {
                    MSIMECandidatePagerButton *arrow = (id)CandidateButton(chrome, -1);
                    assert([arrow isKindOfClass:MSIMECandidatePagerButton.class]);
                    if (vertical) {
                        MSIMECandidateButton *last = CandidateButton(chrome, 1);
                        assert(NSMaxY(arrow.frame) < NSMinY(last.frame));
                    } else {
                        const CGFloat pagerOffset = NSMinY(arrow.frame) - NSMinY(first.frame);
                        if (fabs(pagerOffset - arrow.pagerMetrics.marginBottom) >= 0.01)
                            fprintf(stderr, "canvas pager offset: skin=%s scale=%.1f shown=%d actual=%.2f expected=%.2f\n",
                                identifier, scale, shown, pagerOffset, arrow.pagerMetrics.marginBottom);
                        assert(fabs(pagerOffset - arrow.pagerMetrics.marginBottom) < 0.01);
                    }
                    for (NSView *child in chrome.candidateContentView.subviews)
                        if ([child.identifier isEqual:@"candidate-pager-divider"])
                            assert(fabs(NSMinY(child.frame) - NSMinY(arrow.frame)) < 0.01);
                }
                // 透明空隙不属于候选按钮的命中区域。
                const NSPoint top = NSMakePoint(NSMidX(CandidateFrame(first)), NSMaxY(CandidateFrame(first)) + expectedGap / 2);
                if (!shown && expectedGap > 0) {
                    NSView *hit = [chrome.candidateContentView hitTest:[chrome.candidateContentView convertPoint:top toView:chrome.candidateContentView.superview]];
                    assert(hit != first && ![hit isDescendantOf:first]);
                }
            }
        [controller setValue:nil forKey:@"expandedCandidates"];
        [panel orderOut:nil];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

// 对真实视图矩形检查预编辑外边距，避免只改窗口高度而文字仍挤在原位。
static void TestFactoryPreeditMargins(RowLayoutClient *client)
{
    char temporary[] = "/tmp/msime-preedit-margin-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    struct Gaps { CGFloat top = 0, bottom = 0; } baseline[2][2][2];
    for (const char *identifier : {"plain", "margin", "canvas-margin", "canvas-zero"}) {
        const bool plain = std::string(identifier) == "plain";
        const bool zeroGap = std::string(identifier) == "canvas-zero";
        const bool canvas = zeroGap || std::string(identifier) == "canvas-margin";
        std::filesystem::create_directory(root / identifier);
        {
            std::ofstream manifest(root / identifier / "skin.toml");
            manifest << "schema_version = 2\nid = \"" << identifier << "\"\nname = \"Synthetic Margin\"\nversion = \"1\"\nbase = \""
                     << (canvas ? "canvas" : "microsoft") << "\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n"
                     << "[candidate_window]\npreedit_margin_dip = " << (plain ? "[0]" : "[7, 11, 13, 17]") << "\n";
            if (canvas) manifest << "line_gap_dip = " << (zeroGap ? 0 : 6) << "\n";
            manifest << "[candidate_window.vertical]\npreedit_margin_dip = " << (plain ? "[0]" : "[3, 5, 7, 9]") << "\n";
            assert(manifest.good());
        }
        NSString *suite = [@"app.msime.test.preedit-margin." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(identifier) forKey:@"MSIMEClientCustomCandidateSkin"];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
            skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.showsAppLogo = NO;
        appearance.showsCandidatePageNumber = NO;
        appearance.showsCandidateExpandButton = NO;
        appearance.showsCandidatePreedit = YES;
        appearance.candidateTranslations = NO;
        appearance.fontSize = 16;
        appearance.preeditFontSize = 12;
        appearance.themeMode = @"light";
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];
        NSString *preedit = @"synthetic012345678901234567890";
        NSArray *candidates = @[@{@"text": @"甲", @"highlighted": @YES}, @{@"text": @"乙"}];
        [controller setValue:@{@"focused": @YES, @"editing_text": preedit, @"page": @0, @"page_count": @1,
            @"candidates": candidates} forKey:@"view"];
        for (NSUInteger scaling = 0; scaling < 2; ++scaling) for (NSUInteger vertical = 0; vertical < 2; ++vertical)
            for (NSUInteger expanded = 0; expanded < (vertical ? 1 : 2); ++expanded) {
                appearance.candidateScalePercent = scaling ? 150 : 100;
                appearance.vertical = vertical;
                const CGFloat scale = scaling ? 1.5 : 1.0;
                [controller setValue:expanded ? candidates : nil forKey:@"expandedCandidates"];
                if (expanded) assert([controller renderExpandedCandidates]);
                else [controller renderCandidates];
                MSIMECandidateButton *first = CandidateButton(panel.contentView, 0);
                NSTextField *label = nil;
                for (NSView *child in MSIMECandidateControlsView(panel.contentView).subviews)
                    if ([child.identifier isEqual:@"candidate-preedit"]) label = (id)child;
                assert(first && first.hasSkinLayout && label);
                const CGFloat inset = NSMinX(CandidateFrame(first));
                const CGFloat top = NSHeight(MSIMECandidateControlsView(panel.contentView).bounds) - inset - NSMaxY(label.frame);
                const CGFloat bottom = NSMinY(label.frame) - NSMaxY(CandidateFrame(first));
                if (plain) baseline[scaling][vertical][expanded] = {top, bottom};
                const auto before = baseline[scaling][vertical][expanded];
                const CGFloat marginTop = plain ? 0 : vertical ? 3 : 7;
                const CGFloat marginRight = plain ? 0 : vertical ? 5 : 11;
                const CGFloat marginBottom = canvas ? (zeroGap ? 0 : 6) : plain ? 0 : vertical ? 7 : 13;
                const CGFloat marginLeft = plain ? 0 : vertical ? 9 : 17;
                assert(fabs(NSMinX(label.frame) - inset - marginLeft * scale) < 0.1);
                assert(fabs(NSWidth(MSIMECandidateControlsView(panel.contentView).bounds) - inset - NSMaxX(label.frame) - marginRight * scale) < 0.1);
                // 画布调整窗口后按背板像素对齐，顶部允许一像素的取整差；子视图之间仍精确比较。
                const CGFloat edgeTolerance = 1.0 / panel.backingScaleFactor + 0.01;
                if (fabs(top - before.top - marginTop * scale) > edgeTolerance)
                    fprintf(stderr, "preedit margin: skin=%s scale=%.1f vertical=%lu expanded=%lu top=%.2f baseline=%.2f expected=%.2f\n",
                        identifier, scale, (unsigned long)vertical, (unsigned long)expanded, top, before.top, marginTop * scale);
                assert(fabs(top - before.top - marginTop * scale) <= edgeTolerance);
                assert(fabs(bottom - before.bottom - marginBottom * scale + (canvas ? 2 * scale : 0)) < 0.1);
                if (vertical || expanded)
                    assert(NSWidth(label.frame) + 0.1 >= [preedit sizeWithAttributes:@{NSFontAttributeName:label.font}].width);
                else
                    assert(NSWidth(label.frame) >= 0);
            }
        if (!plain && !canvas) {
            // 首行翻页和展开收起控件在外边距以内居中，不跟着总高度再次偏移。
            appearance.candidateScalePercent = 100;
            appearance.vertical = NO;
            appearance.showsCandidatePageNumber = YES;
            [controller setValue:@{@"focused": @YES, @"editing_text": preedit, @"page": @0, @"page_count": @3,
                @"candidates": candidates} forKey:@"view"];
            for (BOOL expanded : {NO, YES}) {
                [controller setValue:expanded ? candidates : nil forKey:@"expandedCandidates"];
                if (expanded) assert([controller renderExpandedCandidates]);
                else [controller renderCandidates];
                NSView *label = nil;
                for (NSView *child in MSIMECandidateControlsView(panel.contentView).subviews)
                    if ([child.identifier isEqual:@"candidate-preedit"]) label = child;
                MSIMECandidateButton *control = CandidateButton(panel.contentView, expanded ? -3 : -1);
                assert(label && control);
                if (expanded) assert(fabs(NSMidY(label.frame) - NSMidY(control.frame)) < 0.6);
                else assert([control isKindOfClass:MSIMECandidatePagerButton.class] && NSMaxY(control.frame) < NSMinY(label.frame));
            }
            appearance.showsCandidatePageNumber = NO;
        }
        // 隐藏预编辑时不残留其外边距；canvas 的出厂未收紧行距仍保留 2 DIP。
        appearance.showsCandidatePreedit = NO;
        [controller setValue:nil forKey:@"expandedCandidates"];
        [controller renderCandidates];
        MSIMECandidateButton *first = CandidateButton(panel.contentView, 0);
        assert(fabs(NSHeight(MSIMECandidateControlsView(panel.contentView).bounds) - NSMinX(first.frame) - NSMaxY(first.frame)
            - (canvas ? 2.0 * appearance.candidateScalePercent / 100.0 : 0.0)) < 0.1);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

// 真正经过普通候选与展开渲染，验证明暗来自客户端颜色且没有增加跨进程读取。
static void TestHostAppearance(BOOL visual)
{
    NSString *suite = [@"msime.host-appearance." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.globalTheme = @"system";
    appearance.themeMode = @"system";
    appearance.candidateTheme = @"follow";
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    RowLayoutClient *client = [RowLayoutClient new];
    client.caret = NSMakeRect(400, 500, 1, 24);
    MSIMECandidatePanel *panel = visual ? [[MSIMECandidatePanel alloc] initWithContentRect:NSZeroRect styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO] :
        [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    NSDictionary *view = @{@"focused":@YES, @"editing_text":@"d", @"page":@0, @"page_count":@12,
        @"candidates":@[@{@"text":@"在", @"translation":@"in", @"highlighted":@YES}, @{@"text":@"石", @"translation":@"stone"}, @{@"text":@"帮助", @"translation":@"assistance"}]};
    [controller setValue:view forKey:@"view"];
    NSWindow *backdrop = nil;
    if (visual) {
        backdrop = [[NSWindow alloc] initWithContentRect:NSMakeRect(300, 200, 900, 500) styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
        backdrop.title = @"合成输入区明暗验收";
        [backdrop makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
    }
    if (@available(macOS 26.0, *)) {
        for (NSNumber *dark in @[@YES, @NO, @YES]) {
            NSColor *background = dark.boolValue ? [NSColor colorWithSRGBRed:0.1 green:0.1 blue:0.15 alpha:1] : NSColor.whiteColor;
            client.colors = @{NSBackgroundColorAttributeName:background};
            const NSUInteger reads = client.attributeReads;
            [controller renderCandidates];
            assert(client.attributeReads == reads + 1);
            assert([panel.appearance.name isEqual:dark.boolValue ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua]);
            MSIMECandidateChromeView *chrome = (id)panel.contentView;
            assert(chrome.liquidGlassEnabled);
            auto tokens = [appearance candidateWindowSkinForDark:dark.boolValue].tokens;
            if (chrome.liquidGlassActive) msime::mac::ApplyNativeGlassCandidateContrast(tokens, dark.boolValue);
            assert([CandidateButton(chrome, 1).numberColor isEqual:SkinColor(tokens.number)]);
            assert([CandidateButton(chrome, 1).translationColor isEqual:SkinColor(tokens.number)]);
            if (chrome.liquidGlassActive) {
                MSIMECandidateButton *selected = CandidateButton(chrome, 0);
                assert(CandidateContrast(selected.titleColor, selected.fillColor) >= 4.5);
            }
            if (visual) {
                backdrop.backgroundColor = background;
                [panel setFrameOrigin:NSMakePoint(400, 400)];
                printf("HOST visual window=%ld background=%s\n", (long)panel.windowNumber, dark.boolValue ? "dark" : "light");
                fflush(stdout);
                Spin(3);
            }
        }
        client.colors = @{NSForegroundColorAttributeName:NSColor.whiteColor};
        [controller renderCandidates];
        assert([panel.appearance.name isEqual:NSAppearanceNameDarkAqua]);
        client.colors = @{};
        [controller renderCandidates];
        assert(panel.appearance == nil);
        client.colors = @{NSBackgroundColorAttributeName:NSColor.blackColor};
        appearance.candidateTheme = @"light";
        [controller renderCandidates];
        assert([panel.appearance.name isEqual:NSAppearanceNameAqua]);
        appearance.candidateTheme = @"follow";
        appearance.themeMode = @"light";
        [controller renderCandidates];
        assert([panel.appearance.name isEqual:NSAppearanceNameAqua]);
        appearance.themeMode = @"system";
        NSMutableArray *expanded = [NSMutableArray array];
        for (NSUInteger at = 0; at < 9; ++at) [expanded addObject:@{@"text":@"合成", @"highlighted":@(at == 0)}];
        [controller setValue:expanded forKey:@"expandedCandidates"];
        [controller setValue:@0 forKey:@"expandedCursor"];
        assert([controller renderExpandedCandidates]);
        assert([panel.appearance.name isEqual:NSAppearanceNameDarkAqua]);
        client.colors = @{NSBackgroundColorAttributeName:NSColor.whiteColor};
        assert([controller renderExpandedCandidates]);
        assert([panel.appearance.name isEqual:NSAppearanceNameAqua]);
        // 保留玻璃而不是把自适应明暗写回窗口；连续按键与普通／展开切换只替换候选控件。
        [controller setValue:nil forKey:@"expandedCandidates"];
        [controller renderCandidates];
        MSIMECandidateChromeView *stableChrome = (id)panel.contentView;
        NSView *stableContent = stableChrome.candidateContentView;
        NSView *glass = [stableChrome.subviews filteredArrayUsingPredicate:
            [NSPredicate predicateWithBlock:^BOOL(NSView *view, NSDictionary *_) {
                return [view.identifier isEqual:@"candidate-liquid-glass"];
            }]].firstObject;
        if (stableChrome.liquidGlassActive) {
            assert(glass != nil);
            for (NSNumber *expandedMode in @[@NO, @YES, @NO]) {
                [controller setValue:expandedMode.boolValue ? expanded : nil forKey:@"expandedCandidates"];
                if (expandedMode.boolValue) assert([controller renderExpandedCandidates]);
                else [controller renderCandidates];
                assert(panel.contentView == stableChrome);
                assert(stableContent.clipsToBounds == expandedMode.boolValue);
                for (NSString *name in @[NSAppearanceNameDarkAqua, NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) {
                    const NSUInteger reads = client.attributeReads;
                    stableContent.appearance = [NSAppearance appearanceNamed:name];
                    [stableContent viewDidChangeEffectiveAppearance];
                    assert(client.attributeReads == reads);
                    const BOOL dark = [name isEqual:NSAppearanceNameDarkAqua];
                    auto tokens = [appearance candidateWindowSkinForDark:dark].tokens;
                    msime::mac::ApplyNativeGlassCandidateContrast(tokens, dark);
                    MSIMECandidateButton *row = CandidateButton(stableChrome, 1);
                    assert([row.titleColor isEqual:SkinColor(tokens.text)]);
                    assert([row.numberColor isEqual:SkinColor(tokens.number)]);
                    assert([row.translationColor isEqual:SkinColor(tokens.number)]);
                    MSIMECandidateButton *selected = CandidateButton(stableChrome, 0);
                    assert([selected.titleColor isEqual:SkinColor(tokens.selectedText)]);
                    assert(CandidateContrast(selected.titleColor, selected.fillColor) >= 4.5);
                    for (NSUInteger key = 0; key < 4; ++key) {
                        MSIMECandidateButton *previous = CandidateButton(stableChrome, 1);
                        [controller setValue:nil forKey:@"renderedCandidateView"];
                        if (key == 0) [controller hideCandidatePanel:"empty"];
                        if (expandedMode.boolValue) assert([controller renderExpandedCandidates]);
                        else [controller renderCandidates];
                        assert(client.attributeReads == reads + key + 1);
                        assert([panel.appearance.name isEqual:NSAppearanceNameAqua]);
                        assert(panel.contentView == stableChrome && stableChrome.candidateContentView == stableContent);
                        assert(glass.superview == stableChrome);
                        assert([stableContent.effectiveAppearance.name isEqual:name]);
                        assert(CandidateButton(stableChrome, 1) != previous && ![previous isDescendantOf:stableContent]);
                        assert([CandidateButton(stableChrome, 1).titleColor isEqual:SkinColor(tokens.text)]);
                    }
                }
            }
            stableContent.appearance = nil;
        }
        appearance.candidateTheme = @"light";
        [controller renderCandidates];
        assert([panel.appearance.name isEqual:NSAppearanceNameAqua]);
        assert(panel.contentView != stableChrome);
        appearance.candidateTheme = @"follow";
        [controller renderCandidates];
        NSView *previousClientChrome = panel.contentView;
        RowLayoutClient *otherClient = [RowLayoutClient new];
        otherClient.caret = client.caret;
        [controller setValue:otherClient forKey:@"activeClient"];
        [controller renderCandidates];
        assert(panel.appearance == nil);
        assert(panel.contentView != previousClientChrome);
        for (NSDictionary *invalid in @[@{NSBackgroundColorAttributeName:@"not a color"}, @{NSBackgroundColorAttributeName:NSColor.clearColor}, @{NSForegroundColorAttributeName:NSColor.redColor}])
            assert(MSIMECandidateHostAppearance(invalid) == nil);
        assert([MSIMECandidateHostAppearance(@{NSBackgroundColorAttributeName:NSColor.whiteColor, NSForegroundColorAttributeName:NSColor.whiteColor}).name isEqual:NSAppearanceNameAqua]);
    }
    [panel orderOut:nil];
    [backdrop orderOut:nil];
    [controller setValue:nil forKey:@"activeClient"];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 使用生产控制器渲染 canvas，点击与候选身份仍通过原来的内容容器。
static void TestCanvasHost(const std::filesystem::path &root, NSString *identifier, NSString *output)
{
    NSString *suite = [@"app.msime.test.canvas-host." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:identifier forKey:@"MSIMEClientCustomCandidateSkin"];
    [defaults setObject:identifier forKey:@"MSIMEClientCustomCandidateSkinDark"];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
        skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    appearance.showsAppLogo = NO;
    appearance.showsCandidatePreedit = YES;
    appearance.showsCandidatePageNumber = NO;
    appearance.candidateTranslations = YES;
    appearance.candidateTheme = @"follow";
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    RowLayoutClient *client = [RowLayoutClient new];
    const NSRect screen = NSScreen.mainScreen.visibleFrame;
    client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
    RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    NSMutableArray *rows = [NSMutableArray array];
    for (NSUInteger at = 0; at < 3; ++at) [rows addObject:@{
        @"id":@{@"session":@1, @"generation":@7, @"index":@(at)}, @"text":@[@"合成", @"测试", @"候选"][at],
        @"translation":@[@"synthetic", @"test", @"candidate"][at], @"highlighted":@(at == 0)}];
    NSDictionary *view = @{@"session":@1, @"generation":@7, @"focused":@YES,
        @"editing_text":@"ceshi", @"page":@0, @"page_count":@1, @"candidates":rows};
    NSEvent *rightClick = [NSEvent mouseEventWithType:NSEventTypeRightMouseDown location:NSZeroPoint modifierFlags:0
        timestamp:0 windowNumber:0 context:nil eventNumber:1 clickCount:1 pressure:1];
    for (NSNumber *percent in @[@100, @150]) for (NSNumber *vertical in @[@NO, @YES]) for (NSNumber *dark in @[@NO, @YES]) {
        appearance.candidateScalePercent = percent.integerValue;
        appearance.candidateOpacityPercent = percent.integerValue == 150 ? 75 : 100;
        appearance.vertical = vertical.boolValue;
        appearance.themeMode = dark.boolValue ? @"dark" : @"light";
        [controller setValue:nil forKey:@"expandedCandidates"];
        [controller setValue:view forKey:@"view"];
        const auto skin = [appearance candidateWindowSkinForDark:dark.boolValue];
        assert(skin.canvas && skin.canvasPanelDrawn);
        assert([appearance canvasAssetsForDark:dark.boolValue] == [appearance canvasAssetsForDark:dark.boolValue]);
        const auto padding = msime::mac::CanvasContentInsets(*skin.canvas, msime::mac::CandidateInset(skin.tokens));
        for (NSNumber *expanded in @[@NO, @YES]) {
            if (expanded.boolValue) {
                [controller setValue:rows forKey:@"expandedCandidates"];
                [controller setValue:@0 forKey:@"expandedCursor"];
                assert([controller renderExpandedCandidates]);
            } else [controller renderCandidates];
            MSIMECandidateChromeView *chrome = (id)panel.contentView;
            assert(chrome.canvasPanelLoaded && !chrome.liquidGlassActive && !panel.hasShadow);
            const NSRect card = chrome.cardBounds;
            // NSWindow 按物理像素对齐，150% 的外留白可能落在半像素上。
            const CGFloat positionTolerance = 1.0 / panel.backingScaleFactor + 0.01;
            if (std::abs(NSMinX(panel.frame) + NSMinX(card) - NSMinX(client.caret)) > positionTolerance)
                fprintf(stderr, "canvas x window=%.3f card=%.3f caret=%.3f backing=%.3f percent=%ld expanded=%d\n",
                    NSMinX(panel.frame), NSMinX(card), NSMinX(client.caret), panel.backingScaleFactor,
                    (long)percent.integerValue, expanded.boolValue);
            assert(std::abs(NSMinX(panel.frame) + NSMinX(card) - NSMinX(client.caret)) <= positionTolerance);
            assert(std::abs(NSMinX(card) - skin.canvas->outer.left) < 0.01);
            assert(std::abs(NSMinY(card) - skin.canvas->outer.bottom) < 0.01);
            assert(std::abs(NSWidth(chrome.bounds) - NSWidth(card) - skin.canvas->outer.left - skin.canvas->outer.right) < 0.01);
            assert(std::abs(NSHeight(chrome.bounds) - NSHeight(card) - skin.canvas->outer.top - skin.canvas->outer.bottom) < 0.01);
            assert(NSEqualRects(chrome.candidateContentView.frame, card));
            MSIMECandidateButton *first = CandidateButton(chrome, 0);
            assert([first isDescendantOf:chrome.candidateContentView]);
            if (expanded.boolValue) {
                assert([first.enclosingScrollView isKindOfClass:MSIMEExpandedCandidateScrollView.class] &&
                    first.enclosingScrollView.contentView.clipsToBounds);
                NSScrollView *viewport = first.enclosingScrollView;
                assert(fabs(NSMinX(viewport.frame) - padding.left + msime::mac::CandidateInset(skin.tokens)) < 0.01);
                assert(fabs(NSMinX(first.frame) - msime::mac::CandidateInset(skin.tokens)) < 0.01);
                assert(NSMaxX(viewport.frame) <= NSWidth(card) + positionTolerance);
                if (!skin.expandPanel.value_or(false)) {
                    if (fabs(NSMaxX(viewport.frame) - NSWidth(card)) > positionTolerance)
                        fprintf(stderr, "canvas viewport=%s card=%s padding=%.2f,%.2f inset=%.2f scale=%ld skin=%s\n",
                            NSStringFromRect(viewport.frame).UTF8String, NSStringFromRect(card).UTF8String,
                            padding.left, padding.right, msime::mac::CandidateInset(skin.tokens),
                            (long)percent.integerValue, identifier.UTF8String);
                    assert(fabs(NSMaxX(viewport.frame) - NSWidth(card)) <= positionTolerance);
                }
                assert(fabs(NSWidth(viewport.documentView.frame) - NSWidth(viewport.contentView.bounds)) < 0.01);
                assert(!viewport.hasHorizontalScroller);
                viewport.autohidesScrollers = NO;
                [viewport tile];
                const NSRect scroller = [viewport.verticalScroller convertRect:viewport.verticalScroller.bounds
                    toView:chrome.candidateContentView];
                assert(!NSIsEmptyRect(scroller) && NSMaxX(scroller) <= NSWidth(card) + positionTolerance);
                for (NSInteger tag = 0; tag < 3; ++tag)
                    assert(NSMaxX(CandidateFrame(CandidateButton(chrome, tag))) <=
                        NSMinX(scroller) - 4 * percent.doubleValue / 100 + 0.01);
            }
            const CGFloat itemRadius = [identifier isEqual:@"canvas-host"] ? 3.5 : [identifier isEqual:@"green_leaf"] ? 5 : 6;
            assert(std::abs(first.cornerRadius - itemRadius * percent.doubleValue / 100) < 0.01);
            // 原生裁剪视口会按 backing 像素定位；逻辑单元格仍在上面严格检查。
            const CGFloat rowPositionTolerance = expanded.boolValue ? positionTolerance : 0.01;
            if (std::abs(NSMinX(CandidateFrame(first)) - padding.left) > rowPositionTolerance)
                fprintf(stderr, "canvas candidate x: percent=%ld vertical=%d dark=%d expanded=%d backing=%.1f frame=%s expected=%.3f viewport=%s clip=%s document=%s\n",
                    (long)percent.integerValue, vertical.boolValue, dark.boolValue, expanded.boolValue, panel.backingScaleFactor,
                    NSStringFromRect(CandidateFrame(first)).UTF8String, padding.left,
                    NSStringFromRect(first.enclosingScrollView.frame).UTF8String,
                    NSStringFromRect(first.enclosingScrollView.contentView.bounds).UTF8String,
                    NSStringFromRect(first.enclosingScrollView.documentView.frame).UTF8String);
            assert(std::abs(NSMinX(CandidateFrame(first)) - padding.left) <= rowPositionTolerance);
            CGFloat bottom = CGFLOAT_MAX;
            for (NSInteger tag = 0; tag < 3; ++tag) {
                MSIMECandidateButton *button = CandidateButton(chrome, tag);
                assert(button && NSMaxX(CandidateFrame(button)) <= NSWidth(card) - padding.right + positionTolerance);
                bottom = MIN(bottom, NSMinY(CandidateFrame(button)));
            }
            if (std::abs(bottom - padding.bottom) > rowPositionTolerance)
                fprintf(stderr, "canvas candidate bottom: percent=%ld vertical=%d dark=%d expanded=%d backing=%.1f actual=%.3f expected=%.3f\n",
                    (long)percent.integerValue, vertical.boolValue, dark.boolValue, expanded.boolValue,
                    panel.backingScaleFactor, bottom, padding.bottom);
            assert(std::abs(bottom - padding.bottom) <= rowPositionTolerance);
            const NSPoint hit = [first convertPoint:NSMakePoint(NSMidX(first.bounds), NSMidY(first.bounds)) toView:chrome];
            assert([chrome hitTest:hit] == first);
            assert([first.titleColor isEqual:SkinColor(skin.tokens.selectedText)]);
            assert([CandidateButton(chrome, 1).titleColor isEqual:SkinColor(skin.tokens.text)]);
            assert([first.barColor isEqual:SkinColor(skin.tokens.selectedBar.value_or(skin.tokens.accent))]);
            if (expanded.boolValue) assert(first.action == @selector(selectExpandedCandidate:));
            else assert([first menuForEvent:rightClick] != nil);
            MSIMECandidatePreeditField *preedit = nil;
            NSMutableArray *levels = [NSMutableArray array];
            for (NSView *child in chrome.candidateContentView.subviews)
                if ([child.identifier isEqual:@"candidate-preedit"]) preedit = (id)child;
            assert(preedit && [preedit.textColor isEqual:SkinColor(skin.tokens.preeditText.value_or(skin.tokens.accent))]);
            assert([preedit.caretColor isEqual:SkinColor(skin.tokens.preeditCaret.value_or(skin.tokens.accent))]);
            // 预编辑在整条首行内居中；字号小于展开按钮时，下缘还包含居中偏移。
            const CGFloat scale = MSIMECandidateScale(appearance);
            const BOOL headerHasControls = expanded.boolValue || (appearance.showsCandidateExpandButton && [controller expandedCandidatesOffered]);
            const CGFloat headerHeight = MAX(NSHeight(preedit.frame), headerHasControls ? MSIMECandidateHeaderHeight * scale : 0);
            const CGFloat centeredOffset = floor((headerHeight - NSHeight(preedit.frame)) / 2);
            const auto preeditMargin = MSIMECandidatePreeditMargins(skin, YES, scale);
            if (expanded.boolValue)
                assert(std::abs(NSMinY(preedit.frame) - NSMaxY(first.enclosingScrollView.frame) -
                    skin.canvas->lineGap.value_or(0) - preeditMargin.bottom - centeredOffset) < 0.01);
            assert(std::abs(NSMinY(preedit.frame) - NSMaxY(CandidateFrame(first)) -
                skin.canvas->lineGap.value_or(0) - preeditMargin.bottom - centeredOffset) <= rowPositionTolerance);
            for (NSView *child in chrome.subviews) {
                if ([child isKindOfClass:MSIMECandidateCanvasView.class]) {
                    [levels addObject:child.identifier];
                    assert([child hitTest:hit] == nil);
                    for (CALayer *sprite in child.layer.sublayers)
                        assert(std::abs(sprite.opacity - ([child.identifier isEqual:@"candidate-canvas-0"] ? skin.canvas->panelOpacity : 1.0)) < 0.01);
                } else if ([child isKindOfClass:MSIMECandidateCardBackgroundView.class]) [levels addObject:@"background"];
                else if (child == chrome.candidateContentView) [levels addObject:@"controls"];
            }
            assert(([levels isEqual:@[@"candidate-canvas--1", @"background", @"candidate-canvas-0", @"controls", @"candidate-canvas-1"]]));
            // 系统切换明暗而包 ID 不变时，候选按钮保持身份，贴图和文字一起换槽位。
            NSAppearance *originalAppearance = chrome.appearance;
            chrome.appearance = [NSAppearance appearanceNamed:dark.boolValue ? NSAppearanceNameAqua : NSAppearanceNameDarkAqua];
            [controller refreshCandidateSkin];
            assert(panel.contentView == chrome && CandidateButton(chrome, 0) == first);
            assert(chrome.canvasAssets == [appearance canvasAssetsForDark:!dark.boolValue]);
            const auto otherSkin = [appearance candidateWindowSkinForDark:!dark.boolValue];
            assert([first.titleColor isEqual:SkinColor(otherSkin.tokens.selectedText)]);
            if ([identifier isEqual:@"canvas-host"]) {
                for (NSView *child in chrome.subviews) if ([child.identifier isEqual:@"candidate-canvas-0"]) {
                    // 零边框只生成中间贴图，不能假设始终有九个切片。
                    size_t widest = 0;
                    for (CALayer *sprite in child.layer.sublayers)
                        widest = MAX(widest, CGImageGetWidth((__bridge CGImageRef)sprite.contents));
                    assert(widest == (dark.boolValue ? 10 : 8));
                }
            }
            chrome.appearance = originalAppearance;
            [controller refreshCandidateSkin];
            assert(chrome.canvasAssets == [appearance canvasAssetsForDark:dark.boolValue]);
            NSUInteger canvasViews = 0;
            for (NSView *child in chrome.subviews) if ([child isKindOfClass:MSIMECandidateCanvasView.class]) ++canvasViews;
            assert(canvasViews == 3);
            if (output && percent.integerValue == 100) {
                chrome.wantsLayer = YES;
                [chrome displayIfNeeded];
                NSBitmapImageRep *bitmap = [chrome bitmapImageRepForCachingDisplayInRect:chrome.bounds];
                [chrome cacheDisplayInRect:chrome.bounds toBitmapImageRep:bitmap];
                NSString *file = [NSString stringWithFormat:@"%@-%@-%@-%@.png", identifier,
                    dark.boolValue ? @"dark" : @"light", vertical.boolValue ? @"vertical" : @"horizontal", expanded.boolValue ? @"expanded" : @"page"];
                assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
                    writeToFile:[output stringByAppendingPathComponent:file] atomically:YES]);
            }
        }
        [controller setValue:nil forKey:@"expandedCandidates"];
        [controller renderCandidates];
        MSIMECandidateButton *stale = CandidateButton(panel.contentView, 0);
        [controller renderCandidates];
        assert([stale menuForEvent:rightClick] == nil);
    }
    // 没有预编辑和翻页首行时，line_gap 不得在卡片顶部留下空行。
    appearance.showsCandidatePreedit = NO;
    client.caret = NSMakeRect(NSMidX(screen), NSMinY(screen) + 1, 1, 20);
    [controller setValue:nil forKey:@"expandedCandidates"];
    for (NSNumber *expanded in @[@NO, @YES]) {
        if (expanded.boolValue) {
            [controller setValue:rows forKey:@"expandedCandidates"];
            assert([controller renderExpandedCandidates]);
        } else [controller renderCandidates];
        MSIMECandidateChromeView *chrome = (id)panel.contentView;
        const auto skin = [appearance candidateWindowSkinForDark:YES];
        // 底部挂件留白较大时，整窗夹紧会把卡片向上推，不能用未夹紧的光标位置作期望。
        const CGFloat cardBottom = MAX(NSMaxY(client.caret) + 4, NSMinY(screen) + NSMinY(chrome.cardBounds));
        assert(std::abs(NSMinY(panel.frame) + NSMinY(chrome.cardBounds) - cardBottom) <= 1.0 / panel.backingScaleFactor + 0.01);
        const CGFloat topInset = skin.canvas->content->top + 2.0 * appearance.candidateScalePercent / 100.0;
        const CGFloat actualTop = NSMaxY(CandidateFrame(CandidateButton(chrome, 0)));
        const CGFloat expectedTop = NSHeight(chrome.cardBounds) - topInset;
        const CGFloat topTolerance = 1.0 / panel.backingScaleFactor + 0.01;
        if (std::abs(actualTop - expectedTop) > topTolerance)
            fprintf(stderr, "canvas headerless top: expanded=%d percent=%ld backing=%.1f actual=%.3f expected=%.3f card=%s\n",
                expanded.boolValue, (long)appearance.candidateScalePercent, panel.backingScaleFactor, actualTop,
                expectedTop, NSStringFromRect(chrome.cardBounds).UTF8String);
        assert(std::abs(actualTop - expectedTop) <= topTolerance);
    }
    [controller hideCandidatePanel:"empty"];
    [controller setValue:nil forKey:@"activeClient"];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestCanvasHostFixture()
{
    const auto root = std::filesystem::temp_directory_path() / ("msime-canvas-host-" + std::string(NSUUID.UUID.UUIDString.UTF8String));
    std::filesystem::create_directories(root / "canvas-host");
    WriteApng(root / "canvas-host" / "panel.png", 12, 12, 1, 0.1);
    WriteApng(root / "canvas-host" / "panel-light.png", 14, 12, 1, 0.1);
    for (int right : {0, 2, 21}) for (int left : {0, 23}) for (bool expand : {false, true}) {
        std::ofstream manifest(root / "canvas-host" / "skin.toml");
        manifest << R"toml(schema_version = 2
id = "canvas-host"
name = "合成 canvas 控制器夹具"
version = "1"
base = "canvas"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["light", "dark"]
[candidate_window]
min_width_dip = 240
line_gap_dip = 9
item_corner_radius_dip = 3.5
)toml" << "expand_panel = " << (expand ? "true" : "false") << R"toml(
[candidate_window.border_image]
source = "panel.png"
light_source = "panel-light.png"
slice_dip = [2]
width_dip = [0]
[candidate_window.content_insets_dip]
top = 17
right = )toml" << right << R"toml(
bottom = 19
left = )toml" << left << R"toml(
[candidate_window.outer_insets_dip]
top = 11
right = 13
bottom = 7
left = 5
[[candidate_window.decorations]]
image = "panel.png"
width_dip = 12
height_dip = 18
anchor = "top-left"
z_index = 0
[[candidate_window.decorations]]
image = "panel.png"
width_dip = 16
height_dip = 12
anchor = "bottom-right"
z_index = 2
[candidate.light]
text = "#123456"
preedit_text = "#aabbcc"
preedit_caret = "#778899"
selected = "#112233"
selected_text = "#ffffff"
selected_bar = "#eecc77"
[candidate.dark]
text = "#abcdef"
preedit_text = "#ccbbaa"
preedit_caret = "#998877"
selected = "#112233"
selected_text = "#ffffff"
selected_bar = "#77ccee"
)toml";
        assert(manifest.good());
        manifest.close();
        TestCanvasHost(root, @"canvas-host", nil);
    }
    std::filesystem::remove_all(root);
}

static void TestUserPaddingSelectedBar()
{
    char temporary[] = "/tmp/msime-padding-selected-bar-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    std::filesystem::create_directory(root / "bar");
    {
        std::ofstream manifest(root / "bar" / "skin.toml");
        manifest << "schema_version = 2\nid = \"bar\"\nname = \"Synthetic Selected Bar\"\nversion = \"1\"\nbase = \"system\"\n"
                 << "[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n"
                 << "[candidate_window]\nmin_width_dip = 0\n"
                 << "[candidate.light]\nshow_selected_bar = true\n[candidate.dark]\nshow_selected_bar = true\n";
        assert(manifest.good());
    }
    std::string loadError;
    const auto package = msime::mac::LoadSkinPackage(root, "bar", &loadError);
    if (!package) NSLog(@"Synthetic selected bar skin failed to load: %s", loadError.c_str());
    assert(package && package->light.showSelectedBar == true && package->dark.showSelectedBar == true);
    assert(!package->resolvedLayout);
    NSString *suite = [@"app.msime.test.padding-selected-bar." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:@"bar" forKey:@"MSIMEClientCustomCandidateSkin"];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
        skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    appearance.themeMode = @"light";
    appearance.showsCandidatePreedit = NO;
    appearance.showsAppLogo = NO;
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    RowLayoutClient *client = [RowLayoutClient new];
    const NSRect screen = NSScreen.mainScreen.visibleFrame;
    client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
    RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    [controller setValue:@{@"focused": @YES, @"editing_text": @"synthetic", @"page": @0, @"page_count": @1,
        @"candidates": @[@{@"text": @"合成", @"highlighted": @YES}]} forKey:@"view"];
    for (NSNumber *percent in @[@50, @100, @150, @200]) for (NSNumber *vertical in @[@NO, @YES]) {
        [appearance applySharedCandidatePreferences:@{
            @"global_theme": @"custom",
            @"custom_theme": @{@"base": @"system", @"candidate_skin": @"bar"},
            @"candidate_scale_percent": percent, @"candidate_item_padding_horizontal": @8}];
        const auto skin = [appearance candidateWindowSkinForDark:NO];
        assert(skin.candidateSkin == "bar" && skin.tokens.showSelectedBar && !skin.layout);
        appearance.vertical = vertical.boolValue;
        [controller renderCandidates];
        MSIMECandidateButton *button = CandidateButton(panel.contentView, 0);
        const CGFloat scale = percent.doubleValue / 100;
        assert(button && button.showSelectedBar && button.hasSkinLayout);
        assert(button.selectedBarInside);
        assert(fabs(button.textLeftInset - 8 * scale) < 0.01);
        assert(fabs(button.textRightInset - 8 * scale) < 0.01);
        const CGFloat barLeft = MAX(1.0, button.selectionLeftInset);
        assert(barLeft >= 0 && barLeft + button.selectedBarWidth <= NSWidth(button.bounds));
    }
    [panel orderOut:nil];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    std::filesystem::remove_all(root);
}

static void TestUserGeometry()
{
    NSString *suite = [@"app.msime.test.item-geometry." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    RowLayoutClient *client = [RowLayoutClient new];
    const NSRect screen = NSScreen.mainScreen.visibleFrame;
    client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
    RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
        backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    NSDictionary *view = @{@"focused": @YES, @"editing_text": @"ceshi", @"page": @0, @"page_count": @1,
        @"candidates": @[@{@"text": @"合成候选甲", @"highlighted": @YES},
                         @{@"text": @"合成候选乙"}, @{@"text": @"合成候选丙"}]};
    [controller setValue:view forKey:@"view"];
    for (NSNumber *percent in @[@50, @100, @150, @200]) {
        const CGFloat scale = percent.doubleValue / 100;
        for (NSNumber *vertical in @[@NO, @YES]) {
            NSMutableDictionary *document = [@{
                @"candidate_scale_percent": percent,
                @"candidate_padding_horizontal": @3, @"candidate_padding_vertical": @9,
                @"candidate_item_padding_horizontal": @4, @"candidate_item_padding_vertical": @2,
                @"candidate_item_corner_radius": @0, @"candidate_corner_radius": @18,
                @"candidate_item_spacing": @5,
            } mutableCopy];
            [appearance applySharedCandidatePreferences:document];
            appearance.vertical = vertical.boolValue;
            appearance.showsCandidatePreedit = NO;
            appearance.showsAppLogo = NO;
            [controller renderCandidates];
            MSIMECandidateChromeView *chrome = (id)panel.contentView;
            MSIMECandidateButton *first = CandidateButton(chrome, 0);
            MSIMECandidateButton *second = CandidateButton(chrome, 1);
            assert(first && second);
            assert(fabs(NSMinX(first.frame) - 3 * scale) < 0.01);
            assert(fabs(NSMinY(chrome.candidateRowsRect) - 9 * scale) < 0.01);
            assert(fabs(first.textLeftInset - 4 * scale) < 0.01);
            assert(fabs(first.textRightInset - 4 * scale) < 0.01);
            assert(fabs(first.itemLayout.contentTop - 2 * scale) < 0.01);
            assert(first.cornerRadius == 0 && !first.hasCornerRadii);
            assert(chrome.cornerRadius == 18 * scale && chrome.clipsCandidateContent);
            [chrome layoutSubtreeIfNeeded];
            assert(chrome.candidateContentView.layer.mask);
            const CGFloat gap = vertical.boolValue ? NSMinY(first.frame) - NSMaxY(second.frame)
                                                  : NSMinX(second.frame) - NSMaxX(first.frame);
            assert(fabs(gap - 5 * scale) < 1.01);
            const NSSize original = chrome.frame.size;
            document[@"candidate_padding_vertical"] = @13;
            [appearance applySharedCandidatePreferences:document];
            [controller renderCandidates];
            assert(fabs(NSHeight(panel.contentView.frame) - original.height - 8 * scale) < 0.01);
            assert(fabs(NSWidth(panel.contentView.frame) - original.width) < 0.01);
            document[@"candidate_padding_vertical"] = @9;
            document[@"candidate_item_padding_vertical"] = @7;
            [appearance applySharedCandidatePreferences:document];
            [controller renderCandidates];
            MSIMECandidateButton *padded = CandidateButton(panel.contentView, 0);
            assert(fabs(padded.itemLayout.contentTop - 7 * scale) < 0.01);
            assert(fabs(padded.itemLayout.height - first.itemLayout.height - 10 * scale) < 0.01);
            document[@"candidate_padding_horizontal"] = @0;
            document[@"candidate_padding_vertical"] = @0;
            [appearance applySharedCandidatePreferences:document];
            [controller renderCandidates];
            assert(fabs(NSMinX(CandidateButton(panel.contentView, 0).frame)) < 0.01);
            assert(fabs(NSMinY(((MSIMECandidateChromeView *)panel.contentView).candidateRowsRect)) < 0.01);
            const CGFloat flushHeight = NSHeight(panel.contentView.frame);
            [document removeObjectForKey:@"candidate_item_corner_radius"];
            [appearance applySharedCandidatePreferences:document];
            [controller renderCandidates];
            chrome = (id)panel.contentView;
            [chrome layoutSubtreeIfNeeded];
            assert(![appearance candidateWindowSkinForDark:NO].tokens.explicitItemRadius);
            assert(chrome.clipsCandidateContent && chrome.candidateContentView.layer.mask);
            CAShapeLayer *mask = (id)chrome.candidateContentView.layer.mask;
            const CGFloat halfBorder = MAX(0.0, chrome.lineWidth) / 2;
            assert(CGRectEqualToRect(CGPathGetBoundingBox(mask.path),
                NSInsetRect(chrome.candidateContentView.bounds, halfBorder, halfBorder)));
            document[@"candidate_padding_horizontal"] = @1;
            document[@"candidate_padding_vertical"] = @1;
            [appearance applySharedCandidatePreferences:document];
            [controller renderCandidates];
            assert(fabs(NSMinX(CandidateButton(panel.contentView, 0).frame) - scale) < 0.01);
            assert(fabs(NSHeight(panel.contentView.frame) - flushHeight - 2 * scale) < 0.01);
            assert(!((MSIMECandidateChromeView *)panel.contentView).clipsCandidateContent);
        }
    }
    {
        [appearance applySharedCandidatePreferences:@{
            @"candidate_padding_horizontal": @2, @"candidate_padding_vertical": @8,
            @"candidate_item_padding_vertical": @3, @"candidate_item_spacing": @7,
            @"candidate_item_corner_radius": @2,
        }];
        appearance.vertical = NO;
        appearance.pageSize = 3;
        NSMutableArray *expanded = [NSMutableArray array];
        for (NSUInteger index = 0; index < 18; ++index)
            [expanded addObject:@{@"text": @"合成候选", @"highlighted": @(index == 0)}];
        [controller setValue:expanded forKey:@"expandedCandidates"];
        [controller setValue:@0 forKey:@"expandedCursor"];
        [controller setValue:@0 forKey:@"expandedFirstRow"];
        assert([controller renderExpandedCandidates]);
        MSIMECandidateChromeView *chrome = (id)panel.contentView;
        MSIMECandidateButton *first = CandidateButton(chrome, 0);
        MSIMECandidateButton *nextRow = CandidateButton(chrome, 3);
        assert(first && nextRow);
        assert(fabs(NSMinX(first.frame) - 2) < 0.01);
        assert(fabs(NSMinY(chrome.candidateRowsRect) - 8) < 0.01);
        assert(fabs(NSMinY(first.frame) - NSMaxY(nextRow.frame) - 7) < 0.01);
        assert(first.cornerRadius == 2);
        [controller setValue:nil forKey:@"expandedCandidates"];
    }
    // 恢复后不保留旧覆盖；共享文档中的统一留白仍可作为逐轴回退。
    [appearance applySharedCandidatePreferences:@{@"candidate_padding": @7, @"candidate_padding_vertical": @2}];
    auto tokens = [appearance candidateWindowSkinForDark:NO].tokens;
    assert(msime::mac::CandidateInset(tokens) == 7 && msime::mac::CandidateVerticalInset(tokens) == 2);
    [appearance applySharedCandidatePreferences:@{}];
    const auto style = [appearance candidateWindowStyle];
    assert(!style.paddingHorizontal && !style.paddingVertical && !style.itemCornerRadius && !style.itemSpacing);
    [defaults removePersistentDomainForName:suite];
}

// 经真实清单、宿主布局和背景绘制检查阴影，不能用属性值代替卡片外的像素。
static void TestFactoryShadows(RowLayoutClient *client)
{
    const auto root = std::filesystem::temp_directory_path() / ("msime-shadow-host-" + std::string(NSUUID.UUID.UUIDString.UTF8String));
    double strengths[2][4]{};
    for (const char *kind : {"none", "soft", "normal", "strong", "panel", "broken"}) {
        const std::string identifier = std::string("shadow-") + kind;
        const bool canvas = identifier == "shadow-panel" || identifier == "shadow-broken";
        std::filesystem::create_directories(root / identifier);
        {
            std::ofstream manifest(root / identifier / "skin.toml");
            manifest << "schema_version = 2\nid = \"" << identifier << "\"\nname = \"合成阴影\"\nversion = \"1\"\nbase = \""
                << (canvas ? "canvas" : "microsoft") << "\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n[candidate_window]\n";
            if (std::string(kind) != "normal") manifest << "shadow = \"" << (canvas ? "strong" : kind) << "\"\n";
            if (canvas) {
                manifest << "[candidate_window.border_image]\n";
                if (std::string(kind) == "panel") manifest << "source = \"broken.png\"\nlight_source = \"panel.png\"\n";
                else manifest << "source = \"panel.png\"\n";
                manifest << "slice_dip = [2]\nwidth_dip = [3]\n";
            }
            manifest << "[candidate.light]\nsurface = \"#FFFFFF80\"\n[candidate.dark]\nsurface = \"#FFFFFF80\"\n";
        }
        if (canvas) {
            if (std::string(kind) == "panel") {
                WriteApng(root / identifier / "panel.png", 12, 12, 1, 0.1);
                std::ofstream broken(root / identifier / "broken.png"); broken << "synthetic-broken-png";
            }
            else { std::ofstream image(root / identifier / "panel.png"); image << "synthetic-broken-png"; }
        }
        NSString *suite = [@"app.msime.test.shadow." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(identifier.c_str()) forKey:@"MSIMEClientCustomCandidateSkin"];
        [defaults setObject:@(identifier.c_str()) forKey:@"MSIMEClientCustomCandidateSkinDark"];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
            skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.showsAppLogo = NO;
        appearance.showsCandidatePreedit = NO;
        appearance.showsCandidatePageNumber = NO;
        appearance.showsCandidateExpandButton = NO;
        appearance.candidateTranslations = NO;
        appearance.candidateTheme = @"follow";
        MSIMEInputController *controller = [MSIMEInputController new];
        RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];
        NSArray *candidates = @[@{@"text": @"合成", @"highlighted": @YES}, @{@"text": @"验证"}];
        [controller setValue:@{@"focused": @YES, @"editing_text": @"ce", @"page": @0, @"page_count": @1,
            @"candidates": candidates} forKey:@"view"];
        for (NSNumber *dark in @[@NO, @YES]) for (NSNumber *percent in @[@100, @150])
            for (NSNumber *vertical in @[@NO, @YES]) for (NSNumber *expanded in @[@NO, @YES]) {
                if (expanded.boolValue && vertical.boolValue) continue;
                appearance.themeMode = dark.boolValue ? @"dark" : @"light";
                appearance.candidateScalePercent = percent.integerValue;
                appearance.vertical = vertical.boolValue;
                [controller setValue:expanded.boolValue ? candidates : nil forKey:@"expandedCandidates"];
                if (expanded.boolValue) assert([controller renderExpandedCandidates]);
                else [controller renderCandidates];
                // 连续组字逐次完整重画；最终大小相同也不能在中途先缩到裸卡片再补回阴影。
                const NSSize stableSize = panel.frame.size;
                panel.resizedSizes = [NSMutableArray array];
                for (NSString *editing in @[@"ces", @"cesh", @"ceshi"]) {
                    NSMutableDictionary *view = [[controller valueForKey:@"view"] mutableCopy];
                    view[@"editing_text"] = editing;
                    [controller setValue:view forKey:@"view"];
                    if (expanded.boolValue) assert([controller renderExpandedCandidates]);
                    else [controller renderCandidates];
                    assert(NSEqualSizes(panel.frame.size, stableSize));
                }
                if (panel.resizedSizes.count)
                    fprintf(stderr, "candidate intermediate resize: skin=%s dark=%d scale=%ld vertical=%d expanded=%d sizes=%s\n",
                        kind, dark.boolValue, (long)percent.integerValue, vertical.boolValue, expanded.boolValue,
                        panel.resizedSizes.description.UTF8String);
                assert(panel.resizedSizes.count == 0);
                panel.resizedSizes = nil;
                MSIMECandidateChromeView *chrome = (id)panel.contentView;
                assert(!panel.hasShadow && !chrome.nativeWindowShadowEnabled);
                const auto skin = [appearance candidateWindowSkinForDark:dark.boolValue];
                assert(skin.candidateSkin == identifier && skin.shadow.has_value());
                const bool panelLoaded = std::string(kind) == "panel" && !dark.boolValue;
                const bool disabled = std::string(kind) == "none" || panelLoaded;
                assert((chrome.candidateShadow.passes[0].alpha == 0) == disabled);
                assert(chrome.canvasPanelLoaded == panelLoaded);
                const NSRect card = chrome.cardBounds;
                const NSPoint cardOnScreen = [chrome convertPoint:card.origin toView:nil];
                assert(std::abs(NSMinX(panel.frame) + cardOnScreen.x - NSMinX(client.caret)) < 1.1);
                assert(std::abs(NSMinY(panel.frame) + NSMaxY(card) - NSMinY(client.caret) + 4) < 1.1);
                NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr
                    pixelsWide:ceil(NSWidth(chrome.bounds)) pixelsHigh:ceil(NSHeight(chrome.bounds)) bitsPerSample:8
                    samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
                [NSGraphicsContext saveGraphicsState];
                NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap];
                [chrome drawCandidateCard];
                [NSGraphicsContext restoreGraphicsState];
                double outside = 0;
                for (NSInteger x = 0; x < bitmap.pixelsWide; ++x) {
                    if (x >= NSMinX(card) && x < NSMaxX(card)) continue;
                    for (NSInteger y = 0; y < bitmap.pixelsHigh; ++y) outside += [bitmap colorAtX:x y:y].alphaComponent;
                }
                assert(disabled ? outside == 0 : outside > 5);
                if (!disabled) {
                    // 位图以左上为原点；来源阴影应向右下偏移，两侧等距取样检查真实方向。
                    const NSInteger offset = round(8 * percent.doubleValue / 100.0);
                    const NSInteger midX = round(NSMidX(card)), midY = round(NSHeight(chrome.bounds) - NSMidY(card));
                    const CGFloat left = [bitmap colorAtX:floor(NSMinX(card)) - offset y:midY].alphaComponent;
                    const CGFloat right = [bitmap colorAtX:ceil(NSMaxX(card)) + offset y:midY].alphaComponent;
                    const CGFloat top = [bitmap colorAtX:midX y:floor(NSHeight(chrome.bounds) - NSMaxY(card)) - offset].alphaComponent;
                    const CGFloat bottom = [bitmap colorAtX:midX y:ceil(NSHeight(chrome.bounds) - NSMinY(card)) + offset].alphaComponent;
                    assert(right > left && bottom > top);
                }
                if (!chrome.canvasPanelLoaded) {
                    NSColor *center = [bitmap colorAtX:round(NSMidX(card)) y:bitmap.pixelsHigh / 2];
                    assert(std::abs(center.alphaComponent - chrome.fillColor.alphaComponent) < 0.02);
                    NSColor *actual = [center colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
                    NSColor *expected = [chrome.fillColor colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
                    if (std::abs(actual.redComponent - expected.redComponent) >= 0.03)
                        fprintf(stderr, "shadow center: kind=%s dark=%d scale=%ld actual=%.4f expected=%.4f alpha=%.4f expected_alpha=%.4f\n",
                            kind, dark.boolValue, (long)percent.integerValue, actual.redComponent, expected.redComponent,
                            center.alphaComponent, chrome.fillColor.alphaComponent);
                    assert(std::abs(actual.redComponent - expected.redComponent) < 0.03);
                }
                if (!canvas && percent.integerValue == 100 && !vertical.boolValue && !expanded.boolValue) {
                    const int index = std::string(kind) == "none" ? 0 : std::string(kind) == "soft" ? 1 : std::string(kind) == "normal" ? 2 : 3;
                    strengths[dark.boolValue ? 1 : 0][index] = outside;
                    if (const char *output = getenv("MSIME_TEST_SHADOW_ARTIFACTS")) {
                        NSString *directory = @(output);
                        assert([NSFileManager.defaultManager createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:nil]);
                        NSString *file = [NSString stringWithFormat:@"%s-%@.png", kind, dark.boolValue ? @"dark" : @"light"];
                        assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
                            writeToFile:[directory stringByAppendingPathComponent:file] atomically:YES]);
                    }
                }
            }
        if (std::string(kind) == "panel") {
            // 同一包的明暗资源加载结果不同，原地刷新必须同步重排阴影边带。
            MSIMECandidatePanel *visible = [[MSIMECandidatePanel alloc] initWithContentRect:NSZeroRect
                styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
            [controller setValue:visible forKey:@"panel"];
            [controller setValue:nil forKey:@"expandedCandidates"];
            appearance.vertical = NO;
            appearance.themeMode = @"light";
            [controller renderCandidates];
            const NSSize initial = visible.frame.size;
            assert(visible.isVisible && ((MSIMECandidateChromeView *)visible.contentView).canvasPanelLoaded);
            appearance.themeMode = @"dark";
            visible.appearance = [appearance candidateAppearanceOverride];
            [controller refreshCandidateSkin];
            MSIMECandidateChromeView *darkContent = (id)visible.contentView;
            assert(!darkContent.canvasPanelLoaded && darkContent.shadowOuterInsets.left > 0 && !visible.hasShadow);
            assert(visible.frame.size.width > initial.width);
            appearance.themeMode = @"light";
            visible.appearance = [appearance candidateAppearanceOverride];
            [controller refreshCandidateSkin];
            assert(((MSIMECandidateChromeView *)visible.contentView).canvasPanelLoaded && !visible.hasShadow);
            assert(std::abs(visible.frame.size.width - initial.width) < 1.1);
        }
        [controller hideCandidatePanel:"empty"];
        [controller setValue:nil forKey:@"activeClient"];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    for (const auto &mode : strengths) assert(mode[0] == 0 && mode[1] > 5 && mode[2] > mode[1] && mode[3] > mode[2]);
    assert(strengths[1][2] > strengths[0][2]);
    // 显式进场预设不关闭原生阴影，换成皮肤阴影后也不留异步恢复回调。
    NSPanel *motionPanel = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, 200, 100)
        styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
    MSIMECandidateChromeView *native = [[MSIMECandidateChromeView alloc] initWithFrame:motionPanel.contentView.bounds];
    motionPanel.contentView = native;
    motionPanel.hasShadow = YES;
    MSIMECandidateMotion *motion = [MSIMECandidateMotion new];
    motion.reduceMotion = NO;
    msime::mac::SkinAnimation animation;
    animation.enter = msime::mac::SkinAnimationPreset::fade;
    animation.durationMs = 100;
    [motion showPanel:motionPanel animation:animation rows:@[]];
    assert(motionPanel.hasShadow && [native.layer animationForKey:@"msime-candidate-enter-opacity"] == nil);
    MSIMECandidateChromeView *replacement = [[MSIMECandidateChromeView alloc] initWithFrame:native.bounds];
    replacement.candidateShadow = msime::mac::ResolveCandidateShadow("none", false, false, false, 1);
    motionPanel.contentView = replacement;
    motionPanel.hasShadow = replacement.nativeWindowShadowEnabled;
    [motion showPanel:motionPanel animation:animation rows:@[]];
    assert([replacement.layer animationForKey:@"msime-candidate-enter-opacity"] == nil);
    assert(!motionPanel.hasShadow);
    [motion stop];
    [motionPanel orderOut:nil];
    std::filesystem::remove_all(root);
}

static double PagerInk(MSIMECandidatePagerButton *button, NSString *name)
{
    NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr
        pixelsWide:ceil(NSWidth(button.bounds)) pixelsHigh:ceil(NSHeight(button.bounds)) bitsPerSample:8
        samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    [NSGraphicsContext saveGraphicsState];
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap];
    [button drawRect:button.bounds];
    [NSGraphicsContext restoreGraphicsState];
    double ink = 0;
    for (NSInteger x = 0; x < bitmap.pixelsWide; ++x) for (NSInteger y = 0; y < bitmap.pixelsHigh; ++y)
        ink += [bitmap colorAtX:x y:y].alphaComponent;
    if (name) if (const char *output = getenv("MSIME_TEST_PAGER_ARTIFACTS")) {
        NSString *directory = @(output);
        assert([NSFileManager.defaultManager createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:nil]);
        assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
            writeToFile:[directory stringByAppendingPathComponent:name] atomically:YES]);
    }
    return ink;
}

// 卷轴按钮用候选字号与候选首行定位，预编辑及其行距不得移动按钮。
static void TestFactoryExpandToggle(RowLayoutClient *client)
{
    const auto root = std::filesystem::temp_directory_path() / ("msime-expand-toggle-" + std::string(NSUUID.UUID.UUIDString.UTF8String));
    for (BOOL canvas : {NO, YES}) {
        const char *identifier = canvas ? "canvas-expand" : "plain-expand";
        std::filesystem::create_directories(root / identifier);
        {
            std::ofstream manifest(root / identifier / "skin.toml");
            manifest << "schema_version = 2\nid = \"" << identifier << "\"\nname = \"合成卷轴\"\nversion = \"1\"\nbase = \""
                << (canvas ? "canvas" : "microsoft") << "\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n"
                << "[candidate_window]\nexpand_panel = true\npage_arrows = true\nitem_extra_height = 2\n";
            if (canvas) manifest << "line_gap_dip = 19\ncontent_insets_dip = { top = 5, right = 9, bottom = 13, left = 17 }\n";
            assert(manifest.good());
        }
        NSString *suite = [@"app.msime.test.expand-toggle." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@(identifier) forKey:@"MSIMEClientCustomCandidateSkin"];
        [defaults setObject:@(identifier) forKey:@"MSIMEClientCustomCandidateSkinDark"];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
            skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.vertical = NO; appearance.showsAppLogo = NO;
        appearance.showsCandidateExpandButton = YES; appearance.expandCandidates = YES;
        appearance.candidateTranslations = NO; appearance.preeditFontSize = 32;
        PagerTestController *controller = [PagerTestController new];
        VisibleRowLayoutPanel *panel = [[VisibleRowLayoutPanel alloc] initWithContentRect:NSZeroRect
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"]; [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"]; [controller setValue:[PagerTestSession new] forKey:@"session"];
        NSArray *rows = @[@{@"text": @"合成", @"highlighted": @YES}, @{@"text": @"验证"}, @{@"text": @"三"}, @{@"text": @"四"}];
        appearance.pageSize = 2;
        [controller setValue:@{@"session": @11, @"generation": @22, @"focused": @YES, @"editing_text": @"ce",
            @"page": @0, @"page_count": @2, @"candidates": [rows subarrayWithRange:NSMakeRange(0, 2)]} forKey:@"view"];
        for (BOOL dark : {NO, YES}) for (NSInteger font : {12, 24}) for (NSInteger percent : {100, 150})
            for (BOOL preedit : {NO, YES}) for (BOOL pageControls : {NO, YES}) for (BOOL expanded : {NO, YES}) {
                appearance.themeMode = dark ? @"dark" : @"light"; appearance.fontSize = font;
                appearance.candidateScalePercent = percent; appearance.showsCandidatePreedit = preedit;
                appearance.showsCandidatePageNumber = pageControls;
                [controller setValue:expanded ? rows : nil forKey:@"expandedCandidates"];
                [controller setValue:@0 forKey:@"expandedFirstRow"];
                if (expanded) assert([controller renderExpandedCandidates]); else [controller renderCandidates];
                MSIMECandidateChromeView *chrome = (id)panel.contentView;
                MSIMECandidateButton *toggle = CandidateButton(chrome, -3), *first = CandidateButton(chrome, 0);
                assert(toggle && first);
                const CGFloat scale = percent / 100.0, pointSize = font * scale;
                const CGFloat side = round(pointSize * 1.1);
                if (fabs(NSWidth(toggle.frame) - side) >= 0.01)
                    fprintf(stderr, "expand toggle size: font=%ld scale=%.1f expanded=%d actual=%.2f expected=%.2f\n",
                        (long)font, scale, expanded, NSWidth(toggle.frame), side);
                assert(fabs(NSWidth(toggle.frame) - side) < 0.01 && fabs(NSHeight(toggle.frame) - side) < 0.01);
                const CGFloat top = MAX(0, (pointSize * 1.35 + 2 * scale - side) / 2);
                if (expanded)
                    assert(fabs(NSMaxY(first.enclosingScrollView.frame) - NSMaxY(toggle.frame) - top) < 0.01);
                const CGFloat topTolerance = expanded ? 1.0 / panel.backingScaleFactor + 0.01 : 0.01;
                if (fabs(NSMaxY(CandidateFrame(first)) - NSMaxY(toggle.frame) - top) > topTolerance)
                    fprintf(stderr, "expand toggle top: canvas=%d dark=%d font=%ld percent=%ld preedit=%d pages=%d expanded=%d backing=%.1f first=%s toggle=%s expected=%.3f viewport=%s clip=%s document=%s\n",
                        canvas, dark, (long)font, (long)percent, preedit, pageControls, expanded, panel.backingScaleFactor,
                        NSStringFromRect(CandidateFrame(first)).UTF8String, NSStringFromRect(toggle.frame).UTF8String, top,
                        NSStringFromRect(first.enclosingScrollView.frame).UTF8String,
                        NSStringFromRect(first.enclosingScrollView.contentView.bounds).UTF8String,
                        NSStringFromRect(first.enclosingScrollView.documentView.frame).UTF8String);
                assert(fabs(NSMaxY(CandidateFrame(first)) - NSMaxY(toggle.frame) - top) <= topTolerance);
                const CGFloat right = canvas ? 9 * scale : NSMinX(CandidateFrame(first));
                if (fabs(NSWidth(chrome.cardBounds) - NSMaxX(toggle.frame) - right - scale) >= 0.01)
                    fprintf(stderr, "expand toggle right: canvas=%d font=%ld scale=%.1f expanded=%d card=%.2f toggle=%.2f right=%.2f\n",
                        canvas, (long)font, scale, expanded, NSWidth(chrome.cardBounds), NSMaxX(toggle.frame), right);
                assert(fabs(NSWidth(chrome.cardBounds) - NSMaxX(toggle.frame) - right - scale) < 0.01);
                for (NSView *child in chrome.candidateContentView.subviews) {
                    assert(![child.identifier isEqual:@"candidate-page-indicator"]);
                    if ([child isKindOfClass:MSIMECandidateButton.class] && child.tag >= 0)
                        assert(NSMaxX(child.frame) <= NSMinX(toggle.frame) - 2 * scale + 0.01);
                }
                assert([toggle.accessibilityLabel isEqual:expanded ? @"收起候选" : @"展开候选"]);
                assert([toggle isKindOfClass:MSIMECandidateExpandToggleButton.class]);
                assert(((MSIMECandidateExpandToggleButton *)toggle).expanded == expanded);
                const double ink = PagerInk((id)toggle, nil);
                assert(ink > 2);
                toggle.candidateHovered = YES;
                assert(PagerInk((id)toggle, nil) > ink);
                toggle.candidateHovered = NO;
                [toggle highlight:YES];
                assert(PagerInk((id)toggle, nil) > ink);
                [toggle highlight:NO];
                if (font == 24 && percent == 100 && !pageControls) if (const char *output = getenv("MSIME_TEST_EXPAND_ARTIFACTS")) {
                    NSString *directory = @(output);
                    assert([NSFileManager.defaultManager createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:nil]);
                    [chrome displayIfNeeded];
                    NSBitmapImageRep *bitmap = [chrome bitmapImageRepForCachingDisplayInRect:chrome.bounds];
                    [chrome cacheDisplayInRect:chrome.bounds toBitmapImageRep:bitmap];
                    NSString *name = [NSString stringWithFormat:@"%s-%@-%@-%@.png", identifier, dark ? @"dark" : @"light",
                        preedit ? @"preedit" : @"hidden", expanded ? @"expanded" : @"page"];
                    assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
                        writeToFile:[directory stringByAppendingPathComponent:name] atomically:YES]);
                }
                NSPoint center = [toggle convertPoint:NSMakePoint(NSMidX(toggle.bounds), NSMidY(toggle.bounds)) toView:chrome];
                assert([chrome hitTest:center] == toggle);
            }
        MSIMECandidateButton *stale = CandidateButton(panel.contentView, -3);
        assert([controller renderExpandedCandidates]);
        assert([NSApp sendAction:stale.action to:stale.target from:stale]);
        assert([controller valueForKey:@"expandedCandidates"] != nil);
        MSIMECandidateButton *collapse = CandidateButton(panel.contentView, -3);
        assert([NSApp sendAction:collapse.action to:collapse.target from:collapse]);
        assert([controller valueForKey:@"expandedCandidates"] == nil && controller.pointerReports == 0);
        appearance.showsCandidateExpandButton = NO;
        [controller setValue:nil forKey:@"expandedCandidates"]; [controller renderCandidates];
        assert(CandidateButton(panel.contentView, -3) == nil);
        [controller setValue:rows forKey:@"expandedCandidates"]; assert([controller renderExpandedCandidates]);
        assert(CandidateButton(panel.contentView, -3) == nil);
        [controller hideCandidatePanel:"empty"]; [controller setValue:nil forKey:@"activeClient"];
        [controller setValue:nil forKey:@"session"]; MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

static void TestFactoryPager(RowLayoutClient *client)
{
    const auto root = std::filesystem::temp_directory_path() / ("msime-pager-" + std::string(NSUUID.UUID.UUIDString.UTF8String));
    for (const char *kind : {"native", "default", "chevron", "off", "canvas", "expand"}) {
        const std::string identifier = std::string("pager-") + kind;
        const bool native = std::string(kind) == "native", canvas = std::string(kind) == "canvas";
        const bool off = std::string(kind) == "off", expand = std::string(kind) == "expand";
        const bool triangle = std::string(kind) == "default" || expand || canvas;
        if (!native) {
            std::filesystem::create_directories(root / identifier);
            std::ofstream manifest(root / identifier / "skin.toml");
            manifest << "schema_version = 2\nid = \"" << identifier << "\"\nname = \"合成翻页\"\nversion = \"1\"\nbase = \""
                << (canvas ? "canvas" : "microsoft") << "\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"light\", \"dark\"]\n[candidate_window]\n";
            if (off) manifest << "page_arrows = false\n";
            if (canvas) manifest << "page_arrows = true\npage_arrow_style = \"triangle\"\n";
            if (expand) manifest << "expand_panel = true\n";
            if (!triangle) manifest << "page_arrow_style = \"chevron\"\n";
            manifest << "[candidate.light]\ntext = \"#112233\"\nnumber = \"#334455\"\n";
            if (!canvas) manifest << "page_arrow_divider = \"#123456\"\n";
            manifest << "[candidate.dark]\ntext = \"#DDEEFF\"\nnumber = \"#BBCCDD\"\n";
            if (!canvas) manifest << "page_arrow_divider = \"#ABCDEF\"\n";
        }
        NSString *suite = [@"app.msime.test.pager." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        if (!native) {
            [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
            [defaults setObject:@(identifier.c_str()) forKey:@"MSIMEClientCustomCandidateSkin"];
            [defaults setObject:@(identifier.c_str()) forKey:@"MSIMEClientCustomCandidateSkinDark"];
        }
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
            skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        appearance.showsAppLogo = NO; appearance.showsCandidatePreedit = NO;
        appearance.showsCandidateExpandButton = YES; appearance.expandCandidates = YES;
        appearance.candidateTranslations = NO; appearance.candidateTheme = @"follow";
        PagerTestController *controller = [PagerTestController new];
        PagerTestSession *session = [PagerTestSession new];
        VisibleRowLayoutPanel *panel = [[VisibleRowLayoutPanel alloc] initWithContentRect:NSZeroRect
            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        [controller setValue:appearance forKey:@"appearance"]; [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"]; [controller setValue:session forKey:@"session"];
        for (BOOL dark : {NO, YES}) for (NSInteger percent : {100, 150}) for (BOOL vertical : {NO, YES})
            for (BOOL shown : {NO, YES}) for (NSUInteger page : {0u, 2u}) {
                appearance.themeMode = dark ? @"dark" : @"light"; appearance.vertical = vertical;
                appearance.candidateScalePercent = percent; appearance.showsCandidatePageNumber = shown;
                [controller setValue:@{@"session": @11, @"generation": @22, @"focused": @YES, @"editing_text": @"ce",
                    @"page": @(page), @"page_count": @3, @"candidates": @[@{@"text": @"合成", @"highlighted": @YES}, @{@"text": @"验证"}]} forKey:@"view"];
                [controller renderCandidates];
                MSIMECandidateChromeView *chrome = (id)panel.contentView;
                MSIMECandidateButton *previous = CandidateButton(chrome, -1), *next = CandidateButton(chrome, -2);
                if (!native) assert([controller expandedCandidatesOffered] == (expand && !vertical));
                const bool expected = shown && !off && !(expand && !vertical);
                assert((previous != nil) == expected && (next != nil) == expected);
                NSUInteger indicators = 0;
                for (NSView *child in chrome.candidateContentView.subviews)
                    if ([child.identifier isEqual:@"candidate-page-indicator"]) ++indicators;
                assert(indicators == (native && shown ? 1u : 0u));
                if (!expected) continue;
                assert(previous.enabled == (page > 0) && next.enabled == (page < 2));
                assert([previous.accessibilityLabel isEqual:@"上一页候选"] && [next.accessibilityLabel isEqual:@"下一页候选"]);
                if (!native) {
                    assert([previous isKindOfClass:MSIMECandidatePagerButton.class]);
                    const auto metrics = ((MSIMECandidatePagerButton *)previous).pagerMetrics;
                    assert(metrics.triangle == triangle);
                    MSIMECandidateButton *first = CandidateButton(chrome, 0), *last = CandidateButton(chrome, 1);
                    assert(NSMinY(previous.frame) == NSMinY(next.frame) && NSMaxX(previous.frame) <= NSMinX(next.frame));
                    if (vertical) {
                        assert(NSMaxY(previous.frame) < NSMinY(last.frame));
                        assert(std::abs(NSMinY(chrome.candidateRowsRect) - NSMinY(last.frame)) < 0.1);
                        assert(std::abs(NSMinX(previous.frame) + (metrics.buttonWidth - metrics.glyphWidth()) / 2 - NSMinX(first.frame) - first.textLeftInset) < 0.1);
                    } else {
                        assert(NSMinX(previous.frame) >= NSMaxX(last.frame));
                        assert(NSMinY(first.frame) == NSMinY(last.frame));
                        assert(std::abs(NSHeight(chrome.cardBounds) - 2 * NSMinX(first.frame) - NSHeight(first.frame)
                            - (canvas ? 2.0 * percent / 100.0 : 0)) < 0.1);
                        assert(std::abs(NSMinY(previous.frame) - NSMinY(first.frame) - metrics.marginBottom) < 0.1);
                    }
                    const auto tokens = [appearance candidateWindowSkinForDark:dark].tokens;
                    const auto glyph = triangle ? tokens.text : tokens.number;
                    assert([next.titleColor isEqual:next.enabled ? SkinColor(glyph) : [SkinColor(glyph) colorWithAlphaComponent:glyph.a * (triangle ? 0.25 : 0.3)]]);
                    NSBox *divider = nil;
                    for (NSView *child in chrome.candidateContentView.subviews)
                        if ([child.identifier isEqual:@"candidate-pager-divider"]) divider = (id)child;
                    assert((divider != nil) == (triangle && !vertical));
                    if (divider) assert([divider.fillColor isEqual:SkinColor(tokens.pageArrowDivider.value_or(msime::mac::Rgba{0, 0, 0, 0}))]);
                    if (divider && canvas) assert(!tokens.pageArrowDivider && divider.fillColor.alphaComponent == 0);
                    const double previousInk = PagerInk((id)previous, percent == 100 && !vertical && page == 0
                        ? [NSString stringWithFormat:@"%s-%@-disabled.png", kind, dark ? @"dark" : @"light"] : nil);
                    const double nextInk = PagerInk((id)next, percent == 100 && !vertical && page == 0
                        ? [NSString stringWithFormat:@"%s-%@-enabled.png", kind, dark ? @"dark" : @"light"] : nil);
                    const double enabledInk = page == 0 ? nextInk : previousInk;
                    const double disabledInk = page == 0 ? previousInk : nextInk;
                    assert(enabledInk > 2 && std::abs(disabledInk / enabledInk - (triangle ? 0.25 : 0.3)) < 0.04);
                    next.candidateHovered = next.enabled;
                    const double hoveredInk = PagerInk((id)next, nil);
                    assert(!next.enabled || hoveredInk > nextInk);
                    next.candidateHovered = NO;
                    if (percent == 100 && page == 0) if (const char *output = getenv("MSIME_TEST_PAGER_ARTIFACTS")) {
                        [chrome displayIfNeeded];
                        NSBitmapImageRep *bitmap = [chrome bitmapImageRepForCachingDisplayInRect:chrome.bounds];
                        [chrome cacheDisplayInRect:chrome.bounds toBitmapImageRep:bitmap];
                        NSString *name = [NSString stringWithFormat:@"panel-%s-%@-%@.png", kind,
                            dark ? @"dark" : @"light", vertical ? @"vertical" : @"horizontal"];
                        assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
                            writeToFile:[@(output) stringByAppendingPathComponent:name] atomically:YES]);
                    }

                }
                // 点击走原有真实控制器校验；重建后保留的旧按钮与禁用按钮都不能派发。
                MSIMECandidateButton *active = page == 0 ? next : previous;
                const NSUInteger before = session.commands;
                NSPoint hit = [active convertPoint:NSMakePoint(NSMidX(active.bounds), NSMidY(active.bounds)) toView:chrome];
                assert([chrome hitTest:hit] == active);
                assert([NSApp sendAction:active.action to:active.target from:active]);
                assert(session.commands == before + 1 && session.lastCommand == (page == 0 ? MSIME_NEXT_PAGE : MSIME_PREVIOUS_PAGE));
                assert(controller.pointerReports == session.commands);
                [controller changeCandidatePage:page == 0 ? previous : next];
                assert(session.commands == before + 1);
                [controller renderCandidates];
                [controller changeCandidatePage:active];
                assert(session.commands == before + 1);
            }
        if (!native && !off) for (NSInteger percent : {100, 150}) for (BOOL vertical : {NO, YES}) {
            if (expand && !vertical) continue;
            appearance.candidateScalePercent = percent; appearance.vertical = vertical;
            appearance.showsCandidatePageNumber = YES;
            const NSRect screen = NSScreen.mainScreen.visibleFrame;
            NSString *longText = [@"" stringByPaddingToLength:ceil(NSWidth(screen)) withString:@"合成长句" startingAtIndex:0];
            NSMutableArray *longRows = [NSMutableArray array];
            for (NSUInteger index = 0; index < 6; ++index) [longRows addObject:@{@"text": longText}];
            [controller setValue:@{@"session": @11, @"generation": @22, @"focused": @YES, @"editing_text": @"ce",
                @"page": @0, @"page_count": @3, @"candidates": longRows} forKey:@"view"];
            [controller renderCandidates];
            MSIMECandidateButton *previous = CandidateButton(panel.contentView, -1);
            assert(previous && NSWidth(panel.frame) <= floor(NSWidth(screen) / 2) + 1);
            for (NSUInteger index = 0; index < longRows.count; ++index) {
                MSIMECandidateButton *row = CandidateButton(panel.contentView, index);
                assert(row && NSMaxX(row.frame) <= (vertical ? NSWidth(panel.contentView.bounds) : NSMinX(previous.frame)) + 0.1);
                assert(!NSIntersectsRect(row.frame, previous.frame));
            }
        }
        [controller hideCandidatePanel:"empty"]; [controller setValue:nil forKey:@"activeClient"];
        [controller setValue:nil forKey:@"session"]; MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    std::filesystem::remove_all(root);
}

// 首行里按 identifier 找一个子视图，没有时为 nil。
static NSView *HeaderChild(NSView *content, NSString *identifier)
{
    for (NSView *child in MSIMECandidateControlsView(content).subviews)
        if ([child.identifier isEqual:identifier]) return child;
    return nil;
}

// 按钮画一遍取位图，比较画淡与否时两次用同一尺寸。
static NSBitmapImageRep *RenderedButton(MSIMECandidateButton *button)
{
    NSBitmapImageRep *bitmap = [button bitmapImageRepForCachingDisplayInRect:button.bounds];
    [button cacheDisplayInRect:button.bounds toBitmapImageRep:bitmap];
    return bitmap;
}

// 光标后编辑（`caret_edit_suffix` / `caret_edit_prefix`）：视图字段的读法、首行右侧的提示只在生效时出现、首选开头固定的那段画淡。
static void TestCaretEditSuffix(RowLayoutClient *client)
{
    // 字段缺省是没生效、空串；只认真布尔值，前缀在模式没生效时不算数。
    assert(!MSIMECaretEditSuffixActive(@{}) && !MSIMECaretEditSuffixActive(nil));
    assert(!MSIMECaretEditSuffixActive(@{@"caret_edit_suffix": @1}) && !MSIMECaretEditSuffixActive(@{@"caret_edit_suffix": @NO}));
    assert(MSIMECaretEditSuffixActive(@{@"caret_edit_suffix": @YES}));
    assert([MSIMECaretEditPrefix(@{}) isEqual:@""] && [MSIMECaretEditPrefix(@{@"caret_edit_suffix": @YES}) isEqual:@""]);
    assert([MSIMECaretEditPrefix(@{@"caret_edit_prefix": @"我好现"}) isEqual:@""]);
    assert([MSIMECaretEditPrefix(@{@"caret_edit_suffix": @YES, @"caret_edit_prefix": @3}) isEqual:@""]);
    assert([MSIMECaretEditPrefix(@{@"caret_edit_suffix": @YES, @"caret_edit_prefix": @"我好现"}) isEqual:@"我好现"]);

    // 画淡长度：只有 `id.index` 为 0 的首选，按标量数在画出来的文字上换算成 UTF-16 长度。
    NSDictionary *active = @{@"caret_edit_suffix": @YES, @"caret_edit_prefix": @"我好现"};
    NSDictionary *first = @{@"text": @"我好现吃火锅", @"id": @{@"session": @1, @"generation": @1, @"index": @0}};
    NSDictionary *second = @{@"text": @"吃火锅", @"id": @{@"session": @1, @"generation": @1, @"index": @1}};
    assert(MSIMECaretEditDimmedLength(active, first, @"我好现吃火锅") == 3);
    assert(MSIMECaretEditDimmedLength(active, second, @"吃火锅") == 0);
    assert(MSIMECaretEditDimmedLength(@{@"caret_edit_prefix": @"我好现"}, first, @"我好现吃火锅") == 0);
    assert(MSIMECaretEditDimmedLength(@{@"caret_edit_suffix": @YES}, first, @"我好现吃火锅") == 0);
    assert(MSIMECaretEditDimmedLength(active, @{@"text": @"我好现吃火锅"}, @"我好现吃火锅") == 0);
    assert(MSIMECaretEditDimmedLength(active, @{@"text": @"我", @"id": @{@"index": @"0"}}, @"我") == 0);
    // 画出来的是繁体时数的仍是同样多个字；文字比固定那段短时不越界。
    assert(MSIMECaretEditDimmedLength(@{@"caret_edit_suffix": @YES, @"caret_edit_prefix": @"这里"}, first, @"這裡好") == 2);
    assert(MSIMECaretEditDimmedLength(active, first, @"我好") == 2);
    // 增补平面的字是一个标量、两个 UTF-16 码元，不切开代理对。
    NSDictionary *astral = @{@"caret_edit_suffix": @YES, @"caret_edit_prefix": @"𠮷野"};
    assert(@"𠮷野".length == 3);
    assert(MSIMECaretEditDimmedLength(astral, first, @"𠮷野家") == 3);
    assert(MSIMECaretEditDimmedLength(@{@"caret_edit_suffix": @YES, @"caret_edit_prefix": @"a𠮷"}, first, @"b𠮷c") == 3);

    NSString *suite = [@"app.msime.test.caret-edit." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.themeMode = @"light";
    appearance.showsCandidatePreedit = YES;
    appearance.showsCandidatePageNumber = YES;
    appearance.showsCandidateExpandButton = NO;
    appearance.candidateTranslations = NO;
    MSIMEInputController *controller = [[MSIMEInputController alloc] init];
    RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    NSArray *candidates = @[
        @{@"text": @"我好现吃火锅", @"highlighted": @YES, @"source": @0, @"id": @{@"session": @1, @"generation": @2, @"index": @0}},
        @{@"text": @"吃火锅", @"source": @0, @"id": @{@"session": @1, @"generation": @2, @"index": @1}},
        @{@"text": @"吃", @"source": @0, @"id": @{@"session": @1, @"generation": @2, @"index": @2}}];
    NSDictionary *plain = @{@"focused": @YES, @"session": @1, @"generation": @2, @"editing_text": @"wohaoxianchihuoguo",
                            @"caret_position": @9, @"page": @0, @"page_count": @3, @"candidates": candidates};
    NSMutableDictionary *editing = [plain mutableCopy];
    editing[@"caret_edit_suffix"] = @YES;
    editing[@"caret_edit_prefix"] = @"我好现";
    for (BOOL vertical : {NO, YES}) for (BOOL logo : {NO, YES}) for (NSNumber *pages : @[@1, @3]) {
        appearance.vertical = vertical;
        appearance.showsAppLogo = logo;
        NSMutableDictionary *off = [plain mutableCopy], *on = [editing mutableCopy];
        off[@"page_count"] = pages;
        on[@"page_count"] = pages;
        // 没有生效：没有提示，首选整段照常画。
        [controller setValue:[off copy] forKey:@"view"];
        [controller renderCandidates];
        assert(!HeaderChild(panel.contentView, @"candidate-caret-edit-hint"));
        for (NSInteger tag = 0; tag < 3; ++tag) assert(CandidateButton(panel.contentView, tag).dimmedPrefixLength == 0);
        const CGFloat offWidth = NSWidth(panel.frame);

        // 生效：首行右侧有提示，在翻页控件左边、拼音右边，整段在卡片里；首选开头三个字画淡，别的格不变。
        [controller setValue:[on copy] forKey:@"view"];
        [controller renderCandidates];
        NSTextField *hint = (NSTextField *)HeaderChild(panel.contentView, @"candidate-caret-edit-hint");
        assert([hint isKindOfClass:NSTextField.class] && [hint.stringValue isEqual:@"候选编辑：光标后"]);
        assert([hint.accessibilityLabel isEqual:@"候选编辑：光标后"]);
        const CGFloat cardWidth = NSWidth(MSIMECandidateControlsView(panel.contentView).bounds);
        assert(NSMinX(hint.frame) >= 0 && NSMaxX(hint.frame) <= cardWidth + 0.5 && NSWidth(hint.frame) > 0);
        assert(ceil([hint.stringValue sizeWithAttributes:@{NSFontAttributeName: hint.font}].width) <= NSWidth(hint.frame) + 0.5);
        NSView *preedit = HeaderChild(panel.contentView, @"candidate-preedit");
        assert(preedit && NSMaxX(preedit.frame) <= NSMinX(hint.frame) + 0.5);
        if (NSView *mark = HeaderChild(panel.contentView, @"candidate-logo")) assert(NSMaxX(mark.frame) <= NSMinX(hint.frame) + 0.5);
        if (pages.unsignedIntegerValue > 1) {
            NSView *indicator = HeaderChild(panel.contentView, @"candidate-page-indicator");
            MSIMECandidateButton *previous = CandidateButton(panel.contentView, -1);
            assert(indicator && previous && NSMaxX(hint.frame) <= NSMinX(indicator.frame) + 0.5 && NSMaxX(hint.frame) <= NSMinX(previous.frame));
            // 颜色是次要色，和「1 / 3」一样。
            assert(hint.textColor && [hint.textColor isEqual:((NSTextField *)indicator).textColor]);
        }
        MSIMECandidateButton *head = CandidateButton(panel.contentView, 0);
        assert(head.dimmedPrefixLength == 3 && CandidateButton(panel.contentView, 1).dimmedPrefixLength == 0 &&
               CandidateButton(panel.contentView, 2).dimmedPrefixLength == 0);
        // 提示在首行，不压在候选行上。
        assert(NSMinY(hint.frame) >= NSMaxY(head.frame) - 0.5);
        assert(NSWidth(panel.frame) >= offWidth - 0.5);
    }

    // 不显示拼音、logo 和翻页时首行原本不占高度，生效时为提示留出首行。
    appearance.vertical = NO;
    appearance.showsAppLogo = NO;
    appearance.showsCandidatePreedit = NO;
    NSMutableDictionary *bare = [editing mutableCopy];
    bare[@"page_count"] = @1;
    NSMutableDictionary *bareOff = [plain mutableCopy];
    bareOff[@"page_count"] = @1;
    [controller setValue:[bareOff copy] forKey:@"view"];
    [controller renderCandidates];
    assert(!HeaderChild(panel.contentView, @"candidate-caret-edit-hint"));
    const CGFloat bareHeight = NSHeight(panel.frame);
    [controller setValue:[bare copy] forKey:@"view"];
    [controller renderCandidates];
    NSView *bareHint = HeaderChild(panel.contentView, @"candidate-caret-edit-hint");
    assert(bareHint && NSHeight(panel.frame) > bareHeight);
    assert(NSMinY(bareHint.frame) >= NSMaxY(CandidateButton(panel.contentView, 0).frame) - 0.5);

    // 画淡确实落在固定那段上：同一格去掉画淡再画一遍，只有开头那几个字的像素变了，后面的字一模一样。高亮放到第二格，首选用普通文字色和次要色比较。
    NSMutableDictionary *unhighlighted = [bare mutableCopy];
    unhighlighted[@"candidates"] = @[
        @{@"text": @"我好现吃火锅", @"source": @0, @"id": @{@"session": @1, @"generation": @2, @"index": @0}},
        @{@"text": @"吃火锅", @"highlighted": @YES, @"source": @0, @"id": @{@"session": @1, @"generation": @2, @"index": @1}}];
    [controller setValue:[unhighlighted copy] forKey:@"view"];
    [controller renderCandidates];
    MSIMECandidateButton *head = CandidateButton(panel.contentView, 0);
    assert(head.dimmedPrefixLength == 3 && !head.candidateHighlighted);
    NSBitmapImageRep *dimmed = RenderedButton(head);
    head.dimmedPrefixLength = 0;
    NSBitmapImageRep *full = RenderedButton(head);
    assert(dimmed.pixelsWide == full.pixelsWide && dimmed.pixelsHigh == full.pixelsHigh && dimmed.pixelsWide > 0);
    const CGFloat pixelsPerPoint = dimmed.pixelsWide / NSWidth(head.bounds);
    const CGFloat boundary = head.contentLeft + [@"我好现" sizeWithAttributes:@{NSFontAttributeName: head.font}].width;
    BOOL prefixChanged = NO, suffixChanged = NO;
    for (NSInteger y = 0; y < dimmed.pixelsHigh; ++y)
        for (NSInteger x = 0; x < dimmed.pixelsWide; ++x) {
            if (x > (boundary - 1) * pixelsPerPoint && x < (boundary + 1) * pixelsPerPoint) continue;
            NSColor *a = [dimmed colorAtX:x y:y], *b = [full colorAtX:x y:y];
            if ([a isEqual:b]) continue;
            if (x < boundary * pixelsPerPoint) prefixChanged = YES;
            else suffixChanged = YES;
        }
    assert(prefixChanged && !suffixChanged);

    // 增补平面的字：首选开头的「𠮷」是两个 UTF-16 码元，画淡长度跟着是 2。
    NSMutableDictionary *astralView = [bare mutableCopy];
    astralView[@"caret_edit_prefix"] = @"𠮷";
    astralView[@"candidates"] = @[@{@"text": @"𠮷野家", @"highlighted": @YES, @"source": @0, @"id": @{@"session": @1, @"generation": @2, @"index": @0}}];
    [controller setValue:[astralView copy] forKey:@"view"];
    [controller renderCandidates];
    assert(CandidateButton(panel.contentView, 0).dimmedPrefixLength == 2);
    appearance.showsCandidatePreedit = YES;
}

int main(int argc, char **argv)
{
    @autoreleasepool
    {
        [NSApplication sharedApplication];
        if (argc == 2 && std::string(argv[1]) == "--adaptive") {
            RowLayoutClient *client = [RowLayoutClient new];
            const NSRect screen = NSScreen.mainScreen.visibleFrame;
            client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
            TestAdaptiveCandidateLayout(client);
            TestAdaptiveReviewFixes(client);
            TestFilteredFixedPages(client);
            return 0;
        }
        // 动效需要独立的主事件循环，避免大布局矩阵的旧窗口回调占用计时阶段。
        if (argc == 2 && std::string(argv[1]) == "--declared-motion") {
            RowLayoutClient *client = [RowLayoutClient new];
            const NSRect screen = NSScreen.mainScreen.visibleFrame;
            client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
            TestDeclaredMotion(client);
            TestDecorationMotion(client);
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--factory-layout") {
            RowLayoutClient *client = [RowLayoutClient new];
            const NSRect screen = NSScreen.mainScreen.visibleFrame;
            client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
            @autoreleasepool { TestFactoryFitAndJustify(client); }
            @autoreleasepool { TestFactoryPreeditMargins(client); }
            @autoreleasepool { TestSkinPreeditGap(client); }
            @autoreleasepool { TestFactoryCornerRadii(client); }
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--user-geometry") {
            TestCandidateFillEdges();
            TestUserPaddingSelectedBar();
            TestUserGeometry();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--skin-preedit-gap") {
            RowLayoutClient *client = [RowLayoutClient new];
            const NSRect screen = NSScreen.mainScreen.visibleFrame;
            client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
            @autoreleasepool { TestSkinPreeditGap(client); }
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--shadow-host") {
            RowLayoutClient *client = [RowLayoutClient new];
            NSRect screen = NSScreen.mainScreen.visibleFrame;
            client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
            @autoreleasepool { TestFactoryShadows(client); }
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--expand-toggle") {
            RowLayoutClient *client = [RowLayoutClient new];
            const NSRect screen = NSScreen.mainScreen.visibleFrame;
            client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
            @autoreleasepool { TestFactoryExpandToggle(client); }
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--pager-host") {
            RowLayoutClient *client = [RowLayoutClient new];
            NSRect screen = NSScreen.mainScreen.visibleFrame;
            client.caret = NSMakeRect(NSMinX(screen) + 100, NSMidY(screen), 1, 20);
            @autoreleasepool { TestFactoryPager(client); }
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--caret-edit-suffix") {
            RowLayoutClient *client = [RowLayoutClient new];
            const NSRect screen = NSScreen.mainScreen.visibleFrame;
            client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
            @autoreleasepool { TestCaretEditSuffix(client); }
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--glass-host-visual") { TestHostAppearance(YES); return 0; }
        if (argc == 4 && std::string(argv[1]) == "--canvas-upstream") {
            NSString *output = @(argv[3]);
            assert([NSFileManager.defaultManager createDirectoryAtPath:output withIntermediateDirectories:YES attributes:nil error:nil]);
            TestCanvasHost(std::filesystem::path(argv[2]), @"green_leaf", output);
            TestCanvasHost(std::filesystem::path(argv[2]), @"sakura-night", output);
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--host-appearance") {
            @autoreleasepool { TestCanvasHostFixture(); }
            @autoreleasepool { TestHostAppearance(NO); }
            return 0;
        }
        NSString *suite = [@"app.msime.test.rowfit." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        appearance.vertical = NO;
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        RowLayoutClient *client = [RowLayoutClient new];
        NSRect screen = NSScreen.mainScreen.visibleFrame;
        client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
        RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
                                                                  styleMask:NSWindowStyleMaskBorderless |
                                                                            NSWindowStyleMaskNonactivatingPanel
                                                                    backing:NSBackingStoreBuffered
                                                                      defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];

        NSDictionary *singleView = @{@"focused": @YES, @"editing_text": @"ceshi", @"page": @0, @"page_count": @1,
                                     @"candidates": @[@{@"text": @"测试", @"highlighted": @YES}]};
        [controller setValue:singleView forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *measured = CandidateButton(panel.contentView, 0);
        assert(measured);
        // 实际产品渲染入口启用系统材质，切到自定义主题后恢复原来的皮肤底板。
        assert(((MSIMECandidateChromeView *)panel.contentView).liquidGlassEnabled);
        appearance.globalTheme = @"custom";
        [controller renderCandidates];
        assert(!((MSIMECandidateChromeView *)panel.contentView).liquidGlassEnabled);
        appearance.globalTheme = @"system";
        [controller renderCandidates];
        measured = CandidateButton(panel.contentView, 0);
        NSFont *rowFont = measured.font;
        const CGFloat glyphWidth = [@"水" sizeWithAttributes:@{NSFontAttributeName: rowFont}].width;
        assert(glyphWidth > 0);

        const CGFloat halfScreen = MAX(80, floor(screen.size.width * 0.5));

        // 固定数量横排不再补最小宽度；竖排保留原来的 7em 下限。
        NSMutableDictionary *oneCharacterView = [singleView mutableCopy];
        oneCharacterView[@"candidates"] = @[@{@"text": @"的", @"highlighted": @YES}];
        [controller setValue:[oneCharacterView copy] forKey:@"view"];
        [controller renderCandidates];
        assert(panel.contentView.frame.size.width < MIN(halfScreen, 7 * rowFont.pointSize) - 0.5);
        appearance.vertical = YES;
        [controller renderCandidates];
        assert(panel.contentView.frame.size.width >= MIN(halfScreen, 7 * rowFont.pointSize) - 0.5);
        // A vertical row stretches across the whole card.
        MSIMECandidateButton *oneCharacter = CandidateButton(panel.contentView, 0);
        assert(oneCharacter && fabs(NSWidth(oneCharacter.frame) - (panel.contentView.frame.size.width - 2 * NSMinX(oneCharacter.frame))) <= 1.0);
        appearance.vertical = NO;
        NSString *(^glyphs)(CGFloat) = ^NSString *(CGFloat points) {
            return [@"" stringByPaddingToLength:(NSUInteger)MAX(2.0, floor(points / glyphWidth))
                                     withString:@"水杉输入法" startingAtIndex:0];
        };

        // A sentence worth four fifths of half the screen, then eight candidates that together take far more than a whole screen line: the card grows past half the screen towards the screen less its margins, and the page still breaks onto new lines.
        NSString *sentence = glyphs(halfScreen * 0.8 - 60);
        NSString *shortCandidate = glyphs(halfScreen * 0.24);
        NSMutableArray *page = [NSMutableArray arrayWithObject:@{@"text": sentence, @"highlighted": @YES}];
        while (page.count < 9) [page addObject:@{@"text": shortCandidate}];
        NSMutableDictionary *pageView = [singleView mutableCopy];
        pageView[@"candidates"] = [page copy];
        [controller setValue:[pageView copy] forKey:@"view"];
        [controller renderCandidates];

        MSIMECandidateButton *sentenceButton = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *tailButton = CandidateButton(panel.contentView, 8);
        assert(sentenceButton && tailButton);
        const CGFloat sentenceWidth = ceil([sentence sizeWithAttributes:@{NSFontAttributeName: rowFont}].width);
        // The head keeps its full width and every candidate keeps its natural width; the ones that do not fit start new lines below.
        assert(sentenceButton.frame.size.width >= sentenceWidth);
        assert(!sentenceButton.itemLayout.textWrapped);
        assert(NSMaxY(tailButton.frame) <= NSMinY(sentenceButton.frame) + 0.5);
        assert(!tailButton.itemLayout.textWrapped);
        assert(fabs(tailButton.frame.size.width - CandidateButton(panel.contentView, 1).frame.size.width) <= 1.0);
        for (NSInteger tag = 0; tag < 9; ++tag) {
            NSRect frame = CandidateButton(panel.contentView, tag).frame;
            assert(NSMinX(frame) >= 0 && NSMaxX(frame) <= panel.frame.size.width + 0.5 && NSMinY(frame) >= 0);
        }
        assert(panel.frame.size.width <= MAX(halfScreen, floor(screen.size.width - 2 * MSIMECandidateScreenMargin)) + 0.5);

        // A page that fits keeps every candidate at its natural width on one line.
        NSMutableDictionary *narrowView = [singleView mutableCopy];
        narrowView[@"candidates"] = @[@{@"text": @"测试", @"highlighted": @YES}, @{@"text": @"测试测试"}];
        [controller setValue:[narrowView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *shorter = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *longer = CandidateButton(panel.contentView, 1);
        assert(shorter && longer);
        assert(longer.frame.size.width > shorter.frame.size.width);
        assert(longer.frame.size.width - shorter.frame.size.width >= 2 * glyphWidth - 1.0);
        assert(shorter.frame.origin.y == longer.frame.origin.y && NSMaxX(shorter.frame) == NSMinX(longer.frame));
        const CGFloat oneLine = shorter.frame.size.height;

        // One candidate wider than a whole line is narrowed to the line and wraps inside it rather than being cut off.
        NSString *paragraph = glyphs(screen.size.width * 3);
        NSMutableDictionary *wideView = [singleView mutableCopy];
        wideView[@"candidates"] = @[@{@"text": paragraph, @"highlighted": @YES}, @{@"text": @"测试"}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *wrapped = CandidateButton(panel.contentView, 0);
        assert(wrapped.itemLayout.textWrapped && wrapped.frame.size.height > oneLine * 1.5);
        assert(panel.frame.size.width <= MAX(halfScreen, floor(screen.size.width - 2 * MSIMECandidateScreenMargin)) + 0.5 && NSMaxX(wrapped.frame) <= panel.frame.size.width + 0.5);
        assert(NSMaxY(CandidateButton(panel.contentView, 1).frame) <= NSMinY(wrapped.frame) + 0.5);

        // A horizontal page whose glosses are wider than half the screen grows past it instead of wrapping them, as long as the screen has room: every gloss keeps the single line a short one gets.
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"translation": @"Chinese", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *shortGloss = CandidateButton(panel.contentView, 0);
        const CGFloat glossLine = shortGloss.itemLayout.translation.height;
        assert(glossLine > 0 && shortGloss.translationFont);
        // Six glosses of a quarter of half the screen each: half again as wide as a half-screen card, still well inside the screen.
        const CGFloat glossUnit = [@"gloss " sizeWithAttributes:@{NSFontAttributeName: shortGloss.translationFont}].width;
        NSString *sentenceGloss = [@"" stringByPaddingToLength:(NSUInteger)MAX(8.0, ceil(halfScreen / 4 / glossUnit * 6)) withString:@"gloss " startingAtIndex:0];
        NSMutableArray *glossPage = [NSMutableArray array];
        while (glossPage.count < 6) [glossPage addObject:@{@"text": @"测试", @"translation": sentenceGloss}];
        glossPage[0] = @{@"text": @"测试", @"translation": sentenceGloss, @"highlighted": @YES};
        wideView[@"candidates"] = [glossPage copy];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        CGFloat glossTotal = 0;
        for (NSInteger tag = 0; tag < 6; ++tag) {
            MSIMECandidateButton *button = CandidateButton(panel.contentView, tag);
            assert(button && fabs(button.itemLayout.translation.height - glossLine) < 0.5);
            assert(fabs(NSMinY(button.frame) - NSMinY(CandidateButton(panel.contentView, 0).frame)) < 0.5);
            glossTotal += button.itemLayout.translation.width;
        }
        assert(glossTotal > halfScreen && panel.frame.size.width > halfScreen + 0.5);
        assert(panel.frame.size.width <= screen.size.width + 0.5);

        // Vertical: the card is capped at half the screen, a long sentence wraps into a taller row than its neighbours, and rows stack at their own heights.
        appearance.vertical = YES;
        wideView[@"candidates"] = @[@{@"text": @"测试", @"highlighted": @YES}, @{@"text": paragraph}, @{@"text": @"测试测试"}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *first = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *tall = CandidateButton(panel.contentView, 1);
        MSIMECandidateButton *last = CandidateButton(panel.contentView, 2);
        assert(first && tall && last);
        assert(panel.frame.size.width <= halfScreen + 0.5);
        assert(tall.itemLayout.textWrapped && tall.frame.size.height > first.frame.size.height * 2);
        assert(fabs(first.frame.size.height - last.frame.size.height) < 0.5);
        assert(fabs(NSMinY(first.frame) - NSMaxY(tall.frame)) < 0.5 && fabs(NSMinY(tall.frame) - NSMaxY(last.frame)) < 0.5);
        assert(first.frame.size.width == tall.frame.size.width && tall.frame.size.width == last.frame.size.width);

        // The 辅助码 is a run of its own: the button's text is the candidate alone, the annotation is drawn after it, and the tooltip and accessibility label keep the combined form.
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"annotation": @"(aB)", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *annotated = CandidateButton(panel.contentView, 0);
        assert(![annotated.title containsString:@"(aB)"] && [annotated.title containsString:@"汉语"]);
        assert([annotated.annotation isEqual:@"(aB)"] && !annotated.itemLayout.annotation.below);
        assert([annotated.toolTip isEqual:@"汉语(aB)"] && [annotated.accessibilityLabel containsString:@"汉语(aB)"]);
        // A text that fills the column pushes the annotation under it, where it takes a line of its own.
        wideView[@"candidates"] = @[@{@"text": paragraph, @"annotation": @"(aB)", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        annotated = CandidateButton(panel.contentView, 0);
        assert(annotated.itemLayout.annotation.below && annotated.itemLayout.annotation.y >= annotated.itemLayout.textHeight);
        assert(annotated.frame.size.height >= annotated.itemLayout.height - 0.5);

        // A vertical gloss stays on the candidate's line when it fits and wraps under it when it does not.
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"translation": @"Chinese", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        assert(!CandidateButton(panel.contentView, 0).itemLayout.translation.below);
        NSString *longGloss = [@"" stringByPaddingToLength:(NSUInteger)(screen.size.width / 4) withString:@"gloss " startingAtIndex:0];
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"translation": longGloss, @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *glossed = CandidateButton(panel.contentView, 0);
        assert(glossed.itemLayout.translation.below && glossed.translationBelow);
        assert(glossed.itemLayout.translation.height > glossed.itemLayout.textHeight);
        assert(panel.frame.size.width <= halfScreen + 0.5);
        NSBitmapImageRep *bitmap = [glossed bitmapImageRepForCachingDisplayInRect:glossed.bounds];
        assert(bitmap);
        [glossed cacheDisplayInRect:glossed.bounds toBitmapImageRep:bitmap];

        // A Korean Hanja row shows only the Hanja on its line. Its 훈음 is drawn on the gloss line under it whatever the translation switches say, never as `translation`, which is what the gloss chords commit; the tooltip and accessibility label keep it apart from the Hanja.
        appearance.vertical = NO;
        appearance.candidateTranslations = NO;
        appearance.candidateEnglishGloss = NO;
        NSString *reading = @"나라 이름 한, 한나라 한";
        NSDictionary *hanjaBase = @{@"focused": @YES, @"scheme": @(msime::mac::KoreanScheme), @"local_mode": @"none", @"editing_text": @"한",
                                    @"candidate_list_open": @YES, @"page": @0, @"page_count": @1};
        NSMutableDictionary *hanjaView = [hanjaBase mutableCopy];
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"annotation": reading, @"highlighted": @YES}, @{@"text": @"漢", @"annotation": @"한수 한"}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *hanja = CandidateButton(panel.contentView, 0);
        assert(hanja && [hanja.title isEqual:@"1  韓"] && hanja.annotation.length == 0 && hanja.itemLayout.annotation.width == 0);
        assert([hanja.glossReading isEqual:reading] && hanja.translation.length == 0);
        assert(hanja.itemLayout.translation.width > 0 && hanja.itemLayout.translation.below && hanja.translationBelow);
        assert(hanja.itemLayout.translation.y >= hanja.itemLayout.textHeight);
        assert([hanja.toolTip isEqual:[@"韓\n" stringByAppendingString:reading]]);
        assert([hanja.accessibilityLabel isEqual:[@"1  韓 " stringByAppendingString:reading]]);
        // The Hanja's own column is no wider than the reading line under it needs: an inline 훈음 at the candidate size made it several times wider.
        const CGFloat readingWidth = ceil([reading sizeWithAttributes:@{NSFontAttributeName: hanja.translationFont}].width);
        const CGFloat inlineWidth = ceil([reading sizeWithAttributes:@{NSFontAttributeName: rowFont}].width);
        assert(hanja.itemLayout.translation.width <= readingWidth + 0.5 && readingWidth < inlineWidth);
        bitmap = [hanja bitmapImageRepForCachingDisplayInRect:hanja.bounds];
        [hanja cacheDisplayInRect:hanja.bounds toBitmapImageRep:bitmap];

        // The 훈음 line is reserved with both translation switches off, so a Hanja with no 훈음 is as tall as one with it and the card does not jump; a Chinese page reserves nothing then.
        const CGFloat hanjaHeight = NSHeight(hanja.frame);
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"highlighted": @YES}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *bare = CandidateButton(panel.contentView, 0);
        assert(bare.glossReading.length == 0 && fabs(NSHeight(bare.frame) - hanjaHeight) < 0.5);
        [controller setValue:@{@"focused": @YES, @"editing_text": @"han", @"page": @0, @"page_count": @1,
                               @"candidates": @[@{@"text": @"韩", @"highlighted": @YES}]} forKey:@"view"];
        [controller renderCandidates];
        assert(NSHeight(CandidateButton(panel.contentView, 0).frame) + glossLine * 0.5 < hanjaHeight);

        // A real translation goes on the line after the 훈음, and only it is the row's `translation`.
        appearance.candidateTranslations = YES;
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"annotation": reading, @"translation": @"Korea", @"highlighted": @YES}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *translated = CandidateButton(panel.contentView, 0);
        assert([translated.translation isEqual:@"Korea"] && [translated.glossReading isEqual:reading]);
        assert(translated.itemLayout.translation.height > glossLine * 1.5);
        NSString *translatedTip = [NSString stringWithFormat:@"韓\n%@\nKorea", reading];
        assert([translated.toolTip isEqual:translatedTip]);
        // An armed translation column underlines the translation, not the reading above it.
        translated.armedGlossColumn = 1;
        bitmap = [translated bitmapImageRepForCachingDisplayInRect:translated.bounds];
        [translated cacheDisplayInRect:translated.bounds toBitmapImageRep:bitmap];

        // Vertical follows the vertical gloss rule: the 훈음 stays beside the Hanja when it fits, and nothing is reserved.
        appearance.vertical = YES;
        appearance.candidateTranslations = NO;
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"annotation": reading, @"highlighted": @YES}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *verticalHanja = CandidateButton(panel.contentView, 0);
        assert(verticalHanja.itemLayout.translation.width > 0 && !verticalHanja.itemLayout.translation.below && !verticalHanja.translationBelow);
        assert(verticalHanja.annotation.length == 0 && [verticalHanja.glossReading isEqual:reading]);

        // 脚本插件的注释收拾成一行再画：换行、U+2028、双向控制符和零宽字符连成的一段变成一个空格，首尾空白去掉，不是字符串时为空，超过 32 个 UTF-16 码元时在完整字符处截断补「…」，不拆开代理对。
        assert([MSIMECandidateScriptAnnotationRun(@"甲\n乙\u2028丙\u202E丁\u200B戊\u2029") isEqual:@"甲 乙 丙 丁 戊"]);
        assert([MSIMECandidateScriptAnnotationRun(@"\r\n 常用 \t") isEqual:@"常用"]);
        assert(MSIMECandidateScriptAnnotationRun(@42).length == 0 && MSIMECandidateScriptAnnotationRun(nil).length == 0);
        assert(MSIMECandidateScriptAnnotationRun(@"\u202E \u200B").length == 0);
        NSString *thirtyTwo = [@"" stringByPaddingToLength:32 withString:@"水" startingAtIndex:0];
        assert([MSIMECandidateScriptAnnotationRun(thirtyTwo) isEqual:thirtyTwo]);
        NSString *cutCJK = MSIMECandidateScriptAnnotationRun([thirtyTwo stringByAppendingString:@"杉杉"]);
        assert(cutCJK.length == 32 && [cutCJK hasSuffix:@"…"] && [cutCJK hasPrefix:[thirtyTwo substringToIndex:31]]);
        NSString *emoji = [@"" stringByPaddingToLength:40 withString:@"😀" startingAtIndex:0];
        NSString *cutEmoji = MSIMECandidateScriptAnnotationRun(emoji);
        assert(cutEmoji.length == 31 && [cutEmoji isEqual:[[emoji substringToIndex:30] stringByAppendingString:@"…"]]);

        // 横排：插件注释是释义行的最后一行，排在译文之后；它不进引擎辅助码那一段，也不是按列上屏的 `translation`。悬停提示带着它，读屏标签说明它是插件注释；没有注释的行什么都不多。
        appearance.vertical = NO;
        appearance.candidateTranslations = YES;
        NSDictionary *scriptBase = @{@"focused": @YES, @"editing_text": @"hanyu", @"page": @0, @"page_count": @1};
        NSMutableDictionary *scriptView = [scriptBase mutableCopy];
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"annotation": @"(aB)", @"translation": @"Chinese", @"script_annotation": @"常用词", @"highlighted": @YES},
                                      @{@"text": @"韩语", @"annotation": @"(aB)", @"translation": @"Korean"}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *scripted = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *unscripted = CandidateButton(panel.contentView, 1);
        assert(scripted && unscripted);
        assert([scripted.title isEqual:@"1  汉语"] && [scripted.annotation isEqual:@"(aB)"] && [scripted.translation isEqual:@"Chinese"]);
        assert([scripted.scriptAnnotation isEqual:@"常用词"] && unscripted.scriptAnnotation.length == 0);
        assert(scripted.itemLayout.translation.below && scripted.translationBelow);
        assert(scripted.itemLayout.translation.height > unscripted.itemLayout.translation.height + glossLine * 0.5);
        assert([scripted.toolTip isEqual:@"汉语(aB)\nChinese\n常用词"] && [unscripted.toolTip isEqual:@"韩语(aB)\nKorean"]);
        assert([scripted.accessibilityLabel isEqual:@"1  汉语(aB)，插件注释：常用词"]);
        assert([unscripted.accessibilityLabel isEqual:@"2  韩语(aB)"]);
        bitmap = [scripted bitmapImageRepForCachingDisplayInRect:scripted.bounds];
        [scripted cacheDisplayInRect:scripted.bounds toBitmapImageRep:bitmap];

        // 没有译文时插件注释自己占释义行；插件写进去的换行和零宽字符画不出第二行。
        appearance.candidateTranslations = NO;
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"script_annotation": @"第一行\n\u200B第二行", @"highlighted": @YES}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *alone = CandidateButton(panel.contentView, 0);
        assert([alone.scriptAnnotation isEqual:@"第一行 第二行"] && alone.translation.length == 0);
        assert(alone.itemLayout.translation.width > 0 && alone.itemLayout.translation.below);
        assert(alone.itemLayout.translation.height < glossLine * 1.5);
        assert([alone.toolTip isEqual:@"汉语\n第一行 第二行"]);

        // 只有注释变了的视图不走只重画译文的捷径，按钮换成新的注释。
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"script_annotation": @"别的注释", @"highlighted": @YES}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        assert([CandidateButton(panel.contentView, 0).scriptAnnotation isEqual:@"别的注释"]);

        // 竖排：注释放得下时和释义一样留在候选那一行；比整列还长时在列内折行，不撑出卡片。
        appearance.vertical = YES;
        [controller renderCandidates];
        MSIMECandidateButton *verticalScripted = CandidateButton(panel.contentView, 0);
        assert(!verticalScripted.itemLayout.translation.below && !verticalScripted.translationBelow);
        NSString *wideAnnotation = [@"" stringByPaddingToLength:32 withString:@"﷽" startingAtIndex:0];
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"script_annotation": wideAnnotation, @"highlighted": @YES}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *wideScripted = CandidateButton(panel.contentView, 0);
        assert([wideScripted.scriptAnnotation isEqual:wideAnnotation]);
        assert(panel.frame.size.width <= halfScreen + 0.5 && NSMaxX(wideScripted.frame) <= panel.frame.size.width + 0.5);
        assert(wideScripted.itemLayout.translation.x + wideScripted.itemLayout.translation.width <= NSWidth(wideScripted.frame) + 0.5);
        assert(NSHeight(wideScripted.frame) >= wideScripted.itemLayout.height - 0.5);
        bitmap = [wideScripted bitmapImageRepForCachingDisplayInRect:wideScripted.bounds];
        [wideScripted cacheDisplayInRect:wideScripted.bounds toBitmapImageRep:bitmap];

        // 状态开关不改变拼音预编辑，关闭时状态文字和分隔线一起撤下。
        const BOOL previousPreedit = appearance.showsCandidatePreedit;
        appearance.showsCandidatePreedit = YES;
        appearance.vertical = NO;
        [controller setValue:@{@"focused": @YES, @"editing_text": @"ceshi", @"page": @0, @"page_count": @1,
            @"candidates": @[@{@"text": @"合成候选用于验证状态开关保留拼音预编辑", @"highlighted": @YES}]} forKey:@"view"];
        appearance.showsCandidateStatus = YES;
        [controller renderCandidates];
        assert(HeaderChild(panel.contentView, @"candidate-preedit"));
        assert(HeaderChild(panel.contentView, @"candidate-status"));
        appearance.showsCandidateStatus = NO;
        [controller renderCandidates];
        assert(HeaderChild(panel.contentView, @"candidate-preedit"));
        assert(!HeaderChild(panel.contentView, @"candidate-status"));
        assert(!HeaderChild(panel.contentView, @"candidate-status-divider"));
        appearance.showsCandidateStatus = YES;
        appearance.showsCandidatePreedit = previousPreedit;

        // 读屏：高亮的那一行报告自己被选中，别的行和翻页按钮都不报告；每画一页就请求朗读高亮的那一行。云候选和 AI 占位行不创建按钮，也不参与读屏或编号。
        appearance.vertical = NO;
        NSDictionary *accessibleBase = @{@"focused": @YES, @"editing_text": @"ceshi", @"page": @0, @"page_count": @2};
        NSArray<NSDictionary *> *(^accessibleRows)(NSUInteger, NSString *) = ^NSArray<NSDictionary *> *(NSUInteger highlighted, NSString *placeholder) {
            NSArray<NSDictionary *> *rows = @[@{@"text": @"测试"}, @{@"text": @"侧室"}, @{@"text": placeholder, @"source": @12}];
            NSMutableArray<NSDictionary *> *marked = [NSMutableArray array];
            for (NSUInteger index = 0; index < rows.count; ++index) {
                NSMutableDictionary *row = [rows[index] mutableCopy];
                row[@"highlighted"] = @(index == highlighted);
                [marked addObject:row];
            }
            return marked;
        };
        void (^renderAccessible)(NSUInteger, NSString *) = ^(NSUInteger highlighted, NSString *placeholder) {
            NSMutableDictionary *view = [accessibleBase mutableCopy];
            view[@"candidates"] = accessibleRows(highlighted, placeholder);
            [controller setValue:[view copy] forKey:@"view"];
            [controller renderCandidates];
            for (NSInteger tag = 0; tag < 2; ++tag)
                assert(CandidateButton(panel.contentView, tag).isAccessibilitySelected == (tag == (NSInteger)highlighted));
            for (NSInteger tag : {-1, -2})
                if (MSIMECandidateButton *arrow = CandidateButton(panel.contentView, tag)) assert(!arrow.isAccessibilitySelected);
        };
        renderAccessible(0, @"…");
        assert([[controller valueForKey:@"announcedCandidate"] isEqual:@"1  测试"]);
        assert(CandidateButton(panel.contentView, 2) == nil);
        renderAccessible(1, @"…");
        assert([[controller valueForKey:@"announcedCandidate"] isEqual:@"2  侧室"]);
        renderAccessible(2, @"…");
        assert([[controller valueForKey:@"announcedCandidate"] length] == 0);
        renderAccessible(2, @"—");
        assert([[controller valueForKey:@"announcedCandidate"] length] == 0);
        NSMutableDictionary *pendingView = [accessibleBase mutableCopy];
        pendingView[@"candidates"] = @[
            @{@"text": @"测试", @"highlighted": @YES},
            @{@"text": @"☁", @"source": @15},
            @{@"text": @"侧室", @"id": @{@"session": @1, @"generation": @7, @"index": @2}}
        ];
        [controller setValue:pendingView forKey:@"view"];
        [controller renderCandidates];
        assert([CandidateButton(panel.contentView, 1).title isEqual:@"2  侧室"]);
        assert([CandidateButton(panel.contentView, 1).candidateID[@"index"] isEqual:@2]);
        assert(CandidateButton(panel.contentView, 2) == nil);
        pendingView[@"candidates"] = @[@{@"text": @"☁", @"source": @15}];
        [controller setValue:pendingView forKey:@"view"];
        [controller renderCandidates];
        assert(!panel.isVisible);
        renderAccessible(0, @"…");
        // 没有画皮肤包时原生控制器立即显示与隐藏：没有进场、选中装饰和退场。皮肤包写了动效时的行为由 candidate-panel 测试覆盖。
        assert(panel.alphaValue == 1.0);
        assert(panel.animationBehavior == NSWindowAnimationBehaviorNone);
        assert(CATransform3DIsIdentity(panel.contentView.layer.transform));
        assert([panel.contentView.layer animationForKey:@"msime-candidate-enter-opacity"] == nil);
        for (NSView *row in MSIMECandidateControlsView(panel.contentView).subviews)
            assert([row.layer animationForKey:@"msime-candidate-selection"] == nil);
        [controller hideCandidatePanel:"empty"];
        assert(!panel.isVisible);
        [controller renderCandidates];
        assert(panel.alphaValue == 1.0);
        // 使用有效的合成候选身份，验证按需创建、帧内复用和旧按钮拒绝菜单。
        NSDictionary *menuView = @{@"session": @1, @"generation": @7, @"focused": @YES,
                                  @"editing_text": @"d", @"page": @0, @"page_count": @1,
                                  @"candidates": @[@{@"id": @{@"session": @1, @"generation": @7, @"index": @0},
                                                     @"text": @"合成", @"highlighted": @YES}]};
        [controller setValue:menuView forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *menuRow = CandidateButton(panel.contentView, 0);
        assert(menuRow.menu == nil);
        NSEvent *rightClick = [NSEvent mouseEventWithType:NSEventTypeRightMouseDown location:NSZeroPoint modifierFlags:0
                                               timestamp:0 windowNumber:0 context:nil eventNumber:1 clickCount:1 pressure:1];
        NSMenu *menu = [menuRow menuForEvent:rightClick];
        assert(menu && menu.numberOfItems == 3);
        assert([menuRow menuForEvent:rightClick] == menu);
        [controller renderCandidates];
        assert([menuRow menuForEvent:rightClick] == nil);
        assert(CandidateButton(panel.contentView, 0).menu == nil);

        // 可选的合成显隐样本，仅输出耗时与菜单构建次数，不输出输入内容。
        if (getenv("MSIME_CANDIDATE_BENCHMARK")) {
            NSMutableArray *sample = [NSMutableArray array];
            for (NSUInteger slot = 0; slot < 9; ++slot)
                [sample addObject:@{@"id": @{@"session": @1, @"generation": @7, @"index": @(slot)},
                                    @"text": @"合成", @"highlighted": @(slot == 0)}];
            NSDictionary *shown = @{@"session": @1, @"generation": @7, @"focused": @YES,
                                    @"editing_text": @"d", @"candidates": sample, @"page": @0, @"page_count": @1};
            NSDictionary *hidden = @{@"candidates": @[]};
            std::vector<double> timings;
            for (NSUInteger iteration = 0; iteration < 60; ++iteration) {
                @autoreleasepool {
                    const uint64_t started = clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
                    [controller setValue:shown forKey:@"view"];
                    [controller renderCandidates];
                    [controller setValue:hidden forKey:@"view"];
                    [controller renderCandidates];
                    timings.push_back((clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - started) / 1e6);
                }
            }
            std::sort(timings.begin(), timings.end());
            printf("candidate show/hide: median=%.3fms p95=%.3fms\n", timings[30], timings[57]);
        }
        // 首选与悬停行按相同内边距推导圆角；零内边距在各档缩放下都不受边框宽度限制。
        appearance.candidateCornerRadius = @32;
        appearance.candidatePadding = @0;
        for (NSNumber *scale in @[@50, @100, @150, @200]) {
            appearance.candidateScalePercent = scale.integerValue;
            renderAccessible(0, @"…");
            MSIMECandidateButton *selected = CandidateButton(panel.contentView, 0);
            MSIMECandidateButton *hovered = CandidateButton(panel.contentView, 1);
            hovered.candidateHovered = YES;
            assert(selected.cornerRadius == 32.0 * scale.doubleValue / 100.0);
            assert(hovered.cornerRadius == selected.cornerRadius);
            assert(std::abs(NSMinX(selected.frame)) < 0.001);
        }
        // 前一组覆盖到 200% 缩放，首行布局断言使用 100% 的坐标。
        appearance.candidateScalePercent = 100;
        // 直接驱动实际展开渲染路径，避免其它快捷键测试的失败遮住布局回归。
        NSMutableArray *expanded = [NSMutableArray array];
        for (NSUInteger slot = 0; slot < 9; ++slot)
            [expanded addObject:@{@"text": @"合成", @"highlighted": @(slot == 0)}];
        [controller setValue:expanded forKey:@"expandedCandidates"];
        [controller setValue:@0 forKey:@"expandedCursor"];
        appearance.vertical = NO;
        appearance.showsCandidatePageNumber = NO;
        appearance.showsAppLogo = NO;
        appearance.showsCandidatePreedit = NO;
        assert([controller renderExpandedCandidates]);
        assert(((MSIMECandidateChromeView *)panel.contentView).liquidGlassEnabled);
        for (NSView *row in MSIMECandidateControlsView(panel.contentView).subviews) {
            assert(![row.identifier isEqual:@"candidate-page-indicator"]);
            assert(![row.identifier isEqual:@"candidate-collapse"]);
        }
        const CGFloat inset = msime::mac::CandidateInset([appearance candidateWindowSkinForDark:NO].tokens);
        assert(std::abs(NSMaxY(CandidateButton(panel.contentView, 0).frame) - (NSHeight(panel.contentView.bounds) - inset)) < .01);
        appearance.showsCandidatePreedit = YES;
        assert([controller renderExpandedCandidates]);
        assert(NSMaxY(CandidateButton(panel.contentView, 0).frame) < NSHeight(panel.contentView.bounds) - inset);
        appearance.showsCandidatePageNumber = YES;
        assert([controller renderExpandedCandidates]);
        assert([CandidateButton(panel.contentView, -3).identifier isEqual:@"candidate-collapse"]);
        TestPronunciationLine(controller, panel, appearance);
        TestVerticalReservedGlossRow(controller, panel, appearance);
        @autoreleasepool { TestSelectedGlossColour(client); }
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    return 0;
}
