#pragma once
#import <Foundation/Foundation.h>
#include <errno.h>
#include <sys/stat.h>

// Info.plist.in 声明的九个输入模式。每个模式的图标是铺满图块的一个大字：中、双、五、粤、注、日、한、越或英；选中的那条在输入菜单里打勾，并在输入源列表里列出名称。info-plist-names 对照 plist 检查这些字面量。
static NSString *const MSIMEChineseInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Hans";
static NSString *const MSIMEShuangpinInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Shuangpin";
static NSString *const MSIMEWubiInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Wubi";
static NSString *const MSIMEEnglishInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Roman";
static NSString *const MSIMEJapaneseInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Japanese";
static NSString *const MSIMEKoreanInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Korean";
static NSString *const MSIMECantoneseInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Cantonese";
static NSString *const MSIMEZhuyinInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Zhuyin";
static NSString *const MSIMEVietnameseInputModeID = @"app.msime.inputmethod.MetasequoiaIME.Vietnamese";

// 等用户选中对应方案才打开的模式。安装和更新都不启用它们：登记时跳过，MSIMEEnableNewInputModes 只记录不启用，免得从没用过粤拼、注音或越南文的人输入菜单里平白多出三项。选中方案时请求启用对应模式（MSIMEOptInInputModeToEnable），切走方案也不关掉它。macOS 27 不允许进程启用键盘输入模式，这个请求在那里不生效，只能由用户在系统设置里添加，设置页的「菜单栏入口」告诉用户去哪里加。
static inline NSArray<NSString *> *MSIMEOptInInputModeIDs(void) {
    return @[ MSIMECantoneseInputModeID, MSIMEZhuyinInputModeID, MSIMEVietnameseInputModeID ];
}

static inline BOOL MSIMEIsOptInInputModeID(NSString *identifier) {
    return identifier != nil && [MSIMEOptInInputModeIDs() containsObject:identifier];
}

// Every scheme name in engine wire order (0-7): the index is the number the View reports as `scheme`.
static inline NSArray<NSString *> *MSIMEInputSchemeNames(void) {
    return @[ @"quanpin", @"shuangpin", @"wubi", @"japanese", @"korean", @"cantonese", @"zhuyin", @"vietnamese" ];
}

// 路径上是否是指定类型的真实条目：attributesOfItemAtPath: 不跟随符号链接，符号链接本身的类型是 NSFileTypeSymbolicLink，因此会被拒绝，与 host-api 的 resource_packs::installed_file 一致。
static inline BOOL MSIMEItemHasFileType(NSString *path, NSFileAttributeType type) {
    return [[NSFileManager.defaultManager attributesOfItemAtPath:path error:nil][NSFileType] isEqualToString:type];
}

// 资源包路径的每一层父目录都必须是真实目录。`attributesOfItemAtPath:` 会跟随父目录的符号链接，所以这里逐层用 `lstat` 检查；macOS 的 `/var` 和 `/tmp` 别名是受信任的例外。
static inline BOOL MSIMEPathAncestorsAreReal(NSString *path) {
    if (![path isKindOfClass:NSString.class] || !path.isAbsolutePath) return NO;
    NSString *current = @"/";
    for (NSString *component in path.stringByStandardizingPath.pathComponents) {
        if ([component isEqualToString:@"/"]) continue;
        current = [current stringByAppendingPathComponent:component];
        if ([current isEqualToString:@"/var"] || [current isEqualToString:@"/tmp"]) continue;
        struct stat status = {};
        if (lstat(current.fileSystemRepresentation, &status) == 0) {
            if (S_ISLNK(status.st_mode) || !S_ISDIR(status.st_mode)) return NO;
        } else if (errno != ENOENT) {
            return NO;
        }
    }
    return YES;
}

