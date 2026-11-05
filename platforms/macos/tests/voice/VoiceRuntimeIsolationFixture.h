#pragma once
#import <objc/runtime.h>
#include <cassert>

// 用假的系统定位器抓住夹具意外回退；不读取本机用户配置，也不启动会话或偏好轮询。
static inline void MSIMEAssertVoiceRuntimeIsolation(Class fixture) {
    Method method = class_getInstanceMethod(MSIMEInputController.class, @selector(runtimeOptions));
    IMP original = method_getImplementation(method);
    NSString *trap = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    IMP fallback = imp_implementationWithBlock(^NSDictionary *(id receiver) {
        (void)receiver;
        return @{@"preferences_directory":trap};
    });
    method_setImplementation(method, fallback);
    MSIMEInputController *controller = [fixture alloc];
    [controller startPreferencesMonitoring];
    const BOOL isolated = [controller valueForKey:@"preferencesDirectory"] == nil;
    method_setImplementation(method, original);
    imp_removeBlock(fallback);
    assert(isolated && "合成语音夹具不得采用真实宿主的运行时定位器");
}
