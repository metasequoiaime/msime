#import "../../src/core/DesktopInputSession.h"
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/stat.h>
#include <arpa/inet.h>
#include <unistd.h>
#include <atomic>
#include <cerrno>
#include <cassert>
#include <thread>
#include <vector>

static bool SendAll(int fd, const void *bytes, size_t count) {
    const auto *cursor = static_cast<const unsigned char *>(bytes);
    while (count > 0) {
        const ssize_t sent = send(fd, cursor, count, 0);
        if (sent < 0 && errno == EINTR) continue;
        if (sent <= 0) return false;
        cursor += sent;
        count -= static_cast<size_t>(sent);
    }
    return true;
}

static void Check(std::vector<unsigned char> body, uint32_t declared, bool authorized,
                  bool accept, bool expectedCall, int expectedResponse, bool clipboard = false, bool fragmented = false) {
    __block unsigned calls = 0;
    MSIMEDesktopInputSession *session = [[MSIMEDesktopInputSession alloc]
        initWithTargetPID:getpid() launchTime:42 clipboard:clipboard handler:^(NSString *text, double deadline,
                                                        MSIMEPanelTextCompletion completion) {
            assert(NSThread.isMainThread);
            assert([text isEqualToString:[[NSString alloc] initWithBytes:body.data() length:body.size() encoding:NSUTF8StringEncoding]]);
            assert(deadline > NSProcessInfo.processInfo.systemUptime);
            ++calls;
            completion(accept);
            completion(!accept); // A duplicate completion cannot reverse the result.
        }];
    assert(session);
    [session authorizePID:authorized ? getpid() : getpid() + 1 stillValid:^BOOL { return YES; }];
    NSData *json = [session.launchEnvironment[@"MSIME_CLIENT_PANEL_SESSION"] dataUsingEncoding:NSUTF8StringEncoding];
    NSDictionary *configuration = [NSJSONSerialization JSONObjectWithData:json options:0 error:nil];
    NSString *path = configuration[@"path"];
    struct stat info{};
    assert(stat(path.fileSystemRepresentation, &info) == 0 && (info.st_mode & 0777) == 0600);
    assert(stat(path.stringByDeletingLastPathComponent.fileSystemRepresentation, &info) == 0 && (info.st_mode & 0777) == 0700);
    std::atomic<bool> done{false};
    std::thread worker([&] {
        @autoreleasepool {
            int fd = socket(AF_UNIX, SOCK_STREAM, 0);
            assert(fd >= 0);
            int noSignal = 1;
            setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &noSignal, sizeof(noSignal));
            timeval timeout{5, 0};
            setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));
            sockaddr_un address{};
            address.sun_family = AF_UNIX; address.sun_len = sizeof(address);
            strlcpy(address.sun_path, path.fileSystemRepresentation, sizeof(address.sun_path));
            assert(connect(fd, reinterpret_cast<sockaddr *>(&address), sizeof(address)) == 0);
            uint32_t length = htonl(declared);
            // 未授权连接可能在发送期间被关闭；有效请求必须完整送出测试帧。
            const bool headerSent = SendAll(fd, &length, sizeof(length));
            if (expectedCall) assert(headerSent);
            if (fragmented) std::this_thread::sleep_for(std::chrono::milliseconds(10));
            const bool bodySent = body.empty() || SendAll(fd, body.data(), body.size());
            if (expectedCall) assert(bodySent);
            shutdown(fd, SHUT_WR);
            unsigned char response = 255;
            ssize_t count = recv(fd, &response, 1, 0);
            if (expectedResponse >= 0 && (count != 1 || response != expectedResponse))
                fprintf(stderr, "synthetic frame declared=%u clipboard=%d response=%d count=%zd\n", declared, clipboard, response, count);
            if (expectedResponse < 0) assert(count <= 0);
            else assert(count == 1 && response == expectedResponse);
            close(fd);
            done.store(true);
        }
    });
    NSDate *timeout = [NSDate dateWithTimeIntervalSinceNow:8];
    while (!done.load() && timeout.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(done.load());
    worker.join();
    assert(calls == unsigned(expectedCall));
    [session stop];
    NSString *directory = path.stringByDeletingLastPathComponent;
    for (int attempt = 0; attempt < 100 &&
         (access(path.fileSystemRepresentation, F_OK) == 0 ||
          access(directory.fileSystemRepresentation, F_OK) == 0); ++attempt)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(access(path.fileSystemRepresentation, F_OK) != 0);
    assert(access(directory.fileSystemRepresentation, F_OK) != 0);
}

int main() {
    @autoreleasepool {
        const std::vector<unsigned char> text{'s','y','n','t','h','e','t','i','c'};
        Check(text, 9, true, true, true, 0);
        Check(text, 9, true, false, true, 1);
        Check(text, 9, false, true, false, -1);
        Check({}, 0, true, true, false, -1);
        Check({}, 4097, true, true, false, 1);
        Check({0xff}, 1, true, true, false, -1);
        Check({0}, 1, true, true, false, -1);
        Check(text, 8, true, true, false, -1); // Trailing bytes.
        Check(text, 10, true, true, false, -1); // Truncated frame.
        Check(text, 9, true, true, true, 0, true);
        Check({}, 12001, true, true, false, 1, true);
        Check(std::vector<unsigned char>(4001, 'a'), 4001, true, true, false, 1, true);
        Check({'a','\n','\r','\t'}, 4, true, true, true, 0, true);
        Check({'a',0x1b,0x0c,0x7f}, 4, true, true, true, 0, true); // Clipboard control characters other than NUL are user content.
        Check({'a',0}, 2, true, true, false, -1, true);
        Check({0xff}, 1, true, true, false, -1, true);
        std::vector<unsigned char> cjk;
        for (int i = 0; i < 4000; ++i) cjk.insert(cjk.end(), {0xe4, 0xb8, 0xad});
        Check(cjk, 12000, true, true, true, 0, true);
        Check(cjk, 12000, true, true, true, 0, true, true);
        Check(text, 9, true, true, true, 0, false, true);
        NSData *joinedEmoji = [@"👩‍💻" dataUsingEncoding:NSUTF8StringEncoding];
        const auto *emojiBytes = static_cast<const unsigned char *>(joinedEmoji.bytes);
        Check(std::vector<unsigned char>(emojiBytes, emojiBytes + joinedEmoji.length), uint32_t(joinedEmoji.length), true, true, true, 0, true);
        Check({0xc2, 0x85}, 2, true, true, true, 0, true);
        Check({}, 12000, true, true, false, 1); // Candidate sessions reject the oversized header.
    }
}
