#import "GlobalVoiceBackground.h"
#import "../settings/RuntimeOptions.h"

namespace {
NSString *const RevisionKey = @"MSIMEClientGlobalVoiceRegisteredRevision";
}
NSString *MSIMEGlobalVoiceBackgroundRevision(NSBundle *bundle) {
    // 开发重编可能不改变 Git 修订或版本；可执行文件和 plist 的修改时间也属于构建身份。
    NSDictionary *executable = [NSFileManager.defaultManager attributesOfItemAtPath:bundle.executablePath error:nil];
    NSDictionary *agent = [NSFileManager.defaultManager attributesOfItemAtPath:
        [bundle.bundlePath stringByAppendingPathComponent:@"Contents/Library/LaunchAgents/app.msime.global-voice.plist"] error:nil];
    return [NSString stringWithFormat:@"%@:%@:%@:%@", bundle.infoDictionary[@"MSIMEInputSourceRevision"] ?: @"",
        bundle.infoDictionary[@"CFBundleVersion"] ?: @"", executable[NSFileModificationDate] ?: @"", agent[NSFileModificationDate] ?: @""];
}
@implementation MSIMEGlobalVoiceBackground {
    BOOL _busy;
    NSError *_error;
    BOOL _enabled;
    NSString *_revision;
    SMAppService *_service;
    NSUserDefaults *_defaults;
    void (^_changed)(void);
}
- (BOOL)busy { return _busy; }
- (NSError *)error { return _error; }
- (void)configureService:(SMAppService *)service enabled:(BOOL)enabled revision:(NSString *)revision
               defaults:(NSUserDefaults *)defaults changed:(void (^)(void))changed {
    _service = service; _enabled = enabled; _revision = [revision copy]; _defaults = defaults; _changed = [changed copy];
    if (_busy) return;
    _error = nil;
    const SMAppServiceStatus status = service.status;
    // 待批准也包括系统设置里的明确拒绝；不能通过反复注销和注册撤销这个选择。
    if (enabled && status == SMAppServiceStatusRequiresApproval) return;
    if (enabled && status == SMAppServiceStatusEnabled && ![[defaults stringForKey:RevisionKey] isEqual:revision]) {
        _busy = YES;
        // Apple 要求完成注销后再登记更新后的 executable/plist；等待期间的关闭优先于旧启用请求。
        [service unregisterWithCompletionHandler:^(NSError *error) {
            dispatch_async(dispatch_get_main_queue(), ^{
                self->_busy = NO;
                self->_error = error;
                if (!error) {
                    // 完成回调已保证可重新登记；不因 status 的短暂旧值再次发起注销。
                    if (self->_enabled && self->_service.status != SMAppServiceStatusRequiresApproval) {
                        NSError *registrationError = nil;
                        if ([self->_service registerAndReturnError:&registrationError])
                            [self->_defaults setObject:self->_revision forKey:RevisionKey];
                        else self->_error = registrationError ?: [NSError errorWithDomain:@"MSIMEGlobalVoiceBackground" code:1 userInfo:nil];
                    } else if (!self->_enabled) [self->_defaults removeObjectForKey:RevisionKey];
                }
                if (self->_changed) self->_changed();
            });
        }];
        return;
    }
    NSError *error = nil;
    if (enabled && status != SMAppServiceStatusEnabled) {
        if ([service registerAndReturnError:&error]) [defaults setObject:revision forKey:RevisionKey];
        else _error = error ?: [NSError errorWithDomain:@"MSIMEGlobalVoiceBackground" code:1 userInfo:nil];
    } else if (!enabled && (status == SMAppServiceStatusEnabled || status == SMAppServiceStatusRequiresApproval)) {
        if ([service unregisterAndReturnError:&error]) [defaults removeObjectForKey:RevisionKey];
        else _error = error ?: [NSError errorWithDomain:@"MSIMEGlobalVoiceBackground" code:2 userInfo:nil];
    }
}
@end

BOOL MSIMEGlobalVoiceInstalledHostAtPaths(NSString *bundle, NSString *userInputMethods, NSString *options,
                                       NSString *defaultOptions, BOOL overridden) {
    if (overridden || !bundle.isAbsolutePath || !options.isAbsolutePath || !defaultOptions.isAbsolutePath ||
        ![options.stringByResolvingSymlinksInPath isEqual:defaultOptions.stringByResolvingSymlinksInPath]) return NO;
    NSString *parent = bundle.stringByResolvingSymlinksInPath.stringByDeletingLastPathComponent;
    return [bundle.pathExtension isEqual:@"app"] &&
        ([parent isEqual:userInputMethods.stringByResolvingSymlinksInPath] || [parent isEqual:@"/Library/Input Methods"]);
}
BOOL MSIMEGlobalVoiceInstalledHost(NSBundle *bundle) {
    NSDictionary *environment = NSProcessInfo.processInfo.environment;
    BOOL overridden = environment[@"MSIME_CLIENT_HOST_OPTIONS"] || environment[@"MSIME_IBUS_OPTIONS"] || environment[@"MSIME_CLIENT_STATE_DIR"];
    NSString *options = [bundle pathForResource:@"runtime-options" ofType:@"json"] ?: MSIMEDefaultRuntimeOptionsPath(NSFileManager.defaultManager);
    return MSIMEGlobalVoiceInstalledHostAtPaths(bundle.bundlePath, [NSHomeDirectory() stringByAppendingPathComponent:@"Library/Input Methods"],
        options, MSIMEDefaultRuntimeOptionsPath(NSFileManager.defaultManager), overridden);
}
