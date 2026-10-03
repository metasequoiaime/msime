#pragma once
#import <Foundation/Foundation.h>

// 本输入法属于哪个版本（edition），以及随版本而变的标识。
//
// 多个版本可以同时装在一台 Mac 上，彼此完全隔离。版本身份写在 bundle 的 Info.plist 里，由 scripts/edition_bundle.py 从版本表 shared/contracts/editions.json 生成：MSIMEEdition 是版本 id，MSIMEInputSchemes 是本版本提供的方案，MSIMEDefaultScheme 是回退方案，MSIMESettingsBundleIdentifier 是设置应用和状态目录的名字，MSIMEKeychainService 是原生账号窗口的钥匙串服务名，MSIMEWubiMixedPinyinDefault 是五笔混拼的默认值。
//
// full 的 Info.plist 不带这些键，与引入版本之前逐字节相同，所以没有 MSIMEEdition 就是 full，每个值都取 full 今天的那个。测试进程不是 bundle，同样读到 full。每个函数都有一个接收 Info.plist 字典的版本，测试用合成的字典验证其他版本。

static NSString *const MSIMEFullEditionIdentifier = @"full";
static NSString *const MSIMEFullInputMethodBundleIdentifier = @"app.msime.inputmethod.MetasequoiaIME";
static NSString *const MSIMEFullSettingsBundleIdentifier = @"app.msime.macos";
static NSString *const MSIMEFullKeychainService = @"com.metasequoia.msime.account";
static NSString *const MSIMEFullDefaultScheme = @"quanpin";
static NSString *const MSIMEFullVoiceProviderKeychainService = @"app.msime.client.voice.providers";
static NSString *const MSIMEFullUsageReportingDirectoryName = @"MSIME/telemetry";

static inline NSDictionary *MSIMEEditionInfo(void) { return NSBundle.mainBundle.infoDictionary ?: @{}; }

static inline NSString *MSIMEEditionStringIn(NSDictionary *info, NSString *key) {
    id value = info[key];
    return [value isKindOfClass:NSString.class] && [value length] ? value : nil;
}

// 版本 id；没有声明就是 full。
static inline NSString *MSIMEEditionIdentifierIn(NSDictionary *info) {
    return MSIMEEditionStringIn(info, @"MSIMEEdition") ?: MSIMEFullEditionIdentifier;
}
static inline BOOL MSIMEEditionIsFullIn(NSDictionary *info) {
    return [MSIMEEditionIdentifierIn(info) isEqualToString:MSIMEFullEditionIdentifier];
}

// 输入法 bundle 的标识：输入模式标识符的前缀、NSUserDefaults 域和统一日志的子系统。
static inline NSString *MSIMEInputMethodBundleIdentifierIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return MSIMEFullInputMethodBundleIdentifier;
    return MSIMEEditionStringIn(info, @"CFBundleIdentifier") ?: MSIMEFullInputMethodBundleIdentifier;
}

// 产品名（菜单标题、无障碍标签用），full 是「水杉输入法」。
static inline NSString *MSIMEEditionDisplayNameIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return @"水杉输入法";
    return MSIMEEditionStringIn(info, @"CFBundleDisplayName") ?: @"水杉输入法";
}

// 输入法 bundle 在 ~/Library/Input Methods 下的文件名。full 是「水杉输入法.app」；其他版本是可执行文件名加 .app（scripts/edition_bundle.py 让两者同名）。卸载按它找要移到废纸篓的 bundle，所以不能退回 full 的名字。
static inline NSString *MSIMEInputMethodBundleNameIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return @"水杉输入法.app";
    NSString *executable = MSIMEEditionStringIn(info, @"CFBundleExecutable");
    return executable ? [executable stringByAppendingString:@".app"] : nil;
}

// 设置应用的 bundle identifier，也是 Application Support 下状态目录的名字。
static inline NSString *MSIMESettingsBundleIdentifierIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return MSIMEFullSettingsBundleIdentifier;
    return MSIMEEditionStringIn(info, @"MSIMESettingsBundleIdentifier") ?: MSIMEFullSettingsBundleIdentifier;
}

static inline NSString *MSIMEKeychainServiceIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return MSIMEFullKeychainService;
    return MSIMEEditionStringIn(info, @"MSIMEKeychainService") ?: MSIMEFullKeychainService;
}

