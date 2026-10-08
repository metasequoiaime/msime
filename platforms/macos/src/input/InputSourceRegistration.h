#pragma once
#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <Foundation/Foundation.h>

using MSIMEInputSourceRegistrar = OSStatus (*)(CFURLRef);
using MSIMEInputSourceLister = CFArrayRef (*)(CFDictionaryRef, Boolean);
using MSIMEInputSourcePropertyGetter = void *(*)(TISInputSourceRef, CFStringRef);
using MSIMEInputSourceEnabler = OSStatus (*)(TISInputSourceRef);
using MSIMEInputSourceCopier = TISInputSourceRef (*)(void);

@interface MSIMEInputSourceMonitor : NSObject
- (instancetype)initWithCenter:(NSNotificationCenter *)center bundleIdentifier:(NSString *)identifier
                    copySource:(MSIMEInputSourceCopier)copier propertyGetter:(MSIMEInputSourcePropertyGetter)getter
                    switchedAway:(void (^)(void))action;
- (void)stop;
@end

bool MSIMEShouldRegisterInputSource(int argc, const char *argv[]);
OSStatus MSIMERegisterInputSource(NSURL *bundleURL, MSIMEInputSourceRegistrar registrar);
OSStatus MSIMERegisterAndEnableInputSources(NSURL *bundleURL, NSString *bundleIdentifier,
                                            MSIMEInputSourceRegistrar registrar,
                                            MSIMEInputSourceLister lister,
                                            MSIMEInputSourcePropertyGetter propertyGetter,
                                            MSIMEInputSourceEnabler enabler);
/// 把 bundle 里 `offered` 还没列出的输入模式各启用一次，返回加上这些模式后的 `offered`，由调用方持久化。经更新只替换 bundle 时不会重新登记，新版本加的模式会一直关着，除非用户自己去系统设置的「添加」对话框里找——而那里按每个模式声明的语言分组，新模式不一定和其它模式排在一起。已经记录的模式不再动，用户移除的模式保持移除。`offered` 为 nil 时，从此前每次安装都会启用的那几个模式开始。按需模式（MSIMEOptInInputModeIDs）只记录不启用，已经启用的也不关掉：那多半是用户刚在系统设置里加的，见实现里的说明。
NSArray<NSString *> *MSIMEEnableNewInputModes(NSString *bundleIdentifier, NSArray<NSString *> *offered,
                                              MSIMEInputSourceLister lister,
                                              MSIMEInputSourcePropertyGetter propertyGetter,
                                              MSIMEInputSourceEnabler enabler);
/// 启用这个标识符对应的已安装输入源，不论它当前是否启用。用户选中粤拼、注音、越南文、藏文或笔画时就靠它打开对应的按需模式，用户不必去系统设置「添加」对话框的「粤语」「繁体中文」「越南语」「藏语」或「简体中文」下面找。
OSStatus MSIMEEnableInputMode(NSString *identifier, MSIMEInputSourceLister lister, MSIMEInputSourceEnabler enabler);
/// Whether the input source with this identifier is enabled, so the system can select it.
BOOL MSIMEInputSourceIsEnabled(NSString *identifier);
/// Starts a separate non-activating helper instance of the current input method
/// to re-register its source. Completion is always delivered on the main thread.
void MSIMELaunchInputSourceReregistration(NSURL *bundleURL, NSWorkspace *workspace,
                                          void (^completion)(BOOL launched));
