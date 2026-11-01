#pragma once
#import <Foundation/Foundation.h>
#include <cmath>
#include <limits>
#include "../core/SystemPathAlias.h"

#include "../core/EditionIdentity.h"

// Info.plist.in 声明的十一个输入模式。每个模式的图标是铺满图块的一个大字：中、双、五、粤、注、日、한、越、ཀ、笔或英；选中的那条在输入菜单里打勾，并在输入源列表里列出名称。模式标识符是本版本输入法的 bundle id 加上模式后缀：full 是 `app.msime.inputmethod.MetasequoiaIME.Hans` 这些，info-plist-names 对照 plist 检查它们；其他版本只声明自己的方案对应的模式和「英」，见 scripts/edition_bundle.py。进程里 bundle id 不会变，所以每个后缀只拼一次。
static inline NSString *MSIMEInputModeIdentifier(NSString *suffix) {
    static NSDictionary<NSString *, NSString *> *identifiers;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        NSString *bundle = MSIMEInputMethodBundleIdentifier();
        NSMutableDictionary<NSString *, NSString *> *all = [NSMutableDictionary dictionary];
        for (NSString *mode in @[ @"Hans", @"Shuangpin", @"Wubi", @"Roman", @"Japanese", @"Korean", @"Cantonese", @"Zhuyin", @"Vietnamese", @"Tibetan", @"Stroke" ])
            all[mode] = [NSString stringWithFormat:@"%@.%@", bundle, mode];
        identifiers = all;
    });
    return identifiers[suffix];
}
#define MSIMEChineseInputModeID MSIMEInputModeIdentifier(@"Hans")
#define MSIMEShuangpinInputModeID MSIMEInputModeIdentifier(@"Shuangpin")
#define MSIMEWubiInputModeID MSIMEInputModeIdentifier(@"Wubi")
#define MSIMEEnglishInputModeID MSIMEInputModeIdentifier(@"Roman")
#define MSIMEJapaneseInputModeID MSIMEInputModeIdentifier(@"Japanese")
#define MSIMEKoreanInputModeID MSIMEInputModeIdentifier(@"Korean")
#define MSIMECantoneseInputModeID MSIMEInputModeIdentifier(@"Cantonese")
#define MSIMEZhuyinInputModeID MSIMEInputModeIdentifier(@"Zhuyin")
#define MSIMEVietnameseInputModeID MSIMEInputModeIdentifier(@"Vietnamese")
#define MSIMETibetanInputModeID MSIMEInputModeIdentifier(@"Tibetan")
#define MSIMEStrokeInputModeID MSIMEInputModeIdentifier(@"Stroke")

// 等用户选中对应方案才打开的模式。安装和更新都不启用它们：登记时跳过，MSIMEEnableNewInputModes 只记录不启用，免得从没用过粤拼、注音、越南文、藏文或笔画的人输入菜单里平白多出五项。选中方案时请求启用对应模式（MSIMEOptInInputModeToEnable），切走方案也不关掉它。macOS 27 不允许进程启用键盘输入模式，这个请求在那里不生效，只能由用户在系统设置里添加，设置页的「菜单栏入口」告诉用户去哪里加。
static inline NSArray<NSString *> *MSIMEOptInInputModeIDs(void) {
    return @[ MSIMECantoneseInputModeID, MSIMEZhuyinInputModeID, MSIMEVietnameseInputModeID, MSIMETibetanInputModeID, MSIMEStrokeInputModeID ];
}

static inline BOOL MSIMEIsOptInInputModeID(NSString *identifier) {
    return identifier != nil && [MSIMEOptInInputModeIDs() containsObject:identifier];
}

// 按引擎线上顺序（0-9）排列的全部方案名：下标就是视图在 `scheme` 里报告的数字。
static inline NSArray<NSString *> *MSIMEInputSchemeNames(void) {
    return @[ @"quanpin", @"shuangpin", @"wubi", @"japanese", @"korean", @"cantonese", @"zhuyin", @"vietnamese", @"tibetan", @"stroke" ];
}