// 某个方案此处能否真正运行。粤拼和注音需要各自的词典：按 host-api 的顺序，先找设置应用按需下载到 `<preferences_directory>/resource-packs/language-dictionaries/` 的副本（preferences_directory 须为绝对路径，资源包目录须是真实目录且带 msime-model.json，词典须是普通文件，符号链接一律不认），再找 HostOptions 的 `language_dictionaries` 目录；两处都没有时 host-api 会回退，此时提供该方案只会选中一个永远不生效的方案。其余方案（包括缺少日文词典时退化为纯假名的日文）不需要资源集之外的数据。输入法自身从不下载，下载只在设置应用里进行。
static inline BOOL MSIMEInputSchemeAvailable(NSString *scheme, NSDictionary *hostOptions) {
    NSString *file = [scheme isEqualToString:@"cantonese"] ? @"cantonese.db" : ([scheme isEqualToString:@"zhuyin"] ? @"zhuyin.db" : nil);
    if (!file) return [MSIMEInputSchemeNames() containsObject:scheme];
    id stateRoot = hostOptions[@"preferences_directory"];
    if ([stateRoot isKindOfClass:NSString.class] && [stateRoot length] && [stateRoot isAbsolutePath]) {
        NSString *pack = [[stateRoot stringByAppendingPathComponent:@"resource-packs"] stringByAppendingPathComponent:@"language-dictionaries"];
        if (MSIMEPathAncestorsAreReal(pack) && MSIMEItemHasFileType(pack, NSFileTypeDirectory) &&
            MSIMEItemHasFileType([pack stringByAppendingPathComponent:@"msime-model.json"], NSFileTypeRegular) &&
            MSIMEItemHasFileType([pack stringByAppendingPathComponent:file], NSFileTypeRegular))
            return YES;
    }
    id directory = hostOptions[@"language_dictionaries"];
    if (![directory isKindOfClass:NSString.class] || ![directory length]) return NO;
    BOOL isDirectory = NO;
    return [NSFileManager.defaultManager fileExistsAtPath:[directory stringByAppendingPathComponent:file] isDirectory:&isDirectory] && !isDirectory;
}

// The scheme the Engine actually runs for these preferences, mirroring host-api's effective_scheme: a scheme that cannot run here gives way to the Chinese scheme it was entered from, or to quanpin. The input menu checks this one and the menu bar shows its mode, so neither claims a scheme the user is not typing in.
static inline NSString *MSIMEEffectiveInputScheme(NSString *scheme, NSString *lastChineseScheme, NSDictionary *hostOptions) {
    if (MSIMEInputSchemeAvailable(scheme, hostOptions)) return scheme;
    return MSIMEInputSchemeAvailable(lastChineseScheme, hostOptions) ? lastChineseScheme : @"quanpin";
}

// The mode the menu bar shows. English is the controller passing keys through; the others follow the scheme behind it.
enum class MSIMEInputMode { Chinese, Shuangpin, Wubi, English, Japanese, Korean, Cantonese, Zhuyin, Vietnamese };

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
    case MSIMEInputMode::Chinese: break;
    }
    return MSIMEChineseInputModeID;
}

// 一个模式在输入法菜单里的名字（与 InfoPlist.strings 一致），以及系统设置「添加」对话框把它归在哪个语言下（对应 Info.plist.in 的 TISIntendedLanguage）。macOS 27 不允许进程启用键盘输入模式，设置窗口靠这两项告诉用户去哪里自己添加；共享设置页 `macos-input-mode-entries-section.tsx` 里有同一张表。
static inline NSString *MSIMEInputModeMenuName(NSString *identifier) {
    NSDictionary<NSString *, NSString *> *names = @{
        MSIMEChineseInputModeID: @"水杉输入法 · 中", MSIMEShuangpinInputModeID: @"水杉输入法 · 双",
        MSIMEWubiInputModeID: @"水杉输入法 · 五", MSIMECantoneseInputModeID: @"水杉输入法 · 粤",
        MSIMEZhuyinInputModeID: @"水杉输入法 · 注", MSIMEJapaneseInputModeID: @"水杉输入法 · 日",
        MSIMEKoreanInputModeID: @"水杉输入法 · 韩", MSIMEVietnameseInputModeID: @"水杉输入法 · 越",
        MSIMEEnglishInputModeID: @"水杉输入法 · 英",
    };
    return identifier ? names[identifier] : nil;
}
static inline NSString *MSIMEInputModeAddDialogLanguage(NSString *identifier) {
    if ([identifier isEqualToString:MSIMECantoneseInputModeID]) return @"粤语";
    if ([identifier isEqualToString:MSIMEZhuyinInputModeID]) return @"繁体中文";
    if ([identifier isEqualToString:MSIMEJapaneseInputModeID]) return @"日语";
    if ([identifier isEqualToString:MSIMEKoreanInputModeID]) return @"韩语";
    if ([identifier isEqualToString:MSIMEVietnameseInputModeID]) return @"越南语";
    return @"简体中文";
}

