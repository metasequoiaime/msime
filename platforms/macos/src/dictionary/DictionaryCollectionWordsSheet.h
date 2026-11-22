#pragma once

#import <AppKit/AppKit.h>
#include <nlohmann/json.hpp>

using MSIMECollectionWordsPageRequest = void (^)(NSUInteger);
using MSIMECollectionWordsRemoval = void (^)(const nlohmann::json &);

@interface MSIMEDictionaryCollectionWordsSheet : NSWindowController <NSTableViewDataSource, NSTableViewDelegate>
- (instancetype)initWithPageRequest:(MSIMECollectionWordsPageRequest)request
                            removal:(MSIMECollectionWordsRemoval)removal;
- (void)showPage:(const nlohmann::json &)page;
- (void)setBusy:(BOOL)busy message:(NSString *)message;
@end
