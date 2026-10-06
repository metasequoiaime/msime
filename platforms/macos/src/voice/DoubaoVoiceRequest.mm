#import "DoubaoVoiceRequest.h"
#import "VoiceFailureMessages.h"
#include "msime_client.h"
#include <cmath>
#include <cstring>
#include <memory>
#include <stdexcept>
#include <vector>

// The frame layout (header, gzip, sequence numbers) is client-core's, reached through the host-api Doubao ABI that iOS and Android use too. This file owns the socket, the PCM chunking and reading the transcript out of a decoded payload.
namespace {
constexpr std::size_t kDoubaoChunkSamples = 3200; // 200 ms at 16 kHz.
constexpr NSUInteger kDoubaoResponseLimit = 1024 * 1024;
constexpr NSUInteger kDoubaoQueuedBytesLimit = 320000;
NSError *DoubaoFailure(NSString *detail = nil) {
    NSMutableDictionary *info = [@{NSLocalizedDescriptionKey:@"豆包语音请求失败，请检查服务设置或重试"} mutableCopy];
    if (detail.length) info[MSIMEVoiceFailureDetailKey] = detail;
    return [NSError errorWithDomain:@"app.msime.client.voice.doubao" code:1 userInfo:info];
}
// The full client request, sequence 1. Nil when the options cannot be encoded.
NSData *StartFrame(bool enableITN, bool enablePunctuation, bool enableDDC, NSString *boostingTable) {
    NSData *table = [boostingTable dataUsingEncoding:NSUTF8StringEncoding] ?: NSData.data;
    const auto *tableBytes = static_cast<const uint8_t *>(table.bytes);
    size_t length = 0;
    // A null, zero-capacity output only asks for the size.
    msime_client_doubao_start_frame(enableITN, enablePunctuation, enableDDC, tableBytes, table.length, nullptr, 0, &length);
    if (!length) return nil;
    NSMutableData *frame = [NSMutableData dataWithLength:length];
    if (!msime_client_doubao_start_frame(enableITN, enablePunctuation, enableDDC, tableBytes, table.length,
            static_cast<uint8_t *>(frame.mutableBytes), frame.length, &length) || length != frame.length)
        return nil;
    return frame;
}
// One audio packet: finite mono float PCM in [-1, 1], sent as little-endian signed 16-bit. At most one 200 ms chunk, only the final packet may be empty, and audio sequence numbers start at 2. Nil for anything else.
NSData *AudioFrame(const float *samples, std::size_t count, int32_t sequence, bool last) {
    if (sequence < 2 || count > kDoubaoChunkSamples || (!samples && count) || (!last && !count)) return nil;
    std::vector<uint8_t> pcm(count * 2);
    for (std::size_t i = 0; i < count; ++i) {
        if (!std::isfinite(samples[i]) || std::fabs(samples[i]) > 1) return nil;
        const auto value = static_cast<uint16_t>(static_cast<int16_t>(samples[i] * 32767.0f));
        pcm[2 * i] = static_cast<uint8_t>(value);
        pcm[2 * i + 1] = static_cast<uint8_t>(value >> 8);
    }
    size_t length = 0;
    msime_client_doubao_audio_frame(sequence, pcm.data(), pcm.size(), last, nullptr, 0, &length);
    if (!length) return nil;
    NSMutableData *frame = [NSMutableData dataWithLength:length];
    if (!msime_client_doubao_audio_frame(sequence, pcm.data(), pcm.size(), last,
            static_cast<uint8_t *>(frame.mutableBytes), frame.length, &length) || length != frame.length)
        return nil;
    return frame;
}
// The text of one `result`: an object with `text` (bigmodel_async), or a list of such segments joined in order (bigmodel_nostream). A body without `result` has no text; `text` that is present but not a string makes the whole message invalid.
BOOL Transcript(id body, NSString **text) {
    *text = @"";
    if (![body isKindOfClass:NSDictionary.class] || !body[@"result"]) return YES;
    id result = body[@"result"];
    NSArray *segments = [result isKindOfClass:NSDictionary.class] ? @[result] : result;
    if (![segments isKindOfClass:NSArray.class]) return NO;
    NSMutableString *joined = [NSMutableString string];
    for (id segment in segments) {
        if (![segment isKindOfClass:NSDictionary.class] || !segment[@"text"]) continue;
        if (![segment[@"text"] isKindOfClass:NSString.class]) return NO;
        [joined appendString:segment[@"text"]];
    }
    *text = joined;
    return YES;
}
struct DoubaoResponse {
    BOOL last = NO;
    int32_t code = 0;
    NSString *text = @"";
};
static BOOL MSIMEStrictBoolean(id value) {
    if (![value isKindOfClass:NSNumber.class] ||
        CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID()) return NO;
    return CFBooleanGetValue((CFBooleanRef)(__bridge CFTypeRef)value);
}
// One complete WebSocket binary message. NO when it is not a valid Doubao response; an error frame's code is reported and its body never becomes text.
BOOL ParseResponse(NSData *message, DoubaoResponse *response) {
    if (!message.length || message.length > kDoubaoResponseLimit) return NO;
    std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
        msime_client_doubao_decode_frame(static_cast<const uint8_t *>(message.bytes), message.length),
        msime_client_string_free);
    if (!raw) return NO;
    id envelope = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:raw.get() length:std::strlen(raw.get())]
                                                  options:0 error:nil];
    if (![envelope isKindOfClass:NSDictionary.class] || !MSIMEStrictBoolean(envelope[@"ok"]) ||
        ![envelope[@"value"] isKindOfClass:NSDictionary.class])
        return NO;
    NSDictionary *value = envelope[@"value"];
    if (value[@"error_code"]) {
        if (![value[@"error_code"] isKindOfClass:NSNumber.class] || ![value[@"error_code"] intValue]) return NO;
        response->last = YES;
        response->code = [value[@"error_code"] intValue];
        return YES;
    }
    id last = value[@"last"];
    if (![last isKindOfClass:NSNumber.class] ||
        CFGetTypeID((__bridge CFTypeRef)last) != CFBooleanGetTypeID() ||
        ![value[@"payload"] isKindOfClass:NSString.class]) return NO;
    response->last = MSIMEStrictBoolean(last);
    NSData *payload = [value[@"payload"] dataUsingEncoding:NSUTF8StringEncoding];
    id body = payload ? [NSJSONSerialization JSONObjectWithData:payload options:0 error:nil] : nil;
    if (![body isKindOfClass:NSDictionary.class]) return NO;
    NSString *text = nil;
    if (!Transcript(body, &text)) return NO;
    if (!text.length && body[@"payload_msg"] && !Transcript(body[@"payload_msg"], &text)) return NO;
    response->text = text;
    return YES;
}
}
// Never follow a redirect with custom authentication headers. This separate
// delegate does not retain the request owner, so dropping it cancels the socket.
@interface MSIMEDoubaoSocketPolicy : NSObject <NSURLSessionTaskDelegate>
@end
@implementation MSIMEDoubaoSocketPolicy
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task
    willPerformHTTPRedirection:(NSHTTPURLResponse *)response newRequest:(NSURLRequest *)request
    completionHandler:(void (^)(NSURLRequest *))completionHandler {
    (void)session; (void)task; (void)response; (void)request;
    completionHandler(nil);
}
@end

