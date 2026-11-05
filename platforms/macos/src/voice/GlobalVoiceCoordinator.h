#pragma once
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>

typedef NS_ENUM(NSInteger, MSIMEGlobalVoiceStartResult) {
    MSIMEGlobalVoiceStartRetry,
    MSIMEGlobalVoiceStartAccepted,
    MSIMEGlobalVoiceStartFailed,
};

// 暂未取得当前 IMK 客户端时继续等待；成功交付只记一次。
typedef NS_ENUM(NSInteger, MSIMEGlobalVoiceDeliveryResult) {
    MSIMEGlobalVoiceDeliveryRetry,
    MSIMEGlobalVoiceDeliveryAccepted,
};

@protocol MSIMEGlobalVoiceDeliveryTarget <NSObject>
- (pid_t)globalVoiceTargetPID;
- (BOOL)globalVoiceBusy;
@optional
// 全局入口可以停止菜单或工具栏发起的局部录音，不接管其提交目标。
- (BOOL)toggleLocalVoiceIfBusy;
- (BOOL)prepareGlobalVoiceTextForCommit:(id)token;
- (MSIMEGlobalVoiceDeliveryResult)commitGlobalVoiceText:(NSString *)text token:(id)token;
@end

@protocol MSIMEGlobalVoiceTarget <MSIMEGlobalVoiceDeliveryTarget>
- (BOOL)globalVoiceProcessing;
- (MSIMEGlobalVoiceStartResult)startGlobalVoiceHolding:(BOOL)holding;
- (void)stopGlobalVoice;
- (void)cancelGlobalVoice;
- (void)endGlobalVoice;
- (void)lockGlobalVoice;
- (void)detachGlobalVoice;
@end

// 单进程桥接只在主线程访问，不接受外部录音命令。测试可替换回调而不用加载事件 tap。
struct MSIMEGlobalVoiceBridge {
    bool keyboard = false;
    bool delivering = false;
    __weak id<MSIMEGlobalVoiceTarget> owner = nil;
    void (^deliver)(NSString *, NSString *, void (^)(void)) = nil;
    void (^didCommit)(NSString *) = nil;
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
                deliveryTarget:(id<MSIMEGlobalVoiceDeliveryTarget> (^)(void))deliveryTarget;
@end

// 只处理明确的引导参数，其余参数继续走 IMK 的正常启动路径。
BOOL MSIMEHandleGlobalVoiceBootstrap(int argc, const char *argv[], int *exitCode);

// 仅供设置窗口的录键按钮识别焦点，不用于录音或结果交付。
template <typename CopyAttribute, typename ElementPID>
inline id MSIMEGlobalVoiceFocusIdentityFor(pid_t expectedPID, AXUIElementRef system, AXUIElementRef application,
                                         CopyAttribute copy, ElementPID elementPID) {
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
    (void)subroleError;
    if (secure) { CFRelease(element); return nil; }
    return CFBridgingRelease(element);
}

// Web 设置窗口可从应用级 AX 回退识别录键按钮，不读取输入框内容。
inline id MSIMEGlobalVoiceFocusIdentity() {
    NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (!front || front.processIdentifier <= 0) return nil;
    AXUIElementRef system = AXUIElementCreateSystemWide();
    AXUIElementRef application = AXUIElementCreateApplication(front.processIdentifier);
    AXUIElementSetMessagingTimeout(system, 0.05f);
    AXUIElementSetMessagingTimeout(application, 0.15f);
    id focus = MSIMEGlobalVoiceFocusIdentityFor(front.processIdentifier, system, application,
        AXUIElementCopyAttributeValue, AXUIElementGetPid);
    CFRelease(application);
    CFRelease(system);
    if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != front.processIdentifier) return nil;
    return focus;
}
