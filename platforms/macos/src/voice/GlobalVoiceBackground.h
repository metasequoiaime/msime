#pragma once
#import <Foundation/Foundation.h>
#import <ServiceManagement/ServiceManagement.h>

// 只管理注册生命周期，不监听键盘、不加载用户输入。所有入口及完成回调都在主线程。
@interface MSIMEGlobalVoiceBackground : NSObject
@property(nonatomic, readonly) BOOL busy;
@property(nonatomic, readonly) NSError *error;
- (void)configureService:(SMAppService *)service enabled:(BOOL)enabled revision:(NSString *)revision
               defaults:(NSUserDefaults *)defaults changed:(void (^)(void))changed;
@end
NSString *MSIMEGlobalVoiceBackgroundRevision(NSBundle *bundle);
