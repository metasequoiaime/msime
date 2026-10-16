#pragma once
#import <Foundation/Foundation.h>
#import "DoubaoVoiceRequest.h"

NS_ASSUME_NONNULL_BEGIN

// Whether `path` is an installed on-device model: a directory the model installer finished, which it marks by writing msime-model.json last. A Whisper model file, a half-installed directory or a path the user has since moved is not one.
FOUNDATION_EXPORT BOOL MSIMELocalVoiceModelDirectory(NSString * _Nullable path);

// The msime-voice-local helper this process spawns: MSIME_VOICE_LOCAL_HELPER when set (tests point it at a build tree), otherwise the copy bundled beside the input method's executable in Contents/MacOS. Nil when neither is an executable file.
FOUNDATION_EXPORT NSString * _Nullable MSIMELocalVoiceHelperPath(void);

// 通过 msime-voice-local 辅助进程（shared/voice/LocalAsrHelper.cpp）进行本机流式识别。输入法运行在每个接受文字输入的应用里，所以语音模型和 sherpa-onnx 运行时都放在辅助进程里，从不放进本进程；一个辅助进程服务所有请求，空闲后自行退出。
//
// `options` 是控制器的语音查询：`asr_model_path` 必须是已安装的模型目录，`language` 原样透传。`hostOptions` 是会话的宿主选项，用来读取用户的词库词作为热词：对原生支持热词的模型直接交给模型，对清单写明 `"hotwords": "pinyin"` 的模型则经 client-providers 的拼音纠错作用到最终文本上。只能使用一次；回调在主线程上执行。
@interface MSIMELocalVoiceRequest : NSObject <MSIMEStreamingVoiceRequest>
- (nullable instancetype)initWithOptions:(NSDictionary *)options
                             hostOptions:(nullable NSDictionary *)hostOptions
                                   error:(NSError **)error;
@end

NS_ASSUME_NONNULL_END
