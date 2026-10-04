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
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Wubi, @"japanese", @"quanpin", Available) isEqualToString:@"wubi"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Shuangpin, @"quanpin", nil, Available) isEqualToString:@"shuangpin"] &&
                    MSIMESchemeForReportedInputMode(MSIMEInputMode::English, @"wubi", nil, Available) == nil,
                "A mode other than 中 did not select its own scheme.");
        // 中 keeps quanpin, and returns from 日 or 한 to the Chinese scheme they were entered from.
        require(MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"quanpin", nil, Available) == nil &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"korean", @"quanpin", Available) isEqualToString:@"quanpin"],
                "中 did not keep quanpin or return to it.");
        // With 双 and 五 offered, 中 was picked over them and means quanpin, even when 日 was entered from shuangpin.
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"shuangpin", nil, Available) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"wubi", nil, Available) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"japanese", @"shuangpin", Available) isEqualToString:@"quanpin"],
                "中 picked over an offered 双 or 五 did not move to quanpin.");
        // Without them, 中 is what shuangpin and wubi show, so picking it keeps the scheme or returns to it.
        gShuangpinModeEnabled = NO;
        gWubiModeEnabled = NO;
        require(MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"shuangpin", nil, Available) == nil &&
                    MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"wubi", nil, Available) == nil &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"japanese", @"shuangpin", Available) isEqualToString:@"shuangpin"],
                "中 standing in for a Shuangpin or Wubi mode the user removed moved the scheme.");
        gShuangpinModeEnabled = YES;
        gWubiModeEnabled = YES;

        MSIMESystemInputModeState state;
        InputModeRecordingClient *client = [InputModeRecordingClient new];
        client.state = &state;
        client.echoes = YES;

        // Activation with nothing recorded selects the stored state, and the system's synchronous echo is not a new choice.
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
        MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
        require(MSIMESharedSystemInputModeState().current == nil, "The shared record did not reset.");
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
            require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, scheme, scheme, Available) isEqualToString:scheme],
                    "中 in a single-language edition left its only scheme.");
        gJapaneseModeEnabled = YES;
        gEnglishModeEnabled = YES;

        // 中 from Vietnamese goes back to the Chinese scheme it was entered from, like Japanese and Korean; 中 picked over 粤, 注 or 笔 means quanpin, like over 双 or 五.
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"vietnamese", @"wubi", Available) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"vietnamese", @"quanpin", Available) isEqualToString:@"quanpin"],
                "中 from Vietnamese did not leave it.");
        gWubiModeEnabled = NO;
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"vietnamese", @"wubi", Available) isEqualToString:@"wubi"],
                "中 from Vietnamese did not return to the Chinese scheme it was entered from.");
        gWubiModeEnabled = YES;
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"cantonese", @"quanpin", Available) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"zhuyin", @"quanpin", Available) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"stroke", @"quanpin", Available) isEqualToString:@"quanpin"],
                "中 picked over 粤, 注 or 笔 did not move to quanpin.");
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Stroke, @"quanpin", @"quanpin", Available) isEqualToString:@"stroke"],
                "The Stroke mode did not select its scheme.");
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Vietnamese, @"quanpin", @"quanpin", Available) isEqualToString:@"vietnamese"],
                "The Vietnamese mode did not select its scheme.");
        // 藏文和越南文一样不是中文方案：从藏文选 中 回到进入前的中文方案，选 藏 切到藏文。
        gWubiModeEnabled = NO;
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"tibetan", @"wubi", Available) isEqualToString:@"wubi"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"tibetan", @"quanpin", Available) isEqualToString:@"quanpin"],
                "中 from Tibetan did not return to the Chinese scheme it was entered from.");
        gWubiModeEnabled = YES;
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Tibetan, @"quanpin", @"quanpin", Available) isEqualToString:@"tibetan"],
                "The Tibetan mode did not select its scheme.");

        // Cantonese, Zhuyin and Stroke are available only with their dictionary in the HostOptions language_dictionaries directory; everything else needs nothing more.
        NSString *directory = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        [NSFileManager.defaultManager createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:nil];
        [NSData.data writeToFile:[directory stringByAppendingPathComponent:@"cantonese.db"] atomically:YES];
        [NSFileManager.defaultManager createDirectoryAtPath:[directory stringByAppendingPathComponent:@"zhuyin.db"] withIntermediateDirectories:YES attributes:nil error:nil];
        NSDictionary *hostOptions = @{@"language_dictionaries": directory};
        require(!MSIMEInputSchemeAvailable(@"stroke", hostOptions) && [MSIMEEffectiveInputScheme(@"stroke", @"wubi", hostOptions) isEqualToString:@"wubi"],
                "Stroke was available without stroke.db.");
        [NSData.data writeToFile:[directory stringByAppendingPathComponent:@"stroke.db"] atomically:YES];
        require(MSIMEInputSchemeAvailable(@"stroke", hostOptions) && [MSIMEEffectiveInputScheme(@"stroke", @"wubi", hostOptions) isEqualToString:@"stroke"] &&
                    [MSIMEEffectiveInputScheme(@"japanese", @"stroke", hostOptions) isEqualToString:@"japanese"] &&
                    [MSIMEEffectiveInputScheme(@"stroke", @"zhuyin", @{}) isEqualToString:@"quanpin"],
                "stroke.db in language_dictionaries did not make Stroke available.");
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

        // 设置应用按需下载的语言词典包（<preferences_directory>/resource-packs/language-dictionaries）同样让方案可用：只认带 msime-model.json 的真实目录里的普通文件，preferences_directory 必须是绝对路径。
        NSFileManager *files = NSFileManager.defaultManager;
        NSString *stateRoot = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSString *pack = [stateRoot stringByAppendingPathComponent:@"resource-packs/language-dictionaries"];
        [files createDirectoryAtPath:pack withIntermediateDirectories:YES attributes:nil error:nil];
        [NSData.data writeToFile:[pack stringByAppendingPathComponent:@"cantonese.db"] atomically:YES];
        NSDictionary *packOptions = @{@"preferences_directory": stateRoot};
        require(!MSIMEInputSchemeAvailable(@"cantonese", packOptions),
                "A language dictionary pack without msime-model.json made Cantonese available.");
        [@"{}" writeToFile:[pack stringByAppendingPathComponent:@"msime-model.json"] atomically:YES encoding:NSUTF8StringEncoding error:nil];
        require(MSIMEInputSchemeAvailable(@"cantonese", packOptions) && !MSIMEInputSchemeAvailable(@"zhuyin", packOptions) &&
                    !MSIMEInputSchemeAvailable(@"stroke", packOptions),
                "A downloaded language dictionary pack did not make exactly Cantonese available.");
        [NSData.data writeToFile:[pack stringByAppendingPathComponent:@"stroke.db"] atomically:YES];
        require(MSIMEInputSchemeAvailable(@"stroke", packOptions) && [MSIMEEffectiveInputScheme(@"stroke", @"wubi", packOptions) isEqualToString:@"stroke"],
                "stroke.db in the downloaded pack did not make Stroke available.");
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
        [files moveItemAtPath:[pack stringByAppendingPathComponent:@"cantonese.db"] toPath:target error:nil];
        [files createSymbolicLinkAtPath:[pack stringByAppendingPathComponent:@"cantonese.db"] withDestinationPath:target error:nil];
        require(!MSIMEInputSchemeAvailable(@"cantonese", packOptions),
                "A symlinked cantonese.db in the downloaded pack made Cantonese available.");
        [files removeItemAtPath:stateRoot error:nil];

        // 资源包父目录是符号链接时也不能把外部词库当作已安装资源。
        NSString *linkedStateRoot = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSString *linkedOutside = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSString *linkedOutsidePack = [linkedOutside stringByAppendingPathComponent:@"resource-packs/language-dictionaries"];
        [files createDirectoryAtPath:linkedOutsidePack withIntermediateDirectories:YES attributes:nil error:nil];
        [NSData.data writeToFile:[linkedOutsidePack stringByAppendingPathComponent:@"cantonese.db"] atomically:YES];
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
