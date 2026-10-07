#import "LocalVoiceRequest.h"
#import "VoiceFailureMessages.h"
#include "msime_client.h"
#include <atomic>
#include <array>
#include <cerrno>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fcntl.h>
#include <filesystem>
#include <sys/stat.h>
#include <memory>
#include <iterator>
#include <unistd.h>
#include <vector>

namespace {
NSString *const LocalVoiceDomain = @"app.msime.client.voice.local";
// A protocol line is a few kilobytes of base64 audio or a sentence of text; anything this long is a helper gone wrong, not a message to keep buffering.
constexpr NSUInteger MaximumHelperLine = 4 * 1024 * 1024;
// 安装器拒绝超过 64 KiB 的模型清单；读取时也必须限制累计字节数，避免检查后文件膨胀。
constexpr NSUInteger MaximumModelManifestBytes = 64 * 1024;

NSError *LocalVoiceFailure(NSString *detail = nil) {
    NSMutableDictionary *info = [@{NSLocalizedDescriptionKey : @"本地语音识别失败，请检查模型或重试"} mutableCopy];
    if (detail.length) info[MSIMEVoiceFailureDetailKey] = detail;
    return [NSError errorWithDomain:LocalVoiceDomain code:1 userInfo:info];
}

// One host-api JSON call: the `value` of an `{"ok":true}` response, or nil for a failed call or a request that is not JSON.
id HostValue(char *(*function)(const uint8_t *, size_t), NSDictionary *request) {
    if (![NSJSONSerialization isValidJSONObject:request]) return nil;
    NSData *body = [NSJSONSerialization dataWithJSONObject:request options:0 error:nil];
    if (!body) return nil;
    std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
        function(static_cast<const uint8_t *>(body.bytes), body.length), msime_client_string_free);
    if (!raw) return nil;
    id response = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytesNoCopy:raw.get() length:std::strlen(raw.get()) freeWhenDone:NO]
                                                  options:0 error:nil];
    if (![response isKindOfClass:NSDictionary.class] || ![response[@"ok"] isEqual:@YES]) return nil;
    return response[@"value"];
}

// The user's own dictionary words, heaviest first, as client-core picks them. A dictionary that is busy or unreadable only costs this session its hotwords.
NSArray<NSDictionary *> *FetchHotwords(NSDictionary *hostOptions) {
    id value = HostValue(msime_client_voice_hotwords, @{@"options" : hostOptions, @"limit" : @200});
    id words = [value isKindOfClass:NSDictionary.class] ? value[@"hotwords"] : nil;
    if (![words isKindOfClass:NSArray.class]) return @[];
    NSMutableArray<NSDictionary *> *hotwords = [NSMutableArray array];
    for (id word in words)
        if ([word isKindOfClass:NSDictionary.class] && [word[@"text"] isKindOfClass:NSString.class] && [word[@"text"] length] &&
            [word[@"pinyin"] isKindOfClass:NSString.class])
            [hotwords addObject:@{@"text" : word[@"text"], @"pinyin" : word[@"pinyin"]}];
    return hotwords;
}

NSString *CorrectedText(NSString *text, NSArray<NSDictionary *> *hotwords) {
    id value = HostValue(msime_client_voice_hotword_correct, @{@"text" : text, @"hotwords" : hotwords});
    id corrected = [value isKindOfClass:NSDictionary.class] ? value[@"text"] : nil;
    return [corrected isKindOfClass:NSString.class] ? corrected : text;
}

