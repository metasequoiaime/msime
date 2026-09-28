#pragma once

#import <Foundation/Foundation.h>

typedef void (^MSIMECloudDataCompletion)(NSData *data, NSURLResponse *response, NSError *error);

/// Starts a credential-bearing cloud request with a hard response-body bound.
FOUNDATION_EXPORT void MSIMEStartCloudDataTask(NSURLRequest *request, NSUInteger maximumBytes,
                                                MSIMECloudDataCompletion completion);