// 方案名是否是本版本提供的方案：引擎认识它，并且在本版本的方案列表里（full 不限制）。偏好里存着别的方案时按本版本的默认方案读。
static inline BOOL MSIMEEditionOffersScheme(NSString *scheme) {
    NSArray<NSString *> *offered = MSIMEEditionInputSchemes();
    return [MSIMEInputSchemeNames() containsObject:scheme] && (!offered || [offered containsObject:scheme]);
}

// 路径上是否是指定类型的真实条目：attributesOfItemAtPath: 不跟随符号链接，符号链接本身的类型是 NSFileTypeSymbolicLink，因此会被拒绝，与 host-api 的 resource_packs::installed_file 一致。
static inline BOOL MSIMEItemHasFileType(NSString *path, NSFileAttributeType type) {
    return [[NSFileManager.defaultManager attributesOfItemAtPath:path error:nil][NSFileType] isEqualToString:type];
}

// 资源包路径的每一层都必须是真实目录。`attributesOfItemAtPath:` 会跟随父目录的符号链接，所以这里交给 `StoragePathIsSafe` 逐层 `lstat`；只有目标核对过的 macOS `/var`、`/tmp` 系统别名可以经过，规则与 `crates/path-trust` 相同。
static inline BOOL MSIMEPathAncestorsAreReal(NSString *path) {
    if (![path isKindOfClass:NSString.class] || !path.isAbsolutePath) return NO;
    return msime::mac::StoragePathIsSafe(path.stringByStandardizingPath.fileSystemRepresentation, true);
}

// 某个方案此处能否真正运行。五笔需要码表（见下），粤拼、注音和笔画需要各自的词典（msime-cantonese.db、msime-zhuyin.db、msime-stroke.db）：按 host-api 的顺序，先找设置应用按需下载到 `<preferences_directory>/resource-packs/language-dictionaries/` 的副本（preferences_directory 须为绝对路径，资源包目录须是真实目录且带 msime-model.json，词典须是普通文件，符号链接一律不认），再找 HostOptions 的 `language_dictionaries` 目录；两处都没有时 host-api 会回退，此时提供该方案只会选中一个永远不生效的方案。其余方案（包括缺少日文词典时退化为纯假名的日文）不需要资源集之外的数据。输入法自身从不下载，下载只在设置应用里进行。
//
// 本版本不提供的方案在这里不存在：不出现在菜单和设置窗口里，偏好里写着它也会回退（MSIMEEditionInputSchemes，full 不限制）。
// 下载的资源包 `<preferences_directory>/resource-packs/<pack>/` 里的 `file`：preferences_directory 须为绝对路径，资源包目录须是真实目录且带 msime-model.json，文件须是普通文件，符号链接一律不认。
static inline BOOL MSIMEResourcePackHasFile(NSDictionary *hostOptions, NSString *pack, NSString *file) {
    id stateRoot = hostOptions[@"preferences_directory"];
    if (![stateRoot isKindOfClass:NSString.class] || ![stateRoot length] || ![stateRoot isAbsolutePath]) return NO;
    NSString *directory = [[stateRoot stringByAppendingPathComponent:@"resource-packs"] stringByAppendingPathComponent:pack];
    return MSIMEPathAncestorsAreReal(directory) && MSIMEItemHasFileType(directory, NSFileTypeDirectory) &&
           MSIMEItemHasFileType([directory stringByAppendingPathComponent:@"msime-model.json"], NSFileTypeRegular) &&
           MSIMEItemHasFileType([directory stringByAppendingPathComponent:file], NSFileTypeRegular);
}