NSData *BoundedModelManifestData(NSString *path) {
    const int descriptor = open(path.fileSystemRepresentation, O_RDONLY | O_NOFOLLOW);
    if (descriptor < 0) return nil;
    struct stat fileStat = {};
    if (fstat(descriptor, &fileStat) != 0 || !S_ISREG(fileStat.st_mode) ||
        fileStat.st_size < 0 ||
        static_cast<uint64_t>(fileStat.st_size) > MaximumModelManifestBytes) {
        close(descriptor);
        return nil;
    }
    NSMutableData *data = [NSMutableData dataWithCapacity:static_cast<NSUInteger>(fileStat.st_size)];
    std::array<uint8_t, 8192> buffer{};
    for (;;) {
        const ssize_t count = read(descriptor, buffer.data(), buffer.size());
        if (count == 0) break;
        if (count < 0) {
            if (errno == EINTR) continue;
            close(descriptor);
            return nil;
        }
        if (data.length > MaximumModelManifestBytes - static_cast<NSUInteger>(count)) {
            close(descriptor);
            return nil;
        }
        [data appendBytes:buffer.data() length:static_cast<NSUInteger>(count)];
    }
    close(descriptor);
    return data;
}

bool TrustedModelPathLink(const std::filesystem::path &path) {
    const std::filesystem::path expected = path == "/var"   ? "/private/var"
                                         : path == "/tmp"   ? "/private/tmp"
                                                             : std::filesystem::path();
    if (expected.empty()) return false;
    std::error_code error;
    const auto target = std::filesystem::read_symlink(path, error);
    return !error && (path.parent_path() / target).lexically_normal() == expected;
}

bool ModelDirectoryHasRealAncestors(NSString *path) {
    const std::filesystem::path directory(path.fileSystemRepresentation);
    if (!directory.is_absolute()) return false;
    std::vector<std::filesystem::path> ancestors;
    for (auto current = directory;; current = current.parent_path()) {
        ancestors.push_back(current);
        if (current.parent_path() == current) break;
    }
    std::error_code error;
    bool sawTrustedLink = false;
    for (auto it = ancestors.rbegin(); it != ancestors.rend(); ++it) {
        const auto status = std::filesystem::symlink_status(*it, error);
        if (error) return false;
        if (std::filesystem::is_symlink(status)) {
            const bool last = std::next(it) == ancestors.rend();
            if (last || sawTrustedLink || !TrustedModelPathLink(*it)) return false;
            sawTrustedLink = true;
        } else if (!std::filesystem::is_directory(status)) {
            return false;
        }
    }
    return true;
}

NSDictionary *ModelManifest(NSString *path) {
    if (!path.isAbsolutePath) return nil;
    if (!ModelDirectoryHasRealAncestors(path)) return nil;
    struct stat directoryStat = {};
    if (lstat(path.fileSystemRepresentation, &directoryStat) != 0 || !S_ISDIR(directoryStat.st_mode)) return nil;
    NSString *manifestPath = [path stringByAppendingPathComponent:@"msime-model.json"];
    struct stat manifestStat = {};
    if (lstat(manifestPath.fileSystemRepresentation, &manifestStat) != 0 || !S_ISREG(manifestStat.st_mode)) return nil;
    NSData *data = BoundedModelManifestData(manifestPath);
    id manifest = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
    return [manifest isKindOfClass:NSDictionary.class] ? manifest : nil;
}
} // namespace

BOOL MSIMELocalVoiceModelDirectory(NSString *path) {
    return ModelManifest(path) != nil;
}

NSString *MSIMELocalVoiceHelperPath(void) {
    NSFileManager *files = NSFileManager.defaultManager;
    NSString *configured = NSProcessInfo.processInfo.environment[@"MSIME_VOICE_LOCAL_HELPER"];
    if (configured.length) return configured.isAbsolutePath && [files isExecutableFileAtPath:configured] ? configured : nil;
    NSString *bundled = [NSBundle.mainBundle.executablePath.stringByDeletingLastPathComponent stringByAppendingPathComponent:@"msime-voice-local"];
    return bundled.isAbsolutePath && [files isExecutableFileAtPath:bundled] ? bundled : nil;
}

@interface MSIMELocalVoiceRequest ()
- (void)helperMessage:(NSDictionary *)message;
- (void)helperFailed:(NSError *)error session:(NSNumber *)session;
@end

