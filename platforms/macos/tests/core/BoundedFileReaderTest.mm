#import <Foundation/Foundation.h>
#import "../../src/core/BoundedFileReader.h"

#include <cassert>

int main() {
    @autoreleasepool {
        NSFileManager *files = NSFileManager.defaultManager;
        NSURL *root = [NSURL fileURLWithPath:[NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString]];
        assert([files createDirectoryAtURL:root withIntermediateDirectories:YES attributes:nil error:nil]);
        NSURL *exact = [root URLByAppendingPathComponent:@"exact.txt"];
        NSData *exactBytes = [NSMutableData dataWithLength:65536];
        assert([exactBytes writeToURL:exact options:NSDataWritingAtomic error:nil]);
        NSError *error = nil;
        NSData *read = MSIMEReadFileUpTo(exact, 65536, &error);
        assert(read.length == 65536 && error == nil);

        NSURL *oversized = [root URLByAppendingPathComponent:@"oversized.txt"];
        NSData *oversizedBytes = [NSMutableData dataWithLength:65537];
        assert([oversizedBytes writeToURL:oversized options:NSDataWritingAtomic error:nil]);
        error = nil;
        assert(MSIMEReadFileUpTo(oversized, 65536, &error) == nil && error != nil);

        NSURL *missing = [root URLByAppendingPathComponent:@"missing.txt"];
        error = nil;
        assert(MSIMEReadFileUpTo(missing, 65536, &error) == nil && error != nil);
        [files removeItemAtURL:root error:nil];
    }
    return 0;
}
