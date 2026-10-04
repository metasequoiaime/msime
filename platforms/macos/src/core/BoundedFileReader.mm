#import "BoundedFileReader.h"

static NSString *const MSIMEBoundedFileReaderError = @"app.msime.client.bounded-file-reader";

NSData *MSIMEReadFileUpTo(NSURL *url, NSUInteger maximumBytes, NSError **error) {
    if (!url.isFileURL || maximumBytes == NSUIntegerMax) {
        if (error) *error = [NSError errorWithDomain:MSIMEBoundedFileReaderError
                                                  code:1
                                              userInfo:@{NSLocalizedDescriptionKey: @"文件参数无效"}];
        return nil;
    }
    NSFileHandle *handle = [NSFileHandle fileHandleForReadingFromURL:url error:error];
    if (!handle) return nil;
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