// The one helper process this input method talks to, and the pipe to it. Everything that touches the process or writes to it runs on `queue`, which also orders a session's start, audio and finish exactly as the request issued them; what the helper prints is handed to the active request on main.
@interface MSIMELocalVoiceHelper : NSObject
@property(nonatomic, readonly) dispatch_queue_t queue;
@property(nonatomic, weak) MSIMELocalVoiceRequest *activeRequest; // main only
+ (instancetype)shared;
- (BOOL)startSession:(NSNumber *)session request:(MSIMELocalVoiceRequest *)request message:(NSDictionary *)message error:(NSError **)error;
- (void)sendMessage:(NSDictionary *)message session:(NSNumber *)session;
- (void)endSession:(NSNumber *)session;
@end

@implementation MSIMELocalVoiceHelper {
    NSTask *_task;
    NSFileHandle *_input;
    uint64_t _generation;
    // The session whose start went to the running process, so that process ending can fail it.
    NSNumber *_session;
    __weak MSIMELocalVoiceRequest *_sessionRequest;
}

+ (instancetype)shared {
    static MSIMELocalVoiceHelper *helper;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ helper = [MSIMELocalVoiceHelper new]; });
    return helper;
}

- (instancetype)init {
    if (!(self = [super init])) return nil;
    _queue = dispatch_queue_create("app.msime.client.voice.local", dispatch_queue_attr_make_with_qos_class(DISPATCH_QUEUE_SERIAL, QOS_CLASS_USER_INITIATED, 0));
    return self;
}

- (void)route:(NSDictionary *)message {
    [self.activeRequest helperMessage:message];
}

- (BOOL)launch:(NSError **)error {
    NSString *path = MSIMELocalVoiceHelperPath();
    if (!path) {
        if (error) *error = LocalVoiceFailure(@"找不到本地识别组件 msime-voice-local，请重新安装输入法。");
        return NO;
    }
    NSTask *task = [NSTask new];
    task.executableURL = [NSURL fileURLWithPath:path];
    NSPipe *input = [NSPipe pipe];
    NSPipe *output = [NSPipe pipe];
    task.standardInput = input;
    task.standardOutput = output;
    task.standardError = NSFileHandle.fileHandleWithNullDevice;
    task.qualityOfService = NSQualityOfServiceUserInitiated;
    const uint64_t generation = ++_generation;
    __weak MSIMELocalVoiceHelper *weakSelf = self;
    NSMutableData *pending = [NSMutableData data];
    __block BOOL droppingOversizedLine = NO;
    output.fileHandleForReading.readabilityHandler = ^(NSFileHandle *handle) {
        NSData *chunk = handle.availableData;
        if (!chunk.length) { handle.readabilityHandler = nil; return; }
        NSMutableArray<NSDictionary *> *messages = [NSMutableArray array];
        @synchronized(pending) {
            [pending appendData:chunk];
            for (;;) {
                if (!pending.length) break;
                const char *bytes = static_cast<const char *>(pending.bytes);
                const void *newline = std::memchr(bytes, '\n', pending.length);
                if (!newline) {
                    if (droppingOversizedLine || pending.length > MaximumHelperLine) {
                        pending.length = 0;
                        droppingOversizedLine = YES;
                    }
                    break;
                }
                const NSUInteger length = static_cast<NSUInteger>(static_cast<const char *>(newline) - bytes);
                if (droppingOversizedLine || length > MaximumHelperLine) {
                    [pending replaceBytesInRange:NSMakeRange(0, length + 1) withBytes:nullptr length:0];
                    droppingOversizedLine = NO;
                    continue;
                }
                id message = [NSJSONSerialization JSONObjectWithData:[pending subdataWithRange:NSMakeRange(0, length)] options:0 error:nil];
                if ([message isKindOfClass:NSDictionary.class]) [messages addObject:message];
                [pending replaceBytesInRange:NSMakeRange(0, length + 1) withBytes:nullptr length:0];
            }
        }
        if (messages.count)
            dispatch_async(dispatch_get_main_queue(), ^{
                for (NSDictionary *message in messages) [weakSelf route:message];
            });
    };
    task.terminationHandler = ^(NSTask *ended) {
        (void)ended;
        MSIMELocalVoiceHelper *helper = weakSelf;
        if (helper) dispatch_async(helper->_queue, ^{ [helper process:generation endedWithDetail:@"本地识别进程意外退出。"]; });
    };
    NSError *launchError = nil;
    if (![task launchAndReturnError:&launchError]) {
        output.fileHandleForReading.readabilityHandler = nil;
        if (error) *error = LocalVoiceFailure(@"无法启动本地识别组件 msime-voice-local。");
        return NO;
    }
    // A helper that has died between two writes must fail the write, not raise SIGPIPE in the input method.
    fcntl(input.fileHandleForWriting.fileDescriptor, F_SETNOSIGPIPE, 1);
    _task = task;
    _input = input.fileHandleForWriting;
    _session = nil;
    _sessionRequest = nil;
    return YES;
}