// 语音服务凭据（识别和润色服务的 API key）在钥匙串里的服务名。full 沿用今天的名字；其他版本是输入法 bundle id 加 `.voice`，正是卸载「同时删除本机数据」时删掉的那个服务（crates/host-macos/native/uninstaller.mm），所以一个版本存的 key 不会出现在另一个版本里，卸载也只带走自己的。
static inline NSString *MSIMEVoiceProviderKeychainServiceIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return MSIMEFullVoiceProviderKeychainService;
    return [MSIMEInputMethodBundleIdentifierIn(info) stringByAppendingString:@".voice"];
}

// 使用统计（install_id、事件队列、每日活跃标记、崩溃记录）放在 Application Support 下的哪个相对目录。full 是今天的 MSIME/telemetry；其他版本是 MSIME/<版本 id>/telemetry，同时安装的版本各有各的 install_id 和队列，日活不会被另一个版本去重掉，崩溃记录也不会在另一个版本启动时报出去。
static inline NSString *MSIMEUsageReportingDirectoryNameIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return MSIMEFullUsageReportingDirectoryName;
    return [NSString stringWithFormat:@"MSIME/%@/telemetry", MSIMEEditionIdentifierIn(info)];
}

// 本版本提供的方案；nil 表示 full，即全部方案。
static inline NSArray<NSString *> *MSIMEEditionInputSchemesIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return nil;
    id schemes = info[@"MSIMEInputSchemes"];
    if (![schemes isKindOfClass:NSArray.class]) return nil;
    NSMutableArray<NSString *> *names = [NSMutableArray array];
    for (id scheme in schemes)
        if ([scheme isKindOfClass:NSString.class]) [names addObject:scheme];
    return names;
}

// 偏好里的方案不可用时退回的方案。
static inline NSString *MSIMEEditionDefaultSchemeIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return MSIMEFullDefaultScheme;
    return MSIMEEditionStringIn(info, @"MSIMEDefaultScheme") ?: MSIMEFullDefaultScheme;
}

// 用户从没设置过五笔混拼时它是否打开。full 是关。
static inline BOOL MSIMEEditionWubiMixedPinyinDefaultIn(NSDictionary *info) {
    id value = info[@"MSIMEWubiMixedPinyinDefault"];
    return !MSIMEEditionIsFullIn(info) && [value isKindOfClass:NSNumber.class] && [value boolValue];
}

// 设置应用和输入法之间的分布式通知名。它们在整个登录会话里广播，所以不是 full 的版本在名字后面加上「.版本 id」，一个版本的设置应用不会叫醒或改动另一个版本的输入法。与 crates/host-macos 的 edition_notification_name 一致。
static inline NSString *MSIMEEditionNotificationNameIn(NSDictionary *info, NSString *base) {
    if (MSIMEEditionIsFullIn(info)) return base;
    return [NSString stringWithFormat:@"%@.%@", base, MSIMEEditionIdentifierIn(info)];
}

static inline NSString *MSIMEEditionIdentifier(void) { return MSIMEEditionIdentifierIn(MSIMEEditionInfo()); }
static inline BOOL MSIMEEditionIsFull(void) { return MSIMEEditionIsFullIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEInputMethodBundleIdentifier(void) { return MSIMEInputMethodBundleIdentifierIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEEditionDisplayName(void) { return MSIMEEditionDisplayNameIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEInputMethodBundleName(void) { return MSIMEInputMethodBundleNameIn(MSIMEEditionInfo()); }
static inline NSString *MSIMESettingsBundleIdentifier(void) { return MSIMESettingsBundleIdentifierIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEKeychainService(void) { return MSIMEKeychainServiceIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEVoiceProviderKeychainService(void) { return MSIMEVoiceProviderKeychainServiceIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEUsageReportingDirectoryName(void) { return MSIMEUsageReportingDirectoryNameIn(MSIMEEditionInfo()); }
static inline NSArray<NSString *> *MSIMEEditionInputSchemes(void) { return MSIMEEditionInputSchemesIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEEditionDefaultScheme(void) { return MSIMEEditionDefaultSchemeIn(MSIMEEditionInfo()); }
static inline BOOL MSIMEEditionWubiMixedPinyinDefault(void) { return MSIMEEditionWubiMixedPinyinDefaultIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEEditionNotificationName(NSString *base) { return MSIMEEditionNotificationNameIn(MSIMEEditionInfo(), base); }
