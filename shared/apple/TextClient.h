#import <Foundation/Foundation.h>

@protocol MSIMETextClient <NSObject>
- (void)insertText:(id)text replacementRange:(NSRange)range;
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement;
@optional
// NSTextInputClient-compatible context queries. Hosts that cannot expose
// document context may omit these; callers must treat the result as unknown.
- (NSRange)selectedRange;
- (NSAttributedString *)attributedSubstringFromRange:(NSRange)range;
@end

typedef NS_ENUM(NSInteger, MSIMEInlinePreeditStyle) {
    MSIMEInlinePreeditStyleRaw,
    MSIMEInlinePreeditStylePinyin,
    MSIMEInlinePreeditStyleEmpty,
};

void MSIMEApplyTransition(NSDictionary *transition, id<MSIMETextClient> client);
/// Applies a transition using the shared inline preedit display preference.
/// The legacy entry point above remains the pinyin-display default for callers
/// that do not consume shared preferences yet.
void MSIMEApplyTransitionWithPreeditStyle(NSDictionary *transition, id<MSIMETextClient> client,
                                          MSIMEInlinePreeditStyle style);
/// 同一提交路径，额外携带宿主尚未交付的闭合标点。
///
/// IMK 没有直接设置插入点的接口，且并非所有客户端都支持绝对文档范围。
/// 成对标点保留既有兼容路径：开符号正常提交，闭符号作为标记尾段跟随组字，
/// 用户先看到 `（|）`，再看到 `（你好|）`，组字结束才提交尾段。
/// nil 表示没有待闭合符，等同普通提交。
void MSIMEApplyTransitionWithPendingClosing(NSDictionary *transition, id<MSIMETextClient> client,
                                            MSIMEInlinePreeditStyle style, NSString *closing);
/// The same, for a host that knows whether the client still holds marked text this host wrote.
///
/// Every call into an IMK client is a synchronous round trip into another process, and a focus change makes one while the client is itself blocked waiting for the input method to activate: the two stall each other until IMK's XPC timeout, which is the 1-3 s hitch Chrome shows on every tab or window switch. Clearing marked text that was never set is the one write that changes nothing, so with nothing to commit, nothing to mark and `*clientHasMarkedText` NO the client is not called at all. A commit still sends the clear after it, exactly as above. `*clientHasMarkedText` is updated to what the client holds afterwards; NULL means unknown and always sends.
void MSIMEApplyTransitionTrackingMarkedText(NSDictionary *transition, id<MSIMETextClient> client,
                                            MSIMEInlinePreeditStyle style, NSString *closing,
                                            BOOL *clientHasMarkedText);
/// 客户端同步写入后先运行 observer；macOS 随后按已核对的 commit_cursor_suffix 定位，再写新预编辑。
/// observer 接收含待闭合符的实际上屏文本；每次同步调用后均检查旧转换是否仍有效。
/// canPosition 可额外阻止透传键之后的旧定位；nil 只使用转换有效性保护。
void MSIMEApplyTransitionWithCommitObserver(NSDictionary *transition, id<MSIMETextClient> client,
                                            MSIMEInlinePreeditStyle style, NSString *closing,
                                            BOOL *clientHasMarkedText, BOOL (^isCurrent)(void),
                                            void (^didCommit)(NSString *text), BOOL (^canPosition)(void));
/// 同步客户端调用可能处理较新的转换；宿主判定旧转换过期后，不再写它的预编辑。
void MSIMEApplyTransitionWithWriteGuard(NSDictionary *transition, id<MSIMETextClient> client,
                                       MSIMEInlinePreeditStyle style, NSString *closing,
                                       BOOL *clientHasMarkedText, BOOL (^isCurrent)(void));
// UTF-16 display offset shared by marked text and the candidate preedit row.
NSUInteger MSIMEPreeditCaretPosition(NSString *editing, NSString *preedit, id position);

// Returns the single UTF-16 character immediately following the selection,
// or nil when the host cannot safely expose document context.
NSString * MSIMETextClientFollowingCharacter(id<MSIMETextClient> client);
/// Returns the Unicode scalar immediately preceding the selection, or zero when
/// the host cannot safely expose document context.
uint32_t MSIMETextClientPrecedingUnicodeScalar(id<MSIMETextClient> client);

/// 校验刚上屏文本仍紧邻折叠光标，返回其文档范围与左侧标量；未知、过长或过期时返回 NSNotFound。
NSRange MSIMETextClientCommittedRange(id<MSIMETextClient> client, NSString *text,
                                      uint32_t *preceding, BOOL (^isCurrent)(void));