// The running process is gone, or is being given up on: forget it and fail the session it was serving.
- (void)process:(uint64_t)generation endedWithDetail:(NSString *)detail {
    if (generation != _generation || !_task) return;
    if (_task.running) [_task terminate];
    [_input closeFile];
    _task = nil;
    _input = nil;
    NSNumber *session = _session;
    MSIMELocalVoiceRequest *request = _sessionRequest;
    _session = nil;
    _sessionRequest = nil;
    if (session && request)
        dispatch_async(dispatch_get_main_queue(), ^{ [request helperFailed:LocalVoiceFailure(detail) session:session]; });
}

- (BOOL)write:(NSDictionary *)message error:(NSError **)error {
    NSMutableData *line = [[NSJSONSerialization dataWithJSONObject:message options:0 error:nil] mutableCopy];
    if (!line) {
        if (error) *error = LocalVoiceFailure();
        return NO;
    }
    [line appendBytes:"\n" length:1];
    const int descriptor = _input.fileDescriptor;
    const uint8_t *bytes = static_cast<const uint8_t *>(line.bytes);
    size_t remaining = line.length;
    while (remaining) {
        const ssize_t written = ::write(descriptor, bytes, remaining);
        if (written < 0 && errno == EINTR) continue;
        if (written <= 0) {
            [self process:_generation endedWithDetail:@"本地识别进程已停止响应。"];
            if (error) *error = LocalVoiceFailure(@"本地识别进程已停止响应。");
            return NO;
        }
        bytes += written;
        remaining -= static_cast<size_t>(written);
    }
    return YES;
}

- (BOOL)startSession:(NSNumber *)session request:(MSIMELocalVoiceRequest *)request message:(NSDictionary *)message error:(NSError **)error {
    if (!_task && ![self launch:error]) return NO;
    if (![self write:message error:error]) return NO;
    _session = session;
    _sessionRequest = request;
    return YES;
}

- (void)sendMessage:(NSDictionary *)message session:(NSNumber *)session {
    if (!_task || ![_session isEqual:session]) return;
    [self write:message error:nil];
}

- (void)endSession:(NSNumber *)session {
    if (![_session isEqual:session]) return;
    _session = nil;
    _sessionRequest = nil;
}
@end

namespace {
enum class LocalRequestState : int { idle, streaming, finishing, closed };
}

@implementation MSIMELocalVoiceRequest {
    NSString *_model;
    NSString *_language;
    NSString *_hotwordMode;
    NSDictionary *_hostOptions;
    NSNumber *_session;
    MSIMEDoubaoResult _result; // main only
    std::atomic<LocalRequestState> _state;
    NSArray<NSDictionary *> *_hotwords; // helper queue only
}