static inline BOOL MSIMEInputSchemeAvailable(NSString *scheme, NSDictionary *hostOptions) {
    NSArray<NSString *> *offered = MSIMEEditionInputSchemes();
    if (offered && ![offered containsObject:scheme]) return NO;
    // 五笔要有码表 msime-wubi.db：资源目录（随包或词库更新）里有，或者设置应用下载了 wubi 资源包；macOS 发布包不带它时两处都没有，host-api 回退（wubi_dictionary_missing），这里也不提供。
    if ([scheme isKindOfClass:NSString.class] && [scheme isEqualToString:@"wubi"]) {
        id resources = hostOptions[@"resources"];
        if (![resources isKindOfClass:NSString.class] || ![resources length]) return YES;
        return MSIMEItemHasFileType([resources stringByAppendingPathComponent:@"msime-wubi.db"], NSFileTypeRegular) ||
               MSIMEResourcePackHasFile(hostOptions, @"wubi", @"msime-wubi.db");
    }
    NSDictionary<NSString *, NSString *> *files = @{@"cantonese": @"msime-cantonese.db", @"zhuyin": @"msime-zhuyin.db", @"stroke": @"msime-stroke.db"};
    NSString *file = [scheme isKindOfClass:NSString.class] ? files[scheme] : nil;
    if (!file) return [MSIMEInputSchemeNames() containsObject:scheme];
    if (MSIMEResourcePackHasFile(hostOptions, @"language-dictionaries", file)) return YES;
    id directory = hostOptions[@"language_dictionaries"];
    if (![directory isKindOfClass:NSString.class] || ![directory length]) return NO;
    BOOL isDirectory = NO;
    return [NSFileManager.defaultManager fileExistsAtPath:[directory stringByAppendingPathComponent:file] isDirectory:&isDirectory] && !isDirectory;
}

// 引擎按这些偏好实际运行的方案，与 host-api 的 effective_scheme 一致：此处跑不起来的方案让位给进入它之前的中文方案，再不行就是本版本的默认方案（full 是全拼）。输入菜单勾选的是它，菜单栏显示的也是它的模式，所以两处都不会声称一个用户并没有在用的方案。
static inline NSString *MSIMEEffectiveInputScheme(NSString *scheme, NSString *lastChineseScheme, NSDictionary *hostOptions) {
    if (MSIMEInputSchemeAvailable(scheme, hostOptions)) return scheme;
    return MSIMEInputSchemeAvailable(lastChineseScheme, hostOptions) ? lastChineseScheme : MSIMEEditionDefaultScheme();
}

// The mode the menu bar shows. English is the controller passing keys through; the others follow the scheme behind it.
enum class MSIMEInputMode { Chinese, Shuangpin, Wubi, English, Japanese, Korean, Cantonese, Zhuyin, Vietnamese, Tibetan, Stroke };

// English mode wins over the scheme: 英 is shown whenever the controller passes keys through, whatever scheme is behind it. Otherwise each scheme with a mode of its own shows that mode, and quanpin shows 中. A mode the system does not offer falls back to 中 in MSIMESelectSystemInputMode.
static inline MSIMEInputMode MSIMEInputModeFor(BOOL english, NSString *scheme) {
    if (english) return MSIMEInputMode::English;
    if ([scheme isEqualToString:@"shuangpin"]) return MSIMEInputMode::Shuangpin;
    if ([scheme isEqualToString:@"wubi"]) return MSIMEInputMode::Wubi;
    if ([scheme isEqualToString:@"japanese"]) return MSIMEInputMode::Japanese;
    if ([scheme isEqualToString:@"korean"]) return MSIMEInputMode::Korean;
    if ([scheme isEqualToString:@"cantonese"]) return MSIMEInputMode::Cantonese;
    if ([scheme isEqualToString:@"zhuyin"]) return MSIMEInputMode::Zhuyin;
    if ([scheme isEqualToString:@"vietnamese"]) return MSIMEInputMode::Vietnamese;
    if ([scheme isEqualToString:@"tibetan"]) return MSIMEInputMode::Tibetan;
    if ([scheme isEqualToString:@"stroke"]) return MSIMEInputMode::Stroke;
    return MSIMEInputMode::Chinese;
}

