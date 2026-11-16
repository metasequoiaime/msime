#pragma once

#import <Carbon/Carbon.h>
#import <InputMethodKit/InputMethodKit.h>

// 布局覆盖属于客户端的 TSM 文档，必须经 IMKTextInput 送入客户端，不能只在输入法进程里设置。
static inline void MSIMEOverrideClientKeyboardLayout(id client, NSString *identifier) {
    if (![identifier isKindOfClass:NSString.class] || identifier.length == 0 ||
        ![client respondsToSelector:@selector(overrideKeyboardWithKeyboardNamed:)]) return;
    [client overrideKeyboardWithKeyboardNamed:identifier];
}

static inline void MSIMEPrepareClientKeyboardLayout(id client,
    OSStatus (*setOverride)(TISInputSourceRef) = TISSetInputMethodKeyboardLayoutOverride) {
    if (![client respondsToSelector:@selector(overrideKeyboardWithKeyboardNamed:)]) return;
    TISInputSourceRef source = TISCopyCurrentASCIICapableKeyboardLayoutInputSource();
    if (!source) return;
    // Qt 直接读取输入模式的 TIS 覆盖；只覆盖客户端的 TSM 文档不会填充这个查询结果。
    setOverride(source);
    // 沿用用户的 ASCII 布局，避免把 Dvorak、Colemak 等布局强制改成美式键位。
    CFTypeRef identifier = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
    if (identifier && CFGetTypeID(identifier) == CFStringGetTypeID()) {
        MSIMEOverrideClientKeyboardLayout(client, (__bridge NSString *)identifier);
    }
    CFRelease(source);
}
