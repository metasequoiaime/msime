#pragma once
#import <Foundation/Foundation.h>
#import "EditionIdentity.h"

// 钥匙串服务名随版本而变（full 是 com.metasequoia.msime.account），同时安装的版本各自登录、各自退出，互不删对方的令牌。刷新令牌在它加 `.refresh` 的服务名下。
static inline NSString *MSIMEKeychainRefreshService(void) { return [MSIMEKeychainService() stringByAppendingString:@".refresh"]; }
FOUNDATION_EXPORT NSString *MSIMEKeychainToken(NSString *accountID, NSError **error);
FOUNDATION_EXPORT BOOL MSIMEStoreKeychainToken(NSString *accountID, NSString *token, NSError **error);
FOUNDATION_EXPORT BOOL MSIMERemoveKeychainToken(NSString *accountID, NSError **error);
FOUNDATION_EXPORT NSString *MSIMEKeychainRefreshToken(NSString *accountID, NSError **error);
FOUNDATION_EXPORT BOOL MSIMEStoreKeychainRefreshToken(NSString *accountID, NSString *token, NSError **error);