static inline NSString *MSIMEInputModeID(MSIMEInputMode mode) {
    switch (mode) {
    case MSIMEInputMode::Shuangpin: return MSIMEShuangpinInputModeID;
    case MSIMEInputMode::Wubi: return MSIMEWubiInputModeID;
    case MSIMEInputMode::English: return MSIMEEnglishInputModeID;
    case MSIMEInputMode::Japanese: return MSIMEJapaneseInputModeID;
    case MSIMEInputMode::Korean: return MSIMEKoreanInputModeID;
    case MSIMEInputMode::Cantonese: return MSIMECantoneseInputModeID;
    case MSIMEInputMode::Zhuyin: return MSIMEZhuyinInputModeID;
    case MSIMEInputMode::Vietnamese: return MSIMEVietnameseInputModeID;
    case MSIMEInputMode::Tibetan: return MSIMETibetanInputModeID;
    case MSIMEInputMode::Stroke: return MSIMEStrokeInputModeID;
    case MSIMEInputMode::Chinese: break;
    }
    return MSIMEChineseInputModeID;
}

// 本版本的 bundle 是否声明了这个模式。full 声明全部十一个；其他版本只声明 scripts/edition_bundle.py 的 mode_plan 给出的那几个，例如五笔版把五笔的名字和图标放在 .Hans 上，不声明 .Wubi。
static inline BOOL MSIMEInputModeDeclaredIn(NSDictionary *info, NSString *identifier) {
    if (MSIMEEditionIsFullIn(info)) return YES;
    id component = info[@"ComponentInputModeDict"];
    id modes = [component isKindOfClass:NSDictionary.class] ? component[@"tsInputModeListKey"] : nil;
    return identifier && [modes isKindOfClass:NSDictionary.class] && modes[identifier] != nil;
}

// 菜单栏里代表这个方案的模式：方案自己的模式，本版本没有声明它时是 中（.Hans），与 MSIMESelectSystemInputMode 的回退一致。设置窗口的「菜单栏入口」检查的就是它。
static inline NSString *MSIMEInputModeIDForSchemeIn(NSDictionary *info, NSString *scheme) {
    NSString *mode = MSIMEInputModeID(MSIMEInputModeFor(NO, scheme));
    return MSIMEInputModeDeclaredIn(info, mode) ? mode : MSIMEChineseInputModeID;
}

// 系统输入法菜单里这个模式使用的图标文件名：取 bundle 里该模式的 tsInputModeMenuIconFileKey，取值路径与 MSIMEInputModeDeclaredIn 相同。本版本没有声明这个模式时（五笔版把五笔的图标放在 .Hans 上）取主模式 .Hans 的，再没有时取输入法本身的 tsInputMethodIconFileKey，最后是 MSIMEClientInputMethodMenuIcon.tiff。菜单栏状态项固定使用产品 logo，不走这条模式图标路径。
static inline NSString *MSIMEInputModeMenuIconFileIn(NSDictionary *info, NSString *identifier) {
    id component = info[@"ComponentInputModeDict"];
    id modes = [component isKindOfClass:NSDictionary.class] ? component[@"tsInputModeListKey"] : nil;
    for (NSString *mode in @[identifier ?: @"", MSIMEChineseInputModeID]) {
        id entry = [modes isKindOfClass:NSDictionary.class] ? modes[mode] : nil;
        id file = [entry isKindOfClass:NSDictionary.class] ? entry[@"tsInputModeMenuIconFileKey"] : nil;
        if ([file isKindOfClass:NSString.class] && [file length]) return file;
    }
    id file = info[@"tsInputMethodIconFileKey"];
    return [file isKindOfClass:NSString.class] && [file length] ? file : @"MSIMEClientInputMethodMenuIcon.tiff";
}

