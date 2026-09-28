#import "CloudURLSession.h"

@interface MSIMECloudRedirectPolicy : NSObject <NSURLSessionTaskDelegate>
@end

@implementation MSIMECloudRedirectPolicy
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task
    willPerformHTTPRedirection:(NSHTTPURLResponse *)response newRequest:(NSURLRequest *)request
             completionHandler:(void (^)(NSURLRequest *))completionHandler {
    (void)session;
    (void)task;
    (void)response;
    (void)request;
    // A redirect can change the origin. Refuse it before NSURLSession can replay an
    // Authorization header or send private request data to the new endpoint.
    completionHandler(nil);
}
@end

NSURLSession *MSIMECloudURLSession(void) {
    static NSURLSession *session;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        NSURLSessionConfiguration *configuration =
            [NSURLSessionConfiguration ephemeralSessionConfiguration];
        configuration.URLCache = nil;
        configuration.HTTPCookieStorage = nil;
        configuration.URLCredentialStorage = nil;
        configuration.HTTPShouldSetCookies = NO;
        session = [NSURLSession sessionWithConfiguration:configuration
                                                 delegate:[MSIMECloudRedirectPolicy new]
                                            delegateQueue:nil];
    });
    return session;
}
