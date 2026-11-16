#import <Cocoa/Cocoa.h>

/// 独立的原生分组管理窗口，不扩展冻结的设置回退容器。
@interface MSIMEDictionaryCollectionsWindow : NSWindowController
+ (void)showWithOptions:(NSDictionary *)options;
@end
