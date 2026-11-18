#import "DesktopInputSession.h"
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/stat.h>
#include <unistd.h>
#include <fcntl.h>
#include <arpa/inet.h>
#include <poll.h>
#include <atomic>
#include <memory>
#include <cmath>
#include <cstring>

namespace {
constexpr uint32_t limit = 4096;
constexpr int receiveTimeoutMilliseconds = 3000;

static bool WaitForReadable(int fd) {
    pollfd event{fd, POLLIN, 0};
    for (;;) {
        const int result = poll(&event, 1, receiveTimeoutMilliseconds);
        if (result < 0 && errno == EINTR) continue;
        return result > 0 && (event.revents & (POLLIN | POLLHUP | POLLERR));
    }
}

bool ReadAll(int fd, void *bytes, size_t count) {
    auto *cursor = static_cast<char *>(bytes);
    while (count) {
        const ssize_t n = recv(fd, cursor, count, 0);
        if (n < 0 && errno == EINTR) continue;
        if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            // Darwin 从非阻塞监听 socket 继承的连接可能在清除标志前先到这里，等待下一段帧数据。
            if (!WaitForReadable(fd)) return false;
            continue;
        }
        if (n <= 0) return false;
        cursor += n; count -= static_cast<size_t>(n);
    }
    return true;
}
struct Submission {
    std::atomic<int> result{-1};
    std::atomic<bool> live{true};
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
};
}

