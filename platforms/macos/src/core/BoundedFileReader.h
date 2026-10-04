#import <Foundation/Foundation.h>

/// Reads at most `maximumBytes` from a local file without allocating for an
/// untrusted file's full size. A file with even one additional byte is refused.
FOUNDATION_EXPORT NSData *MSIMEReadFileUpTo(NSURL *url, NSUInteger maximumBytes, NSError **error);
