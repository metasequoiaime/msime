#pragma once
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>

typedef NS_ENUM(NSInteger, MSIMEGlobalVoiceStartResult) {
    MSIMEGlobalVoiceStartRetry,
    MSIMEGlobalVoiceStartAccepted,
    MSIMEGlobalVoiceStartFailed,
};

// 暂未取得客户端可以继续等待；校验期间输入改变则本轮只允许复制，不再重试插入。
typedef NS_ENUM(NSInteger, MSIMEGlobalVoiceDeliveryResult) {
    MSIMEGlobalVoiceDeliveryRetry,
    MSIMEGlobalVoiceDeliveryAccepted,
    MSIMEGlobalVoiceDeliveryChanged,
};

@protocol MSIMEGlobalVoiceTarget <NSObject>
- (pid_t)globalVoiceTargetPID;
- (BOOL)globalVoiceBusy;
- (BOOL)globalVoiceProcessing;
- (MSIMEGlobalVoiceStartResult)startGlobalVoiceHolding:(BOOL)holding;
- (void)stopGlobalVoice;
- (void)cancelGlobalVoice;
- (void)endGlobalVoice;
- (void)lockGlobalVoice;
- (void)detachGlobalVoice;
@optional
// 全局入口可以停止当前控制器由菜单或工具栏发起的局部录音，不接管其提交目标。
- (BOOL)toggleLocalVoiceIfBusy;
// 外部提交前先保留原字段的组字，并重新验证客户端与插入许可。
- (MSIMEGlobalVoiceDeliveryResult)prepareGlobalVoiceDestinationForPID:(pid_t)pid focus:(id)focus token:(id)token;
// 只允许当前真实 IMK 客户端在原字段接收这一轮的最终文本。
- (MSIMEGlobalVoiceDeliveryResult)commitGlobalVoiceText:(NSString *)text pid:(pid_t)pid focus:(id)focus token:(id)token;
@end

// 单进程桥接只在主线程访问，不接受外部录音命令。测试可替换回调而不用加载事件 tap。
struct MSIMEGlobalVoiceBridge {
    bool keyboard = false;
    bool delivering = false;
    __weak id<MSIMEGlobalVoiceTarget> owner = nil;
    void (^recover)(NSString *) = nil;
    void (^deliver)(NSString *, NSString *, void (^)(void)) = nil;
    void (^didCommit)(NSString *) = nil;
    id focusIdentity = nil;
    id sessionToken = nil;
    void (^selectedMode)(NSString *) = nil;
    BOOL (^toggleHandled)(NSEvent *) = nil;
    BOOL (^toggleCurrent)(void) = nil;
};
inline MSIMEGlobalVoiceBridge &MSIMEGlobalVoiceHost() {
    static MSIMEGlobalVoiceBridge bridge;
    return bridge;
}

inline BOOL MSIMEGlobalVoiceOwnsToggle(NSEvent *event) {
    auto &host = MSIMEGlobalVoiceHost();
    return host.keyboard && (!host.toggleHandled || host.toggleHandled(event));
}

@interface MSIMEGlobalVoiceCoordinator : NSObject
- (instancetype)initWithTarget:(id<MSIMEGlobalVoiceTarget> (^)(void))target;
- (instancetype)initWithTarget:(id<MSIMEGlobalVoiceTarget> (^)(void))target
                deliveryTarget:(id<MSIMEGlobalVoiceTarget> (^)(void))deliveryTarget;
@end

// 只处理明确的引导参数，其余参数继续走 IMK 的正常启动路径。
BOOL MSIMEHandleGlobalVoiceBootstrap(int argc, const char *argv[], int *exitCode);

// 系统级焦点在部分 Web 编辑器里不可用；应用级查询仍必须绑定同一个前台进程。
template <typename CopyAttribute, typename ElementPID>
inline id MSIMEGlobalVoiceFocusIdentityFor(pid_t expectedPID, AXUIElementRef system, AXUIElementRef application,
                                         CopyAttribute copy, ElementPID elementPID, BOOL requireEditable = NO) {
    CFTypeRef element = nullptr;
    AXError error = copy(system, kAXFocusedUIElementAttribute, &element);
    if (error != kAXErrorSuccess || !element) {
        if (element) CFRelease(element);
        element = nullptr;
        error = copy(application, kAXFocusedUIElementAttribute, &element);
    }
    if (error != kAXErrorSuccess || !element || CFGetTypeID(element) != AXUIElementGetTypeID()) {
        if (element) CFRelease(element);
        return nil;
    }
    pid_t pid = 0;
    if (elementPID((AXUIElementRef)element, &pid) != kAXErrorSuccess || pid != expectedPID) {
        CFRelease(element);
        return nil;
    }
    AXUIElementSetMessagingTimeout((AXUIElementRef)element, 0.05f);
    CFTypeRef subrole = nullptr;
    const AXError subroleError = copy((AXUIElementRef)element, kAXSubroleAttribute, &subrole);
    const BOOL secure = subrole && CFEqual(subrole, CFSTR("AXSecureTextField"));
    if (subrole) CFRelease(subrole);
    if (secure || (requireEditable && subroleError != kAXErrorSuccess &&
        subroleError != kAXErrorAttributeUnsupported && subroleError != kAXErrorNoValue)) { CFRelease(element); return nil; }
    if (requireEditable) {
        CFTypeRef role = nullptr, enabled = nullptr, editable = nullptr;
        copy((AXUIElementRef)element, kAXRoleAttribute, &role);
        const AXError enabledError = copy((AXUIElementRef)element, kAXEnabledAttribute, &enabled);
        const AXError editableError = copy((AXUIElementRef)element, CFSTR("AXEditable"), &editable);
        const BOOL text = role && (CFEqual(role, kAXTextAreaRole) || CFEqual(role, kAXTextFieldRole) || CFEqual(role, kAXComboBoxRole));
        const BOOL failed = enabledError == kAXErrorCannotComplete || editableError == kAXErrorCannotComplete;
        const BOOL denied = failed || (enabled && CFEqual(enabled, kCFBooleanFalse)) || (editable && CFEqual(editable, kCFBooleanFalse));
        if (role) CFRelease(role); if (enabled) CFRelease(enabled); if (editable) CFRelease(editable);
        if (!text || denied) { CFRelease(element); return nil; }
    }
    return CFBridgingRelease(element);
}

// 绑定焦点元素而非仅绑定进程；同一编辑器里换一个输入框也不沿用插入权限。
inline id MSIMEGlobalVoiceFocusIdentity(BOOL requireEditable = NO) {
    NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (!front || front.processIdentifier <= 0) return nil;
    AXUIElementRef system = AXUIElementCreateSystemWide();
    AXUIElementRef application = AXUIElementCreateApplication(front.processIdentifier);
    AXUIElementSetMessagingTimeout(system, 0.05f);
    AXUIElementSetMessagingTimeout(application, 0.15f);
    id focus = MSIMEGlobalVoiceFocusIdentityFor(front.processIdentifier, system, application,
        AXUIElementCopyAttributeValue, AXUIElementGetPid, requireEditable);
    CFRelease(application);
    CFRelease(system);
    if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != front.processIdentifier) return nil;
    return focus;
}