@implementation MSIMEDoubaoVoiceRequest {
    NSURLRequest *_request;
    NSData *_initialPacket;
    NSURLSession *_session;
    NSURLSessionWebSocketTask *_socket;
    NSMutableArray<NSData *> *_packets;
    std::vector<float> _pending;
    NSUInteger _queuedBytes;
    int32_t _sequence;
    BOOL _started, _finishing, _done, _cancelled, _sending, _finalDispatched, _legacyAuth, _opened;
    NSString *_lastText;
    MSIMEDoubaoResult _result;
}
- (instancetype)initWithOptions:(NSDictionary *)options error:(NSError **)error {
    self = [super init];
    if (!self) return nil;
    if (![options isKindOfClass:NSDictionary.class]) { if (error) *error = DoubaoFailure(); return nil; }
    NSMutableDictionary *snapshot = [NSMutableDictionary dictionary];
    for (NSString *key in @[@"asr_endpoint", @"asr_token", @"asr_app_key", @"asr_resource_id",
        @"doubao_auth_mode", @"doubao_boosting_table_id"]) {
        id value = options[key];
        if (value && (![value isKindOfClass:NSString.class] || [value length] > 8192 ||
            ![value UTF8String])) {
            if (error) *error = DoubaoFailure(); return nil;
        }
        if (([key isEqual:@"asr_endpoint"] || [key isEqual:@"doubao_boosting_table_id"]) && value &&
            [value rangeOfCharacterFromSet:NSCharacterSet.controlCharacterSet].location != NSNotFound) {
            if (error) *error = DoubaoFailure(); return nil;
        }
        if (value) snapshot[key] = [value copy];
    }
    NSString *endpoint = [snapshot[@"asr_endpoint"] length] ? snapshot[@"asr_endpoint"] : @"wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";
    NSURLComponents *url = [NSURLComponents componentsWithString:endpoint];
    BOOL local = [@[@"127.0.0.1", @"localhost", @"::1"] containsObject:url.host.lowercaseString];
    if (!url.URL || !url.host.length || url.user || url.password || url.fragment ||
        (![url.scheme.lowercaseString isEqual:@"wss"] && !(local && [url.scheme.lowercaseString isEqual:@"ws"]))) {
        if (error) *error = DoubaoFailure(); return nil;
    }
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:url.URL];
    request.timeoutInterval = 10;
    request.cachePolicy = NSURLRequestReloadIgnoringLocalCacheData;
    // The same client-core policy owns probe and recording authentication on all
    // hosts. This adapter only translates the sensitive ABI result to NSURLRequest.
    NSData *authInput = [NSJSONSerialization dataWithJSONObject:@{
        @"auth_mode":snapshot[@"doubao_auth_mode"] ?: @"", @"app_id":snapshot[@"asr_app_key"] ?: @"",
        @"token":snapshot[@"asr_token"] ?: @"", @"resource_id":[snapshot[@"asr_resource_id"] length]
            ? snapshot[@"asr_resource_id"] : @"volc.bigasr.sauc.duration"} options:0 error:nil];
    std::unique_ptr<char, decltype(&msime_client_string_free)> authRaw(
        msime_client_doubao_auth_headers(static_cast<const uint8_t *>(authInput.bytes), authInput.length),
        msime_client_string_free);
    id auth = authRaw ? [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:authRaw.get()
        length:std::strlen(authRaw.get())] options:0 error:nil] : nil;
    if (![auth isKindOfClass:NSDictionary.class] || ![auth[@"ok"] isEqual:@YES] ||
        ![auth[@"value"] isKindOfClass:NSDictionary.class] ||
        ![auth[@"value"][@"headers"] isKindOfClass:NSArray.class] || ![auth[@"value"][@"headers"] count]) {
        if (error) *error = DoubaoFailure(); return nil;
    }
    for (id header in auth[@"value"][@"headers"]) {
        if (![header isKindOfClass:NSArray.class] || [header count] != 2 ||
            ![header[0] isKindOfClass:NSString.class] || ![header[1] isKindOfClass:NSString.class]) {
            if (error) *error = DoubaoFailure(); return nil;
        }
        [request setValue:header[1] forHTTPHeaderField:header[0]];
        // Which console the credentials belong to decides which of them a failure message asks the user to check; the shared policy has already resolved it, so read it back from the headers rather than inferring it again.
        if ([header[0] caseInsensitiveCompare:@"x-api-app-key"] == NSOrderedSame) _legacyAuth = YES;
    }
    bool flags[] = {true, true, false};
    NSArray *keys = @[@"doubao_enable_itn", @"doubao_enable_punc", @"doubao_enable_ddc"];
    for (NSUInteger i = 0; i < keys.count; ++i) {
        id value = options[keys[i]];
        if (value && (![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID())) {
            if (error) *error = DoubaoFailure(); return nil;
        }
        if (value) flags[i] = [value boolValue];
    }
    _initialPacket = StartFrame(flags[0], flags[1], flags[2], snapshot[@"doubao_boosting_table_id"]);
    if (!_initialPacket) { if (error) *error = DoubaoFailure(); return nil; }
    _request = [request copy];
    _packets = [NSMutableArray array];
    _sequence = 2;
    return self;
}
- (void)deliver:(NSString *)text final:(BOOL)final error:(NSError *)error {
    MSIMEDoubaoResult result = _result;
    if (final) {
        _done = YES;
        _result = nil;
        [_packets removeAllObjects]; _pending.clear(); _queuedBytes = 0;
        [_socket cancelWithCloseCode:NSURLSessionWebSocketCloseCodeNormalClosure reason:nil];
        [_session invalidateAndCancel]; _socket = nil; _session = nil;
    }
    __weak MSIMEDoubaoVoiceRequest *weakSelf = self;
    dispatch_async(dispatch_get_main_queue(), ^{
        MSIMEDoubaoVoiceRequest *owner = weakSelf;
        if (!owner) return;
        @synchronized(owner) { if (owner->_cancelled) return; }
        if (result) result(text, final, error);
    });
}
// Whether the websocket upgrade completed: a message has arrived, or the task holds the 101 answer. A refused connection has no response and a rejected upgrade (a wrong key is answered 401) has another status, and both are the connection failure Windows reports.
- (BOOL)socketOpened {
    if (_opened) return YES;
    NSHTTPURLResponse *response = (NSHTTPURLResponse *)_socket.response;
    return [response isKindOfClass:NSHTTPURLResponse.class] && response.statusCode == 101;
}
- (void)receive {
    __weak MSIMEDoubaoVoiceRequest *weakSelf = self;
    [_socket receiveMessageWithCompletionHandler:^(NSURLSessionWebSocketMessage *message, NSError *error) {
        MSIMEDoubaoVoiceRequest *owner = weakSelf;
        if (!owner) return;
        @synchronized(owner) {
            if (owner->_done || owner->_cancelled) return;
            if (error) {
                [owner deliver:nil final:YES error:[owner socketOpened] ? DoubaoFailure()
                    : DoubaoFailure(MSIMEDoubaoFailureMessage(MSIMEDoubaoFailureConnect, owner->_legacyAuth, 0))];
                return;
            }
            owner->_opened = YES;
            if (message.type != NSURLSessionWebSocketMessageTypeData) {
                [owner deliver:nil final:YES error:DoubaoFailure()]; return;
            }
            DoubaoResponse response;
            if (!ParseResponse(message.data, &response)) { [owner deliver:nil final:YES error:DoubaoFailure()]; return; }
            if (response.code) {
                [owner deliver:nil final:YES error:DoubaoFailure(MSIMEDoubaoFailureMessage(MSIMEDoubaoFailureServerCode,
                    owner->_legacyAuth, response.code))];
                return;
            }
            if (response.last && !owner->_finalDispatched) {
                [owner deliver:nil final:YES error:DoubaoFailure()]; return;
            }
            NSString *text = response.text;
            BOOL changed = text.length && ![text isEqual:owner->_lastText];
            if (text.length) owner->_lastText = text;
            if (response.last) {
                [owner deliver:owner->_lastText final:YES error:owner->_lastText.length ? nil : DoubaoFailure()]; return;
            }
            if (changed) [owner deliver:text final:NO error:nil];
            [owner receive];
        }
    }];
}
- (void)pump {
    if (_sending || _done || _cancelled || !_packets.count) return;
    NSData *data = _packets.firstObject;
    [_packets removeObjectAtIndex:0];
    _sending = YES;
    if (_finishing && !_packets.count) _finalDispatched = YES;
    __weak MSIMEDoubaoVoiceRequest *weakSelf = self;
    [_socket sendMessage:[[NSURLSessionWebSocketMessage alloc] initWithData:data] completionHandler:^(NSError *error) {
        MSIMEDoubaoVoiceRequest *owner = weakSelf;
        if (!owner) return;
        @synchronized(owner) {
            if (owner->_done || owner->_cancelled) return;
            owner->_sending = NO;
            owner->_queuedBytes -= data.length;
            if (!error) { [owner pump]; return; }
            // Windows names the two failures before any audio moves: a socket that never opened, and an opening request that could not be sent over one that did. A later audio send failing is left to the generic message, as Windows shows none of its own.
            NSString *detail = nil;
            if (![owner socketOpened]) detail = MSIMEDoubaoFailureMessage(MSIMEDoubaoFailureConnect, owner->_legacyAuth, 0);
            else if (data == owner->_initialPacket) detail = MSIMEDoubaoFailureMessage(MSIMEDoubaoFailureHandshake, owner->_legacyAuth, 0);
            [owner deliver:nil final:YES error:DoubaoFailure(detail)];
        }
    }];
}
- (BOOL)startWithResult:(MSIMEDoubaoResult)result error:(NSError **)error {
    @synchronized(self) {
        if (_started || _cancelled || !result) { if (error) *error = DoubaoFailure(); return NO; }
        _started = YES; _result = [result copy];
        NSURLSessionConfiguration *configuration = NSURLSessionConfiguration.ephemeralSessionConfiguration;
        configuration.HTTPCookieStorage = nil; configuration.URLCredentialStorage = nil; configuration.URLCache = nil;
        // No whole-session budget. The old 100 s one was sized for a 60 s recording, while MSIME-Windows streams for as long as the user records and bounds only individual network operations; a fixed session length cut dictation off while the user was still speaking. The 30 s request timeout stays, and a finished stream still has its own deadline in finishWithError:.
        configuration.timeoutIntervalForRequest = 30;
        _session = [NSURLSession sessionWithConfiguration:configuration delegate:[MSIMEDoubaoSocketPolicy new] delegateQueue:nil];
        _socket = [_session webSocketTaskWithRequest:_request];
        _socket.maximumMessageSize = kDoubaoResponseLimit;
        [_socket resume];
        [_packets addObject:_initialPacket]; _queuedBytes = _initialPacket.length;
        [self receive]; [self pump];
        return YES;
    }
}
- (BOOL)appendPCM:(NSData *)pcm error:(NSError **)error {
    @synchronized(self) {
        if (!_started || _done || _cancelled || _finishing) { if (error) *error = DoubaoFailure(); return NO; }
        try {
            if (!pcm.length || pcm.length % sizeof(float))
                throw std::invalid_argument("size");
            std::vector<float> samples(pcm.length / sizeof(float));
            std::memcpy(samples.data(), pcm.bytes, pcm.length);
            for (float sample : samples) if (!std::isfinite(sample) || std::fabs(sample) > 1) throw std::invalid_argument("pcm");
            _pending.insert(_pending.end(), samples.begin(), samples.end());
            std::size_t offset = 0;
            while (_pending.size() - offset >= kDoubaoChunkSamples) {
                NSData *packet = AudioFrame(_pending.data() + offset, kDoubaoChunkSamples, _sequence++, false);
                if (!packet) throw std::invalid_argument("pcm");
                _queuedBytes += packet.length;
                if (_queuedBytes > kDoubaoQueuedBytesLimit) throw std::invalid_argument("backpressure");
                [_packets addObject:packet]; offset += kDoubaoChunkSamples;
            }
            _pending.erase(_pending.begin(), _pending.begin() + offset);
            [self pump]; return YES;
        } catch (const std::exception &) {
            if (error) *error = DoubaoFailure(); [self deliver:nil final:YES error:DoubaoFailure()]; return NO;
        }
    }
}
- (BOOL)finishWithError:(NSError **)error {
    @synchronized(self) {
        if (!_started || _done || _cancelled || _finishing) { if (error) *error = DoubaoFailure(); return NO; }
        _finishing = YES;
        NSData *packet = AudioFrame(_pending.data(), _pending.size(), _sequence, true);
        if (!packet) {
            if (error) *error = DoubaoFailure(); [self deliver:nil final:YES error:DoubaoFailure()]; return NO;
        }
        [_packets addObject:packet]; _queuedBytes += packet.length; _pending.clear(); [self pump];
        __weak MSIMEDoubaoVoiceRequest *weakSelf = self;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 30 * NSEC_PER_SEC), dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            MSIMEDoubaoVoiceRequest *owner = weakSelf;
            if (!owner) return;
            @synchronized(owner) { if (!owner->_done && !owner->_cancelled) [owner deliver:nil final:YES error:DoubaoFailure()]; }
        });
        return YES;
    }
}
- (void)cancel {
    @synchronized(self) {
        _cancelled = YES; _done = YES; _result = nil;
        [_socket cancelWithCloseCode:NSURLSessionWebSocketCloseCodeGoingAway reason:nil];
        [_session invalidateAndCancel]; _socket = nil; _session = nil;
        [_packets removeAllObjects]; _pending.clear(); _queuedBytes = 0;
    }
}
- (void)dealloc { [self cancel]; }
@end
