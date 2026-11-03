#pragma once
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>

@protocol MSIMEGlobalVoiceTarget <NSObject>
- (pid_t)globalVoiceTargetPID;
- (BOOL)globalVoiceBusy;
- (BOOL)globalVoiceProcessing;
- (void)startGlobalVoiceHolding:(BOOL)holding;
- (void)stopGlobalVoice;
- (void)cancelGlobalVoice;
- (void)endGlobalVoice;
- (void)lockGlobalVoice;
- (void)detachGlobalVoice;
@end

// 单进程桥接只在主线程访问，不接受外部录音命令。测试可替换回调而不用加载事件 tap。
struct MSIMEGlobalVoiceBridge {
    bool keyboard = false;
    __weak id<MSIMEGlobalVoiceTarget> owner = nil;
    void (^recover)(NSString *) = nil;
    id focusIdentity = nil;
    void (^selectedMode)(NSString *) = nil;
    BOOL (^toggleHandled)(NSEvent *) = nil;
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
@end

// 只处理明确的引导参数，其余参数继续走 IMK 的正常启动路径。
BOOL MSIMEHandleGlobalVoiceBootstrap(int argc, const char *argv[], int *exitCode);

// 绑定焦点元素而非仅绑定进程；同一编辑器里换一个输入框也不沿用插入权限。
inline id MSIMEGlobalVoiceFocusIdentity() {
    AXUIElementRef system = AXUIElementCreateSystemWide();
    AXUIElementSetMessagingTimeout(system, 0.05f);
    CFTypeRef element = nullptr;
    AXError error = AXUIElementCopyAttributeValue(system, kAXFocusedUIElementAttribute, &element);
    CFRelease(system);
    if (error != kAXErrorSuccess || !element || CFGetTypeID(element) != AXUIElementGetTypeID()) {
        if (element) CFRelease(element);
        return nil;
    }
    AXUIElementSetMessagingTimeout((AXUIElementRef)element, 0.05f);
    CFTypeRef subrole = nullptr;
    AXUIElementCopyAttributeValue((AXUIElementRef)element, kAXSubroleAttribute, &subrole);
    BOOL secure = subrole && CFEqual(subrole, CFSTR("AXSecureTextField"));
    if (subrole) CFRelease(subrole);
    if (secure) { CFRelease(element); return nil; }
    return CFBridgingRelease(element);
}