// 一个模式在输入法菜单里的名字（与 InfoPlist.strings 一致），以及系统设置「添加」对话框把它归在哪个语言下（对应 Info.plist.in 的 TISIntendedLanguage）。macOS 27 不允许进程启用键盘输入模式，设置窗口靠这两项告诉用户去哪里自己添加；共享设置页 `macos-input-mode-entries-section.tsx` 里有同一张表。不是 full 的版本名字随版本而变，直接读 bundle 里 zh-Hans 的 InfoPlist.strings。
static inline NSString *MSIMEInputModeMenuName(NSString *identifier) {
    if (!MSIMEEditionIsFull()) {
        NSString *path = [NSBundle.mainBundle pathForResource:@"InfoPlist" ofType:@"strings" inDirectory:nil forLocalization:@"zh-Hans"];
        NSDictionary *names = path ? [NSDictionary dictionaryWithContentsOfFile:path] : nil;
        id name = identifier ? names[identifier] : nil;
        return [name isKindOfClass:NSString.class] ? name : nil;
    }
    NSDictionary<NSString *, NSString *> *names = @{
        MSIMEChineseInputModeID: @"水杉输入法 · 中", MSIMEShuangpinInputModeID: @"水杉输入法 · 双",
        MSIMEWubiInputModeID: @"水杉输入法 · 五", MSIMECantoneseInputModeID: @"水杉输入法 · 粤",
        MSIMEZhuyinInputModeID: @"水杉输入法 · 注", MSIMEJapaneseInputModeID: @"水杉输入法 · 日",
        MSIMEKoreanInputModeID: @"水杉输入法 · 韩", MSIMEVietnameseInputModeID: @"水杉输入法 · 越",
        MSIMETibetanInputModeID: @"水杉输入法 · 藏", MSIMEStrokeInputModeID: @"水杉输入法 · 笔",
        MSIMEEnglishInputModeID: @"水杉输入法 · 英",
    };
    return identifier ? names[identifier] : nil;
}
// 不是 full 的版本按 bundle 里这个模式的 TISIntendedLanguage 说语言：日文、越南文、藏文版的主模式和「英」登记在 ja、vi、bo 下（scripts/edition_bundle.py），模式标识符本身看不出来。
static inline NSString *MSIMEInputModeAddDialogLanguageIn(NSDictionary *info, NSString *identifier) {
    if (!MSIMEEditionIsFullIn(info)) {
        id component = info[@"ComponentInputModeDict"];
        id modes = [component isKindOfClass:NSDictionary.class] ? component[@"tsInputModeListKey"] : nil;
        id mode = identifier && [modes isKindOfClass:NSDictionary.class] ? modes[identifier] : nil;
        id language = [mode isKindOfClass:NSDictionary.class] ? mode[@"TISIntendedLanguage"] : nil;
        NSDictionary<NSString *, NSString *> *names = @{
            @"zh-Hans": @"简体中文", @"zh-Hant": @"繁体中文", @"yue-Hant": @"粤语", @"ja": @"日语",
            @"ko": @"韩语", @"vi": @"越南语", @"bo": @"藏语",
        };
        NSString *name = [language isKindOfClass:NSString.class] ? names[language] : nil;
        if (name) return name;
    }
    if ([identifier isEqualToString:MSIMECantoneseInputModeID]) return @"粤语";
    if ([identifier isEqualToString:MSIMEZhuyinInputModeID]) return @"繁体中文";
    if ([identifier isEqualToString:MSIMEJapaneseInputModeID]) return @"日语";
    if ([identifier isEqualToString:MSIMEKoreanInputModeID]) return @"韩语";
    if ([identifier isEqualToString:MSIMEVietnameseInputModeID]) return @"越南语";
    if ([identifier isEqualToString:MSIMETibetanInputModeID]) return @"藏语";
    return @"简体中文";
}
static inline NSString *MSIMEInputModeAddDialogLanguage(NSString *identifier) { return MSIMEInputModeAddDialogLanguageIn(MSIMEEditionInfo(), identifier); }

static inline BOOL MSIMEIsInputModeID(id value) {
    return [value isKindOfClass:NSString.class] &&
           ([value isEqualToString:MSIMEChineseInputModeID] || [value isEqualToString:MSIMEShuangpinInputModeID] ||
            [value isEqualToString:MSIMEWubiInputModeID] || [value isEqualToString:MSIMEEnglishInputModeID] ||
            [value isEqualToString:MSIMEJapaneseInputModeID] || [value isEqualToString:MSIMEKoreanInputModeID] ||
            [value isEqualToString:MSIMECantoneseInputModeID] || [value isEqualToString:MSIMEZhuyinInputModeID] ||
            [value isEqualToString:MSIMEVietnameseInputModeID] || [value isEqualToString:MSIMETibetanInputModeID] ||
            [value isEqualToString:MSIMEStrokeInputModeID]);
}

