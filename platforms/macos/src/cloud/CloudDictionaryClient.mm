#import "CloudDictionaryClient.h"
#import "CloudURLSession.h"
void MSIMEFetchCloudDictionary(NSString *kind, NSString *search, NSUInteger offset, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (kind.length == 0 || search.length > 1024 || offset > 1000000 || bearerToken.length == 0) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSURLComponents *components = [NSURLComponents componentsWithString:@"https://api.msime.app/v1/users/me/dictionaries"];
    components.path = [components.path stringByAppendingPathComponent:kind];
    components.queryItems = @[[NSURLQueryItem queryItemWithName:@"q" value:search], [NSURLQueryItem queryItemWithName:@"offset" value:[NSString stringWithFormat:@"%lu", (unsigned long)offset]], [NSURLQueryItem queryItemWithName:@"limit" value:@"100"]];
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:components.URL];
    [request setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"];
    MSIMEStartCloudDataTask(request, 1024 * 1024, ^(NSData *data, NSURLResponse *response, NSError *error) { NSInteger status = [(NSHTTPURLResponse *)response statusCode]; dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(data, status, error); }); });
}

void MSIMEMutateCloudDictionary(NSString *method, NSString *kind, NSString *entryID, NSData *body, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (![@[@"POST", @"PUT", @"DELETE"] containsObject:method] || kind.length == 0 || bearerToken.length == 0 || (entryID.length == 0 && ![method isEqualToString:@"POST"])) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSString *path = [NSString stringWithFormat:@"https://api.msime.app/v1/users/me/dictionaries/%@%@", kind, entryID.length ? [@"/" stringByAppendingString:entryID] : @""];
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:path]]; request.HTTPMethod = method; request.HTTPBody = body;
    [request setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"]; [request setValue:@"application/json" forHTTPHeaderField:@"Content-Type"];
    MSIMEStartCloudDataTask(request, 1024 * 1024, ^(NSData *data, NSURLResponse *response, NSError *error) { NSInteger status = [(NSHTTPURLResponse *)response statusCode]; dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(data, status, error); }); });
}

void MSIMEImportCloudDictionary(NSString *kind, NSString *format, NSData *body, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (kind.length == 0 || ![@[@"standard", @"windows", @"hans"] containsObject:format] || body.length == 0 || body.length > 65536 || bearerToken.length == 0) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSString *path = [NSString stringWithFormat:@"https://api.msime.app/v1/users/me/dictionaries/%@/import%@", kind, [format isEqualToString:@"hans"] ? @"-hans" : @""];
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:path]]; request.HTTPMethod = @"POST";
    NSString *text = [[NSString alloc] initWithData:body encoding:NSUTF8StringEncoding] ?: @"";
    NSDictionary *payload = [format isEqualToString:@"hans"] ? @{ @"text": text, @"weight": @100000 } : @{ @"text": text, @"format": format };
    request.HTTPBody = [NSJSONSerialization dataWithJSONObject:payload options:0 error:nil];
    [request setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"]; [request setValue:@"text/plain; charset=utf-8" forHTTPHeaderField:@"Content-Type"];
    MSIMEStartCloudDataTask(request, 1024 * 1024, ^(NSData *data, NSURLResponse *response, NSError *error) { NSInteger status = [(NSHTTPURLResponse *)response statusCode]; dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(data, status, error); }); });
}

void MSIMEExportCloudDictionary(NSString *kind, NSString *format, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (kind.length == 0 || ![@[@"standard", @"windows"] containsObject:format] || bearerToken.length == 0) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSString *path = [NSString stringWithFormat:@"https://api.msime.app/v1/users/me/dictionaries/%@/export?format=%@", kind, format];
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:path]]; request.HTTPMethod = @"GET";
    [request setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"]; [request setValue:@"text/plain" forHTTPHeaderField:@"Content-Type"];
    MSIMEStartCloudDataTask(request, 1024 * 1024, ^(NSData *data, NSURLResponse *response, NSError *error) { NSInteger status = [(NSHTTPURLResponse *)response statusCode]; dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(data, status, error); }); });
}