static inline BOOL MSIMEIsInputModeID(id value) {
    return [value isKindOfClass:NSString.class] &&
           ([value isEqualToString:MSIMEChineseInputModeID] || [value isEqualToString:MSIMEShuangpinInputModeID] ||
            [value isEqualToString:MSIMEWubiInputModeID] || [value isEqualToString:MSIMEEnglishInputModeID] ||
            [value isEqualToString:MSIMEJapaneseInputModeID] || [value isEqualToString:MSIMEKoreanInputModeID] ||
            [value isEqualToString:MSIMECantoneseInputModeID] || [value isEqualToString:MSIMEZhuyinInputModeID] ||
            [value isEqualToString:MSIMEVietnameseInputModeID]);
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
    case MSIMEInputMode::Chinese:
    case MSIMEInputMode::English: break;
    }
    return nil;
}

// Keeps the system's selected input mode and the controller's Chinese/English state and scheme in step without either side echoing the other. The selected mode is global to the login session, so one state serves every controller instance.
//
// `current` is the mode last reported by the system or last requested by the controller; a report that repeats it is the system confirming what is already shown, not a user choice, so it does not flip the controller's state. `selecting` is set while the controller is asking the client to switch, so a report delivered synchronously from inside that call is not treated as a new choice either.
struct MSIMESystemInputModeState {
    NSString *current = nil;
    bool selecting = false;
};

// The system's selected input mode is global to the login session, so every controller instance shares one record of it. Leaving the input method clears it, so the first report after coming back is adopted even if it names the mode shown before leaving.
inline MSIMESystemInputModeState &MSIMESharedSystemInputModeState() {
    static MSIMESystemInputModeState state;
    return state;
}

static inline void MSIMEResetSystemInputModeState(MSIMESystemInputModeState &state) { state = MSIMESystemInputModeState{}; }

// Records a mode the system reported through setValue:forTag:client:. Returns YES when the controller should adopt it: a known mode that differs from the one already shown and that the controller did not just request itself.
static inline BOOL MSIMEAdoptReportedInputMode(MSIMESystemInputModeState &state, id value) {
    if (!MSIMEIsInputModeID(value)) return NO;
    const BOOL changed = ![value isEqualToString:state.current];
    state.current = [value copy];
    return changed && !state.selecting;
}

// Whether the system offers a mode for selection. A mode the user removed in System Settings, or one an install from before the mode existed has not registered yet, cannot be selected, and asking for it would leave `current` naming a mode the menu bar does not show - the next report of the real one would then flip the controller's state back.
using MSIMEInputModeAvailability = BOOL (*)(NSString *identifier);

// The scheme a mode the user picked moves to, or nil to leave the scheme alone. 中 goes back to the Chinese scheme japanese, korean or vietnamese was entered from, or keeps the Chinese scheme already there - unless that scheme has a mode of its own the system offers: then 中 was picked over 双, 五, 粤 or 注 and means quanpin, and keeping the scheme would select that mode straight back.
static inline NSString *MSIMESchemeForReportedInputMode(MSIMEInputMode mode, NSString *scheme, NSString *lastChineseScheme,
                                                        MSIMEInputModeAvailability available) {
    if (mode != MSIMEInputMode::Chinese) return MSIMESchemeForInputMode(mode);
    const BOOL returning = [@[@"japanese", @"korean", @"vietnamese"] containsObject:scheme];
    NSString *chinese = returning ? lastChineseScheme : scheme;
    const MSIMEInputMode own = MSIMEInputModeFor(NO, chinese);
    if (own != MSIMEInputMode::Chinese && available && available(MSIMEInputModeID(own))) return @"quanpin";
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
static inline NSString *MSIMEOptInInputModeToEnable(NSString *previous, NSString *scheme, BOOL available) {
    if ([previous isEqualToString:scheme] || !available) return nil;
    NSString *mode = MSIMEInputModeID(MSIMEInputModeFor(NO, scheme));
    return MSIMEIsOptInInputModeID(mode) ? mode : nil;
}
