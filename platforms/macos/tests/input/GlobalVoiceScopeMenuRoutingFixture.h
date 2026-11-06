#pragma once
#import <objc/runtime.h>

// 真实 IMK 分派必须传入字典；用无目录的合成菜单项验证入口，不触碰实际偏好。
static void TestGlobalVoiceScopeIMKRouting(MSIMEInputController *controller) {
    SEL action = NSSelectorFromString(@"selectGlobalVoiceScope:");
    Method method = class_getInstanceMethod(MSIMEInputController.class, action);
    IMP original = method_getImplementation(method);
    __block NSUInteger calls = 0;
    IMP spy = imp_implementationWithBlock(^(id target, id sender) {
        ++calls;
        assert([sender isKindOfClass:NSDictionary.class]);
        ((void (*)(id, SEL, id))original)(target, action, sender);
    });
    method_setImplementation(method, spy);
    @try {
        NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:@"合成范围选项" action:action keyEquivalent:@""];
        item.tag = 1;
        [controller doCommandBySelector:action commandDictionary:@{kIMKCommandMenuItemName:item}];
    } @finally {
        method_setImplementation(method, original);
        imp_removeBlock(spy);
    }
    assert(calls == 1);
}
