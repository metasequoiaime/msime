#import <Foundation/Foundation.h>

static inline BOOL MSIMEClipboardStrictRevision(id value, uint64_t *result) {
    if (![value isKindOfClass:NSNumber.class] ||
        CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID() ||
        CFNumberIsFloatType((__bridge CFNumberRef)value)) return NO;
    NSNumber *number = (NSNumber *)value;
    if ([number compare:@0] == NSOrderedAscending) return NO;
    uint64_t revision = number.unsignedLongLongValue;
    if ([number compare:@(revision)] != NSOrderedSame) return NO;
    if (result) *result = revision;
    return YES;
}

// Only change this preference. The supplied save must enforce the loaded revision.
static inline NSDictionary *MSIMEEnableClipboardHistory(
    NSDictionary *(^load)(void),
    NSDictionary *(^save)(uint64_t, NSDictionary *)) {
    NSDictionary *current = load();
    uint64_t revision = 0;
    if (![current isKindOfClass:NSDictionary.class] ||
        !MSIMEClipboardStrictRevision(current[@"revision"], &revision) ||
        ![current[@"preferences"] isKindOfClass:NSDictionary.class]) return @{ @"error": @YES };
    NSDictionary *preferences = current[@"preferences"];
    if (![preferences[@"clipboard_history"] isKindOfClass:NSNumber.class]) return @{ @"error": @YES };
    if ([preferences[@"clipboard_history"] boolValue]) return @{ @"enabled": @YES };
    NSMutableDictionary *next = [current mutableCopy];
    NSMutableDictionary *values = [preferences mutableCopy];
    values[@"clipboard_history"] = @YES;
    next[@"preferences"] = values;
    NSDictionary *result = save(revision, next);
    if (![result[@"preferences"] isKindOfClass:NSDictionary.class] ||
        ![result[@"preferences"][@"clipboard_history"] isEqual:@YES]) return @{ @"error": @YES };
    return @{ @"enabled": @YES };
}
