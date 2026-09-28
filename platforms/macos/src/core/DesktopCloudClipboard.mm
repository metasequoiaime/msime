#import "DesktopCloudClipboard.h"
#import "DesktopSettingsLauncher.h"
#import "DesktopInputSession.h"
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/stat.h>
#include <arpa/inet.h>
#include <fcntl.h>
#include <unistd.h>
#include <atomic>
#include <memory>

@protocol MSIMEDesktopCloudClipboardPreparing
+ (void)prepareWithCompletion:(void (^)(id<MSIMEDesktopCloudClipboardProvider>))completion;
@end

namespace {
constexpr size_t requestLimit = 64 * 1024;
constexpr size_t responseLimit = 2 * 1024 * 1024;
bool Read(int fd, void *bytes, size_t length, double deadline) {
    auto *cursor = static_cast<char *>(bytes);
    while (length && NSProcessInfo.processInfo.systemUptime < deadline) {
        ssize_t count = recv(fd, cursor, length, 0);
        if (count < 0 && errno == EINTR) continue;
        if (count <= 0) return false;
        cursor += count; length -= size_t(count);
    }
    return length == 0;
}
bool Write(int fd, const void *bytes, size_t length) {
    const auto *cursor = static_cast<const char *>(bytes);
    while (length) {
        ssize_t count = send(fd, cursor, length, 0);
        if (count < 0 && errno == EINTR) continue;
        if (count <= 0) return false;
        cursor += count; length -= size_t(count);
    }
    return true;
}
struct Reply {
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    std::atomic<bool> live{true};
    NSData *data = nil;
    NSProgress *progress = nil;
};
}