- (instancetype)initWithOptions:(NSDictionary *)options hostOptions:(NSDictionary *)hostOptions error:(NSError **)error {
    if (!(self = [super init])) return nil;
    id model = options[@"asr_model_path"];
    NSDictionary *manifest = [model isKindOfClass:NSString.class] ? ModelManifest(model) : nil;
    if (!manifest) {
        if (error) *error = LocalVoiceFailure(@"本地语音模型未安装或已被移动，请在语音设置中重新下载。");
        return nil;
    }
    if (!MSIMELocalVoiceHelperPath()) {
        if (error) *error = LocalVoiceFailure(@"找不到本地识别组件 msime-voice-local，请重新安装输入法。");
        return nil;
    }
    _model = [model copy];
    id language = options[@"language"];
    _language = [language isKindOfClass:NSString.class] ? [language copy] : @"";
    id hotwords = manifest[@"hotwords"];
    _hotwordMode = [hotwords isKindOfClass:NSString.class] ? [hotwords copy] : @"";
    _hostOptions = [hostOptions isKindOfClass:NSDictionary.class] ? [hostOptions copy] : nil;
    _state.store(LocalRequestState::idle);
    return self;
}

- (void)dealloc {
    const LocalRequestState previous = _state.exchange(LocalRequestState::closed);
    if (previous == LocalRequestState::streaming || previous == LocalRequestState::finishing) {
        MSIMELocalVoiceHelper *helper = MSIMELocalVoiceHelper.shared;
        NSNumber *session = _session;
        dispatch_async(helper.queue, ^{
            [helper sendMessage:@{@"op" : @"cancel"} session:session];
            [helper endSession:session];
        });
    }
}

- (BOOL)startWithResult:(MSIMEDoubaoResult)result error:(NSError **)error {
    LocalRequestState expected = LocalRequestState::idle;
    if (!result || !_state.compare_exchange_strong(expected, LocalRequestState::streaming)) {
        if (error) *error = LocalVoiceFailure();
        return NO;
    }
    static uint64_t sessions = 0;
    _session = @(++sessions);
    _result = [result copy];
    MSIMELocalVoiceHelper *helper = MSIMELocalVoiceHelper.shared;
    helper.activeRequest = self;
    NSNumber *session = _session;
    NSDictionary *hostOptions = _hostOptions;
    const BOOL native = [_hotwordMode isEqual:@"native"];
    const BOOL pinyin = [_hotwordMode isEqual:@"pinyin"];
    NSDictionary *start = @{@"op" : @"start", @"id" : session, @"model" : _model, @"language" : _language, @"threads" : @0};
    __weak MSIMELocalVoiceRequest *weakSelf = self;
    dispatch_async(helper.queue, ^{
        MSIMELocalVoiceRequest *request = weakSelf;
        if (!request || request->_state.load() == LocalRequestState::closed) return;
        // Read here rather than on main: the dictionary lives behind the same store the rest of the session uses, and the first audio chunks queue behind this start instead of being dropped by a helper that has no session yet.
        NSArray<NSDictionary *> *hotwords = (native || pinyin) && hostOptions ? FetchHotwords(hostOptions) : @[];
        request->_hotwords = pinyin ? hotwords : @[];
        NSMutableDictionary *message = [start mutableCopy];
        if (native) message[@"hotwords"] = [hotwords valueForKey:@"text"];
        NSError *failure = nil;
        if (![helper startSession:session request:request message:message error:&failure])
            dispatch_async(dispatch_get_main_queue(), ^{ [request helperFailed:failure session:session]; });
    });
    return YES;
}

- (BOOL)appendPCM:(NSData *)pcm error:(NSError **)error {
    if (_state.load() != LocalRequestState::streaming || pcm.length % sizeof(float)) {
        if (error) *error = LocalVoiceFailure();
        return NO;
    }
    if (!pcm.length) return YES;
    const float *samples = static_cast<const float *>(pcm.bytes);
    const NSUInteger count = pcm.length / sizeof(float);
    std::vector<uint8_t> bytes(count * 2);
    for (NSUInteger index = 0; index < count; ++index) {
        const float sample = std::isfinite(samples[index]) ? std::fmax(-1.0f, std::fmin(1.0f, samples[index])) : 0.0f;
        const auto value = static_cast<uint16_t>(static_cast<int16_t>(std::lrint(sample * 32767.0f)));
        bytes[2 * index] = static_cast<uint8_t>(value & 0xFF);
        bytes[2 * index + 1] = static_cast<uint8_t>(value >> 8);
    }
    NSString *encoded = [[NSData dataWithBytes:bytes.data() length:bytes.size()] base64EncodedStringWithOptions:0];
    MSIMELocalVoiceHelper *helper = MSIMELocalVoiceHelper.shared;
    NSNumber *session = _session;
    dispatch_async(helper.queue, ^{ [helper sendMessage:@{@"op" : @"audio", @"pcm16" : encoded} session:session]; });
    return YES;
}

