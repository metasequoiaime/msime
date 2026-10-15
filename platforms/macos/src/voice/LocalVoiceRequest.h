#pragma once
#import <Foundation/Foundation.h>
#import "DoubaoVoiceRequest.h"

NS_ASSUME_NONNULL_BEGIN

// Whether `path` is an installed on-device model: a directory the model installer finished, which it marks by writing msime-model.json last. A Whisper model file, a half-installed directory or a path the user has since moved is not one.
FOUNDATION_EXPORT BOOL MSIMELocalVoiceModelDirectory(NSString * _Nullable path);

// The msime-voice-local helper this process spawns: MSIME_VOICE_LOCAL_HELPER when set (tests point it at a build tree), otherwise the copy bundled beside the input method's executable in Contents/MacOS. Nil when neither is an executable file.
FOUNDATION_EXPORT NSString * _Nullable MSIMELocalVoiceHelperPath(void);

// On-device streaming recognition through the msime-voice-local helper process (shared/voice/LocalAsrHelper.cpp). The input method runs inside every app that takes text, so the speech model and the sherpa-onnx runtime live in the helper, never in this process; one helper serves every request and exits by itself once idle.
//
// Options are the controller's voice query: `asr_model_path` must be an installed model directory and `language` is passed through. `hostOptions` are the session's host options, used to read the user's dictionary words as hotwords: given to the model for models that take them natively, and applied to the final text through client-providers' pinyin correction for models whose manifest says `"hotwords": "pinyin"`. Single use; callbacks run on main.
@interface MSIMELocalVoiceRequest : NSObject <MSIMEStreamingVoiceRequest>
- (nullable instancetype)initWithOptions:(NSDictionary *)options
                             hostOptions:(nullable NSDictionary *)hostOptions
                                   error:(NSError **)error;
@end

NS_ASSUME_NONNULL_END