// The mode an identifier names. Anything that is not one of this bundle's modes reads as Chinese; callers check MSIMEIsInputModeID first when that matters.
static inline MSIMEInputMode MSIMEInputModeForID(NSString *identifier) {
    if ([identifier isEqualToString:MSIMEShuangpinInputModeID]) return MSIMEInputMode::Shuangpin;
    if ([identifier isEqualToString:MSIMEWubiInputModeID]) return MSIMEInputMode::Wubi;
    if ([identifier isEqualToString:MSIMEEnglishInputModeID]) return MSIMEInputMode::English;
    if ([identifier isEqualToString:MSIMEJapaneseInputModeID]) return MSIMEInputMode::Japanese;
    if ([identifier isEqualToString:MSIMEKoreanInputModeID]) return MSIMEInputMode::Korean;
    if ([identifier isEqualToString:MSIMECantoneseInputModeID]) return MSIMEInputMode::Cantonese;
    if ([identifier isEqualToString:MSIMEZhuyinInputModeID]) return MSIMEInputMode::Zhuyin;
    if ([identifier isEqualToString:MSIMEVietnameseInputModeID]) return MSIMEInputMode::Vietnamese;
    if ([identifier isEqualToString:MSIMETibetanInputModeID]) return MSIMEInputMode::Tibetan;
    if ([identifier isEqualToString:MSIMEStrokeInputModeID]) return MSIMEInputMode::Stroke;
    return MSIMEInputMode::Chinese;
}

// The scheme a mode selects, or nil for English, which leaves the scheme alone. 中 has no scheme of its own: it goes back to the Chinese scheme the user left, which only the preferences know - see MSIMESchemeForReportedInputMode.
static inline NSString *MSIMESchemeForInputMode(MSIMEInputMode mode) {
    switch (mode) {
    case MSIMEInputMode::Shuangpin: return @"shuangpin";
    case MSIMEInputMode::Wubi: return @"wubi";
    case MSIMEInputMode::Japanese: return @"japanese";
    case MSIMEInputMode::Korean: return @"korean";
    case MSIMEInputMode::Cantonese: return @"cantonese";
    case MSIMEInputMode::Zhuyin: return @"zhuyin";
    case MSIMEInputMode::Vietnamese: return @"vietnamese";
    case MSIMEInputMode::Tibetan: return @"tibetan";
    case MSIMEInputMode::Stroke: return @"stroke";
    case MSIMEInputMode::Chinese:
    case MSIMEInputMode::English: break;
    }
    return nil;
}

// Keeps the system's selected input mode and the controller's Chinese/English state and scheme in step without either side echoing the other. The selected mode is global to the login session, so one state serves every controller instance.
//
// `current` is the mode last reported by the system or last requested by the controller; a report that repeats it is the system confirming what is already shown, not a user choice, so it does not flip the controller's state. `selecting` is set while the controller is asking the client to switch, so a report delivered synchronously from inside that call is not treated as a new choice either.
//
// `switchedIn` 表示切进水杉之后还没见到用户在水杉里做过选择：离开水杉时的清空和进程启动都把它置为真，第一次被采纳的报告或第一次按下的键把它清掉。激活后的校准（MSIMERecordRestoredSystemInputMode）和控制器自己的选择（MSIMESelectSystemInputMode）都不碰它，所以切回时系统落在 中 上的那次报告不论排在校准和随后选回 双 之前还是之后，都仍按切回处理，不当作用户越过 双 选了 中（#10199）。进程启动也算切进：Sparkle 在切走时装更新、进程退出，切回时系统从新 bundle 拉起的正是这种情况；代价是进程刚启动、还没按键时用户在输入菜单里明确选 中，这一次不改方案。它依赖离开时的清空真的发生：离开回调在通知送达时才读当前输入源，用户很快地切走又切回（连按两次 Caps Lock 或 Ctrl+Space）、或者通知送达得晚，读到的已经是水杉，就不会清空，这时切回报告的 中 仍按水杉里的选择处理。
//
// `restoringUntil` 是系统自己恢复输入源的那段时间（systemUptime 秒）：开盖时系统会恢复输入源并报告一个模式，这份报告不是用户的选择，有用户每次开盖方案都被改成没碰过的注音。合盖睡眠或锁屏时它是无穷大（锁着屏用户在水杉里什么也选不了），解锁、或者没锁屏的唤醒之后再保留几秒。期间的报告只记下，不改方案也不改中英状态，控制器随后把菜单栏选回自己的模式；解锁之后的第一次报告或用户按下的第一个键结束它。从锁屏切走时清空共享记录也保留它，否则解锁时的报告若先于解锁通知送到就不受保护。代价是解锁后几秒内、还没按键时在输入菜单里明确选别的模式，这一次不生效。
struct MSIMESystemInputModeState {
    NSString *current = nil;
    bool selecting = false;
    bool switchedIn = true;
    double restoringUntil = 0;
};

