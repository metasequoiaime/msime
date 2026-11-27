#include "skin_folder_copy.h"
#include <dirent.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#include <cerrno>
#include <cstdint>
#include <cstring>
#ifdef MSIME_SKIN_COPY_TESTING
#include <functional>
#endif

namespace {
struct Fd {
    explicit Fd(int fd) : value(fd) {}
    ~Fd() { if (value >= 0) close(value); }
    Fd(const Fd &) = delete;
    Fd &operator=(const Fd &) = delete;
    int value;
};

struct Dir {
    explicit Dir(int fd) : value(fdopendir(fd)) { if (!value && fd >= 0) close(fd); }
    ~Dir() { if (value) closedir(value); }
    DIR *value;
};

struct Budget {
    unsigned entries = 4096;
    uint64_t bytes = 256ULL * 1024 * 1024;
};

#ifdef MSIME_SKIN_COPY_TESTING
using Hook = std::function<void(int, const char *)>;
static Hook afterOpened = [](int, const char *) {};
#else
struct Hook { void operator()(int, const char *) const {} };
#endif

// 子项只能是单段名称，否则 openat 的 NOFOLLOW 不保护中间路径。
bool singleName(const char *name) {
    return name && name[0] != '\0' && !strchr(name, '/')
        && strcmp(name, ".") != 0 && strcmp(name, "..") != 0;
}

bool same(const struct stat &first, const struct stat &second) {
    return first.st_ino == second.st_ino && first.st_dev == second.st_dev
        && ((first.st_mode & S_IFMT) == (second.st_mode & S_IFMT));
}

bool tree(int source, int destination, unsigned depth, Budget &limit, const Hook &beforeOpen) {
    if (depth > 16) return false;
    // 独立打开目录本身，避免 readdir 改变调用方目录 fd 的偏移。
    Dir directory(openat(source, ".", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC));
    if (!directory.value) return false;
    errno = 0;
    while (auto *entry = readdir(directory.value)) {
        const char *name = entry->d_name;
        if (!strcmp(name, ".") || !strcmp(name, "..")) continue;
        if (!singleName(name)) return false;
        if (limit.entries == 0) return false;
        --limit.entries;
        struct stat observed {};
        if (fstatat(source, name, &observed, AT_SYMLINK_NOFOLLOW)) return false;
        // Match the shared importer: links and special files are left behind without ever being opened, so nothing is followed.
        if (!S_ISDIR(observed.st_mode) && !S_ISREG(observed.st_mode)) continue;
#ifdef MSIME_SKIN_COPY_TESTING
        if (beforeOpen) beforeOpen(source, name);
#else
        beforeOpen(source, name);
#endif
        int flags = O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK;
        if (S_ISDIR(observed.st_mode)) flags |= O_DIRECTORY;
        Fd input(openat(source, name, flags));
        if (input.value < 0) return false;
        struct stat opened {};
        if (fstat(input.value, &opened) || !same(observed, opened)) return false;
#ifdef MSIME_SKIN_COPY_TESTING
        afterOpened(source, name);
#endif
        if (S_ISDIR(opened.st_mode)) {
            if (mkdirat(destination, name, 0700)) return false;
            Fd child(openat(destination, name, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC));
            if (child.value < 0 || !tree(input.value, child.value, depth + 1, limit, beforeOpen)) return false;
        } else {
            if (opened.st_nlink != 1 || opened.st_size < 0
                    || static_cast<uint64_t>(opened.st_size) > limit.bytes) return false;
            uint64_t remaining = static_cast<uint64_t>(opened.st_size);
            limit.bytes -= remaining;
            Fd output(openat(destination, name, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600));
            if (output.value < 0) return false;
            char buffer[16384];
            while (remaining > 0) {
                size_t want = remaining < sizeof(buffer) ? static_cast<size_t>(remaining) : sizeof(buffer);
                ssize_t count = read(input.value, buffer, want);
                if (count < 0 && errno == EINTR) continue;
                if (count <= 0 || static_cast<size_t>(count) > want) return false;
                ssize_t written = 0;
                while (written < count) {
                    ssize_t result = write(output.value, buffer + written, count - written);
                    if (result < 0 && errno == EINTR) continue;
                    if (result <= 0) return false;
                    written += result;
                }
                remaining -= static_cast<uint64_t>(count);
            }
            // 文件增长时不把额外数据写到暂存区，也不能把截断的文件当作有效皮肤。
            char extra;
            ssize_t count;
            do { count = read(input.value, &extra, 1); } while (count < 0 && errno == EINTR);
            if (count != 0) return false;
        }
        errno = 0;
    }
    return errno == 0;
}
} // namespace

bool copySkinFolderByFd(int source, int destination) {
    struct stat sourceStat {}, destinationStat {};
    if (fstat(source, &sourceStat) || !S_ISDIR(sourceStat.st_mode)
            || fstat(destination, &destinationStat) || !S_ISDIR(destinationStat.st_mode)) return false;
    Budget budget;
    return tree(source, destination, 0, budget, Hook{});
}