@implementation MSIMEDesktopCloudClipboardSession {
    dispatch_source_t _source, _timer;
    dispatch_queue_t _queue;
    NSDictionary *_launchEnvironment;
    id<MSIMEDesktopCloudClipboardProvider> _provider;
    MSIMEDesktopCloudClipboardSession *_keepAlive;
    pid_t _peer;
    BOOL (^_valid)(void);
    std::atomic<bool> _stopped;
    BOOL _dictionary;
}
- (instancetype)initWithProvider:(id<MSIMEDesktopCloudClipboardProvider>)provider {
    return [self initWithProvider:provider dictionary:NO];
}
- (instancetype)initWithProvider:(id<MSIMEDesktopCloudClipboardProvider>)provider dictionary:(BOOL)dictionary {
    if (!(self = [super init])) return nil;
    _stopped.store(false);
    _dictionary = dictionary;
    if (!provider) return nil;
    char directory[] = "/tmp/msime-cloud-XXXXXX";
    if (!mkdtemp(directory)) return nil;
    NSString *root = [NSString stringWithUTF8String:directory];
    NSString *path = [root stringByAppendingPathComponent:@"clipboard.sock"];
    int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    if (listener < 0) { rmdir(directory); return nil; }
    sockaddr_un address{}; address.sun_family = AF_UNIX; address.sun_len = sizeof(address);
    strlcpy(address.sun_path, path.fileSystemRepresentation, sizeof(address.sun_path));
    if (bind(listener, reinterpret_cast<sockaddr *>(&address), sizeof(address)) ||
        chmod(address.sun_path, 0600) || listen(listener, 4) || fcntl(listener, F_SETFL, O_NONBLOCK)) {
        close(listener); unlink(address.sun_path); rmdir(directory); return nil;
    }
    fcntl(listener, F_SETFD, FD_CLOEXEC);
    _provider = provider;
    NSData *configuration = [NSJSONSerialization dataWithJSONObject:@{@"version":@1, @"path":path, @"host_pid":@(getpid())} options:0 error:nil];
    _launchEnvironment = @{dictionary ? @"MSIME_CLIENT_CLOUD_DICTIONARY_SESSION" : @"MSIME_CLIENT_CLOUD_CLIPBOARD_SESSION":[[NSString alloc] initWithData:configuration encoding:NSUTF8StringEncoding]};
    _queue = dispatch_queue_create("app.msime.cloud-clipboard-session", DISPATCH_QUEUE_SERIAL);
    _source = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ, listener, 0, _queue);
    dispatch_source_set_cancel_handler(_source, ^{ close(listener); unlink(path.fileSystemRepresentation); rmdir(root.fileSystemRepresentation); });
    __weak MSIMEDesktopCloudClipboardSession *weakSelf = self;
    dispatch_source_set_event_handler(_source, ^{
        MSIMEDesktopCloudClipboardSession *session = weakSelf;
        if (!session || session->_stopped.load()) return;
        int fd = accept(listener, nullptr, nullptr);
        if (fd < 0) return;
        const int flags = fcntl(fd, F_GETFL, 0);
        if (flags < 0 || fcntl(fd, F_SETFL, flags & ~O_NONBLOCK) != 0) { close(fd); return; }
        fcntl(fd, F_SETFD, FD_CLOEXEC);
        timeval timeout{5, 0}; int noSignal = 1;
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof(timeout));
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &noSignal, sizeof(noSignal));
        [session receive:fd]; close(fd);
    });
    _timer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, _queue);
    dispatch_source_set_timer(_timer, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC), NSEC_PER_SEC, NSEC_PER_SEC / 10);
    const double started = NSProcessInfo.processInfo.systemUptime;
    dispatch_source_set_event_handler(_timer, ^{
        MSIMEDesktopCloudClipboardSession *session = weakSelf;
        if (!session) return;
        BOOL (^valid)(void); pid_t peer;
        @synchronized(session) { valid = session->_valid; peer = session->_peer; }
        if ((peer > 0 && (!valid || !valid())) || (peer <= 0 && NSProcessInfo.processInfo.systemUptime - started > 30)) [session stop];
    });
    // Own the transport until launch failure, explicit close, or peer exit.
    _keepAlive = self;
    dispatch_resume(_source); dispatch_resume(_timer);
    return self;
}
- (NSDictionary<NSString *, NSString *> *)launchEnvironment { return _launchEnvironment; }
- (void)authorizePID:(pid_t)pid stillValid:(BOOL (^)(void))valid {
    @synchronized(self) { _peer = pid; _valid = [valid copy]; }
}
- (void)receive:(int)fd {
    uid_t uid = 0; gid_t gid = 0; pid_t pid = 0; socklen_t size = sizeof(pid);
    pid_t peer; BOOL (^valid)(void);
    @synchronized(self) { peer = _peer; valid = _valid; }
    if (getpeereid(fd, &uid, &gid) || uid != getuid() ||
        getsockopt(fd, SOL_LOCAL, LOCAL_PEERPID, &pid, &size) || size != sizeof(pid) ||
        peer <= 0 || peer != pid || !valid || !valid()) return;
    double deadline = NSProcessInfo.processInfo.systemUptime + 10;
    uint32_t length = 0;
    if (!Read(fd, &length, sizeof(length), deadline)) return;
    length = ntohl(length);
    if (!length) { [self stop]; return; }
    if (length > (_dictionary ? 512 * 1024 : requestLimit)) return;
    NSMutableData *body = [NSMutableData dataWithLength:length];
    if (!Read(fd, body.mutableBytes, length, deadline)) return;
    char extra; if (recv(fd, &extra, 1, 0) != 0) return;
    id request = [NSJSONSerialization JSONObjectWithData:body options:0 error:nil];
    if (![request isKindOfClass:NSDictionary.class]) return;
    auto reply = std::make_shared<Reply>();
    dispatch_async(dispatch_get_main_queue(), ^{
        if (!reply->live.load() || self->_stopped.load() || !valid()) { dispatch_semaphore_signal(reply->done); return; }
        reply->progress = [self->_provider request:request completion:^(NSDictionary *result) {
            // Provider completions run on main; semaphore publishes the result.
            if (!reply->live.exchange(false)) return;
            reply->data = [NSJSONSerialization dataWithJSONObject:result options:0 error:nil];
            dispatch_semaphore_signal(reply->done);
        }];
    });
    const double responseDeadline = NSProcessInfo.processInfo.systemUptime +
        (_dictionary && [request[@"operation"] isEqual:@"export"] ? 605 : 35);
    bool completed = false;
    while (!_stopped.load() && valid() && NSProcessInfo.processInfo.systemUptime < responseDeadline) {
        if (dispatch_semaphore_wait(reply->done, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC / 10)) == 0) { completed = true; break; }
    }
    reply->live.store(false);
    if (!completed) { dispatch_async(dispatch_get_main_queue(), ^{ [reply->progress cancel]; }); return; }
    if (_stopped.load() || !valid() || !reply->data.length || reply->data.length > responseLimit) return;
    uint32_t responseLength = htonl(uint32_t(reply->data.length));
    if (Write(fd, &responseLength, sizeof(responseLength))) Write(fd, reply->data.bytes, reply->data.length);
}
- (void)stop {
    if (_stopped.exchange(true)) return;
    if (_source) dispatch_source_cancel(_source);
    if (_timer) dispatch_source_cancel(_timer);
    _keepAlive = nil;
}
- (void)dealloc { [self stop]; }
@end