// 解锁或唤醒之后多久内的报告仍按系统恢复处理。
static constexpr double MSIMESystemInputModeRestoreSeconds = 10;

// 合盖睡眠或锁屏：直到解锁都按系统恢复处理。
static inline void MSIMEHoldSystemInputModeRestore(MSIMESystemInputModeState &state) {
    state.restoringUntil = std::numeric_limits<double>::infinity();
}

// 解锁，或者没锁屏的唤醒：从 `now`（systemUptime）起再保留几秒。
static inline void MSIMEBeginSystemInputModeRestore(MSIMESystemInputModeState &state, double now) {
    state.restoringUntil = now + MSIMESystemInputModeRestoreSeconds;
}

// 一次报告按系统恢复处理之后：解锁后的窗口到此结束，锁着屏时继续保持。
static inline void MSIMEFinishSystemInputModeRestore(MSIMESystemInputModeState &state) {
    if (!std::isinf(state.restoringUntil)) state.restoringUntil = 0;
}

static inline BOOL MSIMESystemInputModeRestoring(const MSIMESystemInputModeState &state, double now) {
    return now < state.restoringUntil;
}

// The system's selected input mode is global to the login session, so every controller instance shares one record of it. Leaving the input method clears it, so the first report after coming back is adopted even if it names the mode shown before leaving.
inline MSIMESystemInputModeState &MSIMESharedSystemInputModeState() {
    static MSIMESystemInputModeState state;
    return state;
}

// 离开水杉时清空记录，系统恢复输入源的那段时间保留：锁屏本身就会切走（见 restoringUntil）。
static inline void MSIMEResetSystemInputModeState(MSIMESystemInputModeState &state) {
    const double restoringUntil = state.restoringUntil;
    state = MSIMESystemInputModeState{};
    state.restoringUntil = restoringUntil;
}

// Records a mode the system reported through setValue:forTag:client:. Returns YES when the controller should adopt it: a known mode that differs from the one already shown and that the controller did not just request itself.
static inline BOOL MSIMEAdoptReportedInputMode(MSIMESystemInputModeState &state, id value) {
    if (!MSIMEIsInputModeID(value)) return NO;
    const BOOL changed = ![value isEqualToString:state.current];
    state.current = [value copy];
    return changed && !state.selecting;
}

// Whether the system offers a mode for selection. A mode the user removed in System Settings, or one an install from before the mode existed has not registered yet, cannot be selected, and asking for it would leave `current` naming a mode the menu bar does not show - the next report of the real one would then flip the controller's state back.
using MSIMEInputModeAvailability = BOOL (*)(NSString *identifier);

// 激活回调先于系统恢复输入源时，不能把空记录当成需要切换。只用此刻实际选中的本 bundle 模式校准记录。
static inline BOOL MSIMERecordRestoredSystemInputMode(MSIMESystemInputModeState &state, NSString *selected) {
    if (state.selecting || !MSIMEIsInputModeID(selected)) return NO;
    state.current = [selected copy];
    return YES;
}

