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
        require([MSIMEInputModeID(MSIMEInputModeFor(NO, @"quanpin")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Hans"] &&
                    [MSIMEInputModeID(MSIMEInputModeFor(YES, @"quanpin")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Roman"] &&
                    [MSIMEInputModeID(MSIMEInputModeFor(NO, @"japanese")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Japanese"] &&
                    [MSIMEInputModeID(MSIMEInputModeFor(NO, @"korean")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Korean"],
                "The Chinese, English, Japanese and Korean states do not map to the modes Info.plist.in declares.");
        require([MSIMEInputModeID(MSIMEInputModeFor(NO, @"shuangpin")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Shuangpin"] &&
                    [MSIMEInputModeID(MSIMEInputModeFor(NO, @"wubi")) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Wubi"],
                "The shuangpin and wubi schemes do not map to the modes Info.plist.in declares.");
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

        // 粤拼、注音、越南文和藏文各有自己的模式，每个模式都映射回自己的方案。
        for (NSString *scheme in @[@"cantonese", @"zhuyin", @"vietnamese", @"tibetan"]) {
            NSString *identifier = MSIMEInputModeID(MSIMEInputModeFor(NO, scheme));
            require(MSIMEIsInputModeID(identifier) && MSIMEIsOptInInputModeID(identifier) &&
                        [MSIMESchemeForInputMode(MSIMEInputModeForID(identifier)) isEqualToString:scheme] &&
                        MSIMEInputModeFor(YES, scheme) == MSIMEInputMode::English,
                    "A Cantonese, Zhuyin, Vietnamese or Tibetan scheme does not round-trip through its opt-in mode.");
        }
        require([MSIMEInputModeID(MSIMEInputMode::Cantonese) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Cantonese"] &&
                    [MSIMEInputModeID(MSIMEInputMode::Zhuyin) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Zhuyin"] &&
                    [MSIMEInputModeID(MSIMEInputMode::Vietnamese) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Vietnamese"] &&
                    [MSIMEInputModeID(MSIMEInputMode::Tibetan) isEqualToString:@"app.msime.inputmethod.MetasequoiaIME.Tibetan"],
                "The new modes do not use the identifiers Info.plist.in declares.");
        require(!MSIMEIsOptInInputModeID(MSIMEChineseInputModeID) && !MSIMEIsOptInInputModeID(MSIMEKoreanInputModeID) && !MSIMEIsOptInInputModeID(nil),
                "A mode every install enables was treated as opt-in.");
        require([MSIMEInputSchemeNames() isEqualToArray:@[@"quanpin", @"shuangpin", @"wubi", @"japanese", @"korean", @"cantonese", @"zhuyin", @"vietnamese", @"tibetan"]],
                "The scheme names are not in the Engine's wire order.");
        // 菜单名和「添加」对话框里的语言与 InfoPlist.strings、Info.plist.in 和共享设置页的表一致。
        require([MSIMEInputModeMenuName(MSIMETibetanInputModeID) isEqualToString:@"水杉输入法 · 藏"] &&
                    [MSIMEInputModeAddDialogLanguage(MSIMETibetanInputModeID) isEqualToString:@"藏语"],
                "The Tibetan mode is named or grouped differently from the plist and the settings page.");

        // 中 from Vietnamese goes back to the Chinese scheme it was entered from, like Japanese and Korean; 中 picked over 粤 or 注 means quanpin, like over 双 or 五.
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"vietnamese", @"wubi", Available) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"vietnamese", @"quanpin", Available) isEqualToString:@"quanpin"],
                "中 from Vietnamese did not leave it.");
        gWubiModeEnabled = NO;
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"vietnamese", @"wubi", Available) isEqualToString:@"wubi"],
                "中 from Vietnamese did not return to the Chinese scheme it was entered from.");
        gWubiModeEnabled = YES;
        require([MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"cantonese", @"quanpin", Available) isEqualToString:@"quanpin"] &&
                    [MSIMESchemeForReportedInputMode(MSIMEInputMode::Chinese, @"zhuyin", @"quanpin", Available) isEqualToString:@"quanpin"],
                "中 picked over 粤 or 注 did not move to quanpin.");
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

        // Cantonese and Zhuyin are available only with their dictionary in the HostOptions language_dictionaries directory; everything else needs nothing more.
        NSString *directory = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        [NSFileManager.defaultManager createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:nil];
        [NSData.data writeToFile:[directory stringByAppendingPathComponent:@"cantonese.db"] atomically:YES];
        [NSFileManager.defaultManager createDirectoryAtPath:[directory stringByAppendingPathComponent:@"zhuyin.db"] withIntermediateDirectories:YES attributes:nil error:nil];
        NSDictionary *hostOptions = @{@"language_dictionaries": directory};
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
        require(MSIMEInputSchemeAvailable(@"cantonese", packOptions) && !MSIMEInputSchemeAvailable(@"zhuyin", packOptions),
                "A downloaded language dictionary pack did not make exactly Cantonese available.");
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

        // 方案切到粤、注、越、藏时启用对应模式，第一次同步就落在这类方案上也启用；方案没变、方案跑不起来、或方案没有按需模式时都不启用。
        require([MSIMEOptInInputModeToEnable(@"quanpin", @"cantonese", YES) isEqualToString:MSIMECantoneseInputModeID] &&
                    [MSIMEOptInInputModeToEnable(@"korean", @"vietnamese", YES) isEqualToString:MSIMEVietnameseInputModeID] &&
                    [MSIMEOptInInputModeToEnable(@"vietnamese", @"tibetan", YES) isEqualToString:MSIMETibetanInputModeID] &&
                    [MSIMEOptInInputModeToEnable(nil, @"zhuyin", YES) isEqualToString:MSIMEZhuyinInputModeID] &&
                    !MSIMEOptInInputModeToEnable(nil, @"quanpin", YES) && !MSIMEOptInInputModeToEnable(@"zhuyin", @"zhuyin", YES) &&
                    !MSIMEOptInInputModeToEnable(@"quanpin", @"zhuyin", NO) && !MSIMEOptInInputModeToEnable(@"quanpin", @"wubi", YES),
                "An opt-in mode was enabled at the wrong time.");

        // A client that cannot switch modes is left alone.
        require(!MSIMESelectSystemInputMode(state, MSIMEEnglishInputModeID, [NSObject new], Available) && [state.current isEqualToString:MSIMEChineseInputModeID],
                "A client without selectInputMode: was recorded as switched.");
        require(!MSIMESelectSystemInputMode(state, MSIMEEnglishInputModeID, nil, Available), "A missing client was asked to switch.");
    }
    std::puts("input mode identifiers keep the menu bar mode and the Chinese/English state and scheme in step");
    return 0;
}
