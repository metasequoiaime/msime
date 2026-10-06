#pragma once
#import <AppKit/AppKit.h>
NS_ASSUME_NONNULL_BEGIN
FOUNDATION_EXPORT NSNotificationName const MSIMEVoiceProviderSettingsDidChangeNotification;
FOUNDATION_EXPORT NSArray<NSString *> *MSIMEVoiceASRProviderIDs(void);
FOUNDATION_EXPORT NSArray<NSString *> *MSIMEVoiceASRProviderTitles(void);
FOUNDATION_EXPORT NSString *MSIMEVoiceASRProviderDefaultEndpoint(NSString *provider);
FOUNDATION_EXPORT NSString *MSIMEVoiceASRProviderDefaultModel(NSString *provider);
FOUNDATION_EXPORT BOOL MSIMEVoiceASRProviderUsesService(NSString *provider);
@interface MetasequoiaVoiceProviderSettings : NSObject
@property(nonatomic, copy) NSString *provider;
@property(nonatomic, copy) NSString *endpoint;
@property(nonatomic, copy) NSString *model;
@property(nonatomic, copy) NSString *token;
@property(nonatomic, copy) NSDictionary<NSString *, NSString *> *tokenSlots;
@property(nonatomic, copy) NSString *modelPath;
@property(nonatomic) BOOL polishEnabled;
@property(nonatomic, copy) NSString *polishEndpoint;
@property(nonatomic, copy) NSString *polishModel;
@property(nonatomic, copy) NSString *polishToken;
/// Which of the seven polish presets is in force, and the three slots a custom preset stores its own wording in. A built-in preset uses its own text, as does a custom preset whose slot is empty.
@property(nonatomic, copy) NSString *polishPromptID;
@property(nonatomic, copy) NSString *polishPromptCustom1;
@property(nonatomic, copy) NSString *polishPromptCustom2;
@property(nonatomic, copy) NSString *polishPromptCustom3;
/// CoreAudio device UID; an empty value means the system default input.
@property(nonatomic, copy) NSString *captureDevice;
+ (instancetype)loadSettings;
- (BOOL)validate:(NSError **)error;
- (BOOL)save:(NSError **)error;
@end
/// The voice form. It is the 语音输入 page of the settings window and the contents of the
/// standalone window the input method's toolbar opens, so both show the same controls.
/// Every change is stored as it is made; there is no confirmation step.
@interface MetasequoiaVoiceProviderSettingsView : NSView
/// Fills the controls from storage. Call it whenever the form becomes visible.
- (void)reloadSettings;
@end

@interface MetasequoiaVoiceProviderSettingsWindow : NSWindowController
+ (instancetype)sharedController;
- (void)showAndActivate;
@end
NS_ASSUME_NONNULL_END