void MSIMEFetchCloudDictionaryCatalog(NSString *kind, NSString *code, NSUInteger offset, NSString *scheme, NSString *profile, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (kind.length == 0 || code.length > 256 || [code rangeOfString:@"\0"].location != NSNotFound || offset > 1000000 || scheme.length == 0 || profile.length == 0 || bearerToken.length == 0) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSURLComponents *c = [NSURLComponents componentsWithString:[NSString stringWithFormat:@"https://api.msime.app/v1/users/me/dictionaries/%@/catalog", kind]];
    c.queryItems = @[[NSURLQueryItem queryItemWithName:@"q" value:code], [NSURLQueryItem queryItemWithName:@"offset" value:[NSString stringWithFormat:@"%lu", (unsigned long)offset]], [NSURLQueryItem queryItemWithName:@"limit" value:@"100"], [NSURLQueryItem queryItemWithName:@"scheme" value:scheme], [NSURLQueryItem queryItemWithName:@"profile" value:profile]];
    NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:c.URL]; [r setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"];
    MSIMEStartCloudDataTask(r, 1024 * 1024, ^(NSData *d, NSURLResponse *response, NSError *e) { dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(d, [(NSHTTPURLResponse *)response statusCode], e); }); });
}

void MSIMEEditCloudDictionaryCatalog(NSString *kind, NSData *body, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (kind.length == 0 || body.length == 0 || body.length > 65536 || bearerToken.length == 0) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSString *path = [NSString stringWithFormat:@"https://api.msime.app/v1/users/me/dictionaries/%@/edit", kind]; NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:path]]; r.HTTPMethod = @"POST"; r.HTTPBody = body;
    [r setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"]; [r setValue:@"application/json" forHTTPHeaderField:@"Content-Type"];
    MSIMEStartCloudDataTask(r, 1024 * 1024, ^(NSData *d, NSURLResponse *response, NSError *e) { dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(d, [(NSHTTPURLResponse *)response statusCode], e); }); });
}

void MSIMEFetchCloudDictionaryChanges(long long after, NSUInteger limit, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (after < 0 || limit < 1 || limit > 100 || bearerToken.length == 0) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSString *path = [NSString stringWithFormat:@"https://api.msime.app/v1/users/me/dictionary/changes?after=%lld&limit=%lu", after, (unsigned long)limit]; NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:path]];
    [r setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"];
    MSIMEStartCloudDataTask(r, 1024 * 1024, ^(NSData *d, NSURLResponse *response, NSError *e) { dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(d, [(NSHTTPURLResponse *)response statusCode], e); }); });
}

NSString *MSIMEReadDictionaryImportFile(NSURL *url, NSError **error) {
    if (!url || ![url isFileURL]) { if (error) *error = [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]; return nil; }
    NSData *data = [NSData dataWithContentsOfURL:url options:NSDataReadingMappedIfSafe error:error];
    if (!data || data.length == 0 || data.length > 65536) { if (error && !*error) *error = [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]; return nil; }
    NSString *text = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    if (!text.length || [text rangeOfString:@"\0"].location != NSNotFound) { if (error) *error = [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]; return nil; }
    return text;
}

BOOL MSIMESaveDictionaryExportFile(NSData *data, NSURL *url, NSError **error) {
    return data && url && [url isFileURL] && data.length <= 384 * 1024 * 1024 && [data writeToURL:url options:NSDataWritingAtomic error:error];
}

void MSIMESendCloudCandidateRequest(NSString *path, NSData *body, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (path.length == 0 || ![path hasPrefix:@"/v1/users/me/dictionary/"] || body.length == 0 || body.length > 65536 || bearerToken.length == 0) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:[@"https://api.msime.app" stringByAppendingString:path]]]; r.HTTPMethod = @"POST"; r.HTTPBody = body;
    [r setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"]; [r setValue:@"application/json" forHTTPHeaderField:@"Content-Type"];
    MSIMEStartCloudDataTask(r, 1024 * 1024, ^(NSData *d, NSURLResponse *response, NSError *e) { dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(d, [(NSHTTPURLResponse *)response statusCode], e); }); });
}

