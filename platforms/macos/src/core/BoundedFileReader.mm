#import "BoundedFileReader.h"

#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

static NSString *const MSIMEBoundedFileReaderError = @"app.msime.client.bounded-file-reader";

NSData *MSIMEReadFileUpTo(NSURL *url, NSUInteger maximumBytes, NSError **error) {
    if (!url.isFileURL || maximumBytes == NSUIntegerMax) {
        if (error) *error = [NSError errorWithDomain:MSIMEBoundedFileReaderError
                                                  code:1
                                              userInfo:@{NSLocalizedDescriptionKey: @"文件参数无效"}];
        return nil;
    }
    int descriptor = open(url.fileSystemRepresentation, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK);
    if (descriptor < 0) {
        if (error) *error = [NSError errorWithDomain:NSPOSIXErrorDomain code:errno userInfo:nil];
        return nil;
    }
    struct stat metadata;
    if (fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode)) {
        int saved = errno;
        close(descriptor);
        if (error) *error = [NSError errorWithDomain:NSPOSIXErrorDomain code:saved userInfo:nil];
        return nil;
    }
    NSFileHandle *handle = [[NSFileHandle alloc] initWithFileDescriptor:descriptor closeOnDealloc:YES];
    NSError *readError = nil;
    NSData *data = [handle readDataUpToLength:maximumBytes + 1 error:&readError];
    NSError *closeError = nil;
    [handle closeAndReturnError:&closeError];
    if (!data) {
        if (error) *error = readError ?: closeError;
        return nil;
    }
    if (readError || closeError) {
        if (error) *error = readError ?: closeError;
        return nil;
    }
    if (data.length > maximumBytes) {
        if (error) *error = [NSError errorWithDomain:MSIMEBoundedFileReaderError
                                                  code:2
                                              userInfo:@{NSLocalizedDescriptionKey: @"文件超过大小限制"}];
        return nil;
    }
    return data;
}