// 用户选中某个模式后方案要切到哪里，返回 nil 表示不动方案。中 回到进入 japanese、korean、vietnamese 或 tibetan 之前的中文方案，或者保留已经在用的中文方案——除非那个方案有自己的、系统提供的模式：那说明用户是越过 双、五、粤、注 或 笔 选了 中，意思是本版本的默认方案（full 是全拼），保留方案会立刻又选回那个模式。
//
// `switchedIn` 取自共享记录的同名字段（MSIMESystemInputModeState），表示切进水杉后还没见到用户的选择。这时系统报告的 中 不算用户越过 双 等模式做的选择：macOS 切回时可能落在 中 入口上，而用户并没有在 中 和 双 之间挑过（#10199）。所以照样回到或保留中文方案，控制器随后的同步会把菜单栏重新选回这个方案自己的模式；只有已经在水杉里按过键或做过选择之后从 双 等模式切到 中，才改用默认方案。
static inline NSString *MSIMESchemeForReportedInputMode(MSIMEInputMode mode, NSString *scheme, NSString *lastChineseScheme,
                                                        MSIMEInputModeAvailability available, BOOL switchedIn) {
    if (mode != MSIMEInputMode::Chinese) return MSIMESchemeForInputMode(mode);
    const BOOL returning = [@[@"japanese", @"korean", @"vietnamese", @"tibetan"] containsObject:scheme];
    NSString *chinese = returning ? lastChineseScheme : scheme;
    const MSIMEInputMode own = MSIMEInputModeFor(NO, chinese);
    if (!switchedIn && own != MSIMEInputMode::Chinese && available && available(MSIMEInputModeID(own))) return MSIMEEditionDefaultScheme();
    return returning ? lastChineseScheme : nil;
}

// Asks the client to show `mode`, unless it already does or the system does not offer it. Returns whether a switch was requested. A scheme whose own mode is not offered falls back to 中: the controller is still composing through the Engine, and 中 is closer to that than an 英 left over from before. A mode the system has just reported as shown is offered by definition, so it is kept rather than second-guessed.
static inline BOOL MSIMESelectSystemInputMode(MSIMESystemInputModeState &state, NSString *mode, id client,
                                              MSIMEInputModeAvailability available) {
    if (!available) return NO;
    if (![mode isEqualToString:MSIMEChineseInputModeID] && ![mode isEqualToString:MSIMEEnglishInputModeID] &&
        ![mode isEqualToString:state.current] && !available(mode))
        mode = MSIMEChineseInputModeID;
    if (state.selecting || [mode isEqualToString:state.current] || ![client respondsToSelector:@selector(selectInputMode:)] ||
        !available(mode))
        return NO;
    state.current = mode;
    state.selecting = true;
    [client performSelector:@selector(selectInputMode:) withObject:mode];
    state.selecting = false;
    return YES;
}

// 方案从 `previous` 变成 `scheme` 时要启用的按需模式，没有则返回 nil。只有切到带按需模式的方案才算：模式在用户选中方案时启用——在菜单、任一设置窗口或经由它的模式——而不是每次输入法启动在这个方案上就启用，用户可能已经把它从输入菜单移除了。`previous` 为 nil 是第一次同步，同样算一次切换：这时用户正用着这个方案，而第一次同步只会发生一次，不会把用户之后移除的模式再加回来。在这台机器上跑不起来的方案（MSIMEInputSchemeAvailable）不启用任何模式，所以输入菜单不会提供一个会退回其它方案的模式。
//
// 本版本的 bundle 没有声明这个模式时同样返回 nil：越南文、藏文版只有 .Hans 和「英」，方案自己的模式不存在，请求启用它只会失败（fnfErr），这次切换就永远记不下来，之后每次同步都再按全部已安装输入源查一遍。
static inline NSString *MSIMEOptInInputModeToEnableIn(NSDictionary *info, NSString *previous, NSString *scheme, BOOL available) {
    if ([previous isEqualToString:scheme] || !available) return nil;
    NSString *mode = MSIMEInputModeID(MSIMEInputModeFor(NO, scheme));
    return MSIMEIsOptInInputModeID(mode) && MSIMEInputModeDeclaredIn(info, mode) ? mode : nil;
}
static inline NSString *MSIMEOptInInputModeToEnable(NSString *previous, NSString *scheme, BOOL available) {
    return MSIMEOptInInputModeToEnableIn(MSIMEEditionInfo(), previous, scheme, available);
}
