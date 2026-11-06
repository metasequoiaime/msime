#pragma once
#import "GlobalVoiceCoordinator.h"

// 原生宿主内的独立录音会话；IMK 只在最终文本交付时参与。
@interface MSIMEGlobalVoiceSession : NSObject <MSIMEGlobalVoiceTarget>
@end