@implementation MSIMEDesktopInputSession {
    dispatch_source_t _source;
    dispatch_queue_t _queue;
    NSString *_directory;
    NSString *_path;
    NSDictionary *_launchEnvironment;
    MSIMEPanelTextHandler _handler;
    pid_t _peer;
    BOOL (^_peerValid)(void);
    std::atomic<bool> _stopped;
    BOOL _clipboard;
}
- (instancetype)initWithTargetPID:(pid_t)pid launchTime:(double)launched handler:(MSIMEPanelTextHandler)handler {
    return [self initWithTargetPID:pid launchTime:launched clipboard:NO handler:handler];
}
- (instancetype)initWithTargetPID:(pid_t)pid launchTime:(double)launched clipboard:(BOOL)clipboard handler:(MSIMEPanelTextHandler)handler {
    if (!(self = [super init])) return nil;
    _stopped.store(false);
    _clipboard = clipboard;
    if (pid <= 0 || !std::isfinite(launched) || launched <= 0 || !handler) return nil;
    char directory[] = "/tmp/msime-panel-XXXXXX";
    if (!mkdtemp(directory)) return nil;
    _directory = [NSString stringWithUTF8String:directory];
    _path = [_directory stringByAppendingPathComponent:@"input.sock"];
    const int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    if (listener < 0) { rmdir(directory); return nil; }
    sockaddr_un address{};
    address.sun_family = AF_UNIX;
    address.sun_len = sizeof(address);
    strlcpy(address.sun_path, _path.fileSystemRepresentation, sizeof(address.sun_path));
    if (bind(listener, reinterpret_cast<sockaddr *>(&address), sizeof(address)) != 0 ||
        chmod(address.sun_path, 0600) != 0 || listen(listener, 4) != 0 ||
        fcntl(listener, F_SETFL, O_NONBLOCK) != 0) {
        close(listener); unlink(address.sun_path); rmdir(directory); return nil;
    }
    fcntl(listener, F_SETFD, FD_CLOEXEC);
    _handler = [handler copy];
    NSMutableDictionary *configuration = [@{
        @"version":@1, @"path":_path, @"host_pid":@(getpid()),
        @"target_pid":@(pid), @"target_started":@(launched)
    } mutableCopy];
    if (clipboard) configuration[@"clipboard"] = @YES;
    NSData *data = [NSJSONSerialization dataWithJSONObject:configuration options:0 error:nil];
    _launchEnvironment = @{@"MSIME_CLIENT_PANEL_SESSION":[[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding]};
    _queue = dispatch_queue_create("app.msime.panel-input", DISPATCH_QUEUE_SERIAL);
    _source = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ, listener, 0, _queue);
    NSString *path = _path, *root = _directory;
    dispatch_source_set_cancel_handler(_source, ^{
        close(listener); unlink(path.fileSystemRepresentation); rmdir(root.fileSystemRepresentation);
    });
    __weak MSIMEDesktopInputSession *weakSelf = self;
    dispatch_source_set_event_handler(_source, ^{
        MSIMEDesktopInputSession *session = weakSelf;
        if (!session || session->_stopped.load()) return;
        const int fd = accept(listener, nullptr, nullptr);
        if (fd < 0) return;
        // Darwin inherits O_NONBLOCK from the listener. Framed reads must wait
        // for later chunks, bounded by SO_RCVTIMEO, rather than fail on EAGAIN.
        const int flags = fcntl(fd, F_GETFL, 0);
        if (flags < 0 || fcntl(fd, F_SETFL, flags & ~O_NONBLOCK) != 0) { close(fd); return; }
        fcntl(fd, F_SETFD, FD_CLOEXEC);
        timeval timeout{receiveTimeoutMilliseconds / 1000, 0};
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof(timeout));
        const int noSignal = 1;
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &noSignal, sizeof(noSignal));
        [session receive:fd];
        close(fd);
    });
    dispatch_resume(_source);
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 5 * 60 * NSEC_PER_SEC), _queue, ^{ [weakSelf stop]; });
    return self;
}
- (NSDictionary<NSString *, NSString *> *)launchEnvironment { return _launchEnvironment; }
- (void)authorizePID:(pid_t)pid stillValid:(BOOL (^)(void))valid {
    @synchronized (self) { _peer = pid; _peerValid = [valid copy]; }
}
- (BOOL)isAuthorizedPeerAlive {
    BOOL (^valid)(void);
    @synchronized (self) { valid = _peerValid; }
    return !_stopped.load() && valid && valid();
}
- (void)receive:(int)fd {
    uid_t uid = 0; gid_t gid = 0; pid_t pid = 0; socklen_t size = sizeof(pid);
    pid_t peer; BOOL (^valid)(void);
    @synchronized (self) { peer = _peer; valid = _peerValid; }
    if (getpeereid(fd, &uid, &gid) != 0 || uid != getuid() ||
        getsockopt(fd, SOL_LOCAL, LOCAL_PEERPID, &pid, &size) != 0 ||
        size != sizeof(pid) || peer <= 0 || pid != peer || !valid || !valid()) return;
    uint32_t length = 0;
    if (!ReadAll(fd, &length, sizeof(length))) return;
    length = ntohl(length);
    if (!length) { [self stop]; return; }
    if (length > (_clipboard ? 12000 : limit)) { const char failure = 1; send(fd, &failure, 1, 0); return; }
    NSMutableData *body = [NSMutableData dataWithLength:length];
    if (!ReadAll(fd, body.mutableBytes, length)) return;
    char extra;
    for (;;) {
        const ssize_t received = recv(fd, &extra, 1, 0);
        if (received < 0 && errno == EINTR) continue;
        if (received < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            if (!WaitForReadable(fd)) return;
            continue;
        }
        if (received != 0 || memchr(body.bytes, 0, length)) return;
        break;
    }
    NSString *text = [[NSString alloc] initWithData:body encoding:NSUTF8StringEncoding];
    if (!text.length || _stopped.load() || !valid()) return;
    if (_clipboard) {
        // Clipboard history keeps every control character except NUL as user content, matching the Windows store; NUL is already rejected above.
        if (text.length > 4000) { const char failure = 1; send(fd, &failure, 1, 0); return; }
    }
    auto submission = std::make_shared<Submission>();
    const double deadline = NSProcessInfo.processInfo.systemUptime + 2;
    dispatch_async(dispatch_get_main_queue(), ^{
        if (!submission->live.load() || self->_stopped.load() ||
            NSProcessInfo.processInfo.systemUptime >= deadline || !valid()) {
            submission->result.store(1); dispatch_semaphore_signal(submission->done); return;
        }
        self->_handler(text, deadline, ^(BOOL committed) {
            int expected = -1;
            if (submission->result.compare_exchange_strong(expected, committed ? 0 : 1))
                dispatch_semaphore_signal(submission->done);
        });
    });
    const bool answered = dispatch_semaphore_wait(submission->done,
        dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC)) == 0;
    submission->live.store(false);
    const char result = answered && submission->result.load() == 0 ? 0 : 1;
    send(fd, &result, 1, 0);
    [self stop];
}
- (void)stop {
    if (_stopped.exchange(true)) return;
    if (_source) dispatch_source_cancel(_source);
}
- (void)dealloc { [self stop]; }
@end
