#pragma once

#import "MSIMEClientSession.h"

typedef NSDictionary *(*MSIMEPreferenceEditsMerge)(NSDictionary *preferences, NSDictionary *edits);

/// A preference revision must be an exact, non-negative integer rather than a bridged boolean or float.
static inline BOOL MSIMEStrictPreferenceRevision(id value, uint64_t *result) {
    if (![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID() || CFNumberIsFloatType((__bridge CFNumberRef)value)) return NO;
    NSNumber *number = (NSNumber *)value;
    if ([number compare:@0] == NSOrderedAscending) return NO;
    uint64_t revision = number.unsignedLongLongValue;
    if ([number compare:@(revision)] != NSOrderedSame) return NO;
    if (result) *result = revision;
    return YES;
}

/// Save only the caller's edits. If another writer advanced the revision, rebase once onto its snapshot.
/// A save rejected at the same revision is not retried. This performs synchronous I/O.
static inline NSDictionary *MSIMESavePreferenceEdits(NSString *directory, NSDictionary *snapshot,
                                                     NSDictionary *edits, MSIMEPreferenceEditsMerge merge) {
    NSMutableDictionary *next = [snapshot mutableCopy];
    next[@"preferences"] = merge(snapshot[@"preferences"], edits);
    uint64_t revision = 0;
    if (!MSIMEStrictPreferenceRevision(snapshot[@"revision"], &revision)) return nil;
    NSDictionary *saved = [MSIMEClientSession savePreferencesInDirectory:directory expectedRevision:revision snapshot:next error:nil];
    if (saved) return saved;
    NSDictionary *latest = [MSIMEClientSession loadPreferencesInDirectory:directory error:nil];
    uint64_t latestRevision = 0;
    if (!latest || !MSIMEStrictPreferenceRevision(latest[@"revision"], &latestRevision) || latestRevision == revision) return nil;
    next = [latest mutableCopy];
    next[@"preferences"] = merge(latest[@"preferences"], edits);
    return [MSIMEClientSession savePreferencesInDirectory:directory expectedRevision:latestRevision snapshot:next error:nil];
}