- (BOOL)finishWithError:(NSError **)error {
    LocalRequestState expected = LocalRequestState::streaming;
    if (!_state.compare_exchange_strong(expected, LocalRequestState::finishing)) {
        if (error) *error = LocalVoiceFailure();
        return NO;
    }
    MSIMELocalVoiceHelper *helper = MSIMELocalVoiceHelper.shared;
    NSNumber *session = _session;
    dispatch_async(helper.queue, ^{ [helper sendMessage:@{@"op" : @"finish"} session:session]; });
    return YES;
}

- (void)cancel {
    const LocalRequestState previous = _state.exchange(LocalRequestState::closed);
    _result = nil;
    MSIMELocalVoiceHelper *helper = MSIMELocalVoiceHelper.shared;
    if (helper.activeRequest == self) helper.activeRequest = nil;
    if (previous != LocalRequestState::streaming && previous != LocalRequestState::finishing) return;
    NSNumber *session = _session;
    dispatch_async(helper.queue, ^{
        [helper sendMessage:@{@"op" : @"cancel"} session:session];
        [helper endSession:session];
    });
}

// Ends the request with its last callback. Later helper messages for this session find it closed.
- (void)closeWithText:(NSString *)text error:(NSError *)error {
    _state.store(LocalRequestState::closed);
    MSIMELocalVoiceHelper *helper = MSIMELocalVoiceHelper.shared;
    if (helper.activeRequest == self) helper.activeRequest = nil;
    NSNumber *session = _session;
    dispatch_async(helper.queue, ^{ [helper endSession:session]; });
    MSIMEDoubaoResult result = _result;
    _result = nil;
    if (result) result(error ? nil : (text ?: @""), !error, error);
}

- (void)helperMessage:(NSDictionary *)message {
    if (![message[@"id"] isEqual:_session]) return;
    const LocalRequestState state = _state.load();
    if (state != LocalRequestState::streaming && state != LocalRequestState::finishing) return;
    NSString *type = [message[@"type"] isKindOfClass:NSString.class] ? message[@"type"] : @"";
    NSString *text = [message[@"text"] isKindOfClass:NSString.class] ? message[@"text"] : @"";
    if ([type isEqual:@"partial"]) {
        if (_result) _result(text, NO, nil);
    } else if ([type isEqual:@"final"]) {
        _state.store(LocalRequestState::closed);
        MSIMELocalVoiceHelper *helper = MSIMELocalVoiceHelper.shared;
        if (!text.length) { [self closeWithText:text error:nil]; return; }
        // Pinyin correction reads the hotwords the start fetched, which only the helper queue touches.
        __weak MSIMELocalVoiceRequest *weakSelf = self;
        dispatch_async(helper.queue, ^{
            MSIMELocalVoiceRequest *request = weakSelf;
            if (!request) return;
            NSArray<NSDictionary *> *hotwords = request->_hotwords;
            NSString *corrected = hotwords.count ? CorrectedText(text, hotwords) : text;
            dispatch_async(dispatch_get_main_queue(), ^{ [weakSelf closeWithText:corrected error:nil]; });
        });
    } else if ([type isEqual:@"error"] || [type isEqual:@"cancelled"]) {
        id detail = message[@"message"];
        [self closeWithText:nil error:LocalVoiceFailure([detail isKindOfClass:NSString.class] ? detail : nil)];
    }
}

- (void)helperFailed:(NSError *)error session:(NSNumber *)session {
    if (![session isEqual:_session]) return;
    const LocalRequestState state = _state.load();
    if (state != LocalRequestState::streaming && state != LocalRequestState::finishing) return;
    [self closeWithText:nil error:error ?: LocalVoiceFailure()];
}
@end
