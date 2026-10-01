#include "DictionarySessionLease.h"
#include <sys/file.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <cerrno>
#include <filesystem>
#include <stdexcept>

namespace metasequoia::apple
{
namespace
{
int Lock(int fd, int flags)
{
    int result;
    do
    {
        result = flock(fd, flags);
    } while (result < 0 && errno == EINTR);
    return result;
}

bool SafeDirectoryPath(NSURL *user)
{
    if (!user || !user.isFileURL)
        return false;
    const std::filesystem::path path(user.fileSystemRepresentation);
    if (!path.is_absolute())
        return false;
    auto current = path.root_path();
    bool sawPrefixAlias = false;
    bool sawRealComponent = false;
    for (const auto &component : path.relative_path())
    {
        current /= component;
        struct stat info = {};
        if (lstat(current.c_str(), &info) != 0)
        {
            if (errno == ENOENT)
                return true;
            return false;
        }
        if (S_ISLNK(info.st_mode))
        {
            const bool systemAlias = !sawRealComponent && !sawPrefixAlias &&
                                     (current == "/tmp" || current == "/var");
            if (!systemAlias)
                return false;
            sawPrefixAlias = true;
        }
        else if (!S_ISDIR(info.st_mode))
            return false;
        else
            sawRealComponent = true;
    }
    return true;
}
} // namespace
DictionarySessionLease::DictionarySessionLease(NSURL *user)
{
    if (!SafeDirectoryPath(user))
        throw std::runtime_error("Cannot use a symbolic-link dictionary session directory");
    if (![NSFileManager.defaultManager createDirectoryAtURL:user
                                withIntermediateDirectories:YES
                                                 attributes:@{
                                                     NSFilePosixPermissions : @0700
                                                 }
                                                      error:nil])
        throw std::runtime_error("Cannot create dictionary session directory");
    sessions_ = open([user URLByAppendingPathComponent:@"dictionary-sessions.lock"].fileSystemRepresentation,
                     O_CREAT | O_RDWR | O_CLOEXEC | O_NOFOLLOW, 0600);
    gate_ = open([user URLByAppendingPathComponent:@"dictionary-publication.lock"].fileSystemRepresentation,
                 O_CREAT | O_RDWR | O_CLOEXEC | O_NOFOLLOW, 0600);
    if (sessions_ < 0 || gate_ < 0 || Lock(sessions_, LOCK_SH) != 0)
    {
        if (sessions_ >= 0)
            close(sessions_);
        if (gate_ >= 0)
            close(gate_);
        throw std::runtime_error("Cannot acquire dictionary session lease");
    }
}
DictionarySessionLease::~DictionarySessionLease()
{
    Lock(sessions_, LOCK_UN);
    close(sessions_);
    close(gate_);
}
bool DictionarySessionLease::exclusively(const std::function<void()> &operation)
{
    if (Lock(gate_, LOCK_EX | LOCK_NB) != 0)
        return false;
    // Keep the publisher gate until the shared lease has been restored. Another
    // publisher cannot acquire exclusivity in the conversion's unlocked window.
    Lock(sessions_, LOCK_UN);
    if (Lock(sessions_, LOCK_EX | LOCK_NB) != 0)
    {
        Lock(sessions_, LOCK_SH);
        Lock(gate_, LOCK_UN);
        return false;
    }
    try
    {
        operation();
    }
    catch (...)
    {
        Lock(sessions_, LOCK_SH);
        Lock(gate_, LOCK_UN);
        throw;
    }
    Lock(sessions_, LOCK_SH);
    Lock(gate_, LOCK_UN);
    return true;
}
} // namespace metasequoia::apple
