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
/// 本登录会话的输入源注册表里有没有这个 bundle 的可启用输入源。查法与 MSIMERegisterAndEnableInputSources 登记后的那次查找相同，Missing 正对应它返回的 fnfErr。
typedef NS_ENUM(NSInteger, MSIMEInputSourceRegistryState) {
    /// 没有 bundle 标识或查询函数，无从判断。
    MSIMEInputSourceRegistryStateUnknown,
    MSIMEInputSourceRegistryStateListed,
    MSIMEInputSourceRegistryStateMissing,
};
/// `--input-source-registered`：只读地查注册表并以退出码报告，不登记、不启用任何东西。设置应用据此分辨「装好了但这次登录看不到」。
bool MSIMEShouldReportInputSourceRegistration(int argc, const char *argv[]);
MSIMEInputSourceRegistryState MSIMEInputSourceRegistryStateFor(NSString *bundleIdentifier, MSIMEInputSourceLister lister);
/// `--input-source-registered` 的退出码：Listed 为 0，Missing 为 3，Unknown 为 1。apps/desktop/src-tauri 的 macos_input_source.rs 按同样的值解读，两边要一起改。
int MSIMEInputSourceRegistryExitCode(MSIMEInputSourceRegistryState state);
/// `--enabled-input-modes`：只读地列出本 bundle 已启用的输入模式标识符，每行一个，退出码 0；无从判断时退出码 1。设置应用的「菜单栏入口」据此说还差哪几个模式：系统设置的偏好列表只记用户亲手加的模式，随输入法一起默认启用的模式（中、双、五、日、韩、英）不在里面，单看它会把这些也报成没加。
bool MSIMEShouldReportEnabledInputModes(int argc, const char *argv[]);
/// 本 bundle 已启用的输入模式：TIS 列出的全部已装输入源里，标识符以 bundle 标识加「.」开头、`kTISPropertyInputSourceIsEnabled` 为真的那些；没有 bundle 标识、查询函数或列表时为 nil。
NSArray<NSString *> *MSIMEEnabledInputModeIDs(NSString *bundleIdentifier, MSIMEInputSourceLister lister,
                                              MSIMEInputSourcePropertyGetter propertyGetter);
/// `--input-source-enabled`：只读地报告本 bundle 的顶层输入源是否仍在系统输入源列表里；输出由调用方解释，命令本身不登记或启用输入源。
bool MSIMEShouldReportInputSourceEnabled(int argc, const char *argv[]);
OSStatus MSIMERegisterInputSource(NSURL *bundleURL, MSIMEInputSourceRegistrar registrar);
/// 登记前已有输入源时保留启用状态；首次安装只启用主模式和存在时的顶层输入源。
OSStatus MSIMERegisterAndEnableInputSources(NSURL *bundleURL, NSString *bundleIdentifier,
                                            MSIMEInputSourceRegistrar registrar,
                                            MSIMEInputSourceLister lister,
                                            MSIMEInputSourcePropertyGetter propertyGetter,
                                            MSIMEInputSourceEnabler enabler);
/// 将本次发现的模式加入 offered 记录，由调用方持久化；启动不启用或关闭模式，避免补回用户删除的输入源。保留函数名与参数以兼容现有调用方。
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