void MSIMEOpenDesktopCloudClipboard(NSString *optionsPath, NSWorkspace *workspace, dispatch_block_t fallback) {
    MSIMEOpenDesktopCloudClipboardWithInput(optionsPath, workspace, nil, fallback);
}

void MSIMEOpenDesktopCloudDictionary(NSString *optionsPath, NSWorkspace *workspace, dispatch_block_t fallback) {
    Class bridge = NSClassFromString(@"MSIMEBackendCloudDictionaryProvider");
    if (![bridge respondsToSelector:@selector(prepareWithCompletion:)]) {
        os_log(MSIMEUILog(), "desktop_route_fallback route=cloud-dictionary reason=backend_bridge_missing");
        fallback();
        return;
    }
    [(Class<MSIMEDesktopCloudClipboardPreparing>)bridge prepareWithCompletion:^(id<MSIMEDesktopCloudClipboardProvider> provider) {
        if (!provider) {
            os_log(MSIMEUILog(), "desktop_route_fallback route=cloud-dictionary reason=no_provider");
            fallback();
            return;
        }
        MSIMEDesktopCloudClipboardSession *session = [[MSIMEDesktopCloudClipboardSession alloc] initWithProvider:provider dictionary:YES];
        if (!session) {
            os_log(MSIMEUILog(), "desktop_route_fallback route=cloud-dictionary reason=session_failed");
            fallback();
            return;
        }
        MSIMEOpenDesktopRouteWithContext(@"cloud-dictionary", optionsPath, session.launchEnvironment, workspace,
            ^(NSRunningApplication *application) { [session authorizePID:application.processIdentifier stillValid:^BOOL { return !application.terminated; }]; },
            ^{ [session stop]; fallback(); });
    }];
}

void MSIMEOpenDesktopCloudClipboardWithInput(NSString *optionsPath, NSWorkspace *workspace,
    MSIMEDesktopInputSession *inputSession, dispatch_block_t fallback) {
    Class bridge = NSClassFromString(@"MSIMEBackendCloudClipboardProvider");
    if (![bridge respondsToSelector:@selector(prepareWithCompletion:)]) {
        os_log(MSIMEUILog(), "desktop_route_fallback route=cloud-clipboard reason=backend_bridge_missing");
        fallback();
        return;
    }
    [(Class<MSIMEDesktopCloudClipboardPreparing>)bridge prepareWithCompletion:^(id<MSIMEDesktopCloudClipboardProvider> provider) {
        if (!provider) {
            os_log(MSIMEUILog(), "desktop_route_fallback route=cloud-clipboard reason=no_provider");
            fallback();
            return;
        }
        MSIMEDesktopCloudClipboardSession *session = [[MSIMEDesktopCloudClipboardSession alloc] initWithProvider:provider];
        if (!session) {
            os_log(MSIMEUILog(), "desktop_route_fallback route=cloud-clipboard reason=session_failed");
            fallback();
            return;
        }
        NSMutableDictionary *environment = [session.launchEnvironment mutableCopy];
        if (inputSession) [environment addEntriesFromDictionary:inputSession.launchEnvironment];
        MSIMEOpenDesktopRouteWithContext(@"cloud-clipboard", optionsPath, environment, workspace,
            ^(NSRunningApplication *application) {
                [session authorizePID:application.processIdentifier stillValid:^BOOL { return !application.terminated; }];
                [inputSession authorizePID:application.processIdentifier stillValid:^BOOL { return !application.terminated; }];
            },
            ^{ [session stop]; fallback(); });
    }];
}
