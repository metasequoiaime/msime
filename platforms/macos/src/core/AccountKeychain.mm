#import "AccountKeychain.h"
#import <Security/Security.h>
NSString *MSIMEKeychainToken(NSString *accountID, NSError **error) {
    if (accountID.length == 0 || [accountID rangeOfCharacterFromSet:[NSCharacterSet controlCharacterSet]].location != NSNotFound) { if (error) *error = [NSError errorWithDomain:@"MSIMEAccount" code:400 userInfo:nil]; return nil; }
    NSDictionary *query = @{(__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword, (__bridge id)kSecAttrService: MSIMEKeychainService(), (__bridge id)kSecAttrAccount: accountID, (__bridge id)kSecReturnData: @YES};
    CFTypeRef result = NULL; OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &result);
    if (status != errSecSuccess) { if (error) *error = [NSError errorWithDomain:NSOSStatusErrorDomain code:status userInfo:nil]; return nil; }
    return [[NSString alloc] initWithData:CFBridgingRelease(result) encoding:NSUTF8StringEncoding];
}

BOOL MSIMEStoreKeychainToken(NSString *accountID, NSString *token, NSError **error) {
    if (accountID.length == 0 || token.length == 0 || [accountID rangeOfCharacterFromSet:[NSCharacterSet controlCharacterSet]].location != NSNotFound) { if (error) *error = [NSError errorWithDomain:@"MSIMEAccount" code:400 userInfo:nil]; return NO; }
    NSData *data = [token dataUsingEncoding:NSUTF8StringEncoding]; NSDictionary *query = @{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,(__bridge id)kSecAttrService:MSIMEKeychainService(),(__bridge id)kSecAttrAccount:accountID};
    OSStatus status = SecItemUpdate((__bridge CFDictionaryRef)query, (__bridge CFDictionaryRef)@{(__bridge id)kSecValueData:data});
    if (status == errSecItemNotFound) { NSMutableDictionary *item = [query mutableCopy]; item[(__bridge id)kSecValueData] = data; status = SecItemAdd((__bridge CFDictionaryRef)item, NULL); }
    if (status != errSecSuccess) { if (error) *error = [NSError errorWithDomain:@"MSIMEAccount" code:status userInfo:nil]; return NO; } return YES;
}

NSString *MSIMEKeychainRefreshToken(NSString *accountID, NSError **error) { if (!accountID.length) { if (error) *error = [NSError errorWithDomain:@"MSIMEAccount" code:400 userInfo:nil]; return nil; } NSDictionary *q=@{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,(__bridge id)kSecAttrService:MSIMEKeychainRefreshService(),(__bridge id)kSecAttrAccount:accountID,(__bridge id)kSecReturnData:@YES}; CFTypeRef value=NULL; OSStatus s=SecItemCopyMatching((__bridge CFDictionaryRef)q,&value); if(s!=errSecSuccess)return nil; return [[NSString alloc]initWithData:CFBridgingRelease(value) encoding:NSUTF8StringEncoding]; }
BOOL MSIMEStoreKeychainRefreshToken(NSString *accountID, NSString *token, NSError **error) { if(!accountID.length||!token.length){if(error)*error=[NSError errorWithDomain:@"MSIMEAccount" code:400 userInfo:nil];return NO;} NSDictionary*q=@{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,(__bridge id)kSecAttrService:MSIMEKeychainRefreshService(),(__bridge id)kSecAttrAccount:accountID}; NSData*d=[token dataUsingEncoding:NSUTF8StringEncoding]; OSStatus s=SecItemUpdate((__bridge CFDictionaryRef)q,(__bridge CFDictionaryRef)@{(__bridge id)kSecValueData:d}); if(s==errSecItemNotFound){NSMutableDictionary*i=[q mutableCopy];i[(__bridge id)kSecValueData]=d;s=SecItemAdd((__bridge CFDictionaryRef)i,NULL);} if(s!=errSecSuccess&&error)*error=[NSError errorWithDomain:@"MSIMEAccount" code:s userInfo:nil];return s==errSecSuccess;}

BOOL MSIMERemoveKeychainToken(NSString *accountID, NSError **error) {
    if (accountID.length == 0 || [accountID rangeOfCharacterFromSet:[NSCharacterSet controlCharacterSet]].location != NSNotFound) { if (error) *error = [NSError errorWithDomain:@"MSIMEAccount" code:400 userInfo:nil]; return NO; }
    NSDictionary *query = @{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,(__bridge id)kSecAttrService:MSIMEKeychainService(),(__bridge id)kSecAttrAccount:accountID};
    OSStatus status = SecItemDelete((__bridge CFDictionaryRef)query);
    if (status == errSecItemNotFound) status = errSecSuccess;
    if (status != errSecSuccess && error) *error = [NSError errorWithDomain:NSOSStatusErrorDomain code:status userInfo:nil];
    return status == errSecSuccess;
}
