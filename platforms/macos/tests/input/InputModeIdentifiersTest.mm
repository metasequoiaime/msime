#import "../../src/input/InputModeIdentifiers.h"

#include <cstdio>
#include <cstdlib>

// Stands in for the client: records every selectInputMode: and, like the system, can report the new mode back from inside the call.
@interface InputModeRecordingClient : NSObject
@property(nonatomic, strong) NSMutableArray<NSString *> *selected;
@property(nonatomic) MSIMESystemInputModeState *state;
@property(nonatomic) BOOL echoes;
@property(nonatomic) BOOL echoAdopted;
- (void)selectInputMode:(NSString *)identifier;
@end

@implementation InputModeRecordingClient
- (instancetype)init {
    if ((self = [super init])) _selected = [NSMutableArray array];
    return self;
}
- (void)selectInputMode:(NSString *)identifier {
    [self.selected addObject:identifier];
    if (self.echoes && self.state && MSIMEAdoptReportedInputMode(*self.state, identifier)) self.echoAdopted = YES;
}
@end

namespace {
BOOL gEnglishModeEnabled = YES;
BOOL gJapaneseModeEnabled = YES;
BOOL gKoreanModeEnabled = YES;
BOOL gShuangpinModeEnabled = YES;
BOOL gWubiModeEnabled = YES;
BOOL Available(NSString *identifier) {
    if ([identifier isEqualToString:MSIMEShuangpinInputModeID]) return gShuangpinModeEnabled;
    if ([identifier isEqualToString:MSIMEWubiInputModeID]) return gWubiModeEnabled;
    if ([identifier isEqualToString:MSIMEJapaneseInputModeID]) return gJapaneseModeEnabled;
    if ([identifier isEqualToString:MSIMEKoreanInputModeID]) return gKoreanModeEnabled;
    return [identifier isEqualToString:MSIMEChineseInputModeID] || gEnglishModeEnabled;
}

void require(bool condition, const char *message) {
    if (!condition) {
        std::fprintf(stderr, "%s\n", message);
        std::exit(1);
    }
}
} // namespace

