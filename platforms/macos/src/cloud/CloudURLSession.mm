#import "CloudURLSession.h"

@interface MSIMECloudDataTask : NSObject <NSURLSessionDataDelegate, NSURLSessionTaskDelegate>
@property(nonatomic, strong) NSURLSession *session;
@property(nonatomic, strong) NSMutableData *body;
@property(nonatomic, copy) MSIMECloudDataCompletion completion;
@property(nonatomic) NSUInteger maximumBytes;
@property(nonatomic, strong) NSURLResponse *response;
@property(nonatomic) BOOL finished;
@end

@implementation MSIMECloudDataTask
- (instancetype)initWithRequest:(NSURLRequest *)request maximumBytes:(NSUInteger)maximumBytes
                      completion:(MSIMECloudDataCompletion)completion {
    if (!(self = [super init])) return nil;
    _body = [NSMutableData data];
    _maximumBytes = maximumBytes;
    _completion = [completion copy];
    NSURLSessionConfiguration *configuration =
        [NSURLSessionConfiguration ephemeralSessionConfiguration];
    configuration.URLCache = nil;
    configuration.HTTPCookieStorage = nil;
    configuration.URLCredentialStorage = nil;
    configuration.HTTPShouldSetCookies = NO;
    _session = [NSURLSession sessionWithConfiguration:configuration delegate:self delegateQueue:nil];
    [[_session dataTaskWithRequest:request] resume];
    return self;
}

- (void)finishWithData:(NSData *)data response:(NSURLResponse *)response error:(NSError *)error {
    if (_finished) return;
    _finished = YES;
    MSIMECloudDataCompletion completion = _completion;
    _completion = nil;
    [_session finishTasksAndInvalidate];
    _session = nil;
    if (completion) completion(data, response, error);
}

- (void)URLSession:(NSURLSession *)session dataTask:(NSURLSessionDataTask *)task
    didReceiveResponse:(NSURLResponse *)response
    completionHandler:(void (^)(NSURLSessionResponseDisposition))completionHandler {
    (void)session;
    (void)task;
    _response = response;
    if (response.expectedContentLength >= 0 &&
        (uint64_t)response.expectedContentLength > (uint64_t)_maximumBytes) {
        completionHandler(NSURLSessionResponseCancel);
        [self finishWithData:nil response:response error:nil];
        return;
    }
    completionHandler(NSURLSessionResponseAllow);
}

- (void)URLSession:(NSURLSession *)session dataTask:(NSURLSessionDataTask *)task
    didReceiveData:(NSData *)data {
    (void)session;
    if (_finished || data.length > _maximumBytes - _body.length) {
        [task cancel];
        [self finishWithData:nil response:_response error:nil];
        return;
    }
    [_body appendData:data];
}

- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task
    didCompleteWithError:(NSError *)error {
    (void)session;
    (void)task;
    [self finishWithData:_finished ? nil : [_body copy] response:_response error:error];
}

- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task
    willPerformHTTPRedirection:(NSHTTPURLResponse *)response newRequest:(NSURLRequest *)request
             completionHandler:(void (^)(NSURLRequest *))completionHandler {
    (void)session;
    (void)task;
    (void)request;
    completionHandler(nil);
    [self finishWithData:nil response:response error:nil];
}
@end

void MSIMEStartCloudDataTask(NSURLRequest *request, NSUInteger maximumBytes,
                             MSIMECloudDataCompletion completion) {
    if (!request || maximumBytes == 0 || !completion) {
        if (completion) completion(nil, nil, nil);
        return;
    }
    // NSURLSession retains its delegate for the lifetime of the task.
    (void)[[MSIMECloudDataTask alloc] initWithRequest:request maximumBytes:maximumBytes
                                            completion:completion];
}