void MSIMEListCloudFixedPositions(NSString *context, NSUInteger offset, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (context.length > 256 || offset > 1000000 || bearerToken.length == 0) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSURLComponents *components = [NSURLComponents componentsWithString:@"https://api.msime.app/v1/users/me/dictionary/positions"];
    components.queryItems = @[[NSURLQueryItem queryItemWithName:@"context" value:context ?: @""], [NSURLQueryItem queryItemWithName:@"offset" value:[NSString stringWithFormat:@"%lu", (unsigned long)offset]], [NSURLQueryItem queryItemWithName:@"limit" value:@"100"]];
    NSString *path = [components.percentEncodedPath stringByAppendingFormat:@"?%@", components.percentEncodedQuery]; NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:[@"https://api.msime.app" stringByAppendingString:path]]];
    [r setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"];
    MSIMEStartCloudDataTask(r, 1024 * 1024, ^(NSData *d, NSURLResponse *response, NSError *e) { dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(d, [(NSHTTPURLResponse *)response statusCode], e); }); });
}

void MSIMEMutateCloudFixedPosition(NSString *method, NSData *body, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (![method isEqualToString:@"PUT"] && ![method isEqualToString:@"DELETE"]) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:@"https://api.msime.app/v1/users/me/dictionary/positions"]]; r.HTTPMethod = method; r.HTTPBody = body;
    [r setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"]; [r setValue:@"application/json" forHTTPHeaderField:@"Content-Type"];
    MSIMEStartCloudDataTask(r, 1024 * 1024, ^(NSData *d, NSURLResponse *response, NSError *e) { dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(d, [(NSHTTPURLResponse *)response statusCode], e); }); });
}

void MSIMEFetchCloudDictionarySnapshot(NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (bearerToken.length == 0) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSMutableURLRequest *r = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:@"https://api.msime.app/v1/users/me/dictionary/snapshot"]];
    [r setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"];
    MSIMEStartCloudDataTask(r, 512 * 1024 * 1024, ^(NSData *d, NSURLResponse *response, NSError *e) {
        NSInteger status = [(NSHTTPURLResponse *)response statusCode];
        if (d.length > 512 * 1024 * 1024) { d = nil; status = 413; }
        dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(d, status, e); });
    });
}

void MSIMERestoreCloudDictionarySnapshot(NSURL *file, long long revision, NSString *expectedSHA256, NSString *bearerToken, MSIMECloudDictionaryCompletion completion) {
    if (!file.isFileURL || revision < 0 || bearerToken.length == 0 || expectedSHA256.length != 64) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSNumber *size = [NSFileManager.defaultManager attributesOfItemAtPath:file.path error:nil][NSFileSize];
    if (!size || size.longLongValue <= 0 || size.longLongValue > 512LL * 1024 * 1024) { if (completion) completion(nil, 400, [NSError errorWithDomain:@"MSIMECloud" code:400 userInfo:nil]); return; }
    NSURLComponents *components = [NSURLComponents componentsWithString:@"https://api.msime.app/v1/users/me/dictionary/snapshot"];
    components.queryItems = @[[NSURLQueryItem queryItemWithName:@"revision" value:[NSString stringWithFormat:@"%lld", revision]]];
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:components.URL]; request.HTTPMethod = @"PUT"; request.HTTPBodyStream = [NSInputStream inputStreamWithURL:file];
    [request setValue:[@"Bearer " stringByAppendingString:bearerToken] forHTTPHeaderField:@"Authorization"]; [request setValue:@"application/x-ndjson" forHTTPHeaderField:@"Content-Type"]; [request setValue:size.stringValue forHTTPHeaderField:@"Content-Length"];
    MSIMEStartCloudDataTask(request, 1024 * 1024, ^(NSData *data, NSURLResponse *response, NSError *error) { NSInteger status = [(NSHTTPURLResponse *)response statusCode]; dispatch_async(dispatch_get_main_queue(), ^{ if (completion) completion(data, status, error); }); });
}