int main() {
    @autoreleasepool {
        // 版本身份：没有 MSIMEEdition 的 Info.plist（full 的，以及不是 bundle 的测试进程）每个值都是 full 今天的那个；声明了版本的取 plist 里的值。
        NSDictionary *wubi = @{@"MSIMEEdition": @"wubi", @"CFBundleIdentifier": @"app.msime.inputmethod.wubi",
                               @"MSIMEInputSchemes": @[@"wubi"], @"MSIMEDefaultScheme": @"wubi",
                               @"MSIMESettingsBundleIdentifier": @"app.msime.macos.wubi",
                               @"MSIMEKeychainService": @"com.metasequoia.msime.wubi.account", @"MSIMEWubiMixedPinyinDefault": @YES};
        require([MSIMEEditionIdentifierIn(@{}) isEqualToString:@"full"] && MSIMEEditionIsFullIn(@{@"CFBundleIdentifier": @"x"}) &&
                    [MSIMEInputMethodBundleIdentifierIn(@{@"CFBundleIdentifier": @"x"}) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME"] &&
                    [MSIMESettingsBundleIdentifierIn(@{}) isEqualToString:@"app.msime.macos"] &&
                    [MSIMEKeychainServiceIn(@{}) isEqualToString:@"com.metasequoia.msime.account"] &&
                    MSIMEEditionInputSchemesIn(@{}) == nil && [MSIMEEditionDefaultSchemeIn(@{}) isEqualToString:@"quanpin"] &&
                    !MSIMEEditionWubiMixedPinyinDefaultIn(@{@"MSIMEWubiMixedPinyinDefault": @YES}) &&
                    [MSIMEEditionNotificationNameIn(@{}, @"N") isEqualToString:@"N"],
                "An Info.plist without an edition did not read as full.");
        require([MSIMEEditionIdentifierIn(wubi) isEqualToString:@"wubi"] &&
                    [MSIMEInputMethodBundleIdentifierIn(wubi) isEqualToString:@"app.msime.inputmethod.wubi"] &&
                    [MSIMESettingsBundleIdentifierIn(wubi) isEqualToString:@"app.msime.macos.wubi"] &&
                    [MSIMEKeychainServiceIn(wubi) isEqualToString:@"com.metasequoia.msime.wubi.account"] &&
                    [MSIMEEditionInputSchemesIn(wubi) isEqualToArray:@[@"wubi"]] && [MSIMEEditionDefaultSchemeIn(wubi) isEqualToString:@"wubi"] &&
                    MSIMEEditionWubiMixedPinyinDefaultIn(wubi) && [MSIMEEditionNotificationNameIn(wubi, @"N") isEqualToString:@"N.wubi"],
                "The wubi edition's Info.plist did not give the wubi identity.");
        // 手写：full 和没写 MSIMEHandwriting 的版本都有；不提供中文方案的版本写了 false 就没有，full 不认这个键。
        require(MSIMEEditionOffersHandwritingIn(@{}) && MSIMEEditionOffersHandwritingIn(wubi) &&
                    MSIMEEditionOffersHandwritingIn(@{@"MSIMEHandwriting": @NO}) &&
                    !MSIMEEditionOffersHandwritingIn(@{@"MSIMEEdition": @"vietnamese", @"MSIMEHandwriting": @NO}) &&
                    MSIMEEditionOffersHandwritingIn(@{@"MSIMEEdition": @"pinyin", @"MSIMEHandwriting": @YES}),
                "Handwriting did not follow the edition's MSIMEHandwriting declaration.");
        // 语音服务凭据和使用统计目录：full 沿用今天的名字，其他版本各有各的，语音服务名正是卸载时删掉的 `<bundle id>.voice`。
        require([MSIMEVoiceProviderKeychainServiceIn(@{}) isEqualToString:@"app.msime.client.voice.providers"] &&
                    [MSIMEVoiceProviderKeychainServiceIn(wubi) isEqualToString:[wubi[@"CFBundleIdentifier"] stringByAppendingString:@".voice"]] &&
                    [MSIMEUsageReportingDirectoryNameIn(@{}) isEqualToString:@"MSIME/telemetry"] &&
                    [MSIMEUsageReportingDirectoryNameIn(wubi) isEqualToString:@"MSIME/wubi/telemetry"],
                "The voice provider keychain service or the usage reporting directory did not follow the edition.");
        // 卸载按 bundle 文件名找要移走的 bundle：五笔版只能是它自己的，名字缺了时宁可不卸载也不退回 full 的。
        NSMutableDictionary *named = [wubi mutableCopy];
        named[@"CFBundleExecutable"] = @"水杉五笔";
        named[@"CFBundleDisplayName"] = @"水杉五笔";
        require([MSIMEInputMethodBundleNameIn(@{}) isEqualToString:@"水杉输入法.app"] &&
                    [MSIMEInputMethodBundleNameIn(named) isEqualToString:@"水杉五笔.app"] &&
                    MSIMEInputMethodBundleNameIn(wubi) == nil &&
                    [MSIMEEditionDisplayNameIn(@{}) isEqualToString:@"水杉输入法"] &&
                    [MSIMEEditionDisplayNameIn(named) isEqualToString:@"水杉五笔"],
                "The bundle file name or the display name did not follow the edition.");
        require(MSIMEEditionIsFull() && MSIMEEditionOffersScheme(@"tibetan") && !MSIMEEditionOffersScheme(@"klingon") &&
                    [MSIMEEffectiveInputScheme(@"cantonese", @"klingon", @{}) isEqualToString:@"quanpin"],
                "The test process, which is full, did not offer every scheme or fall back to quanpin.");
        require([MSIMEInputModeID(MSIMEInputModeFor(NO, @"quanpin")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Hans"] &&
                    [MSIMEInputModeID(MSIMEInputModeFor(YES, @"quanpin")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Roman"] &&
                    [MSIMEInputModeID(MSIMEInputModeFor(NO, @"japanese")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Japanese"] &&
                    [MSIMEInputModeID(MSIMEInputModeFor(NO, @"korean")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Korean"],
                "The Chinese, English, Japanese and Korean states do not map to the modes Info.plist.in declares.");
        require([MSIMEInputModeID(MSIMEInputModeFor(NO, @"shuangpin")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Shuangpin"] &&
                    [MSIMEInputModeID(MSIMEInputModeFor(NO, @"wubi")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Wubi"],
                "The shuangpin and wubi schemes do not map to the modes Info.plist.in declares.");
        // 五笔版只声明 .Hans 和 .Roman，五笔的名字和图标在 .Hans 上：设置窗口检查菜单栏入口时不能去找一个不存在的 .Wubi，否则提示永远消不掉。full 每个方案仍是它自己的模式。
        NSDictionary *wubiModes = @{@"MSIMEEdition": @"wubi", @"ComponentInputModeDict": @{@"tsInputModeListKey": @{
            MSIMEChineseInputModeID: @{}, MSIMEEnglishInputModeID: @{}}}};
        require(MSIMEInputModeDeclaredIn(@{}, MSIMETibetanInputModeID) && MSIMEInputModeDeclaredIn(wubiModes, MSIMEChineseInputModeID) &&
                    !MSIMEInputModeDeclaredIn(wubiModes, MSIMEWubiInputModeID) && !MSIMEInputModeDeclaredIn(@{@"MSIMEEdition": @"wubi"}, MSIMEChineseInputModeID) &&
                    [MSIMEInputModeIDForSchemeIn(wubiModes, @"wubi") isEqualToString:MSIMEChineseInputModeID] &&
                    [MSIMEInputModeIDForSchemeIn(@{}, @"wubi") isEqualToString:MSIMEWubiInputModeID] &&
                    [MSIMEInputModeIDForSchemeIn(@{}, @"shuangpin") isEqualToString:MSIMEShuangpinInputModeID],
                "The settings window would look for a mode the edition does not declare.");
        // 菜单栏状态项（#7066）的图标与系统输入法菜单同一张：full 每个模式各有自己的，「英」是 English.tiff；五笔版没有 .Wubi，取 .Hans 上的五笔图标；缺项时回退到输入法本身的图标。
        NSDictionary *fullIcons = @{@"ComponentInputModeDict": @{@"tsInputModeListKey": @{
            MSIMEChineseInputModeID: @{@"tsInputModeMenuIconFileKey": @"MSIMEClientInputMethodMenuIconChinese.tiff"},
            MSIMEShuangpinInputModeID: @{@"tsInputModeMenuIconFileKey": @"MSIMEClientInputMethodMenuIconShuangpin.tiff"},
            MSIMEEnglishInputModeID: @{@"tsInputModeMenuIconFileKey": @"MSIMEClientInputMethodMenuIconEnglish.tiff"}}},
            @"tsInputMethodIconFileKey": @"MSIMEClientInputMethodMenuIcon.tiff"};
        NSDictionary *wubiIcons = @{@"MSIMEEdition": @"wubi", @"ComponentInputModeDict": @{@"tsInputModeListKey": @{
            MSIMEChineseInputModeID: @{@"tsInputModeMenuIconFileKey": @"MSIMEClientInputMethodMenuIconWubi.tiff"},
            MSIMEEnglishInputModeID: @{@"tsInputModeMenuIconFileKey": @"MSIMEClientInputMethodMenuIconEnglish.tiff"}}}};
        require([MSIMEInputModeMenuIconFileIn(fullIcons, MSIMEEnglishInputModeID) isEqualToString:@"MSIMEClientInputMethodMenuIconEnglish.tiff"] &&
                    [MSIMEInputModeMenuIconFileIn(fullIcons, MSIMEChineseInputModeID) isEqualToString:@"MSIMEClientInputMethodMenuIconChinese.tiff"] &&
                    [MSIMEInputModeMenuIconFileIn(fullIcons, MSIMEShuangpinInputModeID) isEqualToString:@"MSIMEClientInputMethodMenuIconShuangpin.tiff"] &&
                    [MSIMEInputModeMenuIconFileIn(wubiIcons, MSIMEWubiInputModeID) isEqualToString:@"MSIMEClientInputMethodMenuIconWubi.tiff"] &&
                    [MSIMEInputModeMenuIconFileIn(wubiIcons, MSIMEEnglishInputModeID) isEqualToString:@"MSIMEClientInputMethodMenuIconEnglish.tiff"],
                "The menu bar status item would not show the icon the input menu shows for the mode.");
        require([MSIMEInputModeMenuIconFileIn(@{@"ComponentInputModeDict": @{@"tsInputModeListKey": @{}}, @"tsInputMethodIconFileKey": @"Custom.tiff"}, MSIMEJapaneseInputModeID) isEqualToString:@"Custom.tiff"] &&
                    [MSIMEInputModeMenuIconFileIn(@{}, MSIMEEnglishInputModeID) isEqualToString:@"MSIMEClientInputMethodMenuIcon.tiff"] &&
                    [MSIMEInputModeMenuIconFileIn(@{@"ComponentInputModeDict": @[]}, nil) isEqualToString:@"MSIMEClientInputMethodMenuIcon.tiff"] &&
                    [MSIMEInputModeMenuIconFileIn(@{@"ComponentInputModeDict": @{@"tsInputModeListKey": @{MSIMEEnglishInputModeID: @{@"tsInputModeMenuIconFileKey": @""}}}}, MSIMEEnglishInputModeID) isEqualToString:@"MSIMEClientInputMethodMenuIcon.tiff"],
                "A bundle without the mode's icon left the status item without a fallback.");
        require(MSIMEInputModeFor(NO, @"quanpin") == MSIMEInputMode::Chinese && MSIMEInputModeFor(NO, nil) == MSIMEInputMode::Chinese,
                "Quanpin or an unset scheme did not show 中.");
        require(MSIMEInputModeFor(YES, @"japanese") == MSIMEInputMode::English && MSIMEInputModeFor(YES, @"korean") == MSIMEInputMode::English &&
                    MSIMEInputModeFor(YES, @"wubi") == MSIMEInputMode::English,
                "English mode over another scheme did not show 英.");
        for (NSString *identifier in @[MSIMEChineseInputModeID, MSIMEShuangpinInputModeID, MSIMEWubiInputModeID, MSIMEEnglishInputModeID,
                                       MSIMEJapaneseInputModeID, MSIMEKoreanInputModeID])
            require([MSIMEInputModeID(MSIMEInputModeForID(identifier)) isEqualToString:identifier],
                    "A mode identifier does not map back to the mode it names.");
        require(MSIMEInputModeForID(@"com.apple.keylayout.ABC") == MSIMEInputMode::Chinese,
                "An unknown identifier did not read as the Chinese mode.");
        require([MSIMESchemeForInputMode(MSIMEInputMode::Shuangpin) isEqualToString:@"shuangpin"] &&
                    [MSIMESchemeForInputMode(MSIMEInputMode::Wubi) isEqualToString:@"wubi"] &&
                    [MSIMESchemeForInputMode(MSIMEInputMode::Japanese) isEqualToString:@"japanese"] &&
                    [MSIMESchemeForInputMode(MSIMEInputMode::Korean) isEqualToString:@"korean"] &&
                    MSIMESchemeForInputMode(MSIMEInputMode::Chinese) == nil && MSIMESchemeForInputMode(MSIMEInputMode::English) == nil,
                "A mode selects the wrong scheme.");
        require(MSIMEIsInputModeID(MSIMEChineseInputModeID) && MSIMEIsInputModeID(MSIMEEnglishInputModeID) &&
                    MSIMEIsInputModeID(MSIMEJapaneseInputModeID) && MSIMEIsInputModeID(MSIMEKoreanInputModeID) &&
                    MSIMEIsInputModeID(MSIMEShuangpinInputModeID) && MSIMEIsInputModeID(MSIMEWubiInputModeID) &&
                    !MSIMEIsInputModeID(@"com.apple.keylayout.ABC") && !MSIMEIsInputModeID(@"app.msime.inputmethod.MetasequoiaIME") &&
                    !MSIMEIsInputModeID(@42) && !MSIMEIsInputModeID(nil),
                "Something other than this bundle's six modes was taken for one of them.");

        // 双, 五, 日 and 한 name their scheme and 英 leaves it alone, whatever scheme is behind them.
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Wubi, @"japanese", @"quanpin", Available, NO) isEqualToString:@"wubi"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Shuangpin, @"quanpin", nil, Available, NO) isEqualToString:@"shuangpin"] &&
                    MSIMESchemeForReportedInputMode(MSIMEInputMode::English, @"wubi", nil, Available, NO) == nil,
                "A mode other than 中 did not select its own scheme.");
        // 中 keeps quanpin, and returns from 日 or 한 to the Chinese scheme they were entered from.
        require(MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"quanpin", nil, Available, NO) == nil &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"korean", @"quanpin", Available, NO) isEqualToString:@"quanpin"],
                "中 did not keep quanpin or return to it.");
        // With 双 and 五 offered, 中 was picked over them and means quanpin, even when 日 was entered from shuangpin.
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"shuangpin", nil, Available, NO) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"wubi", nil, Available, NO) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"japanese", @"shuangpin", Available, NO) isEqualToString:@"quanpin"],
                "中 picked over an offered 双 or 五 did not move to quanpin.");
        // Without them, 中 is what shuangpin and wubi show, so picking it keeps the scheme or returns to it.
        gShuangpinModeEnabled = NO;
        gWubiModeEnabled = NO;
        require(MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"shuangpin", nil, Available, NO) == nil &&
                    MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"wubi", nil, Available, NO) == nil &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"japanese", @"shuangpin", Available, NO) isEqualToString:@"shuangpin"],
                "中 standing in for a Shuangpin or Wubi mode the user removed moved the scheme.");
        gShuangpinModeEnabled = YES;
        gWubiModeEnabled = YES;

        // #10199：从别的输入源切回水杉后的第一次报告是 中 时，用户没有在 中 和 双 之间挑过，即使 双、五 可用也保留中文方案，从 日 回来的照样回到进入前的中文方案；明确报告的 双、五、日 仍然选各自的方案。
        require(MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"shuangpin", nil, Available, YES) == nil &&
                    MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"wubi", nil, Available, YES) == nil &&
                    MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"quanpin", nil, Available, YES) == nil &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"japanese", @"shuangpin", Available, YES) isEqualToString:@"shuangpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Shuangpin, @"quanpin", nil, Available, YES) isEqualToString:@"shuangpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Japanese, @"shuangpin", nil, Available, YES) isEqualToString:@"japanese"],
                "中 reported on switching back to the input method moved the scheme off the one in use.");
        {
            // 合盖或锁屏到解锁之后几秒：系统恢复输入源的报告不当作用户的选择。
            MSIMESystemInputModeState state;
            require(!MSIMESystemInputModeRestoring(state, 100), "A fresh record was taken as restoring.");
            MSIMEHoldSystemInputModeRestore(state);
            require(MSIMESystemInputModeRestoring(state, 1e9), "Sleep or lock did not hold until unlock.");
            // 锁屏会切走，清空记录不能把它一起清掉；锁着屏时的报告也不结束它。
            MSIMEResetSystemInputModeState(state);
            MSIMEFinishSystemInputModeRestore(state);
            require(MSIMESystemInputModeRestoring(state, 1e9) && state.switchedIn,
                    "Switching away at the lock screen or a report while locked ended the hold.");
            MSIMEBeginSystemInputModeRestore(state, 100);
            require(MSIMESystemInputModeRestoring(state, 100 + MSIMESystemInputModeRestoreSeconds - 1) &&
                        !MSIMESystemInputModeRestoring(state, 100 + MSIMESystemInputModeRestoreSeconds),
                    "Unlocking did not keep the window for a few seconds and then close it.");
            MSIMEFinishSystemInputModeRestore(state);
            require(!MSIMESystemInputModeRestoring(state, 101), "The first report after unlocking did not close the window.");
        }
        // 和控制器的顺序一致：采纳之前读 switchedIn，采纳之后清掉它。
        auto report = [](MSIMESystemInputModeState &state, NSString *mode, NSString *scheme) -> NSString * {
            const BOOL switchedIn = state.switchedIn;
            if (!MSIMEAdoptReportedInputMode(state, mode)) return @"not adopted";
            state.switchedIn = false;
            return MSIMESchemeForReportedInputMode(MSIMEInputModeForID(mode), scheme, nil, Available, switchedIn) ?: @"kept";
        };
        {
            // 进程刚启动和离开水杉清空记录之后都算刚切进；清空时把标记一起恢复。
            MSIMESystemInputModeState state;
            require(state.switchedIn, "A fresh record was not marked as just switched in.");
            state.switchedIn = false;
            MSIMEAdoptReportedInputMode(state, MSIMEShuangpinInputModeID);
            MSIMEResetSystemInputModeState(state);
            require(state.switchedIn && state.current == nil, "Leaving the input method did not mark the record as just switched in.");
        }
        {
            // 报告先于激活校准：切回后第一次报告 中，方案不动，同步把菜单栏重新选回 双；之后在水杉里从 双 切到 中，才改用全拼。
            MSIMESystemInputModeState state;
            InputModeRecordingClient *client = [InputModeRecordingClient new];
            require([report(state, MSIMEChineseInputModeID, @"shuangpin") isEqualToString:@"kept"] && !state.switchedIn,
                    "The first 中 after switching back moved shuangpin to quanpin.");
            require(MSIMESelectSystemInputMode(state, MSIMEShuangpinInputModeID, client, Available) &&
                        [client.selected isEqualToArray:@[MSIMEShuangpinInputModeID]] && [state.current isEqualToString:MSIMEShuangpinInputModeID],
                    "Keeping shuangpin after switching back did not select 双 again.");
            require([report(state, MSIMEChineseInputModeID, @"shuangpin") isEqualToString:@"quanpin"],
                    "中 picked over 双 inside the input method did not move to quanpin.");
        }
        {
            // 激活校准先于报告：校准记下系统落在的 中，随后的同步选回 双，滞后到达的 中 报告虽然和记录不同、会被采纳，仍按切回处理而不改全拼。
            MSIMESystemInputModeState state;
            InputModeRecordingClient *client = [InputModeRecordingClient new];
            require(MSIMERecordRestoredSystemInputMode(state, MSIMEChineseInputModeID) &&
                        MSIMESelectSystemInputMode(state, MSIMEShuangpinInputModeID, client, Available) && state.switchedIn,
                    "Calibration or the select after it cleared the switched-in mark.");
            require([report(state, MSIMEChineseInputModeID, @"shuangpin") isEqualToString:@"kept"],
                    "A 中 arriving after calibration had selected 双 moved shuangpin to quanpin.");
            require(MSIMESelectSystemInputMode(state, MSIMEShuangpinInputModeID, client, Available) &&
                        [report(state, MSIMEChineseInputModeID, @"shuangpin") isEqualToString:@"quanpin"],
                    "中 picked after the switch back had been handled did not move to quanpin.");
        }
        {
            // 校准记下的 中 被重复报告时不采纳，也不清掉标记：之后滞后的选择仍按切回处理，直到第一次采纳或按键。
            MSIMESystemInputModeState state;
            require(MSIMERecordRestoredSystemInputMode(state, MSIMEChineseInputModeID) &&
                        [report(state, MSIMEChineseInputModeID, @"shuangpin") isEqualToString:@"not adopted"] && state.switchedIn,
                    "A 中 repeating the restored mode was adopted as a choice.");
        }

        // 恢复到目标模式时先校准记录，通用同步不再重复请求；外部输入源和未知状态不触发自动选择。
        MSIMESystemInputModeState restored;
        InputModeRecordingClient *restoredClient = [InputModeRecordingClient new];
        require(!MSIMERecordRestoredSystemInputMode(restored, @"com.apple.keylayout.US") &&
                    !MSIMERecordRestoredSystemInputMode(restored, nil) && restored.current == nil,
                "尚未恢复到本输入法时不能记录或选择目标模式。");
        require(MSIMERecordRestoredSystemInputMode(restored, MSIMEChineseInputModeID) &&
                    !MSIMESelectSystemInputMode(restored, MSIMEChineseInputModeID, restoredClient, Available) &&
                    restoredClient.selected.count == 0,
                "系统已恢复到中文时不应重复请求选择中文。");
        require(MSIMESelectSystemInputMode(restored, MSIMEEnglishInputModeID, restoredClient, Available) &&
                    restoredClient.selected.count == 1,
                "校准恢复状态后仍应允许显式切换英文。");
        require(MSIMERecordRestoredSystemInputMode(restored, MSIMEChineseInputModeID) &&
                    MSIMESelectSystemInputMode(restored, MSIMEEnglishInputModeID, restoredClient, Available) &&
                    restoredClient.selected.count == 2,
                "实际恢复的模式与当前应用目标不同时仍应按规则切换。");
        restored.selecting = true;
        require(!MSIMERecordRestoredSystemInputMode(restored, MSIMEChineseInputModeID) &&
                    [restored.current isEqualToString:MSIMEEnglishInputModeID],
                "同步选择中的回调不能覆盖当前记录。");

        MSIMESystemInputModeState state;
        InputModeRecordingClient *client = [InputModeRecordingClient new];
        client.state = &state;
        client.echoes = YES;

        // 通用同步在没有记录时选择目标；同步回报不能当成用户的新选择。
        require(MSIMESelectSystemInputMode(state, MSIMEChineseInputModeID, client, Available) && [client.selected isEqualToArray:@[MSIMEChineseInputModeID]],
                "The first alignment did not select the Chinese mode.");
        require(!client.echoAdopted && !state.selecting, "The echo of the controller's own selection was adopted.");
        require(!MSIMESelectSystemInputMode(state, MSIMEChineseInputModeID, client, Available) && client.selected.count == 1,
                "Aligning to the mode already shown asked the client again.");

        // A Shift toggle into English selects the English mode once.
        require(MSIMESelectSystemInputMode(state, MSIMEEnglishInputModeID, client, Available) &&
                    [client.selected.lastObject isEqualToString:MSIMEEnglishInputModeID] && client.selected.count == 2,
                "Toggling into English did not select the English mode.");
        require(!client.echoAdopted, "The echo of the toggle flipped the state back.");
        require(!MSIMEAdoptReportedInputMode(state, MSIMEEnglishInputModeID),
                "A later report that only repeats the shown mode was adopted as a change.");

        // The user picks the Chinese entry from the input menu: adopt it, and the resulting state sync does not call back.
        require(MSIMEAdoptReportedInputMode(state, MSIMEChineseInputModeID), "A mode picked from the input menu was not adopted.");
        require(!MSIMESelectSystemInputMode(state, MSIMEChineseInputModeID, client, Available) && client.selected.count == 2,
                "Adopting a system-reported mode echoed it back through selectInputMode:.");

        // setValue:forTag: with the English mode turns English on without selecting it back.
        require(MSIMEAdoptReportedInputMode(state, MSIMEEnglishInputModeID) &&
                    MSIMEInputModeForID(MSIMEEnglishInputModeID) == MSIMEInputMode::English,
                "Reporting the English mode did not turn English on.");
        require(!MSIMESelectSystemInputMode(state, MSIMEEnglishInputModeID, client, Available) && client.selected.count == 2,
                "Turning English on from a report called selectInputMode: back.");

        // An unknown identifier is ignored and leaves the record alone.
        require(!MSIMEAdoptReportedInputMode(state, @"com.apple.keylayout.ABC") && !MSIMEAdoptReportedInputMode(state, nil) &&
                    [state.current isEqualToString:MSIMEEnglishInputModeID],
                "An unknown mode identifier was adopted or overwrote the record.");

        // A report delivered while the controller is itself selecting is recorded but not adopted.
        state.selecting = true;
        require(!MSIMEAdoptReportedInputMode(state, MSIMEChineseInputModeID) && [state.current isEqualToString:MSIMEChineseInputModeID],
                "A report from inside the controller's own switch was adopted.");
        state.selecting = false;

        // When the English mode is not enabled - removed in System Settings, or not yet registered - nothing is requested and the record keeps naming the mode actually shown.
        gEnglishModeEnabled = NO;
        require(!MSIMESelectSystemInputMode(state, MSIMEEnglishInputModeID, client, Available) && client.selected.count == 2 &&
                    [state.current isEqualToString:MSIMEChineseInputModeID],
                "An unavailable mode was requested or recorded as shown.");
        require(!MSIMEAdoptReportedInputMode(state, MSIMEChineseInputModeID),
                "With the English mode unavailable, a report of the Chinese mode flipped English off.");
        gEnglishModeEnabled = YES;

        // The japanese scheme selects 日 once, and a later pick of 日 from the menu after 中 is adopted.
        require(MSIMESelectSystemInputMode(state, MSIMEJapaneseInputModeID, client, Available) &&
                    [client.selected.lastObject isEqualToString:MSIMEJapaneseInputModeID] && client.selected.count == 3 &&
                    !client.echoAdopted,
                "Switching to the japanese scheme did not select the Japanese mode, or its echo was adopted.");
        require(MSIMEAdoptReportedInputMode(state, MSIMEChineseInputModeID) && MSIMEAdoptReportedInputMode(state, MSIMEJapaneseInputModeID),
                "Picking 中 and then 日 from the input menu was not adopted.");
        require(!MSIMESelectSystemInputMode(state, MSIMEJapaneseInputModeID, client, Available) && client.selected.count == 3,
                "Adopting a reported Japanese mode echoed it back through selectInputMode:.");
        MSIMEAdoptReportedInputMode(state, MSIMEEnglishInputModeID);

        // An install that has not registered the Japanese mode shows 中 for the japanese scheme rather than leaving 英 up.
        gJapaneseModeEnabled = NO;
        require(MSIMESelectSystemInputMode(state, MSIMEJapaneseInputModeID, client, Available) &&
                    [client.selected.lastObject isEqualToString:MSIMEChineseInputModeID] && client.selected.count == 4 &&
                    [state.current isEqualToString:MSIMEChineseInputModeID],
                "An unavailable Japanese mode did not fall back to the Chinese mode.");
        require(!MSIMESelectSystemInputMode(state, MSIMEJapaneseInputModeID, client, Available) && client.selected.count == 4,
                "The Chinese fallback for an unavailable Japanese mode asked the client again.");
        require(MSIMEAdoptReportedInputMode(state, MSIMEJapaneseInputModeID) &&
                    !MSIMESelectSystemInputMode(state, MSIMEJapaneseInputModeID, client, Available) && client.selected.count == 4,
                "A Japanese mode the system reported as shown was replaced by the Chinese fallback.");
        gJapaneseModeEnabled = YES;
        MSIMEAdoptReportedInputMode(state, MSIMEChineseInputModeID);

        // The korean scheme selects 한 once, and an install that has not registered the Korean mode shows 中 for it, as for Japanese.
        const NSUInteger beforeKorean = client.selected.count;
        require(MSIMESelectSystemInputMode(state, MSIMEKoreanInputModeID, client, Available) &&
                    [client.selected.lastObject isEqualToString:MSIMEKoreanInputModeID] && client.selected.count == beforeKorean + 1 &&
                    !client.echoAdopted,
                "Switching to the korean scheme did not select the Korean mode, or its echo was adopted.");
        require(!MSIMESelectSystemInputMode(state, MSIMEKoreanInputModeID, client, Available) && client.selected.count == beforeKorean + 1,
                "Aligning to the Korean mode already shown asked the client again.");
        MSIMEAdoptReportedInputMode(state, MSIMEEnglishInputModeID);
        gKoreanModeEnabled = NO;
        require(MSIMESelectSystemInputMode(state, MSIMEKoreanInputModeID, client, Available) &&
                    [client.selected.lastObject isEqualToString:MSIMEChineseInputModeID] && client.selected.count == beforeKorean + 2 &&
                    [state.current isEqualToString:MSIMEChineseInputModeID],
                "An unavailable Korean mode did not fall back to the Chinese mode.");
        require(MSIMEAdoptReportedInputMode(state, MSIMEKoreanInputModeID) &&
                    !MSIMESelectSystemInputMode(state, MSIMEKoreanInputModeID, client, Available) && client.selected.count == beforeKorean + 2,
                "A Korean mode the system reported as shown was replaced by the Chinese fallback.");
        gKoreanModeEnabled = YES;
        MSIMEAdoptReportedInputMode(state, MSIMEChineseInputModeID);

        // A Shuangpin or Wubi mode the user removed shows 中 for its scheme, and one still enabled is selected.
        const NSUInteger beforeShuangpin = client.selected.count;
        gShuangpinModeEnabled = NO;
        require(!MSIMESelectSystemInputMode(state, MSIMEShuangpinInputModeID, client, Available) && client.selected.count == beforeShuangpin &&
                    [state.current isEqualToString:MSIMEChineseInputModeID],
                "An unavailable Shuangpin mode did not stay on the Chinese mode.");
        gShuangpinModeEnabled = YES;
        require(MSIMESelectSystemInputMode(state, MSIMEShuangpinInputModeID, client, Available) &&
                    [client.selected.lastObject isEqualToString:MSIMEShuangpinInputModeID] && client.selected.count == beforeShuangpin + 1,
                "An added Shuangpin mode was not selected for the shuangpin scheme.");
        gWubiModeEnabled = NO;
        require(MSIMESelectSystemInputMode(state, MSIMEWubiInputModeID, client, Available) &&
                    [client.selected.lastObject isEqualToString:MSIMEChineseInputModeID] && client.selected.count == beforeShuangpin + 2,
                "An unavailable Wubi mode did not fall back to the Chinese mode.");
        gWubiModeEnabled = YES;

        // Leaving the input method clears the record, so picking the entry shown before leaving is adopted on the way back instead of being taken for an echo.
        require(MSIMEAdoptReportedInputMode(state, MSIMEEnglishInputModeID) && !MSIMEAdoptReportedInputMode(state, MSIMEEnglishInputModeID),
                "The English report before leaving was not recorded.");
        MSIMEResetSystemInputModeState(state);
        require(state.current == nil && !state.selecting, "Resetting left the record naming a mode.");
        require(MSIMEAdoptReportedInputMode(state, MSIMEEnglishInputModeID) && [state.current isEqualToString:MSIMEEnglishInputModeID],
                "A report after leaving the input method was not adopted.");
        MSIMESharedSystemInputModeState().switchedIn = false;
        MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
        require(MSIMESharedSystemInputModeState().current == nil && MSIMESharedSystemInputModeState().switchedIn,
                "The shared record did not reset.");
        MSIMEAdoptReportedInputMode(state, MSIMEChineseInputModeID);

        // 粤拼、注音、越南文、藏文和笔画各有自己的模式，每个模式都映射回自己的方案。
        for (NSString *scheme in @[@"cantonese", @"zhuyin", @"vietnamese", @"tibetan", @"stroke"]) {
            NSString *identifier = MSIMEInputModeID(MSIMEInputModeFor(NO, scheme));
            require(MSIMEIsInputModeID(identifier) && MSIMEIsOptInInputModeID(identifier) &&
                        [MSIMESchemeForInputMode(MSIMEInputModeForID(identifier)) isEqualToString:scheme] &&
                        MSIMEInputModeFor(YES, scheme) == MSIMEInputMode::English,
                    "A Cantonese, Zhuyin, Vietnamese, Tibetan or Stroke scheme does not round-trip through its opt-in mode.");
        }
        require([MSIMEInputModeID(MSIMEInputMode::Cantonese) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Cantonese"] &&
                    [MSIMEInputModeID(MSIMEInputMode::Zhuyin) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Zhuyin"] &&
                    [MSIMEInputModeID(MSIMEInputMode::Vietnamese) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Vietnamese"] &&
                    [MSIMEInputModeID(MSIMEInputMode::Tibetan) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Tibetan"] &&
                    [MSIMEInputModeID(MSIMEInputMode::Stroke) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Stroke"],
                "The new modes do not use the identifiers Info.plist.in declares.");
        require(!MSIMEIsOptInInputModeID(MSIMEChineseInputModeID) && !MSIMEIsOptInInputModeID(MSIMEKoreanInputModeID) && !MSIMEIsOptInInputModeID(nil),
                "A mode every install enables was treated as opt-in.");
        require([MSIMEInputSchemeNames() isEqualToArray:@[@"quanpin", @"shuangpin", @"wubi", @"japanese", @"korean", @"cantonese", @"zhuyin", @"vietnamese", @"tibetan", @"stroke"]],
                "The scheme names are not in the Engine's wire order.");
        // 菜单名和「添加」对话框里的语言与 InfoPlist.strings、Info.plist.in 和共享设置页的表一致。
        require([MSIMEInputModeMenuName(MSIMETibetanInputModeID) isEqualToString:@"水杉输入法 · 藏"] &&
                    [MSIMEInputModeAddDialogLanguage(MSIMETibetanInputModeID) isEqualToString:@"藏语"],
                "The Tibetan mode is named or grouped differently from the plist and the settings page.");
        // 笔画在输入菜单里叫「水杉输入法 · 笔」，在系统设置「添加」对话框的「简体中文」下。
        require([MSIMEInputModeMenuName(MSIMEStrokeInputModeID) isEqualToString:@"水杉输入法 · 笔"] &&
                    [MSIMEInputModeAddDialogLanguage(MSIMEStrokeInputModeID) isEqualToString:@"简体中文"],
                "The Stroke mode's menu name or Add dialog language is wrong.");
        // 日文、越南文、藏文版的 中（.Hans）和 英 登记在本版本的语言下，设置窗口按 bundle 里的 TISIntendedLanguage 说去哪个语言下添加；五笔版的仍在「简体中文」下。
        for (NSArray<NSString *> *edition in @[@[@"japanese", @"ja", @"日语"], @[@"vietnamese", @"vi", @"越南语"], @[@"tibetan", @"bo", @"藏语"],
                                               @[@"wubi", @"zh-Hans", @"简体中文"]]) {
            NSDictionary *info = @{@"MSIMEEdition": edition[0], @"ComponentInputModeDict": @{@"tsInputModeListKey": @{
                MSIMEChineseInputModeID: @{@"TISIntendedLanguage": edition[1]}, MSIMEEnglishInputModeID: @{@"TISIntendedLanguage": edition[1]}}}};
            require([MSIMEInputModeAddDialogLanguageIn(info, MSIMEChineseInputModeID) isEqualToString:edition[2]] &&
                        [MSIMEInputModeAddDialogLanguageIn(info, MSIMEEnglishInputModeID) isEqualToString:edition[2]],
                    "A single-language edition's modes are not looked for under its own language.");
        }
        require([MSIMEInputModeAddDialogLanguageIn(@{}, MSIMEEnglishInputModeID) isEqualToString:@"简体中文"] &&
                    [MSIMEInputModeAddDialogLanguageIn(@{}, MSIMEJapaneseInputModeID) isEqualToString:@"日语"],
                "full's Add dialog languages changed.");
        // 只有一个语言方案的版本：中（.Hans）就是这个方案的模式，方案自己的模式不存在。选中 中 留在这个方案上，不会去找一个本版本没有的中文方案。
        gJapaneseModeEnabled = NO;
        gEnglishModeEnabled = NO;
        for (NSString *scheme in @[@"japanese", @"vietnamese", @"tibetan"])
            require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, scheme, scheme, Available, NO) isEqualToString:scheme],
                    "中 in a single-language edition left its only scheme.");
        gJapaneseModeEnabled = YES;
        gEnglishModeEnabled = YES;

        // 中 from Vietnamese goes back to the Chinese scheme it was entered from, like Japanese and Korean; 中 picked over 粤, 注 or 笔 means quanpin, like over 双 or 五.
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"vietnamese", @"wubi", Available, NO) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"vietnamese", @"quanpin", Available, NO) isEqualToString:@"quanpin"],
                "中 from Vietnamese did not leave it.");
        gWubiModeEnabled = NO;
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"vietnamese", @"wubi", Available, NO) isEqualToString:@"wubi"],
                "中 from Vietnamese did not return to the Chinese scheme it was entered from.");
        gWubiModeEnabled = YES;
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"cantonese", @"quanpin", Available, NO) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"zhuyin", @"quanpin", Available, NO) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"stroke", @"quanpin", Available, NO) isEqualToString:@"quanpin"],
                "中 picked over 粤, 注 or 笔 did not move to quanpin.");
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Stroke, @"quanpin", @"quanpin", Available, NO) isEqualToString:@"stroke"],
                "The Stroke mode did not select its scheme.");
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Vietnamese, @"quanpin", @"quanpin", Available, NO) isEqualToString:@"vietnamese"],
                "The Vietnamese mode did not select its scheme.");
        // 藏文和越南文一样不是中文方案：从藏文选 中 回到进入前的中文方案，选 藏 切到藏文。
        gWubiModeEnabled = NO;
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"tibetan", @"wubi", Available, NO) isEqualToString:@"wubi"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"tibetan", @"quanpin", Available, NO) isEqualToString:@"quanpin"],
                "中 from Tibetan did not return to the Chinese scheme it was entered from.");
        gWubiModeEnabled = YES;
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Tibetan, @"quanpin", @"quanpin", Available, NO) isEqualToString:@"tibetan"],
                "The Tibetan mode did not select its scheme.");

        // Cantonese, Zhuyin and Stroke are available only with their dictionary in the HostOptions language_dictionaries directory; everything else needs nothing more.
        NSString *directory = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        [NSFileManager.defaultManager createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:nil];
        [NSData.data writeToFile:[directory stringByAppendingPathComponent:@"msime-cantonese.db"] atomically:YES];
        [NSFileManager.defaultManager createDirectoryAtPath:[directory stringByAppendingPathComponent:@"msime-zhuyin.db"] withIntermediateDirectories:YES attributes:nil error:nil];
        NSDictionary *hostOptions = @{@"language_dictionaries": directory};
        require(!MSIMEInputSchemeAvailable(@"stroke", hostOptions) && [MSIMEEffectiveInputScheme(@"stroke", @"wubi", hostOptions) isEqualToString:@"wubi"],
                "Stroke was available without msime-stroke.db.");
        [NSData.data writeToFile:[directory stringByAppendingPathComponent:@"msime-stroke.db"] atomically:YES];
        require(MSIMEInputSchemeAvailable(@"stroke", hostOptions) && [MSIMEEffectiveInputScheme(@"stroke", @"wubi", hostOptions) isEqualToString:@"stroke"] &&
                    [MSIMEEffectiveInputScheme(@"japanese", @"stroke", hostOptions) isEqualToString:@"japanese"] &&
                    [MSIMEEffectiveInputScheme(@"stroke", @"zhuyin", @{}) isEqualToString:@"quanpin"],
                "msime-stroke.db in language_dictionaries did not make Stroke available.");
        require(MSIMEInputSchemeAvailable(@"cantonese", hostOptions) && !MSIMEInputSchemeAvailable(@"zhuyin", hostOptions) &&
                    !MSIMEInputSchemeAvailable(@"cantonese", @{}) && !MSIMEInputSchemeAvailable(@"cantonese", nil) &&
                    MSIMEInputSchemeAvailable(@"vietnamese", nil) && MSIMEInputSchemeAvailable(@"tibetan", nil) && MSIMEInputSchemeAvailable(@"korean", @{}) &&
                    !MSIMEInputSchemeAvailable(@"pinyin", hostOptions) && !MSIMEInputSchemeAvailable(nil, hostOptions),
                "Scheme availability does not follow the installed language dictionaries.");
        require([MSIMEEffectiveInputScheme(@"cantonese", @"wubi", hostOptions) isEqualToString:@"cantonese"] &&
                    [MSIMEEffectiveInputScheme(@"zhuyin", @"wubi", hostOptions) isEqualToString:@"wubi"] &&
                    [MSIMEEffectiveInputScheme(@"zhuyin", @"cantonese", @{}) isEqualToString:@"quanpin"] &&
                    [MSIMEEffectiveInputScheme(@"zhuyin", nil, @{}) isEqualToString:@"quanpin"],
                "An unavailable scheme did not fall back the way host-api does.");
        [NSFileManager.defaultManager removeItemAtPath:directory error:nil];

        // 五笔要有码表 msime-wubi.db：资源目录里有，或者下载了 wubi 资源包（带 msime-model.json 的真实目录）。macOS 发布包不带它时两处都没有，五笔像缺词典的粤拼一样回退；HostOptions 没写资源目录时不判断。
        {
            NSFileManager *files = NSFileManager.defaultManager;
            NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
            NSString *resources = [root stringByAppendingPathComponent:@"EngineResources"];
            NSString *state = [root stringByAppendingPathComponent:@"state"];
            NSString *pack = [[state stringByAppendingPathComponent:@"resource-packs"] stringByAppendingPathComponent:@"wubi"];
            [files createDirectoryAtPath:resources withIntermediateDirectories:YES attributes:nil error:nil];
            [files createDirectoryAtPath:pack withIntermediateDirectories:YES attributes:nil error:nil];
            NSDictionary *slim = @{@"resources" : resources, @"preferences_directory" : state};
            require(!MSIMEInputSchemeAvailable(@"wubi", slim) && [MSIMEEffectiveInputScheme(@"wubi", nil, slim) isEqualToString:@"quanpin"] &&
                        MSIMEInputSchemeAvailable(@"quanpin", slim) && MSIMEInputSchemeAvailable(@"wubi", @{}) && MSIMEInputSchemeAvailable(@"wubi", nil),
                    "Wubi was available without msime-wubi.db, or needed one when HostOptions names no resources.");
            [NSData.data writeToFile:[pack stringByAppendingPathComponent:@"msime-wubi.db"] atomically:YES];
            require(!MSIMEInputSchemeAvailable(@"wubi", slim), "A wubi pack without msime-model.json made Wubi available.");
            [NSData.data writeToFile:[pack stringByAppendingPathComponent:@"msime-model.json"] atomically:YES];
            require(MSIMEInputSchemeAvailable(@"wubi", slim) && [MSIMEEffectiveInputScheme(@"wubi", nil, slim) isEqualToString:@"wubi"],
                    "The downloaded wubi pack did not make Wubi available.");
            [files removeItemAtPath:pack error:nil];
            [NSData.data writeToFile:[resources stringByAppendingPathComponent:@"msime-wubi.db"] atomically:YES];
            require(MSIMEInputSchemeAvailable(@"wubi", slim), "msime-wubi.db in the resource directory did not make Wubi available.");
            [files removeItemAtPath:root error:nil];
        }

        // 设置应用按需下载的语言词典包（<preferences_directory>/resource-packs/language-dictionaries）同样让方案可用：只认带 msime-model.json 的真实目录里的普通文件，preferences_directory 必须是绝对路径。
        NSFileManager *files = NSFileManager.defaultManager;
        NSString *stateRoot = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSString *pack = [stateRoot stringByAppendingPathComponent:@"resource-packs/language-dictionaries"];
        [files createDirectoryAtPath:pack withIntermediateDirectories:YES attributes:nil error:nil];
        [NSData.data writeToFile:[pack stringByAppendingPathComponent:@"msime-cantonese.db"] atomically:YES];
        NSDictionary *packOptions = @{@"preferences_directory": stateRoot};
        require(!MSIMEInputSchemeAvailable(@"cantonese", packOptions),
                "A language dictionary pack without msime-model.json made Cantonese available.");
        [@"{}" writeToFile:[pack stringByAppendingPathComponent:@"msime-model.json"] atomically:YES encoding:NSUTF8StringEncoding error:nil];
        require(MSIMEInputSchemeAvailable(@"cantonese", packOptions) && !MSIMEInputSchemeAvailable(@"zhuyin", packOptions) &&
                    !MSIMEInputSchemeAvailable(@"stroke", packOptions),
                "A downloaded language dictionary pack did not make exactly Cantonese available.");
        [NSData.data writeToFile:[pack stringByAppendingPathComponent:@"msime-stroke.db"] atomically:YES];
        require(MSIMEInputSchemeAvailable(@"stroke", packOptions) && [MSIMEEffectiveInputScheme(@"stroke", @"wubi", packOptions) isEqualToString:@"stroke"],
                "msime-stroke.db in the downloaded pack did not make Stroke available.");
        require([MSIMEEffectiveInputScheme(@"cantonese", @"wubi", packOptions) isEqualToString:@"cantonese"] &&
                    [MSIMEEffectiveInputScheme(@"cantonese", @"wubi", @{@"preferences_directory": stateRoot, @"language_dictionaries": @""}) isEqualToString:@"cantonese"],
                "The effective scheme ignored Cantonese from the downloaded pack.");
        // 相对路径即使能从当前目录解析到这个资源包也不认。
        NSString *previousDirectory = files.currentDirectoryPath;
        [files changeCurrentDirectoryPath:[stateRoot stringByDeletingLastPathComponent]];
        BOOL relativeAccepted = MSIMEInputSchemeAvailable(@"cantonese", @{@"preferences_directory": [stateRoot lastPathComponent]});
        [files changeCurrentDirectoryPath:previousDirectory];
        require(!relativeAccepted && !MSIMEInputSchemeAvailable(@"cantonese", @{@"preferences_directory": @""}) &&
                    !MSIMEInputSchemeAvailable(@"cantonese", @{@"preferences_directory": @42}),
                "A relative, empty or non-string preferences_directory was used to find a pack.");
        NSString *target = [stateRoot stringByAppendingPathComponent:@"cantonese-target.db"];
        [files moveItemAtPath:[pack stringByAppendingPathComponent:@"msime-cantonese.db"] toPath:target error:nil];
        [files createSymbolicLinkAtPath:[pack stringByAppendingPathComponent:@"msime-cantonese.db"] withDestinationPath:target error:nil];
        require(!MSIMEInputSchemeAvailable(@"cantonese", packOptions),
                "A symlinked msime-cantonese.db in the downloaded pack made Cantonese available.");
        [files removeItemAtPath:stateRoot error:nil];

        // 资源包父目录是符号链接时也不能把外部词库当作已安装资源。
        NSString *linkedStateRoot = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSString *linkedOutside = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSString *linkedOutsidePack = [linkedOutside stringByAppendingPathComponent:@"resource-packs/language-dictionaries"];
        [files createDirectoryAtPath:linkedOutsidePack withIntermediateDirectories:YES attributes:nil error:nil];
        [NSData.data writeToFile:[linkedOutsidePack stringByAppendingPathComponent:@"msime-cantonese.db"] atomically:YES];
        [@"{}" writeToFile:[linkedOutsidePack stringByAppendingPathComponent:@"msime-model.json"] atomically:YES encoding:NSUTF8StringEncoding error:nil];
        [files createDirectoryAtPath:linkedStateRoot withIntermediateDirectories:YES attributes:nil error:nil];
        [files createSymbolicLinkAtPath:[linkedStateRoot stringByAppendingPathComponent:@"resource-packs"]
                    withDestinationPath:[linkedOutside stringByAppendingPathComponent:@"resource-packs"] error:nil];
        require(!MSIMEInputSchemeAvailable(@"cantonese", @{ @"preferences_directory": linkedStateRoot }),
                "A symlinked resource-packs parent made Cantonese available.");
        [files removeItemAtPath:linkedStateRoot error:nil];
        [files removeItemAtPath:linkedOutside error:nil];

        // 方案切到粤、注、越、藏、笔时启用对应模式，第一次同步就落在这类方案上也启用；方案没变、方案跑不起来、或方案没有按需模式时都不启用。
        require([MSIMEOptInInputModeToEnable(@"quanpin", @"cantonese", YES) isEqualToString:MSIMECantoneseInputModeID] &&
                    [MSIMEOptInInputModeToEnable(@"korean", @"vietnamese", YES) isEqualToString:MSIMEVietnameseInputModeID] &&
                    [MSIMEOptInInputModeToEnable(@"vietnamese", @"tibetan", YES) isEqualToString:MSIMETibetanInputModeID] &&
                    [MSIMEOptInInputModeToEnable(nil, @"zhuyin", YES) isEqualToString:MSIMEZhuyinInputModeID] &&
                    [MSIMEOptInInputModeToEnable(@"wubi", @"stroke", YES) isEqualToString:MSIMEStrokeInputModeID] &&
                    !MSIMEOptInInputModeToEnable(@"wubi", @"stroke", NO) && !MSIMEOptInInputModeToEnable(@"stroke", @"stroke", YES) &&
                    !MSIMEOptInInputModeToEnable(nil, @"quanpin", YES) && !MSIMEOptInInputModeToEnable(@"zhuyin", @"zhuyin", YES) &&
                    !MSIMEOptInInputModeToEnable(@"quanpin", @"zhuyin", NO) && !MSIMEOptInInputModeToEnable(@"quanpin", @"wubi", YES),
                "An opt-in mode was enabled at the wrong time.");
        // 越南文、藏文版的 bundle 只声明 中（.Hans）和 英：方案自己的按需模式不存在，不去启用，第一次同步就能记下这个方案。
        for (NSString *scheme in @[@"vietnamese", @"tibetan"]) {
            NSDictionary *info = @{@"MSIMEEdition": scheme, @"ComponentInputModeDict": @{@"tsInputModeListKey": @{
                MSIMEChineseInputModeID: @{}, MSIMEEnglishInputModeID: @{}}}};
            require(!MSIMEOptInInputModeToEnableIn(info, nil, scheme, YES) && !MSIMEOptInInputModeToEnableIn(info, @"quanpin", scheme, YES),
                    "A single-language edition asked for an opt-in mode its bundle does not declare.");
        }

        // A client that cannot switch modes is left alone.
        require(!MSIMESelectSystemInputMode(state, MSIMEEnglishInputModeID, [NSObject new], Available) && [state.current isEqualToString:MSIMEChineseInputModeID],
                "A client without selectInputMode: was recorded as switched.");
        require(!MSIMESelectSystemInputMode(state, MSIMEEnglishInputModeID, nil, Available), "A missing client was asked to switch.");
    }
    std::puts("input mode identifiers keep the menu bar mode and the Chinese/English state and scheme in step");
    return 0;
}
