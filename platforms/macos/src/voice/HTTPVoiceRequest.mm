#import "HTTPVoiceRequest.h"
#import "VoiceFailureMessages.h"
#include "../../../../shared/voice/VoiceProviders.h"
#include "../../../../shared/voice/PolishPrompt.h"
#include <algorithm>
#include <cmath>
#include <cstring>

namespace {
NSError *Failure(const std::string &detail = {}) {
    NSMutableDictionary *info = [@{NSLocalizedDescriptionKey: @"语音请求失败，请检查识别服务设置"} mutableCopy];
    // The shared provider layer builds the detail from the answer, never from the token or the upload; a body that is not UTF-8 is simply not shown.
    NSString *text = detail.empty() ? nil
        : [[NSString alloc] initWithBytes:detail.data() length:detail.size() encoding:NSUTF8StringEncoding];
    if (text.length) info[MSIMEVoiceFailureDetailKey] = text;
    return [NSError errorWithDomain:@"app.msime.client.voice" code:6 userInfo:info];
}
std::string String(NSDictionary *options, NSString *key) {
    NSString *value = options[key];
    return value ? std::string(value.UTF8String) : std::string();
}
BOOL Endpoint(const std::string &value) {
    NSURLComponents *url = [NSURLComponents componentsWithString:@(value.c_str())];
    BOOL loopback = [@[@"127.0.0.1", @"localhost", @"::1"] containsObject:url.host.lowercaseString];
    return url.host.length && !url.user && !url.password && !url.fragment &&
        ([url.scheme.lowercaseString isEqual:@"https"] || (loopback && [url.scheme.lowercaseString isEqual:@"http"]));
}
std::string Polish(std::string text, NSDictionary *options, const std::shared_ptr<std::atomic_bool> &cancelled, void (^polishing)(void) = nil) {
    if (text.empty() || cancelled->load() ||
        !([options[@"polish_enabled"] boolValue] || [options[@"polish_text"] boolValue]) || ![options[@"polish_token"] length]) return text;
    try {
        auto provider = String(options, @"polish_provider");
        auto endpoint = String(options, @"polish_endpoint");
        auto model = String(options, @"polish_model");
        if (endpoint.empty()) endpoint = msime::voice::default_polish_endpoint(provider);
        if (model.empty()) model = msime::voice::default_polish_model(provider);
        msime::windows::PolishPromptSlots slots;
        slots.id = String(options, @"polish_prompt_id");
        slots.custom_1 = String(options, @"polish_prompt_custom_1");
        slots.custom_2 = String(options, @"polish_prompt_custom_2");
        slots.custom_3 = String(options, @"polish_prompt_custom_3");
        auto prompt = msime::windows::polish_prompt_for(slots);
        if (Endpoint(endpoint)) {
            // A block captures a C++ reference as the reference, not as a copy of what it names. This one
            // runs on main after Polish has returned, when the request owning `cancelled` may already be
            // gone, so it has to hold its own share of the flag - reading through the parameter crashed.
            if (polishing) {
                std::shared_ptr<std::atomic_bool> flag = cancelled;
                dispatch_async(dispatch_get_main_queue(), ^{ if (!flag->load()) polishing(); });
            }
            // 30s, the budget the reference host uses and for the reason it measured: a chat completion
            // cleaning up to a minute of transcript does not answer inside the 3s default, and the catch
            // below keeps the ASR text without telling anyone - so the transcript reached the provider and
            // the cleaned answer was discarded every time. Waiting is the lesser cost; sending the text and
            // binning the reply is the one nobody asked for.
            auto polished = msime::voice::polish_cloud_text(text, provider, endpoint, model,
                String(options, @"polish_token"), prompt, cancelled, 30000);
            if (!polished.empty() && polished.size() <= 65536) text = std::move(polished);
        }
    } catch (const std::exception &) { /* Optional polish failure preserves ASR. */ }
    return text;
}
}
@implementation MSIMEHTTPVoiceRequest {
    NSDictionary *_options;
    std::shared_ptr<std::atomic_bool> _cancelled;
    BOOL _started;
    BOOL _recognitionRequired;
}
- (instancetype)initWithOptions:(NSDictionary *)options error:(NSError **)error {
    return [self initWithOptions:options recognitionRequired:YES error:error];
}
- (instancetype)initWithPolishOptions:(NSDictionary *)options error:(NSError **)error {
    return [self initWithOptions:options recognitionRequired:NO error:error];
}
- (instancetype)initWithOptions:(NSDictionary *)options recognitionRequired:(BOOL)recognitionRequired error:(NSError **)error {
    self = [super init];
    if (!self) return nil;
    _cancelled = std::make_shared<std::atomic_bool>(false);
    _recognitionRequired = recognitionRequired;
    if (![options isKindOfClass:NSDictionary.class]) { if (error) *error = Failure(); return nil; }
    NSMutableDictionary *snapshot = [NSMutableDictionary dictionary];
    for (NSString *key in @[@"asr_provider", @"asr_endpoint", @"asr_model", @"asr_model_path", @"asr_token", @"language",
        @"polish_provider", @"polish_endpoint", @"polish_model", @"polish_token", @"polish_prompt_id",
        @"polish_prompt_custom_1", @"polish_prompt_custom_2", @"polish_prompt_custom_3"]) {
        id value = options[key];
        if (value && (![value isKindOfClass:NSString.class] || [value length] > 8192 ||
            ![value dataUsingEncoding:NSUTF8StringEncoding])) {
            if (error) *error = Failure(); return nil;
        }
        if (value) snapshot[key] = [value copy];
    }
    for (NSString *key in @[@"polish_enabled", @"polish_text"]) {
        id value = options[key];
        if ([value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID())
            snapshot[key] = value;
    }
    for (NSString *key in @[@"asr_token", @"polish_token"]) {
        NSString *token = snapshot[key];
        if (token && [token rangeOfCharacterFromSet:NSCharacterSet.controlCharacterSet].location != NSNotFound) {
            if (error) *error = Failure(); return nil;
        }
    }
    if (recognitionRequired) {
        const auto provider = msime::voice::normalize_voice_provider(String(snapshot, @"asr_provider"));
        const auto endpoint = msime::voice::resolved_asr_endpoint(provider, String(snapshot, @"asr_endpoint"));
        // 只接整句上传的服务（multipart 或 chat_audio），请求体由 shared/voice 按 asr_request_format 拼。豆包是流式 WebSocket，不会走到这里；其他值是过期的配置，不是用户选的服务。
        const auto format = msime::voice::asr_request_format(provider);
        if ((format != "multipart" && format != "chat_audio") ||
            !Endpoint(endpoint) || ![snapshot[@"asr_token"] length]) {
            if (error) *error = Failure(); return nil;
        }
        snapshot[@"asr_provider"] = @(provider.c_str());
        snapshot[@"asr_endpoint"] = @(endpoint.c_str());
        if (![snapshot[@"asr_model"] length]) snapshot[@"asr_model"] = @(msime::voice::default_asr_model(provider).c_str());
    }
    _options = [snapshot copy];
    return self;
}
- (NSUInteger)sampleLimit {
    // 阿里云百炼的 chat_audio 上限（约 218 秒）比 multipart 小得多，录音按它截取，不让整段在上传时被拒。
    return msime::voice::batch_capture_sample_limit_for(String(_options, @"asr_provider"));
}
- (BOOL)recognizePCM:(NSData *)pcm completion:(void (^)(NSString *, NSError *))completion error:(NSError **)error {
    @synchronized(self) {
        if (!_recognitionRequired || _started || _cancelled->load() || !completion || !pcm.length ||
            pcm.length % sizeof(float)) {
            if (error) *error = Failure(); return NO;
        }
        // Submit what was captured up to what the provider can take, as the capture buffer does.
        const std::size_t limit = self.sampleLimit;
        std::vector<float> samples(std::min<std::size_t>(pcm.length / sizeof(float), limit));
        std::memcpy(samples.data(), pcm.bytes, samples.size() * sizeof(float));
        for (float value : samples) if (!std::isfinite(value) || std::fabs(value) > 1) {
            if (error) *error = Failure(); return NO;
        }
        _started = YES;
        auto cancelled = _cancelled;
        NSDictionary *options = _options;
        void (^polishing)(void) = [self.polishingHandler copy];
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            NSString *result = nil;
            NSError *failure = nil;
            try {
                auto language = String(options, @"language");
                if (language == "en-US" || language == "en-us") language = "en";
                if (language == "zh-CN") language = "zh-cn";
                auto text = msime::voice::recognize_cloud_asr(samples, String(options, @"asr_provider"),
                    String(options, @"asr_endpoint"), String(options, @"asr_model"), String(options, @"asr_token"), language, cancelled);
                text = Polish(std::move(text), options, cancelled, polishing);
                result = [[NSString alloc] initWithBytes:text.data() length:text.size() encoding:NSUTF8StringEncoding];
                if (!result.length) failure = Failure();
            } catch (const msime::voice::CloudAsrError &cloudError) { failure = Failure(cloudError.user_message()); }
            catch (const std::exception &) { failure = Failure(); }
            dispatch_async(dispatch_get_main_queue(), ^{ if (!cancelled->load()) completion(failure ? nil : result, failure); });
        });
        return YES;
    }
}
- (BOOL)polishText:(NSString *)text completion:(void (^)(NSString *, NSError *))completion error:(NSError **)error {
    @synchronized(self) {
        NSData *data = [text dataUsingEncoding:NSUTF8StringEncoding];
        if (_recognitionRequired || _started || _cancelled->load() || !completion || !data.length || data.length > 65536) {
            if (error) *error = Failure(); return NO;
        }
        _started = YES;
        auto cancelled = _cancelled;
        NSDictionary *options = _options;
        NSString *original = [text copy];
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            auto polished = Polish(std::string(static_cast<const char *>(data.bytes), data.length), options, cancelled);
            NSString *result = [[NSString alloc] initWithBytes:polished.data() length:polished.size() encoding:NSUTF8StringEncoding];
            dispatch_async(dispatch_get_main_queue(), ^{ if (!cancelled->load()) completion(result.length ? result : original, nil); });
        });
        return YES;
    }
}
- (void)cancel { if (_cancelled) _cancelled->store(true); }
- (void)dealloc { [self cancel]; }
@end
