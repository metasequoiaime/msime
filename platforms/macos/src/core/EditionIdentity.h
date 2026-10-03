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

// 设置应用的 bundle identifier，也是 Application Support 下状态目录的名字。
static inline NSString *MSIMESettingsBundleIdentifierIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return MSIMEFullSettingsBundleIdentifier;
    return MSIMEEditionStringIn(info, @"MSIMESettingsBundleIdentifier") ?: MSIMEFullSettingsBundleIdentifier;
}

static inline NSString *MSIMEKeychainServiceIn(NSDictionary *info) {
    if (MSIMEEditionIsFullIn(info)) return MSIMEFullKeychainService;
    return MSIMEEditionStringIn(info, @"MSIMEKeychainService") ?: MSIMEFullKeychainService;
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
static inline NSString *MSIMESettingsBundleIdentifier(void) { return MSIMESettingsBundleIdentifierIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEKeychainService(void) { return MSIMEKeychainServiceIn(MSIMEEditionInfo()); }
static inline NSArray<NSString *> *MSIMEEditionInputSchemes(void) { return MSIMEEditionInputSchemesIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEEditionDefaultScheme(void) { return MSIMEEditionDefaultSchemeIn(MSIMEEditionInfo()); }
static inline BOOL MSIMEEditionWubiMixedPinyinDefault(void) { return MSIMEEditionWubiMixedPinyinDefaultIn(MSIMEEditionInfo()); }
static inline NSString *MSIMEEditionNotificationName(NSString *base) { return MSIMEEditionNotificationNameIn(MSIMEEditionInfo(), base); }
